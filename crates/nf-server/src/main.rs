//! NEXUS FABRIC application server (single binary, offline-capable).
//!
//! Serves the web client and the JSON API. Everything the client shows comes
//! from this API; features that do not exist return nothing and are labelled
//! as not implemented in the client.
//!
//! Rules this file keeps:
//! - The effective state of a job is computed by ONE function (`evaluate`),
//!   used by the detail view, the lists, the dashboard column and the DXF
//!   download. It fails closed: missing, unreadable or inconsistent stored
//!   data gives REJECTED with an integrity finding. APPROVED does not exist
//!   as an output of this server.
//! - A refused request is REFUSED end to end, never a result under review.
//! - Database writes that belong together are one transaction.
//! - No request path unwraps a database or lock result: errors become a JSON
//!   response (503 when the database is busy, 500 otherwise) and a log line.

mod crypto;
mod membrane;
mod storage;
#[cfg(test)]
mod tests;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use nf_core::job::{self, Request};
use nf_core::{Family, Gate, GateResult, Record, rel};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

pub(crate) struct App {
    db: Mutex<Connection>,
    key: [u8; 32],
    data_dir: PathBuf,
    started: u64,
    cookie_secure: bool,
    key_outside_data_dir: bool,
    /// Host names this server answers to (V-36, DNS rebinding).
    allowed_hosts: Vec<String>,
    /// Peers whose X-Forwarded-For is believed (V-33). Empty: the header is never used.
    trusted_proxies: Vec<IpAddr>,
    /// SHA-256 of the one-time setup token while no administrator exists (V-36).
    setup_token: Mutex<Option<String>>,
    /// Executable used for the replay child; `None` = this executable.
    replay_exe: Option<PathBuf>,
    replay_timeout: Duration,
    /// Longest a request may take from its first header byte to its answer (S-3).
    request_timeout: Duration,
    hash_slots: Semaphore,
    upload_slots: Semaphore,
    replay_slots: Semaphore,
    /// (window start, unauthenticated audit rows written, suppressed) — V-34.
    anon_audit: Mutex<(u64, u32, u32)>,
    /// (database data_version, jobs whose stored summary was checked against the full result at that version).
    verified: Mutex<(i64, std::collections::HashSet<i64>)>,
    #[cfg(test)]
    fault: Mutex<Option<&'static str>>,
}

type Ctx = State<Arc<App>>;

#[derive(Clone)]
struct User {
    id: i64,
    username: String,
    role: String,
}

/// Address of the client as far as it can be known (V-33).
#[derive(Clone)]
struct ClientAddr(String);

/// Address of the TCP peer, attached to every request by the accept loop.
#[derive(Clone)]
struct PeerAddr(IpAddr);

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- errors

/// Every way a request can end without its normal answer.
pub(crate) enum ApiError {
    /// A deliberate answer (4xx and the like).
    Resp(Box<Response>),
    Db(rusqlite::Error),
    Internal(String),
}

type Api = Result<Response, ApiError>;

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::Db(e)
    }
}

/// The disk (or the database's size limit) is full: nothing was written.
fn db_is_full(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if matches!(f.code, rusqlite::ErrorCode::DiskFull))
}

fn db_is_busy(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if matches!(f.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Resp(r) => *r,
            ApiError::Db(e) if db_is_busy(&e) => {
                eprintln!("nf-server: database busy: {e}");
                let mut r = (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "the database is busy; nothing was changed. Retry shortly.", "retry": true }))).into_response();
                r.headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from_static("2"));
                r
            }
            ApiError::Db(e) if db_is_full(&e) => {
                eprintln!("nf-server: STORAGE FULL: {e}");
                (StatusCode::INSUFFICIENT_STORAGE, Json(json!({ "error": "the server's storage is full; nothing was changed and nothing was recorded. An administrator must free space.", "storage_full": true }))).into_response()
            }
            ApiError::Db(e) => {
                eprintln!("nf-server: database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal storage error; nothing was changed. The server log has the details." }))).into_response()
            }
            ApiError::Internal(e) => {
                eprintln!("nf-server: internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal error; nothing was changed. The server log has the details." }))).into_response()
            }
        }
    }
}

fn fail(status: StatusCode, msg: &str) -> ApiError {
    ApiError::Resp(Box::new(
        (status, Json(json!({ "error": msg }))).into_response(),
    ))
}

fn fail_json(status: StatusCode, body: Value) -> ApiError {
    ApiError::Resp(Box::new((status, Json(body)).into_response()))
}

fn ok(v: Value) -> Api {
    Ok(Json(v).into_response())
}

/// Hide the existence of things the caller may not see.
fn not_found() -> ApiError {
    fail(StatusCode::NOT_FOUND, "not found")
}

// ---------------------------------------------------------------- small helpers

/// Bound a string before it is stored or logged (V-34). Cuts on a character boundary.
fn clip(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

const MAX_USERNAME: usize = 64;
const MAX_TEXT: usize = 2000;

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// A free-text field that will be stored in immutable records: bounded (BE-029).
fn text_field(v: &Value, k: &str) -> Result<String, ApiError> {
    let t = s(v, k).trim().to_string();
    if t.chars().count() > MAX_TEXT {
        return Err(fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("{k} is too long (at most {MAX_TEXT} characters)"),
        ));
    }
    Ok(t)
}

fn q_all<T, P: rusqlite::Params, F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>(
    db: &Connection,
    sql: &str,
    p: P,
    f: F,
) -> rusqlite::Result<Vec<T>> {
    let mut st = db.prepare(sql)?;
    let rows = st.query_map(p, f)?;
    rows.collect()
}

fn q_count<P: rusqlite::Params>(db: &Connection, sql: &str, p: P) -> rusqlite::Result<i64> {
    db.query_row(sql, p, |r| r.get(0))
}

/// Text column that may hold anything after tampering: only text counts.
fn text_col(r: &rusqlite::Row<'_>, i: usize) -> rusqlite::Result<Option<String>> {
    Ok(match r.get_ref(i)? {
        rusqlite::types::ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        _ => None,
    })
}

fn audit_hash(prev: &str, at: i64, user: &str, action: &str, detail: &str) -> String {
    nf_core::sha256_hex(
        serde_json::to_string(&json!([prev, at, user, action, detail]))
            .unwrap_or_default()
            .as_bytes(),
    )
}

fn anchor_mac(key: &[u8; 32], head: &str, count: i64, last_id: i64) -> String {
    storage::hmac_hex(
        key,
        format!("nexus-fabric audit anchor v1|{head}|{count}|{last_id}").as_bytes(),
    )
}

/// Unauthenticated events written to the audit log per minute before they are
/// counted instead of stored (V-34).
const ANON_AUDIT_PER_MINUTE: u32 = 30;

impl App {
    /// The database lock. A panic in one request must not take the service
    /// down for everyone, so a poisoned lock is recovered instead of propagated.
    /// Writes happen in transactions, so a panic leaves no partial state.
    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Fault injection for tests (BE-005). Compiled out of the product.
    #[cfg(test)]
    fn fault(&self, point: &'static str) -> Result<(), ApiError> {
        let hit = *self.fault.lock().unwrap_or_else(|e| e.into_inner()) == Some(point);
        if hit {
            Err(ApiError::Internal(format!("injected fault at {point}")))
        } else {
            Ok(())
        }
    }
    #[cfg(not(test))]
    #[inline(always)]
    fn fault(&self, _point: &'static str) -> Result<(), ApiError> {
        Ok(())
    }

    /// Append to the audit log. Each entry carries the hash of its predecessor,
    /// so removing or altering an entry inside the chain is detectable. The
    /// newest hash, the entry count and the newest id are kept as an anchor,
    /// authenticated with the storage key, so that removing the newest entries
    /// is detectable too (V-25). Call inside the transaction of the event.
    fn audit(
        &self,
        db: &Connection,
        user: &str,
        action: &str,
        detail: &str,
    ) -> rusqlite::Result<()> {
        let (user, action, detail) = (
            clip(user, MAX_USERNAME),
            clip(action, 64),
            clip(detail, 1000),
        );
        let prev: String = db
            .query_row("SELECT hash FROM audit ORDER BY id DESC LIMIT 1", [], |r| {
                r.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten()
            .unwrap_or_else(|| "genesis".into());
        let at = now() as i64;
        let hash = audit_hash(&prev, at, &user, &action, &detail);
        db.execute(
            "INSERT INTO audit(at, username, action, detail, prev, hash) VALUES (?,?,?,?,?,?)",
            params![at, user, action, detail, prev, hash],
        )?;
        let last_id = db.last_insert_rowid();
        let count = storage::meta_get(db, "audit_count")?
            .and_then(|c| c.parse::<i64>().ok())
            .unwrap_or(-1);
        // an unreadable counter is never guessed: recount
        let count = if count < 0 {
            q_count(db, "SELECT COUNT(*) FROM audit", [])?
        } else {
            count + 1
        };
        self.write_anchor(db, &hash, count, last_id)
    }

    fn write_anchor(
        &self,
        db: &Connection,
        head: &str,
        count: i64,
        last_id: i64,
    ) -> rusqlite::Result<()> {
        storage::meta_set(db, "audit_count", &count.to_string())?;
        storage::meta_set(db, "audit_anchor", &json!({ "head": head, "count": count, "last_id": last_id, "mac": anchor_mac(&self.key, head, count, last_id) }).to_string())
    }

    /// Audit an event caused by someone who is not signed in. Bounded in rate:
    /// beyond the limit events are counted, and the count is logged once the
    /// minute is over, so an anonymous client cannot fill the disk (V-34).
    fn audit_anon(
        &self,
        db: &Connection,
        user: &str,
        action: &str,
        detail: &str,
    ) -> rusqlite::Result<()> {
        let minute = now() / 60;
        let (write, report) = {
            let mut w = self.anon_audit.lock().unwrap_or_else(|e| e.into_inner());
            let mut report = 0;
            if w.0 != minute {
                report = w.2;
                *w = (minute, 0, 0);
            }
            if w.1 < ANON_AUDIT_PER_MINUTE {
                w.1 += 1;
                (true, report)
            } else {
                w.2 += 1;
                (false, report)
            }
        };
        if report > 0 {
            self.audit(db, "-", "audit_suppressed", &format!("{report} unauthenticated event(s) were not logged individually in an earlier minute (rate limit)"))?;
        }
        if write {
            self.audit(db, user, action, detail)?;
        }
        Ok(())
    }

    /// Append-only: an existing id is never overwritten.
    fn put_record(&self, db: &Connection, project: i64, r: &Record) -> rusqlite::Result<()> {
        let fam = serde_json::to_value(r.family)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        db.execute(
            "INSERT OR IGNORE INTO records(id, project_id, family, kind, json) VALUES (?,?,?,?,?)",
            params![
                r.id,
                project,
                fam,
                r.kind,
                serde_json::to_string(r).unwrap_or_default()
            ],
        )?;
        Ok(())
    }

    fn store_blob(&self, bytes: &[u8]) -> Result<String, ApiError> {
        let (sha, repaired) =
            storage::store_blob(&self.data_dir, &self.key, bytes).map_err(ApiError::Internal)?;
        if repaired {
            eprintln!(
                "nf-server: stored file {sha} failed its integrity check and was rewritten from identical content"
            );
        }
        Ok(sha)
    }

    fn cookie_name(&self) -> &'static str {
        // The __Host- prefix needs Secure, so it is used exactly when the cookie is Secure (V-39).
        if self.cookie_secure {
            "__Host-nf_session"
        } else {
            "nf_session"
        }
    }
}

fn session_token(app: &App, headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{}=", app.cookie_name());
    cookies
        .split(';')
        .filter_map(|c| c.trim().strip_prefix(prefix.as_str()))
        .next()
        .map(String::from)
}

fn auth_db(db: &Connection, app: &App, headers: &HeaderMap) -> Result<User, ApiError> {
    let deny = || fail(StatusCode::UNAUTHORIZED, "not signed in");
    let token = session_token(app, headers).ok_or_else(deny)?;
    let th = nf_core::sha256_hex(token.as_bytes());
    db.query_row(
        "SELECT u.id, u.username, u.role FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > ? AND u.active = 1",
        params![th, now() as i64],
        |r| Ok(User { id: r.get(0)?, username: r.get(1)?, role: r.get(2)? }),
    )
    .optional()?
    .ok_or_else(deny)
}

fn auth(app: &App, headers: &HeaderMap) -> Result<User, ApiError> {
    auth_db(&app.db(), app, headers)
}

fn require(user: &User, roles: &[&str]) -> Result<(), ApiError> {
    if roles.contains(&user.role.as_str()) {
        Ok(())
    } else {
        Err(fail(
            StatusCode::FORBIDDEN,
            "your role does not permit this action",
        ))
    }
}

/// SQL condition restricting the project-id column `col` to projects the
/// user may see. Built from an integer id and a fixed column name only, never
/// from request text.
fn scope_col(u: &User, col: &str) -> String {
    if u.role == "admin" {
        "1=1".into()
    } else {
        format!(
            "{col} IN (SELECT project_id FROM project_members WHERE user_id = {})",
            u.id
        )
    }
}

fn scope(u: &User) -> String {
    scope_col(u, "project_id")
}

/// Fails closed: a database error is an error, never "allowed".
fn can_access(db: &Connection, u: &User, project: i64) -> rusqlite::Result<bool> {
    Ok(u.role == "admin"
        || q_count(
            db,
            "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND user_id = ?",
            params![project, u.id],
        )? == 1)
}

fn project_exists(db: &Connection, project: i64) -> rusqlite::Result<bool> {
    Ok(q_count(
        db,
        "SELECT COUNT(*) FROM projects WHERE id = ?",
        params![project],
    )? == 1)
}

/// The project exists and the user may use it; otherwise 404 either way. A
/// refusal of an existing project is audited (V-41).
fn project_guard(
    app: &App,
    db: &Connection,
    u: &User,
    project: i64,
    what: &str,
) -> Result<(), ApiError> {
    if !project_exists(db, project)? {
        return Err(not_found());
    }
    if !can_access(db, u, project)? {
        if let Err(e) = app.audit(
            db,
            &u.username,
            "access_denied",
            &format!("{what}: project #{project}"),
        ) {
            eprintln!("nf-server: could not audit a refused access: {e}");
        }
        return Err(not_found());
    }
    Ok(())
}

/// Passwords that are refused whatever their length (V-46). Deliberately short:
/// a real deny-list belongs to the identity work that is still open (V-23).
const COMMON_PASSWORDS: [&str; 12] = [
    "password12",
    "password123",
    "password1234",
    "1234567890",
    "12345678901",
    "0123456789",
    "qwertyuiop",
    "qwertyuiop1",
    "administrator",
    "letmein123",
    "changeme123",
    "welcome123",
];

fn password_policy(username: &str, password: &str) -> Result<(), String> {
    if password.chars().count() < 10 {
        return Err("password must have at least 10 characters".into());
    }
    if password.len() > 1024 {
        return Err("password is too long (at most 1024 bytes)".into());
    }
    let lower = password.to_lowercase();
    if lower == username.to_lowercase() {
        return Err("password must not be the user name".into());
    }
    if COMMON_PASSWORDS.contains(&lower.as_str()) {
        return Err(
            "this password is on the list of commonly used passwords; choose another".into(),
        );
    }
    Ok(())
}

fn valid_username(username: &str) -> bool {
    username.len() >= 3
        && username.len() <= MAX_USERNAME
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// Everything about a new account that can be checked without the database.
fn check_new_user(username: &str, password: &str, role: &str) -> Result<(), ApiError> {
    let bad = |m: &str| fail(StatusCode::BAD_REQUEST, m);
    if !valid_username(username) {
        return Err(bad(
            "username must be 3-64 characters: letters, digits, . _ -",
        ));
    }
    password_policy(username, password).map_err(|m| bad(&m))?;
    if !["admin", "engineer", "viewer"].contains(&role) {
        return Err(bad("role must be admin, engineer or viewer"));
    }
    Ok(())
}

/// Insert an account whose password was already hashed (outside the database lock).
fn insert_user(
    db: &Connection,
    username: &str,
    role: &str,
    salt_hex: &str,
    hash: &str,
) -> Result<i64, ApiError> {
    if q_count(
        db,
        "SELECT COUNT(*) FROM users WHERE username = ?",
        params![username],
    )? > 0
    {
        return Err(fail(StatusCode::BAD_REQUEST, "username already exists"));
    }
    db.execute(
        "INSERT INTO users(username, role, salt, hash, created_at) VALUES (?,?,?,?,?)",
        params![username, role, salt_hex, hash, now() as i64],
    )?;
    Ok(db.last_insert_rowid())
}

// ---------------------------------------------------------------- unauthenticated handlers

/// Liveness only. No version, no uptime (V-49). `setup_required` stays: the
/// client needs it to show the first-run screen, and setup needs the token.
async fn health(State(app): Ctx) -> Api {
    let db = app.db();
    let users = q_count(&db, "SELECT COUNT(*) FROM users", [])?;
    ok(
        json!({ "status": "ok", "setup_required": users == 0, "setup_token_required": users == 0,
               "build_notice": "Development build. Not validated. No output of this software may be used for manufacturing." }),
    )
}

const THROTTLE_WINDOW: i64 = 900;
const PER_ACCOUNT_FAILURES: i64 = 5;
const PER_CLIENT_WINDOW: i64 = 600;
const PER_CLIENT_FAILURES: i64 = 100;
const SETUP_FAILURES: i64 = 10;

fn failures_of(db: &Connection, key: &str, since: i64) -> rusqlite::Result<i64> {
    q_count(
        db,
        "SELECT COUNT(*) FROM login_failures WHERE username = ? AND at > ?",
        params![key, since],
    )
}

fn failures_from(db: &Connection, client: &str, since: i64) -> rusqlite::Result<i64> {
    q_count(
        db,
        "SELECT COUNT(*) FROM login_failures WHERE client = ? AND at > ?",
        params![client, since],
    )
}

/// First-run setup: creates the first administrator. Requires the one-time
/// setup token that only the installer has (V-36); the token is consumed.
async fn setup(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Api {
    let t = now() as i64;
    let key = "setup:";
    let (username, password) = (
        s(&body, "username").trim().to_string(),
        s(&body, "password"),
    );
    {
        let mut db = app.db();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if q_count(&tx, "SELECT COUNT(*) FROM users", [])? != 0 {
            return Err(fail(StatusCode::CONFLICT, "setup already completed"));
        }
        if failures_of(&tx, key, t - PER_CLIENT_WINDOW)? >= SETUP_FAILURES {
            return Err(fail(
                StatusCode::TOO_MANY_REQUESTS,
                "too many setup attempts with a wrong token; try again in 10 minutes",
            ));
        }
        let given = headers
            .get("x-nf-setup-token")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| s(&body, "setup_token"));
        let expected = app
            .setup_token
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let token_ok = match &expected {
            Some(h) => {
                !given.is_empty()
                    && crypto::constant_time_eq(&nf_core::sha256_hex(given.trim().as_bytes()), h)
            }
            None => false,
        };
        if !token_ok {
            tx.execute(
                "INSERT INTO login_failures(username, at, client) VALUES (?,?,?)",
                params![key, t, client],
            )?;
            app.audit_anon(
                &tx,
                "-",
                "setup_refused",
                &format!("missing or wrong setup token, from {client}"),
            )?;
            tx.commit()?;
            return Err(fail(
                StatusCode::FORBIDDEN,
                "setup token required: the first administrator can only be created with the one-time setup token of this installation (shown to the installer at first start, or set with NF_SETUP_TOKEN)",
            ));
        }
        check_new_user(&username, &password, "admin")?;
    }
    let salt = hex::encode(crypto::random_bytes::<16>());
    let hash = hash_blocking(&app, password, salt.clone()).await?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // decided again inside the transaction that creates the account: exactly one caller can win
    if q_count(&tx, "SELECT COUNT(*) FROM users", [])? != 0 {
        return Err(fail(StatusCode::CONFLICT, "setup already completed"));
    }
    insert_user(&tx, &username, "admin", &salt, &hash)?;
    tx.execute(
        "DELETE FROM login_failures WHERE username = ?",
        params![key],
    )?;
    app.audit(
        &tx,
        &username,
        "setup",
        &format!("first administrator created with the setup token, from {client}"),
    )?;
    tx.commit()?;
    *app.setup_token.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let _ = std::fs::remove_file(app.data_dir.join("setup-token"));
    ok(json!({ "created": true }))
}

/// Argon2 off the async workers and outside the database lock, a few at a time (V-37).
async fn hash_blocking(
    app: &Arc<App>,
    password: String,
    salt_hex: String,
) -> Result<String, ApiError> {
    let _slot = app
        .hash_slots
        .acquire()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    tokio::task::spawn_blocking(move || {
        crypto::hash_password(&password, &hex::decode(&salt_hex).unwrap_or_default())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(ApiError::Internal)
}

/// Sign-in. Throttled per account (5 failures in 15 minutes: password guessing)
/// and per client address (100 failures in 10 minutes: spraying). There is no
/// global limit: failures by other names or other clients never block a user (V-33).
async fn login(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    Json(body): Json<Value>,
) -> Api {
    let (raw_name, password) = (s(&body, "username"), s(&body, "password"));
    let username = clip(&raw_name, MAX_USERNAME);
    let plausible = valid_username(&raw_name) && password.len() <= 1024;
    let t = now() as i64;
    let row: Option<(i64, String, String, String)> = {
        let db = app.db();
        db.execute(
            "DELETE FROM login_failures WHERE at < ?",
            params![t - THROTTLE_WINDOW],
        )?;
        let throttled = if failures_of(&db, &username, t - THROTTLE_WINDOW)? >= PER_ACCOUNT_FAILURES
        {
            Some("too many failed sign-in attempts for this account; try again in 15 minutes")
        } else if failures_from(&db, &client, t - PER_CLIENT_WINDOW)? >= PER_CLIENT_FAILURES {
            Some("too many failed sign-in attempts from this address; try again in 10 minutes")
        } else {
            None
        };
        if let Some(msg) = throttled {
            app.audit_anon(&db, &username, "login_throttled", &format!("from {client}"))?;
            return Err(fail(StatusCode::TOO_MANY_REQUESTS, msg));
        }
        if plausible {
            db.query_row(
                "SELECT id, role, salt, hash FROM users WHERE username = ? AND active = 1",
                params![raw_name],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
        } else {
            None
        }
    };
    // hash even for unknown users so that timing does not reveal which names exist
    let (id, role, salt, hash) = row.unwrap_or((
        0,
        String::new(),
        "00000000000000000000000000000000".into(),
        String::new(),
    ));
    let computed = if plausible {
        hash_blocking(&app, password, salt).await?
    } else {
        String::new()
    };
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if id == 0 || !crypto::constant_time_eq(&computed, &hash) {
        tx.execute(
            "INSERT INTO login_failures(username, at, client) VALUES (?,?,?)",
            params![username, t, client],
        )?;
        app.audit_anon(&tx, &username, "login_failed", &format!("from {client}"))?;
        tx.commit()?;
        return Err(fail(
            StatusCode::UNAUTHORIZED,
            "invalid username or password",
        ));
    }
    tx.execute(
        "DELETE FROM login_failures WHERE username = ?",
        params![username],
    )?;
    let token = hex::encode(crypto::random_bytes::<32>());
    tx.execute("DELETE FROM sessions WHERE expires_at <= ?", params![t])?;
    tx.execute(
        "INSERT INTO sessions(token_hash, user_id, expires_at) VALUES (?,?,?)",
        params![
            nf_core::sha256_hex(token.as_bytes()),
            id,
            (now() + 12 * 3600) as i64
        ],
    )?;
    app.audit(&tx, &username, "login", &format!("from {client}"))?;
    tx.commit()?;
    let cookie = format!(
        "{}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200{}",
        app.cookie_name(),
        if app.cookie_secure { "; Secure" } else { "" }
    );
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(json!({ "username": username, "role": role })),
    )
        .into_response())
}

async fn logout(State(app): Ctx, headers: HeaderMap) -> Api {
    if let Some(t) = session_token(&app, &headers) {
        let mut db = app.db();
        let who = auth_db(&db, &app, &headers).ok();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM sessions WHERE token_hash = ?",
            params![nf_core::sha256_hex(t.as_bytes())],
        )?;
        if let Some(u) = who {
            app.audit(&tx, &u.username, "logout", "")?;
        }
        tx.commit()?;
    }
    let cookie = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        app.cookie_name(),
        if app.cookie_secure { "; Secure" } else { "" }
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

// ---------------------------------------------------------------- accounts

async fn me(State(app): Ctx, headers: HeaderMap) -> Api {
    let u = auth(&app, &headers)?;
    ok(json!({ "id": u.id, "username": u.username, "role": u.role }))
}

async fn list_users(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin"])?;
    let rows = q_all(
        &db,
        "SELECT id, username, role, created_at, active FROM users ORDER BY id",
        [],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "username": r.get::<_, String>(1)?, "role": r.get::<_, String>(2)?, "created_at": r.get::<_, i64>(3)?, "active": r.get::<_, bool>(4)? }),
            )
        },
    )?;
    ok(json!(rows))
}

async fn add_user(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Api {
    let (username, password, role) = (
        s(&body, "username").trim().to_string(),
        s(&body, "password"),
        s(&body, "role"),
    );
    let u = auth(&app, &headers)?;
    require(&u, &["admin"])?;
    check_new_user(&username, &password, &role)?;
    let salt = hex::encode(crypto::random_bytes::<16>());
    let hash = hash_blocking(&app, password, salt.clone()).await?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let id = insert_user(&tx, &username, &role, &salt, &hash)?;
    app.audit(
        &tx,
        &u.username,
        "user_created",
        &format!("{username} ({role})"),
    )?;
    tx.commit()?;
    ok(json!({ "id": id }))
}

/// Change one's own password. All sessions of the account are revoked.
/// Wrong current passwords are throttled like sign-in failures (V-37).
async fn change_password(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Api {
    let (current, new) = (s(&body, "current"), s(&body, "new"));
    let t = now() as i64;
    let (u, salt, hash, key) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        password_policy(&u.username, &new)
            .map_err(|m| fail(StatusCode::BAD_REQUEST, &format!("new {m}")))?;
        let key = format!("pwchange:{}", u.id);
        if failures_of(&db, &key, t - THROTTLE_WINDOW)? >= PER_ACCOUNT_FAILURES {
            return Err(fail(
                StatusCode::TOO_MANY_REQUESTS,
                "too many wrong current passwords; try again in 15 minutes",
            ));
        }
        let (salt, hash): (String, String) = db.query_row(
            "SELECT salt, hash FROM users WHERE id = ?",
            params![u.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        (u, salt, hash, key)
    };
    if current.len() > 1024 {
        return Err(fail(StatusCode::FORBIDDEN, "current password is incorrect"));
    }
    let computed = hash_blocking(&app, current, salt).await?;
    let nsalt: [u8; 16] = crypto::random_bytes();
    let nhash = if crypto::constant_time_eq(&computed, &hash) {
        Some(hash_blocking(&app, new, hex::encode(nsalt)).await?)
    } else {
        None
    };
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(nhash) = nhash else {
        tx.execute(
            "INSERT INTO login_failures(username, at, client) VALUES (?,?,NULL)",
            params![key, t],
        )?;
        app.audit(&tx, &u.username, "password_change_failed", "")?;
        tx.commit()?;
        return Err(fail(StatusCode::FORBIDDEN, "current password is incorrect"));
    };
    tx.execute(
        "UPDATE users SET salt = ?, hash = ? WHERE id = ?",
        params![hex::encode(nsalt), nhash, u.id],
    )?;
    tx.execute("DELETE FROM sessions WHERE user_id = ?", params![u.id])?;
    tx.execute(
        "DELETE FROM login_failures WHERE username = ?",
        params![key],
    )?;
    app.audit(&tx, &u.username, "password_changed", "all sessions revoked")?;
    tx.commit()?;
    ok(json!({ "changed": true, "sessions_revoked": true }))
}

/// Activate or deactivate an account (administrator). Deactivation revokes its sessions.
async fn set_user_active(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let mut db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin"])?;
    let Some(active) = body.get("active").and_then(|v| v.as_bool()) else {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "active must be true or false",
        ));
    };
    if id == u.id && !active {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "you cannot deactivate your own account",
        ));
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if tx.execute(
        "UPDATE users SET active = ? WHERE id = ?",
        params![active, id],
    )? == 0
    {
        return Err(not_found());
    }
    if !active {
        tx.execute("DELETE FROM sessions WHERE user_id = ?", params![id])?;
    }
    app.audit(
        &tx,
        &u.username,
        if active {
            "user_activated"
        } else {
            "user_deactivated"
        },
        &format!("user #{id}"),
    )?;
    tx.commit()?;
    ok(json!({ "active": active }))
}

async fn add_member(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let mut db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin"])?;
    let name = clip(&s(&body, "username"), MAX_USERNAME);
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let uid: Option<i64> = tx
        .query_row(
            "SELECT id FROM users WHERE username = ?",
            params![name],
            |r| r.get(0),
        )
        .optional()?;
    let (Some(uid), true) = (uid, project_exists(&tx, id)?) else {
        return Err(not_found());
    };
    let remove = body
        .get("remove")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if remove {
        tx.execute(
            "DELETE FROM project_members WHERE project_id = ? AND user_id = ?",
            params![id, uid],
        )?;
    } else {
        tx.execute(
            "INSERT OR IGNORE INTO project_members(project_id, user_id) VALUES (?,?)",
            params![id, uid],
        )?;
    }
    app.audit(
        &tx,
        &u.username,
        if remove {
            "member_removed"
        } else {
            "member_added"
        },
        &format!("project #{id} user {name}"),
    )?;
    tx.commit()?;
    ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------- audit log

/// (id, at, username, action, detail, prev, hash)
type AuditRow = (
    i64,
    i64,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);

/// Walk the whole chain, then compare its end with the anchor.
fn verify_audit(app: &App, db: &Connection) -> rusqlite::Result<Value> {
    let rows: Vec<AuditRow> = q_all(
        db,
        "SELECT id, at, username, action, detail, prev, hash FROM audit ORDER BY id",
        [],
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
    )?;
    let mut prev = "genesis".to_string();
    let mut broken: Option<i64> = None;
    let mut last_id = 0;
    for (id, at, user, action, detail, p, h) in &rows {
        let expect = audit_hash(&prev, *at, user, action, detail);
        if p.as_deref() != Some(prev.as_str()) || h.as_deref() != Some(expect.as_str()) {
            broken = Some(*id);
            break;
        }
        prev = expect;
        last_id = *id;
    }
    let anchor: Option<Value> =
        storage::meta_get(db, "audit_anchor")?.and_then(|a| serde_json::from_str(&a).ok());
    let anchor_problem: Option<String> = match &anchor {
        None => Some("no anchor is stored: the end of the log cannot be confirmed".into()),
        Some(a) => {
            let (head, count, aid, mac) = (
                a["head"].as_str().unwrap_or(""),
                a["count"].as_i64().unwrap_or(-1),
                a["last_id"].as_i64().unwrap_or(-1),
                a["mac"].as_str().unwrap_or(""),
            );
            if !crypto::constant_time_eq(mac, &anchor_mac(&app.key, head, count, aid)) {
                Some(
                    "the anchor itself was altered (its authentication code does not verify)"
                        .into(),
                )
            } else if broken.is_some() {
                None // reported as a broken chain
            } else if count != rows.len() as i64 || aid != last_id || head != prev {
                Some(format!(
                    "the log ends at entry #{last_id} with {} entries, but the anchor records entry #{aid} and {count} entries: the newest entries were removed or the log was replaced",
                    rows.len()
                ))
            } else {
                None
            }
        }
    };
    // S-2: unreadable rows are an error above, never skipped; and the database itself is checked here,
    // so that a damaged file is not reported as an intact log.
    let database_problem = storage::integrity_check(db, false)?;
    Ok(
        json!({ "intact": broken.is_none() && anchor_problem.is_none() && database_problem.is_none(), "entries": rows.len(), "first_broken_entry": broken,
               "database_integrity": database_problem.clone().unwrap_or_else(|| "ok".into()), "database_intact": database_problem.is_none(),
               "chain_intact": broken.is_none(), "anchor_intact": anchor_problem.is_none(), "anchor_problem": anchor_problem,
               "limitation": "Detects alteration or removal of entries inside the chain, and removal of the newest entries: the latest hash, the entry count and the latest id are stored as an anchor authenticated with the storage key. Someone who can write the database file AND read the storage key can rebuild the chain and the anchor without trace. Someone who can write the database file can also restore an older copy of the whole database together with its older anchor (rollback); that is not detected. An anchor held outside this machine is not implemented." }),
    )
}

async fn audit_verify(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin"])?;
    match verify_audit(&app, &db) {
        Ok(v) => ok(v),
        Err(e) if db_is_busy(&e) => Err(e.into()),
        // S-2: a log that cannot be read is not an intact log
        Err(e) => {
            eprintln!("nf-server: audit log cannot be read: {e}");
            ok(
                json!({ "intact": false, "chain_intact": false, "anchor_intact": false, "database_intact": false, "entries": Value::Null, "first_broken_entry": Value::Null,
                       "database_integrity": "the audit log could not be read: the database is damaged", "anchor_problem": Value::Null }),
            )
        }
    }
}

/// Newest first, at most `limit` (default 300, at most 1000) entries older
/// than `before`. Long stored texts are shortened in the listing (V-34).
async fn audit_log(
    State(app): Ctx,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin"])?;
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(300)
        .clamp(1, 1000);
    let before = q
        .get("before")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(i64::MAX);
    let rows = q_all(
        &db,
        "SELECT id, at, substr(username, 1, 64), substr(action, 1, 64), substr(detail, 1, 1000) FROM audit WHERE id < ? ORDER BY id DESC LIMIT ?",
        params![before, limit],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "at": r.get::<_, i64>(1)?, "username": r.get::<_, String>(2)?, "action": r.get::<_, String>(3)?, "detail": r.get::<_, String>(4)? }),
            )
        },
    )?;
    ok(json!(rows))
}

async fn security_status(State(app): Ctx, headers: HeaderMap) -> Api {
    let u = auth(&app, &headers)?;
    require(&u, &["admin"])?;
    ok(json!({
        "transport": "This server speaks plain HTTP. TLS must be provided by a terminating proxy; none is built in.",
        "cookie_secure_flag": app.cookie_secure,
        "session_lifetime_hours": 12,
        "password_hashing": "Argon2id (library defaults), per-user random salt",
        "login_throttling": "5 failed attempts per account per 15 minutes; 100 failed attempts per client address per 10 minutes; no global limit",
        "client_address_source": if app.trusted_proxies.is_empty() { "connection peer address (X-Forwarded-For is ignored: NF_TRUSTED_PROXY is not set)".to_string() } else { format!("X-Forwarded-For as set by the trusted proxy {:?}", app.trusted_proxies) },
        "allowed_hosts": app.allowed_hosts,
        "first_run_setup": "requires the one-time setup token of the installation",
        "multi_factor_authentication": false,
        "blob_encryption": "AES-256-GCM, one key for all blobs",
        "storage_key_outside_data_directory": app.key_outside_data_dir,
        "key_rotation": false,
        "database_encrypted_at_rest": false,
        "project_isolation": "Per-project membership enforced on every project, job, file, obligation and summary endpoint; administrators see all projects",
        "anti_forgery": "State-changing API requests require the X-NF-Request header and a same-origin Origin; session cookie is SameSite=Strict",
        "audit_log": "Hash-chained with an anchored end; verify with /api/audit/verify",
        "sandboxed_parsers": "Not applicable yet: uploaded files are stored, never parsed",
    }))
}

// ---------------------------------------------------------------- projects and documents

async fn list_projects(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let rows = q_all(
        &db,
        &format!(
            "SELECT p.id, p.name, p.description, p.created_by, p.created_at, (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id), (SELECT COUNT(*) FROM calculations c WHERE c.project_id = p.id) FROM projects p WHERE {} ORDER BY p.id DESC",
            scope_col(&u, "p.id")
        ),
        [],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "description": r.get::<_, String>(2)?, "created_by": r.get::<_, String>(3)?, "created_at": r.get::<_, i64>(4)?, "documents": r.get::<_, i64>(5)?, "calculations": r.get::<_, i64>(6)? }),
            )
        },
    )?;
    ok(json!(rows))
}

async fn add_project(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Api {
    let mut db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin", "engineer"])?;
    let name = s(&body, "name").trim().to_string();
    if name.is_empty() || name.len() > 200 {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "project name is required (max 200 characters)",
        ));
    }
    let description = text_field(&body, "description")?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "INSERT INTO projects(name, description, created_by, created_at) VALUES (?,?,?,?)",
        params![name, description, u.username, now() as i64],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "INSERT OR IGNORE INTO project_members(project_id, user_id) VALUES (?,?)",
        params![id, u.id],
    )?;
    app.audit(
        &tx,
        &u.username,
        "project_created",
        &format!("#{id} {name}"),
    )?;
    tx.commit()?;
    ok(json!({ "id": id }))
}

async fn get_project(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    project_guard(&app, &db, &u, id, "read project")?;
    let mut p: Value = db.query_row("SELECT id, name, description, created_by, created_at FROM projects WHERE id = ?", params![id], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "description": r.get::<_, String>(2)?, "created_by": r.get::<_, String>(3)?, "created_at": r.get::<_, i64>(4)? })))?;
    let docs = q_all(
        &db,
        "SELECT id, filename, detected_type, size, sha256, extension_mismatch, created_by, created_at FROM documents WHERE project_id = ? ORDER BY id",
        params![id],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "filename": r.get::<_, String>(1)?, "detected_type": r.get::<_, String>(2)?, "size": r.get::<_, i64>(3)?, "sha256": r.get::<_, String>(4)?, "extension_mismatch": r.get::<_, bool>(5)?, "created_by": r.get::<_, String>(6)?, "created_at": r.get::<_, i64>(7)? }),
            )
        },
    )?;
    let snapshot: Option<String> = db.query_row("SELECT id FROM records WHERE project_id = ? AND kind = 'snapshot' ORDER BY rowid DESC LIMIT 1", params![id], |r| r.get(0)).optional()?;
    p["documents"] = json!(docs);
    p["calculations"] = json!(calc_rows(
        &app,
        &db,
        &format!("WHERE c.project_id = {id}"),
        200
    )?);
    p["members"] = json!(q_all(
        &db,
        "SELECT u.username FROM project_members m JOIN users u ON u.id = m.user_id WHERE m.project_id = ? ORDER BY u.username",
        params![id],
        |r| r.get::<_, String>(0)
    )?);
    p["snapshot"] = json!(snapshot);
    ok(p)
}

/// Store a document. The caller is authenticated and authorised BEFORE one
/// byte of the body is read; the size limit is enforced while the body
/// streams in; only a few uploads run at a time (V-35).
async fn upload(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    body: axum::body::Body,
) -> Api {
    use http_body_util::BodyExt;
    let u = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        project_guard(&app, &db, &u, id, "upload")?;
        u
    };
    let too_large = || {
        fail(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!(
                "file exceeds the upload limit of {} MiB",
                membrane::MAX_UPLOAD_BYTES / (1024 * 1024)
            ),
        )
    };
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|n| n > membrane::MAX_UPLOAD_BYTES as u64)
    {
        return Err(too_large());
    }
    let Ok(_slot) = app.upload_slots.try_acquire() else {
        return Err(fail(
            StatusCode::SERVICE_UNAVAILABLE,
            "too many uploads are in progress; retry shortly",
        ));
    };
    let mut bytes: Vec<u8> = Vec::new();
    let mut body = body;
    while let Some(frame) = body.frame().await {
        let frame =
            frame.map_err(|_| fail(StatusCode::BAD_REQUEST, "the upload was interrupted"))?;
        if let Some(chunk) = frame.data_ref() {
            if bytes.len() + chunk.len() > membrane::MAX_UPLOAD_BYTES {
                return Err(too_large());
            }
            bytes.extend_from_slice(chunk);
        }
    }
    let filename: String = clip(
        &headers
            .get("x-filename")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unnamed")
            .chars()
            .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
            .collect::<String>(),
        200,
    );
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    project_guard(&app, &tx, &u, id, "upload")?;
    let detected = match membrane::admit(&bytes) {
        membrane::Admission::Accepted { detected_type } => detected_type,
        membrane::Admission::Rejected { reason } => {
            app.audit(
                &tx,
                &u.username,
                "upload_rejected",
                &format!("project #{id} '{filename}': {reason}"),
            )?;
            tx.commit()?;
            return Err(fail(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!("file rejected at the membrane: {reason}"),
            ));
        }
    };
    let sha = app.store_blob(&bytes)?;
    let mismatch = membrane::extension_mismatch(&filename, detected);
    let artifact = Record::new(
        Family::Source,
        "source_artifact",
        json!({ "sha256": sha, "bytes": bytes.len(), "detected_type": detected, "filename": filename, "extension_mismatch": mismatch, "uploaded_by": u.username }),
        vec![],
        None,
    );
    app.put_record(&tx, id, &artifact)?;
    tx.execute(
        "INSERT INTO documents(project_id, filename, detected_type, size, sha256, extension_mismatch, record_id, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?)",
        params![id, filename, detected, bytes.len() as i64, sha, mismatch, artifact.id, u.username, now() as i64],
    )?;
    app.fault("upload.after_document")?;
    // New immutable snapshot of the whole project (engulfment), linked to its predecessor.
    let members: Vec<String> = q_all(
        &tx,
        "SELECT record_id FROM documents WHERE project_id = ? ORDER BY record_id",
        params![id],
        |r| r.get(0),
    )?;
    let prev: Option<String> = tx.query_row("SELECT id FROM records WHERE project_id = ? AND kind = 'snapshot' ORDER BY rowid DESC LIMIT 1", params![id], |r| r.get(0)).optional()?;
    let mut rels: Vec<nf_core::Rel> = members.iter().map(|m| rel("contains", m)).collect();
    if let Some(p) = &prev {
        rels.push(rel("predecessor", p));
    }
    let snap = Record::new(
        Family::Source,
        "snapshot",
        json!({ "project": id, "artifact_count": members.len() }),
        rels,
        None,
    );
    app.put_record(&tx, id, &snap)?;
    app.audit(
        &tx,
        &u.username,
        "upload_accepted",
        &format!("project #{id} '{filename}' {detected} {sha}"),
    )?;
    tx.commit()?;
    ok(
        json!({ "sha256": sha, "detected_type": detected, "extension_mismatch": mismatch, "snapshot": snap.id,
               "interpretation": "not implemented: the file is stored encrypted and hashed; no drawing content is read" }),
    )
}

// ---------------------------------------------------------------- effective state of a job (ONE function)

pub(crate) const REVIEW_REQUIRED: &str = "REVIEW_REQUIRED";
pub(crate) const REJECTED: &str = "REJECTED";
pub(crate) const REFUSED: &str = "REFUSED";

/// Gates the server evaluates itself at read time. A stored copy is never trusted.
const SERVER_GATES: [&str; 7] = ["G00", "G16", "G18", "G19", "G20", "G21", "G22"];

/// What every view needs to know about a stored job, read without trusting it.
struct Core {
    id: i64,
    project: i64,
    family: String,
    closure_col: String,
    created_by: String,
    created_at: i64,
    supersedes: Option<i64>,
    superseded_by: Option<i64>,
    replay_raw: Option<String>,
    state_col: String,
    json_ok: bool,
    gates_raw: Option<String>,
    engine: Option<String>,
    /// 1 if the stored result has at least one pattern, 0 if its pattern list is empty, None if it has no list.
    patterns: Option<i64>,
    outcome: Option<String>,
    closure_json: Option<String>,
    root_id: Option<String>,
    state_json: Option<String>,
}

/// The values `evaluate` needs from a stored result, as one JSON array, derived by SQL so that the
/// derivation is the same wherever it runs: gates, engine, outcome, closure hash, id of the last record
/// (the gate evaluation), state, part name of the first pattern, JSON type of the pattern list.
const CORE_EXPR: &str = "CASE WHEN json_valid(c.result_json) THEN json_array(json_extract(c.result_json, '$.gates'), json_extract(c.result_json, '$.engine'), json_extract(c.result_json, '$.outcome'), json_extract(c.result_json, '$.closure_hash'), json_extract(c.result_json, '$.records[#-1].id'), json_extract(c.result_json, '$.state'), json_extract(c.result_json, '$.patterns[0].part'), json_type(c.result_json, '$.patterns')) END";

/// A stored result is about 150 KB for an assembly, and parsing 200 of them per list request held the
/// database lock for 0.2 s (S-7). `core_json` is CORE_EXPR as computed when the job was last written,
/// `result_bytes` the byte length of the result it was computed from. A list uses the stored core only
/// while the result still has exactly that length; otherwise it derives it again from the result.
/// The detail view, the DXF download, replay, corrections and every write always derive it from the
/// result itself (`full = true`). Limit of the short cut: an alteration of a stored result that keeps
/// its byte length is seen by those, not by a list.
fn core_sql(full: bool) -> String {
    let fresh = "c.core_json IS NOT NULL AND c.result_bytes = length(CAST(c.result_json AS BLOB))";
    let (ok, core) = if full {
        (
            "json_valid(c.result_json)".to_string(),
            CORE_EXPR.to_string(),
        )
    } else {
        (
            format!("CASE WHEN {fresh} THEN 1 ELSE json_valid(c.result_json) END"),
            format!("CASE WHEN {fresh} THEN c.core_json ELSE {CORE_EXPR} END"),
        )
    };
    format!(
        "SELECT c.id, c.project_id, c.family, c.closure_hash, c.created_by, c.created_at, c.supersedes,
  (SELECT MIN(n.id) FROM calculations n WHERE n.supersedes = c.id),
  c.replay_json, c.state, {ok}, {core} FROM calculations c"
    )
}

fn core_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Core> {
    let parts: Vec<Value> = text_col(r, 11)?
        .and_then(|t| serde_json::from_str::<Vec<Value>>(&t).ok())
        .filter(|v| v.len() == 8)
        .unwrap_or_else(|| vec![Value::Null; 8]);
    let text = |i: usize| parts[i].as_str().map(String::from);
    Ok(Core {
        id: r.get(0)?,
        project: r.get(1)?,
        family: r.get(2)?,
        closure_col: r.get(3)?,
        created_by: r.get(4)?,
        created_at: r.get(5)?,
        supersedes: r.get(6)?,
        superseded_by: r.get(7)?,
        replay_raw: text_col(r, 8)?,
        state_col: text_col(r, 9)?.unwrap_or_default(),
        json_ok: r.get::<_, Option<bool>>(10)?.unwrap_or(false),
        gates_raw: if parts[0].is_array() {
            Some(parts[0].to_string())
        } else {
            None
        },
        engine: text(1),
        // a pattern counts only if it names its part; anything else is "no pattern" (the restrictive reading)
        patterns: if parts[7] == "array" {
            Some(i64::from(parts[6].is_string()))
        } else {
            None
        },
        outcome: text(2),
        closure_json: text(3),
        root_id: text(4),
        state_json: text(5),
    })
}

fn load_core(db: &Connection, id: i64) -> rusqlite::Result<Option<Core>> {
    db.query_row(
        &format!("{} WHERE c.id = ?", core_sql(true)),
        params![id],
        core_row,
    )
    .optional()
}

struct Eval {
    gates: Vec<Gate>,
    /// REVIEW_REQUIRED, REJECTED or REFUSED. There is no fourth value.
    state: &'static str,
    /// Reasons the stored result is not what the engine produced.
    integrity: Vec<String>,
    engine_current: bool,
    open_obligations: i64,
}

impl Eval {
    fn refused(&self) -> bool {
        self.state == REFUSED
    }
}

fn gate(id: &str, name: &str, result: GateResult, detail: String) -> Gate {
    Gate {
        id: id.into(),
        name: name.into(),
        critical: true,
        result,
        detail,
    }
}

/// THE state function. Detail view, lists, dashboard column, DXF download,
/// replay and corrections all go through it.
///
/// Fail closed, in this order:
/// 1. The stored result must be readable and consistent with its own
///    gate-evaluation record; otherwise integrity findings -> G22 FAIL -> REJECTED.
/// 2. A result without patterns is REFUSED (the engine computed nothing).
/// 3. The gate table is built from the required set: a required gate missing
///    from storage is a FAIL. G16 (approval) is always NOT_IMPLEMENTED, G18,
///    G19, G20, G00 and G21 are computed here from current data; stored
///    copies of them are discarded.
/// 4. A result computed by any engine other than the running one is REJECTED
///    (G21: "computed by a superseded engine version — recompute").
///
/// A database error is returned as an error; it is never read as "no
/// obligations" or "no advisories". The result can only be REVIEW_REQUIRED,
/// REJECTED or REFUSED.
fn evaluate(db: &Connection, c: &Core) -> rusqlite::Result<Eval> {
    use GateResult as R;
    let mut integrity: Vec<String> = Vec::new();
    let stored: Option<Vec<Gate>> = c
        .gates_raw
        .as_deref()
        .and_then(|g| serde_json::from_str(g).ok());
    if !c.json_ok {
        integrity.push("the stored result is not readable".into());
    } else {
        if stored.is_none() {
            integrity.push("the gate table is missing or unreadable".into());
        }
        if c.patterns.is_none() {
            integrity.push("the pattern list is missing".into());
        }
        if c.engine.is_none() {
            integrity.push("the engine identifier is missing".into());
        }
        if c.closure_json.as_deref() != Some(c.closure_col.as_str()) {
            integrity.push("the closure hash inside the stored result differs from the one recorded for the job".into());
        }
        match &c.root_id {
            None => integrity.push("the stored result has no gate-evaluation record".into()),
            Some(rid) => {
                let json: Option<String> = db
                    .query_row("SELECT json FROM records WHERE id = ?", params![rid], |r| {
                        text_col(r, 0)
                    })
                    .optional()?
                    .flatten();
                match json.and_then(|j| serde_json::from_str::<Record>(&j).ok()) {
                    Some(rec) if rec.verify_id() && &rec.id == rid && rec.kind == "gate_evaluation" => {
                        let rec_gates: Option<Vec<Gate>> = serde_json::from_value(rec.payload["gates"].clone()).ok();
                        if stored.is_some() && rec_gates != stored {
                            integrity.push("the gate table differs from the gate-evaluation record of the job".into());
                        }
                        if rec.payload["state"].as_str() != c.state_json.as_deref() {
                            integrity.push("the stored state differs from the gate-evaluation record of the job".into());
                        }
                    }
                    _ => integrity.push("the gate-evaluation record of the job is missing or fails its identity check".into()),
                }
            }
        }
    }
    let open: i64 = q_count(
        db,
        "SELECT COUNT(*) FROM obligations WHERE target_calc = ? AND state = 'open'",
        params![c.id],
    )?;
    let total: i64 = q_count(
        db,
        "SELECT COUNT(*) FROM obligations WHERE target_calc = ?",
        params![c.id],
    )?;
    let engine_current = c.engine.as_deref().is_some_and(nf_core::engine_is_current);

    if integrity.is_empty() && c.patterns == Some(0) {
        // The engine refused the request. Its own short gate table is shown as it is; nothing is added
        // that could make a refusal look like a result.
        return Ok(Eval {
            gates: stored.unwrap_or_default(),
            state: REFUSED,
            integrity,
            engine_current,
            open_obligations: open,
        });
    }

    let mut gates: Vec<Gate> = stored
        .unwrap_or_default()
        .into_iter()
        .filter(|g| !SERVER_GATES.contains(&g.id.as_str()))
        .collect();
    gates.push(gate("G16", "human_approval", R::NotImplemented, "Engineer approval workflow is not implemented. This result cannot be released for manufacturing.".into()));
    gates.push(gate(
        "G18",
        "obligation_closure",
        R::Inconclusive,
        if open > 0 {
            format!("{open} of {total} mandatory scrutiny obligations are OPEN. They cannot be closed by budget or time, only by evidence or an engineer's decision.")
        } else if total > 0 {
            format!("All {total} scrutiny obligations are resolved. The propagation rule table is provisional ({}), so closure cannot be certified.", nf_core::deps::RULE_TABLE_VERSION)
        } else {
            format!("No scrutiny obligations recorded for this result. The propagation rule table is provisional ({}).", nf_core::deps::RULE_TABLE_VERSION)
        },
    ));
    let (g19, d19) = match c.replay_raw.as_deref() {
        None => (R::NotRun, "No replay audit has been run for this result. Audits A1-A4 are not implemented.".to_string()),
        Some(raw) => match serde_json::from_str::<Value>(raw) {
            Err(_) => (R::Fail, "The stored replay report is unreadable. The result is rejected until a replay audit succeeds.".to_string()),
            // Reports written by earlier builds recorded a process failure as a mismatch (BE-008).
            Ok(r) if r["process_error"].is_string() => (R::NotRun, "An earlier replay attempt could not be run (infrastructure failure, not a finding about this result). Run the replay audit again.".to_string()),
            Ok(r) if r["matches"] == true => (R::Inconclusive, format!("Replay audit ({}) reproduced the release closure. Audits A1-A4 are not implemented, and the replay used the same machine and build.", r["kind"].as_str().unwrap_or("replay"))),
            Ok(_) => (R::Fail, "Replay audit did NOT reproduce the stored result. The result is rejected and must be investigated.".to_string()),
        },
    };
    gates.push(gate("G19", "self_consistency", g19, d19));
    let adv_open: i64 = q_count(
        db,
        "SELECT COUNT(*) FROM advisories WHERE calc_id = ? AND state = 'open'",
        params![c.id],
    )?;
    let adv_all: i64 = q_count(
        db,
        "SELECT COUNT(*) FROM advisories WHERE calc_id = ?",
        params![c.id],
    )?;
    let lessons_n: i64 = q_count(
        db,
        "SELECT COUNT(*) FROM lessons WHERE project_id = ?",
        params![c.project],
    )?;
    gates.push(gate(
        "G20",
        "known_mistakes",
        if adv_open > 0 { R::Inconclusive } else { R::Pass },
        if adv_open > 0 {
            format!("{adv_open} advisory(ies) open: this job repeats something that went wrong before in this project. An engineer must acknowledge each with a reason.")
        } else if adv_all > 0 {
            format!("{adv_all} advisory(ies) raised and acknowledged by an engineer.")
        } else {
            format!("No match among the {lessons_n} lesson(s) recorded in this project. Lessons cover only mistakes already seen here.")
        },
    ));
    // every required gate that storage did not deliver is a failed gate
    let mut gates = nf_core::with_required_gates(gates);
    if let Some(n) = c.superseded_by {
        gates.push(gate(
            "G00",
            "current_revision",
            R::Fail,
            format!("Superseded by job #{n} after a correction. This result must not be used."),
        ));
    }
    if !engine_current {
        let id = c.engine.as_deref().unwrap_or("not recorded");
        let why = nf_core::superseded_engine_reason(id)
            .map(|w| format!(" {w}"))
            .unwrap_or_default();
        gates.push(gate("G21", "engine_current", R::Fail, format!("Computed by a superseded engine version ({id}) — recompute. The running engine is {}; results of any other engine build are not trusted and their files are withheld.{why}", nf_core::ENGINE_VERSION)));
    }
    if !integrity.is_empty() {
        gates.push(gate("G22", "stored_result_integrity", R::Fail, format!("The stored result failed its integrity check: {}. Nothing of this job may be used.", integrity.join("; "))));
    }
    gates.sort_by(|a, b| a.id.cmp(&b.id));
    let state = match nf_core::release_state(&gates) {
        nf_core::ReleaseState::ReviewRequired if integrity.is_empty() => REVIEW_REQUIRED,
        // Rejected; and, failing closed, anything else the rule could ever say.
        _ => REJECTED,
    };
    Ok(Eval {
        gates,
        state,
        integrity,
        engine_current,
        open_obligations: open,
    })
}

/// Materialise the state column from `evaluate`, inside the caller's transaction.
fn refresh_state(db: &Connection, id: i64) -> rusqlite::Result<&'static str> {
    let Some(c) = load_core(db, id)? else {
        return Ok(REJECTED);
    };
    let e = evaluate(db, &c)?;
    db.execute(
        &format!("UPDATE calculations AS c SET state = ?, state_by = ?, core_json = {CORE_EXPR}, result_bytes = length(CAST(c.result_json AS BLOB)) WHERE c.id = ?"),
        params![e.state, nf_core::ENGINE_VERSION, id],
    )?;
    Ok(e.state)
}

/// At start-up: every job whose state column was written by another build is
/// evaluated again (an engine change alone turns its results into "recompute").
fn rematerialise_states(db: &mut Connection) -> rusqlite::Result<usize> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let ids: Vec<i64> = q_all(
        &tx,
        "SELECT id FROM calculations WHERE state_by IS NOT ? OR core_json IS NULL",
        params![nf_core::ENGINE_VERSION],
        |r| r.get(0),
    )?;
    for id in &ids {
        refresh_state(&tx, *id)?;
    }
    tx.commit()?;
    Ok(ids.len())
}

fn row_json(c: &Core, e: &Eval) -> Value {
    json!({ "id": c.id, "project_id": c.project, "family": c.family, "state": e.state, "closure_hash": c.closure_col, "created_by": c.created_by, "created_at": c.created_at,
            "replayed": c.replay_raw.is_some(), "superseded": c.superseded_by.is_some(), "open_obligations": e.open_obligations,
            "no_result": e.refused(), "outcome": if e.refused() { json!(c.outcome) } else { Value::Null }, "engine_current": e.engine_current, "integrity_failure": !e.integrity.is_empty() })
}

/// Job list rows. The state of each row comes from `evaluate`, exactly as in
/// the detail view (BE-009). A stale state column is corrected on the way.
///
/// The stored summary of a result (`core_json`) is used only for jobs that were
/// evaluated from their full stored result since the database last changed
/// under this process: SQLite's `data_version` changes whenever another
/// connection commits, and after a restart nothing counts as checked. So an
/// alteration made by another process or while the server was stopped, also
/// one that keeps the byte length, is seen by the next list (closes the gap
/// left by S-7). This process's own writes always re-derive the summary.
fn calc_rows(
    app: &App,
    db: &Connection,
    where_sql: &str,
    limit: usize,
) -> rusqlite::Result<Vec<Value>> {
    let version: i64 = db.query_row("PRAGMA data_version", [], |r| r.get(0))?;
    let mut cores = q_all(
        db,
        &format!(
            "{} {where_sql} ORDER BY c.id DESC LIMIT {}",
            core_sql(false),
            limit.clamp(1, 200)
        ),
        [],
        core_row,
    )?;
    {
        let mut seen = app.verified.lock().unwrap_or_else(|e| e.into_inner());
        if seen.0 != version {
            *seen = (version, Default::default());
        }
        for c in cores.iter_mut() {
            if !seen.1.contains(&c.id) {
                if let Some(full) = load_core(db, c.id)? {
                    *c = full;
                }
                // bring the stored summary in line with the result as it is now; only then may later lists use it
                match db.execute(&format!("UPDATE calculations AS c SET core_json = {CORE_EXPR}, result_bytes = length(CAST(c.result_json AS BLOB)) WHERE c.id = ?"), params![c.id]) {
                    Ok(_) => {
                        seen.1.insert(c.id);
                    }
                    Err(err) => eprintln!("nf-server: could not refresh the stored summary of job #{}: {err}", c.id),
                }
            }
        }
    }
    let mut out = Vec::with_capacity(cores.len());
    for c in &cores {
        let e = evaluate(db, c)?;
        if c.state_col != e.state
            && let Err(err) = db.execute(
                "UPDATE calculations SET state = ?, state_by = ? WHERE id = ?",
                params![e.state, nf_core::ENGINE_VERSION, c.id],
            )
        {
            eprintln!(
                "nf-server: could not correct the stored state of job #{}: {err}",
                c.id
            );
        }
        out.push(row_json(c, &e));
    }
    Ok(out)
}

/// Load a job the user may see, with its evaluation; 404 otherwise.
fn calc_guard(
    app: &App,
    db: &Connection,
    u: &User,
    id: i64,
    what: &str,
) -> Result<(Core, Eval), ApiError> {
    let Some(c) = load_core(db, id)? else {
        return Err(not_found());
    };
    if !can_access(db, u, c.project)? {
        if let Err(e) = app.audit(
            db,
            &u.username,
            "access_denied",
            &format!("{what}: job #{id}"),
        ) {
            eprintln!("nf-server: could not audit a refused access: {e}");
        }
        return Err(not_found());
    }
    let e = evaluate(db, &c)?;
    Ok((c, e))
}

/// The stored result as JSON, or `None` if it is not a readable JSON object.
fn stored_result(db: &Connection, id: i64) -> rusqlite::Result<Option<Value>> {
    let raw: Option<String> = db
        .query_row(
            "SELECT result_json FROM calculations WHERE id = ?",
            params![id],
            |r| text_col(r, 0),
        )
        .optional()?
        .flatten();
    Ok(raw
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(|v| v.is_object()))
}

// ---------------------------------------------------------------- jobs

/// Persist a finished job in the caller's transaction: records (append-only),
/// encrypted DXF blobs (atomic files, written before the commit), calculation row.
fn persist_calc(
    app: &App,
    db: &Connection,
    project: i64,
    out: &job::JobOutput,
    user: &str,
    supersedes: Option<i64>,
) -> Result<i64, ApiError> {
    for r in &out.records {
        app.put_record(db, project, r)?;
    }
    for (_, text) in &out.dxf {
        app.store_blob(text.as_bytes())?;
    }
    let result_json = serde_json::to_string(out).map_err(|e| ApiError::Internal(e.to_string()))?;
    db.execute(
        "INSERT INTO calculations(project_id, family, state, closure_hash, result_json, supersedes, created_by, created_at, engine) VALUES (?,?,?,?,?,?,?,?,?)",
        // the state is written by refresh_state below; REJECTED is the placeholder because it is the most restrictive
        params![project, out.request.family(), REJECTED, out.closure_hash, result_json, supersedes, user, now() as i64, out.engine],
    )?;
    let id = db.last_insert_rowid();
    refresh_state(db, id)?;
    Ok(id)
}

// A lesson is an immutable record of something that went wrong in a project:
// a value an engineer had to correct, a result the verifiers rejected, a
// stored result that failed its integrity check. Lessons never change a value
// and never approve anything. They are matched against every later job in the
// same project; a match raises an advisory that keeps the job in REVIEW
// REQUIRED until an engineer acknowledges it with a reason.

#[allow(clippy::too_many_arguments)]
fn record_lesson(
    app: &App,
    db: &Connection,
    project: i64,
    kind: &str,
    family: &str,
    parameter: &str,
    wrong: Option<f64>,
    right: Option<f64>,
    detail: &str,
    source_calc: i64,
    user: &str,
) -> rusqlite::Result<()> {
    let rec = Record::new(
        Family::Finding,
        "lesson",
        json!({ "kind": kind, "family": family, "parameter": parameter, "wrong_value": wrong, "right_value": right, "detail": detail, "source_calculation": source_calc, "recorded_by": user, "at": now() }),
        vec![],
        None,
    );
    app.put_record(db, project, &rec)?;
    db.execute(
        "INSERT INTO lessons(project_id, kind, family, parameter, wrong_value, right_value, detail, source_calc, record_id, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        params![project, kind, family, parameter, wrong, right, detail, source_calc, rec.id, user, now() as i64],
    )?;
    app.audit(
        db,
        user,
        "lesson_recorded",
        &format!("project #{project} {kind} {family} {parameter}"),
    )
}

/// (id, kind, parameter, wrong value, right value, detail, source job)
type LessonRow = (i64, String, String, Option<f64>, Option<f64>, String, i64);

/// Compare a new job with the lessons of its project and raise advisories.
fn raise_advisories(
    db: &Connection,
    project: i64,
    calc_id: i64,
    req: &Request,
) -> rusqlite::Result<usize> {
    let params_now = req.numeric_params();
    let req_json = serde_json::to_string(req).unwrap_or_default();
    let rows: Vec<LessonRow> = q_all(
        db,
        "SELECT id, kind, parameter, wrong_value, right_value, detail, source_calc FROM lessons WHERE project_id = ? AND family = ? ORDER BY id",
        params![project, req.family()],
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
    )?;
    let mut n = 0;
    for (lid, kind, parameter, wrong, right, detail, source) in rows {
        let message = match kind.as_str() {
            "corrected_value" => {
                let now_v = params_now
                    .iter()
                    .find(|(k, _)| *k == parameter)
                    .map(|(_, v)| *v);
                match (now_v, wrong) {
                    (Some(v), Some(w)) if (v - w).abs() <= 1e-9 * w.abs().max(1.0) => format!(
                        "{parameter} = {v} repeats a value that was corrected in this project (job #{source}: {w} -> {}; reason: {detail}). Confirm that {v} is right this time.",
                        right.unwrap_or(f64::NAN)
                    ),
                    _ => continue,
                }
            }
            "engine_failure" | "integrity_failure" if detail == req_json => format!(
                "This exact request produced a {} before in this project (job #{source}).",
                if kind == "engine_failure" {
                    "result rejected by verification"
                } else {
                    "stored result that failed its integrity check"
                }
            ),
            _ => continue,
        };
        db.execute("INSERT INTO advisories(calc_id, lesson_id, message, state, note) VALUES (?,?,?, 'open', '')", params![calc_id, lid, message])?;
        n += 1;
    }
    Ok(n)
}

async fn list_lessons(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let rows = q_all(
        &db,
        &format!(
            "SELECT id, project_id, kind, family, parameter, wrong_value, right_value, detail, source_calc, created_by, created_at, (SELECT COUNT(*) FROM advisories a WHERE a.lesson_id = lessons.id) FROM lessons WHERE {} ORDER BY id DESC LIMIT 300",
            scope(&u)
        ),
        [],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "project_id": r.get::<_, i64>(1)?, "kind": r.get::<_, String>(2)?, "family": r.get::<_, String>(3)?, "parameter": r.get::<_, String>(4)?, "wrong_value": r.get::<_, Option<f64>>(5)?, "right_value": r.get::<_, Option<f64>>(6)?,
                       "detail": r.get::<_, String>(7)?, "source_calc": r.get::<_, i64>(8)?, "created_by": r.get::<_, String>(9)?, "created_at": r.get::<_, i64>(10)?, "times_matched": r.get::<_, i64>(11)? }),
            )
        },
    )?;
    ok(json!(rows))
}

async fn acknowledge_advisory(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let mut db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin", "engineer"])?;
    let reason = text_field(&body, "reason")?;
    if reason.len() < 5 {
        return Err(fail(StatusCode::BAD_REQUEST, "a reason is required"));
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let row: Option<(i64, String)> = tx
        .query_row(
            "SELECT calc_id, state FROM advisories WHERE id = ?",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((calc_id, state)) = row else {
        return Err(not_found());
    };
    let project: Option<i64> = tx
        .query_row(
            "SELECT project_id FROM calculations WHERE id = ?",
            params![calc_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(project) = project else {
        return Err(not_found());
    };
    if !can_access(&tx, &u, project)? {
        return Err(not_found());
    }
    if state != "open" {
        return Err(fail(StatusCode::CONFLICT, "advisory is not open"));
    }
    let att = Record::new(
        Family::Attestation,
        "advisory_acknowledgement",
        json!({ "advisory": id, "calculation": calc_id, "reason": reason, "by": u.username, "role": u.role, "at": now() }),
        vec![],
        None,
    );
    app.put_record(&tx, project, &att)?;
    tx.execute("UPDATE advisories SET state = 'acknowledged', note = ?, resolved_by = ?, resolved_at = ? WHERE id = ? AND state = 'open'", params![reason, u.username, now() as i64, id])?;
    app.audit(
        &tx,
        &u.username,
        "advisory_acknowledged",
        &format!("#{id} job #{calc_id}: {reason}"),
    )?;
    refresh_state(&tx, calc_id)?;
    tx.commit()?;
    ok(json!({ "state": "acknowledged" }))
}

/// Run a job.
///
/// Contract:
/// - 200 `{id, state, advisories}`: a result was computed and stored (state
///   REVIEW_REQUIRED, or REJECTED if its own verification failed).
/// - 422 `{error, id, state: "REFUSED", outcome, field, advisories: 0}`: the
///   engine refused the request and computed nothing. The attempt is kept as
///   job `id` in state REFUSED (never REVIEW_REQUIRED) and in the audit log.
/// - 400: the body is not a request this build understands (unknown family,
///   wrong type, unknown field). Nothing is stored.
async fn calculate(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let u = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        // access is decided before any work is done (BE-028)
        project_guard(&app, &db, &u, id, "calculate")?;
        u
    };
    let req = Request::parse_strict(&body).map_err(|e| fail(StatusCode::BAD_REQUEST, &e))?;
    let out = job::run(&req, &u.username);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    project_guard(&app, &tx, &u, id, "calculate")?;
    let cid = persist_calc(&app, &tx, id, &out, &u.username, None)?;
    app.fault("calculate.after_persist")?;
    if out.state == nf_core::ReleaseState::Refused {
        let outcome = out
            .outcome
            .clone()
            .unwrap_or_else(|| "no pattern was produced".into());
        app.audit(
            &tx,
            &u.username,
            "calculation_refused",
            &format!("#{cid} project #{id}: {outcome}"),
        )?;
        tx.commit()?;
        return Err(fail_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": format!("request refused, nothing was computed: {outcome}"), "id": cid, "state": REFUSED, "outcome": outcome, "field": out.refused_field, "advisories": 0 }),
        ));
    }
    let advisories = raise_advisories(&tx, id, cid, &req)?;
    if out.state == nf_core::ReleaseState::Rejected {
        // the engine produced a pattern that its own verification rejected: remember the exact request
        record_lesson(
            &app,
            &tx,
            id,
            "engine_failure",
            &req.family(),
            "",
            None,
            None,
            &serde_json::to_string(&req).unwrap_or_default(),
            cid,
            &u.username,
        )?;
    }
    let state = refresh_state(&tx, cid)?;
    app.audit(
        &tx,
        &u.username,
        "calculation",
        &format!("#{cid} project #{id} -> {state}"),
    )?;
    tx.commit()?;
    ok(json!({ "id": cid, "state": state, "advisories": advisories }))
}

fn obligation_rows(
    db: &Connection,
    where_sql: &str,
    arg: Option<i64>,
) -> rusqlite::Result<Vec<Value>> {
    let sql = format!(
        "SELECT id, origin_calc, target_calc, kind, label, direction, via, required_check, state, note, resolved_by, resolved_at, created_at FROM obligations {where_sql} ORDER BY id DESC LIMIT 500"
    );
    let map = |r: &rusqlite::Row<'_>| {
        Ok(
            json!({ "id": r.get::<_, i64>(0)?, "origin_calc": r.get::<_, i64>(1)?, "target_calc": r.get::<_, i64>(2)?, "kind": r.get::<_, String>(3)?, "label": r.get::<_, String>(4)?,
                   "direction": r.get::<_, String>(5)?, "via": r.get::<_, String>(6)?, "required_check": r.get::<_, String>(7)?, "state": r.get::<_, String>(8)?, "note": r.get::<_, String>(9)?,
                   "resolved_by": r.get::<_, Option<String>>(10)?, "resolved_at": r.get::<_, Option<i64>>(11)?, "created_at": r.get::<_, i64>(12)? }),
        )
    };
    match arg {
        Some(a) => q_all(db, &sql, params![a], map),
        None => q_all(db, &sql, [], map),
    }
}

/// Newest first, at most 200 per request. `?before=<id>` continues with older
/// jobs and `?limit=` (1..200) shortens a page; without them the answer is the
/// newest 200, as before (S-7).
async fn list_calcs(
    State(app): Ctx,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let before = q
        .get("before")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(i64::MAX);
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(200)
        .clamp(1, 200);
    ok(json!(calc_rows(
        &app,
        &db,
        &format!(
            "WHERE {} AND c.id < {before}",
            scope_col(&u, "c.project_id")
        ),
        limit
    )?))
}

async fn get_calc(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let (c, e) = calc_guard(&app, &db, &u, id, "read job")?;
    let mut result = stored_result(&db, id)?.unwrap_or_else(|| json!({}));
    // Whatever storage delivered, the client always gets the fields it reads; absent ones are empty, never invented.
    for (k, empty) in [
        ("patterns", json!([])),
        ("checks", json!([])),
        ("records", json!([])),
        ("closure_size", json!(0)),
        ("work_units", json!(0)),
        ("engine", json!(c.engine.clone().unwrap_or_default())),
    ] {
        if result.get(k).is_none_or(|v| v.is_null()) || (empty.is_array() && !result[k].is_array())
        {
            result[k] = empty;
        }
    }
    if !result["request"].is_object() {
        result["request"] = json!({ "family": c.family });
    }
    if !e.integrity.is_empty() {
        // nothing of a result that failed its integrity check is offered as geometry
        result["patterns"] = json!([]);
        result["checks"] = json!([]);
        result["outcome"] = json!(format!(
            "The stored result failed its integrity check ({}). Nothing of this job may be used.",
            e.integrity.join("; ")
        ));
    }
    result["id"] = json!(id);
    result["project_id"] = json!(c.project);
    result["created_by"] = json!(c.created_by);
    result["closure_hash"] = json!(c.closure_col);
    result["gates"] = json!(e.gates);
    result["state"] = json!(e.state);
    result["refused"] = json!(e.refused());
    result["integrity"] = json!(e.integrity);
    result["engine_current"] = json!(e.engine_current);
    result["running_engine"] = json!(nf_core::ENGINE_VERSION);
    result["replay"] = c
        .replay_raw
        .as_deref()
        .and_then(|r| serde_json::from_str::<Value>(r).ok())
        .unwrap_or(Value::Null);
    result["supersedes"] = json!(c.supersedes);
    result["superseded_by"] = json!(c.superseded_by);
    result["obligations"] = json!(obligation_rows(&db, "WHERE target_calc = ?", Some(id))?);
    result["advisories"] = json!(q_all(
        &db,
        "SELECT id, lesson_id, message, state, note, resolved_by, resolved_at FROM advisories WHERE calc_id = ? ORDER BY id",
        params![id],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "lesson_id": r.get::<_, i64>(1)?, "message": r.get::<_, String>(2)?, "state": r.get::<_, String>(3)?, "note": r.get::<_, String>(4)?, "resolved_by": r.get::<_, Option<String>>(5)?, "resolved_at": r.get::<_, Option<i64>>(6)? }),
            )
        }
    )?);
    let correctable = e.state == REVIEW_REQUIRED && c.superseded_by.is_none();
    result["correctable_parameters"] = json!(
        serde_json::from_value::<Request>(result["request"].clone())
            .ok()
            .filter(|r| correctable && matches!(r, Request::HeaderWithBranch { .. }))
            .map(|r| r.numeric_params())
            .unwrap_or_default()
    );
    ok(result)
}

// ---------------------------------------------------------------- replay

/// `nf-server replay-check`: read {request, entered_by} on stdin, recompute in
/// this fresh process, print the engine id, the closure hash and record ids.
fn replay_child() {
    let mut input = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).is_err() {
        eprintln!("replay-check: cannot read the request");
        std::process::exit(2);
    }
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let Ok(req) = serde_json::from_value::<Request>(v["request"].clone()) else {
        eprintln!("replay-check: unreadable request");
        std::process::exit(2);
    };
    let out = job::run(&req, v["entered_by"].as_str().unwrap_or(""));
    println!(
        "{}",
        json!({ "engine": nf_core::ENGINE_VERSION, "closure_hash": out.closure_hash, "ids": out.records.iter().map(|r| r.id.clone()).collect::<Vec<_>>() })
    );
}

/// Run the replay child. `Err` means the replay could not be RUN (start
/// failure, crash, time-out, unreadable or foreign-build output): an
/// infrastructure problem that says nothing about the job (BE-008).
fn replay_in_separate_process(
    exe: Option<&std::path::Path>,
    timeout: Duration,
    request: &Value,
    entered_by: &str,
) -> Result<Value, String> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let exe = match exe {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe()
            .map_err(|e| format!("cannot locate the server executable: {e}"))?,
    };
    let mut child = Command::new(&exe)
        .arg("replay-check")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start the replay process: {e}"))?;
    let fed = match child.stdin.take() {
        Some(mut stdin) => stdin
            .write_all(
                json!({ "request": request, "entered_by": entered_by })
                    .to_string()
                    .as_bytes(),
            )
            .map_err(|e| format!("cannot send the request to the replay process: {e}")),
        None => Err("replay process has no input".to_string()),
    };
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Ok(st),
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!(
                    "replay process did not finish within {} s and was stopped",
                    timeout.as_secs_f32()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("cannot wait for the replay process: {e}"));
            }
        }
    };
    let (mut stdout, mut stderr) = (String::new(), String::new());
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut stdout);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }
    if !stderr.trim().is_empty() {
        eprintln!(
            "nf-server: replay process stderr: {}",
            clip(stderr.trim(), 2000)
        );
    }
    let status = status?;
    fed?;
    if !status.success() {
        return Err(format!("replay process exited with {status}"));
    }
    let v: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("replay process output is unreadable: {e}"))?;
    if !v["closure_hash"].is_string() || !v["ids"].is_array() {
        return Err("replay process output is incomplete".into());
    }
    match v["engine"].as_str() {
        Some(e) if e == nf_core::ENGINE_VERSION => Ok(v),
        other => Err(format!(
            "the replay executable is a different build (engine {}), not the running one ({}); restart the server after an upgrade",
            other.unwrap_or("not stated"),
            nf_core::ENGINE_VERSION
        )),
    }
}

/// Replay audit.
///
/// - 200 report: the replay RAN. `matches` says whether it reproduced the
///   stored closure and whether the stored records pass their identity check.
///   Only this outcome changes the job (G19, and REJECTED plus a lesson on a mismatch).
/// - 503 `{error, infrastructure: true}`: the replay could not be run. The job,
///   its state and its lessons are untouched; the attempt is in the audit log.
/// - 409: there is nothing to replay (refused request, superseded engine,
///   stored result failing its integrity check).
async fn replay(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let (u, c, request, ids) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        let (c, e) = calc_guard(&app, &db, &u, id, "replay")?;
        if !e.integrity.is_empty() {
            return Err(fail(
                StatusCode::CONFLICT,
                "the stored result failed its integrity check; it is REJECTED and cannot be replayed",
            ));
        }
        if e.refused() {
            return Err(fail(
                StatusCode::CONFLICT,
                "nothing to replay: the request was refused and nothing was computed",
            ));
        }
        if !e.engine_current {
            return Err(fail(
                StatusCode::CONFLICT,
                "computed by a superseded engine version — recompute. A replay by the running engine cannot reproduce it",
            ));
        }
        let Some(result) = stored_result(&db, id)? else {
            return Err(fail(
                StatusCode::CONFLICT,
                "the stored result is unreadable",
            ));
        };
        let ids: Vec<String> = result["records"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|r| r["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        (u, c, result["request"].clone(), ids)
    };
    // Recompute in a separate operating-system process that receives only the request.
    let child = {
        let _slot = app
            .replay_slots
            .acquire()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        let (exe, timeout, req, by) = (
            app.replay_exe.clone(),
            app.replay_timeout,
            request.clone(),
            c.created_by.clone(),
        );
        tokio::task::spawn_blocking(move || {
            replay_in_separate_process(exe.as_deref(), timeout, &req, &by)
        })
        .await
        .unwrap_or_else(|e| Err(format!("replay task failed: {e}")))
    };
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let v = match child {
        Ok(v) => v,
        Err(problem) => {
            eprintln!("nf-server: replay of job #{id} could not be run: {problem}");
            app.audit(
                &tx,
                &u.username,
                "replay_unavailable",
                &format!("calculation #{id}: {problem}"),
            )?;
            tx.commit()?;
            return Err(fail_json(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({ "error": format!("the replay audit could not be run ({problem}). This is an infrastructure problem, not a finding about the job: its state is unchanged. Retry later."), "infrastructure": true }),
            ));
        }
    };
    // Confirm that the stored records are intact (identity recomputed from content).
    let mut tampered = 0;
    for rid in &ids {
        let stored: Option<String> = tx
            .query_row("SELECT json FROM records WHERE id = ?", params![rid], |r| {
                text_col(r, 0)
            })
            .optional()?
            .flatten();
        match stored.and_then(|j| serde_json::from_str::<Record>(&j).ok()) {
            Some(r) if r.verify_id() && &r.id == rid => {}
            _ => tampered += 1,
        }
    }
    let got_ids: Vec<String> = v["ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let got = v["closure_hash"].as_str().unwrap_or("").to_string();
    let matches = got == c.closure_col && got_ids == ids && tampered == 0 && !ids.is_empty();
    let report = json!({ "kind": "separate_process_replay", "matches": matches, "expected_closure_hash": c.closure_col, "replayed_closure_hash": got, "process_error": Value::Null,
                         "engine": nf_core::ENGINE_VERSION, "stored_records_checked": ids.len(), "stored_records_failing_identity_check": tampered, "run_by": u.username, "at": now(),
                         "limitation": "Replay ran in a separate process that received only the request, on the same machine with the same build (the child reported the same engine identifier). A separately built binary or separate machine is not used. Discrepancies are reported, never repaired. This audit shows reproducibility, not correctness." });
    let audit_rec = Record::new(
        Family::Assessment,
        "replay_audit",
        report.clone(),
        ids.last()
            .map(|g| vec![rel("audits", g)])
            .unwrap_or_default(),
        None,
    );
    app.put_record(&tx, c.project, &audit_rec)?;
    tx.execute(
        "UPDATE calculations SET replay_json = ? WHERE id = ?",
        params![report.to_string(), id],
    )?;
    app.fault("replay.after_report")?;
    if !matches && let Ok(req) = serde_json::from_value::<Request>(request) {
        record_lesson(
            &app,
            &tx,
            c.project,
            "integrity_failure",
            &req.family(),
            "",
            None,
            None,
            &serde_json::to_string(&req).unwrap_or_default(),
            id,
            &u.username,
        )?;
    }
    refresh_state(&tx, id)?;
    app.audit(
        &tx,
        &u.username,
        "replay_audit",
        &format!(
            "calculation #{id}: {}",
            if matches { "match" } else { "MISMATCH" }
        ),
    )?;
    tx.commit()?;
    ok(report)
}

// ---------------------------------------------------------------- DXF download

/// A DXF is served for exactly one situation: the job is the current revision
/// and `evaluate` says REVIEW_REQUIRED. Everything else is refused.
async fn get_dxf(
    State(app): Ctx,
    headers: HeaderMap,
    Path((id, part)): Path<(i64, String)>,
) -> Api {
    let mut db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let (c, e) = calc_guard(&app, &db, &u, id, "download DXF")?;
    if let Some(n) = c.superseded_by {
        return Err(fail(
            StatusCode::CONFLICT,
            &format!("this result was superseded by job #{n}; its files are withheld"),
        ));
    }
    if e.refused() {
        return Err(fail(
            StatusCode::NOT_FOUND,
            "no DXF for this part: the request was refused and nothing was computed",
        ));
    }
    if e.state != REVIEW_REQUIRED {
        let why = if !e.integrity.is_empty() {
            " (the stored result failed its integrity check)"
        } else if !e.engine_current {
            " (computed by a superseded engine version — recompute)"
        } else {
            ""
        };
        return Err(fail(
            StatusCode::CONFLICT,
            &format!("this result is REJECTED{why}; its files are withheld"),
        ));
    }
    let Some(result) = stored_result(&db, id)? else {
        return Err(fail(
            StatusCode::CONFLICT,
            "this result is REJECTED; its files are withheld",
        ));
    };
    let sha = result["patterns"]
        .as_array()
        .and_then(|a| a.iter().find(|p| p["part"] == part.as_str()))
        .and_then(|p| p["dxf_sha256"].as_str().map(String::from));
    let Some(sha) = sha else {
        return Err(fail(StatusCode::NOT_FOUND, "no DXF for this part"));
    };
    if !storage::is_sha256_hex(&sha) {
        eprintln!("nf-server: job #{id} part {part} has an invalid file reference");
        return Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the stored file reference of this part is invalid (integrity failure); the file is withheld",
        ));
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let bytes = match storage::load_blob(&app.data_dir, &app.key, &sha) {
        Ok(b) => b,
        Err(problem) => {
            // The job is deterministic: the file can be restored from the request, and only if the
            // recomputed text has exactly the recorded hash (BE-006).
            eprintln!(
                "nf-server: DXF {sha} of job #{id} part {part}: {problem}; trying to restore it by recomputation"
            );
            let again = serde_json::from_value::<Request>(result["request"].clone())
                .ok()
                .map(|req| job::run(&req, &c.created_by));
            let text = again.and_then(|o| {
                o.dxf
                    .into_iter()
                    .find(|(p, t)| *p == part && nf_core::sha256_hex(t.as_bytes()) == sha)
                    .map(|(_, t)| t)
            });
            match text {
                Some(t) => {
                    app.store_blob(t.as_bytes())?;
                    app.audit(&tx, "-", "stored_file_restored", &format!("calculation #{id} part {part} sha {sha}: {problem}; restored by recomputation with identical hash"))?;
                    t.into_bytes()
                }
                None => {
                    app.audit(
                        &tx,
                        "-",
                        "stored_file_damaged",
                        &format!(
                            "calculation #{id} part {part} sha {sha}: {problem}; not restorable"
                        ),
                    )?;
                    tx.commit()?;
                    return Err(fail_json(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        json!({ "error": format!("the stored DXF of this part is damaged ({problem}) and recomputation did not reproduce it. The file is withheld. Run the calculation again to obtain a new, verified file."), "recoverable": "run the calculation again" }),
                    ));
                }
            }
        }
    };
    app.audit(
        &tx,
        &u.username,
        "dxf_export",
        &format!("calculation #{id} part {part} sha {sha}"),
    )?;
    tx.commit()?;
    let name = format!(
        "NOT-RELEASED_REVIEW_REQUIRED_calc{id}_{part}_{}.dxf",
        &sha[..8]
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/dxf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

// ---------------------------------------------------------------- corrections and obligations

/// Record an engineer's correction to one parameter of an assembly job:
/// propagate along the dependency edges of the old result, recompute, and
/// create a scrutiny obligation for everything affected.
///
/// The corrected request is computed FIRST. If the engine refuses it or its
/// verification rejects it, the answer is 422 and nothing changes: the old job
/// stays current, no lesson is recorded (the old value was not shown to be a
/// mistake), no obligation is raised; only the audit log keeps the attempt
/// (BE-004). Otherwise everything is written in one transaction (BE-005).
async fn correct(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let pname = clip(&s(&body, "parameter"), 64);
    let reason = text_field(&body, "reason")?;
    let (u, c, req, records) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        let Some(new_value) = body
            .get("new_value")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
        else {
            return Err(fail(
                StatusCode::BAD_REQUEST,
                "new_value must be a finite number",
            ));
        };
        let _ = new_value;
        if reason.len() < 5 {
            return Err(fail(
                StatusCode::BAD_REQUEST,
                "a reason is required for every correction",
            ));
        }
        let (c, e) = calc_guard(&app, &db, &u, id, "correct")?;
        if c.superseded_by.is_some() {
            return Err(fail(
                StatusCode::CONFLICT,
                "this result is already superseded; correct the current one",
            ));
        }
        if e.refused() {
            return Err(fail(
                StatusCode::CONFLICT,
                "this request was refused and has no result to correct; submit a new calculation",
            ));
        }
        if !e.integrity.is_empty() || !e.engine_current {
            return Err(fail(
                StatusCode::CONFLICT,
                "this result cannot be corrected: it failed its integrity check or was computed by a superseded engine version — recompute it first",
            ));
        }
        let Some(result) = stored_result(&db, id)? else {
            return Err(fail(
                StatusCode::CONFLICT,
                "the stored result is unreadable",
            ));
        };
        let Ok(req) = serde_json::from_value::<Request>(result["request"].clone()) else {
            return Err(fail(
                StatusCode::CONFLICT,
                "the stored request is unreadable",
            ));
        };
        let records: std::collections::BTreeMap<String, Record> =
            serde_json::from_value::<Vec<Record>>(result["records"].clone())
                .unwrap_or_default()
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect();
        (u, c, req, records)
    };
    let new_value = body
        .get("new_value")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::NAN);
    if !matches!(req, Request::HeaderWithBranch { .. }) {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "corrections with dependency propagation are implemented for assembly jobs only",
        ));
    }
    let Some(old_value) = req
        .numeric_params()
        .into_iter()
        .find(|(k, _)| *k == pname)
        .map(|(_, v)| v)
    else {
        return Err(fail(StatusCode::BAD_REQUEST, "unknown parameter"));
    };
    if old_value == new_value {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "new value equals the current value",
        ));
    }
    let Some(new_req) = req.with_param(&pname, new_value) else {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "parameter cannot be corrected",
        ));
    };
    let Some(old_param) = records
        .values()
        .find(|r| r.kind == "parameter" && r.payload["name"] == pname.as_str())
        .map(|r| r.id.clone())
    else {
        return Err(fail(
            StatusCode::CONFLICT,
            "this job has no record of that parameter, so a correction cannot be propagated",
        ));
    };
    // 1. compute the corrected job; nothing has been written yet
    let out = job::run(&new_req, &u.username);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let refused = match out.state {
        nf_core::ReleaseState::ReviewRequired if !out.patterns.is_empty() => None,
        nf_core::ReleaseState::Rejected => Some(
            "the corrected request produced a pattern that failed its own verification".to_string(),
        ),
        _ => Some(
            out.outcome
                .clone()
                .unwrap_or_else(|| "the corrected request produced no pattern".into()),
        ),
    };
    if let Some(why) = refused {
        app.audit(&tx, &u.username, "correction_refused", &format!("calculation #{id} {pname}: {old_value} -> {new_value} ({reason}); refused: {why}; nothing changed"))?;
        tx.commit()?;
        return Err(fail_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": format!("correction refused, nothing was changed: {pname} = {new_value} cannot be computed ({why}). Job #{id} is still the current result."), "outcome": why, "field": out.refused_field.clone().unwrap_or_else(|| pname.clone()), "state": REFUSED }),
        ));
    }
    // 2. the old job must still be the current revision (another correction may have won the race)
    if load_core(&tx, id)?.is_none_or(|now| now.superseded_by.is_some()) {
        return Err(fail(
            StatusCode::CONFLICT,
            "this result is already superseded; correct the current one",
        ));
    }
    // 3. one transaction: attestation, new job, lesson, advisories, obligations, audit
    let attestation = Record::new(
        Family::Attestation,
        "correction",
        json!({ "parameter": pname, "old_value": old_value, "new_value": new_value, "reason": reason, "by": u.username, "at": now(), "calculation": id }),
        vec![rel("corrects", &old_param)],
        None,
    );
    app.put_record(&tx, c.project, &attestation)?;
    let affected = nf_core::deps::propagate(&records, &old_param);
    let new_id = persist_calc(&app, &tx, c.project, &out, &u.username, Some(id))?;
    app.fault("correct.after_persist")?;
    record_lesson(
        &app,
        &tx,
        c.project,
        "corrected_value",
        &req.family(),
        &pname,
        Some(old_value),
        Some(new_value),
        &reason,
        id,
        &u.username,
    )?;
    raise_advisories(&tx, c.project, new_id, &new_req)?;
    let mut open = 0;
    for a in &affected {
        let (done, note) = nf_core::deps::discharge(a, &out);
        if !done {
            open += 1;
        }
        let dir = serde_json::to_value(a.direction)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let finding = Record::new(
            Family::Finding,
            "obligation",
            json!({ "target": a, "rule_table": nf_core::deps::RULE_TABLE_VERSION, "initial_state": if done { "discharged" } else { "open" }, "note": note, "new_calculation_closure": out.closure_hash }),
            vec![rel("origin", &attestation.id), rel("target", &a.record_id)],
            None,
        );
        app.put_record(&tx, c.project, &finding)?;
        tx.execute(
            "INSERT INTO obligations(origin_calc, target_calc, record_id, kind, label, direction, via, required_check, state, note, created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?)",
            params![id, new_id, finding.id, a.kind, a.label, dir, a.via, a.required_check, if done { "discharged" } else { "open" }, note, now() as i64],
        )?;
        app.fault("correct.in_obligations")?;
    }
    refresh_state(&tx, id)?;
    refresh_state(&tx, new_id)?;
    app.audit(&tx, &u.username, "correction", &format!("calculation #{id} {pname}: {old_value} -> {new_value} ({reason}); new #{new_id}; {} obligations, {open} open", affected.len()))?;
    tx.commit()?;
    ok(json!({ "new_calculation": new_id, "obligations": affected.len(), "open": open }))
}

async fn list_obligations(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    ok(json!(obligation_rows(
        &db,
        &format!(
            "WHERE target_calc IN (SELECT id FROM calculations WHERE {})",
            scope(&u)
        ),
        None
    )?))
}

async fn resolve_obligation(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let mut db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    require(&u, &["admin", "engineer"])?;
    let (decision, reason) = (s(&body, "decision"), text_field(&body, "reason")?);
    let new_state = match decision.as_str() {
        "confirmed" => "discharged_by_decision",
        "waived" => "waived",
        _ => {
            return Err(fail(
                StatusCode::BAD_REQUEST,
                "decision must be 'confirmed' or 'waived'",
            ));
        }
    };
    if reason.len() < 5 {
        return Err(fail(StatusCode::BAD_REQUEST, "a reason is required"));
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let row: Option<(String, i64, String)> = tx
        .query_row(
            "SELECT state, target_calc, record_id FROM obligations WHERE id = ?",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((state, target, record_id)) = row else {
        return Err(not_found());
    };
    let project: Option<i64> = tx
        .query_row(
            "SELECT project_id FROM calculations WHERE id = ?",
            params![target],
            |r| r.get(0),
        )
        .optional()?;
    let Some(project) = project else {
        return Err(not_found());
    };
    if !can_access(&tx, &u, project)? {
        return Err(not_found());
    }
    if state != "open" {
        return Err(fail(StatusCode::CONFLICT, "obligation is not open"));
    }
    let att = Record::new(
        Family::Attestation,
        "obligation_resolution",
        json!({ "decision": decision, "reason": reason, "by": u.username, "role": u.role, "at": now() }),
        vec![rel("resolves", &record_id)],
        None,
    );
    app.put_record(&tx, project, &att)?;
    tx.execute("UPDATE obligations SET state = ?, note = ?, resolved_by = ?, resolved_at = ? WHERE id = ? AND state = 'open'", params![new_state, reason, u.username, now() as i64, id])?;
    app.audit(
        &tx,
        &u.username,
        "obligation_resolved",
        &format!("#{id} {decision}: {reason}"),
    )?;
    refresh_state(&tx, target)?;
    tx.commit()?;
    ok(json!({ "state": new_state }))
}

async fn dependencies(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    calc_guard(&app, &db, &u, id, "read dependencies")?;
    let records: Vec<Record> = stored_result(&db, id)?
        .and_then(|r| serde_json::from_value(r["records"].clone()).ok())
        .unwrap_or_default();
    let name = |r: &Record| -> String {
        r.payload
            .get("part")
            .or_else(|| r.payload.get("name"))
            .and_then(|v| v.as_str())
            .map(|x| format!("{} · {x}", r.kind))
            .unwrap_or_else(|| r.kind.clone())
    };
    let by_id: std::collections::BTreeMap<&str, &Record> =
        records.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut edges = Vec::new();
    for r in records.iter().filter(|r| r.family == Family::Claim) {
        for e in &r.relations {
            if let Some(t) = by_id.get(e.to.as_str())
                && [
                    "computed_from",
                    "developed_from",
                    "rendered_from",
                    "defined_by",
                    "pattern_of",
                    "mates_at",
                    "interface_feature",
                    "positioned_by",
                    "convention_of",
                ]
                .contains(&e.rel.as_str())
            {
                edges.push(json!({ "from": name(r), "rel": e.rel, "to": name(t) }));
            }
        }
    }
    ok(json!({ "edges": edges, "rule_table": nf_core::deps::RULE_TABLE_VERSION }))
}

// ---------------------------------------------------------------- summaries

/// Counts by effective state. The state column is written by `refresh_state`
/// (the same `evaluate` as the detail view) in the transaction of every event
/// that can change it. A superseded job is counted as superseded only, a
/// refused request as refused only; neither is "review required" (BE-009).
/// `approved` is the constant 0: this build cannot approve anything.
async fn dashboard(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let sc = scope(&u);
    let recent = calc_rows(
        &app,
        &db,
        &format!("WHERE {}", scope_col(&u, "c.project_id")),
        8,
    )?;
    let n = |sql: String| q_count(&db, &sql, []);
    let calcs = format!("SELECT COUNT(*) FROM calculations WHERE {sc}");
    let current = "NOT EXISTS (SELECT 1 FROM calculations n WHERE n.supersedes = calculations.id)";
    ok(json!({
        "projects": n(format!("SELECT COUNT(*) FROM projects WHERE {}", scope_col(&u, "id")))?,
        "documents": n(format!("SELECT COUNT(*) FROM documents WHERE {sc}"))?,
        "calculations": n(calcs.clone())?,
        "review_required": n(format!("{calcs} AND state = 'REVIEW_REQUIRED' AND {current}"))?,
        "rejected": n(format!("{calcs} AND state = 'REJECTED' AND {current}"))?,
        "superseded": n(format!("{calcs} AND NOT {current}"))?,
        "refused": n(format!("{calcs} AND state = 'REFUSED'"))?,
        "approved": 0,
        "open_obligations": n(format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND target_calc IN (SELECT id FROM calculations WHERE {sc})"))?,
        "records": n(format!("SELECT COUNT(*) FROM records WHERE {sc}"))?,
        "recent": recent,
        "uptime_seconds": now().saturating_sub(app.started),
        "engine": nf_core::ENGINE_VERSION,
    }))
}

async fn amoeba_summary(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let sc = scope(&u);
    let kinds = q_all(
        &db,
        &format!(
            "SELECT family, kind, COUNT(*) FROM records WHERE {sc} GROUP BY family, kind ORDER BY family, kind"
        ),
        [],
        |r| {
            Ok(
                json!({ "family": r.get::<_, String>(0)?, "kind": r.get::<_, String>(1)?, "count": r.get::<_, i64>(2)? }),
            )
        },
    )?;
    let mut work: std::collections::BTreeMap<String, (i64, i64)> = Default::default();
    for (kind, wu) in q_all(
        &db,
        &format!(
            "SELECT kind, CASE WHEN json_valid(json) THEN json_extract(json, '$.payload.work_units') END FROM records WHERE family = 'activity' AND {sc}"
        ),
        [],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1).unwrap_or(None),
            ))
        },
    )? {
        let e = work.entry(kind).or_insert((0, 0));
        e.0 += 1;
        e.1 += wu.unwrap_or(0);
    }
    let snaps = q_all(
        &db,
        &format!(
            "SELECT project_id, id FROM records WHERE kind = 'snapshot' AND {sc} ORDER BY rowid DESC LIMIT 10"
        ),
        [],
        |r| Ok(json!({ "project_id": r.get::<_, i64>(0)?, "snapshot": r.get::<_, String>(1)? })),
    )?;
    let in_scope = format!("target_calc IN (SELECT id FROM calculations WHERE {sc})");
    ok(json!({
        "records_by_kind": kinds,
        "activities": work.into_iter().map(|(k, (n, wu))| json!({ "type": k, "distinct_activities": n, "declared_work_units": wu })).collect::<Vec<_>>(),
        "recent_snapshots": snaps,
        "implemented": ["content-addressed append-only records", "project snapshots with predecessor links", "typed dependency edges (derivation, interface, common cause)", "dependency propagation with scrutiny obligations (provisional rule table)", "interface constraint verification between parts", "lessons from corrections and failures, matched against every later job in the project", "corrections as attestations with supersession", "release closure hash", "gate evaluation with obligation and replay state", "replay audit in a separate process", "membrane admission checks"],
        "obligations": { "open": q_count(&db, &format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND {in_scope}"), [])?, "total": q_count(&db, &format!("SELECT COUNT(*) FROM obligations WHERE {in_scope}"), [])? },
        "not_implemented": ["round scheduler and budgets", "exploration probes", "hypothesis-driven investigation", "coverage ledger", "lanes and overlays", "quarantine", "strategy switching", "self-consistency audits A1-A4", "replay on a separate machine or build", "engineer approval"],
    }))
}

// ---------------------------------------------------------------- transport

fn asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

/// Host header without its port, lower case.
fn host_name(host: &str) -> String {
    let h = host.trim();
    let name = if h.starts_with('[') {
        h.split_inclusive(']').next().unwrap_or(h)
    } else {
        h.rsplit_once(':').map(|(n, _)| n).unwrap_or(h)
    };
    name.to_ascii_lowercase()
}

/// The client's address. X-Forwarded-For is believed only when the connection
/// comes from a configured trusted proxy; then the right-most address that is
/// not itself a trusted proxy is the client (V-33).
fn client_address(app: &App, peer: Option<IpAddr>, headers: &HeaderMap) -> String {
    let Some(peer) = peer else {
        return "unknown".into();
    };
    if app.trusted_proxies.contains(&peer) {
        let hops: Vec<IpAddr> = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .filter_map(|h| h.trim().parse::<IpAddr>().ok())
            .collect();
        if let Some(ip) = hops
            .iter()
            .rev()
            .find(|ip| !app.trusted_proxies.contains(ip))
        {
            return ip.to_string();
        }
    }
    peer.to_string()
}

/// Applied to every request: Host allow-list, anti-forgery checks on
/// state-changing API calls, and security headers on every response.
async fn security_layer(
    State(app): Ctx,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let state_changing = !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    let is_api = req.uri().path().starts_with("/api/");
    let peer = req.extensions().get::<PeerAddr>().map(|p| p.0);
    let client = ClientAddr(client_address(&app, peer, req.headers()));
    req.extensions_mut().insert(client);
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .or_else(|| req.uri().authority().map(|a| a.to_string()))
        .unwrap_or_default();
    let refusal: Option<ApiError> = (|| {
        // A browser tricked into resolving a foreign name to this server (DNS rebinding) still sends
        // the foreign name as Host; only configured names are answered (V-36).
        if !app.allowed_hosts.contains(&host_name(&host)) {
            return Some(fail(
                StatusCode::MISDIRECTED_REQUEST,
                "this server does not answer to that host name (set NF_ALLOWED_HOSTS)",
            ));
        }
        if is_api && state_changing {
            let h = req.headers();
            if h.get("x-nf-request").is_none() {
                return Some(fail(StatusCode::FORBIDDEN, "missing anti-forgery header"));
            }
            if let Some(origin) = h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
                let host = h
                    .get(header::HOST)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if origin.split("://").nth(1) != Some(host) {
                    return Some(fail(StatusCode::FORBIDDEN, "cross-origin request refused"));
                }
            }
        }
        None
    })();
    let mut resp = match refusal {
        Some(r) => r.into_response(),
        // S-3: a request (headers are timed by the connection, the body and the work by this) may not
        // be held open indefinitely. Handlers write inside synchronous transactions, so a request that
        // is abandoned here has either committed completely or not at all.
        None => match tokio::time::timeout(app.request_timeout, next.run(req)).await {
            Ok(r) => r,
            Err(_) => fail(
                StatusCode::REQUEST_TIMEOUT,
                "the request was not completed in time and was abandoned",
            )
            .into_response(),
        },
    };
    let h = resp.headers_mut();
    let set = |h: &mut HeaderMap, k: &'static str, v: &'static str| {
        h.insert(k, HeaderValue::from_static(v));
    };
    set(
        h,
        "content-security-policy",
        "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
    );
    set(h, "x-content-type-options", "nosniff");
    set(h, "x-frame-options", "DENY");
    set(h, "referrer-policy", "no-referrer");
    set(
        h,
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()",
    );
    set(h, "cross-origin-opener-policy", "same-origin");
    if is_api {
        set(h, "cache-control", "no-store");
    }
    resp
}

pub(crate) fn router(app: Arc<App>) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async {
                asset(
                    "text/html; charset=utf-8",
                    include_str!("../../../web/index.html"),
                )
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../../../web/app.js"),
                )
            }),
        )
        .route(
            "/viewer.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../../../web/viewer.js"),
                )
            }),
        )
        .route(
            "/styles.css",
            get(|| async {
                asset(
                    "text/css; charset=utf-8",
                    include_str!("../../../web/styles.css"),
                )
            }),
        )
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
        // the upload handler reads its body itself, after authentication, with its own limit (V-35)
        .route("/api/projects/{id}/documents", post(upload))
        .route("/api/projects/{id}/calculations", post(calculate))
        .route("/api/calculations", get(list_calcs))
        .route("/api/calculations/{id}", get(get_calc))
        .route("/api/calculations/{id}/replay", post(replay))
        .route("/api/calculations/{id}/dxf/{part}", get(get_dxf))
        .route("/api/calculations/{id}/corrections", post(correct))
        .route("/api/calculations/{id}/dependencies", get(dependencies))
        .route("/api/lessons", get(list_lessons))
        .route(
            "/api/advisories/{id}/acknowledge",
            post(acknowledge_advisory),
        )
        .route("/api/obligations", get(list_obligations))
        .route("/api/obligations/{id}/resolve", post(resolve_obligation))
        .route("/api/dashboard", get(dashboard))
        .route("/api/amoeba/summary", get(amoeba_summary))
        .route("/api/audit", get(audit_log))
        .layer(axum::extract::DefaultBodyLimit::max(256 * 1024))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            security_layer,
        ))
        .with_state(app)
}

// ---------------------------------------------------------------- start-up

pub(crate) struct Config {
    pub data_dir: PathBuf,
    pub key_path: PathBuf,
    pub cookie_secure: bool,
    pub allowed_hosts: Vec<String>,
    pub trusted_proxies: Vec<IpAddr>,
    /// A setup token supplied by the installer; otherwise one is generated when needed.
    pub setup_token: Option<String>,
    pub request_timeout: Duration,
}

/// Connection limits (S-3) and shutdown behaviour (S-8). All configurable.
struct Limits {
    /// Longest wait for a complete request head, also on an idle connection.
    header_read: Duration,
    max_connections: usize,
    /// How long in-flight requests get to finish after SIGTERM / SIGINT.
    shutdown_grace: Duration,
}

fn env_secs(name: &str, default: u64, min: u64, max: u64) -> Result<u64, String> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(v) => v
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|n| (min..=max).contains(n))
            .ok_or_else(|| format!("{name} must be a whole number from {min} to {max}")),
    }
}

impl Limits {
    fn from_env() -> Result<Limits, String> {
        Ok(Limits {
            header_read: Duration::from_secs(env_secs("NF_HEADER_TIMEOUT_SECS", 15, 1, 600)?),
            max_connections: env_secs("NF_MAX_CONNECTIONS", 512, 1, 100_000)? as usize,
            shutdown_grace: Duration::from_secs(env_secs("NF_SHUTDOWN_GRACE_SECS", 20, 0, 600)?),
        })
    }
}

fn default_hosts() -> Vec<String> {
    vec!["localhost".into(), "127.0.0.1".into(), "[::1]".into()]
}

impl Config {
    fn from_env() -> Result<Config, String> {
        let data_dir =
            PathBuf::from(std::env::var("NF_DATA_DIR").unwrap_or_else(|_| "data".into()));
        // The storage key should live apart from the data it protects: set NF_KEY_FILE to a path outside the data directory.
        let key_path = std::env::var("NF_KEY_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("storage.key"));
        let mut allowed_hosts = default_hosts();
        if let Ok(v) = std::env::var("NF_ALLOWED_HOSTS") {
            for h in v.split(',').map(|h| h.trim()).filter(|h| !h.is_empty()) {
                if !h
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
                {
                    return Err(format!("NF_ALLOWED_HOSTS: '{h}' is not a host name"));
                }
                allowed_hosts.push(host_name(h));
            }
        }
        let mut trusted_proxies = Vec::new();
        if let Ok(v) = std::env::var("NF_TRUSTED_PROXY") {
            for p in v.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()) {
                trusted_proxies.push(
                    p.parse::<IpAddr>()
                        .map_err(|_| format!("NF_TRUSTED_PROXY: '{p}' is not an IP address"))?,
                );
            }
        }
        let setup_token = match std::env::var("NF_SETUP_TOKEN") {
            Ok(t) if t.trim().len() >= 16 => Some(t.trim().to_string()),
            Ok(_) => return Err("NF_SETUP_TOKEN must have at least 16 characters".into()),
            Err(_) => None,
        };
        Ok(Config {
            data_dir,
            key_path,
            cookie_secure: std::env::var("NF_COOKIE_SECURE")
                .map(|v| v == "1")
                .unwrap_or(false),
            allowed_hosts,
            trusted_proxies,
            setup_token,
            request_timeout: Duration::from_secs(env_secs(
                "NF_REQUEST_TIMEOUT_SECS",
                120,
                1,
                3600,
            )?),
        })
    }
}

/// Open storage and build the application state. Every failure is a message
/// for the operator; the caller exits non-zero.
pub(crate) fn build_app(cfg: Config) -> Result<Arc<App>, String> {
    let (mut db, key) = storage::open_storage(&cfg.data_dir, &cfg.key_path)?;
    storage::remove_stale_temporaries(&cfg.data_dir);
    let n = rematerialise_states(&mut db)
        .map_err(|e| format!("cannot evaluate stored jobs at start-up: {e}"))?;
    if n > 0 {
        println!(
            "{n} stored job(s) were evaluated again by this build (engine {}). Results of other engine builds are flagged: computed by a superseded engine version — recompute.",
            nf_core::ENGINE_VERSION
        );
    }
    let users: i64 = q_count(&db, "SELECT COUNT(*) FROM users", [])
        .map_err(|e| format!("cannot read users: {e}"))?;
    let token_file = cfg.data_dir.join("setup-token");
    let setup_token = if users == 0 {
        let token = match &cfg.setup_token {
            Some(t) => {
                println!(
                    "First-run setup: no administrator exists yet. Setup requires the token given in NF_SETUP_TOKEN."
                );
                t.clone()
            }
            None => {
                let t = hex::encode(crypto::random_bytes::<16>());
                let _ = std::fs::remove_file(&token_file);
                let mut opts = std::fs::OpenOptions::new();
                opts.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    opts.mode(0o600);
                }
                let mut f = opts.open(&token_file).map_err(|e| {
                    format!(
                        "cannot write the setup token file {}: {e}",
                        token_file.display()
                    )
                })?;
                std::io::Write::write_all(&mut f, format!("{t}\n").as_bytes()).map_err(|e| {
                    format!(
                        "cannot write the setup token file {}: {e}",
                        token_file.display()
                    )
                })?;
                println!(
                    "First-run setup: no administrator exists yet. The one-time setup token is in {} (readable by the service user only). It is needed to create the first administrator and is deleted afterwards.",
                    token_file.display()
                );
                t
            }
        };
        Some(nf_core::sha256_hex(token.as_bytes()))
    } else {
        let _ = std::fs::remove_file(&token_file);
        None
    };
    let app = App {
        db: Mutex::new(db),
        key,
        key_outside_data_dir: !cfg.key_path.starts_with(&cfg.data_dir),
        data_dir: cfg.data_dir,
        started: now(),
        cookie_secure: cfg.cookie_secure,
        allowed_hosts: cfg.allowed_hosts,
        trusted_proxies: cfg.trusted_proxies,
        setup_token: Mutex::new(setup_token),
        replay_exe: None,
        replay_timeout: Duration::from_secs(60),
        request_timeout: cfg.request_timeout,
        hash_slots: Semaphore::new(2),
        upload_slots: Semaphore::new(4),
        replay_slots: Semaphore::new(2),
        anon_audit: Mutex::new((0, 0, 0)),
        verified: Mutex::new((-1, Default::default())),
        #[cfg(test)]
        fault: Mutex::new(None),
    };
    {
        // V-25: a database from before the anchor gets one for the log as it stands now.
        let mut db = app.db();
        let mut anchor = || -> rusqlite::Result<()> {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if storage::meta_get(&tx, "audit_anchor")?.is_none() {
                let last: Option<(i64, Option<String>)> = tx
                    .query_row(
                        "SELECT id, hash FROM audit ORDER BY id DESC LIMIT 1",
                        [],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let count = q_count(&tx, "SELECT COUNT(*) FROM audit", [])?;
                let (last_id, head) = last
                    .map(|(i, h)| (i, h.unwrap_or_default()))
                    .unwrap_or((0, "genesis".into()));
                app.write_anchor(&tx, &head, count, last_id)?;
                if count > 0 {
                    app.audit(&tx, "-", "audit_anchored", &format!("anchor created for an existing log of {count} entries; removal of entries before this point cannot be ruled out"))?;
                }
            }
            tx.commit()
        };
        anchor().map_err(|e| format!("cannot anchor the audit log: {e}"))?;
    }
    Ok(Arc::new(app))
}

/// Accept loop with the limits axum's own `serve` does not offer (S-3, S-8):
/// a time limit for the request head, a cap on open connections, and a
/// graceful stop: on SIGTERM or SIGINT no new connection is accepted and
/// requests in flight get `shutdown_grace` to finish.
async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    limits: Limits,
) -> Result<(), String> {
    use tokio::signal::unix::{SignalKind, signal};
    let slots = Arc::new(Semaphore::new(limits.max_connections));
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut term = signal(SignalKind::terminate())
        .map_err(|e| format!("cannot install the SIGTERM handler: {e}"))?;
    let mut int = signal(SignalKind::interrupt())
        .map_err(|e| format!("cannot install the SIGINT handler: {e}"))?;
    let mut builder = hyper::server::conn::http1::Builder::new();
    builder
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(limits.header_read);
    loop {
        // a connection is accepted only when a slot is free; the rest wait in the kernel's queue
        let accepted = tokio::select! {
            _ = term.recv() => None,
            _ = int.recv() => None,
            a = async {
                let permit = slots.clone().acquire_owned().await;
                (permit, listener.accept().await)
            } => Some(a),
        };
        let Some((permit, accepted)) = accepted else {
            break;
        };
        let (stream, addr) = match accepted {
            Ok(x) => x,
            Err(e) => {
                eprintln!("nf-server: cannot accept a connection: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let service = hyper_util::service::TowerToHyperService::new(
            router.clone().layer(Extension(PeerAddr(addr.ip()))),
        );
        let conn =
            graceful.watch(builder.serve_connection(hyper_util::rt::TokioIo::new(stream), service));
        tokio::spawn(async move {
            let _ = conn.await; // a broken or timed-out connection is the client's problem, not an error of the server
            drop(permit);
        });
    }
    println!(
        "nf-server: stop requested; no new connections. Waiting up to {} s for requests in flight.",
        limits.shutdown_grace.as_secs()
    );
    tokio::select! {
        _ = graceful.shutdown() => println!("nf-server: all requests finished; stopped."),
        _ = tokio::time::sleep(limits.shutdown_grace) => eprintln!("nf-server: requests still in flight after the grace period; stopping. Their transactions are rolled back by the database."),
    }
    Ok(())
}

/// Remove expired sessions and sign-in failures older than the throttle window. Returns rows removed.
fn housekeeping(app: &App) -> rusqlite::Result<usize> {
    let db = app.db();
    let t = now() as i64;
    Ok(
        db.execute("DELETE FROM sessions WHERE expires_at <= ?", params![t])?
            + db.execute(
                "DELETE FROM login_failures WHERE at < ?",
                params![t - THROTTLE_WINDOW],
            )?,
    )
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("replay-check") {
        replay_child();
        return;
    }
    let stop = |msg: String| -> ! {
        eprintln!("nf-server: cannot start: {msg}");
        std::process::exit(2);
    };
    let addr = std::env::var("NF_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let cfg = Config::from_env().unwrap_or_else(|e| stop(e));
    let limits = Limits::from_env().unwrap_or_else(|e| stop(e));
    let app = build_app(cfg).unwrap_or_else(|e| stop(e));
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| stop(format!("cannot bind {addr}: {e}")));
    println!(
        "NEXUS FABRIC development build listening on http://{addr}  (data: {}; engine {})",
        app.data_dir.display(),
        nf_core::ENGINE_VERSION
    );
    println!(
        "Answering to host names: {}. Client address from: {}.",
        app.allowed_hosts.join(", "),
        if app.trusted_proxies.is_empty() {
            "the connection (NF_TRUSTED_PROXY not set)".to_string()
        } else {
            format!("X-Forwarded-For of {:?}", app.trusted_proxies)
        }
    );
    println!("NOT VALIDATED - no output of this software may be used for manufacturing.");
    println!(
        "Limits: request head {} s, whole request {} s, {} connections.",
        limits.header_read.as_secs(),
        app.request_timeout.as_secs(),
        limits.max_connections
    );
    // BE-033: expired sessions and old sign-in failures are removed every hour, not only at the next sign-in.
    let keeper = app.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = housekeeping(&keeper) {
                eprintln!("nf-server: housekeeping failed: {e}");
            }
        }
    });
    let service = router(app.clone());
    if let Err(e) = serve(listener, service, limits).await {
        eprintln!("nf-server: server error: {e}");
        std::process::exit(1);
    }
    // flush: fold the write-ahead log into the database file before the process ends
    if let Err(e) = app.db().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
        eprintln!("nf-server: could not checkpoint the database at shutdown: {e}");
    }
}
