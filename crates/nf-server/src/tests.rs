//! Regression tests for the backend defect register (BE-001 … BE-011 and the
//! mediums fixed with them) and the security findings V-25, V-33 … V-36.
//! Each test drives the real router against a private data directory.

use super::*;
use axum::body::Body;
use axum::http::Request as HttpRequest;
use std::sync::atomic::{AtomicU32, Ordering};
use tower::ServiceExt;

static SEQ: AtomicU32 = AtomicU32::new(0);
const TOKEN: &str = "unit-test-setup-token";

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "nf-test-{}-{}-{tag}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn config(dir: &std::path::Path) -> Config {
    Config {
        data_dir: dir.to_path_buf(),
        key_path: dir.join("storage.key"),
        cookie_secure: false,
        allowed_hosts: default_hosts(),
        trusted_proxies: vec![],
        setup_token: Some(TOKEN.into()),
        request_timeout: Duration::from_secs(60),
        desktop: false,
        licence_path: dir.join("licence.nflic"),
        // the licence tests run with enforcement ON; the off-mode tests build their own config
        licence_enforce: true,
        export_dirs: vec![],
    }
}

struct T {
    app: Arc<App>,
    dir: PathBuf,
    cookie: String,
}

impl Drop for T {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Insert a user and a session directly (no Argon2: debug builds are slow).
fn add_session(app: &App, name: &str, role: &str) -> String {
    let db = app.db();
    db.execute(
        "INSERT INTO users(username, role, salt, hash, created_at) VALUES (?,?,?,?,0)",
        params![name, role, "00", "x"],
    )
    .unwrap();
    let uid = db.last_insert_rowid();
    let token = format!("token-of-{name}");
    db.execute(
        "INSERT INTO sessions(token_hash, user_id, expires_at) VALUES (?,?,?)",
        params![
            nf_core::sha256_hex(token.as_bytes()),
            uid,
            (now() + 3600) as i64
        ],
    )
    .unwrap();
    format!("nf_session={token}")
}

fn boot() -> T {
    let dir = temp_dir("app");
    let app = build_app(config(&dir)).unwrap();
    let cookie = add_session(&app, "admin", "admin");
    *app.setup_token.lock().unwrap() = None;
    let t = T { app, dir, cookie };
    t.app.db().execute("INSERT INTO projects(name, description, created_by, created_at) VALUES ('p','', 'admin', 0)", []).unwrap();
    t
}

impl T {
    async fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        extra: &[(&str, &str)],
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut b = HttpRequest::builder()
            .method(method)
            .uri(path)
            .header("x-nf-request", "1");
        if !extra.iter().any(|(k, _)| *k == "host") {
            b = b.header("host", "127.0.0.1:8787");
        }
        if !extra.iter().any(|(k, _)| *k == "cookie") {
            b = b.header("cookie", &self.cookie);
        }
        for (k, v) in extra {
            b = b.header(*k, *v);
        }
        let req = match body {
            Some(v) => b
                .header("content-type", "application/json")
                .body(Body::from(v.to_string()))
                .unwrap(),
            None => b.body(Body::empty()).unwrap(),
        };
        let resp = router(self.app.clone()).oneshot(req).await.unwrap();
        let (parts, body) = resp.into_parts();
        let bytes = axum::body::to_bytes(body, 64 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec();
        (parts.status, parts.headers, bytes)
    }
    async fn call(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let (st, _, bytes) = self.send(method, path, body, &[]).await;
        (st, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }
    async fn get(&self, path: &str) -> Value {
        let (st, v) = self.call("GET", path, None).await;
        assert_eq!(st, StatusCode::OK, "GET {path}: {v}");
        v
    }
    async fn calc(&self, body: Value) -> (StatusCode, Value) {
        self.call("POST", "/api/projects/1/calculations", Some(body))
            .await
    }
    async fn job(&self, body: Value) -> i64 {
        let (st, v) = self.calc(body).await;
        assert_eq!(st, StatusCode::OK, "{v}");
        v["id"].as_i64().unwrap()
    }
    async fn dxf_status(&self, id: i64, part: &str) -> StatusCode {
        self.send(
            "GET",
            &format!("/api/calculations/{id}/dxf/{part}"),
            None,
            &[],
        )
        .await
        .0
    }
    fn count(&self, sql: &str) -> i64 {
        self.app.db().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn sql(&self, sql: &str) {
        self.app.db().execute(sql, []).unwrap();
    }
    fn set_fault(&self, f: Option<&'static str>) {
        *self.app.fault.lock().unwrap() = f;
    }
    /// A second handle on the same data with another replay executable.
    fn with_replay_exe(&mut self, script: &str) {
        let path = self
            .dir
            .join(format!("replay-{}.sh", SEQ.fetch_add(1, Ordering::SeqCst)));
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let app = Arc::get_mut(&mut self.app).expect("no request in flight");
        app.replay_exe = Some(path);
    }
}

fn assembly(sref: &str, hr: f64, hdev: f64, br: f64, bdev: f64) -> Value {
    json!({ "family": "header_with_branch", "header_radius": hr, "header_dev_radius": hdev, "header_length": 1200, "branch_station": 600, "branch_radius": br, "branch_dev_radius": bdev, "beta_deg": 60, "delta": 0, "s_end": 400, "surface_reference": sref })
}
fn good_assembly() -> Value {
    assembly("outside", 150.0, 150.0, 100.0, 100.0)
}
fn cone() -> Value {
    json!({ "family": "cone_frustum", "r1": 210, "r2": 100, "h": 300 })
}
fn gate_of<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["gates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == id)
        .unwrap_or(&Value::Null)
}

/// No response anywhere may say APPROVED, and every list row must agree with the detail view.
async fn assert_consistent_and_never_approved(t: &T) {
    let list = t.get("/api/calculations").await;
    let dash = t.get("/api/dashboard").await;
    assert_eq!(dash["approved"], 0);
    let (mut review, mut rejected, mut superseded, mut refused) = (0, 0, 0, 0);
    for row in list.as_array().unwrap() {
        let id = row["id"].as_i64().unwrap();
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(
            row["state"], d["state"],
            "list and detail disagree on job #{id}"
        );
        assert_ne!(d["state"], "APPROVED");
        assert!(["REVIEW_REQUIRED", "REJECTED", "REFUSED"].contains(&d["state"].as_str().unwrap()));
        assert_eq!(row["superseded"], json!(!d["superseded_by"].is_null()));
        let col: String = t
            .app
            .db()
            .query_row(
                "SELECT state FROM calculations WHERE id = ?",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            col,
            d["state"].as_str().unwrap(),
            "state column of job #{id}"
        );
        match (d["superseded_by"].is_null(), d["state"].as_str().unwrap()) {
            (false, _) => superseded += 1,
            (true, "REVIEW_REQUIRED") => review += 1,
            (true, "REJECTED") => rejected += 1,
            _ => refused += 1,
        }
    }
    assert_eq!(
        (
            dash["review_required"].as_i64(),
            dash["rejected"].as_i64(),
            dash["superseded"].as_i64(),
            dash["refused"].as_i64()
        ),
        (
            Some(review),
            Some(rejected),
            Some(superseded),
            Some(refused)
        ),
        "dashboard counts"
    );
}

// ---------------------------------------------------------------- BE-001

#[tokio::test]
async fn be001_inconsistent_surface_reference_is_refused_and_nothing_is_downloadable() {
    let t = boot();
    // the audit's reproduction: outside 150 with development radius 160
    let (st, v) = t
        .calc(assembly("outside", 150.0, 160.0, 100.0, 110.0))
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["state"], "REFUSED");
    assert_eq!(v["field"], "header_dev_radius");
    assert!(
        v["outcome"]
            .as_str()
            .unwrap()
            .contains("negative wall thickness"),
        "{v}"
    );
    let id = v["id"].as_i64().unwrap();
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["state"], "REFUSED");
    assert_eq!(d["patterns"], json!([]));
    assert_eq!(t.dxf_status(id, "header").await, StatusCode::NOT_FOUND);
    assert_eq!(t.dxf_status(id, "branch").await, StatusCode::NOT_FOUND);
    for (sref, hr, hdev, expect) in [
        ("outside", 150.0, 146.0, 200),
        ("inside", 142.0, 146.0, 200),
        ("neutral", 150.0, 150.0, 200),
        ("neutral", 150.0, 150.5, 422),
        ("inside", 150.0, 146.0, 422),
    ] {
        let (st, v) = t.calc(assembly(sref, hr, hdev, 100.0, 100.0)).await;
        assert_eq!(st.as_u16(), expect, "{sref} {hr}/{hdev}: {v}");
    }
    // equal radii under "outside": accepted, and the open question is stated on G02
    let ok_id = t.job(good_assembly()).await;
    let d = t.get(&format!("/api/calculations/{ok_id}")).await;
    assert_eq!(gate_of(&d, "G02")["result"], "INCONCLUSIVE");
    assert!(
        gate_of(&d, "G02")["detail"]
            .as_str()
            .unwrap()
            .contains("zero thickness")
    );
    assert_consistent_and_never_approved(&t).await;
}

// ---------------------------------------------------------------- BE-002

#[tokio::test]
async fn be002_corrupt_or_incomplete_stored_result_is_rejected_never_approved() {
    let t = boot();
    let a = t.job(good_assembly()).await;
    let (st, v) = t.call("POST", &format!("/api/calculations/{a}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter per drawing" }))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let with_obligations = v["new_calculation"].as_i64().unwrap();
    assert_eq!(v["open"], 3);
    let ids = [
        t.job(cone()).await,
        t.job(cone()).await,
        t.job(cone()).await,
        t.job(cone()).await,
        t.job(cone()).await,
    ];
    let all_pass = "json_set(result_json, '$.gates', json('[{\"id\":\"G16\",\"name\":\"human_approval\",\"critical\":true,\"result\":\"PASS\",\"detail\":\"x\"},{\"id\":\"G10\",\"name\":\"v\",\"critical\":true,\"result\":\"PASS\",\"detail\":\"x\"}]'))";
    t.sql(&format!(
        "UPDATE calculations SET result_json = '{{}}' WHERE id = {}",
        ids[0]
    ));
    t.sql(&format!(
        "UPDATE calculations SET result_json = 'not json' WHERE id = {}",
        ids[1]
    ));
    t.sql(&format!(
        "UPDATE calculations SET result_json = json_remove(result_json, '$.gates') WHERE id = {}",
        ids[2]
    ));
    t.sql(&format!(
        "UPDATE calculations SET result_json = {all_pass} WHERE id = {}",
        ids[3]
    ));
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.gates', json('[]'), '$.state', 'APPROVED') WHERE id = {}", ids[4]));
    t.sql(&format!("UPDATE calculations SET result_json = json_remove(result_json, '$.gates') WHERE id = {with_obligations}"));
    for id in ids.iter().copied().chain([with_obligations]) {
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(d["state"], "REJECTED", "job #{id}: {}", d["gates"]);
        assert_eq!(
            gate_of(&d, "G22")["result"],
            "FAIL",
            "integrity finding for job #{id}"
        );
        assert_eq!(
            gate_of(&d, "G16")["result"],
            "NOT_IMPLEMENTED",
            "approval can never be read from storage"
        );
        assert!(!d["integrity"].as_array().unwrap().is_empty());
        assert_eq!(
            d["patterns"],
            json!([]),
            "nothing of a corrupt job is offered as geometry"
        );
        for part in ["reducer", "header", "branch"] {
            assert_eq!(
                t.dxf_status(id, part).await,
                StatusCode::CONFLICT,
                "job #{id} {part}"
            );
        }
        assert_eq!(
            t.call("POST", &format!("/api/calculations/{id}/replay"), None)
                .await
                .0,
            StatusCode::CONFLICT
        );
    }
    // the open obligations of the corrected job are still reported although its gate table is gone
    let d = t
        .get(&format!("/api/calculations/{with_obligations}"))
        .await;
    assert!(
        gate_of(&d, "G18")["detail"]
            .as_str()
            .unwrap()
            .contains("3 of"),
        "{}",
        gate_of(&d, "G18")
    );
    assert!(
        gate_of(&d, "G18")["detail"]
            .as_str()
            .unwrap()
            .contains("OPEN")
    );
    // required gates missing from storage are failed gates
    let d = t.get(&format!("/api/calculations/{}", ids[0])).await;
    for g in ["G01", "G05", "G10", "G11", "G15"] {
        assert_eq!(gate_of(&d, g)["result"], "FAIL", "{g}");
    }
    assert_consistent_and_never_approved(&t).await;
    let text = serde_json::to_string(&t.get("/api/calculations").await).unwrap()
        + &serde_json::to_string(&t.get("/api/dashboard").await).unwrap();
    assert!(!text.contains("APPROVED"));
}

#[tokio::test]
async fn be002_a_database_error_is_never_read_as_no_obligations() {
    let t = boot();
    let id = t.job(cone()).await;
    t.app
        .db()
        .execute_batch("ALTER TABLE obligations RENAME TO obligations_gone")
        .unwrap();
    let (st, v) = t
        .call("GET", &format!("/api/calculations/{id}"), None)
        .await;
    assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert!(v["error"].is_string());
    assert_eq!(
        t.dxf_status(id, "reducer").await,
        StatusCode::INTERNAL_SERVER_ERROR,
        "no file without a gate evaluation"
    );
    assert_eq!(
        t.call("GET", "/api/calculations", None).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

// ---------------------------------------------------------------- BE-003

#[tokio::test]
async fn be003_refused_requests_are_refused_end_to_end() {
    let t = boot();
    let good = t.job(cone()).await;
    let before = t.get("/api/dashboard").await;
    let mut refused = vec![];
    for (body, field) in [
        (
            assembly("outside", -150.0, 150.0, 100.0, 100.0),
            "header_radius",
        ),
        (assembly("outside", 150.0, 150.0, 400.0, 400.0), "delta+r"),
        (
            json!({ "family": "cylinder_shell", "r_dev": 1e300, "length": 1e300 }),
            "r_dev",
        ),
        (
            json!({ "family": "cylinder_shell", "r_dev": 1e-300, "length": 10 }),
            "r_dev",
        ),
    ] {
        let (st, v) = t.calc(body).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
        assert_eq!(
            (
                v["state"].as_str(),
                v["field"].as_str(),
                v["advisories"].as_i64()
            ),
            (Some("REFUSED"), Some(field), Some(0)),
            "{v}"
        );
        assert!(v["error"].as_str().unwrap().contains("refused") && v["outcome"].is_string());
        refused.push(v["id"].as_i64().unwrap());
    }
    for id in &refused {
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(d["state"], "REFUSED");
        assert_eq!(d["refused"], true);
        assert!(
            d["outcome"].is_string()
                && d["patterns"] == json!([])
                && d["correctable_parameters"] == json!([])
        );
        assert_eq!(t.dxf_status(*id, "header").await, StatusCode::NOT_FOUND);
        assert_eq!(
            t.call("POST", &format!("/api/calculations/{id}/replay"), None)
                .await
                .0,
            StatusCode::CONFLICT
        );
        assert_eq!(t.call("POST", &format!("/api/calculations/{id}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 150, "reason": "retry attempt" }))).await.0, StatusCode::CONFLICT);
        let row = t
            .get("/api/calculations")
            .await
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == *id)
            .cloned()
            .unwrap();
        assert_eq!(
            (row["state"].as_str(), row["no_result"].as_bool()),
            (Some("REFUSED"), Some(true))
        );
        assert!(row["outcome"].is_string());
    }
    let after = t.get("/api/dashboard").await;
    assert_eq!(
        after["review_required"], before["review_required"],
        "a refusal is not under review"
    );
    assert_eq!(after["refused"], 4);
    assert_eq!(after["rejected"], 0);
    assert_eq!(t.count("SELECT COUNT(*) FROM advisories"), 0);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'calculation_refused'"),
        4,
        "the attempt is kept in the audit log"
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM calculations WHERE state = 'REVIEW_REQUIRED'"),
        1
    );
    // malformed requests and unknown fields are 400 and store nothing (BE-016)
    let n = t.count("SELECT COUNT(*) FROM calculations");
    let (st, v) = t
        .calc(json!({ "family": "cylinder_shell", "r_dev": 100, "length": 250, "thickness": 12 }))
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(v["error"].as_str().unwrap().contains("thickness"));
    assert_eq!(
        t.calc(json!({ "family": "teapot" })).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM calculations"), n);
    assert_eq!(
        t.get(&format!("/api/calculations/{good}")).await["state"],
        "REVIEW_REQUIRED"
    );
    assert_consistent_and_never_approved(&t).await;
}

// ---------------------------------------------------------------- BE-004

#[tokio::test]
async fn be004_a_refused_correction_changes_nothing() {
    let mut t = boot();
    let a = t.job(good_assembly()).await;
    let snapshot = |t: &T| {
        (
            t.count("SELECT COUNT(*) FROM calculations"),
            t.count("SELECT COUNT(*) FROM lessons"),
            t.count("SELECT COUNT(*) FROM obligations"),
            t.count("SELECT COUNT(*) FROM records"),
            t.count("SELECT COUNT(*) FROM advisories"),
        )
    };
    let before = snapshot(&t);
    // a typo (-5), a value the engine cannot intersect (90 < branch 100), and a value that contradicts the declared surface (BE-001)
    for (param, value, word) in [
        ("header_radius", -5.0, "header_radius"),
        ("s_end", 10.0, "s_end"),
        ("header_radius", 142.0, "negative wall thickness"),
    ] {
        let (st, v) = t.call("POST", &format!("/api/calculations/{a}/corrections"), Some(json!({ "parameter": param, "new_value": value, "reason": "engineer typo test" }))).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{value}: {v}");
        assert!(
            v["error"].as_str().unwrap().contains("nothing was changed")
                && v["outcome"].as_str().unwrap().contains(word),
            "{v}"
        );
        assert!(v["new_calculation"].is_null());
    }
    assert_eq!(
        snapshot(&t),
        before,
        "no job, lesson, obligation, record or advisory may be written"
    );
    let d = t.get(&format!("/api/calculations/{a}")).await;
    assert_eq!(d["state"], "REVIEW_REQUIRED");
    assert!(d["superseded_by"].is_null());
    replay_matches(&mut t, a).await;
    assert_eq!(
        t.dxf_status(a, "header").await,
        StatusCode::OK,
        "the valid job stays current and downloadable"
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'correction_refused'"),
        3
    );
    // the correct value was not recorded as a mistake: an identical new job raises no advisory
    let (_, v) = t.calc(good_assembly()).await;
    assert_eq!(v["advisories"], 0);
    // a valid correction still works, and only then is the lesson recorded
    let (st, v) = t.call("POST", &format!("/api/calculations/{a}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter per drawing" }))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let lessons = t.get("/api/lessons").await;
    assert_eq!(lessons.as_array().unwrap().len(), 1);
    assert_eq!(
        (
            lessons[0]["wrong_value"].as_f64(),
            lessons[0]["right_value"].as_f64()
        ),
        (Some(150.0), Some(158.0))
    );
    assert_eq!(
        t.get(&format!("/api/calculations/{a}")).await["superseded_by"],
        v["new_calculation"]
    );
    assert_consistent_and_never_approved(&t).await;
}

// ---------------------------------------------------------------- BE-005

#[tokio::test]
async fn be005_a_failure_in_the_middle_of_a_write_leaves_nothing_behind() {
    let t = boot();
    let a = t.job(good_assembly()).await;
    let tables = [
        "calculations",
        "lessons",
        "obligations",
        "records",
        "advisories",
        "audit",
        "documents",
    ];
    let snapshot = |t: &T| tables.map(|x| t.count(&format!("SELECT COUNT(*) FROM {x}")));
    let before = snapshot(&t);
    let correction = json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter per drawing" });
    for point in ["correct.after_persist", "correct.in_obligations"] {
        t.set_fault(Some(point));
        let (st, v) = t
            .call(
                "POST",
                &format!("/api/calculations/{a}/corrections"),
                Some(correction.clone()),
            )
            .await;
        assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR, "{point}: {v}");
        assert!(
            v["error"].is_string(),
            "a JSON error, not a dropped connection"
        );
        assert_eq!(snapshot(&t), before, "{point}: partial state");
        let d = t.get(&format!("/api/calculations/{a}")).await;
        assert!(
            d["superseded_by"].is_null() && d["state"] == "REVIEW_REQUIRED",
            "{point}: the old job must not be superseded"
        );
    }
    t.set_fault(Some("calculate.after_persist"));
    assert_eq!(t.calc(cone()).await.0, StatusCode::INTERNAL_SERVER_ERROR);
    t.set_fault(Some("upload.after_document"));
    let (st, _, _) = t
        .send(
            "POST",
            "/api/projects/1/documents",
            None,
            &[("x-filename", "a.pdf")],
        )
        .await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "empty upload is refused at the membrane"
    );
    let pdf = HttpRequest::builder()
        .method("POST")
        .uri("/api/projects/1/documents")
        .header("host", "localhost")
        .header("x-nf-request", "1")
        .header("cookie", &t.cookie)
        .body(Body::from("%PDF-1.7\nfault test"))
        .unwrap();
    assert_eq!(
        router(t.app.clone()).oneshot(pdf).await.unwrap().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    t.set_fault(None);
    let mut expect = before;
    expect[5] += 1; // only the audit entry of the refused empty upload
    assert_eq!(snapshot(&t), expect);
    // without the fault the same correction goes through completely
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/calculations/{a}/corrections"),
            Some(correction),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM obligations"),
        v["obligations"].as_i64().unwrap()
    );
    assert_eq!(t.get("/api/audit/verify").await["intact"], true);
}

// ---------------------------------------------------------------- BE-006

#[tokio::test]
async fn be006_blobs_are_atomic_verified_and_repaired() {
    let mut t = boot();
    let id = t.job(cone()).await;
    let d = t.get(&format!("/api/calculations/{id}")).await;
    let sha = d["patterns"][0]["dxf_sha256"].as_str().unwrap().to_string();
    let path = storage::blob_path(&t.dir, &sha);
    let blobs_dir = t.dir.join("blobs");
    let blobs = || {
        std::fs::read_dir(&blobs_dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    assert!(
        blobs().iter().all(|n| !n.starts_with(".tmp-")),
        "no temporary file is left behind: {:?}",
        blobs()
    );
    // a torn write: the file exists but is empty
    std::fs::write(&path, b"").unwrap();
    assert!(matches!(
        storage::load_blob(&t.dir, &t.app.key, &sha),
        Err(storage::BlobError::Corrupt(_))
    ));
    // the identical job is run again: the blob is rewritten, not trusted because it "exists"
    let again = t.job(cone()).await;
    assert!(storage::load_blob(&t.dir, &t.app.key, &sha).is_ok());
    replay_matches(&mut t, again).await;
    assert_eq!(t.dxf_status(again, "reducer").await, StatusCode::OK);
    // damaged again, and this time only read: restored by recomputation with the identical hash
    replay_matches(&mut t, id).await;
    std::fs::write(
        &path,
        b"garbage that is long enough to look like a sealed blob",
    )
    .unwrap();
    let (st, _, bytes) = t
        .send(
            "GET",
            &format!("/api/calculations/{id}/dxf/reducer"),
            None,
            &[],
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    assert_eq!(nf_core::sha256_hex(&bytes), sha);
    assert!(String::from_utf8_lossy(&bytes).contains(nf_dxf_mark()));
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'stored_file_restored'"),
        1
    );
    // missing entirely: same recovery
    std::fs::remove_file(&path).unwrap();
    assert_eq!(t.dxf_status(id, "reducer").await, StatusCode::OK);
    assert!(blobs().iter().all(|n| !n.starts_with(".tmp-")));
    // a file that cannot be restored gives a clear JSON error, and recovers as soon as the content is stored again
    let (doc_sha, _) = storage::store_blob(&t.dir, &t.app.key, b"%PDF-1.7 a document").unwrap();
    std::fs::write(storage::blob_path(&t.dir, &doc_sha), b"").unwrap();
    assert!(storage::load_blob(&t.dir, &t.app.key, &doc_sha).is_err());
    assert_eq!(
        storage::store_blob(&t.dir, &t.app.key, b"%PDF-1.7 a document").unwrap(),
        (doc_sha.clone(), true)
    );
    assert_eq!(
        storage::load_blob(&t.dir, &t.app.key, &doc_sha).unwrap(),
        b"%PDF-1.7 a document"
    );
    // BE-032: a tampered, short file reference is a JSON error, not a panic
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.patterns[0].dxf_sha256', 'abc') WHERE id = {again}"));
    let (st, v) = t
        .call(
            "GET",
            &format!("/api/calculations/{again}/dxf/reducer"),
            None,
        )
        .await;
    // RC-01: the check against the records rejects the job first (was 500 "invalid file reference")
    assert_eq!(st, StatusCode::CONFLICT);
    assert!(v["error"].as_str().unwrap().contains("integrity check"));
}

fn nf_dxf_mark() -> &'static str {
    "NOT RELEASED - DEVELOPMENT BUILD"
}

/// Owner decision, 24 Nov 2026: a part's DXF is handed out only while every check on its cut
/// geometry is decided. A header_with_branch assembly whose checks are all decided downloads with
/// 200 and reports an empty `dxf_withheld`; a cylinder_shell whose surface reference is unknown
/// leaves its cut geometry undecided, so its part is listed in `dxf_withheld` and answers 409.
#[tokio::test]
async fn dxf_is_withheld_only_while_a_check_on_the_cut_geometry_is_undecided() {
    let mut t = boot();
    // every check decided: nothing is withheld and the file is served
    let id = t.job(good_assembly()).await;
    // G19 is NOT_RUN until the replay audit has run; a DXF is held by the export rule until then
    replay_matches(&mut t, id).await;
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert!(
        d["dxf_withheld"].as_object().unwrap().is_empty(),
        "nothing is withheld: {d}"
    );
    let mut parts: Vec<String> = Vec::new();
    for r in d["records"].as_array().cloned().unwrap_or_default() {
        for c in r["payload"]["checks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            if let Some(scope) = c["scope"].as_str() {
                parts.push(scope.to_string());
            }
        }
    }
    for p in d["patterns"].as_array().cloned().unwrap_or_default() {
        if let Some(name) = p["part"].as_str().or_else(|| p["name"].as_str()) {
            parts.push(name.to_string());
        }
    }
    parts.retain(|s| !s.is_empty() && s.as_str() != "interface");
    parts.sort();
    parts.dedup();
    assert!(!parts.is_empty(), "the job names its parts: {d}");
    let mut served = 0;
    for part in &parts {
        let (st, _, bytes) = t
            .send(
                "GET",
                &format!("/api/calculations/{id}/dxf/{part}"),
                None,
                &[],
            )
            .await;
        assert_ne!(st, StatusCode::CONFLICT, "part {part} is decided");
        if st == StatusCode::OK {
            served += 1;
            assert!(String::from_utf8_lossy(&bytes).contains(nf_dxf_mark()));
        }
    }
    assert!(served > 0, "the assembly DXF is served: {parts:?}");
    // a shell whose surface reference is unknown: its cut geometry stays undecided
    let (st, v) = t
        .calc(json!({ "family": "cylinder_shell", "r_dev": 150, "length": 1200, "surface_reference": "unknown" }))
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let shell = v["id"].as_i64().unwrap();
    let d = t.get(&format!("/api/calculations/{shell}")).await;
    let withheld = d["dxf_withheld"].as_object().unwrap();
    assert!(!withheld.is_empty(), "the undecided part is listed: {d}");
    for (part, names) in withheld {
        let names = names.as_array().unwrap();
        assert!(!names.is_empty());
        let (st, v) = t
            .call(
                "GET",
                &format!("/api/calculations/{shell}/dxf/{part}"),
                None,
            )
            .await;
        assert_eq!(st, StatusCode::CONFLICT, "{v}");
        assert_eq!(v["code"], "dxf_withheld_inconclusive");
        let err = v["error"].as_str().unwrap();
        assert!(err.contains("could not be confirmed"));
        assert_eq!(v["checks"].as_array().unwrap().len(), names.len());
        // every withheld check says why it is not confirmed (R3-8: an empty reason is never sent)
        for c in v["checks"].as_array().unwrap() {
            assert!(!c["reason"].as_str().unwrap_or("").is_empty(), "{c}");
        }
    }
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'dxf_withheld'") >= 1);
}

// ---------------------------------------------------------------- BE-007

#[tokio::test]
async fn be007_a_busy_database_gives_503_json_and_no_partial_rows() {
    let t = boot();
    t.app.db().busy_timeout(Duration::from_millis(100)).unwrap();
    let other = Connection::open(t.dir.join("nexus-fabric.sqlite")).unwrap();
    other.execute_batch("BEGIN IMMEDIATE").unwrap();
    let projects = t.count("SELECT COUNT(*) FROM projects");
    for (method, path, body) in [
        ("POST", "/api/projects", Some(json!({ "name": "busy" }))),
        ("POST", "/api/projects/1/calculations", Some(cone())),
        ("POST", "/api/logout", None),
    ] {
        let (st, headers, bytes) = t.send(method, path, body, &[]).await;
        let v: Value = serde_json::from_slice(&bytes).expect("a JSON body, not an empty reply");
        assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "{method} {path}: {v}");
        assert_eq!(v["retry"], true);
        assert!(headers.contains_key("retry-after"));
    }
    // reads are not blocked by a writer (write-ahead log)
    assert_eq!(t.call("GET", "/api/projects", None).await.0, StatusCode::OK);
    other.execute_batch("ROLLBACK").unwrap();
    assert_eq!(t.count("SELECT COUNT(*) FROM projects"), projects);
    assert_eq!(t.count("SELECT COUNT(*) FROM calculations"), 0);
    assert_eq!(
        t.call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "free again" }))
        )
        .await
        .0,
        StatusCode::OK
    );
}

/// BE-007: the product code has no `.unwrap()` and no `.expect(` at all, so no
/// request can reach one. (Lock poisoning is recovered with `unwrap_or_else`.)
#[test]
fn be007_no_unwrap_or_expect_in_product_code() {
    for (name, src) in [
        ("main.rs", include_str!("main.rs")),
        ("storage.rs", include_str!("storage.rs")),
        ("membrane.rs", include_str!("membrane.rs")),
        ("orders.rs", include_str!("orders.rs")),
        ("crypto.rs", include_str!("crypto.rs")),
        ("licence.rs", include_str!("licence.rs")),
    ] {
        let product = src.split("#[cfg(test)]\nmod tests {").next().unwrap();
        for (n, line) in product.lines().enumerate() {
            let code = line.split("//").next().unwrap();
            assert!(
                !code.contains(".unwrap()") && !code.contains(".expect("),
                "{name}:{}: {line}",
                n + 1
            );
        }
    }
}

// ---------------------------------------------------------------- BE-008

#[tokio::test]
async fn be008_replay_infrastructure_failure_is_not_an_integrity_failure() {
    let mut t = boot();
    let id = t.job(cone()).await;
    let untouched = |t: &T| {
        let (state, replay): (String, Option<String>) = t
            .app
            .db()
            .query_row(
                "SELECT state, replay_json FROM calculations WHERE id = ?",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((state.as_str(), replay), ("REVIEW_REQUIRED", None));
        assert_eq!(t.count("SELECT COUNT(*) FROM lessons"), 0);
        assert_eq!(
            t.count("SELECT COUNT(*) FROM records WHERE kind = 'replay_audit'"),
            0
        );
    };
    let other_engine = format!(
        "cat >/dev/null; echo '{}'",
        json!({ "engine": "nf-engine 9.9.9", "closure_hash": "x", "ids": [] })
    );
    let cases: [(&str, String); 5] = [
        ("missing executable", String::new()),
        ("crash", "cat >/dev/null; echo boom >&2; exit 3".into()),
        ("garbage output", "cat >/dev/null; echo not-json".into()),
        ("another build", other_engine),
        ("hang", "exec sleep 30".into()),
    ];
    for (what, script) in cases {
        if script.is_empty() {
            Arc::get_mut(&mut t.app).unwrap().replay_exe = Some(t.dir.join("no-such-executable"));
        } else {
            t.with_replay_exe(&script);
        }
        Arc::get_mut(&mut t.app).unwrap().replay_timeout =
            Duration::from_millis(if what == "hang" { 300 } else { 10_000 });
        let (st, v) = t
            .call("POST", &format!("/api/calculations/{id}/replay"), None)
            .await;
        assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "{what}: {v}");
        assert_eq!(v["infrastructure"], true, "{what}");
        untouched(&t);
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(
            (d["state"].as_str(), gate_of(&d, "G19")["result"].as_str()),
            (Some("REVIEW_REQUIRED"), Some("NOT_RUN")),
            "{what}"
        );
        assert_eq!(
            t.dxf_status(id, "reducer").await,
            StatusCode::CONFLICT,
            "{what}"
        );
        assert_eq!(d["export"]["held_by"]["gate"], "G19", "{what}");
    }
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'replay_unavailable'"),
        5
    );
    // the next identical job carries no advisory
    assert_eq!(t.calc(cone()).await.1["advisories"], 0);
    // a replay that RAN and differs is an integrity failure: REJECTED, lesson, files withheld
    t.with_replay_exe(&format!(
        "cat >/dev/null; echo '{}'",
        json!({ "engine": nf_core::ENGINE_VERSION, "closure_hash": "0000", "ids": [] })
    ));
    let (st, v) = t
        .call("POST", &format!("/api/calculations/{id}/replay"), None)
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["matches"], false);
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(
        (d["state"].as_str(), gate_of(&d, "G19")["result"].as_str()),
        (Some("REJECTED"), Some("FAIL"))
    );
    assert_eq!(t.dxf_status(id, "reducer").await, StatusCode::CONFLICT);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM lessons WHERE kind = 'integrity_failure'"),
        1
    );
    // a replay that ran and matches: the real closure, reported by the child
    let other = t
        .job(json!({ "family": "cylinder_shell", "r_dev": 100, "length": 250 }))
        .await;
    let stored = stored_result(&t.app.db(), other).unwrap().unwrap();
    let ids: Vec<&str> = stored["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    t.with_replay_exe(&format!("cat >/dev/null; echo '{}'", json!({ "engine": nf_core::ENGINE_VERSION, "closure_hash": stored["closure_hash"], "ids": ids })));
    let (st, v) = t
        .call("POST", &format!("/api/calculations/{other}/replay"), None)
        .await;
    assert_eq!(
        (st, v["matches"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{v}"
    );
    let d = t.get(&format!("/api/calculations/{other}")).await;
    assert_eq!(
        (d["state"].as_str(), gate_of(&d, "G19")["result"].as_str()),
        (Some("REVIEW_REQUIRED"), Some("INCONCLUSIVE"))
    );
    assert_eq!(d["export"]["available"], true);
    assert_eq!(t.dxf_status(other, "shell").await, StatusCode::OK);
    // a report written by an earlier build for a process failure no longer condemns the job
    t.sql(&format!("UPDATE calculations SET replay_json = '{{\"matches\":false,\"process_error\":\"No such file or directory (os error 2)\"}}', state = 'REJECTED' WHERE id = {other}"));
    let d = t.get(&format!("/api/calculations/{other}")).await;
    assert_eq!(
        (d["state"].as_str(), gate_of(&d, "G19")["result"].as_str()),
        (Some("REVIEW_REQUIRED"), Some("NOT_RUN"))
    );
    // an unreadable report fails closed
    t.sql(&format!(
        "UPDATE calculations SET replay_json = 'garbage' WHERE id = {other}"
    ));
    assert_eq!(
        t.get(&format!("/api/calculations/{other}")).await["state"],
        "REJECTED"
    );
    assert_consistent_and_never_approved(&t).await;
}

// ---------------------------------------------------------------- BE-009

#[tokio::test]
async fn be009_list_project_and_dashboard_agree_with_the_detail_view() {
    let t = boot();
    let a = t.job(good_assembly()).await;
    let _plain = t.job(cone()).await;
    let _ = t
        .calc(assembly("outside", -150.0, 150.0, 100.0, 100.0))
        .await; // a refusal
    let (_, v) = t.call("POST", &format!("/api/calculations/{a}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter per drawing" }))).await;
    let n = v["new_calculation"].as_i64().unwrap();
    let dash = t.get("/api/dashboard").await;
    assert_eq!(
        (
            dash["calculations"].as_i64(),
            dash["review_required"].as_i64(),
            dash["superseded"].as_i64(),
            dash["refused"].as_i64(),
            dash["rejected"].as_i64()
        ),
        (Some(4), Some(2), Some(1), Some(1), Some(0)),
        "{dash}"
    );
    for rows in [
        t.get("/api/calculations").await,
        t.get("/api/projects/1").await["calculations"].clone(),
        dash["recent"].clone(),
    ] {
        let old = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == a)
            .unwrap()
            .clone();
        assert_eq!(
            (old["state"].as_str(), old["superseded"].as_bool()),
            (Some("REJECTED"), Some(true)),
            "the corrected job is superseded everywhere"
        );
        let new = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == n)
            .unwrap()
            .clone();
        assert_eq!(
            (new["state"].as_str(), new["open_obligations"].as_i64()),
            (Some("REVIEW_REQUIRED"), Some(3))
        );
    }
    assert_consistent_and_never_approved(&t).await;
    // an event that changes the state (a repeated mistake acknowledged, obligations resolved) keeps them in step
    let again = t.job(good_assembly()).await;
    assert_eq!(
        gate_of(&t.get(&format!("/api/calculations/{again}")).await, "G20")["result"],
        "INCONCLUSIVE"
    );
    let adv = t.get(&format!("/api/calculations/{again}")).await["advisories"][0]["id"]
        .as_i64()
        .unwrap();
    assert_eq!(
        t.call(
            "POST",
            &format!("/api/advisories/{adv}/acknowledge"),
            Some(json!({ "reason": "this header is dimensioned outside" }))
        )
        .await
        .0,
        StatusCode::OK
    );
    for o in t
        .get("/api/obligations")
        .await
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["state"] == "open")
    {
        assert_eq!(
            t.call(
                "POST",
                &format!("/api/obligations/{}/resolve", o["id"]),
                Some(json!({ "decision": "confirmed", "reason": "checked against the drawing" }))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert_consistent_and_never_approved(&t).await;
    // a state column that drifted (older build, manual edit) is corrected by the next list
    t.sql("UPDATE calculations SET state = 'APPROVED'");
    assert_consistent_and_never_approved(&t).await;
    assert_eq!(
        t.count("SELECT COUNT(*) FROM calculations WHERE state = 'APPROVED'"),
        0
    );
}

// ---------------------------------------------------------------- BE-010

#[test]
fn be010_missing_or_wrong_key_with_existing_data_stops_startup() {
    let dir = temp_dir("key");
    let key = dir.join("keys").join("storage.key");
    // first start of an empty installation creates the key, owner-only
    let (db, k1) = storage::open_storage(&dir, &key).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(dir.join("nexus-fabric.sqlite"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    db.execute("INSERT INTO users(username, role, salt, hash, created_at) VALUES ('admin','admin','00','x',0)", []).unwrap();
    storage::store_blob(&dir, &k1, b"a stored drawing").unwrap();
    drop(db);
    assert_eq!(
        storage::open_storage(&dir, &key).unwrap().1,
        k1,
        "a normal restart keeps the key"
    );
    // key file missing (typo in NF_KEY_FILE, unmounted directory)
    let typo = dir.join("keys").join("typo.key");
    let e = storage::open_storage(&dir, &typo).unwrap_err();
    assert!(
        e.contains("MISSING") && e.contains("No key was created") && e.contains("typo.key"),
        "{e}"
    );
    assert!(!typo.exists(), "no key may be minted");
    // a different key
    std::fs::write(&typo, [7u8; 32]).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&typo, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(
        storage::open_storage(&dir, &typo)
            .unwrap_err()
            .contains("not the key this data directory was created with")
    );
    // wrong length, and (V-42) a key every local user can read
    std::fs::write(&typo, [7u8; 5]).unwrap();
    assert!(
        storage::open_storage(&dir, &typo)
            .unwrap_err()
            .contains("wrong length")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            storage::open_storage(&dir, &key)
                .unwrap_err()
                .contains("readable by every local user")
        );
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    // a database from before fingerprints: the key is proven against the stored files
    let db = Connection::open(dir.join("nexus-fabric.sqlite")).unwrap();
    db.execute("DELETE FROM meta WHERE key = 'key_fingerprint'", [])
        .unwrap();
    drop(db);
    std::fs::write(&typo, [9u8; 32]).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&typo, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(
        storage::open_storage(&dir, &typo)
            .unwrap_err()
            .contains("does not decrypt")
    );
    assert_eq!(storage::open_storage(&dir, &key).unwrap().1, k1);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- BE-011

#[tokio::test]
async fn be011_results_of_a_superseded_engine_are_flagged_and_withheld() {
    let t = boot();
    let old = t.job(good_assembly()).await;
    let current = t.job(cone()).await;
    assert_eq!(
        t.get(&format!("/api/calculations/{current}")).await["engine"],
        nf_core::ENGINE_VERSION
    );
    let col: String = t
        .app
        .db()
        .query_row(
            "SELECT engine FROM calculations WHERE id = ?",
            params![current],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        col,
        nf_core::ENGINE_VERSION,
        "the engine is recorded per result"
    );
    // a result as stored by the build before the header-outline fix: same shape, the old identifier
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-core 0.1.0 / uem 0.1.0'), engine = 'nf-core 0.1.0 / uem 0.1.0' WHERE id = {old}"));
    let d = t.get(&format!("/api/calculations/{old}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert_eq!(d["engine_current"], false);
    let g = gate_of(&d, "G21");
    assert_eq!(g["result"], "FAIL");
    let detail = g["detail"].as_str().unwrap();
    assert!(
        detail.contains(
            "Computed by a superseded engine version (nf-core 0.1.0 / uem 0.1.0) — recompute"
        ) && detail.contains("Defect 002"),
        "{detail}"
    );
    assert_eq!(d["correctable_parameters"], json!([]));
    assert_eq!(t.dxf_status(old, "header").await, StatusCode::CONFLICT);
    let (st, v) = t
        .call("POST", &format!("/api/calculations/{old}/replay"), None)
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert!(v["error"].as_str().unwrap().contains("recompute"));
    assert_eq!(t.call("POST", &format!("/api/calculations/{old}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter" }))).await.0, StatusCode::CONFLICT);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM lessons"),
        0,
        "an old engine is not an integrity failure of the job"
    );
    let row = t
        .get("/api/calculations")
        .await
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == old)
        .cloned()
        .unwrap();
    assert_eq!(
        (row["state"].as_str(), row["engine_current"].as_bool()),
        (Some("REJECTED"), Some(false))
    );
    // the current result is untouched
    assert_eq!(
        t.get(&format!("/api/calculations/{current}")).await["state"],
        "REVIEW_REQUIRED"
    );
    assert!(gate_of(&t.get(&format!("/api/calculations/{current}")).await, "G21").is_null());
    assert_consistent_and_never_approved(&t).await;
}

#[tokio::test]
async fn be011_upgrade_migration_flags_old_results_without_rewriting_them() {
    let t = boot();
    let id = t.job(good_assembly()).await;
    let refused = t
        .calc(assembly("outside", -150.0, 150.0, 100.0, 100.0))
        .await
        .1["id"]
        .as_i64()
        .unwrap();
    // turn the data directory into what the previous build left behind
    t.sql("UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-core 0.1.0 / uem 0.1.0'), state = 'REVIEW_REQUIRED'");
    let stored: String = t
        .app
        .db()
        .query_row(
            "SELECT result_json FROM calculations WHERE id = ?",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    t.app.db().execute_batch("ALTER TABLE calculations DROP COLUMN engine; ALTER TABLE calculations DROP COLUMN state_by; ALTER TABLE calculations DROP COLUMN core_json; ALTER TABLE calculations DROP COLUMN result_bytes; DELETE FROM meta WHERE key LIKE 'audit_%'; PRAGMA user_version = 0;").unwrap();
    // the same data directory, opened as the upgraded server would open it. The audit log here has entries but no
    // anchor (it predates the anchor), which only an explicit acceptance may anchor (RA-003)
    let app = build_app_with(config(&t.dir), true).unwrap();
    let db = app.db();
    let (engine, state, after): (String, String, String) = db
        .query_row(
            "SELECT engine, state, result_json FROM calculations WHERE id = ?",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(engine, "nf-core 0.1.0 / uem 0.1.0");
    assert_eq!(
        state, "REJECTED",
        "an old-engine result may no longer be listed as review required"
    );
    assert_eq!(
        after, stored,
        "the stored engineering result is not rewritten"
    );
    let refused_state: String = db
        .query_row(
            "SELECT state FROM calculations WHERE id = ?",
            params![refused],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        refused_state, "REFUSED",
        "a refusal stored as REVIEW_REQUIRED by the old build is now REFUSED"
    );
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        storage::SCHEMA_VERSION
    );
    assert_eq!(
        verify_audit(&app, &db).unwrap()["intact"],
        true,
        "an existing log is anchored at upgrade"
    );
}

// ---------------------------------------------------------------- DXF

#[tokio::test]
async fn dxf_download_is_marked_not_released_and_only_for_reviewable_current_jobs() {
    let mut t = boot();
    let id = t.job(good_assembly()).await;
    replay_matches(&mut t, id).await;
    let (st, headers, bytes) = t
        .send(
            "GET",
            &format!("/api/calculations/{id}/dxf/header"),
            None,
            &[],
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        headers["content-disposition"]
            .to_str()
            .unwrap()
            .contains("NOT-RELEASED_REVIEW_REQUIRED")
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.contains("NF_NOT_RELEASED")
            && text.contains(nf_dxf_mark())
            && text.contains("$LUNITS")
            && text.contains("UNITS: MILLIMETRES")
    );
    assert_eq!(t.dxf_status(id, "nonexistent").await, StatusCode::NOT_FOUND);
    assert_eq!(t.dxf_status(9999, "header").await, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------- V-25

#[tokio::test]
async fn v25_removal_of_the_newest_audit_entries_is_detected() {
    let t = boot();
    for i in 0..6 {
        assert_eq!(
            t.call(
                "POST",
                "/api/projects",
                Some(json!({ "name": format!("p{i}") }))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let v = t.get("/api/audit/verify").await;
    assert_eq!(
        (
            v["intact"].as_bool(),
            v["anchor_intact"].as_bool(),
            v["entries"].as_i64()
        ),
        // six projects plus the licence status entry written at start-up
        (Some(true), Some(true), Some(7))
    );
    assert!(v["limitation"].as_str().unwrap().contains("rollback"));
    attacker_drops_triggers(&t);
    t.sql("DELETE FROM audit WHERE id > 4");
    let v = t.get("/api/audit/verify").await;
    assert_eq!(
        (
            v["intact"].as_bool(),
            v["chain_intact"].as_bool(),
            v["anchor_intact"].as_bool()
        ),
        (Some(false), Some(true), Some(false)),
        "{v}"
    );
    assert!(v["anchor_problem"].as_str().unwrap().contains("removed"));
    // an attacker without the storage key cannot write a matching anchor
    let head: String = t
        .app
        .db()
        .query_row("SELECT hash FROM audit WHERE id = 4", [], |r| r.get(0))
        .unwrap();
    t.app
        .db()
        .execute(
            "UPDATE meta SET value = ? WHERE key = 'audit_anchor'",
            params![json!({ "head": head, "count": 4, "last_id": 4, "mac": "00" }).to_string()],
        )
        .unwrap();
    let v = t.get("/api/audit/verify").await;
    assert_eq!(v["intact"], false);
    assert!(v["anchor_problem"].as_str().unwrap().contains("altered"));
    t.sql("DELETE FROM meta WHERE key = 'audit_anchor'");
    assert_eq!(
        t.get("/api/audit/verify").await["intact"],
        false,
        "a missing anchor is not an intact log"
    );
    // an altered entry inside the chain is still reported at its position
    let t2 = boot();
    for i in 0..3 {
        t2.call(
            "POST",
            "/api/projects",
            Some(json!({ "name": format!("p{i}") })),
        )
        .await;
    }
    attacker_drops_triggers(&t2);
    t2.sql("UPDATE audit SET detail = 'tampered' WHERE id = 2");
    let v = t2.get("/api/audit/verify").await;
    assert_eq!(
        (v["intact"].as_bool(), v["first_broken_entry"].as_i64()),
        (Some(false), Some(2))
    );
}

// ---------------------------------------------------------------- V-33

async fn login_as(t: &T, name: &str, password: &str, extra: &[(&str, &str)]) -> StatusCode {
    let mut h: Vec<(&str, &str)> = vec![("cookie", "none=1")];
    h.extend_from_slice(extra);
    t.send(
        "POST",
        "/api/login",
        Some(json!({ "username": name, "password": password })),
        &h,
    )
    .await
    .0
}

fn add_real_user(t: &T, name: &str, password: &str) {
    let salt = [1u8; 16];
    let hash = crypto::hash_password(password, &salt).unwrap();
    t.app
        .db()
        .execute(
            "INSERT INTO users(username, role, salt, hash, created_at) VALUES (?,?,?,?,0)",
            params![name, "engineer", hex::encode(salt), hash],
        )
        .unwrap();
}

#[tokio::test]
async fn v33_failures_by_other_names_do_not_lock_a_user_out_but_guessing_one_account_does() {
    let t = boot();
    add_real_user(&t, "carol", "carols-long-password");
    // 45 failed sign-ins under 45 invented names (inserted as the handler would, to keep the test fast)
    for i in 0..45 {
        t.app
            .db()
            .execute(
                "INSERT INTO login_failures(username, at, client) VALUES (?,?,?)",
                params![format!("nobody-{i}"), now() as i64, "unknown"],
            )
            .unwrap();
    }
    assert_eq!(
        login_as(&t, "nobody-99", "x", &[]).await,
        StatusCode::UNAUTHORIZED,
        "still answered, not globally blocked"
    );
    assert_eq!(
        login_as(&t, "carol", "carols-long-password", &[]).await,
        StatusCode::OK,
        "a correct sign-in is not blocked by other names' failures"
    );
    // guessing one account is still stopped after 5 failures, even for the right password
    for i in 0..5 {
        assert_eq!(
            login_as(&t, "carol", &format!("guess-{i}"), &[]).await,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        login_as(&t, "carol", "carols-long-password", &[]).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // one client address that keeps failing is throttled on its own
    t.sql("DELETE FROM login_failures");
    for i in 0..PER_CLIENT_FAILURES {
        t.app
            .db()
            .execute(
                "INSERT INTO login_failures(username, at, client) VALUES (?,?,?)",
                params![format!("spray-{i}"), now() as i64, "unknown"],
            )
            .unwrap();
    }
    assert_eq!(
        login_as(&t, "carol", "carols-long-password", &[]).await,
        StatusCode::TOO_MANY_REQUESTS,
        "this client is throttled"
    );
    t.sql("UPDATE login_failures SET client = '203.0.113.9'");
    assert_eq!(
        login_as(&t, "carol", "carols-long-password", &[]).await,
        StatusCode::OK,
        "another client is not"
    );
}

#[test]
fn v33_forwarded_address_is_used_only_from_a_trusted_proxy() {
    let dir = temp_dir("xff");
    let mut cfg = config(&dir);
    let app = build_app(Config {
        trusted_proxies: vec![],
        ..config(&dir)
    })
    .unwrap();
    let mut h = HeaderMap::new();
    h.insert(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.7, 203.0.113.5"),
    );
    let peer: IpAddr = "127.0.0.1".parse().unwrap();
    assert_eq!(
        client_address(&app, Some(peer), &h),
        "127.0.0.1",
        "the header is never trusted unless configured"
    );
    drop(app);
    cfg.trusted_proxies = vec![peer];
    let app = build_app(cfg).unwrap();
    assert_eq!(
        client_address(&app, Some(peer), &h),
        "203.0.113.5",
        "right-most hop that is not the proxy"
    );
    assert_eq!(
        client_address(&app, Some("192.0.2.1".parse().unwrap()), &h),
        "192.0.2.1",
        "a direct client cannot choose its own address"
    );
    assert_eq!(
        client_address(&app, Some(peer), &HeaderMap::new()),
        "127.0.0.1"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- V-34

#[tokio::test]
async fn v34_attacker_controlled_strings_are_bounded_and_anonymous_audit_is_rate_limited() {
    let t = boot();
    // the window is pinned: the 70 events fall into one minute whatever the clock says (no boundary flake)
    *t.app.anon_minute.lock().unwrap() = Some(now() / 60);
    let long = "A".repeat(200_000);
    let before = t.count("SELECT COUNT(*) FROM audit");
    for _ in 0..(ANON_AUDIT_PER_MINUTE as usize + 40) {
        assert_ne!(login_as(&t, &long, "x", &[]).await, StatusCode::OK);
    }
    let written = t.count("SELECT COUNT(*) FROM audit") - before;
    assert!(
        written <= ANON_AUDIT_PER_MINUTE as i64 + 2,
        "{written} rows for 70 anonymous events"
    );
    assert!(t.count("SELECT MAX(LENGTH(username)) FROM audit") <= 64);
    assert!(t.count("SELECT MAX(LENGTH(detail)) FROM audit") <= 1000);
    assert!(t.count("SELECT MAX(LENGTH(username)) FROM login_failures") <= 64);
    // user names are bounded at the API
    let (st, v) = t.call("POST", "/api/users", Some(json!({ "username": "b".repeat(65), "password": "long-enough-password", "role": "viewer" }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
    // free text that goes into immutable records is bounded (BE-029)
    assert_eq!(
        t.call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "x", "description": "d".repeat(3000) }))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // the listing is paginated and small
    let (st, _, bytes) = t.send("GET", "/api/audit?limit=5", None, &[]).await;
    let rows: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!((st, rows.as_array().unwrap().len()), (StatusCode::OK, 5));
    let older = t
        .get(&format!("/api/audit?limit=5&before={}", rows[4]["id"]))
        .await;
    assert!(older[0]["id"].as_i64().unwrap() < rows[4]["id"].as_i64().unwrap());
    assert!(t.send("GET", "/api/audit", None, &[]).await.2.len() < 1024 * 1024);
    assert_eq!(t.get("/api/audit/verify").await["intact"], true);
}

// ---------------------------------------------------------------- V-35

#[tokio::test]
async fn v35_upload_is_authorised_before_the_body_is_read_and_limited_while_streaming() {
    let t = boot();
    let viewer = add_session(&t.app, "vic", "viewer");
    let outsider = add_session(&t.app, "eve", "engineer");
    // A body that fails as soon as it is polled: if the server answers, it did not read it.
    let poisoned = || {
        Body::from_stream(Once(Some(Err(std::io::Error::other(
            "the body must not be read",
        )))))
    };
    for (cookie, expect) in [
        ("none=1".to_string(), StatusCode::UNAUTHORIZED),
        (viewer, StatusCode::FORBIDDEN),
        (outsider, StatusCode::NOT_FOUND),
    ] {
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/api/projects/1/documents")
            .header("host", "localhost")
            .header("x-nf-request", "1")
            .header("cookie", cookie)
            .body(poisoned())
            .unwrap();
        assert_eq!(
            router(t.app.clone()).oneshot(req).await.unwrap().status(),
            expect
        );
    }
    let post = |body: Body, len: Option<usize>| {
        let mut b = HttpRequest::builder()
            .method("POST")
            .uri("/api/projects/1/documents")
            .header("host", "localhost")
            .header("x-nf-request", "1")
            .header("cookie", &t.cookie)
            .header("x-filename", "a.pdf");
        if let Some(n) = len {
            b = b.header("content-length", n.to_string());
        }
        router(t.app.clone()).oneshot(b.body(body).unwrap())
    };
    // declared too large: refused from the header, body untouched
    assert_eq!(
        post(poisoned(), Some(membrane::MAX_UPLOAD_BYTES + 1))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    // too large without a declared length: refused while streaming
    let mut big = b"%PDF-1.7\n".to_vec();
    big.resize(membrane::MAX_UPLOAD_BYTES + 10, 0);
    assert_eq!(
        post(Body::from(big), None).await.unwrap().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM documents"), 0);
    // a normal upload works; when every slot is taken the answer is 503, not unbounded memory
    assert_eq!(
        post(Body::from("%PDF-1.7\nfine"), None)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let held: Vec<_> = (0..4)
        .map(|_| t.app.upload_slots.try_acquire().unwrap())
        .collect();
    assert_eq!(
        post(Body::from("%PDF-1.7\nfine"), None)
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    drop(held);
    assert_eq!(
        post(Body::from("%PDF-1.7\nfine again"), None)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

/// A one-item stream that yields an error: reading the body fails.
struct Once(Option<Result<axum::body::Bytes, std::io::Error>>);
impl futures_core::Stream for Once {
    type Item = Result<axum::body::Bytes, std::io::Error>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.0.take())
    }
}

// ---------------------------------------------------------------- V-36

#[tokio::test]
async fn v36_first_administrator_needs_the_setup_token_and_hosts_are_allow_listed() {
    let dir = temp_dir("setup");
    let app = build_app(config(&dir)).unwrap();
    let t = T {
        app,
        dir,
        cookie: "none=1".into(),
    };
    let admin = json!({ "username": "admin", "password": "correct-horse-9" });
    assert_eq!(t.get("/api/health").await["setup_required"], true);
    // no token, wrong token
    let (st, v) = t.call("POST", "/api/setup", Some(admin.clone())).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(
        t.send(
            "POST",
            "/api/setup",
            Some(admin.clone()),
            &[("x-nf-setup-token", "guess")]
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mut wrong = admin.clone();
    wrong["setup_token"] = json!("another-guess");
    assert_eq!(
        t.call("POST", "/api/setup", Some(wrong)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM users"), 0);
    // a foreign Host name (DNS rebinding) is refused for every request, even with the right token
    let rebind = [
        ("host", "rebind.attacker.example:8787"),
        ("x-nf-setup-token", TOKEN),
    ];
    assert_eq!(
        t.send("POST", "/api/setup", Some(admin.clone()), &rebind)
            .await
            .0,
        StatusCode::MISDIRECTED_REQUEST
    );
    assert_eq!(
        t.send(
            "GET",
            "/api/health",
            None,
            &[("host", "rebind.attacker.example")]
        )
        .await
        .0,
        StatusCode::MISDIRECTED_REQUEST
    );
    assert_eq!(
        t.send("GET", "/", None, &[("host", "evil.example")])
            .await
            .0,
        StatusCode::MISDIRECTED_REQUEST
    );
    for good in ["localhost:8787", "LOCALHOST", "127.0.0.1:1", "[::1]:8787"] {
        assert_eq!(
            t.send("GET", "/api/health", None, &[("host", good)])
                .await
                .0,
            StatusCode::OK,
            "{good}"
        );
    }
    assert_eq!(t.count("SELECT COUNT(*) FROM users"), 0);
    // weak password with the right token: refused, token not consumed
    assert_eq!(
        t.send(
            "POST",
            "/api/setup",
            Some(json!({ "username": "admin", "password": "short" })),
            &[("x-nf-setup-token", TOKEN)]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    // the right token, in the body this time
    let mut right = admin.clone();
    right["setup_token"] = json!(TOKEN);
    assert_eq!(
        t.call("POST", "/api/setup", Some(right.clone())).await.0,
        StatusCode::OK
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM users WHERE role = 'admin'"),
        1
    );
    assert!(
        t.app.setup_token.lock().unwrap().is_none(),
        "the token is consumed"
    );
    assert_eq!(
        t.call("POST", "/api/setup", Some(right)).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(t.get("/api/health").await["setup_required"], false);
    assert!(
        t.get("/api/health").await.get("engine").is_none()
            && t.get("/api/health").await.get("uptime_seconds").is_none(),
        "V-49"
    );
    // repeated wrong tokens are throttled
    let dir2 = temp_dir("setup2");
    let t2 = T {
        app: build_app(Config {
            setup_token: None,
            ..config(&dir2)
        })
        .unwrap(),
        dir: dir2,
        cookie: "none=1".into(),
    };
    let file = t2.dir.join("setup-token");
    let generated = std::fs::read_to_string(&file).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    for _ in 0..SETUP_FAILURES {
        assert_eq!(
            t2.call("POST", "/api/setup", Some(admin.clone())).await.0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        t2.send(
            "POST",
            "/api/setup",
            Some(admin.clone()),
            &[("x-nf-setup-token", generated.trim())]
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    t2.sql("DELETE FROM login_failures");
    assert_eq!(
        t2.send(
            "POST",
            "/api/setup",
            Some(admin),
            &[("x-nf-setup-token", generated.trim())]
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(!file.exists(), "the token file is deleted once used");
}

// ---------------------------------------------------------------- V-37, V-39, V-41, V-46

#[tokio::test]
async fn v37_v41_v46_password_change_throttle_audited_signout_and_password_policy() {
    let t = boot();
    add_real_user(&t, "dave", "daves-long-password");
    let cookie = {
        let db = t.app.db();
        let uid: i64 = db
            .query_row("SELECT id FROM users WHERE username = 'dave'", [], |r| {
                r.get(0)
            })
            .unwrap();
        db.execute(
            "INSERT INTO sessions(token_hash, user_id, expires_at) VALUES (?,?,?)",
            params![
                nf_core::sha256_hex(b"dave-token"),
                uid,
                (now() + 3600) as i64
            ],
        )
        .unwrap();
        "nf_session=dave-token".to_string()
    };
    let headers = [("cookie", cookie.as_str())];
    let change = async |current: &str| {
        t.send(
            "POST",
            "/api/password",
            Some(json!({ "current": current, "new": "a-brand-new-password" })),
            &headers,
        )
        .await
        .0
    };
    for i in 0..5 {
        assert_eq!(change(&format!("guess-{i}")).await, StatusCode::FORBIDDEN);
    }
    assert_eq!(
        change("daves-long-password").await,
        StatusCode::TOO_MANY_REQUESTS,
        "wrong current passwords are throttled"
    );
    assert_eq!(
        login_as(&t, "dave", "daves-long-password", &[]).await,
        StatusCode::OK,
        "sign-in has its own counter"
    );
    // policy
    for (name, pw) in [
        ("weak1", "password12"),
        ("samesame12", "samesame12"),
        ("short1", "short"),
    ] {
        assert_eq!(
            t.call(
                "POST",
                "/api/users",
                Some(json!({ "username": name, "password": pw, "role": "viewer" }))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST,
            "{name}"
        );
    }
    // sign-out and a refused project are in the audit log
    assert_eq!(
        t.send(
            "GET",
            "/api/projects/1",
            None,
            &[("cookie", cookie.as_str())]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        t.send("POST", "/api/logout", None, &[("cookie", cookie.as_str())])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'logout' AND username = 'dave'"),
        1
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'access_denied' AND username = 'dave'"),
        1
    );
    assert_eq!(
        t.send("GET", "/api/me", None, &[("cookie", cookie.as_str())])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn v39_secure_mode_uses_the_host_prefixed_cookie_everywhere() {
    let dir = temp_dir("cookie");
    let app = build_app(Config {
        cookie_secure: true,
        ..config(&dir)
    })
    .unwrap();
    *app.setup_token.lock().unwrap() = None;
    let t = T {
        app,
        dir,
        cookie: "none=1".into(),
    };
    add_real_user(&t, "erin", "erins-long-password");
    let (st, headers, _) = t
        .send(
            "POST",
            "/api/login",
            Some(json!({ "username": "erin", "password": "erins-long-password" })),
            &[],
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let set = headers["set-cookie"].to_str().unwrap().to_string();
    assert!(
        set.starts_with("__Host-nf_session=")
            && set.contains("Secure")
            && set.contains("HttpOnly")
            && set.contains("SameSite=Strict")
            && set.contains("Path=/"),
        "{set}"
    );
    let cookie = set.split(';').next().unwrap().to_string();
    assert_eq!(
        t.send("GET", "/api/me", None, &[("cookie", cookie.as_str())])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        t.send(
            "GET",
            "/api/me",
            None,
            &[("cookie", cookie.replace("__Host-", "").as_str())]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, headers, _) = t
        .send("POST", "/api/logout", None, &[("cookie", cookie.as_str())])
        .await;
    let out = headers["set-cookie"].to_str().unwrap();
    assert!(
        out.starts_with("__Host-nf_session=;") && out.contains("Secure"),
        "{out}"
    );
}

// ---------------------------------------------------------------- headers stay as they were

#[tokio::test]
async fn security_headers_and_anti_forgery_are_unchanged() {
    let t = boot();
    let (_, h, _) = t.send("GET", "/", None, &[]).await;
    assert_eq!(
        h["content-security-policy"],
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
    );
    assert_eq!(
        (
            h["x-frame-options"].to_str().unwrap(),
            h["x-content-type-options"].to_str().unwrap(),
            h["referrer-policy"].to_str().unwrap()
        ),
        ("DENY", "nosniff", "no-referrer")
    );
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/api/projects")
        .header("host", "localhost")
        .header("cookie", &t.cookie)
        .header("content-type", "application/json")
        .body(Body::from("{\"name\":\"x\"}"))
        .unwrap();
    assert_eq!(
        router(t.app.clone()).oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN,
        "missing anti-forgery header"
    );
    assert_eq!(
        t.send(
            "POST",
            "/api/projects",
            Some(json!({ "name": "x" })),
            &[("origin", "http://evil.example")]
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        t.send(
            "POST",
            "/api/projects",
            Some(json!({ "name": "x" })),
            &[("origin", "http://127.0.0.1:8787")]
        )
        .await
        .0,
        StatusCode::OK
    );
    // a refused host still carries the headers
    assert!(
        t.send("GET", "/", None, &[("host", "evil.example")])
            .await
            .1
            .contains_key("content-security-policy")
    );
}

// ---------------------------------------------------------------- S-2 (lost or damaged database)

#[test]
fn s2_an_empty_or_damaged_database_beside_an_existing_installation_stops_startup() {
    let dir = temp_dir("lostdb");
    let db_file = dir.join("nexus-fabric.sqlite");
    let wipe = |with_blobs: bool| {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(dir.join(format!("nexus-fabric.sqlite{suffix}")));
        }
        if !with_blobs {
            for p in std::fs::read_dir(dir.join("blobs"))
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "bin"))
            {
                std::fs::remove_file(p).unwrap();
            }
        }
    };
    // an installation that was started but never set up can be restarted as often as needed
    drop(build_app(config(&dir)).unwrap());
    drop(build_app(config(&dir)).unwrap());
    {
        let app = build_app(config(&dir)).unwrap();
        add_session(&app, "admin", "admin");
        storage::store_blob(&dir, &app.key, b"a stored drawing").unwrap();
        assert_eq!(
            verify_audit(&app, &app.db()).unwrap()["database_integrity"],
            "ok"
        );
    }
    assert!(build_app(config(&dir)).is_ok(), "a normal restart");
    // the database file is deleted (or truncated to nothing): key, marker and stored files remain
    wipe(true);
    let e = build_app(config(&dir))
        .map(|_| ())
        .expect_err("must refuse");
    assert!(
        e.contains("does not belong") && e.contains("Refusing to start in first-run mode"),
        "{e}"
    );
    // even with the stored files gone, the marker alone is enough
    wipe(false);
    assert!(
        build_app(config(&dir))
            .map(|_| ())
            .expect_err("must refuse")
            .contains("installation marker present")
    );
    // truncated to zero bytes: the same
    std::fs::write(&db_file, b"").unwrap();
    assert!(build_app(config(&dir)).is_err());
    // garbage instead of a database
    wipe(true);
    std::fs::write(&db_file, vec![0x5au8; 16384]).unwrap();
    let e = build_app(config(&dir))
        .map(|_| ())
        .expect_err("must refuse");
    assert!(e.contains("Refusing to start"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn s2_page_corruption_is_not_reported_as_an_intact_log() {
    let t = boot();
    for i in 0..40 {
        t.call(
            "POST",
            "/api/projects",
            Some(json!({ "name": format!("project {i}"), "description": "x".repeat(1500) })),
        )
        .await;
    }
    let v = t.get("/api/audit/verify").await;
    assert_eq!(
        (
            v["intact"].as_bool(),
            v["database_intact"].as_bool(),
            v["database_integrity"].as_str()
        ),
        (Some(true), Some(true), Some("ok"))
    );
    // overwrite pages in the middle of the file, as a failing disk would
    t.app
        .db()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let path = t.dir.join("nexus-fabric.sqlite");
    let mut bytes = std::fs::read(&path).unwrap();
    let n = bytes.len();
    for b in &mut bytes[n / 3..n / 3 + 3 * 4096] {
        *b = 0xa5;
    }
    std::fs::write(&path, &bytes).unwrap();
    // a fresh start refuses the file …
    let e = storage::open_storage(&t.dir, &t.dir.join("storage.key")).expect_err("must refuse");
    assert!(e.contains("Refusing to start"), "{e}");
    // … and a server that was already running does not call the log intact
    let app2 = Arc::new(App {
        db: Mutex::new(Connection::open(&path).unwrap()),
        key: t.app.key,
        data_dir: t.dir.clone(),
        started: 0,
        cookie_secure: false,
        key_outside_data_dir: false,
        allowed_hosts: default_hosts(),
        trusted_proxies: vec![],
        setup_token: Mutex::new(None),
        replay_exe: None,
        replay_timeout: Duration::from_secs(1),
        request_timeout: Duration::from_secs(30),
        desktop: false,
        body_idle: Duration::from_secs(10),
        per_client_in_flight: 32,
        in_flight: Mutex::new(Default::default()),
        hash_slots: Semaphore::new(1),
        auth: Default::default(),
        upload_slots: Semaphore::new(1),
        replay_slots: Semaphore::new(1),
        pdf2dxf_exe: None,
        convert_timeout: Duration::from_secs(1),
        convert_slots: Arc::new(Semaphore::new(1)),
        anon_audit: Mutex::new((0, 0, 0)),
        anon_minute: Mutex::new(None),
        verified: Mutex::new((-1, Default::default())),
        licence: Mutex::new(crate::licence::LicenceCache::default()),
        licence_keys: nf_licence::VERIFY_KEYS,
        licence_path: t.dir.join("licence.nflic"),
        licence_enforce: false,
        export_dirs: vec![],
        fault: Mutex::new(None),
    });
    let verdict = match verify_audit(&app2, &app2.db()) {
        Ok(v) => v["intact"].as_bool(),
        Err(_) => Some(false), // the handler turns an unreadable log into intact:false
    };
    assert_eq!(verdict, Some(false));
}

// ---------------------------------------------------------------- S-4 (storage full)

#[tokio::test]
async fn s4_a_failed_write_is_never_answered_with_success() {
    let t = boot();
    add_real_user(&t, "frank", "franks-long-password");
    let id = t.job(cone()).await;
    let tables = [
        "projects",
        "calculations",
        "sessions",
        "audit",
        "records",
        "login_failures",
    ];
    let snapshot = |t: &T| tables.map(|x| t.count(&format!("SELECT COUNT(*) FROM {x}")));
    // the database may not grow by a single page: every insert of any size fails with "full"
    t.app
        .db()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let pages: i64 = t
        .app
        .db()
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    let limit: i64 = t
        .app
        .db()
        .query_row(&format!("PRAGMA max_page_count = {pages}"), [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(limit, pages);
    let before = snapshot(&t);
    let big = "d".repeat(1900);
    let mut full = 0;
    let mut last = StatusCode::OK;
    for i in 0..60 {
        let (st, v) = t
            .call(
                "POST",
                "/api/projects",
                Some(json!({ "name": format!("p{i}"), "description": big })),
            )
            .await;
        last = st;
        assert!(
            st == StatusCode::OK || st == StatusCode::INSUFFICIENT_STORAGE,
            "{st}: {v}"
        );
        if st == StatusCode::INSUFFICIENT_STORAGE {
            assert_eq!(v["storage_full"], true);
            full += 1;
        }
    }
    assert!(
        full >= 20 && last == StatusCode::INSUFFICIENT_STORAGE,
        "free space inside existing pages absorbs a few rows, then every write must fail ({full})"
    );
    let (st, v) = t.calc(good_assembly()).await;
    assert_eq!(st, StatusCode::INSUFFICIENT_STORAGE, "{v}");
    // sign-in: no cookie for a session that could not be stored
    let (st, headers, _) = t
        .send(
            "POST",
            "/api/login",
            Some(json!({ "username": "frank", "password": "franks-long-password" })),
            &[("cookie", "none=1")],
        )
        .await;
    if st == StatusCode::OK {
        let cookie = headers["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        assert_eq!(
            t.send("GET", "/api/me", None, &[("cookie", cookie.as_str())])
                .await
                .0,
            StatusCode::OK,
            "a 200 sign-in must be a stored session"
        );
    } else {
        assert_eq!(st, StatusCode::INSUFFICIENT_STORAGE);
        assert!(!headers.contains_key("set-cookie"));
    }
    // no state change without its audit entry: every project that exists has its audit row, and nothing else changed
    let after = snapshot(&t);
    assert_eq!(
        after[0] - before[0],
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'project_created'")
    );
    assert_eq!(
        after[1], before[1],
        "no calculation row from the refused job"
    );
    assert_eq!(t.get("/api/audit/verify").await["intact"], true);
    // reads still work, and writes work again once there is room
    assert_eq!(
        t.get(&format!("/api/calculations/{id}")).await["state"],
        "REVIEW_REQUIRED"
    );
    t.app
        .db()
        .query_row("PRAGMA max_page_count = 1000000", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();
    assert_eq!(t.calc(good_assembly()).await.0, StatusCode::OK);
}

// ---------------------------------------------------------------- S-3 (time limits)

/// A request body that never arrives.
struct Never;
impl futures_core::Stream for Never {
    type Item = Result<axum::body::Bytes, std::io::Error>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Pending
    }
}

#[tokio::test]
async fn s3_a_request_whose_body_never_arrives_is_abandoned() {
    let dir = temp_dir("timeout");
    let app = build_app(Config {
        request_timeout: Duration::from_millis(300),
        ..config(&dir)
    })
    .unwrap();
    let cookie = add_session(&app, "admin", "admin");
    let t = T { app, dir, cookie };
    let started = std::time::Instant::now();
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/api/projects")
        .header("host", "localhost")
        .header("x-nf-request", "1")
        .header("cookie", &t.cookie)
        .header("content-type", "application/json")
        .header("content-length", "1000")
        .body(Body::from_stream(Never))
        .unwrap();
    let resp = router(t.app.clone()).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::REQUEST_TIMEOUT);
    assert!(resp.headers().contains_key("content-security-policy"));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(t.count("SELECT COUNT(*) FROM projects"), 0);
    assert_eq!(
        t.call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "afterwards" }))
        )
        .await
        .0,
        StatusCode::OK
    );
}

// ---------------------------------------------------------------- S-7 (listings)

#[tokio::test]
async fn s7_listing_uses_indexes_is_paginated_and_returns_the_same_rows() {
    let t = boot();
    let mut ids = vec![];
    for i in 0..7 {
        ids.push(
            t.job(json!({ "family": "cylinder_shell", "r_dev": 100 + i, "length": 250 }))
                .await,
        );
    }
    let a = t.job(good_assembly()).await;
    let (_, v) = t.call("POST", &format!("/api/calculations/{a}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter per drawing" }))).await;
    let n = v["new_calculation"].as_i64().unwrap();
    // the rows, computed independently of the listing code: every job, newest first, with the fields the old listing had
    let full = t.get("/api/calculations").await;
    let rows = full.as_array().unwrap();
    let expected: Vec<i64> = {
        let mut x = ids.clone();
        x.extend([a, n]);
        x.reverse();
        x
    };
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    for r in rows {
        let id = r["id"].as_i64().unwrap();
        let (family, closure, by, sup, open): (String, String, String, i64, i64) = t.app.db().query_row(
            "SELECT family, closure_hash, created_by, (SELECT COUNT(*) FROM calculations n WHERE n.supersedes = c.id), (SELECT COUNT(*) FROM obligations o WHERE o.target_calc = c.id AND o.state = 'open') FROM calculations c WHERE id = ?",
            params![id],
            |q| Ok((q.get(0)?, q.get(1)?, q.get(2)?, q.get(3)?, q.get(4)?)),
        ).unwrap();
        assert_eq!(
            (
                r["family"].as_str().unwrap(),
                r["closure_hash"].as_str().unwrap(),
                r["created_by"].as_str().unwrap(),
                r["superseded"].as_bool().unwrap(),
                r["open_obligations"].as_i64().unwrap()
            ),
            (
                family.as_str(),
                closure.as_str(),
                by.as_str(),
                sup > 0,
                open
            ),
            "job #{id}"
        );
        for k in ["project_id", "state", "created_at", "replayed"] {
            assert!(!r[k].is_null(), "{k}");
        }
    }
    // pages of 4 give exactly the same rows in the same order
    let mut paged: Vec<Value> = vec![];
    let mut before = i64::MAX;
    loop {
        let page = t
            .get(&format!("/api/calculations?limit=4&before={before}"))
            .await;
        let page = page.as_array().unwrap().clone();
        if page.is_empty() {
            break;
        }
        assert!(page.len() <= 4);
        before = page.last().unwrap()["id"].as_i64().unwrap();
        paged.extend(page);
    }
    assert_eq!(&paged, rows);
    // no per-row table scan: the correlated lookups go through indexes
    let db = t.app.db();
    for sql in [format!("{} WHERE c.project_id = 1 ORDER BY c.id DESC LIMIT 200", core_sql(false)), "SELECT COUNT(*) FROM obligations WHERE target_calc = 1 AND state = 'open'".to_string(), "SELECT COUNT(*) FROM advisories WHERE calc_id = 1 AND state = 'open'".to_string(), "SELECT p.id, (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id), (SELECT COUNT(*) FROM calculations c WHERE c.project_id = p.id) FROM projects p".to_string()] {
        let plan: Vec<String> = q_all(&db, &format!("EXPLAIN QUERY PLAN {sql}"), [], |r| r.get::<_, String>(3)).unwrap();
        for step in &plan {
            assert!(!(step.starts_with("SCAN n") || step.starts_with("SCAN o") || step.starts_with("SCAN obligations") || step.starts_with("SCAN advisories") || step.starts_with("SCAN d") || step.starts_with("SCAN c")) || step.contains("USING") , "table scan in: {sql}\n{plan:?}");
        }
        assert!(plan.iter().any(|s| s.contains("INDEX") || s.contains("PRIMARY KEY")), "{plan:?}");
    }
}

// ---------------------------------------------------------------- second pass (validation defects, follow-ups)

/// The gap left by the fast list (S-7): an alteration of a stored result that keeps its byte length.
#[tokio::test]
async fn list_sees_a_same_length_alteration_made_by_another_process() {
    let t = boot();
    let id = t.job(cone()).await;
    let row = |v: Value| {
        v.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .cloned()
            .unwrap()
    };
    assert_eq!(
        row(t.get("/api/calculations").await)["state"],
        "REVIEW_REQUIRED"
    );
    assert_eq!(
        row(t.get("/api/calculations").await)["state"],
        "REVIEW_REQUIRED",
        "second list: served from the stored summary"
    );
    let before: i64 = t.count(&format!(
        "SELECT length(CAST(result_json AS BLOB)) FROM calculations WHERE id = {id}"
    ));
    {
        // another process (a database shell, a restore tool) flips a gate result: same number of bytes
        let other = Connection::open(t.dir.join("nexus-fabric.sqlite")).unwrap();
        let n = other.execute("UPDATE calculations SET result_json = replace(result_json, '\"result\":\"PASS\"', '\"result\":\"FAIL\"') WHERE id = ?", params![id]).unwrap();
        assert_eq!(n, 1);
    }
    assert_eq!(
        t.count(&format!(
            "SELECT length(CAST(result_json AS BLOB)) FROM calculations WHERE id = {id}"
        )),
        before,
        "the alteration keeps the length"
    );
    let r = row(t.get("/api/calculations").await);
    assert_eq!(
        (r["state"].as_str(), r["integrity_failure"].as_bool()),
        (Some("REJECTED"), Some(true)),
        "{r}"
    );
    assert_eq!(
        row(t.get("/api/dashboard").await["recent"].clone())["state"],
        "REJECTED"
    );
    assert_eq!(
        t.get(&format!("/api/calculations/{id}")).await["state"],
        "REJECTED"
    );
    assert_consistent_and_never_approved(&t).await;
}

/// G21 covers every earlier engine, also 0.2.0 (branch drawn from the inside: validation defect D1).
#[tokio::test]
async fn d1_results_of_engine_0_2_0_are_flagged_for_recomputation() {
    let mut t = boot();
    let id = t.job(good_assembly()).await;
    assert!(nf_core::ENGINE_VERSION.starts_with("nf-engine 0.6.0+src."));
    // engine 0.4.4 (work plan task 1.2) changes reported bounds and verdicts: a result of 0.4.3 is marked for recomputation
    let old43 = t.job(good_assembly()).await;
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-engine 0.4.3+src.1ec0f97c0ffee123') WHERE id = {old43}"));
    let d = t.get(&format!("/api/calculations/{old43}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(gate_of(&d, "G21")["detail"].as_str().unwrap().contains(
        "Computed by a superseded engine version (nf-engine 0.4.3+src.1ec0f97c0ffee123) — recompute"
    ));
    // engine 0.4.3 (work plan task 1.1) changes what is accepted: a result of 0.4.2 is marked for recomputation too
    let old = t.job(good_assembly()).await;
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-engine 0.4.2+src.7d97912629c96ba8') WHERE id = {old}"));
    let d = t.get(&format!("/api/calculations/{old}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(gate_of(&d, "G21")["detail"].as_str().unwrap().contains(
        "Computed by a superseded engine version (nf-engine 0.4.2+src.7d97912629c96ba8) — recompute"
    ));
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-engine 0.2.0+src.0123456789abcdef') WHERE id = {id}"));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(gate_of(&d, "G21")["detail"].as_str().unwrap().contains(
        "Computed by a superseded engine version (nf-engine 0.2.0+src.0123456789abcdef) — recompute"
    ));
    assert_eq!(t.dxf_status(id, "branch").await, StatusCode::CONFLICT);
    // a current result states its marking side in the result, the gate table and the DXF
    let now = t.job(assembly("outside", 150.0, 150.0, 100.0, 100.0)).await;
    replay_matches(&mut t, now).await;
    let d = t.get(&format!("/api/calculations/{now}")).await;
    assert!(
        d["patterns"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["marking_side"] == "outside")
    );
    assert!(
        gate_of(&d, "G02")["detail"]
            .as_str()
            .unwrap()
            .contains("PROVISIONAL convention")
    );
    assert_eq!(gate_of(&d, "G02")["result"], "INCONCLUSIVE");
    let (_, _, bytes) = t
        .send(
            "GET",
            &format!("/api/calculations/{now}/dxf/branch"),
            None,
            &[],
        )
        .await;
    assert!(String::from_utf8(bytes.clone()).unwrap().contains("MARKING SIDE: PATTERN DRAWN AS SEEN FROM THE OUTSIDE OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED"), "{}", String::from_utf8_lossy(&bytes));
    // a stored result that lost the marking-side gate is incomplete: REJECTED
    t.sql(&format!("UPDATE calculations SET result_json = json_remove(result_json, '$.gates[1]') WHERE id = {now} AND json_extract(result_json, '$.gates[1].id') = 'G02'"));
    assert_eq!(
        t.get(&format!("/api/calculations/{now}")).await["state"],
        "REJECTED"
    );
}

/// BE-033: expired sessions and old sign-in failures are removed by the periodic task.
#[tokio::test]
async fn be033_housekeeping_removes_expired_sessions_and_old_failures() {
    let t = boot();
    let past = now() as i64 - 10_000;
    t.app
        .db()
        .execute(
            "INSERT INTO sessions(token_hash, user_id, expires_at) VALUES ('old', 1, ?)",
            params![past],
        )
        .unwrap();
    t.app
        .db()
        .execute(
            "INSERT INTO login_failures(username, at, client) VALUES ('x', ?, 'c'), ('y', ?, 'c')",
            params![past, now() as i64],
        )
        .unwrap();
    assert_eq!(housekeeping(&t.app).unwrap(), 2);
    assert_eq!(
        (
            t.count("SELECT COUNT(*) FROM sessions"),
            t.count("SELECT COUNT(*) FROM login_failures")
        ),
        (1, 1)
    );
    assert_eq!(
        t.call("GET", "/api/me", None).await.0,
        StatusCode::OK,
        "the live session is kept"
    );
}

// ---------------------------------------------------------------- BH-009

#[tokio::test]
async fn bh009_project_name_refusals_say_what_is_wrong_and_count_characters() {
    let t = boot();
    // a name of only spaces is missing
    let (st, v) = t
        .call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "   ", "description": "d" })),
        )
        .await;
    assert_eq!(
        (st, v["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("project name is required")),
        "{v}"
    );
    // a name that is present but too long is not 'required': the message says it is too long
    let (st, v) = t
        .call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "A".repeat(10_000), "description": "d" })),
        )
        .await;
    assert_eq!(
        (st, v["error"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("project name is too long (max 200 characters)")
        ),
        "{v}"
    );
    // the limit is counted in characters, as the name field (maxlength 200) and the message say:
    // 200 Japanese characters are 600 bytes and are accepted; 201 characters are refused
    let (st, v) = t
        .call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "あ".repeat(200), "description": "d" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, v) = t
        .call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "B".repeat(200), "description": "d" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, v) = t
        .call(
            "POST",
            "/api/projects",
            Some(json!({ "name": "C".repeat(201), "description": "d" })),
        )
        .await;
    assert_eq!(
        (st, v["error"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("project name is too long (max 200 characters)")
        ),
        "{v}"
    );
}

// ---------------------------------------------------------------- BH-015

#[tokio::test]
async fn bh015_header_with_branch_accepts_the_part_surfaces_without_the_shared_one() {
    let t = boot();
    let parts = json!({
        "family": "header_with_branch", "header_radius": 150, "header_dev_radius": 150, "header_length": 1200,
        "branch_station": 600, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": 400,
        "header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on"
    });
    let (st, v) = t.calc(parts.clone()).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["state"], "REVIEW_REQUIRED", "{v}");
    // the same request with the shared value is accepted too, and gives the same state
    let mut shared = parts.clone();
    shared["surface_reference"] = json!("outside");
    let (st, v2) = t.calc(shared).await;
    assert_eq!((st, &v2["state"]), (StatusCode::OK, &v["state"]), "{v2}");
    // one part surface missing: refused as before, and nothing is stored
    let before = t.count("SELECT COUNT(*) FROM calculations");
    let mut one = parts.clone();
    one.as_object_mut()
        .unwrap()
        .remove("branch_surface_reference");
    let (st, v3) = t.calc(one).await;
    assert_eq!(
        (st, v3["error"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("invalid request: missing field `surface_reference`")
        ),
        "{v3}"
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM calculations"), before);
    // the stored job reads back, and the record keeps the part surfaces
    let id = v["id"].as_i64().unwrap();
    let got = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(got["state"], "REVIEW_REQUIRED", "{got}");
}

// ---------------------------------------------------------------- BH-008

#[tokio::test]
async fn bh008_the_start_up_probe_answers_200_for_a_visitor_who_is_not_signed_in() {
    let t = boot();
    // a visitor with no session, and one with a forged cookie: 200 and signed_in false (no failed request)
    for cookie in ["x=1", "nf_session=00"] {
        let (st, _, bytes) = t
            .send("GET", "/api/session", None, &[("cookie", cookie)])
            .await;
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            (st, &v),
            (
                StatusCode::OK,
                &json!({ "signed_in": false, "mode": "server", "auto_signin": false })
            ),
            "{cookie}"
        );
    }
    // the signed-in administrator: 200 with the identity and role
    let v = t.get("/api/session").await;
    assert_eq!(v["signed_in"], true);
    assert_eq!(
        (v["username"].as_str(), v["role"].as_str()),
        (Some("admin"), Some("admin"))
    );
    // the other endpoints keep their answers: /api/me still refuses a visitor who is not signed in
    let (st, _, _) = t.send("GET", "/api/me", None, &[("cookie", "x=1")]).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------- BH-002

#[tokio::test]
async fn bh002_obligations_on_a_superseded_result_are_counted_apart_and_stay_open() {
    let t = boot();
    let id = t.job(good_assembly()).await;
    // a correction creates a job with open obligations; correcting that job again supersedes it
    let (st, first) = t
        .call("POST", &format!("/api/calculations/{id}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 151.0, "reason": "first correction" })))
        .await;
    assert_eq!(st, StatusCode::OK, "{first}");
    let second = first["new_calculation"].as_i64().unwrap();
    let (st, v) = t
        .call("POST", &format!("/api/calculations/{second}/corrections"), Some(json!({ "parameter": "header_radius", "new_value": 152.0, "reason": "second correction" })))
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    // every open obligation is counted once: on current work, or on a superseded result. None is closed.
    let open_total = t.count("SELECT COUNT(*) FROM obligations WHERE state = 'open'");
    let summary = t.get("/api/amoeba/summary").await;
    let open_now = summary["obligations"]["open"].as_i64().unwrap();
    let open_old = summary["obligations"]["open_on_superseded"]
        .as_i64()
        .unwrap();
    assert_eq!(open_now + open_old, open_total, "{summary}");
    assert!(
        open_old >= 1,
        "an obligation on the superseded result stays open: {summary}"
    );
    // the job that the second correction superseded still has its open obligations
    assert!(
        t.count(&format!(
            "SELECT COUNT(*) FROM obligations WHERE state = 'open' AND target_calc = {second}"
        )) >= 1
    );
    // the dashboard gives the same split
    let dash = t.get("/api/dashboard").await;
    assert_eq!(dash["open_obligations"].as_i64(), Some(open_now));
    assert_eq!(
        dash["open_obligations_on_superseded"].as_i64(),
        Some(open_old)
    );
    // each obligation row says whether its result is superseded; the flagged rows are exactly the open ones on it
    let rows = t.get("/api/obligations").await;
    let flagged = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["state"] == "open" && r["target_superseded"] == true)
        .count() as i64;
    assert_eq!(flagged, open_old);
    // the records are not rewritten: the obligations of the superseded result are still open rows
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .any(|r| r["target_calc"] == json!(second) && r["state"] == "open")
    );
}

/// A POST whose body is sent as it is, with its content type (or none): the refusals of the body itself.
async fn send_raw(t: &T, path: &str, raw: &str, content_type: Option<&str>) -> (StatusCode, Value) {
    let mut b = HttpRequest::builder()
        .method("POST")
        .uri(path)
        .header("x-nf-request", "1")
        .header("host", "127.0.0.1:8787")
        .header("cookie", &t.cookie);
    if let Some(ct) = content_type {
        b = b.header("content-type", ct);
    }
    let req = b.body(Body::from(raw.to_string())).unwrap();
    let resp = router(t.app.clone()).oneshot(req).await.unwrap();
    let (parts, body) = resp.into_parts();
    let bytes = axum::body::to_bytes(body, 64 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (
        parts.status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// BH-014: each refusal of a calculation or a correction answers with its stable code, the fields it concerns and the
/// message the API has always given; `detail` (the exact limit and measured value) only where the engine computes one.
#[tokio::test]
async fn bh014_every_refusal_answers_with_a_code_fields_and_the_message() {
    let t = boot();
    // the request structure (400): the code, its field, and the text kept as it was
    let (st, v) = t.calc(json!({ "family": "teapot" })).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("family_unknown"))
    );
    assert_eq!(v["fields"], json!(["family"]));
    assert_eq!(
        v["error"], v["message"],
        "the text the API has always given is kept"
    );
    assert!(
        v.get("detail").is_none(),
        "a structural refusal has no detail"
    );
    let (st, v) = t
        .calc(json!({ "family": "cylinder_shell", "r_dev": 100.0, "length": 250.0, "thickness": 12.0 }))
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("field_unknown"))
    );
    assert_eq!(v["fields"], json!(["thickness"]));
    // the body itself: not JSON (400), and not sent as JSON (415), both coded
    let (st, v) = send_raw(
        &t,
        "/api/projects/1/calculations",
        "{not json",
        Some("application/json"),
    )
    .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("request_malformed"))
    );
    let (st, v) = send_raw(&t, "/api/projects/1/calculations", "{}", Some("text/plain")).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Some("request_malformed")
        )
    );
    // an engine refusal (422): the job is recorded as REFUSED, with the rule's code, field and exact limit
    let (st, v) = t
        .calc(json!({ "family": "cylinder_shell", "r_dev": 100.0, "length": 1e9 }))
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        (v["state"].as_str(), v["code"].as_str()),
        (Some("REFUSED"), Some("magnitude_above_limit"))
    );
    assert_eq!(
        (v["fields"].clone(), v["field"].clone()),
        (json!(["length"]), json!("length"))
    );
    assert_eq!(
        (
            v["detail"]["limit"].as_f64(),
            v["detail"]["measured"].as_f64()
        ),
        (Some(1e8), Some(1e9))
    );
    let refused_id = v["id"].as_i64().unwrap();
    assert_eq!(
        t.get(&format!("/api/calculations/{refused_id}")).await["state"],
        "REFUSED"
    );
    // a zero dimension is refused with no detail
    let (st, v) = t
        .calc(json!({ "family": "cylinder_shell", "r_dev": 0.0, "length": 250.0 }))
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("value_not_positive"))
    );
    assert!(v.get("detail").is_none());
    // corrections: the body rules (400, 422), the state rules (409) and the propagation rules (400, 422)
    let id = t.job(good_assembly()).await;
    let url = format!("/api/calculations/{id}/corrections");
    let reason = "a reason of five";
    let (st, v) = t
        .call(
            "POST",
            &url,
            Some(json!({ "parameter": "header_radius", "new_value": 150.5, "reason": "abc" })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("reason_required"))
    );
    assert_eq!(v["fields"], json!(["reason"]));
    let long = "x".repeat(2001);
    let (st, v) = t
        .call(
            "POST",
            &url,
            Some(json!({ "parameter": "header_radius", "new_value": 150.5, "reason": long })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("reason_too_long"))
    );
    assert_eq!(v["fields"], json!(["reason"]));
    let (st, v) = t
        .call(
            "POST",
            &url,
            Some(json!({ "parameter": "header_radius", "new_value": "abc", "reason": reason })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("value_not_finite"))
    );
    assert_eq!(v["fields"], json!(["new_value"]));
    let (st, v) = t
        .call(
            "POST",
            &url,
            Some(json!({ "parameter": "header_radius", "new_value": 150.0, "reason": reason })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("correction_value_unchanged"))
    );
    let (st, v) = t
        .call(
            "POST",
            &url,
            Some(json!({ "parameter": "nonsense", "new_value": 1.0, "reason": reason })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("correction_unknown_parameter")
        )
    );
    assert_eq!(v["fields"], json!(["parameter"]));
    let (st, v) = t
        .call(
            "POST",
            &url,
            Some(json!({ "parameter": "header_radius", "new_value": 1e9, "reason": reason })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Some("magnitude_above_limit")
        )
    );
    assert_eq!(
        (v["fields"].clone(), v["detail"]["limit"].as_f64()),
        (json!(["header_radius"]), Some(1e8))
    );
    let cone_id = t.job(cone()).await;
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/calculations/{cone_id}/corrections"),
            Some(json!({ "parameter": "r1", "new_value": 220.0, "reason": reason })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("correction_not_for_family"))
    );
    assert_eq!(v["fields"], json!(["family"]));
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/calculations/{refused_id}/corrections"),
            Some(json!({ "parameter": "length", "new_value": 250.0, "reason": reason })),
        )
        .await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("correction_on_refused_job"))
    );
    assert_eq!(v["fields"], json!([]));
    // the stored result of an accepted job: no refusal fields are added to its answer
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert!(
        d.get("code").is_none() && d.get("detail").is_none(),
        "an accepted job carries no refusal fields"
    );
}

/// Work plan 3.2: every length rule counts characters (Unicode scalar values, after `str::trim`), never bytes and never
/// UTF-16 units, exactly as the client counts them (web/app.js `charCount`, `trimRs`). A byte count let a reason of two
/// Japanese characters (six bytes) pass a minimum of five; a UTF-16 count would let three characters outside the BMP
/// (six units) pass. Corrections, obligation decisions, advisory acknowledgements, project names and passwords.
#[tokio::test]
async fn length_rules_count_characters_not_bytes_or_utf16_units() {
    let t = boot();
    // the evaluation covers one seat; this test creates a second account, so install a two-seat licence
    licence::install_seats(&t, 2).await;
    let a = t.job(good_assembly()).await;
    let url = format!("/api/calculations/{a}/corrections");
    let corr =
        |reason: &str| json!({ "parameter": "header_radius", "new_value": 158, "reason": reason });
    // too short: two Japanese characters (6 bytes), four outside the BMP (16 bytes, 8 UTF-16 units), three between
    // ideographic spaces (U+3000 is White_Space: trimmed), two between ASCII spaces
    for short in [
        "日本",
        "𠮷𠮷𠮷𠮷",
        "\u{3000}\u{3000}日本語\u{3000}",
        "  理由  ",
    ] {
        let (st, v) = t.call("POST", &url, Some(corr(short))).await;
        assert_eq!(
            (st, v["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("reason_required")),
            "{short:?}: {v}"
        );
        assert_eq!(v["fields"], json!(["reason"]));
    }
    // too long: 2001 characters (6003 bytes)
    let (st, v) = t.call("POST", &url, Some(corr(&"理".repeat(2001)))).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("reason_too_long"))
    );
    // exactly five Japanese characters: the correction is made
    let (st, v) = t.call("POST", &url, Some(corr("図面を確認"))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let n = v["new_calculation"].as_i64().unwrap();
    // obligation decisions: the same rule (text_field, then at least five characters)
    let open: Vec<i64> = t.get(&format!("/api/calculations/{n}")).await["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["state"] == "open")
        .map(|o| o["id"].as_i64().unwrap())
        .collect();
    assert!(open.len() >= 3, "{open:?}");
    let resolve = |id: i64, reason: String| {
        let t = &t;
        async move {
            t.call(
                "POST",
                &format!("/api/obligations/{id}/resolve"),
                Some(json!({ "decision": "confirmed", "reason": reason })),
            )
            .await
            .0
        }
    };
    assert_eq!(
        resolve(open[0], "日本".into()).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        resolve(open[0], "\u{3000}日本\u{3000}\u{3000}".into()).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        resolve(open[0], "理".repeat(2001)).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(resolve(open[0], "𠮷".repeat(5)).await, StatusCode::OK);
    assert_eq!(
        resolve(open[1], format!("\u{3000}{}\u{3000}", "理".repeat(2000))).await,
        StatusCode::OK
    );
    // advisory acknowledgement: a repeated mistake raises one; the same rule applies
    let again = t.job(good_assembly()).await;
    let adv = t.get(&format!("/api/calculations/{again}")).await["advisories"][0]["id"]
        .as_i64()
        .unwrap();
    let ack = |reason: &str| {
        let t = &t;
        let reason = reason.to_string();
        async move {
            t.call(
                "POST",
                &format!("/api/advisories/{adv}/acknowledge"),
                Some(json!({ "reason": reason })),
            )
            .await
            .0
        }
    };
    assert_eq!(ack("日本").await, StatusCode::BAD_REQUEST);
    assert_eq!(ack("外径で確認済み").await, StatusCode::OK);
    // project names: at most 200 characters, whatever their byte or UTF-16 length
    let project = |name: String| {
        let t = &t;
        async move {
            t.call(
                "POST",
                "/api/projects",
                Some(json!({ "name": name, "description": "" })),
            )
            .await
            .0
        }
    };
    assert_eq!(project("日".repeat(200)).await, StatusCode::OK);
    assert_eq!(project("𠮷".repeat(200)).await, StatusCode::OK);
    assert_eq!(project("日".repeat(201)).await, StatusCode::BAD_REQUEST);
    // passwords: at least 10 characters (the upper limit, 1024 bytes, is stated in bytes and stays so)
    let user = |name: &str, password: String| {
        let t = &t;
        let name = name.to_string();
        async move {
            t.call(
                "POST",
                "/api/users",
                Some(json!({ "username": name, "password": password, "role": "viewer" })),
            )
            .await
            .0
        }
    };
    assert_eq!(
        user("astral9", "𠮷".repeat(9)).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(user("astral10", "𠮷".repeat(10)).await, StatusCode::OK);
}

/// BH-004 (engine 0.4.1): a result stored by 0.4.0 is marked for recomputation (gate G21), is never shown as current and
/// has no DXF; a result of the running engine is current and its DXF is offered.
#[tokio::test]
async fn bh004_engine_0_4_0_results_are_recomputed_and_0_4_1_results_are_current() {
    let mut t = boot();
    let id = t.job(good_assembly()).await;
    t.sql(&format!(
        "UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-engine 0.4.0+src.be53b63158593bf7') WHERE id = {id}"
    ));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(
        gate_of(&d, "G21")["detail"].as_str().unwrap().contains(
            "Computed by a superseded engine version (nf-engine 0.4.0+src.be53b63158593bf7) — recompute"
        ),
        "{}",
        gate_of(&d, "G21")
    );
    assert_eq!(t.dxf_status(id, "branch").await, StatusCode::CONFLICT);
    let now = t.job(good_assembly()).await;
    let d = t.get(&format!("/api/calculations/{now}")).await;
    assert_eq!(d["engine"], nf_core::ENGINE_VERSION);
    assert_ne!(d["state"], "REJECTED", "a 0.4.2 result is current");
    replay_matches(&mut t, now).await;
    assert_eq!(t.dxf_status(now, "branch").await, StatusCode::OK);
}

/// B2-005/006 (engine 0.4.2): a result stored by 0.4.1 is marked for recomputation (gate G21), is never shown as
/// current and has no DXF: quantities and refusal outcomes changed.
#[tokio::test]
async fn b2_engine_0_4_1_results_are_recomputed() {
    let t = boot();
    let id = t.job(good_assembly()).await;
    t.sql(&format!(
        "UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-engine 0.4.1+src.a94274f3449dc907') WHERE id = {id}"
    ));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(
        gate_of(&d, "G21")["detail"].as_str().unwrap().contains(
            "Computed by a superseded engine version (nf-engine 0.4.1+src.a94274f3449dc907) — recompute"
        ),
        "{}",
        gate_of(&d, "G21")
    );
    assert_eq!(t.dxf_status(id, "branch").await, StatusCode::CONFLICT);
}

// ---------------------------------------------------------------- RC-01 / RC-22

/// The SHA-256 of the DXF of `part` named by the job's own `dxf_output` record.
fn dxf_record_sha(t: &T, id: i64, part: &str) -> String {
    let stored = stored_result(&t.app.db(), id).unwrap().unwrap();
    stored["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "dxf_output" && r["payload"]["part"] == part)
        .and_then(|r| r["payload"]["sha256"].as_str())
        .unwrap()
        .to_string()
}

/// RC-01 (verification row of Review C): the pattern list and the DXF references inside a stored result are
/// checked against the content-addressed `pattern` and `dxf_output` records of its release closure. An
/// altered point, a reference swapped to another job's DXF and a reference swapped to an uploaded PDF are
/// each an integrity failure: G22 FAIL, REJECTED, no geometry and no file; a replay runs, reports the
/// disagreement and does not match. Gates, state and closure
/// hash of the stored result are left as they were, as in the reproduction.
#[tokio::test]
async fn rc01_stored_patterns_and_dxf_references_are_checked_against_the_records() {
    let mut t = boot();
    let other = t
        .job(json!({ "family": "cylinder_shell", "r_dev": 100, "length": 250 }))
        .await;
    let other_part = t.get(&format!("/api/calculations/{other}")).await["patterns"][0]["part"]
        .as_str()
        .unwrap()
        .to_string();
    let other_sha = dxf_record_sha(&t, other, &other_part);
    // an uploaded document is stored as a blob in the same store as the DXF files
    let pdf = HttpRequest::builder()
        .method("POST")
        .uri("/api/projects/1/documents")
        .header("host", "localhost")
        .header("x-nf-request", "1")
        .header("x-filename", "drawing.pdf")
        .header("cookie", &t.cookie)
        .body(Body::from("%PDF-1.7\nan uploaded drawing"))
        .unwrap();
    assert_eq!(
        router(t.app.clone()).oneshot(pdf).await.unwrap().status(),
        StatusCode::OK
    );
    let pdf_sha: String = t
        .app
        .db()
        .query_row("SELECT sha256 FROM documents ORDER BY id DESC", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(storage::load_blob(&t.dir, &t.app.key, &pdf_sha).is_ok());

    // untouched jobs pass the check and serve exactly the file of their own DXF record
    let clean = t.job(cone()).await;
    replay_matches(&mut t, clean).await;
    replay_matches(&mut t, other).await;
    for (id, part) in [(clean, "reducer"), (other, other_part.as_str())] {
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(d["integrity"], json!([]), "job #{id}: {d}");
        assert_eq!(d["state"], "REVIEW_REQUIRED", "job #{id}");
        let (st, h, bytes) = t
            .send(
                "GET",
                &format!("/api/calculations/{id}/dxf/{part}"),
                None,
                &[],
            )
            .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(h[header::CONTENT_TYPE], "application/dxf");
        assert_eq!(nf_core::sha256_hex(&bytes), dxf_record_sha(&t, id, part));
    }

    let cases = [
        (
            "an altered point",
            "json_set(result_json, '$.patterns[0].curves[0].points[0][0]', 12345.678)".to_string(),
            "differs from its pattern record",
        ),
        (
            "a reference swapped to another job's DXF",
            format!("json_set(result_json, '$.patterns[0].dxf_sha256', '{other_sha}')"),
            "DXF reference of part reducer differs from the DXF record",
        ),
        (
            "a reference swapped to an uploaded PDF",
            format!("json_set(result_json, '$.patterns[0].dxf_sha256', '{pdf_sha}')"),
            "DXF reference of part reducer differs from the DXF record",
        ),
    ];
    let mut tampered = Vec::new();
    for (what, expr, finding) in cases {
        let id = t.job(cone()).await;
        let before: (String, String) = t
            .app
            .db()
            .query_row(
                "SELECT closure_hash, json_extract(result_json, '$.gates') FROM calculations WHERE id = ?",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        t.sql(&format!(
            "UPDATE calculations SET result_json = {expr} WHERE id = {id}"
        ));
        let after: (String, String) = t
            .app
            .db()
            .query_row(
                "SELECT closure_hash, json_extract(result_json, '$.gates') FROM calculations WHERE id = ?",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(before, after, "{what}: gates and closure hash are kept");
        tampered.push((id, what, finding));
    }
    // the replay child reports the true closure of the request (the same for every cone job)
    let stored = stored_result(&t.app.db(), clean).unwrap().unwrap();
    let ids: Vec<&str> = stored["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    t.with_replay_exe(&format!("cat >/dev/null; echo '{}'", json!({ "engine": nf_core::ENGINE_VERSION, "closure_hash": stored["closure_hash"], "ids": ids })));
    // the list reads each tampered job in full (nothing of it was listed before)
    assert_consistent_and_never_approved(&t).await;
    for (id, what, finding) in &tampered {
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(d["state"], "REJECTED", "{what}");
        assert_eq!(gate_of(&d, "G22")["result"], "FAIL", "{what}");
        assert_eq!(d["patterns"], json!([]), "{what}: no geometry is shown");
        let findings = d["integrity"].to_string();
        assert!(findings.contains(finding), "{what}: {findings}");
        let (st, h, bytes) = t
            .send(
                "GET",
                &format!("/api/calculations/{id}/dxf/reducer"),
                None,
                &[],
            )
            .await;
        assert_eq!(st, StatusCode::CONFLICT, "{what}: the file is withheld");
        assert_ne!(
            h.get(header::CONTENT_TYPE).map(|v| v.as_bytes()),
            Some(&b"application/dxf"[..]),
            "{what}"
        );
        assert!(!String::from_utf8_lossy(&bytes).contains("%PDF"), "{what}");
        // the replay runs, reports the disagreement and does not match
        let (st, v) = t
            .call("POST", &format!("/api/calculations/{id}/replay"), None)
            .await;
        assert_eq!(st, StatusCode::OK, "{what}: {v}");
        assert_eq!(
            (
                v["matches"].as_bool(),
                v["stored_result_matches_records"].as_bool()
            ),
            (Some(false), Some(false)),
            "{what}: {v}"
        );
        assert!(
            v["stored_result_findings"].to_string().contains(finding),
            "{what}: {v}"
        );
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(
            (d["state"].as_str(), gate_of(&d, "G19")["result"].as_str()),
            (Some("REJECTED"), Some("FAIL")),
            "{what}"
        );
        // the list agrees, also on a second list, which reads a disagreeing job in full again
        for _ in 0..2 {
            let list = t.get("/api/calculations").await;
            let row = list
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == *id)
                .unwrap()
                .clone();
            assert_eq!(
                (row["state"].as_str(), row["integrity_failure"].as_bool()),
                (Some("REJECTED"), Some(true)),
                "{what}"
            );
        }
    }
    // a replay of a tampered job records no lesson: one built from it would raise advisories on honest jobs
    assert_eq!(
        t.count("SELECT COUNT(*) FROM lessons WHERE kind = 'integrity_failure'"),
        0
    );
    // each replay audit names the job's intact gate-evaluation record (identical reports are stored once)
    let root = stored_result(&t.app.db(), clean).unwrap().unwrap()["records"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    // the replay of the other valid job (made above, for its own DXF) names that job's record
    let other_root = stored_result(&t.app.db(), other).unwrap().unwrap()["records"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        t.count(&format!("SELECT COUNT(*) FROM records WHERE kind = 'replay_audit' AND json_extract(json, '$.relations[0].to') IN ('{root}', '{other_root}')")),
        t.count("SELECT COUNT(*) FROM records WHERE kind = 'replay_audit'")
    );
    assert!(t.count("SELECT COUNT(*) FROM records WHERE kind = 'replay_audit'") >= 1);
    // the untampered jobs are unaffected
    assert_eq!(t.dxf_status(clean, "reducer").await, StatusCode::OK);
    assert_eq!(t.dxf_status(other, &other_part).await, StatusCode::OK);
}

/// RC-01: a record of the release closure that is missing or altered in the records table makes the job
/// fail its integrity check, although the stored result itself is untouched.
#[tokio::test]
async fn rc01_a_damaged_or_missing_closure_record_rejects_the_job() {
    let t = boot();
    let altered = t.job(cone()).await;
    attacker_drops_triggers(&t);
    let pattern_id: String = t
        .app
        .db()
        .query_row(
            "SELECT id FROM records WHERE kind = 'pattern' AND json_extract(json, '$.payload.part') = 'reducer'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    t.sql(&format!(
        "UPDATE records SET json = json_set(json, '$.payload.pattern.eps_num', 1.0) WHERE id = '{pattern_id}'"
    ));
    let d = t.get(&format!("/api/calculations/{altered}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(
        d["integrity"]
            .to_string()
            .contains("fail their identity check"),
        "{}",
        d["integrity"]
    );
    assert_eq!(t.dxf_status(altered, "reducer").await, StatusCode::CONFLICT);
    t.sql(&format!("DELETE FROM records WHERE id = '{pattern_id}'"));
    let d = t.get(&format!("/api/calculations/{altered}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(
        d["integrity"]
            .to_string()
            .contains("missing from the records"),
        "{}",
        d["integrity"]
    );
    assert_eq!(t.dxf_status(altered, "reducer").await, StatusCode::CONFLICT);
}

/// RC-22: a replay that runs reports whether the stored pattern list matches the records of the closure, and
/// a match requires it.
#[tokio::test]
async fn rc22_replay_reports_the_check_against_the_records() {
    let mut t = boot();
    let id = t
        .job(json!({ "family": "cylinder_shell", "r_dev": 100, "length": 250 }))
        .await;
    let stored = stored_result(&t.app.db(), id).unwrap().unwrap();
    let ids: Vec<&str> = stored["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    t.with_replay_exe(&format!("cat >/dev/null; echo '{}'", json!({ "engine": nf_core::ENGINE_VERSION, "closure_hash": stored["closure_hash"], "ids": ids })));
    let (st, v) = t
        .call("POST", &format!("/api/calculations/{id}/replay"), None)
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["matches"], true, "{v}");
    assert_eq!(v["stored_result_matches_records"], true, "{v}");
    assert_eq!(v["stored_result_findings"], json!([]), "{v}");
}

// ---------------------------------------------------------------- RC-08 / V-47

#[test]
fn rc08_origin_is_parsed_strictly() {
    let h = "127.0.0.1:8787";
    // accepted: exactly the listener's scheme, host and port
    assert!(origin_allowed("http://127.0.0.1:8787", h, false));
    assert!(origin_allowed("HTTP://127.0.0.1:8787", h, false));
    assert!(origin_allowed(
        "http://LocalHost:8787",
        "localhost:8787",
        false
    ));
    assert!(origin_allowed(
        "http://localhost:8787",
        "LOCALHOST:8787",
        false
    ));
    assert!(origin_allowed("https://fab.example", "fab.example", true));
    assert!(origin_allowed(
        "https://fab.example",
        "fab.example:443",
        true
    ));
    assert!(origin_allowed(
        "https://fab.example:443",
        "fab.example",
        true
    ));
    assert!(origin_allowed("http://[::1]:8787", "[::1]:8787", false));
    // refused: non-web schemes, the wrong scheme for the listener, anything after the authority
    for (origin, host, https_listener) in [
        ("file://127.0.0.1:8787", h, false),
        ("javascript://127.0.0.1:8787", h, false),
        ("data://127.0.0.1:8787", h, false),
        ("data:text/html,x", h, false),
        ("null", h, false),
        ("", h, false),
        ("https://127.0.0.1:8787", h, false),
        ("http://fab.example", "fab.example", true),
        ("ws://fab.example", "fab.example", true),
        ("http://127.0.0.1:8787://evil", h, false),
        ("http://127.0.0.1:8787://evil.example", h, false),
        ("http://127.0.0.1:8787/", h, false),
        ("http://127.0.0.1:8787/path", h, false),
        ("http://127.0.0.1:8787?q", h, false),
        ("http://127.0.0.1:8787#f", h, false),
        ("http://user@127.0.0.1:8787", h, false),
        ("http://user:pw@127.0.0.1:8787", h, false),
        ("http://evil.example@127.0.0.1:8787", h, false),
        ("http://127.0.0.1:8787@evil.example", h, false),
        ("http://127.0.0.1\\@evil:8787", h, false),
        ("http:// 127.0.0.1:8787", h, false),
        ("http://127.0.0.1:8787 ", h, false),
        ("http://127.0.0.1:8788", h, false),
        ("http://127.0.0.1", h, false),
        ("http://127.0.0.1:", h, false),
        ("http://127.0.0.1:08787x", h, false),
        ("http://127.0.0.1:0", "127.0.0.1:0", false),
        ("http://127.0.0.1:99999", "127.0.0.1:99999", false),
        ("http://127.0.0.2:8787", h, false),
        ("http://evil.example", h, false),
        ("http://[::1]:8787", h, false),
        ("http://[::1]x:8787", "[::1]:8787", false),
        ("http://:8787", ":8787", false),
        ("http://127.0.0.1:8787", "", false),
        ("http//127.0.0.1:8787", h, false),
    ] {
        assert!(
            !origin_allowed(origin, host, https_listener),
            "{origin:?} for Host {host:?} (https listener: {https_listener})"
        );
    }
}

#[tokio::test]
async fn rc08_v47_origin_scheme_follows_the_listener() {
    let t = boot();
    let post = |origin: &'static str| {
        let t = &t;
        async move {
            t.send(
                "POST",
                "/api/projects",
                Some(json!({ "name": "o" })),
                &[("origin", origin)],
            )
            .await
            .0
        }
    };
    assert_eq!(post("file://127.0.0.1:8787").await, StatusCode::FORBIDDEN);
    assert_eq!(post("https://127.0.0.1:8787").await, StatusCode::FORBIDDEN);
    assert_eq!(
        post("http://127.0.0.1:8787://evil").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(post("http://127.0.0.1:8787/x").await, StatusCode::FORBIDDEN);
    assert_eq!(post("http://127.0.0.1:8787").await, StatusCode::OK);
    // behind HTTPS (NF_COOKIE_SECURE) only an https origin is accepted
    let dir = temp_dir("origin-https");
    let app = build_app(Config {
        cookie_secure: true,
        ..config(&dir)
    })
    .unwrap();
    *app.setup_token.lock().unwrap() = None;
    let s = T {
        cookie: add_session(&app, "admin", "admin").replace("nf_session", "__Host-nf_session"),
        app,
        dir,
    };
    s.app.db().execute("INSERT INTO projects(name, description, created_by, created_at) VALUES ('p','', 'admin', 0)", []).unwrap();
    for (origin, want) in [
        ("http://127.0.0.1:8787", StatusCode::FORBIDDEN),
        ("https://127.0.0.1:8787", StatusCode::OK),
    ] {
        let st = s
            .send(
                "POST",
                "/api/projects",
                Some(json!({ "name": "o" })),
                &[("origin", origin)],
            )
            .await
            .0;
        assert_eq!(st, want, "{origin}");
    }
}

// ---------------------------------------------------------------- task 2.4: the client learns the mode

/// The health answer says whether the server runs in desktop mode, so that the first-run screen can show what desktop
/// mode does and does not protect. Server mode says "server"; nothing else about the installation is disclosed.
#[tokio::test]
async fn task24_health_tells_the_client_whether_it_runs_in_desktop_mode() {
    let t = boot();
    assert_eq!(t.get("/api/health").await["mode"], "server");
    let dir = temp_dir("mode-desktop");
    let app = build_app(Config {
        desktop: true,
        allowed_hosts: vec!["127.0.0.1".into(), "localhost".into()],
        ..config(&dir)
    })
    .unwrap();
    let d = T {
        cookie: String::new(),
        app,
        dir,
    };
    let h = d.get("/api/health").await;
    assert_eq!(h["mode"], "desktop", "{h}");
    assert_eq!(h["setup_required"], true, "{h}");
    assert!(
        h.get("engine").is_none() && h.get("uptime_seconds").is_none(),
        "{h}"
    );
}

/// RC-01: a correction is computed from the job's request; a stored request that differs from the request
/// record makes the job REJECTED, and the correction is refused.
#[tokio::test]
async fn rc01_a_correction_on_a_tampered_request_is_refused() {
    let t = boot();
    let id = t.job(good_assembly()).await;
    t.sql(&format!(
        "UPDATE calculations SET result_json = json_set(result_json, '$.request.header_radius', 151.0) WHERE id = {id}"
    ));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(
        d["integrity"]
            .to_string()
            .contains("stored request differs from the request record"),
        "{}",
        d["integrity"]
    );
    assert_eq!(
        d["request"]["header_radius"], 150.0,
        "the request shown is the recorded one"
    );
    assert_eq!(d["correctable_parameters"], json!([]));
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/calculations/{id}/corrections"),
            Some(json!({ "parameter": "header_radius", "new_value": 158, "reason": "outside diameter per drawing" })),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM calculations"),
        1,
        "nothing was computed"
    );
}

/// RC-01: the dependency graph is built only from the verified records of the release closure; a job that
/// fails its integrity check has none to offer.
#[tokio::test]
async fn rc01_dependencies_of_a_tampered_job_are_refused() {
    let t = boot();
    let clean = t.job(good_assembly()).await;
    let tampered = t.job(good_assembly()).await;
    let (st, v) = t
        .call(
            "GET",
            &format!("/api/calculations/{clean}/dependencies"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["edges"].as_array().unwrap().len() >= 20, "{v}");
    t.sql(&format!(
        "UPDATE calculations SET result_json = json_set(result_json, '$.records[0].payload', json('{{}}')) WHERE id = {tampered}"
    ));
    let (st, v) = t
        .call(
            "GET",
            &format!("/api/calculations/{tampered}/dependencies"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    let d = t.get(&format!("/api/calculations/{tampered}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert_eq!(
        d["records"],
        json!([]),
        "no records of a tampered job are served"
    );
    assert!(
        d["integrity"]
            .to_string()
            .contains("stored record list differs")
    );
}

/// RC-01: a part listed twice, and a check whose verdict was changed, are integrity failures.
#[tokio::test]
async fn rc01_duplicate_parts_and_altered_checks_are_detected() {
    let t = boot();
    let dup = t.job(cone()).await;
    t.sql(&format!(
        "UPDATE calculations SET result_json = json_insert(result_json, '$.patterns[#]', json_extract(result_json, '$.patterns[0]')) WHERE id = {dup}"
    ));
    let d = t.get(&format!("/api/calculations/{dup}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(
        d["integrity"]
            .to_string()
            .contains("lists part reducer more than once")
    );
    assert_eq!(t.dxf_status(dup, "reducer").await, StatusCode::CONFLICT);
    let chk = t.job(cone()).await;
    let verdict: String = t
        .app
        .db()
        .query_row(
            &format!("SELECT json_extract(result_json, '$.checks[0].verdict') FROM calculations WHERE id = {chk}"),
            [],
            |r| r.get(0),
        )
        .unwrap();
    let other = if verdict == "PASS" { "FAIL" } else { "PASS" };
    t.sql(&format!(
        "UPDATE calculations SET result_json = json_set(result_json, '$.checks[0].verdict', '{other}') WHERE id = {chk}"
    ));
    let d = t.get(&format!("/api/calculations/{chk}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert_eq!(d["checks"], json!([]));
    assert!(
        d["integrity"]
            .to_string()
            .contains("stored checks differ from the evidence records")
    );
}

/// RC-01, the comparison itself: records are altered in memory (identities are checked by the walk, not here).
#[test]
fn rc01_compare_with_records_finds_every_difference() {
    let req: Request = serde_json::from_value(cone()).unwrap();
    let out = job::run(&req, "u");
    let res = serde_json::to_value(&out).unwrap();
    let recs = out.records.clone();
    let (f, dxf, ordered) = compare_with_records(&res, &recs);
    assert!(f.is_empty(), "{f:?}");
    assert_eq!(dxf.len(), 1);
    assert_eq!(ordered.len(), recs.len());
    let with = |edit: &dyn Fn(&mut Vec<Record>)| {
        let mut r = recs.clone();
        edit(&mut r);
        compare_with_records(&res, &r).0.join("; ")
    };
    // a second pattern record for the same part
    let f = with(&|r| {
        let mut p = r.iter().find(|x| x.kind == "pattern").unwrap().clone();
        p.id = "another".into();
        r.push(p);
    });
    assert!(
        f.contains("more than one pattern record for part reducer"),
        "{f}"
    );
    // the pattern record and the DXF record name different files
    let f = with(&|r| {
        let d = r.iter_mut().find(|x| x.kind == "dxf_output").unwrap();
        d.payload["sha256"] = json!("ab".repeat(32));
    });
    assert!(
        f.contains(
            "pattern record of part reducer and the DXF record of the job name different files"
        ),
        "{f}"
    );
    // a check recorded as FAIL that the stored result shows as PASS
    let f = with(&|r| {
        let e = r
            .iter_mut()
            .find(|x| x.kind == "verification_evidence")
            .unwrap();
        e.payload["checks"][0]["verdict"] = json!("FAIL");
    });
    assert!(
        f.contains("stored checks differ from the evidence records"),
        "{f}"
    );
    // a second request record, and none
    let f = with(&|r| {
        let mut q = r
            .iter()
            .find(|x| x.kind == "development_request")
            .unwrap()
            .clone();
        q.id = "another".into();
        r.push(q);
    });
    assert!(f.contains("more than one request record"), "{f}");
    let f = with(&|r| r.retain(|x| x.kind != "development_request"));
    assert!(f.contains("no request record"), "{f}");
    // work units and engine
    let f = with(&|r| {
        let a = r.iter_mut().find(|x| x.family == Family::Activity).unwrap();
        a.payload["work_units"] = json!(1);
    });
    assert!(f.contains("work units"), "{f}");
    let f = with(&|r| {
        let a = r.iter_mut().find(|x| x.family == Family::Activity).unwrap();
        a.payload["engine"] = json!("nf-engine 0.0.1");
    });
    assert!(f.contains("engine identifier"), "{f}");
}
/// Task 3.1: PDF to DXF through the server.
mod pdfconv;

/// NFJ-1 S1-L1: orders (job data, files, pages, parts, values, decisions).
mod orders;

/// PAR-2: factory rule packs (import, person-only writes, interview, engine overlay).
mod rulepacks;

/// AIOP-1: operator event stream, pause, undo.
mod operator;

/// Licence system: state machine glue, the read-only gate, seats, renewal (plan cases 12-22
/// and the owner's enforcement-off cases).
mod licence;

// ---------------------------------------------------------------- architecture review A (RA-001 … RA-016)

/// The attacker of RA-003 has write access to the database file. Test-only: drops the append-only triggers
/// so that the tampering a file writer could do can still be simulated (the chain check must catch it).
fn attacker_drops_triggers(t: &T) {
    for name in [
        "audit_no_update",
        "audit_no_delete",
        "records_no_update",
        "records_no_delete",
    ] {
        t.sql(&format!("DROP TRIGGER IF EXISTS {name}"));
    }
}

/// A sign-in or password request from one peer address, through the real router.
fn peer_request(
    method: &str,
    path: &str,
    body: Value,
    cookie: Option<&str>,
    peer: &str,
) -> HttpRequest<Body> {
    let mut b = HttpRequest::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:8787")
        .header("x-nf-request", "1")
        .header("content-type", "application/json");
    b = b.header("cookie", cookie.unwrap_or("none=1"));
    let mut r = b.body(Body::from(body.to_string())).unwrap();
    r.extensions_mut().insert(PeerAddr(peer.parse().unwrap()));
    r
}

async fn answer(app: &Arc<App>, r: HttpRequest<Body>) -> StatusCode {
    router(app.clone()).oneshot(r).await.unwrap().status()
}

/// A session for a user created with `add_real_user`.
fn session_for(t: &T, name: &str) -> String {
    let db = t.app.db();
    let uid: i64 = db
        .query_row(
            "SELECT id FROM users WHERE username = ?",
            params![name],
            |r| r.get(0),
        )
        .unwrap();
    let token = format!("token-of-{name}-real");
    db.execute(
        "INSERT INTO sessions(token_hash, user_id, expires_at) VALUES (?,?,?)",
        params![
            nf_core::sha256_hex(token.as_bytes()),
            uid,
            (now() + 3600) as i64
        ],
    )
    .unwrap();
    format!("nf_session={token}")
}

#[tokio::test]
async fn ra003_a_truncated_audit_log_is_not_re_signed_by_the_next_event() {
    let t = boot();
    for i in 0..3 {
        assert_eq!(
            t.call(
                "POST",
                "/api/projects",
                Some(json!({ "name": format!("q{i}") }))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    // the attacker: drop the triggers, remove the newest entries, remove the counter
    attacker_drops_triggers(&t);
    t.sql("DELETE FROM audit WHERE id > 2");
    t.sql("DELETE FROM meta WHERE key = 'audit_count'");
    // the next audited event must not extend the shortened chain and sign it
    let (st, body) = t
        .call(
            "POST",
            "/api/login",
            Some(json!({ "username": "nobody", "password": "x" })),
        )
        .await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["audit_broken"], true);
    assert!(body["error_ja"].as_str().is_some_and(|s| !s.is_empty()));
    let v = t.get("/api/audit/verify").await;
    assert_eq!(v["intact"], false, "{v}");
    assert_eq!(v["anchor_intact"], false, "{v}");
    assert!(v["anchor_problem"].as_str().unwrap().contains("removed"));
}

#[tokio::test]
async fn ra003_audit_and_records_cannot_be_updated_or_deleted_through_the_schema() {
    let t = boot();
    t.call("POST", "/api/projects", Some(json!({ "name": "z" })))
        .await;
    {
        let db = t.app.db();
        db.execute(
            "INSERT INTO records(id, project_id, family, kind, json) VALUES ('rec-1', 1, 'f', 'k', '{}')",
            [],
        )
        .unwrap();
        let audit_id: i64 = db
            .query_row("SELECT MIN(id) FROM audit", [], |r| r.get(0))
            .unwrap();
        assert!(
            db.execute(
                "UPDATE audit SET detail = 'x' WHERE id = ?",
                params![audit_id]
            )
            .is_err()
        );
        assert!(
            db.execute("DELETE FROM audit WHERE id = ?", params![audit_id])
                .is_err()
        );
        assert!(
            db.execute(
                "UPDATE records SET json = '{\"x\":1}' WHERE id = 'rec-1'",
                []
            )
            .is_err()
        );
        assert!(
            db.execute("DELETE FROM records WHERE id = 'rec-1'", [])
                .is_err()
        );
    }
    // and the application itself still appends
    assert_eq!(
        t.call("POST", "/api/projects", Some(json!({ "name": "y" })))
            .await
            .0,
        StatusCode::OK
    );
}

#[test]
fn ra003_a_log_with_entries_but_no_anchor_stops_start_up() {
    let dir = temp_dir("anchor-gone");
    {
        let app = build_app(config(&dir)).unwrap();
        app.audit(&app.db(), "admin", "one", "two").unwrap();
    }
    {
        let db = Connection::open(dir.join("nexus-fabric.sqlite")).unwrap();
        db.execute("DELETE FROM meta WHERE key = 'audit_anchor'", [])
            .unwrap();
    }
    match build_app(config(&dir)) {
        Ok(_) => panic!("a log with entries and no anchor must not be re-anchored silently"),
        Err(e) => assert!(e.contains("anchor"), "{e}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ra002_concurrent_wrong_sign_ins_for_one_account_are_counted_before_hashing() {
    let t = boot();
    add_real_user(&t, "erin", "erins-long-password");
    let mut tasks = Vec::new();
    for i in 0..30 {
        let app = t.app.clone();
        tasks.push(tokio::spawn(async move {
            answer(
                &app,
                peer_request(
                    "POST",
                    "/api/login",
                    json!({ "username": "erin", "password": format!("wrong-{i}") }),
                    None,
                    "10.1.0.1",
                ),
            )
            .await
        }));
    }
    let mut answers = Vec::new();
    for task in tasks {
        answers.push(task.await.unwrap());
    }
    let wrong = answers
        .iter()
        .filter(|s| **s == StatusCode::UNAUTHORIZED)
        .count();
    let throttled = answers
        .iter()
        .filter(|s| **s == StatusCode::TOO_MANY_REQUESTS)
        .count();
    assert_eq!((wrong, throttled), (5, 25), "{answers:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ra002_concurrent_wrong_current_passwords_are_counted_before_hashing() {
    let t = boot();
    add_real_user(&t, "gina", "ginas-long-password");
    let cookie = session_for(&t, "gina");
    let mut tasks = Vec::new();
    for i in 0..20 {
        let app = t.app.clone();
        let cookie = cookie.clone();
        tasks.push(tokio::spawn(async move {
            answer(
                &app,
                peer_request(
                    "POST",
                    "/api/password",
                    json!({ "current": format!("wrong-{i}"), "new": "a-brand-new-password-1" }),
                    Some(&cookie),
                    "10.2.0.1",
                ),
            )
            .await
        }));
    }
    let mut answers = Vec::new();
    for task in tasks {
        answers.push(task.await.unwrap());
    }
    let wrong = answers
        .iter()
        .filter(|s| **s == StatusCode::FORBIDDEN)
        .count();
    assert_eq!(wrong, 5, "{answers:?}");
    assert_eq!(
        answers
            .iter()
            .filter(|s| **s == StatusCode::TOO_MANY_REQUESTS)
            .count(),
        15
    );
}

#[tokio::test]
async fn ra006_a_guesser_cannot_lock_the_account_for_another_client() {
    let t = boot();
    add_real_user(&t, "carol", "carols-long-password");
    for i in 0..5 {
        assert_eq!(
            answer(
                &t.app,
                peer_request(
                    "POST",
                    "/api/login",
                    json!({ "username": "carol", "password": format!("g{i}") }),
                    None,
                    "10.3.0.1"
                )
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
    }
    // the guesser's own address is throttled
    assert_eq!(
        answer(
            &t.app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "carol", "password": "carols-long-password" }),
                None,
                "10.3.0.1"
            )
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // another client signs in with the right password
    assert_eq!(
        answer(
            &t.app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "carol", "password": "carols-long-password" }),
                None,
                "10.3.0.2"
            )
        )
        .await,
        StatusCode::OK,
        "one client's guesses must not lock the account for every client"
    );
}

#[tokio::test]
async fn ra006_once_an_account_is_under_attack_only_known_clients_get_through_at_full_speed() {
    let t = boot();
    add_real_user(&t, "dan", "dans-long-password");
    // the owner signs in from a known address first
    assert_eq!(
        answer(
            &t.app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "dan", "password": "dans-long-password" }),
                None,
                "10.4.0.9"
            )
        )
        .await,
        StatusCode::OK
    );
    // 20 failures from 20 other addresses: the account-wide gate engages
    for i in 0..20 {
        assert_eq!(
            answer(
                &t.app,
                peer_request(
                    "POST",
                    "/api/login",
                    json!({ "username": "dan", "password": "guess" }),
                    None,
                    &format!("10.5.0.{}", i + 1)
                )
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
    }
    // CHANGED (RA-006 residual, see REVIEW_A §6): the slot is per (account, address). A new address gets its own
    // first attempt; its second attempt within 10 seconds is refused.
    assert_eq!(
        answer(
            &t.app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "dan", "password": "guess" }),
                None,
                "10.6.0.1"
            )
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        answer(
            &t.app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "dan", "password": "dans-long-password" }),
                None,
                "10.6.0.1"
            )
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS,
        "an unknown address is slowed down on its own slot while the account is attacked"
    );
    assert_eq!(
        answer(
            &t.app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "dan", "password": "dans-long-password" }),
                None,
                "10.4.0.9"
            )
        )
        .await,
        StatusCode::OK,
        "the owner's known address is not held back by the gate"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ra001_a_request_body_that_stalls_is_abandoned_after_the_body_idle_limit() {
    let dir = temp_dir("body-idle");
    let mut app = build_app(config(&dir)).unwrap();
    Arc::get_mut(&mut app).unwrap().body_idle = Duration::from_millis(300);
    let started = std::time::Instant::now();
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/api/login")
        .header("host", "localhost")
        .header("x-nf-request", "1")
        .header("content-type", "application/json")
        .header("content-length", "1000")
        .body(Body::from_stream(OneThenStall(false)))
        .unwrap();
    let resp = tokio::time::timeout(Duration::from_secs(5), router(app.clone()).oneshot(req))
        .await
        .expect("the stalled body held the request for more than 5 s")
        .unwrap();
    assert_eq!(resp.status(), StatusCode::REQUEST_TIMEOUT);
    assert!(started.elapsed() < Duration::from_secs(5));
    let _ = std::fs::remove_dir_all(&dir);
}

/// One chunk of a JSON body, then nothing ever again.
struct OneThenStall(bool);
impl futures_core::Stream for OneThenStall {
    type Item = Result<axum::body::Bytes, std::io::Error>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.0 {
            std::task::Poll::Pending
        } else {
            self.0 = true;
            std::task::Poll::Ready(Some(Ok(axum::body::Bytes::from_static(b"{\"username\":"))))
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ra001_one_client_cannot_hold_more_requests_open_than_its_share() {
    let dir = temp_dir("per-client");
    let mut app = build_app(config(&dir)).unwrap();
    {
        let a = Arc::get_mut(&mut app).unwrap();
        a.per_client_in_flight = 2;
        a.body_idle = Duration::from_secs(30);
    }
    let mut held = Vec::new();
    for _ in 0..2 {
        let app = app.clone();
        held.push(tokio::spawn(async move {
            let mut r = HttpRequest::builder()
                .method("POST")
                .uri("/api/login")
                .header("host", "localhost")
                .header("x-nf-request", "1")
                .header("content-type", "application/json")
                .header("content-length", "1000")
                .body(Body::from_stream(OneThenStall(false)))
                .unwrap();
            r.extensions_mut()
                .insert(PeerAddr("10.8.0.1".parse().unwrap()));
            let _ = router(app).oneshot(r).await;
        }));
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        answer(
            &app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "x", "password": "y" }),
                None,
                "10.8.0.1"
            )
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_ne!(
        answer(
            &app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "x", "password": "y" }),
                None,
                "10.8.0.2"
            )
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS,
        "another client is not affected"
    );
    for h in held {
        h.abort();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ra009_setup_failures_are_counted_per_client() {
    let dir = temp_dir("setup-client");
    let app = build_app(config(&dir)).unwrap();
    let setup = |token: &str, peer: &str| {
        let mut r = peer_request(
            "POST",
            "/api/setup",
            json!({ "username": "owner", "password": "owners-setup-password-1" }),
            None,
            peer,
        );
        r.headers_mut()
            .insert("x-nf-setup-token", token.parse().unwrap());
        r
    };
    for _ in 0..SETUP_FAILURES {
        assert_eq!(
            answer(&app, setup("wrong-token", "10.9.0.1")).await,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        answer(&app, setup("wrong-token", "10.9.0.1")).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        answer(&app, setup(TOKEN, "10.9.0.2")).await,
        StatusCode::OK,
        "one client's wrong tokens must not block the installer"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ra016_ipv6_clients_share_their_prefix_and_bracketed_host_names_parse() {
    let t = boot();
    let h = HeaderMap::new();
    let a = client_address(&t.app, Some("2001:db8:1:2::1".parse().unwrap()), &h);
    let b = client_address(&t.app, Some("2001:db8:1:2:ffff::9".parse().unwrap()), &h);
    assert_eq!(a, b, "one /64 is one client");
    assert_ne!(
        a,
        client_address(&t.app, Some("2001:db8:1:3::1".parse().unwrap()), &h)
    );
    assert_eq!(
        client_address(&t.app, Some("192.0.2.7".parse().unwrap()), &h),
        "192.0.2.7"
    );
    assert_eq!(host_name("[2001:db8::1]:8787"), "[2001:db8::1]");
    assert_eq!(host_name("LocalHost:8787"), "localhost");
    assert_eq!(host_name("[::1]"), "[::1]");
}

#[tokio::test]
async fn ra014_a_restored_pre_upgrade_database_is_accepted_when_its_key_matches() {
    let dir = temp_dir("restore-old");
    {
        let _ = build_app(config(&dir)).unwrap();
    }
    {
        let db = Connection::open(dir.join("nexus-fabric.sqlite")).unwrap();
        db.execute("DELETE FROM meta WHERE key = 'installation_id'", [])
            .unwrap();
    }
    let key = dir.join("storage.key");
    match storage::open_storage(&dir, &key) {
        Ok(_) => {}
        Err(e) => panic!("a restored database with the same key must start: {e}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ra013_a_failed_audit_write_leaves_no_entry_behind() {
    let t = boot();
    t.sql("CREATE TRIGGER block_anchor BEFORE INSERT ON meta WHEN NEW.key = 'audit_anchor' BEGIN SELECT RAISE(ABORT, 'blocked'); END");
    let before = t.count("SELECT COUNT(*) FROM audit");
    let res = {
        let db = t.app.db();
        t.app.audit(&db, "admin", "probe", "anchor write fails")
    };
    assert!(res.is_err());
    t.sql("DROP TRIGGER block_anchor");
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit"),
        before,
        "no half-written entry"
    );
    assert_eq!(t.get("/api/audit/verify").await["intact"], true);
}

#[tokio::test]
async fn ra011_a_panicking_handler_answers_500_and_the_server_keeps_serving() {
    let t = boot();
    let (st, _, _) = t.send("GET", "/api/__test_panic", None, &[]).await;
    assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(t.call("GET", "/api/health", None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn ra012_a_stored_result_with_a_duplicate_key_is_rejected() {
    let t = boot();
    let id = t.job(good_assembly()).await;
    let raw: String = t
        .app
        .db()
        .query_row(
            "SELECT result_json FROM calculations WHERE id = ?",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    let engine = serde_json::from_str::<Value>(&raw).unwrap()["engine"].clone();
    // the same key twice with the same value: SQLite and serde read it the same way, the copy is still refused
    let dup = format!("{{\"engine\":{},{}", engine, &raw[1..]);
    t.app
        .db()
        .execute(
            "UPDATE calculations SET result_json = ? WHERE id = ?",
            params![dup, id],
        )
        .unwrap();
    let detail = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(detail["state"], "REJECTED", "{detail}");
    assert_eq!(t.dxf_status(id, "shell").await, StatusCode::CONFLICT);
}

#[tokio::test]
async fn cli32_a_reason_is_counted_in_characters_not_bytes() {
    let t = boot();
    // three Japanese characters are nine bytes: refused, as a reason of fewer than five characters
    let (st, v) = t
        .call(
            "POST",
            "/api/advisories/1/acknowledge",
            Some(json!({ "reason": "日本語" })),
        )
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
}

#[test]
fn ra010_a_full_file_store_is_classified_as_out_of_space() {
    assert!(storage::is_out_of_space(
        &std::io::Error::from_raw_os_error(28)
    ));
    assert!(!storage::is_out_of_space(
        &std::io::Error::from_raw_os_error(13)
    ));
}

#[tokio::test]
async fn ra007_a_result_of_a_superseded_engine_is_recomputed_and_the_old_one_stays() {
    let t = boot();
    let old = t.job(good_assembly()).await;
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-core 0.1.0 / uem 0.1.0'), engine = 'nf-core 0.1.0 / uem 0.1.0' WHERE id = {old}"));
    assert_eq!(
        t.get(&format!("/api/calculations/{old}")).await["engine_current"],
        false
    );
    let (st, v) = t
        .call("POST", &format!("/api/calculations/{old}/recompute"), None)
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let new = v["id"].as_i64().unwrap();
    let d = t.get(&format!("/api/calculations/{new}")).await;
    assert_eq!(d["engine_current"], true, "{d}");
    assert_eq!(d["supersedes"], old);
    assert_eq!(
        t.count(&format!(
            "SELECT COUNT(*) FROM calculations WHERE id = {old}"
        )),
        1
    );
    // a result of the running engine has nothing to recompute
    assert_eq!(
        t.call("POST", &format!("/api/calculations/{new}/recompute"), None)
            .await
            .0,
        StatusCode::CONFLICT
    );
}

#[test]
fn ra007_a_non_loopback_listener_needs_the_allowed_host_list() {
    assert!(listen_needs_allowed_hosts("127.0.0.1:8787", None).is_ok());
    assert!(listen_needs_allowed_hosts("[::1]:8787", None).is_ok());
    assert!(listen_needs_allowed_hosts("localhost:8787", None).is_ok());
    assert!(listen_needs_allowed_hosts("0.0.0.0:8787", None).is_err());
    assert!(listen_needs_allowed_hosts("0.0.0.0:8787", Some("nexus.example")).is_ok());
    // an empty or blank list is not a list (RA-007)
    assert!(listen_needs_allowed_hosts("0.0.0.0:8787", Some("")).is_err());
    assert!(listen_needs_allowed_hosts("0.0.0.0:8787", Some(" , ")).is_err());
}

#[test]
fn ra009_a_setup_token_of_one_repeated_character_is_refused() {
    assert_eq!(distinct_chars("aaaaaaaaaaaaaaaaaaaa"), 1);
    assert!(distinct_chars("0123456789abcdef") >= 8);
}

#[test]
fn ra016_an_unbracketed_ipv6_allowed_host_is_refused_with_the_bracketed_form() {
    assert_eq!(
        allowed_host_entry("[2001:db8::1]").unwrap(),
        "[2001:db8::1]"
    );
    assert_eq!(
        allowed_host_entry("Nexus.Example").unwrap(),
        "nexus.example"
    );
    let e = allowed_host_entry("2001:db8::1").unwrap_err();
    assert!(e.contains("[2001:db8::1]"), "{e}");
}

#[test]
fn ra016_ipv4_mapped_clients_are_keyed_as_ipv4() {
    assert_eq!(client_key("::ffff:192.0.2.7".parse().unwrap()), "192.0.2.7");
    assert_eq!(
        client_key("2001:db8:1:2::1".parse().unwrap()),
        "2001:db8:1:2::/64"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ra001_behind_an_unconfigured_local_proxy_one_client_cannot_deny_others() {
    // The live deployment: Caddy on the same host forwards every request from 127.0.0.1, and NF_TRUSTED_PROXY is
    // not set. One client must not be able to take the open-request slots of everybody.
    let dir = temp_dir("local-proxy-flight");
    let mut app = build_app(config(&dir)).unwrap();
    {
        let a = Arc::get_mut(&mut app).unwrap();
        a.per_client_in_flight = 2;
        a.body_idle = Duration::from_secs(30);
    }
    let mut held = Vec::new();
    for i in 0..3 {
        let app = app.clone();
        held.push(tokio::spawn(async move {
            let r = HttpRequest::builder()
                .method("POST")
                .uri("/api/login")
                .header("host", "localhost")
                .header("x-nf-request", "1")
                .header("x-forwarded-for", format!("198.51.100.{}", i + 1))
                .header("content-type", "application/json")
                .header("content-length", "1000")
                .body(Body::from_stream(OneThenStall(false)))
                .unwrap();
            let mut r = r;
            r.extensions_mut()
                .insert(PeerAddr("127.0.0.1".parse().unwrap()));
            let _ = router(app).oneshot(r).await;
        }));
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut other = peer_request(
        "POST",
        "/api/login",
        json!({ "username": "x", "password": "y" }),
        None,
        "127.0.0.1",
    );
    other
        .headers_mut()
        .insert("x-forwarded-for", "203.0.113.1".parse().unwrap());
    assert_ne!(
        answer(&app, other).await,
        StatusCode::TOO_MANY_REQUESTS,
        "another client behind the same proxy must not be refused"
    );
    for h in held {
        h.abort();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ra006_behind_an_unconfigured_local_proxy_one_client_cannot_lock_another_account() {
    let t = boot();
    add_real_user(&t, "carol", "carols-long-password");
    add_real_user(&t, "dan", "dans-long-password");
    let via = |user: &str, pw: &str, xff: &str| {
        let mut r = peer_request(
            "POST",
            "/api/login",
            json!({ "username": user, "password": pw }),
            None,
            "127.0.0.1",
        );
        r.headers_mut()
            .insert("x-forwarded-for", xff.parse().unwrap());
        r
    };
    // 20 wrong guesses at carol from one local proxy: the account gate, not a lock for everyone
    for i in 0..20 {
        assert_eq!(
            answer(
                &t.app,
                via("carol", "guess", &format!("198.51.100.{}", i + 1))
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        answer(&t.app, via("dan", "dans-long-password", "203.0.113.5")).await,
        StatusCode::OK,
        "one client must not lock another account"
    );
    assert_eq!(
        answer(&t.app, via("carol", "carols-long-password", "203.0.113.6")).await,
        StatusCode::TOO_MANY_REQUESTS,
        "carol is slowed down while she is attacked"
    );
    // the slot reopens: the account is not locked
    t.sql("UPDATE login_failures SET at = at - 20 WHERE username = 'carol'");
    assert_eq!(
        answer(&t.app, via("carol", "carols-long-password", "203.0.113.6")).await,
        StatusCode::OK,
        "carol signs in again once the slot is free"
    );
}

#[tokio::test]
async fn ra006_refused_attempts_are_recorded() {
    let t = boot();
    add_real_user(&t, "carol", "carols-long-password");
    for i in 0..5 {
        assert_eq!(
            answer(
                &t.app,
                peer_request(
                    "POST",
                    "/api/login",
                    json!({ "username": "carol", "password": format!("g{i}") }),
                    None,
                    "10.3.0.1"
                )
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
    }
    let refused_before = t.count("SELECT COUNT(*) FROM login_failures WHERE admitted = 0");
    assert_eq!(
        answer(
            &t.app,
            peer_request(
                "POST",
                "/api/login",
                json!({ "username": "carol", "password": "g9" }),
                None,
                "10.3.0.1"
            )
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM login_failures WHERE admitted = 0"),
        refused_before + 1,
        "a refused guess is counted"
    );
}

#[test]
fn ra006_known_addresses_are_seeded_from_the_audit_history() {
    let t = boot();
    t.sql("DELETE FROM meta WHERE key = 'login_clients_seeded'");
    t.sql("INSERT INTO audit(at, username, action, detail, prev, hash) VALUES (1, 'dan', 'login', 'from 2001:db8:1:2::77', 'genesis', 'x')");
    let db = t.app.db();
    seed_known_clients(&db).unwrap();
    assert!(known_client(&db, "dan", &client_key("2001:db8:1:2::9".parse().unwrap())).unwrap());
    assert!(!known_client(&db, "dan", "203.0.113.9").unwrap());
}

#[tokio::test]
async fn ra003_an_anchored_log_from_an_earlier_start_passes_without_the_flag() {
    let dir = temp_dir("anchored-restart");
    {
        let app = build_app(config(&dir)).unwrap();
        let db = app.db();
        for i in 0..3 {
            app.audit(&db, "admin", "earlier", &format!("entry {i}"))
                .unwrap();
        }
    }
    // the upgraded server starts again with the same data and no NF_ACCEPT_UNANCHORED_AUDIT
    let app = build_app(config(&dir)).expect("a healthy anchored log must start without the flag");
    let v = verify_audit(&app, &app.db()).unwrap();
    assert_eq!(
        (v["intact"].as_bool(), v["anchor_intact"].as_bool()),
        (Some(true), Some(true)),
        "{v}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ra016_an_ipv4_client_on_a_mapped_loopback_listener_is_the_same_client() {
    // item 4: `::ffff:127.0.0.1` is the loopback proxy, not a remote client
    let t = boot();
    let mut h = HeaderMap::new();
    h.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.4"));
    assert!(unattributed_peer(
        &t.app,
        Some("::ffff:127.0.0.1".parse().unwrap()),
        &h
    ));
    assert!(unattributed_peer(
        &t.app,
        Some("127.0.0.1".parse().unwrap()),
        &h
    ));
    assert!(!unattributed_peer(
        &t.app,
        Some("::ffff:198.51.100.9".parse().unwrap()),
        &h
    ));
}

#[tokio::test]
async fn ra006_a_remote_peer_with_a_forged_forwarded_address_is_still_limited_by_its_real_address()
{
    // item 5: a non-loopback peer cannot choose its key by sending X-Forwarded-For. Its key is its own address.
    let t = boot();
    add_real_user(&t, "carol", "carols-long-password");
    let via = |pw: &str, peer: &str, xff: &str| {
        let mut r = peer_request(
            "POST",
            "/api/login",
            json!({ "username": "carol", "password": pw }),
            None,
            peer,
        );
        r.headers_mut()
            .insert("x-forwarded-for", xff.parse().unwrap());
        r
    };
    for i in 0..5 {
        assert_eq!(
            answer(
                &t.app,
                via("guess", "198.51.100.77", &format!("203.0.113.{}", i + 1))
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
    }
    // a new forged address does not escape the pair limit of the real peer
    assert_eq!(
        answer(
            &t.app,
            via("carols-long-password", "198.51.100.77", "192.0.2.200")
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS,
        "the forged header must not reach the unattributed branch"
    );
    // every refusal and failure of this peer is keyed by its real address
    assert_eq!(
        t.count("SELECT COUNT(*) FROM login_failures WHERE client = '198.51.100.77'"),
        6
    );
    // another remote peer is not locked by it
    assert_eq!(
        answer(
            &t.app,
            via("carols-long-password", "198.51.100.78", "203.0.113.1")
        )
        .await,
        StatusCode::OK
    );
}

// ---------------------------------------------------------------- AMOEBA digest (Phase 1)

fn part_names(d: &Value) -> Vec<String> {
    let mut v: Vec<String> = d["patterns"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| p["part"].as_str().map(str::to_string))
        .collect();
    v.sort();
    v.dedup();
    v
}

fn stage<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage"] == name)
        .unwrap_or_else(|| panic!("no stage {name}: {v}"))
}

/// Every DXF of a job that passes the digest is snapshotted, gate G23 passes, the stages report it, and the
/// files are served. The files are written by the engine's own writer: the round trip digests PASS.
#[tokio::test]
async fn amoeba_passing_files_get_a_snapshot_pass_gate_g23_and_are_served() {
    let mut t = boot();
    let bodies = [
        good_assembly(),
        cone(),
        json!({ "family": "cylinder_shell", "r_dev": 150, "length": 1200, "surface_reference": "outside" }),
    ];
    for body in bodies {
        let id = t.job(body).await;
        // G19 is NOT_RUN until the replay audit has run; a DXF is held by the export rule until then
        replay_matches(&mut t, id).await;
        let d = t.get(&format!("/api/calculations/{id}")).await;
        assert_eq!(gate_of(&d, "G23")["result"], "PASS", "{d}");
        let parts = part_names(&d);
        assert!(!parts.is_empty());
        let snaps = t.count(&format!(
            "SELECT COUNT(*) FROM amoeba_digests WHERE calc_id = {id} AND verdict = 'PASS'"
        ));
        assert_eq!(snaps as usize, parts.len(), "one PASS snapshot per DXF");
        let st = t.get(&format!("/api/calculations/{id}/amoeba")).await;
        assert_eq!(stage(&st, "digest")["status"], "PASS", "{st}");
        assert_eq!(stage(&st, "export")["status"], "AVAILABLE", "{st}");
        for p in &parts {
            assert_eq!(
                t.dxf_status(id, p).await,
                StatusCode::OK,
                "part {p} is served"
            );
        }
    }
}

/// A snapshot that failed holds the file (409 with the failing checks), makes G23 FAIL and the result REJECTED.
#[tokio::test]
async fn amoeba_a_failing_snapshot_withholds_the_file_and_fails_gate_g23() {
    let t = boot();
    let id = t.job(good_assembly()).await;
    let part = part_names(&t.get(&format!("/api/calculations/{id}")).await)[0].clone();
    t.sql("DROP TRIGGER amoeba_no_update");
    t.sql(&format!(
        "UPDATE amoeba_digests SET verdict = 'FAIL', checks_json = '[{{\"name\":\"area\",\"verdict\":\"FAIL\",\"measured\":1.0,\"expected\":2.0,\"tolerance\":0.1,\"detail\":\"planted\"}}]' WHERE calc_id = {id} AND part = '{part}'"
    ));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(gate_of(&d, "G23")["result"], "FAIL", "{d}");
    assert_eq!(d["state"], "REJECTED");
    let (st, v) = t
        .call("GET", &format!("/api/calculations/{id}/dxf/{part}"), None)
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["code"], "amoeba_digest_failed");
    assert_eq!(v["checks"][0]["name"], "area");
    let s = t.get(&format!("/api/calculations/{id}/amoeba")).await;
    assert_eq!(stage(&s, "export")["status"], "WITHHELD");
    assert_eq!(stage(&s, "digest")["status"], "FAIL");
}

/// A check that could not be judged holds the file (409) and leaves the result reviewable but G23 INCONCLUSIVE.
#[tokio::test]
async fn amoeba_an_inconclusive_snapshot_holds_the_file() {
    let t = boot();
    let id = t.job(cone()).await;
    let part = part_names(&t.get(&format!("/api/calculations/{id}")).await)[0].clone();
    t.sql("DROP TRIGGER amoeba_no_update");
    t.sql(&format!("UPDATE amoeba_digests SET verdict = 'INCONCLUSIVE', checks_json = '[{{\"name\":\"area\",\"verdict\":\"INCONCLUSIVE\",\"detail\":\"planted\"}}]' WHERE calc_id = {id}"));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(gate_of(&d, "G23")["result"], "INCONCLUSIVE", "{d}");
    // the job detail names the AMOEBA hold of the file: its code and its checks
    assert_eq!(
        d["export"]["amoeba_holds"][0]["code"], "amoeba_digest_inconclusive",
        "{d}"
    );
    assert_eq!(d["export"]["available"], false, "{d}");
    let (st, v) = t
        .call("GET", &format!("/api/calculations/{id}/dxf/{part}"), None)
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["code"], "amoeba_digest_inconclusive");
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'dxf_withheld' AND detail LIKE '%AMOEBA digest amoeba_digest_inconclusive%'") >= 1);
}

/// A result with no snapshot (none was recorded) is held: nothing can be handed out without one.
#[tokio::test]
async fn amoeba_a_missing_snapshot_holds_the_file() {
    let t = boot();
    let id = t.job(cone()).await;
    let part = part_names(&t.get(&format!("/api/calculations/{id}")).await)[0].clone();
    t.sql("DROP TRIGGER amoeba_no_delete");
    t.sql(&format!("DELETE FROM amoeba_digests WHERE calc_id = {id}"));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(gate_of(&d, "G23")["result"], "INCONCLUSIVE", "{d}");
    let (st, v) = t
        .call("GET", &format!("/api/calculations/{id}/dxf/{part}"), None)
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["code"], "amoeba_digest_missing");
}

/// The file served must be the file the snapshot describes: a different hash is withheld.
#[tokio::test]
async fn amoeba_the_served_file_must_be_the_snapshotted_file() {
    let t = boot();
    let id = t.job(cone()).await;
    let part = part_names(&t.get(&format!("/api/calculations/{id}")).await)[0].clone();
    t.sql("DROP TRIGGER amoeba_no_update");
    t.sql(&format!(
        "UPDATE amoeba_digests SET dxf_sha256 = '{}' WHERE calc_id = {id}",
        "0".repeat(64)
    ));
    let (st, v) = t
        .call("GET", &format!("/api/calculations/{id}/dxf/{part}"), None)
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["code"], "amoeba_snapshot_mismatch");
}

/// A snapshot of another file holds the export of the job: the job detail and the AMOEBA stage view name it.
#[tokio::test]
async fn amoeba_a_tampered_snapshot_is_named_by_the_export_and_the_stage_view() {
    let mut t = boot();
    let id = t.job(cone()).await;
    replay_matches(&mut t, id).await;
    let part = part_names(&t.get(&format!("/api/calculations/{id}")).await)[0].clone();
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["export"]["available"], true, "{d}");
    t.sql("DROP TRIGGER amoeba_no_update");
    t.sql(&format!(
        "UPDATE amoeba_digests SET dxf_sha256 = '{}' WHERE calc_id = {id} AND part = '{part}'",
        "0".repeat(64)
    ));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["export"]["available"], false, "{d}");
    assert_eq!(
        d["export"]["amoeba_holds"][0]["code"], "amoeba_snapshot_mismatch",
        "{d}"
    );
    let st = t.get(&format!("/api/calculations/{id}/amoeba")).await;
    assert_eq!(
        st["amoeba_holds"][0]["code"], "amoeba_snapshot_mismatch",
        "{st}"
    );
    assert_eq!(stage(&st, "export")["status"], "WITHHELD", "{st}");
}

/// Snapshots are immutable at the database level: an update or a delete is refused.
#[tokio::test]
async fn amoeba_snapshots_cannot_be_changed_or_deleted() {
    let t = boot();
    let id = t.job(cone()).await;
    let db = t.app.db();
    assert!(
        db.execute(
            &format!("UPDATE amoeba_digests SET verdict = 'PASS' WHERE calc_id = {id}"),
            []
        )
        .is_err()
    );
    assert!(
        db.execute(
            &format!("DELETE FROM amoeba_digests WHERE calc_id = {id}"),
            []
        )
        .is_err()
    );
}

/// The stage view: the five stages in order, plain JSON, for signed-in users only.
#[tokio::test]
async fn amoeba_stage_view_lists_the_stages_and_needs_a_session() {
    let t = boot();
    let id = t.job(good_assembly()).await;
    let v = t.get(&format!("/api/calculations/{id}/amoeba")).await;
    let names: Vec<&str> = v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["stage"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["surround", "engulf", "digest", "confirm", "export"]);
    assert_eq!(
        stage(&v, "confirm")["status"],
        "HOLD",
        "engineer approval (G16) is not implemented"
    );
    let (st, _, _) = t
        .send(
            "GET",
            &format!("/api/calculations/{id}/amoeba"),
            None,
            &[("cookie", "")],
        )
        .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn zz_diag_corpus_cases() {
    let t = boot();
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../validation/corpus/cases/"
    );
    for id in [
        "A-CONE-FLAT-0.01",
        "B-cone-apex-below-1e8",
        "B-cone-seam-vs-eps-num-above-band",
        "B-cone_frustum-seam-gap-above",
        "B-eccentric_reducer-seam-gap-above",
        "R-cone_frustum-0008",
        "R-cone_frustum-0009",
        "R-eccentric_reducer-0010",
        "N-aspect-cone-flat-h0.01",
    ] {
        let Ok(text) = std::fs::read_to_string(format!("{dir}{id}.json")) else {
            println!("DIAG {id}: no case file");
            continue;
        };
        let v: Value = serde_json::from_str(&text).unwrap();
        let (st, r) = t.calc(v["request"].clone()).await;
        if st != StatusCode::OK {
            println!("DIAG {id}: status {st}");
            continue;
        }
        let calc = r["id"].as_i64().unwrap();
        let s = t.get(&format!("/api/calculations/{calc}/amoeba")).await;
        for p in s["stages"][2]["parts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            for c in p["checks"].as_array().cloned().unwrap_or_default() {
                if c["verdict"] != "PASS" {
                    println!(
                        "DIAG {id} {} {} {}: {} m={} e={} tol={}",
                        p["part"],
                        c["name"],
                        c["verdict"],
                        c["detail"],
                        c["measured"],
                        c["expected"],
                        c["tolerance"]
                    );
                }
            }
        }
    }
}

/// Runs the replay audit of job `id` in a child that reproduces its release closure: G19 becomes INCONCLUSIVE, the
/// allow-listed result of a matching replay. A DXF is held (NOT_RUN) until this has run.
async fn replay_matches(t: &mut T, id: i64) {
    let stored = stored_result(&t.app.db(), id).unwrap().unwrap();
    let ids: Vec<&str> = stored["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    t.with_replay_exe(&format!("cat >/dev/null; echo '{}'", json!({ "engine": nf_core::ENGINE_VERSION, "closure_hash": stored["closure_hash"], "ids": ids })));
    let (st, v) = t
        .call("POST", &format!("/api/calculations/{id}/replay"), None)
        .await;
    assert_eq!(
        (st, v["matches"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{v}"
    );
}

/// The export rule (`export_hold`) on constructed gate tables. Holds: FAIL (critical or not), NOT_RUN, and
/// INCONCLUSIVE or NOT_IMPLEMENTED on a gate outside the allow-list. Passes: PASS, NOT_APPLICABLE, allow-listed gates.
#[test]
fn export_rule_holds_fail_not_run_and_unlisted_inconclusive_gates() {
    use GateResult as R;
    let eval = |gates: Vec<(&str, bool, R)>, open: i64| Eval {
        gates: gates
            .into_iter()
            .map(|(id, critical, result)| Gate {
                id: id.into(),
                name: "n".into(),
                critical,
                result,
                detail: "d".into(),
            })
            .collect(),
        state: REVIEW_REQUIRED,
        integrity: vec![],
        engine_current: true,
        open_obligations: open,
    };
    let held = |gates: Vec<(&str, bool, R)>, open: i64| {
        export_hold(&eval(gates, open)).map(|g| g.id.clone())
    };
    // PASS and NOT_APPLICABLE never hold
    assert_eq!(
        held(
            vec![("G05", true, R::Pass), ("G06", false, R::NotApplicable)],
            0
        ),
        None
    );
    // the exploit of SECURITY-UXF1-1 F1: a non-critical FAIL must hold
    assert_eq!(
        held(vec![("G05", true, R::Pass), ("G06", false, R::Fail)], 0),
        Some("G06".into())
    );
    // a FAIL of an allow-listed gate still holds
    assert_eq!(held(vec![("G16", false, R::Fail)], 0), Some("G16".into()));
    // NOT_RUN holds
    assert_eq!(held(vec![("G19", true, R::NotRun)], 0), Some("G19".into()));
    // INCONCLUSIVE or NOT_IMPLEMENTED on a gate outside the allow-list holds
    assert_eq!(
        held(vec![("G05", true, R::Inconclusive)], 0),
        Some("G05".into())
    );
    assert_eq!(
        held(vec![("G05", true, R::NotImplemented)], 0),
        Some("G05".into())
    );
    // allow-listed gates do not hold
    assert_eq!(
        held(
            vec![
                ("G16", true, R::NotImplemented),
                ("G02", true, R::Inconclusive),
                ("G19", true, R::Inconclusive)
            ],
            0
        ),
        None
    );
    // G18 holds while an obligation is open, and passes otherwise
    assert_eq!(
        held(vec![("G18", true, R::Inconclusive)], 1),
        Some("G18".into())
    );
    assert_eq!(held(vec![("G18", true, R::Inconclusive)], 0), None);
    // the first holding gate is the one named, with its result and one fix
    let e = eval(
        vec![("G02", true, R::Inconclusive), ("G05", true, R::Fail)],
        0,
    );
    let g = export_hold(&e).unwrap();
    let h = hold_json(g);
    assert_eq!(
        (h["gate"].as_str(), h["result"].as_str()),
        (Some("G05"), Some("FAIL"))
    );
    assert!(
        h["error"]
            .as_str()
            .unwrap()
            .starts_with("held by AMOEBA: gate G05 (n) is FAIL: d Fix: "),
        "{h}"
    );
}

/// R3-8: a withheld check with no engine reason is explained by its own figures, never by an empty string.
#[test]
fn withheld_reason_falls_back_to_the_check_figures() {
    let engine = json!({ "reason": "surface unknown", "difference": 0.0, "tolerance": 1.0 });
    assert_eq!(withheld_reason(&engine), "surface unknown");
    let within = json!({ "difference": 1e-9, "tolerance": 1e-6 });
    assert!(
        withheld_reason(&within).starts_with("difference 0.000000001 is within tolerance 0.000001")
    );
    let beyond = json!({ "reason": "", "difference": 2.5, "tolerance": 1.0 });
    assert!(withheld_reason(&beyond).starts_with("difference 2.5 exceeds tolerance 1"));
    let nan = json!({ "difference": null, "tolerance": 1.0 });
    assert!(withheld_reason(&nan).starts_with("a value of the check is not a finite number"));
}

// ---------------------------------------------------------------- AUTH-1: recovery and automatic sign-in

const A1_PW: &str = "correct-horse-9";

fn a1_app(desktop: bool, tag: &str) -> T {
    let dir = temp_dir(tag);
    let app = build_app(Config {
        desktop,
        allowed_hosts: vec!["127.0.0.1".into(), "localhost".into()],
        licence_enforce: false,
        export_dirs: vec![],
        ..config(&dir)
    })
    .unwrap();
    T {
        app,
        dir,
        cookie: "none=1".into(),
    }
}

/// First-run setup of "admin"; returns the recovery code.
async fn a1_setup(t: &T, auto: bool) -> (StatusCode, Value) {
    t.send(
        "POST",
        "/api/setup",
        Some(json!({ "username": "admin", "password": A1_PW, "auto_signin": auto })),
        &[("x-nf-setup-token", TOKEN)],
    )
    .await
    .pipe_json()
}

trait PipeJson {
    fn pipe_json(self) -> (StatusCode, Value);
}
impl PipeJson for (StatusCode, HeaderMap, Vec<u8>) {
    fn pipe_json(self) -> (StatusCode, Value) {
        (
            self.0,
            serde_json::from_slice(&self.2).unwrap_or(Value::Null),
        )
    }
}

async fn a1_login(t: &T, name: &str, pw: &str) -> Option<String> {
    let (st, h, _) = t
        .send(
            "POST",
            "/api/login",
            Some(json!({ "username": name, "password": pw })),
            &[("cookie", "none=1")],
        )
        .await;
    if st != StatusCode::OK {
        return None;
    }
    let c = h.get("set-cookie")?.to_str().ok()?;
    Some(c.split(';').next()?.to_string())
}

async fn a1_as(t: &T, cookie: &str, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    t.send(method, path, Some(body), &[("cookie", cookie)])
        .await
        .pipe_json()
}

async fn a1_launch(t: &T, tok: impl Into<String>) -> (StatusCode, HeaderMap, Vec<u8>) {
    t.send(
        "POST",
        "/api/launch",
        Some(json!({ "token": tok.into() })),
        &[("cookie", "none=1")],
    )
    .await
}

async fn a1_recover(t: &T, name: &str, code: &str, pw: &str) -> (StatusCode, Value) {
    t.send(
        "POST",
        "/api/recover",
        Some(json!({ "username": name, "recovery_code": code, "new_password": pw })),
        &[("cookie", "none=1")],
    )
    .await
    .pipe_json()
}

#[tokio::test]
async fn auth1_setup_returns_a_recovery_code_stored_only_as_a_hash() {
    let t = a1_app(false, "a1-setup");
    let (st, v) = a1_setup(&t, false).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let code = v["recovery_code"].as_str().unwrap().to_string();
    let groups: Vec<&str> = code.split('-').collect();
    assert_eq!(groups.len(), 4, "{code}");
    assert!(groups.iter().all(|g| g.len() == 5), "{code}");
    assert!(
        code.chars()
            .filter(|c| *c != '-')
            .all(|c| "23456789ABCDEFGHJKMNPQRSTUVWXYZ@".contains(c)),
        "{code}"
    );
    assert!(
        !code.contains(['0', 'O', '1', 'I', 'L']),
        "ambiguous symbol in {code}"
    );
    // 32 symbols per character, 20 characters
    assert_eq!(CODE_BITS, 100);
    let (salt, hash): (String, String) = t
        .app
        .db()
        .query_row(
            "SELECT recovery_salt, recovery_hash FROM users WHERE username = 'admin'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(!salt.is_empty() && hash.len() == 64 && !hash.contains(&code.replace('-', "")));
    let dump: String = t
        .app
        .db()
        .query_row(
            "SELECT group_concat(username || salt || hash || ifnull(recovery_salt,'') || ifnull(recovery_hash,''), '|') FROM users",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !dump.contains(&code.replace('-', "")),
        "the code is stored in clear"
    );
    // two codes differ
    let t2 = a1_app(false, "a1-setup2");
    let (_, v2) = a1_setup(&t2, false).await;
    assert_ne!(v2["recovery_code"], v["recovery_code"]);
}

const CODE_BITS: u32 = (auth::CODE_LEN as u32) * 5;

#[tokio::test]
async fn auth1_recovery_happy_path_single_use_sessions_revoked() {
    let t = a1_app(false, "a1-happy");
    let (_, v) = a1_setup(&t, false).await;
    let code = v["recovery_code"].as_str().unwrap().to_string();
    let old = a1_login(&t, "admin", A1_PW).await.expect("sign-in");
    assert_eq!(
        a1_as(&t, &old, "GET", "/api/me", json!(null)).await.0,
        StatusCode::OK
    );
    // typed sloppily: lower case, spaces instead of dashes
    let typed = code.to_lowercase().replace('-', " ");
    let (st, v) = a1_recover(&t, "admin", &typed, "brand-new-pass-7").await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["ok"], true);
    let new_code = v["recovery_code"].as_str().unwrap().to_string();
    assert_ne!(new_code, code);
    // sessions revoked, old password dead, new password works
    assert_eq!(
        a1_as(&t, &old, "GET", "/api/me", json!(null)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(a1_login(&t, "admin", A1_PW).await.is_none());
    assert!(a1_login(&t, "admin", "brand-new-pass-7").await.is_some());
    // the used code is dead
    let (st, v) = a1_recover(&t, "admin", &code, "another-pass-88").await;
    assert_eq!(
        (st, v["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("recovery failed"))
    );
    assert!(a1_login(&t, "admin", "brand-new-pass-7").await.is_some());
    // the new code works, once
    let (st, v) = a1_recover(&t, "admin", &new_code, "third-pass-123").await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        a1_recover(&t, "admin", &new_code, "fourth-pass-123")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // password rules apply
    let c3 = v["recovery_code"].as_str().unwrap();
    let (st, _) = a1_recover(&t, "admin", c3, "short").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'account.recovered'") == 2);
}

#[tokio::test]
async fn auth1_wrong_code_gives_a_generic_error_and_is_throttled() {
    let t = a1_app(false, "a1-throttle");
    let (_, v) = a1_setup(&t, false).await;
    let code = v["recovery_code"].as_str().unwrap().to_string();
    let (s1, b1) = a1_recover(&t, "admin", "AAAAA-AAAAA-AAAAA-AAAAA", "brand-new-pass-7").await;
    let (s2, b2) = a1_recover(&t, "nobody", &code, "brand-new-pass-7").await;
    let (s3, b3) = a1_recover(&t, "admin", "x", "brand-new-pass-7").await;
    assert_eq!(
        (s1, s2, s3),
        (
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED
        )
    );
    assert_eq!(b1, b2, "unknown user and wrong code must look the same");
    assert_eq!(b1, b3);
    assert_eq!(b1["error"], "recovery failed");
    // 5 failures of the account from this address lock the pair (two are counted above); even the right code is then refused
    let _ = a1_recover(&t, "admin", "DDDDD-DDDDD-DDDDD-DDDDD", "brand-new-pass-7").await;
    let _ = a1_recover(&t, "admin", "BBBBB-BBBBB-BBBBB-BBBBB", "brand-new-pass-7").await;
    let _ = a1_recover(&t, "admin", "CCCCC-CCCCC-CCCCC-CCCCC", "brand-new-pass-7").await;
    let (st, v) = a1_recover(&t, "admin", &code, "brand-new-pass-7").await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS, "{v}");
    assert_eq!(v["throttled"], true);
    // and the password was not changed
    assert!(
        a1_login(&t, "admin", A1_PW).await.is_none(),
        "sign-in shares the lock"
    );
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'account.recovered'") == 0);
}

#[tokio::test]
async fn auth1_admin_reset_forces_a_change_and_never_on_self() {
    let t = a1_app(false, "a1-reset");
    let _ = a1_setup(&t, false).await;
    let adm = a1_login(&t, "admin", A1_PW).await.unwrap();
    let (st, v) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/users",
        json!({"username":"bob","password":"bobs-first-pass-1","role":"engineer"}),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let bob_id = v["id"].as_i64().unwrap();
    let bob = a1_login(&t, "bob", "bobs-first-pass-1").await.unwrap();
    // not on self, not by a non-admin, not for a weak password, not for an unknown id
    let admin_id = t.count("SELECT id FROM users WHERE username = 'admin'");
    let path = |id: i64| format!("/api/users/{id}/reset-password");
    assert_eq!(
        a1_as(
            &t,
            &adm,
            "POST",
            &path(admin_id),
            json!({"new_password":"whatever-pass-12","password":A1_PW})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        a1_as(
            &t,
            &bob,
            "POST",
            &path(admin_id),
            json!({"new_password":"whatever-pass-12"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        a1_as(
            &t,
            &adm,
            "POST",
            &path(bob_id),
            json!({"new_password":"short","password":A1_PW})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        a1_as(
            &t,
            &adm,
            "POST",
            &path(999),
            json!({"new_password":"whatever-pass-12","password":A1_PW})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // the administrator's own password is required: missing and wrong change nothing
    for body in [
        json!({"new_password":"temporary-pass-55"}),
        json!({"new_password":"temporary-pass-55","password":"wrong-password-1"}),
    ] {
        let (st, v) = a1_as(&t, &adm, "POST", &path(bob_id), body).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{v}");
    }
    assert_eq!(
        a1_as(&t, &bob, "GET", "/api/me", json!(null)).await.0,
        StatusCode::OK,
        "a refused reset leaves the user signed in"
    );
    assert!(a1_login(&t, "bob", "bobs-first-pass-1").await.is_some());
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'account.password_reset'"),
        0
    );
    let bob = a1_login(&t, "bob", "bobs-first-pass-1").await.unwrap();
    let (st, v) = a1_as(
        &t,
        &adm,
        "POST",
        &path(bob_id),
        json!({"new_password":"temporary-pass-55","password":A1_PW}),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    // bob's session is gone, the old password is dead
    assert_eq!(
        a1_as(&t, &bob, "GET", "/api/me", json!(null)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(a1_login(&t, "bob", "bobs-first-pass-1").await.is_none());
    let bob = a1_login(&t, "bob", "temporary-pass-55").await.unwrap();
    let (st, me) = a1_as(&t, &bob, "GET", "/api/me", json!(null)).await;
    assert_eq!(
        (st, &me["must_change_password"]),
        (StatusCode::OK, &json!(true)),
        "{me}"
    );
    // everything else is refused until the password is changed
    let (st, v) = a1_as(&t, &bob, "GET", "/api/projects", json!(null)).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["must_change_password"], true);
    let (st, v) = a1_as(
        &t,
        &bob,
        "POST",
        "/api/password",
        json!({"current":"temporary-pass-55","new":"bobs-own-pass-99"}),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let bob = a1_login(&t, "bob", "bobs-own-pass-99").await.unwrap();
    let (_, me) = a1_as(&t, &bob, "GET", "/api/me", json!(null)).await;
    assert_eq!(me["must_change_password"], false);
    assert_eq!(
        a1_as(&t, &bob, "GET", "/api/projects", json!(null)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'account.password_reset'"),
        1
    );
}

#[tokio::test]
async fn auth1_new_recovery_code_needs_the_password_and_replaces_the_old_one() {
    let t = a1_app(false, "a1-newcode");
    let (_, v) = a1_setup(&t, false).await;
    let old = v["recovery_code"].as_str().unwrap().to_string();
    let adm = a1_login(&t, "admin", A1_PW).await.unwrap();
    let (st, _) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/me/recovery-code",
        json!({"password":"wrong-password-1"}),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, v) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/me/recovery-code",
        json!({"password":A1_PW}),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let fresh = v["recovery_code"].as_str().unwrap().to_string();
    assert_ne!(fresh, old);
    assert_eq!(
        a1_recover(&t, "admin", &old, "brand-new-pass-7").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        a1_recover(&t, "admin", &fresh, "brand-new-pass-7").await.0,
        StatusCode::OK
    );
    // an engineer cannot
    let adm = a1_login(&t, "admin", "brand-new-pass-7").await.unwrap();
    a1_as(
        &t,
        &adm,
        "POST",
        "/api/users",
        json!({"username":"eve","password":"eves-first-pass-1","role":"engineer"}),
    )
    .await;
    let eve = a1_login(&t, "eve", "eves-first-pass-1").await.unwrap();
    assert_eq!(
        a1_as(
            &t,
            &eve,
            "POST",
            "/api/me/recovery-code",
            json!({"password":"eves-first-pass-1"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn auth1_an_existing_database_without_a_code_can_get_one() {
    let t = a1_app(false, "a1-migrate");
    let _ = a1_setup(&t, false).await;
    // as an installation from before this change: no recovery code, flag says so
    t.sql("UPDATE users SET recovery_salt = NULL, recovery_hash = NULL");
    let adm = a1_login(&t, "admin", A1_PW).await.unwrap();
    let (_, me) = a1_as(&t, &adm, "GET", "/api/me", json!(null)).await;
    assert_eq!(me["has_recovery_code"], false, "{me}");
    assert_eq!(
        a1_recover(&t, "admin", "AAAAA-AAAAA-AAAAA-AAAAA", "brand-new-pass-7")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (st, v) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/me/recovery-code",
        json!({"password":A1_PW}),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (_, me) = a1_as(&t, &adm, "GET", "/api/me", json!(null)).await;
    assert_eq!(me["has_recovery_code"], true);
    // the columns are added to a database that lacks them
    let t2 = a1_app(false, "a1-migrate2");
    drop(t2.app.db());
    let c = t2.app.db();
    for col in ["recovery_salt", "recovery_hash", "must_change"] {
        assert!(
            c.prepare(&format!("SELECT {col} FROM users")).is_ok(),
            "{col}"
        );
    }
}

#[tokio::test]
async fn auth1_server_mode_has_no_launch_and_no_auto_signin() {
    let t = a1_app(false, "a1-server");
    let (st, v) = a1_setup(&t, true).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(
        t.get("/api/health").await["setup_required"],
        true,
        "refused setup creates nothing"
    );
    let (st, v) = a1_setup(&t, false).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, _) = t
        .send(
            "POST",
            "/api/launch",
            Some(json!({"token":"abc"})),
            &[("cookie", "none=1")],
        )
        .await
        .pipe_json();
    assert_eq!(st, StatusCode::NOT_FOUND);
    let adm = a1_login(&t, "admin", A1_PW).await.unwrap();
    let (st, v) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/settings/auto-signin",
        json!({"enabled":true,"password":A1_PW}),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
    let (_, s) = t
        .send("GET", "/api/session", None, &[("cookie", "none=1")])
        .await
        .pipe_json();
    assert_eq!(
        (s["mode"].as_str(), s["auto_signin"].as_bool()),
        (Some("server"), Some(false)),
        "{s}"
    );
    assert_eq!(auth::issue_launch_token(&t.app), None);
}

#[tokio::test]
async fn auth1_launch_token_single_use_expiry_replay_forged_and_os_user_never_admin() {
    let t = a1_app(true, "a1-launch");
    t.app.auth.lock().unwrap().os_user = Some("tester".into());
    let (st, v) = a1_setup(&t, true).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (_, s) = t
        .send("GET", "/api/session", None, &[("cookie", "none=1")])
        .await
        .pipe_json();
    assert_eq!(
        (s["mode"].as_str(), s["auto_signin"].as_bool()),
        (Some("desktop"), Some(true)),
        "{s}"
    );
    let tok = auth::issue_launch_token(&t.app).expect("token when auto sign-in is on");
    assert_eq!(tok.len(), 64);
    // forged
    assert_eq!(
        a1_launch(&t, "0".repeat(64)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        a1_launch(&t, String::new()).await.0,
        StatusCode::UNAUTHORIZED
    );
    // the real one works once
    let (st, h, body) = a1_launch(&t, tok.clone()).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        (v["username"].as_str(), v["role"].as_str()),
        (Some("os:tester"), Some("engineer"))
    );
    let cookie = h
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let (st, me) = a1_as(&t, &cookie, "GET", "/api/me", json!(null)).await;
    assert_eq!(
        (st, me["role"].as_str()),
        (StatusCode::OK, Some("engineer")),
        "{me}"
    );
    // replay
    assert_eq!(a1_launch(&t, tok).await.0, StatusCode::UNAUTHORIZED);
    // an engineer session cannot do administrator things, and the account has no password
    assert_eq!(
        a1_as(&t, &cookie, "GET", "/api/users", json!(null)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        a1_as(
            &t,
            &cookie,
            "POST",
            "/api/settings/auto-signin",
            json!({"enabled":false,"password":"x"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert!(a1_login(&t, "os:tester", "").await.is_none());
    assert_eq!(
        t.count("SELECT COUNT(*) FROM users WHERE username = 'os:tester' AND role = 'engineer'"),
        1
    );
    assert_eq!(
        t.count("SELECT COUNT(*) FROM users WHERE username LIKE 'os:%' AND role = 'admin'"),
        0
    );
    // the audit log names the operating-system user
    assert_eq!(
        t.count(
            "SELECT COUNT(*) FROM audit WHERE action = 'login.auto' AND detail LIKE '%tester%'"
        ),
        1
    );
    // a second launch reuses the account
    let tok2 = auth::issue_launch_token(&t.app).unwrap();
    assert_eq!(a1_launch(&t, tok2).await.0, StatusCode::OK);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM users WHERE username = 'os:tester'"),
        1
    );
    // expiry
    let tok3 = auth::issue_launch_token(&t.app).unwrap();
    for e in t.app.auth.lock().unwrap().launch.iter_mut() {
        e.1 = std::time::Instant::now() - Duration::from_secs(1);
    }
    assert_eq!(
        a1_launch(&t, tok3).await.0,
        StatusCode::UNAUTHORIZED,
        "expired token"
    );
    // the administrator can reset nobody's os account, and switching the setting off stops launches
    let adm = a1_login(&t, "admin", A1_PW).await.unwrap();
    let os_id = t.count("SELECT id FROM users WHERE username = 'os:tester'");
    assert_eq!(
        a1_as(
            &t,
            &adm,
            "POST",
            &format!("/api/users/{os_id}/reset-password"),
            json!({"new_password":"whatever-pass-12","password":A1_PW})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let tok4 = auth::issue_launch_token(&t.app).unwrap();
    assert_eq!(
        a1_as(
            &t,
            &adm,
            "POST",
            "/api/settings/auto-signin",
            json!({"enabled":false,"password":"wrong-password-1"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (st, v) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/settings/auto-signin",
        json!({"enabled":false,"password":A1_PW}),
    )
    .await;
    assert_eq!(
        (st, &v["auto_signin"]),
        (StatusCode::OK, &json!(false)),
        "{v}"
    );
    assert_eq!(
        a1_launch(&t, tok4).await.0,
        StatusCode::UNAUTHORIZED,
        "tokens are dropped when the setting is off"
    );
    assert_eq!(
        a1_as(&t, &cookie, "GET", "/api/me", json!(null)).await.0,
        StatusCode::UNAUTHORIZED,
        "an automatic sign-in session ends when the setting is turned off"
    );
    assert_eq!(
        a1_as(&t, &adm, "GET", "/api/me", json!(null)).await.0,
        StatusCode::OK,
        "other sessions stay"
    );
    assert_eq!(auth::issue_launch_token(&t.app), None);
    // back on later, by the administrator
    let (st, _) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/settings/auto-signin",
        json!({"enabled":true,"password":A1_PW}),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(auth::issue_launch_token(&t.app).is_some());
}

#[tokio::test]
async fn auth1_no_secret_reaches_the_audit_log_or_the_plain_database() {
    let t = a1_app(true, "a1-secrets");
    t.app.auth.lock().unwrap().os_user = Some("tester".into());
    let (_, v) = a1_setup(&t, true).await;
    let code = v["recovery_code"].as_str().unwrap().to_string();
    let adm = a1_login(&t, "admin", A1_PW).await.unwrap();
    let (_, v2) = a1_as(
        &t,
        &adm,
        "POST",
        "/api/me/recovery-code",
        json!({"password":A1_PW}),
    )
    .await;
    let code2 = v2["recovery_code"].as_str().unwrap().to_string();
    let _ = a1_recover(&t, "admin", "AAAAA-AAAAA-AAAAA-AAAAA", "never-used-pass-1").await;
    let (_, v3) = a1_recover(&t, "admin", &code2, "recovered-pass-42").await;
    let code3 = v3["recovery_code"].as_str().unwrap().to_string();
    let tok = auth::issue_launch_token(&t.app).unwrap();
    let _ = t
        .send(
            "POST",
            "/api/launch",
            Some(json!({"token": tok.clone()})),
            &[("cookie", "none=1")],
        )
        .await;
    let _ = t
        .send(
            "POST",
            "/api/launch",
            Some(json!({"token": "f".repeat(64)})),
            &[("cookie", "none=1")],
        )
        .await;
    let log: String = t
        .app
        .db()
        .query_row(
            "SELECT group_concat(username || '|' || action || '|' || detail, char(10)) FROM audit",
            [],
            |r| r.get(0),
        )
        .unwrap();
    for secret in [
        &code,
        &code2,
        &code3,
        &tok,
        &A1_PW.to_string(),
        &"recovered-pass-42".to_string(),
        &"never-used-pass-1".to_string(),
        &"f".repeat(64),
    ] {
        for needle in [secret.clone(), secret.replace('-', "")] {
            assert!(
                !log.contains(&needle),
                "secret in the audit log: {}",
                &needle[..4]
            );
        }
    }
    assert!(
        log.contains("account.recovered")
            && log.contains("login.auto")
            && log.contains("recover_failed")
    );
    // the codes are not in the database in clear either; the launch token only as a hash in memory
    let users: String = t.app.db().query_row(
        "SELECT group_concat(ifnull(recovery_salt,'') || ifnull(recovery_hash,'') || hash, '|') FROM users",
        [],
        |r| r.get(0),
    ).unwrap();
    for c in [&code, &code2, &code3] {
        assert!(!users.contains(&c.replace('-', "")));
    }
    assert!(auth::os_user_name().map(|n| !n.is_empty()).unwrap_or(true));
}

// ---------------------------------------------------------------- NF CAD phase 1 (read-only)

async fn send_bytes(t: &T, method: &str, path: &str, body: &[u8], filename: &str, cookie: &str) -> (StatusCode, Value) {
    let req = HttpRequest::builder()
        .method(method)
        .uri(path)
        .header("x-nf-request", "1")
        .header("host", "127.0.0.1:8787")
        .header("cookie", cookie)
        .header("x-filename", filename)
        .header("content-type", "application/octet-stream")
        .body(Body::from(body.to_vec()))
        .unwrap();
    let resp = router(t.app.clone()).oneshot(req).await.unwrap();
    let (parts, b) = resp.into_parts();
    let bytes = axum::body::to_bytes(b, 64 * 1024 * 1024).await.unwrap().to_vec();
    (parts.status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

const CAD_BASIC: &[u8] = include_bytes!("../../nf-cad/tests/fixtures/basic.dxf");

#[tokio::test]
async fn cad_open_measure_pick_and_checks_on_an_uploaded_dxf() {
    let t = boot();
    let (st, up) = send_bytes(&t, "POST", "/api/projects/1/documents", CAD_BASIC, "basic.dxf", &t.cookie).await;
    assert_eq!(st, StatusCode::OK, "{up}");
    let id = up["documents"][0]["id"].as_i64().or(up["id"].as_i64()).unwrap_or(1);
    let m = t.get(&format!("/api/cad/documents/{id}")).await;
    assert_eq!(m["units"], "millimeter");
    assert_eq!(m["entity_count"], 7);
    assert_eq!(m["entities"][0]["id"], "E0");
    let meas = t.get(&format!("/api/cad/documents/{id}/measure?kind=length&entity=E0")).await;
    assert!((meas["values"][0]["value"].as_f64().unwrap() - 5.0).abs() < 1e-9);
    let pk = t.get(&format!("/api/cad/documents/{id}/pick?at=1.5,2&tol=0.01")).await;
    assert_eq!(pk["entity"]["id"], "E0");
    let ck = t.get(&format!("/api/cad/documents/{id}/checks")).await;
    assert_eq!(ck["rules"].as_array().unwrap().len(), 14);
    assert_eq!(ck["rules"][0]["rule"], "readable");
    assert_eq!(ck["rules"][0]["pass"], false);
    // each open is audited
    assert!(t.count("SELECT COUNT(*) FROM audit WHERE action = 'cad_open'") >= 1);
    // bad requests are refused with a reason, not a panic
    let (st, v) = t.call("GET", &format!("/api/cad/documents/{id}/measure?kind=area&entity=E0"), None).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    let (st, _) = t.call("GET", &format!("/api/cad/documents/{id}/measure?kind=teleport"), None).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = t.call("GET", &format!("/api/cad/documents/{id}/pick?at=x"), None).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn cad_needs_a_session_and_project_membership() {
    let t = boot();
    let (st, _) = send_bytes(&t, "POST", "/api/projects/1/documents", CAD_BASIC, "basic.dxf", &t.cookie).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, _) = t.send("GET", "/api/cad/documents/1", None, &[("cookie", "none=1")]).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let outsider = add_session(&t.app, "outsider", "viewer");
    let (st, _) = t.call("GET", "/api/cad/documents/1", None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, _) = t.send("GET", "/api/cad/documents/1", None, &[("cookie", &outsider)]).await;
    assert_ne!(st, StatusCode::OK, "a user outside the project must not read the drawing");
    let (st, _, _) = t.send("GET", "/api/cad/documents/999", None, &[]).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cad_refuses_malformed_input_without_panic() {
    let t = boot();
    let bad: &[u8] = b"0\nSECTION\n2\nENTITIES\n0\nLINE\n10\nnan\n";
    let (st, up) = send_bytes(&t, "POST", "/api/projects/1/documents", bad, "bad.dxf", &t.cookie).await;
    assert!(st.is_success() || st.is_client_error(), "{up}");
    if st == StatusCode::OK {
        let id = up["documents"][0]["id"].as_i64().or(up["id"].as_i64()).unwrap_or(1);
        let (st, v) = t.call("GET", &format!("/api/cad/documents/{id}"), None).await;
        assert!(st.is_client_error(), "malformed DXF must be refused: {v}");
        assert!(v["code"].is_string());
    }
    let (st, _) = t.call("GET", "/api/cad/documents/1/checks", None).await;
    assert!(st.is_success() || st.is_client_error());
}

#[test]
fn cad_reads_stay_outside_the_licence_gate() {
    use axum::http::Method;
    for p in ["/api/cad/documents/1", "/api/cad/documents/1/measure", "/api/cad/calculations/1/cut/pick"] {
        assert!(!crate::licence::gated(&Method::GET, p), "{p} is a read and must stay available on a read-only licence");
    }
}

// ---------------------------------------------------------------- machine export (nf-machine)

fn boot_with_exports(dirs: Vec<PathBuf>) -> T {
    let dir = temp_dir("machine");
    let mut cfg = config(&dir);
    cfg.export_dirs = dirs;
    let app = build_app(cfg).unwrap();
    let cookie = add_session(&app, "admin", "admin");
    *app.setup_token.lock().unwrap() = None;
    let t = T { app, dir, cookie };
    t.app.db().execute("INSERT INTO projects(name, description, created_by, created_at) VALUES ('p','', 'admin', 0)", []).unwrap();
    t
}

fn machine_profile_json() -> Value {
    json!({
        "id": "PLASMA-A", "version": 1, "name": "Plasma A (test fixture)",
        "dxf_version": "R12", "text_encoding": "shift_jis", "units": "mm",
        "layer_map": { "CUT_OUTER": "OUTER", "CUT_INNER": "INNER", "NF_NOT_RELEASED": "NOTE", "0": "0" },
        "file_name": "{job}_{part}_{profile}.dxf",
        "kerf_mm": null, "lead_in_mm": null,
        "sources": ["test fixture, not a machine manual"], "tested_by": null
    })
}

fn files_in(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
}

#[tokio::test]
async fn machine_export_agent_is_refused_and_a_person_writes_only_certified_parts() {
    let out = temp_dir("machine-out");
    std::fs::create_dir_all(&out).unwrap();
    let mut t = boot_with_exports(vec![out.clone()]);
    let (st, v) = t
        .call(
            "POST",
            "/api/machine/profiles",
            Some(machine_profile_json()),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        t.call(
            "POST",
            "/api/machine/profiles",
            Some(json!({ "id": "BAD ID", "version": 1 }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let id = t.job(good_assembly()).await;
    replay_matches(&mut t, id).await;

    let body = json!({ "profile": "PLASMA-A", "folder": 0 });
    let path = format!("/api/calculations/{id}/export");
    // an agent identity may propose, never export
    let agent = add_session(&t.app, "amoeba", "engineer");
    t.sql("UPDATE users SET kind = 'agent' WHERE username = 'amoeba'");
    let (st, _, _) = t
        .send("POST", &path, Some(body.clone()), &[("cookie", &agent)])
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(files_in(&out), 0, "agent export must write nothing");

    // a person exports the engine's DXF: it states its units (NF-UNITS: mm) with no $INSUNITS, so the hand-off
    // file gets $INSUNITS 4 added to its header and the manifest is written with the export
    let (st, v) = t.call("POST", &path, Some(body.clone())).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["written"], json!(true));
    assert!(out.join(format!("manifest-calc-{id}-v1.json")).exists());
    let dxf_file = v["files"][0]["file"].as_str().unwrap().to_string();
    let written = std::fs::read_to_string(out.join(&dxf_file)).unwrap();
    assert!(
        written.contains("$INSUNITS\r\n70\r\n4\r\n"),
        "hand-off header lacks $INSUNITS 4"
    );
    assert!(t.app.data_dir.join("machine-audit.jsonl").exists());

    // a folder that was not configured is refused
    let (st, _) = t
        .call(
            "POST",
            &path,
            Some(json!({ "profile": "PLASMA-A", "folder": 7 })),
        )
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn machine_export_refuses_a_held_part_and_writes_nothing() {
    let out = temp_dir("machine-held");
    std::fs::create_dir_all(&out).unwrap();
    let t = boot_with_exports(vec![out.clone()]);
    t.call(
        "POST",
        "/api/machine/profiles",
        Some(machine_profile_json()),
    )
    .await;
    // no replay: the export rule holds the file (G19 not run), so nothing is certified
    let id = t.job(good_assembly()).await;
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/calculations/{id}/export"),
            Some(json!({ "profile": "PLASMA-A", "folder": 0 })),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["written"], json!(false));
    assert_eq!(files_in(&out), 0);
}

#[tokio::test]
async fn machine_export_refuses_a_digest_that_does_not_match_the_file() {
    let out = temp_dir("machine-hash");
    std::fs::create_dir_all(&out).unwrap();
    let mut t = boot_with_exports(vec![out.clone()]);
    t.call(
        "POST",
        "/api/machine/profiles",
        Some(machine_profile_json()),
    )
    .await;
    let id = t.job(good_assembly()).await;
    replay_matches(&mut t, id).await;
    // the stored digest of every part now names a different file (test-only: the append-only trigger is removed)
    t.sql("DROP TRIGGER amoeba_no_update");
    t.sql("UPDATE amoeba_digests SET dxf_sha256 = '0000000000000000000000000000000000000000000000000000000000000000'");
    let (st, v) = t
        .call(
            "POST",
            &format!("/api/calculations/{id}/export"),
            Some(json!({ "profile": "PLASMA-A", "folder": 0 })),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert!(
        matches!(v["code"].as_str(), Some("held") | Some("hash_mismatch")),
        "a mismatched digest must be refused: {v}"
    );
    assert_eq!(files_in(&out), 0);
}

#[tokio::test]
async fn machine_panel_has_no_inline_style_or_script() {
    // the CSP is style-src 'self': the panel must load its CSS and JS from files, never inline
    let t = boot_with_exports(vec![temp_dir("machine-ui")]);
    let (st, _, bytes) = t.send("GET", "/machine", None, &[]).await;
    assert_eq!(st, StatusCode::OK);
    let html = String::from_utf8(bytes).unwrap();
    assert!(
        !html.contains("<style"),
        "inline <style> block in the machine panel"
    );
    assert!(
        !html.to_ascii_lowercase().contains(" style="),
        "inline style attribute in the machine panel"
    );
    assert!(html.contains(r#"href="/machine/app.css""#));
    for (i, _) in html.match_indices("<script") {
        let end = html[i..].find('>').unwrap();
        assert!(
            html[i..i + end].contains(" src="),
            "inline <script> in the machine panel"
        );
    }
    let (st, h, css) = t.send("GET", "/machine/app.css", None, &[]).await;
    assert_eq!(st, StatusCode::OK);
    assert!(h["content-type"].to_str().unwrap().starts_with("text/css"));
    assert!(!css.is_empty());
    let (st, _, js) = t.send("GET", "/machine/app.mjs", None, &[]).await;
    assert_eq!(st, StatusCode::OK);
    let js = String::from_utf8(js).unwrap();
    assert!(
        !js.contains(".style") && !js.contains("cssText") && !js.contains("\"style\""),
        "script writes inline styles"
    );
}

// ---------------------------------------------------------------- NF CAD phase 2: edits, versions, certified refusal

const CAD_DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$HANDSEED\n5\nFF\n0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n0\nLAYER\n2\n0\n62\n7\n70\n0\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n5\nB1\n10\n0\n20\n0\n11\n3\n21\n0\n0\nENDSEC\n0\nEOF\n";

async fn upload_raw(t: &T, cookie: &str, name: &str, bytes: Vec<u8>) -> StatusCode {
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/api/projects/1/documents")
        .header("host", "localhost")
        .header("x-nf-request", "1")
        .header("cookie", cookie)
        .header("x-filename", name)
        .body(Body::from(bytes))
        .unwrap();
    router(t.app.clone()).oneshot(req).await.unwrap().status()
}

fn last_doc(t: &T) -> i64 {
    t.app.db().query_row("SELECT id FROM documents ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).unwrap()
}

fn doc_sha(t: &T, id: i64) -> String {
    t.app.db().query_row("SELECT sha256 FROM documents WHERE id = ?", [id], |r| r.get(0)).unwrap()
}

fn move_op() -> Value {
    json!({ "ops": [{ "op": "move", "ids": ["E0"], "dx": 2.0, "dy": 0.0 }] })
}

#[tokio::test]
async fn cad_edit_creates_a_new_version_and_never_changes_the_parent() {
    let t = boot();
    let parent_status = upload_raw(&t, &t.cookie, "part.dxf", CAD_DXF.as_bytes().to_vec()).await;
    assert_eq!(parent_status, StatusCode::OK, "the DXF is admitted");
    let parent = last_doc(&t);
    let parent_sha = doc_sha(&t, parent);
    let (st, v) = t.call("POST", &format!("/api/cad/documents/{parent}/edit"), Some(move_op())).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let child = v["document_id"].as_i64().unwrap();
    assert_ne!(child, parent, "a new document");
    assert_eq!(v["parent_sha256"], parent_sha);
    assert_eq!(doc_sha(&t, parent), parent_sha, "the parent row is unchanged");
    assert_eq!(v["checks"].as_array().unwrap().len(), 14, "the quality gate ran on the new version");
    let flag: i64 = t.app.db().query_row("SELECT cad_edited FROM documents WHERE id = ?", [child], |r| r.get(0)).unwrap();
    assert_eq!(flag, 1, "the child is flagged");
    let pflag: i64 = t.app.db().query_row("SELECT cad_edited FROM documents WHERE id = ?", [parent], |r| r.get(0)).unwrap();
    assert_eq!(pflag, 0, "the parent is not flagged");
    let vers = t.get(&format!("/api/cad/documents/{parent}/versions")).await;
    assert_eq!(vers["children"][0]["document_id"], child);
}

#[tokio::test]
async fn agent_identity_gets_403_on_edit_and_the_refusal_is_audited() {
    let t = boot();
    upload_raw(&t, &t.cookie, "part.dxf", CAD_DXF.as_bytes().to_vec()).await;
    let parent = last_doc(&t);
    let bot = add_session(&t.app, "bot", "engineer");
    t.app.db().execute("UPDATE users SET kind = 'agent' WHERE username = 'bot'", []).unwrap();
    let (st, _, _) = t.send("POST", &format!("/api/cad/documents/{parent}/edit"), Some(move_op()), &[("cookie", &bot)]).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let n: i64 = t.app.db().query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "no version was written");
    let audited: i64 = t.app.db().query_row("SELECT COUNT(*) FROM audit WHERE action = 'cad_edit_refused'", [], |r| r.get(0)).unwrap();
    assert_eq!(audited, 1);
    // the agent may still read the versions and is refused on undo too
    let (st, _, _) = t.send("POST", &format!("/api/cad/documents/{parent}/undo"), None, &[("cookie", &bot)]).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn undo_reverts_to_the_parent_and_keeps_the_edit() {
    let t = boot();
    upload_raw(&t, &t.cookie, "part.dxf", CAD_DXF.as_bytes().to_vec()).await;
    let parent = last_doc(&t);
    let (_, v) = t.call("POST", &format!("/api/cad/documents/{parent}/edit"), Some(move_op())).await;
    let child = v["document_id"].as_i64().unwrap();
    let (st, u) = t.call("POST", &format!("/api/cad/documents/{child}/undo"), None).await;
    assert_eq!(st, StatusCode::OK, "{u}");
    assert_eq!(u["revert_to"], parent);
    assert!(doc_sha(&t, child).len() == 64, "the edited version still exists");
    let (st, u) = t.call("POST", &format!("/api/cad/documents/{parent}/undo"), None).await;
    assert_eq!(st, StatusCode::CONFLICT, "an original has nothing to undo: {u}");
}

#[tokio::test]
async fn bad_edits_are_refused_and_an_edited_upload_is_marked() {
    let t = boot();
    upload_raw(&t, &t.cookie, "part.dxf", CAD_DXF.as_bytes().to_vec()).await;
    let parent = last_doc(&t);
    let (st, v) = t.call("POST", &format!("/api/cad/documents/{parent}/edit"), Some(move_op())).await;
    assert_eq!(st, StatusCode::OK, "a move of a line is fine: {v}");
    let too_many = json!({ "ops": vec![json!({ "op": "delete", "ids": ["E0"] }); 51] });
    let (st, _) = t.call("POST", &format!("/api/cad/documents/{parent}/edit"), Some(too_many)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "more than 50 operations");
    let (st, v) = t.call("POST", &format!("/api/cad/documents/{parent}/edit"), Some(json!({ "ops": [{ "op": "delete", "ids": ["E9"] }] }))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["code"], "unknown_entity");
    // an edited file re-uploaded keeps its mark (no laundering through download and upload)
    let (_, v) = t.call("POST", &format!("/api/cad/documents/{parent}/edit"), Some(move_op())).await;
    let child = v["document_id"].as_i64().unwrap();
    let edited = t.app.db().query_row("SELECT filename FROM documents WHERE id = ?", [child], |r| r.get::<_, String>(0)).unwrap();
    assert!(edited.ends_with("(NF CAD edit).dxf"));
    let _ = edited;
    let text = {
        let blob = nf_cad::edit::apply(CAD_DXF.as_bytes(), &[nf_cad::edit::Op::Move { ids: vec!["E0".into()], dx: 1.0, dy: 0.0 }]).unwrap();
        blob
    };
    assert_eq!(upload_raw(&t, &t.cookie, "again.dxf", text).await, StatusCode::OK);
    let re = last_doc(&t);
    let flag: i64 = t.app.db().query_row("SELECT cad_edited FROM documents WHERE id = ?", [re], |r| r.get(0)).unwrap();
    assert_eq!(flag, 1, "a marked upload is flagged");
}

#[test]
fn an_edited_file_is_never_served_as_a_certified_pattern() {
    let marked = nf_cad::edit::apply(CAD_DXF.as_bytes(), &[nf_cad::edit::Op::Move { ids: vec!["E0".into()], dx: 1.0, dy: 0.0 }]).unwrap();
    assert!(crate::cad::refuse_if_edited(&marked).is_err());
    assert!(crate::cad::refuse_if_edited(CAD_DXF.as_bytes()).is_ok());
    // the edit route is a licence-gated write
    assert!(crate::licence::gated(&axum::http::Method::POST, "/api/cad/documents/3/edit"));
    assert!(crate::licence::gated(&axum::http::Method::POST, "/api/cad/documents/3/undo"));
    assert!(!crate::licence::gated(&axum::http::Method::GET, "/api/cad/documents/3/versions"));
}

#[tokio::test]
async fn cad_versions_cannot_be_updated_or_deleted_and_the_helper_sees_both_sides() {
    let t = boot();
    upload_raw(&t, &t.cookie, "part.dxf", CAD_DXF.as_bytes().to_vec()).await;
    let parent = last_doc(&t);
    let (st, v) = t.call("POST", &format!("/api/cad/documents/{parent}/edit"), Some(move_op())).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let child = v["document_id"].as_i64().unwrap();
    let db = t.app.db();
    assert!(db.execute("UPDATE cad_versions SET author = 'x'", []).is_err(), "update refused");
    assert!(db.execute("DELETE FROM cad_versions", []).is_err(), "delete refused");
    assert!(crate::cad::is_cad_edited(&db, child), "child is an edit");
    assert!(!crate::cad::is_cad_edited(&db, parent), "the original stays certifiable");
    let psha: String = db.query_row("SELECT sha256 FROM documents WHERE id = ?", [parent], |r| r.get(0)).unwrap();
    let csha: String = db.query_row("SELECT sha256 FROM documents WHERE id = ?", [child], |r| r.get(0)).unwrap();
    assert!(crate::cad::refuse_if_edited_sha(&db, &csha).is_err(), "the edited bytes are refused by hash");
    assert!(crate::cad::refuse_if_edited_sha(&db, &psha).is_ok(), "the original bytes stay certifiable");
    assert!(!crate::cad::is_cad_edited(&db, 999), "unknown document is not edited");
}
