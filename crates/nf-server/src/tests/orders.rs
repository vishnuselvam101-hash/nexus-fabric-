//! NFJ-1 S1-L1: orders. Each test drives the real router against a private data directory.

use super::*;

const PDF_2_PAGES: &[u8] = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Count 2/Kids[3 0 R 4 0 R]>>endobj\n3 0 obj<</Type /Page/Parent 2 0 R>>endobj\n4 0 obj<</Type/Page/Parent 2 0 R>>endobj\nSECRET-MARKER-ORDER-PDF\n%%EOF\n";

pub(super) fn png(w: u32, h: u32) -> Vec<u8> {
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
    raw_with(t, method, path, body, cookie, filename, &[]).await
}

pub(super) async fn raw_with(
    t: &T,
    method: &str,
    path: &str,
    body: Vec<u8>,
    cookie: &str,
    filename: Option<&str>,
    extra: &[(&str, &str)],
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
    for (k, v) in extra {
        b = b.header(*k, *v);
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

pub(super) async fn order_with_parts(t: &T) -> i64 {
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

/// The AI operator's own identity: an engineer account of kind "agent" in project 1.
pub(super) fn add_agent(t: &T, name: &str) -> String {
    let c = add_session(&t.app, name, "engineer");
    t.sql(&format!(
        "UPDATE users SET kind = 'agent' WHERE username = '{name}'"
    ));
    t.sql(&format!("INSERT INTO project_members(project_id, user_id) SELECT 1, id FROM users WHERE username = '{name}'"));
    c
}

/// The operator events every write of the AI operator carries (one is enough for the server).
pub(super) fn with_ev(mut body: Value) -> Value {
    body["events"] = json!([{ "step": "type-1", "action": "type", "target": { "ui": "part-1-D" },
        "value": null, "reason": "typed from the page", "confidence": 0.97, "flags": [] }]);
    body
}

/// A reading the server's reader would have stored for a page of the order: the record an agent confirms against.
pub(super) fn add_reading(
    t: &T,
    oid: i64,
    text: &str,
    value: f64,
    flags: &[&str],
    conf: f64,
) -> i64 {
    let db = t.app.db();
    db.execute(
        "INSERT INTO order_readings(order_id, page_no, bbox_json, text, value, kind, flags_json, confidence, reader_id, model_sha256, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        params![oid, 1, "[10.0,20.0,80.0,40.0]", text, value, "number", json!(flags).to_string(), conf, "test reader", "0".repeat(64), "test", 0],
    )
    .unwrap();
    db.last_insert_rowid()
}

/// An order value the AI operator wrote and confirmed as a clean page reading (creates its own decision).
pub(super) async fn agent_clean_read(t: &T, ac: &str, oid: i64, key: &str, v: f64, text: &str) {
    let rid = add_reading(t, oid, text, v, &[], 0.97);
    let (st, _, b) = t
        .send(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(with_ev(json!({ "values": [{ "part": "1", "key": key, "value": v, "text": text, "source": "read",
                "flags": [], "confidence": 0.97, "confirm": true, "reading_id": rid, "note": "read from the page" }] }))),
            &[("cookie", ac)],
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
}

pub(super) fn audit_details(t: &T, action: &str) -> Vec<String> {
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
    // automation confirms the value with its reason, under its own service identity
    let dec = format!("/api/orders/{oid}/decisions");
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let (st, _, v) = t
        .send(
            "POST",
            &dec,
            Some(with_ev(
                json!({ "kind": "confirm", "part": "1", "key": "D" }),
            )),
            &ac,
        )
        .await;
    let v: Value = serde_json::from_slice(&v).unwrap();
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("agent_cannot_confirm"))
    );
    let (st, _, d) = t.send("POST", &dec, Some(with_ev(json!({ "kind": "confirm", "part": "1", "key": "D", "reason": "in range of the standard table" }))), &ac).await;
    let d: Value = serde_json::from_slice(&d).unwrap();
    assert_eq!(
        (st, d["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("agent_cannot_confirm"))
    );
    // a person confirms, under the person's identity
    let (st, d) = t.call("POST", &dec, Some(json!({ "kind": "confirm", "part": "1", "key": "D", "reason": "in range of the standard table" }))).await;
    assert_eq!(st, StatusCode::OK, "{d}");
    let did = d["id"].as_i64().unwrap();
    assert_eq!(
        (
            d["effective"].clone(),
            d["stale"].clone(),
            d["source"].clone(),
            d["decided_by"].clone()
        ),
        (json!(true), json!(false), json!("person"), json!("admin"))
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
        a.iter().all(|x| !x.contains("by auto")) && a.iter().any(|x| x.contains("by person")),
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
    assert!(o["checked"].is_null(), "a stale mark is not 'checked': {o}");
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
    assert!(!crate::licence::gated(&Method::PUT, "/api/orders/1"));
}

// ---------------------------------------------------------------- review round 1

async fn mark(t: &T, oid: i64) -> i64 {
    let (st, k) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/decisions"),
            Some(json!({ "kind": "mark_checked" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{k}");
    k["id"].as_i64().unwrap()
}

async fn read_val(t: &T, oid: i64, key: &str, v: f64, text: &str) {
    let (st, r) = t
        .call(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(json!({ "values": [{ "part": "1", "key": key, "value": v, "text": text, "source": "read" }] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
}

#[tokio::test]
async fn l1r1_the_source_of_a_decision_comes_from_the_identity() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    read_val(&t, oid, "D", 216.3, "φ216.3").await;
    let dec = format!("/api/orders/{oid}/decisions");
    // a person cannot claim to be automation or a rule, and cannot name an agent
    for claim in ["auto", "rule"] {
        let (st, v) = t.call("POST", &dec, Some(json!({ "kind": "confirm", "part": "1", "key": "D", "source": claim, "reason": "x" }))).await;
        assert_eq!(
            (st, v["code"].as_str()),
            (StatusCode::UNPROCESSABLE_ENTITY, Some("decision_source")),
            "{v}"
        );
    }
    let (st, d) = t
        .call(
            "POST",
            &dec,
            Some(json!({ "kind": "confirm", "part": "1", "key": "D", "agent": "someone" })),
        )
        .await;
    assert_eq!(
        (st, d["source"].clone(), d["agent"].clone()),
        (StatusCode::OK, json!("person"), Value::Null),
        "{d}"
    );
    // the agent cannot claim to be a person, and cannot mark checked
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let (st, _, b) = t
        .send(
            "POST",
            &dec,
            Some(with_ev(json!({ "kind": "mark_checked" }))),
            &ac,
        )
        .await;
    let b: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(
        (st, b["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("agent_cannot_check")),
        "{b}"
    );
    let (st, _, b) = t.send("POST", &dec, Some(with_ev(json!({ "kind": "accept_flag", "part": "1", "key": "D", "flag": "f", "reason": "r", "source": "person" }))), &ac).await;
    let b: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(
        (st, b["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("decision_source")),
        "{b}"
    );
    // bulk confirm and the confirm flag of a value edit follow the identity too, and the agent must give a reason
    read_val(&t, oid, "t", 6.0, "t6").await;
    let confirm = format!("/api/orders/{oid}/confirm");
    for body in [
        json!({ "all": true }),
        json!({ "all": true, "note": "in range" }),
    ] {
        let (st, _, b) = t.send("POST", &confirm, Some(body), &ac).await;
        let b: Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(
            (st, b["code"].as_str()),
            (StatusCode::FORBIDDEN, Some("agent_cannot_confirm")),
            "{b}"
        );
    }
    let n = t.count("SELECT COUNT(*) FROM order_decisions WHERE source = 'auto'");
    assert_eq!(n, 0, "the operator made no confirmation");
    assert_eq!(
        t.count(
            "SELECT COUNT(*) FROM order_decisions WHERE source = 'person' AND decided_by = 'admin'"
        ),
        1
    );
}

#[tokio::test]
async fn l1r1_a_checked_mark_goes_stale_on_files_pages_and_value_changes() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    read_val(&t, oid, "D", 216.3, "φ216.3").await;
    let dec = format!("/api/orders/{oid}/decisions");
    let effective = |o: &Value, k: i64| decision(o, k)["effective"] == true;
    // a new upload
    let k = mark(&t, oid).await;
    assert_eq!(add_file(&t, oid, jpeg(), "c.jpg").await.0, StatusCode::OK);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert!(!effective(&o, k), "a new file must make the mark stale");
    assert!(o["checked"].is_null());
    // a confirmation made, then undone
    let k = mark(&t, oid).await;
    let (_, c) = t
        .call(
            "POST",
            &dec,
            Some(json!({ "kind": "confirm", "part": "1", "key": "D" })),
        )
        .await;
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert!(
        !effective(&o, k),
        "a later confirmation changes what was checked"
    );
    let k = mark(&t, oid).await;
    let (st, _) = t
        .call("POST", &format!("{dec}/{}/undo", c["id"]), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        !effective(&t.get(&format!("/api/orders/{oid}")).await, k),
        "an undone confirmation"
    );
    // source and box
    let k = mark(&t, oid).await;
    let (st, _) = t.call("PUT", &format!("/api/orders/{oid}/values"), Some(json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "bbox": [1, 2, 30, 40] }] }))).await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        !effective(&t.get(&format!("/api/orders/{oid}")).await, k),
        "a moved box"
    );
    let k = mark(&t, oid).await;
    let (st, _) = t.call("PUT", &format!("/api/orders/{oid}/values"), Some(json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "drawing", "bbox": [1, 2, 30, 40] }] }))).await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        !effective(&t.get(&format!("/api/orders/{oid}")).await, k),
        "a changed source"
    );
}

#[tokio::test]
async fn l1r1_a_confirmation_does_not_survive_a_changed_source_or_box() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    read_val(&t, oid, "D", 216.3, "φ216.3").await;
    let dec = format!("/api/orders/{oid}/decisions");
    let (_, c) = t
        .call(
            "POST",
            &dec,
            Some(json!({ "kind": "confirm", "part": "1", "key": "D" })),
        )
        .await;
    let id = c["id"].as_i64().unwrap();
    let (st, _) = t.call("PUT", &format!("/api/orders/{oid}/values"), Some(json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "bbox": [5, 5, 50, 50] }] }))).await;
    assert_eq!(st, StatusCode::OK);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["parts"][0]["values"][0]["state"], "PROPOSED");
    assert_eq!(decision(&o, id)["effective"], false, "{o}");
    // confirm again on the new box, then change only the source: that confirmation stops counting too
    let (_, c2) = t
        .call(
            "POST",
            &dec,
            Some(json!({ "kind": "confirm", "part": "1", "key": "D" })),
        )
        .await;
    let id2 = c2["id"].as_i64().unwrap();
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(decision(&o, id2)["effective"], true, "{o}");
    let (st, _) = t.call("PUT", &format!("/api/orders/{oid}/values"), Some(json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "drawing", "bbox": [5, 5, 50, 50] }] }))).await;
    assert_eq!(st, StatusCode::OK);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(o["parts"][0]["values"][0]["state"], "PROPOSED");
    assert_eq!(decision(&o, id2)["effective"], false, "{o}");
}

#[tokio::test]
async fn l1r1_a_second_confirm_is_refused_and_undo_keeps_a_counting_confirm() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    read_val(&t, oid, "D", 216.3, "φ216.3").await;
    let dec = format!("/api/orders/{oid}/decisions");
    let body = json!({ "kind": "confirm", "part": "1", "key": "D" });
    let (_, a) = t.call("POST", &dec, Some(body.clone())).await;
    let (st, v) = t.call("POST", &dec, Some(body.clone())).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("already_confirmed")),
        "{v}"
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM order_decisions"), 1);
    // change the value and back: the first decision counts again; confirm again, then undo the first
    let vals = format!("/api/orders/{oid}/values");
    read_val(&t, oid, "D", 219.1, "φ219.1").await;
    assert_eq!(
        decision(
            &t.get(&format!("/api/orders/{oid}")).await,
            a["id"].as_i64().unwrap()
        )["effective"],
        false
    );
    read_val(&t, oid, "D", 216.3, "φ216.3").await;
    let _ = vals;
    let (st, b2) = t.call("POST", &dec, Some(body)).await;
    assert_eq!(st, StatusCode::OK, "{b2}");
    let (st, _) = t
        .call("POST", &format!("{dec}/{}/undo", a["id"]), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(
        o["parts"][0]["values"][0]["state"], "CONFIRMED",
        "the second confirmation still counts: {o}"
    );
    assert_eq!(decision(&o, b2["id"].as_i64().unwrap())["effective"], true);
}

#[tokio::test]
async fn l1r1_parts_and_values_are_capped_counting_excluded_rows() {
    let t = boot();
    let oid = new_order(&t).await;
    let put = |nos: Vec<String>| {
        let parts: Vec<Value> = nos
            .iter()
            .map(|n| json!({ "no": n, "shape": "cylinder" }))
            .collect();
        json!({ "parts": parts })
    };
    let path = format!("/api/orders/{oid}/parts");
    let (st, v) = t
        .call(
            "PUT",
            &path,
            Some(put((0..500).map(|i| format!("a{i}")).collect())),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    // a list of one new part excludes the other 500; the rows stay, so the cap holds
    let (st, v) = t.call("PUT", &path, Some(put(vec!["b0".into()]))).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("parts_limit")),
        "{v}"
    );
    assert_eq!(
        t.count(&format!(
            "SELECT COUNT(*) FROM order_parts WHERE order_id = {oid}"
        )),
        500
    );
    // values per part
    let oid = order_with_parts(&t).await;
    let vals = format!("/api/orders/{oid}/values");
    for batch in 0..2 {
        let list: Vec<Value> = (0..40).map(|i| json!({ "part": "1", "key": format!("k{batch}_{i}"), "value": 1, "source": "entered" })).collect();
        let (st, v) = t.call("PUT", &vals, Some(json!({ "values": list }))).await;
        if batch == 0 {
            assert_eq!(st, StatusCode::OK, "{v}");
        } else {
            assert_eq!(
                (st, v["code"].as_str()),
                (StatusCode::CONFLICT, Some("values_limit")),
                "{v}"
            );
        }
    }
    assert_eq!(t.count(&format!("SELECT COUNT(*) FROM order_values WHERE part_id = (SELECT id FROM order_parts WHERE order_id = {oid} AND no = '1')")), 40);
}

#[tokio::test]
async fn l1r1_rename_to_an_unlisted_part_no_is_409_and_refusals_are_audited() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let id1 = o["parts"][0]["id"].as_i64().unwrap();
    // part 1 is renamed "2", while part "2" is left out of the list (to be excluded)
    let (st, v) = t
        .call(
            "PUT",
            &format!("/api/orders/{oid}/parts"),
            Some(json!({ "parts": [{ "id": id1, "no": "2", "shape": "cylinder" }] })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("part_no_exists")),
        "{v}"
    );
    // a file of a type that is no order file, and the file limit
    let before = audit_details(&t, "order_file_rejected").len();
    let (st, _) = add_file(&t, oid, b"MZ\x90\x00\x03\x00\x00\x00".to_vec(), "x.exe").await;
    assert_ne!(st, StatusCode::OK);
    t.sql(&format!("INSERT INTO order_pages(order_id, file_id, page_no, file_page, kind, read_state) SELECT {oid}, 1, 100 + value, 1, 'scan', 'none' FROM (WITH RECURSIVE c(value) AS (SELECT 1 UNION ALL SELECT value + 1 FROM c WHERE value < 300) SELECT value FROM c)"));
    let (st, v) = add_file(&t, oid, png(10, 10), "d.png").await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("order_limit")),
        "{v}"
    );
    let a = audit_details(&t, "order_file_rejected");
    assert!(a.iter().any(|x| x.contains("order_limit")), "{a:?}");
    assert!(a.len() > before);
}

#[tokio::test]
async fn l1r1_numbers_keep_their_sign() {
    for (text, v, want) in [
        ("5", -5.0, false),
        ("-5", -5.0, true),
        ("－5", -5.0, true),
        ("−5.5", -5.5, true),
        ("-5", 5.0, false),
        ("PL-9×200", 9.0, true),
        ("100-200", 200.0, true),
        ("φ216.3", 216.3, true),
        ("L＝1,250", 1250.0, true),
    ] {
        assert_eq!(
            crate::orders::text_contains_number(text, v),
            want,
            "{text} / {v}"
        );
    }
}

#[tokio::test]
async fn l1r1_a_read_only_licence_blocks_every_order_write() {
    use axum::http::Method;
    for (m, p) in [
        (Method::PUT, "/api/orders/1/parts"),
        (Method::PUT, "/api/orders/1/values"),
        (Method::POST, "/api/orders/1/confirm"),
        (Method::POST, "/api/orders/1/compute"),
        (Method::POST, "/api/orders/1/decisions"),
        (Method::POST, "/api/orders/1/decisions/2/undo"),
        (Method::POST, "/api/orders/1/pages/3/skip"),
        (Method::POST, "/api/orders/1/pages/3/read"),
        (Method::POST, "/api/orders/1/files"),
        (Method::POST, "/api/projects/1/orders"),
    ] {
        assert!(crate::licence::gated(&m, p), "{m} {p}");
    }
    assert!(!crate::licence::gated(&Method::GET, "/api/orders/1"));
}

#[tokio::test]
async fn l1r1_the_ai_operator_gives_a_reason_for_every_order_write() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let code = |b: &[u8]| serde_json::from_slice::<Value>(b).unwrap()["code"].clone();
    // a value edit, a parts edit and a page skip without a reason are refused
    let vals = format!("/api/orders/{oid}/values");
    let (st, _, b) = t
        .send("PUT", &vals, Some(json!({ "values": [{ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read" }] })), &ac)
        .await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason"))
    );
    let parts = format!("/api/orders/{oid}/parts");
    let (st, _, b) = t
        .send(
            "PUT",
            &parts,
            Some(json!({ "parts": [{ "no": "1", "shape": "cylinder" }] })),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason"))
    );
    let (st, _, b) = t
        .send(
            "POST",
            &format!("/api/orders/{oid}/pages/1/skip"),
            Some(json!({})),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason"))
    );
    // a decision that the operator later undoes (its own: it made this one by a clean confirmed reading)
    let dec = format!("/api/orders/{oid}/decisions");
    let (st, _, b) = t
        .send("POST", &dec, Some(with_ev(json!({ "kind": "accept_flag", "part": "1", "key": "D", "flag": "f", "reason": "r" }))), &ac)
        .await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::FORBIDDEN, json!("agent_cannot_confirm"))
    );
    agent_clean_read(&t, &agent, oid, "D", 216.3, "φ216.3").await;
    let did = t.count("SELECT id FROM order_decisions WHERE agent = 'operator-ai'");
    // with a reason they go through, and the parts reason is audited
    let (st, _, b) = t
        .send("PUT", &vals, Some(with_ev(json!({ "values": [{ "part": "1", "key": "L", "value": 216.3, "text": "φ216.3", "source": "read", "note": "box 3" }] }))), &ac)
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let (st, _, b) = t
        .send("PUT", &parts, Some(with_ev(json!({ "parts": [
            { "id": 1, "no": "1", "name": "main", "shape": "cylinder", "qty": 2, "page": 1 },
            { "id": 2, "no": "2", "name": "branch", "shape": "tee_branch", "page": 1, "attach_part": "1" },
            { "no": "9", "shape": "cylinder", "qty": 2 }], "note": "qty per drawing" }))), &ac)
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    // the refused edit above (no reason) and this accepted one each keep their reason row
    assert_eq!(audit_details(&t, "order_parts_reason").len(), 2);
    let undo = format!("{dec}/{did}/undo");
    // an undo by the operator needs a reason as well
    let (st, _, b) = t.send("POST", &undo, Some(json!({})), &ac).await;
    assert_eq!(
        (st, code(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason"))
    );
    let (st, _, b) = t
        .send("POST", &undo, Some(json!({ "note": "wrong flag" })), &ac)
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
}

#[tokio::test]
async fn aiop_a_confirmed_page_reading_must_be_clean_and_confident() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    let vals = format!("/api/orders/{oid}/values");
    let value = |flags: Option<Value>, confidence: Option<Value>| {
        let mut v = json!({ "part": "1", "key": "D", "value": 216.3, "text": "φ216.3", "source": "read", "confirm": true, "note": "matches the list" });
        if let Some(f) = flags {
            v["flags"] = f;
        }
        if let Some(c) = confidence {
            v["confidence"] = c;
        }
        with_ev(json!({ "values": [v] }))
    };
    for (want, body) in [
        ("confirm_needs_reading_state", value(None, None)),
        (
            "confirm_needs_reading_state",
            value(Some(Value::Null), Some(json!(0.97))),
        ),
        (
            "confirm_needs_reading_state",
            value(Some(json!([])), Some(Value::Null)),
        ),
        (
            "confirm_needs_reading_state",
            value(Some(Value::Null), Some(Value::Null)),
        ),
        (
            "value_flagged",
            value(Some(json!(["repaired"])), Some(json!(0.97))),
        ),
        (
            "value_low_confidence",
            value(Some(json!([])), Some(json!(0.60))),
        ),
    ] {
        let (st, _, b) = t.send("PUT", &vals, Some(body), &ac).await;
        assert_eq!(
            (st, code_of(&b)),
            (StatusCode::UNPROCESSABLE_ENTITY, json!(want)),
            "{}",
            String::from_utf8_lossy(&b)
        );
    }
    let rid = add_reading(&t, oid, "φ216.3", 216.3, &[], 0.97);
    let mut body = value(Some(json!([])), Some(json!(0.97)));
    body["values"][0]["reading_id"] = json!(rid);
    let (st, _, b) = t.send("PUT", &vals, Some(body), &ac).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let got = o["parts"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == "D")
        .unwrap();
    assert_eq!(got["state"], json!("CONFIRMED"));
    // a derived number is not a reading: it needs no reader state to be confirmed by a person, and the AI operator
    // may not confirm it at all (it is proposed; a person decides)
    let (st, _, b) = t
        .send("PUT", &vals, Some(with_ev(json!({ "values": [{ "part": "1", "key": "t", "value": 8.0, "text": "8", "source": "derived", "confirm": true, "note": "from the rule" }] }))), &ac)
        .await;
    assert_eq!(
        (st, code_of(&b)),
        (StatusCode::FORBIDDEN, json!("agent_cannot_confirm")),
        "{}",
        String::from_utf8_lossy(&b)
    );
    let (st, r) = t.call("PUT", &vals, Some(json!({ "values": [{ "part": "1", "key": "t", "value": 8.0, "text": "8", "source": "derived", "confirm": true, "note": "from the rule" }] }))).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    // flags and low confidence forbid confirmation whatever the source is called: an agent must
    // not be able to relabel a flagged reading `derived` and slip it through the boundary
    for source in ["derived", "inferred", "fixed", "auto", "entered"] {
        for (want, flags, confidence) in [
            ("value_flagged", json!(["ambiguous"]), 0.97),
            ("value_low_confidence", json!([]), 0.60),
        ] {
            let body = with_ev(
                json!({ "values": [{ "part": "1", "key": "t", "value": 8.0, "text": "8", "source": source, "confirm": true, "flags": flags, "confidence": confidence, "note": "relabelled" }] }),
            );
            let (st, _, b) = t.send("PUT", &vals, Some(body), &ac).await;
            assert_eq!(
                (st, code_of(&b)),
                (StatusCode::UNPROCESSABLE_ENTITY, json!(want)),
                "{}",
                String::from_utf8_lossy(&b)
            );
        }
    }
}

fn code_of(b: &[u8]) -> Value {
    serde_json::from_slice::<Value>(b).unwrap()["code"].clone()
}

#[tokio::test]
async fn l1r2_limit_refusals_are_audited_and_leave_no_partial_write() {
    let t = boot();
    let oid = new_order(&t).await;
    let ac = add_agent(&t, "operator-ai");
    let ac = [("cookie", ac.as_str())];
    // a batch over the limit is refused before anything is read, and the refusal is on record
    let parts_path = format!("/api/orders/{oid}/parts");
    let big: Vec<Value> = (0..501)
        .map(|i| json!({ "no": format!("p{i}"), "shape": "cylinder" }))
        .collect();
    let (st, _, b) = t
        .send(
            "PUT",
            &parts_path,
            Some(with_ev(json!({ "parts": big, "note": "bulk" }))),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code_of(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("parts_limit"))
    );
    let r = audit_details(&t, "order_refused");
    assert!(
        r.iter()
            .any(|x| x.contains("parts_limit") && x.contains("bulk")),
        "{r:?}"
    );
    // 500 parts by the operator, then one more: refused in the request's transaction, which rolls back and is audited
    let parts500: Vec<Value> = (0..500)
        .map(|i| json!({ "no": format!("p{i}"), "shape": "cylinder" }))
        .collect();
    let (st, _, b) = t
        .send(
            "PUT",
            &parts_path,
            Some(with_ev(json!({ "parts": parts500, "note": "bulk load" }))),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let (st, _, b) = t
        .send(
            "PUT",
            &parts_path,
            Some(with_ev(
                json!({ "parts": [{ "no": "extra", "shape": "cylinder" }], "note": "one more" }),
            )),
            &ac,
        )
        .await;
    assert_eq!(
        (st, code_of(&b)),
        (StatusCode::CONFLICT, json!("parts_limit"))
    );
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(
        o["parts"].as_array().unwrap().len(),
        500,
        "the refused part was not written"
    );
    let refusals = audit_details(&t, "order_parts_reason");
    assert!(
        refusals
            .iter()
            .any(|x| x.contains("refused (parts_limit)") && x.contains("one more")),
        "{refusals:?}"
    );
    assert!(
        audit_details(&t, "order_refused")
            .iter()
            .any(|x| x.contains("parts_limit (409)"))
    );
    // values: 64 keys on one part, then a batch whose first edit is valid and whose second is one key too many
    let vals = format!("/api/orders/{oid}/values");
    let sixty4: Vec<Value> = (0..64).map(|i| json!({ "part": "p0", "key": format!("k{i:02}"), "value": 1.0, "source": "entered", "note": "fill" })).collect();
    let (st, _, b) = t
        .send(
            "PUT",
            &vals,
            Some(with_ev(json!({ "values": sixty4 }))),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let (st, _, b) = t
        .send("PUT", &vals, Some(with_ev(json!({ "values": [
            { "part": "p0", "key": "k00", "value": 2.0, "source": "entered", "note": "edit" },
            { "part": "p0", "key": "extra", "value": 3.0, "source": "entered", "note": "edit" } ] }))), &ac)
        .await;
    assert_eq!(
        (st, code_of(&b)),
        (StatusCode::CONFLICT, json!("values_limit"))
    );
    let o = t.get(&format!("/api/orders/{oid}")).await;
    let k00 = o["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["no"] == "p0")
        .unwrap()["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["key"] == "k00")
        .unwrap()["value"]
        .clone();
    assert_eq!(
        k00,
        json!(1.0),
        "the earlier edit of the refused batch was rolled back"
    );
    assert!(
        audit_details(&t, "order_refused")
            .iter()
            .any(|x| x.contains("values_limit (409)"))
    );
    // a batch of values over the limit: refused before anything is read
    let many: Vec<Value> = (0..501)
        .map(|_| json!({ "part": "p0", "key": "k00", "value": 1.0, "source": "entered" }))
        .collect();
    let (st, _, b) = t
        .send("PUT", &vals, Some(json!({ "values": many })), &ac)
        .await;
    assert_eq!(
        (st, code_of(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("values_limit"))
    );
    assert!(
        audit_details(&t, "order_refused")
            .iter()
            .any(|x| x.contains("values_limit (422)"))
    );
}

#[tokio::test]
async fn l1r2_the_ai_operator_gives_a_reason_on_create_and_upload() {
    let t = boot();
    let ac = add_agent(&t, "operator-ai");
    let ac = [("cookie", ac.as_str())];
    let path = "/api/projects/1/orders";
    let (st, _, b) = t
        .send("POST", path, Some(json!({ "name": "Job A" })), &ac)
        .await;
    assert_eq!(
        (st, code_of(&b)),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("decision_reason"))
    );
    let (st, _, b) = t
        .send(
            "POST",
            path,
            Some(json!({ "name": "Job A", "reason": "from the customer PO" })),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    let oid = serde_json::from_slice::<Value>(&b).unwrap()["id"]
        .as_i64()
        .unwrap();
    assert!(
        audit_details(&t, "order_created")
            .iter()
            .any(|x| x.contains("reason: from the customer PO"))
    );
    // a person creates an order without a reason: persons stay optional
    let (st, _) = t.call("POST", path, Some(json!({ "name": "Job B" }))).await;
    assert_eq!(st, StatusCode::OK);
    // the upload without a reason is refused; with ?reason= it goes through
    let up = format!("/api/orders/{oid}/files");
    let (st, body) = raw(
        &t,
        "POST",
        &up,
        PDF_2_PAGES.to_vec(),
        ac[0].1,
        Some("a.pdf"),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(code_of(&body), json!("decision_reason"));
    // the refusal is on record, in its own transaction
    assert_eq!(
        audit_details(&t, "order_refused")
            .iter()
            .filter(|x| x.contains("file upload refused: decision_reason"))
            .count(),
        1
    );
    let ev = hex::encode(with_ev(json!({}))["events"].to_string());
    let (st, body) = raw_with(
        &t,
        "POST",
        &format!("{up}?reason=new-scan"),
        PDF_2_PAGES.to_vec(),
        ac[0].1,
        Some("a.pdf"),
        &[("x-operator-events", ev.as_str())],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert!(
        audit_details(&t, "order_file_added")
            .iter()
            .any(|x| x.contains("reason: new-scan"))
    );
    // a person uploads without one
    let (st, _) = add_file(&t, oid, PDF_2_PAGES.to_vec(), "b.pdf").await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn l1r2_the_write_checks_role_and_active_again_in_the_transaction() {
    let t = boot();
    let _ = add_session(&t.app, "eve", "engineer");
    let uid: i64 = t
        .app
        .db()
        .query_row("SELECT id FROM users WHERE username = 'eve'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let who = crate::User {
        id: uid,
        username: "eve".into(),
        role: "engineer".into(),
        kind: "person".into(),
    };
    let status = |r: Result<(), crate::ApiError>| match r {
        Err(crate::ApiError::Resp(resp)) => Some(resp.status()),
        _ => None,
    };
    let check = |t: &T| {
        let db = t.app.db();
        status(crate::orders::still_writer(&db, &who))
    };
    assert_eq!(check(&t), None);
    t.sql("UPDATE users SET role = 'viewer' WHERE username = 'eve'");
    assert_eq!(check(&t), Some(StatusCode::FORBIDDEN));
    t.sql("UPDATE users SET role = 'engineer', active = 0 WHERE username = 'eve'");
    assert_eq!(check(&t), Some(StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn l1r2_an_ai_operator_undoes_only_its_own_decisions() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    let agent = add_agent(&t, "operator-ai");
    let ac = [("cookie", agent.as_str())];
    read_val(&t, oid, "D", 216.3, "φ216.3").await;
    let dec = format!("/api/orders/{oid}/decisions");
    let (st, c) = t
        .call(
            "POST",
            &dec,
            Some(json!({ "kind": "confirm", "part": "1", "key": "D" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{c}");
    let person_confirm = c["id"].as_i64().unwrap();
    let checked = mark(&t, oid).await;
    // the operator's own decision: a clean confirmed reading of another value
    agent_clean_read(&t, &agent, oid, "t", 6.0, "t6").await;
    let own = t.count("SELECT id FROM order_decisions WHERE agent = 'operator-ai'");
    for (id, what) in [
        (person_confirm, "a person's confirmation"),
        (checked, "a checked mark"),
    ] {
        let (st, _, b) = t
            .send(
                "POST",
                &format!("{dec}/{id}/undo"),
                Some(json!({ "note": "try" })),
                &ac,
            )
            .await;
        assert_eq!(
            (st, code_of(&b)),
            (StatusCode::FORBIDDEN, json!("agent_cannot_undo")),
            "{what}"
        );
    }
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(decision(&o, person_confirm)["undone"], false);
    assert_eq!(decision(&o, checked)["undone"], false);
    let (st, _, b) = t
        .send(
            "POST",
            &format!("{dec}/{own}/undo"),
            Some(json!({ "note": "wrong flag" })),
            &ac,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
}

#[tokio::test]
async fn l1r2_an_accepted_flag_takes_a_checked_mark_out_of_force() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    read_val(&t, oid, "D", 216.3, "φ216.3").await;
    let m = mark(&t, oid).await;
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(decision(&o, m)["effective"], true, "{o}");
    let (st, _) = t
        .call("POST", &format!("/api/orders/{oid}/decisions"), Some(json!({ "kind": "accept_flag", "part": "1", "key": "D", "flag": "bom_mismatch", "reason": "agreed with the shop" })))
        .await;
    assert_eq!(st, StatusCode::OK);
    let o = t.get(&format!("/api/orders/{oid}")).await;
    assert_eq!(
        decision(&o, m)["effective"],
        false,
        "the mark was made before the flag was accepted: {o}"
    );
    assert!(o["checked"].is_null(), "{o}");
}

#[tokio::test]
async fn l1r2_a_comma_number_is_one_ambiguous_reading_not_two() {
    use crate::orders::text_contains_number as has;
    assert!(!has("1,5", 15.0), "not the grouped reading");
    assert!(
        !has("1,5", 1.5),
        "an ambiguous reading does not count as on the page"
    );
    assert!(has("1,500", 1500.0));
    assert!(has("12,345.6", 12345.6));
    assert!(has("1.5", 1.5));
    let t = boot();
    let oid = order_with_parts(&t).await;
    let vals = format!("/api/orders/{oid}/values");
    let put = |v: f64, text: &str| json!({ "values": [{ "part": "1", "key": "L", "value": v, "text": text, "source": "read" }] });
    let (st, _, b) = t.send("PUT", &vals, Some(put(1.5, "1,5")), &[]).await;
    assert_eq!(
        (st, code_of(&b)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("value_text_ambiguous")
        ),
        "{}",
        String::from_utf8_lossy(&b)
    );
    let (st, _, b) = t.send("PUT", &vals, Some(put(15.0, "1,5")), &[]).await;
    assert_eq!(
        (st, code_of(&b)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("value_text_mismatch")
        )
    );
    let (st, _, b) = t.send("PUT", &vals, Some(put(1500.0, "1,500")), &[]).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
}

#[tokio::test]
async fn l1r2_a_persons_note_and_the_refusals_are_audited() {
    let t = boot();
    let oid = order_with_parts(&t).await;
    // a person's parts note is on record
    let (st, _) = t
        .call("PUT", &format!("/api/orders/{oid}/parts"), Some(json!({ "parts": [{ "id": 1, "no": "1", "shape": "cylinder", "qty": 3, "page": 1 }, { "id": 2, "no": "2", "shape": "tee_branch", "page": 1, "attach_part": "1" }], "note": "per drawing rev B" })))
        .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(audit_details(&t, "order_parts_reason").len(), 1);
    assert!(audit_details(&t, "order_parts_reason")[0].contains("per drawing rev B"));
    // a skip of a page that has not failed is refused (409); the refusal keeps the note
    t.sql("UPDATE order_pages SET read_state = 'ok' WHERE order_id = 1 AND page_no = 1");
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/pages/1/skip"),
            Some(json!({ "note": "skip reason" })),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    let r = audit_details(&t, "order_refused");
    assert!(
        r.iter()
            .any(|x| x.contains("page_not_skippable") && x.contains("skip reason")),
        "{r:?}"
    );
    // a skip that changes nothing still records the person's note
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/pages/2/skip"),
            Some(json!({ "skipped": false, "note": "kept" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        audit_details(&t, "order_page_note")
            .iter()
            .any(|x| x.contains("kept"))
    );
    // a read of a page is refused by this build and that refusal is on record
    let (st, _) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/pages/1/read"),
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(audit_details(&t, "order_page_read_refused").len(), 1);
}

// ---------------------------------------------------------------- NFJ-1 L10: compute

async fn put_parts_values(t: &T, oid: i64, parts: Value, values: Value) {
    let (st, v) = t
        .call(
            "PUT",
            &format!("/api/orders/{oid}/parts"),
            Some(json!({ "parts": parts })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, v) = t
        .call(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(json!({ "values": values })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
}

fn val(part: &str, key: &str, v: f64) -> Value {
    json!({ "part": part, "key": key, "value": v, "text": format!("{v}"), "source": "entered" })
}

#[tokio::test]
async fn l10_compute_holds_missing_values_and_edits_supersede() {
    let t = boot();
    let oid = new_order(&t).await;
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
    // nothing is confirmed yet: every value is PROPOSED or entered-as-proposed, so nothing may be computed
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let id0 = v["id"].as_i64().unwrap();
    let c1 = v["result"]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["no"] == "C1")
        .unwrap()
        .clone();
    assert_ne!(
        c1["state"], "computed",
        "an unconfirmed value is never computed: {c1}"
    );
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/confirm"),
            Some(json!({ "all": true })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, first) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{first}");
    let parts = first["result"]["parts"].as_array().unwrap();
    let get = |no: &str| parts.iter().find(|p| p["no"] == no).unwrap().clone();
    // the cylinder is computed (pattern + mass), the plate misses plate_H: HOLD-MISSING, no pattern, no DXF
    assert_eq!(get("C1")["state"], "computed", "{}", get("C1"));
    assert!(get("C1")["unit_mass_kg"].as_f64().unwrap() > 0.0);
    assert_eq!(get("C1")["qty"], 2);
    assert_eq!(get("P1")["state"], "held_missing", "{}", get("P1"));
    assert!(get("P1")["pattern"].as_array().unwrap().is_empty());
    assert!(get("P1")["dxf"].is_null() && get("P1")["dxf_allowed"] == false);
    assert_eq!(first["counts"]["dxf_files"], 0);
    assert_eq!(first["engine"], nf_core::ENGINE_VERSION);
    assert!(
        first["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["no"] == "P1" && i["code"] == "missing")
    );
    assert!(
        first["material"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["no"] == "C1" && m["mass_calc_kg"].as_f64().is_some())
    );
    assert!(
        first["material"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["no"] != "P1" || m["mass_calc_kg"].is_null())
    );
    let id1 = first["id"].as_i64().unwrap();
    assert_eq!(first["supersedes"], id0);
    // the same order again: idempotent, no new row
    let (_, again) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(
        (again["unchanged"].clone(), again["id"].as_i64()),
        (json!(true), Some(id1)),
        "{again}"
    );
    // an edit: the next compute is a new row that supersedes the old one
    let (st, v) = t
        .call(
            "PUT",
            &format!("/api/orders/{oid}/values"),
            Some(json!({ "values": [val("P1", "plate_H", 300.0)] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    t.call(
        "POST",
        &format!("/api/orders/{oid}/confirm"),
        Some(json!({ "all": true })),
    )
    .await;
    let (st, second) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{second}");
    let id2 = second["id"].as_i64().unwrap();
    assert_ne!(id2, id1);
    assert_eq!(second["supersedes"], id1);
    let p1 = second["result"]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["no"] == "P1")
        .unwrap()
        .clone();
    assert_eq!(p1["state"], "computed", "{p1}");
    assert!(!p1["pattern"].as_array().unwrap().is_empty());
    // a second edit: a chain of three
    t.call(
        "PUT",
        &format!("/api/orders/{oid}/values"),
        Some(json!({ "values": [val("C1", "L", 2100.0)] })),
    )
    .await;
    t.call(
        "POST",
        &format!("/api/orders/{oid}/confirm"),
        Some(json!({ "all": true })),
    )
    .await;
    let (_, third) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(third["supersedes"], id2, "{third}");
    let chain = t.get(&format!("/api/orders/{oid}/compute")).await;
    let states: Vec<String> = chain["chain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["state"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        states,
        vec!["SUPERSEDED", "SUPERSEDED", "SUPERSEDED", "CURRENT"],
        "{chain}"
    );
    assert_eq!(chain["current"]["id"], third["id"]);
    assert!(
        chain["chain"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["recompute_required"] == false)
    );
    // computed results are immutable: only state -> SUPERSEDED may change
    {
        let db = t.app.db();
        for sql in [
            "UPDATE order_results SET result_json = '{}' WHERE id = 1",
            "UPDATE order_results SET input_hash = 'x' WHERE id = 1",
            "UPDATE order_results SET supersedes = 99 WHERE id = 2",
            "UPDATE order_results SET state = 'CURRENT' WHERE id = 1",
            "UPDATE order_results SET state = 'SUPERSEDED', result_json = '{}' WHERE id = 4",
        ] {
            assert!(db.execute(sql, []).is_err(), "{sql}");
        }
        assert!(
            db.execute(
                "UPDATE order_results SET state = 'SUPERSEDED' WHERE id = 4",
                []
            )
            .is_ok()
        );
        db.execute(
            "UPDATE order_results SET state = 'CURRENT' WHERE id = 4",
            [],
        )
        .expect_err("a superseded result cannot be revived");
    }
    // a result stored by another engine is shown but marked for recompute (G21); the test simulates the old build
    t.sql("DROP TRIGGER order_results_immutable");
    t.sql("UPDATE order_results SET engine = 'nf-engine 0.4.12+src.0000000000000000' WHERE id = 1");
    let chain = t.get(&format!("/api/orders/{oid}/compute")).await;
    assert_eq!(chain["chain"][0]["recompute_required"], true);
    // computed results are kept: no delete
    let db = t.app.db();
    assert!(db.execute("DELETE FROM order_results", []).is_err());
    drop(db);
    assert_eq!(audit_details(&t, "order_compute").len(), 4);
}

#[tokio::test]
async fn l10_a_held_part_gets_no_dxf_and_the_rules_come_from_the_order() {
    let t = boot();
    let oid = new_order(&t).await;
    put_parts_values(
        &t,
        oid,
        json!([
            { "no": "T1", "name": "main", "shape": "tee_main" },
            { "no": "T1b", "name": "branch", "shape": "tee_branch", "attach_part": "T1" }
        ]),
        json!([
            val("T1", "D", 1016.0),
            val("T1", "t", 12.0),
            val("T1", "L", 2000.0),
            val("T1", "branch_D", 508.0),
            val("T1", "branch_t", 9.0),
            val("T1", "branch_x", 1000.0),
            val("T1", "dir", 0.0),
            val("T1", "e", -40.0),
            val("T1b", "D", 508.0),
            val("T1b", "t", 9.0),
            val("T1b", "H", 600.0)
        ]),
    )
    .await;
    t.call(
        "POST",
        &format!("/api/orders/{oid}/confirm"),
        Some(json!({ "all": true })),
    )
    .await;
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    for p in v["result"]["parts"].as_array().unwrap() {
        // a negative offset without a side is refused, never abs()'d
        assert_eq!(
            p["refusal"]["code"], "tee_offset_negative_without_side",
            "{p}"
        );
        assert!(p["dxf"].is_null() && p["dxf_allowed"] == false);
    }
    // unknown rule keys are refused by name
    t.sql(&format!(
        "UPDATE orders SET rules_json = '{{\"bogus\": 1}}' WHERE id = {oid}"
    ));
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["code"], "order_rules");
    // stored rules that are not even JSON are refused, never replaced by the defaults; the refusal is audited
    let refusals_before = audit_details(&t, "order_refused").len();
    t.sql(&format!(
        "UPDATE orders SET rules_json = 'not json {{' WHERE id = {oid}"
    ));
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/orders/{oid}/compute"),
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["code"], "order_rules_corrupt");
    let refusals = audit_details(&t, "order_refused");
    assert_eq!(refusals.len(), refusals_before + 1, "{refusals:?}");
    assert!(
        refusals.last().unwrap().contains("order_rules_corrupt"),
        "{refusals:?}"
    );
    // null/array rules are not an empty object: silently taking the defaults is refused and audited too
    for corrupt in ["null", "[]"] {
        t.sql(&format!(
            "UPDATE orders SET rules_json = '{corrupt}' WHERE id = {oid}"
        ));
        let (st, v) = t
            .call(
                "POST",
                &format!("/api/orders/{oid}/compute"),
                Some(json!({})),
            )
            .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
        assert_eq!(v["code"], "order_rules_corrupt", "{v}");
        let refusals = audit_details(&t, "order_refused");
        assert_eq!(
            refusals.len(),
            refusals_before + 2 + (corrupt == "[]") as usize,
            "{refusals:?}"
        );
        assert!(
            refusals.last().unwrap().contains("order_rules_corrupt"),
            "{refusals:?}"
        );
    }
}
