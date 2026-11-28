//! NF CAD phase 1 routes: a read-only view of a stored DXF (an uploaded document, or a job's own DXF through the
//! same gates as `GET /api/calculations/{id}/dxf/{part}`). All routes are GET, so the licence gate and the agent
//! identity see them as reads. Each open is audited. Files are capped by `nf_cad::MAX_BYTES`.

use crate::*;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
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
