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
    cookie_secure: bool,
    key_outside_data_dir: bool,
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
CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY, username TEXT UNIQUE NOT NULL, role TEXT NOT NULL, salt TEXT NOT NULL, hash TEXT NOT NULL, active INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS sessions(token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL, expires_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS projects(id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS documents(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, filename TEXT NOT NULL, detected_type TEXT NOT NULL, size INTEGER NOT NULL, sha256 TEXT NOT NULL, extension_mismatch INTEGER NOT NULL, record_id TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY, project_id INTEGER NOT NULL, family TEXT NOT NULL, kind TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS calculations(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, family TEXT NOT NULL, state TEXT NOT NULL, closure_hash TEXT NOT NULL, result_json TEXT NOT NULL, replay_json TEXT, dxf_sha256 TEXT, supersedes INTEGER, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS obligations(id INTEGER PRIMARY KEY, origin_calc INTEGER NOT NULL, target_calc INTEGER NOT NULL, record_id TEXT NOT NULL, kind TEXT NOT NULL, label TEXT NOT NULL, direction TEXT NOT NULL, via TEXT NOT NULL, required_check TEXT NOT NULL, state TEXT NOT NULL, note TEXT NOT NULL, resolved_by TEXT, resolved_at INTEGER, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY, at INTEGER NOT NULL, username TEXT NOT NULL, action TEXT NOT NULL, detail TEXT NOT NULL, prev TEXT, hash TEXT);
CREATE TABLE IF NOT EXISTS lessons(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, kind TEXT NOT NULL, family TEXT NOT NULL, parameter TEXT NOT NULL, wrong_value REAL, right_value REAL, detail TEXT NOT NULL, source_calc INTEGER NOT NULL, record_id TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS advisories(id INTEGER PRIMARY KEY, calc_id INTEGER NOT NULL, lesson_id INTEGER NOT NULL, message TEXT NOT NULL, state TEXT NOT NULL, note TEXT NOT NULL, resolved_by TEXT, resolved_at INTEGER);
CREATE TABLE IF NOT EXISTS project_members(project_id INTEGER NOT NULL, user_id INTEGER NOT NULL, PRIMARY KEY(project_id, user_id));
CREATE TABLE IF NOT EXISTS login_failures(username TEXT NOT NULL, at INTEGER NOT NULL);
";

fn audit_hash(prev: &str, at: i64, user: &str, action: &str, detail: &str) -> String {
    nf_core::sha256_hex(serde_json::to_string(&json!([prev, at, user, action, detail])).unwrap_or_default().as_bytes())
}

impl App {
    /// The database lock. A panic in one request must not take the service
    /// down for everyone, so a poisoned lock is recovered instead of propagated.
    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Append to the audit log. Each entry carries the hash of its predecessor,
    /// so removing or altering an entry is detectable (`/api/audit/verify`).
    fn audit(&self, db: &Connection, user: &str, action: &str, detail: &str) {
        let prev: String = db.query_row("SELECT hash FROM audit ORDER BY id DESC LIMIT 1", [], |r| r.get::<_, Option<String>>(0)).ok().flatten().unwrap_or_else(|| "genesis".into());
        let at = now() as i64;
        let hash = audit_hash(&prev, at, user, action, detail);
        let _ = db.execute("INSERT INTO audit(at, username, action, detail, prev, hash) VALUES (?,?,?,?,?,?)", params![at, user, action, detail, prev, hash]);
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

fn auth(app: &App, headers: &HeaderMap) -> Result<User, Box<Response>> {
    let deny = || fail(StatusCode::UNAUTHORIZED, "not signed in");
    let token = session_token(headers).ok_or_else(deny)?;
    let th = nf_core::sha256_hex(token.as_bytes());
    let db = app.db();
    db.query_row(
        "SELECT u.id, u.username, u.role FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > ? AND u.active = 1",
        params![th, now() as i64],
        |r| Ok(User { id: r.get(0)?, username: r.get(1)?, role: r.get(2)? }),
    )
    .optional()
    .ok()
    .flatten()
    .ok_or_else(|| Box::new(deny()))
}

fn require(user: &User, roles: &[&str]) -> Result<(), Box<Response>> {
    if roles.contains(&user.role.as_str()) {
        Ok(())
    } else {
        Err(Box::new(fail(StatusCode::FORBIDDEN, "your role does not permit this action")))
    }
}

/// SQL condition restricting `project_id` to projects the user may see.
/// Built from an integer id only, never from request text.
fn scope(u: &User) -> String {
    if u.role == "admin" { "1=1".into() } else { format!("project_id IN (SELECT project_id FROM project_members WHERE user_id = {})", u.id) }
}

fn can_access(db: &Connection, u: &User, project: i64) -> bool {
    u.role == "admin" || db.query_row("SELECT COUNT(*) FROM project_members WHERE project_id = ? AND user_id = ?", params![project, u.id], |r| r.get::<_, i64>(0)).unwrap_or(0) == 1
}

/// Hide the existence of things the caller may not see.
fn not_found() -> Response {
    fail(StatusCode::NOT_FOUND, "not found")
}

macro_rules! guard {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(resp) => return *resp,
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
    let db = app.db();
    let users: i64 = db.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap_or(0);
    ok(json!({ "status": "ok", "engine": nf_core::ENGINE_VERSION, "setup_required": users == 0, "uptime_seconds": now() - app.started,
               "build_notice": "Development build. Not validated. No output of this software may be used for manufacturing." }))
}

async fn setup(State(app): Ctx, Json(body): Json<Value>) -> Response {
    let db = app.db();
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
    let db = app.db();
    // Throttling: 5 failures per account in 15 minutes, 40 failures overall per minute.
    let t = now() as i64;
    let _ = db.execute("DELETE FROM login_failures WHERE at < ?", params![t - 900]);
    let per_user: i64 = db.query_row("SELECT COUNT(*) FROM login_failures WHERE username = ?", params![username], |r| r.get(0)).unwrap_or(0);
    let overall: i64 = db.query_row("SELECT COUNT(*) FROM login_failures WHERE at > ?", params![t - 60], |r| r.get(0)).unwrap_or(0);
    if per_user >= 5 || overall >= 40 {
        app.audit(&db, &username, "login_throttled", "");
        return fail(StatusCode::TOO_MANY_REQUESTS, "too many failed sign-in attempts; try again in 15 minutes");
    }
    let row: Option<(i64, String, String, String)> = db
        .query_row("SELECT id, role, salt, hash FROM users WHERE username = ? AND active = 1", params![username], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()
        .ok()
        .flatten();
    // hash even for unknown users so that timing does not reveal which names exist
    let (id, role, salt, hash) = row.unwrap_or((0, String::new(), "00000000000000000000000000000000".into(), String::new()));
    let computed = crypto::hash_password(&password, &hex::decode(&salt).unwrap_or_default()).unwrap_or_default();
    if id == 0 || !crypto::constant_time_eq(&computed, &hash) {
        let _ = db.execute("INSERT INTO login_failures(username, at) VALUES (?,?)", params![username, t]);
        app.audit(&db, &username, "login_failed", "");
        return fail(StatusCode::UNAUTHORIZED, "invalid username or password");
    }
    let _ = db.execute("DELETE FROM login_failures WHERE username = ?", params![username]);
    let token = hex::encode(crypto::random_bytes::<32>());
    let _ = db.execute("DELETE FROM sessions WHERE expires_at <= ?", params![now() as i64]);
    let _ = db.execute("INSERT INTO sessions(token_hash, user_id, expires_at) VALUES (?,?,?)", params![nf_core::sha256_hex(token.as_bytes()), id, (now() + 12 * 3600) as i64]);
    app.audit(&db, &username, "login", "");
    let cookie = format!("nf_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200{}", if app.cookie_secure { "; Secure" } else { "" });
    ([(header::SET_COOKIE, cookie)], Json(json!({ "username": username, "role": role }))).into_response()
}

async fn logout(State(app): Ctx, headers: HeaderMap) -> Response {
    if let Some(t) = session_token(&headers) {
        let db = app.db();
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
    let db = app.db();
    let mut st = db.prepare("SELECT id, username, role, created_at, active FROM users ORDER BY id").unwrap();
    let rows: Vec<Value> = st.query_map([], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "username": r.get::<_, String>(1)?, "role": r.get::<_, String>(2)?, "created_at": r.get::<_, i64>(3)?, "active": r.get::<_, bool>(4)? }))).unwrap().flatten().collect();
    ok(json!(rows))
}

async fn add_user(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let db = app.db();
    match create_user(&db, &s(&body, "username"), &s(&body, "password"), &s(&body, "role")) {
        Ok(id) => {
            app.audit(&db, &u.username, "user_created", &format!("{} ({})", s(&body, "username"), s(&body, "role")));
            ok(json!({ "id": id }))
        }
        Err(e) => fail(StatusCode::BAD_REQUEST, &e),
    }
}

/// Change one's own password. All sessions of the account are revoked.
async fn change_password(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    let (current, new) = (s(&body, "current"), s(&body, "new"));
    if new.chars().count() < 10 {
        return fail(StatusCode::BAD_REQUEST, "new password must have at least 10 characters");
    }
    let db = app.db();
    let (salt, hash): (String, String) = match db.query_row("SELECT salt, hash FROM users WHERE id = ?", params![u.id], |r| Ok((r.get(0)?, r.get(1)?))) {
        Ok(x) => x,
        Err(_) => return fail(StatusCode::UNAUTHORIZED, "not signed in"),
    };
    let computed = crypto::hash_password(&current, &hex::decode(&salt).unwrap_or_default()).unwrap_or_default();
    if !crypto::constant_time_eq(&computed, &hash) {
        app.audit(&db, &u.username, "password_change_failed", "");
        return fail(StatusCode::FORBIDDEN, "current password is incorrect");
    }
    let nsalt: [u8; 16] = crypto::random_bytes();
    let nhash = match crypto::hash_password(&new, &nsalt) {
        Ok(h) => h,
        Err(e) => return fail(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    let _ = db.execute("UPDATE users SET salt = ?, hash = ? WHERE id = ?", params![hex::encode(nsalt), nhash, u.id]);
    let _ = db.execute("DELETE FROM sessions WHERE user_id = ?", params![u.id]);
    app.audit(&db, &u.username, "password_changed", "all sessions revoked");
    ok(json!({ "changed": true, "sessions_revoked": true }))
}

/// Activate or deactivate an account (administrator). Deactivation revokes its sessions.
async fn set_user_active(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let Some(active) = body.get("active").and_then(|v| v.as_bool()) else { return fail(StatusCode::BAD_REQUEST, "active must be true or false") };
    if id == u.id && !active {
        return fail(StatusCode::BAD_REQUEST, "you cannot deactivate your own account");
    }
    let db = app.db();
    if db.execute("UPDATE users SET active = ? WHERE id = ?", params![active, id]).unwrap_or(0) == 0 {
        return not_found();
    }
    if !active {
        let _ = db.execute("DELETE FROM sessions WHERE user_id = ?", params![id]);
    }
    app.audit(&db, &u.username, if active { "user_activated" } else { "user_deactivated" }, &format!("user #{id}"));
    ok(json!({ "active": active }))
}

async fn add_member(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let db = app.db();
    let uid: Option<i64> = db.query_row("SELECT id FROM users WHERE username = ?", params![s(&body, "username")], |r| r.get(0)).optional().ok().flatten();
    let exists: bool = db.query_row("SELECT COUNT(*) FROM projects WHERE id = ?", params![id], |r| r.get::<_, i64>(0)).unwrap_or(0) == 1;
    let (Some(uid), true) = (uid, exists) else { return not_found() };
    let remove = body.get("remove").and_then(|v| v.as_bool()).unwrap_or(false);
    if remove {
        let _ = db.execute("DELETE FROM project_members WHERE project_id = ? AND user_id = ?", params![id, uid]);
    } else {
        let _ = db.execute("INSERT OR IGNORE INTO project_members(project_id, user_id) VALUES (?,?)", params![id, uid]);
    }
    app.audit(&db, &u.username, if remove { "member_removed" } else { "member_added" }, &format!("project #{id} user {}", s(&body, "username")));
    ok(json!({ "ok": true }))
}

async fn audit_verify(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let db = app.db();
    let mut st = db.prepare("SELECT id, at, username, action, detail, prev, hash FROM audit ORDER BY id").unwrap();
    type AuditRow = (i64, i64, String, String, String, Option<String>, Option<String>);
    let rows: Vec<AuditRow> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))).unwrap().flatten().collect();
    let mut prev = "genesis".to_string();
    let mut broken: Option<i64> = None;
    for (id, at, user, action, detail, p, h) in &rows {
        let expect = audit_hash(&prev, *at, user, action, detail);
        if p.as_deref() != Some(prev.as_str()) || h.as_deref() != Some(expect.as_str()) {
            broken = Some(*id);
            break;
        }
        prev = expect;
    }
    ok(json!({ "intact": broken.is_none(), "entries": rows.len(), "first_broken_entry": broken,
               "limitation": "Detects alteration or removal of entries inside the chain. Someone with write access to the database file could rebuild the whole chain or truncate its tail; an external anchor for the latest hash is not implemented." }))
}

async fn security_status(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    ok(json!({
        "transport": "This server speaks plain HTTP. TLS must be provided by a terminating proxy; none is built in.",
        "cookie_secure_flag": app.cookie_secure,
        "session_lifetime_hours": 12,
        "password_hashing": "Argon2id (library defaults), per-user random salt",
        "login_throttling": "5 failed attempts per account per 15 minutes; 40 failed attempts overall per minute",
        "multi_factor_authentication": false,
        "blob_encryption": "AES-256-GCM, one key for all blobs",
        "storage_key_outside_data_directory": app.key_outside_data_dir,
        "key_rotation": false,
        "database_encrypted_at_rest": false,
        "project_isolation": "Per-project membership enforced on every project, job, file, obligation and summary endpoint; administrators see all projects",
        "anti_forgery": "State-changing API requests require the X-NF-Request header and a same-origin Origin; session cookie is SameSite=Strict",
        "audit_log": "Hash-chained; verify with /api/audit/verify",
        "sandboxed_parsers": "Not applicable yet: uploaded files are stored, never parsed",
    }))
}

async fn list_projects(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    let mut st = db
        .prepare(&format!("SELECT p.id, p.name, p.description, p.created_by, p.created_at, (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id), (SELECT COUNT(*) FROM calculations c WHERE c.project_id = p.id) FROM projects p WHERE {} ORDER BY p.id DESC", scope(&u).replace("project_id IN", "p.id IN")))
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
    let db = app.db();
    db.execute("INSERT INTO projects(name, description, created_by, created_at) VALUES (?,?,?,?)", params![name, s(&body, "description"), u.username, now() as i64]).unwrap();
    let id = db.last_insert_rowid();
    let _ = db.execute("INSERT OR IGNORE INTO project_members(project_id, user_id) VALUES (?,?)", params![id, u.id]);
    app.audit(&db, &u.username, "project_created", &format!("#{id} {name}"));
    ok(json!({ "id": id }))
}

fn calc_rows(db: &Connection, where_sql: &str) -> Vec<Value> {
    let sql = format!("SELECT id, project_id, family, state, closure_hash, created_by, created_at, replay_json IS NOT NULL, (SELECT COUNT(*) FROM calculations n WHERE n.supersedes = calculations.id), (SELECT COUNT(*) FROM obligations o WHERE o.target_calc = calculations.id AND o.state = 'open') FROM calculations {where_sql} ORDER BY id DESC LIMIT 200");
    let mut st = db.prepare(&sql).unwrap();
    let map = |r: &rusqlite::Row| Ok(json!({ "id": r.get::<_, i64>(0)?, "project_id": r.get::<_, i64>(1)?, "family": r.get::<_, String>(2)?, "state": r.get::<_, String>(3)?, "closure_hash": r.get::<_, String>(4)?, "created_by": r.get::<_, String>(5)?, "created_at": r.get::<_, i64>(6)?, "replayed": r.get::<_, bool>(7)?, "superseded": r.get::<_, i64>(8)? > 0, "open_obligations": r.get::<_, i64>(9)? }));
    st.query_map([], map).unwrap().flatten().collect()
}

async fn get_project(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    if !can_access(&db, &u, id) {
        return not_found();
    }
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
    p["calculations"] = json!(calc_rows(&db, &format!("WHERE project_id = {id}")));
    let mut st = db.prepare("SELECT u.username FROM project_members m JOIN users u ON u.id = m.user_id WHERE m.project_id = ? ORDER BY u.username").unwrap();
    p["members"] = json!(st.query_map(params![id], |r| r.get::<_, String>(0)).unwrap().flatten().collect::<Vec<_>>());
    p["snapshot"] = json!(snapshot);
    ok(p)
}

async fn upload(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, body: Bytes) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let filename: String = headers.get("x-filename").and_then(|v| v.to_str().ok()).unwrap_or("unnamed").chars().filter(|c| !c.is_control() && *c != '/' && *c != '\\').take(200).collect();
    let db = app.db();
    let exists: bool = db.query_row("SELECT COUNT(*) FROM projects WHERE id = ?", params![id], |r| r.get::<_, i64>(0)).unwrap_or(0) == 1;
    if !exists || !can_access(&db, &u, id) {
        return not_found();
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

/// Persist a finished job: records (append-only), encrypted DXF blobs, calculation row.
fn persist_calc(app: &App, db: &Connection, project: i64, out: &job::JobOutput, user: &str, supersedes: Option<i64>) -> Result<i64, String> {
    for r in &out.records {
        app.put_record(db, project, r).map_err(|e| e.to_string())?;
    }
    for (_, text) in &out.dxf {
        app.store_blob(text.as_bytes())?;
    }
    let family = serde_json::to_value(&out.request).ok().and_then(|v| v.get("family").and_then(|f| f.as_str().map(String::from))).unwrap_or_default();
    let state = serde_json::to_value(out.state).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    db.execute(
        "INSERT INTO calculations(project_id, family, state, closure_hash, result_json, supersedes, created_by, created_at) VALUES (?,?,?,?,?,?,?,?)",
        params![project, family, state, out.closure_hash, serde_json::to_string(out).unwrap_or_default(), supersedes, user, now() as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(db.last_insert_rowid())
}

// ---------- learning from mistakes ----------
// A lesson is an immutable record of something that went wrong in a project:
// a value an engineer had to correct, a result the verifiers rejected, a
// stored result that failed its integrity check. Lessons never change a value
// and never approve anything. They are matched against every later job in the
// same project; a match raises an advisory that keeps the job in REVIEW
// REQUIRED until an engineer acknowledges it with a reason.

fn family_of(req: &Request) -> String {
    serde_json::to_value(req).ok().and_then(|v| v.get("family").and_then(|f| f.as_str().map(String::from))).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn record_lesson(app: &App, db: &Connection, project: i64, kind: &str, family: &str, parameter: &str, wrong: Option<f64>, right: Option<f64>, detail: &str, source_calc: i64, user: &str) {
    let rec = Record::new(Family::Finding, "lesson", json!({ "kind": kind, "family": family, "parameter": parameter, "wrong_value": wrong, "right_value": right, "detail": detail, "source_calculation": source_calc, "recorded_by": user, "at": now() }), vec![], None);
    let _ = app.put_record(db, project, &rec);
    let _ = db.execute(
        "INSERT INTO lessons(project_id, kind, family, parameter, wrong_value, right_value, detail, source_calc, record_id, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        params![project, kind, family, parameter, wrong, right, detail, source_calc, rec.id, user, now() as i64],
    );
    app.audit(db, user, "lesson_recorded", &format!("project #{project} {kind} {family} {parameter}"));
}

/// Compare a new job with the lessons of its project and raise advisories.
fn raise_advisories(db: &Connection, project: i64, calc_id: i64, req: &Request) -> usize {
    let family = family_of(req);
    let params_now = req.numeric_params();
    let req_json = serde_json::to_string(req).unwrap_or_default();
    let mut st = db.prepare("SELECT id, kind, parameter, wrong_value, right_value, detail, source_calc FROM lessons WHERE project_id = ? AND family = ? ORDER BY id").unwrap();
    type LessonRow = (i64, String, String, Option<f64>, Option<f64>, String, i64);
    let rows: Vec<LessonRow> = st.query_map(params![project, family], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))).unwrap().flatten().collect();
    let mut n = 0;
    for (lid, kind, parameter, wrong, right, detail, source) in rows {
        let message = match kind.as_str() {
            "corrected_value" => {
                let now_v = params_now.iter().find(|(k, _)| *k == parameter).map(|(_, v)| *v);
                match (now_v, wrong) {
                    (Some(v), Some(w)) if (v - w).abs() <= 1e-9 * w.abs().max(1.0) => format!("{parameter} = {v} repeats a value that was corrected in this project (job #{source}: {w} -> {}; reason: {detail}). Confirm that {v} is right this time.", right.unwrap_or(f64::NAN)),
                    _ => continue,
                }
            }
            "engine_failure" | "integrity_failure" if detail == req_json => format!("This exact request produced a {} before in this project (job #{source}).", if kind == "engine_failure" { "result rejected by verification" } else { "stored result that failed its integrity check" }),
            _ => continue,
        };
        let _ = db.execute("INSERT INTO advisories(calc_id, lesson_id, message, state, note) VALUES (?,?,?, 'open', '')", params![calc_id, lid, message]);
        n += 1;
    }
    n
}

async fn list_lessons(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    let mut st = db.prepare(&format!("SELECT id, project_id, kind, family, parameter, wrong_value, right_value, detail, source_calc, created_by, created_at, (SELECT COUNT(*) FROM advisories a WHERE a.lesson_id = lessons.id) FROM lessons WHERE {} ORDER BY id DESC LIMIT 300", scope(&u))).unwrap();
    let rows: Vec<Value> = st
        .query_map([], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "project_id": r.get::<_, i64>(1)?, "kind": r.get::<_, String>(2)?, "family": r.get::<_, String>(3)?, "parameter": r.get::<_, String>(4)?, "wrong_value": r.get::<_, Option<f64>>(5)?, "right_value": r.get::<_, Option<f64>>(6)?,
                              "detail": r.get::<_, String>(7)?, "source_calc": r.get::<_, i64>(8)?, "created_by": r.get::<_, String>(9)?, "created_at": r.get::<_, i64>(10)?, "times_matched": r.get::<_, i64>(11)? })))
        .unwrap()
        .flatten()
        .collect();
    ok(json!(rows))
}

async fn acknowledge_advisory(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let reason = s(&body, "reason").trim().to_string();
    if reason.len() < 5 {
        return fail(StatusCode::BAD_REQUEST, "a reason is required");
    }
    let db = app.db();
    let row: Option<(i64, String)> = db.query_row("SELECT calc_id, state FROM advisories WHERE id = ?", params![id], |r| Ok((r.get(0)?, r.get(1)?))).optional().ok().flatten();
    let Some((calc_id, state)) = row else { return not_found() };
    let project: i64 = db.query_row("SELECT project_id FROM calculations WHERE id = ?", params![calc_id], |r| r.get(0)).unwrap_or(-1);
    if !can_access(&db, &u, project) {
        return not_found();
    }
    if state != "open" {
        return fail(StatusCode::CONFLICT, "advisory is not open");
    }
    let att = Record::new(Family::Attestation, "advisory_acknowledgement", json!({ "advisory": id, "calculation": calc_id, "reason": reason, "by": u.username, "role": u.role, "at": now() }), vec![], None);
    let _ = app.put_record(&db, project, &att);
    db.execute("UPDATE advisories SET state = 'acknowledged', note = ?, resolved_by = ?, resolved_at = ? WHERE id = ? AND state = 'open'", params![reason, u.username, now() as i64, id]).unwrap();
    app.audit(&db, &u.username, "advisory_acknowledged", &format!("#{id} job #{calc_id}: {reason}"));
    ok(json!({ "state": "acknowledged" }))
}

async fn calculate(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let req: Request = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return fail(StatusCode::BAD_REQUEST, &format!("invalid request: {e}")),
    };
    let out = job::run(&req, &u.username);
    let db = app.db();
    let exists: bool = db.query_row("SELECT COUNT(*) FROM projects WHERE id = ?", params![id], |r| r.get::<_, i64>(0)).unwrap_or(0) == 1;
    if !exists || !can_access(&db, &u, id) {
        return not_found();
    }
    match persist_calc(&app, &db, id, &out, &u.username, None) {
        Ok(cid) => {
            app.audit(&db, &u.username, "calculation", &format!("#{cid} project #{id} -> {:?}", out.state));
            let advisories = raise_advisories(&db, id, cid, &req);
            if out.state == nf_core::ReleaseState::Rejected && !out.patterns.is_empty() {
                // the engine produced a pattern that its own verification rejected: remember the exact request
                record_lesson(&app, &db, id, "engine_failure", &family_of(&req), "", None, None, &serde_json::to_string(&req).unwrap_or_default(), cid, &u.username);
            }
            ok(json!({ "id": cid, "state": out.state, "advisories": advisories }))
        }
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

struct Calc {
    result: Value,
    replay: Option<Value>,
    created_by: String,
    project: i64,
    supersedes: Option<i64>,
    superseded_by: Option<i64>,
}

fn load_calc(db: &Connection, id: i64) -> Option<Calc> {
    let row = db
        .query_row("SELECT result_json, replay_json, created_by, project_id, supersedes FROM calculations WHERE id = ?", params![id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, Option<i64>>(4)?))
        })
        .optional()
        .ok()
        .flatten()?;
    let superseded_by: Option<i64> = db.query_row("SELECT id FROM calculations WHERE supersedes = ? ORDER BY id LIMIT 1", params![id], |r| r.get(0)).optional().ok().flatten();
    Some(Calc { result: serde_json::from_str(&row.0).unwrap_or(Value::Null), replay: row.1.and_then(|r| serde_json::from_str(&r).ok()), created_by: row.2, project: row.3, supersedes: row.4, superseded_by })
}

fn obligation_rows(db: &Connection, where_sql: &str, arg: i64) -> Vec<Value> {
    let sql = format!("SELECT id, origin_calc, target_calc, kind, label, direction, via, required_check, state, note, resolved_by, resolved_at, created_at FROM obligations {where_sql} ORDER BY id DESC LIMIT 500");
    let mut st = db.prepare(&sql).unwrap();
    let map = |r: &rusqlite::Row| {
        Ok(json!({ "id": r.get::<_, i64>(0)?, "origin_calc": r.get::<_, i64>(1)?, "target_calc": r.get::<_, i64>(2)?, "kind": r.get::<_, String>(3)?, "label": r.get::<_, String>(4)?,
                   "direction": r.get::<_, String>(5)?, "via": r.get::<_, String>(6)?, "required_check": r.get::<_, String>(7)?, "state": r.get::<_, String>(8)?, "note": r.get::<_, String>(9)?,
                   "resolved_by": r.get::<_, Option<String>>(10)?, "resolved_at": r.get::<_, Option<i64>>(11)?, "created_at": r.get::<_, i64>(12)? }))
    };
    if where_sql.contains('?') { st.query_map(params![arg], map).unwrap().flatten().collect() } else { st.query_map([], map).unwrap().flatten().collect() }
}

/// Gates G18 and G19 depend on state that changes after the job ran; they are
/// evaluated here, at read time, and the release state is recomputed from the
/// resulting table. Nothing in this function can produce APPROVED: G16 stays
/// NOT_IMPLEMENTED.
fn effective_gates(db: &Connection, id: i64, c: &Calc) -> (Vec<nf_core::Gate>, nf_core::ReleaseState) {
    use nf_core::GateResult as R;
    let mut gates: Vec<nf_core::Gate> = serde_json::from_value(c.result["gates"].clone()).unwrap_or_default();
    let open: i64 = db.query_row("SELECT COUNT(*) FROM obligations WHERE target_calc = ? AND state = 'open'", params![id], |r| r.get(0)).unwrap_or(0);
    let total: i64 = db.query_row("SELECT COUNT(*) FROM obligations WHERE target_calc = ?", params![id], |r| r.get(0)).unwrap_or(0);
    for g in gates.iter_mut() {
        if g.id == "G18" {
            g.result = R::Inconclusive;
            g.detail = if open > 0 {
                format!("{open} of {total} mandatory scrutiny obligations are OPEN. They cannot be closed by budget or time, only by evidence or an engineer's decision.")
            } else if total > 0 {
                format!("All {total} scrutiny obligations are resolved. The propagation rule table is provisional ({}), so closure cannot be certified.", nf_core::deps::RULE_TABLE_VERSION)
            } else {
                format!("No scrutiny obligations recorded for this result. The propagation rule table is provisional ({}).", nf_core::deps::RULE_TABLE_VERSION)
            };
        }
        if g.id == "G19" {
            match &c.replay {
                None => {}
                Some(r) if r["matches"] == true => {
                    g.result = R::Inconclusive;
                    g.detail = format!("Replay audit ({}) reproduced the release closure. Audits A1-A4 are not implemented, and the replay used the same machine and build.", r["kind"].as_str().unwrap_or("replay"));
                }
                Some(_) => {
                    g.result = R::Fail;
                    g.detail = "Replay audit did NOT reproduce the stored result. The result is rejected and must be investigated.".into();
                }
            }
        }
    }
    let adv_open: i64 = db.query_row("SELECT COUNT(*) FROM advisories WHERE calc_id = ? AND state = 'open'", params![id], |r| r.get(0)).unwrap_or(0);
    let adv_all: i64 = db.query_row("SELECT COUNT(*) FROM advisories WHERE calc_id = ?", params![id], |r| r.get(0)).unwrap_or(0);
    let lessons_n: i64 = db.query_row("SELECT COUNT(*) FROM lessons WHERE project_id = ?", params![c.project], |r| r.get(0)).unwrap_or(0);
    gates.push(nf_core::Gate {
        id: "G20".into(),
        name: "known_mistakes".into(),
        critical: true,
        result: if adv_open > 0 { R::Inconclusive } else { R::Pass },
        detail: if adv_open > 0 {
            format!("{adv_open} advisory(ies) open: this job repeats something that went wrong before in this project. An engineer must acknowledge each with a reason.")
        } else if adv_all > 0 {
            format!("{adv_all} advisory(ies) raised and acknowledged by an engineer.")
        } else {
            format!("No match among the {lessons_n} lesson(s) recorded in this project. Lessons cover only mistakes already seen here.")
        },
    });
    if c.superseded_by.is_some() {
        gates.push(nf_core::Gate { id: "G00".into(), name: "current_revision".into(), critical: true, result: R::Fail, detail: format!("Superseded by job #{} after a correction. This result must not be used.", c.superseded_by.unwrap_or(0)) });
    }
    let state = nf_core::release_state(&gates);
    (gates, state)
}

async fn list_calcs(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    ok(json!(calc_rows(&db, &format!("WHERE {}", scope(&u)))))
}

async fn get_calc(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    let Some(c) = load_calc(&db, id).filter(|c| can_access(&db, &u, c.project)) else { return not_found() };
    let (gates, state) = effective_gates(&db, id, &c);
    let mut result = c.result.clone();
    result["id"] = json!(id);
    result["project_id"] = json!(c.project);
    result["created_by"] = json!(c.created_by);
    result["gates"] = json!(gates);
    result["state"] = json!(state);
    result["replay"] = c.replay.clone().unwrap_or(Value::Null);
    result["supersedes"] = json!(c.supersedes);
    result["superseded_by"] = json!(c.superseded_by);
    result["obligations"] = json!(obligation_rows(&db, "WHERE target_calc = ?", id));
    let mut st = db.prepare("SELECT id, lesson_id, message, state, note, resolved_by, resolved_at FROM advisories WHERE calc_id = ? ORDER BY id").unwrap();
    result["advisories"] = json!(st.query_map(params![id], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "lesson_id": r.get::<_, i64>(1)?, "message": r.get::<_, String>(2)?, "state": r.get::<_, String>(3)?, "note": r.get::<_, String>(4)?, "resolved_by": r.get::<_, Option<String>>(5)?, "resolved_at": r.get::<_, Option<i64>>(6)? }))).unwrap().flatten().collect::<Vec<_>>());
    result["correctable_parameters"] = json!(serde_json::from_value::<Request>(c.result["request"].clone()).ok().filter(|r| matches!(r, Request::HeaderWithBranch { .. })).map(|r| r.numeric_params()).unwrap_or_default());
    ok(result)
}

/// `nf-server replay-check`: read {request, entered_by} on stdin, recompute in
/// this fresh process, print the closure hash and record ids.
fn replay_child() {
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).ok();
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let Ok(req) = serde_json::from_value::<Request>(v["request"].clone()) else {
        eprintln!("replay-check: unreadable request");
        std::process::exit(2);
    };
    let out = job::run(&req, v["entered_by"].as_str().unwrap_or(""));
    println!("{}", json!({ "closure_hash": out.closure_hash, "ids": out.records.iter().map(|r| r.id.clone()).collect::<Vec<_>>() }));
}

fn replay_in_separate_process(request: &Value, entered_by: &str) -> Result<Value, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe).arg("replay-check").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    child.stdin.take().ok_or("no stdin")?.write_all(json!({ "request": request, "entered_by": entered_by }).to_string().as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("replay process exited with {}", out.status));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

async fn replay(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let c = {
        let db = app.db();
        match load_calc(&db, id).filter(|c| can_access(&db, &u, c.project)) {
            Some(c) => c,
            None => return not_found(),
        }
    };
    let expected_hash = c.result["closure_hash"].as_str().unwrap_or("").to_string();
    let ids: Vec<String> = c.result["records"].as_array().map(|a| a.iter().filter_map(|r| r["id"].as_str().map(String::from)).collect()).unwrap_or_default();
    // Recompute in a separate operating-system process that receives only the request.
    let child = replay_in_separate_process(&c.result["request"], &c.created_by);
    let db = app.db();
    // Confirm that the stored records are intact (identity recomputed from content).
    let mut tampered = 0;
    for rid in &ids {
        let stored: Option<String> = db.query_row("SELECT json FROM records WHERE id = ?", params![rid], |r| r.get(0)).optional().ok().flatten();
        match stored.and_then(|j| serde_json::from_str::<Record>(&j).ok()) {
            Some(r) if r.verify_id() && &r.id == rid => {}
            _ => tampered += 1,
        }
    }
    let (matches, got, problem) = match &child {
        Ok(v) => {
            let got_ids: Vec<String> = v["ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            (v["closure_hash"] == expected_hash.as_str() && got_ids == ids, v["closure_hash"].as_str().unwrap_or("").to_string(), None)
        }
        Err(e) => (false, String::new(), Some(e.clone())),
    };
    let report = json!({ "kind": "separate_process_replay", "matches": matches && tampered == 0, "expected_closure_hash": expected_hash, "replayed_closure_hash": got, "process_error": problem,
                         "stored_records_checked": ids.len(), "stored_records_failing_identity_check": tampered, "run_by": u.username, "at": now(),
                         "limitation": "Replay ran in a separate process that received only the request, on the same machine with the same build. A separately built binary or separate machine is not used. Discrepancies are reported, never repaired. This audit shows reproducibility, not correctness." });
    let audit_rec = Record::new(Family::Assessment, "replay_audit", report.clone(), ids.last().map(|g| vec![rel("audits", g)]).unwrap_or_default(), None);
    app.put_record(&db, c.project, &audit_rec).unwrap();
    db.execute("UPDATE calculations SET replay_json = ? WHERE id = ?", params![report.to_string(), id]).unwrap();
    if !(matches && tampered == 0) {
        db.execute("UPDATE calculations SET state = 'REJECTED' WHERE id = ?", params![id]).unwrap();
        if let Ok(req) = serde_json::from_value::<Request>(c.result["request"].clone()) {
            record_lesson(&app, &db, c.project, "integrity_failure", &family_of(&req), "", None, None, &serde_json::to_string(&req).unwrap_or_default(), id, &u.username);
        }
    }
    app.audit(&db, &u.username, "replay_audit", &format!("calculation #{id}: {}", if matches && tampered == 0 { "match" } else { "MISMATCH" }));
    ok(report)
}

async fn get_dxf(State(app): Ctx, headers: HeaderMap, Path((id, part)): Path<(i64, String)>) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    let Some(c) = load_calc(&db, id).filter(|c| can_access(&db, &u, c.project)) else { return not_found() };
    if let Some(n) = c.superseded_by {
        return fail(StatusCode::CONFLICT, &format!("this result was superseded by job #{n}; its files are withheld"));
    }
    let (_, state) = effective_gates(&db, id, &c);
    if state == nf_core::ReleaseState::Rejected {
        return fail(StatusCode::CONFLICT, "this result is REJECTED; its files are withheld");
    }
    let sha = c.result["patterns"].as_array().and_then(|a| a.iter().find(|p| p["part"] == part.as_str())).and_then(|p| p["dxf_sha256"].as_str().map(String::from));
    let Some(sha) = sha else { return fail(StatusCode::NOT_FOUND, "no DXF for this part") };
    let bytes = match app.load_blob(&sha) {
        Ok(b) => b,
        Err(e) => return fail(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    app.audit(&db, &u.username, "dxf_export", &format!("calculation #{id} part {part} sha {sha}"));
    let name = format!("NOT-RELEASED_REVIEW_REQUIRED_calc{id}_{part}_{}.dxf", &sha[..8]);
    ([(header::CONTENT_TYPE, "application/dxf".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))], bytes).into_response()
}

/// Record an engineer's correction to one parameter of an assembly job:
/// propagate along the dependency edges of the old result, recompute, and
/// create a scrutiny obligation for everything affected.
async fn correct(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let (pname, reason) = (s(&body, "parameter"), s(&body, "reason").trim().to_string());
    let Some(new_value) = body.get("new_value").and_then(|v| v.as_f64()).filter(|v| v.is_finite()) else { return fail(StatusCode::BAD_REQUEST, "new_value must be a finite number") };
    if reason.len() < 5 {
        return fail(StatusCode::BAD_REQUEST, "a reason is required for every correction");
    }
    let db = app.db();
    let Some(c) = load_calc(&db, id).filter(|c| can_access(&db, &u, c.project)) else { return not_found() };
    if c.superseded_by.is_some() {
        return fail(StatusCode::CONFLICT, "this result is already superseded; correct the current one");
    }
    let Ok(req) = serde_json::from_value::<Request>(c.result["request"].clone()) else { return fail(StatusCode::INTERNAL_SERVER_ERROR, "stored request unreadable") };
    if !matches!(req, Request::HeaderWithBranch { .. }) {
        return fail(StatusCode::BAD_REQUEST, "corrections with dependency propagation are implemented for assembly jobs only");
    }
    let Some(old_value) = req.numeric_params().into_iter().find(|(k, _)| *k == pname).map(|(_, v)| v) else { return fail(StatusCode::BAD_REQUEST, "unknown parameter") };
    if old_value == new_value {
        return fail(StatusCode::BAD_REQUEST, "new value equals the current value");
    }
    let Some(new_req) = req.with_param(&pname, new_value) else { return fail(StatusCode::BAD_REQUEST, "parameter cannot be corrected") };
    let records: std::collections::BTreeMap<String, Record> = serde_json::from_value::<Vec<Record>>(c.result["records"].clone()).unwrap_or_default().into_iter().map(|r| (r.id.clone(), r)).collect();
    let Some(old_param) = records.values().find(|r| r.kind == "parameter" && r.payload["name"] == pname.as_str()).map(|r| r.id.clone()) else { return fail(StatusCode::INTERNAL_SERVER_ERROR, "parameter record not found") };

    let attestation = Record::new(Family::Attestation, "correction", json!({ "parameter": pname, "old_value": old_value, "new_value": new_value, "reason": reason, "by": u.username, "at": now(), "calculation": id }), vec![rel("corrects", &old_param)], None);
    app.put_record(&db, c.project, &attestation).unwrap();
    let affected = nf_core::deps::propagate(&records, &old_param);
    let out = job::run(&new_req, &u.username);
    let new_id = match persist_calc(&app, &db, c.project, &out, &u.username, Some(id)) {
        Ok(x) => x,
        Err(e) => return fail(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    record_lesson(&app, &db, c.project, "corrected_value", &family_of(&req), &pname, Some(old_value), Some(new_value), &reason, id, &u.username);
    raise_advisories(&db, c.project, new_id, &new_req);
    let mut open = 0;
    for a in &affected {
        let (done, note) = nf_core::deps::discharge(a, &out);
        if !done {
            open += 1;
        }
        let dir = serde_json::to_value(a.direction).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let finding = Record::new(Family::Finding, "obligation", json!({ "target": a, "rule_table": nf_core::deps::RULE_TABLE_VERSION, "initial_state": if done { "discharged" } else { "open" }, "note": note, "new_calculation_closure": out.closure_hash }), vec![rel("origin", &attestation.id), rel("target", &a.record_id)], None);
        app.put_record(&db, c.project, &finding).unwrap();
        db.execute(
            "INSERT INTO obligations(origin_calc, target_calc, record_id, kind, label, direction, via, required_check, state, note, created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?)",
            params![id, new_id, finding.id, a.kind, a.label, dir, a.via, a.required_check, if done { "discharged" } else { "open" }, note, now() as i64],
        )
        .unwrap();
    }
    app.audit(&db, &u.username, "correction", &format!("calculation #{id} {pname}: {old_value} -> {new_value} ({reason}); new #{new_id}; {} obligations, {open} open", affected.len()));
    ok(json!({ "new_calculation": new_id, "obligations": affected.len(), "open": open }))
}

async fn list_obligations(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    ok(json!(obligation_rows(&db, &format!("WHERE target_calc IN (SELECT id FROM calculations WHERE {})", scope(&u)), 0)))
}

async fn resolve_obligation(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Value>) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin", "engineer"]));
    let (decision, reason) = (s(&body, "decision"), s(&body, "reason").trim().to_string());
    let new_state = match decision.as_str() {
        "confirmed" => "discharged_by_decision",
        "waived" => "waived",
        _ => return fail(StatusCode::BAD_REQUEST, "decision must be 'confirmed' or 'waived'"),
    };
    if reason.len() < 5 {
        return fail(StatusCode::BAD_REQUEST, "a reason is required");
    }
    let db = app.db();
    let row: Option<(String, i64, String)> = db.query_row("SELECT state, target_calc, record_id FROM obligations WHERE id = ?", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().ok().flatten();
    let Some((state, target, record_id)) = row else { return not_found() };
    let project_of: i64 = db.query_row("SELECT project_id FROM calculations WHERE id = ?", params![target], |r| r.get(0)).unwrap_or(-1);
    if !can_access(&db, &u, project_of) {
        return not_found();
    }
    if state != "open" {
        return fail(StatusCode::CONFLICT, "obligation is not open");
    }
    let project: i64 = db.query_row("SELECT project_id FROM calculations WHERE id = ?", params![target], |r| r.get(0)).unwrap_or(0);
    let att = Record::new(Family::Attestation, "obligation_resolution", json!({ "decision": decision, "reason": reason, "by": u.username, "role": u.role, "at": now() }), vec![rel("resolves", &record_id)], None);
    app.put_record(&db, project, &att).unwrap();
    db.execute("UPDATE obligations SET state = ?, note = ?, resolved_by = ?, resolved_at = ? WHERE id = ? AND state = 'open'", params![new_state, reason, u.username, now() as i64, id]).unwrap();
    app.audit(&db, &u.username, "obligation_resolved", &format!("#{id} {decision}: {reason}"));
    ok(json!({ "state": new_state }))
}

async fn dependencies(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    let Some(c) = load_calc(&db, id).filter(|c| can_access(&db, &u, c.project)) else { return not_found() };
    let records: Vec<Record> = serde_json::from_value(c.result["records"].clone()).unwrap_or_default();
    let name = |r: &Record| -> String { r.payload.get("part").or_else(|| r.payload.get("name")).and_then(|v| v.as_str()).map(|x| format!("{} · {x}", r.kind)).unwrap_or_else(|| r.kind.clone()) };
    let by_id: std::collections::BTreeMap<&str, &Record> = records.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut edges = Vec::new();
    for r in records.iter().filter(|r| r.family == Family::Claim) {
        for e in &r.relations {
            let Some(t) = by_id.get(e.to.as_str()) else { continue };
            if ["computed_from", "developed_from", "rendered_from", "defined_by", "pattern_of", "mates_at", "interface_feature", "positioned_by", "convention_of"].contains(&e.rel.as_str()) {
                edges.push(json!({ "from": name(r), "rel": e.rel, "to": name(t) }));
            }
        }
    }
    ok(json!({ "edges": edges, "rule_table": nf_core::deps::RULE_TABLE_VERSION }))
}

async fn dashboard(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    let sc = scope(&u);
    let n = |sql: String| -> i64 { db.query_row(&sql, [], |r| r.get(0)).unwrap_or(0) };
    let calcs = format!("SELECT COUNT(*) FROM calculations WHERE {sc}");
    ok(json!({
        "projects": n(format!("SELECT COUNT(*) FROM projects WHERE {}", sc.replace("project_id IN", "id IN"))),
        "documents": n(format!("SELECT COUNT(*) FROM documents WHERE {sc}")),
        "calculations": n(calcs.clone()),
        "review_required": n(format!("{calcs} AND state = 'REVIEW_REQUIRED'")),
        "rejected": n(format!("{calcs} AND state = 'REJECTED'")),
        "approved": n(format!("{calcs} AND state = 'APPROVED'")),
        "open_obligations": n(format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND target_calc IN (SELECT id FROM calculations WHERE {sc})")),
        "records": n(format!("SELECT COUNT(*) FROM records WHERE {sc}")),
        "recent": calc_rows(&db, &format!("WHERE {sc}")).into_iter().take(8).collect::<Vec<_>>(),
        "uptime_seconds": now() - app.started,
        "engine": nf_core::ENGINE_VERSION,
    }))
}

async fn amoeba_summary(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    let db = app.db();
    let sc = scope(&u);
    let mut st = db.prepare(&format!("SELECT family, kind, COUNT(*) FROM records WHERE {sc} GROUP BY family, kind ORDER BY family, kind")).unwrap();
    let kinds: Vec<Value> = st.query_map([], |r| Ok(json!({ "family": r.get::<_, String>(0)?, "kind": r.get::<_, String>(1)?, "count": r.get::<_, i64>(2)? }))).unwrap().flatten().collect();
    let mut st = db.prepare(&format!("SELECT kind, json FROM records WHERE family = 'activity' AND {sc}")).unwrap();
    let mut work: std::collections::BTreeMap<String, (i64, i64)> = Default::default();
    for (kind, j) in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).unwrap().flatten() {
        let wu = serde_json::from_str::<Value>(&j).ok().and_then(|v| v["payload"]["work_units"].as_i64()).unwrap_or(0);
        let e = work.entry(kind).or_insert((0, 0));
        e.0 += 1;
        e.1 += wu;
    }
    let mut st = db.prepare(&format!("SELECT project_id, id FROM records WHERE kind = 'snapshot' AND {sc} ORDER BY rowid DESC LIMIT 10")).unwrap();
    let snaps: Vec<Value> = st.query_map([], |r| Ok(json!({ "project_id": r.get::<_, i64>(0)?, "snapshot": r.get::<_, String>(1)? }))).unwrap().flatten().collect();
    ok(json!({
        "records_by_kind": kinds,
        "activities": work.into_iter().map(|(k, (n, wu))| json!({ "type": k, "distinct_activities": n, "declared_work_units": wu })).collect::<Vec<_>>(),
        "recent_snapshots": snaps,
        "implemented": ["content-addressed append-only records", "project snapshots with predecessor links", "typed dependency edges (derivation, interface, common cause)", "dependency propagation with scrutiny obligations (provisional rule table)", "interface constraint verification between parts", "lessons from corrections and failures, matched against every later job in the project", "corrections as attestations with supersession", "release closure hash", "gate evaluation with obligation and replay state", "replay audit in a separate process", "membrane admission checks"],
        "obligations": { "open": db.query_row(&format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND target_calc IN (SELECT id FROM calculations WHERE {sc})"), [], |r| r.get::<_, i64>(0)).unwrap_or(0), "total": db.query_row(&format!("SELECT COUNT(*) FROM obligations WHERE target_calc IN (SELECT id FROM calculations WHERE {sc})"), [], |r| r.get::<_, i64>(0)).unwrap_or(0) },
        "not_implemented": ["round scheduler and budgets", "exploration probes", "hypothesis-driven investigation", "coverage ledger", "lanes and overlays", "quarantine", "strategy switching", "self-consistency audits A1-A4", "replay on a separate machine or build", "engineer approval"],
    }))
}

async fn audit_log(State(app): Ctx, headers: HeaderMap) -> Response {
    let u = guard!(auth(&app, &headers));
    guard!(require(&u, &["admin"]));
    let db = app.db();
    let mut st = db.prepare("SELECT at, username, action, detail FROM audit ORDER BY id DESC LIMIT 300").unwrap();
    let rows: Vec<Value> = st.query_map([], |r| Ok(json!({ "at": r.get::<_, i64>(0)?, "username": r.get::<_, String>(1)?, "action": r.get::<_, String>(2)?, "detail": r.get::<_, String>(3)? }))).unwrap().flatten().collect();
    ok(json!(rows))
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-store")], body).into_response()
}

/// Applied to every request: anti-forgery checks on state-changing API calls
/// and security headers on every response.
async fn security_layer(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let state_changing = !matches!(*req.method(), axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS);
    let is_api = req.uri().path().starts_with("/api/");
    if is_api && state_changing {
        let h = req.headers();
        if h.get("x-nf-request").is_none() {
            return fail(StatusCode::FORBIDDEN, "missing anti-forgery header");
        }
        if let Some(origin) = h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            let host = h.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
            if origin.split("://").nth(1) != Some(host) {
                return fail(StatusCode::FORBIDDEN, "cross-origin request refused");
            }
        }
    }
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    let set = |h: &mut HeaderMap, k: &'static str, v: &'static str| {
        h.insert(k, axum::http::HeaderValue::from_static(v));
    };
    set(h, "content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'");
    set(h, "x-content-type-options", "nosniff");
    set(h, "x-frame-options", "DENY");
    set(h, "referrer-policy", "no-referrer");
    set(h, "permissions-policy", "camera=(), microphone=(), geolocation=()");
    set(h, "cross-origin-opener-policy", "same-origin");
    if is_api {
        set(h, "cache-control", "no-store");
    }
    resp
}

fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/", get(|| async { asset("text/html; charset=utf-8", include_str!("../../../web/index.html")) }))
        .route("/app.js", get(|| async { asset("text/javascript; charset=utf-8", include_str!("../../../web/app.js")) }))
        .route("/viewer.js", get(|| async { asset("text/javascript; charset=utf-8", include_str!("../../../web/viewer.js")) }))
        .route("/styles.css", get(|| async { asset("text/css; charset=utf-8", include_str!("../../../web/styles.css")) }))
        .route("/api/health", get(health))
        .route("/api/setup", post(setup))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/users", get(list_users).post(add_user))
        .route("/api/users/{id}/active", post(set_user_active))
        .route("/api/password", post(change_password))
        .route("/api/projects/{id}/members", post(add_member))
        .route("/api/audit/verify", get(audit_verify))
        .route("/api/security/status", get(security_status))
        .route("/api/projects", get(list_projects).post(add_project))
        .route("/api/projects/{id}", get(get_project))
        .route("/api/projects/{id}/documents", post(upload).layer(DefaultBodyLimit::max(membrane::MAX_UPLOAD_BYTES + 1024)))
        .route("/api/projects/{id}/calculations", post(calculate))
        .route("/api/calculations", get(list_calcs))
        .route("/api/calculations/{id}", get(get_calc))
        .route("/api/calculations/{id}/replay", post(replay))
        .route("/api/calculations/{id}/dxf/{part}", get(get_dxf))
        .route("/api/calculations/{id}/corrections", post(correct))
        .route("/api/calculations/{id}/dependencies", get(dependencies))
        .route("/api/lessons", get(list_lessons))
        .route("/api/advisories/{id}/acknowledge", post(acknowledge_advisory))
        .route("/api/obligations", get(list_obligations))
        .route("/api/obligations/{id}/resolve", post(resolve_obligation))
        .route("/api/dashboard", get(dashboard))
        .route("/api/amoeba/summary", get(amoeba_summary))
        .route("/api/audit", get(audit_log))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(axum::middleware::from_fn(security_layer))
        .with_state(app)
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("replay-check") {
        replay_child();
        return;
    }
    let data_dir = PathBuf::from(std::env::var("NF_DATA_DIR").unwrap_or_else(|_| "data".into()));
    let addr = std::env::var("NF_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    std::fs::create_dir_all(data_dir.join("blobs")).expect("cannot create data directory");
    // The storage key should live apart from the data it protects: set NF_KEY_FILE to a path outside the data directory.
    let key_path = std::env::var("NF_KEY_FILE").map(PathBuf::from).unwrap_or_else(|_| data_dir.join("storage.key"));
    let key_outside_data_dir = !key_path.starts_with(&data_dir);
    let key = crypto::load_or_create_key(&key_path).expect("cannot load or create storage key");
    let db = Connection::open(data_dir.join("nexus-fabric.sqlite")).expect("cannot open database");
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;").ok();
    db.execute_batch(SCHEMA).expect("cannot create schema");
    let _ = db.execute("ALTER TABLE calculations ADD COLUMN supersedes INTEGER", []); // older development databases
    for migration in ["ALTER TABLE users ADD COLUMN active INTEGER NOT NULL DEFAULT 1", "ALTER TABLE audit ADD COLUMN prev TEXT", "ALTER TABLE audit ADD COLUMN hash TEXT"] {
        let _ = db.execute(migration, []); // older development databases
    }
    let cookie_secure = std::env::var("NF_COOKIE_SECURE").map(|v| v == "1").unwrap_or(false);
    let app = Arc::new(App { db: Mutex::new(db), key, data_dir, started: now(), cookie_secure, key_outside_data_dir });
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("cannot bind listen address");
    println!("NEXUS FABRIC development build listening on http://{addr}  (data: {})", app.data_dir.display());
    println!("NOT VALIDATED - no output of this software may be used for manufacturing.");
    axum::serve(listener, router(app)).await.expect("server error");
}
