//! Universal Engineering Mathematics engine (UEM) — first implemented subset.
//!
//! Pure library: no I/O, no global state, no AI. Implements parts of
//! Specification 02 (which is PROVISIONAL and not independently reviewed):
//! cylinder shell, right cone frustum, oblique cone frustum (eccentric
//! reducer), cylinder-on-cylinder branch, press-brake bend formulas, and the
//! independent verification checks for those families.
//!
//! Numerical regime of this build: IEEE binary64 with *estimated* error
//! (comparison of two quadrature orders / step sizes). Interval-certified
//! bounds are NOT implemented; every result says so.

pub mod develop;
pub mod profile;
pub mod verify;

use std::f64::consts::PI;

pub const TAU: f64 = 2.0 * PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P2 {
    pub x: f64,
    pub y: f64,
}

impl P2 {
    pub fn new(x: f64, y: f64) -> Self {
        P2 { x, y }
    }
    pub fn dist(self, o: P2) -> f64 {
        (self.x - o.x).hypot(self.y - o.y)
    }
    pub fn polar(rho: f64, psi: f64) -> Self {
        P2::new(rho * psi.cos(), rho * psi.sin())
    }
}

/// Role of a curve in the manufacturing drawing (maps to DXF layers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    CutOuter,
    CutInner,
    Reference,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Geom {
    Line {
        a: P2,
        b: P2,
    },
    /// Circular arc traversed from angle `a0` to `a1` (radians): counter-clockwise
    /// if a1 > a0, clockwise if a1 < a0.
    Arc {
        c: P2,
        r: f64,
        a0: f64,
        a1: f64,
    },
    /// Polyline approximation of an exact curve; `eps_rep` is the estimated
    /// maximum deviation from the exact curve (mm).
    Polyline {
        pts: Vec<P2>,
        eps_rep: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub name: &'static str,
    pub layer: Layer,
    pub geom: Geom,
}

/// Development class labels follow the scheme *proposed* in Spec 02 §0.2
/// (not yet adopted). Only the two exact sub-classes occur in this build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevClass {
    /// Exact isometry, closed-form evaluation.
    ExactClosedForm,
    /// Exact isometry, evaluated numerically with estimated error.
    ExactNumeric,
}

impl DevClass {
    pub fn code(self) -> &'static str {
        match self {
            DevClass::ExactClosedForm => "E-C",
            DevClass::ExactNumeric => "E-N",
        }
    }
}

/// The face of the formed part from which a pattern is seen: the pattern
/// normal +z points to this side (Spec 02 §0.3, "marking side"). Marking a
/// pattern on the other face mirrors the part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewSide {
    Outside,
    Inside,
}

impl ViewSide {
    pub fn code(self) -> &'static str {
        match self {
            ViewSide::Outside => "outside",
            ViewSide::Inside => "inside",
        }
    }
}

/// The side every pattern of this build is drawn from.
///
/// Spec 02 §0.3 requires a declared side but does not choose one. The frames
/// the specification writes down decide it for the parts built here: §4.1
/// (u = r·θ, v = z) and §4.4 (branch pattern (r·φ, s)) are right-handed with
/// the outward surface normal, i.e. they are the view from OUTSIDE. This is a
/// convention declared by the software, recorded with every result and every
/// DXF; whether a workshop marks on that face has NOT been reviewed by a
/// fabrication engineer.
pub const MARKING_SIDE: ViewSide = ViewSide::Outside;

#[derive(Clone, Debug)]
pub struct Pattern {
    pub family: &'static str,
    /// Face of the formed part the pattern is seen from. Every part of one job must declare the same side.
    pub marking_side: ViewSide,
    pub class: DevClass,
    /// Closed outer profile, curves in order, end of each = start of next.
    pub outline: Vec<Curve>,
    /// Closed inner profiles (openings), each a single closed polyline, clockwise.
    pub holes: Vec<Curve>,
    /// ESTIMATE of the numerical error of pattern coordinates (mm). Not a
    /// bound: independent validation measured up to 6 times this value near
    /// tangency (cancellation in R² − y²) and 2 times for a very flat oblique reducer.
    pub eps_num: f64,
    /// ESTIMATE of the largest distance between a polyline and the exact curve
    /// (mm): the largest deviation found at three interior samples of every
    /// segment. Not a bound; 0 only for patterns made of lines and exact arcs.
    pub eps_rep: f64,
    /// Named scalar results (mm, rad, mm²) for reporting and verification.
    pub quantities: Vec<(&'static str, f64)>,
    /// Assumptions under which the pattern is valid for fabrication.
    pub assumptions: Vec<&'static str>,
}

impl Pattern {
    pub fn quantity(&self, name: &str) -> Option<f64> {
        self.quantities
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UemError {
    /// Parameters outside the range in which this implementation is valid.
    OutOfRange { param: &'static str, reason: String },
    /// Degenerate geometry.
    Degenerate(String),
    /// Development would overlap itself or is otherwise not producible.
    NotDevelopable(String),
}

impl std::fmt::Display for UemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UemError::OutOfRange { param, reason } => write!(f, "out of range ({param}): {reason}"),
            UemError::Degenerate(s) => write!(f, "degenerate geometry: {s}"),
            UemError::NotDevelopable(s) => write!(f, "not developable: {s}"),
        }
    }
}

pub(crate) fn finite_pos(name: &'static str, v: f64) -> Result<(), UemError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(UemError::OutOfRange {
            param: name,
            reason: format!("must be finite and > 0, got {v}"),
        })
    }
}

/// Composite Gauss–Legendre quadrature on [a,b] with `n` panels.
/// Returns (5-point result, |5-point − 3-point|) — the second value is an
/// error *estimate*, not a bound.
pub fn gauss(f: &dyn Fn(f64) -> f64, a: f64, b: f64, n: usize) -> (f64, f64) {
    const X5: [f64; 3] = [0.0, 0.538_469_310_105_683_1, 0.906_179_845_938_664];
    const W5: [f64; 3] = [
        0.568_888_888_888_888_9,
        0.478_628_670_499_366_47,
        0.236_926_885_056_189_08,
    ];
    const X3: f64 = 0.774_596_669_241_483_4;
    let h = (b - a) / n as f64;
    let (mut s5, mut s3) = (0.0, 0.0);
    for i in 0..n {
        let m = a + (i as f64 + 0.5) * h;
        let k = 0.5 * h;
        s5 += k
            * (W5[0] * f(m)
                + W5[1] * (f(m - k * X5[1]) + f(m + k * X5[1]))
                + W5[2] * (f(m - k * X5[2]) + f(m + k * X5[2])));
        s3 += k * (8.0 / 9.0 * f(m) + 5.0 / 9.0 * (f(m - k * X3) + f(m + k * X3)));
    }
    (s5, (s5 - s3).abs())
}

/// Press-brake bend allowance: neutral arc length. `theta` = bend angle (rad),
/// `ri` inside radius, `k` neutral factor, `t` thickness. `k` is a calibrated
/// process parameter; no default exists (Spec 02 MATH-16).
pub fn bend_allowance(theta: f64, ri: f64, k: f64, t: f64) -> f64 {
    theta * (ri + k * t)
}

/// Bend deduction for 0 < theta <= pi/2 (outside dimensions to mould line).
pub fn bend_deduction(theta: f64, ri: f64, k: f64, t: f64) -> Result<f64, UemError> {
    if !(theta > 0.0 && theta <= PI / 2.0 + 1e-12) {
        return Err(UemError::OutOfRange { param: "theta", reason: "bend deduction defined here for 0 < theta <= 90 deg; use neutral-arc lengths otherwise".into() });
    }
    Ok(2.0 * (ri + t) * (theta / 2.0).tan() - bend_allowance(theta, ri, k, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bend_reference_case() {
        // BEND-01: t=3, ri=3, theta=90 deg, test value k=0.4
        let ba = bend_allowance(PI / 2.0, 3.0, 0.4, 3.0);
        let bd = bend_deduction(PI / 2.0, 3.0, 0.4, 3.0).unwrap();
        assert!((ba - 6.597344572539).abs() < 1e-11);
        assert!((bd - 5.402655427461).abs() < 1e-11);
        assert!((100.0 - bd - 94.59734457254).abs() < 1e-10);
        assert!(bend_deduction(2.0, 3.0, 0.4, 3.0).is_err());
    }

    #[test]
    fn gauss_integrates_polynomial_and_sine() {
        let (v, e) = gauss(&|x| x * x, 0.0, 3.0, 4);
        assert!((v - 9.0).abs() < 1e-12 && e < 1e-12);
        let (v, _) = gauss(&|x| x.sin(), 0.0, PI, 64);
        assert!((v - 2.0).abs() < 1e-12);
    }
}
