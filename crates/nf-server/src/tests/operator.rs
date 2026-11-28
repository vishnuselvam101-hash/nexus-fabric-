//! AIOP-1: the operator event stream, pause and undo, driven through the real router.

use super::orders::{
    add_agent, add_reading, add_reading_on_page, agent_clean_read, order_with_parts, png, raw_with,
    with_ev,
};
use super::*;

fn ev(step: &str, action: &str, target: Value, reason: &str) -> Value {
    json!({ "step": step, "action": action, "target": target, "value": null, "reason": reason, "confidence": 0.97, "flags": [] })
}

fn page_target() -> Value {
    json!({ "page": 1, "bbox": [10.0, 20.0, 80.0, 40.0] })
}

async fn post_ev(t: &T, oid: i64, cookie: &str, events: Vec<Value>) -> (StatusCode, Value) {
    let (st, _, b) = t
        .send(
            "POST",
            &format!("/api/orders/{oid}/operator/events"),
            Some(json!({ "events": events })),
            &[("cookie", cookie)],
        )
        .await;
    (st, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn person(t: &T, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    t.call(method, path, Some(body)).await
}

/// Propose a value from a stored reading with a tool-write event, then confirm it with the same reading.
/// Returns the two statuses and the first response's code (null on success). The event's caller value is an
/// object on purpose: the server must store the number it wrote so undo can match it.
async fn agent_typed_confirm(
    t: &T,
    oid: i64,
    ac: &str,
    key: &str,
    v: f64,
    text: &str,
) -> (StatusCode, StatusCode, Value) {
    let vals = format!("/api/orders/{oid}/values");
    let rid = add_reading(t, oid, text, v, &[], 0.97);
    let mut e = ev(
        "type-x",
        "type",
        json!({ "ui": format!("part-1-{key}") }),
        "typed from the page",
    );
    e["value"] = json!({ "typed": text });
    e["write_ref"] = json!({ "part": "1", "key": key });
    let value = json!({ "part": "1", "key": key, "value": v, "text": text, "source": "read",
        "flags": [], "confidence": 0.97, "note": "matches the list" });
    let mut propose = value.clone();
    propose["confirm"] = json!(false);
    propose["reading_id"] = json!(rid);
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(json!({ "values": [propose], "events": [e] })),
            &[("cookie", ac)],
        )
        .await;
    let code = serde_json::from_slice::<Value>(&b).unwrap_or(Value::Null)["code"].clone();
    if st != StatusCode::OK {
        return (st, st, code);
    }
    let mut confirm = value;
    confirm["confirm"] = json!(true);
    confirm["reading_id"] = json!(rid);
    let mut ce = ev(
        "confirm",
        "confirm",
        json!({ "ui": format!("part-1-{key}") }),
        "typed from the page",
    );
    ce["write_ref"] = json!({ "part": "1", "key": key });
    let mut confirm_body = with_ev(json!({ "values": [confirm] }));
    confirm_body["events"] = json!([ce]);
    let (st, _, _) = t
        .send("PUT", &vals, Some(confirm_body), &[("cookie", ac)])
        .await;
    (StatusCode::OK, st, code)
}

#[tokio::test]
async fn aiop_only_a_value_write_may_carry_a_write_ref() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let mut e = ev("look", "look", page_target(), "looking at the page");
    e["write_ref"] = json!({ "part": "1", "key": "D" });

    // neither the event route nor a page read may append a replayable value write
    let (st, body) = post_ev(&t, oid, &agent, vec![e.clone()]).await;
    assert_eq!(
        (st, body["code"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("operator_event"))
    );
    let (st, _, body) = t
        .send(
            "POST",
            &format!("/api/orders/{oid}/pages/1/read"),
            Some(json!({ "events": [e] })),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code(&body)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("operator_event"))
    );

    // a value write refuses a ref naming a value that is not in the batch, and stores nothing
    let mut bad = ev(
        "type",
        "type",
        json!({ "ui": "part-1-D" }),
        "typed from the page",
    );
    bad["value"] = json!(216.3);
    bad["write_ref"] = json!({ "part": "1", "key": "t" });
    let body = json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read", "flags": [], "confidence": 0.97, "confirm": false, "note": "n" }], "events": [bad] });
    let (st, _, body) = t
        .send("PUT", &format!("/api/orders/{oid}/values"), Some(body), &ac)
        .await;
    assert_eq!(
        (st, code(&body)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("operator_event"))
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM operator_events"), 0);

    // a valid ref stores the number the server wrote, not the caller's object
    let rid = add_reading(&t, oid, "216.3", 216.3, &[], 0.97);
    let mut good = ev(
        "type",
        "type",
        json!({ "ui": "part-1-D" }),
        "typed from the page",
    );
    good["value"] = json!({ "caller": "claim" });
    good["write_ref"] = json!({ "part": 1, "key": "D" });
    let body = json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read", "flags": [], "confidence": 0.97, "confirm": false, "reading_id": rid, "note": "n" }], "events": [good] });
    let (st, _, body) = t
        .send("PUT", &format!("/api/orders/{oid}/values"), Some(body), &ac)
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let el = t.get(&format!("/api/orders/{oid}/operator/events")).await;
    assert_eq!(el["events"][0]["value"], json!(216.3));
    assert_eq!(el["events"][0]["write_ref"]["part"], json!(1));
}

#[tokio::test]
async fn aiop_a_numeric_write_ref_is_undoable() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    let rid = add_reading(&t, oid, "216.3", 216.3, &[], 0.97);
    let mut e = ev(
        "type",
        "type",
        json!({ "ui": "part-1-D" }),
        "typed from the page",
    );
    e["value"] = json!({ "caller": "claim" });
    e["write_ref"] = json!({ "part": 1, "key": "D" });
    let propose = json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read", "flags": [], "confidence": 0.97, "confirm": false, "reading_id": rid, "note": "n" }], "events": [e] });
    let (st, _, body) = t.send("PUT", &vals, Some(propose), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let confirm = with_ev(
        json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read", "flags": [], "confidence": 0.97, "confirm": true, "reading_id": rid, "note": "n" }] }),
    );
    let (st, _, body) = t.send("PUT", &vals, Some(confirm), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let (st, body) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/operator/undo"),
            Some(json!({ "note": "n" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM operator_events WHERE undone_at IS NOT NULL"),
        1
    );
}

#[tokio::test]
async fn aiop_a_confirmation_cannot_use_a_reading_of_another_part() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    let part_1_flagged = add_reading_on_page(&t, oid, 1, "216.3", 216.3, &["ambiguous"], 0.70);
    let part_2_clean = add_reading_on_page(&t, oid, 1, "216.3", 216.3, &[], 0.97);

    // a direct confirmation from another part's clean reading has no server-owned attachment
    let (st, _, body) = t
        .send(
            "PUT",
            &vals,
            Some(confirm_body(
                "D",
                216.3,
                Some(part_2_clean),
                json!([]),
                0.99,
            )),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code(&body)),
        (StatusCode::FORBIDDEN, json!("agent_cannot_confirm"))
    );

    // a proposal fixes the reading; another part's reading cannot take it over later
    let mut propose = put_body("D", 216.3, "read", json!([]), 0.70, false);
    propose["values"][0]["reading_id"] = json!(part_1_flagged);
    let (st, _, body) = t.send("PUT", &vals, Some(propose), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let db = t.app.db();
    let attached: Option<i64> = db
        .query_row(
            "SELECT attached_reading_id FROM order_values WHERE key = 'D'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(attached, Some(part_1_flagged));
    drop(db);
    for (name, rid) in [
        ("another part's clean reading", part_2_clean),
        ("the flagged reading it is attached to", part_1_flagged),
    ] {
        let (st, _, body) = t
            .send(
                "PUT",
                &vals,
                Some(confirm_body("D", 216.3, Some(rid), json!([]), 0.99)),
                &ac,
            )
            .await;
        assert_eq!(
            (st, code(&body)),
            (StatusCode::FORBIDDEN, json!("agent_cannot_confirm")),
            "{name}"
        );
    }
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values WHERE key = 'D' AND state = 'PROPOSED'"),
        1
    );
}

#[tokio::test]
async fn aiop_events_are_stored_in_order_audited_and_polled_by_seq() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let (st, v) = post_ev(
        &t,
        oid,
        &agent,
        vec![
            ev("read-1", "look", page_target(), "reading page 1"),
            ev(
                "slot-1",
                "point",
                json!({ "ui": "part-1-D" }),
                "the diameter slot",
            ),
            // a CAD target and a new action word need no server change
            ev(
                "cad-1",
                "measure_entity",
                json!({ "cad": { "canvas": [12.5, 40.0], "entity": "e17" } }),
                "measure",
            ),
        ],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["first_seq"].clone(), v["last_seq"].clone()),
        (json!(1), json!(3))
    );
    let (_, v2) = post_ev(
        &t,
        oid,
        &agent,
        vec![ev(
            "c",
            "compute",
            json!({ "ui": "compute-btn" }),
            "compute",
        )],
    )
    .await;
    assert_eq!(v2["first_seq"], 4);
    let all = t.get(&format!("/api/orders/{oid}/operator/events")).await;
    assert_eq!(all["events"].as_array().unwrap().len(), 4);
    assert_eq!(all["last_seq"], 4);
    assert_eq!(all["paused"], false);
    assert_eq!(all["events"][0]["by"], "operator-ai");
    let tail = t
        .get(&format!("/api/orders/{oid}/operator/events?after=3"))
        .await;
    assert_eq!(tail["events"].as_array().unwrap().len(), 1);
    assert_eq!(tail["events"][0]["seq"], 4);
    // events are kept: the table refuses a delete and an edit of a written event
    assert!(
        t.app
            .db()
            .execute("DELETE FROM operator_events", [])
            .is_err()
    );
    assert!(
        t.app
            .db()
            .execute("UPDATE operator_events SET event_json = '{}'", [])
            .is_err()
    );
    let a = audit_details("operator_events", &t);
    assert_eq!(a.len(), 2, "{a:?}");
    assert!(
        a[0].contains("events 1..3") && a[0].contains("reading page 1"),
        "{a:?}"
    );
}

fn audit_details(action: &str, t: &T) -> Vec<String> {
    super::orders::audit_details(t, action)
}

#[tokio::test]
async fn aiop_event_checks_refuse_what_the_stream_cannot_carry() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let code = |v: &Value| v["code"].clone();
    // a person cannot write operator events
    let (st, v) = post_ev(
        &t,
        oid,
        &t.cookie.clone(),
        vec![ev("a", "look", page_target(), "x")],
    )
    .await;
    assert_eq!(
        (st, code(&v)),
        (StatusCode::FORBIDDEN, json!("operator_agent_only"))
    );
    // no reason
    let (st, v) = post_ev(&t, oid, &agent, vec![ev("a", "look", page_target(), "  ")]).await;
    assert_eq!(
        (st, code(&v)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason"))
    );
    // two kinds of target, a bad action word, a bad confidence, an empty batch
    for e in [
        ev("a", "look", json!({ "ui": "x", "page": 1 }), "r"),
        ev("a", "Look!", page_target(), "r"),
        ev("a", "look", json!({ "cad": {} }), "r"),
        ev("a", "look", json!({ "page": 0, "bbox": [1, 2, 3, 4] }), "r"),
        json!({ "step": "a", "action": "look", "target": { "ui": "x" }, "reason": "r", "confidence": 1.5 }),
    ] {
        let (st, v) = post_ev(&t, oid, &agent, vec![e]).await;
        assert_eq!(
            (st, code(&v)),
            (StatusCode::UNPROCESSABLE_ENTITY, json!("operator_event")),
            "{v}"
        );
    }
    let (st, _) = post_ev(&t, oid, &agent, vec![]).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    // nothing was stored by any of those
    assert_eq!(t.count("SELECT COUNT(*) FROM operator_events"), 0);
    // an agent outside the order's project is told 404
    let c = add_session(&t.app, "outsider-ai", "engineer");
    t.sql("UPDATE users SET kind = 'agent' WHERE username = 'outsider-ai'");
    t.sql(
        "INSERT INTO projects(name, description, created_by, created_at) VALUES ('q','','admin',0)",
    );
    let (st, _) = post_ev(&t, oid, &c, vec![ev("a", "look", page_target(), "r")]).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn aiop_pause_blocks_the_operator_and_only_a_person_resumes() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let base = format!("/api/orders/{oid}/operator");
    // the operator cannot pause or resume itself
    for p in ["pause", "resume"] {
        let (st, _, _) = t
            .send("POST", &format!("{base}/{p}"), Some(json!({})), &ac)
            .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{p}");
    }
    let (st, v) = person(&t, "POST", &format!("{base}/pause"), json!({})).await;
    assert_eq!((st, v["paused"].clone()), (StatusCode::OK, json!(true)));
    assert_eq!(t.get(&base_events(&base)).await["paused"], true);
    let (st, v) = post_ev(&t, oid, &agent, vec![ev("a", "look", page_target(), "r")]).await;
    assert_eq!(
        (st, v["code"].clone()),
        (StatusCode::CONFLICT, json!("operator_paused"))
    );
    // its order writes wait too
    let (st, _, b) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "note": "n" }] })),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{}", String::from_utf8_lossy(&b));
    // a person can still edit
    let (st, _) = person(
        &t,
        "PUT",
        &format!("/api/orders/{oid}/values"),
        json!({ "values": [{ "part": "1", "key": "t", "value": 8.0, "text": "8", "source": "entered" }] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = person(&t, "POST", &format!("{base}/resume"), json!({})).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = post_ev(&t, oid, &agent, vec![ev("a", "look", page_target(), "r")]).await;
    assert_eq!(st, StatusCode::OK);
    let pa = audit_details("operator_pause", &t);
    let ra = audit_details("operator_resume", &t);
    assert_eq!((pa.len(), ra.len()), (1, 1));
    // the refused operator writes are on record
    assert!(
        audit_details("order_refused", &t)
            .iter()
            .any(|d| d.contains("operator_paused"))
    );
}

fn base_events(base: &str) -> String {
    format!("{base}/events")
}

#[tokio::test]
async fn aiop_values_and_their_type_events_commit_together() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    let mut typed = ev(
        "type-d",
        "type",
        json!({ "ui": "part-1-D" }),
        "D from the page",
    );
    typed["value"] = json!(216.3);
    typed["write_ref"] = json!({ "part": "1", "key": "D" });
    let rid = add_reading(&t, oid, "φ216.3", 216.3, &[], 0.97);
    let propose = with_ev(json!({
        "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "flags": [], "confidence": 0.97, "confirm": false, "reading_id": rid, "note": "matches the list" }],
    }));
    let (st, _, b) = t.send("PUT", &vals, Some(propose), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let body = json!({
        "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "flags": [], "confidence": 0.97, "confirm": true, "reading_id": rid, "note": "matches the list" }],
        "events": [typed],
    });
    let (st, _, b) = t.send("PUT", &vals, Some(body.clone()), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let val = o["parts"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == "D")
        .unwrap();
    assert_eq!(val["state"], json!("CONFIRMED"));
    let el = t.get(&format!("/api/orders/{oid}/operator/events")).await;
    assert_eq!(el["events"].as_array().unwrap().len(), 2);
    // the proposal and the confirmation each carry their own event and write_ref; the latter stores the number
    assert_eq!(el["events"][1]["seq"], 2);
    assert_eq!(el["events"][1]["write_ref"]["key"], "D");
    assert_eq!(el["events"][1]["value"], json!(216.3));
    assert_eq!(el["events"][1]["reason_key"], Value::Null);

    // A pause between the value and its event is impossible: the whole agent write is refused.
    let base = format!("/api/orders/{oid}/operator");
    assert_eq!(
        person(&t, "POST", &format!("{base}/pause"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    let (st, _, _) = t.send("PUT", &vals, Some(body), &ac).await;
    assert_eq!(st, StatusCode::CONFLICT);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["parts"][0]["values"].as_array().unwrap().len(), 1);
    let el = t.get(&format!("/api/orders/{oid}/operator/events")).await;
    assert_eq!(el["events"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn aiop_undo_puts_a_typed_value_back_and_refuses_when_it_was_changed() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let base = format!("/api/orders/{oid}/operator");
    // the value, its server-owned reading attachment and the tool-write event commit together
    let (propose, confirm, _) = agent_typed_confirm(&t, oid, &agent, "D", 216.3, "φ216.3").await;
    assert_eq!((propose, confirm), (StatusCode::OK, StatusCode::OK));
    let el = t.get(&format!("{base}/events")).await;
    assert_eq!(
        el["events"][0]["value"],
        json!(216.3),
        "undo matches the number the server wrote, not the caller's object"
    );
    // the operator cannot undo
    let (st, _, _) = t
        .send("POST", &format!("{base}/undo"), Some(json!({})), &ac)
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, v) = person(
        &t,
        "POST",
        &format!("{base}/undo"),
        json!({ "note": "wrong one" }),
    )
    .await;
    assert_eq!((st, v["undone"].clone()), (StatusCode::OK, json!([2])));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let val = &o["parts"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == "D")
        .unwrap()
        .clone();
    assert_eq!(
        (val["state"].clone(), val["value"].clone()),
        (json!("MISSING"), Value::Null),
        "{val}"
    );
    assert!(
        o["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| !d["undone_at"].is_null()),
        "{}",
        o["decisions"]
    );
    let ev_list = t.get(&format!("{base}/events")).await;
    assert_eq!(ev_list["events"][0]["undone"], false);
    assert_eq!(ev_list["events"][1]["undone"], true);
    // nothing left to undo
    let (st, v) = person(&t, "POST", &format!("{base}/undo"), json!({})).await;
    assert_eq!(
        (st, v["code"].clone()),
        (StatusCode::CONFLICT, json!("value_changed_since")),
        "the earlier proposal is no longer the operator's value to undo"
    );
    // after a person's undo the value is the person's: the operator does not type over it again
    let (propose, _, code) = agent_typed_confirm(&t, oid, &agent, "D", 100.0, "100").await;
    assert_eq!(
        (propose, code),
        (StatusCode::CONFLICT, json!("person_value_protected"))
    );
    // a value a person changed after the operator typed it is not rolled back
    let (propose, confirm, _) = agent_typed_confirm(&t, oid, &agent, "t", 100.0, "100").await;
    assert_eq!((propose, confirm), (StatusCode::OK, StatusCode::OK));
    let (st, _) = person(
        &t,
        "PUT",
        &format!("/api/orders/{oid}/values"),
        json!({ "values": [{ "part": "1", "key": "t", "value": 101.0, "text": "101", "source": "entered" }] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, v) = person(&t, "POST", &format!("{base}/undo"), json!({})).await;
    assert_eq!(
        (st, v["code"].clone()),
        (StatusCode::CONFLICT, json!("value_changed_since"))
    );
    assert_eq!(audit_details("operator_undo", &t).len(), 1);
}

#[tokio::test]
async fn aiop_operator_routes_are_behind_the_licence_gate() {
    use axum::http::Method;
    for p in ["events", "undo"] {
        assert!(
            crate::licence::gated(&Method::POST, &format!("/api/orders/1/operator/{p}")),
            "{p}"
        );
    }
    assert!(!crate::licence::gated(
        &Method::GET,
        "/api/orders/1/operator/events"
    ));
}

// ---------------------------------------------------------------- round 1: what the operator may and may not decide

fn code(b: &[u8]) -> Value {
    serde_json::from_slice::<Value>(b).unwrap()["code"].clone()
}

fn d_value(o: &Value, key: &str) -> Value {
    o["parts"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == key)
        .cloned()
        .unwrap_or(Value::Null)
}

fn put_body(key: &str, v: f64, source: &str, flags: Value, conf: f64, confirm: bool) -> Value {
    with_ev(
        json!({ "values": [{ "part": "1", "key": key, "value": v, "text": format!("{v}"), "source": source,
        "flags": flags, "confidence": conf, "confirm": confirm, "note": "n" }] }),
    )
}

#[tokio::test]
async fn aiop_the_operator_cannot_confirm_by_confirm_all_or_by_decision() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    // a flagged reading is stored proposed, with its flags and confidence kept on the server
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(put_body(
                "D",
                216.3,
                "read",
                json!(["repaired"]),
                0.6,
                false,
            )),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values WHERE key = 'D' AND flags_json = '[\"repaired\"]' AND confidence = 0.6 AND edited_kind = 'agent' AND state = 'PROPOSED'"),
        1
    );
    for (path, body) in [
        (
            format!("/api/orders/{oid}/confirm"),
            json!({ "all": true, "note": "n" }),
        ),
        (
            format!("/api/orders/{oid}/confirm"),
            json!({ "part_ids": [1], "note": "n" }),
        ),
        (
            format!("/api/orders/{oid}/decisions"),
            with_ev(json!({ "kind": "confirm", "part": "1", "key": "D", "reason": "r" })),
        ),
        (
            format!("/api/orders/{oid}/decisions"),
            with_ev(
                json!({ "kind": "accept_flag", "part": "1", "key": "D", "flag": "repaired", "reason": "r" }),
            ),
        ),
    ] {
        let (st, _, b) = t.send("POST", &path, Some(body.clone()), &ac).await;
        assert_eq!(
            (st, code(&b)),
            (StatusCode::FORBIDDEN, json!("agent_cannot_confirm")),
            "{path} {body}"
        );
    }
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(d_value(&o, "D")["state"], "PROPOSED");
    assert_eq!(t.count("SELECT COUNT(*) FROM order_decisions"), 0);
    // the refusals are on record
    assert!(
        audit_details("order_refused", &t)
            .iter()
            .filter(|d| d.contains("agent_cannot_confirm"))
            .count()
            >= 4
    );
    // a person confirms the same value
    let (st, v) = person(
        &t,
        "POST",
        &format!("/api/orders/{oid}/confirm"),
        json!({ "all": true }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(d_value(&o, "D")["state"], "CONFIRMED");
}

/// A confirming write of the operator naming a stored reading (or none).
fn confirm_body(key: &str, v: f64, reading: Option<i64>, flags: Value, conf: f64) -> Value {
    let mut b = put_body(key, v, "read", flags, conf, true);
    if let Some(r) = reading {
        b["values"][0]["reading_id"] = json!(r);
    }
    b
}

#[tokio::test]
async fn aiop_the_operator_confirms_only_against_a_clean_server_side_reading() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    let clean = add_reading(&t, oid, "216.3", 216.3, &[], 0.97);
    let at_threshold = add_reading(&t, oid, "216.3", 216.3, &[], 0.85);
    let flagged = add_reading(&t, oid, "216.3", 216.3, &["ambiguous"], 0.99);
    let weak = add_reading(&t, oid, "216.3", 216.3, &[], 0.84);
    let other_value = add_reading(&t, oid, "100", 100.0, &[], 0.99);
    let no_value = {
        let db = t.app.db();
        db.execute("INSERT INTO order_readings(order_id, page_no, bbox_json, text, value, kind, flags_json, confidence, reader_id, model_sha256, created_by, created_at) VALUES (?,1,'[1.0,1.0,2.0,2.0]','1/1',NULL,'number','[]',0.99,'r','0','t',0)", params![oid]).unwrap();
        db.last_insert_rowid()
    };
    let other_order = {
        let o2 = order_with_parts(&t).await;
        add_reading(&t, o2, "216.3", 216.3, &[], 0.99)
    };
    // a proposal fixes the server-owned attachment: no later confirmation can create or switch it
    let mut propose = put_body("D", 216.3, "read", json!([]), 0.97, false);
    propose["values"][0]["reading_id"] = json!(clean);
    let (st, _, b) = t.send("PUT", &vals, Some(propose), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let forbidden = StatusCode::FORBIDDEN;
    let refused = StatusCode::UNPROCESSABLE_ENTITY;
    for (name, body, want, status) in [
        // the agent must name the server-stored reading; claimed flags/confidence do not grant authority
        (
            "claimed 0.99, no reading_id",
            confirm_body("D", 216.3, None, json!([]), 0.99),
            "agent_cannot_confirm",
            forbidden,
        ),
        (
            "reading_id of another value",
            confirm_body("D", 216.3, Some(other_value), json!([]), 0.99),
            "agent_cannot_confirm",
            forbidden,
        ),
        (
            "stored flagged reading",
            confirm_body("D", 216.3, Some(flagged), json!([]), 0.99),
            "agent_cannot_confirm",
            forbidden,
        ),
        (
            "stored low confidence",
            confirm_body("D", 216.3, Some(weak), json!([]), 0.99),
            "agent_cannot_confirm",
            forbidden,
        ),
        (
            "reading without a number",
            confirm_body("D", 216.3, Some(no_value), json!([]), 0.99),
            "agent_cannot_confirm",
            forbidden,
        ),
        (
            "reading of another order",
            confirm_body("D", 216.3, Some(other_order), json!([]), 0.99),
            "agent_cannot_confirm",
            forbidden,
        ),
        (
            "unknown reading",
            confirm_body("D", 216.3, Some(9999), json!([]), 0.99),
            "agent_cannot_confirm",
            forbidden,
        ),
        // claims can only make it stricter
        (
            "claimed flags",
            confirm_body("D", 216.3, Some(clean), json!(["ambiguous"]), 0.99),
            "value_flagged",
            refused,
        ),
        (
            "claimed low confidence",
            confirm_body("D", 216.3, Some(clean), json!([]), 0.84),
            "value_low_confidence",
            refused,
        ),
    ] {
        let (st, _, b) = t.send("PUT", &vals, Some(body.clone()), &ac).await;
        assert_eq!((st, code(&b)), (status, json!(want)), "{name}: {body}");
    }
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values"),
        1,
        "the earlier proposal is the only value"
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM operator_events"), 1);
    // the attached clean reading confirms; the stored record, not the claim, is what is kept
    let mut body = confirm_body("D", 216.3, Some(clean), json!([]), 0.99);
    body["values"][0]["source"] = json!("derived");
    body["values"][0]["text"] = json!("216.3");
    let (st, _, b) = t.send("PUT", &vals, Some(body), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(d_value(&o, "D")["state"], "CONFIRMED");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values WHERE key = 'D' AND source = 'read' AND confidence = 0.97 AND flags_json = '[]'"),
        1,
        "source, flags and confidence come from the reading"
    );
    // a clean reading at the threshold works too, on a value first proposed from it
    let mut propose = put_body("q", 216.3, "read", json!([]), 0.85, false);
    propose["values"][0]["reading_id"] = json!(at_threshold);
    let (st, _, b) = t.send("PUT", &vals, Some(propose), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(confirm_body(
                "q",
                216.3,
                Some(at_threshold),
                json!([]),
                0.99,
            )),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values WHERE key = 'q' AND confidence = 0.85 AND state = 'CONFIRMED'"),
        1
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_decisions WHERE source = 'auto' AND decided_by = 'operator-ai'"),
        2
    );
    // an unconfirmed proposal of another source is fine
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(put_body("t", 8.0, "derived", json!([]), 0.9, false)),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(d_value(&o, "t")["state"], "PROPOSED");
    // a person cannot use reading_id, and a reading cannot be changed or deleted
    let mut pb = json!({ "values": [{ "part": "1", "key": "L", "value": 5.0, "text": "5", "source": "read" }] });
    pb["values"][0]["reading_id"] = json!(clean);
    let (st, v) = person(&t, "PUT", &vals, pb).await;
    assert_eq!(
        (st, v["code"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("reading_agent_only")
        )
    );
    assert!(
        t.app
            .db()
            .execute("DELETE FROM order_readings", [])
            .is_err()
    );
    assert!(
        t.app
            .db()
            .execute("UPDATE order_readings SET confidence = 1.0", [])
            .is_err()
    );
}

#[tokio::test]
async fn aiop_a_clean_reading_of_another_page_cannot_confirm_a_page_one_value() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    // the agent has a clean, high-confidence server reading, but it was read on page 2
    let page_2 = add_reading_on_page(&t, oid, 2, "216.3", 216.3, &[], 0.97);
    let cross_page = confirm_body("D", 216.3, Some(page_2), json!([]), 0.99);
    let (st, _, b) = t.send("PUT", &vals, Some(cross_page), &ac).await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::FORBIDDEN, json!("agent_cannot_confirm")),
        "a page-2 reading is not proof of a page-1 value: {}",
        String::from_utf8_lossy(&b)
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM order_values"), 0);
    assert_eq!(t.count("SELECT COUNT(*) FROM operator_events"), 0);
    // the same value read on the part's own page can be attached by a proposal and then confirmed
    let page_1 = add_reading_on_page(&t, oid, 1, "216.3", 216.3, &[], 0.97);
    let mut propose = put_body("D", 216.3, "read", json!([]), 0.97, false);
    propose["values"][0]["reading_id"] = json!(page_1);
    let (st, _, b) = t.send("PUT", &vals, Some(propose), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let own_page = confirm_body("D", 216.3, Some(page_1), json!([]), 0.99);
    let (st, _, b) = t.send("PUT", &vals, Some(own_page), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(d_value(&o, "D")["state"], "CONFIRMED");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values WHERE key = 'D' AND source = 'read'"),
        1
    );
    // a reading of another page cannot attach a different value either
    let mut wrong_page = put_body("L", 216.3, "read", json!([]), 0.97, false);
    wrong_page["values"][0]["reading_id"] = json!(page_2);
    let (st, _, b) = t.send("PUT", &vals, Some(wrong_page), &ac).await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::FORBIDDEN, json!("agent_cannot_confirm"))
    );
}

#[tokio::test]
async fn aiop_a_value_a_person_set_or_checked_is_not_overwritten_by_the_operator() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    // a person types D (proposed) and confirms t
    let (st, _) = person(&t, "PUT", &vals, json!({ "values": [
        { "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read" },
        { "part": "1", "key": "t", "value": 8.0, "text": "8", "source": "entered", "confirm": true } ] })).await;
    assert_eq!(st, StatusCode::OK);
    for (key, v) in [("D", 100.0), ("t", 9.0)] {
        let (st, _, b) = t
            .send(
                "PUT",
                &vals,
                Some(put_body(key, v, "read", json!([]), 0.97, false)),
                &ac,
            )
            .await;
        assert_eq!(
            (st, code(&b)),
            (StatusCode::CONFLICT, json!("person_value_protected")),
            "{key}"
        );
    }
    // a CORRECTED value (a person changed what was proposed) stays the person's too
    let (st, _, _) = t
        .send(
            "PUT",
            &vals,
            Some(put_body("L", 500.0, "read", json!([]), 0.9, false)),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = person(&t, "PUT", &vals, json!({ "values": [{ "part": "1", "key": "L", "value": 510.0, "text": "510", "source": "entered" }] })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values WHERE key = 'L' AND state = 'CORRECTED'"),
        1
    );
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(put_body("L", 500.0, "read", json!([]), 0.9, false)),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::CONFLICT, json!("person_value_protected"))
    );
    // the operator's own earlier proposal is still its to revise
    let (st, _, _) = t
        .send(
            "PUT",
            &vals,
            Some(put_body("N", 3.0, "read", json!([]), 0.9, false)),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(put_body("N", 4.0, "read", json!([]), 0.9, false)),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    // nothing of the person's changed, and the refusals are on record with no partial write
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(d_value(&o, "D")["value"], 216.3);
    assert_eq!(d_value(&o, "t")["value"], 8.0);
    assert_eq!(d_value(&o, "L")["value"], 510.0);
    assert!(
        audit_details("order_refused", &t)
            .iter()
            .any(|d| d.contains("person_value_protected (409)"))
    );
    // a batch with one protected value writes none of it
    let (st, _, _) = t
        .send("PUT", &vals, Some(with_ev(json!({ "values": [
            { "part": "1", "key": "M", "value": 1.0, "text": "1", "source": "read", "flags": [], "confidence": 0.9, "note": "n" },
            { "part": "1", "key": "D", "value": 7.0, "text": "7", "source": "read", "flags": [], "confidence": 0.9, "note": "n" } ] }))), &ac)
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values WHERE key = 'M'"),
        0
    );
}

#[tokio::test]
async fn aiop_every_operator_write_carries_an_event_and_a_pause_wins_even_without_one() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    let plain = json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read", "flags": [], "confidence": 0.9, "note": "n" }] });
    // no event: refused, nothing written
    let (st, _, b) = t.send("PUT", &vals, Some(plain.clone()), &ac).await;
    assert_eq!(
        (st, code(&b)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("operator_event_required")
        )
    );
    let mut empty = plain.clone();
    empty["events"] = json!([]);
    let (st, _, _) = t.send("PUT", &vals, Some(empty), &ac).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(t.count("SELECT COUNT(*) FROM order_values"), 0);
    // a person's value edit needs no event
    let (st, _) = person(&t, "PUT", &vals, plain.clone()).await;
    assert_eq!(st, StatusCode::OK);
    // paused: a write without events is refused as paused (409), not as missing events
    let base = format!("/api/orders/{oid}/operator");
    assert_eq!(
        person(&t, "POST", &format!("{base}/pause"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    let (st, _, b) = t.send("PUT", &vals, Some(plain), &ac).await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::CONFLICT, json!("operator_paused"))
    );
    // the other writes of the operator wait as well
    let (st, _, b) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/parts"),
            Some(json!({ "parts": [{ "no": "9", "shape": "cylinder" }], "note": "n" })),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::CONFLICT, json!("operator_paused"))
    );
}

/// A pause set after a request passed its first check (the race in the review) is seen inside the writing
/// transaction: the handler is called directly, past `write_access`, with the pause already set.
#[tokio::test]
async fn aiop_a_pause_is_rechecked_inside_the_transaction_of_every_agent_write() {
    use axum::http::{HeaderMap, HeaderValue, header};
    let t = boot();
    let oid = order_with_parts(&t).await;
    add_agent(&t, "operator-ai");
    let id = t.count("SELECT id FROM users WHERE username = 'operator-ai'");
    let u = crate::User {
        id,
        username: "operator-ai".into(),
        role: "engineer".into(),
        kind: "agent".into(),
    };
    let mut h = HeaderMap::new();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let body = put_body("D", 216.3, "read", json!([]), 0.9, false).to_string();
    // not paused: the write goes through the same entry
    let r = crate::orders::put_values_do(&t.app, &h, &u, oid, body.as_bytes());
    assert!(r.is_ok());
    t.sql("DELETE FROM order_values");
    t.sql(&format!(
        "INSERT INTO operator_state(order_id, paused, by_user, at) VALUES ({oid}, 1, 'admin', 0)"
    ));
    let r = crate::orders::put_values_do(&t.app, &h, &u, oid, body.as_bytes());
    let Err(crate::ApiError::Resp(resp)) = r else {
        panic!("a paused operator write must be refused")
    };
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(t.count("SELECT COUNT(*) FROM order_values"), 0);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM operator_events"),
        1,
        "only the first write's event"
    );
}

#[tokio::test]
async fn aiop_columns_are_additive_over_a_schema_8_database() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    person(&t, "PUT", &format!("/api/orders/{oid}/values"), json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read" }] })).await;
    assert_eq!(storage::SCHEMA_VERSION, 10);
    // what a schema 8 build (order-compute, no AI operator) left behind: no operator columns, version 8
    t.app.db().execute_batch("ALTER TABLE order_values DROP COLUMN flags_json; ALTER TABLE order_values DROP COLUMN confidence; ALTER TABLE order_values DROP COLUMN edited_kind; ALTER TABLE order_values DROP COLUMN attached_reading_id; PRAGMA user_version = 8;").unwrap();
    drop(t.app.db());
    let app = build_app_with(config(&t.dir), true).unwrap();
    let db = app.db();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        10
    );
    let (f, c, k, a): (String, Option<f64>, String, Option<i64>) = db
        .query_row(
            "SELECT flags_json, confidence, edited_kind, attached_reading_id FROM order_values WHERE key = 'D'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (f.as_str(), c, k.as_str()),
        ("[]", None, "person"),
        "an old value reads as a person's"
    );
    assert_eq!(
        a, None,
        "an old value has no server-owned reading attachment"
    );
    // the schema 8 trigger of order-compute is still there
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name IN ('order_results_immutable', 'operator_events_no_delete')", [], |r| r.get::<_, i64>(0)).unwrap(),
        2
    );
}

// ---------------------------------------------------------------- round 2: person parts, events on every write

fn parts_body(parts: Value) -> Value {
    with_ev(json!({ "parts": parts, "note": "n" }))
}

fn part_1(qty: i64, exclude: bool) -> Value {
    let mut p =
        json!({ "id": 1, "no": "1", "name": "main", "shape": "cylinder", "qty": qty, "page": 1 });
    if exclude {
        p["state"] = json!("EXCLUDED");
    }
    p
}
fn part_2() -> Value {
    json!({ "id": 2, "no": "2", "name": "branch", "shape": "tee_branch", "page": 1, "attach_part": "1" })
}

#[tokio::test]
async fn aiop_the_operator_does_not_change_exclude_drop_or_skip_what_a_person_set() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let parts = format!("/api/orders/{oid}/parts");
    // before any person value the operator may reshape part 1
    let (st, _, b) = t
        .send(
            "PUT",
            &parts,
            Some(parts_body(json!([part_1(3, false), part_2()]))),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    // a person types D of part 1
    let (st, _) = person(&t, "PUT", &format!("/api/orders/{oid}/values"), json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read" }] })).await;
    assert_eq!(st, StatusCode::OK);
    let protected = StatusCode::CONFLICT;
    let mut renamed = part_1(3, false);
    renamed["name"] = json!("other");
    let mut reshaped = part_1(3, false);
    reshaped["shape"] = json!("cone");
    let mut renumbered = part_1(3, false);
    renumbered["no"] = json!("7");
    let mut p2_free = part_2();
    p2_free["attach_part"] = Value::Null;
    let mut p2_follows = part_2();
    p2_follows["attach_part"] = json!("7");
    for (name, list) in [
        ("exclude", json!([part_1(3, true), part_2()])),
        ("qty", json!([part_1(4, false), part_2()])),
        ("rename", json!([renamed, part_2()])),
        ("shape", json!([reshaped, part_2()])),
        ("renumber", json!([renumbered, p2_follows])),
        ("drop", json!([p2_free])),
    ] {
        let (st, _, b) = t.send("PUT", &parts, Some(parts_body(list)), &ac).await;
        assert_eq!(
            (st, code(&b)),
            (protected, json!("person_value_protected")),
            "{name}"
        );
    }
    assert_eq!(
        t.count(
            "SELECT COUNT(*) FROM order_parts WHERE state = 'EXCLUDED' OR (id = 1 AND qty <> 3)"
        ),
        0,
        "a refused edit changes nothing"
    );
    // the same part sent as it is, and another part without a person's value, are the operator's to change
    let mut p2 = part_2();
    p2["qty"] = json!(5);
    let (st, _, b) = t
        .send(
            "PUT",
            &parts,
            Some(parts_body(json!([part_1(3, false), p2]))),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    // a page whose part holds a person's value cannot be skipped by the operator; a free page can
    let skip = |n: i64, skipped: bool| {
        (
            format!("/api/orders/{oid}/pages/{n}/skip"),
            with_ev(json!({ "skipped": skipped, "note": "n" })),
        )
    };
    let (path, body) = skip(1, true);
    let (st, _, b) = t.send("POST", &path, Some(body), &ac).await;
    assert_eq!((st, code(&b)), (protected, json!("person_value_protected")));
    let (path, body) = skip(2, true);
    let (st, _, b) = t.send("POST", &path, Some(body), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    // a CONFIRMED value of the operator's own protects its part and page the same way
    t.sql("UPDATE order_values SET state = 'CONFIRMED', edited_kind = 'agent'");
    let (st, _, b) = t
        .send(
            "PUT",
            &parts,
            Some(parts_body(json!([part_1(3, true), part_2()]))),
            &ac,
        )
        .await;
    assert_eq!((st, code(&b)), (protected, json!("person_value_protected")));
    // a person may do all of it
    let (st, _) = person(
        &t,
        "PUT",
        &parts,
        json!({ "parts": [part_1(3, true), part_2()] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        audit_details("order_refused", &t)
            .iter()
            .any(|d| d.contains("person_value_protected (409)"))
    );
}

#[tokio::test]
async fn aiop_every_write_of_the_operator_needs_an_event() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let no_event = (
        StatusCode::UNPROCESSABLE_ENTITY,
        json!("operator_event_required"),
    );
    let events_before = t.count("SELECT COUNT(*) FROM operator_events");
    for (what, method, path, body) in [
        (
            "put_parts",
            "PUT",
            format!("/api/orders/{oid}/parts"),
            json!({ "parts": [part_1(2, false), part_2()], "note": "n" }),
        ),
        (
            "skip",
            "POST",
            format!("/api/orders/{oid}/pages/2/skip"),
            json!({ "note": "n" }),
        ),
        (
            "decision",
            "POST",
            format!("/api/orders/{oid}/decisions"),
            json!({ "kind": "confirm", "part": "1", "key": "D", "reason": "r" }),
        ),
        (
            "compute",
            "POST",
            format!("/api/orders/{oid}/compute"),
            json!({ "note": "n" }),
        ),
        (
            "put_values",
            "PUT",
            format!("/api/orders/{oid}/values"),
            json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read", "note": "n" }] }),
        ),
    ] {
        let (st, _, b) = t.send(method, &path, Some(body.clone()), &ac).await;
        assert_eq!((st, code(&b)), no_event, "{what}");
        // an empty list is no event either
        let mut empty = body;
        empty["events"] = json!([]);
        let (st, _, _) = t.send(method, &path, Some(empty), &ac).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{what} empty");
    }
    // compute with no body at all
    let (st, _, b) = t
        .send("POST", &format!("/api/orders/{oid}/compute"), None, &ac)
        .await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason")),
        "a reason is asked first"
    );
    // an upload has no JSON body: its events travel in a header
    let up = format!("/api/orders/{oid}/files?reason=page");
    let (st, b) = raw_with(&t, "POST", &up, png(100, 100), &agent, Some("c.png"), &[]).await;
    assert_eq!(
        (
            st,
            serde_json::from_slice::<Value>(&b).unwrap()["code"].clone()
        ),
        no_event
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM order_files"), 2);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM operator_events"),
        events_before,
        "a refused write stores no event"
    );
    // each of them goes through with an event, and the events are stored with the write
    let (st, _, b) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/parts"),
            Some(parts_body(json!([part_1(2, false), part_2()]))),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let (st, _, b) = t
        .send(
            "POST",
            &format!("/api/orders/{oid}/pages/2/skip"),
            Some(with_ev(json!({ "note": "n" }))),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let (st, _, b) = t
        .send(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(with_ev(json!({ "note": "n" }))),
            &ac,
        )
        .await;
    assert!(
        st == StatusCode::OK || st == StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        String::from_utf8_lossy(&b)
    );
    let ev = hex::encode(
        json!([{ "step": "up-1", "action": "type", "target": { "ui": "upload" }, "value": null, "reason": "a page", "confidence": null, "flags": [] }])
            .to_string(),
    );
    let (st, b) = raw_with(
        &t,
        "POST",
        &up,
        png(100, 100),
        &agent,
        Some("c.png"),
        &[("x-operator-events", ev.as_str())],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    assert!(t.count("SELECT COUNT(*) FROM operator_events") >= events_before + 3);
    // a person's write carries none
    let (st, v) = person(
        &t,
        "PUT",
        &format!("/api/orders/{oid}/parts"),
        parts_body(json!([part_1(2, false), part_2()])),
    )
    .await;
    assert_eq!(
        (st, v["code"].clone()),
        (StatusCode::FORBIDDEN, json!("operator_agent_only"))
    );
    // a pause is read inside the transaction of each of them (events are stored there)
    t.sql(&format!(
        "INSERT INTO operator_state(order_id, paused, by_user, at) VALUES ({oid}, 1, 'admin', 0)"
    ));
    for (method, path, body) in [
        (
            "PUT",
            format!("/api/orders/{oid}/parts"),
            parts_body(json!([part_1(2, false), part_2()])),
        ),
        (
            "POST",
            format!("/api/orders/{oid}/pages/2/skip"),
            with_ev(json!({ "skipped": false, "note": "n" })),
        ),
    ] {
        let (st, _, b) = t.send(method, &path, Some(body), &ac).await;
        assert_eq!(
            (st, code(&b)),
            (StatusCode::CONFLICT, json!("operator_paused"))
        );
    }
}

#[tokio::test]
async fn aiop_create_read_and_undo_writes_carry_events_too() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let no_event = (
        StatusCode::UNPROCESSABLE_ENTITY,
        json!("operator_event_required"),
    );
    // creating an order and reading a page are writes: neither may be event-free
    let create = "/api/projects/1/orders";
    let (st, _, b) = t
        .send(
            "POST",
            create,
            Some(json!({ "name": "agent order", "reason": "r" })),
            &ac,
        )
        .await;
    assert_eq!((st, code(&b)), no_event);
    assert_eq!(t.count("SELECT COUNT(*) FROM orders"), 1);
    let read = format!("/api/orders/{oid}/pages/3/read");
    let (st, _, b) = t.send("POST", &read, Some(json!({})), &ac).await;
    assert_eq!((st, code(&b)), no_event);
    // an agent-made confirmation can be undone, but only with an event of its own
    agent_clean_read(&t, &agent, oid, "D", 216.3, "216.3").await;
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let did = o["decisions"][0]["id"].as_i64().unwrap();
    let undo = format!("/api/orders/{oid}/decisions/{did}/undo");
    let events_before_undo = t.count("SELECT COUNT(*) FROM operator_events");
    let (st, _, b) = t
        .send("POST", &undo, Some(json!({ "note": "n" })), &ac)
        .await;
    assert_eq!((st, code(&b)), no_event);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM operator_events"),
        events_before_undo
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_decisions WHERE undone_at IS NOT NULL"),
        0
    );
    let (st, _, b) = t
        .send("POST", &undo, Some(with_ev(json!({ "note": "n" }))), &ac)
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_decisions WHERE undone_at IS NOT NULL"),
        1
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM operator_events"),
        events_before_undo + 1
    );
    // with an event, the new order holds that event in its own transaction
    let (st, _, b) = t
        .send(
            "POST",
            create,
            Some(with_ev(json!({ "name": "agent order", "reason": "r" }))),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let new_oid = b["id"].as_i64().unwrap();
    assert_eq!(
        t.count(&format!(
            "SELECT COUNT(*) FROM operator_events WHERE order_id = {new_oid}"
        )),
        1
    );
    // the operator's own undo is person-only; a person still sends no events
    let base = format!("/api/orders/{oid}/operator/undo");
    let (st, v) = person(&t, "POST", &base, with_ev(json!({ "note": "n" }))).await;
    assert_eq!(
        (st, v["code"].clone()),
        (StatusCode::FORBIDDEN, json!("operator_agent_only"))
    );
}

#[tokio::test]
async fn aiop_the_server_stores_what_its_reader_found_and_serves_it() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let u = crate::User {
        id: t.count("SELECT id FROM users LIMIT 1"),
        username: "admin".into(),
        role: "admin".into(),
        kind: "person".into(),
    };
    let out = crate::orders::ReaderOut {
        reader_id: "test reader".into(),
        model_sha256: "ab".repeat(32),
        items: vec![
            crate::orders::ReadingItem {
                text: "φ216.3".into(),
                value: Some(216.3),
                kind: "diameter".into(),
                bbox: [10.0, 20.0, 80.0, 40.0],
                flags: vec![],
                confidence: 0.97,
            },
            crate::orders::ReadingItem {
                text: "1/1".into(),
                value: None,
                kind: "number".into(),
                bbox: [1.0, 2.0, 3.0, 4.0],
                flags: vec!["ambiguous".into()],
                confidence: 0.5,
            },
        ],
    };
    let rows = {
        let mut db = t.app.db();
        let tx = db.transaction().unwrap();
        let r = crate::orders::store_readings(&t.app, &tx, &u, oid, 1, &out)
            .unwrap_or_else(|_| panic!("stored"));
        tx.commit().unwrap();
        r
    };
    assert_eq!(rows.len(), 2);
    let v = t.get(&format!("/api/orders/{oid}/pages/1/readings")).await;
    assert_eq!(v["readings"].as_array().unwrap().len(), 2);
    assert_eq!(v["readings"][0]["id"], rows[0]["id"]);
    assert_eq!(
        (
            v["readings"][0]["kind"].clone(),
            v["readings"][0]["reader_id"].clone()
        ),
        (json!("diameter"), json!("test reader"))
    );
    assert_eq!(v["readings"][1]["flags"], json!(["ambiguous"]));
    assert_eq!(v["readings"][1]["value"], Value::Null);
    assert_eq!(
        t.get(&format!("/api/orders/{oid}/pages/2/readings")).await["readings"],
        json!([])
    );
    assert_eq!(audit_details("order_page_read", &t).len(), 1);
    // a page that is not in the order has none to show
    let (st, _) = t
        .call("GET", &format!("/api/orders/{oid}/pages/99/readings"), None)
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}
