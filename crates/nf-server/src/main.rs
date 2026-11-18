//! NEXUS FABRIC application server (single binary, offline-capable).
//!
//! Serves the web client and the JSON API. Everything the client shows comes
//! from this API; features that do not exist return nothing and are labelled
//! as not implemented in the client.

mod crypto;
mod membrane;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use nf_core::job::{self, Request};
use nf_core::{rel, Family, Record};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct App {
    db: Mutex<Connection>,
    key: [u8; 32],
    data_dir: PathBuf,
    started: u64,
}

type Ctx = State<Arc<App>>;

#[derive(Clone)]
struct User {
    id: i64,
    username: String,
    role: String,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn fail(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

fn ok(v: Value) -> Response {
    Json(v).into_response()
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY, username TEXT UNIQUE NOT NULL, role TEXT NOT NULL, salt TEXT NOT NULL, hash TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS sessions(token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL, expires_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS projects(id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS documents(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, filename TEXT NOT NULL, detected_type TEXT NOT NULL, size INTEGER NOT NULL, sha256 TEXT NOT NULL, extension_mismatch INTEGER NOT NULL, record_id TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY, project_id INTEGER NOT NULL, family TEXT NOT NULL, kind TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS calculations(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, family TEXT NOT NULL, state TEXT NOT NULL, closure_hash TEXT NOT NULL, result_json TEXT NOT NULL, replay_json TEXT, dxf_sha256 TEXT, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY, at INTEGER NOT NULL, username TEXT NOT NULL, action TEXT NOT NULL, detail TEXT NOT NULL);
";

impl App {
    fn audit(&self, db: &Connection, user: &str, action: &str, detail: &str) {
        let _ = db.execute("INSERT INTO audit(at, username, action, detail) VALUES (?,?,?,?)", params![now() as i64, user, action, detail]);
    }

    /// Append-only: an existing id is never overwritten.
    fn put_record(&self, db: &Connection, project: i64, r: &Record) -> rusqlite::Result<()> {
        let fam = serde_json::to_value(r.family).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        db.execute(
            "INSERT OR IGNORE INTO records(id, project_id, family, kind, json) VALUES (?,?,?,?,?)",
            params![r.id, project, fam, r.kind, serde_json::to_string(r).unwrap_or_default()],
        )?;
        Ok(())
    }

    fn blob_path(&self, sha: &str) -> PathBuf {
        self.data_dir.join("blobs").join(format!("{sha}.bin"))
    }

    fn store_blob(&self, bytes: &[u8]) -> Result<String, String> {
        let sha = nf_core::sha256_hex(bytes);
        let path = self.blob_path(&sha);
        if !path.exists() {
            std::fs::write(&path, crypto::seal(&self.key, bytes)?).map_err(|e| e.to_string())?;
        }
        Ok(sha)
    }

    fn load_blob(&self, sha: &str) -> Result<Vec<u8>, String> {
        let sealed = std::fs::read(self.blob_path(sha)).map_err(|e| e.to_string())?;
        let plain = crypto::open(&self.key, &sealed)?;
        if nf_core::sha256_hex(&plain) != sha {
            return Err("blob content does not match its hash".into());
        }
        Ok(plain)
    }
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').filter_map(|c| c.trim().strip_prefix("nf_session=")).next().map(String::from)
}

fn auth(app: &App, headers: &HeaderMap) -> Result<User, Response> {
    let deny = || fail(StatusCode::UNAUTHORIZED, "not signed in");
    let token = session_token(headers).ok_or_else(deny)?;
    let th = nf_core::sha256_hex(token.as_bytes());
    let db = app.db.lock().unwrap();
    db.query_row(
        "SELECT u.id, u.username, u.role FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > ?",
        params![th, now() as i64],
        |r| Ok(User { id: r.get(0)?, username: r.get(1)?, role: r.get(2)? }),
    )
    .optional()
    .ok()
    .flatten()
    .ok_or_else(deny)
}

fn require(user: &User, roles: &[&str]) -> Result<(), Response> {
    if roles.contains(&user.role.as_str()) {
        Ok(())
    } else {
        Err(fail(StatusCode::FORBIDDEN, "your role does not permit this action"))
    }
}

macro_rules! guard {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(resp) => return resp,
        }
    };
}

fn create_user(db: &Connection, username: &str, password: &str, role: &str) -> Result<i64, String> {
    let username = username.trim();
    if username.len() < 3 || username.len() > 64 || !username.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        return Err("username must be 3-64 characters: letters, digits, . _ -".into());
    }
    if password.chars().count() < 10 {
        return Err("password must have at least 10 characters".into());
    }
    if !["admin", "engineer", "viewer"].contains(&role) {
        return Err("role must be admin, engineer or viewer".into());
    }
    let salt: [u8; 16] = crypto::random_bytes();
    let hash = crypto::hash_password(password, &salt)?;
    db.execute("INSERT INTO users(username, role, salt, hash, created_at) VALUES (?,?,?,?,?)", params![username, role, hex::encode(salt), hash, now() as i64])
        .map_err(|e| if e.to_string().contains("UNIQUE") { "username already exists".to_string() } else { e.to_string() })?;
    Ok(db.last_insert_rowid())
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

// ---------- handlers ----------

async fn health(State(app): Ctx) -> Response {
    let db = app.db.lock().unwrap();
    let users: i64 = db.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap_or(0);
    ok(json!({ "status": "ok", "engine": nf_core::ENGINE_VERSION, "setup_required": users == 0, "uptime_seconds": now() - app.started,
               "build_notice": "Development build. Not validated. No output of this software may be used for manufacturing." }))
}

async fn setup(State(app): Ctx, Json(body): Json<Value>) -> Response {
    let db = app.db.lock().unwrap();
    let users: i64 = db.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap_or(1);
    if users != 0 {
        return fail(StatusCode::CONFLICT, "setup already completed");
    }
    match create_user(&db, &s(&body, "username"), &s(&body, "password"), "admin") {
        Ok(_) => {
            app.audit(&db, &s(&body, "username"), "setup", "first administrator created");
            ok(json!({ "created": true }))
        }
        Err(e) => fail(StatusCode::BAD_REQUEST, &e),
    }
}

async fn login(State(app): Ctx, Json(body): Json<Value>) -> Response {
    let (username, password) = (s(&body, "username"), s(&body, "password"));
    let db = app.db.lock().unwrap();
    let row: Option<(i64, String, String, String)> = db
        .query_row("SELECT id, role, salt, hash FROM users WHERE username = ?", params![username], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()
        .ok()
        .flatten();
    // hash even for unknown users so that timing does not reveal which names exist
    let (id, role, salt, hash) = row.unwrap_or((0, String::new(), "00000000000000000000000000000000".into(), String::new()));
    let computed = crypto::hash_password(&password, &hex::decode(&salt).unwrap_or_default()).unwrap_or_default();
    if id == 0 || !crypto::constant_time_eq(&computed, &hash) {
        app.audit(&db, &username, "login_failed", "");
        return fail(StatusCode::UNAUTHORIZED, "invalid username or password");
    }
    let token = hex::encode(crypto::random_bytes::<32>());
    let _ = db.execute("DELETE FROM sessions WHERE expires_at <= ?", params![now() as i64]);
    let _ = db.execute("INSERT INTO sessions(token_hash, user_id, expires_at) VALUES (?,?,?)", params![nf_core::sha256_hex(token.as_bytes()), id, (now() + 12 * 3600) as i64]);
    app.audit(&db, &username, "login", "");
    let cookie = format!("nf_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200");
    ([(header::SET_COOKIE, cookie)], Json(json!({ "username": username, "role": role }))).into_response()
}

async fn logout(State(app): Ctx, headers: HeaderMap) -> Response {
    if let Some(t) = session_token(&headers) {
        let db = app.db.lock().unwrap();
        let _ = db.execute("DELETE FROM sessions WHERE token_hash = ?", params![nf_core::sha256_hex(t.as_bytes())]);
    }
    ([(header::SET_COOKIE, "nf_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")], Json(json!({ "ok": true }))).into_response()
}

async fn me(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    ok(json!({ "id": u.id, "username": u.username, "role": u.role }))
}

async fn list_users(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let db = app.db.lock().unwrap();
    let mut st = db.prepare("SELECT id, username, role, created_at FROM users ORDER BY id").unwrap();
    let rows: Vec<Value> = st.query_map([], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "username": r.get::<_, String>(1)?, "role": r.get::<_, String>(2)?, "created_at": r.get::<_, i64>(3)? }))).unwrap().flatten().collect();
    ok(json!(rows))
}

async fn add_user(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let db = app.db.lock().unwrap();
    match create_user(&db, &s(&body, "username"), &s(&body, "password"), &s(&body, "role")) {
        Ok(id) => {
            app.audit(&db, &u.username, "user_created", &format!("{} ({})", s(&body, "username"), s(&body, "role")));
            ok(json!({ "id": id }))
        }
        Err(e) => fail(StatusCode::BAD_REQUEST, &e),
    }
}

async fn list_projects(State(app): Ctx, headers: HeaderMap) -> Response {
    guard!(auth(&app, &headers));
    let db = app.db.lock().unwrap();
    let mut st = db
        .prepare("SELECT p.id, p.name, p.description, p.created_by, p.created_at, (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id), (SELECT COUNT(*) FROM calculations c WHERE c.project_id = p.id) FROM projects p ORDER BY p.id DESC")
        .unwrap();
    let rows: Vec<Value> = st
        .query_map([], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "description": r.get::<_, String>(2)?, "created_by": r.get::<_, String>(3)?, "created_at": r.get::<_, i64>(4)?, "documents": r.get::<_, i64>(5)?, "calculations": r.get::<_, i64>(6)? })))
        .unwrap()
        .flatten()
        .collect();
    ok(json!(rows))
}

async fn add_project(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let name = s(&body, "name").trim().to_string();
    if name.is_empty() || name.len() > 200 {
        return fail(StatusCode::BAD_REQUEST, "project name is required (max 200 characters)");
    }
    let db = app.db.lock().unwrap();
    db.execute("INSERT INTO projects(name, description, created_by, created_at) VALUES (?,?,?,?)", params![name, s(&body, "description"), u.username, now() as i64]).unwrap();
    let id = db.last_insert_rowid();
    app.audit(&db, &u.username, "project_created", &format!("#{id} {name}"));
    ok(json!({ "id": id }))
}

fn calc_rows(db: &Connection, where_sql: &str, arg: i64) -> Vec<Value> {
    let sql = format!("SELECT id, project_id, family, state, closure_hash, created_by, created_at, replay_json IS NOT NULL FROM calculations {where_sql} ORDER BY id DESC LIMIT 200");
    let mut st = db.prepare(&sql).unwrap();
    let map = |r: &rusqlite::Row| Ok(json!({ "id": r.get::<_, i64>(0)?, "project_id": r.get::<_, i64>(1)?, "family": r.get::<_, String>(2)?, "state": r.get::<_, String>(3)?, "closure_hash": r.get::<_, String>(4)?, "created_by": r.get::<_, String>(5)?, "created_at": r.get::<_, i64>(6)?, "replayed": r.get::<_, bool>(7)? }));
    if where_sql.is_empty() { st.query_map([], map).unwrap().flatten().collect() } else { st.query_map(params![arg], map).unwrap().flatten().collect() }
}

async fn get_project(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    guard!(auth(&app, &headers));
    let db = app.db.lock().unwrap();
    let p: Option<Value> = db
        .query_row("SELECT id, name, description, created_by, created_at FROM projects WHERE id = ?", params![id], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "description": r.get::<_, String>(2)?, "created_by": r.get::<_, String>(3)?, "created_at": r.get::<_, i64>(4)? })))
        .optional()
        .ok()
        .flatten();
    let Some(mut p) = p else { return fail(StatusCode::NOT_FOUND, "project not found") };
    let mut st = db.prepare("SELECT id, filename, detected_type, size, sha256, extension_mismatch, created_by, created_at FROM documents WHERE project_id = ? ORDER BY id").unwrap();
    let docs: Vec<Value> = st
        .query_map(params![id], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "filename": r.get::<_, String>(1)?, "detected_type": r.get::<_, String>(2)?, "size": r.get::<_, i64>(3)?, "sha256": r.get::<_, String>(4)?, "extension_mismatch": r.get::<_, bool>(5)?, "created_by": r.get::<_, String>(6)?, "created_at": r.get::<_, i64>(7)? })))
        .unwrap()
        .flatten()
        .collect();
    let snapshot: Option<String> = db.query_row("SELECT id FROM records WHERE project_id = ? AND kind = 'snapshot' ORDER BY rowid DESC LIMIT 1", params![id], |r| r.get(0)).optional().ok().flatten();
    p["documents"] = json!(docs);
    p["calculations"] = json!(calc_rows(&db, "WHERE project_id = ?", id));
    p["snapshot"] = json!(snapshot);
    ok(p)
}

async fn upload(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, body: Bytes) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let filename: String = headers.get("x-filename").and_then(|v| v.to_str().ok()).unwrap_or("unnamed").chars().filter(|c| !c.is_control() && *c != '/' && *c != '\\').take(200).collect();
    let db = app.db.lock().unwrap();
    let exists: bool = db.query_row("SELECT COUNT(*) FROM projects WHERE id = ?", params![id], |r| r.get::<_, i64>(0)).unwrap_or(0) == 1;
    if !exists {
        return fail(StatusCode::NOT_FOUND, "project not found");
    }
    let detected = match membrane::admit(&body) {
        membrane::Admission::Accepted { detected_type } => detected_type,
        membrane::Admission::Rejected { reason } => {
            app.audit(&db, &u.username, "upload_rejected", &format!("project #{id} '{filename}': {reason}"));
            return fail(StatusCode::UNPROCESSABLE_ENTITY, &format!("file rejected at the membrane: {reason}"));
        }
    };
    let sha = match app.store_blob(&body) {
        Ok(x) => x,
        Err(e) => return fail(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    let mismatch = membrane::extension_mismatch(&filename, detected);
    let artifact = Record::new(Family::Source, "source_artifact", json!({ "sha256": sha, "bytes": body.len(), "detected_type": detected, "filename": filename, "extension_mismatch": mismatch, "uploaded_by": u.username }), vec![], None);
    app.put_record(&db, id, &artifact).unwrap();
    db.execute(
        "INSERT INTO documents(project_id, filename, detected_type, size, sha256, extension_mismatch, record_id, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?)",
        params![id, filename, detected, body.len() as i64, sha, mismatch, artifact.id, u.username, now() as i64],
    )
    .unwrap();
    // New immutable snapshot of the whole project (engulfment), linked to its predecessor.
    let mut st = db.prepare("SELECT record_id FROM documents WHERE project_id = ? ORDER BY record_id").unwrap();
    let members: Vec<String> = st.query_map(params![id], |r| r.get(0)).unwrap().flatten().collect();
    let prev: Option<String> = db.query_row("SELECT id FROM records WHERE project_id = ? AND kind = 'snapshot' ORDER BY rowid DESC LIMIT 1", params![id], |r| r.get(0)).optional().ok().flatten();
    let mut rels: Vec<nf_core::Rel> = members.iter().map(|m| rel("contains", m)).collect();
    if let Some(p) = &prev {
        rels.push(rel("predecessor", p));
    }
    let snap = Record::new(Family::Source, "snapshot", json!({ "project": id, "artifact_count": members.len() }), rels, None);
    app.put_record(&db, id, &snap).unwrap();
    app.audit(&db, &u.username, "upload_accepted", &format!("project #{id} '{filename}' {detected} {sha}"));
    ok(json!({ "sha256": sha, "detected_type": detected, "extension_mismatch": mismatch, "snapshot": snap.id,
               "interpretation": "not implemented: the file is stored encrypted and hashed; no drawing content is read" }))
}

async fn calculate(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let req: Request = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return fail(StatusCode::BAD_REQUEST, &format!("invalid request: {e}")),
    };
    let out = job::run(&req, &u.username);
    let db = app.db.lock().unwrap();
    let exists: bool = db.query_row("SELECT COUNT(*) FROM projects WHERE id = ?", params![id], |r| r.get::<_, i64>(0)).unwrap_or(0) == 1;
    if !exists {
        return fail(StatusCode::NOT_FOUND, "project not found");
    }
    for r in &out.records {
        app.put_record(&db, id, r).unwrap();
    }
    let dxf_sha = match &out.dxf {
        Some(t) => match app.store_blob(t.as_bytes()) {
            Ok(x) => Some(x),
            Err(e) => return fail(StatusCode::INTERNAL_SERVER_ERROR, &e),
        },
        None => None,
    };
    let family = serde_json::to_value(&req).ok().and_then(|v| v.get("family").and_then(|f| f.as_str().map(String::from))).unwrap_or_default();
    let state = serde_json::to_value(out.state).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    db.execute(
        "INSERT INTO calculations(project_id, family, state, closure_hash, result_json, dxf_sha256, created_by, created_at) VALUES (?,?,?,?,?,?,?,?)",
        params![id, family, state, out.closure_hash, serde_json::to_string(&out).unwrap_or_default(), dxf_sha, u.username, now() as i64],
    )
    .unwrap();
    let cid = db.last_insert_rowid();
    app.audit(&db, &u.username, "calculation", &format!("#{cid} project #{id} {family} -> {state}"));
    ok(json!({ "id": cid, "state": state }))
}

fn load_calc(db: &Connection, id: i64) -> Option<(Value, Option<String>, Option<String>, String, i64)> {
    db.query_row("SELECT result_json, replay_json, dxf_sha256, created_by, project_id FROM calculations WHERE id = ?", params![id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?))
    })
    .optional()
    .ok()
    .flatten()
    .map(|(j, rp, d, by, pid)| (serde_json::from_str(&j).unwrap_or(Value::Null), rp, d, by, pid))
}

async fn list_calcs(State(app): Ctx, headers: HeaderMap) -> Response {
    guard!(auth(&app, &headers));
    let db = app.db.lock().unwrap();
    ok(json!(calc_rows(&db, "", 0)))
}

async fn get_calc(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    guard!(auth(&app, &headers));
    let db = app.db.lock().unwrap();
    let Some((mut result, replay, dxf, by, pid)) = load_calc(&db, id) else { return fail(StatusCode::NOT_FOUND, "calculation not found") };
    result["id"] = json!(id);
    result["project_id"] = json!(pid);
    result["created_by"] = json!(by);
    result["dxf_available"] = json!(dxf.is_some());
    result["replay"] = replay.and_then(|r| serde_json::from_str::<Value>(&r).ok()).unwrap_or(Value::Null);
    ok(result)
}

async fn replay(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let db = app.db.lock().unwrap();
    let Some((result, _, _, by, pid)) = load_calc(&db, id) else { return fail(StatusCode::NOT_FOUND, "calculation not found") };
    let Ok(req) = serde_json::from_value::<Request>(result["request"].clone()) else { return fail(StatusCode::INTERNAL_SERVER_ERROR, "stored request unreadable") };
    let expected_hash = result["closure_hash"].as_str().unwrap_or("").to_string();
    let ids: Vec<String> = result["records"].as_array().map(|a| a.iter().filter_map(|r| r["id"].as_str().map(String::from)).collect()).unwrap_or_default();
    // also confirm that the stored records are intact (identity recomputed from content)
    let mut tampered = 0;
    for rid in &ids {
        let stored: Option<String> = db.query_row("SELECT json FROM records WHERE id = ?", params![rid], |r| r.get(0)).optional().ok().flatten();
        match stored.and_then(|j| serde_json::from_str::<Record>(&j).ok()) {
            Some(r) if r.verify_id() && &r.id == rid => {}
            _ => tampered += 1,
        }
    }
    let (matches, got) = job::replay_matches(&req, &by, &expected_hash, &ids);
    let report = json!({ "kind": "same_process_replay", "matches": matches && tampered == 0, "expected_closure_hash": expected_hash, "replayed_closure_hash": got,
                         "stored_records_checked": ids.len(), "stored_records_failing_identity_check": tampered, "run_by": u.username, "at": now(),
                         "limitation": "Replay ran in the same process and build. The independent-instance replay required before manufacturing release is not implemented." });
    let audit_rec = Record::new(Family::Assessment, "replay_audit", report.clone(), ids.last().map(|g| vec![rel("audits", g)]).unwrap_or_default(), None);
    app.put_record(&db, pid, &audit_rec).unwrap();
    db.execute("UPDATE calculations SET replay_json = ? WHERE id = ?", params![report.to_string(), id]).unwrap();
    app.audit(&db, &u.username, "replay_audit", &format!("calculation #{id}: {}", if matches && tampered == 0 { "match" } else { "MISMATCH" }));
    ok(report)
}

async fn get_dxf(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db.lock().unwrap();
    let Some((result, _, dxf, _, _)) = load_calc(&db, id) else { return fail(StatusCode::NOT_FOUND, "calculation not found") };
    let Some(sha) = dxf else { return fail(StatusCode::NOT_FOUND, "no DXF was produced for this calculation") };
    let bytes = match app.load_blob(&sha) {
        Ok(b) => b,
        Err(e) => return fail(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    let state = result["state"].as_str().unwrap_or("UNKNOWN");
    app.audit(&db, &u.username, "dxf_export", &format!("calculation #{id} state {state} sha {sha}"));
    let name = format!("NOT-RELEASED_{state}_calc{id}_{}.dxf", &sha[..8]);
    ([(header::CONTENT_TYPE, "application/dxf".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))], bytes).into_response()
}

async fn dashboard(State(app): Ctx, headers: HeaderMap) -> Response {
    guard!(auth(&app, &headers));
    let db = app.db.lock().unwrap();
    let n = |sql: &str| -> i64 { db.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    ok(json!({
        "projects": n("SELECT COUNT(*) FROM projects"),
        "documents": n("SELECT COUNT(*) FROM documents"),
        "calculations": n("SELECT COUNT(*) FROM calculations"),
        "review_required": n("SELECT COUNT(*) FROM calculations WHERE state = 'REVIEW_REQUIRED'"),
        "rejected": n("SELECT COUNT(*) FROM calculations WHERE state = 'REJECTED'"),
        "approved": n("SELECT COUNT(*) FROM calculations WHERE state = 'APPROVED'"),
        "records": n("SELECT COUNT(*) FROM records"),
        "recent": calc_rows(&db, "", 0).into_iter().take(8).collect::<Vec<_>>(),
        "uptime_seconds": now() - app.started,
        "engine": nf_core::ENGINE_VERSION,
    }))
}

async fn amoeba_summary(State(app): Ctx, headers: HeaderMap) -> Response {
    guard!(auth(&app, &headers));
    let db = app.db.lock().unwrap();
    let mut st = db.prepare("SELECT family, kind, COUNT(*) FROM records GROUP BY family, kind ORDER BY family, kind").unwrap();
    let kinds: Vec<Value> = st.query_map([], |r| Ok(json!({ "family": r.get::<_, String>(0)?, "kind": r.get::<_, String>(1)?, "count": r.get::<_, i64>(2)? }))).unwrap().flatten().collect();
    let mut st = db.prepare("SELECT kind, json FROM records WHERE family = 'activity'").unwrap();
    let mut work: std::collections::BTreeMap<String, (i64, i64)> = Default::default();
    for (kind, j) in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).unwrap().flatten() {
        let wu = serde_json::from_str::<Value>(&j).ok().and_then(|v| v["payload"]["work_units"].as_i64()).unwrap_or(0);
        let e = work.entry(kind).or_insert((0, 0));
        e.0 += 1;
        e.1 += wu;
    }
    let mut st = db.prepare("SELECT project_id, id FROM records WHERE kind = 'snapshot' ORDER BY rowid DESC LIMIT 10").unwrap();
    let snaps: Vec<Value> = st.query_map([], |r| Ok(json!({ "project_id": r.get::<_, i64>(0)?, "snapshot": r.get::<_, String>(1)? }))).unwrap().flatten().collect();
    ok(json!({
        "records_by_kind": kinds,
        "activities": work.into_iter().map(|(k, (n, wu))| json!({ "type": k, "distinct_activities": n, "declared_work_units": wu })).collect::<Vec<_>>(),
        "recent_snapshots": snaps,
        "implemented": ["content-addressed append-only records", "project snapshots with predecessor links", "typed relations", "release closure hash", "gate evaluation", "same-process replay audit", "membrane admission checks"],
        "not_implemented": ["round scheduler", "exploration probes", "hypothesis-driven investigation", "dependency propagation and scrutiny obligations", "coverage ledger", "lanes and overlays", "quarantine", "strategy switching", "independent-instance replay", "engineer approval"],
    }))
}

async fn audit_log(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let db = app.db.lock().unwrap();
    let mut st = db.prepare("SELECT at, username, action, detail FROM audit ORDER BY id DESC LIMIT 300").unwrap();
    let rows: Vec<Value> = st.query_map([], |r| Ok(json!({ "at": r.get::<_, i64>(0)?, "username": r.get::<_, String>(1)?, "action": r.get::<_, String>(2)?, "detail": r.get::<_, String>(3)? }))).unwrap().flatten().collect();
    ok(json!(rows))
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-store")], body).into_response()
}

fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/", get(|| async { asset("text/html; charset=utf-8", include_str!("../../../web/index.html")) }))
        .route("/app.js", get(|| async { asset("text/javascript; charset=utf-8", include_str!("../../../web/app.js")) }))
        .route("/styles.css", get(|| async { asset("text/css; charset=utf-8", include_str!("../../../web/styles.css")) }))
        .route("/api/health", get(health))
        .route("/api/setup", post(setup))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/users", get(list_users).post(add_user))
        .route("/api/projects", get(list_projects).post(add_project))
        .route("/api/projects/{id}", get(get_project))
        .route("/api/projects/{id}/documents", post(upload))
        .route("/api/projects/{id}/calculations", post(calculate))
        .route("/api/calculations", get(list_calcs))
        .route("/api/calculations/{id}", get(get_calc))
        .route("/api/calculations/{id}/replay", post(replay))
        .route("/api/calculations/{id}/dxf", get(get_dxf))
        .route("/api/dashboard", get(dashboard))
        .route("/api/amoeba/summary", get(amoeba_summary))
        .route("/api/audit", get(audit_log))
        .layer(DefaultBodyLimit::max(membrane::MAX_UPLOAD_BYTES + 1024))
        .with_state(app)
}

#[tokio::main]
async fn main() {
    let data_dir = PathBuf::from(std::env::var("NF_DATA_DIR").unwrap_or_else(|_| "data".into()));
    let addr = std::env::var("NF_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    std::fs::create_dir_all(data_dir.join("blobs")).expect("cannot create data directory");
    let key = crypto::load_or_create_key(&data_dir.join("storage.key")).expect("cannot load or create storage key");
    let db = Connection::open(data_dir.join("nexus-fabric.sqlite")).expect("cannot open database");
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;").ok();
    db.execute_batch(SCHEMA).expect("cannot create schema");
    let app = Arc::new(App { db: Mutex::new(db), key, data_dir, started: now() });
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("cannot bind listen address");
    println!("NEXUS FABRIC development build listening on http://{addr}  (data: {})", app.data_dir.display());
    println!("NOT VALIDATED - no output of this software may be used for manufacturing.");
    axum::serve(listener, router(app)).await.expect("server error");
}
