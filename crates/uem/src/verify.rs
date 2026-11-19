//! Independent verification checks (Spec 02 §9). Each check uses a formulation
//! different from the one in `develop` and consumes only the original inputs
//! and the primary *result* (ARCH-03 / MATH-40). In this build the checks live
//! in the same crate as the primary code; a separately owned implementation
//! (class I-I/I-L) does not exist yet and the evidence says so.

use crate::develop::{Branch, ObliqueCone};
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
            profile::check(&p.outline, 1e-9).signed_area.signum(),
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

/// Header shell with opening: fold the opening back onto the header surface
/// and test it against the *branch* cylinder's implicit equation.
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
    ]
}

fn seg_dist(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if l2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let c = [
        a[0] + t * ab[0] - p[0],
        a[1] + t * ab[1] - p[1],
        a[2] + t * ab[2] - p[2],
    ];
    (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
}

/// Interface constraint for a set-on branch: the branch end cut and the header
/// opening, each folded back to 3D from its *own* pattern with its own part's
/// parameters, must be the same curve. `hb` / `bb` are the branch geometries
/// as used by the header pattern and by the branch pattern respectively — if
/// the two parts were made from different interpretations, they differ.
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
    let one_way = |from: &[[f64; 3]], to: &[[f64; 3]]| -> f64 {
        if from.is_empty() || to.len() < 2 {
            return f64::NAN;
        }
        from.iter()
            .map(|p| {
                to.windows(2)
                    .map(|w| seg_dist(*p, w[0], w[1]))
                    .fold(f64::MAX, f64::min)
            })
            .fold(0.0, f64::max)
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
            agree,
        ),
        cmp(
            "header_opening_lies_on_branch_cut",
            "3D distance from each folded-back opening point to the folded-back branch cut curve",
            one_way(&open3, &cut3),
            0.0,
            agree,
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
