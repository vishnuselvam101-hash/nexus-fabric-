//! BH-014: every refusal of a calculation request carries a stable code, the request fields it concerns, its
//! message and, where the engine computes it exactly, the limit and the measured value (docs/backend/REFUSAL_CODES.md).
//! Each rule is checked just inside and just outside its boundary, and the table of codes is checked against the build.

use nf_core::job::{MAX_MAGNITUDE_MM, MIN_DIMENSION_MM, Request, run};
use nf_core::{Refusal, ReleaseState};
use serde_json::{Value, json};

/// The refusal of a request as the API reports it (a parse refusal, or the engine's refusal of the job).
/// None when the request is accepted: a pattern is produced, and no gate rejects it.
fn refused(v: &Value) -> Option<Refusal> {
    match Request::parse_refusal(v) {
        Err(r) => Some(r),
        Ok(req) => refused_request(&req),
    }
}

fn refused_request(req: &Request) -> Option<Refusal> {
    let out = run(req, "refusal-test");
    if out.refusal.is_none() {
        assert!(
            out.state != ReleaseState::Rejected,
            "an accepted request was rejected by a gate: {req:?}"
        );
    }
    out.refusal
}

fn fields(r: &Refusal) -> Vec<&str> {
    r.fields.iter().map(String::as_str).collect()
}

/// The request is refused with this code and these fields, and no other stated value is wrong.
fn expect_refused(v: Value, code: &str, names: &[&str]) -> Refusal {
    let r = refused(&v).unwrap_or_else(|| panic!("expected a refusal ({code}) for {v}"));
    assert_eq!(r.code, code, "{v}: {}", r.message);
    assert_eq!(fields(&r), names, "{v}: fields");
    assert!(!r.message.is_empty(), "{v}: message");
    r
}

fn expect_accepted(v: Value) {
    if let Some(r) = refused(&v) {
        panic!(
            "expected acceptance for {v}, refused: {} ({})",
            r.message, r.code
        );
    }
}

fn detail(r: &Refusal, name: &str) -> f64 {
    *r.detail
        .as_ref()
        .unwrap_or_else(|| panic!("{}: no detail", r.code))
        .get(name)
        .unwrap_or_else(|| panic!("{}: no detail {name}", r.code))
}

fn stub(extra: Value) -> Value {
    let mut v = json!({"family": "branch_stub", "header_radius": 150.0, "branch_radius": 100.0,
        "branch_dev_radius": 100.0, "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

#[test]
fn non_positive_and_non_finite_values_are_refused_at_the_boundary() {
    let cyl = |r_dev: Value, length: f64| json!({"family": "cylinder_shell", "r_dev": r_dev, "length": length});
    // zero and negative: not positive; the smallest positive dimension is accepted, a smaller one is below the limit
    expect_refused(cyl(json!(0.0), 250.0), "value_not_positive", &["r_dev"]);
    expect_refused(cyl(json!(-1.0), 250.0), "value_not_positive", &["r_dev"]);
    expect_accepted(cyl(json!(MIN_DIMENSION_MM), 250.0));
    let r = expect_refused(
        cyl(json!(MIN_DIMENSION_MM / 2.0), 250.0),
        "dimension_below_limit",
        &["r_dev"],
    );
    assert_eq!(detail(&r, "limit"), MIN_DIMENSION_MM);
    assert_eq!(detail(&r, "measured"), MIN_DIMENSION_MM / 2.0);
    // non-finite values cannot be written in JSON: they reach the engine from Rust callers
    let nan = refused_request(&Request::CylinderShell {
        r_dev: f64::NAN,
        length: 250.0,
    })
    .expect("NaN refused");
    assert_eq!(
        (nan.code.as_str(), fields(&nan)),
        ("value_not_finite", vec!["r_dev"])
    );
    assert!(nan.detail.is_none());
    let inf = refused_request(&Request::CylinderShell {
        r_dev: 100.0,
        length: f64::INFINITY,
    })
    .expect("infinity refused");
    assert_eq!(
        (inf.code.as_str(), fields(&inf)),
        ("value_not_finite", vec!["length"])
    );
    // a non-finite stub angle or delta is refused, not computed
    let d = refused_request(&Request::BranchStub {
        header_radius: 150.0,
        branch_radius: 100.0,
        branch_dev_radius: 100.0,
        beta_deg: 60.0,
        delta: f64::NAN,
        s_end: 400.0,
        surface_reference: "unknown".into(),
    })
    .expect("NaN delta refused");
    assert_eq!(
        (d.code.as_str(), fields(&d)),
        ("value_not_finite", vec!["delta"])
    );
    // a non-number is a wrong type, not a value
    expect_refused(cyl(json!("abc"), 250.0), "field_wrong_type", &["r_dev"]);
    // the eccentric offset may be zero, not negative
    let red = |e: Value| json!({"family": "eccentric_reducer", "r1": 200.0, "r2": 100.0, "h": 300.0, "e": e});
    expect_accepted(red(json!(0.0)));
    expect_refused(red(json!(-5.0)), "offset_negative", &["e"]);
}

#[test]
fn magnitude_bounds_are_refused_just_outside_and_accepted_just_inside() {
    let cyl = |length: f64| json!({"family": "cylinder_shell", "r_dev": 100.0, "length": length});
    expect_accepted(cyl(MAX_MAGNITUDE_MM));
    let big = expect_refused(
        cyl(MAX_MAGNITUDE_MM * 1.0001),
        "magnitude_above_limit",
        &["length"],
    );
    assert_eq!(detail(&big, "limit"), MAX_MAGNITUDE_MM);
    assert_eq!(detail(&big, "measured"), MAX_MAGNITUDE_MM * 1.0001);
    // a signed parameter is bounded by its size, not by its sign
    expect_refused(
        json!({"family": "branch_stub", "header_radius": 150.0, "branch_radius": 100.0, "branch_dev_radius": 100.0,
               "beta_deg": 60.0, "delta": -2.0 * MAX_MAGNITUDE_MM, "s_end": 400.0}),
        "magnitude_above_limit",
        &["delta"],
    );
}

#[test]
fn radii_are_checked_against_the_declared_surface_on_both_sides() {
    // contact radius 100 (branch_radius); the development radius is the one checked
    let names = ["branch_dev_radius", "branch_radius"];
    expect_accepted(stub(
        json!({"surface_reference": "outside", "branch_dev_radius": 99.5}),
    ));
    expect_accepted(stub(
        json!({"surface_reference": "outside", "branch_dev_radius": 100.0}),
    ));
    let r = expect_refused(
        stub(json!({"surface_reference": "outside", "branch_dev_radius": 100.5})),
        "radius_dev_exceeds_outside",
        &names,
    );
    assert_eq!(
        (detail(&r, "limit"), detail(&r, "measured")),
        (100.0, 100.5)
    );
    expect_accepted(stub(
        json!({"surface_reference": "inside", "branch_dev_radius": 100.5}),
    ));
    let r = expect_refused(
        stub(json!({"surface_reference": "inside", "branch_dev_radius": 99.5})),
        "radius_dev_below_inside",
        &names,
    );
    assert_eq!((detail(&r, "limit"), detail(&r, "measured")), (100.0, 99.5));
    expect_accepted(stub(
        json!({"surface_reference": "neutral", "branch_dev_radius": 100.0}),
    ));
    let r = expect_refused(
        stub(json!({"surface_reference": "neutral", "branch_dev_radius": 100.5})),
        "radius_neutral_mismatch",
        &names,
    );
    assert_eq!(
        (detail(&r, "limit"), detail(&r, "measured")),
        (100.0, 100.5)
    );
    // an unknown surface is accepted with a note: the radii are not checked
    expect_accepted(stub(
        json!({"surface_reference": "unknown", "branch_dev_radius": 110.0}),
    ));
    expect_refused(
        stub(json!({"surface_reference": "bogus"})),
        "surface_reference_invalid",
        &["surface_reference"],
    );
}

#[test]
fn a_branch_as_large_as_the_header_and_a_near_tangent_branch_are_refused() {
    // |delta| + branch radius must be below the header radius (150): the equal-diameter tee is refused
    expect_accepted(stub(
        json!({"branch_radius": 149.9, "branch_dev_radius": 149.9, "s_end": 1e6}),
    ));
    let r = expect_refused(
        stub(json!({"branch_radius": 150.0, "branch_dev_radius": 150.0})),
        "branch_tangent_or_partial",
        &["header_radius", "branch_radius", "delta"],
    );
    assert_eq!(detail(&r, "measured"), 150.0);
    expect_refused(
        stub(json!({"branch_radius": 120.0, "branch_dev_radius": 120.0, "delta": 40.0})),
        "branch_tangent_or_partial",
        &["header_radius", "branch_radius", "delta"],
    );
    // just inside the near-tangent limit (1e-6 of the header radius) is still a branch; just outside is refused
    expect_accepted(stub(
        json!({"branch_radius": 149.9998, "branch_dev_radius": 149.9998, "s_end": 1e6}),
    ));
    expect_refused(
        stub(json!({"branch_radius": 149.99986, "branch_dev_radius": 149.99986, "s_end": 1e6})),
        "branch_tangent_or_partial",
        &["header_radius", "branch_radius", "delta"],
    );
}

#[test]
fn the_angle_between_the_axes_is_limited_to_the_open_interval() {
    // B2-005 (engine 0.4.2): beta is refused only outside the open interval (0, 180) degrees and by the size
    // of the developed branch (1e8 mm). Until 0.4.1 sin(beta) had to exceed 0.17 (about 9.79 degrees): 9.5 and
    // 170.3 were refused, now they and the angles to 0.5 / 179.5 are exact and accepted.
    for beta in [9.5, 5.0, 0.5, 179.5, 170.3, 170.1, 9.9, 169.9] {
        expect_accepted(stub(json!({"beta_deg": beta, "s_end": 1e6})));
    }
    for beta in [0.0, 180.0, -30.0, 1e9] {
        expect_refused(
            stub(json!({"beta_deg": beta})),
            "branch_angle_out_of_range",
            &["beta_deg"],
        );
    }
    // the developed branch of a very shallow angle exceeds 1e8 mm: refused, with the limit in the detail
    let r = expect_refused(
        stub(json!({"beta_deg": 1e-6, "s_end": 1e8})),
        "branch_angle_out_of_range",
        &["beta_deg"],
    );
    assert_eq!(detail(&r, "limit"), 1e8);
    assert!(detail(&r, "measured") > 1e8);
}

/// A header with branch laid out as the validation corpus lays it out (`validation/stability.py` `_asm`): the plate
/// just long enough for the opening, the square end just beyond the saddle.
fn assembly(big_r: f64, r: f64, beta: f64, delta: f64) -> Value {
    let b = uem::develop::Branch {
        big_r,
        r,
        beta: beta.to_radians(),
        delta,
    };
    let (sb, cb) = b.beta.sin_cos();
    let (mut xmin, mut xmax, mut smax) = (f64::MAX, f64::MIN, f64::MIN);
    for i in 0..=40000 {
        let ph = std::f64::consts::TAU * i as f64 / 40000.0;
        let s = b.s(ph);
        xmin = xmin.min(s * cb - r * ph.sin() * sb);
        xmax = xmax.max(s * cb - r * ph.sin() * sb);
        smax = smax.max(s);
    }
    json!({"family": "header_with_branch", "header_radius": big_r, "header_dev_radius": big_r,
           "header_length": (xmax - xmin + 2.0 * big_r).ceil(), "branch_station": (-xmin + big_r).ceil(),
           "branch_radius": r, "branch_dev_radius": r, "beta_deg": beta, "delta": delta,
           "s_end": (smax + big_r).ceil(), "surface_reference": "outside"})
}

/// Largest ratio (bound on the verifier's own rounding) / (budget of its tolerance) over the 3D-intersection checks
/// of a request (`uem::verify` conditioning, TOLERANCE_POLICY section 7): above 1 the verifier is no longer
/// guaranteed to confirm a correct pattern.
fn conditioning(v: &Value) -> f64 {
    let f = |k: &str| v[k].as_f64().unwrap();
    let b = uem::develop::Branch {
        big_r: f("header_radius"),
        r: f("branch_radius"),
        beta: f("beta_deg").to_radians(),
        delta: f("delta"),
    };
    let c = if v["family"] == "branch_stub" {
        uem::verify::branch_stub_conditioning(&b, f("branch_dev_radius"), f("s_end"))
    } else {
        uem::verify::header_with_branch_conditioning(
            &b,
            f("header_dev_radius"),
            f("header_length"),
            f("branch_station"),
            f("branch_dev_radius"),
            f("s_end"),
        )
    };
    c.iter().map(|k| k.rounding / k.budget).fold(0.0, f64::max)
}

/// The angle in (lo, hi) at which `make(beta)` has the conditioning ratio `target` (bisection; the ratio is monotone
/// in beta on either side of 90 degrees).
fn angle_at(make: &dyn Fn(f64) -> Value, lo: f64, hi: f64, target: f64) -> f64 {
    let (mut a, mut b) = (lo, hi);
    let rising = conditioning(&make(b)) > conditioning(&make(a));
    for _ in 0..60 {
        let m = 0.5 * (a + b);
        if (conditioning(&make(m)) > target) == rising {
            b = m
        } else {
            a = m
        }
    }
    0.5 * (a + b)
}

fn accepted_and_verified(v: &Value) -> nf_core::job::JobOutput {
    let out = run(&Request::parse_refusal(v).unwrap(), "refusal-test");
    assert!(
        out.refusal.is_none(),
        "{v}: refused: {:?}",
        out.refusal.map(|r| r.message)
    );
    let g10 = out.gates.iter().find(|g| g.id == "G10").unwrap();
    assert_eq!(g10.result, nf_core::GateResult::Pass, "{v}");
    assert_ne!(out.state, ReleaseState::Rejected, "{v}");
    out
}

/// Work plan task 1.1 (engine 0.4.3): a branch angle the independent verification cannot confirm is refused with
/// its own code naming beta_deg, at both ends of the angle range; just inside the regime boundary (ratio 0.9) the
/// job is accepted and verified; inside the regime a pattern the verifier confirms is accepted (never refused for
/// the bound alone); an accepted near-180 degree part shows eps_num prominently.
#[test]
fn a_branch_angle_the_verification_cannot_confirm_is_refused_at_both_ends() {
    let st = |beta: f64| {
        stub(
            json!({"beta_deg": beta, "s_end": ((250.0 / beta.to_radians().sin()) + 150.0f64).ceil()}),
        )
    };
    // acute end, branch stub: just inside the boundary of the regime, accepted and verified
    let b_in = angle_at(&st, 0.005, 1.0, 0.9);
    assert!((0.01..0.05).contains(&b_in), "{b_in}");
    accepted_and_verified(&st(b_in));
    // inside the regime and confirmed by the verifier (ratio about 40 at 0.003 deg): accepted, not refused
    assert!(conditioning(&st(0.003)) > 1.0);
    accepted_and_verified(&st(0.003));
    // outside: the verifier's 3D check fails (G10 rejected this before 0.4.3): refused with the new code
    for beta in [0.002, 0.001] {
        let r = expect_refused(st(beta), "branch_angle_not_verifiable", &["beta_deg"]);
        assert!(
            r.message.contains(&format!("beta_deg = {beta}")),
            "{}",
            r.message
        );
        assert!(detail(&r, "measured") > detail(&r, "limit"));
    }
    // obtuse end, branch stub: the regime is never entered before the 1e8 mm size limit (the tolerance carries the
    // rounding of beta near pi, which grows as 1/sin²(beta) too); accepted to 179.9997 deg with eps_num prominent,
    // and the size limit refuses beyond
    for beta in [179.997, 179.9997] {
        assert!(conditioning(&st(beta)) < 1.0);
        let out = accepted_and_verified(&st(beta));
        assert!(out.patterns.iter().all(|p| p.eps_num_governs), "{beta}");
    }
    expect_refused(
        stub(json!({"beta_deg": 180.0 - 0.000141821, "s_end": 0.995e8})),
        "branch_angle_out_of_range",
        &["beta_deg"],
    );
    // a part at an ordinary angle does not carry the flag
    assert!(
        accepted_and_verified(&stub(json!({})))
            .patterns
            .iter()
            .all(|p| !p.eps_num_governs)
    );

    // header with branch, acute end: the joint checks govern
    let asm = |beta: f64| assembly(150.0, 100.0, beta, 0.0);
    let a_in = angle_at(&asm, 0.02, 1.0, 0.9);
    assert!((0.03..0.2).contains(&a_in), "{a_in}");
    accepted_and_verified(&asm(a_in));
    let r = expect_refused(asm(0.009), "branch_angle_not_verifiable", &["beta_deg"]);
    assert!(r.message.contains("beta_deg = 0.009"));
    // header with branch, obtuse end: just inside the regime boundary, and inside the regime (ratio above 1) the
    // verifier confirms: accepted, every part with eps_num prominent
    let a_ob = angle_at(&asm, 179.9, 179.9999, 0.9);
    assert!((179.9..179.99).contains(&a_ob), "{a_ob}");
    for beta in [a_ob, 179.99] {
        let out = accepted_and_verified(&asm(beta));
        assert!(out.patterns.iter().all(|p| p.eps_num_governs), "{beta}");
    }
    assert!(conditioning(&asm(179.99)) > 1.0);
    println!(
        "boundary (ratio 0.9): stub {b_in} deg; header with branch {a_in} deg and {a_ob} deg; ratios: stub 0.003 deg {:.1}, 0.002 deg {:.1}; assembly 0.009 deg {:.1}, 179.99 deg {:.2}",
        conditioning(&st(0.003)),
        conditioning(&st(0.002)),
        conditioning(&asm(0.009)),
        conditioning(&asm(179.99))
    );
}

#[test]
fn a_square_end_must_lie_beyond_the_saddle() {
    let r = expect_refused(
        stub(json!({"s_end": 0.5})),
        "square_end_too_short",
        &["s_end"],
    );
    assert!(
        r.detail.is_none(),
        "the saddle height is sampled, so it is not given as an exact limit"
    );
    // the limit stated in the message is the engine's own: a square end just beyond it is accepted
    let limit: f64 = r
        .message
        .rsplit("s_end > ")
        .next()
        .and_then(|s| {
            s.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.')
                .parse()
                .ok()
        })
        .expect("the message states the limit");
    expect_refused(
        stub(json!({"s_end": limit * 0.999})),
        "square_end_too_short",
        &["s_end"],
    );
    expect_accepted(stub(json!({"s_end": limit * 1.001})));
}

#[test]
fn an_opening_must_lie_inside_the_shell() {
    let hwb = |station: f64| {
        json!({"family": "header_with_branch", "header_radius": 161.95, "header_dev_radius": 156.795,
            "header_length": 1200.0, "branch_station": station, "branch_radius": 84.15, "branch_dev_radius": 80.6,
            "beta_deg": 90.0, "delta": 0.0, "s_end": 400.0, "surface_reference": "unknown"})
    };
    expect_accepted(hwb(600.0));
    expect_refused(
        hwb(1199.9),
        "opening_outside_shell",
        &["branch_station", "header_length"],
    );
    expect_refused(
        hwb(10.0),
        "opening_outside_shell",
        &["branch_station", "header_length"],
    );
}

#[test]
fn a_set_on_joint_needs_the_branch_inside_and_the_header_outside_surface() {
    let set_on = |h: &str, b: &str, joint: Value| {
        json!({"family": "header_with_branch", "header_radius": 161.95, "header_dev_radius": 156.795,
            "header_length": 1200.0, "branch_station": 600.0, "branch_radius": 77.095, "branch_dev_radius": 80.6,
            "beta_deg": 90.0, "delta": 0.0, "s_end": 400.0,
            "header_surface_reference": h, "branch_surface_reference": b, "joint": joint})
    };
    expect_accepted(set_on("outside", "inside", json!("set_on")));
    // a null joint is refused as an unknown field, as at 0.4.0 (the accepted set is unchanged): joint is absent instead
    expect_refused(
        set_on("outside", "inside", Value::Null),
        "field_unknown",
        &["joint"],
    );
    expect_refused(
        set_on("inside", "inside", json!("set_on")),
        "joint_pairing_invalid",
        &[
            "joint",
            "header_surface_reference",
            "branch_surface_reference",
        ],
    );
    expect_refused(
        set_on("outside", "outside", json!("set_on")),
        "joint_pairing_invalid",
        &[
            "joint",
            "header_surface_reference",
            "branch_surface_reference",
        ],
    );
    expect_refused(
        set_on("outside", "inside", json!("set-on")),
        "joint_type_unknown",
        &["joint"],
    );
}

#[test]
fn unknown_fields_are_refused_by_name() {
    let r = expect_refused(
        json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 250.0, "thickness": 12.0}),
        "field_unknown",
        &["thickness"],
    );
    assert!(r.message.contains("thickness"));
}

#[test]
fn request_structure_is_refused_with_its_own_codes() {
    expect_refused(json!([1, 2]), "request_not_object", &[]);
    expect_refused(
        json!({"r_dev": 100.0, "length": 250.0}),
        "family_unknown",
        &["family"],
    );
    expect_refused(json!({"family": "teapot"}), "family_unknown", &["family"]);
    expect_refused(
        json!({"family": "cylinder_shell", "r_dev": 100.0}),
        "field_missing",
        &["length"],
    );
    expect_refused(
        json!({"family": "header_with_branch", "header_surface_reference": "outside", "header_radius": 150.0,
               "header_dev_radius": 150.0, "header_length": 1200.0, "branch_station": 600.0, "branch_radius": 100.0,
               "branch_dev_radius": 100.0, "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0}),
        "surface_reference_required",
        &[
            "surface_reference",
            "header_surface_reference",
            "branch_surface_reference",
        ],
    );
}

#[test]
fn reducers_and_developments_refuse_with_their_own_codes() {
    expect_refused(
        json!({"family": "cone_frustum", "r1": 100.0, "r2": 200.0, "h": 300.0}),
        "reducer_radii_order",
        &["r1", "r2"],
    );
    // B2-005: until 0.4.1 (200, 199.99, 300) was refused by sin(alpha) < 1e-3; now only the apex radius
    // r1/sin(alpha) decides (here 6e6 mm) and the limit is 1e8 mm
    expect_accepted(json!({"family": "cone_frustum", "r1": 200.0, "r2": 199.99, "h": 300.0}));
    let r = expect_refused(
        json!({"family": "cone_frustum", "r1": 1e6, "r2": 1e6 - 1.0, "h": 1e6}),
        "reducer_near_cylindrical",
        &["r1", "r2", "h"],
    );
    assert_eq!(detail(&r, "limit"), 1e8);
    assert!(detail(&r, "measured") > 1e8);
    // B2-005: until 0.4.1 h = 0.01 (99.9 % of a turn) was refused as overlapping; the sector never overlaps,
    // it is refused only when its seam gap at the small end is at or below 1e-9 mm
    expect_accepted(json!({"family": "cone_frustum", "r1": 300.0, "r2": 50.0, "h": 0.01}));
    let r = expect_refused(
        json!({"family": "cone_frustum", "r1": 300.0, "r2": 50.0, "h": 1e-5}),
        "developed_angle_overlaps",
        &["r1", "r2", "h"],
    );
    assert_eq!(detail(&r, "limit"), 1e-9);
    assert!(detail(&r, "measured") <= detail(&r, "limit"));
}

#[test]
fn a_curve_that_cannot_be_represented_within_the_chord_error_is_refused() {
    let r = expect_refused(
        json!({"family": "branch_stub", "header_radius": 1e6, "branch_radius": 1e6 * (1.0 - 1.1e-6),
               "branch_dev_radius": 1e6 * (1.0 - 1.1e-6), "beta_deg": 15.0, "delta": 0.0, "s_end": 1e7}),
        "chord_error_unreachable",
        &[
            "header_radius",
            "branch_radius",
            "branch_dev_radius",
            "beta_deg",
            "delta",
        ],
    );
    assert!(r.detail.is_none());
}

/// The codes of the table in docs/backend/REFUSAL_CODES.md: the first cell of each row, in backquotes.
fn table_codes() -> Vec<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/backend/REFUSAL_CODES.md"
    );
    let text = std::fs::read_to_string(path).expect("docs/backend/REFUSAL_CODES.md");
    text.lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|rest| rest.split('`').next())
        .map(str::to_string)
        .collect()
}

#[test]
fn table_lists_every_code_of_the_build_once() {
    let codes = table_codes();
    let mut unique = codes.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        codes.len(),
        "a code is listed twice in the table"
    );
    let mut built: Vec<&str> = nf_core::refusal::ALL.to_vec();
    built.sort();
    let mut listed: Vec<&str> = codes.iter().map(String::as_str).collect();
    listed.sort();
    assert_eq!(
        listed, built,
        "the table and the codes of this build disagree"
    );
    assert!(
        codes
            .iter()
            .all(|c| c.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_')),
        "codes are snake_case"
    );
}

/// B2-005: the developed-size limit of the conical and branch families is the job's magnitude limit.
#[test]
fn the_developed_size_limit_equals_the_magnitude_limit() {
    assert_eq!(uem::develop::MAX_DEVELOPED_MM, MAX_MAGNITUDE_MM);
}
