//! Independent reference geometry for the wye tests (L9b). Written from the definition of the wye as the union of three
//! half solids (cylinders / cones about three centre lines through one point), NOT from nf_core::wye and not from ZK's
//! formulas: a leg's outer surface is pierced, going inward along a generator, by the first neighbour half solid.
//! Nothing here calls nf_core::wye::probe.
#![allow(dead_code)]

pub type V3 = [f64; 3];

pub fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub const NB: [[usize; 2]; 3] = [[1, 2], [0, 2], [0, 1]];

pub struct Ref {
    pub rs: f64,
    pub u: [V3; 3],
    pub e1: [V3; 3],
    pub psi: [f64; 3],
}

impl Ref {
    pub fn new(rs: f64, b1_deg: f64, b2_deg: f64, psi: f64) -> Ref {
        let (b1, b2) = (b1_deg.to_radians(), b2_deg.to_radians());
        let u = [
            [-1.0, 0.0, 0.0],
            [b1.cos(), b1.sin(), 0.0],
            [b2.cos(), -b2.sin(), 0.0],
        ];
        let mut e1 = [[0.0; 3]; 3];
        for i in 0..3 {
            // unit vector in the plane of the Y, square to the leg, on the side of the first neighbour; a neighbour
            // exactly in line with the leg: on the side away from the second neighbour
            let mut e = [-u[i][1], u[i][0], 0.0];
            let a = dot(&e, &u[NB[i][0]]);
            let b = dot(&e, &u[NB[i][1]]);
            if a < -1e-12 || (a.abs() <= 1e-12 && b > 0.0) {
                e = [-e[0], -e[1], 0.0];
            }
            e1[i] = e;
        }
        Ref {
            rs,
            u,
            e1,
            psi: [0.0, psi, psi],
        }
    }

    /// Radius of leg `i`'s surface (offset `ao` along the inward normal) at axial position `s`.
    pub fn rho(&self, i: usize, s: f64, ao: f64) -> f64 {
        (self.rs - s * self.psi[i].sin() - ao) / self.psi[i].cos()
    }

    pub fn point(&self, i: usize, s: f64, th: f64, ao: f64) -> V3 {
        let r = self.rho(i, s, ao);
        let (c, sn) = (th.cos(), th.sin());
        [
            s * self.u[i][0] + r * c * self.e1[i][0],
            s * self.u[i][1] + r * c * self.e1[i][1],
            r * sn,
        ]
    }

    /// (axial position along leg j, distance from leg j's axis) of a point.
    pub fn axis_coords(&self, j: usize, p: &V3) -> (f64, f64) {
        let s = dot(p, &self.u[j]);
        let perp2 = dot(p, p) - s * s;
        (s, perp2.max(0.0).sqrt())
    }

    /// Is `p` inside a neighbour solid of leg `i` (closer to the axis than the surface offset `ao_nb`)? `half`: only the
    /// half solid with axial position >= 0 (right for cylinders; a cone wye's main protrudes ahead of the intersection, so
    /// its solid must be allowed behind it: `half = false`, the infinite solid, empty beyond the cone's apex).
    pub fn inside_neighbour_h(&self, i: usize, p: &V3, ao_nb: f64, half: bool) -> bool {
        NB[i].iter().any(|&j| {
            let (s, perp) = self.axis_coords(j, p);
            (!half || s >= 0.0) && perp < self.rho(j, s, ao_nb)
        })
    }

    pub fn inside_neighbour(&self, i: usize, p: &V3, ao_nb: f64) -> bool {
        self.inside_neighbour_h(i, p, ao_nb, false)
    }

    /// First entry into a neighbour half solid going inward along the generator `th` of leg `i` (surface offset `ao`),
    /// from `s_hi` down to `-3 rs`. `None` when the start is already inside or no entry is found.
    pub fn pierce(&self, i: usize, th: f64, ao: f64, ao_nb: f64, s_hi: f64) -> Option<f64> {
        self.pierce_h(i, th, ao, ao_nb, s_hi, false)
    }

    pub fn pierce_h(
        &self,
        i: usize,
        th: f64,
        ao: f64,
        ao_nb: f64,
        s_hi: f64,
        half: bool,
    ) -> Option<f64> {
        let inside = |s: f64| self.inside_neighbour_h(i, &self.point(i, s, th, ao), ao_nb, half);
        if inside(s_hi) {
            return None;
        }
        let step = self.rs / 400.0;
        let mut hi = s_hi;
        let lo_limit = -3.0 * self.rs;
        let mut s = s_hi;
        while s > lo_limit {
            s -= step;
            if inside(s) {
                let mut lo = s;
                for _ in 0..80 {
                    let mid = 0.5 * (lo + hi);
                    if inside(mid) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                return Some(0.5 * (lo + hi));
            }
            hi = s;
        }
        None
    }

    /// Every meeting (root, bisected from sign changes of `perp_j - rho_j`) of the generator `th` of leg `i` (surface offset
    /// `ao`) with the surface of a neighbour (offset `ao_nb`), scanning inward from `s_hi` to `-3 rs`: `(j, s)`.
    pub fn surface_roots(
        &self,
        i: usize,
        th: f64,
        ao: f64,
        ao_nb: f64,
        s_hi: f64,
    ) -> Vec<(usize, f64)> {
        let mut out = Vec::new();
        let step = self.rs / 400.0;
        for &j in &NB[i] {
            let f = |s: f64| {
                let p = self.point(i, s, th, ao);
                let (sj, perp) = self.axis_coords(j, &p);
                perp - self.rho(j, sj, ao_nb)
            };
            let mut hi = s_hi;
            let mut fh = f(hi);
            let mut s = s_hi;
            while s > -3.0 * self.rs {
                s -= step;
                let fs = f(s);
                if (fs < 0.0) != (fh < 0.0) {
                    let (mut a, mut b) = (s, hi);
                    for _ in 0..90 {
                        let m = 0.5 * (a + b);
                        if (f(m) < 0.0) == (fs < 0.0) {
                            a = m;
                        } else {
                            b = m;
                        }
                    }
                    out.push((j, 0.5 * (a + b)));
                }
                hi = s;
                fh = fs;
            }
        }
        out
    }

    /// A neighbour whose surface the generator touches without crossing (a double root: `perp - rho` never changes sign but
    /// comes within `1e-3 rs` of 0): the sign-change scan cannot see that meeting, so such a sample is skipped by the property
    /// test and counted. (Generic for two cones about one sphere: their surfaces are tangent along a line of the joint planes.)
    pub fn grazing(&self, i: usize, th: f64, ao: f64, s_hi: f64) -> bool {
        let step = self.rs / 400.0;
        NB[i].iter().any(|&j| {
            let f = |s: f64| {
                let p = self.point(i, s, th, ao);
                let (sj, perp) = self.axis_coords(j, &p);
                let rho = self.rho(j, sj, ao);
                if rho <= 0.0 {
                    f64::INFINITY
                } else {
                    perp - rho
                }
            };
            let (mut neg, mut pos, mut minabs) = (false, false, f64::INFINITY);
            let mut s = s_hi;
            while s > -3.0 * self.rs {
                let v = f(s);
                if v.is_finite() {
                    neg |= v < 0.0;
                    pos |= v >= 0.0;
                    minabs = minabs.min(v.abs());
                }
                s -= step;
            }
            !(neg && pos) && minabs < 1e-3 * self.rs
        })
    }

    /// Cut of a cone wye leg on the OUTER surface from surface meetings only: of the meetings with each neighbour, the one on
    /// the joint plane `g_i = g_j` (the other meeting lies in `g_i = -g_j`); the larger over the two neighbours. Also returns
    /// the number of meetings found (>= 1 per neighbour is required).
    pub fn cone_cut_from_surfaces(&self, i: usize, th: f64, s_hi: f64) -> Option<f64> {
        let roots = self.surface_roots(i, th, 0.0, 0.0, s_hi);
        let mut best = f64::NEG_INFINITY;
        for &j in &NB[i] {
            let mut found = None;
            for &(jj, s) in &roots {
                if jj != j {
                    continue;
                }
                let p = self.point(i, s, th, 0.0);
                let gap = self.g(i, self.rs, &p) - self.g(j, self.rs, &p);
                if gap.abs() < 1e-7 * self.rs {
                    found = Some(found.map_or(s, |f: f64| f.max(s)));
                }
            }
            // a neighbour with no meeting on its real (front) nappe cuts nothing: the plane point would lie on the cone's
            // mirror nappe. If the engine's plane value for it were the larger one, the comparison with the engine fails.
            if let Some(f) = found {
                best = best.max(f);
            }
        }
        best.is_finite().then_some(best)
    }

    /// `g_i(p) - g_j(p)` of the common-sphere joint plane for the sphere radius `rp`.
    pub fn g(&self, i: usize, rp: f64, p: &V3) -> f64 {
        (dot(p, &self.u[i]) - rp * self.psi[i].sin()) / self.psi[i].cos()
    }

    /// Cut of leg `i` by the joint PLANES of the sphere `rp`, the generator on the surface offset `ao`: root of
    /// `g_i - g_j` along the generator by bisection (monotone in s), the larger of the two neighbours.
    pub fn plane_cut(&self, i: usize, th: f64, ao: f64, rp: f64) -> f64 {
        let mut best = f64::NEG_INFINITY;
        for &j in &NB[i] {
            let f = |s: f64| {
                let p = self.point(i, s, th, ao);
                self.g(i, rp, &p) - self.g(j, rp, &p)
            };
            let (mut lo, mut hi) = (-300.0 * self.rs, 300.0 * self.rs);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if f(mid) < 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            best = best.max(0.5 * (lo + hi));
        }
        best
    }
}

/// Half angle of the cone whose generator passes (axial `sr`, radius `r`) and is tangent to the circle of radius `rs`
/// (the common sphere seen in a half plane): the smaller root of `rs = r cos(psi) + sr sin(psi)`, by bisection scan.
pub fn psi_from(rs: f64, r: f64, sr: f64) -> f64 {
    let h = |p: f64| rs - r * p.cos() - sr * p.sin();
    // h is negative between the two roots; walk up from -1.5 to the first sign change
    let n = 3000;
    let mut a = -1.5;
    let mut ha = h(a);
    for k in 1..=n {
        let b = -1.5 + 3.0 * k as f64 / n as f64;
        let hb = h(b);
        if ha.signum() != hb.signum() {
            let (mut lo, mut hi) = (a, b);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if h(mid).signum() == ha.signum() {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return 0.5 * (lo + hi);
        }
        a = b;
        ha = hb;
    }
    f64::NAN
}

pub struct Lcg(pub u64);
impl Lcg {
    pub fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    pub fn uni(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
    pub fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[(self.next() * v.len() as f64) as usize % v.len()]
    }
}
