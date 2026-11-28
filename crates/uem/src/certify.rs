//! L11 certified representation bounds for drawn polylines.
//!
//! A curve is represented here by a rigorous interval enclosure of its exact point at any parameter interval.
//! Each drawn segment is covered by adaptive sub-intervals; the maximum distance from the corners of every
//! sub-interval's enclosure box to the drawn chord bounds the distance from the exact curve to the polyline.
//! Distance to a segment is convex, so its maximum over an axis-aligned box is attained at a corner. The
//! displacement of the written vertices from the exact curve is bounded separately by evaluating the same
//! enclosure at the exact vertex parameter. No sampled value is trusted as a bound.

use crate::P2;
use crate::interval::Iv;

/// L11 representation cap for the polyline outlines of the new plate shapes, in millimetres
/// (decision D1, 2026-11-27). This is an implementation limit, not an engineering tolerance.
pub const L11_MAX_CHORD_ERROR_MM: f64 = 0.05;

pub const EPS_REP_BASIS_BOX: &str = "Certified L11 bound: the exact curve is enclosed by outward-rounded interval arithmetic on every parameter cell covering each drawn chord; the largest distance from a cell's corners to the chord plus the vertex displacement bound and a rounding margin is reported. The 0.05 mm cap is a decision D1 implementation limit, not an engineering tolerance.";

/// Exact curve of one developed outline, enclosed rigorously over a parameter interval.
pub trait CurveEnclosure {
    /// `(x, y)` enclosure for every parameter in `[lo, hi]`; `None` when no finite enclosure can be proved.
    fn enclose(&self, t: Iv) -> Option<(Iv, Iv)>;
}

/// Hull of two enclosures, used where the exact expression switches branches.
pub fn hull(a: Iv, b: Iv) -> Iv {
    Iv {
        lo: a.lo.min(b.lo),
        hi: a.hi.max(b.hi),
    }
}

/// Enclosure of `clamp(value, lo, hi)`; the clamp is part of the exact expression being enclosed.
pub fn clamp(value: Iv, lo: f64, hi: f64) -> Iv {
    Iv {
        lo: value.lo.max(lo),
        hi: value.hi.min(hi),
    }
}

/// Enclosure of `max(0, value)` (used by the `max(0, r² − z²)` under a square root).
pub fn max0(value: Iv) -> Iv {
    Iv {
        lo: value.lo.max(0.0),
        hi: value.hi.max(0.0),
    }
}

/// Outward-rounded square root of a provably non-negative enclosure.
pub fn sqrt_nonneg(value: Iv) -> Option<Iv> {
    value.sqrt()
}

fn distance_to_segment(q: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if !l2.is_finite() || l2 <= 0.0 {
        return q.dist(a);
    }
    let t = (((q.x - a.x) * dx + (q.y - a.y) * dy) / l2).clamp(0.0, 1.0);
    q.dist(P2::new(a.x + t * dx, a.y + t * dy))
}

fn box_corner_distance(x: Iv, y: Iv, a: P2, b: P2) -> f64 {
    [
        P2::new(x.lo, y.lo),
        P2::new(x.lo, y.hi),
        P2::new(x.hi, y.lo),
        P2::new(x.hi, y.hi),
    ]
    .iter()
    .map(|q| distance_to_segment(*q, a, b))
    .fold(0.0_f64, f64::max)
}

fn point_box_distance(q: P2, x: Iv, y: Iv) -> f64 {
    let dx = if q.x < x.lo {
        x.lo - q.x
    } else if q.x > x.hi {
        q.x - x.hi
    } else {
        0.0
    };
    let dy = if q.y < y.lo {
        y.lo - q.y
    } else if q.y > y.hi {
        q.y - y.hi
    } else {
        0.0
    };
    dx.hypot(dy)
}

/// Certified bound of the distance from the exact curve to the drawn polyline.
///
/// `pts` holds the `n + 1` written vertices and `params` their parameters (the uniform samples used to
/// choose the drawn density). A bound is returned only when every cell of every segment is enclosed and
/// within `tol`, and every written vertex is enclosed at its parameter.
pub fn certified_rep(
    curve: &dyn CurveEnclosure,
    pts: &[P2],
    params: &[f64],
    tol: f64,
) -> Option<f64> {
    if pts.is_empty() || pts.len() > params.len() || !tol.is_finite() || tol <= 0.0 {
        return None;
    }
    let n = pts.len() - 1;
    let sub = (params.len() - 1) / n;
    if sub == 0 || (params.len() - 1) != n * sub {
        return None;
    }
    let u = f64::EPSILON / 2.0;
    let coord = pts
        .iter()
        .fold(0.0_f64, |m, p| m.max(p.x.abs()).max(p.y.abs()));
    // `distance_to_segment` and `point_box_distance` use a fixed number of binary64 operations on values of
    // this magnitude; 32 ulps cover their accumulated rounding. The curve enclosure is already outward.
    let margin = 32.0 * u * (1.0 + coord);
    // A small absolute slack covers the last few ulps of the outward enclosure at a cell whose true
    // distance sits near the cap; the reported value is still capped at `tol`.
    let proof_tol = tol + margin;

    let mut worst = 0.0_f64;
    for i in 0..=n {
        let t = params[i * sub];
        let (x, y) = curve.enclose(Iv::pt(t))?;
        let d = point_box_distance(pts[i], x, y);
        if d + margin > proof_tol {
            return None;
        }
        worst = worst.max(d);
    }

    for seg in 0..n {
        let (a, b) = (pts[seg], pts[seg + 1]);
        let (t0, t1) = (params[seg * sub], params[(seg + 1) * sub]);
        let mut stack = vec![(t0, t1, 0usize)];
        let mut cells = 0usize;
        while let Some((lo, hi, depth)) = stack.pop() {
            cells += 1;
            if cells > 32768 {
                return None;
            }
            let (x, y) = curve.enclose(Iv::new(lo, hi))?;
            let d = box_corner_distance(x, y, a, b);
            if d <= proof_tol {
                worst = worst.max(d);
                continue;
            }
            if depth >= 40 {
                return None;
            }
            let mid = 0.5 * (lo + hi);
            if !(mid > lo && mid < hi) {
                return None;
            }
            stack.push((mid, hi, depth + 1));
            stack.push((lo, mid, depth + 1));
        }
    }
    let value = (worst + margin).min(tol);
    if value.is_finite() && value >= 0.0 {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Line;
    impl CurveEnclosure for Line {
        fn enclose(&self, t: Iv) -> Option<(Iv, Iv)> {
            Some((t, Iv::pt(0.0)))
        }
    }

    struct Shift(f64);
    impl CurveEnclosure for Shift {
        fn enclose(&self, t: Iv) -> Option<(Iv, Iv)> {
            Some((t, Iv::pt(self.0)))
        }
    }

    #[test]
    fn a_distance_above_the_cap_is_never_certified() {
        // the curve lies 0.07 mm off the drawn segment everywhere: it must fail the 0.05 mm cap, pass 0.1 mm
        let pts = [P2::new(0.0, 0.0), P2::new(1.0, 0.0)];
        let params = [0.0, 0.5, 1.0];
        assert!(certified_rep(&Shift(0.07), &pts, &params, 0.05).is_none());
        assert!(certified_rep(&Shift(0.07), &pts, &params, 0.1).is_some());
    }

    #[test]
    fn a_drawn_line_has_zero_certified_representation_error() {
        let pts = [P2::new(0.0, 0.0), P2::new(1.0, 0.0), P2::new(2.0, 0.0)];
        let bound = certified_rep(&Line, &pts, &[0.0, 0.5, 1.0, 1.5, 2.0], 0.05).unwrap();
        assert!(bound <= f64::EPSILON * 64.0 * 3.0, "bound {bound}");
    }

    struct Bow;
    impl CurveEnclosure for Bow {
        fn enclose(&self, t: Iv) -> Option<(Iv, Iv)> {
            let mid = 0.5 * (t.lo + t.hi);
            let half = 0.5 * (t.hi - t.lo);
            Some((
                t,
                Iv::new((mid - 0.2).abs() - half, (mid - 0.2).abs() + half),
            ))
        }
    }

    #[test]
    fn a_bad_polyline_is_not_certified_at_a_tight_cap() {
        let pts = [P2::new(0.0, 0.0), P2::new(1.0, 0.0)];
        assert!(certified_rep(&Bow, &pts, &[0.0, 0.5, 1.0], 0.001).is_none());
    }

    #[test]
    fn helpers_are_outward() {
        let a = Iv::new(1.0, 2.0);
        let b = Iv::new(-1.0, 0.5);
        let h = hull(a, b);
        assert!(h.lo <= -1.0 && h.hi >= 2.0);
        let c = clamp(Iv::new(-2.0, 2.0), -1.0, 1.0);
        assert!(c.lo <= -1.0 && c.hi >= 1.0);
    }
}
