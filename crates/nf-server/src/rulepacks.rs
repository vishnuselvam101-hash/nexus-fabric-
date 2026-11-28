//! PAR-2 factory rule packs (server): import and export of a pack (JSON), assignment to a project or an order, the
//! onboarding interview, and audited, undoable confirmations. Every write is a person's: the AI operator (user kind
//! "agent") reads packs and gets 403 on every write. An AI suggestion is a question, never a confirmation.
//! The engine reads a pack through `pack_overlay` (called by `load_compute_input` and the preview paths).
use super::orders::still_writer;
use super::*;
use nf_core::rulepack::{self, CONFIRMED, Pack};

/// Largest pack accepted (bytes). A pack is about 20 rules; anything near this is not a rule pack.
const MAX_PACK_BYTES: usize = 64 * 1024;

pub(crate) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/api/rule-packs", get(list_packs).post(import_pack))
        .route("/api/rule-packs/{id}", get(export_pack))
        .route("/api/rule-packs/{id}/assign", post(assign_pack))
        .route("/api/rule-packs/{id}/unassign", post(unassign_pack))
        .route("/api/rule-packs/{id}/interview", get(get_interview))
        .route("/api/rule-packs/{id}/interview/answers", post(answer))
        .route(
            "/api/rule-packs/{id}/confirmations/{cid}/undo",
            post(undo_confirmation),
        )
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn bad(code: &str, msg: &str) -> ApiError {
    fail_code(StatusCode::UNPROCESSABLE_ENTITY, code, &[], msg)
}

/// Person-only: the AI operator is refused with 403 before anything is read or written. The refused attempt is
/// audited (a refusal of anyone else is the ordinary role check).
fn person_writer(app: &App, headers: &HeaderMap) -> Result<User, ApiError> {
    let db = app.db();
    let u = auth_db(&db, app, headers)?;
    if u.kind == "agent"
        && let Err(e) = app.audit(
            &db,
            &u.username,
            "rule_pack_refused",
            "AI operator attempted a rule pack write (403)",
        )
    {
        elog!("nf-server: could not audit a refused rule pack write: {e}");
    }
    require(&u, &["admin", "engineer"])?;
    if u.kind == "agent" {
        return Err(fail_code(
            StatusCode::FORBIDDEN,
            "agent_cannot_edit_rules",
            &[],
            "the AI operator cannot change factory rule packs; a person does, the operator may ask a question",
        ));
    }
    Ok(u)
}

fn load_pack(db: &Connection, id: i64) -> Result<(Pack, String), ApiError> {
    let raw: String = db
        .query_row(
            "SELECT pack_json FROM rule_packs WHERE id = ?",
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(not_found)?;
    let v: Value = serde_json::from_str(&raw).map_err(|_| {
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "rule pack is stored unreadable",
        )
    })?;
    let pack = Pack::from_stored(&v).map_err(|m| {
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("rule pack is stored unreadable: {m}"),
        )
    })?;
    let sha = nf_core::sha256_hex(pack.to_json().to_string().as_bytes());
    Ok((pack, sha))
}

fn save_pack(db: &Connection, id: i64, pack: &Pack) -> Result<(), ApiError> {
    db.execute(
        "UPDATE rule_packs SET pack_json = ? WHERE id = ?",
        params![pack.to_json().to_string(), id],
    )?;
    Ok(())
}

/// A pack is readable by a person who can read a project it is assigned to (order or project assignment), and by
/// the person who imported it. Packs are not a global list: anyone else gets 404.
fn pack_readable(db: &Connection, u: &User, pid: i64) -> Result<bool, ApiError> {
    let creator: Option<String> = db
        .query_row(
            "SELECT created_by FROM rule_packs WHERE id = ?",
            params![pid],
            |r| r.get(0),
        )
        .optional()?;
    if creator.as_deref() == Some(u.username.as_str()) {
        return Ok(true);
    }
    let rows: Vec<(String, i64)> = {
        let mut stmt =
            db.prepare("SELECT scope, target_id FROM rule_pack_assignments WHERE pack_id = ?")?;
        stmt.query_map(params![pid], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    for (scope, target) in rows {
        let project: Option<i64> = if scope == "project" {
            Some(target)
        } else {
            db.query_row(
                "SELECT project_id FROM orders WHERE id = ?",
                params![target],
                |r| r.get(0),
            )
            .optional()?
        };
        if let Some(p) = project
            && can_access(db, u, p)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_readable(db: &Connection, u: &User, pid: i64) -> Result<(), ApiError> {
    if pack_readable(db, u, pid)? {
        Ok(())
    } else {
        Err(not_found())
    }
}

/// Summary of the pack an order uses (order assignment first, then the project's), or `null`.
pub(crate) fn pack_summary(db: &Connection, oid: i64) -> Result<Value, ApiError> {
    let Some(pid) = assigned_pack(db, oid)? else {
        return Ok(Value::Null);
    };
    let (p, sha) = load_pack(db, pid)?;
    let status: BTreeMap<&str, &str> = p
        .rules
        .iter()
        .map(|(k, s)| {
            (
                k.as_str(),
                if s.confirmed.is_some() {
                    CONFIRMED
                } else {
                    "UNVERIFIED"
                },
            )
        })
        .collect();
    Ok(json!({
        "id": pid,
        "name": p.name,
        "version": p.version,
        "sha256": sha,
        "complete": p.complete(),
        "rules": status,
    }))
}

fn assigned_pack(db: &Connection, oid: i64) -> Result<Option<i64>, ApiError> {
    let pid: i64 = db
        .query_row(
            "SELECT project_id FROM orders WHERE id = ?",
            params![oid],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(not_found)?;
    let order: Option<i64> = db
        .query_row(
            "SELECT pack_id FROM rule_pack_assignments WHERE scope = 'order' AND target_id = ?",
            params![oid],
            |r| r.get(0),
        )
        .optional()?;
    if order.is_some() {
        return Ok(order);
    }
    Ok(db
        .query_row(
            "SELECT pack_id FROM rule_pack_assignments WHERE scope = 'project' AND target_id = ?",
            params![pid],
            |r| r.get(0),
        )
        .optional()?)
}

/// The pack identity a decision is bound to (id, name, version, hash, every rule's status), or `None` when no pack
/// is assigned, so that orders without a pack keep their fingerprints.
pub(crate) fn pack_identity(db: &Connection, oid: i64) -> Result<Option<String>, ApiError> {
    let Some(pid) = assigned_pack(db, oid)? else {
        return Ok(None);
    };
    let (pack, _) = load_pack(db, pid)?;
    Ok(Some(format!("#{pid} {}", pack.identity())))
}

/// The rules object the engine reads for an order: the assigned pack's changed engine rules, overlaid by the order's
/// own rules object (the order's explicit keys win). With no pack assigned it returns `own` unchanged.
pub(crate) fn pack_overlay(db: &Connection, oid: i64, own: Value) -> Result<Value, ApiError> {
    let Some(pid) = assigned_pack(db, oid)? else {
        return Ok(own);
    };
    let (pack, _) = load_pack(db, pid)?;
    let mut merged = match pack.order_overlay() {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    if let Value::Object(o) = own {
        merged.extend(o);
    }
    // engine-read rules the pack leaves open: the engine holds the order's parts (never a silent default)
    let open = pack.unconfirmed_engine_rules();
    if !open.is_empty() {
        merged.insert(
            nf_core::order_compute::PACK_HOLDS_KEY.to_string(),
            json!(open),
        );
    }
    Ok(Value::Object(merged))
}

async fn list_packs(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let mut stmt = db.prepare(
        "SELECT id, name, version, created_by, created_at FROM rule_packs ORDER BY id DESC",
    )?;
    let all = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                "version": r.get::<_, String>(2)?, "created_by": r.get::<_, String>(3)?,
                "created_at": r.get::<_, i64>(4)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut rows = Vec::new();
    for row in all {
        let pid = row["id"].as_i64().unwrap_or_default();
        if pack_readable(&db, &u, pid)? {
            rows.push(row);
        }
        if rows.len() == 200 {
            break;
        }
    }
    ok(json!({ "packs": rows }))
}

async fn import_pack(State(app): Ctx, headers: HeaderMap, raw: Bytes) -> Api {
    let u = person_writer(&app, &headers)?;
    if raw.len() > MAX_PACK_BYTES {
        return Err(fail(
            StatusCode::PAYLOAD_TOO_LARGE,
            "the rule pack is larger than 64 KiB",
        ));
    }
    let body = request_json(&headers, &raw)?;
    let imported = Pack::from_json(&body).map_err(|m| bad("rule_pack_invalid", &m))?;
    let pack = imported.pack;
    let sha = pack.sha256();
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, &u)?;
    tx.execute(
        "INSERT INTO rule_packs(name, version, pack_json, sha256, created_by, created_at) VALUES (?,?,?,?,?,?)",
        params![pack.name, pack.version, pack.to_json().to_string(), sha, u.username, now_secs()],
    )?;
    let id = tx.last_insert_rowid();
    app.audit(
        &tx,
        &u.username,
        "rule_pack_imported",
        &format!(
            "pack #{id} {} {} {sha} dropped confirmations: {}",
            pack.name,
            pack.version,
            imported.downgraded.join(",")
        ),
    )?;
    tx.commit()?;
    ok(
        json!({ "id": id, "sha256": sha, "downgraded": imported.downgraded, "pack": pack.to_json() }),
    )
}

async fn export_pack(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    ensure_readable(&db, &u, id)?;
    let (pack, sha) = load_pack(&db, id)?;
    let mut out = pack.to_json();
    out["sha256"] = json!(sha);
    out["id"] = json!(id);
    ok(out)
}

async fn assign_pack(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, raw: Bytes) -> Api {
    let u = person_writer(&app, &headers)?;
    let body = request_json(&headers, &raw)?;
    let scope = body.get("scope").and_then(Value::as_str).unwrap_or("");
    let target = body
        .get("target_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| bad("target_missing", "'target_id' is required"))?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, &u)?;
    load_pack(&tx, id)?;
    let project = assignment_project(&tx, scope, target)?;
    project_guard(&app, &tx, &u, project, "assign a rule pack")?;
    tx.execute(
        "INSERT INTO rule_pack_assignments(scope, target_id, pack_id, assigned_by, assigned_at) VALUES (?,?,?,?,?)
         ON CONFLICT(scope, target_id) DO UPDATE SET pack_id = excluded.pack_id, assigned_by = excluded.assigned_by, assigned_at = excluded.assigned_at",
        params![scope, target, id, u.username, now_secs()],
    )?;
    app.audit(
        &tx,
        &u.username,
        "rule_pack_assigned",
        &format!("pack #{id} to {scope} #{target}"),
    )?;
    tx.commit()?;
    ok(json!({ "pack_id": id, "scope": scope, "target_id": target }))
}

/// The project an assignment scope and target belong to (the target is a project or an order).
fn assignment_project(db: &Connection, scope: &str, target: i64) -> Result<i64, ApiError> {
    match scope {
        "project" => {
            if !project_exists(db, target)? {
                return Err(not_found());
            }
            Ok(target)
        }
        "order" => db
            .query_row(
                "SELECT project_id FROM orders WHERE id = ?",
                params![target],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(not_found),
        _ => Err(bad("scope_invalid", "'scope' must be 'project' or 'order'")),
    }
}

/// Remove one assignment of a pack (the project or order it was given to). The change is audited.
async fn unassign_pack(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    raw: Bytes,
) -> Api {
    let u = person_writer(&app, &headers)?;
    let body = request_json(&headers, &raw)?;
    let scope = body.get("scope").and_then(Value::as_str).unwrap_or("");
    let target = body
        .get("target_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| bad("target_missing", "'target_id' is required"))?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, &u)?;
    let project = assignment_project(&tx, scope, target)?;
    project_guard(&app, &tx, &u, project, "unassign a rule pack")?;
    let removed = tx.execute(
        "DELETE FROM rule_pack_assignments WHERE scope = ? AND target_id = ? AND pack_id = ?",
        params![scope, target, id],
    )?;
    if removed == 0 {
        return Err(not_found());
    }
    app.audit(
        &tx,
        &u.username,
        "rule_pack_unassigned",
        &format!("pack #{id} from {scope} #{target}"),
    )?;
    tx.commit()?;
    ok(json!({ "pack_id": id, "scope": scope, "target_id": target, "unassigned": true }))
}

async fn get_interview(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    ensure_readable(&db, &u, id)?;
    let (pack, sha) = load_pack(&db, id)?;
    ok(json!({
        "id": id,
        "name": pack.name,
        "version": pack.version,
        "sha256": sha,
        "complete": pack.complete(),
        "questions": pack.interview(),
    }))
}

/// One answer of the interview. `accept` confirms the current value, `set` confirms a given value, `skip` leaves the
/// rule UNVERIFIED (its parts hold). The confirmation is named by the session's person and dated today.
async fn answer(State(app): Ctx, headers: HeaderMap, Path(id): Path<i64>, raw: Bytes) -> Api {
    let u = person_writer(&app, &headers)?;
    let body = request_json(&headers, &raw)?;
    let key = body
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("key_missing", "'key' is required"))?
        .to_string();
    let action = body.get("action").and_then(Value::as_str).unwrap_or("");
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, &u)?;
    let (mut pack, _) = load_pack(&tx, id)?;
    if rulepack::entry(&key).is_none() {
        return Err(bad(
            "rule_unknown",
            "no rule with that key in the interview",
        ));
    }
    if action == "skip" {
        app.audit(
            &tx,
            &u.username,
            "rule_skipped",
            &format!("pack #{id} rule {key} stays UNVERIFIED"),
        )?;
        tx.commit()?;
        return ok(json!({ "key": key, "status": "UNVERIFIED" }));
    }
    let value = match action {
        "accept" => None,
        "set" => Some(
            body.get("value")
                .cloned()
                .ok_or_else(|| bad("value_missing", "'value' is required for 'set'"))?,
        ),
        _ => {
            return Err(bad(
                "action_invalid",
                "'action' must be accept, set or skip",
            ));
        }
    };
    let on = rulepack::ymd(now_secs());
    let prev = pack
        .rule_state_json(&key)
        .ok_or_else(|| bad("rule_unknown", "no rule with that key in the interview"))?;
    pack.confirm(&key, value, &u.username, &on)
        .map_err(|m| bad("rule_value_invalid", &m))?;
    let after = pack.rule_state_json(&key).unwrap_or(Value::Null);
    save_pack(&tx, id, &pack)?;
    tx.execute(
        "INSERT INTO rule_confirmations(pack_id, rule_key, prev_json, new_json, confirmed_by, confirmed_at) VALUES (?,?,?,?,?,?)",
        params![id, key, prev.to_string(), after.to_string(), u.username, now_secs()],
    )?;
    let cid = tx.last_insert_rowid();
    app.audit(
        &tx,
        &u.username,
        "rule_confirmed",
        &format!(
            "pack #{id} rule {key} = {} confirmed by {} on {on} (confirmation #{cid})",
            after["value"], u.username
        ),
    )?;
    tx.commit()?;
    ok(json!({
        "confirmation_id": cid,
        "key": key,
        "status": CONFIRMED,
        "confirmed_by": u.username,
        "confirmed_on": on,
        "value": after["value"],
        "complete": pack.complete(),
    }))
}

async fn undo_confirmation(
    State(app): Ctx,
    headers: HeaderMap,
    Path((id, cid)): Path<(i64, i64)>,
) -> Api {
    let u = person_writer(&app, &headers)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, &u)?;
    let (mut pack, _) = load_pack(&tx, id)?;
    let (key, prev, after): (String, String, String) = tx
        .query_row(
            "SELECT rule_key, prev_json, new_json FROM rule_confirmations WHERE id = ? AND pack_id = ? AND undone_at IS NULL",
            params![cid, id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or_else(not_found)?;
    // undo is LIFO per rule: only the newest standing confirmation of a rule can be undone, so an older undo never
    // overwrites a newer confirmation, and the rule must still be in the state that confirmation left it in
    let newest: Option<i64> = tx
        .query_row(
            "SELECT id FROM rule_confirmations WHERE pack_id = ? AND rule_key = ? AND undone_at IS NULL ORDER BY id DESC LIMIT 1",
            params![id, key],
            |r| r.get(0),
        )
        .optional()?;
    if newest != Some(cid) {
        return Err(fail_code(
            StatusCode::CONFLICT,
            "undo_out_of_order",
            &[],
            "undo the newer confirmation of this rule first",
        ));
    }
    let after: Value = serde_json::from_str(&after).unwrap_or(Value::Null);
    if pack.rule_state_json(&key).as_ref() != Some(&after) {
        return Err(fail_code(
            StatusCode::CONFLICT,
            "rule_changed",
            &[],
            "the rule changed since this confirmation; undo is refused",
        ));
    }
    let prev: Value = serde_json::from_str(&prev).map_err(|_| not_found())?;
    pack.rule_set(&key, &prev)
        .map_err(|m| bad("rule_value_invalid", &m))?;
    save_pack(&tx, id, &pack)?;
    tx.execute(
        "UPDATE rule_confirmations SET undone_by = ?, undone_at = ? WHERE id = ?",
        params![u.username, now_secs(), cid],
    )?;
    app.audit(
        &tx,
        &u.username,
        "rule_confirmation_undone",
        &format!("pack #{id} rule {key} back to the state before confirmation #{cid}"),
    )?;
    tx.commit()?;
    ok(json!({ "key": key, "status": prev["status"], "value": prev["value"] }))
}
