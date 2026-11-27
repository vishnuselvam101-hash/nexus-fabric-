//! NFJ-1 S1-L1: job data for a whole order (many scanned PDFs -> pages -> parts -> values).
//!
//! An order is one job. The project stays the access boundary: every route authenticates first, then
//! `project_guard`, and the write routes need the `admin` or `engineer` role. Uploaded files go through
//! the same membrane, encrypted blob store, record and snapshot path as a document upload.
//!
//! Design points that come from the decisions log and the zumen-kirizu study (ZK-STUDY section 10):
//! - A value keeps where it came from (`source`), the text it was read from, its box on the page and the
//!   value it had before (`was`). A value read from a page must contain its own number in its text (flaw 8).
//! - A decision (a confirmation, an accepted flag, a "checked" mark) is a dated per-user record. It is tied
//!   to the value it was made against: when that value changes, the decision is shown as stale and no
//!   longer counts. A decision is never deleted; it can be undone (flaws 1 and 2). Nothing here approves or
//!   releases a result: "mark as checked" is a record of a person's check.
//! - A page that failed to read can be skipped (and the skip undone), so it does not block the order (flaw 9).
//! - Every edit is audited with the old and the new value.
//!
//! SEAM (nf-raster, a later lane): `raster_seam::inspect` decides how many pages a file has and whether a
//! page already has an image. It is a byte-level stand-in. The lane that adds the `nf-raster` child program
//! replaces that one function (and renders the images that `GET .../pages/{n}.png` serves); nothing else here
//! changes.

use super::*;

/// Implementation limits, stated to the user by the UI.
pub(crate) const MAX_ORDER_FILES: i64 = 100;
pub(crate) const MAX_ORDER_PAGES: i64 = 300;
const MAX_ORDER_PARTS: usize = 500;
const MAX_VALUE_BATCH: usize = 500;
const MAX_NAME_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = 500;
const MAX_RULES_BYTES: usize = 8 * 1024;

const FILE_TYPES: [&str; 4] = ["pdf", "png", "jpeg", "tiff"];
const VALUE_SOURCES: [&str; 11] = [
    "read", "drawing", "table", "note", "count", "inferred", "standard", "derived", "fixed",
    "auto", "entered",
];
/// Sources that claim "this number is written on the page": their text must contain the number.
const READ_SOURCES: [&str; 5] = ["read", "drawing", "table", "note", "count"];
const DECISION_SOURCES: [&str; 3] = ["person", "auto", "rule"];
/// Keys that must be larger than zero (lengths, diameters, thicknesses). Other keys (angles, offsets,
/// counts) only have to be finite and within a sane range.
const POSITIVE_KEYS: [&str; 26] = [
    "D",
    "D1",
    "D2",
    "D_end",
    "L",
    "t",
    "R",
    "OD",
    "ID",
    "main_D",
    "main_t",
    "branch_D",
    "branch_t",
    "H",
    "H2",
    "plate_W",
    "plate_H",
    "rib_b",
    "rib_h",
    "pad_D",
    "pad_B",
    "hole_d",
    "cone_s",
    "start_straight",
    "end_straight",
    "joints",
];
const MAX_ABS_VALUE: f64 = 1.0e7;

pub(crate) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route(
            "/api/projects/{id}/orders",
            get(list_orders).post(create_order),
        )
        .route("/api/orders/{oid}", get(get_order))
        .route("/api/orders/{oid}/files", post(upload_file))
        .route("/api/orders/{oid}/pages/{n}", get(get_page_image))
        .route("/api/orders/{oid}/pages/{n}/read", post(read_page))
        .route("/api/orders/{oid}/pages/{n}/skip", post(skip_page))
        .route("/api/orders/{oid}/parts", axum::routing::put(put_parts))
        .route("/api/orders/{oid}/values", axum::routing::put(put_values))
        .route("/api/orders/{oid}/confirm", post(confirm))
        .route("/api/orders/{oid}/decisions", post(post_decision))
        .route(
            "/api/orders/{oid}/decisions/{did}/undo",
            post(undo_decision),
        )
}

// ---------------------------------------------------------------- the nf-raster seam

mod raster_seam {
    /// What the page list needs to know about one page when a file is added.
    pub(super) struct PageSeed {
        pub kind: &'static str,
        pub width: i64,
        pub height: i64,
        /// The uploaded file is itself the page image (a PNG): nothing has to be rendered for it.
        pub image_is_file: bool,
    }

    /// Stand-in for `nf-raster info`. A PNG is one page and its own image. A PDF is counted by a byte scan
    /// for page objects (a PDF whose page tree is packed in compressed object streams counts as one page).
    /// Other images are one page without an image yet. The server still does not parse PDF.
    pub(super) fn inspect(detected: &str, bytes: &[u8]) -> Vec<PageSeed> {
        match detected {
            "png" => {
                let (width, height) = png_size(bytes).unwrap_or((0, 0));
                vec![PageSeed {
                    kind: "scan",
                    width,
                    height,
                    image_is_file: width > 0 && height > 0,
                }]
            }
            "pdf" => (0..pdf_page_count(bytes))
                .map(|_| PageSeed {
                    kind: "other",
                    width: 0,
                    height: 0,
                    image_is_file: false,
                })
                .collect(),
            _ => vec![PageSeed {
                kind: "scan",
                width: 0,
                height: 0,
                image_is_file: false,
            }],
        }
    }

    fn png_size(b: &[u8]) -> Option<(i64, i64)> {
        if b.len() < 24 || &b[12..16] != b"IHDR" {
            return None;
        }
        let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
        (w > 0 && h > 0 && w <= 100_000 && h <= 100_000).then_some((i64::from(w), i64::from(h)))
    }

    /// Occurrences of `/Type /Page` that are not `/Type /Pages`; at least 1, at most 1000.
    fn pdf_page_count(b: &[u8]) -> usize {
        let mut n = 0usize;
        let mut i = 0usize;
        while i + 5 <= b.len() {
            if &b[i..i + 5] == b"/Type" {
                let mut j = i + 5;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if b[j..].starts_with(b"/Page") && b.get(j + 5) != Some(&b's') {
                    n += 1;
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        }
        n.clamp(1, 1000)
    }
}

// ---------------------------------------------------------------- small helpers

fn refuse(status: StatusCode, code: &str, msg: &str) -> ApiError {
    fail_code(status, code, &[], msg)
}

fn unprocessable(code: &str, msg: &str) -> ApiError {
    refuse(StatusCode::UNPROCESSABLE_ENTITY, code, msg)
}

fn fmt_val(v: Option<f64>) -> String {
    v.map_or_else(|| "none".to_string(), |x| x.to_string())
}

/// Fingerprint of the value a decision is made against: the key, the number and the text it was read from.
fn value_fp(key: &str, value: Option<f64>, text: &str) -> String {
    nf_core::sha256_hex(json!([key, value, text]).to_string().as_bytes())
}

struct OrderHead {
    project: i64,
    name: String,
    state: String,
    rules: String,
    created_by: String,
    created_at: i64,
}

fn load_order(db: &Connection, oid: i64) -> Result<OrderHead, ApiError> {
    db.query_row(
        "SELECT project_id, name, state, rules_json, created_by, created_at FROM orders WHERE id = ?",
        params![oid],
        |r| {
            Ok(OrderHead {
                project: r.get(0)?,
                name: r.get(1)?,
                state: r.get(2)?,
                rules: r.get(3)?,
                created_by: r.get(4)?,
                created_at: r.get(5)?,
            })
        },
    )
    .optional()?
    .ok_or_else(not_found)
}

/// The order exists and its project is one the user may use; otherwise 404 either way.
fn guard_order(
    app: &App,
    db: &Connection,
    u: &User,
    oid: i64,
    what: &str,
) -> Result<OrderHead, ApiError> {
    let head = load_order(db, oid)?;
    project_guard(app, db, u, head.project, what)?;
    Ok(head)
}

/// Authenticate, require a writing role and the project, all before a request body is read.
fn write_access(app: &App, headers: &HeaderMap, oid: i64, what: &str) -> Result<User, ApiError> {
    let db = app.db();
    let u = auth_db(&db, app, headers)?;
    require(&u, &["admin", "engineer"])?;
    guard_order(app, &db, &u, oid, what)?;
    Ok(u)
}

fn text_field(v: &Value, key: &str, max_chars: usize) -> Result<Option<String>, ApiError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            if s.chars().count() > max_chars {
                return Err(unprocessable(
                    "field_too_long",
                    &format!("'{key}' is longer than {max_chars} characters"),
                ));
            }
            Ok(Some(s.clone()))
        }
        Some(_) => Err(unprocessable(
            "field_type",
            &format!("'{key}' must be text"),
        )),
    }
}

fn plain_id(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn valid_key(s: &str) -> bool {
    s.len() <= 32
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Full-width digits and separators, as written in Japanese drawings, read as ASCII.
fn ascii_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32('0' as u32 + (c as u32 - '０' as u32)).unwrap_or(c),
            '．' => '.',
            '，' => ',',
            c => c,
        })
        .collect()
}

/// True if some number written in `text` equals `value` (so a read value really is on the page).
fn text_contains_number(text: &str, value: f64) -> bool {
    let t = ascii_digits(text);
    let close = |a: f64| (a - value.abs()).abs() <= 1e-9 * value.abs().max(1.0);
    t.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .filter(|tok| tok.chars().any(|c| c.is_ascii_digit()))
        .any(|tok| {
            let tok = tok.trim_matches(|c| c == '.' || c == ',');
            [tok.replace(',', ""), tok.replace(',', ".")]
                .iter()
                .any(|s| s.parse::<f64>().is_ok_and(close))
        })
}

fn parse_bbox(v: Option<&Value>) -> Result<Option<String>, ApiError> {
    let bad = || {
        unprocessable(
            "bbox_invalid",
            "'bbox' must be [x0, y0, x1, y1] with x1 > x0 and y1 > y0",
        )
    };
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() == 4 => {
            let n: Vec<f64> = a
                .iter()
                .map(|x| {
                    x.as_f64()
                        .filter(|f| f.is_finite() && (0.0..=1.0e6).contains(f))
                })
                .collect::<Option<_>>()
                .ok_or_else(bad)?;
            if n[2] <= n[0] || n[3] <= n[1] {
                return Err(bad());
            }
            Ok(Some(json!(n).to_string()))
        }
        Some(_) => Err(bad()),
    }
}

fn recompute_part_state(db: &Connection, part: i64) -> rusqlite::Result<()> {
    let state: String = db.query_row(
        "SELECT state FROM order_parts WHERE id = ?",
        params![part],
        |r| r.get(0),
    )?;
    if state == "EXCLUDED" {
        return Ok(());
    }
    let (n, open): (i64, i64) = db.query_row(
        "SELECT COUNT(*), COALESCE(SUM(state <> 'CONFIRMED'), 0) FROM order_values WHERE part_id = ?",
        params![part],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let next = if n > 0 && open == 0 {
        "CONFIRMED"
    } else {
        "DRAFT"
    };
    if next != state {
        db.execute(
            "UPDATE order_parts SET state = ? WHERE id = ?",
            params![next, part],
        )?;
    }
    Ok(())
}

/// Everything a "checked" mark is made against: the parts, their values and the skipped pages. Any change
/// to one of them makes an earlier mark stale.
fn order_fingerprint(db: &Connection, oid: i64) -> rusqlite::Result<String> {
    let parts = q_all(
        db,
        "SELECT id, no, shape, qty, COALESCE(attach_part,''), COALESCE(group_key,'') FROM order_parts WHERE order_id = ? AND state <> 'EXCLUDED' ORDER BY no",
        params![oid],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                json!([
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?
                ]),
            ))
        },
    )?;
    let mut all = Vec::new();
    for (id, head) in parts {
        let vals = q_all(
            db,
            "SELECT key, value, text FROM order_values WHERE part_id = ? ORDER BY key",
            params![id],
            |r| {
                Ok(value_fp(
                    &r.get::<_, String>(0)?,
                    r.get::<_, Option<f64>>(1)?,
                    &r.get::<_, String>(2)?,
                ))
            },
        )?;
        all.push(json!([head, vals]));
    }
    let skipped = q_all(
        db,
        "SELECT page_no FROM order_pages WHERE order_id = ? AND skipped = 1 ORDER BY page_no",
        params![oid],
        |r| r.get::<_, i64>(0),
    )?;
    Ok(nf_core::sha256_hex(
        json!([all, skipped]).to_string().as_bytes(),
    ))
}

// ---------------------------------------------------------------- create and list

async fn create_order(
    State(app): Ctx,
    headers: HeaderMap,
    Path(project): Path<i64>,
    raw: Bytes,
) -> Api {
    let u = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        project_guard(&app, &db, &u, project, "create order")?;
        u
    };
    let body = request_json(&headers, &raw)?;
    let name = text_field(&body, "name", MAX_NAME_CHARS)?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            unprocessable("order_name", "the order needs a name (1 to 200 characters)")
        })?;
    let rules = match body.get("rules") {
        None | Some(Value::Null) => "{}".to_string(),
        Some(v @ Value::Object(_)) => {
            let s = v.to_string();
            if s.len() > MAX_RULES_BYTES {
                return Err(unprocessable(
                    "rules_too_large",
                    "'rules' is larger than 8 KiB",
                ));
            }
            s
        }
        Some(_) => return Err(unprocessable("rules_type", "'rules' must be an object")),
    };
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    project_guard(&app, &tx, &u, project, "create order")?;
    tx.execute(
        "INSERT INTO orders(project_id, name, state, rules_json, created_by, created_at) VALUES (?,?,?,?,?,?)",
        params![project, name, "OPEN", rules, u.username, now() as i64],
    )?;
    let oid = tx.last_insert_rowid();
    app.audit(
        &tx,
        &u.username,
        "order_created",
        &format!("project #{project} order #{oid} '{name}'"),
    )?;
    tx.commit()?;
    ok(json!({ "id": oid, "project_id": project, "name": name, "state": "OPEN" }))
}

async fn list_orders(State(app): Ctx, headers: HeaderMap, Path(project): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    project_guard(&app, &db, &u, project, "list orders")?;
    let rows = q_all(
        &db,
        "SELECT o.id, o.name, o.state, o.created_by, o.created_at, (SELECT COUNT(*) FROM order_files f WHERE f.order_id = o.id), (SELECT COUNT(*) FROM order_pages p WHERE p.order_id = o.id), (SELECT COUNT(*) FROM order_parts q WHERE q.order_id = o.id AND q.state <> 'EXCLUDED') FROM orders o WHERE o.project_id = ? ORDER BY o.id DESC LIMIT 200",
        params![project],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "state": r.get::<_, String>(2)?,
                       "created_by": r.get::<_, String>(3)?, "created_at": r.get::<_, i64>(4)?,
                       "files": r.get::<_, i64>(5)?, "pages": r.get::<_, i64>(6)?, "parts": r.get::<_, i64>(7)? }),
            )
        },
    )?;
    ok(json!({ "orders": rows }))
}

// ---------------------------------------------------------------- files and pages

async fn upload_file(
    State(app): Ctx,
    headers: HeaderMap,
    Path(oid): Path<i64>,
    body: axum::body::Body,
) -> Api {
    let u = write_access(&app, &headers, oid, "order file")?;
    let (bytes, _slot) = read_upload_body(&app, &headers, body).await?;
    let filename = upload_filename(&headers);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let head = guard_order(&app, &tx, &u, oid, "order file")?;
    let detected = match membrane::admit(&bytes) {
        membrane::Admission::Accepted { detected_type } => detected_type,
        membrane::Admission::Rejected { reason } => {
            app.audit(
                &tx,
                &u.username,
                "order_file_rejected",
                &format!("order #{oid} '{filename}': {reason}"),
            )?;
            tx.commit()?;
            return Err(unprocessable(
                "membrane_rejected",
                &format!("file rejected at the membrane: {reason}"),
            ));
        }
    };
    if !FILE_TYPES.contains(&detected) {
        return Err(unprocessable(
            "order_file_type",
            &format!(
                "an order takes scanned or drawn pages (PDF, PNG, JPEG, TIFF); this file is {detected}"
            ),
        ));
    }
    let seeds = raster_seam::inspect(detected, &bytes);
    let (files, pages): (i64, i64) = tx.query_row(
        "SELECT (SELECT COUNT(*) FROM order_files WHERE order_id = ?1), (SELECT COUNT(*) FROM order_pages WHERE order_id = ?1)",
        params![oid],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if files >= MAX_ORDER_FILES || pages + seeds.len() as i64 > MAX_ORDER_PAGES {
        return Err(refuse(
            StatusCode::CONFLICT,
            "order_limit",
            &format!(
                "an order holds at most {MAX_ORDER_FILES} files and {MAX_ORDER_PAGES} pages; this one has {files} files and {pages} pages and the new file adds {} page(s)",
                seeds.len()
            ),
        ));
    }
    let doc = register_document(&app, &tx, &u, head.project, &filename, &bytes, detected)?;
    tx.execute(
        "INSERT INTO order_files(order_id, document_id, ord) VALUES (?, ?, ?)",
        params![oid, doc.id, files + 1],
    )?;
    let file_id = tx.last_insert_rowid();
    let mut numbers = Vec::new();
    for (i, seed) in seeds.iter().enumerate() {
        let page_no = pages + 1 + i as i64;
        let png = seed.image_is_file.then(|| doc.sha.clone());
        tx.execute(
            "INSERT INTO order_pages(order_id, file_id, page_no, file_page, png_sha256, width, height, kind, read_state) VALUES (?,?,?,?,?,?,?,?, 'none')",
            params![oid, file_id, page_no, i as i64 + 1, png, seed.width, seed.height, seed.kind],
        )?;
        numbers.push(page_no);
    }
    app.audit(
        &tx,
        &u.username,
        "order_file_added",
        &format!(
            "order #{oid} file #{file_id} '{filename}' {detected} {} pages {}-{}",
            doc.sha,
            numbers.first().copied().unwrap_or(0),
            numbers.last().copied().unwrap_or(0)
        ),
    )?;
    tx.commit()?;
    ok(
        json!({ "file_id": file_id, "document_id": doc.id, "sha256": doc.sha, "detected_type": detected,
               "extension_mismatch": doc.mismatch, "pages": numbers, "page_count": numbers.len() }),
    )
}

async fn get_page_image(
    State(app): Ctx,
    headers: HeaderMap,
    Path((oid, file)): Path<(i64, String)>,
) -> Api {
    let png: Option<String> = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        guard_order(&app, &db, &u, oid, "page image")?;
        let n = file
            .strip_suffix(".png")
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(not_found)?;
        db.query_row(
            "SELECT png_sha256 FROM order_pages WHERE order_id = ? AND page_no = ?",
            params![oid, n],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .ok_or_else(not_found)?
    };
    let Some(sha) = png else {
        return Err(refuse(
            StatusCode::NOT_FOUND,
            "page_image_pending",
            "the image of this page has not been rendered yet",
        ));
    };
    let bytes = storage::load_blob(&app.data_dir, &app.key, &sha).map_err(|e| {
        elog!("nf-server: page image {sha} cannot be read: {e}");
        fail(
            StatusCode::CONFLICT,
            "the stored page image failed its integrity check and is withheld",
        )
    })?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "private, no-store"),
        ],
        bytes,
    )
        .into_response())
}

async fn read_page(State(app): Ctx, headers: HeaderMap, Path((oid, n)): Path<(i64, i64)>) -> Api {
    write_access(&app, &headers, oid, "read page")?;
    let exists = {
        let db = app.db();
        q_count(
            &db,
            "SELECT COUNT(*) FROM order_pages WHERE order_id = ? AND page_no = ?",
            params![oid, n],
        )? == 1
    };
    if !exists {
        return Err(not_found());
    }
    Err(fail_json(
        StatusCode::NOT_IMPLEMENTED,
        json!({ "error": "the page reader is not installed in this build", "error_ja": "このビルドにはページ読み取り機能が入っていません", "code": "reader_not_installed" }),
    ))
}

async fn skip_page(
    State(app): Ctx,
    headers: HeaderMap,
    Path((oid, n)): Path<(i64, i64)>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "skip page")?;
    let body = if raw.is_empty() {
        json!({})
    } else {
        request_json(&headers, &raw)?
    };
    let skip = match body.get("skipped") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(unprocessable(
                "field_type",
                "'skipped' must be true or false",
            ));
        }
    };
    let note = text_field(&body, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    guard_order(&app, &tx, &u, oid, "skip page")?;
    let (state, was): (String, bool) = tx
        .query_row(
            "SELECT read_state, skipped FROM order_pages WHERE order_id = ? AND page_no = ?",
            params![oid, n],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(not_found)?;
    if skip && !was && !matches!(state.as_str(), "failed" | "none") {
        return Err(refuse(
            StatusCode::CONFLICT,
            "page_not_skippable",
            &format!(
                "only a page that failed to read (or has not been read) can be skipped; this page is '{state}'"
            ),
        ));
    }
    if skip != was {
        tx.execute(
            "UPDATE order_pages SET skipped = ?, skipped_by = ?, skipped_at = ? WHERE order_id = ? AND page_no = ?",
            params![skip, u.username, now() as i64, oid, n],
        )?;
        app.audit(
            &tx,
            &u.username,
            if skip {
                "order_page_skipped"
            } else {
                "order_page_unskipped"
            },
            &format!(
                "order #{oid} page {n} (read state {state}) {}",
                clip(&note, 200)
            ),
        )?;
    }
    tx.commit()?;
    ok(json!({ "page_no": n, "skipped": skip, "read_state": state }))
}

// ---------------------------------------------------------------- parts

struct PartIn {
    id: Option<i64>,
    no: String,
    name: String,
    shape: String,
    qty: i64,
    page: Option<i64>,
    attach: Option<String>,
    group: Option<String>,
    rows: String,
    exclude: bool,
}

fn parse_part(v: &Value) -> Result<PartIn, ApiError> {
    let id = match v.get("id") {
        None | Some(Value::Null) => None,
        Some(x) => Some(
            x.as_i64()
                .ok_or_else(|| unprocessable("field_type", "'id' must be an integer"))?,
        ),
    };
    let no = text_field(v, "no", 40)?.unwrap_or_default();
    if !plain_id(&no, 40) {
        return Err(unprocessable(
            "part_no",
            "each part needs a 'no' of 1 to 40 letters, digits, '_', '-' or '.'",
        ));
    }
    let shape = text_field(v, "shape", 40)?.unwrap_or_default();
    if !valid_key(&shape) {
        return Err(unprocessable(
            "part_shape",
            "'shape' must be a shape key such as cylinder or tee_main",
        ));
    }
    let name = text_field(v, "name", MAX_NAME_CHARS)?.unwrap_or_default();
    let qty = match v.get("qty") {
        None | Some(Value::Null) => 1,
        Some(x) => x
            .as_i64()
            .filter(|q| (1..=9999).contains(q))
            .ok_or_else(|| {
                unprocessable("part_qty", "'qty' must be a whole number from 1 to 9999")
            })?,
    };
    let page = match v.get("page") {
        None | Some(Value::Null) => None,
        Some(x) => Some(
            x.as_i64()
                .filter(|p| *p >= 1)
                .ok_or_else(|| unprocessable("part_page", "'page' must be a page number"))?,
        ),
    };
    let attach = text_field(v, "attach_part", 40)?.filter(|s| !s.is_empty());
    if attach.as_deref().is_some_and(|a| !plain_id(a, 40)) {
        return Err(unprocessable(
            "part_attach",
            "'attach_part' must be the 'no' of another part",
        ));
    }
    let group = text_field(v, "group_key", 64)?.filter(|s| !s.is_empty());
    let rows = match v.get("table_rows") {
        None | Some(Value::Null) => "[]".to_string(),
        Some(Value::Array(a))
            if a.len() <= 50
                && a.iter()
                    .all(|x| x.as_str().is_some_and(|s| s.chars().count() <= 40)) =>
        {
            Value::Array(a.clone()).to_string()
        }
        Some(_) => {
            return Err(unprocessable(
                "part_rows",
                "'table_rows' must be a list of at most 50 short texts",
            ));
        }
    };
    let exclude = match v.get("state").and_then(Value::as_str) {
        None | Some("DRAFT") => false,
        Some("EXCLUDED") => true,
        Some(_) => {
            return Err(unprocessable(
                "part_state",
                "'state' may only be DRAFT or EXCLUDED here; confirmation goes through /confirm",
            ));
        }
    };
    Ok(PartIn {
        id,
        no,
        name,
        shape,
        qty,
        page,
        attach,
        group,
        rows,
        exclude,
    })
}

fn part_json(db: &Connection, part: i64) -> rusqlite::Result<String> {
    db.query_row(
        "SELECT json_object('no', no, 'name', name, 'shape', shape, 'qty', qty, 'page', (SELECT page_no FROM order_pages WHERE id = page_id), 'attach', attach_part, 'group', group_key, 'state', state) FROM order_parts WHERE id = ?",
        params![part],
        |r| r.get(0),
    )
}

/// The attach links of a parts list must name other parts of the list, and must not loop.
fn check_attach(parts: &[PartIn]) -> Result<(), ApiError> {
    let nos: BTreeSet<&str> = parts.iter().map(|p| p.no.as_str()).collect();
    let attach_of: BTreeMap<&str, &str> = parts
        .iter()
        .filter_map(|p| p.attach.as_deref().map(|a| (p.no.as_str(), a)))
        .collect();
    for (no, a) in &attach_of {
        if no == a || !nos.contains(a) {
            return Err(unprocessable(
                "part_attach",
                &format!(
                    "part '{no}' is attached to '{a}', which is not another part of this list"
                ),
            ));
        }
        let (mut cur, mut hops) = (*a, 0usize);
        while let Some(next) = attach_of.get(cur) {
            hops += 1;
            if hops > parts.len() || next == no {
                return Err(unprocessable(
                    "part_attach",
                    &format!("part '{no}' is attached in a loop"),
                ));
            }
            cur = next;
        }
    }
    Ok(())
}

async fn put_parts(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "edit parts")?;
    let body = request_json(&headers, &raw)?;
    let list = body
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| unprocessable("parts_missing", "the body needs a 'parts' list"))?;
    if list.len() > MAX_ORDER_PARTS {
        return Err(unprocessable(
            "parts_limit",
            &format!("an order holds at most {MAX_ORDER_PARTS} parts"),
        ));
    }
    let parts: Vec<PartIn> = list.iter().map(parse_part).collect::<Result<_, _>>()?;
    let (mut seen, mut seen_ids) = (BTreeSet::new(), BTreeSet::new());
    for p in &parts {
        if !seen.insert(p.no.as_str()) {
            return Err(unprocessable(
                "part_no_twice",
                &format!("part no '{}' is listed twice", p.no),
            ));
        }
        if let Some(id) = p.id
            && !seen_ids.insert(id)
        {
            return Err(unprocessable(
                "part_id_twice",
                &format!("part id {id} is listed twice"),
            ));
        }
    }
    check_attach(&parts)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    guard_order(&app, &tx, &u, oid, "edit parts")?;
    let existing: BTreeMap<i64, String> = q_all(
        &tx,
        "SELECT id, no FROM order_parts WHERE order_id = ?",
        params![oid],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )?
    .into_iter()
    .collect();
    for p in &parts {
        match p.id {
            Some(id) if !existing.contains_key(&id) => {
                return Err(unprocessable(
                    "part_unknown",
                    &format!("part id {id} is not part of this order"),
                ));
            }
            None if existing.values().any(|n| *n == p.no) => {
                return Err(refuse(
                    StatusCode::CONFLICT,
                    "part_no_exists",
                    &format!(
                        "part no '{}' exists already; send its id to change it",
                        p.no
                    ),
                ));
            }
            _ => {}
        }
    }
    // the old state of every listed part, then free their numbers so that two parts can swap numbers
    let mut olds: BTreeMap<i64, String> = BTreeMap::new();
    for id in parts.iter().filter_map(|p| p.id) {
        olds.insert(id, part_json(&tx, id)?);
        tx.execute(
            "UPDATE order_parts SET no = ? WHERE id = ?",
            params![format!("~{id}"), id],
        )?;
    }
    let (mut added, mut changed, mut excluded) = (0, 0, 0);
    for p in &parts {
        let page_id: Option<i64> = match p.page {
            None => None,
            Some(n) => Some(
                tx.query_row(
                    "SELECT id FROM order_pages WHERE order_id = ? AND page_no = ?",
                    params![oid, n],
                    |r| r.get(0),
                )
                .optional()?
                .ok_or_else(|| {
                    unprocessable("part_page", &format!("page {n} is not part of this order"))
                })?,
            ),
        };
        match p.id {
            None => {
                tx.execute(
                    "INSERT INTO order_parts(order_id, page_id, no, name, shape, attach_part, qty, table_rows_json, group_key, state) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    params![oid, page_id, p.no, p.name, p.shape, p.attach, p.qty, p.rows, p.group, if p.exclude { "EXCLUDED" } else { "DRAFT" }],
                )?;
                let id = tx.last_insert_rowid();
                added += 1;
                app.audit(
                    &tx,
                    &u.username,
                    "order_part_added",
                    &format!(
                        "order #{oid} part #{id} new={}",
                        clip(&part_json(&tx, id)?, 700)
                    ),
                )?;
            }
            Some(id) => {
                let was_excluded: bool = tx.query_row(
                    "SELECT state = 'EXCLUDED' FROM order_parts WHERE id = ?",
                    params![id],
                    |r| r.get(0),
                )?;
                tx.execute(
                    "UPDATE order_parts SET no = ?, page_id = ?, name = ?, shape = ?, attach_part = ?, qty = ?, table_rows_json = ?, group_key = ? WHERE id = ?",
                    params![p.no, page_id, p.name, p.shape, p.attach, p.qty, p.rows, p.group, id],
                )?;
                if p.exclude {
                    tx.execute(
                        "UPDATE order_parts SET state = 'EXCLUDED' WHERE id = ?",
                        params![id],
                    )?;
                } else if was_excluded {
                    tx.execute(
                        "UPDATE order_parts SET state = 'DRAFT' WHERE id = ?",
                        params![id],
                    )?;
                    recompute_part_state(&tx, id)?;
                }
                let new = part_json(&tx, id)?;
                let old = olds.get(&id).cloned().unwrap_or_default();
                if new != old {
                    changed += 1;
                    app.audit(
                        &tx,
                        &u.username,
                        "order_part_changed",
                        &format!(
                            "order #{oid} part #{id} old={} new={}",
                            clip(&old, 450),
                            clip(&new, 450)
                        ),
                    )?;
                }
            }
        }
    }
    // parts left out of the list are excluded, not deleted: sending them again brings them back
    let listed: BTreeSet<i64> = parts.iter().filter_map(|p| p.id).collect();
    for (id, no) in &existing {
        if listed.contains(id) {
            continue;
        }
        let state: String = tx.query_row(
            "SELECT state FROM order_parts WHERE id = ?",
            params![id],
            |r| r.get(0),
        )?;
        if state != "EXCLUDED" {
            tx.execute(
                "UPDATE order_parts SET state = 'EXCLUDED' WHERE id = ?",
                params![id],
            )?;
            excluded += 1;
            app.audit(
                &tx,
                &u.username,
                "order_part_excluded",
                &format!("order #{oid} part #{id} '{no}' old state {state} new state EXCLUDED"),
            )?;
        }
    }
    tx.commit()?;
    ok(json!({ "added": added, "changed": changed, "excluded": excluded }))
}

// ---------------------------------------------------------------- values

struct ValueIn {
    part: Value,
    key: String,
    value: Option<f64>,
    text: String,
    bbox: Option<String>,
    source: String,
    confirm: bool,
    note: String,
}

fn parse_value(v: &Value) -> Result<ValueIn, ApiError> {
    let part = v
        .get("part")
        .cloned()
        .filter(|p| p.is_i64() || p.is_string())
        .ok_or_else(|| {
            unprocessable("value_part", "'part' must be the id or the 'no' of a part")
        })?;
    let key = text_field(v, "key", 32)?.unwrap_or_default();
    if !valid_key(&key) {
        return Err(unprocessable(
            "value_key",
            "'key' must be a parameter name such as D, t or branch_x",
        ));
    }
    let value = match v.get("value") {
        None => {
            return Err(unprocessable(
                "value_missing_field",
                "'value' is required (a number, or null for a missing value)",
            ));
        }
        Some(Value::Null) => None,
        Some(x) => {
            let f = x
                .as_f64()
                .filter(|f| f.is_finite() && f.abs() <= MAX_ABS_VALUE)
                .ok_or_else(|| {
                    unprocessable(
                        "value_number",
                        &format!("'{key}' must be a finite number within +-{MAX_ABS_VALUE}"),
                    )
                })?;
            if POSITIVE_KEYS.contains(&key.as_str()) && f <= 0.0 {
                return Err(unprocessable(
                    "value_not_positive",
                    &format!("'{key}' must be larger than zero"),
                ));
            }
            Some(f)
        }
    };
    let source = text_field(v, "source", 16)?.unwrap_or_else(|| "entered".to_string());
    if !VALUE_SOURCES.contains(&source.as_str()) {
        return Err(unprocessable(
            "value_source",
            &format!("'source' must be one of {}", VALUE_SOURCES.join(", ")),
        ));
    }
    let text = text_field(v, "text", 200)?.unwrap_or_default();
    if let Some(f) = value
        && READ_SOURCES.contains(&source.as_str())
    {
        if text.trim().is_empty() {
            return Err(unprocessable(
                "value_text_missing",
                &format!("a value from the page ('{source}') needs the text it was read from"),
            ));
        }
        if !text_contains_number(&text, f) {
            return Err(unprocessable(
                "value_text_mismatch",
                &format!(
                    "the text '{}' does not contain the number {f}",
                    clip(&text, 60)
                ),
            ));
        }
    }
    let bbox = parse_bbox(v.get("bbox"))?;
    let confirm = match v.get("confirm") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(unprocessable(
                "field_type",
                "'confirm' must be true or false",
            ));
        }
    };
    if confirm && value.is_none() {
        return Err(unprocessable(
            "confirm_missing",
            "a missing value cannot be confirmed",
        ));
    }
    let note = text_field(v, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    Ok(ValueIn {
        part,
        key,
        value,
        text,
        bbox,
        source,
        confirm,
        note,
    })
}

/// (id, no, state) of the part a request names, inside this order.
fn find_part(db: &Connection, oid: i64, r: &Value) -> Result<(i64, String, String), ApiError> {
    let row = match r {
        Value::String(no) => db.query_row(
            "SELECT id, no, state FROM order_parts WHERE order_id = ? AND no = ?",
            params![oid, no],
            |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)),
        ),
        other => db.query_row(
            "SELECT id, no, state FROM order_parts WHERE order_id = ? AND id = ?",
            params![oid, other.as_i64().unwrap_or(-1)],
            |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)),
        ),
    };
    row.optional()?.ok_or_else(|| {
        unprocessable(
            "part_unknown",
            &format!("part {r} is not part of this order"),
        )
    })
}

struct NewDecision<'a> {
    order: i64,
    part: Option<i64>,
    value: Option<i64>,
    kind: &'a str,
    flag: Option<&'a str>,
    source: &'a str,
    agent: Option<&'a str>,
    reason: &'a str,
    against: String,
    prev_state: Option<&'a str>,
    who: &'a str,
}

fn insert_decision(db: &Connection, d: &NewDecision<'_>) -> rusqlite::Result<i64> {
    db.execute(
        "INSERT INTO order_decisions(order_id, part_id, value_id, kind, flag, source, agent, reason, against_hash, prev_state, decided_by, decided_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        params![d.order, d.part, d.value, d.kind, d.flag, d.source, d.agent, d.reason, d.against, d.prev_state, d.who, now() as i64],
    )?;
    Ok(db.last_insert_rowid())
}

type OldValue = (
    i64,
    Option<f64>,
    String,
    Option<String>,
    String,
    String,
    Option<f64>,
);

async fn put_values(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "edit values")?;
    let body = request_json(&headers, &raw)?;
    let list = body
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| unprocessable("values_missing", "the body needs a 'values' list"))?;
    if list.len() > MAX_VALUE_BATCH {
        return Err(unprocessable(
            "values_limit",
            &format!("at most {MAX_VALUE_BATCH} values per request"),
        ));
    }
    let values: Vec<ValueIn> = list.iter().map(parse_value).collect::<Result<_, _>>()?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    guard_order(&app, &tx, &u, oid, "edit values")?;
    let (mut changed, mut unchanged) = (0, 0);
    let mut touched = BTreeSet::new();
    for v in &values {
        let (pid, pno, pstate) = find_part(&tx, oid, &v.part)?;
        if pstate == "EXCLUDED" {
            return Err(refuse(
                StatusCode::CONFLICT,
                "part_excluded",
                &format!("part '{pno}' is excluded; bring it back before editing its values"),
            ));
        }
        let old: Option<OldValue> = tx
            .query_row(
                "SELECT id, value, text, bbox_json, source, state, proposed_value FROM order_values WHERE part_id = ? AND key = ?",
                params![pid, v.key],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                },
            )
            .optional()?;
        let new_state = if v.value.is_none() {
            "MISSING"
        } else if v.confirm {
            "CONFIRMED"
        } else if old
            .as_ref()
            .and_then(|o| o.6)
            .is_some_and(|pv| Some(pv) != v.value)
        {
            "CORRECTED"
        } else {
            "PROPOSED"
        };
        let (vid, old_desc) = match &old {
            Some((id, val, text, bbox, src, state, _)) => {
                let same =
                    *val == v.value && *text == v.text && *bbox == v.bbox && *src == v.source;
                // identical content: nothing to write (and a resend never drops an earlier confirmation)
                if same && (state == new_state || (!v.confirm && state == "CONFIRMED")) {
                    unchanged += 1;
                    continue;
                }
                tx.execute(
                    "UPDATE order_values SET value = ?, text = ?, bbox_json = ?, source = ?, state = ?, was = ?, edited_by = ?, edited_at = ? WHERE id = ?",
                    params![v.value, v.text, v.bbox, v.source, new_state, val, u.username, now() as i64, id],
                )?;
                (*id, format!("{} [{src} {state}]", fmt_val(*val)))
            }
            None => {
                tx.execute(
                    "INSERT INTO order_values(part_id, key, value, text, bbox_json, source, state, was, proposed_value, proposed_by, edited_by, edited_at) VALUES (?,?,?,?,?,?,?,NULL,?,?,?,?)",
                    params![pid, v.key, v.value, v.text, v.bbox, v.source, new_state, v.value, u.username, u.username, now() as i64],
                )?;
                (tx.last_insert_rowid(), "none".to_string())
            }
        };
        changed += 1;
        touched.insert(pid);
        app.audit(
            &tx,
            &u.username,
            "order_value_set",
            &format!(
                "order #{oid} part '{pno}' {}: old {old_desc} new {} [{} {new_state}] {}",
                v.key,
                fmt_val(v.value),
                v.source,
                clip(&v.note, 200)
            ),
        )?;
        if v.confirm {
            let did = insert_decision(
                &tx,
                &NewDecision {
                    order: oid,
                    part: Some(pid),
                    value: Some(vid),
                    kind: "confirm",
                    flag: None,
                    source: "person",
                    agent: None,
                    reason: &v.note,
                    against: value_fp(&v.key, v.value, &v.text),
                    prev_state: old.as_ref().map(|o| o.5.as_str()),
                    who: &u.username,
                },
            )?;
            app.audit(
                &tx,
                &u.username,
                "order_decision",
                &format!(
                    "order #{oid} decision #{did} confirm part '{pno}' {} = {} by person",
                    v.key,
                    fmt_val(v.value)
                ),
            )?;
        }
    }
    for pid in touched {
        recompute_part_state(&tx, pid)?;
    }
    tx.commit()?;
    ok(json!({ "changed": changed, "unchanged": unchanged }))
}

// ---------------------------------------------------------------- decisions

async fn confirm(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "confirm values")?;
    let body = request_json(&headers, &raw)?;
    let all = body.get("all").and_then(Value::as_bool).unwrap_or(false);
    let ids: Vec<i64> = match body.get("part_ids") {
        None | Some(Value::Null) => vec![],
        Some(Value::Array(a)) if a.len() <= MAX_ORDER_PARTS => a
            .iter()
            .map(Value::as_i64)
            .collect::<Option<_>>()
            .ok_or_else(|| unprocessable("field_type", "'part_ids' must be a list of part ids"))?,
        Some(_) => {
            return Err(unprocessable(
                "field_type",
                "'part_ids' must be a list of part ids",
            ));
        }
    };
    if !all && ids.is_empty() {
        return Err(unprocessable(
            "confirm_target",
            "send {\"part_ids\": [...]} or {\"all\": true}",
        ));
    }
    let note = text_field(&body, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    guard_order(&app, &tx, &u, oid, "confirm values")?;
    let parts: Vec<(i64, String)> = if all {
        q_all(
            &tx,
            "SELECT id, no FROM order_parts WHERE order_id = ? AND state <> 'EXCLUDED' ORDER BY id",
            params![oid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
    } else {
        let mut v = Vec::new();
        for id in &ids {
            let (pid, no, st) = find_part(&tx, oid, &json!(id))?;
            if st != "EXCLUDED" {
                v.push((pid, no));
            }
        }
        v
    };
    let (mut confirmed, mut missing) = (0, 0);
    let mut decisions = Vec::new();
    for (pid, pno) in &parts {
        let rows = q_all(
            &tx,
            "SELECT id, key, value, text, state FROM order_values WHERE part_id = ? ORDER BY key",
            params![pid],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )?;
        for (vid, key, val, text, state) in rows {
            if val.is_none() {
                missing += 1;
                continue;
            }
            if state == "CONFIRMED" {
                continue;
            }
            tx.execute(
                "UPDATE order_values SET state = 'CONFIRMED', edited_by = ?, edited_at = ? WHERE id = ?",
                params![u.username, now() as i64, vid],
            )?;
            let did = insert_decision(
                &tx,
                &NewDecision {
                    order: oid,
                    part: Some(*pid),
                    value: Some(vid),
                    kind: "confirm",
                    flag: None,
                    source: "person",
                    agent: None,
                    reason: &note,
                    against: value_fp(&key, val, &text),
                    prev_state: Some(&state),
                    who: &u.username,
                },
            )?;
            app.audit(
                &tx,
                &u.username,
                "order_decision",
                &format!(
                    "order #{oid} decision #{did} confirm part '{pno}' {key} = {} (was {state}) by person",
                    fmt_val(val)
                ),
            )?;
            decisions.push(did);
            confirmed += 1;
        }
        recompute_part_state(&tx, *pid)?;
    }
    tx.commit()?;
    ok(json!({ "confirmed": confirmed, "missing": missing, "decisions": decisions }))
}

async fn post_decision(
    State(app): Ctx,
    headers: HeaderMap,
    Path(oid): Path<i64>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "record decision")?;
    let body = request_json(&headers, &raw)?;
    let kind = text_field(&body, "kind", 16)?.unwrap_or_default();
    if !["confirm", "accept_flag", "mark_checked"].contains(&kind.as_str()) {
        return Err(unprocessable(
            "decision_kind",
            "'kind' must be confirm, accept_flag or mark_checked",
        ));
    }
    let source = text_field(&body, "source", 8)?.unwrap_or_else(|| "person".to_string());
    if !DECISION_SOURCES.contains(&source.as_str()) {
        return Err(unprocessable(
            "decision_source",
            "'source' must be person, auto or rule",
        ));
    }
    let reason = text_field(&body, "reason", MAX_NOTE_CHARS)?.unwrap_or_default();
    if source != "person" && reason.trim().is_empty() {
        return Err(unprocessable(
            "decision_reason",
            "a decision made by automation must give its reason",
        ));
    }
    if kind == "mark_checked" && source != "person" {
        return Err(unprocessable(
            "decision_source",
            "only a person can mark an order as checked",
        ));
    }
    let agent = text_field(&body, "agent", 64)?.filter(|s| !s.is_empty());
    let flag = text_field(&body, "flag", 64)?.filter(|s| !s.is_empty());
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    guard_order(&app, &tx, &u, oid, "record decision")?;
    let mut d = NewDecision {
        order: oid,
        part: None,
        value: None,
        kind: &kind,
        flag: None,
        source: &source,
        agent: agent.as_deref(),
        reason: &reason,
        against: String::new(),
        prev_state: None,
        who: &u.username,
    };
    let prev_state: String;
    if kind == "mark_checked" {
        d.against = order_fingerprint(&tx, oid)?;
    } else {
        let part = body
            .get("part")
            .ok_or_else(|| unprocessable("value_part", "'part' is required"))?;
        let key = text_field(&body, "key", 32)?.unwrap_or_default();
        let (pid, pno, pstate) = find_part(&tx, oid, part)?;
        if pstate == "EXCLUDED" {
            return Err(refuse(
                StatusCode::CONFLICT,
                "part_excluded",
                &format!("part '{pno}' is excluded"),
            ));
        }
        let (vid, val, text, state): (i64, Option<f64>, String, String) = tx
            .query_row(
                "SELECT id, value, text, state FROM order_values WHERE part_id = ? AND key = ?",
                params![pid, key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| {
                unprocessable(
                    "value_unknown",
                    &format!("part '{pno}' has no value '{key}'"),
                )
            })?;
        if val.is_none() {
            return Err(unprocessable(
                "value_missing",
                "a missing value cannot be decided; enter it first",
            ));
        }
        d.part = Some(pid);
        d.value = Some(vid);
        d.against = value_fp(&key, val, &text);
        if kind == "accept_flag" {
            if !flag.as_deref().is_some_and(valid_key) {
                return Err(unprocessable(
                    "decision_flag",
                    "'flag' must name the flag that is accepted, e.g. bom_mismatch",
                ));
            }
            if reason.trim().is_empty() {
                return Err(unprocessable(
                    "decision_reason",
                    "accepting a flag needs a reason",
                ));
            }
            d.flag = flag.as_deref();
        } else {
            tx.execute(
                "UPDATE order_values SET state = 'CONFIRMED', edited_by = ?, edited_at = ? WHERE id = ?",
                params![u.username, now() as i64, vid],
            )?;
            prev_state = state;
            d.prev_state = Some(&prev_state);
        }
    }
    let did = insert_decision(&tx, &d)?;
    if let Some(pid) = d.part {
        recompute_part_state(&tx, pid)?;
    }
    app.audit(
        &tx,
        &u.username,
        "order_decision",
        &format!(
            "order #{oid} decision #{did} {kind} by {source}{} {}",
            agent
                .as_deref()
                .map(|a| format!(" ({a})"))
                .unwrap_or_default(),
            clip(&reason, 300)
        ),
    )?;
    tx.commit()?;
    let v = decision_view(&db, oid, Some(did))?;
    ok(v.into_iter().next().unwrap_or(Value::Null))
}

async fn undo_decision(
    State(app): Ctx,
    headers: HeaderMap,
    Path((oid, did)): Path<(i64, i64)>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "undo decision")?;
    let body = if raw.is_empty() {
        json!({})
    } else {
        request_json(&headers, &raw)?
    };
    let note = text_field(&body, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    guard_order(&app, &tx, &u, oid, "undo decision")?;
    type Row = (
        String,
        Option<i64>,
        Option<i64>,
        String,
        Option<String>,
        bool,
    );
    let row: Option<Row> = tx
        .query_row(
            "SELECT kind, part_id, value_id, against_hash, prev_state, undone_at IS NOT NULL FROM order_decisions WHERE id = ? AND order_id = ?",
            params![did, oid],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((kind, part, value, against, prev, undone)) = row else {
        return Err(not_found());
    };
    if undone {
        return Err(refuse(
            StatusCode::CONFLICT,
            "already_undone",
            "this decision was undone already",
        ));
    }
    tx.execute(
        "UPDATE order_decisions SET undone_by = ?, undone_at = ?, undo_note = ? WHERE id = ?",
        params![u.username, now() as i64, note, did],
    )?;
    let mut reverted = false;
    if kind == "confirm"
        && let (Some(vid), Some(pid), Some(prev)) = (value, part, prev)
    {
        let cur: Option<(String, Option<f64>, String, String)> = tx
            .query_row(
                "SELECT key, value, text, state FROM order_values WHERE id = ?",
                params![vid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        // the value goes back to its earlier state only while it is still the value that was confirmed
        if let Some((key, val, text, state)) = cur
            && state == "CONFIRMED"
            && value_fp(&key, val, &text) == against
        {
            tx.execute(
                "UPDATE order_values SET state = ?, edited_by = ?, edited_at = ? WHERE id = ?",
                params![prev, u.username, now() as i64, vid],
            )?;
            recompute_part_state(&tx, pid)?;
            reverted = true;
        }
    }
    app.audit(
        &tx,
        &u.username,
        "order_decision_undone",
        &format!(
            "order #{oid} decision #{did} {kind} undone, value reverted {reverted} {}",
            clip(&note, 300)
        ),
    )?;
    tx.commit()?;
    let v = decision_view(&db, oid, Some(did))?;
    ok(v.into_iter().next().unwrap_or(Value::Null))
}

/// The decisions of an order (or one of them) with whether they still count: a decision is stale when the
/// value (or, for a "checked" mark, the order) is no longer what it was made against.
fn decision_view(db: &Connection, oid: i64, only: Option<i64>) -> Result<Vec<Value>, ApiError> {
    let order_fp = order_fingerprint(db, oid)?;
    let rows = q_all(
        db,
        "SELECT d.id, d.part_id, p.no, d.value_id, v.key, v.value, v.text, d.kind, d.flag, d.source, d.agent, d.reason, d.against_hash, d.decided_by, d.decided_at, d.undone_by, d.undone_at, d.undo_note
         FROM order_decisions d LEFT JOIN order_parts p ON p.id = d.part_id LEFT JOIN order_values v ON v.id = d.value_id
         WHERE d.order_id = ?1 AND (?2 IS NULL OR d.id = ?2) ORDER BY d.id",
        params![oid, only],
        |r| {
            let key: Option<String> = r.get(4)?;
            let (value, text): (Option<f64>, Option<String>) = (r.get(5)?, r.get(6)?);
            let against: String = r.get(12)?;
            let current = match (&key, &text) {
                (Some(k), Some(t)) => value_fp(k, value, t),
                _ => order_fp.clone(),
            };
            let undone = r.get::<_, Option<i64>>(16)?.is_some();
            let stale = current != against;
            Ok(json!({
                "id": r.get::<_, i64>(0)?, "part_id": r.get::<_, Option<i64>>(1)?, "part": r.get::<_, Option<String>>(2)?,
                "value_id": r.get::<_, Option<i64>>(3)?, "key": key, "current_value": value, "kind": r.get::<_, String>(7)?,
                "flag": r.get::<_, Option<String>>(8)?, "source": r.get::<_, String>(9)?, "agent": r.get::<_, Option<String>>(10)?,
                "reason": r.get::<_, String>(11)?, "decided_by": r.get::<_, String>(13)?, "decided_at": r.get::<_, i64>(14)?,
                "undone": undone, "undone_by": r.get::<_, Option<String>>(15)?, "undone_at": r.get::<_, Option<i64>>(16)?,
                "undo_note": r.get::<_, Option<String>>(17)?,
                "stale": stale, "effective": !undone && !stale,
            }))
        },
    )?;
    Ok(rows)
}

// ---------------------------------------------------------------- the whole order

async fn get_order(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let head = guard_order(&app, &db, &u, oid, "view order")?;
    let files = q_all(
        &db,
        "SELECT f.id, f.ord, d.id, d.filename, d.detected_type, d.size, d.sha256, (SELECT COUNT(*) FROM order_pages p WHERE p.file_id = f.id) FROM order_files f JOIN documents d ON d.id = f.document_id WHERE f.order_id = ? ORDER BY f.ord",
        params![oid],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "ord": r.get::<_, i64>(1)?, "document_id": r.get::<_, i64>(2)?, "filename": r.get::<_, String>(3)?,
                       "detected_type": r.get::<_, String>(4)?, "size": r.get::<_, i64>(5)?, "sha256": r.get::<_, String>(6)?, "pages": r.get::<_, i64>(7)? }),
            )
        },
    )?;
    let pages = q_all(
        &db,
        "SELECT page_no, file_id, file_page, png_sha256 IS NOT NULL, width, height, kind, read_state, skipped, skipped_by, skipped_at FROM order_pages WHERE order_id = ? ORDER BY page_no",
        params![oid],
        |r| {
            Ok(
                json!({ "page_no": r.get::<_, i64>(0)?, "file_id": r.get::<_, i64>(1)?, "file_page": r.get::<_, i64>(2)?, "has_image": r.get::<_, bool>(3)?,
                       "width": r.get::<_, i64>(4)?, "height": r.get::<_, i64>(5)?, "kind": r.get::<_, String>(6)?, "read_state": r.get::<_, String>(7)?,
                       "skipped": r.get::<_, bool>(8)?, "skipped_by": r.get::<_, Option<String>>(9)?, "skipped_at": r.get::<_, Option<i64>>(10)? }),
            )
        },
    )?;
    let part_rows = q_all(
        &db,
        "SELECT p.id, p.no, p.name, p.shape, p.attach_part, p.qty, p.table_rows_json, p.group_key, p.state, (SELECT page_no FROM order_pages g WHERE g.id = p.page_id) FROM order_parts p WHERE p.order_id = ? ORDER BY p.id",
        params![oid],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                json!({ "id": r.get::<_, i64>(0)?, "no": r.get::<_, String>(1)?, "name": r.get::<_, String>(2)?, "shape": r.get::<_, String>(3)?,
                        "attach_part": r.get::<_, Option<String>>(4)?, "qty": r.get::<_, i64>(5)?,
                        "table_rows": serde_json::from_str::<Value>(&r.get::<_, String>(6)?).unwrap_or(json!([])),
                        "group_key": r.get::<_, Option<String>>(7)?, "state": r.get::<_, String>(8)?, "page": r.get::<_, Option<i64>>(9)? }),
            ))
        },
    )?;
    let mut parts = Vec::new();
    let mut issues = Vec::new();
    for (pid, mut p) in part_rows {
        let values = q_all(
            &db,
            "SELECT id, key, value, text, bbox_json, source, state, was, proposed_value, proposed_by, edited_by, edited_at FROM order_values WHERE part_id = ? ORDER BY key",
            params![pid],
            |r| {
                Ok(
                    json!({ "id": r.get::<_, i64>(0)?, "key": r.get::<_, String>(1)?, "value": r.get::<_, Option<f64>>(2)?, "text": r.get::<_, String>(3)?,
                           "bbox": r.get::<_, Option<String>>(4)?.and_then(|b| serde_json::from_str::<Value>(&b).ok()),
                           "source": r.get::<_, String>(5)?, "state": r.get::<_, String>(6)?, "was": r.get::<_, Option<f64>>(7)?,
                           "proposed_value": r.get::<_, Option<f64>>(8)?, "proposed_by": r.get::<_, String>(9)?,
                           "edited_by": r.get::<_, Option<String>>(10)?, "edited_at": r.get::<_, Option<i64>>(11)? }),
                )
            },
        )?;
        if p["state"] != "EXCLUDED" {
            for v in values.iter().filter(|v| v["state"] == "MISSING") {
                issues.push(json!({ "code": "value_missing", "part": p["no"], "key": v["key"] }));
            }
        }
        p["values"] = json!(values);
        parts.push(p);
    }
    for pg in pages
        .iter()
        .filter(|p| p["read_state"] == "failed" && p["skipped"] == false)
    {
        issues.push(json!({ "code": "page_failed", "page": pg["page_no"] }));
    }
    let decisions = decision_view(&db, oid, None)?;
    let checked = decisions
        .iter()
        .rev()
        .find(|d| d["kind"] == "mark_checked" && d["undone"] == false)
        .cloned();
    ok(json!({
        "order": { "id": oid, "project_id": head.project, "name": head.name, "state": head.state,
                   "rules": serde_json::from_str::<Value>(&head.rules).unwrap_or(json!({})),
                   "created_by": head.created_by, "created_at": head.created_at },
        "files": files, "pages": pages, "parts": parts, "decisions": decisions, "issues": issues,
        "checked": checked,
        "limits": { "files": MAX_ORDER_FILES, "pages": MAX_ORDER_PAGES, "parts": MAX_ORDER_PARTS, "upload_mib": membrane::MAX_UPLOAD_BYTES / (1024 * 1024) },
    }))
}
