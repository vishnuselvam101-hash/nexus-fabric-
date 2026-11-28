//! AIOP-1 acceptance: the operator works synthetic orders (generator pages with known truth) through a real
//! `nf-server` in desktop mode on a loopback port of 8796-8829.
//!
//! - `operator_pipeline_with_exact_readings`: the readings come from the truth boxes (no models). Fast.
//! - `acceptance_with_the_number_reader` (ignored; run with --ignored): the real reader (NF_NUMREAD_MODELS)
//!   on the PNG pages. Set NF_ACCEPT_PER_FAMILY (default 4) and NF_ACCEPT_OUT (a JSON result file).
//!
//! Needs a built server: `cargo build -p nf-server` (or NF_SERVER_BIN). Pages: NF_SYNTH_DIR (default
//! /root/nf-data/reader-synth/v1b/test).

use nf_amoeba_agent::agent::Agent;
use nf_amoeba_agent::http::{Reply, Request, Tcp, Transport};
use nf_amoeba_agent::steps::StepLog;
use nf_amoeba_operator::eyes::{Eyes, FixedEyes, NumreadEyes};
use nf_amoeba_operator::operator::{Manifest, Registry, RulePlanner, UnitSpec, run};
use nf_amoeba_operator::replay::replay;
use nf_core::assign::Reading;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
const ADMIN_PASS: &str = "aiop1-admin-pass-2026";
const AGENT_PASS: &str = "aiop1-agent-pass-2026";

fn server_bin() -> PathBuf {
    if let Some(p) = std::env::var_os("NF_SERVER_BIN") {
        return PathBuf::from(p);
    }
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/nf-server");
    assert!(p.is_file(), "server missing at {}: cargo build -p nf-server", p.display());
    p
}

struct Server {
    child: Child,
    port: u16,
    data: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
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
    let port = (8796..=8820).find(|p| TcpListener::bind(("127.0.0.1", *p)).is_ok()).expect("free port");
    let data = std::env::temp_dir().join(format!("nf-aiop-{tag}-{}", std::process::id()));
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
    let (mut actual, mut token) = (None, String::new());
    while token.is_empty() {
        let line = rx.recv_timeout(Duration::from_secs(60)).expect("setup link");
        if let Some(rest) = line.strip_prefix("Address: http://127.0.0.1:") {
            actual = rest.split('/').next().and_then(|p| p.parse::<u16>().ok());
        }
        if let Some(i) = line.find("#setup=") {
            token = line[i + 7..].trim().to_string();
        }
    }
    let s = Server { child, port: actual.expect("address"), data };
    std::fs::write(s.data.join("token"), token).unwrap();
    for _ in 0..100 {
        if Http::new(&s, "").get("/api/health").0 == 200 {
            return s;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("server did not answer");
}

struct Http {
    t: Tcp,
    cookie: String,
}

impl Http {
    fn new(s: &Server, cookie: &str) -> Http {
        Http { t: Tcp { host: "127.0.0.1".into(), port: s.port }, cookie: cookie.into() }
    }
    fn send(&mut self, method: &'static str, path: &str, body: Vec<u8>, mut h: Vec<(String, String)>) -> Reply {
        if !self.cookie.is_empty() {
            h.push(("Cookie".into(), self.cookie.clone()));
        }
        self.t.send(&Request { method, path: path.into(), headers: h, body }).unwrap()
    }
    fn get(&mut self, path: &str) -> (u16, Value) {
        let r = self.send("GET", path, vec![], vec![]);
        (r.status, serde_json::from_slice(&r.body).unwrap_or(Value::Null))
    }
    fn json(&mut self, method: &'static str, path: &str, body: &Value, extra: Vec<(String, String)>) -> (u16, Value, Reply) {
        let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
        h.extend(extra);
        let r = self.send(method, path, body.to_string().into_bytes(), h);
        (r.status, serde_json::from_slice(&r.body).unwrap_or(Value::Null), r)
    }
}

fn login(s: &Server, u: &str, p: &str) -> String {
    let (st, _, r) = Http::new(s, "").json("POST", "/api/login", &json!({ "username": u, "password": p }), vec![]);
    assert_eq!(st, 200, "login {u}");
    r.set_cookie.expect("cookie")
}

// ---------------------------------------------------------------- the synthetic pages

fn synth_dir() -> PathBuf {
    std::env::var_os("NF_SYNTH_DIR").map_or_else(|| PathBuf::from("/root/nf-data/reader-synth/v1b/test"), PathBuf::from)
}

struct Page {
    png: Vec<u8>,
    truth: Value,
}

fn load_pages(per_family: usize) -> Vec<Page> {
    let dir = synth_dir();
    let text = std::fs::read_to_string(dir.join("truth.jsonl")).expect("truth.jsonl");
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    let mut pages = Vec::new();
    for line in text.lines() {
        let t: Value = serde_json::from_str(line).unwrap();
        let fam = t["family"].as_str().unwrap().to_string();
        let c = count.entry(fam).or_default();
        if *c >= per_family {
            continue;
        }
        *c += 1;
        let png = std::fs::read(dir.join("png").join(format!("{}.png", t["page_id"].as_str().unwrap()))).expect("png");
        pages.push(Page { png, truth: t });
    }
    pages
}

/// BOM part names in row order, and the exact readings of a page taken from the truth boxes.
fn parts_of(t: &Value) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for i in t["items"].as_array().unwrap() {
        if i["type"] == "bom" && i["source"]["kind"] == "bbox" {
            let p = i["part"].as_str().unwrap().to_string();
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v
}

fn exact_readings(t: &Value) -> Vec<Reading> {
    let mut out = Vec::new();
    for i in t["items"].as_array().unwrap() {
        if !matches!(i["type"].as_str(), Some("dimension" | "bom")) {
            continue;
        }
        let text = i["text"].as_str().unwrap();
        let Some(v) = i["value"].as_f64() else { continue };
        let b = &i["box"];
        let bb: [f32; 4] = std::array::from_fn(|k| b[k].as_f64().unwrap() as f32);
        // BOM cells: the quantity and the item number are numbers too, in the same row
        out.push(Reading { id: out.len(), text: text.to_string(), value: v, page: 1, bbox: bb, confidence: 0.99, flags: vec![], in_table: false });
    }
    // the item number and quantity of each list row (the reader finds them; the truth lists them as structure)
    for i in t["items"].as_array().unwrap() {
        if i["type"] == "bom" && matches!(i["source"]["what"].as_str(), Some("item number" | "quantity")) {
            let b = &i["box"];
            let bb: [f32; 4] = std::array::from_fn(|k| b[k].as_f64().unwrap() as f32);
            let text = i["text"].as_str().unwrap();
            out.push(Reading { id: out.len(), text: text.into(), value: text.parse().unwrap_or(1.0), page: 1, bbox: bb, confidence: 0.99, flags: vec![], in_table: false });
        }
    }
    out
}

// ---------------------------------------------------------------- the run

struct Setup {
    server: Server,
    oid: i64,
    project: i64,
    admin: String,
    agent: String,
}

fn prepare(pages: &[Page], tag: &str) -> Setup {
    let server = start_server(tag);
    let token = std::fs::read_to_string(server.data.join("token")).unwrap();
    let (st, ..) = Http::new(&server, "").json("POST", "/api/setup", &json!({ "username": "admin", "password": ADMIN_PASS }), vec![("X-NF-Setup-Token".into(), token)]);
    assert_eq!(st, 200, "setup");
    let admin = login(&server, "admin", ADMIN_PASS);
    let mut h = Http::new(&server, &admin);
    let (st, v, _) = h.json("POST", "/api/users", &json!({ "username": "operator-ai", "password": AGENT_PASS, "role": "engineer", "kind": "agent" }), vec![]);
    assert_eq!(st, 200, "agent account {v}");
    let (st, p, _) = h.json("POST", "/api/projects", &json!({ "name": "AIOP-1 acceptance", "description": "synthetic" }), vec![]);
    assert_eq!(st, 200);
    let project = p["id"].as_i64().unwrap();
    let (st, ..) = h.json("POST", &format!("/api/projects/{project}/members"), &json!({ "username": "operator-ai" }), vec![]);
    assert_eq!(st, 200, "member");
    let (st, o, _) = h.json("POST", &format!("/api/projects/{project}/orders"), &json!({ "name": "synthetic order" }), vec![]);
    assert_eq!(st, 200, "order {o}");
    let oid = o["id"].as_i64().unwrap();
    for (i, pg) in pages.iter().enumerate() {
        let r = h.send("POST", &format!("/api/orders/{oid}/files"), pg.png.clone(), vec![("Content-Type".into(), "application/octet-stream".into()), ("X-Filename".into(), format!("page{i}.png"))]);
        assert_eq!(r.status, 200, "upload {}", String::from_utf8_lossy(&r.body));
    }
    let agent = login(&server, "operator-ai", AGENT_PASS);
    Setup { server, oid, project, admin, agent }
}

fn manifest_for(s: &Setup, pages: &[Page]) -> Manifest {
    let units = pages
        .iter()
        .enumerate()
        .map(|(i, p)| UnitSpec {
            id: format!("u{}", i + 1),
            no: format!("{}", i + 1),
            name: p.truth["request"]["family"].as_str().unwrap().to_string(),
            family: p.truth["family"].as_str().unwrap().to_string(),
            page: (i + 1) as i64,
            parts: parts_of(&p.truth),
        })
        .collect();
    Manifest { order: s.oid, project: s.project, units, order_compute: false }
}

fn operator_agent(s: &Setup) -> Agent<Tcp> {
    let log = StepLog::open(&s.server.data.join("steps.jsonl")).unwrap();
    let mut a = Agent::new(Tcp { host: "127.0.0.1".into(), port: s.server.port }, log);
    a.login("operator-ai", AGENT_PASS).unwrap();
    a
}

#[derive(Default, Debug)]
struct Score {
    slots: usize,
    confirmed_correct: usize,
    proposed_correct: usize,
    wrong_confirmed: usize,
    wrong_proposed: usize,
    unfilled: usize,
    /// slots of pages on which every number the family needs is printed, and how many of them are confirmed correctly
    complete_slots: usize,
    complete_correct: usize,
    questions: usize,
    held_with_dxf: usize,
    served: usize,
    held: usize,
    events: usize,
}

/// Compare the order's values with the truth, check holds and replay.
fn score(s: &Setup, pages: &[Page], rep: &nf_amoeba_operator::operator::RunReport) -> (Score, Value) {
    let mut h = Http::new(&s.server, &s.admin);
    let (_, order) = h.get(&format!("/api/orders/{}", s.oid));
    let mut sc = Score::default();
    let mut detail = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        let no = format!("{}", i + 1);
        let part = order["parts"].as_array().unwrap().iter().find(|x| x["no"] == no).unwrap();
        let vals = part["values"].as_array().cloned().unwrap_or_default();
        // slots = request fields that the page shows (a dimension item whose source names the field)
        let mut fields: BTreeMap<String, f64> = BTreeMap::new();
        for it in p.truth["items"].as_array().unwrap() {
            if let Some(f) = it["source"]["field"].as_str() {
                if let Some(v) = p.truth["request"][f].as_f64() {
                    fields.insert(f.to_string(), v);
                }
            }
        }
        let needed = nf_core::assign::family_unit("x", p.truth["family"].as_str().unwrap(), vec![]).unwrap();
        let complete = needed.slots.iter().all(|sl| fields.contains_key(&sl.key));
        for (f, want) in fields {
            sc.slots += 1;
            if complete {
                sc.complete_slots += 1;
            }
            let got = vals.iter().find(|v| v["key"] == f.as_str());
            let (state, value) = got.map_or(("none", None), |g| (g["state"].as_str().unwrap_or(""), g["value"].as_f64()));
            let right = value.is_some_and(|x| (x - want).abs() < 1e-6);
            match (state, right) {
                ("CONFIRMED", true) => {
                    sc.confirmed_correct += 1;
                    if complete {
                        sc.complete_correct += 1;
                    }
                }
                ("CONFIRMED", false) => sc.wrong_confirmed += 1,
                ("PROPOSED", true) => sc.proposed_correct += 1,
                ("PROPOSED", false) => sc.wrong_proposed += 1,
                _ => sc.unfilled += 1,
            }
            detail.push(json!({ "page": i + 1, "family": p.truth["family"], "key": f, "want": want, "got": value, "state": state }));
        }
    }
    // holds: a held part has no DXF (the server refuses with 409), a served part has one
    for u in &rep.units {
        for part in &u.held {
            let st = h.get(&format!("/api/calculations/{}/dxf/{part}", u.job.unwrap())).0;
            if st == 200 {
                sc.held_with_dxf += 1;
            }
            sc.held += 1;
        }
        for part in &u.dxf_served {
            assert_eq!(h.get(&format!("/api/calculations/{}/dxf/{part}", u.job.unwrap())).0, 200);
            sc.served += 1;
        }
    }
    // events, replay
    let (_, ev) = h.get(&format!("/api/orders/{}/operator/events?after=0&limit=500", s.oid));
    let events = ev["events"].as_array().cloned().unwrap_or_default();
    sc.events = events.len();
    sc.questions = events.iter().filter(|e| e["action"] == "ask").count();
    let r = replay(&events);
    // every typed value the stream replays equals what the order holds
    for ((pno, key), v) in &r.values {
        let part = order["parts"].as_array().unwrap().iter().find(|x| x["no"] == pno.as_str()).unwrap();
        let held = part["values"].as_array().unwrap().iter().find(|x| x["key"] == key.as_str()).unwrap();
        assert!((held["value"].as_f64().unwrap() - v).abs() < 1e-9, "replay differs at {pno}.{key}");
    }
    // and the order holds nothing the stream did not type
    let in_order: usize = order["parts"].as_array().unwrap().iter().map(|p| p["values"].as_array().map_or(0, Vec::len)).sum();
    assert_eq!(in_order, r.values.len(), "the stream must reproduce every value of the order");
    (sc, Value::Array(detail))
}

fn print_score(tag: &str, sc: &Score) {
    let pct = |n: usize| if sc.slots == 0 { 0.0 } else { 100.0 * n as f64 / sc.slots as f64 };
    println!(
        "AIOP1 {tag}: slots {} | filled correctly (confirmed) {} = {:.1}% | proposed correct {} | pages with every needed number printed: {}/{} slots correct | WRONG CONFIRMED {} | wrong proposed {} | unfilled {} | questions {} | events {} | jobs served {} held {} held-with-dxf {}",
        sc.slots, sc.confirmed_correct, pct(sc.confirmed_correct), sc.proposed_correct, sc.complete_correct, sc.complete_slots, sc.wrong_confirmed, sc.wrong_proposed, sc.unfilled, sc.questions, sc.events, sc.served, sc.held, sc.held_with_dxf
    );
}

fn run_all(tag: &str, per_family: usize, eyes: &mut dyn Eyes, pages: Vec<Page>) -> (Score, Value, Setup, nf_amoeba_operator::operator::RunReport) {
    let _lock = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let s = prepare(&pages, tag);
    let m = manifest_for(&s, &pages);
    let mut agent = operator_agent(&s);
    let mut brain = RulePlanner::new(&m);
    let rep = run(&mut agent, eyes, &mut brain, &Registry::order_tools(), &m).expect("run");
    assert!(rep.errors.is_empty(), "errors: {:?}", rep.errors);
    let (sc, detail) = score(&s, &pages, &rep);
    print_score(&format!("{tag} ({per_family}/family)"), &sc);
    for u in &rep.units {
        let q = u.assignment.as_ref().and_then(|a| a.question.as_ref()).map(|q| q.code);
        let tried = u.assignment.as_ref().map(|a| a.tried);
        println!("  unit {} {} page {}: question {:?} tried {:?} job {:?} {:?} held {:?} {}", u.spec.id, u.spec.family, u.spec.page, q, tried, u.job, u.job_state, u.held, u.assignment.as_ref().and_then(|a| a.question.as_ref()).map(|q| q.en.clone()).unwrap_or_default());
    }
    for d in detail.as_array().unwrap().iter().filter(|d| d["state"] != "CONFIRMED") {
        println!("  slot {d}");
    }
    (sc, detail, s, rep)
}

#[test]
fn operator_pipeline_with_exact_readings() {
    let pages = load_pages(2);
    let mut eyes = FixedEyes::default();
    for (i, p) in pages.iter().enumerate() {
        let w = p.truth["width"].as_f64().unwrap() as f32;
        let h = p.truth["height"].as_f64().unwrap() as f32;
        let mut r = exact_readings(&p.truth);
        for x in &mut r {
            x.page = (i + 1) as i64;
        }
        eyes.pages.insert((i + 1) as i64, (r, w, h));
    }
    let (sc, _, s, rep) = run_all("exact-readings", 2, &mut eyes, pages);
    assert_eq!(sc.wrong_confirmed, 0, "{sc:?}");
    assert!(sc.complete_correct * 10 >= sc.complete_slots * 9, "at least 90% of the slots of complete pages: {sc:?}");
    assert_eq!(sc.held_with_dxf, 0);
    assert!(sc.events > 0);
    // pause stops the operator; resume lets the same plan finish; undo puts a typed value back
    let mut h = Http::new(&s.server, &s.admin);
    let base = format!("/api/orders/{}/operator", s.oid);
    assert_eq!(h.json("POST", &format!("{base}/pause"), &json!({}), vec![]).0, 200);
    let mut agent = operator_agent(&s);
    let mut brain = RulePlanner::new(&manifest_for_paused(&s, &rep));
    let mut fresh = FixedEyes::default();
    let paused = run(&mut agent, &mut fresh, &mut brain, &Registry::order_tools(), &manifest_for_paused(&s, &rep)).unwrap();
    assert_eq!(paused.status, nf_amoeba_operator::operator::Status2::Paused);
    assert_eq!(paused.events, 0);
    assert_eq!(h.json("POST", &format!("{base}/resume"), &json!({}), vec![]).0, 200);
    let (st, u, _) = h.json("POST", &format!("{base}/undo"), &json!({ "note": "check" }), vec![]);
    assert_eq!(st, 200, "{u}");
}

fn manifest_for_paused(s: &Setup, rep: &nf_amoeba_operator::operator::RunReport) -> Manifest {
    Manifest { order: s.oid, project: s.project, units: rep.units.iter().take(1).map(|u| u.spec.clone()).collect(), order_compute: false }
}

#[test]
#[ignore = "needs the number reader models (NF_NUMREAD_MODELS) and takes minutes"]
fn acceptance_with_the_number_reader() {
    let per: usize = std::env::var("NF_ACCEPT_PER_FAMILY").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let pages = load_pages(per);
    let mut eyes = NumreadEyes::from_dir(&nf_numread::default_model_dir()).expect("models");
    let (sc, detail, _s, rep) = run_all("number-reader", per, &mut eyes, pages);
    if let Some(out) = std::env::var_os("NF_ACCEPT_OUT") {
        let units: Vec<Value> = rep.units.iter().map(|u| json!({ "unit": u.spec.id, "family": u.spec.family, "job": u.job, "state": u.job_state, "held": u.held, "served": u.dxf_served, "question": u.assignment.as_ref().and_then(|a| a.question.as_ref()).map(|q| q.code), "tried": u.assignment.as_ref().map(|a| a.tried) })).collect();
        let doc = json!({ "score": format!("{sc:?}"), "slots": detail, "units": units });
        std::fs::write(out, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    }
    assert_eq!(sc.wrong_confirmed, 0, "{sc:?}");
    assert_eq!(sc.held_with_dxf, 0);
}
