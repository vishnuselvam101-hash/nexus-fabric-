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

mod operator;

/// Implementation limits, stated to the user by the UI.
pub(crate) const MAX_ORDER_FILES: i64 = 100;
pub(crate) const MAX_ORDER_PAGES: i64 = 300;
const MAX_ORDER_PARTS: usize = 500;
const MAX_VALUE_BATCH: usize = 500;
const MAX_NAME_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = 500;
/// Distinct value keys one part may hold (the shapes use far fewer).
const MAX_VALUES_PER_PART: i64 = 64;
const MAX_RULES_BYTES: usize = 8 * 1024;
/// A reading below this confidence cannot be confirmed; it mirrors nf-numread's LOW_CONFIDENCE_BELOW.
const CONFIRM_MIN_CONFIDENCE: f64 = 0.85;
/// The AI operator may set CONFIRMED only for a value read from the page (a READ source) with no reader flags and at
/// least this confidence. Everything else is proposed and a person decides. (AIOP-1, REVIEW-1 HIGH-1)
const AGENT_CONFIRM_MIN_CONFIDENCE: f64 = 0.85;

const FILE_TYPES: [&str; 4] = ["pdf", "png", "jpeg", "tiff"];
const VALUE_SOURCES: [&str; 11] = [
    "read", "drawing", "table", "note", "count", "inferred", "standard", "derived", "fixed",
    "auto", "entered",
];
/// Sources that claim "this number is written on the page": their text must contain the number.
const READ_SOURCES: [&str; 5] = ["read", "drawing", "table", "note", "count"];
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
        .merge(operator::routes())
        .route(
            "/api/projects/{id}/orders",
            get(list_orders).post(create_order),
        )
        .route("/api/orders/{oid}", get(get_order))
        .route("/api/orders/{oid}/files", post(upload_file))
        .route("/api/orders/{oid}/pages/{n}", get(get_page_image))
        .route("/api/orders/{oid}/pages/{n}/read", post(read_page))
        .route("/api/orders/{oid}/pages/{n}/readings", get(list_readings))
        .route("/api/orders/{oid}/pages/{n}/skip", post(skip_page))
        .route("/api/orders/{oid}/parts", axum::routing::put(put_parts))
        .route("/api/orders/{oid}/values", axum::routing::put(put_values))
        .route("/api/orders/{oid}/confirm", post(confirm))
        .route("/api/orders/{oid}/compute", post(compute).get(get_results))
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

    /// NOTE for the raster lane: the header allows 100000 x 100000 pixels. `nf-raster` must cap width x height
    /// (and the decoded size) itself before it decodes or renders; nothing here decodes the image.
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

/// Fingerprint of the value a decision is made against: the key, the number, the text it was read from, its
/// box on the page and where it came from. A change to any of them drops the value out of "confirmed".
fn value_fp(key: &str, value: Option<f64>, text: &str, bbox: Option<&str>, source: &str) -> String {
    nf_core::sha256_hex(
        json!([key, value, text, bbox, source])
            .to_string()
            .as_bytes(),
    )
}

/// Who a request acts as comes from the session identity, never from the request body: a person's session
/// records "person"; the AI operator's own service identity (user kind "agent") records "auto".
fn actor_source(u: &User) -> &'static str {
    if u.kind == "agent" { "auto" } else { "person" }
}

/// The AI operator has to say why for every confirmation it makes.
fn agent_reason(u: &User, reason: &str) -> Result<(), ApiError> {
    if u.kind == "agent" && reason.trim().is_empty() {
        return Err(unprocessable(
            "decision_reason",
            "a decision made by automation must give its reason",
        ));
    }
    Ok(())
}

/// The operator events of a write of the AI operator (`events` in the body). Every write of the AI operator
/// carries at least one, so a person can see and replay it (422 operator_event_required); a person's write carries
/// none (403 operator_agent_only). They are stored by `store_write_events` in the transaction of the write. Only
/// `put_values` may carry a `write_ref`; the flag is explicit, never derived from an action name.
fn write_events(
    u: &User,
    events: Option<&Value>,
    allow_write_ref: bool,
) -> Result<Option<Vec<Value>>, ApiError> {
    let list = match events {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) => Some(operator::parse_event_list(a, allow_write_ref)?),
        Some(_) => {
            return Err(unprocessable(
                "operator_event",
                "operator events must be a list",
            ));
        }
    };
    if u.kind == "agent" && list.is_none() {
        return Err(unprocessable(
            "operator_event_required",
            "every write of the AI operator carries at least one operator event, so a person can see and replay it",
        ));
    }
    if list.is_some() && u.kind != "agent" {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "operator_agent_only",
            "operator events are written by the AI operator's own account",
        ));
    }
    Ok(list)
}

/// Store the events of an agent write inside its transaction (the pause is read again there).
fn store_write_events(
    app: &App,
    tx: &Connection,
    u: &User,
    oid: i64,
    events: &Option<Vec<Value>>,
) -> Result<(), ApiError> {
    if let Some(es) = events {
        operator::store_events(app, tx, u, oid, es)?;
    }
    Ok(())
}

/// True if the part holds a value a person edited, or one that is CONFIRMED or CORRECTED. The AI operator does
/// not change, exclude or drop such a part.
fn part_is_protected(tx: &Connection, part: i64) -> rusqlite::Result<bool> {
    Ok(q_count(
        tx,
        "SELECT COUNT(*) FROM order_values WHERE part_id = ? AND (edited_kind = 'person' OR state IN ('CONFIRMED', 'CORRECTED'))",
        params![part],
    )? > 0)
}

fn person_protected(what: &str) -> ApiError {
    refuse(
        StatusCode::CONFLICT,
        "person_value_protected",
        &format!(
            "{what} holds a value a person set or checked (or one that is CONFIRMED or CORRECTED); the AI operator does not change it"
        ),
    )
}

/// The role and the active flag are read again inside a write's own transaction: a user deactivated or demoted
/// while the request was being read (an upload can take a while) is refused there too.
pub(crate) fn still_writer(db: &Connection, u: &User) -> Result<(), ApiError> {
    let role: Option<String> = db
        .query_row(
            "SELECT role FROM users WHERE id = ? AND active = 1",
            params![u.id],
            |r| r.get(0),
        )
        .optional()?;
    match role.as_deref() {
        None => Err(fail(StatusCode::UNAUTHORIZED, "not signed in")),
        Some("admin" | "engineer") => Ok(()),
        Some(_) => Err(fail(
            StatusCode::FORBIDDEN,
            "your role does not permit this action",
        )),
    }
}

/// A text field of a JSON request body, as far as it can be read, for the record of a refusal.
fn body_text(raw: &[u8], key: &str) -> String {
    serde_json::from_slice::<Value>(raw)
        .ok()
        .and_then(|v| {
            v.get(key)
                .and_then(Value::as_str)
                .map(|s| clip(s, MAX_NOTE_CHARS))
        })
        .unwrap_or_default()
}

/// Writes one audit row in its own transaction. Used after a refused request has rolled back: the refusal is on
/// record and nothing else of the request is.
fn record_alone(app: &App, who: &str, action: &str, detail: &str) -> Result<(), ApiError> {
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    app.audit(&tx, who, action, &clip(detail, 700))?;
    tx.commit()?;
    Ok(())
}

/// Answers a write. A refusal that is evidence is written down in its own transaction, after the refused
/// request's transaction has rolled back: a limit (parts_limit, values_limit), any conflict (409), and any
/// refusal of the AI operator (403), a decision or upload the AI operator sent without a reason (422), and corrupt stored rules (422). An AI operator's refused parts edit keeps its reason row too.
async fn settle(app: &App, u: &User, oid: i64, what: &str, note: String, res: Api) -> Api {
    let resp = match res {
        Ok(r) => return Ok(r),
        Err(ApiError::Resp(resp)) => resp,
        Err(other) => return Err(other),
    };
    let (parts, body) = resp.into_parts();
    let bytes = axum::body::to_bytes(body, 64 * 1024)
        .await
        .unwrap_or_default();
    let answer: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let code = answer["code"].as_str().unwrap_or_default().to_string();
    let msg = answer["error"].as_str().unwrap_or_default().to_string();
    let agent = u.kind == "agent";
    let limit = matches!(code.as_str(), "parts_limit" | "values_limit");
    if limit
        || code == "order_rules_corrupt"
        || parts.status == StatusCode::CONFLICT
        || (agent && parts.status == StatusCode::FORBIDDEN)
        || (agent && (code == "decision_reason" || code == "operator_event_required"))
    {
        let note_text = if note.trim().is_empty() {
            String::new()
        } else {
            format!("; note: {}", clip(&note, 300))
        };
        record_alone(
            app,
            &u.username,
            "order_refused",
            &format!(
                "order #{oid} {what} refused: {code} ({}) {msg}{note_text}",
                parts.status.as_u16()
            ),
        )?;
        if agent && what == "parts edit" {
            record_alone(
                app,
                &u.username,
                "order_parts_reason",
                &format!(
                    "order #{oid} parts edit refused ({code}): {}",
                    clip(&note, 300)
                ),
            )?;
        }
    }
    Err(ApiError::Resp(Box::new(axum::http::Response::from_parts(
        parts,
        axum::body::Body::from(bytes),
    ))))
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

/// `guard_order` for a write inside its transaction. A pause is rechecked here, in the transaction that writes, for
/// every write of the AI operator: a pause a person set after the request passed `write_access` (or while it was
/// queued behind the write lock) is still seen, and the whole write is rolled back. A request that was already
/// committed before the pause landed is complete (its values and its events commit together); the pause
/// stops the next write.
fn guard_order_tx(
    app: &App,
    tx: &Connection,
    u: &User,
    oid: i64,
    what: &str,
) -> Result<OrderHead, ApiError> {
    let head = guard_order(app, tx, u, oid, what)?;
    if u.kind == "agent" && operator::is_paused(tx, oid)? {
        return Err(refuse(
            StatusCode::CONFLICT,
            "operator_paused",
            "a person paused the operator on this order",
        ));
    }
    Ok(head)
}

/// Authenticate, require a writing role and the project, all before a request body is read.
fn write_access(app: &App, headers: &HeaderMap, oid: i64, what: &str) -> Result<User, ApiError> {
    let db = app.db();
    let u = auth_db(&db, app, headers)?;
    require(&u, &["admin", "engineer"])?;
    guard_order(app, &db, &u, oid, what)?;
    // a person paused the operator: its writes wait (the operator's own event post is refused the same way)
    if u.kind == "agent" && operator::is_paused(&db, oid)? {
        drop(db);
        record_alone(
            app,
            &u.username,
            "order_refused",
            &format!("order #{oid} {what} refused: operator_paused (409)"),
        )?;
        return Err(refuse(
            StatusCode::CONFLICT,
            "operator_paused",
            "a person paused the operator on this order",
        ));
    }
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

/// Full-width digits and separators, as written in Japanese drawings, read as ASCII; every minus sign
/// (hyphen, full-width, U+2212, dashes) read as '-'.
fn ascii_digits(text: &str) -> Vec<char> {
    text.chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32('0' as u32 + (c as u32 - '０' as u32)).unwrap_or(c),
            '．' => '.',
            '，' => ',',
            '－' | '−' | '‐' | '‑' | '–' | '—' | '➖' => '-',
            c => c,
        })
        .collect()
}

/// One number as written in a text: its value, and whether its reading is ambiguous.
struct Reading {
    value: f64,
    ambiguous: bool,
}

/// The value of one written number token. A comma is a thousands separator only in groups of three digits
/// ("1,500", "12,345.6"). Any other comma ("1,5", "1,50") gives one reading, flagged ambiguous: it is read as a
/// decimal comma, and an ambiguous reading never counts as the number written on the page.
fn reading(tok: &str) -> Option<(f64, bool)> {
    if !tok.contains(',') {
        return tok.parse::<f64>().ok().map(|f| (f, false));
    }
    let (int_part, frac) = match tok.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (tok, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let groups: Vec<&str> = int_part.split(',').collect();
    let grouped = groups.len() > 1
        && digits(groups[0])
        && groups[0].len() <= 3
        && !groups[0].starts_with('0')
        && groups[1..].iter().all(|g| g.len() == 3 && digits(g))
        && !frac.is_some_and(|f| f.contains(','));
    if grouped {
        tok.replace(',', "").parse::<f64>().ok().map(|f| (f, false))
    } else {
        tok.replace(',', ".").parse::<f64>().ok().map(|f| (f, true))
    }
}

/// The numbers written in `text`, with their sign. A '-' is a sign only when it stands right before the digits
/// and not right after a letter or digit ("PL-9" and "100-200" hold the unsigned 9, 100 and 200).
fn numbers_in(text: &str) -> Vec<Reading> {
    let t = ascii_digits(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let lead = t[i].is_ascii_digit()
            || (t[i] == '.' && t.get(i + 1).is_some_and(char::is_ascii_digit));
        if !lead {
            i += 1;
            continue;
        }
        let start = i;
        while i < t.len() && (t[i].is_ascii_digit() || t[i] == '.' || t[i] == ',') {
            i += 1;
        }
        let tok: String = t[start..i].iter().collect();
        let tok = tok.trim_matches(|c| c == '.' || c == ',');
        let neg =
            start > 0 && t[start - 1] == '-' && (start < 2 || !t[start - 2].is_alphanumeric());
        if let Some((f, ambiguous)) = reading(tok) {
            out.push(Reading {
                value: if neg { -f } else { f },
                ambiguous,
            });
        }
    }
    out
}

fn same_number(a: f64, value: f64) -> bool {
    (a - value).abs() <= 1e-9 * value.abs().max(1.0)
}

/// True if some number written in `text` equals `value`, sign included (so a read value really is on the page).
/// An ambiguous reading does not count.
pub(crate) fn text_contains_number(text: &str, value: f64) -> bool {
    numbers_in(text)
        .into_iter()
        .any(|r| !r.ambiguous && same_number(r.value, value))
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

/// Everything a "checked" mark is made against: the files, the pages (and which are skipped), the parts and
/// every value with its text, box, source and state. Any change to one of them makes an earlier mark stale.
fn order_fingerprint(db: &Connection, oid: i64) -> rusqlite::Result<String> {
    let files = q_all(
        db,
        "SELECT f.ord, d.sha256 FROM order_files f JOIN documents d ON d.id = f.document_id WHERE f.order_id = ? ORDER BY f.ord",
        params![oid],
        |r| Ok(json!([r.get::<_, i64>(0)?, r.get::<_, String>(1)?])),
    )?;
    let pages = q_all(
        db,
        "SELECT page_no, file_id, file_page, skipped FROM order_pages WHERE order_id = ? ORDER BY page_no",
        params![oid],
        |r| {
            Ok(json!([
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, bool>(3)?
            ]))
        },
    )?;
    let parts = q_all(
        db,
        "SELECT id, no, name, shape, qty, COALESCE(attach_part,''), COALESCE(group_key,''), table_rows_json, (SELECT page_no FROM order_pages g WHERE g.id = page_id) FROM order_parts WHERE order_id = ? AND state <> 'EXCLUDED' ORDER BY no",
        params![oid],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                json!([
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, Option<i64>>(8)?
                ]),
            ))
        },
    )?;
    let mut all = Vec::new();
    for (id, head) in parts {
        let vals = q_all(
            db,
            "SELECT key, value, text, bbox_json, source, state FROM order_values WHERE part_id = ? ORDER BY key",
            params![id],
            |r| {
                Ok(json!([
                    value_fp(
                        &r.get::<_, String>(0)?,
                        r.get::<_, Option<f64>>(1)?,
                        &r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?.as_deref(),
                        &r.get::<_, String>(4)?,
                    ),
                    r.get::<_, String>(5)?
                ]))
            },
        )?;
        all.push(json!([head, vals]));
    }
    // an accepted flag, and its undo, change what a "checked" mark was made against
    let accepts = q_all(
        db,
        "SELECT id, COALESCE(value_id, 0), COALESCE(flag, ''), undone_at IS NULL FROM order_decisions WHERE order_id = ? AND kind = 'accept_flag' ORDER BY id",
        params![oid],
        |r| {
            Ok(json!([
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, bool>(3)?
            ]))
        },
    )?;
    Ok(nf_core::sha256_hex(
        json!([files, pages, all, accepts]).to_string().as_bytes(),
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
    let reason = text_field(&body, "reason", MAX_NOTE_CHARS)?.unwrap_or_default();
    agent_reason(&u, &reason)?;
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
    let events = write_events(&u, body.get("events"), false)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, &u)?;
    project_guard(&app, &tx, &u, project, "create order")?;
    tx.execute(
        "INSERT INTO orders(project_id, name, state, rules_json, created_by, created_at) VALUES (?,?,?,?,?,?)",
        params![project, name, "OPEN", rules, u.username, now() as i64],
    )?;
    let oid = tx.last_insert_rowid();
    store_write_events(&app, &tx, &u, oid, &events)?;
    let reason_text = if reason.trim().is_empty() {
        String::new()
    } else {
        format!(" reason: {}", clip(&reason, 300))
    };
    app.audit(
        &tx,
        &u.username,
        "order_created",
        &format!("project #{project} order #{oid} '{name}'{reason_text}"),
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

/// The upload is the raw file body (with its name in `x-filename`). The reason of an AI operator's upload is the
/// `reason` field of the request: a query parameter, since the body is the file itself.
async fn upload_file(
    State(app): Ctx,
    headers: HeaderMap,
    Path(oid): Path<i64>,
    axum::extract::Query(query): axum::extract::Query<BTreeMap<String, String>>,
    body: axum::body::Body,
) -> Api {
    let u = write_access(&app, &headers, oid, "order file")?;
    let reason = query.get("reason").cloned().unwrap_or_default();
    let res = upload_file_do(&app, &headers, &u, oid, &reason, body).await;
    settle(&app, &u, oid, "file upload", reason, res).await
}

async fn upload_file_do(
    app: &App,
    headers: &HeaderMap,
    u: &User,
    oid: i64,
    reason: &str,
    body: axum::body::Body,
) -> Api {
    if reason.chars().count() > MAX_NOTE_CHARS {
        return Err(unprocessable(
            "field_too_long",
            &format!("'reason' is longer than {MAX_NOTE_CHARS} characters"),
        ));
    }
    agent_reason(u, reason)?;
    // a file has no JSON body: the operator's events travel in a header, hex of the JSON list
    let header_events: Option<Value> = match headers.get("x-operator-events") {
        None => None,
        Some(h) => Some(
            h.to_str()
                .ok()
                .and_then(|h| hex::decode(h).ok())
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .ok_or_else(|| {
                    unprocessable(
                        "operator_event",
                        "X-Operator-Events is the hex of a JSON list of events",
                    )
                })?,
        ),
    };
    let events = write_events(u, header_events.as_ref(), false)?;
    let (bytes, _slot) = read_upload_body(app, headers, body).await?;
    let filename = upload_filename(headers);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    let head = guard_order_tx(app, &tx, u, oid, "order file")?;
    store_write_events(app, &tx, u, oid, &events)?;
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
        app.audit(
            &tx,
            &u.username,
            "order_file_rejected",
            &format!("order #{oid} '{filename}': type {detected} is not an order file"),
        )?;
        tx.commit()?;
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
        app.audit(
            &tx,
            &u.username,
            "order_file_rejected",
            &format!(
                "order #{oid} '{filename}': order_limit ({files} files, {pages} pages, +{} page(s))",
                seeds.len()
            ),
        )?;
        tx.commit()?;
        return Err(refuse(
            StatusCode::CONFLICT,
            "order_limit",
            &format!(
                "an order holds at most {MAX_ORDER_FILES} files and {MAX_ORDER_PAGES} pages; this one has {files} files and {pages} pages and the new file adds {} page(s)",
                seeds.len()
            ),
        ));
    }
    let doc = register_document(app, &tx, u, head.project, &filename, &bytes, detected)?;
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
    let reason_text = if reason.trim().is_empty() {
        String::new()
    } else {
        format!(" reason: {}", clip(reason, 300))
    };
    app.audit(
        &tx,
        &u.username,
        "order_file_added",
        &format!(
            "order #{oid} file #{file_id} '{filename}' {detected} {} pages {}-{}{reason_text}",
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

/// One number the reader found on a page, before it is stored.
pub(crate) struct ReadingItem {
    pub text: String,
    pub value: Option<f64>,
    pub kind: String,
    pub bbox: [f64; 4],
    pub flags: Vec<String>,
    pub confidence: f64,
}

/// What the reader produced for one page and who produced it (the reader's name and the SHA-256 of its models).
pub(crate) struct ReaderOut {
    pub reader_id: String,
    pub model_sha256: String,
    pub items: Vec<ReadingItem>,
}

/// Most readings the server keeps for one order.
const MAX_READINGS_PER_ORDER: i64 = 50_000;

/// The number reader (nf-numread) runs only in a build made with the `number-reader` feature.
#[cfg(feature = "number-reader")]
fn run_reader(png: &[u8]) -> Result<ReaderOut, ApiError> {
    use sha2::{Digest, Sha256};
    static READER: std::sync::Mutex<Option<Arc<nf_numread::Reader>>> = std::sync::Mutex::new(None);
    let reader = {
        let mut slot = READER.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            let r = nf_numread::Reader::new().map_err(|e| {
                elog!("nf-server: the number reader cannot start: {e}");
                reader_missing()
            })?;
            *slot = Some(Arc::new(r));
        }
        slot.clone().ok_or_else(reader_missing)?
    };
    let items = reader.read_png(png).map_err(|e| {
        refuse(
            StatusCode::UNPROCESSABLE_ENTITY,
            "page_unreadable",
            &format!("the page could not be read: {e}"),
        )
    })?;
    let models = format!(
        "{}:{}:{}",
        nf_numread::DET_SHA256,
        nf_numread::REC_SHA256,
        nf_numread::DICT_SHA256
    );
    let items = items
        .into_iter()
        .map(|it| {
            let mut flags = Vec::new();
            for (on, name) in [
                (it.repaired, "repaired"),
                (it.ambiguous, "ambiguous"),
                (it.phi_inferred, "phi_inferred"),
                (it.low_confidence, "low_confidence"),
            ] {
                if on {
                    flags.push(name.to_string());
                }
            }
            ReadingItem {
                text: it.text,
                value: it.value,
                kind: it.kind.name().to_string(),
                bbox: it.bbox.map(|x| (f64::from(x) * 10.0).round() / 10.0),
                flags,
                confidence: f64::from(it.confidence),
            }
        })
        .collect();
    Ok(ReaderOut {
        reader_id: "nf-numread R0".to_string(),
        model_sha256: hex::encode(Sha256::digest(models.as_bytes())),
        items,
    })
}

#[cfg(not(feature = "number-reader"))]
fn run_reader(_png: &[u8]) -> Result<ReaderOut, ApiError> {
    Err(reader_missing())
}

fn reader_missing() -> ApiError {
    fail_json(
        StatusCode::NOT_IMPLEMENTED,
        json!({ "error": "the page reader is not installed in this build", "error_ja": "このビルドにはページ読み取り機能が入っていません", "code": "reader_not_installed" }),
    )
}

/// Store what the reader found on one page, inside the caller's transaction. A reading is never changed or deleted;
/// reading a page again adds new rows. Returns the stored rows as JSON.
pub(crate) fn store_readings(
    app: &App,
    tx: &Connection,
    u: &User,
    oid: i64,
    page_no: i64,
    out: &ReaderOut,
) -> Result<Vec<Value>, ApiError> {
    let held = q_count(
        tx,
        "SELECT COUNT(*) FROM order_readings WHERE order_id = ?",
        params![oid],
    )?;
    if held + out.items.len() as i64 > MAX_READINGS_PER_ORDER {
        return Err(refuse(
            StatusCode::CONFLICT,
            "readings_limit",
            &format!("an order keeps at most {MAX_READINGS_PER_ORDER} readings"),
        ));
    }
    let at = now() as i64;
    let mut rows = Vec::with_capacity(out.items.len());
    for it in &out.items {
        let (bbox, flags) = (json!(it.bbox).to_string(), json!(it.flags).to_string());
        tx.execute(
            "INSERT INTO order_readings(order_id, page_no, bbox_json, text, value, kind, flags_json, confidence, reader_id, model_sha256, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
            params![oid, page_no, bbox, it.text, it.value, it.kind, flags, it.confidence, out.reader_id, out.model_sha256, u.username, at],
        )?;
        rows.push(json!({ "id": tx.last_insert_rowid(), "text": it.text, "value": it.value, "kind": it.kind,
            "bbox": it.bbox, "flags": it.flags, "confidence": it.confidence }));
    }
    app.audit(
        tx,
        &u.username,
        "order_page_read",
        &format!(
            "order #{oid} page {page_no}: {} readings by {} (models {})",
            out.items.len(),
            out.reader_id,
            clip(&out.model_sha256, 16)
        ),
    )?;
    Ok(rows)
}

async fn read_page(
    State(app): Ctx,
    headers: HeaderMap,
    Path((oid, n)): Path<(i64, i64)>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "read page")?;
    let body = if raw.is_empty() {
        json!({})
    } else {
        request_json(&headers, &raw)?
    };
    let events = write_events(&u, body.get("events"), false)?;
    let png: Option<String> = {
        let db = app.db();
        db.query_row(
            "SELECT png_sha256 FROM order_pages WHERE order_id = ? AND page_no = ?",
            params![oid, n],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(not_found)?
    };
    let res = read_page_do(&app, &u, oid, n, png, events).await;
    match res {
        Err(ApiError::Resp(r)) if r.status() == StatusCode::NOT_IMPLEMENTED => {
            // the refused read is on record: it is a request the build cannot serve
            record_alone(
                &app,
                &u.username,
                "order_page_read_refused",
                &format!("order #{oid} page {n}: reader_not_installed"),
            )?;
            Err(ApiError::Resp(r))
        }
        other => settle(&app, &u, oid, "read page", String::new(), other).await,
    }
}

async fn read_page_do(
    app: &Arc<App>,
    u: &User,
    oid: i64,
    n: i64,
    png: Option<String>,
    events: Option<Vec<Value>>,
) -> Api {
    if !cfg!(feature = "number-reader") {
        return Err(reader_missing());
    }
    let Some(sha) = png else {
        return Err(refuse(
            StatusCode::CONFLICT,
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
    // the reader runs outside the database lock
    let out = tokio::task::spawn_blocking(move || run_reader(&bytes))
        .await
        .map_err(|_| {
            fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the reader stopped unexpectedly",
            )
        })??;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "read page")?;
    store_write_events(app, &tx, u, oid, &events)?;
    let rows = store_readings(app, &tx, u, oid, n, &out)?;
    tx.execute(
        "UPDATE order_pages SET read_state = 'read' WHERE order_id = ? AND page_no = ? AND read_state IN ('none', 'failed')",
        params![oid, n],
    )?;
    tx.commit()?;
    ok(
        json!({ "page_no": n, "reader": out.reader_id, "model_sha256": out.model_sha256, "count": rows.len(), "readings": rows }),
    )
}

/// The readings the server stored for one page (all reads of it, oldest first).
async fn list_readings(
    State(app): Ctx,
    headers: HeaderMap,
    Path((oid, n)): Path<(i64, i64)>,
) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    guard_order(&app, &db, &u, oid, "view readings")?;
    let (w, h): (i64, i64) = db
        .query_row(
            "SELECT width, height FROM order_pages WHERE order_id = ? AND page_no = ?",
            params![oid, n],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(not_found)?;
    let rows = q_all(
        &db,
        "SELECT id, text, value, kind, bbox_json, flags_json, confidence, reader_id, model_sha256, created_at FROM order_readings WHERE order_id = ? AND page_no = ? ORDER BY id LIMIT 20000",
        params![oid, n],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "text": r.get::<_, String>(1)?, "value": r.get::<_, Option<f64>>(2)?,
                "kind": r.get::<_, String>(3)?,
                "bbox": serde_json::from_str::<Value>(&r.get::<_, String>(4)?).unwrap_or(Value::Null),
                "flags": serde_json::from_str::<Value>(&r.get::<_, String>(5)?).unwrap_or(Value::Null),
                "confidence": r.get::<_, f64>(6)?, "reader_id": r.get::<_, String>(7)?,
                "model_sha256": r.get::<_, String>(8)?, "created_at": r.get::<_, i64>(9)? }),
            )
        },
    )?;
    ok(json!({ "page_no": n, "width": w, "height": h, "readings": rows }))
}

async fn skip_page(
    State(app): Ctx,
    headers: HeaderMap,
    Path((oid, n)): Path<(i64, i64)>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "skip page")?;
    let res = skip_do(&app, &headers, &u, oid, n, &raw);
    settle(&app, &u, oid, "skip page", body_text(&raw, "note"), res).await
}

fn skip_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, n: i64, raw: &[u8]) -> Api {
    let body = if raw.is_empty() {
        json!({})
    } else {
        request_json(headers, raw)?
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
    agent_reason(u, &note)?;
    let events = write_events(u, body.get("events"), false)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "skip page")?;
    store_write_events(app, &tx, u, oid, &events)?;
    let (state, was): (String, bool) = tx
        .query_row(
            "SELECT read_state, skipped FROM order_pages WHERE order_id = ? AND page_no = ?",
            params![oid, n],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(not_found)?;
    // a page whose parts hold a person's, a confirmed or a corrected value is not the operator's to skip or unskip
    if u.kind == "agent" && skip != was {
        let held = q_count(
            &tx,
            "SELECT COUNT(*) FROM order_values v JOIN order_parts p ON p.id = v.part_id JOIN order_pages g ON g.id = p.page_id WHERE g.order_id = ? AND g.page_no = ? AND (v.edited_kind = 'person' OR v.state IN ('CONFIRMED', 'CORRECTED'))",
            params![oid, n],
        )?;
        if held > 0 {
            return Err(person_protected(&format!("page {n}")));
        }
    }
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
    } else if !note.trim().is_empty() {
        app.audit(
            &tx,
            &u.username,
            "order_page_note",
            &format!(
                "order #{oid} page {n} (read state {state}) unchanged, note: {}",
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
    let res = put_parts_do(&app, &headers, &u, oid, &raw);
    settle(&app, &u, oid, "parts edit", body_text(&raw, "note"), res).await
}

/// The AI operator may not change a part that holds a person's, a confirmed or a corrected value: not its number,
/// name, shape, quantity, page, attachment, group or table rows, not exclude it, bring it back, nor leave it out
/// of the list (which excludes it).
fn agent_part_guard(
    tx: &Connection,
    oid: i64,
    parts: &[PartIn],
    existing: &BTreeMap<i64, String>,
    listed: &BTreeSet<i64>,
) -> Result<(), ApiError> {
    type Row = (
        String,
        String,
        String,
        i64,
        Option<i64>,
        Option<String>,
        String,
        Option<String>,
        String,
    );
    for (id, no) in existing {
        if !part_is_protected(tx, *id)? {
            continue;
        }
        let Some(p) = parts.iter().find(|p| p.id == Some(*id)) else {
            if listed.contains(id) {
                continue;
            }
            return Err(person_protected(&format!(
                "part '{no}' (it cannot be left out)"
            )));
        };
        let old: Row = tx.query_row(
            "SELECT no, name, shape, qty, page_id, attach_part, table_rows_json, group_key, state FROM order_parts WHERE id = ?",
            params![id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )?;
        let page_id: Option<i64> = match p.page {
            None => None,
            Some(n) => tx
                .query_row(
                    "SELECT id FROM order_pages WHERE order_id = ? AND page_no = ?",
                    params![oid, n],
                    |r| r.get(0),
                )
                .optional()?,
        };
        let same = old.0 == p.no
            && old.1 == p.name
            && old.2 == p.shape
            && old.3 == p.qty
            && old.4 == page_id
            && old.5 == p.attach
            && old.6 == p.rows
            && old.7 == p.group
            && (old.8 == "EXCLUDED") == p.exclude;
        if !same {
            return Err(person_protected(&format!("part '{no}'")));
        }
    }
    Ok(())
}

fn put_parts_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, raw: &[u8]) -> Api {
    let body = request_json(headers, raw)?;
    let note = text_field(&body, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    agent_reason(u, &note)?;
    let events = write_events(u, body.get("events"), false)?;
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
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "edit parts")?;
    store_write_events(app, &tx, u, oid, &events)?;
    // the reason of an AI operator, and any note a person gives, are on record
    if u.kind == "agent" || !note.trim().is_empty() {
        app.audit(
            &tx,
            &u.username,
            "order_parts_reason",
            &format!("order #{oid} parts edit: {}", clip(&note, 300)),
        )?;
    }
    let existing: BTreeMap<i64, String> = q_all(
        &tx,
        "SELECT id, no FROM order_parts WHERE order_id = ?",
        params![oid],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )?
    .into_iter()
    .collect();
    // every row counts, excluded ones too: they are kept, so they must not pile up without end
    let new_rows = parts.iter().filter(|p| p.id.is_none()).count();
    if existing.len() + new_rows > MAX_ORDER_PARTS {
        return Err(refuse(
            StatusCode::CONFLICT,
            "parts_limit",
            &format!(
                "an order holds at most {MAX_ORDER_PARTS} parts (excluded ones count); it has {} and this adds {new_rows}",
                existing.len()
            ),
        ));
    }
    let listed_ids: BTreeSet<i64> = parts.iter().filter_map(|p| p.id).collect();
    if u.kind == "agent" {
        agent_part_guard(&tx, oid, &parts, &existing, &listed_ids)?;
    }
    for p in &parts {
        // a number held by a part that is not in this list (it is only excluded, never deleted) stays taken
        if let Some((other, _)) = existing
            .iter()
            .find(|(id, no)| **no == p.no && !listed_ids.contains(id))
            && p.id != Some(*other)
        {
            return Err(refuse(
                StatusCode::CONFLICT,
                "part_no_exists",
                &format!("part no '{}' belongs to another part of this order", p.no),
            ));
        }
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
    flags: Vec<String>,
    confidence: Option<f64>,
    confirm: bool,
    note: String,
    /// The server-side reading (`order_readings`) this value comes from. The AI operator confirms only against one.
    reading_id: Option<i64>,
    /// The sticky attachment written with this value: set on its first proposal, never replaced afterwards.
    attached_reading_id: Option<i64>,
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
            let ambiguous = numbers_in(&text)
                .iter()
                .any(|r| r.ambiguous && same_number(r.value, f));
            return Err(if ambiguous {
                unprocessable(
                    "value_text_ambiguous",
                    &format!(
                        "the text '{}' reads {f} only as an ambiguous comma number; write the number as it is on the page, with a point or as a thousands group",
                        clip(&text, 60)
                    ),
                )
            } else {
                unprocessable(
                    "value_text_mismatch",
                    &format!(
                        "the text '{}' does not contain the number {f}",
                        clip(&text, 60)
                    ),
                )
            });
        }
    }
    let bbox = parse_bbox(v.get("bbox"))?;
    let flags = match v.get("flags") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a))
            if a.len() <= 8
                && a.iter().all(|x| {
                    x.as_str().is_some_and(|s| {
                        !s.is_empty()
                            && s.len() <= 32
                            && s.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                    })
                }) =>
        {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        }
        Some(_) => {
            return Err(unprocessable(
                "value_flags",
                "'flags' is a short list of reader flag names",
            ));
        }
    };
    let confidence = match v.get("confidence") {
        None | Some(Value::Null) => None,
        Some(x) => match x.as_f64().filter(|f| (0.0..=1.0).contains(f)) {
            Some(f) => Some(f),
            None => {
                return Err(unprocessable(
                    "value_confidence",
                    "'confidence' is a number from 0 to 1",
                ));
            }
        },
    };
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
    let reading_id = match v.get("reading_id") {
        None | Some(Value::Null) => None,
        Some(x) => Some(x.as_i64().filter(|i| *i >= 1).ok_or_else(|| {
            unprocessable("reading_id", "'reading_id' is the id of a stored reading")
        })?),
    };
    if confirm && reading_id.is_none() && READ_SOURCES.contains(&source.as_str()) {
        // A confirming caller must state typed reading state. `null` is normalised to "no flags"
        // and "no confidence" below, so it must not count as an explicit answer here.
        if !matches!(v.get("flags"), Some(Value::Array(_)))
            || v.get("confidence").and_then(Value::as_f64).is_none()
        {
            return Err(unprocessable(
                "confirm_needs_reading_state",
                "a value read from the page needs its flags and confidence before it can be confirmed",
            ));
        }
    }
    // A flag or a low confidence forbids confirmation whatever the caller calls the source;
    // otherwise a flagged reading could be relabelled `derived` and slip through the boundary.
    if confirm && !flags.is_empty() {
        return Err(unprocessable(
            "value_flagged",
            "a flagged reading is only proposed; a person must check it",
        ));
    }
    if confirm && confidence.is_some_and(|c| c < CONFIRM_MIN_CONFIDENCE) {
        return Err(unprocessable(
            "value_low_confidence",
            "a low-confidence reading is only proposed; a person must check it",
        ));
    }
    let note = text_field(v, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    Ok(ValueIn {
        reading_id,
        attached_reading_id: None,
        part,
        key,
        value,
        text,
        bbox,
        source,
        flags,
        confidence,
        confirm,
        note,
    })
}

/// Validate the `write_ref` of every value-write event against the values actually written in this batch, then
/// put the server-side number into the stored event so a person's undo reverts what the server wrote. Events
/// without a `write_ref` are ordinary replay events and are left alone.
fn validate_value_write_events(
    tx: &Connection,
    oid: i64,
    events: &mut [Value],
    values: &[ValueIn],
) -> Result<(), ApiError> {
    let bad_ref = || {
        unprocessable(
            "operator_event",
            "an event's write_ref must name a value written in this request",
        )
    };
    for e in events.iter_mut() {
        let Some(w) = e.get("write_ref").filter(|w| !w.is_null()) else {
            continue;
        };
        let key = w
            .get("key")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| bad_ref())?;
        let named_part = w.get("part").cloned().ok_or_else(|| bad_ref())?;
        let mut written = None;
        for v in values {
            let (pid, pno, _) = find_part(tx, oid, &v.part).map_err(|_| bad_ref())?;
            let part_matches = match &named_part {
                Value::String(no) => no == &pno,
                Value::Number(_) => named_part == json!(pid),
                _ => false,
            };
            if part_matches && v.key == key {
                written = Some((v, pid));
                break;
            }
        }
        let (v, _pid) = written.ok_or_else(|| bad_ref())?;
        let number = v.value.ok_or_else(|| {
            unprocessable("operator_event", "a write_ref cannot name a missing value")
        })?;
        e["value"] = json!(number);
    }
    Ok(())
}

/// Tie a value to the stored reading it names. The reading's own text, box, flags, confidence and source replace
/// what the caller sent. The AI operator sets CONFIRMED only through a reading whose stored value is the value,
/// whose stored flags are empty, whose stored confidence is at least AGENT_CONFIRM_MIN_CONFIDENCE and which was
/// read on the page of the target part (`order_parts.page_id -> order_pages.page_no`). A caller's bbox is never
/// proof of the page. The attachment is server-owned and sticky: a proposal sets it once, and a confirmation must
/// name that same reading. Without part geometry the server cannot prove which part a reading physically belongs
/// to, so the first attachment remains the operator's assignment; it cannot be switched later.
fn bind_reading(
    tx: &Connection,
    oid: i64,
    u: &User,
    part: i64,
    v: &mut ValueIn,
) -> Result<(), ApiError> {
    let agent = u.kind == "agent";
    let deny = |msg: String| {
        if agent {
            refuse(StatusCode::FORBIDDEN, "agent_cannot_confirm", &msg)
        } else {
            unprocessable("reading_mismatch", &msg)
        }
    };
    let Some(rid) = v.reading_id else {
        if agent && v.confirm {
            return Err(deny(format!(
                "the AI operator confirms only against a reading the server stored (reading_id); '{}' is only proposed, a person decides",
                v.key
            )));
        }
        return Ok(());
    };
    if !agent {
        return Err(unprocessable(
            "reading_agent_only",
            "'reading_id' is for the AI operator's own writes",
        ));
    }
    let attached: Option<i64> = tx
        .query_row(
            "SELECT attached_reading_id FROM order_values WHERE part_id = ? AND key = ?",
            params![part, v.key],
            |r| r.get(0),
        )
        .optional()?;
    if v.confirm {
        // A confirmation that would create (or switch) the attachment is not a confirmation of this value.
        if attached != Some(rid) {
            return Err(deny(format!(
                "reading #{rid} is not the reading '{}' was first proposed from; only a person may change that assignment",
                v.key
            )));
        }
    } else if let Some(id) = attached.filter(|id| *id != rid) {
        return Err(deny(format!(
            "'{}' is already attached to reading #{id}; a proposal cannot switch it",
            v.key
        )));
    }
    let target_page: Option<i64> = tx
        .query_row(
            "SELECT pg.page_no FROM order_parts pt JOIN order_pages pg ON pg.id = pt.page_id AND pg.order_id = pt.order_id WHERE pt.id = ? AND pt.order_id = ?",
            params![part, oid],
            |r| r.get(0),
        )
        .optional()?;
    let Some(target_page) = target_page else {
        return Err(deny(format!(
            "part '{}' is not on a page of this order",
            v.part
        )));
    };
    type Rd = (i64, String, Option<f64>, String, String, f64);
    let row: Option<Rd> = tx
        .query_row(
            "SELECT page_no, text, value, bbox_json, flags_json, confidence FROM order_readings WHERE id = ? AND order_id = ?",
            params![rid, oid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;
    let Some((page, text, value, bbox, flags, conf)) = row else {
        return Err(deny(format!(
            "reading #{rid} is not a reading of this order"
        )));
    };
    if page != target_page {
        return Err(deny(format!(
            "reading #{rid} is of page {page} but part '{}' is on page {target_page}",
            v.part
        )));
    }
    if !matches!((value, v.value), (Some(a), Some(b)) if same_number(a, b)) {
        return Err(deny(format!(
            "reading #{rid} is {} but '{}' is {}",
            fmt_val(value),
            v.key,
            fmt_val(v.value)
        )));
    }
    if v.confirm {
        if flags != "[]" {
            return Err(deny(format!(
                "reading #{rid} is flagged {flags}; it is only proposed, a person must check it"
            )));
        }
        if conf < AGENT_CONFIRM_MIN_CONFIDENCE {
            return Err(deny(format!(
                "reading #{rid} has confidence {conf:.2}, below {AGENT_CONFIRM_MIN_CONFIDENCE}; it is only proposed, a person must check it"
            )));
        }
    }
    v.text = text;
    v.bbox = Some(bbox);
    v.source = "read".to_string();
    v.flags = serde_json::from_str(&flags).unwrap_or_default();
    v.confidence = Some(conf);
    v.attached_reading_id = attached.or(Some(rid));
    Ok(())
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
    String,
    String,
    Option<f64>,
    Option<i64>,
);

async fn put_values(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "edit values")?;
    let res = put_values_do(&app, &headers, &u, oid, &raw);
    settle(&app, &u, oid, "value edit", String::new(), res).await
}

pub(crate) fn put_values_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, raw: &[u8]) -> Api {
    let body = request_json(headers, raw)?;
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
    let mut values: Vec<ValueIn> = list.iter().map(parse_value).collect::<Result<_, _>>()?;
    for v in &values {
        agent_reason(u, &v.note)?;
    }
    // An agent sends its type events with the values so both commit in one transaction: no pause can land
    // between a written value and its replayable event. Only here may an event carry a `write_ref`; it is checked
    // against the values actually written before the batch is stored.
    let mut events = write_events(u, body.get("events"), true)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "edit values")?;
    // The AI operator confirms only against a reading the server itself stored for the target part's page;
    // what the caller says about flags, confidence, source or bbox never grants anything.
    for v in values.iter_mut() {
        let (pid, _, _) = find_part(&tx, oid, &v.part)?;
        bind_reading(&tx, oid, u, pid, v)?;
    }
    let (mut changed, mut unchanged) = (0, 0);
    let mut touched = BTreeSet::new();
    for v in &values {
        let flags_json = json!(v.flags).to_string();
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
                "SELECT id, value, text, bbox_json, source, state, proposed_value, edited_kind, flags_json, confidence, attached_reading_id FROM order_values WHERE part_id = ? AND key = ?",
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
                        r.get(7)?,
                        r.get(8)?,
                        r.get(9)?,
                        r.get(10)?,
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
            Some((id, val, text, bbox, src, state, _, kind, fl, conf, attached)) => {
                let same =
                    *val == v.value && *text == v.text && *bbox == v.bbox && *src == v.source;
                let meta_same = *fl == flags_json
                    && *conf == v.confidence
                    && v.attached_reading_id
                        .map_or(true, |rid| *attached == Some(rid));
                // identical content: nothing to write (and a resend never drops an earlier confirmation)
                if same
                    && (state == new_state || (!v.confirm && state == "CONFIRMED"))
                    && (meta_same || state == "CONFIRMED")
                {
                    unchanged += 1;
                    continue;
                }
                // a person's value, a confirmed value and a corrected value are not the AI operator's to change
                if u.kind == "agent"
                    && (kind == "person" || state == "CONFIRMED" || state == "CORRECTED")
                {
                    return Err(refuse(
                        StatusCode::CONFLICT,
                        "person_value_protected",
                        &format!(
                            "'{}' of part '{pno}' was set or checked by a person (or is {state}); the AI operator does not overwrite it",
                            v.key
                        ),
                    ));
                }
                tx.execute(
                    "UPDATE order_values SET value = ?, text = ?, bbox_json = ?, source = ?, state = ?, was = ?, edited_by = ?, edited_at = ?, flags_json = ?, confidence = ?, edited_kind = ?, attached_reading_id = COALESCE(attached_reading_id, ?) WHERE id = ?",
                    params![v.value, v.text, v.bbox, v.source, new_state, val, u.username, now() as i64, flags_json, v.confidence, u.kind, v.attached_reading_id, id],
                )?;
                (*id, format!("{} [{src} {state}]", fmt_val(*val)))
            }
            None => {
                let held: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM order_values WHERE part_id = ?",
                    params![pid],
                    |r| r.get(0),
                )?;
                if held >= MAX_VALUES_PER_PART {
                    return Err(refuse(
                        StatusCode::CONFLICT,
                        "values_limit",
                        &format!(
                            "part '{pno}' holds at most {MAX_VALUES_PER_PART} values; '{}' would be one more",
                            v.key
                        ),
                    ));
                }
                tx.execute(
                    "INSERT INTO order_values(part_id, key, value, text, bbox_json, source, state, was, proposed_value, proposed_by, edited_by, edited_at, flags_json, confidence, edited_kind, attached_reading_id) VALUES (?,?,?,?,?,?,?,NULL,?,?,?,?,?,?,?,?)",
                    params![pid, v.key, v.value, v.text, v.bbox, v.source, new_state, v.value, u.username, u.username, now() as i64, flags_json, v.confidence, u.kind, v.attached_reading_id],
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
                "order #{oid} part '{pno}' {}: old {old_desc} new {} [{} {new_state}]{} {}",
                v.key,
                fmt_val(v.value),
                v.source,
                v.reading_id
                    .map_or(String::new(), |r| format!(" reading #{r}")),
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
                    source: actor_source(u),
                    agent: (u.kind == "agent").then_some(u.username.as_str()),
                    reason: &v.note,
                    against: value_fp(&v.key, v.value, &v.text, v.bbox.as_deref(), &v.source),
                    prev_state: old.as_ref().map(|o| o.5.as_str()),
                    who: &u.username,
                },
            )?;
            app.audit(
                &tx,
                &u.username,
                "order_decision",
                &format!(
                    "order #{oid} decision #{did} confirm part '{pno}' {} = {} by {}",
                    v.key,
                    fmt_val(v.value),
                    actor_source(u)
                ),
            )?;
        }
    }
    if let Some(es) = events.as_mut() {
        validate_value_write_events(&tx, oid, es, &values)?;
    }
    store_write_events(app, &tx, u, oid, &events)?;
    for pid in touched {
        recompute_part_state(&tx, pid)?;
    }
    tx.commit()?;
    ok(json!({ "changed": changed, "unchanged": unchanged }))
}

// ---------------------------------------------------------------- decisions

async fn confirm(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "confirm values")?;
    let res = confirm_do(&app, &headers, &u, oid, &raw);
    settle(&app, &u, oid, "confirm", body_text(&raw, "note"), res).await
}

/// Confirming a value, with or without flags, is a person's decision; the AI operator confirms only through its
/// own clean-reading rule in `put_values_do`.
fn agent_cannot_confirm(u: &User, what: &str) -> Result<(), ApiError> {
    if u.kind == "agent" {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "agent_cannot_confirm",
            &format!("the AI operator cannot {what}; a person decides"),
        ));
    }
    Ok(())
}

fn confirm_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, raw: &[u8]) -> Api {
    agent_cannot_confirm(u, "confirm values for a part or for the order")?;
    let body = request_json(headers, raw)?;
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
    agent_reason(u, &note)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "confirm values")?;
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
            "SELECT id, key, value, text, state, bbox_json, source FROM order_values WHERE part_id = ? ORDER BY key",
            params![pid],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                ))
            },
        )?;
        for (vid, key, val, text, state, bbox, src) in rows {
            if val.is_none() {
                missing += 1;
                continue;
            }
            if state == "CONFIRMED" {
                continue;
            }
            tx.execute(
                "UPDATE order_values SET state = 'CONFIRMED', edited_by = ?, edited_at = ?, edited_kind = ? WHERE id = ?",
                params![u.username, now() as i64, u.kind, vid],
            )?;
            let did = insert_decision(
                &tx,
                &NewDecision {
                    order: oid,
                    part: Some(*pid),
                    value: Some(vid),
                    kind: "confirm",
                    flag: None,
                    source: actor_source(u),
                    agent: (u.kind == "agent").then_some(u.username.as_str()),
                    reason: &note,
                    against: value_fp(&key, val, &text, bbox.as_deref(), &src),
                    prev_state: Some(&state),
                    who: &u.username,
                },
            )?;
            app.audit(
                &tx,
                &u.username,
                "order_decision",
                &format!(
                    "order #{oid} decision #{did} confirm part '{pno}' {key} = {} (was {state}) by {}",
                    fmt_val(val),
                    actor_source(u)
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
    let res = post_decision_do(&app, &headers, &u, oid, &raw);
    settle(&app, &u, oid, "decision", body_text(&raw, "reason"), res).await
}

fn post_decision_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, raw: &[u8]) -> Api {
    let body = request_json(headers, raw)?;
    let kind = text_field(&body, "kind", 16)?.unwrap_or_default();
    if !["confirm", "accept_flag", "mark_checked"].contains(&kind.as_str()) {
        return Err(unprocessable(
            "decision_kind",
            "'kind' must be confirm, accept_flag or mark_checked",
        ));
    }
    // the source follows from who is signed in; a body that names a different one is refused
    let source = actor_source(u);
    if let Some(claimed) = text_field(&body, "source", 8)?
        && claimed != source
    {
        return Err(unprocessable(
            "decision_source",
            &format!(
                "the source of a decision follows the signed-in identity ({source}); it cannot be set in the request"
            ),
        ));
    }
    let reason = text_field(&body, "reason", MAX_NOTE_CHARS)?.unwrap_or_default();
    let events = write_events(u, body.get("events"), false)?;
    if kind == "mark_checked" && u.kind == "agent" {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "agent_cannot_check",
            "only a person can mark an order as checked",
        ));
    }
    if kind == "confirm" || kind == "accept_flag" {
        agent_cannot_confirm(u, "confirm a value or accept a flag")?;
    }
    agent_reason(u, &reason)?;
    let agent = (u.kind == "agent").then_some(u.username.clone());
    let flag = text_field(&body, "flag", 64)?.filter(|s| !s.is_empty());
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "record decision")?;
    store_write_events(app, &tx, u, oid, &events)?;
    let mut d = NewDecision {
        order: oid,
        part: None,
        value: None,
        kind: &kind,
        flag: None,
        source,
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
        let (vid, val, text, state, bbox, src): (
            i64,
            Option<f64>,
            String,
            String,
            Option<String>,
            String,
        ) = tx
            .query_row(
                "SELECT id, value, text, state, bbox_json, source FROM order_values WHERE part_id = ? AND key = ?",
                params![pid, key],
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
        d.against = value_fp(&key, val, &text, bbox.as_deref(), &src);
        if kind == "confirm" && state == "CONFIRMED" {
            return Err(refuse(
                StatusCode::CONFLICT,
                "already_confirmed",
                &format!("part '{pno}' {key} is confirmed already; undo that decision first"),
            ));
        }
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
                "UPDATE order_values SET state = 'CONFIRMED', edited_by = ?, edited_at = ?, edited_kind = ? WHERE id = ?",
                params![u.username, now() as i64, u.kind, vid],
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
    let res = undo_do(&app, &headers, &u, oid, did, &raw);
    settle(&app, &u, oid, "undo", body_text(&raw, "note"), res).await
}

fn undo_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, did: i64, raw: &[u8]) -> Api {
    let body = if raw.is_empty() {
        json!({})
    } else {
        request_json(headers, raw)?
    };
    let note = text_field(&body, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    agent_reason(u, &note)?;
    let events = write_events(u, body.get("events"), false)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "undo decision")?;
    store_write_events(app, &tx, u, oid, &events)?;
    type Row = (
        String,
        Option<i64>,
        Option<i64>,
        String,
        Option<String>,
        bool,
        String,
    );
    let row: Option<Row> = tx
        .query_row(
            "SELECT kind, part_id, value_id, against_hash, prev_state, undone_at IS NOT NULL, decided_by FROM order_decisions WHERE id = ? AND order_id = ?",
            params![did, oid],
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
    let Some((kind, part, value, against, prev, undone, decided_by)) = row else {
        return Err(not_found());
    };
    // an AI operator undoes only its own decisions; a person's decision and a checked mark are a person's to undo
    if u.kind == "agent" && (kind == "mark_checked" || decided_by != u.username) {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "agent_cannot_undo",
            "an AI operator can undo only its own decisions; a person's decision or a checked mark is undone by a person",
        ));
    }
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
        type Cur = (String, Option<f64>, String, String, Option<String>, String);
        let cur: Option<Cur> = tx
            .query_row(
                "SELECT key, value, text, state, bbox_json, source FROM order_values WHERE id = ?",
                params![vid],
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
        // another confirmation of this very value that still counts keeps it confirmed
        let others = q_count(
            &tx,
            "SELECT COUNT(*) FROM order_decisions WHERE value_id = ? AND kind = 'confirm' AND undone_at IS NULL AND against_hash = ? AND id <> ?",
            params![vid, against, did],
        )?;
        // the value goes back to its earlier state only while it is still the value that was confirmed
        if let Some((key, val, text, state, bbox, src)) = cur
            && state == "CONFIRMED"
            && others == 0
            && value_fp(&key, val, &text, bbox.as_deref(), &src) == against
        {
            tx.execute(
                "UPDATE order_values SET state = ?, edited_by = ?, edited_at = ?, edited_kind = ? WHERE id = ?",
                params![prev, u.username, now() as i64, u.kind, vid],
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
        "SELECT d.id, d.part_id, p.no, d.value_id, v.key, v.value, v.text, d.kind, d.flag, d.source, d.agent, d.reason, d.against_hash, d.decided_by, d.decided_at, d.undone_by, d.undone_at, d.undo_note, v.bbox_json, v.source
         FROM order_decisions d LEFT JOIN order_parts p ON p.id = d.part_id LEFT JOIN order_values v ON v.id = d.value_id
         WHERE d.order_id = ?1 AND (?2 IS NULL OR d.id = ?2) ORDER BY d.id",
        params![oid, only],
        |r| {
            let key: Option<String> = r.get(4)?;
            let (value, text): (Option<f64>, Option<String>) = (r.get(5)?, r.get(6)?);
            let against: String = r.get(12)?;
            let bbox: Option<String> = r.get(18)?;
            let src: Option<String> = r.get(19)?;
            let current = match (&key, &text, &src) {
                (Some(k), Some(t), Some(sr)) => value_fp(k, value, t, bbox.as_deref(), sr),
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
        .find(|d| d["kind"] == "mark_checked" && d["effective"] == true)
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

// ---------------------------------------------------------------- compute (NFJ-1 L10)

use nf_core::order_compute as oc;

/// The confirmed state of the order as the compute reads it, and the hash that identifies it. Only CONFIRMED and
/// CORRECTED values are computed (a PROPOSED value is held by the compute itself).
fn load_compute_input(
    db: &Connection,
    oid: i64,
) -> Result<(Vec<oc::PartIn>, oc::OrderRules, String), ApiError> {
    let head = load_order(db, oid)?;
    // stored rules that cannot be read are an error, never the default rules
    let corrupt = || {
        refuse(
            StatusCode::UNPROCESSABLE_ENTITY,
            "order_rules_corrupt",
            "the order's stored rules are damaged and cannot be read; the compute is refused",
        )
    };
    let rules_v: Value = serde_json::from_str(&head.rules).map_err(|_| corrupt())?;
    // an object is required: null/arrays are not missing keys and must never fall back to provisional defaults
    if !rules_v.is_object() {
        return Err(corrupt());
    }
    let rules = oc::OrderRules::from_json(&rules_v).map_err(|m| {
        unprocessable(
            "order_rules",
            &format!("the order's rules cannot be used: {m}"),
        )
    })?;
    let rows = q_all(
        db,
        "SELECT id, no, name, shape, attach_part, qty, group_key FROM order_parts WHERE order_id = ? AND state <> 'EXCLUDED' ORDER BY no",
        params![oid],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        },
    )?;
    let mut parts = Vec::new();
    let mut fp = Vec::new();
    for (id, no, name, shape, attach, qty, group) in rows {
        let vals = q_all(
            db,
            "SELECT key, value, source, state FROM order_values WHERE part_id = ? ORDER BY key",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<f64>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )?;
        let mut p = oc::PartIn::new(&no, &shape);
        p.name = name.clone();
        p.qty = qty.clamp(1, 9999) as u32;
        p.attach = attach.clone().filter(|a| !a.is_empty());
        p.group = group.clone().filter(|g| !g.is_empty());
        for (key, value, source, state) in &vals {
            let st = match state.as_str() {
                "CONFIRMED" => oc::ValState::Confirmed,
                "CORRECTED" => oc::ValState::Corrected,
                "PROPOSED" => oc::ValState::Proposed,
                _ => oc::ValState::Missing,
            };
            p.values.insert(
                key.clone(),
                oc::Val {
                    value: *value,
                    state: st,
                    source: source.clone(),
                },
            );
        }
        fp.push(json!([
            no,
            name,
            shape,
            qty,
            attach,
            group,
            vals.iter()
                .map(|(k, v, s, st)| json!([k, v.map(f64::to_bits), s, st]))
                .collect::<Vec<_>>()
        ]));
        parts.push(p);
    }
    let hash = nf_core::sha256_hex(
        json!([nf_core::ENGINE_VERSION, rules_v, fp])
            .to_string()
            .as_bytes(),
    );
    Ok((parts, rules, hash))
}

fn counts_of(res: &oc::OrderResult) -> Value {
    let n = |s: &str| res.parts.iter().filter(|p| p.state == s).count();
    json!({ "parts": res.parts.len(), "computed": n("computed"), "held": n("held"), "held_missing": n("held_missing"),
            "refused": n("refused"), "not_computed": n("not_computed"),
            "dxf_files": res.parts.iter().filter(|p| p.dxf.is_some()).count() })
}

fn material_json(res: &oc::OrderResult) -> Vec<Value> {
    oc::material_rows(res)
        .into_iter()
        .map(|m| {
            json!({ "no": m.no, "name": m.name, "shape": m.shape,
                    "kind": if matches!(m.shape.as_str(), "purchased" | "other") { "purchased" } else { "piece" },
                    "thickness_mm": m.thickness_mm, "qty": m.qty, "width_mm": m.width_mm, "height_mm": m.height_mm,
                    "length_mm": m.length_mm, "unit_mass_kg": m.unit_mass_kg, "mass_calc_kg": m.mass_calc_kg,
                    "mass_bom_kg": m.mass_bom_kg, "note_en": m.note_en, "note_ja": m.note_ja })
        })
        .collect()
}

fn issues_json(res: &oc::OrderResult) -> Vec<Value> {
    oc::issues(res)
        .into_iter()
        .map(|i| json!({ "no": i.no, "name": i.name, "code": i.code, "message_en": i.message_en, "message_ja": i.message_ja }))
        .collect()
}

async fn compute(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "compute order")?;
    let note = if raw.is_empty() {
        String::new()
    } else {
        body_text(&raw, "note")
    };
    let res = compute_do(app.clone(), &headers, &u, oid, &note, &raw).await;
    settle(&app, &u, oid, "compute", note, res).await
}

async fn compute_do(
    app: Arc<App>,
    headers: &HeaderMap,
    u: &User,
    oid: i64,
    note: &str,
    raw: &[u8],
) -> Api {
    agent_reason(u, note)?;
    let events = if u.kind == "agent" {
        let body = if raw.is_empty() {
            None
        } else {
            Some(request_json(headers, raw)?)
        };
        write_events(u, body.as_ref().and_then(|b| b.get("events")), false)?
    } else {
        None
    };
    let (parts, rules, hash) = {
        let db = app.db();
        load_compute_input(&db, oid)?
    };
    if parts.is_empty() {
        return Err(unprocessable(
            "order_empty",
            "the order has no parts to compute",
        ));
    }
    // the maths runs outside the database lock
    let result = tokio::task::spawn_blocking(move || oc::compute_order(&parts, &rules))
        .await
        .map_err(|_| {
            fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the compute stopped unexpectedly",
            )
        })?;
    let counts = counts_of(&result);
    let body = oc::to_json(&result);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(&app, &tx, u, oid, "compute order")?;
    store_write_events(&app, &tx, u, oid, &events)?;
    let (_, _, now_hash) = load_compute_input(&tx, oid)?;
    if now_hash != hash {
        return Err(refuse(
            StatusCode::CONFLICT,
            "order_changed",
            "the order was edited while it was being computed; compute again",
        ));
    }
    let last: Option<(i64, String, String)> = tx
        .query_row(
            "SELECT id, input_hash, engine FROM order_results WHERE order_id = ? ORDER BY id DESC LIMIT 1",
            params![oid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((id, h, e)) = &last
        && *h == hash
        && e == nf_core::ENGINE_VERSION
    {
        let (stored, counts_s): (String, String) = tx.query_row(
            "SELECT result_json, counts_json FROM order_results WHERE id = ?",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let stored_v: Value = serde_json::from_str(&stored).unwrap_or(Value::Null);
        let res_obj: oc::OrderResult = result;
        return ok(
            json!({ "id": id, "supersedes": Value::Null, "unchanged": true, "engine": nf_core::ENGINE_VERSION,
                          "counts": serde_json::from_str::<Value>(&counts_s).unwrap_or(Value::Null),
                          "result": stored_v, "material": material_json(&res_obj), "issues": issues_json(&res_obj) }),
        );
    }
    let prev = last.map(|l| l.0);
    tx.execute(
        "INSERT INTO order_results(order_id, input_hash, engine, state, supersedes, result_json, counts_json, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?)",
        params![oid, hash, nf_core::ENGINE_VERSION, "CURRENT", prev, body.to_string(), counts.to_string(), u.username, now() as i64],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE order_results SET state = 'SUPERSEDED' WHERE order_id = ? AND id <> ? AND state <> 'SUPERSEDED'",
        params![oid, id],
    )?;
    app.audit(
        &tx,
        &u.username,
        "order_compute",
        &format!(
            "order #{oid} compute #{id} (supersedes {}) engine {}: {}",
            prev.map_or("none".to_string(), |p| format!("#{p}")),
            nf_core::ENGINE_VERSION,
            counts
        ),
    )?;
    tx.commit()?;
    ok(
        json!({ "id": id, "supersedes": prev, "unchanged": false, "engine": nf_core::ENGINE_VERSION, "counts": counts,
               "result": body, "material": material_json(&result), "issues": issues_json(&result) }),
    )
}

/// The current stored compute of an order and the chain of earlier ones. A stored result of another engine is
/// marked `recompute_required` (G21): it is shown for the record but is not current.
async fn get_results(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    guard_order(&app, &db, &u, oid, "view order results")?;
    let chain = q_all(
        &db,
        "SELECT id, state, supersedes, engine, counts_json, created_by, created_at FROM order_results WHERE order_id = ? ORDER BY id",
        params![oid],
        |r| {
            let engine: String = r.get(3)?;
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "state": r.get::<_, String>(1)?, "supersedes": r.get::<_, Option<i64>>(2)?,
                       "engine": engine, "recompute_required": !nf_core::engine_is_current(&engine),
                       "counts": serde_json::from_str::<Value>(&r.get::<_, String>(4)?).unwrap_or(Value::Null),
                       "created_by": r.get::<_, String>(5)?, "created_at": r.get::<_, i64>(6)? }),
            )
        },
    )?;
    let current = db
        .query_row(
            "SELECT id, result_json FROM order_results WHERE order_id = ? ORDER BY id DESC LIMIT 1",
            params![oid],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
        .map(|(id, j)| json!({ "id": id, "result": serde_json::from_str::<Value>(&j).unwrap_or(Value::Null) }));
    ok(json!({ "current": current, "chain": chain }))
}

#[cfg(test)]
mod reading_tests {
    use super::*;

    #[test]
    fn thousands_groups_are_strict() {
        // (token, value, ambiguous): only a 1-3 digit first group, no leading zero, then exact 3-digit groups.
        let table: &[(&str, f64, bool)] = &[
            ("1,500", 1500.0, false),
            ("12,345,678", 12_345_678.0, false),
            ("12,345.6", 12_345.6, false),
            ("0,500", 0.5, true),
            ("01,500", 1.5, true),
            ("1234,567", 1234.567, true),
            (",500", 0.5, true),
            ("1,5", 1.5, true),
            ("1,50", 1.5, true),
            ("1,5000", 1.5, true),
            ("0,5", 0.5, true),
        ];
        for &(tok, want, amb) in table {
            let (got, flag) = reading(tok).unwrap_or_else(|| panic!("{tok}: no reading"));
            assert_eq!((got, flag), (want, amb), "token {tok}");
        }
    }

    #[test]
    fn sign_and_full_width_comma_are_kept() {
        let r = numbers_in("-1,500");
        assert_eq!((r.len(), r[0].value, r[0].ambiguous), (1, -1500.0, false));
        let r = numbers_in("１２，３４５，６７８");
        assert_eq!(
            (r.len(), r[0].value, r[0].ambiguous),
            (1, 12_345_678.0, false)
        );
        let r = numbers_in("0,500");
        assert_eq!((r.len(), r[0].value, r[0].ambiguous), (1, 0.5, true));
    }
}
