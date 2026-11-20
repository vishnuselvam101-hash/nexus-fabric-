//! Independent formulation of the developed parts (review RB-006).
//!
//! Nothing here is shared with `develop`: no closed form of the saddle
//! (Spec 02 eq. 4.3), no arcsine chart, no 2·pi·r. Every part is set up as a
//! solid in a deliberately tilted world frame; circles and intersection curves
//! are SAMPLED in 3D; each branch ruling is intersected with the header
//! cylinder by a numerical root search on the distance from the header axis;
//! lengths are sums of 3D chords taken to the limit by Richardson
//! extrapolation; plate coordinates of a 3D point come from its projection on
//! the header axis and its angle about it (atan2).
//!
//! The verifiers in `verify` compare the pattern (and the geometry read back
//! from the DXF file) with these values. What IS shared with the primary
//! code: the inputs, the meaning Specification 02 gives them, the same author
//! and the same crate. A separately owned verifier does not exist.

use crate::TAU;

pub type V3 = [f64; 3];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
pub fn dist(a: V3, b: V3) -> f64 {
    norm(sub(a, b))
}

/// A right-handed orthonormal frame that is aligned with no coordinate axis.
/// ex = (2, -1, 2)/3, ey = (1, 2, 0)/sqrt(5), ez = ex x ey.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub o: V3,
    pub ex: V3,
    pub ey: V3,
    pub ez: V3,
}

impl Frame {
    /// The tilted frame, its origin moved off the coordinate origin by a
    /// fraction of `scale` so that no coordinate of the solid is special.
    pub fn tilted(scale: f64) -> Frame {
        let r5 = 5f64.sqrt();
        let ex = [2.0 / 3.0, -1.0 / 3.0, 2.0 / 3.0];
        let ey = [1.0 / r5, 2.0 / r5, 0.0];
        let ez = [-4.0 / (3.0 * r5), 2.0 / (3.0 * r5), 5.0 / (3.0 * r5)];
        Frame {
            o: [0.7 * scale, -0.4 * scale, 0.3 * scale],
            ex,
            ey,
            ez,
        }
    }
    pub fn at(&self, x: f64, y: f64, z: f64) -> V3 {
        add(
            self.o,
            add(mul(self.ex, x), add(mul(self.ey, y), mul(self.ez, z))),
        )
    }
}

/// Length of a closed or open 3D polyline.
pub fn polyline_length(p: &[V3]) -> f64 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Limit of chord sums: `coarse` is the chord sum at some resolution, `fine`
/// at twice that resolution. Chord sums of a smooth curve converge like h², so
/// one Richardson step removes the leading term. Returns (limit, |correction|);
/// the true error of the limit is far below the correction when the curve is
/// resolved, which the caller checks by the size of the correction itself.
pub fn richardson(coarse: f64, fine: f64) -> (f64, f64) {
    let c = (fine - coarse) / 3.0;
    (fine + c, c.abs())
}

/// Circumference of a circle of radius `r` lying in a tilted plane, from 3D
/// chord sums at `n` and `2n` points (no 2·pi·r). Returns (length, resolution).
pub fn circle_length(r: f64, n: usize) -> (f64, f64) {
    let f = Frame::tilted(r);
    let sum = |m: usize| -> f64 {
        let pts: Vec<V3> = (0..=m)
            .map(|i| {
                let t = TAU * i as f64 / m as f64;
                f.at(0.0, r * t.cos(), r * t.sin())
            })
            .collect();
        polyline_length(&pts)
    };
    let (a, b, c) = (sum(n), sum(2 * n), sum(4 * n));
    let (l1, _) = richardson(a, b);
    let (l2, _) = richardson(b, c);
    // chord sums: error h^2; after one step h^4: a second step with weight 1/15
    let l = l2 + (l2 - l1) / 15.0;
    (l, (l2 - l1).abs())
}

/// Cylinder-on-cylinder joint as a solid in the tilted frame.
/// Header axis = ex through the frame origin (station 0 at the origin). The
/// branch axis passes through `station`·ex + `delta`·ey with direction
/// cos(beta)·ex + sin(beta)·ez; branch azimuth phi is measured from ey toward
/// d x ey (Spec 02 §4.4).
#[derive(Clone, Copy, Debug)]
pub struct Joint3 {
    f: Frame,
    pub big_r: f64,
    pub r: f64,
    pub station: f64,
    d: V3,
    w: V3,
    o: V3,
    sin_beta: f64,
}

impl Joint3 {
    pub fn new(big_r: f64, r: f64, beta: f64, delta: f64, station: f64) -> Joint3 {
        Joint3::in_frame(Frame::tilted(big_r), big_r, r, beta, delta, station)
    }

    /// The same joint in a given frame (two parts of one assembly are compared in one frame).
    pub fn in_frame(f: Frame, big_r: f64, r: f64, beta: f64, delta: f64, station: f64) -> Joint3 {
        let (sb, cb) = beta.sin_cos();
        let d = add(mul(f.ex, cb), mul(f.ez, sb));
        // d x ey = cos(beta)·(ex x ey) + sin(beta)·(ez x ey) = cos(beta)·ez − sin(beta)·ex
        let w = sub(mul(f.ez, cb), mul(f.ex, sb));
        Joint3 {
            f,
            big_r,
            r,
            station,
            d,
            w,
            o: f.at(station, delta, 0.0),
            sin_beta: sb,
        }
    }

    /// Point at distance `s` along the branch axis, azimuth `phi`, radius `rho`.
    pub fn branch_point(&self, phi: f64, s: f64, rho: f64) -> V3 {
        let (sp, cp) = phi.sin_cos();
        add(
            add(self.o, mul(self.d, s)),
            add(mul(self.f.ey, rho * cp), mul(self.w, rho * sp)),
        )
    }

    /// Component of (p − header axis) perpendicular to the header axis.
    fn off_axis(&self, p: V3) -> V3 {
        let q = sub(p, self.f.o);
        sub(q, mul(self.f.ex, dot(q, self.f.ex)))
    }

    pub fn header_axis_distance(&self, p: V3) -> f64 {
        norm(self.off_axis(p))
    }

    pub fn branch_axis_distance(&self, p: V3) -> f64 {
        let q = sub(p, self.o);
        norm(sub(q, mul(self.d, dot(q, self.d))))
    }

    /// Where the ruling of the branch surface (radius `self.r`) at azimuth `phi`
    /// leaves the header cylinder (radius `self.big_r`): the larger root of
    /// "distance from the header axis = R", found numerically. `None` when the
    /// ruling misses the header.
    ///
    /// The distance from the axis along a straight line is convex in s with its
    /// minimum at s0; to the right of the root Newton's iteration decreases
    /// monotonically onto it. A bisection fallback covers anything unexpected.
    pub fn s_at(&self, phi: f64) -> Option<f64> {
        let base = self.branch_point(phi, 0.0, self.r);
        let b = self.off_axis(base);
        let dp = sub(self.d, mul(self.f.ex, dot(self.d, self.f.ex)));
        let dp2 = dot(dp, dp);
        if dp2.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        let f = |s: f64| norm(add(b, mul(dp, s))) - self.big_r;
        let s0 = -dot(b, dp) / dp2;
        if f(s0).partial_cmp(&0.0) != Some(std::cmp::Ordering::Less) {
            return None; // the ruling does not enter the header
        }
        let reach = (self.big_r + norm(add(b, mul(dp, s0)))) / dp2.sqrt();
        let (mut lo, mut hi) = (s0, s0 + 1.01 * reach + f64::MIN_POSITIVE);
        if f(hi).partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        let tol = |s: f64| 2.0 * f64::EPSILON * s.abs().max(self.big_r);
        let mut s = hi;
        for _ in 0..80 {
            let q = add(b, mul(dp, s));
            let nq = norm(q);
            let fv = nq - self.big_r;
            if fv == 0.0 {
                break;
            }
            if fv > 0.0 {
                hi = s;
            } else {
                lo = s;
            }
            let mut next = s - fv * nq / dot(q, dp);
            // keep the iterate inside the bracket; otherwise bisect
            if !(next.is_finite() && next > lo && next < hi) {
                next = 0.5 * (lo + hi);
            }
            let step = (next - s).abs();
            s = next;
            if step <= tol(s) || hi - lo <= tol(s) {
                break;
            }
        }
        Some(s)
    }

    /// Header chart of a 3D point: (station coordinate, angle about the header
    /// axis measured from ez, the branch side, toward ey, the positive-delta side).
    pub fn header_chart(&self, p: V3) -> (f64, f64) {
        let q = sub(p, self.f.o);
        (
            dot(q, self.f.ex),
            dot(q, self.f.ey).atan2(dot(q, self.f.ez)),
        )
    }

    /// Azimuth about the branch axis of a 3D point (measured from ey toward d x ey), in [0, 2·pi).
    pub fn branch_azimuth(&self, p: V3) -> f64 {
        let q = sub(p, self.o);
        dot(q, self.w).atan2(dot(q, self.f.ey)).rem_euclid(TAU)
    }

    /// Point of the intersection curve at azimuth `phi` (on both contact surfaces).
    pub fn curve_point(&self, phi: f64) -> Option<V3> {
        self.s_at(phi).map(|s| self.branch_point(phi, s, self.r))
    }

    /// The header surface point with header chart (x, omega) at radius `rho`.
    pub fn header_point(&self, x: f64, omega: f64, rho: f64) -> V3 {
        self.f.at(x, rho * omega.sin(), rho * omega.cos())
    }

    pub fn sin_beta(&self) -> f64 {
        self.sin_beta
    }
}

/// One-way distance from the points `a` to the polyline `b` (largest over `a`
/// of the distance to the nearest segment of `b`). Both are curves traversed
/// in order, so the nearest segment moves along `b` as the point moves along
/// `a`: the search follows it in a window and falls back to the whole of `b`
/// when the window does not contain a nearby segment. Linear time for curves
/// that match; a few full searches (enough to report a failure) otherwise.
pub fn one_way_distance(a: &[V3], b: &[V3], closed: bool) -> f64 {
    if a.is_empty() || b.len() < 2 {
        return f64::NAN;
    }
    let segs = b.len() - 1;
    let seg_dist = |p: V3, j: usize| -> f64 {
        let (u, v) = (b[j], b[j + 1]);
        let uv = sub(v, u);
        let l2 = dot(uv, uv);
        let t = if l2 > 0.0 {
            (dot(sub(p, u), uv) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        dist(p, add(u, mul(uv, t)))
    };
    let full = |p: V3| -> (f64, usize) {
        (0..segs)
            .map(|j| (seg_dist(p, j), j))
            .fold((f64::MAX, 0), |m, x| if x.0 < m.0 { x } else { m })
    };
    // the nearest segment advances by about (segments of b) / (points of a) per point
    let window = (2 * segs / a.len() + 6) as isize;
    let mut budget_full = 64usize; // full searches allowed after the first point
    let (mut worst, mut at) = full(a[0]);
    let mut prev = worst;
    for &p in &a[1..] {
        let mut best = (f64::MAX, at);
        let mut on_edge = false;
        for k in -window..=window {
            let j = at as isize + k;
            let j = if closed {
                j.rem_euclid(segs as isize) as usize
            } else if j < 0 || j >= segs as isize {
                continue;
            } else {
                j as usize
            };
            let d = seg_dist(p, j);
            if d < best.0 {
                best = (d, j);
                on_edge = k.abs() == window;
            }
        }
        // the window lost the curve (nearest at its edge, or the distance jumped): look everywhere
        let jumped = best.0 > 4.0 * prev && best.0 > 1e-9 * dist(b[0], b[segs / 2]).max(1e-300);
        if (on_edge || jumped) && budget_full > 0 {
            budget_full -= 1;
            let g = full(p);
            if g.0 < best.0 {
                best = g;
            }
        }
        at = best.1;
        prev = best.0;
        worst = if best.0.is_finite() {
            worst.max(best.0)
        } else {
            f64::INFINITY
        };
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::Branch;

    #[test]
    fn tilted_frame_is_orthonormal_and_right_handed() {
        let f = Frame::tilted(1.0);
        for (a, b, want) in [
            (f.ex, f.ex, 1.0),
            (f.ey, f.ey, 1.0),
            (f.ez, f.ez, 1.0),
            (f.ex, f.ey, 0.0),
            (f.ex, f.ez, 0.0),
            (f.ey, f.ez, 0.0),
        ] {
            assert!((dot(a, b) - want).abs() < 1e-15);
        }
        let c = [
            f.ex[1] * f.ey[2] - f.ex[2] * f.ey[1],
            f.ex[2] * f.ey[0] - f.ex[0] * f.ey[2],
            f.ex[0] * f.ey[1] - f.ex[1] * f.ey[0],
        ];
        assert!(dist(c, f.ez) < 1e-15, "ex x ey = ez");
    }

    #[test]
    fn sampled_circle_length_needs_no_formula() {
        for r in [1e-3, 1.0, 150.0, 7.5e4] {
            let (l, res) = circle_length(r, 512);
            assert!((l - TAU * r).abs() <= 1e-11 * r, "{l} for r {r}");
            assert!(res <= 1e-9 * r);
        }
    }

    /// The numerical intersection agrees with the published reference values
    /// (BR-01, BR-03, BR-04) and with the closed form everywhere, to rounding.
    #[test]
    fn numerical_intersection_reproduces_the_reference_cases() {
        let j = Joint3::new(150.0, 100.0, std::f64::consts::FRAC_PI_2, 0.0, 0.0);
        assert!((j.s_at(0.0).unwrap() - 111.803398875).abs() < 1e-9);
        assert!((j.s_at(std::f64::consts::FRAC_PI_4).unwrap() - 132.2875655532).abs() < 1e-9);
        let j = Joint3::new(150.0, 100.0, 60f64.to_radians(), 0.0, 0.0);
        assert!((j.s_at(1.5 * std::f64::consts::PI).unwrap() - 230.9401076759).abs() < 1e-9);
        let j = Joint3::new(150.0, 80.0, std::f64::consts::FRAC_PI_2, 40.0, 0.0);
        assert!((j.s_at(0.75 * std::f64::consts::PI).unwrap() - 149.0821364201).abs() < 1e-9);
        for (big_r, r, beta, delta) in [
            (150.0, 100.0, 60.0f64, 20.0),
            (300.0, 60.0, 35.0, -90.0),
            (150.0, 149.9997, 90.0, 0.0),
            (1000.0, 699.0, 15.0, -300.0),
            (0.0015, 0.001, 120.0, 0.0002),
        ] {
            let b = Branch {
                big_r,
                r,
                beta: beta.to_radians(),
                delta,
            };
            let j = Joint3::new(big_r, r, b.beta, delta, 17.0 * big_r);
            for i in 0..=720 {
                let phi = TAU * i as f64 / 720.0;
                let s = j.s_at(phi).unwrap();
                assert!(
                    (s - b.s(phi)).abs() <= 1e-9 * big_r,
                    "R {big_r} r {r} beta {beta} delta {delta} phi {phi}: {s} against {}",
                    b.s(phi)
                );
                let p = j.curve_point(phi).unwrap();
                assert!((j.header_axis_distance(p) - big_r).abs() <= 1e-12 * big_r);
                assert!((j.branch_axis_distance(p) - r).abs() <= 1e-12 * big_r);
            }
        }
        // a ruling that misses the header is reported, not invented
        assert!(Joint3::new(100.0, 150.0, 1.0, 0.0, 0.0).s_at(0.0).is_none());
    }

    #[test]
    fn one_way_distance_follows_the_curve_and_reports_a_mismatch() {
        let ring = |n: usize, r: f64, shift: f64| -> Vec<V3> {
            (0..=n)
                .map(|i| {
                    let t = TAU * i as f64 / n as f64 + shift;
                    [r * t.cos(), r * t.sin(), 0.3 * r * (2.0 * t).sin()]
                })
                .collect()
        };
        let brute = |a: &[V3], b: &[V3]| -> f64 {
            a.iter()
                .map(|p| {
                    b.windows(2)
                        .map(|w| {
                            let uv = sub(w[1], w[0]);
                            let t = (dot(sub(*p, w[0]), uv) / dot(uv, uv)).clamp(0.0, 1.0);
                            dist(*p, add(w[0], mul(uv, t)))
                        })
                        .fold(f64::MAX, f64::min)
                })
                .fold(0.0, f64::max)
        };
        // same curve, different sampling, start and direction
        let (a, b) = (ring(700, 100.0, 0.0), ring(1300, 100.0, 2.1));
        let mut rev = b.clone();
        rev.reverse();
        for other in [&b, &rev] {
            let (fast, slow) = (one_way_distance(&a, other, true), brute(&a, other));
            assert!((fast - slow).abs() <= 1e-12, "{fast} against {slow}");
            assert!(fast < 1e-2);
        }
        // a different curve is far away, and the search says so
        let far = ring(700, 103.0, 0.0);
        assert!(one_way_distance(&a, &far, true) > 2.9);
        assert!(one_way_distance(&[], &b, true).is_nan());
    }
}
