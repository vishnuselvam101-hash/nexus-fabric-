//! AIOP-1: the operator event stream, pause and undo, driven through the real router.

use super::orders::{add_agent, order_with_parts, with_ev};
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
    let body = json!({
        "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "flags": [], "confidence": 0.97, "confirm": true, "note": "matches the list" }],
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
    assert_eq!(el["events"].as_array().unwrap().len(), 1);
    assert_eq!(el["events"][0]["seq"], 1);
    assert_eq!(el["events"][0]["write_ref"]["key"], "D");
    assert_eq!(el["events"][0]["reason_key"], Value::Null);

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
    assert_eq!(el["events"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn aiop_undo_puts_a_typed_value_back_and_refuses_when_it_was_changed() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let base = format!("/api/orders/{oid}/operator");
    // the values and the type event that says what was typed commit together
    let write = |key: &str, v: f64, text: &str| {
        let mut e = ev(
            "type-x",
            "type",
            json!({ "ui": format!("part-1-{key}") }),
            "typed from the page",
        );
        e["value"] = json!(v);
        e["write_ref"] = json!({ "part": "1", "key": key });
        json!({ "values": [{ "part": "1", "key": key, "value": v, "text": text, "source": "read", "flags": [], "confidence": 0.97, "confirm": true, "note": "matches the list" }], "events": [e] })
    };
    let (st, _, b) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(write("D", 216.3, "φ216.3")),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
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
    assert_eq!((st, v["undone"].clone()), (StatusCode::OK, json!([1])));
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
    assert_eq!(ev_list["events"][0]["undone"], true);
    // nothing left to undo
    let (st, v) = person(&t, "POST", &format!("{base}/undo"), json!({})).await;
    assert_eq!(
        (st, v["code"].clone()),
        (StatusCode::CONFLICT, json!("nothing_to_undo"))
    );
    // after a person's undo the value is the person's: the operator does not type over it again
    let (st, _, b) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(write("D", 100.0, "100")),
            &ac,
        )
        .await;
    assert_eq!(
        (
            st,
            serde_json::from_slice::<Value>(&b).unwrap()["code"].clone()
        ),
        (StatusCode::CONFLICT, json!("person_value_protected"))
    );
    // a value a person changed after the operator typed it is not rolled back
    let (st, _, b) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(write("t", 100.0, "100")),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
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
            json!({ "kind": "confirm", "part": "1", "key": "D", "reason": "r" }),
        ),
        (
            format!("/api/orders/{oid}/decisions"),
            json!({ "kind": "accept_flag", "part": "1", "key": "D", "flag": "repaired", "reason": "r" }),
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

#[tokio::test]
async fn aiop_the_operator_confirms_only_a_clean_confident_page_reading() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    // flags, low confidence (just under the threshold) and a source that is not a reading are all refused
    for (body, want, status) in [
        (
            put_body("D", 216.3, "read", json!(["ambiguous"]), 0.99, true),
            "value_flagged",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            put_body("D", 216.3, "read", json!([]), 0.84, true),
            "value_low_confidence",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            put_body("D", 216.3, "derived", json!([]), 0.99, true),
            "agent_cannot_confirm",
            StatusCode::FORBIDDEN,
        ),
        (
            put_body("D", 216.3, "fixed", json!([]), 1.0, true),
            "agent_cannot_confirm",
            StatusCode::FORBIDDEN,
        ),
        (
            put_body("D", 216.3, "entered", json!([]), 1.0, true),
            "agent_cannot_confirm",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let (st, _, b) = t.send("PUT", &vals, Some(body.clone()), &ac).await;
        assert_eq!((st, code(&b)), (status, json!(want)), "{body}");
    }
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_values"),
        0,
        "a refused write leaves nothing"
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM operator_events"), 0);
    // the same reading, clean and at the threshold, is confirmed by the operator and recorded as its own
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(put_body("D", 216.3, "read", json!([]), 0.85, true)),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(d_value(&o, "D")["state"], "CONFIRMED");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_decisions WHERE source = 'auto' AND decided_by = 'operator-ai'"),
        1
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
async fn aiop_schema_9_is_additive_over_a_schema_8_database() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    person(&t, "PUT", &format!("/api/orders/{oid}/values"), json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "216.3", "source": "read" }] })).await;
    assert_eq!(storage::SCHEMA_VERSION, 9);
    // what a schema 8 build (order-compute, no AI operator) left behind: no operator columns, version 8
    t.app.db().execute_batch("ALTER TABLE order_values DROP COLUMN flags_json; ALTER TABLE order_values DROP COLUMN confidence; ALTER TABLE order_values DROP COLUMN edited_kind; PRAGMA user_version = 8;").unwrap();
    drop(t.app.db());
    let app = build_app_with(config(&t.dir), true).unwrap();
    let db = app.db();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        9
    );
    let (f, c, k): (String, Option<f64>, String) = db
        .query_row(
            "SELECT flags_json, confidence, edited_kind FROM order_values WHERE key = 'D'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (f.as_str(), c, k.as_str()),
        ("[]", None, "person"),
        "an old value reads as a person's"
    );
    // the schema 8 trigger of order-compute is still there
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name IN ('order_results_immutable', 'operator_events_no_delete')", [], |r| r.get::<_, i64>(0)).unwrap(),
        2
    );
}
