//! NFJ-1 S1-L5a: table-test runner for the expected values taken from zumen-kirizu (ZK), tests/golden/zumen/*.json.
//! The engine under test sits behind `ZkEngine`, so this compiles before any ported shape exists. Until then the
//! real engine under test is `RealEngine` (see the end of this file; `NotPorted` reports every case PENDING, never PASS). The self-test feeds ZK's own numbers
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
    section: String,
    name: String,
    source: String,
    kind: String,
    want: Value,
    tol: f64,
    zk_got: Value,
    readings: Vec<String>,
    /// Recorded engine calls (`calls`: fn, args) and the bindings of the case (`env`, `helpers`): enough to replay the case
    /// against any engine. `tools/adapter_zk.mjs` replays them against ZK (1844 of 1844 OK, build2-adapter.log).
    inputs: Value,
    /// Expression over `inputs` that yields the engine's answer (`expr`), and its path when it is a plain engine result (`path`).
    got: Value,
    /// Scalars the extractor copied from ZK's own output (`inputs.derived_from_engine_output`). A case with any of
    /// these is NEVER counted as ported (L5a+L6a REVIEW-2 condition) until a real engine recomputes it.
    derived: Vec<String>,
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
    let Some(got) = got else {
        return Verdict::Pending;
    };
    let ok = match (c.kind.as_str(), got) {
        ("num", Got::Num(g)) => c
            .want
            .as_f64()
            .is_some_and(|w| g.is_finite() && (g - w).abs() <= c.tol),
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
                r.ng_lines.push(format!(
                    "NG {} ({}) want {}  [{}]",
                    c.name, c.id, c.want, c.source
                ));
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
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).expect("index.json"))
            .unwrap();
    assert!(
        index["zk_commit"]
            .as_str()
            .unwrap()
            .starts_with(ZK_COMMIT_PREFIX),
        "golden files are not from ZK {ZK_COMMIT_PREFIX}"
    );
    let excluded: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("excluded.json")).expect("excluded.json"),
    )
    .unwrap();
    let n_excluded = excluded["excluded"].as_array().unwrap().len();
    let mut cases = Vec::new();
    for s in index["sections"].as_array().unwrap() {
        let f = dir.join(s["file"].as_str().unwrap());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert!(
            v["zk_commit"]
                .as_str()
                .unwrap()
                .starts_with(ZK_COMMIT_PREFIX),
            "{f:?}"
        );
        let arr = v["cases"].as_array().unwrap();
        assert_eq!(
            arr.len() as u64,
            s["cases"].as_u64().unwrap(),
            "{f:?} case count"
        );
        for c in arr {
            cases.push(Case {
                id: c["id"].as_str().unwrap().into(),
                name: c["name"].as_str().unwrap().into(),
                source: c["source"].as_str().unwrap().into(),
                kind: c["kind"].as_str().unwrap().into(),
                want: c["want"].clone(),
                tol: c["tol"].as_f64().unwrap_or(0.0),
                zk_got: c["zk_got"].clone(),
                readings: c["reading_hint"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap().into())
                    .collect(),
                section: s["id"].as_str().unwrap().into(),
                inputs: c["inputs"].clone(),
                got: c["got"].clone(),
                derived: c["inputs"]["derived_from_engine_output"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }
    (cases, n_excluded)
}

#[test]
fn golden_files_are_complete_and_pinned() {
    let (cases, excluded) = load_cases();
    // every ZK assert is either extracted or listed as excluded: nothing silently dropped
    assert_eq!(
        cases.len() + excluded,
        ZK_TOTAL,
        "extracted + excluded must equal ZK's 1894"
    );
    assert!(cases.len() >= 1800, "extracted {} < 1800", cases.len());
    let mut ids: Vec<_> = cases.iter().map(|c| c.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), cases.len(), "case ids are unique");
    for c in &cases {
        assert!(
            c.source
                .starts_with("design-demos/assets/kirizu/test_build.mjs:"),
            "{} has no ZK source",
            c.id
        );
        assert!(!c.source.ends_with(":0"), "{} has no source line", c.id);
        for r in &c.readings {
            assert!(
                golden_dir().join("readings").join(r).exists(),
                "{}: reading {r} not copied",
                c.id
            );
        }
    }
    assert!(golden_dir().join("README.md").exists());
}

#[test]
fn comparison_uses_zk_tolerance_semantics() {
    let mk = |kind: &str, want: Value, tol: f64| Case {
        id: "t".into(),
        section: "t".into(),
        name: "t".into(),
        source: "x:1".into(),
        kind: kind.into(),
        want,
        tol,
        zk_got: Value::Null,
        readings: vec![],
        inputs: Value::Null,
        got: Value::Null,
        derived: vec![],
    };
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
    println!(
        "self-test: {} OK, {} NG, {} pending of {} extracted ({} excluded, ZK total {})",
        r.ok,
        r.ng,
        r.pending,
        cases.len(),
        excluded,
        ZK_TOTAL
    );
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

/// Every case can be replayed: recorded calls, bindings and a got-expression (no case is a bare number).
#[test]
fn every_case_carries_replayable_inputs() {
    let (cases, _) = load_cases();
    for c in &cases {
        assert!(
            c.inputs["calls"].is_array(),
            "{}: no engine calls field",
            c.id
        );
        assert!(c.inputs["env"].is_object(), "{}: no env field", c.id);
        assert!(
            c.got["expr"].as_str().is_some_and(|e| !e.is_empty()),
            "{}: no got-expression",
            c.id
        );
    }
}

// ======================================================================================================
// The real engine (NFJ-1 S1-L5b). It sees a `CaseKey`: id, section, name, kind and reading file names.
// It cannot see `want`, `zk_got`, `got.expr` or `inputs.derived_from_engine_output`: every input value below is
// written in this file from the ZK test source (test_build.mjs:88-302) and every answer is computed by nf_core.
// ======================================================================================================

use nf_core::order::{self, CylParams, Level, PlateParams, RibGeo, RibParams, TableRow};
use nf_core::shapes::{
    self, Basis, ConeSpec, CylSpec, HoleGrid, PlateSpec, RibSpec, RingSpec, Stock,
};

#[derive(Debug, Clone)]
struct CaseKey {
    id: String,
    kind: String,
    readings: Vec<String>,
}

trait RealEngine {
    fn eval(&self, k: &CaseKey) -> Option<Got>;
}

struct NotPorted;
impl RealEngine for NotPorted {
    fn eval(&self, _k: &CaseKey) -> Option<Got> {
        None
    }
}

fn key_of(c: &Case) -> CaseKey {
    CaseKey {
        id: c.id.clone(),
        kind: c.kind.clone(),
        readings: c.readings.clone(),
    }
}

#[derive(Default, Debug)]
struct RealReport {
    ok: usize,
    ng: usize,
    pending: usize,
    /// Cases with derived inputs: reported here, never in ok/ng (REVIEW-2 condition).
    derived_uncounted: usize,
    ng_lines: Vec<String>,
}

fn run_real(engine: &dyn RealEngine, cases: &[Case]) -> RealReport {
    let mut r = RealReport::default();
    for c in cases {
        let v = judge(c, engine.eval(&key_of(c)));
        if !c.derived.is_empty() {
            r.derived_uncounted += 1;
            continue;
        }
        match v {
            Verdict::Ok => r.ok += 1,
            Verdict::Pending => r.pending += 1,
            Verdict::Ng => {
                r.ng += 1;
                r.ng_lines.push(format!(
                    "NG {} ({}) want {}  [{}]",
                    c.name, c.id, c.want, c.source
                ));
            }
        }
    }
    r
}

fn num(x: f64) -> Option<Got> {
    Some(Got::Num(x))
}
fn count(x: usize) -> Option<Got> {
    Some(Got::Count(x as i64))
}
fn flag(b: bool) -> Option<Got> {
    Some(Got::Flag(b))
}

const DEFAULT_STOCK: [Stock; 1] = [Stock {
    w: 2438.0,
    l: 6096.0,
}];

/// Mirrors ZK's `geo` literal field by field (test table helper).
#[allow(clippy::too_many_arguments)]
fn plate_spec(
    w: f64,
    h: f64,
    w2: Option<f64>,
    off: Option<f64>,
    t: f64,
    c: Option<f64>,
    c_at: u32,
    hole: Option<HoleGrid>,
) -> PlateSpec {
    PlateSpec {
        w,
        h,
        w2,
        off,
        t,
        c,
        c_at,
        hole,
    }
}

fn plate_of(s: PlateSpec) -> shapes::Plate {
    shapes::plate(s).expect("golden plate must build")
}

fn hole(d: f64, x: f64, y: f64) -> HoleGrid {
    HoleGrid {
        d,
        x,
        y,
        nx: 1,
        ny: 1,
        px: 0.0,
        py: 0.0,
    }
}

fn pp(t: f64, w: f64, h: f64) -> PlateParams {
    PlateParams {
        t: Some(t),
        plate_w: Some(w),
        plate_h: Some(h),
        ..Default::default()
    }
}

fn row(size: &str, length: &str) -> TableRow {
    TableRow {
        size: size.into(),
        length: length.into(),
        ..Default::default()
    }
}

/// H_plate.json: parts and material table to the order front half. The reading is a golden input file.
fn h_plate_parts() -> Vec<(PlateParams, TableRow)> {
    let v: Value = serde_json::from_str(
        &std::fs::read_to_string(golden_dir().join("readings/H_plate.json")).unwrap(),
    )
    .unwrap();
    let mut out = Vec::new();
    for p in v["parts"].as_array().unwrap() {
        let g = |k: &str| p["params"][k]["value"].as_f64();
        let params = PlateParams {
            t: g("t"),
            plate_w: g("plate_W"),
            plate_h: g("plate_H"),
            plate_w2: g("plate_W2"),
            plate_off: g("plate_off"),
            plate_c: g("plate_C"),
            plate_c_at: g("plate_C_at"),
            hole_d: g("hole_d"),
            hole_x: g("hole_x"),
            hole_y: g("hole_y"),
            hole_n: g("hole_n"),
            hole_nx: g("hole_nx"),
            hole_ny: g("hole_ny"),
            hole_px: g("hole_px"),
            hole_py: g("hole_py"),
            ..Default::default()
        };
        let no = p["table_rows"][0].as_str().unwrap();
        let r = v["material_table"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["no"] == no)
            .unwrap();
        let mass = r["mass"].as_str().unwrap_or("");
        let row = TableRow {
            size: r["size"].as_str().unwrap().into(),
            length: r["length"].as_str().unwrap().into(),
            mass: mass.parse().ok(),
            mass_decimals: mass.split('.').nth(1).map_or(0, str::len),
            qty: r["qty"].as_str().and_then(|q| q.parse().ok()),
        };
        out.push((params, row));
    }
    out
}

/// H_tee_rib.json rib part with the tee group's branch diameter (89.1 from the sibling branch part: the group fill is
/// the tee lane's job, so the value is given here from the reading's branch part).
fn h_tee_rib_params() -> RibParams {
    let v: Value = serde_json::from_str(
        &std::fs::read_to_string(golden_dir().join("readings/H_tee_rib.json")).unwrap(),
    )
    .unwrap();
    let part = |no: &str| {
        v["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["no"] == no)
            .unwrap()
            .clone()
    };
    let (rib, br) = (part("3"), part("2"));
    let g = |k: &str| rib["params"][k]["value"].as_f64();
    let row = v["material_table"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["no"] == "3")
        .unwrap()
        .clone();
    let mass = row["mass"].as_str().unwrap();
    RibParams {
        t: g("t"),
        main_d: g("main_D"),
        branch_d: br["params"]["D"]["value"].as_f64(),
        e: br["params"]["e"]["value"].as_f64(),
        rib_b: g("rib_b"),
        rib_h: g("rib_h"),
        rib_n1: g("rib_n1"),
        rib_n2: g("rib_n2"),
        table_qty: row["qty"].as_str().and_then(|q| q.parse().ok()),
        table_mass: mass.parse().ok(),
        table_mass_decimals: mass.split('.').nth(1).map_or(0, str::len),
        ..Default::default()
    }
}

fn rib_params(e: Option<f64>, b: f64, h: f64, n1: Option<f64>, n2: Option<f64>) -> RibParams {
    RibParams {
        t: Some(6.0),
        main_d: Some(318.5),
        branch_d: Some(89.1),
        e,
        rib_b: Some(b),
        rib_h: Some(h),
        rib_n1: n1,
        rib_n2: n2,
        ..Default::default()
    }
}

fn circle_area(o: &order::RibOut) -> Option<f64> {
    matches!(o.geo, RibGeo::Circle(_)).then_some(o.area)
}

fn eval_rib_build(n: usize) -> Option<Got> {
    match n {
        46 | 47 | 48 | 50 | 51 | 52 => {
            let r = order::check_tee_rib(&h_tee_rib_params());
            match n {
                46 => num(r.pieces.len() as f64),
                47 => num(r
                    .pieces
                    .iter()
                    .find(|p| matches!(p.geo, RibGeo::Circle(_)))?
                    .kg),
                48 => num(r
                    .pieces
                    .iter()
                    .find(|p| matches!(p.geo, RibGeo::Triangle(_)))?
                    .kg),
                50 => flag(r.has(Level::Ok, "mass_vs_table")),
                51 => flag(r.has(Level::Info, "rib_side")),
                _ => count(r.count(Level::Need)),
            }
        }
        53 | 54 => {
            let p = RibParams {
                t: Some(9.0),
                main_d: Some(1000.0),
                branch_d: Some(300.0),
                e: Some(100.0),
                rib_b: Some(100.0),
                rib_h: Some(80.0),
                rib_n1: Some(2.0),
                ..Default::default()
            };
            let r = order::check_tee_rib(&p);
            circle_area(r.pieces.get(n - 53)?).map(Got::Num)
        }
        55 | 56 => {
            let p = RibParams {
                t: Some(9.0),
                main_d: Some(1000.0),
                branch_d: Some(300.0),
                rib_b: Some(100.0),
                rib_h: Some(80.0),
                rib_n1: Some(2.0),
                ..Default::default()
            };
            let r = order::check_tee_rib(&p);
            if n == 55 {
                num(if r.pieces.len() == 1 {
                    r.pieces[0].n
                } else {
                    0.0
                })
            } else {
                num(r.pieces.first()?.area)
            }
        }
        57 => flag(
            order::check_tee_rib(&rib_params(Some(114.7), 60.0, 50.0, Some(2.0), None))
                .holds
                .iter()
                .any(|h| h.level == Level::Need && h.code == "rib_not_on_pipe"),
        ),
        58 => flag(
            order::check_tee_rib(&rib_params(None, 60.0, 50.0, None, None))
                .has(Level::Need, "rib_count"),
        ),
        59 => {
            let p = RibParams {
                t: Some(6.0),
                main_d: Some(508.0),
                branch_d: Some(114.3),
                e: Some((508.0 - 114.3) / 2.0),
                rib_b: Some(80.0),
                rib_h: Some(60.0),
                rib_n1: Some(1.0),
                rib_n2: Some(2.0),
                rib_top_stated: true,
                ..Default::default()
            };
            flag(order::check_tee_rib(&p).has(Level::Need, "rib_top"))
        }
        60 => flag(
            order::check_tee_rib(&RibParams {
                table_qty: Some(4.0),
                ..rib_params(Some(114.7), 60.0, 50.0, Some(1.0), Some(2.0))
            })
            .has(Level::Need, "rib_qty"),
        ),
        61 => flag(
            order::check_tee_rib(&rib_params(Some(114.7), 10.0, 50.0, Some(1.0), None))
                .has(Level::Need, "rib_tip_inside_pipe"),
        ),
        62 => {
            let r = order::check_tee_rib(&RibParams {
                table_qty: Some(3.0),
                ..rib_params(Some(114.7), 60.0, 50.0, Some(1.0), Some(2.0))
            });
            flag(
                !r.holds
                    .iter()
                    .any(|h| h.code == "tee_qty" || h.code == "rib_qty"),
            )
        }
        _ => None,
    }
}

struct RustShapes;
impl RealEngine for RustShapes {
    fn eval(&self, k: &CaseKey) -> Option<Got> {
        let (sec, n) = k.id.split_once('-')?;
        let n: usize = n.parse().ok()?;
        let _ = (&k.kind, &k.readings);
        match sec {
            "s06" => eval_s06(n),
            "s07" => eval_s07(n),
            "s08" => eval_s08(n),
            "s09" => eval_s09(n),
            "s14" => eval_s14(n),
            "s01" => eval_s01(n),
            "s02" => eval_s02(n),
            "s03" => eval_s03(n),
            "s05" => eval_s05(n),
            "s10" => eval_s10(n),
            "s11" => eval_s11(n),
            "s12" => eval_s12(n),
            "s13" => eval_s13(n),
            _ => None,
        }
    }
}

fn eval_s06(n: usize) -> Option<Got> {
    // test_build.mjs:92 ring OD 800 ID 0 t 16 split 1
    let r = shapes::ring(RingSpec {
        od: 800.0,
        id: 0.0,
        t: 16.0,
        split: 1,
    })
    .ok()?;
    match n {
        1 => num(if r.pieces[0].has_hole { 1.0 } else { 0.0 }),
        2 => num(r.mass_kg),
        _ => None,
    }
}

fn eval_s07(n: usize) -> Option<Got> {
    match n {
        1..=6 => {
            // :100-103 concentric cone D1 800 D2 1200 L 600, t 0.001, split n = 2, 3, 4; odd id chord, even id height
            let split = (n.div_ceil(2) + 1) as u32;
            let c = shapes::cone_split(ConeSpec {
                d1: 800.0,
                d2: 1200.0,
                l: 600.0,
                t: 0.001,
                basis: Basis::Mid,
                split,
            })
            .ok()?;
            num(if n % 2 == 1 { c.chord } else { c.height })
        }
        7..=14 => {
            // :106-109 eccentric cone D1 1200 D2 800 L 700 t 0.001; id 7 -> n 2 piece 1; 8-10 -> n 3; 11-14 -> n 4
            let (split, piece) = match n {
                7 => (2, 0),
                8..=10 => (3, n - 8),
                _ => (4, n - 11),
            };
            let e = shapes::ecc_cone_split(ConeSpec {
                d1: 1200.0,
                d2: 800.0,
                l: 700.0,
                t: 0.001,
                basis: Basis::Mid,
                split,
            })
            .ok()?;
            num(e.pieces[piece].dev_angle.to_degrees())
        }
        _ => None,
    }
}

fn eval_s08(n: usize) -> Option<Got> {
    let hp = |c: Option<f64>, at: u32| plate_spec(300.0, 80.0, None, None, 1.0, c, at, None);
    match n {
        1 => num(plate_of(hp(Some(10.0), 0)).area),
        2 => num(plate_of(hp(Some(10.0), 0)).perimeter),
        3 => num(plate_of(plate_spec(
            300.0,
            200.0,
            Some(150.0),
            Some(0.0),
            1.0,
            None,
            0,
            Some(hole(22.0, 60.0, 50.0)),
        ))
        .area),
        4 => num(plate_of(plate_spec(
            300.0,
            200.0,
            Some(150.0),
            Some(75.0),
            1.0,
            None,
            0,
            None,
        ))
        .area),
        5 | 6 => num(plate_of(plate_spec(
            200.0,
            150.0,
            Some(0.0),
            Some(0.0),
            1.0,
            Some(15.0),
            if n == 5 { 1 } else { 2 },
            None,
        ))
        .area),
        7 => num(plate_of(plate_spec(
            200.0,
            150.0,
            Some(0.0),
            Some(0.0),
            9.0,
            Some(15.0),
            2,
            None,
        ))
        .w),
        8 => num(plate_of(plate_spec(
            400.0,
            400.0,
            None,
            None,
            1.0,
            None,
            0,
            Some(HoleGrid {
                d: 24.0,
                x: 50.0,
                y: 50.0,
                nx: 2,
                ny: 2,
                px: 300.0,
                py: 300.0,
            }),
        ))
        .area),
        // 9: ZK DXF writer line count (derived, a 360-gon per hole): the NF writer draws circles; no equivalent -> pending
        10..=21 => {
            let parts: Vec<_> = h_plate_parts()
                .iter()
                .map(|(p, r)| order::check_plate(p, Some(r)))
                .collect();
            let pc = |i: usize| parts[i].piece.as_ref();
            match n {
                10 => num(pc(0)?.kg),
                11 => num(pc(0)?.plate.w),
                12 => num(pc(0)?.plate.h),
                13 => flag(parts[0].has(Level::Info, "plate_from_table")),
                14 => num(pc(1)?.kg),
                15 => num(pc(1)?.gross_kg),
                16 => num(pc(2)?.kg),
                17 => num(pc(2)?.gross_kg),
                18 => num(pc(3)?.kg),
                19 => flag(parts[1].has(Level::Ok, "mass_vs_table_blank")),
                20 => flag(parts[2].has(Level::Ok, "size_vs_table")),
                _ => count(parts.iter().map(|p| p.count(Level::Need)).sum()),
            }
        }
        22 | 23 => {
            // :150 material-table writings; the extractor flagged `s` as derived: evaluated, never counted
            let (s, l, want) = if n == 22 {
                ("12t×300×80", "", (12.0, 300.0, 80.0))
            } else {
                ("PL-16×800", "2700", (16.0, 800.0, 2700.0))
            };
            flag(order::parse_pl(s, l, None) == Some((want.0, Some(want.1), Some(want.2))))
        }
        24 => flag(
            order::check_plate(&PlateParams::default(), Some(&row("PL-19×φ1900", "φ1600")))
                .has(Level::Need, "missing"),
        ),
        25 => flag(
            order::check_plate(&pp(12.0, 300.0, 80.0), Some(&row("PL-9×300×80", "")))
                .has(Level::Need, "plate_t_vs_table"),
        ),
        26 | 27 => {
            let r = order::check_plate(
                &PlateParams {
                    plate_w2: Some(150.0),
                    ..pp(9.0, 300.0, 200.0)
                },
                None,
            );
            if n == 26 {
                flag(r.has(Level::Need, "plate_off"))
            } else {
                num(r.piece.is_some() as u8 as f64)
            }
        }
        28 | 29 => {
            let r = order::check_plate(
                &PlateParams {
                    plate_w2: Some(0.0),
                    ..pp(9.0, 200.0, 150.0)
                },
                None,
            );
            if n == 28 {
                flag(r.has(Level::Info, "plate_off_defaulted"))
            } else {
                num(r.piece?.plate.area)
            }
        }
        30 => flag(
            order::check_plate(
                &PlateParams {
                    plate_c: Some(10.0),
                    ..pp(9.0, 300.0, 80.0)
                },
                None,
            )
            .has(Level::Need, "plate_C_at"),
        ),
        31 => flag(
            order::check_plate(
                &PlateParams {
                    plate_c: Some(10.0),
                    plate_c_at: Some(0.0),
                    ..pp(9.0, 300.0, 15.0)
                },
                None,
            )
            .has(Level::Need, "plate_C_big"),
        ),
        32 => flag(
            order::check_plate(
                &PlateParams {
                    hole_d: Some(22.0),
                    ..pp(9.0, 300.0, 200.0)
                },
                None,
            )
            .has(Level::Need, "plate_hole_pos"),
        ),
        33 | 34 | 36 => {
            let base = PlateParams {
                hole_d: Some(24.0),
                hole_x: Some(50.0),
                hole_y: Some(50.0),
                ..pp(16.0, 400.0, 400.0)
            };
            let (p, code) = match n {
                33 => (
                    PlateParams {
                        hole_n: Some(4.0),
                        ..base
                    },
                    "plate_hole_grid",
                ),
                34 => (
                    PlateParams {
                        hole_n: Some(6.0),
                        hole_nx: Some(2.0),
                        hole_ny: Some(2.0),
                        hole_px: Some(300.0),
                        hole_py: Some(300.0),
                        ..base
                    },
                    "plate_hole_grid",
                ),
                _ => (
                    PlateParams {
                        hole_n: Some(2.0),
                        hole_nx: Some(2.0),
                        hole_px: Some(20.0),
                        ..base
                    },
                    "plate_hole_overlap",
                ),
            };
            flag(order::check_plate(&p, None).has(Level::Need, code))
        }
        35 => flag(
            order::check_plate(
                &PlateParams {
                    plate_w2: Some(150.0),
                    plate_off: Some(0.0),
                    hole_d: Some(22.0),
                    hole_x: Some(280.0),
                    hole_y: Some(150.0),
                    ..pp(9.0, 300.0, 200.0)
                },
                None,
            )
            .has(Level::Need, "plate_hole_out"),
        ),
        37..=45 => {
            let (ro, vc, b, h, top) = match n {
                37 | 38 => (318.5 / 2.0, 318.5 / 2.0 - 89.1, 60.0, 50.0, 0.0),
                39 | 40 => (1016.0 / 2.0, 1016.0 / 2.0 - 318.5, 140.0, 120.0, 0.0),
                41 | 42 => (2641.6 / 2.0, 2641.6 / 2.0 - 406.4, 220.0, 200.0, 0.0),
                43 => (254.0, 254.0 - 114.3, 80.0, 60.0, 0.0),
                44 => (254.0, 254.0 - 114.3, 80.0, 60.0, 20.0),
                _ => (1e5, 0.0, 60.0, 50.0, 0.0),
            };
            let r = shapes::rib(RibSpec { ro, vc, b, h, top }).ok()?;
            num(if matches!(n, 38 | 40 | 42) {
                r.x_c
            } else {
                r.area
            })
        }
        46..=62 => eval_rib_build(n),
        _ => None,
    }
}

fn eval_s09(n: usize) -> Option<Got> {
    let cyl = |seams| {
        shapes::cylinder(CylSpec {
            d: 2641.6,
            t: 16.0,
            l: 1500.0,
            seams,
        })
        .ok()
    };
    let cp = |l: f64, split: Option<f64>| CylParams {
        d: Some(2641.6),
        t: Some(16.0),
        l: Some(l),
        split,
    };
    let has = |p: &CylParams, stock: &[Stock], level, code: &str| {
        order::check_cylinder(p, stock, 0.0)
            .is_some_and(|c| c.holds.iter().any(|h| h.level == level && h.code == code))
    };
    match n {
        1 => num(cyl(1)?.circ),
        2 => num(f64::from(cyl(2)?.plates)),
        3 => num(cyl(2)?.plate_w),
        4 => num(cyl(2)?.plate_h),
        5 => num(cyl(2)?.area_total),
        6 => num(cyl(2)?.mass_kg),
        7 => num(cyl(3)?.plate_w),
        8 => num(cyl(4)?.plate_w),
        9 => {
            num(shapes::fit_stock(4124.283, 1500.0, &DEFAULT_STOCK, 0.0).map_or(-1.0, |i| i as f64))
        }
        10 => {
            num(shapes::fit_stock(2749.522, 2500.0, &DEFAULT_STOCK, 0.0).map_or(-1.0, |i| i as f64))
        }
        11 => num(shapes::fit_stock(
            2062.142,
            2500.0,
            &[Stock {
                w: 6096.0,
                l: 2438.0,
            }],
            0.0,
        )
        .map_or(-1.0, |i| i as f64)),
        12 => flag(has(
            &cp(1500.0, None),
            &DEFAULT_STOCK,
            Level::Need,
            "stock_fit",
        )),
        13 => num(
            shapes::min_cyl_seams(2641.6, 16.0, 1500.0, 2, &DEFAULT_STOCK, 0.0)
                .ok()??
                .into(),
        ),
        14..=16 => {
            let w = [1524.0, 1829.0, 2438.0][n - 14];
            num(
                shapes::min_cyl_seams(2641.6, 16.0, 2500.0, 2, &[Stock { w, l: 6096.0 }], 0.0)
                    .ok()??
                    .into(),
            )
        }
        17 => flag(has(
            &cp(7000.0, None),
            &DEFAULT_STOCK,
            Level::Need,
            "pipe_longer_than_stock",
        )),
        18 => count(
            order::check_cylinder(&cp(1500.0, Some(2.0)), &DEFAULT_STOCK, 0.0)?
                .holds
                .iter()
                .filter(|h| h.level == Level::Need)
                .count(),
        ),
        19 => num(f64::from(
            order::check_cylinder(&cp(1500.0, Some(2.0)), &DEFAULT_STOCK, 0.0)?.plates,
        )),
        20 => num(order::check_cylinder(&cp(1500.0, Some(2.0)), &DEFAULT_STOCK, 0.0)?.kg),
        21 => num(order::check_cylinder(&cp(1500.0, Some(2.0)), &DEFAULT_STOCK, 0.0)?.plate_w),
        22 => flag(has(
            &cp(1500.0, Some(2.0)),
            &DEFAULT_STOCK,
            Level::Info,
            "stock_checked",
        )),
        23 => flag(
            !order::check_cylinder(&cp(1500.0, None), &[], 0.0)?
                .holds
                .iter()
                .any(|h| h.code.starts_with("stock")),
        ),
        71 => {
            flag(order::check_cylinder(&cp(2500.0, None), &DEFAULT_STOCK, 0.0)?.fits_at == Some(4))
        }
        24 => flag(has(
            &cp(1500.0, Some(8.0)),
            &DEFAULT_STOCK,
            Level::Need,
            "split_seam",
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------- s14: reducing mitered bend (NFJ-1 S1-L6b)
// Inputs below are written from test_build.mjs:622-743 (examples `cases`, readings K_redbend.json and the R800 / J1 / steep
// literals). Cases that need the engine's check list, labels or the DXF writer stay PENDING (the engine is not wired).

use nf_core::bend::{self, BendResult, BendSpec, SwingRule};

#[derive(Clone, Copy)]
struct BendEx {
    d: f64,
    de: f64,
    t: f64,
    r: f64,
    ang: f64,
    j: u32,
    s1: f64,
    s2: f64,
    taper: Option<u32>,
    th: f64,
}

const BEND_EX: [BendEx; 4] = [
    BendEx {
        d: 2641.6,
        de: 2438.4,
        t: 16.0,
        r: 2600.0,
        ang: 90.0,
        j: 3,
        s1: 300.0,
        s2: 300.0,
        taper: None,
        th: 2.0894,
    },
    BendEx {
        d: 1625.6,
        de: 1371.6,
        t: 12.0,
        r: 2400.0,
        ang: 90.0,
        j: 4,
        s1: 400.0,
        s2: 400.0,
        taper: None,
        th: 2.5412,
    },
    BendEx {
        d: 1625.6,
        de: 1371.6,
        t: 12.0,
        r: 2400.0,
        ang: 90.0,
        j: 4,
        s1: 400.0,
        s2: 400.0,
        taper: Some(3),
        th: 7.6439,
    },
    BendEx {
        d: 1524.0,
        de: 1625.6,
        t: 12.0,
        r: 1500.0,
        ang: 45.0,
        j: 2,
        s1: 500.0,
        s2: 400.0,
        taper: None,
        th: -4.8835,
    },
];

/// ZK's test options: basis mid, amp outer (swing = outer), allow 4, seam 45 deg from the heel.
fn bend_spec(e: &BendEx, split: u32) -> BendSpec {
    let mut s = BendSpec::new(e.d, e.t, e.r, e.ang, Some(f64::from(e.j)), e.s1, e.s2);
    s.d_end = Some(e.de);
    s.taper_piece = e.taper;
    s.basis = Basis::Mid;
    s.swing = SwingRule::Outer;
    s.split = split;
    s
}

fn bend_of(e: &BendEx, split: u32) -> Option<BendResult> {
    bend::bend(&bend_spec(e, split)).ok()
}

/// The readings K_redbend.json part 1 (= example 1 with 3 vertical seams).
fn k_spec() -> BendSpec {
    bend_spec(&BEND_EX[0], 3)
}

fn refused_with(s: &BendSpec, code: &str) -> bool {
    match bend::bend(s) {
        Err(r) => r.code == code && r.message.chars().all(|c| c != '\0'),
        Ok(_) => false,
    }
}

fn eval_s14(n: usize) -> Option<Got> {
    let idx = n - 1;
    let starts = [0usize, 47, 105, 163];
    let npieces = [4usize, 5, 5, 3];
    if idx < 199 {
        let b = (0..4).rev().find(|&b| idx >= starts[b])?;
        let e = &BEND_EX[b];
        let r = bend_of(e, 1)?;
        let rel = idx - starts[b];
        if rel < npieces[b] * 11 {
            let p = &r.pieces[rel / 11];
            let band = &p.bands[0];
            return num(match rel % 11 {
                0 => band.w,
                1 => band.h,
                2 => p.axis,
                3 => p.ten,
                4 => p.chi,
                5 => p.yo,
                6 => p.seam_len,
                7 => p.d_lo,
                8 => p.d_hi,
                9 => p.net_mass_kg,
                _ => p.center,
            });
        }
        return match rel - npieces[b] * 11 {
            0 => num(r.total_net_mass_kg),
            1 => num(r.total_axis),
            _ => {
                let cones: Vec<f64> = r
                    .pieces
                    .iter()
                    .filter(|p| p.cone)
                    .map(|p| p.th_deg)
                    .collect();
                let want = if e.taper.is_some() {
                    1
                } else {
                    e.j as usize - 1
                };
                flag(
                    cones.len() == want
                        && cones.iter().all(|x| (x - e.th).abs() <= 1e-4)
                        && r.pieces[0].th_deg == 0.0
                        && r.pieces[e.j as usize].th_deg == 0.0,
                )
            }
        };
    }
    match idx {
        199 => num(bend_of(&BEND_EX[0], 1)?.pieces[1].fan_deg?),
        200..=205 => {
            let (b, m) = [(0, 2), (0, 3), (1, 2), (1, 3), (1, 4), (3, 2)][idx - 200];
            num(bend_of(&BEND_EX[b], 1)?.pieces[m - 1].d_mid?)
        }
        206..=214 => {
            let md = bend::model(&bend_spec(&BEND_EX[0], 1)).ok()?;
            let j = &md.joints[(idx - 206) / 3];
            num(match (idx - 206) % 3 {
                0 => j.ph_a.to_degrees(),
                1 => j.ph_b.to_degrees(),
                _ => j.d,
            })
        }
        215 => num(bend::model(&bend_spec(&BEND_EX[0], 1)).ok()?.pieces[0].zhi),
        216 => num(bend::model(&bend_spec(&BEND_EX[0], 1)).ok()?.pieces[3].zlo),
        217 => num(bend::model(&bend_spec(&BEND_EX[3], 1)).ok()?.pieces[0].zhi),
        218 => num(bend::model(&bend_spec(&BEND_EX[3], 1)).ok()?.pieces[2].zlo),
        219..=256 => {
            // (example, piece, split, first id): 219 ex1 m2 n2; 230 ex1 m3 n3; 246 ex3 m2 n2
            let (b, m, split, first) = match idx {
                219..=229 => (0, 2, 2, 219),
                230..=245 => (0, 3, 3, 230),
                _ => (3, 2, 2, 246),
            };
            let r = bend_of(&BEND_EX[b], split)?;
            let p = &r.pieces[m - 1];
            let k = idx - first;
            if k == 0 {
                return count_num(p.bands.len());
            }
            let band = &p.bands[(k - 1) / 5];
            num(match (k - 1) % 5 {
                0 => band.w,
                1 => band.h,
                2 => band.area,
                3 => band.left_edge,
                _ => band.right_edge,
            })
        }
        257..=265 => eval_s14_continuity(idx - 257),
        266 => flag(refused_with(
            &BendSpec {
                d_end: Some(1371.6),
                ..BendSpec::new(1625.6, 12.0, 2400.0, 30.0, Some(1.0), 400.0, 400.0)
            },
            "bend_reducing_needs_two_joints",
        )),
        267 => flag(refused_with(
            &BendSpec {
                d_end: Some(600.0),
                swing: SwingRule::Outer,
                basis: Basis::Mid,
                ..BendSpec::new(1625.6, 12.0, 600.0, 30.0, Some(2.0), 400.0, 400.0)
            },
            "bend_cone_too_steep",
        )),
        268..=273 => {
            let reducing = idx != 273;
            let mut s = BendSpec::new(1625.6, 12.0, 800.0, 90.0, Some(3.0), 300.0, 300.0);
            s.d_end = Some(if reducing { 1371.6 } else { 1625.6 });
            s.swing = SwingRule::Outer;
            let md = bend::model(&s).ok()?;
            match idx {
                268 => flag(md.cross == [3]),
                269..=272 => num(md.pieces[idx - 269].chi),
                _ => flag(md.cross == [2, 3]),
            }
        }
        _ => eval_s14_k(idx),
    }
}

fn count_num(n: usize) -> Option<Got> {
    num(n as f64)
}

/// Same diameter: the reducing code equals the plain bend's cut curves (ZK `bendPiece`, written here from the spec:
/// z = cl + up * k cos(psi), k = r_swing tan(gamma)), point for point, for 1, 2, 3 seams. Cases 257-265 (index 256..=264)
/// are 3 positions (start, middle, end) x 3 seam counts.
fn eval_s14_continuity(k: usize) -> Option<Got> {
    let (pos, nseam) = (k / 3, k % 3 + 1);
    let mut s = BendSpec::new(2641.6, 16.0, 2600.0, 90.0, Some(3.0), 300.0, 300.0);
    s.d_end = Some(2641.6);
    s.swing = SwingRule::Outer;
    s.split = nseam as u32;
    let md = bend::model(&s).ok()?;
    let p = &md.pieces[if pos == 2 { 3 } else { pos }];
    let gamma = (90.0f64 / 3.0 / 2.0).to_radians();
    let kk = (2641.6 / 2.0) * gamma.tan();
    let (lo, up) = match pos {
        0 => (0.0, 1.0),
        1 => (1.0, 1.0),
        _ => (1.0, 0.0),
    };
    let cl = match pos {
        0 => 300.0 + 2600.0 * gamma.tan(),
        1 => 2.0 * 2600.0 * gamma.tan(),
        _ => 300.0 + 2600.0 * gamma.tan(),
    };
    let ph = 135f64.to_radians();
    let mut mx = 0.0f64;
    for i in 0..=720 {
        let psi = ph + 2.0 * std::f64::consts::PI * f64::from(i) / 720.0;
        let c = psi.cos();
        let lo_plain = -lo * kk * c;
        let up_plain = cl + up * kk * c;
        mx = mx
            .max((bend::cut_z(p, psi, false) - p.zlo - lo_plain).abs())
            .max((bend::cut_z(p, psi, true) - p.zlo - up_plain).abs());
    }
    flag(mx < 1e-6 && (p.axis - cl).abs() < 1e-9)
}

fn eval_s14_k(idx: usize) -> Option<Got> {
    let r = bend::bend(&k_spec()).ok();
    match idx {
        276 => count_num(r?.pieces.len()),
        278..=289 => {
            let p = &r?.pieces[(idx - 278) / 3];
            match (idx - 278) % 3 {
                0 => num(p.mass_with_allow_kg),
                1 => num(p.center),
                _ => count_num(p.bands.len()),
            }
        }
        294 => flag(r?.tilt_note_deg.is_none()),
        297 => count_num(
            bend::bend(&BendSpec {
                d_end: Some(1371.6),
                swing: SwingRule::Outer,
                ..BendSpec::new(1625.6, 12.0, 2400.0, 30.0, Some(1.0), 400.0, 400.0)
            })
            .map_or(0, |r| r.pieces.len()),
        ),
        // s14-0300: PENDING, NF refuses where ZK ignored taper_piece (piece 1 is not a valid cone piece; NF refuses
        // with bend_taper_piece_out_of_range). The expectation is not changed.
        299 => None,
        301 => {
            let mut s = k_spec();
            s.taper_piece = Some(3);
            num(bend::bend(&s).ok()?.pieces[1].th_deg)
        }
        310 => {
            let mut s = BendSpec::new(1625.6, 12.0, 800.0, 90.0, Some(3.0), 300.0, 300.0);
            s.d_end = Some(1371.6);
            s.swing = SwingRule::Outer;
            count_num(bend::bend(&s).map_or(0, |r| r.pieces.len()))
        }
        313..=318 => {
            let mut s = k_spec();
            let code = match idx {
                313 => {
                    s.joints = Some(0.0);
                    "bend_joints_invalid"
                }
                314 => {
                    s.joints = Some(0.0);
                    s.d_end = Some(2641.6);
                    "bend_joints_invalid"
                }
                315 => {
                    s.joints = Some(2.5);
                    "bend_joints_invalid"
                }
                316 => {
                    s.joints = Some(f64::NAN);
                    "bend_joints_invalid"
                }
                317 => {
                    s.t = -5.0;
                    "bend_value_not_positive"
                }
                _ => {
                    s.d = 0.0;
                    "bend_value_not_positive"
                }
            };
            flag(refused_with(&s, code))
        }
        _ => None,
    }
}

/// Sections whose every case is computed by the real engine and none carries derived inputs. Add a section id here
/// only in the commit that makes it so: the test below then requires 0 NG and 0 PENDING and no derived case.
const PORTED_SECTIONS: &[&str] = &["s06", "s07"];

#[test]
fn derived_cases_are_191_and_never_counted_as_ported() {
    let (cases, _) = load_cases();
    let derived: Vec<&Case> = cases.iter().filter(|c| !c.derived.is_empty()).collect();
    assert_eq!(
        derived.len(),
        191,
        "L5a documents 191 cases with derived inputs"
    );
    // a section holding any derived case cannot be listed as ported
    for c in &derived {
        assert!(
            !PORTED_SECTIONS.contains(&c.section.as_str()),
            "{} is derived but its section is listed as ported",
            c.id
        );
    }
    // the real engine's report keeps them out of ok/ng/pending
    let r = run_real(&RustShapes, &cases);
    assert_eq!(r.derived_uncounted, 191);
    assert_eq!(r.ok + r.ng + r.pending + r.derived_uncounted, cases.len());
}

#[test]
fn ported_sections_are_fully_computed_and_within_tolerance() {
    let (cases, _) = load_cases();
    let ported: Vec<Case> = cases
        .into_iter()
        .filter(|c| PORTED_SECTIONS.contains(&c.section.as_str()))
        .collect();
    assert_eq!(ported.len(), 16, "s06 (2) + s07 (14)");
    let r = run_real(&RustShapes, &ported);
    for l in &r.ng_lines {
        println!("{l}");
    }
    assert_eq!((r.ng, r.pending, r.derived_uncounted), (0, 0, 0));
    assert_eq!(r.ok, ported.len());
}

/// Per section report of the real engine (ZK's section = a `//` block of test_build.mjs). Lane S1-L5b owns s06-s09;
/// the rest stay PENDING. A computed case that is outside tolerance fails this test.
#[test]
fn real_engine_status() {
    let (cases, excluded) = load_cases();
    let mut total = RealReport::default();
    for sec in ["s06", "s07", "s08", "s09"] {
        let part: Vec<Case> = cases.iter().filter(|c| c.section == sec).cloned().collect();
        let r = run_real(&RustShapes, &part);
        println!(
            "{sec}: {} OK, {} NG, {} PENDING, {} derived (not counted) of {}",
            r.ok,
            r.ng,
            r.pending,
            r.derived_uncounted,
            part.len()
        );
        for l in &r.ng_lines {
            println!("{l}");
        }
        total.ok += r.ok;
        total.ng += r.ng;
        total.pending += r.pending;
        total.derived_uncounted += r.derived_uncounted;
    }
    println!(
        "lane s06-s09: {} OK, {} NG, {} PENDING, {} derived uncounted; excluded in ZK extraction: {excluded}",
        total.ok, total.ng, total.pending, total.derived_uncounted
    );
    assert_eq!(total.ng, 0);
    // the lane must not lose cases it already computes
    assert!(total.ok >= 80, "only {} cases computed", total.ok);
    // and the stub reports everything pending
    let none = run_real(&NotPorted, &cases);
    assert_eq!((none.ok, none.ng), (0, 0));
}

/// Lane S1-L6b: section s14 (test_build.mjs:622-743, reducing mitered bend). Computed by `nf_core::bend`; the cases that need
/// the engine's check list, labels or the DXF writer stay PENDING (not wired), the 9 derived ones are never counted.
#[test]
fn real_engine_status_s14() {
    let (cases, _) = load_cases();
    let part: Vec<Case> = cases
        .iter()
        .filter(|c| c.section == "s14")
        .cloned()
        .collect();
    assert_eq!(part.len(), 319);
    let r = run_real(&RustShapes, &part);
    println!(
        "s14: {} OK, {} NG, {} PENDING, {} derived (not counted) of {}",
        r.ok,
        r.ng,
        r.pending,
        r.derived_uncounted,
        part.len()
    );
    for l in &r.ng_lines {
        println!("{l}");
    }
    assert_eq!(r.ng, 0, "NG cases in s14");
    assert_eq!(r.derived_uncounted, 9);
    assert!(r.ok >= 270, "only {} s14 cases computed", r.ok);
}

// ======================================================================================================
// Lane S1-L7a: tee main + branch (s01 E/F, s02 thin wall, s03 G, s05 two stops, s10 multi-branch). Every input value is
// written below from the ZK test source (test_build.mjs:25-87, :303-415) or read from the reading JSON as DATA (the
// drawing values); every answer is computed by `nf_core::tee`. Cases that need pads (L7b), the order layer's check list,
// notes, stock or the DXF writer stay PENDING (None). A case with derived inputs is reported, never counted.
// ======================================================================================================

use nf_core::tee::{self, BranchOnMain, BranchPieceSpec, Mount, TeeMainResult, TeeMainSpec};

fn br(name: &str, d: f64, t: f64, x: f64, dir: f64, e: f64) -> BranchOnMain {
    BranchOnMain::new(name, d, t, Some(x), dir, e)
}

fn main_of(d: f64, t: f64, l: f64, bs: Vec<BranchOnMain>) -> Option<TeeMainResult> {
    tee::tee_main(&TeeMainSpec::new(d, t, l, bs)).ok()
}

fn main_with(spec: &TeeMainSpec) -> Option<TeeMainResult> {
    tee::tee_main(spec).ok()
}

fn piece_of(
    d: f64,
    t: f64,
    main_d: f64,
    main_t: f64,
    h: f64,
    e: f64,
    mount: Mount,
) -> Option<tee::BranchPieceResult> {
    tee::branch_piece(&BranchPieceSpec {
        d,
        t,
        main_d,
        main_t,
        h,
        e,
        mount,
        angle_deg: None,
    })
    .ok()
}

fn hole_perimeter(h: &tee::HoleOut) -> f64 {
    let n = h.pts.len();
    (0..n)
        .map(|i| {
            let (p, q) = (h.pts[i], h.pts[(i + 1) % n]);
            (q[0] - p[0]).hypot(q[1] - p[1])
        })
        .sum()
}

fn tee_refused_with(
    r: Result<impl std::fmt::Debug, nf_core::shapes::ShapeRefusal>,
    code: &str,
) -> bool {
    matches!(r, Err(e) if e.code == code)
}

/// A tee group read from a reading JSON (drawing values as data; the group fill of the order layer is written here
/// minimally: a branch takes its diameter and wall from the main part's branch_D / branch_t when its own are absent).
#[allow(clippy::type_complexity)]
struct ReadTee {
    d: f64,
    t: f64,
    l: f64,
    branches: Vec<(String, f64, f64, f64, Option<f64>, f64, Option<f64>, f64)>,
}

fn read_tee(file: &str, edit: impl Fn(&mut Value)) -> ReadTee {
    let mut v: Value = serde_json::from_str(
        &std::fs::read_to_string(golden_dir().join("readings").join(file)).unwrap(),
    )
    .unwrap();
    edit(&mut v);
    let g = |p: &Value, k: &str| p["params"][k]["value"].as_f64();
    let parts = v["parts"].as_array().unwrap();
    let m = parts.iter().find(|p| p["shape"] == "tee_main").unwrap();
    let mut out = ReadTee {
        d: g(m, "D").unwrap(),
        t: g(m, "t").unwrap(),
        l: g(m, "L").unwrap(),
        branches: vec![],
    };
    for p in parts.iter().filter(|p| p["shape"] == "tee_branch") {
        out.branches.push((
            p["no"].as_str().unwrap().into(),
            g(p, "D").or(g(m, "branch_D")).unwrap(),
            g(p, "t").or(g(m, "branch_t")).unwrap(),
            g(p, "H").unwrap(),
            g(p, "branch_x").or(g(m, "branch_x")),
            g(p, "dir").unwrap_or(0.0),
            g(p, "e_side"),
            g(p, "e").unwrap_or(0.0),
        ));
    }
    out
}

impl ReadTee {
    fn spec(&self) -> Option<TeeMainSpec> {
        let mut bs = vec![];
        for (no, d, t, _h, x, dir, side, e) in &self.branches {
            let se = tee::signed_offset(*e, *side, *dir).ok()?;
            bs.push(BranchOnMain::new(no, *d, *t, *x, *dir, se));
        }
        Some(TeeMainSpec::new(self.d, self.t, self.l, bs))
    }
    fn piece(&self, i: usize) -> Option<tee::BranchPieceResult> {
        let (_, d, t, h, _, dir, side, e) = &self.branches[i];
        let se = tee::signed_offset(*e, *side, *dir).ok()?;
        piece_of(*d, *t, self.d, self.t, *h, se, Mount::SetOn)
    }
}

fn eval_s01(n: usize) -> Option<Got> {
    let e_main = || {
        main_of(
            1016.0,
            12.0,
            1500.0,
            vec![BranchOnMain::new("1", 610.0, 9.0, None, 0.0, 0.0)],
        )
    };
    let e_br = |mount| piece_of(610.0, 9.0, 1016.0, 12.0, 900.0, 0.0, mount);
    match n {
        1 => num(e_br(Mount::SetOn)?.circ),
        2 => num(e_br(Mount::SetOn)?.marks[0].len),
        3 => num(e_br(Mount::SetOn)?.marks[4].len),
        4 => num(e_br(Mount::SetOn)?.marks[2].len),
        5 => num(e_main()?.plates[0].w),
        6 => num(e_main()?.holes[0].hole_w),
        7 => num(e_main()?.holes[0].hole_h),
        8 => num(e_main()?.holes[0].hole_u),
        9 => num(e_main()?.holes[0].hole_x),
        17 => num(e_br(Mount::SetIn)?.marks[0].len),
        18 => num(e_br(Mount::SetIn)?.marks[4].len),
        19 => {
            let mut s = TeeMainSpec::new(
                1016.0,
                12.0,
                1500.0,
                vec![BranchOnMain::new("1", 610.0, 9.0, None, 0.0, 0.0)],
            );
            s.mount = Mount::SetIn;
            num(main_with(&s)?.holes[0].hole_w)
        }
        21..=25 => {
            let f = piece_of(1016.0, 12.0, 1016.0, 12.0, 1000.0, 0.0, Mount::SetOn)?;
            num(1000.0 - f.marks[n - 21].len)
        }
        27 => flag(tee_refused_with(
            tee::branch_piece(&BranchPieceSpec {
                d: 1016.0,
                t: 12.0,
                main_d: 1016.0,
                main_t: 12.0,
                h: 1000.0,
                e: 0.0,
                mount: Mount::SetIn,
                angle_deg: None,
            }),
            "tee_branch_too_large_to_insert",
        )),
        // 10-12 pad, 13-16 / 20 / 26 check list of the order layer
        _ => None,
    }
}

fn eval_s02(n: usize) -> Option<Got> {
    match n {
        1 => num(piece_of(600.0, 0.001, 1000.0, 0.001, 900.0, 150.0, Mount::SetOn)?.cut_length),
        2 => {
            let r = main_of(
                1000.0,
                0.001,
                1500.0,
                vec![BranchOnMain::new("1", 600.0, 0.001, None, 0.0, 150.0)],
            )?;
            num(hole_perimeter(&r.holes[0]))
        }
        _ => None,
    }
}

fn eval_s03(n: usize) -> Option<Got> {
    let g = read_tee("G_tee_claude.json", |_| {});
    match n {
        1 => num(g.piece(0)?.marks[0].len),
        2 => num(tee::tee_main(&g.spec()?).ok()?.holes[0].hole_x),
        _ => None,
    }
}

fn eval_s05(n: usize) -> Option<Got> {
    match n {
        3 | 4 => flag(tee_refused_with(
            tee::tee_main(&TeeMainSpec::new(
                1000.0,
                10.0,
                2000.0,
                vec![br("1", 600.0, 10.0, 200.0, 0.0, 0.0)],
            )),
            "tee_hole_beyond_pipe_end",
        )),
        7 => num(piece_of(200.0, 10.0, 1000.0, 10.0, 495.0, 200.0, Mount::SetOn)?.shortest),
        _ => None,
    }
}

fn eval_s10(n: usize) -> Option<Got> {
    let one = || {
        main_of(
            1016.0,
            12.0,
            1500.0,
            vec![br("1", 610.0, 9.0, 750.0, 0.0, 0.0)],
        )
    };
    let cross2 = |d1: f64, d2: f64| {
        main_of(
            1016.0,
            12.0,
            1500.0,
            vec![
                br("a", 610.0, 9.0, 750.0, d1, 0.0),
                br("b", 610.0, 9.0, 750.0, d2, 0.0),
            ],
        )
    };
    let e3 = (609.6 - 216.3) / 2.0;
    let e4 = (1016.0 - 216.3) / 2.0;
    let drain = || {
        main_of(
            609.6,
            6.0,
            1500.0,
            vec![br("d", 216.3, 5.8, 750.0, 90.0, e3)],
        )
    };
    let valve = |dir2: f64, e2: f64, mirror: bool, seams: u32| {
        let mut s = TeeMainSpec::new(
            1016.0,
            9.0,
            2500.0,
            vec![
                br("v", 609.6, 6.0, 1000.0, 0.0, 0.0),
                br("d", 216.3, 5.8, 2100.0, dir2, e2),
            ],
        );
        s.seam_mirror = mirror;
        s.seams = seams;
        main_with(&s)
    };
    match n {
        1 => num(one()?.holes[0].hole_w),
        2 => num(one()?.holes[0].hole_h),
        3 => num(one()?.holes[0].hole_u),
        4 => num(one()?.mass_kg),
        5..=16 => {
            let (i, which) = ((n - 5) / 3, (n - 5) % 3);
            let dir = [45.0, 90.0, 180.0, 270.0][i];
            let r = main_of(
                1016.0,
                12.0,
                1500.0,
                vec![br("1", 610.0, 9.0, 750.0, dir, 0.0)],
            )?;
            num(match which {
                0 => r.holes[0].hole_u,
                1 => r.holes[0].hole_w,
                _ => r.moved_deg,
            })
        }
        // 17-20: t = 1e-9 in ZK; NF's smallest wall is 1e-6 mm (derived inputs, never counted)
        17..=20 => {
            let (rr, r_b, e, dir) = [
                (500.0, 300.0, 0.0, 0.0),
                (500.0, 300.0, 150.0, 90.0),
                (500.0, 100.0, 400.0, 90.0),
                (500.0, 300.0, -150.0, 200.0),
            ][n - 17];
            let r = main_of(
                2.0 * rr,
                1e-6,
                3000.0,
                vec![br("1", 2.0 * r_b, 1e-6, 1500.0, dir, e)],
            )?;
            num(hole_perimeter(&r.holes[0]))
        }
        21 => num(cross2(0.0, 180.0)?.holes.len() as f64),
        22 => num(cross2(0.0, 180.0)?.holes[0].hole_u),
        23 => num(cross2(0.0, 180.0)?.holes[1].hole_u),
        24 => num(cross2(0.0, 180.0)?.holes[1].hole_w),
        25 => num(cross2(0.0, 180.0)?.gaps[0].gap),
        26 => num(cross2(0.0, 180.0)?.holes[1].seam_dist),
        27 => flag(!cross2(0.0, 180.0)?.cross_seam),
        28 => num(cross2(0.0, 180.0)?.mass_kg),
        29 => num(cross2(90.0, 270.0)?.holes[0].hole_u),
        30 => num(cross2(90.0, 270.0)?.holes[1].hole_u),
        31 => num(cross2(90.0, 270.0)?.holes[1].seam_dist),
        32 => num(cross2(0.0, 90.0)?.gaps[0].gap),
        33 => num(cross2(0.0, 90.0)?.gaps[0].weld),
        35 => num(drain()?.holes[0].hole_w),
        36 => num(drain()?.holes[0].hole_h),
        37 => num(drain()?.holes[0].hole_u),
        38 => num(drain()?.holes[0].from_deg),
        39 => num(drain()?.holes[0].to_deg),
        40 => num(drain()?.holes[0].edge),
        41 => num(drain()?.holes[0].seam_dist),
        42 => num(drain()?.mass_kg),
        43..=46 => {
            let p = piece_of(216.3, 5.8, 609.6, 6.0, 500.0, e3, Mount::SetOn)?;
            num(match n {
                43 => p.marks[0].len,
                44 => p.marks[4].len,
                45 => p.marks[8].len,
                _ => p.mass_kg,
            })
        }
        47..=55 => {
            let r = valve(90.0, e4, false, 1)?;
            num(match n {
                47 => r.holes[0].hole_w,
                48 => r.holes[0].hole_h,
                49 => r.holes[0].hole_u,
                50 => r.holes[1].hole_w,
                51 => r.holes[1].hole_u,
                52 => r.holes[1].edge,
                53 => r.holes[1].seam_dist,
                54 => r.gaps[0].gap,
                _ => r.mass_kg,
            })
        }
        56 => {
            let r = valve(270.0, -e4, false, 1)?;
            flag(r.cross_seam && r.holes[1].cross_seam)
        }
        57 => num(valve(270.0, -e4, false, 1)?.holes[1].seam_dist_alt),
        58 => flag(!valve(270.0, -e4, true, 1)?.cross_seam),
        59 => num(valve(270.0, -e4, true, 1)?.holes[0].hole_u),
        60 => num(valve(270.0, -e4, true, 1)?.holes[1].hole_u),
        61 | 62 => {
            let p = piece_of(609.6, 6.0, 1016.0, 9.0, 800.0, 0.0, Mount::SetOn)?;
            num(if n == 61 {
                p.marks[0].len
            } else {
                p.marks[4].len
            })
        }
        63 | 64 => {
            let p = piece_of(216.3, 5.8, 1016.0, 9.0, 670.0, e4, Mount::SetOn)?;
            num(if n == 63 {
                p.marks[0].len
            } else {
                p.marks[8].len
            })
        }
        65 => num(piece_of(609.6, 6.0, 1016.0, 9.0, 800.0, 0.0, Mount::SetOn)?.mass_kg),
        66 => num(piece_of(216.3, 5.8, 1016.0, 9.0, 670.0, e4, Mount::SetOn)?.mass_kg),
        67 | 68 => {
            let mut s = TeeMainSpec::new(
                1016.0,
                9.0,
                1655.0,
                vec![
                    br("v", 609.6, 6.0, 1000.0, 0.0, 0.0),
                    br("d", 216.3, 5.8, 1000.0, 90.0, e4),
                ],
            );
            s.seams = 1;
            let r = main_with(&s)?;
            num(if n == 67 { r.gaps[0].gap } else { r.mass_kg })
        }
        69..=74 => {
            let r = main_of(
                1219.2,
                11.0,
                2400.0,
                vec![
                    br("m", 609.6, 6.0, 800.0, 0.0, 0.0),
                    br("a", 114.3, 4.5, 1500.0, 0.0, 0.0),
                ],
            )?;
            num(match n {
                69 => r.holes[0].hole_w,
                70 => r.holes[0].hole_h,
                71 => r.holes[1].hole_w,
                72 => r.holes[1].hole_h,
                73 => r.gaps[0].gap,
                _ => r.mass_kg,
            })
        }
        76..=80 => {
            let rt = read_tee("H_branch_drain.json", |_| {});
            match n {
                76 => flag(tee::tee_main(&rt.spec()?).is_ok()),
                77 => num(tee::tee_main(&rt.spec()?).ok()?.holes[0].hole_w),
                78 => num(tee::tee_main(&rt.spec()?).ok()?.holes[0].hole_u),
                79 => num(tee::tee_main(&rt.spec()?).ok()?.mass_kg),
                _ => num(rt.piece(0)?.marks[0].len),
            }
        }
        // derived inputs: the edited readings (never counted)
        85 => {
            let rt = read_tee("H_branch_drain.json", |v| {
                v["parts"][1]["params"]["dir"]["value"] = 270.into()
            });
            num(tee::tee_main(&rt.spec()?).ok()?.holes[0].hole_w)
        }
        88 => {
            let rt = read_tee("H_branch_drain.json", |v| {
                v["parts"][1]["params"]["e_side"]["value"] = 90.into()
            });
            num(rt.spec().map_or(0.0, |_| 1.0))
        }
        92..=94 => {
            let rt = read_tee("H_branch_valve.json", |_| {});
            let r = tee::tee_main(&rt.spec()?).ok()?;
            num(match n {
                92 => r.holes[0].hole_u,
                93 => r.holes[1].hole_u,
                _ => r.mass_kg,
            })
        }
        97 | 98 => {
            let rt = read_tee("H_branch_valve.json", |_| {});
            num(rt.piece(if n == 97 { 0 } else { 1 })?.marks[0].len)
        }
        102 => num(valve(90.0, e4, false, 2)?.holes[1].plate as f64),
        103 => num(valve(90.0, e4, false, 2)?.holes[1].hole_u),
        104 => num(valve(90.0, e4, false, 2)?.holes[0].pts.len() as f64),
        110 => {
            let rt = read_tee("H_branch_valve.json", |v| {
                v["parts"][2]["params"]["dir"]["value"] = 270.into()
            });
            let mut s = rt.spec()?;
            s.seam_mirror = true;
            num(tee::tee_main(&s).ok()?.holes[1].hole_u)
        }
        115 => {
            let rt = read_tee("H_branch_cross.json", |_| {});
            num(tee::tee_main(&rt.spec()?).ok()?.mass_kg)
        }
        118 => {
            let rt = read_tee("H_branch_cross.json", |_| {});
            num(tee::tee_main(&rt.spec()?).ok()?.holes[1].hole_u)
        }
        120 => num(cross2(0.0, 90.0)?.gaps[0].gap),
        121 => flag(tee_refused_with(
            tee::tee_main(&TeeMainSpec::new(
                1016.0,
                12.0,
                1500.0,
                vec![
                    br("a", 610.0, 9.0, 750.0, 0.0, 0.0),
                    br("b", 610.0, 9.0, 750.0, 30.0, 0.0),
                ],
            )),
            "tee_holes_overlap",
        )),
        127 => num(one()?.holes[0].hole_u),
        128 => num(one()?.holes.len() as f64),
        _ => None,
    }
}

/// Lane S1-L7a: per section report of the tee engine. Computed cases must be within ZK's tolerance (NG fails the test);
/// cases that need pads, the order layer's checks, stock or the DXF writer stay PENDING.
#[test]
fn real_engine_status_tee() {
    let (cases, _) = load_cases();
    let mut tot = RealReport::default();
    for sec in ["s01", "s02", "s03", "s04", "s05", "s10"] {
        let part: Vec<Case> = cases.iter().filter(|c| c.section == sec).cloned().collect();
        let r = run_real(&RustShapes, &part);
        println!(
            "{sec}: {} OK, {} NG, {} PENDING, {} derived (not counted) of {}",
            r.ok,
            r.ng,
            r.pending,
            r.derived_uncounted,
            part.len()
        );
        for l in &r.ng_lines {
            println!("{l}");
        }
        tot.ok += r.ok;
        tot.ng += r.ng;
        tot.pending += r.pending;
        tot.derived_uncounted += r.derived_uncounted;
    }
    println!(
        "lane L7a s01-s05+s10: {} OK, {} NG, {} PENDING, {} derived uncounted",
        tot.ok, tot.ng, tot.pending, tot.derived_uncounted
    );
    assert_eq!(tot.ng, 0, "NG cases in the tee sections");
    assert!(tot.ok >= 95, "only {} tee cases computed", tot.ok);
    assert_eq!(tot.derived_uncounted, 27, "s01 1 + s05 1 + s10 25");
}

/// The derived tee cases are computed (not just skipped) and agree with ZK's number; they remain uncounted.
#[test]
fn derived_tee_cases_are_computed_but_not_counted() {
    let (cases, _) = load_cases();
    let mut computed = 0;
    for c in cases
        .iter()
        .filter(|c| ["s01", "s05", "s10"].contains(&c.section.as_str()) && !c.derived.is_empty())
    {
        let got = RustShapes.eval(&key_of(c));
        if got.is_some() {
            computed += 1;
            assert_ne!(
                judge(c, got),
                Verdict::Ng,
                "derived case {} disagrees",
                c.id
            );
        }
    }
    println!("derived tee cases computed (still uncounted): {computed}");
    assert!(computed >= 8);
}

// ======================================================================================================
// Lane S1-L7b: section s11 (test_build.mjs:416-499, lateral branch). Every input value below is written from the ZK
// test source; every answer is computed by nf_core::tee (and tee_pad for the pad cases).
// ======================================================================================================

/// (main D, main t, d, t, H, L, x0, e, angle, mount) of the three lateral examples (test_build.mjs:430-436).
#[allow(clippy::type_complexity)]
const LAT_EX: [(f64, f64, f64, f64, f64, f64, f64, f64, f64, Mount); 3] = [
    (
        1016.0,
        12.0,
        1016.0,
        12.0,
        1400.0,
        2400.0,
        900.0,
        0.0,
        45.0,
        Mount::SetOn,
    ),
    (
        1016.0,
        12.0,
        610.0,
        9.0,
        1200.0,
        2000.0,
        700.0,
        0.0,
        45.0,
        Mount::SetOn,
    ),
    (
        1016.0,
        12.0,
        406.4,
        9.0,
        1000.0,
        1600.0,
        600.0,
        150.0,
        60.0,
        Mount::SetIn,
    ),
];

#[allow(clippy::too_many_arguments)]
fn lat_piece(
    d: f64,
    t: f64,
    main_d: f64,
    main_t: f64,
    h: f64,
    e: f64,
    ang: Option<f64>,
    mount: Mount,
) -> Result<tee::BranchPieceResult, nf_core::shapes::ShapeRefusal> {
    tee::branch_piece(&BranchPieceSpec {
        d,
        t,
        main_d,
        main_t,
        h,
        e,
        mount,
        angle_deg: ang,
    })
}

#[allow(clippy::type_complexity)]
fn lat_main(
    (dm, tm, d, t, _h, l, x0, e, ang, mount): (
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        Option<f64>,
        f64,
        Option<f64>,
        Mount,
    ),
) -> Result<TeeMainResult, nf_core::shapes::ShapeRefusal> {
    let mut b = BranchOnMain::new("1", d, t, x0, 0.0, e);
    b.angle_deg = ang;
    let mut sp = TeeMainSpec::new(dm, tm, l, vec![b]);
    sp.mount = mount;
    tee::tee_main(&sp)
}

fn ex_main(i: usize) -> Option<TeeMainResult> {
    let (dm, tm, d, t, h, l, x0, e, a, m) = LAT_EX[i];
    lat_main((dm, tm, d, t, h, l, Some(x0), e, Some(a), m)).ok()
}
fn ex_piece(i: usize) -> Option<tee::BranchPieceResult> {
    let (dm, tm, d, t, h, _l, _x0, e, a, m) = LAT_EX[i];
    lat_piece(d, t, dm, tm, h, e, Some(a), m).ok()
}

/// Independent brute force of the cut position: the wall (radii ri..ro) cut in 4000 steps (ZK check 2, written here
/// from the geometry, not from `cut_tau`).
fn tau_brute(rc: f64, ri: f64, ro: f64, e: f64, th: f64, a_deg: f64) -> f64 {
    let a = a_deg.to_radians();
    let (s, c) = (th.sin(), th.cos());
    let mut best = f64::NEG_INFINITY;
    for j in 0..=4000 {
        let rho = ri + (ro - ri) * j as f64 / 4000.0;
        let z = e + rho * s;
        if z.abs() <= rc {
            best = best.max(((rc * rc - z * z).sqrt() + rho * c * a.cos()) / a.sin());
        }
    }
    best
}

fn eval_s11(n: usize) -> Option<Got> {
    if (1..=75).contains(&n) {
        let (i, k) = ((n - 1) / 25, (n - 1) % 25 + 1);
        return match k {
            1 => num(ex_piece(i)?.circ),
            2..=17 => num(ex_piece(i)?.marks[k - 2].len),
            18 => num(ex_piece(i)?.shortest),
            19 => num(ex_piece(i)?.longest),
            20 => num(ex_piece(i)?.mass_kg),
            21 => num(ex_main(i)?.holes[0].hole_w),
            22 => num(ex_main(i)?.holes[0].xmin),
            23 => num(ex_main(i)?.holes[0].xmax),
            24 => {
                let h = &ex_main(i)?.holes[0];
                num(h.xmax - h.xmin)
            }
            _ => num(ex_main(i)?.mass_kg),
        };
    }
    let plate0 = |r: f64, rr: f64, e: f64, a: f64| {
        let b = lat_piece(
            2.0 * rr,
            1e-6,
            2.0 * r,
            1e-6,
            5.0 * r,
            e,
            Some(a),
            Mount::SetOn,
        )
        .ok()?;
        let m = lat_main((
            2.0 * r,
            1e-6,
            2.0 * rr,
            1e-6,
            0.0,
            20.0 * r,
            Some(10.0 * r),
            e,
            Some(a),
            Mount::SetOn,
        ))
        .ok()?;
        Some((b.cut_length, hole_perimeter(&m.holes[0])))
    };
    match n {
        76 => num(ex_main(2)?.holes[0].marks[0][1]),
        77 => num(ex_main(2)?.holes[0].marks[8][1]),
        83 => num(lat_main((
            1016.0,
            12.0,
            610.0,
            9.0,
            0.0,
            2000.0,
            None,
            0.0,
            Some(45.0),
            Mount::SetOn,
        ))
        .ok()?
        .holes[0]
            .hole_x),
        84 => {
            let a = lat_piece(
                610.0,
                9.0,
                1016.0,
                12.0,
                900.0,
                150.0,
                Some(90.0),
                Mount::SetOn,
            )
            .ok()?;
            let b = lat_piece(610.0, 9.0, 1016.0, 12.0, 900.0, 150.0, None, Mount::SetOn).ok()?;
            flag(a.marks.iter().zip(&b.marks).all(|(p, q)| p.len == q.len))
        }
        85 => {
            let a = lat_main((
                1016.0,
                12.0,
                610.0,
                9.0,
                0.0,
                1500.0,
                Some(750.0),
                150.0,
                Some(90.0),
                Mount::SetOn,
            ))
            .ok()?;
            let b = lat_main((
                1016.0,
                12.0,
                610.0,
                9.0,
                0.0,
                1500.0,
                Some(750.0),
                150.0,
                None,
                Mount::SetOn,
            ))
            .ok()?;
            let (ha, hb) = (&a.holes[0], &b.holes[0]);
            flag(
                ha.hole_w == hb.hole_w
                    && ha.hole_h == hb.hole_h
                    && ha.hole_u == hb.hole_u
                    && ha.hole_x == hb.hole_x,
            )
        }
        86..=89 => {
            let k = [0usize, 4, 8, 12][n - 86];
            num(lat_piece(
                610.0,
                9.0,
                1016.0,
                12.0,
                900.0,
                150.0,
                Some(89.99),
                Mount::SetOn,
            )
            .ok()?
            .marks[k]
                .len)
        }
        90 => num(plate0(508.0, 305.0, 150.0, 60.0)?.0),
        91 => num(plate0(508.0, 305.0, 150.0, 60.0)?.1),
        92 => num(plate0(1.0, 1.0, 0.0, 45.0)?.0),
        93 => num(plate0(1.0, 1.0, 0.0, 45.0)?.1),
        94 => num(plate0(1.0, 0.5, 0.25, 45.0)?.0),
        95 => num(plate0(1.0, 0.5, 0.25, 45.0)?.1),
        96 => {
            let mut worst = 0.0f64;
            for (rc, ri, ro, e, a) in [
                (508.0, 296.0, 305.0, 0.0, 45.0),
                (508.0, 296.0, 305.0, 150.0, 60.0),
                (496.0, 194.2, 203.2, 150.0, 60.0),
                (508.0, 296.0, 305.0, 200.0, 30.0),
            ] {
                for k in 0..64 {
                    let th = 2.0 * std::f64::consts::PI * k as f64 / 64.0;
                    let want = tau_brute(rc, ri, ro, e, th, a);
                    worst = worst.max((tee::cut_tau(rc, ri, ro, e, th, Some(a)) - want).abs());
                }
            }
            num(worst)
        }
        // the reading H_lateral.json is the example 2 geometry (main 1016x12 L2000 x0 700 angle 45, branch 610x9 H1200)
        78..=82 => {
            let r = pad::pad(&lat_pad_spec(None, SplitCut::Cross)).ok()?;
            match n {
                78 => num(r.w),
                79 => num(r.h),
                80 => num(r.hole_w),
                81 => num(r.hole_h),
                _ => num(r.mass_kg),
            }
        }
        103 => num(pad::pad(&lat_pad_spec(None, SplitCut::Cross)).ok()?.mass_kg),
        99 => num(ex_piece(1)?.marks[0].len),
        100 => num(ex_piece(1)?.marks[12].len),
        101 => num(ex_main(1)?.holes[0].xmin),
        102 => num(ex_main(1)?.mass_kg),
        108 => num(ex_main(1)?.holes[0].hole_x),
        110 => num(lat_main((
            1016.0,
            12.0,
            610.0,
            9.0,
            0.0,
            2000.0,
            None,
            0.0,
            Some(45.0),
            Mount::SetOn,
        ))
        .ok()?
        .holes[0]
            .hole_x),
        109 => {
            let r = lat_main((
                1016.0,
                12.0,
                610.0,
                9.0,
                0.0,
                2000.0,
                None,
                0.0,
                Some(45.0),
                Mount::SetOn,
            ))
            .ok()?;
            flag(r.questions.contains(&tee::TEE_X_QUESTION))
        }
        112 => {
            // the corrected (sharp) angle is what the engine takes; a 135 deg input is refused, not converted
            num(ex_piece(1)?.marks[12].len)
        }
        113 => {
            let r = lat_main((
                1016.0,
                12.0,
                610.0,
                9.0,
                0.0,
                4000.0,
                Some(700.0),
                0.0,
                Some(25.0),
                Mount::SetOn,
            ))
            .ok()?;
            flag(r.has_hold("tee_lateral_angle_below_30"))
        }
        114 => {
            let m = lat_main((
                1016.0,
                12.0,
                610.0,
                9.0,
                0.0,
                4000.0,
                Some(700.0),
                0.0,
                Some(25.0),
                Mount::SetOn,
            ));
            let b = lat_piece(
                610.0,
                9.0,
                1016.0,
                12.0,
                2500.0,
                0.0,
                Some(25.0),
                Mount::SetOn,
            );
            flag(m.is_ok() && b.is_ok())
        }
        115 => flag(tee_refused_with(
            lat_piece(
                610.0,
                9.0,
                1016.0,
                12.0,
                1200.0,
                0.0,
                Some(0.0),
                Mount::SetOn,
            ),
            "tee_branch_angle_invalid",
        )),
        116 => num(lat_piece(
            610.0,
            9.0,
            1016.0,
            12.0,
            1200.0,
            0.0,
            Some(0.0),
            Mount::SetOn,
        )
        .map(|_| 1.0)
        .unwrap_or(0.0)),
        117 => flag(tee_refused_with(
            lat_piece(
                610.0,
                9.0,
                1016.0,
                12.0,
                600.0,
                0.0,
                Some(45.0),
                Mount::SetOn,
            ),
            "tee_branch_end_inside_main",
        )),
        118 => flag(tee_refused_with(
            lat_main((
                1016.0,
                12.0,
                610.0,
                9.0,
                0.0,
                2000.0,
                Some(-100.0),
                0.0,
                Some(45.0),
                Mount::SetOn,
            )),
            "tee_hole_beyond_pipe_end",
        )),
        119 => flag(
            lat_main((
                1016.0,
                12.0,
                610.0,
                9.0,
                0.0,
                2000.0,
                Some(-50.0),
                0.0,
                Some(45.0),
                Mount::SetOn,
            ))
            .is_ok(),
        ),
        120 => flag(tee_refused_with(
            tee::rib_on_branch(Some(45.0)),
            "tee_rib_on_lateral_unsupported",
        )),
        122 => num(ex_piece(2)?.marks[4].len),
        123 => num(ex_main(2)?.holes[0].xmax),
        124 => {
            let (dm, tm, d, t, h, l, x0, e, a, _) = LAT_EX[2];
            flag(
                lat_piece(d, t, dm, tm, h, e, Some(a), Mount::SetOn).is_ok()
                    && lat_main((dm, tm, d, t, h, l, Some(x0), e, Some(a), Mount::SetOn)).is_ok(),
            )
        }
        _ => None,
    }
}

#[test]
fn real_engine_status_lateral() {
    let (cases, _) = load_cases();
    let part: Vec<Case> = cases
        .iter()
        .filter(|c| c.section == "s11")
        .cloned()
        .collect();
    let r = run_real(&RustShapes, &part);
    println!(
        "s11: {} OK, {} NG, {} PENDING, {} derived (not counted) of {}",
        r.ok,
        r.ng,
        r.pending,
        r.derived_uncounted,
        part.len()
    );
    for l in &r.ng_lines {
        println!("{l}");
    }
    assert_eq!(r.ng, 0, "NG cases in s11");
}

use nf_core::tee_pad::{self as pad, PadOutline, PadSpec, SplitCut};

/// The lateral example 2 pad (test_build.mjs:446-447): main 1016, branch 610, width 150, e 0, 45 deg, t 12.
fn lat_pad_spec(split: Option<u32>, cut: SplitCut) -> PadSpec {
    PadSpec {
        main_d: 1016.0,
        t: 12.0,
        d: 610.0,
        e: 0.0,
        angle_deg: Some(45.0),
        outline: PadOutline::Width(150.0),
        split,
        cut,
    }
}

// ======================================================================================================
// Section s12 (test_build.mjs:502-581, pad split) and s13 (583-621, review cases).
// ======================================================================================================

/// The four pad examples of test_build.mjs:513-526: (main D, d, outline, e, t) and the (split, cut) lists.
fn pad_ex(i: usize) -> (PadSpec, &'static [(u32, SplitCut)]) {
    use SplitCut::{Along, Cross};
    const L1: [(u32, SplitCut); 3] = [(2, Cross), (2, Along), (4, Cross)];
    const L2: [(u32, SplitCut); 4] = [(2, Cross), (2, Along), (3, Cross), (4, Cross)];
    const L4: [(u32, SplitCut); 2] = [(2, Cross), (2, Along)];
    let mk = |main_d, d, outline, e, t| PadSpec {
        main_d,
        t,
        d,
        e,
        angle_deg: None,
        outline,
        split: None,
        cut: Cross,
    };
    match i {
        0 => (
            mk(1016.0, 610.0, PadOutline::Circle(1000.0), 0.0, 12.0),
            &L1,
        ),
        1 => (
            mk(1016.0, 610.0, PadOutline::Width(150.0), 150.0, 12.0),
            &L2,
        ),
        2 => (mk(2438.4, 1524.0, PadOutline::Width(100.0), 0.0, 9.0), &L1),
        _ => (
            mk(1016.0, 406.4, PadOutline::Circle(700.0), 150.0, 12.0),
            &L4,
        ),
    }
}

fn eval_s12(n: usize) -> Option<Got> {
    // blocks of ids: example start, then one block per (split, cut): pieces P (1 for the perpendicular cross two-way,
    // else the split count), ids = 1 count + 3 P + split seams + angle, sum, n, gross, closed-shape flag
    if n <= 188 {
        let mut at = 1usize;
        for ex in 0..4 {
            let (base, list) = pad_ex(ex);
            if n == at {
                return num(pad::pad(&base).ok()?.mass_kg);
            }
            at += 1;
            for &(sp, cut) in list {
                let p_cnt = if sp == 2 && cut == SplitCut::Cross {
                    1
                } else {
                    sp as usize
                };
                let len = 1 + 3 * p_cnt + sp as usize + 5;
                if n >= at && n < at + len {
                    let k = n - at;
                    let r = pad::pad(&PadSpec {
                        split: Some(sp),
                        cut,
                        ..base
                    })
                    .ok()?;
                    let pc = r.pieces.len();
                    return match k {
                        0 => num(pc as f64),
                        k if k <= 3 * pc => {
                            let q = &r.pieces[(k - 1) / 3];
                            match (k - 1) % 3 {
                                0 => num(q.w),
                                1 => num(q.h),
                                _ => num(q.mass_kg),
                            }
                        }
                        k if k <= 3 * pc + sp as usize => num(r.seams[k - 1 - 3 * pc]),
                        k => match k - (3 * pc + sp as usize) {
                            1 => num(r.seam_angle_deg),
                            2 => num(r.pieces_mass_kg),
                            3 => num(f64::from(r.split)),
                            4 => num(r.gross_box_mass_kg),
                            _ => flag(r.pieces.iter().all(|q| {
                                let (a, b) = (q.pts[0], q.pts[q.pts.len() - 1]);
                                (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9
                            })),
                        },
                    };
                }
                at += len;
            }
        }
        return None;
    }
    let ex1 = |sp: u32, cut| {
        pad::pad(&PadSpec {
            split: Some(sp),
            cut,
            ..pad_ex(0).0
        })
        .ok()
    };
    let lat = |sp: u32| pad::pad(&lat_pad_spec(Some(sp), SplitCut::Cross)).ok();
    match n {
        189 => flag(
            ex1(2, SplitCut::Cross)?
                .cuts
                .iter()
                .all(|c| c[0][1].abs() < 1e-9 && c[1][1].abs() < 1e-9),
        ),
        191..=193 => num(lat((n - 189) as u32)?.pieces.len() as f64),
        195 => num(ex1(2, SplitCut::Cross)?.pieces[0].w),
        196 => num(ex1(2, SplitCut::Cross)?.pieces[0].h),
        197 => num(ex1(2, SplitCut::Cross)?.pieces_mass_kg),
        198 => num(f64::from(ex1(2, SplitCut::Cross)?.split)),
        199 => num(ex1(2, SplitCut::Cross)?.gross_box_mass_kg),
        205 => num(ex1(2, SplitCut::Along)?.pieces[0].w),
        206 => num(ex1(2, SplitCut::Along)?.pieces[0].h),
        207 => num(f64::from(ex1(2, SplitCut::Along)?.split)),
        209 => num(f64::from(ex1(4, SplitCut::Cross)?.split)),
        210 => num(ex1(4, SplitCut::Cross)?.pieces_mass_kg),
        211 => num(ex1(4, SplitCut::Cross)?
            .pieces
            .iter()
            .map(|q| q.w)
            .fold(0.0, f64::max)),
        215 => num(f64::from(pad::pad(&pad_ex(0).0).ok()?.split)),
        216 => num(pad::pad(&pad_ex(0).0).ok()?.w),
        219 => num(f64::from(pad::pad(&pad_ex(0).0).ok()?.split)),
        221 => flag(tee_refused_with(
            pad::pad(&PadSpec {
                split: Some(13),
                ..pad_ex(0).0
            }),
            "pad_split_out_of_range",
        )),
        223 | 224 => {
            let r = pad::pad(&PadSpec {
                split: Some(2),
                cut: SplitCut::Cross,
                ..pad_ex(1).0
            })
            .ok()?;
            num(if n == 223 {
                r.pieces[0].w
            } else {
                r.pieces[0].h
            })
        }
        225 => num(f64::from(lat(2)?.split)),
        226 => num(lat(2)?.pieces_mass_kg),
        _ => None,
    }
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

fn eval_s13(n: usize) -> Option<Got> {
    let deg = 180.0 / std::f64::consts::PI;
    if (1..=4).contains(&n) {
        // (D, T, d, tb, e, mount) of test_build.mjs:589-592
        let rows = [
            (609.6, 6.0, 216.3, 5.8, (609.6 - 216.3) / 2.0, Mount::SetOn),
            (
                2641.6,
                16.0,
                609.6,
                9.0,
                (2641.6 - 609.6) / 2.0,
                Mount::SetOn,
            ),
            (1016.0, 12.0, 216.3, 5.8, 300.0, Mount::SetIn),
            (1016.0, 12.0, 610.0, 9.0, 150.0, Mount::SetOn),
        ];
        let (dm, tm, d, tb, e, mount) = rows[n - 1];
        let mut b = BranchOnMain::new("1", d, tb, Some(2.0 * d + 500.0), 0.0, e);
        b.angle_deg = None;
        let mut sp = TeeMainSpec::new(dm, tm, 4.0 * d + 1000.0, vec![b]);
        sp.mount = mount;
        return num(tee::tee_main(&sp).ok()?.holes[0].from_deg);
    }
    let pads: [(f64, f64, f64, f64, Option<f64>, f64); 6] = [
        (1016.0, 216.3, 60.0, 399.85, None, 6.0),
        (1016.0, 406.4, 100.0, (1016.0 - 406.4) / 2.0, None, 12.0),
        (609.6, 216.3, 60.0, (609.6 - 216.3) / 2.0, None, 6.0),
        (1016.0, 1016.0, 150.0, 0.0, None, 12.0),
        (1016.0, 1016.0, 150.0, 0.0, Some(45.0), 12.0),
        (1016.0, 610.0, 150.0, 150.0, None, 12.0),
    ];
    let pad_of = |i: usize| {
        let (dm, d, b, e, ang, t) = pads[i];
        pad::pad(&PadSpec {
            main_d: dm,
            t,
            d,
            e,
            angle_deg: ang,
            outline: PadOutline::Width(b),
            split: None,
            cut: SplitCut::Cross,
        })
        .ok()
    };
    match n {
        5..=16 => {
            let (i, which) = ((n - 5) / 2, (n - 5) % 2);
            let r = pad_of(i)?;
            if which == 1 {
                return flag(r.outline.len() - 1 == r.hole.len());
            }
            let mut mn = f64::INFINITY;
            for j in 0..r.outline.len() - 1 {
                for k in 0..8 {
                    let f = f64::from(k) / 8.0;
                    let q = [
                        r.outline[j][0] + (r.outline[j + 1][0] - r.outline[j][0]) * f,
                        r.outline[j][1] + (r.outline[j + 1][1] - r.outline[j][1]) * f,
                    ];
                    for h in 0..r.hole.len() {
                        mn = mn.min(seg_dist(q, r.hole[h], r.hole[(h + 1) % r.hole.len()]));
                    }
                }
            }
            flag(mn >= pads[i].2 - 0.1)
        }
        17 => {
            let r = pad_of(0)?;
            let _ = deg;
            num(r.hole.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min))
        }
        22 | 23 => {
            let mut b = BranchOnMain::new("1", 610.0, 9.0, None, 0.0, 1200.0);
            b.angle_deg = Some(45.0);
            let sp = TeeMainSpec::new(1016.0, 12.0, 2000.0, vec![b]);
            flag(tee_refused_with(tee::tee_main(&sp), "tee_hole_off_pipe"))
        }
        _ => None,
    }
}

#[test]
fn real_engine_status_pads_and_review() {
    let (cases, _) = load_cases();
    let mut tot = RealReport::default();
    for sec in ["s11", "s12", "s13"] {
        let part: Vec<Case> = cases.iter().filter(|c| c.section == sec).cloned().collect();
        let r = run_real(&RustShapes, &part);
        println!(
            "{sec}: {} OK, {} NG, {} PENDING, {} derived (not counted) of {}",
            r.ok,
            r.ng,
            r.pending,
            r.derived_uncounted,
            part.len()
        );
        for l in &r.ng_lines {
            println!("{l}");
        }
        tot.ok += r.ok;
        tot.ng += r.ng;
        tot.pending += r.pending;
        tot.derived_uncounted += r.derived_uncounted;
    }
    {
        for c in cases
            .iter()
            .filter(|c| ["s08", "s11", "s12", "s13"].contains(&c.section.as_str()))
        {
            if c.derived.is_empty() && RustShapes.eval(&key_of(c)).is_none() {
                println!("PENDING {} {}", c.id, c.name);
            }
        }
    }
    println!(
        "lane L7b s11+s12+s13: {} OK, {} NG, {} PENDING, {} derived uncounted",
        tot.ok, tot.ng, tot.pending, tot.derived_uncounted
    );
    assert_eq!(tot.ng, 0, "NG cases in the L7b sections");
}
