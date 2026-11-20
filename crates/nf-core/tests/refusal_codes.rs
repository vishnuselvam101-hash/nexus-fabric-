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
fn the_angle_between_the_axes_is_limited_by_its_sine() {
    // sin(beta) must exceed 0.17 (about 9.79 degrees) and beta must lie strictly between 0 and 180
    expect_accepted(stub(json!({"beta_deg": 9.9, "s_end": 1e6})));
    expect_refused(
        stub(json!({"beta_deg": 9.5})),
        "branch_angle_out_of_range",
        &["beta_deg"],
    );
    expect_accepted(stub(json!({"beta_deg": 169.9, "s_end": 1e6})));
    // the limit is symmetric about 90 degrees: sin(170.1) = sin(9.9) is inside it
    expect_accepted(stub(json!({"beta_deg": 170.1, "s_end": 1e6})));
    expect_refused(
        stub(json!({"beta_deg": 170.3})),
        "branch_angle_out_of_range",
        &["beta_deg"],
    );
    expect_refused(
        stub(json!({"beta_deg": 180.0})),
        "branch_angle_out_of_range",
        &["beta_deg"],
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
    let r = expect_refused(
        json!({"family": "cone_frustum", "r1": 200.0, "r2": 199.99, "h": 300.0}),
        "reducer_near_cylindrical",
        &["r1", "r2", "h"],
    );
    assert_eq!(detail(&r, "limit"), 1e-3);
    let r = expect_refused(
        json!({"family": "cone_frustum", "r1": 300.0, "r2": 50.0, "h": 0.01}),
        "developed_angle_overlaps",
        &["r1", "r2", "h"],
    );
    assert!(detail(&r, "measured") >= detail(&r, "limit"));
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
