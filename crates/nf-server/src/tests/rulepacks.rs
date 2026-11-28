//! PAR-2: factory rule packs. Import/export round-trip, person-only writes (the AI operator gets 403), the
//! onboarding interview marks rules CONFIRMED with name and date, undo, and the engine reads the assigned pack
//! (a default pack changes nothing).
use super::orders::{PDF_2_PAGES, add_agent, add_file, new_order, put_parts_values, val};
use super::*;

async fn import(t: &T, pack: Value) -> (StatusCode, Value) {
    t.call("POST", "/api/rule-packs", Some(pack)).await
}

fn pack_with(name: &str, edits: &[(&str, Value)]) -> Value {
    let mut p = nf_core::rulepack::Pack::defaults(name, "1").to_json();
    for (k, v) in edits {
        p["rules"][*k]["value"] = v.clone();
    }
    p
}

fn admin_name(t: &T) -> String {
    t.app
        .db()
        .query_row(
            "SELECT username FROM users WHERE role = 'admin' ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[tokio::test]
async fn par2_export_then_import_round_trips_values_and_sources() {
    let t = boot();
    let (st, v) = import(
        &t,
        pack_with(
            "Shop A",
            &[("allow", json!(5.0)), ("seam_gap", json!(12.5))],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let id = v["id"].as_i64().unwrap();
    let exported = t.get(&format!("/api/rule-packs/{id}")).await;
    assert_eq!(exported["rules"]["allow"]["value"], json!(5.0));
    assert!(exported["sha256"].as_str().unwrap().len() == 64);
    let (st, again) = import(&t, exported.clone()).await;
    assert_eq!(st, StatusCode::OK, "{again}");
    assert_eq!(
        again["pack"]["rules"], exported["rules"],
        "values and sources survive the round trip"
    );
    assert_eq!(again["sha256"], exported["sha256"]);
    // a bad rule is refused and named
    let mut bad = pack_with("x", &[]);
    bad["rules"]["bogus"] = json!({ "value": 1 });
    let (st, v) = import(&t, bad).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["code"], "rule_pack_invalid");
    assert!(v["error"].as_str().unwrap().contains("bogus"), "{v}");
}

#[tokio::test]
async fn par2_a_file_cannot_confirm_a_rule() {
    let t = boot();
    let mut p = pack_with("Forged", &[]);
    p["rules"]["basis"]["status"] = json!("CONFIRMED");
    p["rules"]["basis"]["confirmed_by"] = json!("Someone");
    p["rules"]["basis"]["confirmed_on"] = json!("2026-11-01");
    let (st, v) = import(&t, p).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["downgraded"], json!(["basis"]));
    assert_eq!(v["pack"]["rules"]["basis"]["status"], "UNVERIFIED");
}

#[tokio::test]
async fn par2_the_ai_operator_reads_but_never_writes() {
    let t = boot();
    let (_, v) = import(&t, pack_with("Shop A", &[])).await;
    let id = v["id"].as_i64().unwrap();
    let oid = new_order(&t).await;
    let ac = add_agent(&t, "aiop");
    // the agent reads a pack through the project it works on (project 1), so a person assigns it there first
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/assign"),
            Some(json!({ "scope": "project", "target_id": 1 })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let agent = |method: &'static str, path: String, body: Option<Value>| {
        let t = &t;
        let ac = ac.clone();
        async move {
            t.send(method, &path, body, &[("cookie", ac.as_str())])
                .await
        }
    };
    let (st, _, _) = agent("GET", format!("/api/rule-packs/{id}"), None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, _) = agent("GET", format!("/api/rule-packs/{id}/interview"), None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, b, _) = agent("POST", "/api/rule-packs".into(), Some(pack_with("ai", &[]))).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{b:?}");
    let (st, _, bytes) = agent(
        "POST",
        format!("/api/rule-packs/{id}/interview/answers"),
        Some(json!({ "key": "allow", "action": "accept" })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(String::from_utf8_lossy(&bytes).contains("agent_cannot_edit_rules"));
    let (st, _, _) = agent(
        "POST",
        format!("/api/rule-packs/{id}/assign"),
        Some(json!({ "scope": "order", "target_id": oid })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _, _) = agent(
        "POST",
        format!("/api/rule-packs/{id}/confirmations/1/undo"),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(t.count("SELECT COUNT(*) FROM rule_confirmations"), 0);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM rule_pack_assignments"),
        1,
        "only the person's"
    );
    // every refused attempt is audited: import, answer, assign, undo
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'rule_pack_refused'"),
        4
    );
}

#[tokio::test]
async fn par2_interview_marks_confirmed_with_name_and_date_and_skip_stays_unverified() {
    let t = boot();
    let (_, v) = import(&t, pack_with("Shop A", &[])).await;
    let id = v["id"].as_i64().unwrap();
    let q = t.get(&format!("/api/rule-packs/{id}/interview")).await;
    let n = q["questions"].as_array().unwrap().len();
    assert!(n >= 15, "one question per rule");
    assert!(
        !q["questions"][0]["question_ja"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(
        q["questions"][0]["picture"]
            .as_str()
            .unwrap()
            .starts_with("pic_")
    );
    // skip: stays UNVERIFIED
    let (st, s) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/interview/answers"),
            Some(json!({ "key": "kerf", "action": "skip" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{s}");
    assert_eq!(s["status"], "UNVERIFIED");
    // accept and set confirm
    let (st, a) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/interview/answers"),
            Some(json!({ "key": "allow", "action": "accept" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{a}");
    assert_eq!(a["status"], "CONFIRMED");
    assert_eq!(a["confirmed_by"], admin_name(&t));
    assert_eq!(a["confirmed_on"].as_str().unwrap().len(), 10);
    let (st, b) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/interview/answers"),
            Some(json!({ "key": "kerf", "action": "set", "value": 0.2 })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["value"], json!(0.2));
    // a value the engine cannot use is refused, nothing confirmed
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/interview/answers"),
            Some(json!({ "key": "seam_deg", "action": "set", "value": "forty" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let q = t.get(&format!("/api/rule-packs/{id}/interview")).await;
    let st_of = |k: &str| {
        q["questions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["key"] == k)
            .unwrap()["status"]
            .clone()
    };
    assert_eq!(st_of("allow"), "CONFIRMED");
    assert_eq!(st_of("kerf"), "CONFIRMED");
    assert_eq!(st_of("seam_deg"), "UNVERIFIED");
    assert_eq!(q["complete"], false);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'rule_confirmed'"),
        2
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'rule_skipped'"),
        1
    );
    // undo the kerf confirmation: back to UNVERIFIED, the undo is audited
    let cid = t.count("SELECT id FROM rule_confirmations WHERE rule_key = 'kerf'");
    let (st, u) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/confirmations/{cid}/undo"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{u}");
    assert_eq!(u["status"], "UNVERIFIED");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'rule_confirmation_undone'"),
        1
    );
    // answering every question completes the pack; a rule with no value needs a value, accepting nothing is refused
    let (st, nv) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/interview/answers"),
            Some(json!({ "key": "seam_gap", "action": "accept" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{nv}");
    for e in nf_core::rulepack::REGISTRY {
        let body = if e.key == "seam_gap" {
            json!({ "key": e.key, "action": "set", "value": 12.5 })
        } else if let Some(c) = e.choices.first() {
            json!({ "key": e.key, "action": "set", "value": c })
        } else if e.default == "null" {
            json!({ "key": e.key, "action": "set", "value": 1.0 })
        } else {
            json!({ "key": e.key, "action": "accept" })
        };
        let (st, _) = t
            .call(
                "POST",
                &format!("/api/rule-packs/{id}/interview/answers"),
                Some(body),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{}", e.key);
    }
    let last = t.get(&format!("/api/rule-packs/{id}/interview")).await;
    assert_eq!(last["complete"], true);
    assert!(
        last["questions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|x| x["status"] == "CONFIRMED")
    );
}

#[tokio::test]
async fn par2_compat_default_pack_changes_nothing_and_an_assigned_value_reaches_the_engine() {
    let t = boot();
    let oid = new_order(&t).await;
    let before = t.get(&format!("/api/orders/{oid}/rules")).await;
    let (_, v) = import(&t, pack_with("Defaults", &[])).await;
    let id = v["id"].as_i64().unwrap();
    let (st, a) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/assign"),
            Some(json!({ "scope": "project", "target_id": 1 })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{a}");
    let with_default = t.get(&format!("/api/orders/{oid}/rules")).await;
    assert_eq!(
        with_default["effective"], before["effective"],
        "a default pack is the same rules"
    );
    assert_eq!(with_default["fingerprint"], before["fingerprint"]);
    assert_eq!(with_default["rule_pack"]["name"], "Defaults");
    // a changed engine rule reaches the order's effective rules
    let (_, w) = import(&t, pack_with("Top seam", &[("seam_from", json!("top"))])).await;
    let id2 = w["id"].as_i64().unwrap();
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id2}/assign"),
            Some(json!({ "scope": "order", "target_id": oid })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let top = t.get(&format!("/api/orders/{oid}/rules")).await;
    assert_ne!(top["fingerprint"], before["fingerprint"]);
    let seam = top["effective"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "order.seam")
        .unwrap();
    assert!(
        seam["value"].as_str().unwrap().contains("from the top"),
        "{seam}"
    );
    assert_eq!(
        top["rule_pack"]["name"], "Top seam",
        "an order assignment wins over the project's"
    );
}

/// Confirms every rule of a pack with a real value (the interview's own answers, as one person would give them).
async fn complete_pack(t: &T, id: i64) {
    for e in nf_core::rulepack::REGISTRY {
        let body = if e.key == "seam_gap" {
            json!({ "key": e.key, "action": "set", "value": 12.5 })
        } else if let Some(c) = e.choices.first() {
            json!({ "key": e.key, "action": "set", "value": c })
        } else if e.default == "null" {
            json!({ "key": e.key, "action": "set", "value": 1.0 })
        } else {
            json!({ "key": e.key, "action": "accept" })
        };
        let (st, v) = t
            .call(
                "POST",
                &format!("/api/rule-packs/{id}/interview/answers"),
                Some(body),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{}: {v}", e.key);
    }
}

#[tokio::test]
async fn par2_undo_is_newest_first_per_rule() {
    let t = boot();
    let (_, v) = import(&t, pack_with("Undo", &[])).await;
    let id = v["id"].as_i64().unwrap();
    let answer = |value: Value| {
        let t = &t;
        async move {
            t.call(
                "POST",
                &format!("/api/rule-packs/{id}/interview/answers"),
                Some(json!({ "key": "allow", "action": "set", "value": value })),
            )
            .await
        }
    };
    let (_, first) = answer(json!(4.5)).await;
    let (_, second) = answer(json!(5.0)).await;
    let (c1, c2) = (
        first["confirmation_id"].as_i64().unwrap(),
        second["confirmation_id"].as_i64().unwrap(),
    );
    let undo = |cid: i64| {
        let t = &t;
        async move {
            t.call(
                "POST",
                &format!("/api/rule-packs/{id}/confirmations/{cid}/undo"),
                None,
            )
            .await
        }
    };
    // the older confirmation cannot be undone over the newer one
    let (st, v) = undo(c1).await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["code"], "undo_out_of_order");
    let exported = t.get(&format!("/api/rule-packs/{id}")).await;
    assert_eq!(
        exported["rules"]["allow"]["value"],
        json!(5.0),
        "newer state kept"
    );
    // newest first: back to 4.5 (the first confirmation), then the older one can go
    let (st, v) = undo(c2).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["value"], json!(4.5));
    let (st, v) = undo(c1).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "UNVERIFIED");
    assert_eq!(v["value"], json!(4.0));
    // a standing confirmation cannot be undone twice
    let (st, _) = undo(c1).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn par2_null_is_never_a_confirmation() {
    let t = boot();
    let (_, v) = import(&t, pack_with("Nulls", &[])).await;
    let id = v["id"].as_i64().unwrap();
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/interview/answers"),
            Some(json!({ "key": "seam_gap", "action": "accept" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/interview/answers"),
            Some(json!({ "key": "seam_gap", "action": "set", "value": null })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(t.count("SELECT COUNT(*) FROM rule_confirmations"), 0);
    let q = t.get(&format!("/api/rule-packs/{id}/interview")).await;
    assert_eq!(q["complete"], false);
}

#[tokio::test]
async fn par2_an_open_engine_rule_holds_every_part_until_the_pack_is_complete() {
    let t = boot();
    let oid = new_order(&t).await;
    assert_eq!(
        add_file(&t, oid, PDF_2_PAGES.to_vec(), "a.pdf").await.0,
        StatusCode::OK
    );
    put_parts_values(
        &t,
        oid,
        json!([
            { "no": "C1", "name": "pipe", "shape": "cylinder", "qty": 2 },
            { "no": "P1", "name": "plate", "shape": "plate" }
        ]),
        json!([
            val("C1", "D", 1016.0),
            val("C1", "t", 12.0),
            val("C1", "L", 2000.0),
            val("P1", "t", 16.0),
            val("P1", "plate_W", 400.0)
        ]),
    )
    .await;
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/confirm"),
            Some(json!({ "all": true })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    async fn compute(t: &T, oid: i64) -> Vec<Value> {
        let (st, v) = t
            .call(
                "POST",
                &format!("/api/orders/{oid}/compute"),
                Some(json!({})),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{v}");
        v["result"]["parts"].as_array().unwrap().clone()
    }
    // no pack: computed as before, no pack hold
    let plain = compute(&t, oid).await;
    let c1 = plain.iter().find(|p| p["no"] == "C1").unwrap().clone();
    assert_eq!(c1["state"], "computed", "{c1}");
    // an assigned default pack leaves seam_gap open: every part holds, none is silently computed on a default
    let (_, v) = import(&t, pack_with("Open", &[])).await;
    let id = v["id"].as_i64().unwrap();
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/assign"),
            Some(json!({ "scope": "order", "target_id": oid })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    for p in compute(&t, oid).await {
        assert!(p.to_string().contains("rule_pack_unconfirmed"), "{p}");
        assert_ne!(p["state"], "computed", "{p}");
        assert_eq!(p["dxf_allowed"], false, "{p}");
    }
    // the pack is completed by a person: the hold goes away
    complete_pack(&t, id).await;
    for p in compute(&t, oid).await {
        assert!(!p.to_string().contains("rule_pack_unconfirmed"), "{p}");
    }
    // a person's rules object cannot switch the hold on or off
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/rules/impact"),
            Some(json!({ "rules": { "pack_unconfirmed": [] }, "parts_hash": "0".repeat(64) })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
}

#[tokio::test]
async fn par2_pack_reads_are_limited_to_the_projects_it_is_assigned_to() {
    let t = boot();
    let (_, v) = import(&t, pack_with("Shop A", &[])).await;
    let id = v["id"].as_i64().unwrap();
    // an engineer who is not a member of project 1
    let outsider = add_session(&t.app, "outsider", "engineer");
    let read = |path: String| {
        let t = &t;
        let c = outsider.clone();
        async move { t.send("GET", &path, None, &[("cookie", c.as_str())]).await }
    };
    let (st, _, _) = read(format!("/api/rule-packs/{id}")).await;
    assert_eq!(
        st,
        StatusCode::NOT_FOUND,
        "an unassigned pack is not readable by others"
    );
    let (st, _, _) = read(format!("/api/rule-packs/{id}/interview")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _, bytes) = read("/api/rule-packs".into()).await;
    assert_eq!(st, StatusCode::OK);
    assert!(!String::from_utf8_lossy(&bytes).contains("\"Shop A\""));
    // once the pack is assigned to project 1, a member of project 1 reads it; the outsider still does not
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/assign"),
            Some(json!({ "scope": "project", "target_id": 1 })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, _) = read(format!("/api/rule-packs/{id}")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let listed = t.get("/api/rule-packs").await;
    assert!(
        listed["packs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == id)
    );
}

#[tokio::test]
async fn par2_unassign_removes_one_assignment_and_is_audited() {
    let t = boot();
    let (_, v) = import(&t, pack_with("Shop A", &[])).await;
    let id = v["id"].as_i64().unwrap();
    let oid = new_order(&t).await;
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/assign"),
            Some(json!({ "scope": "order", "target_id": oid })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/unassign"),
            Some(json!({ "scope": "order", "target_id": oid })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(t.count("SELECT COUNT(*) FROM rule_pack_assignments"), 0);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'rule_pack_unassigned'"),
        1
    );
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/rule-packs/{id}/unassign"),
            Some(json!({ "scope": "order", "target_id": oid })),
        )
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}
