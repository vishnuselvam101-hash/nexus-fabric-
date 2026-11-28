//! NF CAD phase 1 tests: fixture DXFs measured against closed-form values, refusals, a fuzz-style table,
//! the quality re-check, and the AMOEBA tool bindings.

use nf_cad::geom::{Geom, Vtx};
use nf_cad::{Measure, open, quality_checks};

const BASIC: &[u8] = include_bytes!("fixtures/basic.dxf");
const TOL: f64 = 1e-9;
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < TOL
}
fn pi() -> f64 {
    std::f64::consts::PI
}

fn val(d: &nf_cad::Drawing, m: Measure) -> f64 {
    let v = d.measure(&m).expect("measure");
    assert_eq!(v.len(), 1, "one value");
    v[0].value
}

#[test]
fn header_layers_units_and_counts() {
    let d = open(BASIC).unwrap();
    assert_eq!(d.version, "AC1009");
    assert_eq!(d.units, "millimeter");
    assert_eq!(d.entities.len(), 7);
    let cut = d.layers.iter().find(|l| l.name == "CUT").unwrap();
    assert_eq!((cut.colour, cut.on, cut.count), (1, true, 5));
    let mark = d.layers.iter().find(|l| l.name == "MARK").unwrap();
    assert_eq!((mark.colour, mark.on, mark.count), (3, false, 2));
    assert_eq!(d.on_layer("CUT"), vec!["E0", "E1", "E2", "E3", "E4"]);
}

#[test]
fn line_and_distance_are_exact() {
    let d = open(BASIC).unwrap();
    assert!(near(val(&d, Measure::Length { entity: "E0".into() }), 5.0));
    assert!(near(val(&d, Measure::Distance { from: [0.0, 0.0], to: [3.0, 4.0] }), 5.0));
    let v = d.measure(&Measure::Vector { from: [0.0, 0.0], to: [3.0, 4.0] }).unwrap();
    assert!(near(v[0].value, 3.0) && near(v[1].value, 4.0));
    assert!(near(v[2].value, 4.0_f64.atan2(3.0).to_degrees()));
}

#[test]
fn circle_and_arc_match_closed_forms() {
    let d = open(BASIC).unwrap();
    assert!(near(val(&d, Measure::Length { entity: "E1".into() }), 4.0 * pi()));
    assert!(near(val(&d, Measure::Area { entity: "E1".into() }), 4.0 * pi()));
    assert!(near(val(&d, Measure::ArcRadius { entity: "E1".into() }), 2.0));
    // quarter arc of radius 2: 2 * pi / 2
    assert!(near(val(&d, Measure::Length { entity: "E2".into() }), pi()));
    let sweep = d.measure(&Measure::ArcRadius { entity: "E2".into() }).unwrap();
    assert!(near(sweep[1].value, 90.0));
}

#[test]
fn closed_polyline_area_and_length() {
    let d = open(BASIC).unwrap();
    assert!(near(val(&d, Measure::Area { entity: "E3".into() }), 1.0));
    assert!(near(val(&d, Measure::Length { entity: "E3".into() }), 4.0));
}

#[test]
fn bulge_segments_are_included_in_length_and_area() {
    let d = open(BASIC).unwrap();
    // semicircle of radius 1 (bulge 1) plus the closing chord of length 2
    assert!(near(val(&d, Measure::Length { entity: "E4".into() }), pi() + 2.0));
    assert!(near(val(&d, Measure::Area { entity: "E4".into() }), pi() / 2.0));
}

#[test]
fn open_polyline_has_length_and_no_area() {
    let d = open(BASIC).unwrap();
    assert!(near(val(&d, Measure::Length { entity: "E5".into() }), 7.0));
    assert!(d.measure(&Measure::Area { entity: "E5".into() }).is_err());
}

#[test]
fn angles_and_radius_on_wrong_entities_are_refused() {
    let d = open(BASIC).unwrap();
    assert!(near(
        val(&d, Measure::LineAngle { a: [[0.0, 0.0], [1.0, 0.0]], b: [[0.0, 0.0], [0.0, 1.0]] }),
        90.0
    ));
    assert!(near(val(&d, Measure::VertexAngle { at: [0.0, 0.0], a: [1.0, 0.0], b: [0.0, 1.0] }), 90.0));
    assert!(d.measure(&Measure::ArcRadius { entity: "E0".into() }).is_err());
    assert!(d.measure(&Measure::Length { entity: "E99".into() }).is_err());
}

#[test]
fn extents_include_arcs_and_bulges() {
    let d = open(BASIC).unwrap();
    // circle centre (10,0) r 2 gives y down to -2 and x up to 12; the bulge arc reaches x 22
    // the TEXT anchor at (0,-5) sets the lowest y
    assert_eq!(d.extents, Some([0.0, -5.0, 22.0, 12.0]));
}

#[test]
fn pick_selects_the_nearest_entity_within_tolerance() {
    let d = open(BASIC).unwrap();
    assert_eq!(d.pick([1.5, 2.0], 0.01).map(|e| e.id.as_str()), Some("E0"));
    assert_eq!(d.pick([12.0, 0.0], 0.01).map(|e| e.id.as_str()), Some("E1"));
    assert!(d.pick([100.0, 100.0], 0.5).is_none());
}

#[test]
fn units_are_unknown_when_not_declared() {
    let d = open(include_bytes!("fixtures/no_units.dxf")).unwrap();
    assert_eq!(d.units, "unknown");
    assert_eq!(d.entities.len(), 1);
}

#[test]
fn zumen_style_file_has_no_layer_table_and_reads_units_unknown() {
    let d = open(include_bytes!("fixtures/zumen_like.dxf")).unwrap();
    assert_eq!(d.units, "unknown");
    assert_eq!(d.layers.len(), 1);
    assert_eq!(d.layers[0].name, "CUT");
    assert!(near(val(&d, Measure::Length { entity: "E0".into() }), 2.0_f64.sqrt()));
    // the second LINE has no end point: kept, not measured, with a warning
    assert!(matches!(d.entity("E1").unwrap().geom, Geom::Unsupported));
    assert!(!d.entity("E1").unwrap().warnings.is_empty());
}

#[test]
fn refusals_are_typed_and_never_panic() {
    let big = vec![b' '; (8 << 20) + 1];
    assert_eq!(open(&big).unwrap_err().code, "too_large");
    let mut many = String::from("0\nSECTION\n2\nENTITIES\n");
    for _ in 0..(nf_cad::MAX_ENTITIES + 1) {
        many.push_str("0\nPOINT\n10\n1\n20\n1\n");
    }
    many.push_str("0\nENDSEC\n0\nEOF\n");
    let err = open(many.as_bytes()).unwrap_err();
    assert_eq!(err.code, "too_many_entities", "{}", err.message);
}

#[test]
fn fuzz_style_table_never_panics() {
    // every truncation of the fixture, and single-byte edits of it, must give a Drawing or a refusal
    for n in 0..BASIC.len() {
        let _ = open(&BASIC[..n]);
    }
    let edits: [u8; 8] = [0, b'\n', b'9', b'-', b'e', 0xff, b' ', b'.'];
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..4000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut v = BASIC.to_vec();
        let at = (seed >> 33) as usize % v.len();
        v[at] = edits[(seed >> 13) as usize % edits.len()];
        let _ = open(&v);
    }
    let table: [&[u8]; 6] = [
        b"0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\nnan\n20\n0\n40\n-1\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n90\n1\n10\n1e400\n20\n0\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nPOLYLINE\n70\n1\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nARC\n10\n0\n20\n0\n40\n0\n50\n0\n51\n0\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nTABLES\n0\nLAYER\n62\nx\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nTEXT\n10\n0\n20\n0\n40\n2\n0\nENDSEC\n",
    ];
    for t in table {
        let _ = open(t);
    }
}

#[test]
fn degenerate_geometry_is_measured_without_panic() {
    let text = b"0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n90\n3\n70\n1\n10\n0\n20\n0\n10\n0\n20\n0\n10\n0\n20\n0\n42\n1e308\n0\nENDSEC\n0\nEOF\n";
    let d = open(text).unwrap();
    let _ = d.measure(&Measure::Area { entity: "E0".into() });
    let _ = d.measure(&Measure::Length { entity: "E0".into() });
    assert!(matches!(d.entity("E0").unwrap().geom, Geom::Poly { .. }));
    let _ = Vtx { p: [0.0, 0.0], bulge: 0.0 };
}

#[test]
fn quality_gate_runs_all_fourteen_rules_and_fails_a_non_nf_file() {
    let checks = quality_checks(BASIC);
    assert_eq!(checks.len(), 14);
    let readable = checks.iter().find(|c| c.rule == "readable").unwrap();
    assert!(!readable.pass, "a non-NF file must not pass readable");
    assert!(!readable.findings.is_empty());
}

// ---------------------------------------------------------------- AMOEBA tool kinds

struct NoHands;
impl nf_amoeba_operator::operator::Hands for NoHands {
    fn call(&mut self, _tool: &str, _args: &serde_json::Value) -> Result<nf_amoeba_agent::agent::Answer, String> {
        Err("CAD tools never call the server".into())
    }
    fn take_binary(&mut self) -> Option<Vec<u8>> {
        None
    }
}

fn run_tool(
    reg: &nf_amoeba_operator::operator::Registry,
    st: &mut nf_cad::tools::CadState,
    tool: &str,
    args: serde_json::Value,
) -> Result<nf_amoeba_operator::operator::ToolOut, String> {
    let manifest = nf_amoeba_operator::operator::Manifest::from_json(&serde_json::json!({
        "order": 1, "project": 1, "units": [], "order_compute": false
    }))
    .unwrap();
    let mut hands = NoHands;
    let mut ctx = nf_amoeba_operator::operator::Ctx::new(&mut hands, &manifest);
    ctx.ext.insert(std::mem::take(st));
    let out = reg.run(&mut ctx, &nf_amoeba_operator::operator::Action::new(tool, args));
    *st = ctx.ext.get_mut::<nf_cad::tools::CadState>().map(std::mem::take).unwrap_or_default();
    out
}

#[test]
fn cad_tools_are_registered_typed_and_targeted() {
    use nf_amoeba_operator::events::Target;
    let mut reg = nf_amoeba_operator::operator::Registry::default();
    nf_cad::tools::register(&mut reg);
    for n in ["cad_open", "cad_layers", "cad_select", "cad_pick", "cad_measure", "cad_check"] {
        assert!(reg.names().contains(&n), "{n} is registered");
    }
    let mut st = nf_cad::tools::CadState::load(BASIC).unwrap();

    let out = run_tool(&reg, &mut st, "cad_measure", serde_json::json!({"kind": "length", "entity": "E0"})).unwrap();
    assert!((out.result[0]["value"].as_f64().unwrap() - 5.0).abs() < TOL);
    assert!(matches!(&out.events[0].target, Target::Cad { entity: Some(e), .. } if e == "E0"));

    let out = run_tool(&reg, &mut st, "cad_pick", serde_json::json!({"at": [1.5, 2.0], "tol": 0.01})).unwrap();
    assert_eq!(out.result["entity"], "E0");
    assert!(matches!(&out.events[0].target, Target::Cad { canvas: Some(_), .. }));

    let out = run_tool(&reg, &mut st, "cad_select", serde_json::json!({"layer": "CUT"})).unwrap();
    assert_eq!(out.result["count"], 5);
    assert_eq!(st.selection.len(), 5);

    let out = run_tool(&reg, &mut st, "cad_check", serde_json::json!({})).unwrap();
    assert_eq!(out.result.as_array().unwrap().len(), 14);
}

#[test]
fn cad_tool_arguments_are_refused_when_malformed() {
    let mut reg = nf_amoeba_operator::operator::Registry::default();
    nf_cad::tools::register(&mut reg);
    let mut st = nf_cad::tools::CadState::load(BASIC).unwrap();
    assert!(run_tool(&reg, &mut st, "cad_measure", serde_json::json!({"kind": "teleport"})).is_err());
    assert!(run_tool(&reg, &mut st, "cad_measure", serde_json::json!({"kind": "length", "entity": "E0", "extra": 1})).is_err());
    assert!(run_tool(&reg, &mut st, "cad_pick", serde_json::json!({"at": [0, 0], "tol": -1})).is_err());
    assert!(run_tool(&reg, &mut st, "cad_measure", serde_json::json!({"kind": "area", "entity": "E0"})).is_err());
    assert!(run_tool(&reg, &mut st, "cad_nope", serde_json::json!({})).is_err());
    // with no drawing open every tool refuses
    let mut empty = nf_cad::tools::CadState::default();
    assert!(run_tool(&reg, &mut empty, "cad_layers", serde_json::json!({})).is_err());
}

#[test]
fn hundred_thousand_entities_on_unique_layers_open_fast() {
    // Layer lookup and counts must be linear: 100k layers would cost ~1e10 compares with a scan per entity.
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for i in 0..nf_cad::MAX_ENTITIES {
        text.push_str(&format!("0\nLINE\n8\nL{i}\n10\n0\n20\n0\n11\n1\n21\n1\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    assert!(text.len() <= nf_cad::MAX_BYTES, "fixture is {} bytes", text.len());
    let start = std::time::Instant::now();
    let d = open(text.as_bytes()).expect("100k entities must open");
    let took = start.elapsed();
    assert_eq!(d.entities.len(), nf_cad::MAX_ENTITIES);
    assert_eq!(d.layers.len(), nf_cad::MAX_ENTITIES);
    assert!(d.layers.iter().all(|l| l.count == 1));
    assert!(took < std::time::Duration::from_secs(20), "took {took:?}");
}

#[test]
fn polyline_without_seqend_is_warned() {
    let d = open(b"0\nSECTION\n2\nENTITIES\n0\nPOLYLINE\n70\n0\n8\nA\n0\nVERTEX\n10\n0\n20\n0\n0\nLINE\n8\nB\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDSEC\n0\nEOF\n").unwrap();
    assert!(d.entities[0].warnings.iter().any(|w| w.contains("without SEQEND")), "{:?}", d.entities[0].warnings);
}
