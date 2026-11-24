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

/// A log line on stderr that cannot panic: a closed or full stderr must not turn an answer into an empty reply (RA-011).
macro_rules! elog {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod crypto;
mod desktop;
mod licence;
mod membrane;
mod pdfconv;
mod storage;
#[cfg(test)]
mod tests;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use nf_core::job::{self, Request};
use nf_core::{Family, Gate, GateResult, Record, rel};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
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
    /// Desktop mode (`--desktop`): one person on their own computer. Told to the client (task 2.4), which then shows
    /// what desktop mode does and does not protect on its first-run screen.
    desktop: bool,
    /// Longest a request body may stall between two chunks (RA-001).
    body_idle: Duration,
    /// Requests one client address may have open at the same time (RA-001).
    per_client_in_flight: usize,
    in_flight: Mutex<std::collections::HashMap<String, usize>>,
    hash_slots: Semaphore,
    upload_slots: Semaphore,
    replay_slots: Semaphore,
    /// The PDF to DXF converter program (task 3.1); `None`: conversions are unavailable.
    pdf2dxf_exe: Option<PathBuf>,
    /// The server's own kill timeout for one conversion.
    convert_timeout: Duration,
    /// One conversion at a time; the permit is held by the task that waits for the child.
    convert_slots: Arc<Semaphore>,
    /// (window start, unauthenticated audit rows written, suppressed) — V-34.
    anon_audit: Mutex<(u64, u32, u32)>,
    /// Tests pin the minute of the anonymous-audit window, so that the rate limit does not depend on the clock.
    anon_minute: Mutex<Option<u64>>,
    /// (database data_version, jobs whose stored summary was checked against the full result at that version).
    /// (database version, job id -> findings of the full check against the records). See `calc_rows`.
    verified: Mutex<(i64, std::collections::HashMap<i64, Vec<String>>)>,
    /// Licence cache: file payload, persisted fallback, monotonic time (licence.rs).
    licence: Mutex<licence::LicenceCache>,
    /// Issuer public keys compiled into the build.
    licence_keys: &'static [(&'static str, [u8; 32])],
    /// The licence key file (NF_LICENCE_FILE).
    licence_path: PathBuf,
    /// Owner's switch: refuse new work in read-only (NF_LICENCE_ENFORCE; default off).
    licence_enforce: bool,
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
    /// The audit log does not end where its anchor says: nothing is written until an administrator checks it (RA-003).
    AuditBroken(String),
    /// A stored file could not be written because the disk is full (RA-010).
    StorageFull(String),
}

/// Carried inside `rusqlite::Error::ToSqlConversionFailure` so that the handler can tell a broken audit chain
/// from other database errors (RA-003). The text is for the log only; the client gets a fixed message.
#[derive(Debug)]
struct AuditChainBroken(String);
impl std::fmt::Display for AuditChainBroken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for AuditChainBroken {}

type Api = Result<Response, ApiError>;

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::ToSqlConversionFailure(b) = &e
            && let Some(broken) = b.downcast_ref::<AuditChainBroken>()
        {
            elog!("nf-server: audit chain refused a write: {broken}");
            return ApiError::AuditBroken(broken.0.clone());
        }
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
                elog!("nf-server: database busy: {e}");
                let mut r = (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "the database is busy; nothing was changed. Retry shortly.", "retry": true }))).into_response();
                r.headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from_static("2"));
                r
            }
            ApiError::Db(e) if db_is_full(&e) => {
                elog!("nf-server: STORAGE FULL: {e}");
                (StatusCode::INSUFFICIENT_STORAGE, Json(json!({ "error": "the server's storage is full; nothing was changed and nothing was recorded. An administrator must free space.", "storage_full": true }))).into_response()
            }
            ApiError::Db(e) => {
                elog!("nf-server: database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal storage error; nothing was changed. The server log has the details." }))).into_response()
            }
            ApiError::Internal(e) => {
                elog!("nf-server: internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal error; nothing was changed. The server log has the details." }))).into_response()
            }
            ApiError::AuditBroken(why) => {
                elog!("nf-server: audit log is broken; writes refused: {why}");
                (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
                    "error": "the audit log does not match its anchor (entries were removed or the log was replaced). Nothing was changed; an administrator must check /api/audit/verify.",
                    "error_ja": "監査ログがアンカーと一致しません（エントリが削除されたか、ログが置き換えられた可能性があります）。何も変更されていません。管理者が /api/audit/verify を確認してください。",
                    "audit_broken": true
                }))).into_response()
            }
            ApiError::StorageFull(why) => {
                elog!("nf-server: storage full: {why}");
                (StatusCode::INSUFFICIENT_STORAGE, Json(json!({
                    "error": "the server's storage is full; nothing was changed and nothing was recorded. An administrator must free space.",
                    "error_ja": "サーバーのストレージが満杯です。何も変更・記録されていません。管理者が空き容量を確保してください。",
                    "storage_full": true
                }))).into_response()
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

/// The body of a calculation or correction request, read as JSON before anything else (as the framework's JSON
/// extractor did, so the order of the answers is unchanged). A body that is not JSON, or sent with another
/// content type, is refused with a code; the statuses are those the extractor gave (400, 415).
fn request_json(headers: &HeaderMap, raw: &[u8]) -> Result<Value, ApiError> {
    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let mime = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if !(mime == "application/json"
        || (mime.starts_with("application/") && mime.ends_with("+json")))
    {
        return Err(fail_code(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            nf_core::refusal::REQUEST_MALFORMED,
            &[],
            "the request must be sent with Content-Type: application/json",
        ));
    }
    serde_json::from_slice(raw).map_err(|e| {
        fail_code(
            StatusCode::BAD_REQUEST,
            nf_core::refusal::REQUEST_MALFORMED,
            &[],
            &format!("the request body is not JSON: {e}"),
        )
    })
}

/// A refused calculation or correction request: the answer `fail` gives, with the stable code, the request
/// fields the rule concerns, the message and the exact limit where the engine gives one (REFUSAL_CODES.md).
/// Adds the detail of a refusal to an answer, and only where the engine computes one (REFUSAL_CODES.md).
fn with_detail(mut v: Value, r: &nf_core::Refusal) -> Value {
    if let Some(d) = &r.detail {
        v["detail"] = json!(d);
    }
    v
}

fn fail_refused(status: StatusCode, r: &nf_core::Refusal) -> ApiError {
    fail_json(
        status,
        with_detail(
            json!({ "error": r.message, "code": r.code, "fields": r.fields, "message": r.message }),
            r,
        ),
    )
}

/// `fail_refused` for a rule the API applies itself.
fn fail_code(status: StatusCode, code: &str, fields: &[&str], msg: &str) -> ApiError {
    fail_refused(status, &nf_core::Refusal::new(code, fields, msg))
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

/// Longest a request body may stall between two chunks (RA-001). This is an idle bound, not a whole-body bound: a
/// large upload over a slow mobile link keeps sending chunks and is not cut. A client that sends nothing for this
/// long is answered 408 and its slot is released.
const BODY_IDLE: Duration = Duration::from_secs(10);

/// The client key of a request that reached this server through a local proxy that is not configured in
/// NF_TRUSTED_PROXY. Every such request would share one key, so the per-address limits do not apply to it (they
/// would otherwise lock every account and cap every client at once). Only the account gate applies.
const UNATTRIBUTED: &str = "unattributed";

/// A loopback peer that forwards a client address, while that peer is not a configured trusted proxy. This is
/// the live deployment when NF_TRUSTED_PROXY is missing (Caddy on the same host).
fn unattributed_peer(app: &App, peer: Option<IpAddr>, headers: &HeaderMap) -> bool {
    peer.map(v4_of)
        .is_some_and(|p| p.is_loopback() && !app.trusted_proxies.contains(&p))
        && headers.contains_key("x-forwarded-for")
}

/// An IPv4 client that reaches a `[::]` listener appears as `::ffff:a.b.c.d`. It is the same client as `a.b.c.d`,
/// so every check (loopback, trusted proxy, key) uses the IPv4 form (review item 3 residual).
fn v4_of(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        IpAddr::V4(_) => ip,
    }
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
        // RA-013: an entry and its anchor are written together, also when the caller has no transaction of its
        // own (a savepoint nests inside a transaction and is a transaction of its own otherwise).
        db.execute_batch("SAVEPOINT nf_audit")?;
        match self.audit_entry(db, user, action, detail) {
            Ok(()) => db.execute_batch("RELEASE nf_audit"),
            Err(e) => {
                let _ = db.execute_batch("ROLLBACK TO nf_audit; RELEASE nf_audit");
                Err(e)
            }
        }
    }

    fn audit_entry(
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
        // RA-003: the chain is extended only from a verified anchor that matches the newest entry. A log that was
        // truncated (and whose anchor was therefore not updated) is refused here, never re-signed.
        let (head, count, _last) = self.checked_chain(db)?;
        let at = now() as i64;
        let hash = audit_hash(&head, at, &user, &action, &detail);
        db.execute(
            "INSERT INTO audit(at, username, action, detail, prev, hash) VALUES (?,?,?,?,?,?)",
            params![at, user, action, detail, head, hash],
        )?;
        let last_id = db.last_insert_rowid();
        self.write_anchor(db, &hash, count + 1, last_id)
    }

    /// The anchor (head, count, last id), once it is verified against the key and the log. Any mismatch is an
    /// error that the caller's transaction turns into nothing written.
    fn checked_chain(&self, db: &Connection) -> rusqlite::Result<(String, i64, i64)> {
        let broken =
            |why: String| rusqlite::Error::ToSqlConversionFailure(Box::new(AuditChainBroken(why)));
        let anchor: Value = storage::meta_get(db, "audit_anchor")?
            .and_then(|a| serde_json::from_str(&a).ok())
            .ok_or_else(|| broken("the audit anchor is missing or unreadable".into()))?;
        let head = anchor["head"].as_str().unwrap_or("").to_string();
        let count = anchor["count"].as_i64().unwrap_or(-1);
        let aid = anchor["last_id"].as_i64().unwrap_or(-1);
        let mac = anchor["mac"].as_str().unwrap_or("");
        if !crypto::constant_time_eq(mac, &anchor_mac(&self.key, &head, count, aid)) {
            return Err(broken("the audit anchor does not verify".into()));
        }
        let newest: Option<(i64, Option<String>)> = db
            .query_row(
                "SELECT id, hash FROM audit ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let rows = q_count(db, "SELECT COUNT(*) FROM audit", [])?;
        let (nid, nhash) = newest
            .map(|(i, h)| (i, h.unwrap_or_default()))
            .unwrap_or((0, "genesis".into()));
        if head != nhash || aid != nid || count != rows {
            return Err(broken(format!(
                "the log ends at entry #{nid} with {rows} entries, but the anchor records entry #{aid} and {count} entries: entries were removed or the log was replaced"
            )));
        }
        Ok((head, count, aid))
    }

    fn write_anchor(
        &self,
        db: &Connection,
        head: &str,
        count: i64,
        last_id: i64,
    ) -> rusqlite::Result<()> {
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
        let minute = self
            .anon_minute
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap_or(now() / 60);
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
            storage::store_blob(&self.data_dir, &self.key, bytes).map_err(|e| match e {
                storage::StoreError::Full(m) => ApiError::StorageFull(m),
                storage::StoreError::Failed(m) => ApiError::Internal(m),
            })?;
        if repaired {
            elog!(
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
            elog!("nf-server: could not audit a refused access: {e}");
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
/// `mode` ("desktop" or "server") tells the client whether to show the desktop
/// security notice on that screen (task 2.4).
async fn health(State(app): Ctx) -> Api {
    // licence cache before the database lock (lock order, licence.rs)
    let licence = licence::health_field(&app);
    let db = app.db();
    let users = q_count(&db, "SELECT COUNT(*) FROM users", [])?;
    ok(
        json!({ "status": "ok", "setup_required": users == 0, "setup_token_required": users == 0,
               "mode": if app.desktop { "desktop" } else { "server" },
               "licence": licence,
               "build_notice": "Development build. Not validated. No output of this software may be used for manufacturing." }),
    )
}

const THROTTLE_WINDOW: i64 = 900;
const PER_ACCOUNT_FAILURES: i64 = 5;
const PER_CLIENT_WINDOW: i64 = 600;
const PER_CLIENT_FAILURES: i64 = 100;
const SETUP_FAILURES: i64 = 10;
/// RA-006: once an account has this many failures in the window, sign-ins from addresses that never signed in to it
/// are admitted one at a time, `ACCOUNT_GATE_GAP_SECS` apart. Known addresses are not held back.
const ACCOUNT_GATE_FAILURES: i64 = 20;
const ACCOUNT_GATE_GAP_SECS: i64 = 10;

fn failures_of_pair(db: &Connection, key: &str, client: &str, since: i64) -> rusqlite::Result<i64> {
    q_count(
        db,
        "SELECT COUNT(*) FROM login_failures WHERE username = ? AND client = ? AND at > ?",
        params![key, client, since],
    )
}

/// Has this address signed in to this account before (successfully)?
fn known_client(db: &Connection, username: &str, client: &str) -> rusqlite::Result<bool> {
    Ok(q_count(
        db,
        "SELECT COUNT(*) FROM login_clients WHERE username = ? AND client = ?",
        params![username, client],
    )? > 0)
}

/// The sign-in throttle (RA-006). Refused attempts are recorded too (`admitted = 0`), so refusing does not make
/// guessing free. Three rules:
/// * the pair (account, address): 5 failures in 15 minutes lock that pair only, never the account for others.
/// * the address: 100 failures in 10 minutes (spraying from one place).
/// * the account gate: once 20 failures are in the window, an address that has not signed in to the account
///   before gets one admitted attempt per 10 seconds on its own slot for that account. A second attempt from the
///   same address inside 10 seconds is refused. Known addresses are not held back.
///   Consequence (REVIEW_A §6, A11): a new address gets its first attempt, so the total number of guesses on one
///   account is bounded by the number of addresses the attacker controls (about 5 per 15 minutes each), not by a
///   constant. A slot shared by all unknown addresses was rejected: one rotating attacker would starve the owner.
///
/// Unattributed clients (a loopback proxy without NF_TRUSTED_PROXY) have no address of their own, so the pair and
/// address rules are skipped for them and only the account gate applies (see REVIEW_A §6).
///
/// A global limit across accounts was rejected: one attacker could then deny sign-in to every account.
fn login_throttle(
    db: &Connection,
    username: &str,
    client: &str,
    t: i64,
) -> rusqlite::Result<Option<ApiError>> {
    // An unattributed client (see UNATTRIBUTED) has no address of its own: only the account rules below apply.
    let attributed = client != UNATTRIBUTED;
    if attributed
        && failures_of_pair(db, username, client, t - THROTTLE_WINDOW)? >= PER_ACCOUNT_FAILURES
    {
        return Ok(Some(fail_json(
            StatusCode::TOO_MANY_REQUESTS,
            json!({
                "error": "too many failed sign-in attempts for this account from this address; try again in 15 minutes",
                "error_ja": "このアドレスからこのアカウントへのサインイン失敗が多すぎます。15分後に再試行してください。",
                "throttled": true
            }),
        )));
    }
    if attributed && failures_from(db, client, t - PER_CLIENT_WINDOW)? >= PER_CLIENT_FAILURES {
        return Ok(Some(fail(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed sign-in attempts from this address; try again in 10 minutes",
        )));
    }
    if failures_of(db, username, t - THROTTLE_WINDOW)? >= ACCOUNT_GATE_FAILURES
        && !(attributed && known_client(db, username, client)?)
    {
        // A slot per (account, address): one address that rotates its attempts cannot take the slot of another.
        // Only admitted attempts open a slot; refused ones are counted but do not move it.
        let last: i64 = db.query_row(
            "SELECT COALESCE(MAX(at), 0) FROM login_failures WHERE username = ? AND client = ? AND admitted = 1",
            params![username, client],
            |r| r.get(0),
        )?;
        if t - last < ACCOUNT_GATE_GAP_SECS {
            return Ok(Some(fail_json(
                StatusCode::TOO_MANY_REQUESTS,
                json!({
                    "error": "this account is under attack: sign-ins from an address that has not signed in to it before are slowed down; try again in a few seconds",
                    "error_ja": "このアカウントは攻撃を受けています。このアカウントに以前サインインしたことのないアドレスからのサインインは速度を制限しています。数秒後に再試行してください。",
                    "throttled": true
                }),
            )));
        }
    }
    Ok(None)
}

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
    // RA-009: failed setup tokens are counted per address, so one client cannot block the installer
    let key = format!("setup:{client}");
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
        if failures_of(&tx, &key, t - PER_CLIENT_WINDOW)? >= SETUP_FAILURES {
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
        "DELETE FROM login_failures WHERE username LIKE 'setup:%'",
        [],
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
    // RA-002: the throttle is checked and this attempt is reserved (a failure row) in one transaction, BEFORE any
    // hashing. Concurrent guesses then see each other's reservations; a failed attempt is never re-counted here.
    let row: Option<(i64, String, String, String)> = {
        let mut db = app.db();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM login_failures WHERE at < ?",
            params![t - THROTTLE_WINDOW],
        )?;
        if let Some(refusal) = login_throttle(&tx, &username, &client, t)? {
            // a refused attempt is counted too: refusing must not make guessing free
            tx.execute(
                "INSERT INTO login_failures(username, at, client, admitted) VALUES (?,?,?,0)",
                params![username, t, client],
            )?;
            app.audit_anon(&tx, &username, "login_throttled", &format!("from {client}"))?;
            tx.commit()?;
            return Err(refusal);
        }
        tx.execute(
            "INSERT INTO login_failures(username, at, client, admitted) VALUES (?,?,?,1)",
            params![username, t, client],
        )?;
        let row = if plausible {
            tx.query_row(
                "SELECT id, role, salt, hash FROM users WHERE username = ? AND active = 1",
                params![raw_name],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
        } else {
            None
        };
        tx.commit()?;
        row
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
        // the attempt was counted when it was reserved above
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
    tx.execute(
        "INSERT INTO login_clients(username, client, at) VALUES (?,?,?) ON CONFLICT(username, client) DO UPDATE SET at = excluded.at",
        params![username, client, t],
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

/// `GET /api/session`: the client's start-up probe (BH-008). It answers 200 for a visitor who is not signed in
/// too, with `signed_in: false`, so the sign-in screen makes no failed request and the browser logs no error.
/// Any other failure is passed on. `/api/me` keeps its 401 for a visitor who is not signed in.
async fn session(State(app): Ctx, headers: HeaderMap) -> Api {
    match auth(&app, &headers) {
        Ok(u) => {
            ok(json!({ "signed_in": true, "id": u.id, "username": u.username, "role": u.role }))
        }
        Err(ApiError::Resp(r)) if r.status() == StatusCode::UNAUTHORIZED => {
            ok(json!({ "signed_in": false }))
        }
        Err(e) => Err(e),
    }
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
    // seat allowance before the database lock (lock order, licence.rs)
    let seats = licence::seats_allowed(&app);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let used = q_count(&tx, "SELECT COUNT(*) FROM users WHERE active = 1", [])?;
    if let Err(e) = licence::seat_gate(&app, &tx, &u.username, used, seats, "add user") {
        // the refusal was audited inside the transaction: commit so the entry survives
        if matches!(e, ApiError::Resp(_)) {
            tx.commit()?;
        }
        return Err(e);
    }
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
async fn change_password(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Api {
    let (current, new) = (s(&body, "current"), s(&body, "new"));
    let t = now() as i64;
    let (u, salt, hash, key) = {
        let mut db = app.db();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let u = auth_db(&tx, &app, &headers)?;
        password_policy(&u.username, &new)
            .map_err(|m| fail(StatusCode::BAD_REQUEST, &format!("new {m}")))?;
        let key = format!("pwchange:{}", u.id);
        if failures_of(&tx, &key, t - THROTTLE_WINDOW)? >= PER_ACCOUNT_FAILURES {
            return Err(fail(
                StatusCode::TOO_MANY_REQUESTS,
                "too many wrong current passwords; try again in 15 minutes",
            ));
        }
        // RA-002 / RC-04: reserved here, with the client address, before the current password is hashed
        tx.execute(
            "INSERT INTO login_failures(username, at, client) VALUES (?,?,?)",
            params![key, t, client],
        )?;
        let (salt, hash): (String, String) = tx.query_row(
            "SELECT salt, hash FROM users WHERE id = ?",
            params![u.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        tx.commit()?;
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
        // the attempt was counted when it was reserved above
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
    // seat allowance before the database lock (lock order, licence.rs)
    let seats = licence::seats_allowed(&app);
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
    if active {
        // reactivation counts against the seats; deactivation never does
        let used = q_count(
            &tx,
            "SELECT COUNT(*) FROM users WHERE active = 1 AND id != ?",
            params![id],
        )?;
        if let Err(e) = licence::seat_gate(&app, &tx, &u.username, used, seats, "reactivate user") {
            if matches!(e, ApiError::Resp(_)) {
                tx.commit()?;
            }
            return Err(e);
        }
    }
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
            elog!("nf-server: audit log cannot be read: {e}");
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
    if name.is_empty() {
        return Err(fail(StatusCode::BAD_REQUEST, "project name is required"));
    }
    // Counted in characters, as the name field's maxlength of 200 and the message say.
    if name.chars().count() > 200 {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "project name is too long (max 200 characters)",
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
        "SELECT id, filename, detected_type, size, sha256, extension_mismatch, created_by, created_at, conversion_id FROM documents WHERE project_id = ? ORDER BY id",
        params![id],
        |r| {
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "filename": r.get::<_, String>(1)?, "detected_type": r.get::<_, String>(2)?, "size": r.get::<_, i64>(3)?, "sha256": r.get::<_, String>(4)?, "extension_mismatch": r.get::<_, bool>(5)?, "created_by": r.get::<_, String>(6)?, "created_at": r.get::<_, i64>(7)?, "conversion_id": r.get::<_, Option<i64>>(8)? }),
            )
        },
    )?;
    // PDF to DXF conversions of this project (task 3.1), newest first.
    let conversions = q_all(
        &db,
        &format!(
            "SELECT {} FROM pdf_conversions c WHERE c.project_id = ? ORDER BY c.id DESC LIMIT 200",
            pdfconv::CONV_COLS
        ),
        params![id],
        pdfconv::conv_row,
    )?;
    let snapshot: Option<String> = db.query_row("SELECT id FROM records WHERE project_id = ? AND kind = 'snapshot' ORDER BY rowid DESC LIMIT 1", params![id], |r| r.get(0)).optional()?;
    p["documents"] = json!(docs);
    p["conversions"] = json!(conversions);
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
    /// The check of the stored result against its records (RC-01). Present on every full read
    /// (`load_core`); `None` on a list row served from the stored summary, which is used only for jobs
    /// whose full read under the current database version passed this check (see `calc_rows`).
    records: Option<RecordsCheck>,
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
        records: None,
    })
}

/// Full read of a stored job: the summary derived from the stored result, plus the check of its
/// pattern list against the content-addressed records (RC-01).
fn load_core(db: &Connection, id: i64) -> rusqlite::Result<Option<Core>> {
    let Some(mut c) = db
        .query_row(
            &format!("{} WHERE c.id = ?", core_sql(true)),
            params![id],
            core_row,
        )
        .optional()?
    else {
        return Ok(None);
    };
    c.records = Some(records_check(db, &c)?);
    Ok(Some(c))
}

/// What the records table says about a job, independent of the copy in `result_json` (RC-01).
#[derive(Clone, Default)]
struct RecordsCheck {
    /// Differences between the stored result and its records; each one is an integrity failure (G22).
    findings: Vec<String>,
    /// part -> SHA-256 of the DXF named by the job's own, verified `dxf_output` record. Empty unless
    /// the check found nothing.
    dxf: BTreeMap<String, String>,
    /// The request of the job's own `development_request` record, when exactly one record of that kind
    /// with a valid identity was reached (also when other records failed).
    request: Option<Value>,
    /// The verified records of the release closure, in the order of the stored result. Empty unless the
    /// check found nothing.
    records: Vec<Value>,
    /// The gate-evaluation record the walk started from, when it exists and passes its identity check.
    root: Option<String>,
}

/// Fields of a stored result that no record carries; they are served, but not as checked.
const NOT_CHECKED_AGAINST_RECORDS: [&str; 2] = ["engine_advisories", "refused_field"];

/// Walk the release closure of a job in the records table, starting at its gate-evaluation record,
/// verify the identity of every record and the closure hash, and compare the stored result with the
/// records of that closure (`compare_with_records`). Nothing is repaired; every difference is a
/// finding. A database error is returned as an error.
fn records_check(db: &Connection, c: &Core) -> rusqlite::Result<RecordsCheck> {
    let mut out = RecordsCheck::default();
    if !c.json_ok {
        // evaluate reports an unreadable result; there is nothing to compare
        return Ok(out);
    }
    let Some(root) = c.root_id.clone() else {
        out.findings
            .push("the stored result names no records to check it against".into());
        return Ok(out);
    };
    // the closure, read from the records table (not from the copy inside the stored result)
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack = vec![root.clone()];
    let (mut missing, mut failing) = (0usize, 0usize);
    let mut verified: Vec<Record> = Vec::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let json: Option<String> = db
            .query_row("SELECT json FROM records WHERE id = ?", params![id], |r| {
                text_col(r, 0)
            })
            .optional()?
            .flatten();
        let Some(rec) = json.and_then(|j| serde_json::from_str::<Record>(&j).ok()) else {
            missing += 1;
            continue;
        };
        // the edges of a failing record are followed only to count what is reachable; its content is not used
        for e in &rec.relations {
            stack.push(e.to.clone());
        }
        if let Some(a) = &rec.activity {
            stack.push(a.clone());
        }
        if rec.id == id && rec.verify_id() {
            if id == root && rec.kind == "gate_evaluation" {
                out.root = Some(root.clone());
            }
            verified.push(rec);
        } else {
            failing += 1;
        }
    }
    if missing > 0 {
        out.findings.push(format!(
            "{missing} record(s) of the release closure are missing from the records"
        ));
    }
    if failing > 0 {
        out.findings.push(format!(
            "{failing} record(s) of the release closure fail their identity check"
        ));
    }
    if missing == 0 && failing == 0 && nf_core::closure_hash(&seen) != c.closure_col {
        out.findings.push(
            "the records reached from the gate-evaluation record do not have the closure hash recorded for the job".into(),
        );
    }
    let requests: Vec<&Record> = verified
        .iter()
        .filter(|r| r.kind == "development_request")
        .collect();
    if let [one] = requests.as_slice() {
        out.request = Some(one.payload["request"].clone());
    }
    let stored = stored_result(db, c.id)?.unwrap_or(Value::Null);
    let (findings, dxf, records) = compare_with_records(&stored, &verified);
    out.findings.extend(findings);
    if out.findings.is_empty() {
        out.dxf = dxf;
        out.records = records;
    }
    Ok(out)
}

/// Compare a stored result with the verified records of its release closure: request, pattern list and DXF
/// references, checks, record list, closure size, work units, engine and refusal. Returns the findings, the
/// DXF hash of every part and the records in the order of the stored result.
#[allow(clippy::type_complexity)]
fn compare_with_records(
    res: &Value,
    recs: &[Record],
) -> (Vec<String>, BTreeMap<String, String>, Vec<Value>) {
    let mut f: Vec<String> = Vec::new();
    let of = |kind: &str| -> Vec<&Record> { recs.iter().filter(|r| r.kind == kind).collect() };
    // the request
    match of("development_request").as_slice() {
        [one] if one.payload["request"] == res["request"] => {}
        [_] => f.push("the stored request differs from the request record of the job".into()),
        [] => f.push("the release closure holds no request record".into()),
        _ => f.push("the release closure holds more than one request record".into()),
    }
    // patterns and DXF records, part by part
    let mut pattern_recs: BTreeMap<String, Value> = BTreeMap::new();
    let mut dxf_recs: BTreeMap<String, String> = BTreeMap::new();
    for r in recs
        .iter()
        .filter(|r| r.kind == "pattern" || r.kind == "dxf_output")
    {
        let Some(part) = r.payload["part"].as_str().map(String::from) else {
            f.push(format!("a {} record names no part", r.kind));
            continue;
        };
        let dup = if r.kind == "pattern" {
            pattern_recs
                .insert(part.clone(), r.payload["pattern"].clone())
                .is_some()
        } else {
            match r.payload["sha256"].as_str() {
                Some(sha) if storage::is_sha256_hex(sha) => {
                    dxf_recs.insert(part.clone(), sha.to_string()).is_some()
                }
                _ => {
                    f.push(format!(
                        "the DXF record of part {part} has no valid file reference"
                    ));
                    false
                }
            }
        };
        if dup {
            f.push(format!(
                "the release closure holds more than one {} record for part {part}",
                r.kind
            ));
        }
    }
    let stored_patterns: &[Value] = match &res["patterns"] {
        Value::Array(a) => a,
        // evaluate reports a missing pattern list; one of another type is a finding here
        Value::Null => &[],
        _ => {
            f.push("the stored pattern list is not a list".into());
            &[]
        }
    };
    let mut stored_parts: BTreeSet<String> = BTreeSet::new();
    for p in stored_patterns {
        let Some(part) = p["part"].as_str() else {
            f.push("a stored pattern names no part".into());
            continue;
        };
        if !stored_parts.insert(part.to_string()) {
            f.push(format!(
                "the stored result lists part {part} more than once"
            ));
            continue;
        }
        match pattern_recs.get(part) {
            None => f.push(format!(
                "the stored pattern of part {part} has no pattern record in the release closure"
            )),
            Some(rec) if rec != p => f.push(format!(
                "the stored pattern of part {part} differs from its pattern record"
            )),
            Some(_) => {}
        }
        if p["dxf_sha256"].as_str() != dxf_recs.get(part).map(String::as_str) {
            f.push(format!(
                "the stored DXF reference of part {part} differs from the DXF record of the job"
            ));
        }
    }
    for (part, rec) in &pattern_recs {
        if !stored_parts.contains(part) {
            f.push(format!(
                "the pattern record of part {part} is missing from the stored result"
            ));
        }
        if rec["dxf_sha256"].as_str() != dxf_recs.get(part).map(String::as_str) {
            f.push(format!(
                "the pattern record of part {part} and the DXF record of the job name different files"
            ));
        }
    }
    // the checks: the same multiset as the evidence records carry
    let canon = |v: &Value| serde_json::to_string(v).unwrap_or_default();
    let mut rec_checks: Vec<String> = recs
        .iter()
        .filter(|r| r.kind == "verification_evidence" || r.kind == "interface_evidence")
        .flat_map(|r| r.payload["checks"].as_array().cloned().unwrap_or_default())
        .map(|c| canon(&c))
        .collect();
    let mut stored_checks: Vec<String> = match &res["checks"] {
        Value::Array(a) => a.iter().map(canon).collect(),
        _ => vec!["<not a list>".into()],
    };
    rec_checks.sort();
    stored_checks.sort();
    if rec_checks != stored_checks {
        f.push("the stored checks differ from the evidence records of the job".into());
    }
    // the record list: every record of the closure, once, each as stored in the records table. Records of
    // the job outside its release closure (a refusal's unevaluated steps) are not covered by the closure
    // hash; they are not served.
    let by_id: BTreeMap<&str, &Record> = recs.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut ordered: Vec<Value> = Vec::with_capacity(recs.len());
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    let mut list_ok = res["records"].is_array();
    for r in res["records"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let id = r["id"].as_str().unwrap_or("");
        if let Some(rec) = by_id.get(id) {
            let v = serde_json::to_value(rec).unwrap_or(Value::Null);
            list_ok &= listed.insert(id) && v == *r;
            ordered.push(v);
        }
    }
    if !list_ok || listed.len() != recs.len() {
        f.push("the stored record list differs from the records of the release closure".into());
    }
    if res["closure_size"].as_u64() != Some(recs.len() as u64) {
        f.push("the stored closure size differs from the release closure".into());
    }
    let activities: Vec<&Record> = recs
        .iter()
        .filter(|r| r.family == Family::Activity)
        .collect();
    let work_units: Option<u64> = activities
        .iter()
        .map(|r| r.payload["work_units"].as_u64())
        .sum();
    if work_units.is_none() || res["work_units"].as_u64() != work_units {
        f.push("the stored work units differ from the activity records of the job".into());
    }
    // A result that names the running engine must have been computed by it in every step; one that names
    // another engine is withheld as superseded (G21) whatever its records say.
    if res["engine"]
        .as_str()
        .is_some_and(nf_core::engine_is_current)
        && activities
            .iter()
            .any(|r| r.payload["engine"] != res["engine"])
    {
        f.push("the stored engine identifier differs from the activity records of the job".into());
    }
    // a refusal: outcome and refusal are those of the engineering-outcome record, or absent
    match of("engineering_outcome").as_slice() {
        [] if res["outcome"].is_null() && res["refusal"].is_null() => {}
        [o] if res["outcome"] == o.payload["message"]
            && ["code", "fields", "message", "detail"]
                .iter()
                .all(|k| res["refusal"][k] == o.payload[k]) => {}
        _ => f.push(
            "the stored outcome differs from the engineering-outcome record of the job".into(),
        ),
    }
    (f, dxf_recs, ordered)
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
    // RA-012: SQLite reads the first copy of a repeated key and serde the last; such a result is not trusted.
    if c.json_ok && result_repeats_a_key(db, c.id)? {
        integrity.push(
            "the stored result repeats a key, so its readers could see different values".into(),
        );
    }
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
        // RC-01: the pattern list and the DXF references are trusted only as far as the records agree
        if let Some(rc) = &c.records {
            integrity.extend(rc.findings.iter().cloned());
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
/// left by S-7). This process's own writes always re-derive the summary. The
/// verdict of the check against the records (RC-01) is remembered with it, for
/// the same database version, so a failing job is not walked on every list.
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
            if let Some(findings) = seen.1.get(&c.id) {
                // RC-01: the verdict of the full check against the records, made under this database version,
                // travels with the summary, so a list says what the detail view says without walking again
                c.records = Some(RecordsCheck {
                    findings: findings.clone(),
                    ..Default::default()
                });
                continue;
            }
            if let Some(full) = load_core(db, c.id)? {
                *c = full;
            }
            // bring the stored summary in line with the result as it is now; only then may later lists use it
            match db.execute(&format!("UPDATE calculations AS c SET core_json = {CORE_EXPR}, result_bytes = length(CAST(c.result_json AS BLOB)) WHERE c.id = ?"), params![c.id]) {
                Ok(_) => {
                    // no check (a job deleted meanwhile) is remembered as a failure: fail closed
                    let findings = c.records.as_ref().map_or_else(
                        || vec!["the stored result could not be checked against its records".to_string()],
                        |r| r.findings.clone(),
                    );
                    seen.1.insert(c.id, findings);
                }
                Err(err) => elog!("nf-server: could not refresh the stored summary of job #{}: {err}", c.id),
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
            elog!(
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
            elog!("nf-server: could not audit a refused access: {e}");
        }
        return Err(not_found());
    }
    let e = evaluate(db, &c)?;
    Ok((c, e))
}

/// The stored result as JSON, or `None` if it is not a readable JSON object.
/// Does the stored result of a job repeat a key inside an object? (RA-012)
fn result_repeats_a_key(db: &Connection, id: i64) -> rusqlite::Result<bool> {
    let raw: Option<String> = db
        .query_row(
            "SELECT result_json FROM calculations WHERE id = ?",
            params![id],
            |r| text_col(r, 0),
        )
        .optional()?
        .flatten();
    Ok(raw.is_some_and(|t| !json_keys_are_unique(&t)))
}

/// A JSON text in which no object repeats a key. Parsing refuses a repeated key; any other parse error also counts
/// as "not clean", which is fine: the caller only asks about text that SQLite already accepted as JSON.
fn json_keys_are_unique(text: &str) -> bool {
    struct Unique;
    impl<'de> serde::Deserialize<'de> for Unique {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visit;
            impl<'de> serde::de::Visitor<'de> for Visit {
                type Value = ();
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("JSON")
                }
                fn visit_bool<E>(self, _: bool) -> Result<(), E> {
                    Ok(())
                }
                fn visit_i64<E>(self, _: i64) -> Result<(), E> {
                    Ok(())
                }
                fn visit_u64<E>(self, _: u64) -> Result<(), E> {
                    Ok(())
                }
                fn visit_f64<E>(self, _: f64) -> Result<(), E> {
                    Ok(())
                }
                fn visit_str<E>(self, _: &str) -> Result<(), E> {
                    Ok(())
                }
                fn visit_unit<E>(self) -> Result<(), E> {
                    Ok(())
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut seq: A,
                ) -> Result<(), A::Error> {
                    while seq.next_element::<Unique>()?.is_some() {}
                    Ok(())
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<(), A::Error> {
                    let mut seen = std::collections::HashSet::new();
                    while let Some(key) = map.next_key::<String>()? {
                        if !seen.insert(key.clone()) {
                            return Err(serde::de::Error::custom(format!("duplicate key {key}")));
                        }
                        map.next_value::<Unique>()?;
                    }
                    Ok(())
                }
            }
            d.deserialize_any(Visit).map(|_| Unique)
        }
    }
    serde_json::from_str::<Unique>(text).is_ok()
}

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
    if reason.chars().count() < 5 {
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
async fn calculate(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, raw: Bytes) -> Api {
    let body = request_json(&headers, &raw)?;
    let u = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        // access is decided before any work is done (BE-028)
        project_guard(&app, &db, &u, id, "calculate")?;
        u
    };
    let req =
        Request::parse_refusal(&body).map_err(|r| fail_refused(StatusCode::BAD_REQUEST, &r))?;
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
        let r = out.refusal.clone().unwrap_or_else(|| {
            nf_core::Refusal::new(
                nf_core::refusal::NO_PATTERN_PRODUCED,
                &[],
                "no pattern was produced",
            )
        });
        return Err(fail_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            with_detail(
                json!({ "error": format!("request refused, nothing was computed: {outcome}"), "id": cid, "state": REFUSED, "outcome": outcome, "field": out.refused_field, "advisories": 0, "code": r.code, "fields": r.fields, "message": r.message }),
                &r,
            ),
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
        "SELECT id, origin_calc, target_calc, kind, label, direction, via, required_check, state, note, resolved_by, resolved_at, created_at, EXISTS (SELECT 1 FROM calculations n WHERE n.supersedes = obligations.target_calc) FROM obligations {where_sql} ORDER BY id DESC LIMIT 500"
    );
    let map = |r: &rusqlite::Row<'_>| {
        Ok(
            json!({ "id": r.get::<_, i64>(0)?, "origin_calc": r.get::<_, i64>(1)?, "target_calc": r.get::<_, i64>(2)?, "kind": r.get::<_, String>(3)?, "label": r.get::<_, String>(4)?,
                   "direction": r.get::<_, String>(5)?, "via": r.get::<_, String>(6)?, "required_check": r.get::<_, String>(7)?, "state": r.get::<_, String>(8)?, "note": r.get::<_, String>(9)?,
                   "resolved_by": r.get::<_, Option<String>>(10)?, "resolved_at": r.get::<_, Option<i64>>(11)?, "created_at": r.get::<_, i64>(12)?,
                   "target_superseded": r.get::<_, bool>(13)? }),
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
        // nothing of a result that failed its integrity check is offered as geometry, evidence or records;
        // the request shown is the one of the job's request record where that record is intact (RC-01)
        result["patterns"] = json!([]);
        result["checks"] = json!([]);
        result["records"] = json!([]);
        result["closure_size"] = json!(0);
        result["work_units"] = json!(0);
        result["request"] = c
            .records
            .as_ref()
            .and_then(|r| r.request.clone())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({ "family": c.family }));
        result["outcome"] = json!(format!(
            "The stored result failed its integrity check ({}). Nothing of this job may be used.",
            e.integrity.join("; ")
        ));
    }
    if e.integrity.is_empty()
        && let Some(r) = c.records.as_ref()
    {
        // RC-01: the records served are the verified ones of the release closure
        result["records"] = json!(r.records);
    }
    // the parts whose DXF is withheld, from the same verified records get_dxf reads (RC-01)
    result["dxf_withheld"] = json!(
        c.records
            .as_ref()
            .map(|r| {
                dxf_withheld(r)
                    .into_iter()
                    .map(|(part, hits)| {
                        (
                            part,
                            hits.into_iter()
                                .map(|(name, _)| name)
                                .collect::<Vec<String>>(),
                        )
                    })
                    .collect::<BTreeMap<String, Vec<String>>>()
            })
            .unwrap_or_default()
    );
    result["not_checked_against_records"] = json!(NOT_CHECKED_AGAINST_RECORDS);
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
        elog!("replay-check: cannot read the request");
        std::process::exit(2);
    }
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let Ok(req) = serde_json::from_value::<Request>(v["request"].clone()) else {
        elog!("replay-check: unreadable request");
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
        elog!(
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
///   stored result failing its integrity check). A stored result whose only
///   integrity findings are differences from its records (RC-01) is replayed,
///   and the report says it does not match.
async fn replay(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let (u, c, request, ids) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        let (c, e) = calc_guard(&app, &db, &u, id, "replay")?;
        // A stored result that disagrees only with its records (RC-01) is still replayed: the audit runs,
        // reports the disagreement and does not match. Any other integrity failure leaves nothing to replay.
        let records_findings = c.records.as_ref().map_or(0, |r| r.findings.len());
        if e.integrity.len() > records_findings {
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
            return Err(fail_code(
                StatusCode::CONFLICT,
                nf_core::refusal::STORED_RESULT_UNREADABLE,
                &[],
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
        // RC-01: the request replayed is the one of the job's own request record, never the stored copy
        let Some(request) = c.records.as_ref().and_then(|r| r.request.clone()) else {
            return Err(fail(
                StatusCode::CONFLICT,
                "the request record of this job is missing or damaged; it is REJECTED and cannot be replayed",
            ));
        };
        (u, c, request, ids)
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
            elog!("nf-server: replay of job #{id} could not be run: {problem}");
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
    // RC-01 / RC-22: what is served from the stored result (request, patterns, DXF references, checks,
    // records) must be what the records closure says. Checked again here, inside the transaction that writes the report.
    let records_findings: Vec<String> = match load_core(&tx, id)? {
        Some(fresh) if fresh.closure_col == c.closure_col => {
            fresh.records.map(|r| r.findings).unwrap_or_else(|| {
                vec!["the stored result could not be checked against its records".into()]
            })
        }
        _ => vec!["the job changed while the replay ran".into()],
    };
    let matches = got == c.closure_col
        && got_ids == ids
        && tampered == 0
        && !ids.is_empty()
        && records_findings.is_empty();
    let report = json!({ "kind": "separate_process_replay", "matches": matches, "expected_closure_hash": c.closure_col, "replayed_closure_hash": got, "process_error": Value::Null,
                         "engine": nf_core::ENGINE_VERSION, "stored_records_checked": ids.len(), "stored_records_failing_identity_check": tampered,
                         "stored_result_matches_records": records_findings.is_empty(), "stored_result_findings": records_findings, "run_by": u.username, "at": now(),
                         "limitation": "Replay ran in a separate process that received only the request, on the same machine with the same build (the child reported the same engine identifier). A separately built binary or separate machine is not used. The stored request, pattern list, DXF references, checks and record list were compared with the records of the release closure. Discrepancies are reported, never repaired. This audit shows reproducibility, not correctness." });
    // the audit names the job's gate-evaluation record only where the records table holds it intact
    let audit_rec = Record::new(
        Family::Assessment,
        "replay_audit",
        report.clone(),
        c.records
            .as_ref()
            .and_then(|r| r.root.as_ref())
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
    // A lesson is recorded only for a job whose stored result agrees with its records: a lesson built from
    // a tampered job would raise advisories on honest jobs with the same request.
    if !matches
        && records_findings.is_empty()
        && let Ok(req) = serde_json::from_value::<Request>(request)
    {
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
/// The checks on a part's cut geometry that could not be confirmed, read from the VERIFIED release
/// records (RC-01) — never from the copy inside the stored result. A check is undecided for part P
/// when its verdict is INCONCLUSIVE and its scope is P or "interface" (a joint check bears on every
/// part of the job). Owner decision, 24 Nov 2026: no DXF is handed out for such a part. The map is
/// empty for every part whose checks are all decided; each entry lists (name, reason) per check.
fn dxf_withheld(records: &RecordsCheck) -> BTreeMap<String, Vec<(String, String)>> {
    let undecided: Vec<(String, String, String)> = records
        .records
        .iter()
        .filter(|r| r["kind"] == "verification_evidence" || r["kind"] == "interface_evidence")
        .flat_map(|r| {
            r["payload"]["checks"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|c| c["verdict"] == "INCONCLUSIVE")
        .map(|c| {
            (
                c["name"].as_str().unwrap_or("").to_string(),
                c["scope"].as_str().unwrap_or("").to_string(),
                c["reason"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let mut out: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for part in records.dxf.keys() {
        let hits: Vec<(String, String)> = undecided
            .iter()
            .filter(|(_, scope, _)| scope == part || scope == "interface")
            .map(|(name, _, reason)| (name.clone(), reason.clone()))
            .collect();
        if !hits.is_empty() {
            out.insert(part.clone(), hits);
        }
    }
    out
}

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
    // RC-01: the file is the one named by the job's own DXF record, read from the verified release
    // closure; the copy of the reference inside the stored result is never used to choose a file.
    let Some(records) = c.records.as_ref().filter(|r| r.findings.is_empty()) else {
        return Err(fail(
            StatusCode::CONFLICT,
            "this result is REJECTED (the stored result failed its integrity check); its files are withheld",
        ));
    };
    let Some(sha) = records.dxf.get(part.as_str()).cloned() else {
        return Err(fail(StatusCode::NOT_FOUND, "no DXF for this part"));
    };
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Owner decision (24 Nov 2026): the file of a part is withheld while any check on its cut
    // geometry is INCONCLUSIVE. The checks are those of the verified release records (RC-01).
    let withheld = dxf_withheld(records);
    if let Some(hits) = withheld.get(part.as_str()) {
        let names = hits
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<&str>>()
            .join(", ");
        app.audit(
            &tx,
            &u.username,
            "dxf_withheld",
            &format!("calculation #{id} part {part}: {names}"),
        )?;
        tx.commit()?;
        return Err(fail_json(
            StatusCode::CONFLICT,
            json!({
                "error": format!(
                    "no DXF for part {part}: {} check(s) on its cut geometry could not be confirmed ({names}); the file is withheld until every check on the cut geometry is decided",
                    hits.len()
                ),
                "code": "dxf_withheld_inconclusive",
                "checks": hits
                    .iter()
                    .map(|(name, reason)| json!({ "name": name, "reason": reason }))
                    .collect::<Vec<Value>>(),
            }),
        ));
    }
    let bytes = match storage::load_blob(&app.data_dir, &app.key, &sha) {
        Ok(b) => b,
        Err(problem) => {
            // The job is deterministic: the file can be restored from the request, and only if the
            // recomputed text has exactly the recorded hash (BE-006).
            elog!(
                "nf-server: DXF {sha} of job #{id} part {part}: {problem}; trying to restore it by recomputation"
            );
            let again = records
                .request
                .clone()
                .and_then(|r| serde_json::from_value::<Request>(r).ok())
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
/// `POST /api/calculations/{id}/recompute` (RA-007): a result of a superseded engine is run again from its stored
/// request, with the engine that runs now. The new result supersedes the old one, which stays as a record. A result
/// that fails its integrity check is not recomputed from its own stored request (fail closed).
async fn recompute(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let (u, project, req) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        let (c, e) = calc_guard(&app, &db, &u, id, "recompute")?;
        if c.superseded_by.is_some() {
            return Err(fail(
                StatusCode::CONFLICT,
                "this result is already superseded; recompute the current one",
            ));
        }
        if e.engine_current {
            return Err(fail_json(
                StatusCode::CONFLICT,
                json!({
                    "error": "this result was computed by the engine that is running now; nothing to recompute",
                    "error_ja": "この結果は現在動作中のエンジンで計算されたものです。再計算の必要はありません。"
                }),
            ));
        }
        if !e.integrity.is_empty() {
            return Err(fail(
                StatusCode::CONFLICT,
                "this result failed its integrity check; its stored request cannot be trusted for a recompute",
            ));
        }
        let Some(verified) = c.records.clone().filter(|r| r.findings.is_empty()) else {
            return Err(fail(
                StatusCode::CONFLICT,
                "this result failed its integrity check; its stored request cannot be trusted for a recompute",
            ));
        };
        let Ok(req) =
            serde_json::from_value::<Request>(verified.request.clone().unwrap_or_default())
        else {
            return Err(fail_code(
                StatusCode::CONFLICT,
                nf_core::refusal::STORED_REQUEST_UNREADABLE,
                &[],
                "the stored request is unreadable",
            ));
        };
        (u, c.project, req)
    };
    let out = job::run(&req, &u.username);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    project_guard(&app, &tx, &u, project, "recompute")?;
    let new_id = persist_calc(&app, &tx, project, &out, &u.username, Some(id))?;
    app.audit(
        &tx,
        &u.username,
        "recomputed",
        &format!(
            "calculation #{id} -> #{new_id} with engine {}",
            nf_core::ENGINE_VERSION
        ),
    )?;
    tx.commit()?;
    ok(json!({ "id": new_id, "supersedes": id }))
}

async fn correct(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, raw: Bytes) -> Api {
    let body = request_json(&headers, &raw)?;
    let pname = clip(&s(&body, "parameter"), 64);
    // the same rule as text_field (trimmed, counted in characters), with its own code
    let reason = {
        let t = s(&body, "reason").trim().to_string();
        if t.chars().count() > MAX_TEXT {
            return Err(fail_code(
                StatusCode::UNPROCESSABLE_ENTITY,
                nf_core::refusal::REASON_TOO_LONG,
                &["reason"],
                &format!("reason is too long (at most {MAX_TEXT} characters)"),
            ));
        }
        t
    };
    let (u, c, req, records) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        let Some(new_value) = body
            .get("new_value")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
        else {
            return Err(fail_code(
                StatusCode::BAD_REQUEST,
                nf_core::refusal::VALUE_NOT_FINITE,
                &["new_value"],
                "new_value must be a finite number",
            ));
        };
        let _ = new_value;
        if reason.chars().count() < 5 {
            return Err(fail_code(
                StatusCode::BAD_REQUEST,
                nf_core::refusal::REASON_REQUIRED,
                &["reason"],
                "a reason is required for every correction",
            ));
        }
        let (c, e) = calc_guard(&app, &db, &u, id, "correct")?;
        if c.superseded_by.is_some() {
            return Err(fail_code(
                StatusCode::CONFLICT,
                nf_core::refusal::CORRECTION_ON_SUPERSEDED_JOB,
                &[],
                "this result is already superseded; correct the current one",
            ));
        }
        if e.refused() {
            return Err(fail_code(
                StatusCode::CONFLICT,
                nf_core::refusal::CORRECTION_ON_REFUSED_JOB,
                &[],
                "this request was refused and has no result to correct; submit a new calculation",
            ));
        }
        if !e.integrity.is_empty() || !e.engine_current {
            return Err(fail_code(
                StatusCode::CONFLICT,
                nf_core::refusal::CORRECTION_NEEDS_RECOMPUTE,
                &[],
                "this result cannot be corrected: it failed its integrity check or was computed by a superseded engine version — recompute it first",
            ));
        }
        // RC-01: request and records come from the verified release closure; a job with any finding
        // against its records was refused above (its integrity list is not empty)
        let Some(verified) = c.records.clone().filter(|r| r.findings.is_empty()) else {
            return Err(fail_code(
                StatusCode::CONFLICT,
                nf_core::refusal::CORRECTION_NEEDS_RECOMPUTE,
                &[],
                "this result cannot be corrected: it failed its integrity check or was computed by a superseded engine version — recompute it first",
            ));
        };
        let Ok(req) =
            serde_json::from_value::<Request>(verified.request.clone().unwrap_or_default())
        else {
            return Err(fail_code(
                StatusCode::CONFLICT,
                nf_core::refusal::STORED_REQUEST_UNREADABLE,
                &[],
                "the stored request is unreadable",
            ));
        };
        let records: std::collections::BTreeMap<String, Record> =
            serde_json::from_value::<Vec<Record>>(Value::Array(verified.records))
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
        return Err(fail_code(
            StatusCode::BAD_REQUEST,
            nf_core::refusal::CORRECTION_NOT_FOR_FAMILY,
            &["family"],
            "corrections with dependency propagation are implemented for assembly jobs only",
        ));
    }
    let Some(old_value) = req
        .numeric_params()
        .into_iter()
        .find(|(k, _)| *k == pname)
        .map(|(_, v)| v)
    else {
        return Err(fail_code(
            StatusCode::BAD_REQUEST,
            nf_core::refusal::CORRECTION_UNKNOWN_PARAMETER,
            &["parameter"],
            "unknown parameter",
        ));
    };
    if old_value == new_value {
        return Err(fail_code(
            StatusCode::BAD_REQUEST,
            nf_core::refusal::CORRECTION_VALUE_UNCHANGED,
            &["new_value"],
            "new value equals the current value",
        ));
    }
    let Some(new_req) = req.with_param(&pname, new_value) else {
        return Err(fail_code(
            StatusCode::BAD_REQUEST,
            nf_core::refusal::CORRECTION_PARAMETER_NOT_CORRECTABLE,
            &["parameter"],
            "parameter cannot be corrected",
        ));
    };
    let Some(old_param) = records
        .values()
        .find(|r| r.kind == "parameter" && r.payload["name"] == pname.as_str())
        .map(|r| r.id.clone())
    else {
        return Err(fail_code(
            StatusCode::CONFLICT,
            nf_core::refusal::CORRECTION_PARAMETER_NOT_RECORDED,
            &["parameter"],
            "this job has no record of that parameter, so a correction cannot be propagated",
        ));
    };
    // 1. compute the corrected job; nothing has been written yet
    let out = job::run(&new_req, &u.username);
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let refused = match out.state {
        nf_core::ReleaseState::ReviewRequired if !out.patterns.is_empty() => None,
        nf_core::ReleaseState::Rejected => Some(nf_core::Refusal::new(
            nf_core::refusal::CORRECTED_RESULT_FAILED_VERIFICATION,
            &[],
            "the corrected request produced a pattern that failed its own verification",
        )),
        _ => Some(out.refusal.clone().unwrap_or_else(|| {
            nf_core::Refusal::new(
                nf_core::refusal::NO_PATTERN_PRODUCED,
                &[],
                "the corrected request produced no pattern",
            )
        })),
    };
    if let Some(r) = refused {
        let why = r.message.clone();
        app.audit(&tx, &u.username, "correction_refused", &format!("calculation #{id} {pname}: {old_value} -> {new_value} ({reason}); refused: {why}; nothing changed"))?;
        tx.commit()?;
        return Err(fail_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            with_detail(
                json!({ "error": format!("correction refused, nothing was changed: {pname} = {new_value} cannot be computed ({why}). Job #{id} is still the current result."), "outcome": why, "field": out.refused_field.clone().unwrap_or_else(|| pname.clone()), "state": REFUSED, "code": r.code, "fields": r.fields, "message": why }),
                &r,
            ),
        ));
    }
    // 2. the old job must still be the current revision (another correction may have won the race)
    if load_core(&tx, id)?.is_none_or(|now| now.superseded_by.is_some()) {
        return Err(fail_code(
            StatusCode::CONFLICT,
            nf_core::refusal::CORRECTION_ON_SUPERSEDED_JOB,
            &[],
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
    if reason.chars().count() < 5 {
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
    let (c, e) = calc_guard(&app, &db, &u, id, "read dependencies")?;
    // RC-01: only the verified records of the release closure are shown; a job that failed its
    // integrity check has no dependency graph to offer
    let Some(verified) = c
        .records
        .filter(|r| e.integrity.is_empty() && r.findings.is_empty())
    else {
        return Err(fail(
            StatusCode::CONFLICT,
            "the stored result failed its integrity check; it is REJECTED and its records are withheld",
        ));
    };
    let records: Vec<Record> =
        serde_json::from_value(Value::Array(verified.records)).unwrap_or_default();
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
        "open_obligations": n(format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND target_calc IN (SELECT id FROM calculations WHERE {sc} AND {current})"))?,
        "open_obligations_on_superseded": n(format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND target_calc IN (SELECT id FROM calculations WHERE {sc} AND NOT {current})"))?,
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
    // BH-002: an obligation on a superseded result stays open (history is append-only) but is counted apart from
    // the open work of the current results.
    let current_target = format!(
        "target_calc IN (SELECT id FROM calculations WHERE {sc} AND NOT EXISTS (SELECT 1 FROM calculations n WHERE n.supersedes = calculations.id))"
    );
    let superseded_target = format!(
        "target_calc IN (SELECT id FROM calculations WHERE {sc} AND EXISTS (SELECT 1 FROM calculations n WHERE n.supersedes = calculations.id))"
    );
    ok(json!({
        "records_by_kind": kinds,
        "activities": work.into_iter().map(|(k, (n, wu))| json!({ "type": k, "distinct_activities": n, "declared_work_units": wu })).collect::<Vec<_>>(),
        "recent_snapshots": snaps,
        "implemented": ["content-addressed append-only records", "project snapshots with predecessor links", "typed dependency edges (derivation, interface, common cause)", "dependency propagation with scrutiny obligations (provisional rule table)", "interface constraint verification between parts", "lessons from corrections and failures, matched against every later job in the project", "corrections as attestations with supersession", "release closure hash", "gate evaluation with obligation and replay state", "replay audit in a separate process", "membrane admission checks"],
        "obligations": {
            "open": q_count(&db, &format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND {current_target}"), [])?,
            "open_on_superseded": q_count(&db, &format!("SELECT COUNT(*) FROM obligations WHERE state = 'open' AND {superseded_target}"), [])?,
            "total": q_count(&db, &format!("SELECT COUNT(*) FROM obligations WHERE {in_scope}"), [])?
        },
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

/// `host[:port]` split into the lower-cased host and the port (the scheme's default when absent).
/// Only a plain authority is accepted: a DNS name or IPv4 address, or an IPv6 address in brackets,
/// optionally followed by `:` and 1 to 5 digits (1 to 65535). Anything else is `None`.
fn parse_authority(a: &str, default_port: u16) -> Option<(String, u16)> {
    let (host, rest) = if let Some(inner) = a.strip_prefix('[') {
        let end = inner.find(']')?;
        let h = &inner[..end];
        if h.is_empty()
            || !h
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return None;
        }
        (&a[..end + 2], &inner[end + 1..])
    } else {
        let (h, rest) = a.find(':').map_or((a, ""), |i| (&a[..i], &a[i..]));
        if h.is_empty()
            || !h
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        {
            return None;
        }
        (h, rest)
    };
    let port = match rest {
        "" => default_port,
        r => {
            let digits = r.strip_prefix(':')?;
            if digits.is_empty() || digits.len() > 5 || !digits.chars().all(|c| c.is_ascii_digit())
            {
                return None;
            }
            match digits.parse::<u16>() {
                Ok(p) if p > 0 => p,
                _ => return None,
            }
        }
    };
    Some((host.to_ascii_lowercase(), port))
}

/// RC-08 / V-47: a state-changing request may carry an `Origin` only if it is exactly
/// `<scheme>://<host[:port]>` with the scheme of this listener (`https` when `NF_COOKIE_SECURE` is set,
/// else `http`) and the same host and port as the `Host` header (host compared without regard to case,
/// an absent port read as the scheme's default). No path, query, fragment, user information or second
/// scheme; `null`, `file://`, `data:` and every other scheme are refused.
fn origin_allowed(origin: &str, host: &str, https_listener: bool) -> bool {
    let (scheme, default_port) = if https_listener {
        ("https", 443)
    } else {
        ("http", 80)
    };
    let Some((s, authority)) = origin.split_once("://") else {
        return false;
    };
    if !s.eq_ignore_ascii_case(scheme) {
        return false;
    }
    match (
        parse_authority(authority, default_port),
        parse_authority(host, default_port),
    ) {
        (Some(o), Some(h)) => o == h,
        _ => false,
    }
}

/// Host header without its port, lower case. An unbracketed IPv6 address has no port: it is returned whole.
fn host_name(host: &str) -> String {
    let h = host.trim();
    let name = if h.starts_with('[') {
        h.split_inclusive(']').next().unwrap_or(h)
    } else if h.matches(':').count() > 1 {
        h
    } else {
        h.rsplit_once(':').map(|(n, _)| n).unwrap_or(h)
    };
    name.to_ascii_lowercase()
}

/// One entry of NF_ALLOWED_HOSTS. An IPv6 address must be bracketed, as it is in a Host header (RA-016).
fn allowed_host_entry(h: &str) -> Result<String, String> {
    let h = h.trim();
    if !h
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
    {
        return Err(format!("NF_ALLOWED_HOSTS: '{h}' is not a host name"));
    }
    if h.matches(':').count() > 1 && !h.starts_with('[') {
        return Err(format!(
            "NF_ALLOWED_HOSTS: '{h}' is an IPv6 address without brackets; write it as [{h}]"
        ));
    }
    Ok(host_name(h))
}

/// The key a client address is throttled under. An IPv6 client is its /64 (RA-016): one subscriber cannot take
/// an unlimited number of keys from the 2^64 addresses of its prefix.
fn client_key(addr: IpAddr) -> String {
    let addr = match addr {
        IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(addr),
        IpAddr::V4(_) => addr,
    };
    match addr {
        IpAddr::V4(v) => v.to_string(),
        IpAddr::V6(v) => {
            let g = v.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", g[0], g[1], g[2], g[3])
        }
    }
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
            return client_key(*ip);
        }
    }
    client_key(peer)
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
    let peer = req.extensions().get::<PeerAddr>().map(|p| v4_of(p.0));
    let client_str = if unattributed_peer(&app, peer, req.headers()) {
        UNATTRIBUTED.to_string()
    } else {
        client_address(&app, peer, req.headers())
    };
    let client = ClientAddr(client_str.clone());
    let client_key = client_str;
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
            if let Some(origin) = h.get(header::ORIGIN) {
                // RC-08 / V-47: the Origin must be exactly <scheme>://<host[:port]> of this listener
                let host = h
                    .get(header::HOST)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if !origin
                    .to_str()
                    .is_ok_and(|o| origin_allowed(o, host, app.cookie_secure))
                {
                    return Some(fail(StatusCode::FORBIDDEN, "cross-origin request refused"));
                }
            }
        }
        None
    })();
    let mut resp = match refusal {
        Some(r) => r.into_response(),
        // RA-001: a client may hold only so many requests open at once; the rest are refused at once.
        None => match if client_key == UNATTRIBUTED {
            Some(None)
        } else {
            InFlight::take(&app, &client_key).map(Some)
        } {
            None => fail_json(
                StatusCode::TOO_MANY_REQUESTS,
                json!({
                    "error": "too many requests are open from this address; try again shortly",
                    "error_ja": "このアドレスから開いているリクエストが多すぎます。しばらくしてから再試行してください。",
                    "throttled": true
                }),
            )
            .into_response(),
            Some(_slot) => serve_one(&app, req, next).await,
        },
    };
    let h = resp.headers_mut();
    let set = |h: &mut HeaderMap, k: &'static str, v: &'static str| {
        h.insert(k, HeaderValue::from_static(v));
    };
    set(
        h,
        "content-security-policy",
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
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

/// Aborts a task when dropped: a request that is abandoned or finished stops its helper task.
struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// One open request of one client address (RA-001). Released when the request ends.
struct InFlight {
    app: Arc<App>,
    key: String,
}

impl InFlight {
    fn take(app: &Arc<App>, key: &str) -> Option<InFlight> {
        let mut open = app.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        let n = open.entry(key.to_string()).or_insert(0);
        if *n >= app.per_client_in_flight {
            return None;
        }
        *n += 1;
        Some(InFlight {
            app: app.clone(),
            key: key.to_string(),
        })
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut open = self.app.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = open.get_mut(&self.key) {
            *n -= 1;
            if *n == 0 {
                open.remove(&self.key);
            }
        }
    }
}

/// The request body as the handler sees it: chunks that a pump task has read, in order (RA-001).
struct PumpedBody(tokio::sync::mpsc::Receiver<Result<Bytes, axum::Error>>);

impl axum::body::HttpBody for PumpedBody {
    type Data = Bytes;
    type Error = axum::Error;
    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<hyper::body::Frame<Bytes>, axum::Error>>> {
        match self.0.poll_recv(cx) {
            std::task::Poll::Ready(Some(Ok(chunk))) => {
                std::task::Poll::Ready(Some(Ok(hyper::body::Frame::data(chunk))))
            }
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// The request body, read through a pump that gives up when no chunk arrives for `body_idle` (RA-001). The
/// handler reads the pump's channel; a stalled body ends the request with 408 instead of holding it for the
/// whole request timeout. Chunks are passed on one at a time, so a large upload is still streamed.
fn guard_body(
    app: &Arc<App>,
    req: axum::extract::Request,
) -> (
    axum::extract::Request,
    Option<AbortOnDrop>,
    Arc<std::sync::atomic::AtomicBool>,
) {
    use axum::body::HttpBody as _;
    use http_body_util::BodyExt as _;
    use std::sync::atomic::{AtomicBool, Ordering};
    let stalled = Arc::new(AtomicBool::new(false));
    if req.body().is_end_stream() {
        return (req, None, stalled);
    }
    let idle = app.body_idle;
    let (parts, mut body) = req.into_parts();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, axum::Error>>(2);
    let flag = stalled.clone();
    let pump = tokio::spawn(async move {
        loop {
            match tokio::time::timeout(idle, body.frame()).await {
                Err(_) => {
                    flag.store(true, Ordering::SeqCst);
                    let _ = tx
                        .send(Err(axum::Error::new(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "the request body stopped arriving",
                        ))))
                        .await;
                    return;
                }
                Ok(None) => return,
                Ok(Some(Err(e))) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
                Ok(Some(Ok(frame))) => {
                    if let Ok(data) = frame.into_data()
                        && tx.send(Ok(data)).await.is_err()
                    {
                        return;
                    }
                }
            }
        }
    });
    (
        axum::extract::Request::from_parts(parts, axum::body::Body::new(PumpedBody(rx))),
        Some(AbortOnDrop(pump.abort_handle())),
        stalled,
    )
}

/// Runs the handler for one admitted request. Time-limited as a whole (S-3). A panicking handler answers 500
/// and the server keeps serving (RA-011); a stalled body answers 408 (RA-001).
async fn serve_one(
    app: &Arc<App>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    use std::sync::atomic::Ordering;
    let (req, _pump, stalled) = guard_body(app, req);
    let handle = tokio::spawn(next.run(req));
    // when the time limit ends the wait, the handler is stopped too: its transaction rolls back
    let _stop = AbortOnDrop(handle.abort_handle());
    let resp = match tokio::time::timeout(app.request_timeout, handle).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => {
            elog!("nf-server: a request handler panicked; the request was answered with 500");
            fail_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "internal error; nothing was changed. The server log has the details." }),
            )
            .into_response()
        }
        Err(_) => fail(
            StatusCode::REQUEST_TIMEOUT,
            "the request was not completed in time and was abandoned",
        )
        .into_response(),
    };
    if stalled.load(Ordering::SeqCst) && resp.status().is_client_error() {
        return fail(
            StatusCode::REQUEST_TIMEOUT,
            "the request body stopped arriving and the request was abandoned",
        )
        .into_response();
    }
    resp
}

/// Test-only route that panics, to check that a panicking handler cannot take the server down (RA-011).
#[cfg(test)]
async fn test_panic() -> &'static str {
    panic!("the test handler panics on purpose")
}

#[cfg(test)]
fn test_routes() -> Router<Arc<App>> {
    Router::new().route("/api/__test_panic", get(test_panic))
}
#[cfg(not(test))]
fn test_routes() -> Router<Arc<App>> {
    Router::new()
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
            "/i18n.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../../../web/i18n.js"),
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
        .route("/api/session", get(session))
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
        .route("/api/documents/{id}/pdf-to-dxf", post(pdfconv::convert))
        .route("/api/pdf-conversions/{id}", get(pdfconv::get_conversion))
        .route("/api/pdf-conversions/{id}/{file}", get(pdfconv::get_file))
        .route("/api/calculations", get(list_calcs))
        .route("/api/calculations/{id}", get(get_calc))
        .route("/api/calculations/{id}/replay", post(replay))
        .route("/api/calculations/{id}/recompute", post(recompute))
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
        // the licence key file: read by any signed-in user, installed by an administrator
        .route(
            "/api/licence",
            get(licence::licence_status).post(licence::licence_upload),
        )
        .merge(test_routes())
        .layer(axum::extract::DefaultBodyLimit::max(256 * 1024))
        // the licence gate runs inside the security layer (host/CSRF/rate limits first)
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            licence::licence_layer,
        ))
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
    /// Desktop mode (`nf-server --desktop`); false in server mode.
    pub desktop: bool,
    /// The licence key file (NF_LICENCE_FILE; default licence.nflic in the data folder).
    pub licence_path: PathBuf,
    /// Refuse new work when the licence is read-only (NF_LICENCE_ENFORCE; default off:
    /// the state is shown and audited but nothing is gated).
    pub licence_enforce: bool,
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

/// How many different characters a setup token uses (RA-009: a token of one repeated character is refused).
fn distinct_chars(t: &str) -> usize {
    t.chars()
        .collect::<std::collections::BTreeSet<char>>()
        .len()
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
                allowed_hosts.push(allowed_host_entry(h)?);
            }
        }
        let mut trusted_proxies = Vec::new();
        if let Ok(v) = std::env::var("NF_TRUSTED_PROXY") {
            for p in v.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()) {
                trusted_proxies
                    .push(v4_of(p.parse::<IpAddr>().map_err(|_| {
                        format!("NF_TRUSTED_PROXY: '{p}' is not an IP address")
                    })?));
            }
        }
        let setup_token = match std::env::var("NF_SETUP_TOKEN") {
            Ok(t) if t.trim().len() >= 16 && distinct_chars(t.trim()) >= 8 => {
                Some(t.trim().to_string())
            }
            Ok(_) => {
                return Err("NF_SETUP_TOKEN must have at least 16 characters, of which at least 8 different ones (RA-009)".into());
            }
            Err(_) => None,
        };
        let licence_path = std::env::var("NF_LICENCE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("licence.nflic"));
        // Owner's release-gate switch: off by default. "1" or "true" turns enforcement on.
        let licence_enforce = std::env::var("NF_LICENCE_ENFORCE")
            .map(|v| v.trim() == "1" || v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);
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
            desktop: false,
            licence_path,
            licence_enforce,
        })
    }
}

/// Open storage and build the application state. Every failure is a message
/// for the operator; the caller exits non-zero.
pub(crate) fn build_app(cfg: Config) -> Result<Arc<App>, String> {
    build_app_with(
        cfg,
        std::env::var("NF_ACCEPT_UNANCHORED_AUDIT").as_deref() == Ok("1"),
    )
}

/// `accept_unanchored`: a log with entries and no anchor is accepted (and anchored now). Only for a log that
/// predates the anchor; see the audit section of the start-up code (RA-003).
pub(crate) fn build_app_with(cfg: Config, accept_unanchored: bool) -> Result<Arc<App>, String> {
    let (mut db, key) = storage::open_storage(&cfg.data_dir, &cfg.key_path)?;
    storage::remove_stale_temporaries(&cfg.data_dir);
    pdfconv::remove_stale_work(&cfg.data_dir);
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
                    "First-run setup: no administrator exists yet. Setup requires the one-time setup token (NF_SETUP_TOKEN, or the setup link printed by --desktop)."
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
        desktop: cfg.desktop,
        body_idle: BODY_IDLE,
        per_client_in_flight: 32,
        in_flight: Mutex::new(std::collections::HashMap::new()),
        hash_slots: Semaphore::new(2),
        upload_slots: Semaphore::new(4),
        replay_slots: Semaphore::new(2),
        pdf2dxf_exe: pdfconv::default_converter(),
        convert_timeout: pdfconv::DEFAULT_TIMEOUT,
        convert_slots: Arc::new(Semaphore::new(1)),
        anon_audit: Mutex::new((0, 0, 0)),
        anon_minute: Mutex::new(None),
        verified: Mutex::new((-1, Default::default())),
        licence: Mutex::new(licence::LicenceCache::default()),
        licence_keys: nf_licence::VERIFY_KEYS,
        licence_path: cfg.licence_path,
        licence_enforce: cfg.licence_enforce,
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
                // RA-003: a log with entries and no anchor is what a truncation plus a deleted anchor looks like.
                // It is not re-anchored silently. Only an operator who knows the log predates the anchor may accept it.
                if count > 0 && !accept_unanchored {
                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                        AuditChainBroken(format!(
                            "the audit log has {count} entries but no anchor: refusing to start. If this log predates the anchor, start once with NF_ACCEPT_UNANCHORED_AUDIT=1"
                        )),
                    )));
                }
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
        seed_known_clients(&db)
            .map_err(|e| format!("cannot seed the known sign-in addresses: {e}"))?;
    }
    // Licence: load, verify, evaluate, persist, audit. Never blocks start-up; a broken
    // audit chain keeps the behaviour above and licence logging goes to the log only.
    licence::init(&app);
    Ok(Arc::new(app))
}

/// A listener that is not on loopback answers to the names in NF_ALLOWED_HOSTS only. Without that list every
/// public request would get 421, so the start is refused instead (RA-007).
fn listen_needs_allowed_hosts(addr: &str, allowed_hosts: Option<&str>) -> Result<(), String> {
    let hosts_configured =
        allowed_hosts.is_some_and(|v| v.split(',').any(|h| !h.trim().is_empty()));
    let loopback = match addr.parse::<std::net::SocketAddr>() {
        Ok(sa) => sa.ip().is_loopback(),
        Err(_) => addr.starts_with("localhost:"),
    };
    if loopback || hosts_configured {
        return Ok(());
    }
    Err(format!(
        "NF_LISTEN={addr} is not a loopback address, but NF_ALLOWED_HOSTS is not set: every request would be answered 421. Set NF_ALLOWED_HOSTS to the host names clients use, or listen on 127.0.0.1."
    ))
}

/// Accept loop with the limits axum's own `serve` does not offer (S-3, S-8):
/// a time limit for the request head, a cap on open connections, and a
/// graceful stop: on SIGTERM or SIGINT no new connection is accepted and
/// requests in flight get `shutdown_grace` to finish.
async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    limits: Limits,
    hangup_stops: bool,
) -> Result<(), String> {
    let slots = Arc::new(Semaphore::new(limits.max_connections));
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut stop = stop_requested(hangup_stops)?;
    let mut builder = hyper::server::conn::http1::Builder::new();
    builder
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(limits.header_read);
    loop {
        // a connection is accepted only when a slot is free; the rest wait in the kernel's queue
        let accepted = tokio::select! {
            _ = &mut stop => None,
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
                elog!("nf-server: cannot accept a connection: {e}");
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
        _ = tokio::time::sleep(limits.shutdown_grace) => elog!("nf-server: requests still in flight after the grace period; stopping. Their transactions are rolled back by the database."),
    }
    Ok(())
}

/// Resolves when the process is asked to stop: SIGTERM or SIGINT on Unix (and SIGHUP,
/// which a closed terminal window sends, when `hangup_stops` is set: desktop mode only);
/// Ctrl+C, Ctrl+Break, closing the console window, or shutdown on Windows.
fn stop_requested(
    hangup_stops: bool,
) -> Result<std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>, String> {
    // Only Unix has SIGHUP; on other systems the flag changes nothing.
    let _ = hangup_stops;
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate())
            .map_err(|e| format!("cannot install the SIGTERM handler: {e}"))?;
        let mut int = signal(SignalKind::interrupt())
            .map_err(|e| format!("cannot install the SIGINT handler: {e}"))?;
        let mut hup = if hangup_stops {
            Some(
                signal(SignalKind::hangup())
                    .map_err(|e| format!("cannot install the SIGHUP handler: {e}"))?,
            )
        } else {
            None
        };
        Ok(Box::pin(async move {
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
                _ = async { match hup.as_mut() { Some(h) => { h.recv().await; } None => std::future::pending::<()>().await } } => {}
            }
        }))
    }
    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close, ctrl_shutdown};
        let io = |e: std::io::Error| format!("cannot install the console stop handler: {e}");
        let mut c = ctrl_c().map_err(io)?;
        let mut b = ctrl_break().map_err(io)?;
        let mut close = ctrl_close().map_err(io)?;
        let mut shutdown = ctrl_shutdown().map_err(io)?;
        Ok(Box::pin(async move {
            tokio::select! {
                _ = c.recv() => {}
                _ = b.recv() => {}
                _ = close.recv() => {}
                _ = shutdown.recv() => {}
            }
        }))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(Box::pin(async {
            let _ = tokio::signal::ctrl_c().await;
        }))
    }
}

/// Remove expired sessions and sign-in failures older than the throttle window. Returns rows removed.
/// Once per database: the addresses each account signed in from, taken from the audit log's `login` entries, become
/// known addresses. Without this an administrator's usual network is "unknown" after an upgrade (RA-006 residual).
/// The seeding is recorded in meta so it does not repeat.
fn seed_known_clients(db: &Connection) -> rusqlite::Result<()> {
    if storage::meta_get(db, "login_clients_seeded")?.is_some() {
        return Ok(());
    }
    let rows: Vec<(String, String, i64)> = q_all(
        db,
        "SELECT username, detail, at FROM audit WHERE action = 'login' AND detail LIKE 'from %' ORDER BY id",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    for (user, detail, at) in rows {
        if let Ok(ip) = detail["from ".len()..].trim().parse::<IpAddr>() {
            db.execute(
                "INSERT INTO login_clients(username, client, at) VALUES (?,?,?) ON CONFLICT(username, client) DO UPDATE SET at = MAX(at, excluded.at)",
                params![user, client_key(ip), at],
            )?;
        }
    }
    storage::meta_set(db, "login_clients_seeded", "1")?;
    Ok(())
}

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

/// What the program does, decided from its command-line arguments (the arguments after the program name).
#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    /// No arguments: the server, configured by the `NF_*` environment variables.
    Serve,
    /// `--desktop`: desktop mode. Its own options are read by `desktop::parse_args`.
    Desktop,
    /// `--help` or `-h`: print the usage and exit 0.
    Help,
    /// `--version`: print the version and exit 0.
    Version,
}

/// Decides what the arguments ask for. Nothing is created and no port is opened here: help and
/// version never touch the data folder, and an argument that is not understood is an error (exit 2).
fn invocation(args: &[String]) -> Result<Invocation, String> {
    let (mut help, mut version, mut desktop) = (false, false, false);
    // The first option that only desktop mode accepts; server mode refuses it.
    let mut desktop_only: Option<&str> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" | "-h" => help = true,
            "--version" => version = true,
            "--desktop" => desktop = true,
            "--no-browser" => desktop_only = desktop_only.or(Some("--no-browser")),
            "--port" => {
                // the value is checked by desktop::parse_args when desktop mode runs
                it.next();
                desktop_only = desktop_only.or(Some("--port"));
            }
            other if other.starts_with("--port=") => {
                desktop_only = desktop_only.or(Some("--port"));
            }
            other => {
                return Err(format!(
                    "unknown argument '{other}'; run nf-server --help for the usage"
                ));
            }
        }
    }
    if help {
        return Ok(Invocation::Help);
    }
    if version {
        return Ok(Invocation::Version);
    }
    if desktop {
        return Ok(Invocation::Desktop);
    }
    if let Some(flag) = desktop_only {
        return Err(format!(
            "{flag} is a desktop option and needs --desktop; server mode takes its settings from the NF_* environment variables (run nf-server --help)"
        ));
    }
    Ok(Invocation::Serve)
}

fn version_text() -> String {
    format!(
        "nf-server {} ({})\nNEXUS FABRIC development build. Not validated: no output may be used for manufacturing.",
        env!("CARGO_PKG_VERSION"),
        nf_core::ENGINE_VERSION
    )
}

fn help_text() -> String {
    format!(
        "NEXUS FABRIC server. Development build. Not validated: no output may be used for manufacturing.

usage:
  nf-server                    start the server, configured by the environment variables below
  nf-server --desktop          run desktop mode: one person, on this computer only
  nf-server --help, -h         show this help and exit (writes nothing, opens no port)
  nf-server --version          show the version and exit (writes nothing, opens no port)
Any other argument is an error: the program prints it and exits with status 2.

{desktop}

environment variables (server mode; desktop mode keeps its data in the per-user data folder):
  NF_LISTEN                 address and port to listen on (default 127.0.0.1:8787)
  NF_DATA_DIR               data folder (default: data, in the working directory)
  NF_KEY_FILE               storage key file (default: storage.key inside the data folder).
                            Keep it outside the data folder.
  NF_SETUP_TOKEN            one-time setup token for the first administrator, at least 16 characters.
                            Without it, a token is written to setup-token in the data folder.
  NF_ALLOWED_HOSTS          more host names the server answers to, comma-separated
  NF_TRUSTED_PROXY          addresses of reverse proxies whose X-Forwarded-For is used, comma-separated
  NF_COOKIE_SECURE          1 = session cookies are marked Secure (use behind HTTPS)
  NF_REQUEST_TIMEOUT_SECS   time limit for a whole request, 1 to 3600 (default 120)
  NF_HEADER_TIMEOUT_SECS    time limit for a request head, 1 to 600 (default 15)
  NF_MAX_CONNECTIONS        open connections, 1 to 100000 (default 512)
  NF_SHUTDOWN_GRACE_SECS    seconds requests in flight get when the server stops, 0 to 600 (default 20)
  NF_LICENCE_FILE           licence key file (default: licence.nflic inside the data folder)
  NF_LICENCE_ENFORCE        1 or true = refuse new work when the licence is read-only, and apply
                            seat limits (default: off — the licence state is loaded, shown and
                            audited, but nothing is gated)",
        desktop = desktop::USAGE
    )
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("replay-check") {
        replay_child();
        return;
    }
    // Read before anything is created: help, version and an unknown argument never create the
    // data folder, a key or a setup token, and never open a port.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = invocation(&args).unwrap_or_else(|e| {
        elog!("nf-server: {e}");
        std::process::exit(2);
    });
    match mode {
        Invocation::Help => {
            println!("{}", help_text());
            return;
        }
        Invocation::Version => {
            println!("{}", version_text());
            return;
        }
        Invocation::Desktop => {
            // Desktop mode is strictly opt-in: only an explicit --desktop argument selects it.
            if let Err(e) = desktop::run(&args).await {
                elog!("nf-server: cannot start desktop mode: {e}");
                std::process::exit(2);
            }
            return;
        }
        Invocation::Serve => {}
    }
    let stop = |msg: String| -> ! {
        elog!("nf-server: cannot start: {msg}");
        std::process::exit(2);
    };
    let addr = std::env::var("NF_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    listen_needs_allowed_hosts(&addr, std::env::var("NF_ALLOWED_HOSTS").ok().as_deref())
        .unwrap_or_else(|e| stop(e));
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
    // The licence is rechecked on the same tick: a renewed key file is picked up without a restart.
    let keeper = app.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = housekeeping(&keeper) {
                elog!("nf-server: housekeeping failed: {e}");
            }
            licence::recheck(&keeper);
        }
    });
    let service = router(app.clone());
    if let Err(e) = serve(listener, service, limits, false).await {
        elog!("nf-server: server error: {e}");
        std::process::exit(1);
    }
    // flush: fold the write-ahead log into the database file before the process ends
    if let Err(e) = app.db().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
        elog!("nf-server: could not checkpoint the database at shutdown: {e}");
    }
}

#[cfg(test)]
mod invocation_tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_select_their_mode() {
        assert_eq!(invocation(&args(&[])), Ok(Invocation::Serve));
        assert_eq!(invocation(&args(&["--help"])), Ok(Invocation::Help));
        assert_eq!(invocation(&args(&["-h"])), Ok(Invocation::Help));
        assert_eq!(invocation(&args(&["--version"])), Ok(Invocation::Version));
        assert_eq!(invocation(&args(&["--desktop"])), Ok(Invocation::Desktop));
        assert_eq!(
            invocation(&args(&["--desktop", "--port", "8861", "--no-browser"])),
            Ok(Invocation::Desktop)
        );
        assert_eq!(
            invocation(&args(&["--desktop", "--help"])),
            Ok(Invocation::Help)
        );
        assert_eq!(
            invocation(&args(&["--help", "--version"])),
            Ok(Invocation::Help)
        );
    }

    #[test]
    fn unknown_arguments_are_errors_even_next_to_help() {
        let e = invocation(&args(&["--open-everywhere"])).unwrap_err();
        assert!(e.contains("unknown argument '--open-everywhere'"), "{e}");
        assert!(invocation(&args(&["--help", "--bogus"])).is_err());
        assert!(invocation(&args(&["extra"])).is_err());
        assert!(invocation(&args(&[""])).is_err());
    }

    #[test]
    fn desktop_options_need_desktop_in_server_mode() {
        assert!(
            invocation(&args(&["--port", "8861"]))
                .unwrap_err()
                .contains("--port")
        );
        assert!(invocation(&args(&["--port=8861"])).is_err());
        assert!(
            invocation(&args(&["--no-browser"]))
                .unwrap_err()
                .contains("--no-browser")
        );
    }

    #[test]
    fn help_text_lists_the_environment_and_the_desktop_options() {
        let help = help_text();
        for name in [
            "NF_LISTEN",
            "NF_DATA_DIR",
            "NF_KEY_FILE",
            "NF_SETUP_TOKEN",
            "NF_ALLOWED_HOSTS",
            "NF_TRUSTED_PROXY",
            "NF_COOKIE_SECURE",
            "NF_REQUEST_TIMEOUT_SECS",
            "NF_HEADER_TIMEOUT_SECS",
            "NF_MAX_CONNECTIONS",
            "NF_SHUTDOWN_GRACE_SECS",
            "NF_LICENCE_FILE",
            "NF_LICENCE_ENFORCE",
            "--desktop",
            "--no-browser",
            "--port N",
        ] {
            assert!(help.contains(name), "help does not mention {name}");
        }
        assert!(help.contains("Development build. Not validated"));
        assert!(version_text().starts_with("nf-server "));
        assert!(version_text().contains("Not validated"));
    }
}
