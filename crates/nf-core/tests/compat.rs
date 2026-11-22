//! Compatibility with the requests engine 0.4.0 accepted and refused.
//!
//! The corpus below is a fixed list of request bodies, as a client posts them: hand-written accepted and
//! refused cases for every rule, and a deterministic random sample across the families. `fixtures/compat_0.4.0.json`
//! is the snapshot that engine 0.4.0 produced for it. Every later build must reproduce, for every case: the same
//! parse result (or the same parse error text); the same state, the same refusal text and the same refused field;
//! and, for an accepted case, the same cut geometry byte for byte (the ENTITIES section up to the first TEXT
//! annotation) and the same pattern quantities and curves.
//! What may change is the annotation (TEXT entities and 999 comments), the closure and the records. The
//! fixture is regenerated only by a deliberate change of the engine, with
//! `NF_WRITE_COMPAT_FIXTURE=1 cargo test -p nf-core --test compat`, and the report of that change lists it.
//! Review B2-002 (deliberate, listed in its report): polylines now report `points_exact: false` and their own
//! representation error. That changed the `patterns_sha256` of the 52 accepted eccentric, branch and header cases
//! that contain a polyline (old and new values differ only in that field; every cut geometry is byte-identical).
//! The fixture was regenerated for that change.
//! Review B2-003/B2-004 (deliberate, listed in its report): arc display points now follow a sagitta tolerance,
//! and the header with an opening reports `gross_area`/`net_area`/`opening_area` instead of `area`. That changed
//! the `patterns_sha256` of 50 of 335 cases (42 cone_frustum from the arc display points, 8 header_with_branch
//! from the quantity keys); the values differ only in that field, and every cut geometry is byte-identical.
//! The fixture was regenerated for that change.
//! Review B2-005/B2-006 (engine 0.4.2, deliberate, listed in its report): refusals were reduced to their
//! mathematical conditions and two quantities were improved. Of 335 cases, 20 flipped from REFUSED to accepted
//! (cases 27, 221: cones with sin alpha below 1e-3 and apex radius within 1e8 mm; 29, 80, 90, 105, 127, 134, 166,
//! 167, 189, 210, 227, 228, 251, 254, 292, 305, 314: sectors above 99.9 % of a turn with a seam gap above 1e-9 mm;
//! 33: an eccentric reducer with sin alpha 3.3e-5, a tiny developed angle) and none flipped the other way; 14
//! branch cases changed the text of a refusal (the angle limit and the exact s_max in the message); the
//! `patterns_sha256` of 49 accepted cases changed (30 eccentric: area; 11 branch and 8 header_with_branch: exact
//! s_min/s_max in quantities and notes; counted by a field-level diff of this fixture against the one of 0.4.1). Every cut geometry of a case accepted in both versions is
//! byte-identical. The fixture was regenerated for that change.
//! Review B2 Q2(b) (engine 0.4.2, deliberate, same release): every cone and eccentric reducer now reports its
//! flat-pattern seam gap and the gap below which the profile gate is INCONCLUSIVE (quantities `seam_gap` and
//! `seam_gap_resolution`). That changed the `patterns_sha256` of the 95 accepted cone_frustum (55) and
//! eccentric_reducer (40) cases and nothing else: no state, no outcome and no refusal changed, and every
//! `dxf_cut_geometry` is byte-identical to the fixture before it (and to 0.4.1 for all 158 cases accepted in
//! both versions). The fixture was regenerated for that change.
//! Review fix of Q2(b) (engine 0.4.2, same release): the cone `seam_gap_resolution` is now max(1e-9 mm, 2 eps_num) and the
//! reducer's max(1e-9 mm, 2 eps_rep, 2 eps_num) (it was the bare 1e-9 mm for the cone). Against the fixture before this
//! fix exactly 2 cone_frustum cases changed `patterns_sha256` (their 2 eps_num exceeds 1e-9 mm) and nothing else: no state,
//! no outcome, no refusal, and every `dxf_cut_geometry` byte-identical. The true counts of the accepted cases that carry the
//! seam quantities (field-level diff, 335 cases): 55 cone_frustum and 40 eccentric_reducer. The fixture was regenerated.
//! Review of work plan task 1.2 (engine 0.4.4, deliberate, `NF_WRITE_COMPAT_FIXTURE=1`): the eccentric reducer's
//! `eps_rep` is not certified (its vertex term carries the quadrature estimate of its developed angle), so its two
//! circumference checks are INCONCLUSIVE instead of PASS; the statements of G08 and of the pattern assumptions about
//! `eps_num` and `eps_rep` were corrected to the status each value has (TOLERANCE_POLICY section 6); `interval::library`
//! is one unit wider outward. Against the fixture before this change, of 335 cases: no state, no outcome, no refusal
//! and no parse result changed; every `dxf_cut_geometry` is byte-identical; `patterns_sha256` changed for 148 accepted
//! cases: 40 eccentric_reducer, 55 cone_frustum, 34 cylinder_shell, 11 branch_stub and 8 header_with_branch. Every one of
//! the 148 changes comes from the assumption text (the fixture does not hold the check verdicts, which are the
//! reducer's INCONCLUSIVE circumference checks, nor the eps flags): regenerating with the previous `interval::library`
//! gives the same 148 changes, so the outward margin changed no pattern of this corpus. The fixture was regenerated.
//! Work plan task 1.3 (engine 0.4.5, deliberate, `NF_WRITE_COMPAT_FIXTURE=1`): RB-008, the outer profile of a part made
//! of lines and polylines is one closed polyline (same vertices in the same order; tests/plate_conventions.rs). Of 335
//! cases, no state, outcome, refusal or parse result changed and no `patterns_sha256` changed; `dxf_cut_geometry`
//! changed for 93 accepted cases: 34 cylinder_shell, 40 eccentric_reducer, 11 branch_stub and 8 header_with_branch (the
//! header and the branch of each). The 55 cone_frustum cases (arc profiles) are byte-identical. Every changed profile was
//! checked vertex for vertex against the engine 0.4.4 curves, and every opening entity is byte-identical. The plate
//! conventions are in the annotation lines and the 999 block, which this snapshot does not hash. The fixture was regenerated.
//! Work plan task 1.4 (deliberate, `NF_WRITE_COMPAT_FIXTURE=1`, engine 0.4.6): the cylinder shell, cone and
//! eccentric reducer accept the optional `surface_reference`. Of 335 cases exactly ONE line changed: case 70, a cylinder
//! shell that carried `surface_reference: "outside"`, which 0.4.5 refused as an unknown field and which is accepted now.
//! It has the same pattern, checks, DXF and state as the same shell without the field (tests/surface_declaration.rs).
//! Every case without the field is byte-identical to the previous fixture.
//! Engine 0.4.6 (work plan task 1.4, RB-010): an `unknown` surface makes the radius checks of the branch families INCONCLUSIVE.
//! This snapshot holds no verdicts, so it is unchanged by that (no state, pattern or cut geometry moved: the same 2 lines
//! differ). The verdicts are in `fixtures/compat_verdicts.json` (test `compat_verdicts_reproduce_the_verdict_snapshot`),
//! and there the six flipped cases are listed in docs/backend/TOLERANCE_POLICY.md section 9.
//! Work plan task 1.5a (deliberate, `NF_WRITE_COMPAT_FIXTURE=1`, engine 0.4.7): the cylinder shell (`verify_cylinder`)
//! and the eccentric reducer carry the `outline_counter_clockwise` guard (RC-09/RC-10, RB-006). In `fixtures/compat_verdicts.json`
//! 75 of 335 cases gained that one check (all PASS: every corpus cylinder and eccentric outline is counter-clockwise); no
//! existing check's verdict or reason changed, no case flipped PASS to anything else, and the cut geometry and patterns did not move.
//! `verify_cylinder_solid` also carries the guard, but it is not part of the corpus (test-only).

use nf_core::job::{Request, run};
use serde_json::{Value, json};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/compat_0.4.0.json"
);

/// Deterministic generator (xorshift64), the same construction as the stress campaign.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
    /// Log-uniform: very small and very large sizes occur as often as ordinary ones.
    fn log(&mut self, lo: f64, hi: f64) -> f64 {
        (lo.ln() + (hi.ln() - lo.ln()) * self.next()).exp()
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() * xs.len() as f64) as usize % xs.len()]
    }
    fn chance(&mut self, p: f64) -> bool {
        self.next() < p
    }
}

/// Requests written by hand: accepted cases, and one or more refused case for each rule.
fn hand_written() -> Vec<Value> {
    let header = |hr: f64,
                  hdev: f64,
                  br: f64,
                  bdev: f64,
                  beta: f64,
                  delta: f64,
                  station: f64,
                  s_end: f64| {
        json!({
            "family": "header_with_branch", "header_radius": hr, "header_dev_radius": hdev,
            "header_length": 1200.0, "branch_station": station, "branch_radius": br,
            "branch_dev_radius": bdev, "beta_deg": beta, "delta": delta, "s_end": s_end
        })
    };
    let with = |mut v: Value, extra: Value| {
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let set_on_example = |extra: Value| {
        with(
            header(161.95, 156.795, 77.095, 80.6, 90.0, 0.0, 600.0, 400.0),
            extra,
        )
    };
    let stub = |extra: Value| {
        with(
            json!({"family": "branch_stub", "header_radius": 150.0, "branch_radius": 100.0,
                   "branch_dev_radius": 100.0, "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0}),
            extra,
        )
    };
    vec![
        // accepted: every family, the forms the client sends
        json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 250.0}),
        json!({"family": "cylinder_shell", "r_dev": 500, "length": 1000}),
        json!({"family": "cylinder_shell", "r_dev": 100000.0, "length": 0.01}),
        json!({"family": "cone_frustum", "r1": 200.0, "r2": 100.0, "h": 300.0}),
        json!({"family": "cone_frustum", "r1": 150.0, "r2": 140.0, "h": 50.0}),
        json!({"family": "eccentric_reducer", "r1": 200.0, "r2": 100.0, "h": 300.0, "e": 100.0}),
        json!({"family": "eccentric_reducer", "r1": 200.0, "r2": 100.0, "h": 300.0, "e": 0.0}),
        stub(json!({})),
        stub(json!({"surface_reference": "outside"})),
        stub(json!({"surface_reference": "outside", "branch_dev_radius": 96.0})),
        stub(json!({"surface_reference": "inside", "branch_dev_radius": 104.0})),
        stub(json!({"surface_reference": "neutral", "branch_dev_radius": 100.0})),
        stub(json!({"surface_reference": "unknown", "branch_dev_radius": 110.0})),
        header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0)
            .as_object()
            .map(|o| {
                let mut v = Value::Object(o.clone());
                v["surface_reference"] = json!("outside");
                v
            })
            .unwrap(),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"surface_reference": "unknown"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 20.0, 500.0, 400.0),
            json!({"surface_reference": "outside"}),
        ),
        set_on_example(
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on"}),
        ),
        set_on_example(
            json!({"surface_reference": "outside", "header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on"}),
        ),
        set_on_example(
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside"}),
        ),
        with(
            header(161.95, 156.795, 77.095, 80.6, 60.0, 20.0, 600.0, 400.0),
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on"}),
        ),
        with(
            header(161.95, 156.795, 84.15, 80.6, 90.0, 0.0, 600.0, 400.0),
            json!({"surface_reference": "outside"}),
        ),
        // refused: each rule, at its boundary
        json!({"family": "cylinder_shell", "r_dev": -1.0, "length": 250.0}),
        json!({"family": "cylinder_shell", "r_dev": 0.0, "length": 250.0}),
        json!({"family": "cylinder_shell", "r_dev": 1e300, "length": 1e300}),
        json!({"family": "cylinder_shell", "r_dev": 1e-300, "length": 10.0}),
        json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 1e9}),
        json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 1e-7}),
        json!({"family": "cone_frustum", "r1": 200.0, "r2": 199.99, "h": 300.0}),
        json!({"family": "cone_frustum", "r1": 100.0, "r2": 200.0, "h": 300.0}),
        json!({"family": "cone_frustum", "r1": 1000.0, "r2": 1.0, "h": 1.0}),
        json!({"family": "cone_frustum", "r1": 200.0, "r2": 100.0, "h": -300.0}),
        json!({"family": "eccentric_reducer", "r1": 100.0, "r2": 100.0, "h": 300.0, "e": 50.0}),
        json!({"family": "eccentric_reducer", "r1": 200.0, "r2": 100.0, "h": 300.0, "e": -5.0}),
        json!({"family": "eccentric_reducer", "r1": 200.0, "r2": 199.99, "h": 300.0, "e": 5.0}),
        stub(json!({"beta_deg": 5.0})),
        stub(json!({"beta_deg": 175.0})),
        stub(json!({"beta_deg": 1e9})),
        stub(json!({"header_radius": 100.0, "branch_radius": 100.0})),
        stub(json!({"delta": 1e9})),
        stub(json!({"s_end": 0.5})),
        stub(json!({"s_end": -3.0})),
        stub(json!({"surface_reference": "whatever"})),
        stub(json!({"surface_reference": "outside", "branch_dev_radius": 104.0})),
        stub(json!({"surface_reference": "inside", "branch_dev_radius": 96.0})),
        stub(json!({"surface_reference": "neutral", "branch_dev_radius": 100.5})),
        stub(json!({"header_radius": 0.0})),
        stub(json!({"branch_radius": 1e-7})),
        header(150.0, 160.0, 100.0, 110.0, 60.0, 0.0, 600.0, 400.0)
            .as_object()
            .map(|o| {
                let mut v = Value::Object(o.clone());
                v["surface_reference"] = json!("outside");
                v
            })
            .unwrap(),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 1199.9, 400.0),
            json!({"surface_reference": "unknown"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 10.0, 400.0),
            json!({"surface_reference": "unknown"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 0.5),
            json!({"surface_reference": "unknown"}),
        ),
        with(
            header(150.0, 150.0, 150.0, 150.0, 60.0, 0.0, 600.0, 400.0),
            json!({"surface_reference": "unknown"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_in"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"header_surface_reference": "inside", "branch_surface_reference": "inside", "joint": "set_on"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"header_surface_reference": "outside", "branch_surface_reference": "outside", "joint": "set_on"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"header_surface_reference": "outside", "branch_surface_reference": "bogus"}),
        ),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"surface_reference": "bogus"}),
        ),
        set_on_example(
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on", "branch_dev_radius": 75.0}),
        ),
        with(
            header(161.95, 156.795, 77.095, 80.6, 90.0, 0.0, 600.0, 400.0),
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on", "branch_dev_radius": 75.0}),
        ),
        set_on_example(json!({"header_surface_reference": "outside"})),
        set_on_example(json!({"branch_surface_reference": "inside"})),
        set_on_example(
            json!({"header_surface_reference": "neutral", "branch_surface_reference": "inside"}),
        ),
        set_on_example(
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": null}),
        ),
        set_on_example(
            json!({"header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set-on"}),
        ),
        // refused by the request structure
        json!({"family": "teapot"}),
        json!({"r_dev": 100.0, "length": 250.0}),
        json!({"family": "cylinder_shell", "r_dev": 100.0}),
        json!({"family": "cylinder_shell", "r_dev": null, "length": 250.0}),
        json!({"family": "cylinder_shell", "r_dev": "abc", "length": 250.0}),
        json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 250.0, "thickness": 12.0}),
        json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 250.0, "surface_reference": "outside"}),
        json!({"family": "branch_stub", "header_radius": 150.0, "branch_radius": 100.0, "branch_dev_radius": 100.0, "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0, "surface_reference": null}),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"header_surface_reference": "outside"}),
        ),
        header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
        with(
            header(150.0, 150.0, 100.0, 100.0, 60.0, 0.0, 600.0, 400.0),
            json!({"surface_reference": null, "header_surface_reference": "outside", "branch_surface_reference": "inside"}),
        ),
    ]
}

/// Requests drawn at random across the families, with a share of values that the engine refuses.
fn random_cases(seed: u64, n: usize) -> Vec<Value> {
    let mut rng = Rng(seed);
    let sides = ["outside", "inside", "neutral", "unknown", "bogus"];
    (0..n)
        .map(|_| match (rng.next() * 5.0) as usize {
            0 => json!({"family": "cylinder_shell", "r_dev": rng.log(1e-7, 1e9), "length": rng.log(1e-7, 1e9)}),
            1 => {
                let r1 = rng.range(50.0, 500.0);
                let r2 = r1 * rng.range(0.01, 1.2);
                json!({"family": "cone_frustum", "r1": r1, "r2": r2, "h": rng.log(1.0, 2000.0)})
            }
            2 => {
                let r1 = rng.range(50.0, 500.0);
                let r2 = r1 * rng.range(0.01, 1.2);
                json!({"family": "eccentric_reducer", "r1": r1, "r2": r2, "h": rng.log(1.0, 2000.0), "e": rng.range(-50.0, 300.0)})
            }
            3 => {
                let big = rng.range(50.0, 400.0);
                let r = big * rng.range(0.05, 1.1);
                let mut v = json!({"family": "branch_stub", "header_radius": big, "branch_radius": r,
                    "branch_dev_radius": r * rng.range(0.9, 1.1), "beta_deg": rng.range(0.0, 180.0),
                    "delta": rng.range(-big, big), "s_end": rng.log(0.5, 2000.0)});
                if rng.chance(0.7) {
                    v["surface_reference"] = json!(rng.pick(&sides));
                }
                v
            }
            _ => {
                let big = rng.range(50.0, 400.0);
                let r = big * rng.range(0.05, 1.1);
                let mut v = json!({"family": "header_with_branch", "header_radius": big,
                    "header_dev_radius": big * rng.range(0.95, 1.05), "header_length": rng.log(100.0, 3000.0),
                    "branch_station": rng.range(-100.0, 3100.0), "branch_radius": r,
                    "branch_dev_radius": r * rng.range(0.95, 1.05), "beta_deg": rng.range(0.0, 180.0),
                    "delta": rng.range(-big, big), "s_end": rng.log(1.0, 3000.0)});
                if rng.chance(0.5) {
                    v["surface_reference"] = json!(rng.pick(&sides));
                }
                if rng.chance(0.6) {
                    v["header_surface_reference"] = json!(rng.pick(&sides));
                }
                if rng.chance(0.6) {
                    v["branch_surface_reference"] = json!(rng.pick(&sides));
                }
                if rng.chance(0.4) {
                    v["joint"] = json!(rng.pick(&["set_on", "set_in", "set_on"]));
                }
                if rng.chance(0.05) {
                    v["thickness"] = json!(rng.range(1.0, 20.0));
                }
                v
            }
        })
        .collect()
}

fn corpus() -> Vec<Value> {
    let mut all = hand_written();
    all.extend(random_cases(0x5EED_0411, 260));
    all
}

/// The cut geometry of a DXF: the ENTITIES section up to the first annotation (every line, arc and polyline
/// of the cutting layers, with the exact numbers written).
fn cut_geometry(text: &str) -> &str {
    const START: &str = "  2\nENTITIES\n";
    let start = text.find(START).expect("ENTITIES section") + START.len();
    let end = match text[start..].find("  0\nTEXT\n") {
        Some(i) => start + i,
        None => text.rfind("  0\nENDSEC\n").expect("end of ENTITIES"),
    };
    &text[start..end]
}

fn snapshot(i: usize, body: &Value) -> Value {
    let req = match Request::parse_strict(body) {
        Err(e) => return json!({ "i": i, "request": body, "parse_error": e }),
        Ok(r) => r,
    };
    let out = run(&req, "compat");
    let patterns: Vec<Value> = out
        .patterns
        .iter()
        .map(|p| {
            let mut v = serde_json::to_value(p).expect("pattern");
            v.as_object_mut().expect("object").remove("dxf_sha256");
            v
        })
        .collect();
    let dxf: Vec<Value> = out
        .dxf
        .iter()
        .map(|(part, text)| json!([part, nf_core::sha256_hex(cut_geometry(text).as_bytes())]))
        .collect();
    json!({
        "i": i,
        "request": body,
        "parse_error": null,
        "state": serde_json::to_value(out.state).expect("state"),
        "outcome": out.outcome,
        "refused_field": out.refused_field,
        "patterns_sha256": nf_core::sha256_hex(serde_json::to_string(&patterns).expect("json").as_bytes()),
        "dxf_cut_geometry": dxf,
    })
}

fn snapshots() -> Vec<Value> {
    corpus()
        .iter()
        .enumerate()
        .map(|(i, b)| snapshot(i, b))
        .collect()
}

/// Work plan task 1.4, deliberate: the check verdicts of every corpus case, with the INCONCLUSIVE reason. The snapshot
/// above does not hold verdicts; this one does, so a verdict change is listed case by case, not only in prose.
/// Regenerate with `NF_WRITE_COMPAT_FIXTURE=1`, as the other fixture.
const VERDICT_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/compat_verdicts.json"
);

fn verdict_snapshot(i: usize, body: &Value) -> Value {
    let checks: Vec<Value> = match Request::parse_strict(body) {
        Err(_) => vec![],
        Ok(req) => run(&req, "compat")
            .checks
            .iter()
            .map(|c| json!([c.scope, c.name, c.verdict, c.reason]))
            .collect(),
    };
    json!({ "i": i, "checks": checks })
}

#[test]
fn compat_verdicts_reproduce_the_verdict_snapshot() {
    let now: Vec<Value> = corpus()
        .iter()
        .enumerate()
        .map(|(i, b)| verdict_snapshot(i, b))
        .collect();
    if std::env::var_os("NF_WRITE_COMPAT_FIXTURE").is_some() {
        let mut text = String::from("[\n");
        for (n, s) in now.iter().enumerate() {
            text.push_str(&serde_json::to_string(s).expect("json"));
            text.push_str(if n + 1 < now.len() { ",\n" } else { "\n" });
        }
        text.push_str("]\n");
        std::fs::write(VERDICT_FIXTURE, text).expect("write verdict fixture");
        return;
    }
    let stored: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(VERDICT_FIXTURE).expect("verdict fixture"))
            .expect("verdict fixture json");
    assert_eq!(stored.len(), now.len(), "corpus size changed");
    for (old, new) in stored.iter().zip(&now) {
        assert_eq!(old, new, "case {} verdicts changed", old["i"]);
    }
}

#[test]
fn compat_corpus_reproduces_the_0_4_0_snapshot() {
    let now = snapshots();
    if std::env::var_os("NF_WRITE_COMPAT_FIXTURE").is_some() {
        let mut text = String::from("[\n");
        for (n, s) in now.iter().enumerate() {
            text.push_str(&serde_json::to_string(s).expect("json"));
            text.push_str(if n + 1 < now.len() { ",\n" } else { "\n" });
        }
        text.push_str("]\n");
        std::fs::write(FIXTURE, text).expect("write fixture");
        return;
    }
    let stored: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(FIXTURE).expect("fixture"))
            .expect("fixture json");
    assert_eq!(stored.len(), now.len(), "corpus size changed");
    let mut accepted = 0usize;
    let mut refused = 0usize;
    let mut parse_errors = 0usize;
    for (old, new) in stored.iter().zip(&now) {
        assert_eq!(old, new, "case {} changed", old["i"]);
        match (old["parse_error"].is_null(), old["state"] == "REFUSED") {
            (false, _) => parse_errors += 1,
            (true, true) => refused += 1,
            (true, false) => accepted += 1,
        }
    }
    // the corpus exercises both sides and the parse layer
    assert!(
        accepted >= 40 && refused >= 60 && parse_errors >= 10,
        "{accepted} {refused} {parse_errors}"
    );
}

/// BH-014: every refusal of the corpus, parse refusals included, carries a code of the table, and the refusal's
/// message is the outcome the job has always stated.
#[test]
fn every_refusal_of_the_corpus_carries_a_code_of_the_table() {
    fn check(r: &nf_core::Refusal) {
        assert!(
            nf_core::refusal::ALL.contains(&r.code.as_str()),
            "{} is not a code of the table",
            r.code
        );
        assert!(!r.message.is_empty(), "{} has no message", r.code);
    }
    let mut refused = 0usize;
    for body in corpus() {
        match Request::parse_refusal(&body) {
            Err(r) => {
                check(&r);
                refused += 1;
            }
            Ok(req) => {
                let out = run(&req, "compat");
                match &out.refusal {
                    Some(r) => {
                        check(r);
                        assert_eq!(Some(&r.message), out.outcome.as_ref(), "{body}");
                        refused += 1;
                    }
                    None => assert!(
                        out.outcome.is_none(),
                        "{body}: an outcome without a refusal"
                    ),
                }
            }
        }
    }
    assert!(refused >= 100, "{refused} refusals in the corpus");
}
