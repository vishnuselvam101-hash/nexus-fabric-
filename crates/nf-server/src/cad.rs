//! NF CAD phase 1 routes: a read-only view of a stored DXF (an uploaded document, or a job's own DXF through the
//! same gates as `GET /api/calculations/{id}/dxf/{part}`). All routes are GET, so the licence gate and the agent
//! identity see them as reads. Each open is audited. Files are capped by `nf_cad::MAX_BYTES`.

use crate::*;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use std::collections::HashMap;

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
                let row: Option<(i64, String, String)> = db
                    .query_row(
                        "SELECT project_id, detected_type, sha256 FROM documents WHERE id = ?",
                        [id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()?;
                let Some((project, detected, sha)) = row else {
                    return Err(fail(StatusCode::NOT_FOUND, "no such document"));
                };
                project_guard(app, &db, &u, project, "open a drawing")?;
                if detected != "dxf" {
                    return Err(fail_json(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        json!({ "error": "only a document detected as DXF can be opened in NF CAD", "code": "not_a_dxf" }),
                    ));
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

fn open_drawing(bytes: &[u8]) -> Result<nf_cad::Drawing, ApiError> {
    nf_cad::open(bytes).map_err(refused)
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

fn checks(bytes: &[u8]) -> Api {
    Ok(Json(json!({
        "context": "checked without the job's declared joint or conventions; a file that is not an NF output is expected to fail the rules that need them",
        "rules": nf_cad::quality_checks(bytes),
    }))
    .into_response())
}

async fn doc_model(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Query(q): Query<HashMap<String, String>>) -> Api {
    let b = load(&app, &headers, &Src::Doc(id), "cad_open").await?;
    model(&open_drawing(&b)?, &q)
}
async fn doc_checks(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    checks(&load(&app, &headers, &Src::Doc(id), "cad_check").await?)
}
async fn doc_measure(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Query(q): Query<HashMap<String, String>>) -> Api {
    let b = load(&app, &headers, &Src::Doc(id), "cad_measure").await?;
    measure(&open_drawing(&b)?, &q)
}
async fn doc_pick(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Query(q): Query<HashMap<String, String>>) -> Api {
    let b = load(&app, &headers, &Src::Doc(id), "cad_pick").await?;
    pick(&open_drawing(&b)?, &q)
}
async fn calc_model(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>, Query(q): Query<HashMap<String, String>>) -> Api {
    let b = load(&app, &headers, &Src::Calc(id, part), "cad_open").await?;
    model(&open_drawing(&b)?, &q)
}
async fn calc_checks(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>) -> Api {
    checks(&load(&app, &headers, &Src::Calc(id, part), "cad_check").await?)
}
async fn calc_measure(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>, Query(q): Query<HashMap<String, String>>) -> Api {
    let b = load(&app, &headers, &Src::Calc(id, part), "cad_measure").await?;
    measure(&open_drawing(&b)?, &q)
}
async fn calc_pick(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>, Query(q): Query<HashMap<String, String>>) -> Api {
    let b = load(&app, &headers, &Src::Calc(id, part), "cad_pick").await?;
    pick(&open_drawing(&b)?, &q)
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
