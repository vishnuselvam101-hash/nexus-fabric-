//! Verification checks (Spec 02 §9). Each check consumes only the original
//! inputs and the primary *result* (ARCH-03 / MATH-40).
//!
//! Two kinds of check live here, and the difference matters (review RB-006):
//!
//! * FOLD-BACK checks map pattern points back to 3D with the inverse of the
//!   chart the pattern was built with and test implicit equations. They catch
//!   wrong numbers; they cannot catch a wrong chart, because they share it.
//! * INDEPENDENT-FORMULATION checks compare the pattern with values computed
//!   from the inputs in `independent`: solids in a tilted frame, sampled in
//!   3D, intersected by a numerical root search, with lengths from 3D chord
//!   sums. They share no formula with `develop`.
//!
//! In this build both kinds live in the same crate as the primary code and
//! were written by the same author; a separately owned implementation (class
//! I-I/I-L) does not exist yet and the evidence says so.

use crate::curve_bound::{CurveBound, OpeningCurve, SaddleCurve, length_excess_bound};
use crate::develop::{self, Branch, MAX_CHORD_ERROR_MM, ObliqueCone};
use crate::independent::{self, Joint3, V3};
#[cfg(test)]
use crate::{Curve, Layer};
use crate::{Geom, P2, Pattern, TAU, ViewSide, gauss, profile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Clone, Debug)]
pub struct Check {
    pub name: &'static str,
    /// How the reference value was obtained (different from the primary method).
    pub method: &'static str,
    /// Independence class from Spec 02 §9.1 (I-F = different formulation).
    pub independence: &'static str,
    pub primary: f64,
    pub reference: f64,
    pub difference: f64,
    pub tolerance: f64,
    pub verdict: Verdict,
    /// Why the verdict is INCONCLUSIVE, where a rule of this build sets it (work plan task 1.4). `None` for every
    /// other check, including every check whose verdict is PASS or FAIL.
    pub reason: Option<&'static str>,
}

/// Why a radius-dependent check is INCONCLUSIVE when the surface the entered radius refers to is `unknown` (task
/// 1.4). Bilingual on purpose: the English and the Japanese are the user-visible reason (web/i18n.js carries the
/// Japanese as well).
pub const SURFACE_UNKNOWN_REASON: &str = "surface unknown: cannot tell which surface the radius is on, so the radius check cannot be certified. / 基準面が不明です: 半径がどの面を指すか判別できないため、半径に関する検査は確定できません。";

/// The checks whose REFERENCE value is computed from an entered radius (task 1.4). Their meaning depends on which
/// wall surface the radius is on, so with an `unknown` surface they cannot certify the radius. Every other check
/// compares the pattern with another curve of the same pattern or with the entered length, and is unaffected.
/// The names are asserted against the verifier's output in the tests, so a renamed check cannot drop out silently.
pub const CYLINDER_RADIUS_CHECKS: &[&str] = &["bottom_edge_equals_circumference", "area"];
pub const CONE_RADIUS_CHECKS: &[&str] = &[
    "large_arc_equals_circumference",
    "small_arc_equals_circumference",
    "seam_equals_slant_length",
    "area",
    "large_end_3d_arc_length",
    "small_end_3d_arc_length",
    "fold_back_radius_residual",
];
pub const ECCENTRIC_RADIUS_CHECKS: &[&str] = &[
    "apex_free_development_agrees",
    "large_edge_equals_circumference",
    "small_edge_equals_circumference",
    "seam_equals_3d_ruling",
    "area",
];

/// Branch stub (task 1.4, `verify_branch`): the request declares ONE surface for its contact radii, so every check
/// whose reference uses the header radius R, the branch radius r or the development radius r_dev is radius-dependent.
pub const BRANCH_STUB_RADIUS_CHECKS: &[&str] = &[
    "cut_points_on_header_surface",
    "cut_points_on_branch_surface",
    "saddle_handedness_matches_declared_side",
    "square_end_equals_circumference",
    "saddle_matches_3d_intersection",
    "saddle_on_the_declared_delta_side",
    "saddle_developed_length_equals_3d_arc_length",
    "pattern_area_equals_surface_area",
    "square_end_3d_arc_length",
];
/// Header with branch (task 1.4): each part has its own surface, so the radius checks split by the radii they use.
/// `verify_branch` checks that use the header radius R only.
pub const BRANCH_PART_HEADER_CHECKS: &[&str] = &["cut_points_on_header_surface"];
/// `verify_branch` checks that use the branch radius r or the branch development radius r_dev only.
pub const BRANCH_PART_BRANCH_CHECKS: &[&str] = &[
    "cut_points_on_branch_surface",
    "saddle_handedness_matches_declared_side",
    "square_end_equals_circumference",
    "square_end_3d_arc_length",
];
/// `verify_branch` checks that compare with the 3D intersection of R and r (both surfaces).
pub const BRANCH_PART_JOINT_CHECKS: &[&str] = &[
    "saddle_matches_3d_intersection",
    "saddle_on_the_declared_delta_side",
    "saddle_developed_length_equals_3d_arc_length",
    "pattern_area_equals_surface_area",
];
/// `verify_branch` checks that use no radius.
pub const BRANCH_PART_NO_RADIUS_CHECKS: &[&str] =
    &["outline_counter_clockwise", "seam_edges_equal_length"];
/// `verify_header_opening` checks that use the header development radius only.
pub const HEADER_OPENING_HEADER_CHECKS: &[&str] = &[
    "outline_extent_around_header",
    "shell_edge_equals_circumference",
    "opening_closed_simple_inside_shell",
    "header_circle_3d_arc_length",
];
/// `verify_header_opening` checks that use the branch radius or the 3D intersection (both surfaces).
pub const HEADER_OPENING_JOINT_CHECKS: &[&str] = &[
    "opening_points_on_branch_surface",
    "opening_centre_on_the_declared_delta_side",
    "opening_matches_3d_intersection",
    "opening_developed_length_equals_3d_arc_length",
    "opening_area_equals_surface_area",
];
/// `verify_header_opening` checks that use no radius.
pub const HEADER_OPENING_NO_RADIUS_CHECKS: &[&str] = &[
    "outline_extent_along_header_axis",
    "seam_edges_equal_length",
    "outline_counter_clockwise",
];
/// Interface and joint checks of a header with branch: each compares the two parts with the geometry of both radii.
pub const INTERFACE_JOINT_CHECKS: &[&str] = &[
    "branch_cut_lies_on_header_opening",
    "header_opening_lies_on_branch_cut",
    "opening_on_intersection_of_branch_geometry",
    "saddle_on_intersection_of_header_geometry",
];
/// Interface checks that use no radius.
pub const INTERFACE_NO_RADIUS_CHECKS: &[&str] = &["parts_declare_the_same_marking_side"];

/// Task 1.4: with the surface declared `unknown`, every radius-dependent check that would PASS becomes INCONCLUSIVE
/// with `SURFACE_UNKNOWN_REASON`. A FAIL stays FAIL (the conservative direction, as in `rep_check`). Any other
/// surface, or no declaration (`None`), returns the checks unchanged.
pub fn surface_unknown(
    checks: Vec<Check>,
    surface: Option<&str>,
    radius_checks: &[&str],
) -> Vec<Check> {
    if surface != Some("unknown") {
        return checks;
    }
    checks
        .into_iter()
        .map(|mut c| {
            if radius_checks.contains(&c.name) && c.verdict != Verdict::Fail {
                c.verdict = Verdict::Inconclusive;
                c.reason = Some(SURFACE_UNKNOWN_REASON);
            }
            c
        })
        .collect()
}

/// Agreement threshold used when both sides are binary64 evaluations whose
/// error is only estimated. Implementation default, not an engineering tolerance.
pub const AGREE_ABS: f64 = 1e-6;

/// Agreement threshold for a part whose largest dimension is `size` (mm):
/// `AGREE_ABS` (absolute, 1e-6 mm) for parts of 1 mm and larger, and
/// `AGREE_ABS · size` (relative, 1e-6 of the part) below 1 mm, so that the
/// threshold never approaches the size of the part itself. Nothing changes
/// for parts of 1 mm and above. See docs/backend/TOLERANCE_POLICY.md.
pub fn agree_tol(size: f64) -> f64 {
    if size.is_finite() && size > 0.0 && size < 1.0 {
        AGREE_ABS * size
    } else {
        AGREE_ABS
    }
}

// ------------------------------------------------------------------------------------------------------------
// The verifier's OWN rounding error at a branch angle (work plan task 1.1, engine 0.4.3).
//
// The three 3D-intersection checks of a branch joint (`saddle_matches_3d_intersection`,
// `opening_matches_3d_intersection` and the two checks of `verify_joint`) compare the pattern with the numerical
// intersection of `independent::Joint3`. Their tolerance is `agree_tol(size) + 4·bound`, where `bound` is the
// engine's a-priori error bound of the compared vertices (`develop::saddle_error_bound`,
// `develop::opening_error_bound`). The threshold says nothing about the verifier's own rounding, which grows as
// 1/sin²(beta) (the root search divides an error of the order u·s by the slope sin(beta)·cos(omega), with s itself of
// the order (R + r)/sin(beta)). A correct pattern is GUARANTEED to pass a check only when
//
//     engine error + verifier error <= agree + 4·bound,   i.e. (engine error <= bound)   F <= agree + 3·bound,
//
// with F the a-priori bound below. Where F exceeds that, the check can neither confirm a correct pattern nor tell
// it from a wrong one, so the engine refuses the angle with its own code (`branch_angle_not_verifiable`) instead
// of accepting a job that its own verification then fails (G10). Derivation in docs/backend/TOLERANCE_POLICY.md
// section 7. First-order analysis like `develop::s_error_bound`; u = 2^-53.
// ------------------------------------------------------------------------------------------------------------

/// Unit roundoff.
const U: f64 = develop::U;
/// Largest defect of the stored constants of the tilted frame from an orthonormal frame: |ex·ex − 1| = 1.0 u,
/// |ey·ey − 1| = 0.47 u, |ez·ez − 1| = 0.28 u, |ex·ez| = 0.17 u, ex·ey = ey·ez = 0 (exact rational evaluation of
/// the binary64 constants, asserted by `frame_defect_is_at_most_one_unit_roundoff`).
const FRAME_DEFECT: f64 = U;
/// |frame origin| / scale for `Frame::tilted` (sqrt(0.7² + 0.4² + 0.3²) = 0.8602…), rounded up.
const FRAME_ORIGIN: f64 = 0.87;

/// Geometry terms shared by the bounds: (sin beta, |cos beta|, cos omega_min, S, D) with
/// S = (R + r|cos beta|)/sin beta >= |s(phi)| and D = r (1 + y/root)/sin beta >= |ds/dphi|,
/// y = |delta| + r, root = sqrt(R² − y²), cos omega_min = root/R.
fn branch_terms(b: &Branch) -> (f64, f64, f64, f64, f64) {
    let sb = b.beta.sin().abs();
    let cb = b.beta.cos().abs();
    let y = b.delta.abs() + b.r;
    let root = (b.big_r * b.big_r - y * y).sqrt();
    let s_big = (b.big_r + b.r * cb) / sb;
    let d_big = b.r * (1.0 + y / root) / sb;
    (sb, cb, root / b.big_r, s_big, d_big)
}

/// Rounding of a point formed by `Joint3::branch_point` at |s| <= S in a frame of the given scale and station:
/// the branch origin (3 roundings of |station|, |delta|), the transverse part (radius r, rotated basis), d·s
/// (2 u of d, 1 u of the product), the sums (1 u of each partial sum).
fn branch_point_rounding(b: &Branch, scale: f64, station: f64, s_big: f64) -> f64 {
    U * (3.0 * FRAME_ORIGIN * scale
        + 5.0 * station.abs()
        + 5.0 * b.delta.abs()
        + 13.0 * b.r
        + 4.0 * s_big)
}

/// A-PRIORI BOUND on the error of `Joint3::s_at` (the numerical root on a branch ruling), every phi.
///   g(s) = |b + dp·s| − R,  slope g'(s*) = sin(beta)·cos(omega) >= sin(beta)·cos(omega_min);
///   dp = d − (d·ex)·ex: the part across the header axis carries <= 3u (d·ex and its error lie along ex);
///   b = off_axis(branch_point(phi, 0, r)): <= u·(2|o| + 6|station| + 6|delta| + 15 r);
///   evaluation of g at the root: <= u·(5R + |delta| + r);
///   stopping rule 2·eps·max(|s|, R) = 4u·max(S, R), plus the relative frame defect: 8u·max(S, R) in all.
pub fn intersection_root_bound(b: &Branch, scale: f64, station: f64) -> f64 {
    let (sb, _, cw, s_big, _) = branch_terms(b);
    let y = b.delta.abs() + b.r;
    let e_b =
        U * (2.0 * FRAME_ORIGIN * scale + 6.0 * station.abs() + 6.0 * b.delta.abs() + 15.0 * b.r);
    let e_f = U * (5.0 * b.big_r + y);
    (3.0 * U * s_big + e_b + e_f) / (sb * cw) + 8.0 * U * s_big.max(b.big_r)
}

/// A-priori bound on the verifier's own error in `saddle_matches_3d_intersection` (branch stub): the root bound
/// in the frame of `Joint3::new` (scale R, station 0) plus the fold of the vertex parameter phi = x / r_dev
/// (2u·tau) through |ds/dphi| <= D.
pub fn saddle_check_rounding(b: &Branch) -> f64 {
    let (_, _, _, _, d_big) = branch_terms(b);
    intersection_root_bound(b, b.big_r, 0.0) + 2.0 * U * TAU * d_big
}

/// A-priori bound on the verifier's own error in `opening_matches_3d_intersection`: the computed curve point
/// (root bound and `branch_point` rounding), its header chart (x = q·ex; omega = atan2 of two components of
/// q, |q| <= |station| + S + |delta| + r, at radius R), the plate value v = R_dev·(omega + pi), and the
/// rounding of the vertex parameter (3u·tau) through |dx/dphi| <= D + r and |dv/dphi| <= R_dev·r/root.
pub fn opening_check_rounding(b: &Branch, r_dev_header: f64, station: f64) -> f64 {
    let (_, _, cw, s_big, d_big) = branch_terms(b);
    let q = station.abs() + s_big + b.delta.abs() + b.r;
    let e_pt = intersection_root_bound(b, b.big_r, station)
        + branch_point_rounding(b, b.big_r, station, s_big);
    let dx = e_pt + 4.0 * U * q;
    let d_trans = e_pt + (4.0 * U + FRAME_DEFECT) * q;
    let d_omega = std::f64::consts::SQRT_2 * d_trans / b.big_r + 2.0 * U * TAU;
    let dv = r_dev_header * (d_omega + 3.0 * U * TAU + 1.3e-16);
    let root = cw * b.big_r;
    dx + dv + 3.0 * U * TAU * (d_big + b.r + r_dev_header * b.r / root)
}

/// A-priori bound on the verifier's own error in the two checks of `verify_joint` (frame of scale R): the vertex
/// placed on its surface (opening: `header_point` at |x| <= |station| + S + r, omega from v / R_dev; saddle:
/// `branch_point` and the fold of phi), its branch azimuth recovered by atan2 from q = p − o with
/// |q| <= S + r (rounding of p, o, the subtraction, w = ez cos − ex sin, the dot products, and the defect of d·w,
/// d·ey), the azimuth error moved along the intersection curve by |C'(phi)| <= r + D, and the computed curve
/// point itself.
pub fn joint_check_rounding(b: &Branch, station: f64) -> f64 {
    let (_, _, _, s_big, d_big) = branch_terms(b);
    let scale = b.big_r;
    let q = s_big + b.r;
    let e_open = U * (FRAME_ORIGIN * scale + 3.0 * (station.abs() + s_big + b.r) + 5.0 * b.big_r)
        + b.big_r * (3.0 * std::f64::consts::PI * U + 4.0 * U);
    let e_bp = branch_point_rounding(b, scale, station, s_big);
    let e_p = e_open.max(e_bp + 2.0 * U * TAU * b.r);
    let e_o = U * (FRAME_ORIGIN * scale + 3.0 * station.abs() + 3.0 * b.delta.abs());
    let da = e_p + e_o + q * (7.0 * U + 2.0 * FRAME_DEFECT);
    let db = e_p + e_o + q * (4.0 * U + 1.5 * FRAME_DEFECT);
    let d_phi = da.hypot(db) / b.r + 2.0 * U * TAU;
    let e_ref = intersection_root_bound(b, scale, station) + e_bp;
    e_p + e_ref + (b.r + d_big) * d_phi
}

/// The 3D-intersection point checks that carry an a-priori bound on the verifier's own rounding (`Conditioning`):
/// the only checks whose failure the refusal `branch_angle_not_verifiable` may attribute to the verifier's
/// conditioning. A failure of any other check (fold-back, profile, lengths and areas, circles) rejects the job (G10).
pub const CONDITIONED_CHECKS: [&str; 4] = [
    "saddle_matches_3d_intersection",
    "opening_matches_3d_intersection",
    "opening_on_intersection_of_branch_geometry",
    "saddle_on_intersection_of_header_geometry",
];

/// The conditioning of 3D-intersection point checks of a branch job: the a-priori bound `rounding` on the
/// verifier's own error, the engine's a-priori bound `bound` on the error of the compared vertices, and the `budget`
/// of the checks' tolerance that is left after the engine's own error: agree + 3·bound.
#[derive(Clone, Copy, Debug)]
pub struct Conditioning {
    pub checks: &'static [&'static str],
    pub rounding: f64,
    pub bound: f64,
    pub budget: f64,
}

impl Conditioning {
    /// The verifier is no longer guaranteed to confirm a correct pattern in these checks.
    pub fn exceeded(&self) -> bool {
        // a value that is not a number counts as exceeded (fail closed)
        self.rounding.partial_cmp(&self.budget) != Some(std::cmp::Ordering::Less)
    }
    /// A difference a correct pattern can show: at most the engine's bound plus the verifier's own rounding.
    /// A larger difference is a wrong pattern, whatever the conditioning.
    pub fn explains(&self, difference: f64) -> bool {
        difference <= self.rounding + self.bound
    }
}

/// The conditioning of the 3D-intersection check of a branch stub (`verify_branch`).
pub fn branch_stub_conditioning(b: &Branch, r_dev: f64, s_end: f64) -> Vec<Conditioning> {
    let saddle = develop::saddle_error_bound(b, r_dev);
    vec![Conditioning {
        checks: &["saddle_matches_3d_intersection"],
        rounding: saddle_check_rounding(b),
        bound: saddle,
        budget: agree_tol(b.big_r.max(s_end)) + 3.0 * saddle,
    }]
}

/// The conditioning of the 3D-intersection checks of a header with branch: the branch part, the header opening
/// (`verify_header_opening`) and the joint (`verify_joint`), for a joint whose two parts share one geometry.
pub fn header_with_branch_conditioning(
    b: &Branch,
    r_dev_header: f64,
    length: f64,
    station: f64,
    r_dev_branch: f64,
    s_end: f64,
) -> Vec<Conditioning> {
    let open = develop::opening_error_bound(b, r_dev_header, station);
    let saddle = develop::saddle_error_bound(b, r_dev_branch);
    let mut v = branch_stub_conditioning(b, r_dev_branch, s_end);
    v.push(Conditioning {
        checks: &["opening_matches_3d_intersection"],
        rounding: opening_check_rounding(b, r_dev_header, station),
        bound: open,
        budget: agree_tol(b.big_r.max(length)) + 3.0 * open,
    });
    v.push(Conditioning {
        checks: &[
            "opening_on_intersection_of_branch_geometry",
            "saddle_on_intersection_of_header_geometry",
        ],
        rounding: joint_check_rounding(b, station),
        bound: open + saddle,
        budget: agree_tol(b.big_r.max(s_end)) + 3.0 * (open + saddle),
    });
    v
}

/// Work plan task 1.1: the refusal of a branch angle the verification cannot confirm. It is the verifier's own
/// criterion, evaluated on the final patterns before the job is accepted (`checks` are exactly the checks gate
/// G10 reads), restricted to failures the verifier's conditioning can explain. Refused only when at least one check
/// FAILS and EVERY failing check (a) is one of `CONDITIONED_CHECKS`, (b) has its own conditioning `exceeded` (the
/// verifier is not guaranteed to confirm a correct pattern there), and (c) differs by no more than a correct pattern
/// can (`explains`: the engine's bound plus the verifier's rounding bound). Anything else is a wrong pattern and the
/// job is rejected by G10 as before; a pattern the verifier confirms is accepted.
pub fn branch_angle_refusal(
    beta_deg: f64,
    conditioning: &[Conditioning],
    checks: &[Check],
) -> Option<crate::UemError> {
    let failing: Vec<&Check> = checks
        .iter()
        .filter(|c| c.verdict == Verdict::Fail)
        .collect();
    if failing.is_empty() {
        return None;
    }
    let mut governing: Option<(&Check, &Conditioning)> = None;
    for c in &failing {
        let k = conditioning.iter().find(|k| k.checks.contains(&c.name))?;
        if !(CONDITIONED_CHECKS.contains(&c.name) && k.exceeded() && k.explains(c.difference)) {
            return None;
        }
        if governing.is_none_or(|(_, g)| k.rounding / k.budget > g.rounding / g.budget) {
            governing = Some((c, k));
        }
    }
    let (first, worst) = governing?;
    Some(crate::out_of_range_with(
        crate::refusal::BRANCH_ANGLE_NOT_VERIFIABLE,
        "beta",
        vec!["beta_deg"],
        format!(
            "beta_deg = {beta_deg}: the independent verification cannot confirm the pattern at this branch angle. \
             Its numerical intersection is conditioned by 1/sin²(beta): the bound on its own rounding error in \
             {} is {:e} mm, not below the {:e} mm of its tolerance left after the engine's own error, and the \
             check differs by {:e} mm against a tolerance of {:e} mm. An implementation limit of the \
             verification, not an engineering rule; nothing is offered.",
            first.name, worst.rounding, worst.budget, first.difference, first.tolerance
        ),
        vec![("limit", worst.budget), ("measured", worst.rounding)],
    ))
}

/// 8·eps_num >= the tolerance of the part's 3D-intersection point check (agree + 4·bound). For a branch eps_num is
/// that bound, so the condition is 4·eps_num >= agree; for a header eps_num is the larger of the plate's own estimate
/// and the opening bound, so on a long plate it can hold before the opening bound alone would make it. In practice:
/// a branch angle near 180 degrees, where the rounding of beta itself dominates eps_num. The result then shows
/// eps_num prominently.
/// The plate-convention keys a part must state (work plan task 1.3), by family. Every part states the nine base
/// keys; a part with an offset (eccentric reducer, branch stub, header plate of a header with branch) adds the offset
/// keys, and a part with a branch (branch stub, header plate) adds the branch angle. The strings are the keys of
/// `nf_dxf::CONVENTION_KEYS`; a test in nf-core checks that the two lists agree.
pub const BASE_CONVENTION_KEYS: [&str; 9] = [
    "NF-VIEW-SIDE",
    "NF-PLATE-AXES",
    "NF-ROLLING-AXIS",
    "NF-SEAM-EDGES",
    "NF-END-EDGES",
    "NF-ANGLE-DATUM",
    "NF-ORIGIN",
    "NF-KERF",
    "NF-STATUS",
];

/// The convention keys a part of `family` must state. The header plate of a header with one branch is
/// `header_shell_with_opening` (develop.rs). An unknown family gives an empty list, which is never complete.
pub fn required_convention_keys(family: &str) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = BASE_CONVENTION_KEYS.to_vec();
    match family {
        "cylinder_shell" | "cone_frustum" => {}
        "eccentric_reducer" => keys.extend(["NF-OFFSET-DELTA", "NF-OFFSET-SIGN"]),
        "branch_stub" | "header_shell_with_opening" => {
            keys.extend(["NF-OFFSET-DELTA", "NF-OFFSET-SIGN", "NF-BRANCH-ANGLE"])
        }
        _ => return Vec::new(),
    }
    keys
}

/// True when `stated` holds exactly the keys `required_convention_keys` asks for: each once, none missing, none extra.
pub fn convention_keys_complete(family: &str, stated: &[&str]) -> bool {
    let want = required_convention_keys(family);
    !want.is_empty()
        && stated.len() == want.len()
        && want
            .iter()
            .all(|k| stated.iter().filter(|s| *s == k).count() == 1)
}

pub fn eps_num_governs(p: &Pattern, checks: &[Check]) -> bool {
    checks.iter().any(|c| {
        matches!(
            c.name,
            "saddle_matches_3d_intersection" | "opening_matches_3d_intersection"
        ) && 8.0 * p.eps_num >= c.tolerance
    })
}

/// Fold a point of a BRANCH pattern back to (φ, s) using only the side the
/// pattern DECLARES, never the way it was built (validation defect D1).
/// Outside view is the frame of Spec 02 §4.4, (u, v) = (r_dev·φ, s). The
/// inside view is its mirror image, (u, v) = (r_dev·φ, s_end − s), which is
/// what builds up to engine 0.2.0 drew.
pub fn fold_branch(side: ViewSide, q: &P2, r_dev: f64, s_end: f64) -> (f64, f64) {
    match side {
        ViewSide::Outside => (q.x / r_dev, q.y),
        ViewSide::Inside => (q.x / r_dev, s_end - q.y),
    }
}

/// Fold a point of a HEADER pattern back to 3D using only its declared side.
/// Outside view: x along the axis, v = r_dev·(ω + π). Inside view: the mirror
/// image in v.
pub fn fold_header(
    side: ViewSide,
    q: &P2,
    big_r: f64,
    r_dev_header: f64,
    station: f64,
) -> [f64; 3] {
    let omega = match side {
        ViewSide::Outside => q.y / r_dev_header - std::f64::consts::PI,
        ViewSide::Inside => std::f64::consts::PI - q.y / r_dev_header,
    };
    [q.x - station, big_r * omega.sin(), big_r * omega.cos()]
}

/// A check whose tolerance includes a representation error (`eps_rep`). A would-be PASS becomes INCONCLUSIVE when
/// the error is not a certified bound (work plan task 1.2; TOLERANCE_POLICY section 8.1), and also when the certified
/// bound is coarser than the precision the product requires, `MAX_CHORD_ERROR_MM` (rule of the review after task 1.2,
/// TOLERANCE_POLICY section 8.1: a tolerance of 2·eps_rep above the limit would pass almost any pattern). A measured
/// difference beyond the tolerance stays FAIL in both cases: the conservative direction is kept. The comparison is
/// written so that NaN is coarse too (fail closed).
fn rep_check(mut c: Check, certified: bool, eps_rep: f64) -> Check {
    let coarser_than_required = eps_rep.partial_cmp(&MAX_CHORD_ERROR_MM)
        != Some(std::cmp::Ordering::Less)
        && eps_rep.partial_cmp(&MAX_CHORD_ERROR_MM) != Some(std::cmp::Ordering::Equal);
    if c.verdict == Verdict::Pass && (!certified || coarser_than_required) {
        c.verdict = Verdict::Inconclusive;
    }
    c
}

/// Certified enclosure of the exact developed length of a polyline (work plan task 1.2 item C, TOLERANCE_POLICY
/// section 8). `drawn` holds the n + 1 vertices of the pattern; each carries an error of at most `eps_num`. The
/// exact length of the curve lies in [L − 2·n·eps_num, L + E + 2·n·eps_num], with L the drawn length and E the
/// certified excess of the curve over its chords (`curve_bound::length_excess_bound`). Returns (centre, half
/// width); the half width is f64::MAX where no bound can be proved, so the length check is INCONCLUSIVE.
fn length_enclosure(curve: &dyn CurveBound, drawn: &[P2], n: usize, eps_num: f64) -> (f64, f64) {
    // the excess is summed over LENGTH_SUB sub-intervals per segment (the midpoint enclosure of each sub-interval
    // loses a term proportional to its width, so the finer the sum the closer the bound is to the true excess)
    const LENGTH_SUB: usize = 64;
    let dphi = TAU / (LENGTH_SUB * n) as f64; // segment i starts at parameter i·TAU/n, as the drawn vertex i
    let params: Vec<f64> = (0..=LENGTH_SUB * n).map(|i| i as f64 * dphi).collect();
    let drawn_len: f64 = drawn.windows(2).map(|w| w[0].dist(w[1])).sum();
    match length_excess_bound(curve, drawn, &params, n, LENGTH_SUB) {
        Some(e) if e.is_finite() => (drawn_len + 0.5 * e, 0.5 * e + 2.0 * n as f64 * eps_num),
        _ => (f64::NAN, f64::MAX),
    }
}

/// The plate point of the header opening at parameter phi (the closed form that develop samples).
fn header_plate_point(b: &Branch, r_dev_header: f64, station: f64, phi: f64) -> P2 {
    let (sb, cb) = b.beta.sin_cos();
    let s = b.s(phi);
    let omega = ((b.delta + b.r * phi.cos()) / b.big_r).asin();
    P2::new(
        station + s * cb - b.r * phi.sin() * sb,
        r_dev_header * (omega + std::f64::consts::PI),
    )
}

/// The vertices of a closed header opening in the order of its parameter. develop reverses a loop whose
/// shoelace area is positive (inner profiles are clockwise), so the stored order may be the reverse one; the
/// first step is compared with the exact plate point at the first parameter value.
fn forward_loop(pts: &[P2], first_step_exact: P2) -> Vec<P2> {
    let n = pts.len().saturating_sub(1);
    if n < 2 {
        return pts.to_vec();
    }
    if pts[1].dist(first_step_exact) <= pts[n - 1].dist(first_step_exact) {
        pts.to_vec()
    } else {
        pts.iter().rev().copied().collect()
    }
}

/// The declared side as a number, so that two declarations can be compared by `cmp`.
fn declared(side: ViewSide) -> f64 {
    match side {
        ViewSide::Outside => 1.0,
        ViewSide::Inside => -1.0,
    }
}

fn cmp(name: &'static str, method: &'static str, primary: f64, reference: f64, tol: f64) -> Check {
    let d = (primary - reference).abs();
    let verdict = if !d.is_finite() {
        Verdict::Inconclusive
    } else if d <= tol {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    Check {
        name,
        method,
        independence: "I-F",
        primary,
        reference,
        difference: d,
        tolerance: tol,
        verdict,
        reason: None,
    }
}

fn curve_len(p: &Pattern, name: &str) -> f64 {
    p.outline
        .iter()
        .filter(|c| c.name == name)
        .map(profile::length)
        .sum()
}

/// Independent-formulation checks of a plain shell (review RB-006): the end
/// circle and the lateral area of the solid, sampled in 3D, against the plate.
/// Kept apart from `verify_cylinder` so that the thresholds of the original
/// checks stay exactly as they were (docs/backend/TOLERANCE_POLICY.md).
pub fn verify_cylinder_solid(p: &Pattern, r_dev: f64, length: f64) -> Vec<Check> {
    let agree = agree_tol(r_dev.max(length));
    let (circle, circle_res) = independent::circle_length(r_dev, 1024);
    let outline = profile::check(&p.outline, 1e-9);
    vec![
        cmp(
            "end_circle_3d_arc_length",
            "3D chord sums of the end circle of the shell in a tilted frame, taken to the limit (no 2*pi*r)",
            curve_len(p, "bottom"),
            circle,
            agree + circle_res,
        ),
        cmp(
            "end_circles_3d_arc_length_equal",
            "developed length of the other shell end against the same 3D circle",
            curve_len(p, "top"),
            circle,
            agree + circle_res,
        ),
        cmp(
            "area_equals_sampled_surface_area",
            "3D circle length (chord sums) times the shell length against the signed area of the plate outline",
            outline.signed_area,
            circle * length,
            agree * (r_dev + length) + circle_res * length,
        ),
        cmp(
            "outline_counter_clockwise",
            "sign of the signed area of the outline: outer profiles are counter-clockwise in the pattern frame (Spec 02 section 0.3)",
            outline.signed_area.signum(),
            1.0,
            0.5,
        ),
    ]
}

pub fn verify_cylinder(p: &Pattern, r_dev: f64, length: f64) -> Vec<Check> {
    let agree = agree_tol(r_dev.max(length));
    let outline = profile::check(&p.outline, 1e-9);
    vec![
        cmp(
            "bottom_edge_equals_circumference",
            "circumference 2*pi*r from the input radius",
            curve_len(p, "bottom"),
            TAU * r_dev,
            agree,
        ),
        cmp(
            "seam_edges_equal_length",
            "length of the opposite seam edge",
            curve_len(p, "seam_a"),
            curve_len(p, "seam_b"),
            agree,
        ),
        cmp(
            "seam_length_equals_input",
            "input length",
            curve_len(p, "seam_a"),
            length,
            agree,
        ),
        cmp(
            "area",
            "lateral area 2*pi*r*L",
            outline.signed_area,
            TAU * r_dev * length,
            agree * (r_dev + length),
        ),
        cmp(
            "outline_counter_clockwise",
            "sign of the signed area of the outline: outer profiles are counter-clockwise in the pattern frame (Spec 02 section 0.3)",
            outline.signed_area.signum(),
            1.0,
            0.5,
        ),
    ]
}

pub fn verify_cone(p: &Pattern, r1: f64, r2: f64, h: f64) -> Vec<Check> {
    let s = h.hypot(r1 - r2);
    let agree = agree_tol(r1.max(h));
    let mut v = vec![
        cmp(
            "large_arc_equals_circumference",
            "3D end-circle circumference 2*pi*r1",
            curve_len(p, "large_end"),
            TAU * r1,
            agree,
        ),
        cmp(
            "small_arc_equals_circumference",
            "3D end-circle circumference 2*pi*r2",
            curve_len(p, "small_end"),
            TAU * r2,
            agree,
        ),
        cmp(
            "seam_edges_equal_length",
            "length of the opposite seam edge",
            curve_len(p, "seam_a"),
            curve_len(p, "seam_b"),
            agree,
        ),
        cmp(
            "seam_equals_slant_length",
            "slant length from Pythagoras on the axial section",
            curve_len(p, "seam_a"),
            s,
            agree,
        ),
        cmp(
            "area",
            "lateral area pi*(r1+r2)*s of the 3D frustum",
            p.quantity("area").unwrap_or(f64::NAN),
            std::f64::consts::PI * (r1 + r2) * s,
            agree * s,
        ),
    ];
    for (name, check, r) in [
        ("large_end", "large_end_3d_arc_length", r1),
        ("small_end", "small_end_3d_arc_length", r2),
    ] {
        let (circle, res) = independent::circle_length(r, 1024);
        v.push(cmp(
            check,
            "3D chord sums of the end circle in a tilted frame, taken to the limit (no 2*pi*r)",
            curve_len(p, name),
            circle,
            agree + res,
        ));
    }
    // Fold-back: every pattern arc radius rho maps to a 3D circle of radius rho*sin(alpha)
    // at axial distance rho*cos(alpha) from the apex; compare with the linear taper of the frustum.
    let (sin_a, cos_a) = ((r1 - r2) / s, h / s);
    let z_apex = h * r1 / (r1 - r2);
    let mut worst: f64 = 0.0;
    for c in &p.outline {
        if let Geom::Arc { r, .. } = c.geom {
            let z = z_apex - r * cos_a;
            let expected_radius = r1 - (r1 - r2) * z / h;
            worst = worst.max((r * sin_a - expected_radius).abs());
        }
    }
    v.push(cmp(
        "fold_back_radius_residual",
        "implicit taper equation of the frustum evaluated at folded-back arcs",
        worst,
        0.0,
        agree,
    ));
    v
}

/// Apex-free development of the large end circle by integrating its geodesic
/// curvature (Spec 02 ALG-04) with classical RK4. Returns developed points at
/// `n+1` equally spaced theta values.
fn geodesic_development(g: &ObliqueCone, n: usize) -> Vec<P2> {
    let (a, hh) = g.apex();
    let r1 = g.r1;
    // kappa_g * |C'| for the circle C(theta) with ruling direction D = A - C
    let kgv = |t: f64| -> f64 {
        let c1 = [-r1 * t.sin(), r1 * t.cos(), 0.0];
        let c2 = [-r1 * t.cos(), -r1 * t.sin(), 0.0];
        let d = [a - r1 * t.cos(), -r1 * t.sin(), hh];
        let nrm = [
            c1[1] * d[2] - c1[2] * d[1],
            c1[2] * d[0] - c1[0] * d[2],
            c1[0] * d[1] - c1[1] * d[0],
        ];
        let nn = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        let cr = [
            c1[1] * c2[2] - c1[2] * c2[1],
            c1[2] * c2[0] - c1[0] * c2[2],
            c1[0] * c2[1] - c1[1] * c2[0],
        ];
        (cr[0] * nrm[0] + cr[1] * nrm[1] + cr[2] * nrm[2]) / nn / (r1 * r1)
    };
    let f = |t: f64, y: [f64; 3]| -> [f64; 3] { [kgv(t), r1 * y[0].cos(), r1 * y[0].sin()] };
    let hstep = TAU / n as f64;
    let mut y = [0.0f64; 3];
    let mut out = vec![P2::new(0.0, 0.0)];
    for i in 0..n {
        let t = i as f64 * hstep;
        let add =
            |y: [f64; 3], k: [f64; 3], s: f64| [y[0] + s * k[0], y[1] + s * k[1], y[2] + s * k[2]];
        let k1 = f(t, y);
        let k2 = f(t + 0.5 * hstep, add(y, k1, 0.5 * hstep));
        let k3 = f(t + 0.5 * hstep, add(y, k2, 0.5 * hstep));
        let k4 = f(t + hstep, add(y, k3, hstep));
        for j in 0..3 {
            y[j] += hstep / 6.0 * (k1[j] + 2.0 * k2[j] + 2.0 * k3[j] + k4[j]);
        }
        out.push(P2::new(y[1], y[2]));
    }
    out
}

pub fn verify_eccentric(p: &Pattern, r1: f64, r2: f64, h: f64, e: f64) -> Vec<Check> {
    let agree = agree_tol(r1.max(h).max(e));
    let g = ObliqueCone { r1, r2, h, e };
    let outer: Vec<P2> = p
        .outline
        .iter()
        .find(|c| c.name == "large_end")
        .and_then(|c| {
            if let Geom::Polyline { pts, .. } = &c.geom {
                Some(pts.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    let n = outer.len().saturating_sub(1);
    let mut v = Vec::new();
    if n == 0 {
        v.push(cmp(
            "large_end_present",
            "pattern structure",
            f64::NAN,
            0.0,
            0.0,
        ));
        return v;
    }
    // (1) Two developments of the same curve are congruent: compare distances from the seam point.
    let fine = geodesic_development(&g, 16 * n);
    let coarse = geodesic_development(&g, 8 * n);
    let (mut worst, mut ver_err): (f64, f64) = (0.0, 0.0);
    for i in 1..=n {
        let d_primary = outer[i].dist(outer[0]);
        let d_ref = fine[16 * i].dist(fine[0]);
        worst = worst.max((d_primary - d_ref).abs());
        ver_err = ver_err.max((d_ref - coarse[8 * i].dist(coarse[0])).abs());
    }
    let tol = agree.max(p.eps_num + 10.0 * ver_err);
    v.push(cmp(
        "apex_free_development_agrees",
        "geodesic-curvature integration of the large end circle (no apex), RK4",
        worst,
        0.0,
        tol,
    ));
    // (2) Developed edge lengths equal the 3D circumferences (isometry invariant).
    let chord_bias =
        |r: f64| TAU * r * (std::f64::consts::PI / n as f64).powi(2) / 6.0 * 1.5 + agree;
    v.push(rep_check(
        cmp(
            "large_edge_equals_circumference",
            "3D circumference 2*pi*r1 (polyline chord shortening allowed for)",
            curve_len(p, "large_end"),
            TAU * r1,
            chord_bias(r1) + p.eps_rep,
        ),
        p.eps_rep_certified,
        p.eps_rep,
    ));
    v.push(rep_check(
        cmp(
            "small_edge_equals_circumference",
            "3D circumference 2*pi*r2 (polyline chord shortening allowed for)",
            curve_len(p, "small_end"),
            TAU * r2,
            chord_bias(r2) + p.eps_rep,
        ),
        p.eps_rep_certified,
        p.eps_rep,
    ));
    // (3) Seam edges equal, and equal to the 3D ruling length at theta = 0.
    let ruling0 = ((e + r2 - r1).powi(2) + h * h).sqrt();
    v.push(cmp(
        "seam_edges_equal_length",
        "length of the opposite seam edge",
        curve_len(p, "seam_a"),
        curve_len(p, "seam_b"),
        agree,
    ));
    v.push(cmp(
        "seam_equals_3d_ruling",
        "distance between the 3D end-circle points at theta = 0",
        curve_len(p, "seam_a"),
        ruling0,
        agree,
    ));
    // (4) Area: polar formula (primary) versus 3D surface integral of the ruled surface.
    let surf = {
        let inner = |t: f64| -> f64 {
            let d = [e + (r2 - r1) * t.cos(), (r2 - r1) * t.sin(), h];
            let f = |l: f64| -> f64 {
                let rr = (1.0 - l) * r1 + l * r2;
                let dt = [-rr * t.sin(), rr * t.cos(), 0.0];
                let c = [
                    dt[1] * d[2] - dt[2] * d[1],
                    dt[2] * d[0] - dt[0] * d[2],
                    dt[0] * d[1] - dt[1] * d[0],
                ];
                (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
            };
            gauss(&f, 0.0, 1.0, 4).0
        };
        gauss(&inner, 0.0, TAU, 360).0
    };
    v.push(cmp(
        "area",
        "surface integral of |S_theta x S_lambda| over the 3D ruled surface",
        p.quantity("area").unwrap_or(f64::NAN),
        surf,
        1e-6 * surf.max(1.0),
    ));
    // (5) Handedness: a mirrored development is clockwise, so the sign of the outline's signed area flips.
    v.push(cmp(
        "outline_counter_clockwise",
        "sign of the signed area of the outline: outer profiles are counter-clockwise in the pattern frame (Spec 02 section 0.3)",
        profile::check(&p.outline, 1e-9).signed_area.signum(),
        1.0,
        0.5,
    ));
    v
}

fn saddle_points(p: &Pattern) -> Vec<P2> {
    p.outline
        .iter()
        .find(|c| c.name == "saddle_cut")
        .and_then(|c| {
            if let Geom::Polyline { pts, .. } = &c.geom {
                Some(pts.clone())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Limit of a sequence that converges like h²: `a` at step 4h, `b` at 2h, `c` at h.
/// Returns (limit, resolution): the resolution is the difference between the two
/// once-extrapolated values, which over-states the error of the limit.
fn limit_h2(a: f64, b: f64, c: f64) -> (f64, f64) {
    let (r0, _) = independent::richardson(a, b);
    let (r1, _) = independent::richardson(b, c);
    (r1 + (r1 - r0) / 15.0, (r1 - r0).abs())
}

/// A comparison whose two sides carry their own resolution (Spec 02 ALG-13): PASS needs agreement
/// within `tol + resolution`, and a resolution coarser than `need` is INCONCLUSIVE, never a pass.
fn cmp_resolved(
    name: &'static str,
    method: &'static str,
    primary: f64,
    reference: f64,
    tol: f64,
    resolution: f64,
    need: f64,
) -> Check {
    let mut c = cmp(name, method, primary, reference, tol + resolution);
    if c.verdict == Verdict::Pass && resolution.partial_cmp(&need) != Some(std::cmp::Ordering::Less)
    {
        c.verdict = Verdict::Inconclusive;
    }
    c
}

pub fn verify_branch(p: &Pattern, b: &Branch, r_dev: f64, s_end: f64) -> Vec<Check> {
    let agree = agree_tol(b.big_r.max(s_end));
    let cut = saddle_points(p);
    let (sb, cb) = b.beta.sin_cos();
    let d = [cb, 0.0, sb];
    let (mut header_res, mut branch_res): (f64, f64) = (0.0, 0.0);
    for q in &cut {
        // fold the pattern point back to 3D by the DECLARED side, then test the two implicit cylinder equations
        let (phi, s) = fold_branch(p.marking_side, q, r_dev, s_end);
        let pt = b.point(phi, s);
        header_res = header_res.max(((pt[1] * pt[1] + pt[2] * pt[2]).sqrt() - b.big_r).abs());
        let w = [pt[0], pt[1] - b.delta, pt[2]];
        let c = [
            w[1] * d[2] - w[2] * d[1],
            w[2] * d[0] - w[0] * d[2],
            w[0] * d[1] - w[1] * d[0],
        ];
        branch_res = branch_res.max(((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() - b.r).abs());
    }
    // Handedness, independent of the fold-back: at φ = 90° and 270° the saddle heights differ by
    // 2·r·cot β (eq. 4.3). Read both heights off the pattern at u = w/4 and 3w/4 through the declared side.
    let height_at = |u: f64| -> f64 {
        cut.windows(2)
            .find(|w| (w[0].x - u) * (w[1].x - u) <= 0.0 && w[0].x != w[1].x)
            .map(|w| {
                let t = (u - w[0].x) / (w[1].x - w[0].x);
                let q = P2::new(u, w[0].y + t * (w[1].y - w[0].y));
                fold_branch(p.marking_side, &q, r_dev, s_end).1
            })
            .unwrap_or(f64::NAN)
    };
    let width = TAU * r_dev;
    let handed = height_at(0.75 * width) - height_at(0.25 * width);
    let handed_ref = 2.0 * b.r * cb / sb;

    // ---- independent formulation (review RB-006): the joint as a solid, each ruling intersected numerically.
    // The rounding bound of the closed form is computed here from the INPUTS (not read from the pattern):
    // near tangency both methods are equally ill-conditioned, and the threshold may not be below that.
    let j = Joint3::new(b.big_r, b.r, b.beta, b.delta, 0.0);
    let bound = develop::saddle_error_bound(b, r_dev);
    let tol3 = agree + 4.0 * bound;
    let n = cut.len().saturating_sub(1);
    let usable = n >= 8 && n.is_multiple_of(4);
    let mut solid_res: f64 = 0.0;
    for q in &cut {
        let (phi, s) = fold_branch(p.marking_side, q, r_dev, s_end);
        solid_res = match j.s_at(phi) {
            Some(si) if (s - si).is_finite() => solid_res.max((s - si).abs()),
            _ => f64::NAN,
        };
    }
    // Side of the offset (review RB-002): the generator at the seam (phi = 0, the positive-delta side) against
    // the generator half a turn away. A pattern mirrored in delta swaps the two.
    let delta_side = height_at(0.0) - height_at(0.5 * width);
    let delta_side_ref = match (j.s_at(0.0), j.s_at(0.5 * TAU)) {
        (Some(a), Some(c)) => a - c,
        _ => f64::NAN,
    };
    // Developed length of the cut against the 3D arc length of the cut on the development cylinder
    // (radius r_dev about the branch axis), the latter from the numerical intersection alone.
    let (len_p, res_p) = if usable {
        length_enclosure(&SaddleCurve { b, r_dev }, &cut, n, p.eps_num)
    } else {
        (f64::NAN, f64::NAN)
    };
    let solid_points = |m: usize| -> Option<Vec<V3>> {
        (0..=m)
            .map(|i| {
                let phi = TAU * i as f64 / m as f64;
                j.s_at(phi).map(|s| j.branch_point(phi, s, r_dev))
            })
            .collect()
    };
    let (len_v, res_v, area_v, area_res) = match (usable, solid_points(4 * n.max(1))) {
        (true, Some(fine)) => {
            let every = |k: usize| -> f64 {
                let v: Vec<V3> = fine.iter().step_by(k).copied().collect();
                independent::polyline_length(&v)
            };
            let (l, r) = limit_h2(every(4), every(2), every(1));
            // area of the development between the cut and the square end: r_dev · ∫ (s_end − s) dphi,
            // periodic trapezoid rule on the numerically intersected heights, at two resolutions
            let heights = |k: usize| -> f64 {
                let m = (fine.len() - 1) / k;
                (0..m)
                    .map(|i| {
                        let phi = TAU * (i * k) as f64 / (4 * n) as f64;
                        s_end - j.s_at(phi).unwrap_or(f64::NAN)
                    })
                    .sum::<f64>()
                    * TAU
                    / m as f64
                    * r_dev
            };
            let (a2, a1) = (heights(2), heights(1));
            (l, r, a1, (a1 - a2).abs())
        }
        _ => (f64::NAN, f64::NAN, f64::NAN, f64::NAN),
    };
    let outline = profile::check(&p.outline, 1e-9);
    // the polygon area differs from the area under the exact curve by the trapezoid error of its vertices
    let trap = |stride: usize| -> f64 {
        cut.iter()
            .step_by(stride)
            .collect::<Vec<_>>()
            .windows(2)
            .map(|q| 0.5 * (q[0].y + q[1].y) * (q[1].x - q[0].x))
            .sum()
    };
    let area_p_res = if usable {
        (trap(1) - trap(2)).abs()
    } else {
        f64::NAN
    };
    let (circle, circle_res) = independent::circle_length(r_dev, 1024);
    let v = vec![
        cmp(
            "cut_points_on_header_surface",
            "fold-back by the declared marking side, then distance from the header axis (implicit equation)",
            if cut.is_empty() { f64::NAN } else { header_res },
            0.0,
            agree,
        ),
        cmp(
            "cut_points_on_branch_surface",
            "fold-back by the declared marking side, then |(P-O) x d| distance from the branch axis",
            if cut.is_empty() { f64::NAN } else { branch_res },
            0.0,
            agree,
        ),
        rep_check(
            cmp(
                "saddle_handedness_matches_declared_side",
                "s(270 deg) - s(90 deg) read from the pattern through the declared marking side against 2*r*cot(beta)",
                handed,
                handed_ref,
                // the two heights are read off a polyline: allow its representation estimate
                agree + 2.0 * p.eps_rep,
            ),
            p.eps_rep_certified,
            p.eps_rep,
        ),
        cmp(
            "outline_counter_clockwise",
            "sign of the signed area of the outline: outer profiles are counter-clockwise in the pattern frame (Spec 02 section 0.3)",
            outline.signed_area.signum(),
            1.0,
            0.5,
        ),
        cmp(
            "square_end_equals_circumference",
            "circumference 2*pi*r_dev",
            curve_len(p, "square_end"),
            TAU * r_dev,
            agree,
        ),
        cmp(
            "seam_edges_equal_length",
            "length of the opposite seam edge",
            curve_len(p, "seam_a"),
            curve_len(p, "seam_b"),
            agree,
        ),
        cmp(
            "saddle_matches_3d_intersection",
            "independent formulation: each branch ruling of the solid joint (tilted frame) intersected with the header cylinder by a numerical root search on the distance from the header axis",
            if cut.is_empty() { f64::NAN } else { solid_res },
            0.0,
            tol3,
        ),
        rep_check(
            cmp(
                "saddle_on_the_declared_delta_side",
                "height of the cut at the seam (phi = 0, positive-delta side) minus the height half a turn away, against the numerical intersection of the solid joint",
                delta_side,
                delta_side_ref,
                tol3 + 2.0 * p.eps_rep,
            ),
            p.eps_rep_certified,
            p.eps_rep,
        ),
        cmp_resolved(
            "saddle_developed_length_equals_3d_arc_length",
            "independent formulation: 3D chord sums of the numerically intersected cut on the development cylinder, taken to the limit, against the developed length of the pattern curve taken to the limit",
            len_p,
            len_v,
            agree + 1e-12 * len_v.abs(),
            res_p + res_v,
            1e-5 * len_v.abs(),
        ),
        cmp_resolved(
            "pattern_area_equals_surface_area",
            "independent formulation: area of the development cylinder between the numerically intersected cut and the square end (periodic trapezoid rule) against the signed area of the pattern outline",
            outline.signed_area,
            area_v,
            agree * (width + s_end),
            area_p_res + area_res,
            1e-5 * area_v.abs(),
        ),
        cmp(
            "square_end_3d_arc_length",
            "3D chord sums of the square end circle (radius r_dev) in a tilted frame, taken to the limit",
            curve_len(p, "square_end"),
            circle,
            agree + circle_res,
        ),
    ];
    v
}

fn hole_points(p: &Pattern) -> Vec<P2> {
    p.holes
        .first()
        .and_then(|c| {
            if let Geom::Polyline { pts, .. } = &c.geom {
                Some(pts.clone())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Plate point of a header chart value (x, omega) for the side the pattern declares.
fn header_plate(side: ViewSide, x: f64, omega: f64, r_dev_header: f64) -> P2 {
    match side {
        ViewSide::Outside => P2::new(x, r_dev_header * (omega + std::f64::consts::PI)),
        ViewSide::Inside => P2::new(x, r_dev_header * (std::f64::consts::PI - omega)),
    }
}

/// Header shell with opening: fold the opening back onto the header surface
/// and test it against the *branch* cylinder's implicit equation; and compare
/// the opening with the numerically intersected solid joint.
pub fn verify_header_opening(
    p: &Pattern,
    b: &Branch,
    r_dev_header: f64,
    length: f64,
    station: f64,
) -> Vec<Check> {
    let agree = agree_tol(b.big_r.max(length));
    let (sb, cb) = b.beta.sin_cos();
    let d = [cb, 0.0, sb];
    let mut branch_res: f64 = 0.0;
    let pts = hole_points(p);
    for q in &pts {
        let pt = fold_header(p.marking_side, q, b.big_r, r_dev_header, station);
        let w = [pt[0], pt[1] - b.delta, pt[2]];
        let c = [
            w[1] * d[2] - w[2] * d[1],
            w[2] * d[0] - w[0] * d[2],
            w[0] * d[1] - w[1] * d[0],
        ];
        branch_res = branch_res.max(((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() - b.r).abs());
    }
    // Sign of δ (review RB-002): the opening is centred on the branch axis, i.e. at y = δ of the 3D frame,
    // which is ABOVE the centreline of the plate for δ > 0 in the outside view. Read from the plate through
    // the declared side; a plate mirrored across its centreline gives −δ.
    let centre_y = if pts.is_empty() {
        f64::NAN
    } else {
        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
            let y = fold_header(p.marking_side, q, b.big_r, r_dev_header, station)[1];
            (lo.min(y), hi.max(y))
        });
        0.5 * (lo + hi)
    };
    let hole = profile::check(&p.holes, 1e-9);
    let inside = pts
        .iter()
        .all(|q| q.x > 0.0 && q.x < length && q.y > 0.0 && q.y < TAU * r_dev_header);
    // The outline must be drawn in the frame the opening is computed in: length along x, circumference along y.
    let (mut ex, mut ey) = (0.0f64, 0.0f64);
    for c in &p.outline {
        if let Geom::Line { a, b } = &c.geom {
            ex = ex.max(a.x).max(b.x);
            ey = ey.max(a.y).max(b.y);
        }
    }

    // ---- independent formulation (review RB-006): the opening against the numerically intersected solid.
    // The polyline declares uniform sampling in the branch angle; which way round it runs is not assumed:
    // both directions are tried and the better one counts (a mirrored opening matches neither).
    let j = Joint3::new(b.big_r, b.r, b.beta, b.delta, station);
    let tol3 = agree + 4.0 * develop::opening_error_bound(b, r_dev_header, station);
    let n = pts.len().saturating_sub(1);
    let usable = n >= 8 && n.is_multiple_of(4);
    let plate_at = |phi: f64| -> Option<P2> {
        j.curve_point(phi).map(|x3| {
            let (x, omega) = j.header_chart(x3);
            header_plate(p.marking_side, x, omega, r_dev_header)
        })
    };
    let solid_res = if n == 0 {
        f64::NAN
    } else {
        [1.0f64, -1.0]
            .iter()
            .map(|dir| {
                let mut worst: f64 = 0.0;
                for (i, q) in pts.iter().enumerate() {
                    let phi = (dir * TAU * i as f64 / n as f64).rem_euclid(TAU);
                    worst = match plate_at(phi) {
                        Some(want) if q.dist(want).is_finite() => worst.max(q.dist(want)),
                        _ => f64::NAN,
                    };
                }
                worst
            })
            .fold(
                f64::NAN,
                |best, w| if w < best || best.is_nan() { w } else { best },
            )
    };
    // Developed perimeter of the opening against the 3D arc length of the cut on the development cylinder
    // of the header (radius R_dev), and its developed area against R_dev · ∮ x d(omega), both from the solid.
    let (per_p, per_res) = if usable {
        let step = header_plate_point(b, r_dev_header, station, TAU / n as f64);
        let forward = forward_loop(&pts, step);
        length_enclosure(&OpeningCurve { b, r_dev_header }, &forward, n, p.eps_num)
    } else {
        (f64::NAN, f64::NAN)
    };
    let chart: Option<Vec<(f64, f64)>> = if usable {
        (0..=4 * n)
            .map(|i| {
                j.curve_point(TAU * i as f64 / (4 * n) as f64)
                    .map(|x3| j.header_chart(x3))
            })
            .collect()
    } else {
        None
    };
    let (per_v, per_v_res, area_v, area_v_res) = match &chart {
        Some(c) => {
            let length_of = |k: usize| -> f64 {
                let v: Vec<V3> = c
                    .iter()
                    .step_by(k)
                    .map(|&(x, om)| j.header_point(x, om, r_dev_header))
                    .collect();
                independent::polyline_length(&v)
            };
            let (l, r) = limit_h2(length_of(4), length_of(2), length_of(1));
            // ∮ x dω over the closed loop equals ∮ (x − x0) dω, since ∮ dω = 0: the shift by the first
            // station removes the cancellation of the absolute station (task 1.2 item B)
            let x0 = c.first().map_or(0.0, |q| q.0);
            let area_of = |k: usize| -> f64 {
                let v: Vec<&(f64, f64)> = c.iter().step_by(k).collect();
                (0.5 * r_dev_header
                    * v.windows(2)
                        .map(|w| (w[0].0 + w[1].0 - 2.0 * x0) * (w[1].1 - w[0].1))
                        .sum::<f64>())
                .abs()
            };
            // the area of an inscribed polygon converges like h², as chord sums do
            let (a, ar) = limit_h2(area_of(4), area_of(2), area_of(1));
            (l, r, a, ar)
        }
        None => (f64::NAN, f64::NAN, f64::NAN, f64::NAN),
    };
    // shoelace on coordinates relative to the first vertex of the opening (task 1.2 item B): the absolute
    // coordinates (x ~ the station, y ~ the circumference) cancel to a fraction of the opening's own area
    let shoelace = |stride: usize| -> f64 {
        let o = pts.first().copied().unwrap_or(P2::new(0.0, 0.0));
        let v: Vec<P2> = pts
            .iter()
            .step_by(stride)
            .map(|q| P2::new(q.x - o.x, q.y - o.y))
            .collect();
        (0.5 * v
            .windows(2)
            .map(|w| w[0].x * w[1].y - w[1].x * w[0].y)
            .sum::<f64>())
        .abs()
    };
    let opening_extent: f64 = {
        let (xlo, xhi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
            (lo.min(q.x), hi.max(q.x))
        });
        let (ylo, yhi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
            (lo.min(q.y), hi.max(q.y))
        });
        (xhi - xlo) + (yhi - ylo)
    };
    let (area_p, area_p_res) = if usable {
        limit_h2(shoelace(4), shoelace(2), shoelace(1))
    } else {
        (f64::NAN, f64::NAN)
    };
    let (circle, circle_res) = independent::circle_length(r_dev_header, 1024);
    vec![
        cmp(
            "outline_extent_along_header_axis",
            "largest x of the plate outline against the input header length",
            ex,
            length,
            agree,
        ),
        cmp(
            "outline_extent_around_header",
            "largest y of the plate outline against 2*pi*r from the input radius",
            ey,
            TAU * r_dev_header,
            agree,
        ),
        cmp(
            "opening_points_on_branch_surface",
            "fold-back onto the header by the declared marking side, then |(P-O) x d| distance from the branch axis",
            if pts.is_empty() { f64::NAN } else { branch_res },
            0.0,
            agree,
        ),
        cmp(
            "shell_edge_equals_circumference",
            "circumference 2*pi*r from the input radius",
            curve_len(p, "bottom"),
            TAU * r_dev_header,
            agree,
        ),
        cmp(
            "seam_edges_equal_length",
            "length of the opposite seam edge",
            curve_len(p, "seam_a"),
            curve_len(p, "seam_b"),
            agree,
        ),
        cmp(
            "outline_counter_clockwise",
            "sign of the signed area of the outline: outer profiles are counter-clockwise in the pattern frame (Spec 02 section 0.3)",
            profile::check(&p.outline, 1e-9).signed_area.signum(),
            1.0,
            0.5,
        ),
        cmp(
            "opening_closed_simple_inside_shell",
            "profile test of the opening and bounds test against the shell rectangle",
            if hole.closed && hole.self_intersections == 0 && inside {
                0.0
            } else {
                1.0
            },
            0.0,
            0.5,
        ),
        cmp(
            "opening_centre_on_the_declared_delta_side",
            "mid-height of the folded-back opening across the header (y of the 3D frame, positive above the plate centreline in the outside view) against the input offset delta",
            centre_y,
            b.delta,
            agree,
        ),
        cmp(
            "opening_matches_3d_intersection",
            "independent formulation: the solid joint (tilted frame) intersected numerically at the vertex parameters; plate coordinates from the projection on the header axis and atan2 about it",
            solid_res,
            0.0,
            tol3,
        ),
        cmp_resolved(
            "opening_developed_length_equals_3d_arc_length",
            "independent formulation: 3D chord sums of the numerically intersected cut on the development cylinder of the header, taken to the limit, against the developed perimeter of the pattern opening taken to the limit",
            per_p,
            per_v,
            agree + 1e-12 * per_v.abs(),
            per_res + per_v_res,
            1e-5 * per_v.abs(),
        ),
        cmp_resolved(
            "opening_area_equals_surface_area",
            "independent formulation: area cut out of the development cylinder, R_dev times the loop integral of x d(omega) over the numerically intersected cut, against the area of the pattern opening, both taken to the limit of their polygons",
            area_p,
            area_v,
            // the area error of a polygon scales with the size of the opening, not of the whole plate (task 1.2
            // item B): its extent in x plus its extent in y. The opening lies inside the plate, so this is never
            // larger than agree × (L + 2π R_dev), the 0.4.3 tolerance
            agree * opening_extent,
            area_p_res + area_v_res,
            1e-5 * area_v.abs(),
        ),
        cmp(
            "header_circle_3d_arc_length",
            "3D chord sums of the header end circle (radius R_dev) in a tilted frame, taken to the limit",
            curve_len(p, "bottom"),
            circle,
            agree + circle_res,
        ),
    ]
}

/// Interface constraint for a branch joint: the branch end cut and the header
/// opening, each folded back to 3D from its *own* pattern with its own part's
/// geometry, must be the same curve. `hb` is the geometry the header opening
/// was cut to and `bb` the geometry the branch saddle was cut to: the caller
/// derives each from the surfaces that meet for the declared joint type, and
/// they are NOT assumed equal (review RB-001: when one object served both
/// parts this check could not see a joint whose parts were made for different
/// surfaces).
#[allow(clippy::too_many_arguments)] // two parts, each with its geometry and two development inputs
pub fn verify_interface(
    header: &Pattern,
    hb: &Branch,
    r_dev_header: f64,
    station: f64,
    branch: &Pattern,
    bb: &Branch,
    r_dev_branch: f64,
    s_end: f64,
) -> Vec<Check> {
    let agree = agree_tol(hb.big_r.max(s_end));
    // Each part is folded back by the side IT DECLARES. Two parts drawn from different faces, or a part that
    // does not match its own declaration, do not meet in 3D unless the joint is mirror-symmetric (D1).
    let open3: Vec<[f64; 3]> = hole_points(header)
        .iter()
        .map(|q| fold_header(header.marking_side, q, hb.big_r, r_dev_header, station))
        .collect();
    let cut3: Vec<[f64; 3]> = saddle_points(branch)
        .iter()
        .map(|q| {
            let (phi, s) = fold_branch(branch.marking_side, q, r_dev_branch, s_end);
            bb.point(phi, s)
        })
        .collect();
    // (linear-time search along the curves; the earlier pairwise search was quadratic in the vertex count)
    let one_way = |from: &[[f64; 3]], to: &[[f64; 3]]| -> f64 {
        independent::one_way_distance(from, to, true)
    };
    // the folded vertices lie on one curve but at different parameters when the two parts have different
    // densities: a vertex of one may then be off the other's chord by that polyline's own chord error
    let rep = |a: &Pattern, c: &Pattern| -> f64 {
        if hole_points(header).len() == saddle_points(branch).len() {
            0.0
        } else {
            a.eps_rep.max(c.eps_rep)
        }
    };
    vec![
        cmp(
            "parts_declare_the_same_marking_side",
            "declared marking side of the header pattern against that of the branch pattern (1 = outside, -1 = inside)",
            declared(branch.marking_side),
            declared(header.marking_side),
            0.5,
        ),
        rep_check(
            cmp(
                "branch_cut_lies_on_header_opening",
                "3D distance from each folded-back branch cut point to the folded-back header opening curve",
                one_way(&cut3, &open3),
                0.0,
                agree + rep(header, branch),
            ),
            header.eps_rep_certified && branch.eps_rep_certified,
            rep(header, branch),
        ),
        rep_check(
            cmp(
                "header_opening_lies_on_branch_cut",
                "3D distance from each folded-back opening point to the folded-back branch cut curve",
                one_way(&open3, &cut3),
                0.0,
                agree + rep(header, branch),
            ),
            header.eps_rep_certified && branch.eps_rep_certified,
            rep(header, branch),
        ),
    ]
}

/// Independent checks of a joint (review RB-001, RB-006), from the two finished
/// patterns and the two parts' OWN geometries. Unlike `verify_interface`, the
/// 3D points are not produced by the charts of `develop`: each pattern vertex is
/// placed on its part's surface in the tilted frame of `independent`, and is
/// then tested against the numerical intersection curve of the OTHER part's
/// geometry. Parts made for different surfaces do not pass.
#[allow(clippy::too_many_arguments)]
pub fn verify_joint(
    header: &Pattern,
    hb: &Branch,
    r_dev_header: f64,
    station: f64,
    branch: &Pattern,
    bb: &Branch,
    r_dev_branch: f64,
    s_end: f64,
) -> Vec<Check> {
    let agree = agree_tol(hb.big_r.max(bb.big_r).max(s_end));
    let frame = independent::Frame::tilted(hb.big_r.max(bb.big_r));
    let jh = Joint3::in_frame(frame, hb.big_r, hb.r, hb.beta, hb.delta, station);
    let jb = Joint3::in_frame(frame, bb.big_r, bb.r, bb.beta, bb.delta, station);
    let tol = agree
        + 4.0
            * (develop::opening_error_bound(hb, r_dev_header, station)
                + develop::saddle_error_bound(bb, r_dev_branch));
    // opening vertices on the header surface the opening was cut to
    let open3: Vec<V3> = hole_points(header)
        .iter()
        .map(|q| {
            let omega = match header.marking_side {
                ViewSide::Outside => q.y / r_dev_header - std::f64::consts::PI,
                ViewSide::Inside => std::f64::consts::PI - q.y / r_dev_header,
            };
            jh.header_point(q.x, omega, hb.big_r)
        })
        .collect();
    // saddle vertices on the branch surface the saddle was cut to
    let cut3: Vec<V3> = saddle_points(branch)
        .iter()
        .map(|q| {
            let (phi, s) = fold_branch(branch.marking_side, q, r_dev_branch, s_end);
            jb.branch_point(phi, s, bb.r)
        })
        .collect();
    // each folded vertex against the numerical intersection curve of the other part's geometry,
    // at the vertex's own branch azimuth (no chord error on either side)
    let against = |pts: &[V3], other: &Joint3| -> f64 {
        if pts.is_empty() {
            return f64::NAN;
        }
        let mut worst: f64 = 0.0;
        for x in pts {
            worst = match other.curve_point(other.branch_azimuth(*x)) {
                Some(c) if independent::dist(*x, c).is_finite() => {
                    worst.max(independent::dist(*x, c))
                }
                _ => f64::NAN,
            };
        }
        worst
    };
    // The two folded polylines are the same polygon when both parts share one sampling (develop::*_min), so a
    // comparison of their 3D lengths adds nothing to the two point checks above; and the 3D length of a
    // near-tangent curve cannot be resolved to the 1e-5 that cmp_resolved requires, so that check was INCONCLUSIVE
    // for valid near-tangent joints. It is removed (review RB-001; validation run of 20 Nov 2026).
    vec![
        cmp(
            "opening_on_intersection_of_branch_geometry",
            "independent formulation: each opening vertex, placed on the header surface of the opening's geometry, against the numerical intersection curve of the geometry the BRANCH was cut to",
            against(&open3, &jb),
            0.0,
            tol,
        ),
        cmp(
            "saddle_on_intersection_of_header_geometry",
            "independent formulation: each saddle vertex, placed on the branch surface of the saddle's geometry, against the numerical intersection curve of the geometry the HEADER OPENING was cut to",
            against(&cut3, &jh),
            0.0,
            tol,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::*;

    fn all_pass(v: &[Check]) {
        for c in v {
            assert_eq!(
                c.verdict,
                Verdict::Pass,
                "{} diff {} tol {}",
                c.name,
                c.difference,
                c.tolerance
            );
        }
    }

    /// The eccentric reducer's representation error is not certified (its vertex term carries the quadrature
    /// estimate, TOLERANCE_POLICY 8.2): its two circumference checks are INCONCLUSIVE and every other check PASSes.
    fn pass_but_uncertified_circumferences(v: &[Check]) {
        for c in v {
            let want = if c.name.ends_with("_equals_circumference") {
                Verdict::Inconclusive
            } else {
                Verdict::Pass
            };
            assert_eq!(
                c.verdict, want,
                "{} diff {} tol {}",
                c.name, c.difference, c.tolerance
            );
        }
    }

    #[test]
    fn header_outline_and_opening_share_one_frame() {
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: std::f64::consts::FRAC_PI_3,
            delta: 0.0,
        };
        let hp = header_with_opening(&b, 150.0, 1200.0, 600.0).unwrap();
        all_pass(&verify_header_opening(&hp, &b, 150.0, 1200.0, 600.0));
        // zero offset: the opening is centred on the line opposite the seam, v = pi * r
        let pts = hole_points(&hp);
        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
            (lo.min(q.y), hi.max(q.y))
        });
        assert!(((lo + hi) / 2.0 - std::f64::consts::PI * 150.0).abs() < 1e-9);
        // the old defect: outline drawn with circumference along x. The verifier must reject it.
        let mut bad = hp.clone();
        let w = TAU * 150.0;
        let l = |n, a: (f64, f64), b: (f64, f64)| Curve {
            name: n,
            layer: Layer::CutOuter,
            geom: Geom::Line {
                a: P2::new(a.0, a.1),
                b: P2::new(b.0, b.1),
            },
        };
        bad.outline = vec![
            l("bottom", (0.0, 0.0), (w, 0.0)),
            l("seam_b", (w, 0.0), (w, 1200.0)),
            l("top", (w, 1200.0), (0.0, 1200.0)),
            l("seam_a", (0.0, 1200.0), (0.0, 0.0)),
        ];
        assert!(
            verify_header_opening(&bad, &b, 150.0, 1200.0, 600.0)
                .iter()
                .any(|c| c.verdict != Verdict::Pass)
        );
    }

    #[test]
    fn verifiers_pass_on_correct_patterns() {
        all_pass(&verify_cylinder(
            &cylinder_shell(100.0, 50.0).unwrap(),
            100.0,
            50.0,
        ));
        all_pass(&verify_cone(
            &cone_frustum(200.0, 100.0, 300.0).unwrap(),
            200.0,
            100.0,
            300.0,
        ));
        pass_but_uncertified_circumferences(&verify_eccentric(
            &eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap(),
            200.0,
            100.0,
            300.0,
            100.0,
        ));
        pass_but_uncertified_circumferences(&verify_eccentric(
            &eccentric_reducer(350.0, 120.0, 260.0, 40.0).unwrap(),
            350.0,
            120.0,
            260.0,
            40.0,
        ));
        for (rr, r, beta, delta) in [
            (150.0, 100.0, 90.0, 0.0),
            (150.0, 100.0, 60.0, 0.0),
            (150.0, 80.0, 90.0, 40.0),
            (300.0, 60.0, 35.0, -90.0),
        ] {
            let p = branch_stub(rr, r, r, beta, delta, 900.0).unwrap();
            all_pass(&verify_branch(
                &p,
                &Branch {
                    big_r: rr,
                    r,
                    beta: f64::to_radians(beta),
                    delta,
                },
                r,
                900.0,
            ));
        }
    }

    #[test]
    fn header_opening_and_interface() {
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 20.0,
        };
        let hp = header_with_opening(&b, 150.0, 1200.0, 600.0).unwrap();
        let bp = branch_stub(150.0, 100.0, 100.0, 60.0, 20.0, 400.0).unwrap();
        all_pass(&verify_header_opening(&hp, &b, 150.0, 1200.0, 600.0));
        all_pass(&verify_interface(
            &hp, &b, 150.0, 600.0, &bp, &b, 100.0, 400.0,
        ));
        // seeded fault: branch made against the header's inside surface, header opened for the outside surface
        let wrong = Branch { big_r: 142.0, ..b };
        let bp_wrong = branch_stub(142.0, 100.0, 100.0, 60.0, 20.0, 400.0).unwrap();
        assert!(
            verify_interface(&hp, &b, 150.0, 600.0, &bp_wrong, &wrong, 100.0, 400.0)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
        // seeded fault: branch angle differs between the two parts
        let tilted = Branch {
            beta: 55f64.to_radians(),
            ..b
        };
        let bp_tilt = branch_stub(150.0, 100.0, 100.0, 55.0, 20.0, 400.0).unwrap();
        assert!(
            verify_interface(&hp, &b, 150.0, 600.0, &bp_tilt, &tilted, 100.0, 400.0)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
    }

    // Seeded faults (Spec 02 §12 group 13): each must be caught by a verifier.
    #[test]
    fn seeded_fault_wrong_radius_reference() {
        // pattern made for a radius differing by a wall thickness, verified against the intended one
        let p = cone_frustum(206.0, 100.0, 300.0).unwrap();
        assert!(
            verify_cone(&p, 200.0, 100.0, 300.0)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
        let p = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        assert!(
            verify_eccentric(&p, 200.0, 106.0, 300.0, 100.0)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
    }

    #[test]
    fn seeded_fault_wrong_offset_and_header() {
        let p = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        assert!(
            verify_eccentric(&p, 200.0, 100.0, 300.0, 60.0)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
        // branch cut made for header OD, checked against header ID (inside/outside confusion)
        let p = branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap();
        let wrong = Branch {
            big_r: 142.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 0.0,
        };
        assert!(
            verify_branch(&p, &wrong, 100.0, 400.0)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
    }

    #[test]
    fn seeded_fault_mirrored_or_scaled_pattern() {
        let mut p = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        for c in p.outline.iter_mut() {
            if let Geom::Polyline { pts, .. } = &mut c.geom {
                for q in pts.iter_mut() {
                    q.x *= 1.01; // unit/scale style fault
                }
            }
        }
        assert!(
            verify_eccentric(&p, 200.0, 100.0, 300.0, 100.0)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
    }

    // ---- validation defect D1: marking side

    fn chiral() -> (Branch, f64, f64, f64, f64) {
        // offset and oblique: the joint has no mirror symmetry (the validation report's reproduction)
        (
            Branch {
                big_r: 150.0,
                r: 100.0,
                beta: 60f64.to_radians(),
                delta: 20.0,
            },
            150.0,
            1200.0,
            500.0,
            400.0,
        )
    }

    fn map_points(p: &mut Pattern, f: impl Fn(P2) -> P2) {
        for c in p.outline.iter_mut().chain(p.holes.iter_mut()) {
            match &mut c.geom {
                Geom::Line { a, b } => {
                    *a = f(*a);
                    *b = f(*b);
                }
                Geom::Polyline { pts, .. } => pts.iter_mut().for_each(|q| *q = f(*q)),
                Geom::Arc { .. } => {}
            }
        }
    }

    fn failed(v: &[Check]) -> Vec<&'static str> {
        v.iter()
            .filter(|c| c.verdict != Verdict::Pass)
            .map(|c| c.name)
            .collect()
    }

    #[test]
    fn d1_every_pattern_declares_the_outside_and_the_branch_is_in_the_specification_frame() {
        let (b, hdev, hlen, station, s_end) = chiral();
        let branch = branch_stub(b.big_r, b.r, 100.0, 60.0, b.delta, s_end).unwrap();
        let header = header_with_opening(&b, hdev, hlen, station).unwrap();
        for p in [
            &branch,
            &header,
            &cylinder_shell(100.0, 250.0).unwrap(),
            &cone_frustum(200.0, 100.0, 300.0).unwrap(),
            &eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap(),
        ] {
            assert_eq!(p.marking_side, crate::ViewSide::Outside, "{}", p.family);
            assert_eq!(p.marking_side, crate::MARKING_SIDE);
        }
        // Spec 02 section 4.4: the branch pattern is (r*phi, s), saddle at the bottom, square end at v = s_end
        let cut = saddle_points(&branch);
        for q in cut.iter().step_by(37) {
            assert!((q.y - b.s(q.x / 100.0)).abs() < 1e-9, "{q:?}");
        }
        assert!(
            cut.windows(2).all(|w| w[1].x > w[0].x),
            "phi increases to the right"
        );
        let top = branch
            .outline
            .iter()
            .find(|c| c.name == "square_end")
            .unwrap();
        assert!(matches!(top.geom, Geom::Line { a, b } if a.y == s_end && b.y == s_end));
        // the outward normal: e_u x e_v at phi = 0 is the radial direction (0, 1, 0) of the branch
        let (p0, pu, pv) = (
            b.point(0.0, 300.0),
            b.point(1e-6, 300.0),
            b.point(0.0, 300.0 + 1e-6),
        );
        let (eu, ev) = (
            [pu[0] - p0[0], pu[1] - p0[1], pu[2] - p0[2]],
            [pv[0] - p0[0], pv[1] - p0[1], pv[2] - p0[2]],
        );
        let n = [
            eu[1] * ev[2] - eu[2] * ev[1],
            eu[2] * ev[0] - eu[0] * ev[2],
            eu[0] * ev[1] - eu[1] * ev[0],
        ];
        assert!(
            n[1] > 0.0 && n[0].abs() < 1e-3 * n[1] && n[2].abs() < 1e-3 * n[1],
            "pattern normal must be the outward normal: {n:?}"
        );
        all_pass(&verify_branch(&branch, &b, 100.0, s_end));
        all_pass(&verify_header_opening(&header, &b, hdev, hlen, station));
        all_pass(&verify_interface(
            &header, &b, hdev, station, &branch, &b, 100.0, s_end,
        ));
    }

    /// The state of the product before the fix: header from outside, branch from inside.
    #[test]
    fn d1_parts_drawn_from_different_faces_are_caught() {
        let (b, hdev, hlen, station, s_end) = chiral();
        let header = header_with_opening(&b, hdev, hlen, station).unwrap();
        let good = branch_stub(b.big_r, b.r, 100.0, 60.0, b.delta, s_end).unwrap();
        // the old construction: (r*phi, s_end - s)
        let mut old = good.clone();
        map_points(&mut old, |q| P2::new(q.x, s_end - q.y));
        // 1. it claims to be the outside view: its own fold-back and the interface both fail
        assert_eq!(old.marking_side, crate::ViewSide::Outside);
        let own = failed(&verify_branch(&old, &b, 100.0, s_end));
        assert!(
            own.contains(&"cut_points_on_header_surface")
                && own.contains(&"saddle_handedness_matches_declared_side")
                && own.contains(&"outline_counter_clockwise"),
            "{own:?}"
        );
        let joint = failed(&verify_interface(
            &header, &b, hdev, station, &old, &b, 100.0, s_end,
        ));
        assert!(
            joint.contains(&"branch_cut_lies_on_header_opening")
                && joint.contains(&"header_opening_lies_on_branch_cut"),
            "{joint:?}"
        );
        // 2. it says honestly that it is the inside view: the curves meet in 3D, and the two parts disagree on the side
        old.marking_side = crate::ViewSide::Inside;
        let joint = failed(&verify_interface(
            &header, &b, hdev, station, &old, &b, 100.0, s_end,
        ));
        assert_eq!(
            joint,
            ["parts_declare_the_same_marking_side"],
            "exactly the mismatch the validation found"
        );
    }

    /// Mutation: one part mirrored while it keeps its declaration.
    #[test]
    fn d1_a_mirrored_part_is_caught_whenever_the_joint_is_not_symmetric() {
        let (b, hdev, hlen, station, s_end) = chiral();
        let header = header_with_opening(&b, hdev, hlen, station).unwrap();
        let branch = branch_stub(b.big_r, b.r, 100.0, 60.0, b.delta, s_end).unwrap();
        let (wb, wh) = (TAU * 100.0, TAU * hdev);
        let mut flipped_branch = branch.clone();
        map_points(&mut flipped_branch, |q| P2::new(wb - q.x, q.y));
        let own = failed(&verify_branch(&flipped_branch, &b, 100.0, s_end));
        assert!(
            own.contains(&"cut_points_on_header_surface")
                && own.contains(&"saddle_handedness_matches_declared_side"),
            "{own:?}"
        );
        assert!(
            !failed(&verify_interface(
                &header,
                &b,
                hdev,
                station,
                &flipped_branch,
                &b,
                100.0,
                s_end
            ))
            .is_empty()
        );
        let mut flipped_header = header.clone();
        map_points(&mut flipped_header, |q| P2::new(q.x, wh - q.y));
        let own = failed(&verify_header_opening(
            &flipped_header,
            &b,
            hdev,
            hlen,
            station,
        ));
        assert!(own.contains(&"opening_points_on_branch_surface"), "{own:?}");
        assert!(
            !failed(&verify_interface(
                &flipped_header,
                &b,
                hdev,
                station,
                &branch,
                &b,
                100.0,
                s_end
            ))
            .is_empty()
        );
        // across a sweep of chiral joints the mirrored branch never passes
        for (beta, delta) in [
            (45.0f64, 40.0),
            (75.0, 5.0),
            (120.0, -30.0),
            (60.0, 1.0),
            (30.0, 10.0),
        ] {
            let g = Branch {
                big_r: 150.0,
                r: 100.0,
                beta: beta.to_radians(),
                delta,
            };
            let h = header_with_opening(&g, 150.0, 2400.0, 1200.0).unwrap();
            let mut m = branch_stub(150.0, 100.0, 100.0, beta, delta, 700.0).unwrap();
            all_pass(&verify_interface(
                &h, &g, 150.0, 1200.0, &m, &g, 100.0, 700.0,
            ));
            map_points(&mut m, |q| P2::new(wb - q.x, q.y));
            assert!(
                !failed(&verify_branch(&m, &g, 100.0, 700.0)).is_empty(),
                "beta {beta} delta {delta}"
            );
            assert!(
                !failed(&verify_interface(
                    &h, &g, 150.0, 1200.0, &m, &g, 100.0, 700.0
                ))
                .is_empty(),
                "beta {beta} delta {delta}"
            );
        }
        // a square, centred branch is its own mirror image: the mirrored pattern is the same part, and passes the 3D checks
        let g = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 90f64.to_radians(),
            delta: 0.0,
        };
        let h = header_with_opening(&g, 150.0, 1200.0, 600.0).unwrap();
        let mut m = branch_stub(150.0, 100.0, 100.0, 90.0, 0.0, 400.0).unwrap();
        map_points(&mut m, |q| P2::new(wb - q.x, q.y));
        let joint = failed(&verify_interface(
            &h, &g, 150.0, 600.0, &m, &g, 100.0, 400.0,
        ));
        assert!(joint.is_empty(), "{joint:?}");
    }

    // ---- reviews of 19 November 2026: RB-001, RB-002, RB-006

    fn verdict_of(v: &[Check], name: &str) -> Verdict {
        v.iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no check {name}"))
            .verdict
    }

    /// RB-001. The reviewer's joint: DN300 header (outside radius 161.95), DN150 branch (bore radius
    /// 77.095, outside radius 84.15). A set-on branch sits with its bore on the header outside surface.
    /// The defect: the header opening was cut to the branch OUTSIDE radius while "one geometry for both
    /// parts" made the interface check blind. With each part's own geometry the check sees it.
    #[test]
    fn rb001_parts_cut_for_different_surfaces_fail_the_interface_checks() {
        let seat = Branch {
            big_r: 161.95,
            r: 77.095,
            beta: 90f64.to_radians(),
            delta: 0.0,
        };
        let outside = Branch { r: 84.15, ..seat };
        let (hdev, bdev, station, s_end) = (156.795, 80.6, 600.0, 400.0);
        // correct set-on joint: opening and saddle both where the bore meets the header outside surface
        let header = header_with_opening(&seat, hdev, 1200.0, station).unwrap();
        let branch = branch_stub(seat.big_r, seat.r, bdev, 90.0, 0.0, s_end).unwrap();
        all_pass(&verify_interface(
            &header, &seat, hdev, station, &branch, &seat, bdev, s_end,
        ));
        all_pass(&verify_joint(
            &header, &seat, hdev, station, &branch, &seat, bdev, s_end,
        ));
        // width of the opening across the header: the bore (2 x 77.095 = 154.19), not the outside diameter
        let width = |h: &Pattern, g: &Branch| -> f64 {
            let (lo, hi) = hole_points(h)
                .iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), q| {
                    let y = fold_header(h.marking_side, q, g.big_r, hdev, station)[1];
                    (lo.min(y), hi.max(y))
                });
            hi - lo
        };
        assert!((width(&header, &seat) - 154.19).abs() < 1e-9);
        // the defect: opening cut at the branch outside diameter, saddle for the bore
        let wrong_header = header_with_opening(&outside, hdev, 1200.0, station).unwrap();
        assert!(
            (width(&wrong_header, &outside) - 168.30).abs() < 1e-9,
            "the branch would drop through"
        );
        // 1. as the product called it before the fix (ONE geometry for both parts): everything passes
        all_pass(&verify_header_opening(
            &wrong_header,
            &outside,
            hdev,
            1200.0,
            station,
        ));
        let wrong_branch = branch_stub(outside.big_r, outside.r, bdev, 90.0, 0.0, s_end).unwrap();
        all_pass(&verify_interface(
            &wrong_header,
            &outside,
            hdev,
            station,
            &wrong_branch,
            &outside,
            bdev,
            s_end,
        ));
        // 2. with each part's own geometry the mismatch is a FAIL, in the fold-back and in the independent check
        let joint = failed(&verify_interface(
            &wrong_header,
            &outside,
            hdev,
            station,
            &branch,
            &seat,
            bdev,
            s_end,
        ));
        assert!(
            joint.contains(&"branch_cut_lies_on_header_opening")
                && joint.contains(&"header_opening_lies_on_branch_cut"),
            "{joint:?}"
        );
        let solid = failed(&verify_joint(
            &wrong_header,
            &outside,
            hdev,
            station,
            &branch,
            &seat,
            bdev,
            s_end,
        ));
        assert!(
            solid.contains(&"opening_on_intersection_of_branch_geometry")
                && solid.contains(&"saddle_on_intersection_of_header_geometry"),
            "{solid:?}"
        );
        // the mirror case: saddle shaped for the header INSIDE radius, opening for the outside (1.5 mm misfit)
        let inner = Branch {
            big_r: 151.64,
            ..seat
        };
        let deep = branch_stub(inner.big_r, inner.r, bdev, 90.0, 0.0, s_end).unwrap();
        assert!(
            !failed(&verify_interface(
                &header, &seat, hdev, station, &deep, &inner, bdev, s_end
            ))
            .is_empty()
        );
        assert!(
            !failed(&verify_joint(
                &header, &seat, hdev, station, &deep, &inner, bdev, s_end
            ))
            .is_empty()
        );
    }

    /// RB-002. The sign of the offset is tied to the plate: for delta > 0 the opening lies above the
    /// centreline of the header plate and the seam of the branch is the generator on that side. A part
    /// made for -delta is the mirror image; verified against +delta it fails the delta-side checks.
    #[test]
    fn rb002_a_pattern_made_for_the_other_sign_of_delta_fails_the_delta_side_checks() {
        let (b, hdev, hlen, station, s_end) = chiral();
        let minus = Branch {
            delta: -b.delta,
            ..b
        };
        let header = header_with_opening(&b, hdev, hlen, station).unwrap();
        let branch = branch_stub(b.big_r, b.r, 100.0, 60.0, b.delta, s_end).unwrap();
        // delta = +20: opening centred above the plate centreline, by construction of the outside view
        let pts = hole_points(&header);
        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
            (lo.min(q.y), hi.max(q.y))
        });
        assert!(
            0.5 * (lo + hi) > std::f64::consts::PI * hdev + 10.0,
            "opening on the +delta side of the centreline"
        );
        assert_eq!(
            verdict_of(
                &verify_header_opening(&header, &b, hdev, hlen, station),
                "opening_centre_on_the_declared_delta_side"
            ),
            Verdict::Pass
        );
        assert_eq!(
            verdict_of(
                &verify_branch(&branch, &b, 100.0, s_end),
                "saddle_on_the_declared_delta_side"
            ),
            Verdict::Pass
        );
        // the parts of the mirror-image assembly, checked against the declared sign
        let header_m = header_with_opening(&minus, hdev, hlen, station).unwrap();
        let branch_m = branch_stub(b.big_r, b.r, 100.0, 60.0, -b.delta, s_end).unwrap();
        let own = failed(&verify_header_opening(&header_m, &b, hdev, hlen, station));
        assert!(
            own.contains(&"opening_centre_on_the_declared_delta_side")
                && own.contains(&"opening_matches_3d_intersection"),
            "{own:?}"
        );
        let own = failed(&verify_branch(&branch_m, &b, 100.0, s_end));
        assert!(
            own.contains(&"saddle_on_the_declared_delta_side")
                && own.contains(&"saddle_matches_3d_intersection"),
            "{own:?}"
        );
        // against its own sign the mirror assembly is of course consistent: the sign itself is an input,
        // which is why every pattern and every DXF states what positive delta means
        all_pass(&verify_header_opening(
            &header_m, &minus, hdev, hlen, station,
        ));
        for p in [&header, &branch] {
            assert!(
                p.conventions.iter().any(|c| c.starts_with(
                    "OFFSET DELTA (20 MM HERE): POSITIVE = THE BRANCH AXIS IS DISPLACED TO THE LEFT"
                )),
                "{:?}",
                p.conventions
            );
            assert!(
                p.conventions
                    .iter()
                    .any(|c| c.contains("BELOW 90 DEG THE BRANCH LEANS TOWARD INCREASING STATION"))
            );
        }
    }

    /// RB-006. The independent-formulation checks detect faults on their own: each fault below is judged
    /// by the checks that share nothing with `develop`.
    #[test]
    fn rb006_independent_checks_detect_faults_without_the_fold_back() {
        let (b, hdev, hlen, station, s_end) = chiral();
        let branch = branch_stub(b.big_r, b.r, 100.0, 60.0, b.delta, s_end).unwrap();
        let header = header_with_opening(&b, hdev, hlen, station).unwrap();
        let independent_branch = [
            "saddle_matches_3d_intersection",
            "saddle_developed_length_equals_3d_arc_length",
            "pattern_area_equals_surface_area",
            "square_end_3d_arc_length",
        ];
        let independent_header = [
            "opening_matches_3d_intersection",
            "opening_developed_length_equals_3d_arc_length",
            "opening_area_equals_surface_area",
            "header_circle_3d_arc_length",
        ];
        let good = verify_branch(&branch, &b, 100.0, s_end);
        for name in independent_branch {
            assert_eq!(verdict_of(&good, name), Verdict::Pass, "{name}");
        }
        let good = verify_header_opening(&header, &b, hdev, hlen, station);
        for name in independent_header {
            assert_eq!(verdict_of(&good, name), Verdict::Pass, "{name}");
        }
        // saddle heights 0.05 % too large
        let mut tall = branch.clone();
        for c in tall.outline.iter_mut() {
            if c.name == "saddle_cut"
                && let Geom::Polyline { pts, .. } = &mut c.geom
            {
                pts.iter_mut().for_each(|q| q.y *= 1.0005);
            }
        }
        let v = verify_branch(&tall, &b, 100.0, s_end);
        assert_eq!(
            verdict_of(&v, "saddle_matches_3d_intersection"),
            Verdict::Fail
        );
        assert_eq!(
            verdict_of(&v, "pattern_area_equals_surface_area"),
            Verdict::Fail
        );
        // a branch developed with a radius 0.1 % too large: lengths and area disagree with the solid
        let wide = branch_stub(b.big_r, b.r, 100.1, 60.0, b.delta, s_end).unwrap();
        let v = verify_branch(&wide, &b, 100.0, s_end);
        for name in [
            "saddle_developed_length_equals_3d_arc_length",
            "pattern_area_equals_surface_area",
            "square_end_3d_arc_length",
        ] {
            assert_eq!(verdict_of(&v, name), Verdict::Fail, "{name}");
        }
        // opening shifted 0.05 mm along the header axis; opening scaled 0.1 % about its centre
        let mut shifted = header.clone();
        if let Geom::Polyline { pts, .. } = &mut shifted.holes[0].geom {
            pts.iter_mut().for_each(|q| q.x += 0.05);
        }
        assert_eq!(
            verdict_of(
                &verify_header_opening(&shifted, &b, hdev, hlen, station),
                "opening_matches_3d_intersection"
            ),
            Verdict::Fail
        );
        let mut scaled = header.clone();
        if let Geom::Polyline { pts, .. } = &mut scaled.holes[0].geom {
            let c = pts.iter().fold((0.0, 0.0), |a, q| {
                (a.0 + q.x / pts.len() as f64, a.1 + q.y / pts.len() as f64)
            });
            pts.iter_mut()
                .for_each(|q| *q = P2::new(c.0 + 1.001 * (q.x - c.0), c.1 + 1.001 * (q.y - c.1)));
        }
        let v = verify_header_opening(&scaled, &b, hdev, hlen, station);
        for name in [
            "opening_matches_3d_intersection",
            "opening_developed_length_equals_3d_arc_length",
            "opening_area_equals_surface_area",
        ] {
            assert_eq!(verdict_of(&v, name), Verdict::Fail, "{name}");
        }
        // plain shell and cone: the sampled solid disagrees with a plate made for another radius
        all_pass(&verify_cylinder_solid(
            &cylinder_shell(100.0, 250.0).unwrap(),
            100.0,
            250.0,
        ));
        let v = verify_cylinder_solid(&cylinder_shell(100.05, 250.0).unwrap(), 100.0, 250.0);
        for name in [
            "end_circle_3d_arc_length",
            "end_circles_3d_arc_length_equal",
            "area_equals_sampled_surface_area",
        ] {
            assert_eq!(verdict_of(&v, name), Verdict::Fail, "{name}");
        }
        // the handedness guard does not depend on the radius: the wrong-radius shell is still counter-clockwise
        assert_eq!(verdict_of(&v, "outline_counter_clockwise"), Verdict::Pass);
        let v = verify_cone(
            &cone_frustum(200.2, 100.0, 300.0).unwrap(),
            200.0,
            100.0,
            300.0,
        );
        assert_eq!(verdict_of(&v, "large_end_3d_arc_length"), Verdict::Fail);
        // a check that cannot resolve its own comparison is INCONCLUSIVE, never a pass (ALG-13)
        let c = cmp_resolved("x", "m", 1.0, 1.0, 1e-6, 0.5, 1e-3);
        assert_eq!(c.verdict, Verdict::Inconclusive);
        assert_eq!(
            cmp_resolved("x", "m", 1.0, 1.0, 1e-6, 1e-9, 1e-3).verdict,
            Verdict::Pass
        );
    }

    /// The independent checks hold up to the limits of the accepted range: next to tangency, at grazing
    /// angles, for very small and very large parts, and when the polyline was refined.
    #[test]
    fn rb006_independent_checks_pass_across_the_accepted_range() {
        for (big_r, r, rdev, beta, delta) in [
            (150.0, 149.99978, 149.99978, 90.0, 0.0),
            (150.0, 99.9998, 99.9998, 15.0, -50.0),
            (1000.0, 699.9, 699.9, 15.0, -300.0),
            (300.0, 60.0, 57.0, 35.0, -90.0),
            (0.0015, 0.001, 0.001, 120.0, 0.0002),
            (40_000.0, 25_000.0, 24_990.0, 75.0, 3_000.0),
            (150.0, 100.0, 100.0, 168.0, 20.0),
        ] {
            let b = Branch {
                big_r,
                r,
                beta: f64::to_radians(beta),
                delta,
            };
            let s_end = 12.0 * big_r;
            let p = branch_stub(big_r, r, rdev, beta, delta, s_end).unwrap();
            all_pass(&verify_branch(&p, &b, rdev, s_end));
            let h = header_with_opening(&b, big_r * 0.97, 60.0 * big_r, 30.0 * big_r).unwrap();
            all_pass(&verify_header_opening(
                &h,
                &b,
                big_r * 0.97,
                60.0 * big_r,
                30.0 * big_r,
            ));
            all_pass(&verify_interface(
                &h,
                &b,
                big_r * 0.97,
                30.0 * big_r,
                &p,
                &b,
                rdev,
                s_end,
            ));
            all_pass(&verify_joint(
                &h,
                &b,
                big_r * 0.97,
                30.0 * big_r,
                &p,
                &b,
                rdev,
                s_end,
            ));
        }
    }

    // ---- validation defect D8: tolerance must not approach the size of the part

    #[test]
    fn d8_agreement_tolerance_is_relative_below_one_millimetre_and_unchanged_above() {
        for size in [1.0, 1.5, 100.0, 1e5, 1e8] {
            assert_eq!(agree_tol(size), AGREE_ABS, "{size}");
        }
        assert_eq!(agree_tol(1e-3), 1e-9);
        assert_eq!(agree_tol(f64::NAN), AGREE_ABS);
        // reference case CYL-02 (r 500, L 1000): thresholds exactly as before; the handedness guard (0.5, a sign
        // test with no radius) is the one check added after them
        let before = [1e-6, 1e-6, 1e-6, 1e-6 * 1500.0, 0.5];
        let now: Vec<f64> = verify_cylinder(&cylinder_shell(500.0, 1000.0).unwrap(), 500.0, 1000.0)
            .iter()
            .map(|c| c.tolerance)
            .collect();
        assert_eq!(now, before);
        // a 10 micrometre shell made with a radius 5 % too large used to pass (error 3e-6 mm against 1e-6 mm
        // would fail, but at 1 micrometre every error was below the absolute threshold); now it is caught
        let wrong = cylinder_shell(1.05e-4, 1e-3).unwrap();
        assert!(
            verify_cylinder(&wrong, 1e-4, 1e-3)
                .iter()
                .any(|c| c.verdict == Verdict::Fail)
        );
        let tiny = cylinder_shell(1e-4, 1e-3).unwrap();
        all_pass(&verify_cylinder(&tiny, 1e-4, 1e-3));
    }

    /// Handedness guard of the plain shell and the eccentric reducer (review RC-09/RC-10, RB-006): a genuine
    /// development is counter-clockwise and passes `outline_counter_clockwise`; its mirror image (every x negated) is
    /// clockwise and fails it. The mirror passes every length check, so this guard is the only one that names it.
    #[test]
    fn outline_guard_rejects_a_mirrored_cylinder_and_eccentric() {
        let guard = "outline_counter_clockwise";
        let cyl = cylinder_shell(100.0, 250.0).unwrap();
        let mut cyl_mirror = cyl.clone();
        map_points(&mut cyl_mirror, |q| P2::new(-q.x, q.y));
        let ecc = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        let mut ecc_mirror = ecc.clone();
        map_points(&mut ecc_mirror, |q| P2::new(-q.x, q.y));

        assert_eq!(
            verdict_of(&verify_cylinder(&cyl, 100.0, 250.0), guard),
            Verdict::Pass
        );
        assert_eq!(
            verdict_of(&verify_cylinder_solid(&cyl, 100.0, 250.0), guard),
            Verdict::Pass
        );
        assert_eq!(
            verdict_of(&verify_eccentric(&ecc, 200.0, 100.0, 300.0, 100.0), guard),
            Verdict::Pass
        );
        assert_eq!(
            verdict_of(&verify_cylinder(&cyl_mirror, 100.0, 250.0), guard),
            Verdict::Fail
        );
        assert_eq!(
            verdict_of(&verify_cylinder_solid(&cyl_mirror, 100.0, 250.0), guard),
            Verdict::Fail
        );
        assert_eq!(
            verdict_of(
                &verify_eccentric(&ecc_mirror, 200.0, 100.0, 300.0, 100.0),
                guard
            ),
            Verdict::Fail
        );
    }

    // ---- work plan task 1.1: the verifier's own rounding at shallow and near-180 degree branch angles

    /// a·b + c·d + e·f of binary64 numbers as an unevaluated sum hi + lo (error-free products, compensated sum):
    /// exact to far below 1e-30 for the frame constants.
    fn exact_dot(a: V3, b: V3) -> f64 {
        let (mut hi, mut lo) = (0.0f64, 0.0f64);
        for i in 0..3 {
            let p = a[i] * b[i];
            let e = a[i].mul_add(b[i], -p);
            let s = hi + p;
            let bb = s - hi;
            lo += (hi - (s - bb)) + (p - bb) + e;
            hi = s;
        }
        hi + lo
    }

    #[test]
    fn frame_defect_is_at_most_one_unit_roundoff() {
        let f = independent::Frame::tilted(1.0);
        for (a, b, want) in [
            (f.ex, f.ex, 1.0),
            (f.ey, f.ey, 1.0),
            (f.ez, f.ez, 1.0),
            (f.ex, f.ey, 0.0),
            (f.ex, f.ez, 0.0),
            (f.ey, f.ez, 0.0),
        ] {
            // the difference from the wanted value, taken before rounding to binary64 where it matters
            let p: Vec<f64> = (0..3).map(|i| a[i] * b[i]).collect();
            let e: Vec<f64> = (0..3).map(|i| a[i].mul_add(b[i], -p[i])).collect();
            let defect = ((p[0] - want) + p[1] + p[2] + e[0] + e[1] + e[2]).abs();
            assert!(defect <= FRAME_DEFECT, "{defect:e}");
            assert!((exact_dot(a, b) - want).abs() <= FRAME_DEFECT);
        }
        let o = f.o;
        assert!((o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt() <= FRAME_ORIGIN);
    }

    /// A branch stub and a header with branch laid out as the validation corpus lays them out: the plate just
    /// long enough for the opening, the square end just beyond the saddle, both parts with one sampling.
    fn joint_at(
        big_r: f64,
        r: f64,
        beta_deg: f64,
        delta: f64,
    ) -> Option<(Branch, Pattern, Pattern, f64, f64, f64)> {
        let b = Branch {
            big_r,
            r,
            beta: beta_deg.to_radians(),
            delta,
        };
        let (sb, cb) = b.beta.sin_cos();
        let (mut xmin, mut xmax, mut smax) = (f64::MAX, f64::MIN, f64::MIN);
        for i in 0..=4000 {
            let ph = TAU * i as f64 / 4000.0;
            let s = b.s(ph);
            xmin = xmin.min(s * cb - r * ph.sin() * sb);
            xmax = xmax.max(s * cb - r * ph.sin() * sb);
            smax = smax.max(s);
        }
        let (station, length, s_end) = (
            (-xmin + big_r).ceil(),
            (xmax - xmin + 2.0 * big_r).ceil(),
            (smax + big_r).ceil(),
        );
        let p = branch_stub(big_r, r, r, beta_deg, delta, s_end).ok()?;
        let h = header_with_opening(&b, big_r, length, station).ok()?;
        let n = |q: &Pattern| q.quantity("polyline_segments").unwrap() as usize;
        let k = n(&p).max(n(&h));
        let p = branch_stub_min(big_r, r, r, beta_deg, delta, s_end, k).ok()?;
        let h = header_with_opening_min(&b, big_r, length, station, k).ok()?;
        Some((b, p, h, station, length, s_end))
    }

    /// The a-priori bound on the verifier's own error covers every measured difference of the three point checks, at
    /// both ends of the angle range and across sizes, ratios and offsets; and where the bound is inside the budget
    /// (outside the regime of `branch_angle_refusal`) those checks pass. Largest measured difference / bound in the
    /// sweep of TOLERANCE_POLICY section 7: 0.19 (saddle), 0.08 (opening), 0.15 (joint).
    #[test]
    fn task_1_1_verifier_rounding_bound_covers_the_measured_differences() {
        let mut worst: f64 = 0.0;
        for (big_r, r, delta, betas) in [
            (
                150.0,
                100.0,
                0.0,
                &[0.002, 0.009, 0.03, 0.3, 60.0, 179.9, 179.97, 179.997][..],
            ),
            (150.0, 45.0, 31.5, &[0.004, 0.05, 120.0, 179.99][..]),
            (1e4, 6.67e3, 0.0, &[1.0, 179.0][..]),
            (2.0, 0.6, -0.5, &[0.001, 179.999][..]),
        ] {
            for &beta_deg in betas {
                let Some((b, p, h, station, length, s_end)) = joint_at(big_r, r, beta_deg, delta)
                else {
                    panic!("R {big_r} r {r} delta {delta} beta {beta_deg}: refused");
                };
                let mut all = verify_branch(&p, &b, r, s_end);
                all.extend(verify_header_opening(&h, &b, big_r, length, station));
                all.extend(verify_joint(&h, &b, big_r, station, &p, &b, r, s_end));
                let cond = header_with_branch_conditioning(&b, big_r, length, station, r, s_end);
                let d = |name: &str| all.iter().find(|c| c.name == name).unwrap();
                let pairs = [
                    (d("saddle_matches_3d_intersection"), cond[0]),
                    (d("opening_matches_3d_intersection"), cond[1]),
                    (d("opening_on_intersection_of_branch_geometry"), cond[2]),
                    (d("saddle_on_intersection_of_header_geometry"), cond[2]),
                ];
                for (c, k) in pairs {
                    worst = worst.max(c.difference / k.rounding);
                    assert!(
                        c.difference < k.rounding,
                        "R {big_r} r {r} beta {beta_deg}: {} {:e} >= bound {:e}",
                        c.name,
                        c.difference,
                        k.rounding
                    );
                    if !cond.iter().any(Conditioning::exceeded) {
                        assert_eq!(
                            c.verdict,
                            Verdict::Pass,
                            "R {big_r} r {r} beta {beta_deg}: {} outside the regime",
                            c.name
                        );
                    }
                }
            }
        }
        println!("largest measured difference / bound: {worst:.3}");
        assert!(worst < 0.5);
    }

    #[test]
    fn task_1_1_refusal_is_the_verifiers_criterion_inside_the_regime_only() {
        let check = |name: &'static str, verdict: Verdict, difference: f64| Check {
            name,
            method: "",
            independence: "I-F",
            primary: 0.0,
            reference: 0.0,
            difference,
            tolerance: 1.0,
            verdict,
            reason: None,
        };
        const SADDLE: &[&str] = &["saddle_matches_3d_intersection"];
        let inside = [Conditioning {
            checks: SADDLE,
            rounding: 3.0,
            bound: 0.1,
            budget: 1.0,
        }];
        let outside = [Conditioning {
            checks: SADDLE,
            rounding: 0.5,
            bound: 0.1,
            budget: 1.0,
        }];
        let fail = vec![
            check("saddle_matches_3d_intersection", Verdict::Fail, 2.0),
            check("square_end_equals_circumference", Verdict::Pass, 0.0),
        ];
        // in the regime, a failure the conditioning explains is refused with the code, naming beta_deg, limit and bound
        let e = branch_angle_refusal(0.002, &inside, &fail).expect("refused");
        assert_eq!(e.code(), crate::refusal::BRANCH_ANGLE_NOT_VERIFIABLE);
        assert_eq!(e.fields(), ["beta_deg"]);
        assert_eq!(e.detail(), [("limit", 1.0), ("measured", 3.0)]);
        assert!(e.to_string().contains("beta_deg = 0.002"));
        // outside the regime the same failure is a wrong pattern: no refusal, G10 rejects the job
        assert!(branch_angle_refusal(0.002, &outside, &fail).is_none());
        // in the regime, a difference larger than the engine's bound plus the verifier's rounding is a wrong pattern
        let big = vec![check("saddle_matches_3d_intersection", Verdict::Fail, 3.2)];
        assert!(branch_angle_refusal(0.002, &inside, &big).is_none());
        let nan = vec![check(
            "saddle_matches_3d_intersection",
            Verdict::Fail,
            f64::NAN,
        )];
        assert!(branch_angle_refusal(0.002, &inside, &nan).is_none());
        // in the regime, a pattern the verifier confirms is not refused
        let pass = vec![check("saddle_matches_3d_intersection", Verdict::Pass, 0.5)];
        assert!(branch_angle_refusal(0.002, &inside, &pass).is_none());
        // a failing check without its own bound is not excused: fold-back, or a length check of the same family
        for other in [
            "cut_points_on_header_surface",
            "saddle_developed_length_equals_3d_arc_length",
        ] {
            let v = vec![
                check("saddle_matches_3d_intersection", Verdict::Fail, 2.0),
                check(other, Verdict::Fail, 2.0),
            ];
            assert!(
                branch_angle_refusal(0.002, &inside, &v).is_none(),
                "{other}"
            );
        }
        // the failing check's OWN conditioning must be exceeded: the joint checks failing while only the saddle
        // check is in the regime are a wrong pattern
        const JOINT: &[&str] = &[
            "opening_on_intersection_of_branch_geometry",
            "saddle_on_intersection_of_header_geometry",
        ];
        let mixed = [
            inside[0],
            Conditioning {
                checks: JOINT,
                rounding: 0.5,
                bound: 0.1,
                budget: 1.0,
            },
        ];
        let jf = vec![check(
            "opening_on_intersection_of_branch_geometry",
            Verdict::Fail,
            0.55,
        )];
        assert!(branch_angle_refusal(0.002, &mixed, &jf).is_none());
        // an INCONCLUSIVE check is not a failure of G10
        let inc = vec![check(
            "saddle_matches_3d_intersection",
            Verdict::Inconclusive,
            2.0,
        )];
        assert!(branch_angle_refusal(0.002, &inside, &inc).is_none());
        // a bound that is not a number counts as exceeded
        assert!(
            Conditioning {
                checks: SADDLE,
                rounding: f64::NAN,
                bound: 0.1,
                budget: 1.0
            }
            .exceeded()
        );
    }

    #[test]
    fn task_1_2_a_would_be_pass_with_an_uncertified_or_coarse_bound_is_inconclusive_a_fail_stays_fail()
     {
        // the tolerance stays finite and the difference is zero: only the certification and the precision decide
        let c = cmp("x", "m", 1.0, 1.0, 1e-3);
        assert_eq!(c.verdict, Verdict::Pass);
        assert_eq!(rep_check(c.clone(), true, 0.0096).verdict, Verdict::Pass);
        assert_eq!(
            rep_check(c.clone(), false, 0.0096).verdict,
            Verdict::Inconclusive
        );
        // review after task 1.2: a certified bound coarser than MAX_CHORD_ERROR_MM is INCONCLUSIVE, never PASS
        assert_eq!(
            rep_check(c.clone(), true, 0.0100389).verdict,
            Verdict::Inconclusive
        );
        assert_eq!(
            rep_check(c.clone(), true, 0.5641).verdict,
            Verdict::Inconclusive
        );
        assert_eq!(
            rep_check(c.clone(), true, f64::NAN).verdict,
            Verdict::Inconclusive
        );
        // a measured difference beyond the tolerance is a FAIL whatever the certification and the precision
        let f = cmp("x", "m", 1.0, 2.0, 1e-3);
        assert_eq!(f.verdict, Verdict::Fail);
        assert_eq!(rep_check(f.clone(), true, 0.0096).verdict, Verdict::Fail);
        assert_eq!(rep_check(f.clone(), false, 0.0096).verdict, Verdict::Fail);
        assert_eq!(rep_check(f, true, 0.5641).verdict, Verdict::Fail);
    }

    /// Planted faults INSIDE the regime (stub at 0.003 deg, ratio 39; header with branch at 179.99 deg, ratio 1.11):
    /// a wrong pattern must fail G10 and must NOT be excused as `branch_angle_not_verifiable`.
    #[test]
    fn task_1_1_a_wrong_pattern_inside_the_regime_is_rejected_not_refused() {
        let scale_saddle = |p: &mut Pattern, k: f64| {
            for c in p.outline.iter_mut().filter(|c| c.name == "saddle_cut") {
                if let Geom::Polyline { pts, .. } = &mut c.geom {
                    for q in pts.iter_mut() {
                        q.y *= k;
                    }
                }
            }
        };
        // stub, saddle heights 1 part in 1e6 too large (a radius error of the same order)
        let (b, mut p, _h, _st, _len, s_end) = joint_at(150.0, 100.0, 0.003, 0.0).unwrap();
        let cond = branch_stub_conditioning(&b, 100.0, s_end);
        assert!(cond.iter().any(Conditioning::exceeded));
        assert!(
            verify_branch(&p, &b, 100.0, s_end)
                .iter()
                .all(|c| c.verdict == Verdict::Pass)
        );
        scale_saddle(&mut p, 1.0 + 1e-6);
        let c = verify_branch(&p, &b, 100.0, s_end);
        assert!(c.iter().any(|c| c.verdict == Verdict::Fail));
        assert!(
            branch_angle_refusal(0.003, &cond, &c).is_none(),
            "stub fault excused"
        );
        // header with branch at 179.99 deg: the opening shifted 0.05 mm along the axis, and separately the branch
        // saddle scaled as above
        let (b, p, h, st, len, s_end) = joint_at(150.0, 100.0, 179.99, 0.0).unwrap();
        let cond = header_with_branch_conditioning(&b, 150.0, len, st, 100.0, s_end);
        assert!(cond.iter().any(Conditioning::exceeded));
        let all = |h: &Pattern, p: &Pattern| -> Vec<Check> {
            let mut v = verify_header_opening(h, &b, 150.0, len, st);
            v.extend(verify_branch(p, &b, 100.0, s_end));
            v.extend(verify_interface(h, &b, 150.0, st, p, &b, 100.0, s_end));
            v.extend(verify_joint(h, &b, 150.0, st, p, &b, 100.0, s_end));
            v
        };
        assert!(all(&h, &p).iter().all(|c| c.verdict == Verdict::Pass));
        let mut h2 = h.clone();
        for c in h2.holes.iter_mut() {
            if let Geom::Polyline { pts, .. } = &mut c.geom {
                for q in pts.iter_mut() {
                    q.x += 0.05;
                }
            }
        }
        let c = all(&h2, &p);
        assert!(c.iter().any(|c| c.verdict == Verdict::Fail));
        assert!(
            branch_angle_refusal(179.99, &cond, &c).is_none(),
            "opening fault excused"
        );
        let mut p2 = p.clone();
        scale_saddle(&mut p2, 1.0 + 1e-6);
        let c = all(&h, &p2);
        assert!(c.iter().any(|c| c.verdict == Verdict::Fail));
        assert!(
            branch_angle_refusal(179.99, &cond, &c).is_none(),
            "saddle fault excused"
        );
    }

    /// Task 1.4: `surface_unknown` turns a would-be PASS of a radius check into INCONCLUSIVE with its reason, keeps a
    /// FAIL as FAIL, and leaves every other check and every other surface (including no declaration) unchanged.
    #[test]
    fn task_1_4_unknown_surface_makes_radius_passes_inconclusive_and_keeps_fails() {
        let check = |name: &'static str, verdict: Verdict| Check {
            name,
            method: "",
            independence: "I-F",
            primary: 0.0,
            reference: 0.0,
            difference: 0.0,
            tolerance: 1.0,
            verdict,
            reason: None,
        };
        let input = || {
            vec![
                check("area", Verdict::Pass),
                check("seam_edges_equal_length", Verdict::Pass),
                check("bottom_edge_equals_circumference", Verdict::Fail),
            ]
        };
        let out = surface_unknown(input(), Some("unknown"), CYLINDER_RADIUS_CHECKS);
        assert_eq!(out[0].verdict, Verdict::Inconclusive);
        assert_eq!(out[0].reason, Some(SURFACE_UNKNOWN_REASON));
        assert_eq!(out[1].verdict, Verdict::Pass, "not radius-dependent");
        assert_eq!(out[1].reason, None);
        assert_eq!(out[2].verdict, Verdict::Fail, "a FAIL stays FAIL");
        assert_eq!(out[2].reason, None);
        for surface in [None, Some("outside"), Some("inside"), Some("neutral")] {
            let same = surface_unknown(input(), surface, CYLINDER_RADIUS_CHECKS);
            for (a, b) in same.iter().zip(input().iter()) {
                assert_eq!(a.verdict, b.verdict, "{surface:?}");
                assert_eq!(a.reason, None, "{surface:?}");
            }
        }
        assert!(SURFACE_UNKNOWN_REASON.starts_with("surface unknown:"));
        assert!(SURFACE_UNKNOWN_REASON.contains('基'));
    }
}
