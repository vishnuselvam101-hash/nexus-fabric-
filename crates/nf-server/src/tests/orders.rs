//! NFJ-1 S1-L1: orders. Each test drives the real router against a private data directory.

use super::*;

const PDF_2_PAGES: &[u8] = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Count 2/Kids[3 0 R 4 0 R]>>endobj\n3 0 obj<</Type /Page/Parent 2 0 R>>endobj\n4 0 obj<</Type/Page/Parent 2 0 R>>endobj\nSECRET-MARKER-ORDER-PDF\n%%EOF\n";

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[
        8, 0, 0, 0, 0, 0, 0, 0, 0, b'S', b'E', b'C', b'R', b'E', b'T', b'-', b'P', b'N', b'G',
    ]);
    v
}

fn jpeg() -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16];
    v.extend_from_slice(b"JFIF\0SECRET-MARKER-ORDER-JPEG");
    v
}

/// A raw request (an upload, or anything that is not JSON).
async fn raw(
    t: &T,
    method: &str,
    path: &str,
    body: Vec<u8>,
    cookie: &str,
    filename: Option<&str>,
) -> (StatusCode, Vec<u8>) {
    let mut b = HttpRequest::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:8787")
        .header("x-nf-request", "1")
        .header("cookie", cookie);
    if let Some(f) = filename {
        b = b.header("x-filename", f);
    }
    let resp = router(t.app.clone())
        .oneshot(b.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let (parts, body) = resp.into_parts();
    let bytes = axum::body::to_bytes(body, 64 * 1024 * 1024).await.unwrap();
    (parts.status, bytes.to_vec())
}

async fn new_order(t: &T) -> i64 {
    let (st, v) = t
        .call(
            "POST",
            "/api/projects/1/orders",
            Some(json!({ "name": "Job 1" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    v["id"].as_i64().unwrap()
}

async fn add_file(t: &T, oid: i64, bytes: Vec<u8>, name: &str) -> (StatusCode, Value) {
    let (st, body) = raw(
        t,
        "POST",
        &format!("/api/orders/{oid}/files"),
        bytes,
        &t.cookie,
        Some(name),
    )
    .await;
    (st, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn order_with_parts(t: &T) -> i64 {
    let oid = new_order(t).await;
    for (b, n) in [(PDF_2_PAGES.to_vec(), "a.pdf"), (png(2480, 3508), "b.png")] {
        assert_eq!(add_file(t, oid, b, n).await.0, StatusCode::OK);
    }
    let (st, v) = t
        .call(
            "PUT",
            &format!("/api/orders/{oid}/parts"),
            Some(json!({ "parts": [
                { "no": "1", "name": "main", "shape": "cylinder", "qty": 2, "page": 1 },
                { "no": "2", "name": "branch", "shape": "tee_branch", "page": 1, "attach_part": "1" } ] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    oid
}

fn audit_details(t: &T, action: &str) -> Vec<String> {
    let db = t.app.db();
    let mut st = db
        .prepare("SELECT detail FROM audit WHERE action = ? ORDER BY id")
        .unwrap();
    st.query_map(params![action], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn decision(order: &Value, id: i64) -> &Value {
    order["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == id)
        .unwrap()
}

#[tokio::test]
async fn l1_create_upload_three_files_page_rows_and_encrypted_blobs() {
    let t = boot();
    let oid = new_order(&t).await;
    let (st, a) = add_file(&t, oid, PDF_2_PAGES.to_vec(), "a.pdf").await;
    assert_eq!(st, StatusCode::OK, "{a}");
    assert_eq!(a["pages"], json!([1, 2]));
    let (st, b) = add_file(&t, oid, png(2480, 3508), "b.png").await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["pages"], json!([3]));
    let (st, c) = add_file(&t, oid, jpeg(), "c.jpg").await;
    assert_eq!(st, StatusCode::OK, "{c}");
    assert_eq!(c["pages"], json!([4]));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["files"].as_array().unwrap().len(), 3);
    let pages = o["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 4);
    assert_eq!(pages[2]["width"], 2480);
    assert_eq!(pages[2]["has_image"], true, "a PNG is its own page image");
    assert_eq!(pages[0]["has_image"], false);
    assert_eq!(pages[0]["read_state"], "none");
    // the uploaded files are documents of the project as well
    assert_eq!(t.count("SELECT COUNT(*) FROM documents"), 3);
    // a PNG page is served, an unrendered page says so, an unknown page is 404
    let (st, _, bytes) = t
        .send("GET", &format!("/api/orders/{oid}/pages/3.png"), None, &[])
        .await;
    assert_eq!((st, bytes), (StatusCode::OK, png(2480, 3508)));
    let (st, v) = t
        .call("GET", &format!("/api/orders/{oid}/pages/1.png"), None)
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("page_image_pending"))
    );
    assert_eq!(
        t.call("GET", &format!("/api/orders/{oid}/pages/9.png"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // not an order file type
    let (st, v) = add_file(&t, oid, b"  0\nSECTION\n  2\nHEADER\n".to_vec(), "x.dxf").await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("order_file_type"))
    );
    // blobs on disk are encrypted: no plaintext marker in any of them
    let blobs: Vec<_> = std::fs::read_dir(t.dir.join("blobs"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    assert_eq!(blobs.len(), 3);
    for p in blobs {
        let b = std::fs::read(&p).unwrap();
        assert!(
            !b.windows(13).any(|w| w == b"SECRET-MARKER")
                && !b.windows(11).any(|w| w == b"SECRET-PNG-"[..11].as_ref())
                && !b.starts_with(b"%PDF"),
            "{} is not encrypted",
            p.display()
        );
    }
    assert_eq!(audit_details(&t, "order_file_added").len(), 3);
    assert!(audit_details(&t, "order_file_added")[0].contains("pages 1-2"));
    assert_eq!(
        t.call("POST", &format!("/api/orders/{oid}/pages/1/read"), None)
            .await
            .1["code"],
        "reader_not_installed"
    );
    assert_eq!(t.get("/api/audit/verify").await["intact"], true);
}

#[tokio::test]
async fn l1_roles_and_projects_are_enforced() {
    let t = boot();
    let oid = new_order(&t).await;
    let viewer = add_session(&t.app, "vic", "viewer");
    let member = add_session(&t.app, "eng", "engineer");
    let outsider = add_session(&t.app, "eve", "engineer");
    t.sql("INSERT INTO project_members(project_id, user_id) SELECT 1, id FROM users WHERE username IN ('vic','eng')");
    let with = |cookie: &str| [("cookie", cookie.to_string())];
    let call = |cookie: String, method: &'static str, path: String, body: Option<Value>| {
        let t = &t;
        async move {
            let c = cookie.clone();
            t.send(method, &path, body, &[("cookie", c.as_str())])
                .await
                .0
        }
    };
    let _ = with;
    let o = format!("/api/orders/{oid}");
    // the viewer reads, never writes
    assert_eq!(
        call(viewer.clone(), "GET", o.clone(), None).await,
        StatusCode::OK
    );
    assert_eq!(
        call(viewer.clone(), "GET", "/api/projects/1/orders".into(), None).await,
        StatusCode::OK
    );
    for (m, p, b) in [
        (
            "POST",
            "/api/projects/1/orders".to_string(),
            Some(json!({ "name": "x" })),
        ),
        ("PUT", format!("{o}/parts"), Some(json!({ "parts": [] }))),
        ("PUT", format!("{o}/values"), Some(json!({ "values": [] }))),
        ("POST", format!("{o}/confirm"), Some(json!({ "all": true }))),
        (
            "POST",
            format!("{o}/decisions"),
            Some(json!({ "kind": "mark_checked" })),
        ),
        ("POST", format!("{o}/pages/1/skip"), None),
    ] {
        assert_eq!(
            call(viewer.clone(), m, p.clone(), b).await,
            StatusCode::FORBIDDEN,
            "{m} {p}"
        );
    }
    assert_eq!(
        raw(
            &t,
            "POST",
            &format!("{o}/files"),
            PDF_2_PAGES.to_vec(),
            &viewer,
            Some("a.pdf")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    // a member who is an engineer may write
    assert_eq!(
        raw(
            &t,
            "POST",
            &format!("{o}/files"),
            PDF_2_PAGES.to_vec(),
            &member,
            Some("a.pdf")
        )
        .await
        .0,
        StatusCode::OK
    );
    // an engineer of another project sees nothing: 404 everywhere, and the refusals are audited
    for (m, p, b) in [
        ("GET", o.clone(), None),
        ("GET", format!("{o}/pages/1.png"), None),
        ("GET", "/api/projects/1/orders".to_string(), None),
        (
            "POST",
            "/api/projects/1/orders".to_string(),
            Some(json!({ "name": "x" })),
        ),
        ("PUT", format!("{o}/parts"), Some(json!({ "parts": [] }))),
        (
            "POST",
            format!("{o}/decisions"),
            Some(json!({ "kind": "mark_checked" })),
        ),
    ] {
        assert_eq!(
            call(outsider.clone(), m, p.clone(), b).await,
            StatusCode::NOT_FOUND,
            "{m} {p}"
        );
    }
    assert_eq!(
        raw(
            &t,
            "POST",
            &format!("{o}/files"),
            PDF_2_PAGES.to_vec(),
            &outsider,
            Some("a.pdf")
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM order_files"), 1);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'access_denied'") >= 5);
    // not signed in
    assert_eq!(
        call("none=1".into(), "GET", o.clone(), None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call("none=1".into(), "GET", "/api/orders/999".into(), None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        t.call("GET", "/api/orders/999", None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn l1_a_file_over_the_limit_is_413_and_nothing_is_kept() {
    let t = boot();
    let oid = new_order(&t).await;
    let mut big = b"%PDF-1.7\n".to_vec();
    big.resize(membrane::MAX_UPLOAD_BYTES + 1024 * 1024, 0);
    assert_eq!(big.len(), 51 * 1024 * 1024);
    let (st, _) = raw(
        &t,
        "POST",
        &format!("/api/orders/{oid}/files"),
        big,
        &t.cookie,
        Some("big.pdf"),
    )
    .await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(t.count("SELECT COUNT(*) FROM order_files"), 0);
    assert_eq!(t.count("SELECT COUNT(*) FROM documents"), 0);
    // an order has a file and page cap too
    assert_eq!(
        add_file(&t, oid, PDF_2_PAGES.to_vec(), "first.pdf").await.0,
        StatusCode::OK
    );
    t.sql(&format!("INSERT INTO order_pages(order_id, file_id, page_no, file_page, kind, read_state) WITH RECURSIVE n(i) AS (SELECT 3 UNION ALL SELECT i+1 FROM n WHERE i < {}) SELECT {oid}, 1, i, 1, 'scan', 'none' FROM n", crate::orders::MAX_ORDER_PAGES - 1));
    let (st, v) = add_file(&t, oid, PDF_2_PAGES.to_vec(), "a.pdf").await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("order_limit")),
        "{v}"
    );
    assert_eq!(
        add_file(&t, oid, png(10, 10), "p.png").await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn l1_state_survives_a_restart() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let (st, v) = t
        .call("PUT", &format!("/api/orders/{oid}/values"), Some(json!({ "values": [
            { "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "bbox": [10, 20, 60, 40] } ] })))
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let before = t.get(&format!("/api/orders/{oid}")).await;
    let app2 = build_app(config(&t.dir)).unwrap();
    let t2 = T {
        app: app2,
        dir: t.dir.clone(),
        cookie: t.cookie.clone(),
    };
    let after = t2.get(&format!("/api/orders/{oid}")).await;
    std::mem::forget(t2);
    assert_eq!(before, after);
    assert_eq!(after["pages"].as_array().unwrap().len(), 3);
    assert_eq!(
        after["parts"][0]["values"][0]["bbox"],
        json!([10.0, 20.0, 60.0, 40.0])
    );
}

#[tokio::test]
async fn l1_edits_are_audited_with_old_and_new_and_values_keep_provenance() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let put = |values: Value| {
        let t = &t;
        async move {
            t.call(
                "PUT",
                &format!("/api/orders/{oid}/values"),
                Some(json!({ "values": values })),
            )
            .await
        }
    };
    let (st, v) = put(
        json!([{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read" }]),
    )
    .await;
    assert_eq!((st, &v["changed"]), (StatusCode::OK, &json!(1)), "{v}");
    let (st, v) = put(json!([{ "part": 1, "key": "D", "value": 219.1, "text": "φ219.1", "source": "entered", "note": "typo on the scan" }])).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (_, v) = put(
        json!([{ "part": 1, "key": "D", "value": 219.1, "text": "φ219.1", "source": "entered" }]),
    )
    .await;
    assert_eq!(v["unchanged"], 1, "{v}");
    let a = audit_details(&t, "order_value_set");
    assert_eq!(a.len(), 2, "{a:?}");
    assert!(
        a[0].contains("old none") && a[0].contains("new 216.3") && a[0].contains("read PROPOSED"),
        "{}",
        a[0]
    );
    assert!(
        a[1].contains("old 216.3 [read PROPOSED]")
            && a[1].contains("new 219.1 [entered CORRECTED]")
            && a[1].contains("typo on the scan"),
        "{}",
        a[1]
    );
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let val = &o["parts"][0]["values"][0];
    assert_eq!(
        (
            val["value"].clone(),
            val["was"].clone(),
            val["proposed_value"].clone(),
            val["state"].clone()
        ),
        (json!(219.1), json!(216.3), json!(216.3), json!("CORRECTED"))
    );
    // parts too: change and exclusion are audited with the old state
    let (st, v) = t.call("PUT", &format!("/api/orders/{oid}/parts"), Some(json!({ "parts": [{ "id": 1, "no": "1", "name": "main", "shape": "cylinder", "qty": 3, "page": 1 }] }))).await;
    assert_eq!(
        (st, v["changed"].clone(), v["excluded"].clone()),
        (StatusCode::OK, json!(1), json!(1)),
        "{v}"
    );
    let c = audit_details(&t, "order_part_changed");
    assert!(
        c[0].contains("\"qty\":2") && c[0].contains("\"qty\":3"),
        "{c:?}"
    );
    assert!(audit_details(&t, "order_part_excluded")[0].contains("new state EXCLUDED"));
    // an excluded part takes no values; it comes back when it is listed again
    let (st, v) = put(json!([{ "part": 2, "key": "t", "value": 6, "source": "entered" }])).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("part_excluded"))
    );
    let (st, _) = t.call("PUT", &format!("/api/orders/{oid}/parts"), Some(json!({ "parts": [
        { "id": 1, "no": "1", "shape": "cylinder", "qty": 3 }, { "id": 2, "no": "2", "shape": "tee_branch", "attach_part": "1" } ] }))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        put(json!([{ "part": 2, "key": "t", "value": 6, "source": "entered" }]))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(t.get("/api/audit/verify").await["intact"], true);
}

#[tokio::test]
async fn l1_values_are_validated_text_must_hold_the_number() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let code = |values: Value| {
        let t = &t;
        async move {
            let (st, v) = t
                .call(
                    "PUT",
                    &format!("/api/orders/{oid}/values"),
                    Some(json!({ "values": values })),
                )
                .await;
            (st, v["code"].as_str().unwrap_or("").to_string())
        }
    };
    let bad = StatusCode::UNPROCESSABLE_ENTITY;
    assert_eq!(
        code(
            json!([{ "part": "1", "key": "D", "value": 500, "text": "φ216.3", "source": "read" }])
        )
        .await,
        (bad, "value_text_mismatch".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "D", "value": 216.3, "source": "drawing" }])).await,
        (bad, "value_text_missing".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "D", "value": 0, "source": "entered" }])).await,
        (bad, "value_not_positive".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "t", "value": -2, "source": "entered" }])).await,
        (bad, "value_not_positive".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "D", "value": "216", "source": "entered" }])).await,
        (bad, "value_number".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "D", "value": 1e300, "source": "entered" }])).await,
        (bad, "value_number".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "D", "value": 5, "source": "guess" }])).await,
        (bad, "value_source".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "D", "value": 5, "bbox": [5, 5, 1, 9] }])).await,
        (bad, "bbox_invalid".into())
    );
    assert_eq!(
        code(json!([{ "part": "1", "key": "D", "value": null, "confirm": true }])).await,
        (bad, "confirm_missing".into())
    );
    assert_eq!(
        code(json!([{ "part": "99", "key": "D", "value": 5 }])).await,
        (bad, "part_unknown".into())
    );
    // an angle may be zero or negative; full-width digits and a thousands comma are read as numbers
    assert_eq!(
        code(json!([{ "part": "1", "key": "angle", "value": 0 }]))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(code(json!([{ "part": "1", "key": "L", "value": 1250, "text": "L＝1,250", "source": "read" }])).await.0, StatusCode::OK);
    assert_eq!(code(json!([{ "part": "1", "key": "t", "value": 9, "text": "PL-9×200×300", "source": "table" }])).await.0, StatusCode::OK);
    // a bad entry anywhere in a batch changes nothing
    let n = t.count("SELECT COUNT(*) FROM order_values");
    assert_eq!(code(json!([{ "part": "1", "key": "R", "value": 5 }, { "part": "1", "key": "R2", "value": -1, "text": "" , "source": "read" }])).await.0, bad);
    assert_eq!(t.count("SELECT COUNT(*) FROM order_values"), n);
    // a missing value is an issue until it is entered
    assert_eq!(
        code(json!([{ "part": "1", "key": "R", "value": null }]))
            .await
            .0,
        StatusCode::OK
    );
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(
        o["issues"],
        json!([{ "code": "value_missing", "part": "1", "key": "R" }])
    );
    // wrong content type and bad json
    let (st, _, _) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            None,
            &[("content-type", "text/plain")],
        )
        .await;
    assert_eq!(st, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let (st, v) = t
        .call(
            "PUT",
            &format!("/api/orders/{oid}/parts"),
            Some(json!({ "parts": [{ "no": "7", "shape": "cylinder", "attach_part": "7" }] })),
        )
        .await;
    assert_eq!((st, v["code"].as_str()), (bad, Some("part_attach")), "{v}");
}

#[tokio::test]
async fn l1_a_decision_is_tied_to_its_value_and_can_be_undone() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let vals = format!("/api/orders/{oid}/values");
    let (st, _) = t.call("PUT", &vals, Some(json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read" }] }))).await;
    assert_eq!(st, StatusCode::OK);
    // automation confirms the value with its reason
    let dec = format!("/api/orders/{oid}/decisions");
    let (st, v) = t
        .call(
            "POST",
            &dec,
            Some(json!({ "kind": "confirm", "part": "1", "key": "D", "source": "auto" })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("decision_reason"))
    );
    let (st, d) = t.call("POST", &dec, Some(json!({ "kind": "confirm", "part": "1", "key": "D", "source": "auto", "agent": "operator-ai", "reason": "in range of the standard table" }))).await;
    assert_eq!(st, StatusCode::OK, "{d}");
    let did = d["id"].as_i64().unwrap();
    assert_eq!(
        (
            d["effective"].clone(),
            d["stale"].clone(),
            d["source"].clone(),
            d["decided_by"].clone()
        ),
        (json!(true), json!(false), json!("auto"), json!("admin"))
    );
    assert!(d["decided_at"].as_i64().unwrap() > 0);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["parts"][0]["values"][0]["state"], "CONFIRMED");
    assert_eq!(o["parts"][0]["state"], "CONFIRMED");
    // undo: the value goes back to its earlier state, the decision stays on record
    let (st, u) = t
        .call(
            "POST",
            &format!("{dec}/{did}/undo"),
            Some(json!({ "note": "wrong standard" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{u}");
    assert_eq!(
        (
            u["undone"].clone(),
            u["effective"].clone(),
            u["undone_by"].clone()
        ),
        (json!(true), json!(false), json!("admin"))
    );
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["parts"][0]["values"][0]["state"], "PROPOSED");
    assert_eq!(o["parts"][0]["state"], "DRAFT");
    assert_eq!(decision(&o, did)["undo_note"], "wrong standard");
    let (st, v) = t.call("POST", &format!("{dec}/{did}/undo"), None).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("already_undone"))
    );
    assert_eq!(
        t.call("POST", &format!("{dec}/999/undo"), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM order_decisions"),
        1,
        "undo keeps the record"
    );
    // a decision is never deleted, even by SQL
    assert!(
        t.app
            .db()
            .execute("DELETE FROM order_decisions", [])
            .is_err()
    );
    // staleness: confirm again, then the value changes
    let (_, d2) = t.call("POST", &dec, Some(json!({ "kind": "confirm", "part": "1", "key": "D", "reason": "checked on the scan" }))).await;
    let d2 = d2["id"].as_i64().unwrap();
    let (_, f) = t.call("POST", &dec, Some(json!({ "kind": "accept_flag", "part": "1", "key": "D", "flag": "bom_mismatch", "reason": "BOM rounds the mass" }))).await;
    let f = f["id"].as_i64().unwrap();
    let (_, k) = t
        .call("POST", &dec, Some(json!({ "kind": "mark_checked" })))
        .await;
    let k = k["id"].as_i64().unwrap();
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert!(
        [d2, f, k]
            .iter()
            .all(|i| decision(&o, *i)["effective"] == true),
        "{o}"
    );
    assert_eq!(o["checked"]["id"], k);
    let (st, _) = t.call("PUT", &vals, Some(json!({ "values": [{ "part": "1", "key": "D", "value": 219.1, "text": "φ219.1", "source": "entered" }] }))).await;
    assert_eq!(st, StatusCode::OK);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    for i in [d2, f, k] {
        let d = decision(&o, i);
        assert_eq!(
            (d["stale"].clone(), d["effective"].clone()),
            (json!(true), json!(false)),
            "decision {i} must be stale after the value changed: {d}"
        );
    }
    assert_eq!(
        o["parts"][0]["values"][0]["state"], "CORRECTED",
        "an old confirmation does not keep a changed value confirmed"
    );
    // undoing a stale confirmation does not touch the new value
    let (st, _) = t.call("POST", &format!("{dec}/{d2}/undo"), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        t.get(&format!("/api/orders/{oid}")).await["parts"][0]["values"][0]["state"],
        "CORRECTED"
    );
    // only a person marks an order checked
    let (st, v) = t
        .call(
            "POST",
            &dec,
            Some(json!({ "kind": "mark_checked", "source": "auto", "reason": "all in range" })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("decision_source"))
    );
    // a person confirms through /confirm: one decision per value, recorded under the person
    let (st, c) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/confirm"),
            Some(json!({ "all": true })),
        )
        .await;
    assert_eq!(
        (st, c["confirmed"].clone()),
        (StatusCode::OK, json!(1)),
        "{c}"
    );
    let a = audit_details(&t, "order_decision");
    assert!(
        a.iter().any(|x| x.contains("by auto (operator-ai)"))
            && a.iter().any(|x| x.contains("by person")),
        "{a:?}"
    );
    assert_eq!(audit_details(&t, "order_decision_undone").len(), 2);
}

#[tokio::test]
async fn l1_a_failed_page_can_be_skipped_and_the_skip_undone() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    t.sql(&format!(
        "UPDATE order_pages SET read_state = 'failed' WHERE order_id = {oid} AND page_no = 2"
    ));
    t.sql(&format!(
        "UPDATE order_pages SET read_state = 'proposed' WHERE order_id = {oid} AND page_no = 3"
    ));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["issues"], json!([{ "code": "page_failed", "page": 2 }]));
    let skip = |n: i64, body: Value| {
        let t = &t;
        async move {
            t.call(
                "POST",
                &format!("/api/orders/{oid}/pages/{n}/skip"),
                Some(body),
            )
            .await
        }
    };
    let (st, v) = skip(3, json!({})).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("page_not_skippable"))
    );
    assert_eq!(skip(9, json!({})).await.0, StatusCode::NOT_FOUND);
    let (st, v) = skip(2, json!({ "note": "blank back side" })).await;
    assert_eq!(
        (st, v["skipped"].clone()),
        (StatusCode::OK, json!(true)),
        "{v}"
    );
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(
        o["issues"],
        json!([]),
        "a skipped page no longer blocks the order"
    );
    assert_eq!(
        (
            o["pages"][1]["skipped"].clone(),
            o["pages"][1]["skipped_by"].clone()
        ),
        (json!(true), json!("admin"))
    );
    assert!(
        audit_details(&t, "order_page_skipped")[0]
            .contains("page 2 (read state failed) blank back side")
    );
    // a skipped page changes what a "checked" mark was made against
    let (_, k) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/decisions"),
            Some(json!({ "kind": "mark_checked" })),
        )
        .await;
    assert_eq!(k["effective"], true);
    let (st, v) = skip(2, json!({ "skipped": false })).await;
    assert_eq!((st, v["skipped"].clone()), (StatusCode::OK, json!(false)));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["issues"], json!([{ "code": "page_failed", "page": 2 }]));
    assert_eq!(o["checked"]["effective"], false);
    assert_eq!(audit_details(&t, "order_page_unskipped").len(), 1);
}

#[tokio::test]
async fn l1_licence_gate_covers_the_new_work_routes() {
    use axum::http::Method;
    // the read-only licence refuses new orders and order files (POST), not reads or edits of existing data
    assert!(crate::licence::gated(
        &Method::POST,
        "/api/projects/1/orders"
    ));
    assert!(crate::licence::gated(&Method::POST, "/api/orders/1/files"));
    assert!(!crate::licence::gated(&Method::GET, "/api/orders/1"));
    assert!(!crate::licence::gated(&Method::PUT, "/api/orders/1/values"));
}
