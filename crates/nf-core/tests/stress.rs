//! Stress campaign over hard and extreme geometry ("hard structures").
//!
//! Every generated case must end in exactly one of two honest outcomes:
//!   * a pattern whose independent checks, profile check and DXF re-import all
//!     pass, and which replays to the identical release closure; or
//!   * an explicit refusal (no pattern, no DXF, a stated reason).
//!
//! Anything else — a pattern that fails its own verification, an invalid
//! profile, a non-reproducible result, a panic — is a DEFECT and fails the
//! test. A wrong-but-plausible pattern is the outcome this campaign exists to
//! rule out; it cannot prove its absence, only fail to find one.
//!
//! Scale the number of cases with NF_STRESS_SCALE (default 1).

use nf_core::job::{Request, run};
use nf_core::{GateResult, ReleaseState};

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
    /// Log-uniform: exercises very small and very large sizes equally.
    fn log(&mut self, lo: f64, hi: f64) -> f64 {
        (lo.ln() + (hi.ln() - lo.ln()) * self.next()).exp()
    }
    /// A value pushed close to a limit, where formulas are worst conditioned.
    fn near(&mut self, limit: f64, side: f64) -> f64 {
        limit * (1.0 + side * 10f64.powf(-self.range(1.0, 9.0)))
    }
}

#[derive(Default, Debug)]
struct Tally {
    ok: usize,
    refused: usize,
    defects: Vec<String>,
}

fn judge(req: &Request, t: &mut Tally) {
    let r = std::panic::catch_unwind(|| run(req, "stress"));
    let out = match r {
        Ok(o) => o,
        Err(_) => {
            return t
                .defects
                .push(format!("PANIC {}", serde_json::to_string(req).unwrap()));
        }
    };
    let case = || serde_json::to_string(req).unwrap();
    if out.patterns.is_empty() {
        if out.dxf.is_empty() && out.outcome.is_some() && out.state != ReleaseState::Approved {
            t.refused += 1;
        } else {
            t.defects.push(format!("REFUSAL-INCONSISTENT {}", case()));
        }
        return;
    }
    let gate = |id: &str| out.gates.iter().find(|g| g.id == id).map(|g| g.result);
    // Task 1.2 review (work plan task 1.2, review): the representation error of an eccentric reducer is
    // not certified (its vertex term carries a quadrature estimate), so its two circumference checks are
    // INCONCLUSIVE by design, and only those. Any other non-PASS check, and any FAIL, is still a defect.
    // Work plan task 1.4 (engine 0.4.6, RB-010): a radius check of an `unknown` surface is INCONCLUSIVE with the stated
    // reason. The reason is set only under `unknown`, so a check that carries it is by design as well, and nothing else.
    let by_design = |c: &nf_core::job::CheckView| {
        (c.verdict == "INCONCLUSIVE"
            && matches!(
                c.name.as_str(),
                "large_edge_equals_circumference" | "small_edge_equals_circumference"
            )
            && out.patterns.iter().any(|p| p.family == "eccentric_reducer"))
            || (c.verdict == "INCONCLUSIVE"
                && c.reason.as_deref() == Some(uem::verify::SURFACE_UNKNOWN_REASON))
    };
    let inconclusive_by_design = out.checks.iter().filter(|c| by_design(c)).count();
    let bad_check = out
        .checks
        .iter()
        .find(|c| c.verdict != "PASS" && !by_design(c));
    if let Some(c) = bad_check {
        return t.defects.push(format!(
            "CHECK {} [{}] diff {:e} tol {:e} :: {}",
            c.name,
            c.scope,
            c.difference,
            c.tolerance,
            case()
        ));
    }
    // Q2(b) (engine 0.4.2): the profile gate passes unless a flat-pattern seam gap is below the resolution its
    // adjoining curves allow (the pattern states both numbers); then, and only then, it is INCONCLUSIVE.
    let seam_unresolved = out.patterns.iter().any(|p| {
        matches!(
            (p.quantities.get("seam_gap"), p.quantities.get("seam_gap_resolution")),
            (Some(g), Some(r)) if g < r
        )
    });
    let g11_want = if seam_unresolved {
        GateResult::Inconclusive
    } else {
        GateResult::Pass
    };
    let g10_want = if inconclusive_by_design > 0 {
        GateResult::Inconclusive
    } else {
        GateResult::Pass
    };
    if gate("G10") != Some(g10_want)
        || gate("G11") != Some(g11_want)
        || gate("G15") != Some(GateResult::Pass)
    {
        return t.defects.push(format!(
            "GATE G10={:?} (want {g10_want:?}) G11={:?} G15={:?} :: {}",
            gate("G10"),
            gate("G11"),
            gate("G15"),
            case()
        ));
    }
    if out.state == ReleaseState::Approved {
        return t.defects.push(format!("APPROVED-REACHED {}", case()));
    }
    if out.patterns.iter().any(|p| {
        !p.eps_num.is_finite()
            || !p.eps_rep.is_finite()
            || p.curves.iter().any(|c| {
                c.points
                    .iter()
                    .any(|q| !q[0].is_finite() || !q[1].is_finite())
            })
    }) {
        return t.defects.push(format!("NON-FINITE {}", case()));
    }
    let again = run(req, "stress");
    if again.closure_hash != out.closure_hash || again.dxf != out.dxf {
        return t.defects.push(format!("NOT-REPRODUCIBLE {}", case()));
    }
    t.ok += 1;
}

fn scale() -> usize {
    std::env::var("NF_STRESS_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn report(name: &str, t: &Tally) {
    println!(
        "{name}: {} verified, {} refused, {} DEFECTS",
        t.ok,
        t.refused,
        t.defects.len()
    );
    for d in t.defects.iter().take(12) {
        println!("  {d}");
    }
    assert!(
        t.ok > 0,
        "{name}: campaign produced no verified case at all"
    );
    assert!(
        t.defects.is_empty(),
        "{name}: {} defects (first: {})",
        t.defects.len(),
        t.defects[0]
    );
}

#[test]
fn stress_cylinder_and_cone() {
    let (mut g, mut t) = (Rng(0x9E3779B97F4A7C15), Tally::default());
    for i in 0..400 * scale() {
        let r = g.log(1e-2, 1e5);
        judge(
            &Request::CylinderShell {
                r_dev: r,
                length: g.log(1e-2, 1e5),
                surface_reference: None,
            },
            &mut t,
        );
        // cones: generic, near-cylindrical, very steep (sector angle close to a full turn), huge and tiny
        let r1 = g.log(1.0, 1e5);
        let (r2, h) = match i % 4 {
            0 => (r1 * g.range(0.02, 0.98), r1 * g.log(0.05, 50.0)),
            1 => (g.near(r1, -1.0), r1 * g.log(0.1, 10.0)),
            2 => (r1 * g.range(0.001, 0.2), r1 * g.log(1e-4, 0.05)),
            _ => (r1 * g.log(1e-6, 0.999), r1 * g.log(1e-3, 1e3)),
        };
        judge(
            &Request::ConeFrustum {
                r1,
                r2,
                h,
                surface_reference: None,
            },
            &mut t,
        );
    }
    for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        judge(
            &Request::ConeFrustum {
                r1: bad,
                r2: 1.0,
                h: 1.0,
                surface_reference: None,
            },
            &mut t,
        );
        judge(
            &Request::CylinderShell {
                r_dev: 1.0,
                length: bad,
                surface_reference: None,
            },
            &mut t,
        );
    }
    report("cylinder+cone", &t);
}

#[test]
fn stress_eccentric_reducer() {
    let (mut g, mut t) = (Rng(0xD1B54A32D192ED03), Tally::default());
    for i in 0..150 * scale() {
        let r1 = g.log(1.0, 1e4);
        let r2 = if i % 5 == 0 {
            g.near(r1, -1.0)
        } else {
            r1 * g.range(0.02, 0.97)
        };
        let h = r1 * g.log(0.02, 30.0);
        // offsets from concentric through flat-sided to strongly oblique
        let e = match i % 4 {
            0 => 0.0,
            1 => r1 - r2,
            2 => (r1 - r2) * g.range(0.0, 1.0),
            _ => r1 * g.log(0.01, 20.0),
        };
        judge(
            &Request::EccentricReducer {
                r1,
                r2,
                h,
                e,
                surface_reference: None,
            },
            &mut t,
        );
    }
    report("eccentric reducer", &t);
}

#[test]
fn stress_branch() {
    let (mut g, mut t) = (Rng(0xA0761D6478BD642F), Tally::default());
    for i in 0..150 * scale() {
        let big_r = g.log(5.0, 1e4);
        let r = match i % 4 {
            0 => g.near(big_r, -1.0), // almost equal diameters
            1 => big_r * g.log(1e-3, 0.2),
            _ => big_r * g.range(0.05, 0.98),
        };
        let delta = match i % 3 {
            0 => 0.0,
            1 => (big_r - r) * g.range(-1.05, 1.05), // up to and past tangency
            _ => (big_r - r) * g.range(-0.9, 0.9),
        };
        let beta = match i % 5 {
            0 => g.range(1.0, 15.0), // grazing
            1 => g.range(165.0, 179.0),
            _ => g.range(15.0, 165.0),
        };
        let s_end = big_r * g.log(0.5, 30.0);
        judge(
            &Request::BranchStub {
                header_radius: big_r,
                branch_radius: r,
                branch_dev_radius: r * g.range(0.9, 1.1),
                beta_deg: beta,
                delta,
                s_end,
                surface_reference: "unknown".into(), // field added with BE-001; "unknown" keeps this campaign as it was
            },
            &mut t,
        );
    }
    report("branch stub", &t);
}

#[test]
fn stress_header_with_branch_assembly() {
    let (mut g, mut t) = (Rng(0xE7037ED1A0B428DB), Tally::default());
    for i in 0..40 * scale() {
        let big_r = g.log(20.0, 5e3);
        let r = if i % 4 == 0 {
            g.near(big_r, -1.0)
        } else {
            big_r * g.range(0.05, 0.97)
        };
        let delta = (big_r - r) * g.range(-1.0, 1.0);
        let beta = g.range(8.0, 172.0);
        let len = big_r * g.log(1.0, 40.0);
        let station = len * g.range(-0.1, 1.1); // sometimes off the shell
        let sref = ["outside", "inside", "neutral", "unknown", "bogus"][i % 5];
        judge(
            &Request::HeaderWithBranch {
                header_radius: big_r,
                header_dev_radius: big_r * g.range(0.9, 1.1),
                header_length: len,
                branch_station: station,
                branch_radius: r,
                branch_dev_radius: r * g.range(0.9, 1.1),
                beta_deg: beta,
                delta,
                s_end: big_r * g.log(1.5, 20.0),
                surface_reference: Some(sref.into()),
                header_surface_reference: None,
                branch_surface_reference: None,
                joint: None,
            },
            &mut t,
        );
    }
    report("header with branch", &t);
}

/// Every case that once exposed a defect lives in tests/regressions/ forever.
/// Expectation "verified" = pattern with all checks passing; "refused" = explicit refusal.
#[test]
fn regression_corpus() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regressions");
    let mut n = 0;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    entries.sort();
    for path in entries {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for case in v["cases"].as_array().unwrap() {
            let req: Request = serde_json::from_value(case["request"].clone())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let mut t = Tally::default();
            judge(&req, &mut t);
            assert!(
                t.defects.is_empty(),
                "{}: regression returned: {}",
                path.display(),
                t.defects[0]
            );
            match case["expect"].as_str().unwrap() {
                "verified" => assert_eq!(
                    t.ok,
                    1,
                    "{}: expected a verified pattern for {}",
                    path.display(),
                    case["request"]
                ),
                "refused" => assert_eq!(
                    t.refused,
                    1,
                    "{}: expected a refusal for {}",
                    path.display(),
                    case["request"]
                ),
                other => panic!("unknown expectation {other}"),
            }
            // optional: the result of gate G11 (Q2(b): INCONCLUSIVE for an unresolved seam) and the engine
            // advisories of the job (Q1(i): the B2-001 radius advisory is on the job, not only in G02)
            if let Some(want) = case.get("g11").and_then(|x| x.as_str()) {
                let out = run(&req, "regression");
                let g = out.gates.iter().find(|g| g.id == "G11").expect("G11");
                assert_eq!(
                    serde_json::to_value(g.result).unwrap().as_str().unwrap(),
                    want,
                    "{}: G11 {}",
                    path.display(),
                    g.detail
                );
            }
            if let Some(want) = case.get("advisory_contains").and_then(|x| x.as_str()) {
                let out = run(&req, "regression");
                assert!(
                    out.engine_advisories.iter().any(|a| a.contains(want)),
                    "{}: no engine advisory contains {want:?}: {:?}",
                    path.display(),
                    out.engine_advisories
                );
            }
            // optional: assert on the G02 detail text (review B2-001 advisory)
            if case.get("g02_contains").is_some() || case.get("g02_absent").is_some() {
                let out = run(&req, "regression");
                let d = out
                    .gates
                    .iter()
                    .find(|g| g.id == "G02")
                    .map(|g| g.detail.clone())
                    .unwrap_or_default();
                if let Some(s) = case["g02_contains"].as_str() {
                    assert!(d.contains(s), "{}: G02 lacks {s:?}: {d}", path.display());
                }
                if let Some(s) = case["g02_absent"].as_str() {
                    assert!(!d.contains(s), "{}: G02 has {s:?}: {d}", path.display());
                }
            }
            // optional: curve labels (review B2-002). Each check names a curve of pattern `pattern`, in
            // `curves` (default) or `holes`, and states its points_exact flag and its deviation as a ratio
            // of that pattern's eps_rep (0 for a line, 1 for the curve the pattern's eps_rep belongs to).
            if let Some(checks) = case.get("curve_checks").and_then(|c| c.as_array()) {
                let out = run(&req, "regression");
                for ck in checks {
                    let pat = &out.patterns[ck["pattern"].as_u64().unwrap_or(0) as usize];
                    let list = if ck.get("list").and_then(|l| l.as_str()) == Some("holes") {
                        &pat.holes
                    } else {
                        &pat.curves
                    };
                    let name = ck["name"].as_str().unwrap();
                    let c = list
                        .iter()
                        .find(|c| c.name == name)
                        .unwrap_or_else(|| panic!("{}: no curve {name}", path.display()));
                    assert_eq!(
                        c.points_exact,
                        ck["points_exact"].as_bool().unwrap(),
                        "{}: points_exact of {name}",
                        path.display()
                    );
                    let ratio = ck["deviation_ratio_to_eps_rep"].as_f64().unwrap();
                    assert!(
                        (c.points_max_deviation - ratio * pat.eps_rep).abs() < 1e-15,
                        "{}: {name} deviation {} is not {ratio} x eps_rep {}",
                        path.display(),
                        c.points_max_deviation,
                        pat.eps_rep
                    );
                }
            }
            n += 1;
        }
    }
    println!("regression corpus: {n} cases");
}

/// Mutation campaign: the verifiers must catch a pattern made from a wrong
/// parameter. For random valid parts, one input is altered by 0.1 % to 20 %
/// before development and the result is verified against the true inputs.
/// A mutation that passes verification is a missed error.
#[test]
fn verifiers_catch_every_mutated_parameter() {
    use uem::develop::{self, Branch};
    use uem::verify::{self, Verdict};
    let mut g = Rng(0x8EBC6AF09C88C6E3);
    let (mut caught, mut skipped, mut missed) = (0usize, 0usize, Vec::<String>::new());
    let mut note = |hit: Option<bool>, what: String| match hit {
        None => skipped += 1,
        Some(true) => caught += 1,
        Some(false) => missed.push(what),
    };
    let any_fail = |c: Vec<verify::Check>| c.iter().any(|x| x.verdict == Verdict::Fail);
    for i in 0..300 * scale() {
        let f = 1.0 + g.range(1e-3, 0.2) * if g.next() < 0.5 { -1.0 } else { 1.0 };
        let which = i % 4;
        // right cone
        let (r1, h) = (g.log(10.0, 1e4), 0.0);
        let (r2, h) = (r1 * g.range(0.05, 0.9), r1 * g.log(0.2, 10.0) + h);
        let m = [
            (r1 * f, r2, h),
            (r1, r2 * f, h),
            (r1, r2, h * f),
            (r1 * f, r2 * f, h),
        ][which];
        note(
            develop::cone_frustum(m.0, m.1, m.2)
                .ok()
                .map(|p| any_fail(verify::verify_cone(&p, r1, r2, h))),
            format!("cone mutation {which} x{f}: r1={r1} r2={r2} h={h}"),
        );
        // eccentric reducer
        let e = (r1 - r2) * g.range(0.1, 1.5);
        let m = [
            (r1 * f, r2, h, e),
            (r1, r2 * f, h, e),
            (r1, r2, h * f, e),
            (r1, r2, h, e * f),
        ][which];
        note(
            develop::eccentric_reducer(m.0, m.1, m.2, m.3)
                .ok()
                .map(|p| any_fail(verify::verify_eccentric(&p, r1, r2, h, e))),
            format!("eccentric mutation {which} x{f}: r1={r1} r2={r2} h={h} e={e}"),
        );
        // branch stub (true geometry b; pattern developed from a mutated one)
        let (big_r, beta) = (g.log(20.0, 5e3), g.range(25.0, 155.0));
        let r = big_r * g.range(0.1, 0.8);
        let delta = (big_r - r) * g.range(-0.7, 0.7);
        let s_end = big_r * 12.0;
        let b = Branch {
            big_r,
            r,
            beta: beta.to_radians(),
            delta,
        };
        let m = [
            (big_r * f, r, beta, delta),
            (big_r, r * f, beta, delta),
            (big_r, r, beta + (f - 1.0) * 40.0, delta),
            (big_r, r, beta, delta + (f - 1.0) * big_r * 0.5),
        ][which];
        note(
            develop::branch_stub(m.0, m.1, r, m.2, m.3, s_end)
                .ok()
                .map(|p| any_fail(verify::verify_branch(&p, &b, r, s_end))),
            format!("branch mutation {which} x{f}: R={big_r} r={r} beta={beta} delta={delta}"),
        );
    }
    println!(
        "mutations: {caught} caught, {skipped} skipped (mutated part out of range), {} MISSED",
        missed.len()
    );
    for m in missed.iter().take(10) {
        println!("  {m}");
    }
    assert!(caught > 500 * scale(), "too few effective mutations");
    assert!(
        missed.is_empty(),
        "{} mutated parameters passed verification (first: {})",
        missed.len(),
        missed[0]
    );
}
