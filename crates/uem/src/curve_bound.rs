//! Certified representation error of the polylines of the surface developments (work plan task 1.2,
//! docs/backend/TOLERANCE_POLICY.md section 8). The engine draws each curved edge as a polyline through
//! points of the exact curve c(t). The bound below is an upper bound of the distance from every point of
//! the exact curve to its polyline, computed by interval arithmetic (`interval::Iv`) on the closed-form
//! derivatives of the family. No sampled value is trusted as a bound; the samples only enter as brackets
//! that are themselves checked by the argument of `chord_error_bound`.

use crate::P2;
use crate::geometry::{Branch, ObliqueCone};
use crate::interval::Iv;

/// The exact curve of one family, as far as the chord bound needs it. `t` is the parameter of the
/// uniform sampling (the stub's and the opening's azimuth phi, the cone's angle theta).
pub trait CurveBound {
    /// An upper bound of |c''(t)| over the closed parameter interval [lo, hi] (None if none can be proved).
    fn sup_c2(&self, lo: f64, hi: f64) -> Option<f64>;
    /// Enclosures of the components of c'(t) over [lo, hi] for the Cartesian families (None if not available).
    fn d1(&self, _lo: f64, _hi: f64) -> Option<(Iv, Iv)> {
        None
    }
}

/// s(phi) and its first two derivatives, Branch::s: s = (sqrt(R² − y²) − r·sin φ·cos β)/sin β, y = δ + r·cos φ.
/// Returns (s', s'') as enclosures over `phi`; None if R² − y² is not provably positive.
pub fn saddle_derivatives(b: &Branch, phi: Iv) -> Option<(Iv, Iv)> {
    let r = Iv::pt(b.r);
    let big = Iv::pt(b.big_r);
    let (sb, cb) = (Iv::pt(b.beta).sin(), Iv::pt(b.beta).cos());
    let (sp, cp) = (phi.sin(), phi.cos());
    let y = Iv::pt(b.delta).add(r.mul(cp));
    let yp = r.mul(sp).neg(); // y' = −r sin φ
    let ypp = r.mul(cp).neg(); // y'' = −r cos φ
    let q2 = big.sqr().sub(y.sqr());
    if q2.lo <= 0.0 {
        return None;
    }
    let q = q2.sqrt()?; // q = sqrt(R² − y²)
    let qp = y.mul(yp).neg().div(q)?; // q' = −y y' / q
    let qpp = yp.sqr().add(y.mul(ypp)).add(qp.sqr()).neg().div(q)?; // q'' = −(y'² + y y'' + q'²) / q
    // s = (q − p)/sin β with p = r sin φ cos β
    let p1 = r.mul(cp).mul(cb); // p'
    let p2 = r.mul(sp).mul(cb).neg(); // p''
    let s1 = qp.sub(p1).div(sb)?;
    let s2 = qpp.sub(p2).div(sb)?;
    Some((s1, s2))
}

/// The branch saddle cut: the polyline of (r_dev·φ, s(φ)). Its exact curve has c' = (r_dev, s'), c'' = (0, s'').
pub struct SaddleCurve<'a> {
    pub b: &'a Branch,
    pub r_dev: f64,
}

impl CurveBound for SaddleCurve<'_> {
    fn sup_c2(&self, lo: f64, hi: f64) -> Option<f64> {
        let (_, s2) = saddle_derivatives(self.b, Iv::new(lo, hi))?;
        Some(s2.max_abs())
    }
    fn d1(&self, lo: f64, hi: f64) -> Option<(Iv, Iv)> {
        let (s1, _) = saddle_derivatives(self.b, Iv::new(lo, hi))?;
        Some((Iv::pt(self.r_dev), s1))
    }
}

/// The header opening: x = station + s cos β − r sin φ sin β, y = R_dev·(ω + π), ω = asin((δ + r cos φ)/R).
/// With q = sqrt(R² − Y²), Y = δ + r cos φ: ω' = −r sin φ / q and ω'' = −r cos φ / q + r² Y sin²φ / q³.
pub struct OpeningCurve<'a> {
    pub b: &'a Branch,
    pub r_dev_header: f64,
}

impl CurveBound for OpeningCurve<'_> {
    fn sup_c2(&self, lo: f64, hi: f64) -> Option<f64> {
        let (_, x2, _, y2) = self.derivatives(lo, hi)?;
        Some(x2.sqr().add(y2.sqr()).sqrt()?.hi)
    }
    fn d1(&self, lo: f64, hi: f64) -> Option<(Iv, Iv)> {
        let (x1, _, y1, _) = self.derivatives(lo, hi)?;
        Some((x1, y1))
    }
}

impl OpeningCurve<'_> {
    /// (x', x'', y', y'') of the opening over [lo, hi].
    fn derivatives(&self, lo: f64, hi: f64) -> Option<(Iv, Iv, Iv, Iv)> {
        let b = self.b;
        let phi = Iv::new(lo, hi);
        let (s1, s2) = saddle_derivatives(b, phi)?;
        let (sb, cb) = (Iv::pt(b.beta).sin(), Iv::pt(b.beta).cos());
        let (sp, cp) = (phi.sin(), phi.cos());
        let r = Iv::pt(b.r);
        let big = Iv::pt(b.big_r);
        let y = Iv::pt(b.delta).add(r.mul(cp));
        let q = big.sqr().sub(y.sqr()).sqrt()?;
        let x1 = s1.mul(cb).sub(r.mul(cp).mul(sb));
        let x2 = s2.mul(cb).add(r.mul(sp).mul(sb));
        let w1 = r.mul(sp).neg().div(q)?; // ω'
        let w2 = r
            .mul(cp)
            .neg()
            .div(q)?
            .add(r.sqr().mul(y).mul(sp.sqr()).div(q.mul(q).mul(q))?); // ω''
        let rd = Iv::pt(self.r_dev_header);
        Some((x1, x2, rd.mul(w1), rd.mul(w2)))
    }
}

/// The oblique cone (eccentric reducer), in its polar form about the apex: rho(θ) = sqrt(G(θ)),
/// G = (r1 − a)² + h'² + 4 a r1 sin²(θ/2), and psi' = D(θ) = r1·H/G with H = sqrt(h'² + X²), X = r1 − a cos θ.
/// The curve c = rho·(cos psi, sin psi) has |c'|² = rho'² + rho² D² and c'' = (rho'' − rho D²) e_rho + (2 rho' D + rho D') e_psi.
pub struct ConeCurve<'a> {
    pub g: &'a ObliqueCone,
}

impl ConeCurve<'_> {
    /// (rho, rho', rho'', D, D') over [lo, hi] (the apex is the binary64 apex of the code).
    fn polar(&self, lo: f64, hi: f64) -> Option<(Iv, Iv, Iv, Iv, Iv)> {
        let g = self.g;
        let (a, hh) = g.apex();
        let (r1, a_iv, hh_iv) = (Iv::pt(g.r1), Iv::pt(a), Iv::pt(hh));
        let th = Iv::new(lo, hi);
        let s = th.mul(Iv::pt(0.5)).sin();
        let c0 = Iv::pt(g.r1 - a).sqr().add(hh_iv.sqr());
        let k4 = a_iv.mul(r1).mul(Iv::pt(4.0));
        let gg = c0.add(k4.mul(s.sqr())); // G
        let g1 = a_iv.mul(r1).mul(Iv::pt(2.0)).mul(th.sin()); // G'
        let g2 = a_iv.mul(r1).mul(Iv::pt(2.0)).mul(th.cos()); // G''
        let rho = gg.sqrt()?;
        let rho1 = g1.div(rho.mul(Iv::pt(2.0)))?; // rho' = G' / (2 rho)
        let rho2 = g2
            .div(rho.mul(Iv::pt(2.0)))?
            .sub(g1.sqr().div(rho.mul(rho).mul(rho).mul(Iv::pt(4.0)))?); // rho'' = G''/(2 rho) − G'²/(4 rho³)
        let x = r1.sub(a_iv.mul(th.cos())); // X = r1 − a cos θ
        let x1 = a_iv.mul(th.sin()); // X'
        let h = hh_iv.sqr().add(x.sqr()).sqrt()?; // H = sqrt(h² + X²)
        let h1 = x.mul(x1).div(h)?; // H' = X X' / H
        let d = r1.mul(h).div(gg)?; // D = psi' = r1 H / G
        let d1 = r1.mul(h1.mul(gg).sub(h.mul(g1))).div(gg.sqr())?; // D' = r1 (H' G − H G') / G²
        Some((rho, rho1, rho2, d, d1))
    }
}

impl CurveBound for ConeCurve<'_> {
    fn sup_c2(&self, lo: f64, hi: f64) -> Option<f64> {
        let (rho, rho1, rho2, d, d1) = self.polar(lo, hi)?;
        let e_r = rho2.sub(rho.mul(d.sqr()));
        let e_t = rho1.mul(d).mul(Iv::pt(2.0)).add(rho.mul(d1));
        Some(e_r.sqr().add(e_t.sqr()).sqrt()?.hi)
    }
}

/// Certified upper bound of the total excess of the polyline's length over the length of the exact curve,
/// Σ over the segments of (arc length − length of the drawn chord) (work plan task 1.2 item C).
///
/// For any unit vector u, arc − |chord| ≤ ∫ (|c'(t)| − c'(t)·u) dt, because |chord| ≥ chord·u = ∫ c'·u.
/// With c'_⊥ the component of c' across u, |c'| − c'·u = |c'_⊥|² / (|c'| + c'·u), so each sub-interval of width h
/// contributes at most h·sup|c'_⊥|² / inf(|c'| + c'·u), with both bounds by interval arithmetic. u is the direction
/// of the DRAWN chord, which is allowed because the inequality holds for every unit u. `drawn` holds the n + 1
/// polyline vertices and `params` the n·sub + 1 parameters of the uniform sub-sampling (segment i covers
/// params[i·sub ..= (i+1)·sub]). Returns None where the denominator is not provably positive.
pub fn length_excess_bound(
    curve: &dyn CurveBound,
    drawn: &[P2],
    params: &[f64],
    n: usize,
    sub: usize,
) -> Option<f64> {
    if drawn.len() != n + 1 || params.len() != n * sub + 1 || n == 0 || sub == 0 {
        return None;
    }
    let mut total = 0.0_f64;
    for seg in 0..n {
        let (ia, ib) = (seg * sub, (seg + 1) * sub);
        let (a, b) = (drawn[seg], drawn[seg + 1]);
        let d = (b.x - a.x).hypot(b.y - a.y);
        let (ux, uy) = if d > 0.0 && d.is_finite() {
            ((b.x - a.x) / d, (b.y - a.y) / d)
        } else {
            (1.0, 0.0)
        };
        let (uxi, uyi) = (Iv::pt(ux), Iv::pt(uy));
        for k in ia..ib {
            // Taylor expansion of c' about the midpoint m of the sub-interval: c'(t) = c'(m) + r(t) with
            // |r(t)| <= M·h/2, M = sup|c''| over the sub-interval (a bound by sup_c2). The midpoint enclosure stays
            // narrow, which is what keeps the bound close to the true excess (a box enclosure of c' doubles it).
            let (t0, t1) = (params[k], params[k + 1]);
            let h = t1 - t0;
            let mid = 0.5 * (t0 + t1);
            let (xm, ym) = curve.d1(mid, mid)?;
            let m_bound = curve.sup_c2(t0, t1)?;
            if !m_bound.is_finite() || !h.is_finite() || h < 0.0 {
                return None;
            }
            // sup|c'_⊥| <= |c'(m)_⊥| + M h/2, with c'_⊥ the component across u
            let perp = xm.mul(uyi).sub(ym.mul(uxi)).max_abs() + 0.5 * m_bound * h;
            // inf(|c'| + c'·u) >= (|c'(m)| + c'(m)·u) − M h, since each term moves by at most M h/2
            let speed = xm.sqr().add(ym.sqr()).sqrt()?;
            let along = xm.mul(uxi).add(ym.mul(uyi));
            let denom = speed.lo + along.lo - m_bound * h;
            // sup(|c'| − c'·u) <= sup|c'| − inf(c'·u) <= |c'(m)| − c'(m)·u + M h
            let crude = speed.hi - along.lo + m_bound * h;
            // two valid bounds of the integrand g = |c'| − c'·u on the sub-interval: the fraction P²/denominator
            // where the denominator is provably positive, and the crude one, valid everywhere (it is needed where
            // the curve turns against the chord inside one sub-interval, e.g. at the tip of a saddle near 180 deg)
            let fine = if denom > 0.0 {
                perp * perp / denom
            } else {
                f64::INFINITY
            };
            let g = fine.min(crude);
            if !g.is_finite() || !perp.is_finite() || g < 0.0 {
                return None;
            }
            total += h * g * (1.0 + 8.0 * (f64::EPSILON / 2.0));
        }
    }
    if total.is_finite() { Some(total) } else { None }
}

/// Distance from q to the closed segment [a, b].
fn distance_to_segment(q: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if !l2.is_finite() || l2 <= 0.0 {
        return q.dist(a);
    }
    let t = (((q.x - a.x) * dx + (q.y - a.y) * dy) / l2).clamp(0.0, 1.0);
    q.dist(P2::new(a.x + t * dx, a.y + t * dy))
}

/// Certified bound of the distance from every point of the exact curve to its polyline.
///
/// `pts[k]` and `params[k]` (k = 0..=n·sub) are the sampled points and their curve parameters; the polyline
/// vertices are the points with k = i·sub, so segment i is the chord S_i from pts[i·sub] to pts[(i+1)·sub].
///
/// Argument. Let t lie in the sub-interval [t_k, t_{k+1}] of width h_k, with M_k ≥ |c''| on it. Let L(t) be the
/// point of the sub-chord from c(t_k) to c(t_{k+1}) at the same fraction of the interval. Then
/// 1. |c(t) − L(t)| ≤ M_k·h_k²/8 (for each t, the error of linear interpolation of a C² vector function);
/// 2. dist(·, S_i) is a convex function, so dist(L(t), S_i) ≤ max(dist(c(t_k), S_i), dist(c(t_{k+1}), S_i)).
///
/// Hence dist(c(t), S_i) ≤ max(D_k, D_{k+1}) + M_k·h_k²/8 with D_k the sampled distance to the chord segment.
///
/// The sampled distances carry rounding; a margin of 16 units of roundoff at the largest coordinate covers it.
/// No condition on the speed or the angle of the curve is needed.
///
/// Returns None when a second-derivative bound cannot be proved on some sub-interval.
pub fn chord_error_bound(
    curve: &dyn CurveBound,
    pts: &[P2],
    params: &[f64],
    n: usize,
    sub: usize,
) -> Option<f64> {
    if pts.len() != n * sub + 1 || params.len() != pts.len() || n == 0 || sub == 0 {
        return None;
    }
    let u = f64::EPSILON / 2.0;
    let coord = pts
        .iter()
        .fold(0.0_f64, |m, p| m.max(p.x.abs()).max(p.y.abs()));
    let margin = 16.0 * u * coord;
    let mut worst: f64 = 0.0;
    for seg in 0..n {
        let (ia, ib) = (seg * sub, (seg + 1) * sub);
        let (a, b) = (pts[ia], pts[ib]);
        let dist: Vec<f64> = (ia..=ib)
            .map(|k| distance_to_segment(pts[k], a, b))
            .collect();
        for k in ia..ib {
            let m = curve.sup_c2(params[k], params[k + 1])?;
            if !m.is_finite() {
                return None;
            }
            let h = params[k + 1] - params[k];
            let bracket = dist[k - ia].max(dist[k + 1 - ia]);
            let interp = m * h * h / 8.0 * (1.0 + 8.0 * u);
            worst = worst.max(bracket + interp);
        }
    }
    if !worst.is_finite() {
        return None;
    }
    Some(worst + margin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saddle_derivatives_match_finite_differences() {
        // R 150, r 100, beta 60 deg, delta 20: compare the closed forms with central differences of Branch::s
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 20.0,
        };
        for &phi in &[0.3_f64, 1.2, 2.5, 4.0, 5.9] {
            let h = 1e-4;
            let s = |p: f64| b.s(p);
            let d1 = (s(phi + h) - s(phi - h)) / (2.0 * h);
            let d2 = (s(phi + h) - 2.0 * s(phi) + s(phi - h)) / (h * h);
            let (e1, e2) = saddle_derivatives(&b, Iv::pt(phi)).unwrap();
            assert!(
                (e1.lo - d1).abs() < 1e-5 * (1.0 + d1.abs()),
                "s' at {phi}: {e1:?} vs {d1}"
            );
            assert!(
                (e2.lo - d2).abs() < 1e-3 * (1.0 + d2.abs()),
                "s'' at {phi}: {e2:?} vs {d2}"
            );
        }
    }

    #[test]
    fn chord_bound_covers_a_known_parabola() {
        // y = x²/(2R) sampled at uniform x: the chord error of the parabola's polyline is known in closed form
        struct Parab {
            k: f64,
        }
        impl CurveBound for Parab {
            fn sup_c2(&self, _lo: f64, _hi: f64) -> Option<f64> {
                Some(self.k)
            }
        }
        // c(t) = (t, k t²/2): |c''| = k; chord of a segment of width h deviates by k h²/8 at its midpoint
        let (n, sub, k) = (20usize, 4usize, 0.5);
        let h = 1.0 / n as f64 / sub as f64;
        let params: Vec<f64> = (0..=n * sub).map(|i| i as f64 * h).collect();
        let pts: Vec<P2> = params
            .iter()
            .map(|&t| P2::new(t, 0.5 * k * t * t))
            .collect();
        let bound = chord_error_bound(&Parab { k }, &pts, &params, n, sub).unwrap();
        let exact = k * (h * sub as f64).powi(2) / 8.0;
        assert!(
            bound >= exact,
            "bound {bound:e} below the true chord error {exact:e}"
        );
        assert!(
            bound < 1.5 * exact,
            "bound {bound:e} is not tight (true {exact:e})"
        );
    }

    #[test]
    fn unprovable_segment_gives_none() {
        struct Unknown;
        impl CurveBound for Unknown {
            fn sup_c2(&self, lo: f64, _: f64) -> Option<f64> {
                if lo > 0.25 { None } else { Some(1.0) }
            }
        }
        let params = vec![0.0, 0.5, 1.0];
        let pts = vec![P2::new(0.0, 0.0), P2::new(0.5, 0.0), P2::new(1.0, 0.0)];
        assert!(chord_error_bound(&Unknown, &pts, &params, 1, 2).is_none());
        assert!(chord_error_bound(&Unknown, &pts, &params[..2], 1, 1).is_none());
    }
}
