//! Outward-rounded interval arithmetic for the certified error bounds of work plan task 1.2
//! (docs/backend/TOLERANCE_POLICY.md section 8). An `Iv` always contains every real value that the
//! corresponding expression takes on the input intervals: each binary operation widens its result by one
//! unit in the last place (`next_down` of the lower end, `next_up` of the upper end). Correctly rounded
//! operations (+, −, ·, /, sqrt) are then enclosed exactly. The elementary functions sin, cos and asin are
//! taken from the platform library, which the engine ASSUMES to be accurate to two units in the last place
//! (the same assumption as `develop::TRIG`); each result is widened by four units of relative rounding on
//! top of that. No external crate is used.

use std::f64::consts::PI;

/// Unit round-off of binary64.
const U: f64 = f64::EPSILON / 2.0;

/// The next representable binary64 above `x` (`x` finite or +inf; NaN and +inf are returned unchanged).
pub fn next_up(x: f64) -> f64 {
    if x.is_nan() || x == f64::INFINITY {
        return x;
    }
    if x == 0.0 {
        return f64::from_bits(1);
    }
    let b = x.to_bits();
    if x > 0.0 {
        f64::from_bits(b + 1)
    } else {
        f64::from_bits(b - 1)
    }
}

/// The next representable binary64 below `x` (mirror of `next_up`).
pub fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

/// A closed interval [lo, hi] of binary64 numbers that encloses an exact real range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Iv {
    pub lo: f64,
    pub hi: f64,
}

/// Widening of a platform-library result (sin, cos, asin): four units of relative rounding plus the
/// smallest normal number, on each side, and one more unit outward (`next_down`/`next_up`) for headroom.
fn library(lo: f64, hi: f64) -> Iv {
    Iv {
        lo: next_down(lo - (4.0 * U * lo.abs() + f64::MIN_POSITIVE)),
        hi: next_up(hi + (4.0 * U * hi.abs() + f64::MIN_POSITIVE)),
    }
}

/// Does the arithmetic progression c0 + k·period (k integer) have a member in [a, b]? A tiny slack on b
/// makes a critical point that lies at the rounding level of b count as inside, which only widens the result.
fn has_critical(a: f64, b: f64, c0: f64, period: f64) -> bool {
    let k = ((a - c0) / period).ceil();
    let x = c0 + k * period;
    x <= b + 1e-12 * (1.0 + b.abs())
}

/// The operations are methods named as the operator they compute (not trait impls): `div` returns None on a
/// divisor that contains zero, which the std traits cannot express.
#[allow(clippy::should_implement_trait)]
impl Iv {
    /// The degenerate interval [x, x].
    pub fn pt(x: f64) -> Iv {
        Iv { lo: x, hi: x }
    }

    /// The interval [lo, hi]; panics if lo > hi or a bound is NaN (a programming error, not an input).
    pub fn new(lo: f64, hi: f64) -> Iv {
        assert!(lo <= hi, "interval with lo > hi or NaN: [{lo}, {hi}]");
        Iv { lo, hi }
    }

    /// Outward rounding of an exact range [lo, hi] computed in binary64.
    fn out(lo: f64, hi: f64) -> Iv {
        Iv {
            lo: next_down(lo),
            hi: next_up(hi),
        }
    }

    pub fn add(self, o: Iv) -> Iv {
        Iv::out(self.lo + o.lo, self.hi + o.hi)
    }

    pub fn sub(self, o: Iv) -> Iv {
        Iv::out(self.lo - o.hi, self.hi - o.lo)
    }

    pub fn mul(self, o: Iv) -> Iv {
        let p = [
            self.lo * o.lo,
            self.lo * o.hi,
            self.hi * o.lo,
            self.hi * o.hi,
        ];
        Iv::out(
            p.iter().copied().fold(f64::INFINITY, f64::min),
            p.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Quotient, or None when the divisor interval contains zero (the enclosure would be unbounded).
    pub fn div(self, o: Iv) -> Option<Iv> {
        if o.lo <= 0.0 && o.hi >= 0.0 {
            return None;
        }
        let q = [
            self.lo / o.lo,
            self.lo / o.hi,
            self.hi / o.lo,
            self.hi / o.hi,
        ];
        Some(Iv::out(
            q.iter().copied().fold(f64::INFINITY, f64::min),
            q.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        ))
    }

    pub fn neg(self) -> Iv {
        Iv {
            lo: -self.hi,
            hi: -self.lo,
        }
    }

    /// Square, with the tight enclosure when the interval contains zero.
    pub fn sqr(self) -> Iv {
        if self.lo >= 0.0 || self.hi <= 0.0 {
            return self.mul(self);
        }
        let m = (self.lo * self.lo).max(self.hi * self.hi);
        Iv::out(0.0, m)
    }

    /// Square root (correctly rounded in binary64), or None when the interval reaches below zero by more than
    /// rounding. A lower bound slightly below zero is accepted and clamped: it is the outward rounding of a
    /// quantity that is non-negative in exact arithmetic (a sum of squares), never a real negative value.
    pub fn sqrt(self) -> Option<Iv> {
        if self.hi < 0.0 || self.lo < -(4.0 * U * self.hi.abs() + f64::MIN_POSITIVE) {
            return None;
        }
        Some(Iv::out(self.lo.max(0.0).sqrt(), self.hi.sqrt()))
    }

    /// The largest absolute value in the interval.
    pub fn max_abs(self) -> f64 {
        self.lo.abs().max(self.hi.abs())
    }

    /// Enclosure of sin over the interval; extrema at π/2 + 2kπ (value 1) and 3π/2 + 2kπ (value −1) are included.
    pub fn sin(self) -> Iv {
        if self.hi - self.lo >= 2.0 * PI {
            return Iv { lo: -1.0, hi: 1.0 };
        }
        let mut lo = self.lo.sin().min(self.hi.sin());
        let mut hi = self.lo.sin().max(self.hi.sin());
        if has_critical(self.lo, self.hi, 0.5 * PI, 2.0 * PI) {
            hi = 1.0;
        }
        if has_critical(self.lo, self.hi, 1.5 * PI, 2.0 * PI) {
            lo = -1.0;
        }
        let r = library(lo, hi);
        Iv {
            lo: r.lo.max(-1.0),
            hi: r.hi.min(1.0),
        }
    }

    /// Enclosure of cos over the interval; maxima at 2kπ and minima at π + 2kπ.
    pub fn cos(self) -> Iv {
        if self.hi - self.lo >= 2.0 * PI {
            return Iv { lo: -1.0, hi: 1.0 };
        }
        let mut lo = self.lo.cos().min(self.hi.cos());
        let mut hi = self.lo.cos().max(self.hi.cos());
        if has_critical(self.lo, self.hi, 0.0, 2.0 * PI) {
            hi = 1.0;
        }
        if has_critical(self.lo, self.hi, PI, 2.0 * PI) {
            lo = -1.0;
        }
        let r = library(lo, hi);
        Iv {
            lo: r.lo.max(-1.0),
            hi: r.hi.min(1.0),
        }
    }

    /// Enclosure of asin, which is monotone; None unless the interval lies in [−1, 1].
    pub fn asin(self) -> Option<Iv> {
        if self.lo < -1.0 || self.hi > 1.0 {
            return None;
        }
        let r = library(self.lo.asin(), self.hi.asin());
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seeded linear congruential generator (deterministic, no external crate).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        /// A random interval inside [a, b] with width at most `w`.
        fn iv(&mut self, a: f64, b: f64, w: f64) -> Iv {
            let lo = a + (b - a) * self.next();
            let hi = (lo + w * self.next()).min(b);
            Iv::new(lo, hi)
        }
        fn at(&self, iv: Iv, t: f64) -> f64 {
            iv.lo + (iv.hi - iv.lo) * t
        }
    }

    #[test]
    fn next_up_and_down_are_neighbours() {
        for &x in &[0.0, 1.0, -1.0, 1e-300, -3.5, 150.0, f64::MIN_POSITIVE] {
            let u = next_up(x);
            let d = next_down(x);
            assert!(u > x && d < x, "x = {x:e}");
            assert_eq!(next_down(u), x);
            assert_eq!(next_up(d), x);
        }
        assert_eq!(next_up(f64::INFINITY), f64::INFINITY);
        assert!(next_up(f64::NAN).is_nan());
        assert_eq!(next_up(0.0), f64::from_bits(1));
    }

    #[test]
    fn point_arithmetic_encloses_the_exact_result() {
        // 0.1 + 0.2 is not exact in binary64; its enclosure must contain the exact sum of the two doubles
        let s = Iv::pt(0.1).add(Iv::pt(0.2));
        let exact_sum = 0.1_f64 + 0.2_f64; // rounded, but the enclosure must contain the rounded value
        assert!(s.lo <= exact_sum && exact_sum <= s.hi);
        assert!(s.hi > s.lo, "an inexact sum must have a nonzero width");
        let p = Iv::pt(3.0).mul(Iv::pt(1.0 / 3.0));
        assert!(p.lo <= 1.0 && 1.0 <= p.hi);
    }

    #[test]
    fn random_operations_contain_sampled_values() {
        let mut g = Lcg(0x5eed_1234);
        for _ in 0..20_000 {
            let a = g.iv(-50.0, 50.0, 3.0);
            let b = g.iv(-50.0, 50.0, 3.0);
            let (ta, tb) = (g.next(), g.next());
            let (x, y) = (g.at(a, ta), g.at(b, tb));
            let s = a.add(b);
            assert!(s.lo <= x + y && x + y <= s.hi);
            let d = a.sub(b);
            assert!(d.lo <= x - y && x - y <= d.hi);
            let m = a.mul(b);
            assert!(m.lo <= x * y && x * y <= m.hi);
            if let Some(q) = a.div(b) {
                assert!(q.lo <= x / y && x / y <= q.hi);
            } else {
                assert!(b.lo <= 0.0 && b.hi >= 0.0);
            }
            let sq = a.sqr();
            assert!(sq.lo <= x * x && x * x <= sq.hi);
        }
    }

    #[test]
    fn elementary_functions_contain_sampled_values() {
        let mut g = Lcg(0xfeed_beef);
        for _ in 0..20_000 {
            let a = g.iv(-20.0, 20.0, 0.5);
            let t = g.next();
            let x = g.at(a, t);
            let s = a.sin();
            assert!(s.lo <= x.sin() && x.sin() <= s.hi, "sin on {a:?} at {x}");
            let c = a.cos();
            assert!(c.lo <= x.cos() && x.cos() <= c.hi, "cos on {a:?} at {x}");
        }
        for _ in 0..20_000 {
            let a = g.iv(-1.0, 1.0, 0.3);
            let t = g.next();
            let x = g.at(a, t);
            let r = a.asin().unwrap();
            assert!(r.lo <= x.asin() && x.asin() <= r.hi);
        }
        for _ in 0..2_000 {
            let a = g.iv(0.0, 100.0, 5.0);
            let t = g.next();
            let x = g.at(a, t);
            let r = a.sqrt().unwrap();
            assert!(r.lo <= x.sqrt() && x.sqrt() <= r.hi);
        }
    }

    #[test]
    fn sin_and_cos_include_extrema_inside_the_interval() {
        // the interval [0, 4] contains pi/2, where sin = 1, and [2, 4] contains pi, where cos = -1
        let s = Iv::new(0.0, 4.0).sin();
        assert_eq!(s.hi, 1.0);
        assert!(s.lo <= 4.0_f64.sin());
        let c = Iv::new(2.0, 4.0).cos();
        assert_eq!(c.lo, -1.0);
        // an interval that avoids the extremum keeps a tight enclosure
        let t = Iv::new(0.1, 0.2).sin();
        assert!(t.hi < 0.21 && t.lo > 0.09);
        // a full turn is [-1, 1]
        assert_eq!(Iv::new(0.0, 7.0).cos(), Iv { lo: -1.0, hi: 1.0 });
    }

    #[test]
    fn refusals_are_explicit() {
        assert!(Iv::new(-1.0, 1.0).div(Iv::new(-1.0, 1.0)).is_none());
        assert!(Iv::new(-1.0, 1.0).sqrt().is_none());
        // a sum of squares rounded outward: its lower bound may dip below zero by one unit, which is accepted
        let z = Iv::pt(0.0).sqr().add(Iv::pt(0.0).sqr());
        assert!(z.lo < 0.0 && z.sqrt().is_some());
        assert!(Iv::new(-1.5, 0.0).asin().is_none());
    }
}
