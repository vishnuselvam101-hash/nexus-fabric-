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

use crate::develop::{self, Branch, ObliqueCone};
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
            profile::check(&p.outline, 1e-9).signed_area,
            circle * length,
            agree * (r_dev + length) + circle_res * length,
        ),
    ]
}

pub fn verify_cylinder(p: &Pattern, r_dev: f64, length: f64) -> Vec<Check> {
    let agree = agree_tol(r_dev.max(length));
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
            profile::check(&p.outline, 1e-9).signed_area,
            TAU * r_dev * length,
            agree * (r_dev + length),
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
    v.push(cmp(
        "large_edge_equals_circumference",
        "3D circumference 2*pi*r1 (polyline chord shortening allowed for)",
        curve_len(p, "large_end"),
        TAU * r1,
        chord_bias(r1) + p.eps_rep,
    ));
    v.push(cmp(
        "small_edge_equals_circumference",
        "3D circumference 2*pi*r2 (polyline chord shortening allowed for)",
        curve_len(p, "small_end"),
        TAU * r2,
        chord_bias(r2) + p.eps_rep,
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

/// Length of the polyline through every `stride`-th point.
fn stride_length(pts: &[P2], stride: usize) -> f64 {
    pts.iter()
        .step_by(stride)
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| w[0].dist(*w[1]))
        .sum()
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
        limit_h2(
            stride_length(&cut, 4),
            stride_length(&cut, 2),
            stride_length(&cut, 1),
        )
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
        cmp(
            "saddle_handedness_matches_declared_side",
            "s(270 deg) - s(90 deg) read from the pattern through the declared marking side against 2*r*cot(beta)",
            handed,
            handed_ref,
            // the two heights are read off a polyline: allow its representation estimate
            agree + 2.0 * p.eps_rep,
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
        cmp(
            "saddle_on_the_declared_delta_side",
            "height of the cut at the seam (phi = 0, positive-delta side) minus the height half a turn away, against the numerical intersection of the solid joint",
            delta_side,
            delta_side_ref,
            tol3 + 2.0 * p.eps_rep,
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
        limit_h2(
            stride_length(&pts, 4),
            stride_length(&pts, 2),
            stride_length(&pts, 1),
        )
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
            let area_of = |k: usize| -> f64 {
                let v: Vec<&(f64, f64)> = c.iter().step_by(k).collect();
                (0.5 * r_dev_header
                    * v.windows(2)
                        .map(|w| (w[0].0 + w[1].0) * (w[1].1 - w[0].1))
                        .sum::<f64>())
                .abs()
            };
            // the area of an inscribed polygon converges like h², as chord sums do
            let (a, ar) = limit_h2(area_of(4), area_of(2), area_of(1));
            (l, r, a, ar)
        }
        None => (f64::NAN, f64::NAN, f64::NAN, f64::NAN),
    };
    let shoelace = |stride: usize| -> f64 {
        let v: Vec<&P2> = pts.iter().step_by(stride).collect();
        (0.5 * v
            .windows(2)
            .map(|w| w[0].x * w[1].y - w[1].x * w[0].y)
            .sum::<f64>())
        .abs()
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
            agree * (TAU * r_dev_header + length),
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
        cmp(
            "branch_cut_lies_on_header_opening",
            "3D distance from each folded-back branch cut point to the folded-back header opening curve",
            one_way(&cut3, &open3),
            0.0,
            agree + rep(header, branch),
        ),
        cmp(
            "header_opening_lies_on_branch_cut",
            "3D distance from each folded-back opening point to the folded-back branch cut curve",
            one_way(&open3, &cut3),
            0.0,
            agree + rep(header, branch),
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
        all_pass(&verify_eccentric(
            &eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap(),
            200.0,
            100.0,
            300.0,
            100.0,
        ));
        all_pass(&verify_eccentric(
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
        assert!(v.iter().all(|c| c.verdict == Verdict::Fail), "{v:?}");
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
        // reference case CYL-02 (r 500, L 1000): thresholds exactly as before
        let before = [1e-6, 1e-6, 1e-6, 1e-6 * 1500.0];
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
}
