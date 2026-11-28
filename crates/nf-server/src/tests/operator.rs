//! AIOP-1: the operator event stream, pause and undo, driven through the real router.

use super::orders::{add_agent, order_with_parts};
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
            ev("slot-1", "point", json!({ "ui": "part-1-D" }), "the diameter slot"),
            // a CAD target and a new action word need no server change
            ev("cad-1", "measure_entity", json!({ "cad": { "canvas": [12.5, 40.0], "entity": "e17" } }), "measure"),
        ],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!((v["first_seq"].clone(), v["last_seq"].clone()), (json!(1), json!(3)));
    let (_, v2) = post_ev(&t, oid, &agent, vec![ev("c", "compute", json!({ "ui": "compute-btn" }), "compute")]).await;
    assert_eq!(v2["first_seq"], 4);
    let all = t.get(&format!("/api/orders/{oid}/operator/events")).await;
    assert_eq!(all["events"].as_array().unwrap().len(), 4);
    assert_eq!(all["last_seq"], 4);
    assert_eq!(all["paused"], false);
    assert_eq!(all["events"][0]["by"], "operator-ai");
    let tail = t.get(&format!("/api/orders/{oid}/operator/events?after=3")).await;
    assert_eq!(tail["events"].as_array().unwrap().len(), 1);
    assert_eq!(tail["events"][0]["seq"], 4);
    // events are kept: the table refuses a delete and an edit of a written event
    assert!(t.app.db().execute("DELETE FROM operator_events", []).is_err());
    assert!(t.app.db().execute("UPDATE operator_events SET event_json = '{}'", []).is_err());
    let a = audit_details("operator_events", &t);
    assert_eq!(a.len(), 2, "{a:?}");
    assert!(a[0].contains("events 1..3") && a[0].contains("reading page 1"), "{a:?}");
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
    let (st, v) = post_ev(&t, oid, &t.cookie.clone(), vec![ev("a", "look", page_target(), "x")]).await;
    assert_eq!((st, code(&v)), (StatusCode::FORBIDDEN, json!("operator_agent_only")));
    // no reason
    let (st, v) = post_ev(&t, oid, &agent, vec![ev("a", "look", page_target(), "  ")]).await;
    assert_eq!((st, code(&v)), (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason")));
    // two kinds of target, a bad action word, a bad confidence, an empty batch
    for e in [
        ev("a", "look", json!({ "ui": "x", "page": 1 }), "r"),
        ev("a", "Look!", page_target(), "r"),
        ev("a", "look", json!({ "cad": {} }), "r"),
        ev("a", "look", json!({ "page": 0, "bbox": [1, 2, 3, 4] }), "r"),
        json!({ "step": "a", "action": "look", "target": { "ui": "x" }, "reason": "r", "confidence": 1.5 }),
    ] {
        let (st, v) = post_ev(&t, oid, &agent, vec![e]).await;
        assert_eq!((st, code(&v)), (StatusCode::UNPROCESSABLE_ENTITY, json!("operator_event")), "{v}");
    }
    let (st, _) = post_ev(&t, oid, &agent, vec![]).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    // nothing was stored by any of those
    assert_eq!(t.count("SELECT COUNT(*) FROM operator_events"), 0);
    // an agent outside the order's project is told 404
    let c = add_session(&t.app, "outsider-ai", "engineer");
    t.sql("UPDATE users SET kind = 'agent' WHERE username = 'outsider-ai'");
    t.sql("INSERT INTO projects(name, description, created_by, created_at) VALUES ('q','','admin',0)");
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
        let (st, _, _) = t.send("POST", &format!("{base}/{p}"), Some(json!({})), &ac).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{p}");
    }
    let (st, v) = person(&t, "POST", &format!("{base}/pause"), json!({})).await;
    assert_eq!((st, v["paused"].clone()), (StatusCode::OK, json!(true)));
    assert_eq!(t.get(&base_events(&base)).await["paused"], true);
    let (st, v) = post_ev(&t, oid, &agent, vec![ev("a", "look", page_target(), "r")]).await;
    assert_eq!((st, v["code"].clone()), (StatusCode::CONFLICT, json!("operator_paused")));
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
    assert!(audit_details("order_refused", &t).iter().any(|d| d.contains("operator_paused")));
}

fn base_events(base: &str) -> String {
    format!("{base}/events")
}

#[tokio::test]
async fn aiop_undo_puts_a_typed_value_back_and_refuses_when_it_was_changed() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let base = format!("/api/orders/{oid}/operator");
    let write = |v: f64, text: &str| {
        json!({ "values": [{ "part": "1", "key": "D", "value": v, "text": text, "source": "read", "confirm": true, "note": "matches the list" }] })
    };
    let (st, _, b) = t.send("PUT", &format!("/api/orders/{oid}/values"), Some(write(216.3, "φ216.3")), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let mut e = ev("type-d", "type", json!({ "ui": "part-1-D" }), "D from the page");
    e["value"] = json!(216.3);
    e["write_ref"] = json!({ "part": "1", "key": "D" });
    let (st, _) = post_ev(&t, oid, &agent, vec![e]).await;
    assert_eq!(st, StatusCode::OK);
    // the operator cannot undo
    let (st, _, _) = t.send("POST", &format!("{base}/undo"), Some(json!({})), &ac).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, v) = person(&t, "POST", &format!("{base}/undo"), json!({ "note": "wrong one" })).await;
    assert_eq!((st, v["undone"].clone()), (StatusCode::OK, json!([1])));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let val = &o["parts"][0]["values"].as_array().unwrap().iter().find(|x| x["key"] == "D").unwrap().clone();
    assert_eq!((val["state"].clone(), val["value"].clone()), (json!("MISSING"), Value::Null), "{val}");
    assert!(o["decisions"].as_array().unwrap().iter().all(|d| !d["undone_at"].is_null()), "{}", o["decisions"]);
    let ev_list = t.get(&format!("{base}/events")).await;
    assert_eq!(ev_list["events"][0]["undone"], true);
    // nothing left to undo
    let (st, v) = person(&t, "POST", &format!("{base}/undo"), json!({})).await;
    assert_eq!((st, v["code"].clone()), (StatusCode::CONFLICT, json!("nothing_to_undo")));
    // a value a person changed after the operator typed it is not rolled back
    let (st, _, _) = t.send("PUT", &format!("/api/orders/{oid}/values"), Some(write(100.0, "100")), &ac).await;
    assert_eq!(st, StatusCode::OK);
    let mut e = ev("type-d2", "type", json!({ "ui": "part-1-D" }), "D again");
    e["value"] = json!(100.0);
    e["write_ref"] = json!({ "part": "1", "key": "D" });
    post_ev(&t, oid, &agent, vec![e]).await;
    let (st, _) = person(
        &t,
        "PUT",
        &format!("/api/orders/{oid}/values"),
        json!({ "values": [{ "part": "1", "key": "D", "value": 101.0, "text": "101", "source": "entered" }] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, v) = person(&t, "POST", &format!("{base}/undo"), json!({})).await;
    assert_eq!((st, v["code"].clone()), (StatusCode::CONFLICT, json!("value_changed_since")));
    assert_eq!(audit_details("operator_undo", &t).len(), 1);
}

#[tokio::test]
async fn aiop_operator_routes_are_behind_the_licence_gate() {
    use axum::http::Method;
    for p in ["events", "undo"] {
        assert!(crate::licence::gated(&Method::POST, &format!("/api/orders/1/operator/{p}")), "{p}");
    }
    assert!(!crate::licence::gated(&Method::GET, "/api/orders/1/operator/events"));
}
