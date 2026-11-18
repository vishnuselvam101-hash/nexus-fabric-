//! Independent verification checks (Spec 02 §9). Each check uses a formulation
//! different from the one in `develop` and consumes only the original inputs
//! and the primary *result* (ARCH-03 / MATH-40). In this build the checks live
//! in the same crate as the primary code; a separately owned implementation
//! (class I-I/I-L) does not exist yet and the evidence says so.

use crate::develop::{Branch, ObliqueCone};
use crate::{gauss, profile, Geom, Pattern, P2, TAU};

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

fn cmp(name: &'static str, method: &'static str, primary: f64, reference: f64, tol: f64) -> Check {
    let d = (primary - reference).abs();
    let verdict = if !d.is_finite() {
        Verdict::Inconclusive
    } else if d <= tol {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    Check { name, method, independence: "I-F", primary, reference, difference: d, tolerance: tol, verdict }
}

fn curve_len(p: &Pattern, name: &str) -> f64 {
    p.outline.iter().filter(|c| c.name == name).map(profile::length).sum()
}

pub fn verify_cylinder(p: &Pattern, r_dev: f64, length: f64) -> Vec<Check> {
    vec![
        cmp("bottom_edge_equals_circumference", "circumference 2*pi*r from the input radius", curve_len(p, "bottom"), TAU * r_dev, AGREE_ABS),
        cmp("seam_edges_equal_length", "length of the opposite seam edge", curve_len(p, "seam_a"), curve_len(p, "seam_b"), AGREE_ABS),
        cmp("seam_length_equals_input", "input length", curve_len(p, "seam_a"), length, AGREE_ABS),
        cmp("area", "lateral area 2*pi*r*L", profile::check(&p.outline, 1e-9).signed_area, TAU * r_dev * length, AGREE_ABS * (r_dev + length)),
    ]
}

pub fn verify_cone(p: &Pattern, r1: f64, r2: f64, h: f64) -> Vec<Check> {
    let s = h.hypot(r1 - r2);
    let mut v = vec![
        cmp("large_arc_equals_circumference", "3D end-circle circumference 2*pi*r1", curve_len(p, "large_end"), TAU * r1, AGREE_ABS),
        cmp("small_arc_equals_circumference", "3D end-circle circumference 2*pi*r2", curve_len(p, "small_end"), TAU * r2, AGREE_ABS),
        cmp("seam_edges_equal_length", "length of the opposite seam edge", curve_len(p, "seam_a"), curve_len(p, "seam_b"), AGREE_ABS),
        cmp("seam_equals_slant_length", "slant length from Pythagoras on the axial section", curve_len(p, "seam_a"), s, AGREE_ABS),
        cmp("area", "lateral area pi*(r1+r2)*s of the 3D frustum", p.quantity("area").unwrap_or(f64::NAN), std::f64::consts::PI * (r1 + r2) * s, AGREE_ABS * s),
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
    v.push(cmp("fold_back_radius_residual", "implicit taper equation of the frustum evaluated at folded-back arcs", worst, 0.0, AGREE_ABS));
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
        let nrm = [c1[1] * d[2] - c1[2] * d[1], c1[2] * d[0] - c1[0] * d[2], c1[0] * d[1] - c1[1] * d[0]];
        let nn = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        let cr = [c1[1] * c2[2] - c1[2] * c2[1], c1[2] * c2[0] - c1[0] * c2[2], c1[0] * c2[1] - c1[1] * c2[0]];
        (cr[0] * nrm[0] + cr[1] * nrm[1] + cr[2] * nrm[2]) / nn / (r1 * r1)
    };
    let f = |t: f64, y: [f64; 3]| -> [f64; 3] { [kgv(t), r1 * y[0].cos(), r1 * y[0].sin()] };
    let hstep = TAU / n as f64;
    let mut y = [0.0f64; 3];
    let mut out = vec![P2::new(0.0, 0.0)];
    for i in 0..n {
        let t = i as f64 * hstep;
        let add = |y: [f64; 3], k: [f64; 3], s: f64| [y[0] + s * k[0], y[1] + s * k[1], y[2] + s * k[2]];
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
    let g = ObliqueCone { r1, r2, h, e };
    let outer: Vec<P2> = p
        .outline
        .iter()
        .find(|c| c.name == "large_end")
        .and_then(|c| if let Geom::Polyline { pts, .. } = &c.geom { Some(pts.clone()) } else { None })
        .unwrap_or_default();
    let n = outer.len().saturating_sub(1);
    let mut v = Vec::new();
    if n == 0 {
        v.push(cmp("large_end_present", "pattern structure", f64::NAN, 0.0, 0.0));
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
    let tol = AGREE_ABS.max(p.eps_num + 10.0 * ver_err);
    v.push(cmp("apex_free_development_agrees", "geodesic-curvature integration of the large end circle (no apex), RK4", worst, 0.0, tol));
    // (2) Developed edge lengths equal the 3D circumferences (isometry invariant).
    let chord_bias = |r: f64| TAU * r * (std::f64::consts::PI / n as f64).powi(2) / 6.0 * 1.5 + AGREE_ABS;
    v.push(cmp("large_edge_equals_circumference", "3D circumference 2*pi*r1 (polyline chord shortening allowed for)", curve_len(p, "large_end"), TAU * r1, chord_bias(r1) + p.eps_rep));
    v.push(cmp("small_edge_equals_circumference", "3D circumference 2*pi*r2 (polyline chord shortening allowed for)", curve_len(p, "small_end"), TAU * r2, chord_bias(r2) + p.eps_rep));
    // (3) Seam edges equal, and equal to the 3D ruling length at theta = 0.
    let ruling0 = ((e + r2 - r1).powi(2) + h * h).sqrt();
    v.push(cmp("seam_edges_equal_length", "length of the opposite seam edge", curve_len(p, "seam_a"), curve_len(p, "seam_b"), AGREE_ABS));
    v.push(cmp("seam_equals_3d_ruling", "distance between the 3D end-circle points at theta = 0", curve_len(p, "seam_a"), ruling0, AGREE_ABS));
    // (4) Area: polar formula (primary) versus 3D surface integral of the ruled surface.
    let surf = {
        let inner = |t: f64| -> f64 {
            let d = [e + (r2 - r1) * t.cos(), (r2 - r1) * t.sin(), h];
            let f = |l: f64| -> f64 {
                let rr = (1.0 - l) * r1 + l * r2;
                let dt = [-rr * t.sin(), rr * t.cos(), 0.0];
                let c = [dt[1] * d[2] - dt[2] * d[1], dt[2] * d[0] - dt[0] * d[2], dt[0] * d[1] - dt[1] * d[0]];
                (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
            };
            gauss(&f, 0.0, 1.0, 4).0
        };
        gauss(&inner, 0.0, TAU, 360).0
    };
    v.push(cmp("area", "surface integral of |S_theta x S_lambda| over the 3D ruled surface", p.quantity("area").unwrap_or(f64::NAN), surf, 1e-6 * surf.max(1.0)));
    v
}

pub fn verify_branch(p: &Pattern, b: &Branch, r_dev: f64, s_end: f64) -> Vec<Check> {
    let cut: Vec<P2> = p
        .outline
        .iter()
        .find(|c| c.name == "saddle_cut")
        .and_then(|c| if let Geom::Polyline { pts, .. } = &c.geom { Some(pts.clone()) } else { None })
        .unwrap_or_default();
    let (sb, cb) = b.beta.sin_cos();
    let d = [cb, 0.0, sb];
    let (mut header_res, mut branch_res): (f64, f64) = (0.0, 0.0);
    for q in &cut {
        // fold the pattern point back to 3D, then test the two implicit cylinder equations
        let phi = q.x / r_dev;
        let s = s_end - q.y;
        let pt = b.point(phi, s);
        header_res = header_res.max(((pt[1] * pt[1] + pt[2] * pt[2]).sqrt() - b.big_r).abs());
        let w = [pt[0], pt[1] - b.delta, pt[2]];
        let c = [w[1] * d[2] - w[2] * d[1], w[2] * d[0] - w[0] * d[2], w[0] * d[1] - w[1] * d[0]];
        branch_res = branch_res.max(((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() - b.r).abs());
    }
    let v = vec![
        cmp("cut_points_on_header_surface", "fold-back, then distance from the header axis (implicit equation)", header_res, 0.0, AGREE_ABS),
        cmp("cut_points_on_branch_surface", "fold-back, then |(P-O) x d| distance from the branch axis", branch_res, 0.0, AGREE_ABS),
        cmp("square_end_equals_circumference", "circumference 2*pi*r_dev", curve_len(p, "square_end"), TAU * r_dev, AGREE_ABS),
        cmp("seam_edges_equal_length", "length of the opposite seam edge", curve_len(p, "seam_a"), curve_len(p, "seam_b"), AGREE_ABS),
    ];
    v
}

fn hole_points(p: &Pattern) -> Vec<P2> {
    p.holes.first().and_then(|c| if let Geom::Polyline { pts, .. } = &c.geom { Some(pts.clone()) } else { None }).unwrap_or_default()
}

fn opening_3d(q: &P2, big_r: f64, r_dev_header: f64, station: f64) -> [f64; 3] {
    let omega = q.y / r_dev_header - std::f64::consts::PI;
    [q.x - station, big_r * omega.sin(), big_r * omega.cos()]
}

/// Header shell with opening: fold the opening back onto the header surface
/// and test it against the *branch* cylinder's implicit equation.
pub fn verify_header_opening(p: &Pattern, b: &Branch, r_dev_header: f64, length: f64, station: f64) -> Vec<Check> {
    let (sb, cb) = b.beta.sin_cos();
    let d = [cb, 0.0, sb];
    let mut branch_res: f64 = 0.0;
    let pts = hole_points(p);
    for q in &pts {
        let pt = opening_3d(q, b.big_r, r_dev_header, station);
        let w = [pt[0], pt[1] - b.delta, pt[2]];
        let c = [w[1] * d[2] - w[2] * d[1], w[2] * d[0] - w[0] * d[2], w[0] * d[1] - w[1] * d[0]];
        branch_res = branch_res.max(((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() - b.r).abs());
    }
    let hole = profile::check(&p.holes, 1e-9);
    let inside = pts.iter().all(|q| q.x > 0.0 && q.x < length && q.y > 0.0 && q.y < TAU * r_dev_header);
    vec![
        cmp("opening_points_on_branch_surface", "fold-back onto the header, then |(P-O) x d| distance from the branch axis", if pts.is_empty() { f64::NAN } else { branch_res }, 0.0, AGREE_ABS),
        cmp("shell_edge_equals_circumference", "circumference 2*pi*r from the input radius", curve_len(p, "bottom"), TAU * r_dev_header, AGREE_ABS),
        cmp("seam_edges_equal_length", "length of the opposite seam edge", curve_len(p, "seam_a"), curve_len(p, "seam_b"), AGREE_ABS),
        cmp("opening_closed_simple_inside_shell", "profile test of the opening and bounds test against the shell rectangle", if hole.closed && hole.self_intersections == 0 && inside { 0.0 } else { 1.0 }, 0.0, 0.5),
    ]
}

fn seg_dist(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if l2 > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let c = [a[0] + t * ab[0] - p[0], a[1] + t * ab[1] - p[1], a[2] + t * ab[2] - p[2]];
    (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
}

/// Interface constraint for a set-on branch: the branch end cut and the header
/// opening, each folded back to 3D from its *own* pattern with its own part's
/// parameters, must be the same curve. `hb` / `bb` are the branch geometries
/// as used by the header pattern and by the branch pattern respectively — if
/// the two parts were made from different interpretations, they differ.
pub fn verify_interface(header: &Pattern, hb: &Branch, r_dev_header: f64, station: f64, branch: &Pattern, bb: &Branch, r_dev_branch: f64, s_end: f64) -> Vec<Check> {
    let open3: Vec<[f64; 3]> = hole_points(header).iter().map(|q| opening_3d(q, hb.big_r, r_dev_header, station)).collect();
    let cut: Vec<P2> = branch.outline.iter().find(|c| c.name == "saddle_cut").and_then(|c| if let Geom::Polyline { pts, .. } = &c.geom { Some(pts.clone()) } else { None }).unwrap_or_default();
    let cut3: Vec<[f64; 3]> = cut.iter().map(|q| bb.point(q.x / r_dev_branch, s_end - q.y)).collect();
    let one_way = |from: &[[f64; 3]], to: &[[f64; 3]]| -> f64 {
        if from.is_empty() || to.len() < 2 {
            return f64::NAN;
        }
        from.iter().map(|p| to.windows(2).map(|w| seg_dist(*p, w[0], w[1])).fold(f64::MAX, f64::min)).fold(0.0, f64::max)
    };
    vec![
        cmp("branch_cut_lies_on_header_opening", "3D distance from each folded-back branch cut point to the folded-back header opening curve", one_way(&cut3, &open3), 0.0, AGREE_ABS),
        cmp("header_opening_lies_on_branch_cut", "3D distance from each folded-back opening point to the folded-back branch cut curve", one_way(&open3, &cut3), 0.0, AGREE_ABS),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::*;

    fn all_pass(v: &[Check]) {
        for c in v {
            assert_eq!(c.verdict, Verdict::Pass, "{} diff {} tol {}", c.name, c.difference, c.tolerance);
        }
    }

    #[test]
    fn verifiers_pass_on_correct_patterns() {
        all_pass(&verify_cylinder(&cylinder_shell(100.0, 50.0).unwrap(), 100.0, 50.0));
        all_pass(&verify_cone(&cone_frustum(200.0, 100.0, 300.0).unwrap(), 200.0, 100.0, 300.0));
        all_pass(&verify_eccentric(&eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap(), 200.0, 100.0, 300.0, 100.0));
        all_pass(&verify_eccentric(&eccentric_reducer(350.0, 120.0, 260.0, 40.0).unwrap(), 350.0, 120.0, 260.0, 40.0));
        for (rr, r, beta, delta) in [(150.0, 100.0, 90.0, 0.0), (150.0, 100.0, 60.0, 0.0), (150.0, 80.0, 90.0, 40.0), (300.0, 60.0, 35.0, -90.0)] {
            let p = branch_stub(rr, r, r, beta, delta, 900.0).unwrap();
            all_pass(&verify_branch(&p, &Branch { big_r: rr, r, beta: f64::to_radians(beta), delta }, r, 900.0));
        }
    }

    #[test]
    fn header_opening_and_interface() {
        let b = Branch { big_r: 150.0, r: 100.0, beta: 60f64.to_radians(), delta: 20.0 };
        let hp = header_with_opening(&b, 150.0, 1200.0, 600.0).unwrap();
        let bp = branch_stub(150.0, 100.0, 100.0, 60.0, 20.0, 400.0).unwrap();
        all_pass(&verify_header_opening(&hp, &b, 150.0, 1200.0, 600.0));
        all_pass(&verify_interface(&hp, &b, 150.0, 600.0, &bp, &b, 100.0, 400.0));
        // seeded fault: branch made against the header's inside surface, header opened for the outside surface
        let wrong = Branch { big_r: 142.0, ..b };
        let bp_wrong = branch_stub(142.0, 100.0, 100.0, 60.0, 20.0, 400.0).unwrap();
        assert!(verify_interface(&hp, &b, 150.0, 600.0, &bp_wrong, &wrong, 100.0, 400.0).iter().any(|c| c.verdict == Verdict::Fail));
        // seeded fault: branch angle differs between the two parts
        let tilted = Branch { beta: 55f64.to_radians(), ..b };
        let bp_tilt = branch_stub(150.0, 100.0, 100.0, 55.0, 20.0, 400.0).unwrap();
        assert!(verify_interface(&hp, &b, 150.0, 600.0, &bp_tilt, &tilted, 100.0, 400.0).iter().any(|c| c.verdict == Verdict::Fail));
    }

    // Seeded faults (Spec 02 §12 group 13): each must be caught by a verifier.
    #[test]
    fn seeded_fault_wrong_radius_reference() {
        // pattern made for a radius differing by a wall thickness, verified against the intended one
        let p = cone_frustum(206.0, 100.0, 300.0).unwrap();
        assert!(verify_cone(&p, 200.0, 100.0, 300.0).iter().any(|c| c.verdict == Verdict::Fail));
        let p = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        assert!(verify_eccentric(&p, 200.0, 106.0, 300.0, 100.0).iter().any(|c| c.verdict == Verdict::Fail));
    }

    #[test]
    fn seeded_fault_wrong_offset_and_header() {
        let p = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        assert!(verify_eccentric(&p, 200.0, 100.0, 300.0, 60.0).iter().any(|c| c.verdict == Verdict::Fail));
        // branch cut made for header OD, checked against header ID (inside/outside confusion)
        let p = branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap();
        let wrong = Branch { big_r: 142.0, r: 100.0, beta: 60f64.to_radians(), delta: 0.0 };
        assert!(verify_branch(&p, &wrong, 100.0, 400.0).iter().any(|c| c.verdict == Verdict::Fail));
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
        assert!(verify_eccentric(&p, 200.0, 100.0, 300.0, 100.0).iter().any(|c| c.verdict == Verdict::Fail));
    }
}
