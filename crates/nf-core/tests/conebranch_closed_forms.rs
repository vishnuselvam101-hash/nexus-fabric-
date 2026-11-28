//! NFJ-1 S1-L9a: independent checks of `nf_core::conebranch` (branch on a cone). Nothing here calls ZK or a value that
//! ZK printed; every reference is derived in this file from the definition of the surfaces:
//!   cone       : points whose distance from C(x) = cs x (cos f, sin f) is Ra + k x - d (section circles, axis x)
//!   branch     : the cylinder of radius rho round the line through (x0, e v) with direction u = (cos dir, sin dir)
//! so the contact of a branch wall line with the cone is found by BISECTION on the signed distance (not by the closed
//! form), the hole is found by scanning the plate band along the surface normal taken from a finite-difference cross
//! product (not from the analytic normal), the development is checked for isometry against 3D chords, and the limit of a
//! vanishing half angle is checked against the existing tee closed forms (`nf_core::tee`).

use nf_core::conebranch::{
    self, CB_DEFAULT_SPLIT, CONEBRANCH_NAMED_RULES, CONEBRANCH_REFUSAL_CODES, ConeBranchResult,
    ConeBranchSpec, ThicknessConvention, probe,
};
use nf_core::tee::{self, BranchOnMain, BranchPieceSpec, Mount, TeeMainSpec};
use std::f64::consts::PI;

#[allow(clippy::too_many_arguments)]
fn sp(
    ecc: bool,
    da: f64,
    db: f64,
    l: f64,
    flat: f64,
    d: f64,
    x: f64,
    dir: f64,
    e: f64,
    h: f64,
    n: u32,
    m: Mount,
) -> ConeBranchSpec {
    ConeBranchSpec {
        da,
        db,
        l,
        t: 12.0,
        ecc,
        flat_deg: if ecc { Some(flat) } else { None },
        d,
        tb: 9.0,
        x: Some(x),
        dir_deg: dir,
        e,
        h,
        angle_deg: None,
        mount: m,
        split: Some(n),
        seam_gap: None,
    }
}

trait TapTb {
    fn tap_tb(self, tb: f64) -> Self;
}
impl TapTb for ConeBranchSpec {
    fn tap_tb(mut self, tb: f64) -> Self {
        self.tb = tb;
        self
    }
}

fn examples() -> Vec<ConeBranchSpec> {
    vec![
        sp(
            false,
            1016.0,
            812.8,
            812.8,
            0.0,
            406.4,
            406.4,
            0.0,
            0.0,
            800.0,
            2,
            Mount::SetOn,
        ),
        sp(
            false,
            914.4,
            1219.2,
            600.0,
            0.0,
            508.0,
            300.0,
            90.0,
            100.0,
            900.0,
            3,
            Mount::SetIn,
        ),
        sp(
            true,
            1219.2,
            914.4,
            1200.0,
            0.0,
            406.4,
            600.0,
            0.0,
            0.0,
            900.0,
            2,
            Mount::SetOn,
        ),
        sp(
            true,
            1219.2,
            914.4,
            1200.0,
            0.0,
            406.4,
            600.0,
            90.0,
            0.0,
            900.0,
            2,
            Mount::SetOn,
        ),
    ]
}

/// The earlier tests check the ZK-parity (radial) rule against the physical model written for it; the normal default
/// has its own tests below.
fn cb(s: &ConeBranchSpec) -> Result<ConeBranchResult, nf_core::shapes::ShapeRefusal> {
    conebranch::cone_branch_with(s, ThicknessConvention::Radial)
}

fn run(s: &ConeBranchSpec) -> ConeBranchResult {
    cb(s).unwrap_or_else(|e| panic!("{s:?}: {} {}", e.code, e.message))
}

fn code(s: &ConeBranchSpec) -> &'static str {
    cb(s).expect_err("expected a refusal").code
}

// ------------------------------------------------------------------------------------ physical model (Da frame)

/// The physical solid, written from the definition and seen from the Da mouth (x = 0). No internal flip: a cone written
/// from its smaller mouth is simply `cs < 0`.
struct World {
    ra: f64,
    k: f64,
    cs: f64,
    f: f64,
    t: f64,
    x0: f64,
    e: f64,
    u: [f64; 2],
    v: [f64; 2],
    ri: f64,
    ro: f64,
}

impl World {
    fn of(s: &ConeBranchSpec) -> World {
        let (ra, rb) = (s.da / 2.0, s.db / 2.0);
        let dir = s.dir_deg.to_radians();
        World {
            ra,
            k: (rb - ra) / s.l,
            cs: if s.ecc { (ra - rb) / s.l } else { 0.0 },
            f: if s.ecc {
                s.flat_deg.unwrap_or(0.0).to_radians()
            } else {
                0.0
            },
            t: s.t,
            x0: s.x.unwrap(),
            e: s.e,
            u: [dir.cos(), dir.sin()],
            v: [-dir.sin(), dir.cos()],
            ri: s.d / 2.0 - s.tb,
            ro: s.d / 2.0,
        }
    }
    fn c(&self, x: f64) -> [f64; 2] {
        [self.cs * x * self.f.cos(), self.cs * x * self.f.sin()]
    }
    fn rad(&self, x: f64, d: f64) -> f64 {
        self.ra + self.k * x - d
    }
    /// Distance along `u` at which the line (x, lateral l) leaves the cone surface at radial depth d (bisection on
    /// the signed distance; the foot point of the section centre is inside the section).
    fn contact(&self, x: f64, l: f64, d: f64) -> Option<f64> {
        let c = self.c(x);
        let (cu, cv) = (
            c[0] * self.u[0] + c[1] * self.u[1],
            c[0] * self.v[0] + c[1] * self.v[1],
        );
        let r = self.rad(x, d);
        if (l - cv).abs() > r {
            return None;
        }
        let g = |t: f64| {
            let p = [t * self.u[0] + l * self.v[0], t * self.u[1] + l * self.v[1]];
            (p[0] - c[0]).hypot(p[1] - c[1]) - r
        };
        let (mut a, mut b) = (cu, cu + r + 1.0);
        for _ in 0..70 {
            let m = 0.5 * (a + b);
            if g(m) < 0.0 {
                a = m;
            } else {
                b = m;
            }
        }
        Some(0.5 * (a + b))
    }
    /// Highest contact over the branch wall at angle th (rho sampled finely).
    fn cut_brute(&self, th: f64, d: f64) -> f64 {
        self.cut_brute_n(th, d, 2000)
    }
    fn cut_brute_n(&self, th: f64, d: f64, n: i32) -> f64 {
        (0..=n)
            .filter_map(|i| {
                let rho = self.ri + (self.ro - self.ri) * f64::from(i) / f64::from(n);
                self.contact(self.x0 + rho * th.cos(), self.e + rho * th.sin(), d)
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }
    /// Mid-surface point and the outward unit normal (cross product of finite-difference tangents).
    fn mid(&self, x: f64, a: f64) -> ([f64; 3], [f64; 3]) {
        let p = |x: f64, a: f64| {
            let c = self.c(x);
            let r = self.rad(x, self.t / 2.0);
            [x, c[0] + r * a.cos(), c[1] + r * a.sin()]
        };
        let (hx, ha) = (1e-5, 1e-7);
        let (xp, xm) = (p(x + hx, a), p(x - hx, a));
        let (ap, am) = (p(x, a + ha), p(x, a - ha));
        let tx = [
            (xp[0] - xm[0]) / (2.0 * hx),
            (xp[1] - xm[1]) / (2.0 * hx),
            (xp[2] - xm[2]) / (2.0 * hx),
        ];
        let ta = [
            (ap[0] - am[0]) / (2.0 * ha),
            (ap[1] - am[1]) / (2.0 * ha),
            (ap[2] - am[2]) / (2.0 * ha),
        ];
        let mut n = [
            tx[1] * ta[2] - tx[2] * ta[1],
            tx[2] * ta[0] - tx[0] * ta[2],
            tx[0] * ta[1] - tx[1] * ta[0],
        ];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        for q in &mut n {
            *q /= l;
        }
        if n[1] * a.cos() + n[2] * a.sin() < 0.0 {
            for q in &mut n {
                *q = -*q;
            }
        }
        (p(x, a), n)
    }
    /// Smallest squared distance from the branch axis over the plate band along the normal at (x, a).
    fn band_min(&self, x: f64, a: f64) -> f64 {
        let (m, n) = self.mid(x, a);
        let lat = |p: [f64; 3]| p[1] * self.v[0] + p[2] * self.v[1];
        let (dx, dl) = (m[0] - self.x0, lat(m) - self.e);
        let (nx, nl) = (n[0], n[1] * self.v[0] + n[2] * self.v[1]);
        let (aa, bb) = (nx * nx + nl * nl, 2.0 * (dx * nx + dl * nl));
        let w = if aa > 1e-18 {
            (-bb / (2.0 * aa)).clamp(-self.t / 2.0, self.t / 2.0)
        } else {
            0.0
        };
        let (px, pl) = (dx + w * nx, dl + w * nl);
        px * px + pl * pl
    }
    /// Axial hole interval at azimuth a by scanning x and refining the two boundaries by bisection.
    fn hole_brute(&self, a: f64, rh: f64) -> Option<[f64; 2]> {
        let (lo, hi) = (self.x0 - rh - 4.0 * self.t, self.x0 + rh + 4.0 * self.t);
        let n = 8000usize;
        let xs: Vec<f64> = (0..=n)
            .map(|i| lo + (hi - lo) * i as f64 / n as f64)
            .collect();
        let ins: Vec<bool> = xs.iter().map(|&x| self.band_min(x, a) < rh * rh).collect();
        let first = ins.iter().position(|&b| b)?;
        let last = ins.iter().rposition(|&b| b)?;
        let refine = |mut out: f64, mut inn: f64| {
            for _ in 0..60 {
                let m = 0.5 * (out + inn);
                if self.band_min(m, a) < rh * rh {
                    inn = m;
                } else {
                    out = m;
                }
            }
            inn
        };
        let xl = if first == 0 {
            xs[0]
        } else {
            refine(xs[first - 1], xs[first])
        };
        let xh = if last == n {
            xs[n]
        } else {
            refine(xs[last + 1], xs[last])
        };
        Some([xl, xh])
    }
}

fn is_flipped(s: &ConeBranchSpec) -> bool {
    s.ecc && s.da < s.db
}

// ------------------------------------------------------------------------------------ branch cut

#[test]
fn branch_marks_equal_bisection_of_the_physical_contact() {
    // examples 1-4, plus a set-in eccentric, an oblique azimuth, a negative offset and a cone written from its small mouth
    let mut v = examples();
    v.push(sp(
        true,
        1219.2,
        914.4,
        1200.0,
        30.0,
        406.4,
        500.0,
        100.0,
        -60.0,
        900.0,
        2,
        Mount::SetIn,
    ));
    v.push(sp(
        false,
        2000.0,
        1200.0,
        1500.0,
        0.0,
        600.0,
        750.0,
        45.0,
        250.0,
        3000.0,
        2,
        Mount::SetOn,
    ));
    v.push(sp(
        true,
        914.4,
        1219.2,
        1200.0,
        0.0,
        406.4,
        600.0,
        270.0,
        -152.4,
        900.0,
        2,
        Mount::SetOn,
    ));
    v.push(sp(
        true,
        914.4,
        1219.2,
        1200.0,
        200.0,
        406.4,
        700.0,
        20.0,
        80.0,
        900.0,
        2,
        Mount::SetIn,
    ));
    let mut worst = 0.0f64;
    for s in &v {
        let w = World::of(s);
        let d = if s.mount == Mount::SetIn { s.t } else { 0.0 };
        let r = run(s);
        for (k, m) in r.branch.marks.iter().enumerate() {
            // an eccentric cone written from the smaller mouth is mirrored inside: the angle round the branch is
            // turned by 180 deg (mark 4 faces the larger mouth, rule CONEBRANCH_BRANCH_SEAM_SIDE)
            let th = PI / 2.0 + 2.0 * PI * k as f64 / 16.0 + if is_flipped(s) { PI } else { 0.0 };
            let want = s.h - w.cut_brute(th, d);
            worst = worst.max((m.len - want).abs());
            assert!(
                (m.len - want).abs() < 2e-6,
                "{s:?} mark {k}: {} vs {want}",
                m.len
            );
        }
    }
    println!("max |closed form - bisection| over the marks: {worst:e} mm");
}

#[test]
fn closed_form_cut_is_never_below_the_sampled_wall_maximum() {
    for s in examples() {
        let w = World::of(&s);
        let d = if s.mount == Mount::SetIn { s.t } else { 0.0 };
        for k in 0..128 {
            let th = 2.0 * PI * f64::from(k) / 128.0 + 0.0123;
            let a = probe::cut_t(&s, th).unwrap();
            let b = w.cut_brute(th, d);
            assert!(a >= b - 1e-9 && a - b < 1e-5, "{th}: {a} vs {b}");
        }
    }
}

#[test]
fn tip_of_the_branch_is_the_longest_and_extremes_are_exact() {
    for s in examples() {
        let r = run(&s);
        let w = World::of(&s);
        let d = if s.mount == Mount::SetIn { s.t } else { 0.0 };
        let n = 1000;
        let lens: Vec<f64> = (0..n)
            .map(|i| s.h - w.cut_brute_n(PI / 2.0 + 2.0 * PI * f64::from(i) / f64::from(n), d, 600))
            .collect();
        let lo = lens.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = lens.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (r.branch.shortest - lo).abs() < 2e-3 && r.branch.shortest <= lo + 1e-9,
            "{s:?} {} {lo}",
            r.branch.shortest
        );
        assert!(
            (r.branch.longest - hi).abs() < 2e-3 && r.branch.longest >= hi - 1e-9,
            "{s:?} {} {hi}",
            r.branch.longest
        );
    }
}

// ------------------------------------------------------------------------------------ hole

#[test]
fn hole_interval_equals_scan_of_the_plate_band_along_the_normal() {
    // non flipped specs, flat side at azimuth 0 (internal azimuth = physical azimuth)
    let v = vec![
        examples()[0].clone(),
        examples()[1].clone(),
        examples()[2].clone(),
        examples()[3].clone(),
        sp(
            false,
            2000.0,
            1200.0,
            1500.0,
            0.0,
            600.0,
            750.0,
            45.0,
            250.0,
            3000.0,
            2,
            Mount::SetOn,
        ),
        sp(
            true,
            1219.2,
            914.4,
            1200.0,
            0.0,
            406.4,
            600.0,
            120.0,
            -50.0,
            900.0,
            2,
            Mount::SetIn,
        ),
    ];
    let mut worst = 0.0f64;
    for s in &v {
        let w = World::of(s);
        let r = run(s);
        let rh = if s.mount == Mount::SetIn {
            s.d / 2.0
        } else {
            s.d / 2.0 - s.tb
        };
        let a0 = r.hole.az_start_deg.to_radians();
        let span = r.hole.az_span_deg.to_radians();
        for i in 1..40 {
            // inside the span (the two ends are near-tangent, width ~1e-5: not compared), plus points just outside
            let a = a0 + span * (f64::from(i) / 40.0);
            let want = w.hole_brute(a, rh);
            let got = probe::hole_iv(s, a);
            match (want, got) {
                (Some(b), Some(g)) => {
                    let dd = (b[0] - g[0]).abs().max((b[1] - g[1]).abs());
                    worst = worst.max(dd);
                    assert!(dd < 5e-6, "{s:?} a {a}: {g:?} vs {b:?}");
                }
                (None, None) => {}
                (b, g) => panic!("{s:?} a {a}: scan {b:?} closed {g:?}"),
            }
        }
        for a in [a0 - 0.02, a0 + span + 0.02] {
            assert!(
                probe::hole_iv(s, a).is_none(),
                "{s:?}: interval outside the span"
            );
            assert!(
                w.hole_brute(a, rh).is_none(),
                "{s:?}: scan finds a hole outside the span"
            );
        }
    }
    println!("max |closed form - scan| over hole intervals: {worst:e} mm");
}

#[test]
fn hole_cut_square_to_the_plate_reaches_tee_over_two_sin_gamma_beyond_inner_only() {
    // concentric, branch on the cone axis plane, offset 0: the normal-cut hole is longer than the inner-surface-only
    // hole by 2 (T/2) sin(gamma) in x (spec 4.2 / check 7), derived here from the band geometry
    for (i, s) in examples().iter().take(1).enumerate() {
        let r = run(s);
        let k = (s.db / 2.0 - s.da / 2.0) / s.l;
        let sin_g = k.abs() / (1.0 + k * k).sqrt();
        let inner = r.hole.len_inner_only.unwrap();
        let normal = r.hole.len_normal_cut;
        // the extension is on the side facing the apex of the section line, so 2 x (T/2) sin(gamma) in total
        let ext = normal - inner;
        // inner-only is a chord on the inner circle (radial T), normal-cut is the band: compare with the rule
        assert!(
            ext > 0.0
                && (ext - 2.0 * (s.t / 2.0) * sin_g).abs()
                    < 0.35 * (s.t / 2.0) * sin_g * 2.0 + 0.05,
            "example {i}: extension {ext} vs {}",
            2.0 * (s.t / 2.0) * sin_g
        );
    }
}

#[test]
fn hole_polygon_area_matches_exact_integral_and_the_estimates_are_stated() {
    for s in examples() {
        let r = run(&s);
        let rel = (r.hole.area_poly - r.hole.area).abs() / r.hole.area;
        // an inscribed 1441 gon: polygon below the exact area by about perimeter^2 / (24 n^2) relative
        assert!(
            rel < 5e-5,
            "{s:?}: polygon {} exact {}",
            r.hole.area_poly,
            r.hole.area
        );
        assert!(r.hole.area_poly <= r.hole.area * (1.0 + 1e-9));
        assert!(
            r.hole.area_err <= 1e-6 * r.hole.area,
            "stated error {}",
            r.hole.area_err
        );
        assert!(
            r.hole.eps_rep > 0.0 && r.hole.eps_rep < 0.5,
            "eps_rep {}",
            r.hole.eps_rep
        );
        assert!(!r.certified);
    }
}

// ------------------------------------------------------------------------------------ development (fan)

fn mid3(w: &World, x: f64, a: f64) -> [f64; 3] {
    w.mid(x, a).0
}

fn dist3(p: [f64; 3], q: [f64; 3]) -> f64 {
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

#[test]
fn development_is_an_isometry_along_and_across_generators() {
    // non flipped, flat at 0: internal frame = physical frame
    for s in examples().iter().filter(|s| !is_flipped(s)) {
        let w = World::of(s);
        let fan = probe::fan_of(s).unwrap();
        for (x, a) in [(100.0, 0.3), (400.0, 2.0), (700.0, 4.0), (350.0, 5.9)] {
            if x > s.l {
                continue;
            }
            // along a generator: exact (the mid surface generator is a straight line)
            let (x2, a2) = ((x + 90.0).min(s.l), a);
            let dev = {
                let p = fan.pt(x, a);
                let q = fan.pt(x2, a2);
                (p[0] - q[0]).hypot(p[1] - q[1])
            };
            let d3 = dist3(mid3(&w, x, a), mid3(&w, x2, a2));
            assert!(
                (dev - d3).abs() <= 1e-7 * d3.max(1.0),
                "{s:?} along: {dev} vs {d3}"
            );
            // across: a short arc
            let da = 2e-4;
            let p = fan.pt(x, a);
            let q = fan.pt(x, a + da);
            let dev = (p[0] - q[0]).hypot(p[1] - q[1]);
            let d3 = dist3(mid3(&w, x, a), mid3(&w, x, a + da));
            assert!((dev - d3).abs() <= 2e-6 * d3, "{s:?} across: {dev} vs {d3}");
        }
    }
}

#[test]
fn developed_mouth_circle_has_the_length_of_the_3d_circle() {
    for s in examples().iter().filter(|s| !is_flipped(s)) {
        let w = World::of(s);
        let fan = probe::fan_of(s).unwrap();
        let n = 20_000;
        let mut len = 0.0;
        let mut prev = fan.pt(0.0, 0.0);
        for i in 1..=n {
            let q = fan.pt(0.0, 2.0 * PI * f64::from(i) / f64::from(n));
            len += (q[0] - prev[0]).hypot(q[1] - prev[1]);
            prev = q;
        }
        let want = 2.0 * PI * w.rad(0.0, s.t / 2.0);
        assert!((len - want).abs() < 2e-7 * want, "{s:?}: {len} vs {want}");
    }
}

#[test]
fn development_angle_equals_arc_length_of_generators_on_the_unit_sphere() {
    // psi(a) from the table against a fine polyline of unit generator directions (independent integration)
    for s in examples().iter().filter(|s| s.ecc && !is_flipped(s)) {
        let w = World::of(s);
        let r0 = w.ra - s.t / 2.0;
        let xa = -r0 / w.k;
        let apex = [xa, w.c(xa)[0], w.c(xa)[1]];
        let unit = |a: f64| {
            let p = mid3(&w, 0.0, a);
            let v = [p[0] - apex[0], p[1] - apex[1], p[2] - apex[2]];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / l, v[1] / l, v[2] / l]
        };
        let n = 200_000;
        let mut acc = 0.0;
        let mut prev = unit(0.0);
        let mut checks = vec![];
        for i in 1..=n {
            let a = 2.0 * PI * f64::from(i) / f64::from(n);
            let q = unit(a);
            acc += 2.0 * (dist3(prev, q) / 2.0).asin();
            prev = q;
            if i % 25_000 == 0 {
                checks.push((a, acc));
            }
        }
        let fan = probe::fan_of(s).unwrap();
        for (a, want) in checks {
            let got = fan.psi(a);
            assert!(
                (got - want).abs() < 5e-7 * want.max(1e-3) + 1e-9,
                "{s:?} a {a}: {got} vs {want}"
            );
        }
        let err = fan.err();
        let tot = fan.psi(2.0 * PI);
        assert!(
            err <= 1e-9 * tot,
            "stated table error {err} for total {tot}"
        );
    }
}

#[test]
fn hole_perimeter_cut_line_and_3d_curve_agree_for_thin_plate() {
    // thickness -> 0: the 3D intersection line, the branch cut line and the hole on the development have one length
    let rows = [
        (false, 2000.0, 1200.0, 1500.0, 300.0, 750.0, 0.0, 0.0),
        (false, 2000.0, 1200.0, 1500.0, 300.0, 750.0, 45.0, 250.0),
        (true, 1219.2, 914.4, 1200.0, 203.2, 600.0, 0.0, 0.0),
        (true, 1219.2, 914.4, 1200.0, 203.2, 600.0, 90.0, 100.0),
    ];
    for (ecc, da, db, l, r, x0, dir, e) in rows {
        let mut s = sp(
            ecc,
            da,
            db,
            l,
            0.0,
            2.0 * r + 0.001,
            x0,
            dir,
            e,
            3000.0,
            2,
            Mount::SetOn,
        );
        s.t = 0.001;
        s.tb = 0.001;
        let w = World::of(&s);
        // 3D line length: sample the contact of the outer wall with the cone surface
        let n = 200_000;
        let pt = |i: usize| {
            let th = 2.0 * PI * i as f64 / n as f64;
            let (x, lat) = (w.x0 + w.ro * th.cos(), w.e + w.ro * th.sin());
            let t = w.contact(x, lat, 0.0).unwrap();
            [x, t * w.u[0] + lat * w.v[0], t * w.u[1] + lat * w.v[1]]
        };
        let mut len3 = 0.0;
        let mut prev = pt(0);
        for i in 1..=n {
            let q = pt(i % n);
            len3 += dist3(prev, q);
            prev = q;
        }
        let res = run(&s);
        assert!(
            (res.branch.cut_length - len3).abs() < 0.05,
            "cut {} vs 3D {len3}",
            res.branch.cut_length
        );
        assert!(
            (res.hole.perimeter - len3).abs() < 0.05,
            "hole {} vs 3D {len3}",
            res.hole.perimeter
        );
    }
}

// ------------------------------------------------------------------------------------ limit of a vanishing half angle

#[test]
fn half_angle_to_zero_equals_the_tee_closed_forms() {
    for (e, mount) in [
        (0.0, Mount::SetOn),
        (150.0, Mount::SetOn),
        (-80.0, Mount::SetOn),
        (0.0, Mount::SetIn),
        (150.0, Mount::SetIn),
    ] {
        // k = 1e-7: the cone radius at the branch centre is 508 + k x0; the tee is compared at that radius. The remaining
        // difference is k rho (<= 3e-5 mm) over the width of the branch.
        let db = 1016.0 + 2.0 * 1500.0 * 1e-7;
        let k = (db - 1016.0) / 2.0 / 1500.0;
        let tee_piece = tee::branch_piece(&BranchPieceSpec {
            d: 610.0,
            t: 9.0,
            main_d: 1016.0 + 2.0 * k * 750.0,
            main_t: 12.0,
            h: 900.0,
            e,
            mount,
            angle_deg: None,
        })
        .unwrap();
        let s = sp(
            false, 1016.0, db, 1500.0, 0.0, 610.0, 750.0, 0.0, e, 900.0, 2, mount,
        );
        let c = run(&s);
        assert!((c.branch.circ - tee_piece.circ).abs() < 1e-9);
        for (a, b) in c.branch.marks.iter().zip(&tee_piece.marks) {
            // the cone mark order is the tee order (mark 4 faces x = 0)
            assert!(
                (a.len - b.len).abs() < 1e-4,
                "e {e}: {} vs {}",
                a.len,
                b.len
            );
        }
        assert!(
            (c.branch.mass_kg - tee_piece.mass_kg).abs() < 1e-5 * tee_piece.mass_kg,
            "{} vs {}",
            c.branch.mass_kg,
            tee_piece.mass_kg
        );
        assert!((c.branch.shortest - tee_piece.shortest).abs() < 1e-3);
        assert!((c.branch.longest - tee_piece.longest).abs() < 1e-3);
    }
}

#[test]
fn half_angle_to_zero_hole_equals_the_tee_hole_at_zero_offset() {
    // tee hole: width 2 rm asin(rh / ri) (inner surface), height 2 rh, rh = branch bore (set-on)
    let s = sp(
        false,
        1016.0,
        1016.0 + 2.0 * 1500.0 * 1e-7,
        1500.0,
        0.0,
        610.0,
        750.0,
        0.0,
        0.0,
        900.0,
        2,
        Mount::SetOn,
    );
    let r = run(&s);
    let (rm, ri, rh): (f64, f64, f64) = (508.0 - 6.0, 508.0 - 12.0, 305.0 - 9.0);
    let w = 2.0 * rm * (rh / ri).asin();
    assert!((r.hole.box_w - w).abs() < 0.05, "{} vs {w}", r.hole.box_w);
    assert!(
        (r.hole.box_h - 2.0 * rh).abs() < 1e-3,
        "{} vs {}",
        r.hole.box_h,
        2.0 * rh
    );
    let t = tee::tee_main(&TeeMainSpec::new(
        1016.0,
        12.0,
        1500.0,
        vec![BranchOnMain::new("1", 610.0, 9.0, Some(750.0), 0.0, 0.0)],
    ))
    .unwrap();
    assert!((r.hole.box_w - t.holes[0].hole_w).abs() < 0.05);
    assert!((r.hole.box_h - t.holes[0].hole_h).abs() < 1e-3);
    // set-in: hole cut at the branch outer radius
    let mut s2 = s.clone();
    s2.mount = Mount::SetIn;
    let r2 = run(&s2);
    let rh2 = 305.0;
    let w2 = 2.0 * rm * (rh2 / ri).asin();
    assert!(
        (r2.hole.box_w - w2).abs() < 0.05,
        "{} vs {w2}",
        r2.hole.box_w
    );
}

// ------------------------------------------------------------------------------------ invariances

#[test]
fn rotating_flat_side_and_branch_together_changes_nothing() {
    for (f1, d1, f2, d2) in [
        (180.0, 180.0, 0.0, 0.0),
        (90.0, 180.0, 0.0, 90.0),
        (270.0, 30.0, 0.0, 120.0),
        (45.0, 100.0, 0.0, 55.0),
    ] {
        let a = run(&sp(
            true,
            1219.2,
            914.4,
            1200.0,
            f1,
            406.4,
            600.0,
            d1,
            60.0,
            900.0,
            2,
            Mount::SetOn,
        ));
        let b = run(&sp(
            true,
            1219.2,
            914.4,
            1200.0,
            f2,
            406.4,
            600.0,
            d2,
            60.0,
            900.0,
            2,
            Mount::SetOn,
        ));
        for (p, q) in a.branch.marks.iter().zip(&b.branch.marks) {
            assert!((p.len - q.len).abs() < 1e-6);
        }
        assert!(
            (a.hole.area_poly - b.hole.area_poly).abs() < 1e-3 * 1e-3 * a.hole.area_poly + 1e-6
        );
        assert!((a.cone_mass_kg - b.cone_mass_kg).abs() < 1e-9 * a.cone_mass_kg);
    }
}

#[test]
fn small_mouth_written_first_is_the_same_physical_part() {
    // example 4 written from the other end: azimuths mirror, e shifts by the axis offset 152.4 (H too: none here)
    let big = run(&examples()[3]);
    let small = run(&sp(
        true,
        914.4,
        1219.2,
        1200.0,
        0.0,
        406.4,
        600.0,
        270.0,
        -152.4,
        900.0,
        2,
        Mount::SetOn,
    ));
    for (p, q) in big.branch.marks.iter().zip(&small.branch.marks) {
        assert!((p.len - q.len).abs() < 1e-6);
    }
    assert!((big.hole.area_poly - small.hole.area_poly).abs() < 1e-6 * big.hole.area_poly);
    assert!((small.hole.x_min - (1200.0 - big.hole.x_max)).abs() < 1e-9);
    assert!((small.hole.x_max - (1200.0 - big.hole.x_min)).abs() < 1e-9);
    assert_eq!(small.branch.d4, 1219.2);
    assert!((small.cone_mass_kg - big.cone_mass_kg).abs() < 1e-9 * big.cone_mass_kg);
    // and the physical (Da frame) contact agrees with the marks, so the mirrored model is the same solid (see the
    // bisection test, which includes this specification)
}

#[test]
fn concentric_pieces_are_bit_identical_to_the_cone_split_closed_form() {
    use nf_core::shapes::{Basis, ConeSpec, cone_split};
    for s in examples().iter().take(2) {
        let r = run(s);
        let c = cone_split(ConeSpec {
            d1: s.da.min(s.db),
            d2: s.da.max(s.db),
            l: s.l,
            t: s.t,
            basis: Basis::Mid,
            split: s.split.unwrap(),
        })
        .unwrap();
        assert_eq!(r.pieces[0].chord, Some(c.chord));
        assert_eq!(r.pieces[0].height, Some(c.height));
        assert_eq!(r.pieces[1].copies, s.split.unwrap() - 1);
        assert!(
            (r.cone_area - c.piece_area * f64::from(s.split.unwrap())).abs() < 1e-9 * r.cone_area
        );
    }
}

#[test]
fn eccentric_cone_area_equals_the_existing_quadrature_and_split_does_not_change_it() {
    use nf_core::shapes::{Basis, ConeSpec, ecc_cone_split};
    for n in [2u32, 3, 5, 12] {
        let mut s = examples()[2].clone();
        s.split = Some(n);
        let r = run(&s);
        let c = ecc_cone_split(ConeSpec {
            d1: s.da,
            d2: s.db,
            l: s.l,
            t: s.t,
            basis: Basis::Mid,
            split: 2,
        })
        .unwrap();
        assert!(
            (r.cone_area - c.area_total).abs() < 1e-8 * c.area_total,
            "n {n}: {} vs {}",
            r.cone_area,
            c.area_total
        );
        let sum: f64 = r.pieces.iter().map(|p| p.net_area).sum::<f64>() + r.hole.area;
        assert!((sum - r.cone_area).abs() < 1e-8 * r.cone_area);
        assert_eq!(r.pieces.len(), n as usize);
    }
}

#[test]
fn seam_rule_turns_only_when_the_hole_or_weld_is_in_the_way() {
    let r = run(&examples()[3]);
    assert!(!r.seams.hit && r.seams.moved_deg == 0.0 && !r.seams.cross);
    let r = run(&examples()[0]);
    assert!(r.seams.hit && !r.seams.cross && (r.seams.moved_deg + 90.0).abs() < 1e-9);
    // seams are equally spaced (360 / n) before and after the turn
    for s in [examples()[1].clone(), examples()[2].clone()] {
        let r = run(&s);
        let n = r.seams.deg.len();
        for k in 0..n {
            let step = (r.seams.deg[(k + 1) % n] - r.seams.deg[k]).rem_euclid(360.0);
            assert!((step - 360.0 / n as f64).abs() < 1e-9);
        }
        assert!(r.seams.seam_hole >= r.seams.gap && r.seams.seam_weld >= r.seams.gap);
    }
    // a seam that cannot avoid the hole is a hold that blocks the DXF, not a silent result
    let mut s = examples()[0].clone();
    s.split = Some(12);
    let r = run(&s);
    assert!(r.seams.cross && r.has_hold("conebranch_seam_crosses_hole"));
}

// ------------------------------------------------------------------------------------ the thickness convention

#[test]
fn radial_versus_normal_plate_thickness_is_quantified_not_hidden() {
    // First principles: a plate that is T thick ALONG THE NORMAL is T / cos(gamma) thick radially (section circles),
    // gamma = atan|k|. The engine takes T radially (rule CONEBRANCH_THICKNESS_RADIAL): its plate is T cos(gamma) thick
    // normally. Measured here as the change of the branch cut and of the hole when T_radial = T sqrt(1 + k^2).
    for (i, s) in examples().iter().take(2).enumerate() {
        let k = ((s.db - s.da) / 2.0 / s.l).abs();
        let cos_g = 1.0 / (1.0 + k * k).sqrt();
        let mut s_n = s.clone();
        s_n.t = s.t / cos_g;
        let (a, b) = (run(s), run(&s_n));
        let cut_diff = a
            .branch
            .marks
            .iter()
            .zip(&b.branch.marks)
            .map(|(p, q)| (p.len - q.len).abs())
            .fold(0.0, f64::max);
        let hole_diff = (a.hole.x_max - a.hole.x_min - (b.hole.x_max - b.hole.x_min)).abs();
        println!(
            "example {}: half angle {:.3} deg, plate normal {:.4} (engine) vs {:.4} (spec): max cut change {:.4} mm, hole length change {:.4} mm",
            i + 1,
            k.atan().to_degrees(),
            s.t * cos_g,
            s.t,
            cut_diff,
            hole_diff
        );
        // set-on touches the outer surface only: the cut is unchanged; set-in feels the radial inner surface
        if s.mount == Mount::SetOn {
            assert!(
                cut_diff < 1e-9,
                "set-on cut must not depend on the thickness: {cut_diff}"
            );
        } else {
            assert!(
                cut_diff > 0.2 && cut_diff < 0.6,
                "example 2 set-in: {cut_diff}"
            );
        }
        assert!(hole_diff < 1.0);
    }
    assert!(
        CONEBRANCH_NAMED_RULES
            .iter()
            .any(|r| r.0 == "CONEBRANCH_THICKNESS_RADIAL" && r.3 == "UNVERIFIED")
    );
}

// ------------------------------------------------------------------------------------ refusals, defaults, fuzz

#[test]
fn every_named_rule_is_unverified_and_results_are_uncertified() {
    for r in CONEBRANCH_NAMED_RULES {
        assert_eq!(r.3, "UNVERIFIED", "{}", r.0);
    }
    for s in examples() {
        let r = run(&s);
        assert!(
            !r.certified
                && r.method == conebranch::CONEBRANCH_METHOD
                && r.rules == &CONEBRANCH_NAMED_RULES[..]
        );
    }
}

#[test]
fn refusals_are_coded_and_reachable() {
    let mut seen: Vec<&'static str> = vec![];
    let mut push = |s: &ConeBranchSpec| seen.push(code(s));
    let base = examples()[0].clone();
    let with = |f: &dyn Fn(&mut ConeBranchSpec)| {
        let mut s = base.clone();
        f(&mut s);
        s
    };
    push(&with(&|s| s.d = f64::NAN));
    push(&with(&|s| s.da = 0.0));
    push(&with(&|s| s.l = 1e12));
    push(&with(&|s| s.db = s.da));
    push(&with(&|s| s.t = 600.0));
    push(&with(&|s| s.angle_deg = Some(60.0)));
    push(&with(&|s| s.split = Some(13)));
    push(&with(&|s| s.d = 1000.0));
    push(&with(&|s| s.x = Some(100.0)));
    push(&with(&|s| s.h = 300.0));
    // a bore that fits the section but whose axis lies outside the inner surface: the hole does not pass the plate
    push(
        &sp(
            false,
            1016.0,
            812.8,
            812.8,
            0.0,
            30.0,
            406.4,
            0.0,
            450.0,
            900.0,
            2,
            Mount::SetOn,
        )
        .tap_tb(14.5),
    );
    for want in [
        "conebranch_value_not_finite",
        "conebranch_value_not_positive",
        "conebranch_value_too_large",
        "conebranch_not_conical",
        "conebranch_wall_too_thick",
        "conebranch_branch_not_perpendicular",
        "conebranch_split_out_of_range",
        "conebranch_branch_not_within_cone",
        "conebranch_hole_beyond_mouth",
        "conebranch_branch_end_below_cut",
        "conebranch_hole_not_through_plate",
    ] {
        assert!(seen.contains(&want), "{want} not produced: {seen:?}");
    }
    for c in &seen {
        assert!(CONEBRANCH_REFUSAL_CODES.contains(c), "{c}");
    }
}

#[test]
fn a_blocked_cone_blocks_the_branch() {
    // DESIGN L2: no branch piece is returned when the cone cannot be built (the result type holds both or neither)
    let mut s = examples()[0].clone();
    s.x = Some(-50.0);
    let e = cb(&s).expect_err("refused");
    assert!(
        e.code == "conebranch_hole_beyond_mouth" || e.code == "conebranch_branch_not_within_cone",
        "{}",
        e.code
    );
}

#[test]
fn missing_input_is_asked_never_guessed_silently() {
    let mut s = examples()[0].clone();
    s.x = None;
    let r = run(&s);
    assert_eq!(r.x_used, s.l / 2.0);
    assert!(r.questions.contains(&conebranch::CB_X_QUESTION));
    let mut e = examples()[2].clone();
    e.flat_deg = None;
    let r = run(&e);
    assert!(
        r.questions.contains(&conebranch::CB_FLAT_QUESTION)
            && r.has_hold("conebranch_flat_side_unconfirmed")
    );
    // a split of 0 or 1 is refused (ZK promoted it to 2 silently); None is the named default
    let mut p = examples()[0].clone();
    p.split = None;
    assert_eq!(run(&p).n, CB_DEFAULT_SPLIT);
    for n in [0u32, 1, 13] {
        p.split = Some(n);
        assert_eq!(code(&p), "conebranch_split_out_of_range");
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick(&mut self, pool: &[f64]) -> f64 {
        pool[(self.next() % pool.len() as u64) as usize]
    }
}

#[test]
fn fuzz_no_panic_no_nan_and_coded_refusals() {
    const POOL: [f64; 22] = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
        -1.0,
        1e-9,
        1e-6,
        0.001,
        1.0,
        9.0,
        12.0,
        60.0,
        100.0,
        300.0,
        406.4,
        812.8,
        1016.0,
        1219.2,
        1500.0,
        45.0,
        1e9,
        -200.0,
    ];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut ok, mut refused) = (0usize, 0usize);
    // mostly plausible values, 8 % hostile ones, so that both outcomes are exercised
    const GOOD_D: [f64; 6] = [800.0, 914.4, 1016.0, 1219.2, 1500.0, 2000.0];
    const GOOD_L: [f64; 5] = [400.0, 600.0, 812.8, 1200.0, 1500.0];
    const GOOD_T: [f64; 4] = [6.0, 9.0, 12.0, 16.0];
    const GOOD_B: [f64; 5] = [114.3, 219.1, 406.4, 508.0, 610.0];
    const GOOD_X: [f64; 6] = [150.0, 300.0, 400.0, 600.0, 750.0, 900.0];
    const GOOD_A: [f64; 8] = [0.0, 30.0, 90.0, 120.0, 180.0, 270.0, 45.0, 359.0];
    const GOOD_E: [f64; 7] = [0.0, 0.0, 50.0, -60.0, 100.0, -150.0, 250.0];
    const GOOD_H: [f64; 4] = [500.0, 800.0, 900.0, 1500.0];
    let draw = |rng: &mut Rng, good: &[f64]| {
        if rng.next() % 100 < 8 {
            rng.pick(&POOL)
        } else {
            rng.pick(good)
        }
    };
    for it in 0..1500 {
        let s = ConeBranchSpec {
            da: draw(&mut rng, &GOOD_D),
            db: draw(&mut rng, &GOOD_D),
            l: draw(&mut rng, &GOOD_L),
            t: draw(&mut rng, &GOOD_T),
            ecc: it % 3 == 0,
            flat_deg: if it % 5 == 0 || it % 3 != 0 {
                None
            } else {
                Some(draw(&mut rng, &GOOD_A))
            },
            d: draw(&mut rng, &GOOD_B),
            tb: draw(&mut rng, &GOOD_T),
            x: if it % 7 == 0 {
                None
            } else {
                Some(draw(&mut rng, &GOOD_X))
            },
            dir_deg: draw(&mut rng, &GOOD_A),
            e: draw(&mut rng, &GOOD_E),
            h: draw(&mut rng, &GOOD_H),
            angle_deg: if it % 11 == 0 {
                Some(draw(&mut rng, &GOOD_A))
            } else {
                None
            },
            mount: if it % 2 == 0 {
                Mount::SetOn
            } else {
                Mount::SetIn
            },
            split: if it % 13 == 0 {
                Some((rng.next() % 16) as u32)
            } else {
                Some(2 + (it % 4) as u32)
            },
            seam_gap: if it % 17 == 0 {
                Some(rng.pick(&POOL))
            } else {
                None
            },
        };
        match cb(&s) {
            Ok(r) => {
                ok += 1;
                assert!(r.branch.marks.iter().all(|m| m.len.is_finite()));
                assert!(
                    r.branch.mass_kg.is_finite()
                        && r.cone_mass_kg.is_finite()
                        && r.hole.area.is_finite()
                );
                assert!(
                    r.hole
                        .marks
                        .iter()
                        .all(|m| m.x.is_finite() && m.y.is_finite())
                );
                assert!(r.pieces.iter().all(|p| {
                    p.outline
                        .iter()
                        .all(|q| q[0].is_finite() && q[1].is_finite())
                }));
                assert!(!r.certified);
            }
            Err(e) => {
                refused += 1;
                assert!(
                    CONEBRANCH_REFUSAL_CODES.contains(&e.code),
                    "{} for {s:?}",
                    e.code
                );
                assert!(!e.message.is_empty());
            }
        }
    }
    println!("fuzz: {ok} built, {refused} refused");
    assert!(
        ok > 20 && refused > 20,
        "fuzz must exercise both outcomes: {ok} built, {refused} refused"
    );
    // structured fuzz around the valid examples: every neighbour either builds finite or is refused with a code
    let mut ok2 = 0;
    for s in examples() {
        for (dx, dd, dh, de) in [
            (0.0, 1.0, 0.0, 0.0),
            (50.0, 0.7, 0.0, 30.0),
            (-250.0, 1.0, 0.0, 0.0),
            (0.0, 1.8, -400.0, 0.0),
            (100.0, 1.0, 0.0, 400.0),
            (0.0, 0.2, 0.0, -90.0),
        ] {
            let mut p = s.clone();
            p.x = Some(s.x.unwrap() + dx);
            p.d = s.d * dd;
            p.h += dh;
            p.e += de;
            match cb(&p) {
                Ok(r) => {
                    ok2 += 1;
                    assert!(
                        r.branch
                            .marks
                            .iter()
                            .all(|m| m.len.is_finite() && m.len > 0.0)
                    );
                    assert!(r.cone_mass_kg.is_finite() && r.cone_mass_kg > 0.0);
                }
                Err(e) => assert!(CONEBRANCH_REFUSAL_CODES.contains(&e.code), "{}", e.code),
            }
        }
    }
    assert!(ok2 > 5);
}

// ------------------------------------------------------------------------------------ NORMAL thickness (the default)
//
// First principles, no engine formula: the plate is the set of points `P(x, a) - d n(x, a)`, `0 <= d <= T`, where
// `P` is the OUTER surface (the drawing surface) and `n` its outward unit normal taken from a finite-difference cross
// product. The inner surface is `d = T`, the neutral surface `d = T/2`. The axial coordinate of an offset point is
// `x - d n_x`, so a point of the offset surface with a given axial coordinate is found by bisection.

impl World {
    fn surf(&self, x: f64, a: f64) -> [f64; 3] {
        let c = self.c(x);
        let r = self.rad(x, 0.0);
        [x, c[0] + r * a.cos(), c[1] + r * a.sin()]
    }
    fn outer_normal(&self, x: f64, a: f64) -> [f64; 3] {
        let (hx, ha) = (1e-5, 1e-7);
        let (xp, xm) = (self.surf(x + hx, a), self.surf(x - hx, a));
        let (ap, am) = (self.surf(x, a + ha), self.surf(x, a - ha));
        let sub = |p: [f64; 3], q: [f64; 3], h: f64| {
            [
                (p[0] - q[0]) / (2.0 * h),
                (p[1] - q[1]) / (2.0 * h),
                (p[2] - q[2]) / (2.0 * h),
            ]
        };
        let (tx, ta) = (sub(xp, xm, hx), sub(ap, am, ha));
        let mut n = [
            tx[1] * ta[2] - tx[2] * ta[1],
            tx[2] * ta[0] - tx[0] * ta[2],
            tx[0] * ta[1] - tx[1] * ta[0],
        ];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        for q in &mut n {
            *q /= l;
        }
        if n[1] * a.cos() + n[2] * a.sin() < 0.0 {
            for q in &mut n {
                *q = -*q;
            }
        }
        n
    }
    /// The point of the surface offset by `d` along the inward normal that has axial coordinate `xw` at azimuth `a`,
    /// with the normal of the parent point.
    fn offset_pt(&self, xw: f64, a: f64, d: f64) -> ([f64; 3], [f64; 3]) {
        let (mut lo, mut hi) = (xw - 5.0 * d - 1.0, xw + 5.0 * d + 1.0);
        for _ in 0..50 {
            let m = 0.5 * (lo + hi);
            if m - d * self.outer_normal(m, a)[0] < xw {
                lo = m;
            } else {
                hi = m;
            }
        }
        let xs = 0.5 * (lo + hi);
        let (p, n) = (self.surf(xs, a), self.outer_normal(xs, a));
        ([p[0] - d * n[0], p[1] - d * n[1], p[2] - d * n[2]], n)
    }
    fn lat3(&self, p: [f64; 3]) -> f64 {
        p[1] * self.v[0] + p[2] * self.v[1]
    }
    fn along3(&self, p: [f64; 3]) -> f64 {
        p[1] * self.u[0] + p[2] * self.u[1]
    }
    /// Highest along-branch coordinate at which the wall line (axial `xw`, lateral `lw`) meets the INNER surface:
    /// scan the azimuth for sign changes of the lateral offset, bisect each.
    fn inner_contact(&self, xw: f64, lw: f64) -> Option<f64> {
        const N: usize = 90;
        let f = |a: f64| self.lat3(self.offset_pt(xw, a, self.t).0) - lw;
        let mut best: Option<f64> = None;
        let mut prev = (0.0, f(0.0));
        for i in 1..=N {
            let a = 2.0 * PI * i as f64 / N as f64;
            let fa = f(a);
            if prev.1 * fa <= 0.0 {
                let (mut lo, mut hi, flo) = (prev.0, a, prev.1);
                for _ in 0..50 {
                    let m = 0.5 * (lo + hi);
                    if f(m) * flo <= 0.0 {
                        hi = m;
                    } else {
                        lo = m;
                    }
                }
                let t = self.along3(self.offset_pt(xw, 0.5 * (lo + hi), self.t).0);
                best = Some(best.map_or(t, |b: f64| b.max(t)));
            }
            prev = (a, fa);
        }
        best
    }
    fn cut_brute_inner_normal(&self, th: f64) -> f64 {
        (0..=40)
            .filter_map(|i| {
                let rho = self.ri + (self.ro - self.ri) * f64::from(i) / 40.0;
                self.inner_contact(self.x0 + rho * th.cos(), self.e + rho * th.sin())
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }
    /// Smallest squared distance from the branch axis over the plate through the neutral point of axial coordinate
    /// `xm` at azimuth `a` (points `M + w n`, `|w| <= T/2`; the squared distance is a convex quadratic in `w`).
    fn band_min_normal(&self, xm: f64, a: f64) -> f64 {
        let (m, n) = self.offset_pt(xm, a, self.t / 2.0);
        let (dx, dl) = (m[0] - self.x0, self.lat3(m) - self.e);
        let (nx, nl) = (n[0], n[1] * self.v[0] + n[2] * self.v[1]);
        let (aa, bb) = (nx * nx + nl * nl, 2.0 * (dx * nx + dl * nl));
        let w = if aa > 1e-18 {
            (-bb / (2.0 * aa)).clamp(-self.t / 2.0, self.t / 2.0)
        } else {
            0.0
        };
        let (px, pl) = (dx + w * nx, dl + w * nl);
        px * px + pl * pl
    }
    fn hole_brute_normal(&self, a: f64, rh: f64) -> Option<[f64; 2]> {
        let (lo, hi) = (self.x0 - rh - 4.0 * self.t, self.x0 + rh + 4.0 * self.t);
        let n = 400usize;
        let xs: Vec<f64> = (0..=n)
            .map(|i| lo + (hi - lo) * i as f64 / n as f64)
            .collect();
        let ins: Vec<bool> = xs
            .iter()
            .map(|&x| self.band_min_normal(x, a) < rh * rh)
            .collect();
        let first = ins.iter().position(|&b| b)?;
        let last = ins.iter().rposition(|&b| b)?;
        let refine = |mut out: f64, mut inn: f64| {
            for _ in 0..50 {
                let m = 0.5 * (out + inn);
                if self.band_min_normal(m, a) < rh * rh {
                    inn = m;
                } else {
                    out = m;
                }
            }
            inn
        };
        Some([
            refine(xs[first.saturating_sub(1)], xs[first]),
            refine(xs[(last + 1).min(n)], xs[last]),
        ])
    }
}

fn normal_specs() -> Vec<ConeBranchSpec> {
    let ex = examples();
    vec![
        ex[1].clone(), // concentric set-in, offset
        sp(
            true,
            1219.2,
            914.4,
            1200.0,
            0.0,
            406.4,
            600.0,
            120.0,
            -50.0,
            900.0,
            2,
            Mount::SetIn,
        ), // eccentric set-in
        ex[0].clone(), // concentric set-on
        ex[3].clone(), // eccentric set-on, branch at 90 deg
        sp(
            true,
            1219.2,
            914.4,
            1200.0,
            0.0,
            406.4,
            500.0,
            60.0,
            80.0,
            900.0,
            2,
            Mount::SetIn,
        ),
    ]
}

fn run_n(s: &ConeBranchSpec) -> ConeBranchResult {
    conebranch::cone_branch(s).unwrap_or_else(|e| panic!("{s:?}: {} {}", e.code, e.message))
}

#[test]
fn normal_default_set_in_cut_equals_root_finding_on_the_offset_surface() {
    let mut worst = 0.0f64;
    for s in normal_specs().iter().filter(|s| s.mount == Mount::SetIn) {
        let w = World::of(s);
        for i in 0..8 {
            let th = 2.0 * PI * f64::from(i) / 8.0 + 0.3;
            let want = w.cut_brute_inner_normal(th);
            let got = probe::cut_t_conv(s, th, ThicknessConvention::Normal).unwrap();
            worst = worst.max((want - got).abs());
            assert!(
                (want - got).abs() < 2e-3,
                "{s:?} th {th}: offset-surface scan {want} engine {got}"
            );
        }
    }
    println!("max |engine - offset surface scan| of the set-in normal cut: {worst:e} mm");
}

#[test]
fn normal_default_hole_interval_equals_scan_of_the_plate_normal_to_the_surface() {
    let mut worst = 0.0f64;
    for s in &normal_specs() {
        let w = World::of(s);
        let r = run_n(s);
        let rh = if s.mount == Mount::SetIn {
            s.d / 2.0
        } else {
            s.d / 2.0 - s.tb
        };
        let a0 = r.hole.az_start_deg.to_radians();
        let span = r.hole.az_span_deg.to_radians();
        for i in (2..40).step_by(4) {
            let a = a0 + span * (f64::from(i) / 40.0);
            let (want, got) = (
                w.hole_brute_normal(a, rh),
                probe::hole_iv_conv(s, a, ThicknessConvention::Normal),
            );
            let (b, g) = (want.expect("scan"), got.expect("closed"));
            let dd = (b[0] - g[0]).abs().max((b[1] - g[1]).abs());
            worst = worst.max(dd);
            assert!(dd < 5e-4, "{s:?} a {a}: {g:?} vs {b:?}");
        }
        for a in [a0 - 0.02, a0 + span + 0.02] {
            assert!(
                probe::hole_iv_conv(s, a, ThicknessConvention::Normal).is_none(),
                "{s:?}: interval outside the span"
            );
            assert!(
                w.hole_brute_normal(a, rh).is_none(),
                "{s:?}: scan finds a hole outside the span"
            );
        }
    }
    println!("max |closed form - normal scan| over hole intervals: {worst:e} mm");
}

#[test]
fn normal_default_weld_line_sits_on_the_neutral_surface_along_the_normal() {
    // the weld curve lies on the OUTER surface; its image on the neutral surface is the outer point minus (T/2) n
    for s in normal_specs().iter().take(4) {
        let w = World::of(s);
        let m = 24usize;
        let got = probe::weld(s, ThicknessConvention::Normal, m);
        assert_eq!(got.len(), m);
        for (i, q) in got.iter().enumerate() {
            let th = 2.0 * PI * i as f64 / m as f64;
            let (xw, lw) = (w.x0 + w.ro * th.cos(), w.e + w.ro * th.sin());
            let t = w.contact(xw, lw, 0.0).expect("contact");
            let p = [xw, t * w.u[0] + lw * w.v[0], t * w.u[1] + lw * w.v[1]];
            let c = w.c(xw);
            let a = (p[2] - c[1]).atan2(p[1] - c[0]);
            let n = w.outer_normal(xw, a);
            let x_mid = xw - (w.t / 2.0) * n[0];
            let da = (q[1] - a + PI).rem_euclid(2.0 * PI) - PI;
            assert!(
                (q[0] - x_mid).abs() < 1e-5 && da.abs() < 1e-9,
                "{s:?} i {i}: engine {q:?} vs x {x_mid} a {a}"
            );
        }
    }
}

#[test]
fn normal_is_the_default_and_concentric_normal_is_radial_with_t_over_cos_gamma() {
    // first principles: a concentric plate T thick along the normal is T / cos(gamma) = T sqrt(1 + k^2) thick between
    // section circles, so the branch cut, the developed fan and the cone pieces equal those of the radial rule with
    // that thickness (the hole band and the weld image do NOT: they follow the normal, see their own tests)
    for s in normal_specs()
        .iter()
        .filter(|s| !s.ecc)
        .chain(std::iter::once(&examples()[0]))
    {
        let a = run_n(s);
        assert_eq!(a.thickness_convention, ThicknessConvention::Normal);
        let k = ((s.db - s.da) / 2.0 / s.l).abs();
        let mut s_r = s.clone();
        s_r.t = s.t * (1.0 + k * k).sqrt();
        let b = run(&s_r);
        assert_eq!(b.thickness_convention, ThicknessConvention::Radial);
        for (p, q) in a.branch.marks.iter().zip(&b.branch.marks) {
            assert!(
                (p.len - q.len).abs() < 1e-7,
                "mark {}: {} vs {}",
                p.i,
                p.len,
                q.len
            );
        }
        assert!((a.fan_total_angle - b.fan_total_angle).abs() < 1e-12);
        assert!((a.cone_area - b.cone_area).abs() < 1e-6 * b.cone_area);
        assert!((a.branch.net_area - b.branch.net_area).abs() < 1e-6 * b.branch.net_area);
        assert!(a.cone_mass_kg > 0.0);
    }
}

#[test]
fn thickness_delta_states_the_change_between_the_two_rules() {
    let mut any_hole_change = false;
    for s in &normal_specs() {
        let n = run_n(s);
        let r = run(s);
        let (dn, dr) = (
            n.thickness_delta.as_ref().expect("delta (normal run)"),
            r.thickness_delta.as_ref().expect("delta (radial run)"),
        );
        assert_eq!(n.thickness_convention, ThicknessConvention::Normal);
        assert_eq!(dn.other, ThicknessConvention::Radial);
        assert_eq!(dr.other, ThicknessConvention::Normal);
        assert_eq!(dn.cut_shift_marks.len(), 17);
        // the delta is exactly the difference of the two runs' own numbers
        for (i, d) in dn.cut_shift_marks.iter().enumerate() {
            let diff = n.branch.marks[i].len - r.branch.marks[i].len;
            assert!((d - diff).abs() < 1e-7, "{s:?} mark {i}: {d} vs {diff}");
            assert!((d + dr.cut_shift_marks[i]).abs() < 1e-7);
        }
        // the hole delta is taken at ONE azimuth (the middle of the normal run's hole), through the probes
        let am = (n.hole.az_start_deg + n.hole.az_span_deg / 2.0).to_radians();
        let len = |c| {
            let v = probe::hole_iv_conv(s, am, c).unwrap();
            v[1] - v[0]
        };
        let hn = len(ThicknessConvention::Normal) - len(ThicknessConvention::Radial);
        assert!((dn.hole_len_delta.unwrap() - hn).abs() < 1e-6, "{s:?}");
        assert!((dn.hole_len_this - n.hole.len_normal_cut).abs() < 1e-6);
        let mid = |c| {
            let v = probe::hole_iv_conv(s, am, c).unwrap();
            0.5 * (v[1] + v[0])
        };
        let ms = mid(ThicknessConvention::Normal) - mid(ThicknessConvention::Radial);
        assert!((dn.hole_mid_shift.unwrap() - ms).abs() < 1e-6, "{s:?}");
        any_hole_change |= hn.abs() > 1e-3 || ms.abs() > 1e-3;
        // set-on touches the outer surface only: no cut shift at all
        if s.mount == Mount::SetOn {
            assert!(dn.cut_shift_max == 0.0 && dn.cut_shift_marks.iter().all(|d| *d == 0.0));
        } else {
            assert!(
                dn.cut_shift_max > 0.1,
                "{s:?}: set-in shift {}",
                dn.cut_shift_max
            );
        }
        // concentric: the neutral surface is exact, eccentric: stated deviation (below 1 percent of T)
        if s.ecc {
            assert!(n.mid_surface_dev > 0.0 && n.mid_surface_dev < 0.01 * s.t);
        } else {
            assert!(n.mid_surface_dev < 1e-12);
        }
        println!(
            "{} {:?}: half angle {:.2} deg: set-in cut shift max {:.4} mm (mark 4: {:.4}), hole length {:.4} (radial) -> {:.4} (normal) ({:+.4}), hole centre shift {:+.4}, neutral surface deviation {:.5}",
            if s.ecc { "ecc" } else { "conc" },
            s.mount,
            (((s.db - s.da) / 2.0 / s.l).abs()).atan().to_degrees(),
            dn.cut_shift_max,
            dn.cut_shift_marks[4],
            dn.hole_len_other.unwrap(),
            dn.hole_len_this,
            dn.hole_len_delta.unwrap(),
            dn.hole_mid_shift.unwrap(),
            n.mid_surface_dev
        );
    }
    assert!(
        any_hole_change,
        "the hole length never changes with the rule"
    );
}

#[test]
fn normal_hole_is_consistent_and_inner_only_is_shorter() {
    for s in &normal_specs() {
        let r = run_n(s);
        let inner = r.hole.len_inner_only.expect("inner only");
        assert!(inner > 0.0 && inner < r.hole.len_normal_cut, "{s:?}");
    }
}

#[test]
fn negative_seam_gap_is_refused_and_zero_is_allowed() {
    for conv in [ThicknessConvention::Normal, ThicknessConvention::Radial] {
        let mut s = examples()[0].clone();
        s.seam_gap = Some(-1.0);
        assert_eq!(
            conebranch::cone_branch_with(&s, conv).unwrap_err().code,
            "conebranch_seam_gap_negative"
        );
        s.seam_gap = Some(-1e-9);
        assert_eq!(code(&s), "conebranch_seam_gap_negative");
        s.seam_gap = Some(0.0);
        assert!(conebranch::cone_branch_with(&s, conv).is_ok());
    }
}

#[test]
fn flat_side_on_a_concentric_cone_is_refused_never_ignored() {
    for conv in [ThicknessConvention::Normal, ThicknessConvention::Radial] {
        for f in [0.0, 30.0, f64::NAN] {
            let mut s = examples()[0].clone();
            s.flat_deg = Some(f);
            assert_eq!(
                conebranch::cone_branch_with(&s, conv).unwrap_err().code,
                "conebranch_flat_not_applicable",
                "flat {f}"
            );
        }
    }
    // an eccentric cone still takes it (and still asks when it is missing)
    let mut e = examples()[2].clone();
    e.flat_deg = Some(30.0);
    assert!(conebranch::cone_branch(&e).is_ok());
    for c in [
        "conebranch_seam_gap_negative",
        "conebranch_flat_not_applicable",
    ] {
        assert!(CONEBRANCH_REFUSAL_CODES.contains(&c));
    }
}

#[test]
fn normal_rule_is_named_and_the_radial_one_stays_available() {
    for want in ["CONEBRANCH_THICKNESS_NORMAL", "CONEBRANCH_THICKNESS_RADIAL"] {
        assert!(
            CONEBRANCH_NAMED_RULES
                .iter()
                .any(|r| r.0 == want && r.3 == "UNVERIFIED"),
            "{want}"
        );
    }
}

#[test]
fn small_mouth_written_first_is_the_same_part_under_the_normal_rule() {
    let big = run_n(&examples()[3]);
    let small = run_n(&sp(
        true,
        914.4,
        1219.2,
        1200.0,
        0.0,
        406.4,
        600.0,
        270.0,
        -152.4,
        900.0,
        2,
        Mount::SetOn,
    ));
    for (p, q) in big.branch.marks.iter().zip(&small.branch.marks) {
        assert!((p.len - q.len).abs() < 1e-6);
    }
    assert!((small.hole.x_min - (1200.0 - big.hole.x_max)).abs() < 1e-6);
    assert!((small.hole.x_max - (1200.0 - big.hole.x_min)).abs() < 1e-6);
}
