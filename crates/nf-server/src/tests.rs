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
    let t = boot();
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
    let t = boot();
    let id = t.job(cone()).await;
    let d = t.get(&format!("/api/calculations/{id}")).await;
    let sha = d["patterns"][0]["dxf_sha256"].as_str().unwrap().to_string();
    let path = storage::blob_path(&t.dir, &sha);
    let blobs = || {
        std::fs::read_dir(t.dir.join("blobs"))
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
    assert_eq!(t.dxf_status(again, "reducer").await, StatusCode::OK);
    // damaged again, and this time only read: restored by recomputation with the identical hash
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
    assert_eq!(st, StatusCode::OK);
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
    assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(v["error"].as_str().unwrap().contains("invalid"));
}

fn nf_dxf_mark() -> &'static str {
    "NOT RELEASED - DEVELOPMENT BUILD"
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
        ("crypto.rs", include_str!("crypto.rs")),
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
        assert_eq!(t.dxf_status(id, "reducer").await, StatusCode::OK, "{what}");
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
    // the same data directory, opened as the upgraded server would open it
    let app = build_app(config(&t.dir)).unwrap();
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
    let t = boot();
    let id = t.job(good_assembly()).await;
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
        (Some(true), Some(true), Some(6))
    );
    assert!(v["limitation"].as_str().unwrap().contains("rollback"));
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
        "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
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
        hash_slots: Semaphore::new(1),
        upload_slots: Semaphore::new(1),
        replay_slots: Semaphore::new(1),
        anon_audit: Mutex::new((0, 0, 0)),
        verified: Mutex::new((-1, Default::default())),
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
    let t = boot();
    let id = t.job(good_assembly()).await;
    assert!(nf_core::ENGINE_VERSION.starts_with("nf-engine 0.4.1+src."));
    t.sql(&format!("UPDATE calculations SET result_json = json_set(result_json, '$.engine', 'nf-engine 0.2.0+src.0123456789abcdef') WHERE id = {id}"));
    let d = t.get(&format!("/api/calculations/{id}")).await;
    assert_eq!(d["state"], "REJECTED");
    assert!(gate_of(&d, "G21")["detail"].as_str().unwrap().contains(
        "Computed by a superseded engine version (nf-engine 0.2.0+src.0123456789abcdef) — recompute"
    ));
    assert_eq!(t.dxf_status(id, "branch").await, StatusCode::CONFLICT);
    // a current result states its marking side in the result, the gate table and the DXF
    let now = t.job(assembly("outside", 150.0, 150.0, 100.0, 100.0)).await;
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
    assert!(String::from_utf8(bytes).unwrap().contains("MARKING SIDE: PATTERN DRAWN AS SEEN FROM THE OUTSIDE OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED"));
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
            (StatusCode::OK, &json!({ "signed_in": false })),
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

/// BH-004 (engine 0.4.1): a result stored by 0.4.0 is marked for recomputation (gate G21), is never shown as current and
/// has no DXF; a result of 0.4.1 is current and its DXF is offered.
#[tokio::test]
async fn bh004_engine_0_4_0_results_are_recomputed_and_0_4_1_results_are_current() {
    let t = boot();
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
    assert_ne!(d["state"], "REJECTED", "a 0.4.1 result is current");
    assert_eq!(t.dxf_status(now, "branch").await, StatusCode::OK);
}
