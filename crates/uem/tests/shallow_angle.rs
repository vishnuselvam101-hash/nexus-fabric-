//! Review B2-005 (engine 0.4.2): a branch stub at 1 and 179 degrees is accepted, so its numerical error
//! and its chord error are MEASURED here against a reference of about 31 digits (double-double arithmetic,
//! written for this test and independent of the engine's formulas: it evaluates the saddle height
//! s(phi) = (sqrt(R^2 - (delta + r cos phi)^2) - r sin phi cos beta) / sin beta in about 106-bit arithmetic with
//! beta = beta_deg * pi / 180 taken exactly from the binary64 input).
//!
//! Asserted: the largest distance of a vertex of the saddle cut from the exact (ideal-parameter) point of the
//! developed curve is at most the pattern's `eps_num`; the true chord error (exact curve to the polyline,
//! sampled inside every segment) is at most the 0.01 mm chord limit. Both numbers are printed
//! (`cargo test -p uem --test shallow_angle -- --nocapture`).

use uem::Geom;
use uem::develop::{MAX_CHORD_ERROR_MM, branch_stub};

#[derive(Clone, Copy, Debug)]
struct Dd {
    hi: f64,
    lo: f64,
}

fn two_sum(a: f64, b: f64) -> Dd {
    let s = a + b;
    let bb = s - a;
    Dd {
        hi: s,
        lo: (a - (s - bb)) + (b - bb),
    }
}
fn quick(a: f64, b: f64) -> Dd {
    let s = a + b;
    Dd {
        hi: s,
        lo: b - (s - a),
    }
}
impl Dd {
    fn new(x: f64) -> Dd {
        Dd { hi: x, lo: 0.0 }
    }
    fn add(self, o: Dd) -> Dd {
        let s = two_sum(self.hi, o.hi);
        let t = two_sum(self.lo, o.lo);
        let u = quick(s.hi, s.lo + t.hi);
        quick(u.hi, u.lo + t.lo)
    }
    fn neg(self) -> Dd {
        Dd {
            hi: -self.hi,
            lo: -self.lo,
        }
    }
    fn sub(self, o: Dd) -> Dd {
        self.add(o.neg())
    }
    fn mul(self, o: Dd) -> Dd {
        let p = self.hi * o.hi;
        let e = self.hi.mul_add(o.hi, -p) + (self.hi * o.lo + self.lo * o.hi);
        quick(p, e)
    }
    fn div(self, o: Dd) -> Dd {
        let q1 = self.hi / o.hi;
        let r = self.sub(o.mul(Dd::new(q1)));
        let q2 = r.hi / o.hi;
        let r = r.sub(o.mul(Dd::new(q2)));
        let q3 = r.hi / o.hi;
        quick(q1, q2).add(Dd::new(q3))
    }
    fn sqrt(self) -> Dd {
        let x = 1.0 / self.hi.sqrt();
        let ax = self.hi * x;
        let d = self.sub(Dd::new(ax).mul(Dd::new(ax)));
        two_sum(ax, d.hi * (x * 0.5))
    }
    fn f(self) -> f64 {
        self.hi + self.lo
    }
}

fn pi() -> Dd {
    Dd {
        hi: std::f64::consts::PI,
        lo: 1.2246467991473532e-16,
    }
}

/// sin and cos of x in [-pi, pi] by Taylor series (term count fixed; the last term is below 1e-40).
fn sin_cos(x: Dd) -> (Dd, Dd) {
    let x2 = x.mul(x);
    let (mut s, mut c) = (Dd::new(0.0), Dd::new(1.0));
    let mut ts = x; // x^(2k+1)/(2k+1)!
    let mut tc = Dd::new(1.0);
    s = s.add(ts);
    for k in 1..=40 {
        let kk = k as f64;
        tc = tc.mul(x2).div(Dd::new((2.0 * kk - 1.0) * (2.0 * kk))).neg();
        c = c.add(tc);
        ts = ts.mul(x2).div(Dd::new((2.0 * kk) * (2.0 * kk + 1.0))).neg();
        s = s.add(ts);
    }
    (s, c)
}

/// Exact developed saddle point (u, s) at the ideal parameter phi = 2 pi num / den.
fn exact_point(
    big_r: f64,
    r: f64,
    r_dev: f64,
    beta_deg: f64,
    delta: f64,
    num: usize,
    den: usize,
) -> (Dd, Dd) {
    let two_pi = pi().add(pi());
    let phi = two_pi.mul(Dd::new(num as f64)).div(Dd::new(den as f64));
    // reduce to [-pi, pi] for the series
    let phi_red = if phi.f() > pi().f() {
        phi.sub(two_pi)
    } else {
        phi
    };
    let (sp, cp) = sin_cos(phi_red);
    let beta = Dd::new(beta_deg).mul(pi()).div(Dd::new(180.0));
    let (sb, cb) = sin_cos(beta);
    let y = Dd::new(delta).add(Dd::new(r).mul(cp));
    let d = Dd::new(big_r).mul(Dd::new(big_r)).sub(y.mul(y));
    let s = d.sqrt().sub(Dd::new(r).mul(sp).mul(cb)).div(sb);
    (Dd::new(r_dev).mul(phi), s)
}

fn dist_point_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// Returns (vertex error, chord error, eps_num, eps_rep, segments).
fn measure(beta_deg: f64) -> (f64, f64, f64, f64, usize) {
    measure_s(beta_deg, 60_000.0)
}

fn measure_s(beta_deg: f64, s_end: f64) -> (f64, f64, f64, f64, usize) {
    let (big_r, r, r_dev, delta) = (150.0, 100.0, 100.0, 0.0);
    let p = branch_stub(big_r, r, r_dev, beta_deg, delta, s_end).expect("accepted");
    let curve = p
        .outline
        .iter()
        .find(|c| c.name == "saddle_cut")
        .expect("saddle_cut");
    let (pts, eps_rep) = match &curve.geom {
        Geom::Polyline { pts, eps_rep } => (pts.clone(), *eps_rep),
        _ => panic!("saddle_cut is a polyline"),
    };
    let n = pts.len() - 1;
    let mut vertex = 0.0f64;
    for (i, q) in pts.iter().enumerate() {
        // the closing vertex sits at u = 2 pi r_dev with the height of phi = 0, which is the exact point at i = n
        let (u, s) = exact_point(big_r, r, r_dev, beta_deg, delta, i, n);
        // difference taken in double-double before rounding, so that the error is not below the resolution of f64
        let du = Dd::new(q.x).sub(u).f();
        let ds = Dd::new(q.y).sub(s).f();
        vertex = vertex.max(du.hypot(ds));
    }
    // chord error: exact points inside every segment against the chord of the pattern's own vertices
    let m = 20usize;
    let mut chord = 0.0f64;
    for i in 0..n {
        for k in 1..m {
            let (u, s) = exact_point(big_r, r, r_dev, beta_deg, delta, i * m + k, n * m);
            let d = dist_point_segment(
                (u.f(), s.f()),
                (pts[i].x, pts[i].y),
                (pts[i + 1].x, pts[i + 1].y),
            );
            chord = chord.max(d);
        }
    }
    (vertex, chord, p.eps_num, eps_rep, n)
}

#[test]
fn shallow_and_obtuse_branch_angles_are_within_eps_num_and_the_chord_limit() {
    for beta_deg in [1.0, 179.0] {
        let (vertex, chord, eps_num, eps_rep, n) = measure(beta_deg);
        println!(
            "beta = {beta_deg} deg: {n} segments; vertex error {vertex:.3e} mm vs eps_num {eps_num:.3e} mm (ratio {:.3}); chord error {chord:.4e} mm vs eps_rep {eps_rep:.4e} mm, limit {MAX_CHORD_ERROR_MM} mm",
            vertex / eps_num
        );
        assert!(
            vertex <= eps_num,
            "beta {beta_deg}: vertex error {vertex:e} exceeds eps_num {eps_num:e}"
        );
        assert!(
            chord <= MAX_CHORD_ERROR_MM,
            "beta {beta_deg}: chord error {chord:e} exceeds the {MAX_CHORD_ERROR_MM} mm limit"
        );
        // eps_rep is an estimate (15 interior points); the measured value must not be far above it
        assert!(
            chord <= eps_rep * 1.10 + 1e-9,
            "beta {beta_deg}: chord error {chord:e} vs eps_rep {eps_rep:e}"
        );
    }
}

/// Down to the developed-size limit (beta = 0.000144686 deg: s_max = 9.9e7 mm) the pattern itself stays within
/// `eps_num` of the exact curve and within the chord limit. (The product's own independent verification, a
/// numerically intersected 3D curve conditioned by 1 / sin beta, stops confirming it below about 0.003 deg and then
/// REJECTS the result, which is fail-closed; that is a limit of the verifier, not of the pattern.)
#[test]
fn geometry_stays_within_eps_num_down_to_the_size_limit() {
    for (b, s_end) in [(0.000144686, 99.5e6), (0.001, 2e7), (0.01, 2e6), (0.1, 2e5)] {
        let (vertex, chord, eps_num, eps_rep, n) = measure_s(b, s_end);
        println!(
            "beta = {b} deg: {n} segments; vertex error {vertex:.3e} mm vs eps_num {eps_num:.3e} mm; chord error {chord:.4e} mm vs eps_rep {eps_rep:.4e} mm"
        );
        assert!(
            vertex <= eps_num,
            "beta {b}: vertex {vertex:e} > eps_num {eps_num:e}"
        );
        assert!(chord <= MAX_CHORD_ERROR_MM, "beta {b}: chord {chord:e}");
    }
}
