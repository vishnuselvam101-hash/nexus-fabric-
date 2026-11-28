//! L11 independent recompute of certified cut curves.
//!
//! The digest does not trust the engine's `eps_rep`. For a certified part the job declares the drawing
//! parameters of each curve (radii, wall thickness, offset, mount, placement) and the digest re-derives the
//! plate layout and evaluates the exact curve itself, in closed form, with no code shared with the engine.
//!
//! For every drawn segment `[a, b]` whose end vertices sit at curve parameters `t0 < t1` the bound is
//!
//! `max_q dist(P(q), [a, b]) + max_j K_j * d_j^2 / 8`
//!
//! where `q` runs over `t0`, `t1` and the parameter of every slope kink inside, `d_j` are the parameter
//! lengths of the smooth pieces between them and `K_j >= sup |P''|` on a piece (closed form). The first term
//! is exact at points of the curve (distance to a segment is convex, so the straight chord between two
//! curve points stays within the larger of the two); the second is the linear interpolation error of a C2
//! arc. The bound has to be within the 0.05 mm cap. In addition the curve is sampled densely on every
//! segment and the largest sampled distance must not exceed the declared `eps_rep`.

use crate::reader::P;
use std::f64::consts::PI;

/// Rounding margin (mm) added to every computed distance.
const MARGIN: f64 = 1e-9;
/// Samples of the exact curve per drawn segment.
const SAMPLES: usize = 128;

/// One curve the job defines exactly: which drawn polyline it is, its kind and its drawing parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct ExactDecl {
    /// A hole (CUT_INNER polyline, in file order) or the outline (CUT_OUTER polyline, in file order).
    pub on_hole: bool,
    pub index: usize,
    /// `rect`, `tee_hole`, `tee_cut`, `pad_hole` or `pad_circle`.
    pub kind: String,
    pub params: Vec<(String, f64)>,
}

#[derive(Debug)]
pub struct Outcome {
    /// Proved bound of the distance from the exact curve to the drawn polyline (mm).
    pub bound: f64,
    /// Largest sampled distance (mm).
    pub measured: f64,
}

#[derive(Debug)]
pub struct Problem {
    /// True: the drawn curve is not the declared curve. False: it cannot be judged.
    pub fail: bool,
    pub why: String,
}

fn fail<T>(why: impl Into<String>) -> Result<T, Problem> {
    Err(Problem {
        fail: true,
        why: why.into(),
    })
}

fn inconclusive<T>(why: impl Into<String>) -> Result<T, Problem> {
    Err(Problem {
        fail: false,
        why: why.into(),
    })
}

fn param(d: &ExactDecl, key: &str) -> Result<f64, Problem> {
    match d.params.iter().find(|(k, _)| k == key) {
        Some((_, v)) if v.is_finite() => Ok(*v),
        _ => inconclusive(format!(
            "{}: parameter {key} is missing or not finite",
            d.kind
        )),
    }
}

fn dist_seg(q: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    if l2 <= 0.0 {
        return (q[0] - a[0]).hypot(q[1] - a[1]);
    }
    let t = (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
    (q[0] - (a[0] + t * dx)).hypot(q[1] - (a[1] + t * dy))
}

/// Range of `sin` over `[t0, t1]`.
fn sin_range(t0: f64, t1: f64) -> (f64, f64) {
    let (mut lo, mut hi) = (t0.sin().min(t1.sin()), t0.sin().max(t1.sin()));
    let contains = |c: f64| {
        let k = ((t0 - c) / (2.0 * PI)).ceil();
        c + 2.0 * PI * k <= t1
    };
    if contains(PI / 2.0) {
        hi = 1.0;
    }
    if contains(-PI / 2.0) {
        lo = -1.0;
    }
    (lo, hi)
}

trait Curve {
    fn at(&self, t: f64) -> P;
    /// A bound of `sup |P''|` over `[t0, t1]` (no slope kink inside); None when it is not finite.
    fn k2(&self, t0: f64, t1: f64) -> Option<f64>;
    /// Parameters strictly inside `(t0, t1)` where the slope jumps.
    fn kinks(&self, _t0: f64, _t1: f64) -> Vec<f64> {
        Vec::new()
    }
}

/// `x = xoff + s asin((e + r sin t)/den)`, `y = yoff + r cos t`: a bore (or a circular outline) on a pipe
/// wall, developed on a plate of circumference radius `s`.
struct AsinCircle {
    xoff: f64,
    s: f64,
    den: f64,
    r: f64,
    e: f64,
    yoff: f64,
}

impl AsinCircle {
    fn valid(&self) -> bool {
        [self.xoff, self.s, self.den, self.r, self.e, self.yoff]
            .iter()
            .all(|v| v.is_finite())
            && self.s > 0.0
            && self.den > 0.0
            && self.r > 0.0
    }
}

impl Curve for AsinCircle {
    fn at(&self, t: f64) -> P {
        let a = ((self.e + self.r * t.sin()) / self.den).clamp(-1.0, 1.0);
        [self.xoff + self.s * a.asin(), self.yoff + self.r * t.cos()]
    }
    fn k2(&self, t0: f64, t1: f64) -> Option<f64> {
        let (lo, hi) = sin_range(t0, t1);
        let amax = ((self.e + self.r * lo).abs()).max((self.e + self.r * hi).abs()) / self.den;
        if amax.is_nan() || amax >= 1.0 - 1e-9 {
            return None;
        }
        let q = (1.0 - amax * amax).sqrt();
        let g = self.r / self.den;
        let phi2 = g / q + amax * g * g / (q * q * q);
        Some((self.s * phi2).hypot(self.r))
    }
}

/// Perpendicular branch cut `x = w (t - pi/2)/(2 pi)`, `y = h - sqrt(rc^2 - m^2)` for `t` in `[pi/2, 5 pi/2]`,
/// `m = 0` where the wall straddles the main axis, else the smaller of `|e + ri sin t|`, `|e + ro sin t|`.
struct TeeCut {
    rc: f64,
    ri: f64,
    ro: f64,
    e: f64,
    w: f64,
    h: f64,
}

impl TeeCut {
    fn tau(&self, t: f64) -> f64 {
        let s = t.sin();
        let (a, b) = (self.e + self.ri * s, self.e + self.ro * s);
        let m = if a * b <= 0.0 {
            0.0
        } else {
            a.abs().min(b.abs())
        };
        (self.rc * self.rc - m * m).max(0.0).sqrt()
    }
}

impl Curve for TeeCut {
    fn at(&self, t: f64) -> P {
        [self.w * (t - PI / 2.0) / (2.0 * PI), self.h - self.tau(t)]
    }
    fn k2(&self, t0: f64, t1: f64) -> Option<f64> {
        let (lo, hi) = sin_range(t0, t1);
        let mmax = [self.ri, self.ro]
            .iter()
            .flat_map(|r| [(self.e + r * lo).abs(), (self.e + r * hi).abs()])
            .fold(0.0_f64, f64::max);
        if mmax.is_nan() || mmax >= self.rc * (1.0 - 1e-9) {
            return None;
        }
        let tmin = (self.rc * self.rc - mmax * mmax).sqrt();
        let rho = self.ro;
        Some((rho * rho + mmax * rho) / tmin + mmax * mmax * rho * rho / (tmin * tmin * tmin))
    }
    fn kinks(&self, t0: f64, t1: f64) -> Vec<f64> {
        let mut out = Vec::new();
        let cands = [
            0.0,
            -self.e / self.ri,
            -self.e / self.ro,
            -2.0 * self.e / (self.ri + self.ro),
        ];
        for s in cands {
            if s.is_nan() || s.abs() > 1.0 {
                continue;
            }
            for base in [s.asin(), PI - s.asin()] {
                let k0 = ((t0 - base) / (2.0 * PI)).floor() as i64 - 1;
                let k1 = ((t1 - base) / (2.0 * PI)).ceil() as i64 + 1;
                for k in k0..=k1 {
                    let t = base + 2.0 * PI * k as f64;
                    if t > t0 && t < t1 {
                        out.push(t);
                    }
                }
            }
        }
        out.sort_by(|a, b| a.total_cmp(b));
        out.dedup();
        out
    }
}

/// Bound and sampled deviation of the drawn segments `verts[k] -> verts[k+1]` against the curve, vertex `k`
/// being at parameter `ts[k]` (already unwrapped; `ts` has one more entry than there are segments).
fn judge(c: &dyn Curve, verts: &[P], ts: &[f64], eps: f64) -> Result<Outcome, Problem> {
    let (mut bound, mut measured) = (0.0_f64, 0.0_f64);
    for k in 0..ts.len() - 1 {
        let (a, b) = (verts[k], verts[(k + 1) % verts.len()]);
        let (lo, hi) = (ts[k].min(ts[k + 1]), ts[k].max(ts[k + 1]));
        let mut q = vec![lo];
        q.extend(c.kinks(lo, hi));
        q.push(hi);
        let mut dmax = 0.0_f64;
        let mut kd = 0.0_f64;
        for (j, &p) in q.iter().enumerate() {
            dmax = dmax.max(dist_seg(c.at(p), a, b));
            if j + 1 < q.len() {
                let Some(kk) = c.k2(p, q[j + 1]) else {
                    return inconclusive(
                        "the second derivative of the exact curve is not bounded on a segment",
                    );
                };
                kd = kd.max(kk * (q[j + 1] - p) * (q[j + 1] - p) / 8.0);
            }
        }
        bound = bound.max(dmax + kd + MARGIN);
        for i in 0..=SAMPLES {
            let t = lo + (hi - lo) * i as f64 / SAMPLES as f64;
            measured = measured.max(dist_seg(c.at(t), a, b));
        }
    }
    if !(bound.is_finite() && measured.is_finite()) {
        return inconclusive("the recomputed deviation is not finite");
    }
    if bound > crate::EPS_REP_CAP_MM {
        return fail(format!(
            "the exact curve recomputed from the drawing parameters leaves the drawn polyline by up to {bound:.6} mm, over the {:.2} mm cap",
            crate::EPS_REP_CAP_MM
        ));
    }
    if measured > eps + MARGIN {
        return fail(format!(
            "the exact curve is {measured:.6} mm from the drawn polyline; the declared bound is {eps:.6} mm"
        ));
    }
    Ok(Outcome { bound, measured })
}

/// Parameter of the curve point nearest to `v` (closed curve, `t` in `[0, 2 pi)`).
fn locate(c: &dyn Curve, v: P) -> f64 {
    let n = 8192usize;
    let dt = 2.0 * PI / n as f64;
    let d = |t: f64| {
        let p = c.at(t);
        (p[0] - v[0]).hypot(p[1] - v[1])
    };
    let mut best = 0usize;
    let mut bd = f64::INFINITY;
    for i in 0..n {
        let di = d(dt * i as f64);
        if di < bd {
            bd = di;
            best = i;
        }
    }
    let (mut a, mut b) = (dt * (best as f64 - 1.0), dt * (best as f64 + 1.0));
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut x1, mut x2) = (b - g * (b - a), a + g * (b - a));
    let (mut f1, mut f2) = (d(x1), d(x2));
    for _ in 0..80 {
        if f1 < f2 {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = d(x1);
        } else {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = d(x2);
        }
    }
    (0.5 * (a + b)).rem_euclid(2.0 * PI)
}

/// A closed polyline drawn on a closed curve: every vertex is a curve vertex and the vertices go once round.
fn closed_curve(c: &dyn Curve, pts: &[P], eps: f64) -> Result<Outcome, Problem> {
    if pts.len() < 3 {
        return fail("the drawn polyline has fewer than three vertices");
    }
    let t0: Vec<f64> = pts.iter().map(|v| locate(c, *v)).collect();
    let n = t0.len();
    let wrap = |d: f64| {
        let m = d.rem_euclid(2.0 * PI);
        if m > PI { m - 2.0 * PI } else { m }
    };
    let deltas: Vec<f64> = (0..n).map(|k| wrap(t0[(k + 1) % n] - t0[k])).collect();
    let sign = if deltas.iter().all(|d| *d > 0.0) {
        1.0
    } else if deltas.iter().all(|d| *d < 0.0) {
        -1.0
    } else {
        return fail("the drawn vertices do not follow the exact curve in order");
    };
    let total: f64 = deltas.iter().sum();
    if (total.abs() - 2.0 * PI).abs() > 1e-6 {
        return fail("the drawn vertices do not go once round the exact curve");
    }
    let _ = sign;
    let mut ts = vec![t0[0]];
    for d in &deltas {
        let last = *ts.last().unwrap_or(&0.0);
        ts.push(last + d);
    }
    judge(c, pts, &ts, eps)
}

fn near(a: P, b: P) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) <= 1e-7
}

/// Check one declared curve against its drawn polyline.
pub fn check_curve(d: &ExactDecl, pts: &[P], eps: f64) -> Result<Outcome, Problem> {
    match d.kind.as_str() {
        "rect" => {
            let (main_d, main_t) = (param(d, "main_d")?, param(d, "main_t")?);
            let (seams, l) = (param(d, "seams")?, param(d, "l")?);
            if !(main_d > 2.0 * main_t && main_t > 0.0 && seams >= 1.0 && l > 0.0) {
                return inconclusive("rect: parameters out of range");
            }
            let w = 2.0 * PI * (main_d / 2.0 - main_t / 2.0) / seams;
            let corners = [[0.0, 0.0], [w, 0.0], [w, l], [0.0, l]];
            if pts.len() != 4 || !corners.iter().all(|c| pts.iter().any(|p| near(*p, *c))) {
                return fail(format!(
                    "the plate outline is not the {w:.6} x {l:.6} rectangle the drawing gives"
                ));
            }
            Ok(Outcome {
                bound: 0.0,
                measured: 0.0,
            })
        }
        "tee_hole" => {
            let (main_d, main_t) = (param(d, "main_d")?, param(d, "main_t")?);
            let (bd, bt) = (param(d, "branch_d")?, param(d, "branch_t")?);
            let set_in = param(d, "set_in")? != 0.0;
            let ro = main_d / 2.0;
            let (ri, rm) = (ro - main_t, ro - main_t / 2.0);
            let rh = if set_in { bd / 2.0 } else { bd / 2.0 - bt };
            let c = AsinCircle {
                xoff: rm * param(d, "plate_dir")?,
                s: rm,
                den: ri,
                r: rh,
                e: param(d, "e")?,
                yoff: param(d, "x0")?,
            };
            if !c.valid() {
                return inconclusive("tee_hole: parameters out of range");
            }
            closed_curve(&c, pts, eps)
        }
        "pad_hole" | "pad_circle" => {
            let (main_d, t) = (param(d, "main_d")?, param(d, "t")?);
            let (bd, e) = (param(d, "branch_d")?, param(d, "e")?);
            let rp = main_d / 2.0 + t / 2.0;
            let (den, r) = if d.kind == "pad_hole" {
                (main_d / 2.0, bd / 2.0)
            } else {
                (main_d / 2.0 + t, param(d, "dp")? / 2.0)
            };
            let c = AsinCircle {
                xoff: 0.0,
                s: rp,
                den,
                r,
                e,
                yoff: 0.0,
            };
            if !c.valid() {
                return inconclusive("pad: parameters out of range");
            }
            closed_curve(&c, pts, eps)
        }
        "tee_cut" => {
            let (main_d, main_t) = (param(d, "main_d")?, param(d, "main_t")?);
            let (bd, bt) = (param(d, "branch_d")?, param(d, "branch_t")?);
            let set_in = param(d, "set_in")? != 0.0;
            let (e, h) = (param(d, "e")?, param(d, "h")?);
            let ro_main = main_d / 2.0;
            let ri_main = ro_main - main_t;
            let (ro, ri) = (bd / 2.0, bd / 2.0 - bt);
            let rc = if set_in { ri_main } else { ro_main };
            let w = 2.0 * PI * (ro - bt / 2.0);
            if !(ri > 0.0 && rc > 0.0 && w > 0.0 && h > 0.0) {
                return inconclusive("tee_cut: parameters out of range");
            }
            let c = TeeCut {
                rc,
                ri,
                ro,
                e,
                w,
                h,
            };
            let n = pts.len();
            if n < 5 {
                return fail("the drawn branch outline has too few vertices");
            }
            let (o, wv) = ([0.0, 0.0], [w, 0.0]);
            // the frame is the two vertices (0, 0) and (w, 0); the other vertices are the drawn cut line
            let Some(j) = (0..n).find(|j| {
                near(pts[*j], o) && (near(pts[(j + 1) % n], wv) || near(pts[(j + n - 1) % n], wv))
            }) else {
                return fail(
                    "the branch outline lacks the frame vertices (0, 0) and (circumference, 0)",
                );
            };
            let forward = near(pts[(j + 1) % n], wv);
            let chain: Vec<P> = if forward {
                (2..n).map(|i| pts[(j + i) % n]).collect()
            } else {
                (1..n - 1).map(|i| pts[(j + i) % n]).collect()
            };
            let ts: Vec<f64> = chain
                .iter()
                .map(|v| PI / 2.0 + 2.0 * PI * v[0] / w)
                .collect();
            let (first, last) = (chain[0], chain[chain.len() - 1]);
            let ends_ok = if forward {
                (first[0] - w).abs() <= 1e-7 && last[0].abs() <= 1e-7
            } else {
                first[0].abs() <= 1e-7 && (last[0] - w).abs() <= 1e-7
            };
            if !ends_ok {
                return fail(
                    "the cut line does not run from one side edge of the branch to the other",
                );
            }
            let up = ts.windows(2).all(|p| p[1] > p[0]);
            let down = ts.windows(2).all(|p| p[1] < p[0]);
            if !(up || down) {
                return fail("the cut line vertices are not in order round the branch");
            }
            judge(&c, &chain, &ts, eps)
        }
        other => inconclusive(format!("no independent definition for curve kind {other}")),
    }
}
