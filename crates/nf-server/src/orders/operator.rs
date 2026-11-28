//! AIOP-1: the AI operator's event stream, pause and undo.
//!
//! The operator (AMOEBA, running outside this server under an agent account) says what it is doing as events:
//! `{seq, step, target, action, value, reason, confidence, flags}`. The server stores them, assigns `seq`,
//! audits each batch and serves them for the cursor overlay by polling (`GET .../operator/events?after=N`).
//! A person can pause and resume the operator and undo what it typed, last first. The operator cannot resume
//! itself and cannot undo. Nothing here approves or releases anything.
//!
//! What the operator may decide (server side, not on its word): it may set CONFIRMED only by naming a `reading_id`
//! of a reading this server stored (`order_readings`, written when the server's reader read the page) whose stored
//! value is the value, whose stored flags are empty and whose stored confidence is >= AGENT_CONFIRM_MIN_CONFIDENCE;
//! flags, confidence and source the caller claims never grant anything (403 agent_cannot_confirm). /confirm, a
//! decision `confirm` and `accept_flag` are a person's. It does not overwrite a value last edited by a person or in
//! state CONFIRMED/CORRECTED, and does not change, exclude, drop or skip a part or page that holds one
//! (409 person_value_protected). Every write (parts, skip, upload, decision, compute, values) carries at least one
//! event (422 operator_event_required), stored in the same transaction. Flags, confidence and who edited last are
//! stored per value.
//!
//! Pause in flight: every write of the operator re-reads the pause inside the transaction that writes. A write
//! that committed before the pause landed is complete (values and events commit together, so it is replayable and
//! undoable); a write that starts after it is refused (409 operator_paused) and rolled back whole. A pause never
//! interrupts a transaction half way.
//!
//! The schema is open for new kinds of work: `action` is any lower-case word, `target` names exactly one of a
//! drawing page box, a UI element or a CAD canvas point/entity, and `tool` names the registry entry that made
//! the call. A later CAD tool adds a tool name and `cad` targets; the server, the audit and the cursor do not change.

use super::*;

const MAX_EVENTS_PER_BATCH: usize = 100;
const MAX_EVENT_BYTES: usize = 4096;
const MAX_EVENTS_PER_ORDER: i64 = 20000;

/// (value id, current value, value before the write, who last edited it)
type ValueRow = (i64, Option<f64>, Option<f64>, Option<String>);

pub(super) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route(
            "/api/orders/{oid}/operator/events",
            get(list_events).post(post_events),
        )
        .route("/api/orders/{oid}/operator/pause", post(pause))
        .route("/api/orders/{oid}/operator/resume", post(resume))
        .route("/api/orders/{oid}/operator/undo", post(undo))
}

fn short_word(s: &str, max: usize, extra: &str) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.contains(c))
}

fn num_array(v: &Value, n: usize) -> bool {
    v.as_array().is_some_and(|a| {
        a.len() == n
            && a.iter()
                .all(|x| x.as_f64().is_some_and(|f| f.is_finite() && f.abs() < 1.0e9))
    })
}

fn bad(msg: &str) -> ApiError {
    unprocessable("operator_event", msg)
}

/// Check one event and return the stored form (known fields only; `seq` is added by the server).
fn parse_event(v: &Value) -> Result<Value, ApiError> {
    let o = v.as_object().ok_or_else(|| bad("an event is an object"))?;
    let step = o.get("step").and_then(Value::as_str).unwrap_or_default();
    if !short_word(step, 40, "_.-") {
        return Err(bad(
            "'step' must be 1 to 40 lower-case letters, digits, '_', '.' or '-'",
        ));
    }
    let action = o.get("action").and_then(Value::as_str).unwrap_or_default();
    if !short_word(action, 32, "_") || !action.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err(bad(
            "'action' must be a lower-case word such as look, point, click, type, compute or ask",
        ));
    }
    let target = o
        .get("target")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("'target' must be an object"))?;
    let mut kinds = target.keys().map(String::as_str).collect::<Vec<_>>();
    kinds.sort_unstable();
    let t = match kinds.as_slice() {
        ["bbox", "page"] => {
            let p = target["page"].as_i64().filter(|p| *p >= 1);
            if p.is_none() || !num_array(&target["bbox"], 4) {
                return Err(bad(
                    "a page target needs a page number and a box of four numbers",
                ));
            }
            json!({ "page": p, "bbox": target["bbox"] })
        }
        ["page"] => match target["page"].as_i64().filter(|p| *p >= 1) {
            Some(p) => json!({ "page": p }),
            None => return Err(bad("'page' must be a page number")),
        },
        ["ui"] => match target["ui"].as_str() {
            Some(s) if short_word(&s.to_ascii_lowercase(), 64, "_.-#:/") => json!({ "ui": s }),
            _ => return Err(bad("'ui' must be an element id")),
        },
        ["cad"] => {
            let c = target["cad"]
                .as_object()
                .ok_or_else(|| bad("'cad' must be an object"))?;
            let canvas = c.get("canvas");
            let entity = c.get("entity").and_then(Value::as_str);
            if canvas.is_none() && entity.is_none()
                || canvas.is_some_and(|x| !num_array(x, 2))
                || entity.is_some_and(|e| e.is_empty() || e.len() > 64)
            {
                return Err(bad(
                    "a cad target needs a canvas point [x, y] and/or an entity id",
                ));
            }
            json!({ "cad": { "canvas": canvas, "entity": entity } })
        }
        _ => {
            return Err(bad("'target' names exactly one of: page (+ bbox), ui, cad"));
        }
    };
    let value = o.get("value").cloned().unwrap_or(Value::Null);
    if value.to_string().len() > 512 {
        return Err(bad("'value' is too long"));
    }
    let reason = o
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if reason.is_empty() || reason.chars().count() > 300 {
        return Err(unprocessable(
            "decision_reason",
            "every operator event gives its reason (1 to 300 characters)",
        ));
    }
    let confidence = match o.get("confidence") {
        None | Some(Value::Null) => Value::Null,
        Some(x) => match x.as_f64().filter(|f| (0.0..=1.0).contains(f)) {
            Some(f) => json!(f),
            None => return Err(bad("'confidence' is a number from 0 to 1, or null")),
        },
    };
    let flags = match o.get("flags") {
        None | Some(Value::Null) => json!([]),
        Some(Value::Array(a))
            if a.len() <= 8
                && a.iter()
                    .all(|x| x.as_str().is_some_and(|s| short_word(s, 32, "_"))) =>
        {
            Value::Array(a.clone())
        }
        Some(_) => return Err(bad("'flags' is a short list of flag names")),
    };
    let tool = match o.get("tool") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::String(s)) if short_word(s, 48, "_.") => json!(s),
        Some(_) => return Err(bad("'tool' is a registry name")),
    };
    let reason_key = match o.get("reason_key") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::String(s)) if short_word(s, 64, "_.") => json!(s),
        Some(_) => return Err(bad("'reason_key' is a translation key")),
    };
    let reason_args = match o.get("reason_args") {
        None | Some(Value::Null) => json!({}),
        Some(Value::Object(a))
            if a.len() <= 8 && a.values().all(|v| v.as_str().is_some() || v.is_number()) =>
        {
            Value::Object(a.clone())
        }
        Some(_) => return Err(bad("'reason_args' is a small object of words and numbers")),
    };
    let write_ref = match o.get("write_ref") {
        None | Some(Value::Null) => Value::Null,
        Some(w) => {
            let part = w.get("part").and_then(Value::as_str).unwrap_or_default();
            let key = w.get("key").and_then(Value::as_str).unwrap_or_default();
            if !plain_id(part, 40) || !valid_key(key) {
                return Err(bad("'write_ref' names a part 'no' and a value key"));
            }
            json!({ "part": part, "key": key })
        }
    };
    let ev = json!({ "step": step, "action": action, "target": t, "value": value, "reason": reason,
                     "reason_key": reason_key, "reason_args": reason_args,
                     "confidence": confidence, "flags": flags, "tool": tool, "write_ref": write_ref });
    if ev.to_string().len() > MAX_EVENT_BYTES {
        return Err(bad("the event is too large"));
    }
    Ok(ev)
}

pub(super) fn is_paused(db: &Connection, oid: i64) -> rusqlite::Result<bool> {
    Ok(db
        .query_row(
            "SELECT paused FROM operator_state WHERE order_id = ?",
            params![oid],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0)
        != 0)
}

fn person_only(u: &User, what: &str) -> Result<(), ApiError> {
    if u.kind == "agent" {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "operator_person_only",
            &format!("the AI operator cannot {what}; a person does"),
        ));
    }
    Ok(())
}

async fn post_events(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "operator events")?;
    let res = post_events_do(&app, &headers, &u, oid, &raw);
    settle(&app, &u, oid, "operator events", String::new(), res).await
}

/// Parse an event list shared by the event route and the atomic value write.
pub(super) fn parse_event_list(list: &[Value]) -> Result<Vec<Value>, ApiError> {
    if list.is_empty() || list.len() > MAX_EVENTS_PER_BATCH {
        return Err(unprocessable(
            "operator_event",
            &format!("send 1 to {MAX_EVENTS_PER_BATCH} events per request"),
        ));
    }
    list.iter().map(parse_event).collect()
}

/// Store a checked event batch inside the caller's transaction: values and their events commit together.
pub(super) fn store_events(
    app: &App,
    tx: &Connection,
    u: &User,
    oid: i64,
    events: &[Value],
) -> Result<(i64, i64), ApiError> {
    if is_paused(tx, oid)? {
        return Err(refuse(
            StatusCode::CONFLICT,
            "operator_paused",
            "a person paused the operator on this order",
        ));
    }
    let last: i64 = tx.query_row(
        "SELECT COALESCE(MAX(seq), 0) FROM operator_events WHERE order_id = ?",
        params![oid],
        |r| r.get(0),
    )?;
    if last + events.len() as i64 > MAX_EVENTS_PER_ORDER {
        return Err(refuse(
            StatusCode::CONFLICT,
            "operator_events_limit",
            "this order holds its limit of operator events",
        ));
    }
    let mut seq = last;
    for e in events {
        seq += 1;
        tx.execute(
            "INSERT INTO operator_events(order_id, seq, event_json, created_by, created_at) VALUES (?,?,?,?,?)",
            params![oid, seq, e.to_string(), u.username, now() as i64],
        )?;
    }
    app.audit(
        tx,
        &u.username,
        "operator_events",
        &format!(
            "order #{oid} events {}..{} ({}); first: {} {}",
            last + 1,
            seq,
            events.len(),
            events[0]["action"].as_str().unwrap_or_default(),
            clip(events[0]["reason"].as_str().unwrap_or_default(), 160)
        ),
    )?;
    Ok((last + 1, seq))
}

fn post_events_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, raw: &[u8]) -> Api {
    if u.kind != "agent" {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "operator_agent_only",
            "operator events are written by the AI operator's own account",
        ));
    }
    let body = request_json(headers, raw)?;
    let list = body
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| unprocessable("operator_event", "the body needs an 'events' list"))?;
    let events = parse_event_list(list)?;
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "operator events")?;
    let (first_seq, last_seq) = store_events(app, &tx, u, oid, &events)?;
    tx.commit()?;
    ok(json!({ "stored": events.len(), "first_seq": first_seq, "last_seq": last_seq }))
}

async fn list_events(
    State(app): Ctx,
    headers: HeaderMap,
    Path(oid): Path<i64>,
    Query(q): Query<BTreeMap<String, String>>,
) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    guard_order(&app, &db, &u, oid, "view operator events")?;
    let after: i64 = q
        .get("after")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
        .max(0);
    let limit: i64 = q
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(200)
        .clamp(1, 500);
    let rows = q_all(
        &db,
        "SELECT seq, event_json, created_by, created_at, undone_at IS NOT NULL FROM operator_events WHERE order_id = ? AND seq > ? ORDER BY seq LIMIT ?",
        params![oid, after, limit],
        |r| {
            let mut e: Value = serde_json::from_str(&r.get::<_, String>(1)?).unwrap_or(Value::Null);
            e["seq"] = json!(r.get::<_, i64>(0)?);
            e["by"] = json!(r.get::<_, String>(2)?);
            e["at"] = json!(r.get::<_, i64>(3)?);
            e["undone"] = json!(r.get::<_, bool>(4)?);
            Ok(e)
        },
    )?;
    let last: i64 = db.query_row(
        "SELECT COALESCE(MAX(seq), 0) FROM operator_events WHERE order_id = ?",
        params![oid],
        |r| r.get(0),
    )?;
    ok(json!({ "events": rows, "last_seq": last, "paused": is_paused(&db, oid)? }))
}

async fn pause(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    set_paused(app, headers, oid, raw, true).await
}

async fn resume(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    set_paused(app, headers, oid, raw, false).await
}

async fn set_paused(app: Arc<App>, headers: HeaderMap, oid: i64, raw: Bytes, paused: bool) -> Api {
    let u = write_access(&app, &headers, oid, "pause operator")?;
    let what = if paused { "pause" } else { "resume" };
    let res = (|| -> Api {
        person_only(&u, what)?;
        let mut db = app.db();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        still_writer(&tx, &u)?;
        guard_order_tx(&app, &tx, &u, oid, "pause operator")?;
        tx.execute(
            "INSERT INTO operator_state(order_id, paused, by_user, at) VALUES (?,?,?,?) ON CONFLICT(order_id) DO UPDATE SET paused = excluded.paused, by_user = excluded.by_user, at = excluded.at",
            params![oid, paused as i64, u.username, now() as i64],
        )?;
        let _ = &raw;
        app.audit(
            &tx,
            &u.username,
            if paused {
                "operator_pause"
            } else {
                "operator_resume"
            },
            &format!("order #{oid} operator {what}d"),
        )?;
        tx.commit()?;
        ok(json!({ "paused": paused }))
    })();
    settle(&app, &u, oid, "operator pause", String::new(), res).await
}

async fn undo(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>, raw: Bytes) -> Api {
    let u = write_access(&app, &headers, oid, "undo operator")?;
    let res = undo_do(&app, &headers, &u, oid, &raw);
    settle(&app, &u, oid, "operator undo", String::new(), res).await
}

/// Undo what the operator typed, latest first: the value goes back to what it was, a confirmation the operator
/// made for it is marked undone, and the event is marked undone. Without `to_seq` one write is undone.
fn undo_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, raw: &[u8]) -> Api {
    person_only(u, "undo its own work")?;
    let body = if raw.is_empty() {
        json!({})
    } else {
        request_json(headers, raw)?
    };
    let to_seq = body.get("to_seq").and_then(Value::as_i64);
    let note = text_field(&body, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "undo operator")?;
    let writes = q_all(
        &tx,
        "SELECT seq, event_json, created_by FROM operator_events WHERE order_id = ? AND undone_at IS NULL AND json_extract(event_json, '$.write_ref') IS NOT NULL AND seq >= ? ORDER BY seq DESC",
        params![oid, to_seq.unwrap_or(0)],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )?;
    let take = if to_seq.is_some() {
        writes.len()
    } else {
        writes.len().min(1)
    };
    if take == 0 {
        return Err(refuse(
            StatusCode::CONFLICT,
            "nothing_to_undo",
            "the operator has no value left to undo",
        ));
    }
    let mut undone = Vec::new();
    for (seq, ev, by) in writes.into_iter().take(take) {
        let e: Value = serde_json::from_str(&ev).unwrap_or(Value::Null);
        let (part, key) = (
            e["write_ref"]["part"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            e["write_ref"]["key"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        );
        let (pid, _, _) = find_part(&tx, oid, &json!(part))?;
        let row: Option<ValueRow> = tx
            .query_row(
                "SELECT id, value, was, edited_by FROM order_values WHERE part_id = ? AND key = ?",
                params![pid, key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((vid, val, was, edited_by)) = row else {
            return Err(refuse(
                StatusCode::CONFLICT,
                "value_changed_since",
                "the value is gone",
            ));
        };
        let wrote = e["value"].as_f64();
        if edited_by.as_deref() != Some(by.as_str()) || wrote.is_none() || val != wrote {
            return Err(refuse(
                StatusCode::CONFLICT,
                "value_changed_since",
                &format!(
                    "'{key}' of part '{part}' was changed after the operator typed it; undo is not applied"
                ),
            ));
        }
        let state = if was.is_some() { "PROPOSED" } else { "MISSING" };
        tx.execute(
            "UPDATE order_values SET value = ?, state = ?, was = NULL, edited_by = ?, edited_at = ?, edited_kind = ? WHERE id = ?",
            params![was, state, u.username, now() as i64, u.kind, vid],
        )?;
        tx.execute(
            "UPDATE order_decisions SET undone_by = ?, undone_at = ?, undo_note = ? WHERE value_id = ? AND undone_at IS NULL AND kind = 'confirm'",
            params![u.username, now() as i64, format!("operator undo: {}", clip(&note, 200)), vid],
        )?;
        tx.execute(
            "UPDATE operator_events SET undone_by = ?, undone_at = ? WHERE order_id = ? AND seq = ?",
            params![u.username, now() as i64, oid, seq],
        )?;
        recompute_part_state(&tx, pid)?;
        app.audit(
            &tx,
            &u.username,
            "operator_undo",
            &format!(
                "order #{oid} event {seq}: '{key}' of part '{part}' back to {} (operator had written {}) {}",
                fmt_val(was),
                fmt_val(wrote),
                clip(&note, 200)
            ),
        )?;
        undone.push(seq);
    }
    tx.commit()?;
    ok(json!({ "undone": undone }))
}
