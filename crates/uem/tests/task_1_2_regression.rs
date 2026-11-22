//! Work plan task 1.2 regression cases (TOLERANCE_POLICY section 8). The two near-tangent FAILs of task 0.3 and
//! their expected behaviour after the change: the chord error is bounded above the measured truth and below the
//! 0.01 mm limit, the developed-length checks resolve, and the area check of the tiny opening passes.

use uem::develop::{Branch, branch_stub, header_with_opening};
use uem::verify::{Verdict, verify_branch, verify_header_opening};

const AGREE: f64 = 1e-6;

/// N-tangent-r-b90-g2e-06: R 150, r = R(1 − 2e-6), beta 90 deg, delta 0; the header is 600 long with the
/// branch at station 450 (the request the validation harness builds from this geometry).
fn near_tangent() -> (Branch, f64, f64) {
    let big_r = 150.0_f64;
    let r = big_r * (1.0 - 2e-6);
    (
        Branch {
            big_r,
            r,
            beta: 90f64.to_radians(),
            delta: 0.0,
        },
        r,
        151.0,
    )
}

#[test]
fn near_tangent_chord_bound_is_certified_between_the_truth_and_the_limit() {
    let (b, r, s_end) = near_tangent();
    let stub = branch_stub(b.big_r, r, r, 90.0, 0.0, s_end).unwrap();
    assert!(
        stub.eps_rep_certified,
        "the saddle of this joint must be certified"
    );
    // the measured true chord error of this polyline is 0.0096085 mm (task 0.3); the bound must cover it,
    // stay under the 0.01 mm chord limit, and is 0.0096380 mm at the current density (task 1.2 item A)
    assert!(
        stub.eps_rep >= 0.0096085,
        "bound {} below the true chord error",
        stub.eps_rep
    );
    assert!(
        stub.eps_rep <= 0.00964,
        "bound {} above the expected value",
        stub.eps_rep
    );
    assert!(stub.eps_rep < 0.01);
    assert_eq!(
        stub.quantity("polyline_segments"),
        Some(5760.0),
        "the density is unchanged (cut geometry)"
    );
    let header = header_with_opening(&b, b.big_r, 600.0, 450.0).unwrap();
    assert!(header.eps_rep_certified);
    assert!(header.eps_rep >= 0.0096085 && header.eps_rep < 0.00964);
}

#[test]
fn near_tangent_developed_length_checks_resolve_and_pass() {
    let (b, r, s_end) = near_tangent();
    let stub = branch_stub(b.big_r, r, r, 90.0, 0.0, s_end).unwrap();
    let header = header_with_opening(&b, b.big_r, 600.0, 450.0).unwrap();
    let length_checks = |v: Vec<uem::verify::Check>| -> Vec<uem::verify::Check> {
        v.into_iter()
            .filter(|c| c.name.ends_with("developed_length_equals_3d_arc_length"))
            .collect()
    };
    let s = length_checks(verify_branch(&stub, &b, r, s_end));
    let h = length_checks(verify_header_opening(&header, &b, b.big_r, 600.0, 450.0));
    assert_eq!(s.len(), 1);
    assert_eq!(h.len(), 1);
    for c in s.iter().chain(h.iter()) {
        // task 0.3: INCONCLUSIVE (resolution 1.29e-2 against a need of 1.15e-2); now the certified enclosure resolves
        assert_eq!(
            c.verdict,
            Verdict::Pass,
            "{}: difference {:e}",
            c.name,
            c.difference
        );
        let resolution = c.tolerance - AGREE;
        assert!(
            resolution < 1e-5 * c.reference.abs(),
            "{}: resolution {resolution:e}",
            c.name
        );
        assert!(c.difference <= c.tolerance);
    }
}

#[test]
fn near_tangent_checks_all_pass_without_inconclusive_verdicts() {
    let (b, r, s_end) = near_tangent();
    let stub = branch_stub(b.big_r, r, r, 90.0, 0.0, s_end).unwrap();
    let header = header_with_opening(&b, b.big_r, 600.0, 450.0).unwrap();
    for c in verify_branch(&stub, &b, r, s_end) {
        assert_eq!(c.verdict, Verdict::Pass, "stub {}", c.name);
    }
    for c in verify_header_opening(&header, &b, b.big_r, 600.0, 450.0) {
        assert_eq!(c.verdict, Verdict::Pass, "header {}", c.name);
    }
}

/// N-aspect-branch-tiny: R 1000, r 1e-3 (r/R = 1e-6), beta 60 deg, header 2001 long, station 423.
#[test]
fn tiny_opening_area_is_checked_against_the_opening_not_the_plate() {
    let b = Branch {
        big_r: 1000.0,
        r: 0.001,
        beta: 60f64.to_radians(),
        delta: 0.0,
    };
    let header = header_with_opening(&b, 1000.0, 2001.0, 423.0).unwrap();
    let area = verify_header_opening(&header, &b, 1000.0, 2001.0, 423.0)
        .into_iter()
        .find(|c| c.name == "opening_area_equals_surface_area")
        .unwrap();
    assert_eq!(
        area.verdict,
        Verdict::Pass,
        "difference {:e}",
        area.difference
    );
    // task 0.3: the tolerance was 8.3e-3 mm² (the whole plate); now it is one millionth of the opening's perimeter
    assert!(
        area.tolerance < 1e-7,
        "tolerance {:e} is not scaled to the opening",
        area.tolerance
    );
    // the reported opening area is the area of the 720-gon of the drawn opening, computed on coordinates relative to
    // the first vertex (task 1.2 item B); the limit area differs from it by the polygon's own discretization,
    // about 1.3e-5 relative (measured), so the bound here is 2e-5
    let quantity = header.quantity("opening_area").unwrap();
    assert!(
        (quantity - area.reference).abs() < 2e-5 * area.reference,
        "{quantity:e} vs {:e}",
        area.reference
    );
}

/// stress_branch (task 1.2 regression): a small stub at 140.6 deg. Its developed-length check was PASS at 0.4.3 and
/// became INCONCLUSIVE with the first certified excess bound (the box enclosure of the derivative); it must PASS.
#[test]
fn small_obtuse_stub_developed_length_is_pass() {
    let (big_r, r, rd, beta, s_end) = (
        5.2900810455438725_f64,
        5.284866413537644_f64,
        5.383839537433899_f64,
        140.60539373544853_f64,
        16.0362989826968_f64,
    );
    let stub = branch_stub(big_r, r, rd, beta, 0.0, s_end).unwrap();
    let b = Branch {
        big_r,
        r,
        beta: beta.to_radians(),
        delta: 0.0,
    };
    let length = verify_branch(&stub, &b, rd, s_end)
        .into_iter()
        .find(|c| c.name == "saddle_developed_length_equals_3d_arc_length")
        .unwrap();
    assert_eq!(
        length.verdict,
        Verdict::Pass,
        "difference {:e} tolerance {:e}",
        length.difference,
        length.tolerance
    );
}
