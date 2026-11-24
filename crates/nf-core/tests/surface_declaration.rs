//! Work plan task 1.4: the surface the entered radius refers to, for the cylinder shell, the cone and the eccentric
//! reducer (the same declaration as `branch_stub` and `header_with_branch`).
//!
//! * absent: not declared. The request is exactly as it was before the field existed: no key in its serialised form,
//!   and the same pattern, checks, cut geometry and state as the compat fixture records for it.
//! * `outside` / `inside` / `neutral`: recorded, and the verdicts and the geometry are those of the absent request.
//! * `unknown`: every radius-dependent check that would PASS is INCONCLUSIVE with `SURFACE_UNKNOWN_REASON`; the other
//!   checks are those of the absent request; the cut geometry is unchanged.

use nf_core::job::{Request, run};
use nf_core::refusal;
use serde_json::{Value, json};
use uem::verify::{
    CONE_RADIUS_CHECKS, CYLINDER_RADIUS_CHECKS, ECCENTRIC_RADIUS_CHECKS, SURFACE_UNKNOWN_REASON,
};

/// One valid request per family, without the field.
fn base(family: &str) -> Value {
    match family {
        "cylinder_shell" => json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 250.0}),
        "cone_frustum" => json!({"family": "cone_frustum", "r1": 100.0, "r2": 60.0, "h": 200.0}),
        "eccentric_reducer" => {
            json!({"family": "eccentric_reducer", "r1": 100.0, "r2": 60.0, "h": 200.0, "e": 30.0})
        }
        _ => unreachable!(),
    }
}

fn with_surface(family: &str, s: Value) -> Value {
    let mut v = base(family);
    v["surface_reference"] = s;
    v
}

const FAMILIES: [&str; 3] = ["cylinder_shell", "cone_frustum", "eccentric_reducer"];

fn radius_checks(family: &str) -> &'static [&'static str] {
    match family {
        "cylinder_shell" => CYLINDER_RADIUS_CHECKS,
        "cone_frustum" => CONE_RADIUS_CHECKS,
        _ => ECCENTRIC_RADIUS_CHECKS,
    }
}

fn run_body(body: &Value) -> nf_core::job::JobOutput {
    let req = Request::parse_strict(body).expect("the request parses");
    run(&req, "surface-test")
}

fn verdicts(out: &nf_core::job::JobOutput) -> Vec<(String, String, Option<String>)> {
    out.checks
        .iter()
        .map(|c| (c.name.clone(), c.verdict.clone(), c.reason.clone()))
        .collect()
}

#[test]
fn every_family_accepts_the_four_words_and_the_base_request_is_accepted() {
    for family in FAMILIES {
        let absent = run_body(&base(family));
        assert_eq!(
            absent.outcome, None,
            "{family}: the base request is accepted"
        );
        for s in ["outside", "inside", "neutral", "unknown"] {
            let req = Request::parse_strict(&with_surface(family, json!(s)));
            assert!(req.is_ok(), "{family} {s}: {req:?}");
        }
    }
}

#[test]
fn absent_field_is_not_serialised_so_old_requests_serialise_as_before() {
    for family in FAMILIES {
        let req = Request::parse_strict(&base(family)).expect("parses");
        let v = serde_json::to_value(&req).expect("json");
        assert!(v.get("surface_reference").is_none(), "{family}: {v}");
        let out = run(&req, "surface-test");
        let text = serde_json::to_string(&out).expect("json");
        assert!(
            !text.contains("surface_reference"),
            "{family}: the output of an absent request names the field"
        );
    }
}

#[test]
fn a_wrong_surface_value_is_refused_with_its_code_and_a_wrong_type_names_the_field() {
    for family in FAMILIES {
        let out = run_body(&with_surface(family, json!("bogus")));
        let r = out.refusal.expect("refused");
        assert_eq!(r.code, refusal::SURFACE_REFERENCE_INVALID, "{family}");
        assert_eq!(out.refused_field.as_deref(), Some("surface_reference"));
        for bad in [json!(null), json!(3)] {
            let e = Request::parse_refusal(&with_surface(family, bad)).expect_err("refused");
            assert_eq!(e.code, refusal::FIELD_WRONG_TYPE, "{family}");
            assert!(
                e.fields.iter().any(|f| f == "surface_reference"),
                "{family}: {e:?}"
            );
        }
    }
}

#[test]
fn unknown_makes_every_radius_check_inconclusive_with_its_reason_and_nothing_else_moves() {
    for family in FAMILIES {
        let absent = run_body(&base(family));
        let unknown = run_body(&with_surface(family, json!("unknown")));
        let names: Vec<String> = absent.checks.iter().map(|c| c.name.clone()).collect();
        for want in radius_checks(family) {
            assert!(
                names.iter().any(|n| n == want),
                "{family}: the verifier no longer has the radius check {want}; the list is stale"
            );
        }
        let before = verdicts(&absent);
        let after = verdicts(&unknown);
        assert_eq!(before.len(), after.len(), "{family}: check count");
        let flipped = before
            .iter()
            .zip(&after)
            .filter(|((n, v, _), (_, w, _))| {
                v == "PASS" && w == "INCONCLUSIVE" && radius_checks(family).contains(&n.as_str())
            })
            .count();
        assert!(
            flipped >= 2,
            "{family}: only {flipped} radius checks were PASS to begin with"
        );
        for ((name, v_before, _), (name2, v_after, reason)) in before.iter().zip(&after) {
            assert_eq!(name, name2, "{family}: check order");
            if radius_checks(family).contains(&name.as_str()) {
                assert_ne!(
                    v_after, "PASS",
                    "{family} {name}: an unknown radius never passes"
                );
                if v_before == "PASS" {
                    assert_eq!(v_after, "INCONCLUSIVE", "{family} {name}");
                    assert_eq!(
                        reason.as_deref(),
                        Some(SURFACE_UNKNOWN_REASON),
                        "{family} {name}"
                    );
                } else {
                    assert_eq!(v_after, v_before, "{family} {name}: a FAIL stays FAIL");
                }
            } else {
                assert_eq!(v_after, v_before, "{family} {name}: not radius-dependent");
                assert_eq!(reason, &None, "{family} {name}");
            }
        }
        // the gate of the verifier says so, and the reason is in its text as well
        let g10 = unknown.gates.iter().find(|g| g.id == "G10").expect("G10");
        assert_eq!(
            serde_json::to_value(g10.result).unwrap(),
            "INCONCLUSIVE",
            "{family}"
        );
        assert!(g10.detail.contains(SURFACE_UNKNOWN_REASON), "{family}");
        // the geometry, the patterns and the state are those of the absent request
        assert_eq!(
            unknown.dxf, absent.dxf,
            "{family}: DXF text (cut geometry and annotation)"
        );
        assert_eq!(
            serde_json::to_value(&unknown.patterns).unwrap(),
            serde_json::to_value(&absent.patterns).unwrap(),
            "{family}: patterns"
        );
        assert_eq!(
            serde_json::to_value(unknown.state).unwrap(),
            serde_json::to_value(absent.state).unwrap(),
            "{family}: state"
        );
    }
}

#[test]
fn declared_outside_inside_neutral_leave_verdicts_and_geometry_as_they_are_without_the_field() {
    for family in FAMILIES {
        let absent = run_body(&base(family));
        for s in ["outside", "inside", "neutral"] {
            let out = run_body(&with_surface(family, json!(s)));
            assert_eq!(verdicts(&out), verdicts(&absent), "{family} {s}: verdicts");
            assert_eq!(out.dxf, absent.dxf, "{family} {s}: DXF text");
            assert_eq!(
                serde_json::to_value(&out.patterns).unwrap(),
                serde_json::to_value(&absent.patterns).unwrap(),
                "{family} {s}: patterns"
            );
            assert_eq!(out.state, absent.state, "{family} {s}: state");
            assert_eq!(out.outcome, absent.outcome, "{family} {s}: outcome");
            // the declaration is recorded in the gate that asks for it
            let g02 = out.gates.iter().find(|g| g.id == "G02").expect("G02");
            assert!(
                g02.detail.contains(s),
                "{family} {s}: G02 names the declared surface"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Task 1.4, branch families (RB-010: "unknown never switches a check off", for every family).

use uem::verify::{
    BRANCH_PART_BRANCH_CHECKS, BRANCH_PART_HEADER_CHECKS, BRANCH_PART_JOINT_CHECKS,
    BRANCH_PART_NO_RADIUS_CHECKS, BRANCH_STUB_RADIUS_CHECKS, HEADER_OPENING_HEADER_CHECKS,
    HEADER_OPENING_JOINT_CHECKS, HEADER_OPENING_NO_RADIUS_CHECKS, INTERFACE_JOINT_CHECKS,
    INTERFACE_NO_RADIUS_CHECKS,
};

fn branch_base() -> Value {
    json!({"family": "branch_stub", "header_radius": 150.0, "branch_radius": 100.0,
           "branch_dev_radius": 100.0, "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0})
}

fn assembly_base() -> Value {
    json!({"family": "header_with_branch", "header_radius": 150.0, "header_dev_radius": 150.0,
           "header_length": 1200.0, "branch_station": 600.0, "branch_radius": 100.0,
           "branch_dev_radius": 100.0, "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0})
}

/// (name, verdict, reason) of every check of a run.
fn rows(out: &nf_core::job::JobOutput) -> Vec<(String, String, Option<String>)> {
    out.checks
        .iter()
        .map(|c| (c.name.clone(), c.verdict.clone(), c.reason.clone()))
        .collect()
}

/// Asserts the rule on one run: a check named in `affected` that would PASS is INCONCLUSIVE with the reason; every
/// other check is exactly as in `plain` (the run whose surfaces are all known); no check is PASS when affected.
fn assert_flip_rule(
    what: &str,
    plain: &[(String, String, Option<String>)],
    unknown: &[(String, String, Option<String>)],
    affected: &[&str],
) -> usize {
    assert_eq!(plain.len(), unknown.len(), "{what}: check count");
    let mut flipped = 0;
    for ((n1, v1, _), (n2, v2, reason)) in plain.iter().zip(unknown) {
        assert_eq!(n1, n2, "{what}: order");
        if affected.contains(&n1.as_str()) {
            if v1 == "PASS" {
                assert_eq!(v2, "INCONCLUSIVE", "{what} {n1}");
                assert_eq!(
                    reason.as_deref(),
                    Some(SURFACE_UNKNOWN_REASON),
                    "{what} {n1}"
                );
                flipped += 1;
            } else {
                assert_eq!(v2, v1, "{what} {n1}: a FAIL stays FAIL");
            }
        } else {
            assert_eq!(v2, v1, "{what} {n1}: not affected by this surface");
            assert_eq!(reason, &None, "{what} {n1}");
        }
    }
    flipped
}

#[test]
fn branch_stub_unknown_flips_its_radius_checks_and_a_known_surface_does_not() {
    let plain = run_body(&with_surface_branch(json!("outside")));
    let unknown = run_body(&with_surface_branch(json!("unknown")));
    assert_eq!(plain.outcome, None);
    assert_eq!(unknown.outcome, None);
    let flipped = assert_flip_rule(
        "branch_stub",
        &rows(&plain),
        &rows(&unknown),
        BRANCH_STUB_RADIUS_CHECKS,
    );
    assert!(
        flipped >= 5,
        "branch_stub: only {flipped} radius checks were PASS"
    );
    // a known surface never carries a reason
    for s in ["outside", "inside", "neutral"] {
        let out = run_body(&with_surface_branch(json!(s)));
        assert!(rows(&out).iter().all(|r| r.2.is_none()), "{s}");
    }
    // the default of an absent field is `unknown` (the branch stub's existing default), so it flips the same way
    let absent = run_body(&branch_base());
    assert_eq!(rows(&absent), rows(&unknown));
    // the completeness of the list: every check the verifier produces is listed or is radius-free
    let names: Vec<String> = plain.checks.iter().map(|c| c.name.clone()).collect();
    for n in &names {
        assert!(
            BRANCH_STUB_RADIUS_CHECKS.contains(&n.as_str())
                || BRANCH_PART_NO_RADIUS_CHECKS.contains(&n.as_str()),
            "branch_stub: check {n} is not classified"
        );
    }
    for want in BRANCH_STUB_RADIUS_CHECKS {
        assert!(names.iter().any(|n| n == want), "stale: {want}");
    }
    // dimensional fidelity: the DXF and the pattern are those of the known-surface run
    assert_eq!(unknown.dxf, plain.dxf);
    assert_eq!(
        serde_json::to_value(&unknown.patterns).unwrap(),
        serde_json::to_value(&plain.patterns).unwrap()
    );
}

fn with_surface_branch(s: Value) -> Value {
    let mut v = branch_base();
    v["surface_reference"] = s;
    v
}

#[test]
fn header_with_branch_splits_the_flip_by_the_surface_of_each_part() {
    let mut body = assembly_base();
    body["header_surface_reference"] = json!("outside");
    body["branch_surface_reference"] = json!("outside");
    let plain = run_body(&body);
    assert_eq!(
        plain.outcome, None,
        "the assembly is accepted with known surfaces"
    );
    // branch unknown, header known: the header's own checks keep their verdicts; the branch-part checks that use the
    // branch radius and every joint check (both radii) become INCONCLUSIVE
    body["branch_surface_reference"] = json!("unknown");
    let b_unknown = run_body(&body);
    let affected_b: Vec<&str> = BRANCH_PART_BRANCH_CHECKS
        .iter()
        .chain(BRANCH_PART_JOINT_CHECKS)
        .chain(HEADER_OPENING_JOINT_CHECKS)
        .chain(INTERFACE_JOINT_CHECKS)
        .copied()
        .collect();
    let f_b = assert_flip_rule(
        "branch unknown",
        &rows(&plain),
        &rows(&b_unknown),
        &affected_b,
    );
    assert!(f_b >= 5, "branch unknown: only {f_b} flips");
    // header unknown, branch known
    body["header_surface_reference"] = json!("unknown");
    body["branch_surface_reference"] = json!("outside");
    let h_unknown = run_body(&body);
    let affected_h: Vec<&str> = BRANCH_PART_HEADER_CHECKS
        .iter()
        .chain(HEADER_OPENING_HEADER_CHECKS)
        .chain(BRANCH_PART_JOINT_CHECKS)
        .chain(HEADER_OPENING_JOINT_CHECKS)
        .chain(INTERFACE_JOINT_CHECKS)
        .copied()
        .collect();
    let f_h = assert_flip_rule(
        "header unknown",
        &rows(&plain),
        &rows(&h_unknown),
        &affected_h,
    );
    assert!(f_h >= 5, "header unknown: only {f_h} flips");
}

#[test]
fn every_check_of_an_assembly_is_classified_by_exactly_one_rule() {
    let mut body = assembly_base();
    body["header_surface_reference"] = json!("outside");
    body["branch_surface_reference"] = json!("outside");
    let out = run_body(&body);
    let known: Vec<&str> = [
        HEADER_OPENING_HEADER_CHECKS,
        HEADER_OPENING_JOINT_CHECKS,
        HEADER_OPENING_NO_RADIUS_CHECKS,
        BRANCH_PART_HEADER_CHECKS,
        BRANCH_PART_BRANCH_CHECKS,
        BRANCH_PART_JOINT_CHECKS,
        BRANCH_PART_NO_RADIUS_CHECKS,
        INTERFACE_JOINT_CHECKS,
        INTERFACE_NO_RADIUS_CHECKS,
    ]
    .iter()
    .flat_map(|l| l.iter().copied())
    .collect();
    for c in &out.checks {
        assert!(
            known.contains(&c.name.as_str()),
            "assembly check {} ({}) is not classified",
            c.name,
            c.scope
        );
    }
    for want in known {
        assert!(
            out.checks.iter().any(|c| c.name == want),
            "stale list entry {want}"
        );
    }
}
