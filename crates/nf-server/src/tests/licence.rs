//! Licence system through the real router (plan cases 12-22, plus the owner's
//! enforcement-off cases). The test config runs with enforcement ON; the
//! off-mode tests build their own config. Licences are signed with the
//! development key whose public half is compiled into `nf_licence::VERIFY_KEYS`
//! (the same throwaway key the nf-licence inline tests use; the owner replaces
//! it before any sale).

use super::*;
use nf_licence::{DAY, Edition, GRACE, Payload};

/// Development signing seed, shared with the nf-licence inline tests.
const DEV_SEED: [u8; 32] = [
    65, 41, 102, 160, 89, 98, 20, 92, 203, 222, 5, 44, 127, 163, 111, 170, 247, 81, 158, 210, 113,
    97, 16, 248, 97, 227, 46, 47, 218, 120, 156, 231,
];

fn payload(seats: u32, valid_from: u64, valid_until: u64) -> Payload {
    Payload {
        v: 1,
        licence_id: "LIC-TEST-0001".into(),
        customer: "Test Customer".into(),
        seats,
        edition: Edition::Standard,
        valid_from,
        valid_until,
        issued_at: valid_from.saturating_sub(DAY),
        issuer: "Chuo-Sogo".into(),
        key_id: nf_licence::key_id(&nf_licence::public_key_from_seed(&DEV_SEED)),
    }
}

fn signed(p: &Payload) -> Vec<u8> {
    serde_json::to_vec(&nf_licence::sign(p, &DEV_SEED).unwrap()).unwrap()
}

/// A server with the licence file already in place at start-up (or none).
fn boot_lic(enforce: bool, file: Option<&[u8]>) -> T {
    let dir = temp_dir("licence");
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(b) = file {
        std::fs::write(dir.join("licence.nflic"), b).unwrap();
    }
    let mut cfg = config(&dir);
    cfg.licence_enforce = enforce;
    let app = build_app(cfg).unwrap();
    let cookie = add_session(&app, "admin", "admin");
    *app.setup_token.lock().unwrap() = None;
    let t = T { app, dir, cookie };
    t.app.db().execute("INSERT INTO projects(name, description, created_by, created_at) VALUES ('p','', 'admin', 0)", []).unwrap();
    t
}

/// POST /api/licence with a raw key-file body (the handler reads the body itself).
async fn post_licence(t: &T, bytes: &[u8], cookie: &str) -> (StatusCode, Value) {
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/api/licence")
        .header("x-nf-request", "1")
        .header("host", "127.0.0.1:8787")
        .header("cookie", cookie)
        .header("content-type", "application/octet-stream")
        .body(Body::from(bytes.to_vec()))
        .unwrap();
    let resp = router(t.app.clone()).oneshot(req).await.unwrap();
    let (parts, body) = resp.into_parts();
    let b = axum::body::to_bytes(body, 64 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (
        parts.status,
        serde_json::from_slice(&b).unwrap_or(Value::Null),
    )
}

async fn upload_bytes(t: &T, name: &str, bytes: &[u8]) -> (StatusCode, Value) {
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/api/projects/1/documents")
        .header("x-nf-request", "1")
        .header("host", "127.0.0.1:8787")
        .header("cookie", &t.cookie)
        .header("x-filename", name)
        .body(Body::from(bytes.to_vec()))
        .unwrap();
    let resp = router(t.app.clone()).oneshot(req).await.unwrap();
    let (parts, body) = resp.into_parts();
    let b = axum::body::to_bytes(body, 64 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (
        parts.status,
        serde_json::from_slice(&b).unwrap_or(Value::Null),
    )
}

const PDF: &[u8] = b"%PDF-1.7\n%licence-test\n";

/// Install a valid licence with `seats` seats through the real endpoint. For tests
/// outside this module that create accounts beyond the single evaluation seat.
pub(super) async fn install_seats(t: &T, seats: u32) {
    let t0 = now();
    let p = payload(seats, t0 - DAY, t0 + 30 * DAY);
    let (st, v) = post_licence(t, &signed(&p), &t.cookie).await;
    assert_eq!(st, StatusCode::OK, "{v}");
}

/// A stored conversion with its DXF and report files, as the converter would
/// have produced it (no converter program needed for the read-only tests).
fn stored_conversion(t: &T) -> i64 {
    let dxf = b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
    let report = br#"{"converter_version":"licence-test"}"#;
    let dxf_sha = t.app.store_blob(dxf).ok().unwrap();
    let report_sha = t.app.store_blob(report).ok().unwrap();
    let (doc, src_sha): (i64, String) = t
        .app
        .db()
        .query_row(
            "SELECT id, sha256 FROM documents WHERE detected_type = 'pdf' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    t.sql(&format!("INSERT INTO pdf_conversions(project_id, source_document, page, scale_kind, scale_value, tolerance_mm, include_fill, include_text, status, code, limit_name, message, converter_version, source_sha256, dxf_sha256, report_sha256, record_id, created_by, created_at) VALUES (1, {doc}, 1, 'ratio', 1.0, NULL, 0, 0, 'converted', NULL, NULL, NULL, 'licence-test', '{src_sha}', '{dxf_sha}', '{report_sha}', 'r', 'admin', 0)"));
    let cid = t.app.db().last_insert_rowid();
    for (name, dtype, sha, len) in [
        ("NOT-RELEASED_from-pdf_d.p1.dxf", "dxf", &dxf_sha, dxf.len()),
        (
            "NOT-RELEASED_from-pdf_d.p1.report.json",
            "pdf_conversion_report",
            &report_sha,
            report.len(),
        ),
    ] {
        t.sql(&format!("INSERT INTO documents(project_id, filename, detected_type, size, sha256, extension_mismatch, record_id, created_by, created_at, conversion_id) VALUES (1, '{name}', '{dtype}', {len}, '{sha}', 0, 'r', 'admin', 0, {cid})"));
    }
    cid
}

// ---------------------------------------------------------------- (12) no key

#[tokio::test]
async fn licence12_no_key_is_evaluation_and_calculates() {
    let t = boot_lic(true, None);
    let h = t.get("/api/health").await;
    assert_eq!(h["licence"]["state"], "evaluation");
    assert_eq!(h["licence"]["enforced"], true);
    let id = t.job(cone()).await;
    assert!(id > 0);
    let l = t.get("/api/licence").await;
    assert_eq!(l["state"], "evaluation");
    assert_eq!(l["days_left"], 30);
    assert_eq!(l["seats"], 1, "evaluation covers one seat");
    assert_eq!(l["seats_used"], 1);
    assert_eq!(l["file_verified"], false);
    assert!(l["licence_id"].is_null());
    assert_eq!(l["renewal_contact"], "Chuo-Sogo");
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_status'") >= 1);
}

// ---------------------------------------------------------------- (13) valid key

#[tokio::test]
async fn licence13_valid_key_state_and_seats() {
    let t0 = now();
    let p = payload(2, t0 - DAY, t0 + 30 * DAY);
    let t = boot_lic(true, Some(&signed(&p)));
    let h = t.get("/api/health").await;
    assert_eq!(h["licence"]["state"], "valid");
    let l = t.get("/api/licence").await;
    assert_eq!(l["state"], "valid");
    assert_eq!(l["edition"], "standard");
    assert_eq!(l["licence_id"], "LIC-TEST-0001");
    assert_eq!(l["customer"], "Test Customer");
    assert_eq!(l["seats"], 2);
    assert_eq!(l["seats_used"], 1);
    assert_eq!(l["valid_from"], p.valid_from);
    assert_eq!(l["valid_until"], p.valid_until);
    assert_eq!(l["issuer"], "Chuo-Sogo");
    assert_eq!(l["file_verified"], true);
    assert_eq!(l["clock_rollback"], false);
    t.job(cone()).await;
}

// ---------------------------------------------------------------- (14) expired in grace

#[tokio::test]
async fn licence14_expired_in_grace_still_calculates() {
    let t0 = now();
    let p = payload(2, t0 - 40 * DAY, t0 - DAY);
    let t = boot_lic(true, Some(&signed(&p)));
    let h = t.get("/api/health").await;
    assert_eq!(h["licence"]["state"], "grace");
    let l = t.get("/api/licence").await;
    assert_eq!(l["state"], "grace");
    assert_eq!(l["grace_until"], p.valid_until + GRACE);
    t.job(cone()).await;
}

// ---------------------------------------------------------------- (15) past grace

#[tokio::test]
async fn licence15_past_grace_refuses_new_work_and_keeps_everything_readable() {
    let t0 = now();
    let p = payload(2, t0 - DAY, t0 + 30 * DAY);
    let mut t = boot_lic(true, Some(&signed(&p)));
    // existing work, made while the licence is valid
    let id = t.job(cone()).await;
    // G19 is NOT_RUN until the replay audit has run; a DXF is held by the export rule until then
    replay_matches(&mut t, id).await;
    let a = t.job(good_assembly()).await;
    let (_, cv) = t
        .call(
            "POST",
            &format!("/api/calculations/{a}/corrections"),
            Some(json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter per drawing" })),
        )
        .await;
    assert!(cv["new_calculation"].as_i64().is_some(), "{cv}");
    // the correction records a lesson; the next identical job carries an advisory
    let again = t.job(good_assembly()).await;
    assert_eq!(upload_bytes(&t, "drawing.pdf", PDF).await.0, StatusCode::OK);
    let cid = stored_conversion(&t);
    let obl = t.get("/api/obligations").await.as_array().unwrap()[0]["id"]
        .as_i64()
        .unwrap();
    let adv = t.get(&format!("/api/calculations/{again}")).await["advisories"][0]["id"]
        .as_i64()
        .unwrap();
    let doc: i64 = t
        .app
        .db()
        .query_row(
            "SELECT id FROM documents WHERE detected_type = 'pdf' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // the licence expires past grace; the hourly tick picks the new file up
    let expired = payload(2, t0 - 60 * DAY, t0 - 20 * DAY);
    std::fs::write(t.dir.join("licence.nflic"), signed(&expired)).unwrap();
    crate::licence::recheck(&t.app);
    assert_eq!(t.get("/api/health").await["licence"]["state"], "read_only");
    let tables = [
        "calculations",
        "records",
        "documents",
        "pdf_conversions",
        "obligations",
        "lessons",
        "advisories",
    ];
    let before: Vec<i64> = tables
        .iter()
        .map(|x| t.count(&format!("SELECT COUNT(*) FROM {x}")))
        .collect();
    // every endpoint that starts new work is refused, with the licence marker
    for (path, body) in [
        ("/api/projects/1/calculations", Some(cone())),
        (&format!("/api/calculations/{id}/replay"), None),
        (&format!("/api/calculations/{id}/recompute"), None),
        (
            &format!("/api/calculations/{a}/corrections"),
            Some(json!({ "parameter": "header_radius", "new_value": 159, "reason": "again" })),
        ),
        (
            &format!("/api/documents/{doc}/pdf-to-dxf"),
            Some(json!({ "page": 1, "scale": { "kind": "ratio", "value": 1.0 } })),
        ),
    ] {
        let (st, v) = t.call("POST", path, body).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{path}: {v}");
        assert_eq!(v["licence"], "read_only", "{path}: {v}");
        assert!(v["error"].as_str().is_some_and(|e| !e.is_empty()));
        assert!(v["error_ja"].as_str().is_some_and(|e| !e.is_empty()));
    }
    let (st, v) = upload_bytes(&t, "another.pdf", PDF).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["licence"], "read_only");
    // nothing was written by the refused requests
    let after: Vec<i64> = tables
        .iter()
        .map(|x| t.count(&format!("SELECT COUNT(*) FROM {x}")))
        .collect();
    assert_eq!(before, after, "a refused request wrote a row");
    // reads, exports and the review workflow stay open
    assert_eq!(
        t.call("GET", &format!("/api/calculations/{id}"), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(t.dxf_status(id, "reducer").await, StatusCode::OK);
    assert_eq!(
        t.call("GET", &format!("/api/pdf-conversions/{cid}"), None)
            .await
            .0,
        StatusCode::OK
    );
    for file in ["dxf", "report"] {
        assert_eq!(
            t.call("GET", &format!("/api/pdf-conversions/{cid}/{file}"), None)
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        t.call(
            "POST",
            &format!("/api/obligations/{obl}/resolve"),
            Some(json!({ "decision": "confirmed", "reason": "checked against the drawing" }))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        t.call(
            "POST",
            &format!("/api/advisories/{adv}/acknowledge"),
            Some(json!({ "reason": "dimensioned outside" }))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        t.call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "still allowed" }))
        )
        .await
        .0,
        StatusCode::OK
    );
    let verify = t.get("/api/audit/verify").await;
    assert_eq!(verify["intact"], true);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_blocked'") >= 6);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_readonly'") >= 1);
}

// ---------------------------------------------------------------- (16) tampered file

#[tokio::test]
async fn licence16_tampered_file_falls_back_and_is_audited() {
    let t0 = now();
    let p = payload(2, t0 - DAY, t0 + 30 * DAY);
    let mut v: Value = serde_json::from_slice(&signed(&p)).unwrap();
    v["payload"]["customer"] = json!("Someone Else");
    let tampered = serde_json::to_vec(&v).unwrap();
    // never a verified licence before: evaluation, and the invalid file is audited
    let t = boot_lic(true, Some(&tampered));
    assert_eq!(t.get("/api/health").await["licence"]["state"], "evaluation");
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_invalid'") >= 1);
    t.job(cone()).await;
    // a verified licence before: the persisted payload keeps its state, never a fresh trial
    let expired = payload(2, t0 - 60 * DAY, t0 - 20 * DAY);
    let t2 = boot_lic(true, Some(&signed(&expired)));
    assert_eq!(t2.get("/api/health").await["licence"]["state"], "read_only");
    let mut v: Value = serde_json::from_slice(&signed(&expired)).unwrap();
    v["payload"]["seats"] = json!(9);
    std::fs::write(
        t2.dir.join("licence.nflic"),
        serde_json::to_vec(&v).unwrap(),
    )
    .unwrap();
    crate::licence::recheck(&t2.app);
    let h = t2.get("/api/health").await;
    assert_eq!(
        h["licence"]["state"], "read_only",
        "a tampered file granted a fresh trial"
    );
    assert!(t2.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_invalid'") >= 1);
}

// ---------------------------------------------------------------- (17) wrong-key signature

#[tokio::test]
async fn licence17_wrong_key_signature_falls_back_and_is_audited() {
    let t0 = now();
    let p = payload(2, t0 - DAY, t0 + 30 * DAY);
    // signed by a different seed, but claiming the compiled key_id
    let bytes = serde_json::to_vec(&nf_licence::sign(&p, &[7u8; 32]).unwrap()).unwrap();
    let t = boot_lic(true, Some(&bytes));
    assert_eq!(t.get("/api/health").await["licence"]["state"], "evaluation");
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_invalid'") >= 1);
    t.job(cone()).await;
}

// ---------------------------------------------------------------- (18) clock rollback

#[tokio::test]
async fn licence18_clock_rollback_never_improves_the_state() {
    let t0 = now();
    let p = payload(2, t0 - DAY, t0 + 30 * DAY);
    let t = boot_lic(true, Some(&signed(&p)));
    let id = t.job(cone()).await;
    let calculations = t.count("SELECT COUNT(*) FROM calculations");
    // place the persisted high-water mark far ahead, HMAC'd exactly as the server writes it
    let future = t0 + 100 * DAY;
    {
        let db = t.app.db();
        crate::licence::meta_write(&t.app, &db, "licence_last_seen", &future.to_string()).unwrap();
    }
    crate::licence::recheck(&t.app);
    let l = t.get("/api/licence").await;
    assert_eq!(
        l["state"], "read_only",
        "the rolled-back clock improved the state"
    );
    assert_eq!(l["clock_rollback"], true);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_clock_rollback'") >= 1);
    // data intact, and the gate bites at the effective time
    assert_eq!(
        t.call("GET", &format!("/api/calculations/{id}"), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM calculations"), calculations);
    assert_eq!(t.calc(cone()).await.0, StatusCode::FORBIDDEN);
    // a tampered meta value (bad HMAC) is absent, never a way back in time
    t.app.db().execute(
        "UPDATE meta SET value = '{\"value\":\"0\",\"mac\":\"bad\"}' WHERE key = 'licence_last_seen'",
        [],
    ).unwrap();
    crate::licence::recheck(&t.app);
    assert_eq!(t.get("/api/licence").await["state"], "read_only");
}

// ---------------------------------------------------------------- (19) seat limit

#[tokio::test]
async fn licence19_seat_limit_refuses_second_user_and_reactivation() {
    let t0 = now();
    let p = payload(1, t0 - DAY, t0 + 30 * DAY);
    let t = boot_lic(true, Some(&signed(&p)));
    let (st, v) = t
        .call(
            "POST",
            "/api/users",
            Some(json!({ "username": "second", "password": "seconds-long-password", "role": "engineer" })),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["licence"], "seat_limit");
    assert!(v["error_ja"].as_str().is_some_and(|e| !e.is_empty()));
    assert_eq!(t.count("SELECT COUNT(*) FROM users"), 1);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_seat_limit'") >= 1);
    // an over-limit account that already exists keeps working (never blocked)
    let extra_cookie = add_session(&t.app, "extra", "engineer");
    t.sql("INSERT INTO project_members(project_id, user_id) SELECT 1, id FROM users WHERE username = 'extra'");
    let (st, _, bytes) = t
        .send(
            "POST",
            "/api/projects/1/calculations",
            Some(cone()),
            &[("cookie", &extra_cookie)],
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{bytes:?}");
    let extra: i64 = t
        .app
        .db()
        .query_row("SELECT id FROM users WHERE username = 'extra'", [], |r| {
            r.get(0)
        })
        .unwrap();
    // deactivation is never refused
    assert_eq!(
        t.call(
            "POST",
            &format!("/api/users/{extra}/active"),
            Some(json!({ "active": false }))
        )
        .await
        .0,
        StatusCode::OK
    );
    // reactivation over the limit is refused
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/users/{extra}/active"),
            Some(json!({ "active": true })),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["licence"], "seat_limit");
    // the existing administrator keeps working
    t.job(cone()).await;
}

// ---------------------------------------------------------------- (20) evaluation seat

#[tokio::test]
async fn licence20_evaluation_allows_one_seat() {
    let t = boot_lic(true, None);
    let (st, v) = t
        .call(
            "POST",
            "/api/users",
            Some(json!({ "username": "second", "password": "seconds-long-password", "role": "viewer" })),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["licence"], "seat_limit");
    assert_eq!(t.count("SELECT COUNT(*) FROM users"), 1);
}

// ---------------------------------------------------------------- (21) install / renewal

#[tokio::test]
async fn licence21_install_endpoint_renews_and_refuses() {
    let t0 = now();
    let t = boot_lic(true, None);
    let p = payload(2, t0 - DAY, t0 + 30 * DAY);
    // a non-administrator cannot install
    let eng = add_session(&t.app, "eng", "engineer");
    assert_eq!(
        post_licence(&t, &signed(&p), &eng).await.0,
        StatusCode::FORBIDDEN
    );
    // a visitor cannot install
    assert_eq!(
        post_licence(&t, &signed(&p), "none=1").await.0,
        StatusCode::UNAUTHORIZED
    );
    // an invalid body is refused and changes nothing
    let (st, v) = post_licence(&t, b"not json", &t.cookie).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["licence"], "invalid");
    assert_eq!(t.get("/api/health").await["licence"]["state"], "evaluation");
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_invalid'") >= 1);
    assert!(!t.dir.join("licence.nflic").exists());
    // oversize is refused
    let big = vec![b' '; nf_licence::MAX_FILE_BYTES + 1];
    assert_eq!(
        post_licence(&t, &big, &t.cookie).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    // the administrator installs: valid, audited, owner-only file
    let (st, v) = post_licence(&t, &signed(&p), &t.cookie).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["state"], "valid");
    assert_eq!(v["seats"], 2);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_loaded'") >= 1);
    let path = t.dir.join("licence.nflic");
    assert!(path.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // renewal to next month's key, without a restart
    let mut p2 = payload(3, t0 - DAY, t0 + 60 * DAY);
    p2.licence_id = "LIC-TEST-0002".into();
    let (st, v) = post_licence(&t, &signed(&p2), &t.cookie).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["licence_id"], "LIC-TEST-0002");
    assert_eq!(v["seats"], 3);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_loaded'") >= 2);
    assert_eq!(t.get("/api/health").await["licence"]["state"], "valid");
}

// ---------------------------------------------------------------- (22) identical outcomes

#[tokio::test]
async fn licence22_verification_outcomes_are_identical_with_and_without_a_licence() {
    let t1 = boot_lic(true, None);
    let (st1, v1) = t1.calc(cone()).await;
    let t0 = now();
    let p = payload(2, t0 - DAY, t0 + 30 * DAY);
    let t2 = boot_lic(true, Some(&signed(&p)));
    let (st2, v2) = t2.calc(cone()).await;
    assert_eq!((st1, st2), (StatusCode::OK, StatusCode::OK));
    assert_eq!(v1, v2, "the API answer differs with a licence installed");
    // the stored verification outcome itself, byte for byte
    let row = |t: &T| -> (String, String, String, Option<String>) {
        t.app.db().query_row(
            "SELECT result_json, closure_hash, state, dxf_sha256 FROM calculations WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap()
    };
    let r1 = row(&t1);
    let r2 = row(&t2);
    assert_eq!(r1, r2, "the stored result differs with a licence installed");
    if let (Some(s1), Some(s2)) = (&r1.3, &r2.3) {
        let d1 = storage::load_blob(&t1.dir, &t1.app.key, s1).unwrap();
        let d2 = storage::load_blob(&t2.dir, &t2.app.key, s2).unwrap();
        assert_eq!(d1, d2, "the DXF bytes differ with a licence installed");
    }
}

// ---------------------------------------------------------------- owner: enforcement off

#[tokio::test]
async fn licence_off_expired_past_grace_blocks_nothing() {
    let t0 = now();
    let p = payload(2, t0 - 60 * DAY, t0 - 20 * DAY);
    let t = boot_lic(false, Some(&signed(&p)));
    let h = t.get("/api/health").await;
    assert_eq!(h["licence"]["state"], "read_only");
    assert_eq!(h["licence"]["enforced"], false);
    let l = t.get("/api/licence").await;
    assert_eq!(l["state"], "read_only");
    assert_eq!(l["enforced"], false);
    assert_eq!(l["reason"], "licence_expired");
    // nothing is gated: new work and uploads go through
    t.job(cone()).await;
    assert_eq!(upload_bytes(&t, "drawing.pdf", PDF).await.0, StatusCode::OK);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_blocked'"),
        0
    );
    // the state is still audited truthfully
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_status'") >= 1);
    // installing a renewal key works with enforcement off too
    let renewed = payload(2, t0 - DAY, t0 + 30 * DAY);
    let (st, v) = post_licence(&t, &signed(&renewed), &t.cookie).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["state"], "valid");
    assert_eq!(v["enforced"], false);
}

#[tokio::test]
async fn licence_off_seat_limit_is_logged_not_enforced() {
    let t0 = now();
    let p = payload(1, t0 - DAY, t0 + 30 * DAY);
    let t = boot_lic(false, Some(&signed(&p)));
    let (st, v) = t
        .call(
            "POST",
            "/api/users",
            Some(json!({ "username": "second", "password": "seconds-long-password", "role": "engineer" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(t.count("SELECT COUNT(*) FROM users"), 2);
    assert!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_seat_limit_not_enforced'") >= 1
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_seat_limit'"),
        0
    );
    // an over-limit reactivation is allowed too, and logged again
    let second: i64 = t
        .app
        .db()
        .query_row("SELECT id FROM users WHERE username = 'second'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        t.call(
            "POST",
            &format!("/api/users/{second}/active"),
            Some(json!({ "active": false }))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        t.call(
            "POST",
            &format!("/api/users/{second}/active"),
            Some(json!({ "active": true }))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'licence_seat_limit_not_enforced'") >= 2
    );
}

#[tokio::test]
async fn licence_pz3_preview_and_rules_are_gated_like_other_writes() {
    let t0 = now();
    let valid = payload(2, t0 - DAY, t0 + 30 * DAY);
    let t = boot_lic(true, Some(&signed(&valid)));
    let (st, order) = t
        .call(
            "POST",
            "/api/projects/1/orders",
            Some(json!({ "name": "Preview job" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{order}");
    let oid = order["id"].as_i64().unwrap();

    let expired = payload(2, t0 - 60 * DAY, t0 - 20 * DAY);
    std::fs::write(t.dir.join("licence.nflic"), signed(&expired)).unwrap();
    crate::licence::recheck(&t.app);
    assert_eq!(t.get("/api/health").await["licence"]["state"], "read_only");
    let before = pz3_counts_for_licence(&t);

    for (m, p, b) in [
        (
            "POST",
            format!("/api/orders/{oid}/preview"),
            Some(
                json!({"parts_hash": "0".repeat(64), "edit": {"kind": "seam_side", "seam_from": "top"}}),
            ),
        ),
        ("POST", format!("/api/orders/{oid}/nest"), Some(json!({}))),
        (
            "POST",
            format!("/api/orders/{oid}/rules/impact"),
            Some(json!({"rules": {}, "parts_hash": "0".repeat(64)})),
        ),
        (
            "PUT",
            format!("/api/orders/{oid}/rules"),
            Some(json!({"rules": {}})),
        ),
        (
            "PUT",
            "/api/projects/1/rules".into(),
            Some(json!({"rules": {}})),
        ),
        ("POST", "/api/rule-packs".into(), Some(json!({}))),
        (
            "POST",
            "/api/rule-packs/1/assign".into(),
            Some(json!({ "scope": "project", "target_id": 1 })),
        ),
        (
            "POST",
            "/api/rule-packs/1/unassign".into(),
            Some(json!({ "scope": "project", "target_id": 1 })),
        ),
        (
            "POST",
            "/api/rule-packs/1/interview/answers".into(),
            Some(json!({ "key": "allow", "action": "skip" })),
        ),
        (
            "POST",
            "/api/rule-packs/1/confirmations/1/undo".into(),
            None,
        ),
    ] {
        let (st, _, v) = t.send(m, &p, b, &[]).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{p}: {v:?}");
        let v: Value = serde_json::from_slice(&v).unwrap_or(Value::Null);
        assert_eq!(v["licence"], "read_only", "{p}: {v}");
        assert!(v["error"].as_str().is_some_and(|e| !e.is_empty()));
        assert!(v["error_ja"].as_str().is_some_and(|e| !e.is_empty()));
    }
    assert_eq!(
        pz3_counts_for_licence(&t),
        before,
        "a refused request wrote a row"
    );
    assert_eq!(
        t.call("GET", &format!("/api/orders/{oid}/rules"), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        t.call("GET", "/api/projects/1/rules", None).await.0,
        StatusCode::OK
    );
}

fn pz3_counts_for_licence(t: &T) -> Vec<i64> {
    vec![
        t.count("SELECT COUNT(*) FROM orders"),
        t.count("SELECT COUNT(*) FROM projects WHERE default_rules_json <> '{}'"),
        t.count("SELECT COUNT(*) FROM order_results"),
        t.count("SELECT COUNT(*) FROM order_decisions"),
        t.count("SELECT COUNT(*) FROM audit WHERE action NOT LIKE 'licence%'"),
    ]
}
