//! Affine matrices in the PDF row-vector convention, Bezier flattening with a
//! stated chord bound, and dash patterns applied to polylines.

use crate::limits::{limit, Fault};

/// `[a b c d e f]`: a point `(x, y)` maps to `(a*x + c*y + e, b*x + d*y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct M {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

pub const IDENTITY: M = M {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    e: 0.0,
    f: 0.0,
};

impl M {
    pub fn from_slice(v: &[f64]) -> Option<M> {
        match v {
            [a, b, c, d, e, f] => Some(M {
                a: *a,
                b: *b,
                c: *c,
                d: *d,
                e: *e,
                f: *f,
            }),
            _ => None,
        }
    }

    pub fn translate(tx: f64, ty: f64) -> M {
        M {
            e: tx,
            f: ty,
            ..IDENTITY
        }
    }

    /// `self` applied first, then `n`: for a row vector p, `p * self * n`.
    pub fn then(self, n: M) -> M {
        M {
            a: self.a * n.a + self.b * n.c,
            b: self.a * n.b + self.b * n.d,
            c: self.c * n.a + self.d * n.c,
            d: self.c * n.b + self.d * n.d,
            e: self.e * n.a + self.f * n.c + n.e,
            f: self.e * n.b + self.f * n.d + n.f,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x * self.a + y * self.c + self.e,
            x * self.b + y * self.d + self.f,
        )
    }

    /// Square root of |det|: the length scale of the matrix for uniform scaling.
    pub fn length_scale(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    pub fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .iter()
            .all(|v| v.is_finite())
    }
}

/// Number of pieces used for a cubic Bezier so that the chord error is at most `tol`.
///
/// Let `m = max(|P0 - 2 P1 + P2|, |P1 - 2 P2 + P3|)`. The second derivative of the
/// cubic is bounded by `6 m`, and splitting the parameter range into `n` equal
/// pieces gives a chord error of at most `6 m / 8 * (1/n)^2 = 3 m / (4 n^2)`.
/// The smallest `n` with `3 m / (4 n^2) <= tol` is used.
pub fn cubic_pieces(p: [(f64, f64); 4], tol: f64) -> Result<usize, Fault> {
    let [p0, p1, p2, p3] = p;
    let d1 = (p0.0 - 2.0 * p1.0 + p2.0).hypot(p0.1 - 2.0 * p1.1 + p2.1);
    let d2 = (p1.0 - 2.0 * p2.0 + p3.0).hypot(p1.1 - 2.0 * p2.1 + p3.1);
    let m = d1.max(d2);
    let n = (3.0 * m / (4.0 * tol)).sqrt().ceil();
    if !n.is_finite() || n > 1_000_000.0 {
        return limit(
            "path_segments",
            "a curve needs too many pieces at this tolerance",
        );
    }
    Ok((n as usize).max(1))
}

/// Appends the flattened cubic (excluding its start point, including its end point).
/// Returns the number of pieces appended.
pub fn flatten_cubic(
    p: [(f64, f64); 4],
    tol: f64,
    out: &mut Vec<(f64, f64)>,
) -> Result<usize, Fault> {
    let n = cubic_pieces(p, tol)?;
    let [p0, p1, p2, p3] = p;
    for i in 1..n {
        let t = i as f64 / n as f64;
        let s = 1.0 - t;
        let w0 = s * s * s;
        let w1 = 3.0 * s * s * t;
        let w2 = 3.0 * s * t * t;
        let w3 = t * t * t;
        out.push((
            w0 * p0.0 + w1 * p1.0 + w2 * p2.0 + w3 * p3.0,
            w0 * p0.1 + w1 * p1.1 + w2 * p2.1 + w3 * p3.1,
        ));
    }
    out.push(p3);
    Ok(n)
}

/// Exact point of a cubic at parameter `t` (used by tests to measure flattening error).
#[cfg(test)]
pub fn cubic_at(p: [(f64, f64); 4], t: f64) -> (f64, f64) {
    let [p0, p1, p2, p3] = p;
    let s = 1.0 - t;
    let (w0, w1, w2, w3) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
    (
        w0 * p0.0 + w1 * p1.0 + w2 * p2.0 + w3 * p3.0,
        w0 * p0.1 + w1 * p1.1 + w2 * p2.1 + w3 * p3.1,
    )
}

/// Splits a polyline into the "on" pieces of a dash pattern.
///
/// `pattern` and `phase` are in the same unit as `points`. An odd-length pattern is
/// repeated once, as PDF specifies. The pattern restarts at the start of each
/// subpath. Zero-length pieces are dropped by the caller.
pub fn dash(
    points: &[(f64, f64)],
    closed: bool,
    pattern: &[f64],
    phase: f64,
    max_steps: u64,
) -> Result<Vec<Vec<(f64, f64)>>, Fault> {
    let pat: Vec<f64> = if !pattern.len().is_multiple_of(2) {
        pattern.iter().chain(pattern.iter()).copied().collect()
    } else {
        pattern.to_vec()
    };
    let total: f64 = pat.iter().sum();
    if points.len() < 2 {
        return Ok(Vec::new());
    }
    if pat.is_empty() || total.is_nan() || total <= 0.0 {
        return Ok(vec![points.to_vec()]);
    }
    let mut edges: Vec<((f64, f64), (f64, f64))> = points
        .iter()
        .zip(points.iter().skip(1))
        .map(|(a, b)| (*a, *b))
        .collect();
    if closed {
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            edges.push((*last, *first));
        }
    }
    let mut pieces = Vec::new();
    let mut idx = 0usize;
    let mut rem = pat.first().copied().unwrap_or(0.0);
    let mut left = phase.rem_euclid(total);
    let mut guard = 0usize;
    while left > 0.0 && guard < 4 * pat.len() + 8 {
        guard += 1;
        if rem > left {
            rem -= left;
            left = 0.0;
        } else {
            left -= rem;
            idx = (idx + 1) % pat.len();
            rem = pat.get(idx).copied().unwrap_or(0.0);
        }
    }
    let mut steps: u64 = 0;
    let mut on = idx.is_multiple_of(2);
    let mut cur: Vec<(f64, f64)> = Vec::new();
    if on {
        if let Some(p0) = points.first() {
            cur.push(*p0);
        }
    }
    for (p, q) in edges {
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let len = dx.hypot(dy);
        if len.is_nan() || len <= 0.0 {
            continue;
        }
        let mut pos = 0.0f64;
        while pos < len {
            steps += 1;
            if steps > max_steps {
                return limit("path_segments", "a dash pattern produces too many pieces");
            }
            if rem <= 0.0 {
                if on && cur.len() >= 2 {
                    pieces.push(std::mem::take(&mut cur));
                }
                cur.clear();
                idx = (idx + 1) % pat.len();
                rem = pat.get(idx).copied().unwrap_or(0.0);
                on = idx.is_multiple_of(2);
                if on {
                    let t = pos / len;
                    cur.push((p.0 + dx * t, p.1 + dy * t));
                }
                continue;
            }
            let step = rem.min(len - pos);
            pos += step;
            rem -= step;
            if on {
                let t = (pos / len).min(1.0);
                cur.push((p.0 + dx * t, p.1 + dy * t));
            }
        }
    }
    if on && cur.len() >= 2 {
        pieces.push(cur);
    }
    Ok(pieces)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn dist_to_polyline(pt: (f64, f64), poly: &[(f64, f64)]) -> f64 {
        let mut best = f64::MAX;
        for w in poly.windows(2) {
            if let [a, b] = w {
                let (vx, vy) = (b.0 - a.0, b.1 - a.1);
                let l2 = vx * vx + vy * vy;
                let t = if l2 > 0.0 {
                    (((pt.0 - a.0) * vx + (pt.1 - a.1) * vy) / l2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let d = (pt.0 - (a.0 + vx * t)).hypot(pt.1 - (a.1 + vy * t));
                best = best.min(d);
            }
        }
        best
    }

    #[test]
    fn flattening_stays_within_the_chord_bound() {
        let curves = [
            [(0.0, 0.0), (0.0, 100.0), (100.0, 100.0), (100.0, 0.0)],
            [(0.0, 0.0), (50.0, -80.0), (120.0, 90.0), (200.0, 10.0)],
            [(0.0, 0.0), (1.0, 0.5), (2.0, -0.5), (3.0, 0.0)],
        ];
        for tol in [0.01, 0.1, 1.0] {
            for c in curves {
                let mut poly = vec![c[0]];
                let n = flatten_cubic(c, tol, &mut poly);
                assert!(n.is_ok());
                let mut worst = 0.0f64;
                for k in 0..=2000 {
                    let pt = cubic_at(c, f64::from(k) / 2000.0);
                    worst = worst.max(dist_to_polyline(pt, &poly));
                }
                // The measured error must be inside the stated bound.
                assert!(worst <= tol + 1e-9, "tol {tol}: measured {worst}");
            }
        }
    }

    #[test]
    fn dash_pattern_on_a_line() {
        let pieces =
            dash(&[(0.0, 0.0), (10.0, 0.0)], false, &[2.0, 1.0], 0.0, 1000).unwrap_or_default();
        let ends: Vec<(f64, f64)> = pieces.iter().map(|p| (p[0].0, p[p.len() - 1].0)).collect();
        assert_eq!(ends, vec![(0.0, 2.0), (3.0, 5.0), (6.0, 8.0), (9.0, 10.0)]);
    }

    #[test]
    fn matrix_composition_order() {
        // Scale by 2 first, then translate by (10, 0): (1,1) -> (2,2) -> (12,2).
        let m = M {
            a: 2.0,
            d: 2.0,
            ..IDENTITY
        }
        .then(M::translate(10.0, 0.0));
        assert_eq!(m.apply(1.0, 1.0), (12.0, 2.0));
    }
}
