//! End-to-end: starts the real `nf-server` in desktop mode on a loopback port of 8825-8829, creates the first
//! administrator and a project through the API, then drives the bridge binary over stdin and stdout.
//! Needs the release server: `cargo build --release -p nf-server -p nf-pdf` (or set NF_SERVER_BIN).
//! Run with: `timeout 900 nf-sandbox cargo test -p nf-amoeba-agent --locked --test session`.

use nf_amoeba_agent::http::{Reply, Request, Tcp, Transport};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

static SESSION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const ADMIN_PASS: &str = "amb5t-admin-pass-2026";
const AGENT_PASS: &str = "amb5t-agent-pass-2026";

fn server_bin() -> PathBuf {
    if let Some(p) = std::env::var_os("NF_SERVER_BIN") {
        return PathBuf::from(p);
    }
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/release/nf-server");
    assert!(
        p.is_file(),
        "release server missing at {}: build it with cargo build --release -p nf-server -p nf-pdf",
        p.display()
    );
    p
}

fn free_port() -> u16 {
    (8825..=8829)
        .find(|p| TcpListener::bind(("127.0.0.1", *p)).is_ok())
        .expect("no free port in 8825-8829")
}

struct Server {
    child: Child,
    port: u16,
    token: String,
    data: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        // stop by exact PID: SIGTERM, then SIGKILL if it does not stop
        let pid = self.child.id().to_string();
        let _ = Command::new("kill").args(["-TERM", &pid]).status();
        for _ in 0..100 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = Command::new("kill").args(["-KILL", &pid]).status();
            let _ = self.child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.data);
    }
}

fn start_server(tag: &str) -> Server {
    let port = free_port();
    let data = std::env::temp_dir().join(format!("nf-amb5t-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).unwrap();
    let mut child = Command::new(server_bin())
        .args(["--desktop", "--port", &port.to_string(), "--no-browser"])
        .env("XDG_DATA_HOME", &data)
        .env("HOME", &data)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start nf-server");
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    // desktop mode moves to another port when the preferred one is taken: use the port it reports
    let mut actual_port = None;
    let token = loop {
        let line = rx
            .recv_timeout(Duration::from_secs(60))
            .expect("server printed its setup link");
        if let Some(rest) = line.strip_prefix("Address: http://127.0.0.1:") {
            actual_port = rest.split('/').next().and_then(|p| p.parse::<u16>().ok());
        }
        if let Some(i) = line.find("#setup=") {
            break line[i + 7..].trim().to_string();
        }
    };
    let port = actual_port.expect("server printed its address");
    assert_eq!(token.len(), 64, "setup token is 64 hex characters");
    let s = Server {
        child,
        port,
        token,
        data,
    };
    // wait until it answers
    for _ in 0..100 {
        if tcp(&s)
            .send(&get("/api/health", vec![]))
            .map(|r| r.status == 200)
            .unwrap_or(false)
        {
            return s;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("server did not answer on port {}", s.port);
}

fn tcp(s: &Server) -> Tcp {
    Tcp {
        host: "127.0.0.1".into(),
        port: s.port,
    }
}

fn get(path: &str, headers: Vec<(String, String)>) -> Request {
    Request {
        method: "GET",
        path: path.into(),
        headers,
        body: Vec::new(),
    }
}

fn post(path: &str, headers: Vec<(String, String)>, body: &Value) -> Request {
    let mut h = headers;
    h.push(("Content-Type".into(), "application/json".into()));
    Request {
        method: "POST",
        path: path.into(),
        headers: h,
        body: body.to_string().into_bytes(),
    }
}

fn cookie_of(r: &Reply) -> String {
    r.set_cookie.clone().expect("a session cookie")
}

/// Setup (first administrator), an engineer account for the bridge, and one project. Returns the project id.
fn prepare(s: &Server) -> i64 {
    let mut t = tcp(s);
    let setup = post(
        "/api/setup",
        vec![("X-NF-Setup-Token".into(), s.token.clone())],
        &json!({ "username": "admin", "password": ADMIN_PASS }),
    );
    assert_eq!(t.send(&setup).unwrap().status, 200, "setup");
    let login = |t: &mut Tcp, u: &str, p: &str| {
        let r = t
            .send(&post(
                "/api/login",
                vec![],
                &json!({ "username": u, "password": p }),
            ))
            .unwrap();
        assert_eq!(r.status, 200, "login {u}");
        cookie_of(&r)
    };
    let admin = login(&mut t, "admin", ADMIN_PASS);
    let mk = t
        .send(&post(
            "/api/users",
            vec![("Cookie".into(), admin.clone())],
            &json!({ "username": "agent", "password": AGENT_PASS, "role": "engineer" }),
        ))
        .unwrap();
    assert_eq!(mk.status, 200, "engineer account");
    // the engineer creates the project, so the engineer is its member and can see it
    let agent = login(&mut t, "agent", AGENT_PASS);
    let p = t
        .send(&post(
            "/api/projects",
            vec![("Cookie".into(), agent)],
            &json!({ "name": "AMB-5T bridge session", "description": "integration test" }),
        ))
        .unwrap();
    assert_eq!(p.status, 200, "project");
    serde_json::from_slice::<Value>(&p.body).unwrap()["id"]
        .as_i64()
        .unwrap()
}

fn step(b: &mut Bridge, out: &mut Vec<Value>, tool: &str, args: Value) -> Value {
    let a = b.call(tool, args);
    out.push(a.clone());
    a
}

/// One bridge process: sends a request line, reads the answer line.
struct Bridge {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

fn read_stderr(child: &mut Child) -> String {
    let mut e = String::new();
    if let Some(mut s) = child.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut s, &mut e);
    }
    e
}

/// The documents directory of a test server: a copy of the fixture PDF, the only file uploads may name.
fn docs_dir(s: &Server) -> PathBuf {
    let docs = s.data.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::copy(pdf_fixture(), docs.join("reportlab_lines.pdf")).unwrap();
    docs
}

impl Bridge {
    fn start(s: &Server, steps: &Path, docs: &Path) -> Bridge {
        Self::start_as(s, steps, docs, "agent", AGENT_PASS)
    }

    fn start_as(s: &Server, steps: &Path, docs: &Path, user: &str, pass: &str) -> Bridge {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nf-amoeba-tools"))
            .args(["--base", &format!("http://127.0.0.1:{}", s.port), "--steps"])
            .arg(steps)
            .arg("--docs-dir")
            .arg(docs)
            .env("NF_AGENT_USER", user)
            .env("NF_AGENT_PASS", pass)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Bridge {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({ "id": id, "tool": tool, "args": args })
        )
        .unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line).unwrap();
        if n == 0 {
            panic!(
                "bridge closed its output on {tool}: {}",
                read_stderr(&mut self.child)
            );
        }
        let v: Value = serde_json::from_str(&line).expect("answer is JSON");
        assert_eq!(v["id"], id);
        v
    }

    fn finish(mut self) {
        drop(self.stdin);
        let st = self.child.wait().unwrap();
        assert!(st.success(), "bridge exited with {st}");
    }
}

/// The scripted session. Returns the answers in order, the steps file lines, and the running server.
fn run_session(tag: &str) -> (Vec<Value>, Vec<Value>, Server) {
    // one session at a time: the servers share the port range 8825-8829
    let _one_at_a_time = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let s = start_server(tag);
    let project = prepare(&s);
    let steps = s.data.join("steps.jsonl");
    let docs = docs_dir(&s);
    let mut b = Bridge::start(&s, &steps, &docs);
    let mut out = Vec::new();
    let ok = step;
    let list = ok(&mut b, &mut out, "list_projects", json!({}));
    assert!(
        list["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == project)
    );
    ok(
        &mut b,
        &mut out,
        "get_project",
        json!({ "project": project }),
    );

    let cone = ok(
        &mut b,
        &mut out,
        "calculate",
        json!({ "project": project, "family": "cone_frustum", "params": { "r1": 200, "r2": 100, "h": 300 } }),
    );
    let c_cone = cone["result"]["id"].as_i64().expect("cone job id");
    let ecc = ok(
        &mut b,
        &mut out,
        "calculate",
        json!({ "project": project, "family": "eccentric_reducer", "params": { "r1": 200, "r2": 100, "h": 300, "e": 100 } }),
    );
    let c_ecc = ecc["result"]["id"].as_i64().expect("eccentric job id");
    let cyl = ok(
        &mut b,
        &mut out,
        "calculate",
        json!({ "project": project, "family": "cylinder_shell", "params": { "r_dev": 500, "length": 1000 } }),
    );
    assert!(cyl["ok"] == true, "cylinder job: {cyl}");
    let br = ok(
        &mut b,
        &mut out,
        "calculate",
        json!({ "project": project, "family": "branch_stub", "params": { "header_radius": 150, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": 400, "surface_reference": "outside" } }),
    );
    assert!(br["ok"] == true, "branch job: {br}");
    let hwb = ok(
        &mut b,
        &mut out,
        "calculate",
        json!({ "project": project, "family": "header_with_branch", "params": { "header_radius": 150, "header_dev_radius": 150, "header_length": 1200, "branch_station": 600, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": 400, "surface_reference": "outside" } }),
    );
    let c_hwb = hwb["result"]["id"].as_i64().expect("header job id");

    // the replay audit (gate G19) must run before a part can be measured
    ok(&mut b, &mut out, "replay", json!({ "job": c_cone }));
    ok(&mut b, &mut out, "get_job", json!({ "job": c_cone }));
    ok(&mut b, &mut out, "get_amoeba", json!({ "job": c_cone }));
    ok(
        &mut b,
        &mut out,
        "get_dependencies",
        json!({ "job": c_hwb }),
    );
    ok(
        &mut b,
        &mut out,
        "get_dxf_digest",
        json!({ "job": c_cone, "part": "reducer" }),
    );
    ok(
        &mut b,
        &mut out,
        "get_dxf_digest",
        json!({ "job": c_ecc, "part": "reducer" }),
    );

    let up = ok(
        &mut b,
        &mut out,
        "upload_document",
        json!({ "project": project, "path": docs.join("reportlab_lines.pdf").display().to_string() }),
    );
    let doc = up["result"]["document"].as_i64().expect("document id");
    let conv = ok(
        &mut b,
        &mut out,
        "pdf_to_dxf",
        json!({ "document": doc, "scale_n": 1, "options": {} }),
    );
    let conv_id = conv["result"]["conversion"]["id"].as_i64().unwrap_or(0);
    ok(
        &mut b,
        &mut out,
        "get_conversion",
        json!({ "conversion": conv_id }),
    );
    ok(&mut b, &mut out, "list_obligations", json!({}));
    ok(&mut b, &mut out, "list_lessons", json!({}));
    ok(&mut b, &mut out, "amoeba_summary", json!({}));
    // recompute writes: it supersedes the job (the held-part job c_hwb is not used after this)
    let re = ok(&mut b, &mut out, "recompute", json!({ "job": c_hwb }));
    assert_eq!(re["ok"], true, "recompute: {re}");
    ok(
        &mut b,
        &mut out,
        "propose_read",
        json!({ "document": doc, "page": 1, "box": [0, 0, 10, 10], "value": 150, "unit": "mm", "kind": "header_radius" }),
    );
    // refused without any call: the calculate parameters may not repeat the family
    ok(
        &mut b,
        &mut out,
        "calculate",
        json!({ "project": project, "family": "cone_frustum", "params": { "family": "cone_frustum" } }),
    );
    ok(&mut b, &mut out, "setup", json!({}));
    b.finish();
    let text = std::fs::read_to_string(&steps).unwrap();
    // NF_AMB5T_EVIDENCE=<dir>: keep a copy of the steps file of this run as evidence
    if let Some(dir) = std::env::var_os("NF_AMB5T_EVIDENCE") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(PathBuf::from(dir).join(format!("steps-{tag}.jsonl")), &text);
    }
    let lines = text
        .lines()
        .map(|l| serde_json::from_str(l).expect("each step is JSON"))
        .collect();
    (out, lines, s)
}

fn pdf_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../nf-pdf/tests/fixtures/reportlab_lines.pdf")
}

#[test]
fn scripted_session_against_the_real_server() {
    let (answers, lines, _s) = run_session("a");
    let by_tool = |i: usize| &answers[i];
    // 0 list_projects, 1 get_project, 2-6 calculations, 7 replay, 8 get_job, 9 get_amoeba, 10 get_dependencies
    assert_eq!(by_tool(2)["ok"], true);
    assert_eq!(
        by_tool(8)["result"]["state"],
        "REVIEW_REQUIRED",
        "a computed job is never approved"
    );
    assert_eq!(by_tool(10)["ok"], true);

    // a passing part is measured; the server's bytes are hashed, never returned
    let passing = by_tool(11);
    assert_eq!(passing["result"]["held"], false, "{passing}");
    assert_eq!(passing["result"]["sha256"].as_str().map(str::len), Some(64));
    assert!(passing["result"].get("bytes").is_some());

    // a held part returns the server's hold reasons unchanged and no file content
    let held = by_tool(12);
    assert_eq!(held["result"]["held"], true, "{held}");
    assert!(
        held["result"].get("sha256").is_none(),
        "no hash for a held part"
    );
    assert!(
        held["result"]["hold"]["code"].is_string(),
        "the server's hold code is passed on: {held}"
    );
    assert!(held["result"]["hold"]["checks"].is_array());

    // the PDF path
    assert_eq!(by_tool(13)["ok"], true, "upload: {}", by_tool(13));
    assert!(by_tool(13)["result"]["document"].is_i64());
    assert_eq!(by_tool(14)["ok"], true, "pdf_to_dxf: {}", by_tool(14));
    assert_eq!(by_tool(15)["ok"], true, "get_conversion: {}", by_tool(15));

    // the refusals: no HTTP, tool not available
    let n = answers.len();
    assert_eq!(answers[n - 2]["ok"], false);
    assert!(answers[n - 2]["error"].as_str().unwrap().contains("family"));
    assert_eq!(answers[n - 1]["error"], "tool not available");

    // one steps line per call, plus the sign-in
    assert_eq!(lines.len(), answers.len() + 1);
    assert_eq!(lines[0]["tool"], "login");
    assert_eq!(lines[0]["args"]["password"], "[redacted]");
    assert!(
        lines
            .iter()
            .any(|l| l["tool"] == "propose_read" && l["status"] == "CANDIDATE")
    );
    assert!(
        lines
            .iter()
            .any(|l| l["tool"] == "setup" && l["status"] == "refused")
    );
}

#[test]
fn result_hashes_reproduce_on_a_second_run() {
    let (ra, first, _s1) = run_session("b");
    let (rb, second, _s2) = run_session("c");
    assert_eq!(first.len(), second.len());
    let mut compared = 0;
    for (i, (a, b)) in first.iter().zip(&second).enumerate() {
        let (x, y) = (&a["result_sha256"], &b["result_sha256"]);
        if x.is_string() {
            assert_eq!(
                x,
                y,
                "result hash differs for {} (step {}): {} vs {}",
                a["tool"],
                a["step"],
                ra[i - 1].to_string().chars().take(600).collect::<String>(),
                rb[i - 1].to_string().chars().take(600).collect::<String>()
            );
            compared += 1;
        }
    }
    assert!(compared >= 10, "compared {compared} results");
}

#[test]
fn the_bridge_refuses_to_start_for_an_administrator() {
    let _one_at_a_time = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let s = start_server("admin");
    prepare(&s);
    let docs = docs_dir(&s);
    let steps = s.data.join("steps-admin.jsonl");
    let child = Command::new(env!("CARGO_BIN_EXE_nf-amoeba-tools"))
        .args(["--base", &format!("http://127.0.0.1:{}", s.port), "--steps"])
        .arg(&steps)
        .arg("--docs-dir")
        .arg(&docs)
        .env("NF_AGENT_USER", "admin")
        .env("NF_AGENT_PASS", ADMIN_PASS)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        !out.status.success(),
        "an administrator must not drive the bridge"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("administrator"), "{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn uploads_are_limited_to_the_documents_directory() {
    let _one_at_a_time = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let s = start_server("docs");
    let project = prepare(&s);
    let docs = docs_dir(&s);
    let steps = s.data.join("steps-docs.jsonl");
    let mut b = Bridge::start(&s, &steps, &docs);
    let mut out = Vec::new();
    let inside = step(
        &mut b,
        &mut out,
        "upload_document",
        json!({ "project": project, "path": docs.join("reportlab_lines.pdf").display().to_string() }),
    );
    assert_eq!(inside["ok"], true, "a file inside the directory: {inside}");

    // an existing file outside the directory, and a link inside that points outside, are refused
    let outside = pdf_fixture().display().to_string();
    std::os::unix::fs::symlink(pdf_fixture(), docs.join("link.pdf")).unwrap();
    let refused_outside = step(
        &mut b,
        &mut out,
        "upload_document",
        json!({ "project": project, "path": outside }),
    );
    let refused_link = step(
        &mut b,
        &mut out,
        "upload_document",
        json!({ "project": project, "path": docs.join("link.pdf").display().to_string() }),
    );
    // a missing file gets the same answer as a file outside: no existence oracle
    let missing_outside = step(
        &mut b,
        &mut out,
        "upload_document",
        json!({ "project": project, "path": "/nonexistent-amb5t/x.pdf" }),
    );
    let missing_inside = step(
        &mut b,
        &mut out,
        "upload_document",
        json!({ "project": project, "path": "nope.pdf" }),
    );
    // a PDF inside the directory over 100 MB is refused before it is read
    let big = docs.join("big.pdf");
    std::fs::write(&big, b"%PDF-1.4\n").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&big)
        .unwrap()
        .set_len(100 * 1024 * 1024 + 1)
        .unwrap();
    let too_big = step(
        &mut b,
        &mut out,
        "upload_document",
        json!({ "project": project, "path": big.display().to_string() }),
    );
    b.finish();

    for a in [
        &refused_outside,
        &refused_link,
        &missing_outside,
        &missing_inside,
    ] {
        assert_eq!(a["ok"], false, "{a}");
        assert_eq!(
            a["error"], refused_outside["error"],
            "same answer for all: {a}"
        );
    }
    assert_eq!(too_big["ok"], false, "{too_big}");
    assert!(
        too_big["error"].as_str().unwrap().contains("100 MB"),
        "{too_big}"
    );
    // a refused upload sends no HTTP request at all
    let text = std::fs::read_to_string(&steps).unwrap();
    for l in text.lines() {
        let rec: Value = serde_json::from_str(l).unwrap();
        if rec["tool"] == "upload_document" && rec["status"] == "error" {
            assert_eq!(
                rec["http_status"],
                json!([]),
                "no HTTP for a refused upload: {rec}"
            );
        }
    }
}
