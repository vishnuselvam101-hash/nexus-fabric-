//! NFJ-1 S1-L5a: table-test runner for the expected values taken from zumen-kirizu (ZK), tests/golden/zumen/*.json.
//! The engine under test sits behind `ZkEngine`, so this compiles before any ported shape exists. Until then the
//! real engine is `NotPorted` (every case is reported PENDING, never PASS). The self-test feeds ZK's own numbers
//! (`zk_got`) through the same comparison and must reproduce every extracted case; a second self-test perturbs
//! them and must fail every case, proving that the comparison can fail.
//! Tolerances are ZK's: length 0.1 mm, area/mass 0.05 % (stored per case as `tol`, test_build.mjs:10).

use serde_json::Value;
use std::path::PathBuf;

const ZK_COMMIT_PREFIX: &str = "f718c34";
/// ZK's own count: 1894 OK, 0 NG (node test_build.mjs at the commit above).
const ZK_TOTAL: usize = 1894;

#[derive(Debug, Clone)]
struct Case {
    id: String,
    name: String,
    source: String,
    kind: String,
    want: Value,
    tol: f64,
    zk_got: Value,
    readings: Vec<String>,
}

/// What the engine under test answers for one case.
#[derive(Debug, Clone, PartialEq)]
enum Got {
    Num(f64),
    Count(i64),
    /// check-text present, boolean condition, or "stops with this refusal kind and no NaN".
    Flag(bool),
}

/// The port under test. `None` = not implemented yet (reported PENDING).
trait ZkEngine {
    fn eval(&self, case: &Case) -> Option<Got>;
}

/// The real engine: no ported shape exists yet.
struct NotPorted;
impl ZkEngine for NotPorted {
    fn eval(&self, _case: &Case) -> Option<Got> {
        None
    }
}

/// Self-test engine: answers with the number ZK itself printed for the case.
struct ZkEcho;
impl ZkEngine for ZkEcho {
    fn eval(&self, c: &Case) -> Option<Got> {
        got_from_json(c, &c.zk_got)
    }
}

/// Deliberately wrong engine: every answer is outside the tolerance or flipped.
struct Perturbed;
impl ZkEngine for Perturbed {
    fn eval(&self, c: &Case) -> Option<Got> {
        Some(match got_from_json(c, &c.zk_got)? {
            Got::Num(x) => Got::Num(x + 2.0 * c.tol + 1.0),
            Got::Count(n) => Got::Count(n + 1),
            Got::Flag(b) => Got::Flag(!b),
        })
    }
}

fn got_from_json(c: &Case, v: &Value) -> Option<Got> {
    match c.kind.as_str() {
        "num" => v.as_f64().map(Got::Num),
        "count" => v.as_i64().map(Got::Count),
        _ => v.as_bool().map(Got::Flag),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Ok,
    Ng,
    Pending,
}

/// ZK's `eq`: got is present and |got - want| <= tol (a NaN never passes). Counts compare exactly; flags must be true.
fn judge(c: &Case, got: Option<Got>) -> Verdict {
    let Some(got) = got else { return Verdict::Pending };
    let ok = match (c.kind.as_str(), got) {
        ("num", Got::Num(g)) => c.want.as_f64().is_some_and(|w| g.is_finite() && (g - w).abs() <= c.tol),
        ("count", Got::Count(n)) => c.want.as_i64() == Some(n),
        ("num" | "count", _) => false,
        (_, Got::Flag(b)) => b,
        _ => false,
    };
    if ok { Verdict::Ok } else { Verdict::Ng }
}

#[derive(Default, Debug)]
struct Report {
    ok: usize,
    ng: usize,
    pending: usize,
    ng_lines: Vec<String>,
}

fn run_table(engine: &dyn ZkEngine, cases: &[Case]) -> Report {
    let mut r = Report::default();
    for c in cases {
        match judge(c, engine.eval(c)) {
            Verdict::Ok => r.ok += 1,
            Verdict::Pending => r.pending += 1,
            Verdict::Ng => {
                r.ng += 1;
                // ZK style: NG name got want
                r.ng_lines.push(format!("NG {} ({}) want {}  [{}]", c.name, c.id, c.want, c.source));
            }
        }
    }
    r
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/zumen")
}

fn load_cases() -> (Vec<Case>, usize) {
    let dir = golden_dir();
    let index: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).expect("index.json")).unwrap();
    assert!(index["zk_commit"].as_str().unwrap().starts_with(ZK_COMMIT_PREFIX), "golden files are not from ZK {ZK_COMMIT_PREFIX}");
    let excluded: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("excluded.json")).expect("excluded.json")).unwrap();
    let n_excluded = excluded["excluded"].as_array().unwrap().len();
    let mut cases = Vec::new();
    for s in index["sections"].as_array().unwrap() {
        let f = dir.join(s["file"].as_str().unwrap());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert!(v["zk_commit"].as_str().unwrap().starts_with(ZK_COMMIT_PREFIX), "{f:?}");
        let arr = v["cases"].as_array().unwrap();
        assert_eq!(arr.len() as u64, s["cases"].as_u64().unwrap(), "{f:?} case count");
        for c in arr {
            cases.push(Case {
                id: c["id"].as_str().unwrap().into(),
                name: c["name"].as_str().unwrap().into(),
                source: c["source"].as_str().unwrap().into(),
                kind: c["kind"].as_str().unwrap().into(),
                want: c["want"].clone(),
                tol: c["tol"].as_f64().unwrap_or(0.0),
                zk_got: c["zk_got"].clone(),
                readings: c["reading_hint"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect(),
            });
        }
    }
    (cases, n_excluded)
}

#[test]
fn golden_files_are_complete_and_pinned() {
    let (cases, excluded) = load_cases();
    // every ZK assert is either extracted or listed as excluded: nothing silently dropped
    assert_eq!(cases.len() + excluded, ZK_TOTAL, "extracted + excluded must equal ZK's 1894");
    assert!(cases.len() >= 1800, "extracted {} < 1800", cases.len());
    let mut ids: Vec<_> = cases.iter().map(|c| c.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), cases.len(), "case ids are unique");
    for c in &cases {
        assert!(c.source.starts_with("design-demos/assets/kirizu/test_build.mjs:"), "{} has no ZK source", c.id);
        assert!(!c.source.ends_with(":0"), "{} has no source line", c.id);
        for r in &c.readings {
            assert!(golden_dir().join("readings").join(r).exists(), "{}: reading {r} not copied", c.id);
        }
    }
    assert!(golden_dir().join("README.md").exists());
}

#[test]
fn comparison_uses_zk_tolerance_semantics() {
    let mk = |kind: &str, want: Value, tol: f64| Case { id: "t".into(), name: "t".into(), source: "x:1".into(), kind: kind.into(), want, tol, zk_got: Value::Null, readings: vec![] };
    let n = mk("num", serde_json::json!(100.0), 0.1);
    assert_eq!(judge(&n, Some(Got::Num(100.1))), Verdict::Ok);
    assert_eq!(judge(&n, Some(Got::Num(100.1001))), Verdict::Ng);
    assert_eq!(judge(&n, Some(Got::Num(f64::NAN))), Verdict::Ng);
    assert_eq!(judge(&n, Some(Got::Flag(true))), Verdict::Ng);
    assert_eq!(judge(&n, None), Verdict::Pending);
    let c = mk("count", serde_json::json!(0), 0.0);
    assert_eq!(judge(&c, Some(Got::Count(0))), Verdict::Ok);
    assert_eq!(judge(&c, Some(Got::Count(1))), Verdict::Ng);
    let b = mk("bool", serde_json::json!(true), 0.0);
    assert_eq!(judge(&b, Some(Got::Flag(true))), Verdict::Ok);
    assert_eq!(judge(&b, Some(Got::Flag(false))), Verdict::Ng);
}

/// Harness self-test: ZK's own numbers reproduce every extracted case.
#[test]
fn self_test_zk_numbers_pass() {
    let (cases, excluded) = load_cases();
    let r = run_table(&ZkEcho, &cases);
    println!("self-test: {} OK, {} NG, {} pending of {} extracted ({} excluded, ZK total {})", r.ok, r.ng, r.pending, cases.len(), excluded, ZK_TOTAL);
    for l in r.ng_lines.iter().take(20) {
        println!("{l}");
    }
    assert_eq!((r.ng, r.pending), (0, 0));
    assert_eq!(r.ok, cases.len());
}

/// The harness can fail: a wrong engine is caught on every case.
#[test]
fn self_test_perturbed_numbers_fail_every_case() {
    let (cases, _) = load_cases();
    let r = run_table(&Perturbed, &cases);
    assert_eq!(r.ok, 0, "a perturbed value passed");
    assert_eq!(r.ng, cases.len());
}

/// The real engine: nothing ported yet, so everything is PENDING (not counted as a pass). Replace `NotPorted`
/// with the ported engine per shape lane; a lane is done when its sections have 0 NG and 0 PENDING.
#[test]
fn real_engine_status() {
    let (cases, _) = load_cases();
    let r = run_table(&NotPorted, &cases);
    println!("real engine: {} OK, {} NG, {} PENDING of {} extracted", r.ok, r.ng, r.pending, cases.len());
    assert_eq!(r.ng, 0);
    assert_eq!(r.ok + r.pending, cases.len());
}
