//! NF CAD phase 1 routes: a read-only view of a stored DXF (an uploaded document, or a job's own DXF through the
//! same gates as `GET /api/calculations/{id}/dxf/{part}`). All routes are GET, so the licence gate and the agent
//! identity see them as reads. Each open is audited. Files are capped by `nf_cad::MAX_BYTES`.

use crate::*;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

/// Parsed drawings kept per process, bounded. Keyed by the SHA-256 of the file bytes.
const CACHE_ENTRIES: usize = 16;

/// A file parsed once: the drawing (or its refusal) and the quality gate's rules.
struct Cached {
    drawing: Result<nf_cad::Drawing, nf_cad::Refusal>,
    checks: Vec<nf_cad::RuleCheck>,
}

type Entries = VecDeque<(String, Arc<Cached>)>;

fn cache() -> &'static Mutex<Entries> {
    static CACHE: OnceLock<Mutex<Entries>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// The cache, recovered if an earlier holder panicked (the entries stay valid).
fn locked() -> std::sync::MutexGuard<'static, Entries> {
    cache().lock().unwrap_or_else(|e| e.into_inner())
}

/// Parses and gates `bytes` on the blocking pool, reusing the cached result for the same SHA-256.
async fn parsed(bytes: Vec<u8>) -> Result<Arc<Cached>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let key = nf_core::sha256_hex(&bytes);
        if let Some(hit) = locked().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()) {
            return hit;
        }
        let entry = Arc::new(Cached { drawing: nf_cad::open(&bytes), checks: nf_cad::quality_checks(&bytes) });
        let mut c = locked();
        c.push_back((key, entry.clone()));
        while c.len() > CACHE_ENTRIES {
            c.pop_front();
        }
        entry
    })
    .await
    .map_err(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "the drawing could not be read"))
}

fn drawing_of(c: &Cached) -> Result<&nf_cad::Drawing, ApiError> {
    c.drawing.as_ref().map_err(|e| refused(e.clone()))
}

/// A stored drawing: a document of a project, or a job's DXF part.
enum Src {
    Doc(i64),
    Calc(i64, String),
}

/// Loads the bytes behind `src` after the same authorisation the existing routes use, and audits the open.
async fn load(app: &Arc<App>, headers: &HeaderMap, src: &Src, action: &str) -> Result<Vec<u8>, ApiError> {
    let (bytes, label) = match src {
        Src::Doc(id) => {
            let (u, sha) = {
                let db = app.db();
                let u = auth_db(&db, app, headers)?;
                let row: Option<(i64, String, String, i64)> = db
                    .query_row(
                        "SELECT project_id, detected_type, sha256, size FROM documents WHERE id = ?",
                        [id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()?;
                let Some((project, detected, sha, size)) = row else {
                    return Err(fail(StatusCode::NOT_FOUND, "no such document"));
                };
                project_guard(app, &db, &u, project, "open a drawing")?;
                if detected != "dxf" {
                    return Err(fail_json(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        json!({ "error": "only a document detected as DXF can be opened in NF CAD", "code": "not_a_dxf" }),
                    ));
                }
                // Refuse on the recorded size before the blob is read and decrypted.
                if size < 0 || size as u64 > nf_cad::MAX_BYTES as u64 {
                    return Err(refused(nf_cad::Refusal {
                        code: "too_large",
                        message: format!("file is {size} bytes; the limit is {}", nf_cad::MAX_BYTES),
                    }));
                }
                (u, sha)
            };
            let bytes = storage::load_blob(&app.data_dir, &app.key, &sha)
                .map_err(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "the stored file cannot be read"))?;
            let _ = u;
            (bytes, format!("document {id}"))
        }
        Src::Calc(id, part) => {
            let resp = get_dxf(State(app.clone()), headers.clone(), Path((*id, part.clone()))).await?;
            let bytes = axum::body::to_bytes(resp.into_body(), nf_cad::MAX_BYTES)
                .await
                .map_err(|_| fail(StatusCode::PAYLOAD_TOO_LARGE, "the file is over the NF CAD limit"))?;
            (bytes.to_vec(), format!("job {id} part {part}"))
        }
    };
    let db = app.db();
    let u = auth_db(&db, app, headers)?;
    if let Err(e) = app.audit(&db, &u.username, action, &format!("{label} ({} bytes)", bytes.len())) {
        elog!("nf-server: could not audit a CAD open: {e}");
    }
    Ok(bytes)
}

fn refused(e: nf_cad::Refusal) -> ApiError {
    let status = match e.code {
        "too_large" => StatusCode::PAYLOAD_TOO_LARGE,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    fail_json(status, json!({ "error": e.message, "code": e.code }))
}

fn q_map(q: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
    move |k| q.get(k).cloned()
}

fn model(d: &nf_cad::Drawing, q: &HashMap<String, String>) -> Api {
    let offset: usize = q.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let limit: usize = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(500).clamp(1, 2000);
    let page: Vec<Value> = d.entities.iter().skip(offset).take(limit).map(nf_cad::entity_json).collect();
    Ok(Json(json!({
        "version": d.version,
        "units": d.units,
        "extents": d.extents,
        "layers": d.layers,
        "entity_count": d.entities.len(),
        "offset": offset,
        "entities": page,
        "warnings": d.warnings,
    }))
    .into_response())
}

fn measure(d: &nf_cad::Drawing, q: &HashMap<String, String>) -> Api {
    let m = nf_cad::measure_from_query(&q_map(q)).map_err(|e| fail(StatusCode::UNPROCESSABLE_ENTITY, &e))?;
    let values = d.measure(&m).map_err(|e| fail(StatusCode::UNPROCESSABLE_ENTITY, &e))?;
    Ok(Json(json!({
        "values": values.iter().map(|v| json!({ "value": v.value, "unit": v.unit, "from": v.from })).collect::<Vec<_>>(),
    }))
    .into_response())
}

fn pick(d: &nf_cad::Drawing, q: &HashMap<String, String>) -> Api {
    let at = q.get("at").and_then(|s| nf_cad::parse_point(s)).ok_or_else(|| fail(StatusCode::UNPROCESSABLE_ENTITY, "'at' must be x,y"))?;
    let tol = q.get("tol").and_then(|s| s.parse::<f64>().ok()).filter(|t| t.is_finite() && *t >= 0.0).unwrap_or(1.0);
    let hit = d.pick(at, tol).map(nf_cad::entity_json);
    Ok(Json(json!({ "entity": hit })).into_response())
}

fn checks(c: &Cached) -> Api {
    Ok(Json(json!({
        "context": "checked without the job's declared joint or conventions; a file that is not an NF output is expected to fail the rules that need them",
        "rules": c.checks,
    }))
    .into_response())
}

async fn doc_model(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Query(q): Query<HashMap<String, String>>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Doc(id), "cad_open").await?).await?;
    model(drawing_of(&c)?, &q)
}
async fn doc_checks(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Doc(id), "cad_check").await?).await?;
    checks(&c)
}
async fn doc_measure(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Query(q): Query<HashMap<String, String>>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Doc(id), "cad_measure").await?).await?;
    measure(drawing_of(&c)?, &q)
}
async fn doc_pick(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Query(q): Query<HashMap<String, String>>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Doc(id), "cad_pick").await?).await?;
    pick(drawing_of(&c)?, &q)
}
async fn calc_model(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>, Query(q): Query<HashMap<String, String>>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Calc(id, part), "cad_open").await?).await?;
    model(drawing_of(&c)?, &q)
}
async fn calc_checks(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Calc(id, part), "cad_check").await?).await?;
    checks(&c)
}
async fn calc_measure(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>, Query(q): Query<HashMap<String, String>>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Calc(id, part), "cad_measure").await?).await?;
    measure(drawing_of(&c)?, &q)
}
async fn calc_pick(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>, Query(q): Query<HashMap<String, String>>) -> Api {
    let c = parsed(load(&app, &headers, &Src::Calc(id, part), "cad_pick").await?).await?;
    pick(drawing_of(&c)?, &q)
}

/// The viewer's files: the page and its modules, embedded at build time (no build step, no CDN).
const STATIC: &[(&str, &str, &str)] = &[
    ("style.css", "text/css; charset=utf-8", include_str!("../../../web/cad/style.css")),
    ("main.js", "text/javascript; charset=utf-8", include_str!("../../../web/js/cad/main.js")),
    ("view.js", "text/javascript; charset=utf-8", include_str!("../../../web/js/cad/view.js")),
    ("measure.js", "text/javascript; charset=utf-8", include_str!("../../../web/js/cad/measure.js")),
    ("edit.js", "text/javascript; charset=utf-8", include_str!("../../../web/js/cad/edit.js")),
    ("i18n.js", "text/javascript; charset=utf-8", include_str!("../../../web/js/i18n/cad.js")),
];

async fn page() -> Response {
    let mut r = Html(include_str!("../../../web/cad/index.html")).into_response();
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    r
}

async fn static_file(Path(name): Path<String>) -> Response {
    match STATIC.iter().find(|(n, _, _)| *n == name) {
        Some((_, ct, body)) => {
            let mut r = (*body).into_response();
            r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
            r
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Maximum operations in one edit request.
const MAX_EDIT_OPS: usize = 50;

/// True when a file carries the NF CAD edit marker (anywhere in its text).
pub(crate) fn is_marked(bytes: &[u8]) -> bool {
    nf_cad::edit::is_edited(bytes)
}

/// A file that was edited in NF CAD is never served as a certified pattern.
pub(crate) fn refuse_if_edited(bytes: &[u8]) -> Result<(), ApiError> {
    if is_marked(bytes) {
        return Err(fail_json(
            StatusCode::CONFLICT,
            json!({ "error": "this file was edited in NF CAD: it is not a verified NF pattern and cannot be served as one", "code": "edited_not_certified" }),
        ));
    }
    Ok(())
}

struct Doc {
    project: i64,
    detected: String,
    sha: String,
    filename: String,
    size: i64,
}

fn doc_row(db: &rusqlite::Connection, id: i64) -> Result<Doc, ApiError> {
    db.query_row(
        "SELECT project_id, detected_type, sha256, filename, size FROM documents WHERE id = ?",
        [id],
        |r| Ok(Doc { project: r.get(0)?, detected: r.get(1)?, sha: r.get(2)?, filename: r.get(3)?, size: r.get(4)? }),
    )
    .optional()?
    .ok_or_else(|| fail(StatusCode::NOT_FOUND, "no such document"))
}

/// An agent identity may read but never edit: the refusal is audited.
fn refuse_agent(app: &App, db: &rusqlite::Connection, u: &User, what: &str) -> ApiError {
    if let Err(e) = app.audit(db, &u.username, "cad_edit_refused", what) {
        elog!("nf-server: could not audit a refused CAD edit: {e}");
    }
    fail_json(StatusCode::FORBIDDEN, json!({ "error": "an agent may not edit a drawing: only a person can", "code": "agent_forbidden" }))
}

fn edited_name(name: &str) -> String {
    let stem = name.strip_suffix(".dxf").or_else(|| name.strip_suffix(".DXF")).unwrap_or(name);
    clip(&format!("{stem} (NF CAD edit).dxf"), 200)
}

fn edit_refused(e: nf_cad::edit::EditError) -> ApiError {
    fail_json(StatusCode::UNPROCESSABLE_ENTITY, json!({ "error": e.message, "code": e.code }))
}

#[derive(serde::Deserialize)]
struct EditBody {
    ops: Vec<nf_cad::edit::Op>,
}

/// POST: one person-only edit. Creates a NEW document version; the parent document and its blob are not touched.
async fn doc_edit(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, body: axum::body::Bytes) -> Api {
    let (u, doc) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        if u.kind == "agent" {
            return Err(refuse_agent(&app, &db, &u, &format!("document {id}")));
        }
        require(&u, &["admin", "engineer"])?;
        let doc = doc_row(&db, id)?;
        project_guard(&app, &db, &u, doc.project, "edit a drawing")?;
        if doc.detected != "dxf" {
            return Err(fail_json(StatusCode::UNSUPPORTED_MEDIA_TYPE, json!({ "error": "only a DXF can be edited in NF CAD", "code": "not_a_dxf" })));
        }
        if doc.size < 0 || doc.size as u64 > nf_cad::MAX_BYTES as u64 {
            return Err(refused(nf_cad::Refusal { code: "too_large", message: format!("file is {} bytes; the limit is {}", doc.size, nf_cad::MAX_BYTES) }));
        }
        (u, doc)
    };
    let req: EditBody = serde_json::from_slice(&body)
        .map_err(|e| fail_json(StatusCode::UNPROCESSABLE_ENTITY, json!({ "error": format!("bad edit: {e}"), "code": "bad_request" })))?;
    if req.ops.is_empty() || req.ops.len() > MAX_EDIT_OPS {
        return Err(fail_json(StatusCode::UNPROCESSABLE_ENTITY, json!({ "error": format!("1 to {MAX_EDIT_OPS} operations per edit"), "code": "bad_request" })));
    }
    let ops_json = serde_json::to_string(&req.ops).map_err(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "the edit could not be recorded"))?;
    let n_ops = req.ops.len();
    let bytes = storage::load_blob(&app.data_dir, &app.key, &doc.sha)
        .map_err(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "the stored file cannot be read"))?;
    let ops = req.ops;
    let (new_bytes, checks) = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, Vec<nf_cad::RuleCheck>), nf_cad::edit::EditError> {
        let b = nf_cad::edit::apply(&bytes, &ops)?;
        let c = nf_cad::quality_checks(&b);
        Ok((b, c))
    })
    .await
    .map_err(|_| fail(StatusCode::INTERNAL_SERVER_ERROR, "the edit could not run"))?
    .map_err(edit_refused)?;
    if new_bytes.len() > nf_cad::MAX_BYTES {
        return Err(refused(nf_cad::Refusal { code: "too_large", message: format!("the edited file would be over the limit of {} bytes", nf_cad::MAX_BYTES) }));
    }
    if !matches!(crate::membrane::admit(&new_bytes), crate::membrane::Admission::Accepted { detected_type } if detected_type == "dxf") {
        return Err(fail(StatusCode::UNPROCESSABLE_ENTITY, "the edited file was not admitted as a DXF"));
    }
    let filename = edited_name(&doc.filename);
    let (child, child_sha, snapshot) = {
        let mut db = app.db();
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        project_guard(&app, &tx, &u, doc.project, "edit a drawing")?;
        let stored = register_document(&app, &tx, &u, doc.project, &filename, &new_bytes, "dxf")?;
        tx.execute("UPDATE documents SET cad_edited = 1 WHERE id = ?", [stored.id])?;
        tx.execute(
            "INSERT INTO cad_versions(child_document, parent_document, parent_sha256, child_sha256, kind, ops_json, author, created_at) VALUES (?,?,?,?,?,?,?,?)",
            rusqlite::params![stored.id, id, doc.sha, stored.sha, "edit", ops_json, u.username, now() as i64],
        )?;
        app.audit(&tx, &u.username, "cad_edit", &format!("document #{id} -> #{} parent {} child {} ops {n_ops}", stored.id, doc.sha, stored.sha))?;
        tx.commit()?;
        (stored.id, stored.sha, stored.snapshot)
    };
    ok(json!({
        "document_id": child,
        "sha256": child_sha,
        "parent_document": id,
        "parent_sha256": doc.sha,
        "snapshot": snapshot,
        "edited": true,
        "marker": nf_cad::edit::MARK_TEXT,
        "checks": checks,
    }))
}

/// POST: undo. The edited version is kept; the current version becomes its parent again. Person-only.
async fn doc_undo(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let mut db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    if u.kind == "agent" {
        return Err(refuse_agent(&app, &db, &u, &format!("undo document {id}")));
    }
    require(&u, &["admin", "engineer"])?;
    let doc = doc_row(&db, id)?;
    project_guard(&app, &db, &u, doc.project, "undo a drawing edit")?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // the version row is read inside the transaction, so the undo is recorded against what was current
    let row: Option<(i64, String, String)> = tx
        .query_row(
            "SELECT parent_document, parent_sha256, child_sha256 FROM cad_versions WHERE child_document = ? AND kind = 'edit' ORDER BY id DESC LIMIT 1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((parent, psha, csha)) = row else {
        return Err(fail_json(StatusCode::CONFLICT, json!({ "error": "nothing to undo: this document is not an NF CAD edit", "code": "not_an_edit" })));
    };
    tx.execute(
        "INSERT INTO cad_versions(child_document, parent_document, parent_sha256, child_sha256, kind, ops_json, author, created_at) VALUES (?,?,?,?,?,?,?,?)",
        rusqlite::params![id, parent, psha, csha, "undo", "[]", u.username, now() as i64],
    )?;
    app.audit(&tx, &u.username, "cad_undo", &format!("document #{id} undone; current is parent #{parent} {psha}"))?;
    tx.commit()?;
    ok(json!({ "undone": id, "revert_to": parent, "sha256": psha }))
}

/// GET: the version chain of a document: its parent, and the edits made from it. Redo is opening a child.
async fn doc_versions(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let doc = doc_row(&db, id)?;
    project_guard(&app, &db, &u, doc.project, "read drawing versions")?;
    let parent: Option<i64> = db
        .query_row("SELECT parent_document FROM cad_versions WHERE child_document = ? AND kind = 'edit' ORDER BY id DESC LIMIT 1", [id], |r| r.get(0))
        .optional()?;
    let mut stmt = db.prepare("SELECT child_document, child_sha256, author, created_at FROM cad_versions WHERE parent_document = ? AND kind = 'edit' ORDER BY id")?;
    let children: Vec<Value> = stmt
        .query_map([id], |r| Ok(json!({ "document_id": r.get::<_, i64>(0)?, "sha256": r.get::<_, String>(1)?, "author": r.get::<_, String>(2)?, "created_at": r.get::<_, i64>(3)? })))?
        .collect::<Result<_, _>>()?;
    ok(json!({ "document_id": id, "cad_edited": doc.size >= 0 && is_cad_edited(&db, id), "parent": parent, "children": children }))
}

/// True when a document is an NF CAD edit (the flag set on an edited version, or a version row for it). The original
/// document a version was made from is NOT an edit: it stays certifiable. Shared helper for the export and nest routes.
pub(crate) fn is_cad_edited(db: &rusqlite::Connection, id: i64) -> bool {
    let flagged = db.query_row("SELECT cad_edited FROM documents WHERE id = ?", [id], |r| r.get::<_, i64>(0)).map(|v| v == 1).unwrap_or(false);
    let versioned = db.query_row("SELECT COUNT(*) FROM cad_versions WHERE child_document = ?", [id], |r| r.get::<_, i64>(0)).map(|n| n > 0).unwrap_or(false);
    flagged || versioned
}

/// The same question for a stored file by its hash: a certified pattern whose bytes are an edited document's are refused.
pub(crate) fn refuse_if_edited_sha(db: &rusqlite::Connection, sha: &str) -> Result<(), ApiError> {
    let edited: i64 = db.query_row("SELECT COUNT(*) FROM documents WHERE sha256 = ? AND cad_edited = 1", [sha], |r| r.get(0))?;
    if edited > 0 {
        return Err(fail_json(StatusCode::CONFLICT, json!({ "error": "this file is an NF CAD edit: it is not a verified NF pattern and cannot be served as one", "code": "edited_not_certified" })));
    }
    Ok(())
}

/// The NF CAD routes, merged into the router (so the security and licence layers cover them).
pub(crate) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/cad", get(page))
        .route("/cad/static/{name}", get(static_file))
        .route("/api/cad/documents/{id}", get(doc_model))
        .route("/api/cad/documents/{id}/checks", get(doc_checks))
        .route("/api/cad/documents/{id}/measure", get(doc_measure))
        .route("/api/cad/documents/{id}/pick", get(doc_pick))
        .route("/api/cad/calculations/{id}/{part}", get(calc_model))
        .route("/api/cad/calculations/{id}/{part}/checks", get(calc_checks))
        .route("/api/cad/calculations/{id}/{part}/measure", get(calc_measure))
        .route("/api/cad/calculations/{id}/{part}/pick", get(calc_pick))
        .route("/api/cad/documents/{id}/edit", post(doc_edit))
        .route("/api/cad/documents/{id}/undo", post(doc_undo))
        .route("/api/cad/documents/{id}/versions", get(doc_versions))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get(bytes: Vec<u8>) -> Arc<Cached> {
        parsed(bytes).await.ok().expect("parse")
    }

    fn dxf(i: usize) -> Vec<u8> {
        format!("0\nSECTION\n2\nENTITIES\n0\nPOINT\n10\n{i}\n20\n0\n0\nENDSEC\n0\nEOF\n").into_bytes()
    }

    #[tokio::test]
    async fn cache_reuses_by_sha_and_stays_bounded() {
        let first = get(dxf(1)).await;
        let again = get(dxf(1)).await;
        assert!(Arc::ptr_eq(&first, &again), "same bytes must hit the cache");
        for i in 0..(CACHE_ENTRIES + 5) {
            get(dxf(1000 + i)).await;
        }
        let len = locked().len();
        assert!(len <= CACHE_ENTRIES, "cache holds {len} entries");
    }
}
