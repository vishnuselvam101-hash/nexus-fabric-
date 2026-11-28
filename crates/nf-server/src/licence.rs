//! Licence glue: load, verify, evaluate, persist, audit, and the read-only gate.
//!
//! The state machine itself is `nf_licence::evaluate` (pure, time injected). This
//! module owns the key file, the persisted meta values (each HMAC'd with the
//! storage key, exactly like the audit anchor) and the in-memory cache.
//!
//! Enforcement is the owner's switch (`NF_LICENCE_ENFORCE`, default OFF): the
//! state is always computed truthfully, persisted, shown and audited, but while
//! enforcement is off nothing is refused — the middleware passes every request
//! and a seat limit is only logged (`licence_seat_limit_not_enforced`).
//!
//! Lock order: the licence cache is taken BEFORE the database lock, never the
//! other way round. A handler that needs both reads the cache (or the seat
//! count) first, releases it, and only then takes the database lock.

use crate::{ApiError, App, Ctx, auth_db, fail, fail_json, now, ok, q_count, require, storage};
use axum::http::{HeaderMap, Method, StatusCode, header};
use nf_licence::{GRACE, Payload, State, Status};
use rusqlite::{Connection, TransactionBehavior};
use serde_json::{Value, json};
use std::path::Path;

/// The in-memory licence state, rebuilt from the file and the meta table.
pub(crate) struct LicenceCache {
    /// Payload of the file that verified at the last load (None: no file, or invalid).
    pub payload: Option<Payload>,
    /// Last payload that ever verified (persisted; an invalid or removed file falls back to it).
    pub last_verified: Option<Payload>,
    /// Monotonic high-water mark of time (persisted).
    pub last_seen: u64,
    /// First-run time of the evaluation (persisted).
    pub eval_start: Option<u64>,
    /// Status as of the last load/recheck.
    pub status: Status,
    /// SHA-256 of the file bytes last loaded, valid or not (a changed file is audited once).
    file_sha: Option<String>,
    /// The rollback flag was already audited (audit the transition, not every tick).
    rollback_audited: bool,
}

impl Default for LicenceCache {
    fn default() -> Self {
        LicenceCache {
            payload: None,
            last_verified: None,
            last_seen: 0,
            eval_start: None,
            status: nf_licence::evaluate(None, 0, 0, None, None),
            file_sha: None,
            rollback_audited: false,
        }
    }
}

/// The stable name of a state, as the API and the audit log write it.
pub(crate) fn state_key(state: &State) -> &'static str {
    match state {
        State::Evaluation { .. } => "evaluation",
        State::Valid => "valid",
        State::Grace { .. } => "grace",
        State::ReadOnly { .. } => "read_only",
    }
}

/// The truthful state right now, from the cache (pure, no I/O).
pub(crate) fn current(c: &LicenceCache, t: u64) -> Status {
    nf_licence::evaluate(
        c.payload.as_ref(),
        t,
        c.last_seen,
        c.eval_start,
        c.last_verified.as_ref(),
    )
}

/// Seats the licence allows: the payload's seats, or 1 in evaluation. Read this
/// BEFORE taking the database lock (lock order, see the module comment).
pub(crate) fn seats_allowed(app: &App) -> u32 {
    let c = app.licence.lock().unwrap_or_else(|e| e.into_inner());
    c.payload
        .as_ref()
        .or(c.last_verified.as_ref())
        .map(|p| p.seats)
        .unwrap_or(1)
}

// ---------------------------------------------------------------- persisted meta (HMAC'd)

fn meta_mac(key: &[u8; 32], name: &str, value: &str) -> String {
    storage::hmac_hex(
        key,
        format!("nexus-fabric licence meta v1|{name}|{value}").as_bytes(),
    )
}

/// Read a persisted licence value. A missing, unreadable or tampered value is
/// absent (and logged); it can never improve the state.
fn meta_read(app: &App, db: &Connection, name: &str) -> Option<String> {
    let raw = match storage::meta_get(db, name) {
        Ok(v) => v?,
        Err(e) => {
            elog!("nf-server: cannot read licence meta {name}: {e}");
            return None;
        }
    };
    let v: Value = serde_json::from_str(&raw).ok()?;
    let value = v["value"].as_str()?;
    let mac = v["mac"].as_str()?;
    if !crate::crypto::constant_time_eq(mac, &meta_mac(&app.key, name, value)) {
        elog!(
            "nf-server: licence meta {name} does not verify (tampered, or from another key); treated as absent"
        );
        return None;
    }
    Some(value.to_string())
}

/// Persist one licence value with its HMAC. Tests use this to place a value
/// exactly as the server would have written it.
pub(crate) fn meta_write(
    app: &App,
    db: &Connection,
    name: &str,
    value: &str,
) -> rusqlite::Result<()> {
    storage::meta_set(
        db,
        name,
        &json!({ "value": value, "mac": meta_mac(&app.key, name, value) }).to_string(),
    )
}

// ---------------------------------------------------------------- load / recheck

/// Why a refresh runs: start-up (always audits the state), the hourly tick
/// (audits transitions only), or an installed key file (audits the load).
enum Mode<'a> {
    Startup,
    Periodic,
    Installed(&'a str),
}

/// Start-up: load the file, evaluate, persist, audit `licence_status`. Never
/// blocks start-up; a broken audit chain keeps its own behaviour and licence
/// logging goes to the log only.
pub(crate) fn init(app: &App) {
    refresh(app, Mode::Startup);
}

/// The hourly tick (and any renewal without a restart): reload the file,
/// recompute, advance the monotonic time, audit only transitions.
pub(crate) fn recheck(app: &App) {
    refresh(app, Mode::Periodic);
}

/// After POST /api/licence wrote a verified file: reload and audit the load
/// under the administrator's name.
pub(crate) fn installed(app: &App, user: &str) {
    refresh(app, Mode::Installed(user));
}

fn refresh(app: &App, mode: Mode<'_>) {
    let bytes = match std::fs::read(&app.licence_path) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            elog!(
                "nf-server: cannot read the licence file {}: {e}",
                app.licence_path.display()
            );
            None
        }
    };
    let (payload, invalid) = match &bytes {
        Some(b) => match nf_licence::verify_with_keys(b, app.licence_keys) {
            Ok(p) => (Some(p), None),
            Err(r) => (None, Some(r)),
        },
        None => (None, None),
    };
    let sha = bytes.as_ref().map(|b| nf_core::sha256_hex(b));
    let t = now();
    let mut c = app.licence.lock().unwrap_or_else(|e| e.into_inner());
    let mut db = app.db();
    let stored_seen = meta_read(app, &db, "licence_last_seen")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let mut eval_start =
        meta_read(app, &db, "licence_eval_start").and_then(|v| v.parse::<u64>().ok());
    let stored_verified = meta_read(app, &db, "licence_last_verified")
        .and_then(|v| serde_json::from_str::<Payload>(&v).ok());
    let last_seen = t.max(stored_seen).max(c.last_seen);
    // A file that verifies is the newest truth; otherwise the persisted payload
    // (or the cache, if the meta was tampered with) keeps its own state.
    let last_verified = payload
        .clone()
        .or(stored_verified.clone())
        .or_else(|| c.last_verified.clone());
    let status = nf_licence::evaluate(
        payload.as_ref(),
        t,
        last_seen,
        eval_start,
        last_verified.as_ref(),
    );
    let user = match &mode {
        Mode::Installed(u) => (*u).to_string(),
        _ => "-".to_string(),
    };
    let mut events: Vec<(&'static str, String)> = Vec::new();
    let file_changed = sha != c.file_sha;
    if file_changed && let Some(reason) = invalid {
        events.push((
            "licence_invalid",
            format!("the licence file was refused: {reason}"),
        ));
    }
    if let Some(p) = &payload
        && (file_changed || matches!(mode, Mode::Installed(_)))
    {
        events.push((
            "licence_loaded",
            format!(
                "licence {} for {} ({} seat(s), key {})",
                p.licence_id, p.customer, p.seats, p.key_id
            ),
        ));
    }
    let new_key = state_key(&status.state);
    let old_key = state_key(&c.status.state);
    match &mode {
        Mode::Startup => events.push((
            "licence_status",
            format!(
                "state {new_key}; enforcement {}",
                if app.licence_enforce { "on" } else { "off" }
            ),
        )),
        _ => {
            if new_key != old_key {
                match &status.state {
                    State::Grace { grace_until } => events.push((
                        "licence_expired",
                        format!("licence past its window; full function until {grace_until}"),
                    )),
                    State::ReadOnly { reason } => events.push((
                        "licence_readonly",
                        format!("new work refused when enforced: {reason}"),
                    )),
                    _ => events.push(("licence_status", format!("{old_key} -> {new_key}"))),
                }
            }
        }
    }
    if status.clock_rollback && !c.rollback_audited {
        events.push((
            "licence_clock_rollback",
            format!(
                "the clock is {} s behind the last seen time; the state uses the last seen time",
                last_seen.saturating_sub(t)
            ),
        ));
    }
    // Meta and audit in one transaction. A broken audit chain is logged, never
    // fatal here: the meta values are still persisted.
    let mut write = || -> rusqlite::Result<()> {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if eval_start.is_none() {
            eval_start = Some(t);
            meta_write(app, &tx, "licence_eval_start", &t.to_string())?;
        }
        if last_seen > stored_seen {
            meta_write(app, &tx, "licence_last_seen", &last_seen.to_string())?;
        }
        if let Some(p) = &payload
            && stored_verified.as_ref() != Some(p)
        {
            meta_write(
                app,
                &tx,
                "licence_last_verified",
                &serde_json::to_string(p).unwrap_or_default(),
            )?;
        }
        for (action, detail) in &events {
            if let Err(e) = app.audit(&tx, &user, action, detail) {
                elog!("nf-server: could not audit {action}: {e}");
            }
        }
        tx.commit()
    };
    if let Err(e) = write() {
        elog!("nf-server: could not persist the licence state: {e}");
    }
    c.payload = payload;
    c.last_verified = last_verified;
    c.last_seen = last_seen;
    c.eval_start = eval_start;
    c.rollback_audited = status.clock_rollback;
    c.status = status;
    c.file_sha = sha;
}

// ---------------------------------------------------------------- the read-only gate

/// The POST endpoints that start new work. Every GET, the review workflow
/// (obligations, advisories), project creation, the audit endpoints and
/// /api/licence itself are exempt.
pub(crate) fn gated(method: &Method, path: &str) -> bool {
    let seg: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // every write to an order: parts and values are PUT, the rest POST
    if *method == Method::PUT {
        return matches!(seg.as_slice(), ["api", "orders", id, "parts" | "values" | "rules"] if id.parse::<i64>().is_ok())
            || matches!(seg.as_slice(), ["api", "projects", id, "rules"] if id.parse::<i64>().is_ok());
    }
    if *method != Method::POST {
        return false;
    }
    match seg.as_slice() {
        [
            "api",
            "orders",
            id,
            "confirm" | "decisions" | "compute" | "preview",
        ] => id.parse::<i64>().is_ok(),
        ["api", "orders", id, "rules", "impact"] => id.parse::<i64>().is_ok(),
        ["api", "orders", id, "decisions", did, "undo"] => {
            id.parse::<i64>().is_ok() && did.parse::<i64>().is_ok()
        }
        ["api", "orders", id, "pages", n, "skip" | "read"] => {
            id.parse::<i64>().is_ok() && n.parse::<i64>().is_ok()
        }
        [
            "api",
            "projects",
            id,
            "calculations" | "documents" | "orders",
        ]
        | ["api", "orders", id, "files"]
        | ["api", "documents", id, "pdf-to-dxf"]
        | [
            "api",
            "calculations",
            id,
            "replay" | "recompute" | "corrections",
        ] => id.parse::<i64>().is_ok(),
        _ => false,
    }
}

/// Inside the security layer (host/CSRF/rate limits run first). With enforcement
/// on, a read-only licence refuses the gated endpoints with 403; the refusal is
/// audited, rate-limited like other unauthenticated events (V-34). With
/// enforcement off every request passes, whatever the state.
pub(crate) async fn licence_layer(
    axum::extract::State(app): Ctx,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if gated(req.method(), req.uri().path()) {
        let status = {
            let c = app.licence.lock().unwrap_or_else(|e| e.into_inner());
            current(&c, now())
        };
        if status.blocks_new_work(app.licence_enforce) {
            let reason = match &status.state {
                State::ReadOnly { reason } => *reason,
                _ => "read_only",
            };
            let detail = format!("{} {} refused: {reason}", req.method(), req.uri().path());
            {
                let db = app.db();
                if let Err(e) = app.audit_anon(&db, "-", "licence_blocked", &detail) {
                    elog!("nf-server: could not audit a licence refusal: {e}");
                }
            }
            return fail_json(
                StatusCode::FORBIDDEN,
                json!({
                    "error": "the licence is read-only: new work is refused. Existing jobs, records and files stay readable and exportable. Renew via Chuo-Sogo.",
                    "error_ja": "ライセンスが読み取り専用です：新規作業は拒否されます。既存のジョブ・記録・ファイルは引き続き閲覧・出力できます。更新は中部総合まで。",
                    "licence": "read_only"
                }),
            )
            .into_response();
        }
    }
    next.run(req).await
}

/// Seat gate for add_user / set_user_active (server mode only; desktop is one
/// person). `used` is the active account count the action would produce,
/// `seats` the allowance read before the database lock. Enforced: 403
/// `seat_limit`, audited. Not enforced: the occurrence is audited as
/// `licence_seat_limit_not_enforced` and allowed. The caller commits its
/// transaction before returning the refusal, so the audit entry survives.
pub(crate) fn seat_gate(
    app: &App,
    tx: &Connection,
    user: &str,
    used: i64,
    seats: u32,
    what: &str,
) -> Result<(), ApiError> {
    if app.desktop || used < seats as i64 {
        return Ok(());
    }
    let detail = format!("{what}: {used} active account(s), the licence covers {seats} seat(s)");
    if !app.licence_enforce {
        app.audit(tx, user, "licence_seat_limit_not_enforced", &detail)?;
        return Ok(());
    }
    app.audit(tx, user, "licence_seat_limit", &detail)?;
    Err(fail_json(
        StatusCode::FORBIDDEN,
        json!({
            "error": format!("the licence covers {seats} seat(s) and all are in use. Deactivate an account, or renew with more seats via Chuo-Sogo."),
            "error_ja": format!("ライセンスの席数（{seats}席）はすべて使用中です。アカウントを無効化するか、中部総合で席数を増やして更新してください。"),
            "licence": "seat_limit"
        }),
    ))
}

// ---------------------------------------------------------------- API

/// The licence as the API shows it: the truthful state plus the signed fields.
/// The caller has already released the database lock (lock order).
fn status_json(app: &App, seats_used: i64) -> Value {
    let c = app.licence.lock().unwrap_or_else(|e| e.into_inner());
    let st = current(&c, now());
    let p = c.payload.as_ref().or(c.last_verified.as_ref());
    json!({
        "state": state_key(&st.state),
        "enforced": app.licence_enforce,
        "file_verified": c.payload.is_some(),
        "clock_rollback": st.clock_rollback,
        "edition": p.map(|p| match p.edition {
            nf_licence::Edition::Pilot => "pilot",
            nf_licence::Edition::Standard => "standard",
            nf_licence::Edition::Site => "site",
        }),
        "licence_id": p.map(|p| p.licence_id.as_str()),
        "customer": p.map(|p| p.customer.as_str()),
        "seats": p.map(|p| p.seats).unwrap_or(1),
        "seats_used": seats_used,
        "valid_from": p.map(|p| p.valid_from),
        "valid_until": p.map(|p| p.valid_until),
        "grace_until": p.map(|p| p.valid_until.saturating_add(GRACE)),
        "days_left": match &st.state { State::Evaluation { days_left, .. } => Some(*days_left), _ => None },
        "note": match &st.state { State::Evaluation { note, .. } => *note, _ => None },
        "reason": match &st.state { State::ReadOnly { reason } => Some(*reason), _ => None },
        "issuer": p.map(|p| p.issuer.as_str()),
        "key_id": p.map(|p| p.key_id.as_str()),
        "renewal_contact": "Chuo-Sogo",
    })
}

/// GET /api/licence: any signed-in user.
pub(crate) async fn licence_status(
    axum::extract::State(app): Ctx,
    headers: HeaderMap,
) -> crate::Api {
    let used = {
        let db = app.db();
        auth_db(&db, &app, &headers)?;
        q_count(&db, "SELECT COUNT(*) FROM users WHERE active = 1", [])?
    };
    ok(status_json(&app, used))
}

/// POST /api/licence: an administrator installs a key file (the offline monthly
/// renewal path). The raw `.nflic` body is read here, after authentication, with
/// its own 8 KiB limit. Works with enforcement on or off.
pub(crate) async fn licence_upload(
    axum::extract::State(app): Ctx,
    headers: HeaderMap,
    body: axum::body::Body,
) -> crate::Api {
    use http_body_util::BodyExt;
    let u = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin"])?;
        u
    };
    let too_large = || {
        fail(
            StatusCode::PAYLOAD_TOO_LARGE,
            "the licence file exceeds the 8 KiB limit",
        )
    };
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|n| n > nf_licence::MAX_FILE_BYTES as u64)
    {
        return Err(too_large());
    }
    let mut bytes: Vec<u8> = Vec::new();
    let mut body = body;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| {
            fail(
                StatusCode::BAD_REQUEST,
                "the licence upload was interrupted",
            )
        })?;
        if let Some(chunk) = frame.data_ref() {
            if bytes.len() + chunk.len() > nf_licence::MAX_FILE_BYTES {
                return Err(too_large());
            }
            bytes.extend_from_slice(chunk);
        }
    }
    if let Err(reason) = nf_licence::verify_with_keys(&bytes, app.licence_keys) {
        let db = app.db();
        if let Err(e) = app.audit(
            &db,
            &u.username,
            "licence_invalid",
            &format!("upload refused: {reason}"),
        ) {
            elog!("nf-server: could not audit a refused licence upload: {e}");
        }
        return Err(fail_json(
            StatusCode::BAD_REQUEST,
            json!({
                "error": format!("the licence file was refused: {reason}. Nothing was changed."),
                "error_ja": "ライセンスファイルを拒否しました。形式・署名・発行者鍵を確認してください。何も変更されていません。",
                "licence": "invalid"
            }),
        ));
    }
    write_file(&app.licence_path, &bytes).map_err(|e| {
        ApiError::Internal(format!(
            "cannot write the licence file {}: {e}",
            app.licence_path.display()
        ))
    })?;
    installed(&app, &u.username);
    let used = {
        let db = app.db();
        q_count(&db, "SELECT COUNT(*) FROM users WHERE active = 1", [])?
    };
    ok(status_json(&app, used))
}

/// Atomic, owner-only write: temporary file beside the target, fsync, rename,
/// directory fsync (the blob store's pattern).
fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".licence-tmp-{}",
        hex::encode(crate::crypto::random_bytes::<8>())
    ));
    let result = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)?;
        #[cfg(unix)]
        std::fs::File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The licence field of GET /api/health: the truthful state and the owner's
/// switch, nothing else (unauthenticated).
pub(crate) fn health_field(app: &App) -> Value {
    let c = app.licence.lock().unwrap_or_else(|e| e.into_inner());
    let st = current(&c, now());
    json!({ "state": state_key(&st.state), "enforced": app.licence_enforce })
}
