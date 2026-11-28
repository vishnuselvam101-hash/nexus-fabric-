//! Universal Engineering Mathematics engine (UEM) — first implemented subset.
//!
//! Pure library: no I/O, no global state, no automated interpretation. Implements parts of
//! Specification 02 (which is PROVISIONAL and not independently reviewed):
//! cylinder shell, right cone frustum, oblique cone frustum (eccentric
//! reducer), cylinder-on-cylinder branch, press-brake bend formulas, and the
//! independent verification checks for those families.
//!
//! Numerical regime of this build: IEEE binary64. Rounding errors of the
//! closed-form parts are stated as first-order a-priori bounds, quadrature
//! errors as the difference of two refinement levels, and the distance between
//! a polyline and its curve is measured after the density was chosen for a
//! stated limit. The chord error of every polyline is certified by outward
//! interval arithmetic (`interval.rs`, `curve_bound.rs`; TOLERANCE_POLICY
//! section 8) for the branch saddle and the header opening. The eccentric
//! reducer's vertex term carries a quadrature estimate, so its value is not
//! certified; every result says which bound it is.

pub mod certify;
pub mod curve_bound;
pub mod develop;
pub mod geometry;
pub mod independent;
pub mod interval;
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
    /// Polyline approximation of an exact curve; `eps_rep` bounds the maximum
    /// deviation from the exact curve (mm), certified where a bound can be
    /// proved (see `Pattern::eps_rep`).
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
    /// Numerical error of the pattern coordinates (mm): what kind of figure it
    /// is, and how it was obtained, is stated in `eps_num_basis`. It is never
    /// interval-certified in this build.
    pub eps_num: f64,
    /// How `eps_num` was obtained (a-priori rounding bound, quadrature
    /// estimate, ...), in words that go into the result.
    pub eps_num_basis: &'static str,
    /// Upper bound of the largest distance between a polyline and the exact
    /// curve it stands for (mm). Certified by outward interval arithmetic where
    /// a bound can be proved (`develop::certified_rep`: the chord enclosure, or
    /// the deviation measured at `develop::VERIFY_SUB - 1` interior points of
    /// every segment if larger, plus the vertex displacement term); an
    /// uncertified estimate otherwise (`eps_rep_certified` says which,
    /// TOLERANCE_POLICY section 8). 0 only for patterns made of lines and exact
    /// arcs.
    pub eps_rep: f64,
    /// How `eps_rep` was obtained and which limit the polyline density was
    /// chosen for.
    pub eps_rep_basis: &'static str,
    /// True when `eps_rep` is a certified upper bound (interval arithmetic on the closed-form derivatives,
    /// docs/backend/TOLERANCE_POLICY.md section 8). False only where no bound could be proved: the value is
    /// then the measured estimate, and every check that uses it returns INCONCLUSIVE.
    pub eps_rep_certified: bool,
    /// Named scalar results (mm, rad, mm²) for reporting and verification.
    pub quantities: Vec<(&'static str, f64)>,
    /// What the plate coordinates mean on the formed part, in words a workshop
    /// can act on: plate axes and rolling axis, which edges are seams, where
    /// stations and angles start and in which direction they grow, where the
    /// origin lies, and that kerf is not compensated (review RB-002, RB-003,
    /// RB-007, RB-008, RB-009). This prose is NOT written by this build into
    /// the DXF or the job result: the statement that is written is built by
    /// `nf_core::job::conventions_for` from the pattern and the request
    /// (work plan task 1.3). Provisional, not reviewed by a fabricator.
    pub conventions: Vec<String>,
    /// What each outline curve is on the formed part: (curve name, role).
    pub roles: Vec<(&'static str, &'static str)>,
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
    /// Role of a curve on the formed part ("seam", "end", ...), if stated.
    pub fn role(&self, curve: &str) -> Option<&'static str> {
        self.roles
            .iter()
            .find(|(n, _)| *n == curve)
            .map(|(_, r)| *r)
    }
}

/// Stable refusal codes of the engine: one per distinct rule. Each is documented, with its request
/// fields and whether a detail value is given, in docs/backend/REFUSAL_CODES.md (a test checks that table).
pub mod refusal {
    pub const VALUE_NOT_FINITE: &str = "value_not_finite";
    pub const VALUE_NOT_POSITIVE: &str = "value_not_positive";
    pub const OFFSET_NEGATIVE: &str = "offset_negative";
    pub const REDUCER_RADII_ORDER: &str = "reducer_radii_order";
    pub const REDUCER_NEAR_CYLINDRICAL: &str = "reducer_near_cylindrical";
    pub const DEVELOPED_ANGLE_OVERLAPS: &str = "developed_angle_overlaps";
    pub const BRANCH_ANGLE_OUT_OF_RANGE: &str = "branch_angle_out_of_range";
    pub const BRANCH_TANGENT_OR_PARTIAL: &str = "branch_tangent_or_partial";
    pub const SQUARE_END_TOO_SHORT: &str = "square_end_too_short";
    pub const OPENING_OUTSIDE_SHELL: &str = "opening_outside_shell";
    pub const CHORD_ERROR_UNREACHABLE: &str = "chord_error_unreachable";
    pub const RADIUS_DEV_EXCEEDS_OUTSIDE: &str = "radius_dev_exceeds_outside";
    pub const RADIUS_DEV_BELOW_INSIDE: &str = "radius_dev_below_inside";
    pub const RADIUS_NEUTRAL_MISMATCH: &str = "radius_neutral_mismatch";
    pub const SURFACE_REFERENCE_INVALID: &str = "surface_reference_invalid";
    pub const JOINT_PAIRING_INVALID: &str = "joint_pairing_invalid";
    pub const JOINT_TYPE_UNKNOWN: &str = "joint_type_unknown";
    pub const MAGNITUDE_ABOVE_LIMIT: &str = "magnitude_above_limit";
    pub const DIMENSION_BELOW_LIMIT: &str = "dimension_below_limit";
    pub const BEND_ANGLE_OUT_OF_RANGE: &str = "bend_angle_out_of_range";
    pub const BRANCH_ANGLE_NOT_VERIFIABLE: &str = "branch_angle_not_verifiable";
}

/// A refusal of the engine. `code` is the rule (see `refusal`), `fields` the request fields it concerns (the
/// engine's own label `param` is kept for the existing `refused_field` and may name no request field),
/// `detail` the limit and measured value where the engine computes them exactly (empty otherwise).
#[derive(Clone, Debug, PartialEq)]
pub enum UemError {
    /// Parameters outside the range in which this implementation is valid.
    OutOfRange {
        code: &'static str,
        param: &'static str,
        fields: Vec<&'static str>,
        reason: String,
        detail: Vec<(&'static str, f64)>,
    },
    /// Development would overlap itself or is otherwise not producible.
    NotDevelopable {
        code: &'static str,
        fields: Vec<&'static str>,
        reason: String,
        detail: Vec<(&'static str, f64)>,
    },
}

impl UemError {
    /// The stable code of the rule this refusal applies.
    pub fn code(&self) -> &'static str {
        match self {
            UemError::OutOfRange { code, .. } | UemError::NotDevelopable { code, .. } => code,
        }
    }
    /// The request fields the rule concerns.
    pub fn fields(&self) -> &[&'static str] {
        match self {
            UemError::OutOfRange { fields, .. } | UemError::NotDevelopable { fields, .. } => fields,
        }
    }
    /// The exact limit and measured value, where the engine computes them.
    pub fn detail(&self) -> &[(&'static str, f64)] {
        match self {
            UemError::OutOfRange { detail, .. } | UemError::NotDevelopable { detail, .. } => detail,
        }
    }
}

pub fn out_of_range(
    code: &'static str,
    param: &'static str,
    fields: Vec<&'static str>,
    reason: String,
) -> UemError {
    out_of_range_with(code, param, fields, reason, Vec::new())
}

pub fn out_of_range_with(
    code: &'static str,
    param: &'static str,
    fields: Vec<&'static str>,
    reason: String,
    detail: Vec<(&'static str, f64)>,
) -> UemError {
    UemError::OutOfRange {
        code,
        param,
        fields,
        reason,
        detail,
    }
}

pub fn not_developable(
    code: &'static str,
    fields: Vec<&'static str>,
    reason: String,
    detail: Vec<(&'static str, f64)>,
) -> UemError {
    UemError::NotDevelopable {
        code,
        fields,
        reason,
        detail,
    }
}

impl std::fmt::Display for UemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UemError::OutOfRange { param, reason, .. } => {
                write!(f, "out of range ({param}): {reason}")
            }
            UemError::NotDevelopable { reason, .. } => write!(f, "not developable: {reason}"),
        }
    }
}

pub(crate) fn finite_pos(name: &'static str, v: f64) -> Result<(), UemError> {
    if v.is_finite() && v > 0.0 {
        return Ok(());
    }
    let code = if v.is_finite() {
        refusal::VALUE_NOT_POSITIVE
    } else {
        refusal::VALUE_NOT_FINITE
    };
    Err(out_of_range(
        code,
        name,
        vec![name],
        format!("must be finite and > 0, got {v}"),
    ))
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

/// One 5-point Gauss–Legendre panel on [a,b].
pub fn gauss5(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    const X: [f64; 2] = [0.538_469_310_105_683_1, 0.906_179_845_938_664];
    const W: [f64; 3] = [
        0.568_888_888_888_888_9,
        0.478_628_670_499_366_47,
        0.236_926_885_056_189_08,
    ];
    let (m, k) = (0.5 * (a + b), 0.5 * (b - a));
    k * (W[0] * f(m)
        + W[1] * (f(m - k * X[0]) + f(m + k * X[0]))
        + W[2] * (f(m - k * X[1]) + f(m + k * X[1])))
}

/// Adaptive 5-point Gauss–Legendre on [a,b]: the interval is halved until the
/// whole-interval value and the sum of its halves agree to `rel` of the value
/// (or `depth` halvings were made). Returns (value, sum of the last
/// whole-minus-halves differences). The second value is a conservative
/// ESTIMATE of the error, not a bound: for a resolved integrand the error of
/// the returned value is about 1/1000 of it (the rule is of order 10).
pub fn gauss_adaptive(f: &dyn Fn(f64) -> f64, a: f64, b: f64, rel: f64, depth: u32) -> (f64, f64) {
    fn go(f: &dyn Fn(f64) -> f64, a: f64, b: f64, whole: f64, rel: f64, depth: u32) -> (f64, f64) {
        let m = 0.5 * (a + b);
        let (l, r) = (gauss5(f, a, m), gauss5(f, m, b));
        let d = (whole - (l + r)).abs();
        if depth == 0 || d <= rel * (l + r).abs() || !d.is_finite() {
            return (l + r, d);
        }
        let (lv, le) = go(f, a, m, l, rel, depth - 1);
        let (rv, re) = go(f, m, b, r, rel, depth - 1);
        (lv + rv, le + re)
    }
    go(f, a, b, gauss5(f, a, b), rel, depth)
}

/// Running sum with Neumaier compensation: the rounding error of the total
/// stays at a few units in the last place whatever the number of terms.
#[derive(Clone, Copy, Debug, Default)]
pub struct Compensated {
    sum: f64,
    c: f64,
}

impl Compensated {
    pub fn add(&mut self, x: f64) {
        let t = self.sum + x;
        self.c += if self.sum.abs() >= x.abs() {
            (self.sum - t) + x
        } else {
            (x - t) + self.sum
        };
        self.sum = t;
    }
    pub fn value(&self) -> f64 {
        self.sum + self.c
    }
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
        return Err(out_of_range(refusal::BEND_ANGLE_OUT_OF_RANGE, "theta", vec![], "bend deduction defined here for 0 < theta <= 90 deg; use neutral-arc lengths otherwise".into()));
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
    fn adaptive_quadrature_resolves_a_peak_the_fixed_rule_misses() {
        // a narrow peak of width 1e-3 inside [0, 1]: integral = atan(500/1) * 2 * 1e-3 ... exactly 2*w*atan(1/(2w)) / 1
        let w = 1e-3;
        let f = |x: f64| w / (w * w + (x - 0.5) * (x - 0.5));
        let exact = 2.0 * (0.5 / w).atan();
        let (fixed, fixed_est) = gauss(&f, 0.0, 1.0, 8);
        assert!(
            (fixed - exact).abs() > 10.0 * fixed_est.min(1e-3),
            "the fixed rule is wrong here and its estimate does not say so: {fixed} against {exact}, estimate {fixed_est}"
        );
        let (v, e) = gauss_adaptive(&f, 0.0, 1.0, 1e-13, 40);
        assert!((v - exact).abs() < 1e-11, "{v} against {exact}");
        assert!(e < 1e-9 && (v - exact).abs() <= e.max(1e-12));
        let mut s = Compensated::default();
        for _ in 0..1_000_000 {
            s.add(0.1);
        }
        assert!((s.value() - 100_000.0).abs() < 1e-9);
    }

    #[test]
    fn gauss_integrates_polynomial_and_sine() {
        let (v, e) = gauss(&|x| x * x, 0.0, 3.0, 4);
        assert!((v - 9.0).abs() < 1e-12 && e < 1e-12);
        let (v, _) = gauss(&|x| x.sin(), 0.0, PI, 64);
        assert!((v - 2.0).abs() < 1e-12);
    }
}
