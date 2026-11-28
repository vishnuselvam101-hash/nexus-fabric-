//! Account recovery and optional automatic sign-in on a personal computer (AUTH-1).
//!
//! - A recovery code (20 characters from a 32-symbol alphabet = 100 bits, from the operating system's random
//!   source) is shown once; only an Argon2 hash with its own salt is stored. It is never logged or audited.
//! - Recovery is throttled exactly like sign-in (same table, same rules) and answers every failure the same way.
//! - Automatic sign-in exists in desktop mode only: a single-use launch token (256 bits, 60 s, held hashed in
//!   memory) opens a session for the NF account "os:<operating-system user>", which is never an administrator.

use super::*;
use std::time::Instant;

/// 32 symbols: digits and capitals without 0 O 1 I L, plus '@'. 20 symbols = 100 bits.
const CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ@";
pub(crate) const CODE_LEN: usize = 20;
const LAUNCH_TTL: Duration = Duration::from_secs(60);
const LAUNCH_MAX_PENDING: usize = 8;
const LAUNCH_FAILURES: i64 = 20;
const OS_PREFIX: &str = "os:";

#[derive(Default)]
pub(crate) struct AuthState {
    /// (SHA-256 of an unused launch token, expiry)
    pub(crate) launch: Vec<(String, Instant)>,
    /// The operating-system user of this desktop session (None in server mode).
    pub(crate) os_user: Option<String>,
}

fn os_random<const N: usize>() -> Result<[u8; N], ApiError> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b)
        .map_err(|e| ApiError::Internal(format!("operating system random source: {e}")))?;
    Ok(b)
}

/// A fresh recovery code, shown as four groups of five.
fn new_recovery_code() -> Result<String, ApiError> {
    let b: [u8; CODE_LEN] = os_random()?;
    let raw: Vec<char> = b
        .iter()
        .map(|x| CODE_ALPHABET[(x & 31) as usize] as char)
        .collect();
    Ok(raw
        .chunks(5)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-"))
}

/// What is hashed and compared: capitals only, without separators or spaces.
fn normalise_code(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// Generate a code and its salted hash (Argon2 off the async workers).
/// Returns (code to show, salt hex, hash).
pub(crate) async fn make_recovery(app: &Arc<App>) -> Result<(String, String, String), ApiError> {
    let code = new_recovery_code()?;
    let salt = hex::encode(os_random::<16>()?);
    let hash = hash_blocking(app, normalise_code(&code), salt.clone()).await?;
    Ok((code, salt, hash))
}

pub(crate) fn auto_signin_on(db: &Connection) -> bool {
    storage::meta_get(db, "auto_signin")
        .ok()
        .flatten()
        .is_some_and(|v| v == "1")
}

#[cfg(windows)]
#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetUserNameW(buf: *mut u16, len: *mut u32) -> i32;
}

/// Windows: the account of this process from the operating system (not the environment, which a launcher can set).
#[cfg(windows)]
fn raw_os_user() -> Option<String> {
    let mut buf = [0u16; 257];
    let mut len = buf.len() as u32;
    // SAFETY: buf holds `len` u16 values; GetUserNameW writes at most that many and sets len (with the NUL).
    let ok = unsafe { GetUserNameW(buf.as_mut_ptr(), &mut len) };
    if ok == 0 || len == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(
        &buf[..(len as usize - 1).min(buf.len())],
    ))
}

/// Elsewhere: the real uid looked up in /etc/passwd.
#[cfg(not(windows))]
fn raw_os_user() -> Option<String> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let uid = status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))?
        .split_whitespace()
        .next()?
        .to_string();
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() > 2 && f[2] == uid).then(|| f[0].to_string())
    })
}

/// The operating-system user running this process. None when it cannot be told.
pub(crate) fn os_user_name() -> Option<String> {
    let raw = raw_os_user()?;
    let clean: String = raw.chars().filter(|c| !c.is_control()).collect();
    let clean = clean.trim().to_string();
    (!clean.is_empty()).then(|| clip(&clean, MAX_USERNAME - OS_PREFIX.len()))
}

/// One launch token for the browser start link, when automatic sign-in is on (desktop mode only).
pub(crate) fn issue_launch_token(app: &App) -> Option<String> {
    if !app.desktop || !auto_signin_on(&app.db()) {
        return None;
    }
    let mut st = app.auth.lock().unwrap_or_else(|e| e.into_inner());
    st.os_user.as_ref()?;
    let token = hex::encode(os_random::<32>().ok()?);
    let now_i = Instant::now();
    st.launch.retain(|(_, exp)| *exp > now_i);
    while st.launch.len() >= LAUNCH_MAX_PENDING {
        st.launch.remove(0);
    }
    st.launch
        .push((nf_core::sha256_hex(token.as_bytes()), now_i + LAUNCH_TTL));
    Some(token)
}

fn recovery_failed() -> ApiError {
    fail(StatusCode::UNAUTHORIZED, "recovery failed")
}

fn session_cookie(app: &App, token: &str) -> String {
    format!(
        "{}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200{}",
        app.cookie_name(),
        if app.cookie_secure { "; Secure" } else { "" }
    )
}

/// `POST /api/recover {username, recovery_code, new_password}`.
pub(crate) async fn recover(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    Json(body): Json<Value>,
) -> Api {
    let raw_name = s(&body, "username");
    let code = normalise_code(&s(&body, "recovery_code"));
    let new = s(&body, "new_password");
    let username = clip(&raw_name, MAX_USERNAME);
    password_policy(&raw_name, &new).map_err(|m| fail(StatusCode::BAD_REQUEST, &m))?;
    let plausible = valid_username(&raw_name) && code.chars().count() == CODE_LEN;
    let t = now() as i64;
    let row: Option<(i64, String, String)> = {
        let mut db = app.db();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM login_failures WHERE at < ?",
            params![t - THROTTLE_WINDOW],
        )?;
        if let Some(refusal) = login_throttle(&tx, &username, &client, t)? {
            tx.execute(
                "INSERT INTO login_failures(username, at, client, admitted) VALUES (?,?,?,0)",
                params![username, t, client],
            )?;
            app.audit_anon(
                &tx,
                &username,
                "recover_throttled",
                &format!("from {client}"),
            )?;
            tx.commit()?;
            return Err(refusal);
        }
        // a wrong code counts as a failed sign-in: reserved before any hashing
        tx.execute(
            "INSERT INTO login_failures(username, at, client, admitted) VALUES (?,?,?,1)",
            params![username, t, client],
        )?;
        let row = if plausible {
            tx.query_row(
                "SELECT id, recovery_salt, recovery_hash FROM users WHERE username = ? AND active = 1 AND recovery_hash IS NOT NULL",
                params![raw_name],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
        } else {
            None
        };
        tx.commit()?;
        row
    };
    // hash for unknown names and malformed codes too, so that time does not reveal which accounts exist
    let (id, salt, stored) =
        row.unwrap_or((0, "00000000000000000000000000000000".into(), String::new()));
    let computed = hash_blocking(&app, code, salt).await?;
    if id == 0 || !crypto::constant_time_eq(&computed, &stored) {
        let db = app.db();
        app.audit_anon(&db, &username, "recover_failed", &format!("from {client}"))?;
        return Err(recovery_failed());
    }
    let nsalt = hex::encode(os_random::<16>()?);
    let nhash = hash_blocking(&app, new, nsalt.clone()).await?;
    let (ncode, rsalt, rhash) = make_recovery(&app).await?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // single use: the update only applies while the code that was checked is still the stored one
    let changed = tx.execute(
        "UPDATE users SET salt = ?, hash = ?, recovery_salt = ?, recovery_hash = ?, must_change = 0 WHERE id = ? AND recovery_hash = ? AND active = 1",
        params![nsalt, nhash, rsalt, rhash, id, stored],
    )?;
    if changed == 0 {
        app.audit_anon(&tx, &username, "recover_failed", &format!("from {client}"))?;
        tx.commit()?;
        return Err(recovery_failed());
    }
    tx.execute("DELETE FROM sessions WHERE user_id = ?", params![id])?;
    tx.execute(
        "DELETE FROM login_failures WHERE username = ?",
        params![username],
    )?;
    app.audit(
        &tx,
        &username,
        "account.recovered",
        &format!("from {client}; all sessions revoked; recovery code replaced"),
    )?;
    tx.commit()?;
    ok(json!({ "ok": true, "recovery_code": ncode }))
}

/// The signed-in administrator proves they know their own password. Wrong passwords are throttled with the
/// same counter as `/api/password`.
async fn confirm_admin_password(
    app: &Arc<App>,
    client: &str,
    headers: &HeaderMap,
    password: String,
) -> Result<User, ApiError> {
    let t = now() as i64;
    let (u, salt, hash, key) = {
        let mut db = app.db();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let u = auth_db(&tx, app, headers)?;
        require(&u, &["admin"])?;
        let key = format!("pwchange:{}", u.id);
        if failures_of(&tx, &key, t - THROTTLE_WINDOW)? >= PER_ACCOUNT_FAILURES {
            return Err(fail(
                StatusCode::TOO_MANY_REQUESTS,
                "too many wrong current passwords; try again in 15 minutes",
            ));
        }
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
    let wrong = || fail(StatusCode::FORBIDDEN, "current password is incorrect");
    if password.len() > 1024 {
        return Err(wrong());
    }
    let computed = hash_blocking(app, password, salt).await?;
    let db = app.db();
    if !crypto::constant_time_eq(&computed, &hash) {
        app.audit(&db, &u.username, "admin_password_refused", "")?;
        return Err(wrong());
    }
    db.execute(
        "DELETE FROM login_failures WHERE username = ?",
        params![key],
    )?;
    Ok(u)
}

/// `POST /api/me/recovery-code {password}`: a new code for the signed-in administrator.
pub(crate) async fn new_my_recovery_code(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Api {
    let u = confirm_admin_password(&app, &client, &headers, s(&body, "password")).await?;
    let (code, salt, hash) = make_recovery(&app).await?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "UPDATE users SET recovery_salt = ?, recovery_hash = ? WHERE id = ?",
        params![salt, hash, u.id],
    )?;
    app.audit(
        &tx,
        &u.username,
        "account.recovery_code_created",
        "the previous code is no longer valid",
    )?;
    tx.commit()?;
    ok(json!({ "ok": true, "recovery_code": code }))
}

/// `POST /api/users/{id}/reset-password {new_password, password}` (administrator, own password required, not on
/// oneself).
pub(crate) async fn reset_password(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let new = s(&body, "new_password");
    let u = confirm_admin_password(&app, &client, &headers, s(&body, "password")).await?;
    if id == u.id {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "you cannot reset your own password here; use change password",
        ));
    }
    let target: String = app
        .db()
        .query_row(
            "SELECT username FROM users WHERE id = ?",
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(not_found)?;
    if target.starts_with(OS_PREFIX) {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "this account signs in automatically and has no password",
        ));
    }
    password_policy(&target, &new)
        .map_err(|m| fail(StatusCode::BAD_REQUEST, &format!("new {m}")))?;
    let salt = hex::encode(os_random::<16>()?);
    let hash = hash_blocking(&app, new, salt.clone()).await?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // the administrator must still be one when the change is written
    let still = auth_db(&tx, &app, &headers)?;
    require(&still, &["admin"])?;
    if tx.execute(
        "UPDATE users SET salt = ?, hash = ?, must_change = 1 WHERE id = ?",
        params![salt, hash, id],
    )? == 0
    {
        return Err(not_found());
    }
    tx.execute("DELETE FROM sessions WHERE user_id = ?", params![id])?;
    app.audit(
        &tx,
        &u.username,
        "account.password_reset",
        &format!("user #{id}; sessions revoked; change required at next sign-in"),
    )?;
    tx.commit()?;
    ok(json!({ "ok": true, "must_change_password": true }))
}

/// `POST /api/settings/auto-signin {enabled, password}` (administrator, own password required).
pub(crate) async fn set_auto_signin(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Api {
    let Some(enabled) = body.get("enabled").and_then(|v| v.as_bool()) else {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "enabled must be true or false",
        ));
    };
    if enabled && !app.desktop {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "automatic sign-in is only available in desktop mode",
        ));
    }
    let u = confirm_admin_password(&app, &client, &headers, s(&body, "password")).await?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    storage::meta_set(&tx, "auto_signin", if enabled { "1" } else { "0" })?;
    if !enabled {
        // sessions of automatic sign-in do not outlive the setting
        tx.execute(
            "DELETE FROM sessions WHERE user_id IN (SELECT id FROM users WHERE username LIKE 'os:%')",
            [],
        )?;
    }
    app.audit(
        &tx,
        &u.username,
        "settings.auto_signin",
        if enabled { "enabled" } else { "disabled" },
    )?;
    tx.commit()?;
    drop(db);
    if !enabled {
        app.auth
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .launch
            .clear();
    }
    ok(json!({ "ok": true, "auto_signin": enabled }))
}

/// `POST /api/launch {token}`: trade the single-use launch token for a session of the OS user's account.
pub(crate) async fn launch(
    State(app): Ctx,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    Json(body): Json<Value>,
) -> Api {
    if !app.desktop {
        return Err(not_found());
    }
    let invalid = || fail(StatusCode::UNAUTHORIZED, "launch link invalid or expired");
    let t = now() as i64;
    let key = format!("launch:{client}");
    {
        let db = app.db();
        if failures_of(&db, &key, t - PER_CLIENT_WINDOW)? >= LAUNCH_FAILURES {
            return Err(fail(
                StatusCode::TOO_MANY_REQUESTS,
                "too many failed launch attempts; try again in 10 minutes",
            ));
        }
    }
    let given = s(&body, "token");
    let hit = if given.is_empty() || given.len() > 200 {
        None
    } else {
        let h = nf_core::sha256_hex(given.as_bytes());
        let mut st = app.auth.lock().unwrap_or_else(|e| e.into_inner());
        let now_i = Instant::now();
        let mut found = None;
        for (i, (th, exp)) in st.launch.iter().enumerate() {
            if crypto::constant_time_eq(th, &h) {
                found = Some((i, *exp > now_i));
            }
        }
        // single use: removed whether or not it is still valid
        let valid = found.map(|(i, v)| {
            st.launch.remove(i);
            v
        });
        let os = st.os_user.clone();
        valid.filter(|v| *v).and(os)
    };
    let refuse = |app: &App| -> Result<ApiError, ApiError> {
        let db = app.db();
        db.execute(
            "INSERT INTO login_failures(username, at, client) VALUES (?,?,?)",
            params![key, t, client],
        )?;
        app.audit_anon(&db, "-", "launch_refused", &format!("from {client}"))?;
        Ok(invalid())
    };
    let Some(os) = hit else {
        return Err(refuse(&app)?);
    };
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if !auto_signin_on(&tx) {
        drop(tx);
        drop(db);
        return Err(refuse(&app)?);
    }
    let name = format!("{OS_PREFIX}{os}");
    let existing: Option<(i64, String, bool)> = tx
        .query_row(
            "SELECT id, role, active FROM users WHERE username = ?",
            params![name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (id, role) = match existing {
        // a disabled account, or one that somehow holds the administrator role, is never signed in automatically
        Some((_, role, active)) if !active || role == "admin" => {
            drop(tx);
            drop(db);
            return Err(refuse(&app)?);
        }
        Some((id, role, _)) => (id, role),
        None => {
            // no password exists for this account: a random salt and a random value that nothing hashes to
            let salt = hex::encode(os_random::<16>()?);
            let junk = hex::encode(os_random::<32>()?);
            tx.execute(
                "INSERT INTO users(username, role, salt, hash, created_at) VALUES (?,?,?,?,?)",
                params![name, "engineer", salt, junk, t],
            )?;
            let id = tx.last_insert_rowid();
            app.audit(
                &tx,
                &name,
                "user_created",
                &format!("{name} (engineer), mapped to the operating-system user {os}"),
            )?;
            (id, "engineer".to_string())
        }
    };
    let token = hex::encode(os_random::<32>()?);
    tx.execute("DELETE FROM sessions WHERE expires_at <= ?", params![t])?;
    tx.execute(
        "INSERT INTO sessions(token_hash, user_id, expires_at) VALUES (?,?,?)",
        params![
            nf_core::sha256_hex(token.as_bytes()),
            id,
            (now() + 12 * 3600) as i64
        ],
    )?;
    app.audit(
        &tx,
        &name,
        "login.auto",
        &format!("operating-system user {os}, from {client}"),
    )?;
    tx.commit()?;
    Ok((
        [(header::SET_COOKIE, session_cookie(&app, &token))],
        Json(json!({ "username": name, "role": role })),
    )
        .into_response())
}
