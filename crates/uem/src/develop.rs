//! Surface developments (Spec 02 §4.1–§4.4). All radii are *reference-surface*
//! radii supplied by the caller: thickness and process allowances are NOT
//! modelled in this build, which is recorded as an assumption on every pattern.
//!
//! Every pattern states, in `conventions`, what its plate coordinates mean on
//! the formed part (axes, rolling axis, seam edges, station and angle datums,
//! origin, kerf). The statements carry the numbers of the part, so that they
//! stay unambiguous for a square plate too (review RB-003).

use crate::{
    Compensated, Curve, DevClass, Geom, Layer, MARKING_SIDE, P2, Pattern, TAU, UemError,
    finite_pos, gauss, gauss_adaptive,
};
use crate::{not_developable, out_of_range, out_of_range_with, refusal};
use std::f64::consts::PI;

const ASSUME_REF: &str = "Radii are taken as reference-surface (neutral) radii exactly as entered; thickness, neutral-factor, weld gap, shrinkage and kerf are not modelled.";
const ASSUME_MODEL: &str = "Mathematical model from Specification 02 (provisional; not independently reviewed; evidence level EV-1).";

/// A developed sector at or beyond this fraction of a full turn is refused for every conical family:
/// its seam edges would touch or overlap. Implementation limit (no seam clearance rule exists in the
/// specification); it is the value the eccentric reducer always used.
pub const FULL_TURN_LIMIT: f64 = 0.999;

// ---------------------------------------------------------------- polyline density (review RB-004)

/// Largest distance between a polyline and the exact curve it stands for that this build accepts,
/// in millimetres. The number of segments of every polyline is doubled from its base value until
/// the measured distance is at or below this value. IMPLEMENTATION LIMIT pending engineering
/// review: the specification (ALG-07, §7.4) asks for a budget derived from the tolerance of the
/// part, which this build does not have. A part that cannot be represented within the limit with
/// `MAX_SEGMENTS` segments is refused, not drawn coarser.
pub const MAX_CHORD_ERROR_MM: f64 = 0.01;
/// Base number of segments of a branch saddle and of a header opening (uniform in the angle phi).
pub const SEGMENTS: usize = 720;
/// Base number of segments of each end curve of an oblique cone (uniform in the angle theta).
pub const CONE_SEGMENTS: usize = 360;
/// Largest number of segments of one polyline.
pub const MAX_SEGMENTS: usize = 46_080;
/// Samples per segment used while choosing the density, and when verifying the achieved error.
const SUB: usize = 4;
pub const VERIFY_SUB: usize = 16;

pub const EPS_REP_BASIS: &str = "Largest distance from the exact curve to the polyline, measured at 15 interior points of every segment after the number of segments was chosen (uniform in the curve angle, doubled from the base number until the measured distance is at most 0.01 mm, an implementation limit pending engineering review). Measured, not interval-certified.";
const EPS_REP_EXACT: &str = "Lines and circular arcs are exact entities: no representation error.";

fn seg_distance(q: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 <= 0.0 || !l2.is_finite() {
        return q.dist(a);
    }
    let t = (((q.x - a.x) * dx + (q.y - a.y) * dy) / l2).clamp(0.0, 1.0);
    q.dist(P2::new(a.x + t * dx, a.y + t * dy))
}

/// Largest distance from the exact samples strictly inside each segment to its chord, over all
/// segments. `all` holds `sub` samples per segment (index i*sub is a vertex).
pub(crate) fn chord_deviation(all: &[P2], sub: usize) -> f64 {
    let mut worst: f64 = 0.0;
    for seg in 0..(all.len() - 1) / sub {
        let (a, b) = (all[seg * sub], all[(seg + 1) * sub]);
        for k in 1..sub {
            let d = seg_distance(all[seg * sub + k], a, b);
            // a non-finite sample must never look like a small deviation
            worst = if d.is_finite() {
                worst.max(d)
            } else {
                f64::INFINITY
            };
        }
    }
    worst
}

/// Smallest number of segments on the ladder base, 2·base, 4·base, … for which the MEASURED chord
/// error is at or below `MAX_CHORD_ERROR_MM`; returns it with the error measured at `VERIFY_SUB - 1`
/// interior points per segment. `sample(n, sub)` returns the `sub·n + 1` exact curve points of a
/// uniform subdivision (index `k·sub` is vertex `k`).
fn choose_density(
    base: usize,
    what: &'static str,
    fields: &[&'static str],
    sample: &dyn Fn(usize, usize) -> Vec<P2>,
) -> Result<(usize, f64), UemError> {
    let mut n = base;
    loop {
        let quick = chord_deviation(&sample(n, SUB), SUB);
        let mut measured = quick;
        if quick <= MAX_CHORD_ERROR_MM {
            measured = chord_deviation(&sample(n, VERIFY_SUB), VERIFY_SUB);
            if measured <= MAX_CHORD_ERROR_MM {
                return Ok((n, measured));
            }
        }
        if n >= MAX_SEGMENTS {
            return Err(out_of_range(
                refusal::CHORD_ERROR_UNREACHABLE,
                "chord_error",
                fields.to_vec(),
                format!(
                    "the {what} cannot be represented within the stated maximum chord error of {MAX_CHORD_ERROR_MM} mm: with {MAX_SEGMENTS} segments, the most this build uses, the polyline is still {measured:.4} mm from the exact curve (implementation limit, not an engineering rule)"
                ),
            ));
        }
        n *= 2;
    }
}

// ---------------------------------------------------------------- numerical error (review RB-005)

/// Unit round-off of binary64.
pub const U: f64 = f64::EPSILON / 2.0;
/// Relative accuracy assumed for sin, cos and asin of the platform mathematical library: one unit
/// in the last place. This is an ASSUMPTION about the library, stated with every bound that uses it.
const TRIG: f64 = 2.0 * U;

pub const EPS_NUM_BASIS_BRANCH: &str = "A-priori bound from a first-order rounding analysis of the evaluated expressions (Spec 02 eq. 4.3 and the header chart): the cancellation in R^2 - y^2, amplified by the square root and by the arcsine, plus the remaining sums, products, the parameter and the plate coordinates. Assumes sin, cos and asin of the platform library accurate to one unit in the last place. Checked against 50-digit arithmetic; not interval-certified.";
pub const EPS_NUM_BASIS_EXACT: &str = "A-priori bound from a first-order rounding analysis of the closed-form expressions (a few units in the last place of the largest coordinate; for arcs including the rounding of the sector angle). Not interval-certified.";
pub const EPS_NUM_BASIS_CONE: &str = "First-order rounding bound of the evaluated expressions plus the quadrature error of the developed angle, taken as the sum of the differences between the last two refinement levels of an adaptive rule (a conservative ESTIMATE of that part, not a bound). Checked against 50-digit arithmetic; not interval-certified.";

fn line(name: &'static str, a: P2, b: P2) -> Curve {
    Curve {
        name,
        layer: Layer::CutOuter,
        geom: Geom::Line { a, b },
    }
}

/// A length in the words of a convention note: millimetres, at most three decimals, no trailing zeros.
pub fn mm(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

const KERF_NOTE: &str = "CUT LINES = FINISHED PART EDGES ON THE DEVELOPMENT SURFACE. KERF IS NOT COMPENSATED IN THIS FILE. NO BEVEL, ROOT GAP OR WELD ALLOWANCE IS INCLUDED.";
const CONVENTION_STATUS: &str = "THESE CONVENTIONS ARE DECLARED BY THE SOFTWARE, PROVISIONAL, NOT REVIEWED BY A FABRICATION ENGINEER.";

/// Sign of the axis offset and of the angle between the axes, in words (review RB-002). From the
/// frame of Spec 02 §4.4: header axis = +x (increasing station), branch axis through (0, delta, 0)
/// with direction (cos beta, 0, sin beta). With x forward and z up, +y is to the LEFT.
pub fn delta_notes(delta: f64, beta: f64) -> [String; 2] {
    [
        format!(
            "OFFSET DELTA ({} MM HERE): POSITIVE = THE BRANCH AXIS IS DISPLACED TO THE LEFT OF THE HEADER AXIS FOR AN OBSERVER AT THE STATION-0 END LOOKING ALONG THE HEADER TOWARD INCREASING STATION WITH THE BRANCH POINTING UP.",
            mm(delta)
        ),
        format!(
            "ANGLE BETA ({} DEG HERE) IS MEASURED FROM THE DIRECTION OF INCREASING STATION TO THE BRANCH AXIS: BELOW 90 DEG THE BRANCH LEANS TOWARD INCREASING STATION.",
            mm(beta.to_degrees())
        ),
    ]
}

/// Plain cylindrical shell: rectangle 2*pi*r_dev by length. Spec 02 §4.1.
pub fn cylinder_shell(r_dev: f64, length: f64) -> Result<Pattern, UemError> {
    finite_pos("r_dev", r_dev)?;
    finite_pos("length", length)?;
    let w = TAU * r_dev;
    let (p0, p1, p2, p3) = (
        P2::new(0.0, 0.0),
        P2::new(w, 0.0),
        P2::new(w, length),
        P2::new(0.0, length),
    );
    Ok(Pattern {
        family: "cylinder_shell",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactClosedForm,
        outline: vec![
            line("bottom", p0, p1),
            line("seam_b", p1, p2),
            line("top", p2, p3),
            line("seam_a", p3, p0),
        ],
        holes: vec![],
        conventions: vec![
            format!("PLATE AXES: X = AROUND THE SHELL (DEVELOPED CIRCUMFERENCE {} MM), Y = ALONG THE SHELL AXIS (LENGTH {} MM).", mm(w), mm(length)),
            "ROLLING: ROLL ABOUT AN AXIS PARALLEL TO Y. THE DRAWN FACE BECOMES THE OUTSIDE OF THE SHELL.".into(),
            format!("SEAM EDGES: THE TWO EDGES PARALLEL TO Y, AT X = 0 AND X = {} (CURVES seam_a AND seam_b). SHELL ENDS: THE EDGES AT Y = 0 (CURVE bottom) AND Y = {} (CURVE top).", mm(w), mm(length)),
            format!("ANGLE: THETA = X / R_DEV, ZERO AT THE SEAM X = 0, INCREASING WITH X: COUNTER-CLOCKWISE SEEN FROM THE END Y = {} LOOKING TOWARD THE END Y = 0.", mm(length)),
            "ORIGIN: LOWER LEFT CORNER OF THE PLATE = THE SEAM AT THE END Y = 0.".into(),
            KERF_NOTE.into(),
            CONVENTION_STATUS.into(),
        ],
        roles: vec![
            ("bottom", "shell end (Y = 0)"),
            ("seam_b", "seam edge"),
            ("top", "shell end"),
            ("seam_a", "seam edge (angle datum, X = 0)"),
        ],
        // w = fl(TAU·r): the rounded 2·pi and one product; the length is the input itself
        eps_num: 8.0 * U * w.max(length),
        eps_num_basis: EPS_NUM_BASIS_EXACT,
        eps_rep: 0.0,
        eps_rep_basis: EPS_REP_EXACT,
        quantities: vec![
            ("developed_width", w),
            ("length", length),
            ("area", w * length),
        ],
        assumptions: vec![ASSUME_REF, ASSUME_MODEL],
    })
}

/// Height of a mitre cut above the segment mid-plane (Spec 02 §4.1):
/// v(theta) = (rb - rc*cos(theta)) * tan(gamma), theta = 0 at the intrados.
pub fn mitre_cut_height(rb: f64, rc: f64, gamma: f64, theta: f64) -> f64 {
    (rb - rc * theta.cos()) * gamma.tan()
}

const CONE_ANGLE_NOTE: &str = "ANGLE: ZERO ON seam_a (+X AXIS), INCREASING COUNTER-CLOCKWISE ON THE PLATE. ON THE PART THE ANGLE ABOUT THE AXIS INCREASES COUNTER-CLOCKWISE SEEN FROM THE SMALL END LOOKING TOWARD THE LARGE END.";
const CONE_ROLL_NOTE: &str = "FORMING: ROLL SO THAT THE TWO STRAIGHT EDGES MEET. THE DRAWN FACE BECOMES THE OUTSIDE OF THE CONE.";

/// Right circular cone frustum (concentric reducer). Spec 02 §4.2.
/// r1 > r2 > 0 reference radii at the end planes, h axial height.
pub fn cone_frustum(r1: f64, r2: f64, h: f64) -> Result<Pattern, UemError> {
    finite_pos("r1", r1)?;
    finite_pos("r2", r2)?;
    finite_pos("h", h)?;
    if r1 <= r2 {
        return Err(out_of_range(
            refusal::REDUCER_RADII_ORDER,
            "r1",
            vec!["r1", "r2"],
            "requires r1 > r2 (large end first); equal radii is a cylinder".into(),
        ));
    }
    let s = h.hypot(r1 - r2);
    let sin_a = (r1 - r2) / s;
    if sin_a < 1e-3 {
        return Err(out_of_range_with(refusal::REDUCER_NEAR_CYLINDRICAL, "r1-r2", vec!["r1", "r2", "h"], "near-cylindrical cone: apex-based formula is ill-conditioned and the apex-free method is not implemented in this build".into(), vec![("limit", 1e-3), ("measured", sin_a)]));
    }
    let (l1, l2) = (r1 / sin_a, r2 / sin_a);
    let phi = TAU * sin_a;
    // Same rule, same limit and same words as the eccentric reducer: a sector within 0.1 % of a full turn
    // closes on itself (its seam edges touch), which is not a producible pattern.
    if phi >= TAU * FULL_TURN_LIMIT {
        return Err(not_developable(
            refusal::DEVELOPED_ANGLE_OVERLAPS,
            vec!["r1", "r2", "h"],
            format!("developed angle {phi:.6} rad would overlap"),
            vec![("limit", TAU * FULL_TURN_LIMIT), ("measured", phi)],
        ));
    }
    let c = P2::new(0.0, 0.0);
    let outline = vec![
        Curve {
            name: "large_end",
            layer: Layer::CutOuter,
            geom: Geom::Arc {
                c,
                r: l1,
                a0: 0.0,
                a1: phi,
            },
        },
        line("seam_b", P2::polar(l1, phi), P2::polar(l2, phi)),
        Curve {
            name: "small_end",
            layer: Layer::CutOuter,
            geom: Geom::Arc {
                c,
                r: l2,
                a0: phi,
                a1: 0.0,
            },
        },
        line("seam_a", P2::polar(l2, 0.0), P2::polar(l1, 0.0)),
    ];
    Ok(Pattern {
        family: "cone_frustum",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactClosedForm,
        outline,
        holes: vec![],
        conventions: vec![
            format!(
                "PLATE: FLAT SECTOR OF AN ANNULUS. ORIGIN (0, 0) = APEX OF THE DEVELOPED CONE, {} MM INSIDE THE SMALL-END ARC (NOT ON THE PLATE).",
                mm(l2)
            ),
            format!(
                "EDGES: OUTER ARC (RADIUS {} MM) = LARGE END, INNER ARC (RADIUS {} MM) = SMALL END. SEAM EDGES: THE TWO STRAIGHT EDGES, seam_a ON THE +X AXIS AND seam_b AT {} DEG.",
                mm(l1),
                mm(l2),
                mm(phi.to_degrees())
            ),
            CONE_ROLL_NOTE.into(),
            CONE_ANGLE_NOTE.into(),
            KERF_NOTE.into(),
            CONVENTION_STATUS.into(),
        ],
        roles: vec![
            ("large_end", "large end"),
            ("seam_b", "seam edge"),
            ("small_end", "small end"),
            ("seam_a", "seam edge (angle datum, +X axis)"),
        ],
        // radii: a difference, a hypot, two quotients; end points: the sector angle (five roundings on an
        // angle of up to 2·pi) times the radius, plus the trigonometric evaluation
        eps_num: l1 * U * (16.0 + 8.0 * phi),
        eps_num_basis: EPS_NUM_BASIS_EXACT,
        eps_rep: 0.0,
        eps_rep_basis: EPS_REP_EXACT,
        quantities: vec![
            ("slant_length", s),
            ("sin_half_angle", sin_a),
            ("apex_radius_large", l1),
            ("apex_radius_small", l2),
            ("sector_angle", phi),
            ("area", 0.5 * phi * (l1 * l1 - l2 * l2)),
            ("chord_large_arc", 2.0 * l1 * (phi / 2.0).sin()),
        ],
        assumptions: vec![ASSUME_REF, ASSUME_MODEL],
    })
}

/// Geometry of an oblique circular cone frustum between parallel planes.
#[derive(Clone, Copy, Debug)]
pub struct ObliqueCone {
    pub r1: f64,
    pub r2: f64,
    pub h: f64,
    pub e: f64,
}

impl ObliqueCone {
    /// Apex (a, 0, hh).
    pub fn apex(&self) -> (f64, f64) {
        let k = self.r1 / (self.r1 - self.r2);
        (self.e * k, self.h * k)
    }
    /// rho², written without cancellation: (r1 − a)² + H² + 4·a·r1·sin²(theta/2). Every term is
    /// non-negative. (The expanded form r1² − 2·a·r1·cos(theta) + a² + H² loses digits when the apex
    /// lies close to the rim; independent validation measured twice the stated error there, D4.)
    fn rho2(&self, th: f64) -> f64 {
        let (a, hh) = self.apex();
        let s = (0.5 * th).sin();
        (self.r1 - a) * (self.r1 - a) + hh * hh + 4.0 * a * self.r1 * s * s
    }
    pub fn rho(&self, th: f64) -> f64 {
        self.rho2(th).sqrt()
    }
    /// d(psi)/d(theta), Spec 02 eq. (4.1).
    pub fn dpsi(&self, th: f64) -> f64 {
        let (a, hh) = self.apex();
        let s = (0.5 * th).sin();
        let x = (self.r1 - a) + 2.0 * a * s * s; // r1 − a·cos(theta)
        self.r1 * hh.hypot(x) / self.rho2(th)
    }
}

/// Developed samples of the oblique cone: (theta, rho, psi) on a uniform grid with `steps`
/// intervals, plus the quadrature error estimate (sum over all panels of the difference between the
/// last two refinement levels of the adaptive rule). The running sum is compensated, so its rounding
/// error does not grow with the number of samples.
pub fn oblique_cone_table(g: &ObliqueCone, steps: usize) -> (Vec<(f64, f64, f64)>, f64) {
    let dth = TAU / steps as f64;
    let mut out = Vec::with_capacity(steps + 1);
    let (mut psi, mut err) = (Compensated::default(), 0.0);
    out.push((0.0, g.rho(0.0), 0.0));
    for i in 0..steps {
        let (a, b) = (i as f64 * dth, (i + 1) as f64 * dth);
        let (v, e) = gauss_adaptive(&|t| g.dpsi(t), a, b, 1e-13, 14);
        psi.add(v);
        err += e;
        out.push((b, g.rho(b), psi.value()));
    }
    (out, err)
}

/// As `oblique_cone_table` with `2*n` intervals (the grid earlier builds used).
pub fn oblique_cone_samples(g: &ObliqueCone, n: usize) -> (Vec<(f64, f64, f64)>, f64) {
    oblique_cone_table(g, 2 * n)
}

/// Oblique circular cone frustum (eccentric reducer). Spec 02 §4.3.
/// Seam at theta = 0 (the side toward which the small end is offset).
pub fn eccentric_reducer(r1: f64, r2: f64, h: f64, e: f64) -> Result<Pattern, UemError> {
    finite_pos("r1", r1)?;
    finite_pos("r2", r2)?;
    finite_pos("h", h)?;
    if !e.is_finite() || e < 0.0 {
        return Err(out_of_range(
            if e.is_finite() {
                refusal::OFFSET_NEGATIVE
            } else {
                refusal::VALUE_NOT_FINITE
            },
            "e",
            vec!["e"],
            "offset must be finite and >= 0".into(),
        ));
    }
    if r1 <= r2 {
        return Err(out_of_range(refusal::REDUCER_RADII_ORDER, "r1", vec!["r1", "r2"], "requires r1 > r2; equal radii with offset (oblique cylinder) is not implemented in this build".into()));
    }
    if (r1 - r2) / h.hypot(r1 - r2) < 1e-3 {
        return Err(out_of_range_with(
            refusal::REDUCER_NEAR_CYLINDRICAL,
            "r1-r2",
            vec!["r1", "r2", "h"],
            "near-cylindrical: apex-free method not implemented in this build".into(),
            vec![("limit", 1e-3), ("measured", (r1 - r2) / h.hypot(r1 - r2))],
        ));
    }
    let g = ObliqueCone { r1, r2, h, e };
    // the overlap test needs only the total developed angle: decide it before any refinement
    let (coarse, _) = oblique_cone_table(&g, 2 * CONE_SEGMENTS);
    let psi_coarse = coarse[2 * CONE_SEGMENTS].2;
    if psi_coarse.partial_cmp(&(TAU * FULL_TURN_LIMIT)) != Some(std::cmp::Ordering::Less) {
        return Err(not_developable(
            refusal::DEVELOPED_ANGLE_OVERLAPS,
            vec!["r1", "r2", "h", "e"],
            format!("developed angle {psi_coarse:.6} rad would overlap"),
            vec![("limit", TAU * FULL_TURN_LIMIT), ("measured", psi_coarse)],
        ));
    }
    let sample = |n: usize, sub: usize| -> Vec<P2> {
        oblique_cone_table(&g, sub * n)
            .0
            .iter()
            .map(|&(_, rho, psi)| P2::polar(rho, psi))
            .collect()
    };
    let (n, eps_rep) = choose_density(
        CONE_SEGMENTS,
        "large end of the oblique cone",
        &["r1", "r2", "h", "e"],
        &sample,
    )?;
    let (s, qerr) = oblique_cone_table(&g, 2 * n);
    let psi_total = s[2 * n].2;
    let k = r2 / r1;
    let outer: Vec<P2> = (0..=n).map(|i| P2::polar(s[2 * i].1, s[2 * i].2)).collect();
    let inner: Vec<P2> = outer
        .iter()
        .rev()
        .map(|p| P2::new(p.x * k, p.y * k))
        .collect(); // traversed back toward theta = 0
    let (a, hh) = g.apex();
    let rho_min = (r1 - a).hypot(hh);
    let rho_max = ((r1 - a) * (r1 - a) + hh * hh + 4.0 * a * r1).sqrt();
    let area = {
        let (v, _) = gauss(&|t| 0.5 * g.rho(t).powi(2) * g.dpsi(t), 0.0, TAU, 720);
        v * (1.0 - k * k)
    };
    // Rounding of the developed angle (first order): the integrand r1·hypot(H, x)/rho² has an absolute
    // error of at most r1·u·(16·(r1 + a) + 3·H)/rho_min² and a relative one of 11·u; its nodes are off
    // by at most 8·u·2·pi, which the total variation of the integrand (at most 8·f_max) turns into an
    // error of the integral; the compensated sum adds 4·u. f_max = r1·(H + r1 + a)/rho_min².
    let f_max = r1 * (hh + r1 + a) / (rho_min * rho_min);
    let psi_round = 16.0 * U * psi_total + 96.0 * U * TAU * f_max;
    // the vertex parameters theta_i = i·(2·pi/n) carry three roundings; a point moves by |dP/dtheta|
    // times that: |dP/dtheta| <= (a·r1 + r1·(H + r1 + a))/rho_min
    let param = 4.0 * U * TAU * (a * r1 + r1 * (hh + r1 + a)) / rho_min;
    let eps_num = rho_max * (qerr + psi_round + 8.0 * U) + param;
    let outline = vec![
        Curve {
            name: "large_end",
            layer: Layer::CutOuter,
            geom: Geom::Polyline {
                pts: outer.clone(),
                eps_rep,
            },
        },
        line("seam_b", outer[n], inner[0]),
        Curve {
            name: "small_end",
            layer: Layer::CutOuter,
            geom: Geom::Polyline {
                pts: inner.clone(),
                eps_rep: eps_rep * k,
            },
        },
        line("seam_a", inner[n], outer[0]),
    ];
    Ok(Pattern {
        family: "eccentric_reducer",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactNumeric,
        outline,
        holes: vec![],
        conventions: vec![
            format!("PLATE: FLAT DEVELOPMENT OF AN OBLIQUE CONE. ORIGIN (0, 0) = ITS APEX, {} MM INSIDE THE SMALL-END CURVE ALONG THE SEAM (NOT ON THE PLATE).", mm(g.rho(0.0) * k)),
            format!("EDGES: OUTER CURVE = LARGE END, INNER CURVE = SMALL END. SEAM EDGES: THE TWO STRAIGHT EDGES, seam_a ON THE +X AXIS AND seam_b AT {} DEG.", mm(psi_total.to_degrees())),
            "SEAM POSITION ON THE PART: THE GENERATOR ON THE SIDE TOWARD WHICH THE SMALL END IS OFFSET (THE SHORTEST GENERATOR WHEN THE OFFSET IS POSITIVE).".into(),
            CONE_ROLL_NOTE.into(),
            CONE_ANGLE_NOTE.into(),
            KERF_NOTE.into(),
            CONVENTION_STATUS.into(),
        ],
        roles: vec![
            ("large_end", "large end"),
            ("seam_b", "seam edge"),
            ("small_end", "small end"),
            ("seam_a", "seam edge (angle datum, +X axis, offset side)"),
        ],
        eps_num,
        eps_num_basis: EPS_NUM_BASIS_CONE,
        eps_rep,
        eps_rep_basis: EPS_REP_BASIS,
        quantities: vec![
            ("developed_angle", psi_total),
            ("rho_at_0", g.rho(0.0)),
            ("rho_at_180", g.rho(PI)),
            ("area", area),
            ("polyline_segments", n as f64),
            ("chord_error_limit", MAX_CHORD_ERROR_MM),
        ],
        assumptions: vec![
            ASSUME_REF,
            ASSUME_MODEL,
            "Wall inclination varies around an oblique cone; a uniform reference-surface offset is a simplification (Spec 02 research item R-05).",
            "Seam placed at theta = 0; no seam-placement rule applied.",
        ],
    })
}

/// Cylinder-on-cylinder branch, upper sheet. Spec 02 eq. (4.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Branch {
    /// Header contact radius.
    pub big_r: f64,
    /// Branch contact radius.
    pub r: f64,
    /// Angle between axes (rad), 0 < beta < pi.
    pub beta: f64,
    /// Offset of branch axis from header axis.
    pub delta: f64,
}

impl Branch {
    pub fn s(&self, phi: f64) -> f64 {
        let y = self.delta + self.r * phi.cos();
        ((self.big_r * self.big_r - y * y).sqrt() - self.r * phi.sin() * self.beta.cos())
            / self.beta.sin()
    }
    /// 3D point on the branch surface (header axis = x axis).
    pub fn point(&self, phi: f64, s: f64) -> [f64; 3] {
        let (sb, cb) = self.beta.sin_cos();
        [
            s * cb - self.r * phi.sin() * sb,
            self.delta + self.r * phi.cos(),
            s * sb + self.r * phi.sin() * cb,
        ]
    }
}

/// A-PRIORI BOUND on the rounding error of s(phi) as `Branch::s` evaluates it, for every phi
/// (review RB-005). First-order analysis, u = 2^-53, t = relative error of sin and cos (assumed one
/// unit in the last place, 2u):
///
///   y  = fl(delta + fl(r·cos phi))            |dy| <= r·(t + u) + u·|y|
///   D  = fl(fl(R·R) − fl(y·y))                |dD| <= 2u·R² + 2·R·|dy| + u·D  <=  4u·R² + 2·R·r·(t + u) + u·D
///   q  = fl(sqrt(D))                          |dq| <= |dD| / (2·sqrt(D)) + u·sqrt(D)
///   p  = fl(fl(r·sin phi)·cos beta)           |dp| <= r·(2t + 2u)
///   s  = fl(fl(q − p) / sin beta)             |ds| <= (|dq| + |dp| + u·|q − p|) / sin beta + |s|·(t + u)
///
/// With sqrt(D) <= R, |q − p| <= R + r and D >= R² − (|delta| + r)² this gives
///
///   |ds| <= [ (2u·R² + R·r·(t + u)) / sqrt(R² − (|delta| + r)²) + 1.5u·R + r·(2t + 2u) + (R + r)·(t + 2u) ] / sin beta.
///
/// The first term is the cancellation in R² − y² amplified by the square root; it dominates next to
/// tangency. Second-order terms are below 1e-9 of the bound inside the accepted range. Measured
/// against 60-digit arithmetic at 240,000 points next to tangency and elsewhere, the true error never
/// exceeded 0.57 of this bound. Not an interval evaluation.
pub fn s_error_bound(b: &Branch) -> f64 {
    let y_max = b.delta.abs() + b.r;
    let root = (b.big_r * b.big_r - y_max * y_max).sqrt();
    ((2.0 * U * b.big_r * b.big_r + b.big_r * b.r * (TRIG + U)) / root
        + 1.5 * U * b.big_r
        + b.r * (2.0 * TRIG + 2.0 * U)
        + (b.big_r + b.r) * (TRIG + 2.0 * U))
        / b.beta.sin().abs()
}

/// A-priori bound on the distance of a vertex of the developed saddle (r_dev·phi, s) from the exact
/// developed curve: the bound on s, the rounding of the product r_dev·phi, and the closing vertex,
/// which is placed at u = fl(2·pi·r_dev) with the height of phi = 0 (|ds/dphi| = r·|cot beta| there).
pub fn saddle_error_bound(b: &Branch, r_dev: f64) -> f64 {
    s_error_bound(b) + 2.0 * U * TAU * r_dev + 4.0 * U * TAU * b.r / b.beta.sin().abs()
}

/// A-priori bound on the distance of a vertex of the developed header opening from the exact point
/// for the ideal parameter phi_i = 2·pi·i/n. x = station + s·cos beta − r·sin phi·sin beta carries the
/// bound on s; v = R_dev·(asin(y/R) + pi) carries the conditioning of the arcsine, R/sqrt(R² − y²).
/// The parameter itself carries three roundings, and a point moves by at most |dP/dphi| times that:
/// |ds/dphi| <= (R + r)/sin beta and |d(omega)/dphi| <= 1 for every phi.
pub fn opening_error_bound(b: &Branch, r_dev_header: f64, station: f64) -> f64 {
    let (sb, cb) = (b.beta.sin().abs(), b.beta.cos().abs());
    let y_max = b.delta.abs() + b.r;
    let root = (b.big_r * b.big_r - y_max * y_max).sqrt();
    let s_abs = (b.big_r + b.r) / sb;
    let d_phi = 4.0 * U * TAU;
    let bound_x = s_error_bound(b) * cb
        + s_abs * cb * (TRIG + U)
        + b.r * (2.0 * TRIG + 2.0 * U)
        + U * (s_abs * cb + b.r)
        + U * (station.abs() + s_abs * cb + b.r)
        + (s_abs * cb + b.r) * d_phi;
    let d_y = b.r * (TRIG + U) + U * y_max;
    let d_omega = (d_y + U * b.big_r) / root + TRIG * 0.5 * PI;
    // the constant pi of binary64 is 1.3e-16 below pi; the sum omega + pi and the product add one rounding each
    let bound_v = r_dev_header * (d_omega + 1.3e-16 + 3.0 * U * PI) + r_dev_header * d_phi;
    bound_x + bound_v
}

/// Branch stub pattern: square end at distance `s_end` along the branch axis,
/// saddle cut against the header. `r_dev` is the branch development radius.
pub fn branch_stub(
    big_r: f64,
    r: f64,
    r_dev: f64,
    beta_deg: f64,
    delta: f64,
    s_end: f64,
) -> Result<Pattern, UemError> {
    branch_stub_min(big_r, r, r_dev, beta_deg, delta, s_end, SEGMENTS)
}

/// As `branch_stub`, with at least `min_segments` segments for the saddle. A joint samples both of its parts
/// with the same count, so that their vertices are the same points of the intersection curve (review RB-004).
pub fn branch_stub_min(
    big_r: f64,
    r: f64,
    r_dev: f64,
    beta_deg: f64,
    delta: f64,
    s_end: f64,
    min_segments: usize,
) -> Result<Pattern, UemError> {
    finite_pos("header_radius", big_r)?;
    finite_pos("branch_radius", r)?;
    finite_pos("branch_dev_radius", r_dev)?;
    finite_pos("s_end", s_end)?;
    if !delta.is_finite() {
        return Err(out_of_range(
            refusal::VALUE_NOT_FINITE,
            "delta",
            vec!["delta"],
            "must be finite".into(),
        ));
    }
    let beta = beta_deg.to_radians();
    if !(beta.sin() > 0.17 && beta_deg > 0.0 && beta_deg < 180.0) {
        return Err(out_of_range(if beta_deg.is_finite() { refusal::BRANCH_ANGLE_OUT_OF_RANGE } else { refusal::VALUE_NOT_FINITE }, "beta", vec!["beta_deg"], "angle between axes must be within about 10..170 deg in this build (grazing intersections are ill-conditioned; limit is an implementation choice, not an engineering rule)".into()));
    }
    if delta.abs() + r >= big_r * (1.0 - 1e-6) {
        return Err(out_of_range_with(refusal::BRANCH_TANGENT_OR_PARTIAL, "delta+r", vec!["header_radius", "branch_radius", "delta"], "requires |delta| + r < R: tangent and partial intersections (including the equal-diameter tee) are not implemented in this build".into(), vec![("limit", big_r * (1.0 - 1e-6)), ("measured", delta.abs() + r)]));
    }
    let b = Branch {
        big_r,
        r,
        beta,
        delta,
    };
    // the square end must lie beyond the saddle: decided on the base sampling, before any refinement
    let s_peak = (0..=SUB * SEGMENTS)
        .map(|i| b.s(i as f64 * (TAU / (SUB * SEGMENTS) as f64)))
        .fold(f64::MIN, f64::max);
    if s_end.partial_cmp(&s_peak) != Some(std::cmp::Ordering::Greater) {
        return Err(out_of_range(
            refusal::SQUARE_END_TOO_SHORT,
            "s_end",
            vec!["s_end"],
            format!("square end must lie beyond the saddle: s_end > {s_peak:.6}"),
        ));
    }
    // Spec 02 §4.4: the branch pattern is (r·φ, s). With φ increasing to the right and s upward the pattern
    // normal is the outward surface normal: the view from OUTSIDE (MARKING_SIDE). The saddle cut is at the
    // bottom, the square end at v = s_end. (Until engine 0.2.0 this was drawn as (r·φ, s_end − s), its
    // mirror image, while the header plate was drawn from outside: validation defect D1.)
    let sample = |n: usize, sub: usize| -> Vec<P2> {
        let dphi = TAU / (sub * n) as f64;
        (0..=sub * n)
            .map(|i| {
                let phi = i as f64 * dphi;
                P2::new(r_dev * phi, b.s(phi))
            })
            .collect()
    };
    let (n, eps_rep) = choose_density(
        min_segments.max(SEGMENTS),
        "saddle cut of the branch",
        &[
            "header_radius",
            "branch_radius",
            "branch_dev_radius",
            "beta_deg",
            "delta",
        ],
        &sample,
    )?;
    let all = sample(n, SUB);
    let s_max = all.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    let s_min = all.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    if s_end.partial_cmp(&s_max) != Some(std::cmp::Ordering::Greater) {
        return Err(out_of_range(
            refusal::SQUARE_END_TOO_SHORT,
            "s_end",
            vec!["s_end"],
            format!("square end must lie beyond the saddle: s_end > {s_max:.6}"),
        ));
    }
    let w = TAU * r_dev;
    let mut cut: Vec<P2> = (0..=n).map(|i| all[SUB * i]).collect();
    cut[n] = P2::new(w, cut[0].y); // the seam edges are the same ruling: close exactly
    let (p_top_r, p_top_l) = (P2::new(w, s_end), P2::new(0.0, s_end));
    // area between the saddle and the square end, by the trapezoid rule on the vertices themselves
    let area = w * s_end
        - cut
            .windows(2)
            .map(|q| 0.5 * (q[0].y + q[1].y) * (q[1].x - q[0].x))
            .sum::<f64>();
    // counter-clockwise: cut (left to right) -> right seam (up) -> square end (right to left) -> left seam (down)
    let outline = vec![
        Curve {
            name: "saddle_cut",
            layer: Layer::CutOuter,
            geom: Geom::Polyline {
                pts: cut.clone(),
                eps_rep,
            },
        },
        line("seam_b", cut[n], p_top_r),
        line("square_end", p_top_r, p_top_l),
        line("seam_a", p_top_l, cut[0]),
    ];
    let [delta_note, beta_note] = delta_notes(delta, beta);
    Ok(Pattern {
        family: "branch_stub",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactClosedForm,
        outline,
        holes: vec![],
        conventions: vec![
            format!("PLATE AXES: X = AROUND THE BRANCH (DEVELOPED CIRCUMFERENCE {} MM), Y = ALONG THE BRANCH AXIS (DISTANCE FROM THE HEADER AXIS).", mm(w)),
            "ROLLING: ROLL ABOUT AN AXIS PARALLEL TO Y. THE DRAWN FACE BECOMES THE OUTSIDE OF THE BRANCH.".into(),
            format!("SEAM EDGES: THE TWO EDGES PARALLEL TO Y, AT X = 0 AND X = {} (CURVES seam_a AND seam_b). SADDLE CUT (FITS THE HEADER) = BOTTOM EDGE. SQUARE END = TOP EDGE AT Y = {}.", mm(w), mm(s_end)),
            "ANGLE DATUM: PHI = X / R_DEV. PHI = 0 (THE SEAM) IS THE GENERATOR OF THE BRANCH ON THE POSITIVE-DELTA SIDE. PHI INCREASES WITH X: COUNTER-CLOCKWISE SEEN FROM THE SQUARE END LOOKING TOWARD THE HEADER.".into(),
            "PHI = 90 DEG IS THE GENERATOR ON THE SIDE AWAY FROM THE LEAN OF THE BRANCH (TOWARD DECREASING STATION FOR BETA BELOW 90 DEG), PHI = 270 DEG THE GENERATOR ON THE SIDE OF THE LEAN.".into(),
            format!("ORIGIN: X = 0 AT THE SEAM. Y = 0 IS THE LEVEL OF THE HEADER AXIS, {} MM BELOW THE LOWEST POINT OF THE SADDLE CUT (NOT ON THE PLATE).", mm(s_min)),
            delta_note,
            beta_note,
            KERF_NOTE.into(),
            CONVENTION_STATUS.into(),
        ],
        roles: vec![
            ("saddle_cut", "saddle cut (fits the header)"),
            ("seam_b", "seam edge"),
            ("square_end", "square end"),
            ("seam_a", "seam edge (angle datum, positive-delta side)"),
        ],
        eps_num: saddle_error_bound(&b, r_dev),
        eps_num_basis: EPS_NUM_BASIS_BRANCH,
        eps_rep,
        eps_rep_basis: EPS_REP_BASIS,
        quantities: vec![
            ("developed_width", w),
            ("s_min", s_min),
            ("s_max", s_max),
            ("s_at_0", all[0].y),
            ("s_at_90", all[SUB * n / 4].y),
            ("area", area),
            ("polyline_segments", n as f64),
            ("chord_error_limit", MAX_CHORD_ERROR_MM),
        ],
        assumptions: vec![
            ASSUME_REF,
            ASSUME_MODEL,
            "Saddle cut = the curve where the branch surface of the entered contact radius meets the header surface of the entered contact radius (Spec 02 §4.4). Which wall surfaces these are, and the joint type, are declarations of the user recorded with the job.",
            "The cut is square through the wall (constant height through the thickness, as a cut perpendicular to the flat plate gives). Weld preparation, bevel and root gap are not modelled.",
            "Seam at branch azimuth phi = 0.",
        ],
    })
}

/// Header shell (rolled from plate) with the opening for a branch.
/// Spec 02 §4.4: the opening is the curve where the branch surface of radius `b.r` meets the header
/// surface of radius `b.big_r`, developed on the header. Shell: x along the header axis in [0, length];
/// v = r_dev_header * (omega + pi), i.e. the longitudinal seam is diametrically
/// opposite the branch side. `station` is the header-axis position of the
/// point where the branch axis is closest to the header axis.
pub fn header_with_opening(
    b: &Branch,
    r_dev_header: f64,
    length: f64,
    station: f64,
) -> Result<Pattern, UemError> {
    header_with_opening_min(b, r_dev_header, length, station, SEGMENTS)
}

/// As `header_with_opening`, with at least `min_segments` segments for the opening (see `branch_stub_min`).
pub fn header_with_opening_min(
    b: &Branch,
    r_dev_header: f64,
    length: f64,
    station: f64,
    min_segments: usize,
) -> Result<Pattern, UemError> {
    finite_pos("header_dev_radius", r_dev_header)?;
    finite_pos("header_length", length)?;
    if !station.is_finite() {
        return Err(out_of_range(
            refusal::VALUE_NOT_FINITE,
            "branch_station",
            vec!["branch_station"],
            "must be finite".into(),
        ));
    }
    // (written with partial_cmp so that a NaN angle is refused, exactly as `!(sin > 0.17)` did)
    if b.beta.sin().partial_cmp(&0.17) != Some(std::cmp::Ordering::Greater)
        || b.delta.abs() + b.r >= b.big_r * (1.0 - 1e-6)
    {
        return Err(out_of_range_with(
            if b.beta.sin().partial_cmp(&0.17) != Some(std::cmp::Ordering::Greater) {
                refusal::BRANCH_ANGLE_OUT_OF_RANGE
            } else {
                refusal::BRANCH_TANGENT_OR_PARTIAL
            },
            "branch",
            if b.beta.sin().partial_cmp(&0.17) != Some(std::cmp::Ordering::Greater) {
                vec!["beta_deg"]
            } else {
                vec!["header_radius", "branch_radius", "delta"]
            },
            "branch geometry outside the supported range (see branch stub limits)".into(),
            if b.beta.sin().partial_cmp(&0.17) != Some(std::cmp::Ordering::Greater) {
                vec![]
            } else {
                vec![
                    ("limit", b.big_r * (1.0 - 1e-6)),
                    ("measured", b.delta.abs() + b.r),
                ]
            },
        ));
    }
    let mut shell = cylinder_shell(r_dev_header, length)?;
    // The opening is computed in the frame (x along the header axis, v around it), so the plate outline must be
    // in that same frame: length along x, developed circumference along y. Reusing the plain shell's rectangle
    // (circumference along x) put the opening in the wrong place and turned by a quarter turn; see
    // nf-core/tests/regressions/002-header-outline-frame.json.
    let w = TAU * r_dev_header;
    let (p0, p1, p2, p3) = (
        P2::new(0.0, 0.0),
        P2::new(length, 0.0),
        P2::new(length, w),
        P2::new(0.0, w),
    );
    shell.outline = vec![
        line("seam_a", p0, p1),
        line("top", p1, p2),
        line("seam_b", p2, p3),
        line("bottom", p3, p0),
    ];
    // v = r_dev·(ω + π) with ω increasing from −y to +y, x along the axis: the pattern normal is the
    // outward normal of the header, the view from OUTSIDE (MARKING_SIDE), like the branch.
    let (sb, cb) = b.beta.sin_cos();
    let sample = |n: usize, sub: usize| -> Vec<P2> {
        let dphi = TAU / (sub * n) as f64;
        (0..=sub * n)
            .map(|i| {
                let phi = i as f64 * dphi;
                let s = b.s(phi);
                let omega = ((b.delta + b.r * phi.cos()) / b.big_r).asin();
                P2::new(
                    station + s * cb - b.r * phi.sin() * sb,
                    r_dev_header * (omega + PI),
                )
            })
            .collect()
    };
    // the opening must lie inside the plate: decided on the base sampling, before any refinement
    let inside = |pts: &[P2]| -> Result<(f64, f64), UemError> {
        let (xmin, xmax) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.x), hi.max(p.x))
        });
        if xmin.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
            || xmax.partial_cmp(&length) != Some(std::cmp::Ordering::Less)
        {
            return Err(out_of_range(
                refusal::OPENING_OUTSIDE_SHELL,
                "branch_station",
                vec!["branch_station", "header_length"],
                format!(
                    "opening spans x = {xmin:.3} .. {xmax:.3} and does not lie inside the shell length {length}"
                ),
            ));
        }
        Ok((xmin, xmax))
    };
    inside(&sample(SEGMENTS, 1))?;
    let (n, eps_rep) = choose_density(
        min_segments.max(SEGMENTS),
        "branch opening of the header",
        &[
            "header_radius",
            "header_dev_radius",
            "header_length",
            "branch_station",
            "branch_radius",
            "branch_dev_radius",
            "beta_deg",
            "delta",
        ],
        &sample,
    )?;
    let mut pts = sample(n, 1);
    pts[n] = pts[0]; // close exactly
    let (xmin, xmax) = inside(&pts)?;
    let area2: f64 = pts
        .windows(2)
        .map(|w| w[0].x * w[1].y - w[1].x * w[0].y)
        .sum();
    if area2 > 0.0 {
        pts.reverse(); // inner profiles are clockwise
    }
    let perimeter: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    shell.family = "header_shell_with_opening";
    shell.holes = vec![Curve {
        name: "branch_opening",
        layer: Layer::CutInner,
        geom: Geom::Polyline { pts, eps_rep },
    }];
    shell.eps_rep = eps_rep;
    shell.eps_rep_basis = EPS_REP_BASIS;
    shell.eps_num = shell
        .eps_num
        .max(opening_error_bound(b, r_dev_header, station));
    shell.eps_num_basis = EPS_NUM_BASIS_BRANCH;
    let [delta_note, beta_note] = delta_notes(b.delta, b.beta);
    shell.conventions = vec![
        format!("PLATE AXES: X = ALONG THE HEADER AXIS (STATION, LENGTH {} MM), Y = AROUND THE HEADER (DEVELOPED CIRCUMFERENCE {} MM).", mm(length), mm(w)),
        "NOTE: ON THE BRANCH PLATE AND ON A PLAIN SHELL PLATE OF THIS BUILD THE CIRCUMFERENCE RUNS ALONG X. ON THIS HEADER PLATE IT RUNS ALONG Y.".into(),
        "ROLLING: ROLL ABOUT AN AXIS PARALLEL TO X. THE DRAWN FACE BECOMES THE OUTSIDE OF THE HEADER.".into(),
        format!("SEAM EDGES: THE TWO EDGES PARALLEL TO X, AT Y = 0 AND Y = {} (CURVES seam_a AND seam_b). THE SEAM IS DIAMETRICALLY OPPOSITE THE BRANCH SIDE.", mm(w)),
        format!("HEADER ENDS: THE EDGES AT X = 0 (CURVE bottom, THE STATION-0 END) AND X = {} (CURVE top). STATION INCREASES WITH X.", mm(length)),
        format!("BRANCH STATION {} = THE POINT OF THE HEADER AXIS CLOSEST TO THE BRANCH AXIS (AXIS WORK POINT), NOT THE CENTRE OF THE OPENING. THE OPENING SPANS X = {} TO {}.", mm(station), mm(xmin), mm(xmax)),
        format!("ANGLE DATUM: Y = {} (HALF THE CIRCUMFERENCE) IS THE GENERATOR FACING THE BRANCH. Y INCREASES TOWARD THE POSITIVE-DELTA SIDE: COUNTER-CLOCKWISE SEEN FROM THE STATION-0 END LOOKING TOWARD INCREASING STATION.", mm(0.5 * w)),
        "ORIGIN: LOWER LEFT CORNER OF THE PLATE = THE SEAM AT THE STATION-0 END.".into(),
        delta_note,
        beta_note,
        KERF_NOTE.into(),
        CONVENTION_STATUS.into(),
    ];
    shell.roles = vec![
        ("seam_a", "seam edge (Y = 0)"),
        ("top", "header end (far end, largest station)"),
        ("seam_b", "seam edge"),
        ("bottom", "header end (station 0, X = 0)"),
        ("branch_opening", "opening for the branch"),
    ];
    shell.quantities.push(("polyline_segments", n as f64));
    shell
        .quantities
        .push(("chord_error_limit", MAX_CHORD_ERROR_MM));
    shell.quantities.push(("opening_x_min", xmin));
    shell.quantities.push(("opening_x_max", xmax));
    shell.quantities.push(("opening_perimeter", perimeter));
    shell.quantities.push(("opening_area", 0.5 * area2.abs()));
    shell.assumptions.push("Header rolled from plate; longitudinal seam placed diametrically opposite the branch. No seam-to-opening clearance rule is applied.");
    shell.assumptions.push("Opening = the curve where the branch surface of the declared radius meets the header surface of the declared radius (Spec 02 §4.4), cut square through the wall. Which wall surfaces these are and the joint type are declared by the user and recorded with the job; weld preparation, root gap and bevel are not modelled.");
    Ok(shell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn cyl_reference_cases() {
        let p = cylinder_shell(500.0, 1000.0).unwrap(); // CYL-02
        assert!(close(
            p.quantity("developed_width").unwrap(),
            3141.592653590,
            1e-8
        ));
        assert!(close(
            mitre_cut_height(300.0, 100.0, 15f64.to_radians(), 0.0),
            53.58983848622,
            1e-10
        )); // MITRE-01
        assert!(close(
            mitre_cut_height(300.0, 100.0, 15f64.to_radians(), PI / 2.0),
            80.38475772934,
            1e-10
        ));
        assert!(close(
            mitre_cut_height(300.0, 100.0, 15f64.to_radians(), PI),
            107.1796769724,
            1e-9
        ));
        assert!(cylinder_shell(-1.0, 10.0).is_err());
    }

    #[test]
    fn cone_reference_case_cone_01() {
        let p = cone_frustum(200.0, 100.0, 300.0).unwrap();
        assert!(close(
            p.quantity("slant_length").unwrap(),
            316.2277660168,
            1e-9
        ));
        assert!(close(
            p.quantity("apex_radius_large").unwrap(),
            632.4555320337,
            1e-9
        ));
        assert!(close(
            p.quantity("sector_angle").unwrap(),
            1.986917653159,
            1e-11
        ));
        assert!(close(p.quantity("area").unwrap(), 298037.6479739, 1e-6));
        assert!(close(
            p.quantity("chord_large_arc").unwrap(),
            1059.892757121,
            1e-8
        ));
    }

    #[test]
    fn cone_degenerate_and_conditioning() {
        assert!(cone_frustum(100.0, 100.0, 300.0).is_err());
        assert!(matches!(
            cone_frustum(200.0, 199.9, 300.0),
            Err(UemError::OutOfRange { .. })
        )); // CONE-02 refused
        assert!(cone_frustum(200.0, 0.0, 300.0).is_err()); // DG-01
        assert!(cone_frustum(200.0, 100.0, 0.0).is_err()); // DG-02
    }

    #[test]
    fn eccentric_reference_case_ecc_01() {
        let p = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        assert!(close(
            p.quantity("developed_angle").unwrap(),
            1.861891289225,
            1e-10
        ));
        assert!(close(p.quantity("rho_at_0").unwrap(), 600.0, 1e-10));
        assert!(close(
            p.quantity("rho_at_180").unwrap(),
            721.1102550928,
            1e-9
        ));
        assert!(close(p.quantity("area").unwrap(), 304679.8420026, 1e-5));
        assert!(p.eps_rep > 0.0 && p.eps_rep < 0.05);
        let g = ObliqueCone {
            r1: 200.0,
            r2: 100.0,
            h: 300.0,
            e: 100.0,
        };
        let (s, _) = oblique_cone_samples(&g, 360);
        assert!(close(s[180].2, 0.4919675896291, 1e-11)); // psi(90 deg)
        assert!(close(s[360].2, 0.9309456446123, 1e-11)); // psi(180 deg)
    }

    #[test]
    fn eccentric_tends_to_right_cone() {
        // ECC-02: zero offset reproduces CONE-01
        let p = eccentric_reducer(200.0, 100.0, 300.0, 0.0).unwrap();
        assert!(close(
            p.quantity("developed_angle").unwrap(),
            1.986917653159,
            1e-10
        ));
        assert!(close(p.quantity("area").unwrap(), 298037.6479739, 1e-5));
    }

    #[test]
    fn branch_reference_cases() {
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: PI / 2.0,
            delta: 0.0,
        }; // BR-01
        assert!(close(b.s(0.0), 111.803398875, 1e-9));
        assert!(close(b.s(PI / 4.0), 132.2875655532, 1e-9));
        assert!(close(b.s(PI / 2.0), 150.0, 1e-9));
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 0.0,
        }; // BR-03
        assert!(close(b.s(0.0), 129.0994448736, 1e-9));
        assert!(close(b.s(PI / 4.0), 111.9276941188, 1e-9));
        assert!(close(b.s(1.5 * PI), 230.9401076759, 1e-9));
        let b = Branch {
            big_r: 150.0,
            r: 80.0,
            beta: PI / 2.0,
            delta: 40.0,
        }; // BR-04
        assert!(close(b.s(0.0), 90.0, 1e-9));
        assert!(close(b.s(0.75 * PI), 149.0821364201, 1e-9));
    }

    #[test]
    fn header_opening_is_inside_shell_and_clockwise() {
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 0.0,
        };
        let p = header_with_opening(&b, 150.0, 1200.0, 600.0).unwrap();
        assert_eq!(p.holes.len(), 1);
        let r = crate::profile::check(&p.holes, 1e-9);
        assert!(r.closed && r.self_intersections == 0 && r.signed_area < 0.0);
        assert!(
            p.quantity("opening_x_min").unwrap() > 0.0
                && p.quantity("opening_x_max").unwrap() < 1200.0
        );
        assert!(
            header_with_opening(&b, 150.0, 300.0, 100.0).is_err(),
            "opening outside the shell must be refused"
        );
    }

    #[test]
    fn branch_limits() {
        assert!(branch_stub(150.0, 100.0, 100.0, 90.0, 60.0, 400.0).is_err()); // BR-02 partial intersection
        assert!(branch_stub(100.0, 100.0, 100.0, 90.0, 0.0, 400.0).is_err()); // BR-05 equal tee not implemented
        assert!(branch_stub(150.0, 100.0, 100.0, 5.0, 0.0, 4000.0).is_err()); // grazing
        assert!(branch_stub(150.0, 100.0, 100.0, 90.0, 0.0, 100.0).is_err()); // square end inside saddle
        let p = branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap();
        assert!(close(
            p.quantity("developed_width").unwrap(),
            628.318530718,
            1e-9
        ));
    }

    /// Validation defect D6: both conical families refuse a sector that closes on itself, with the same rule.
    #[test]
    fn d6_right_cone_refuses_a_full_turn_like_the_eccentric_reducer() {
        for h in [0.01, 1e-8] {
            let e = cone_frustum(300.0, 50.0, h).unwrap_err();
            assert!(
                matches!(&e, UemError::NotDevelopable { reason: m, .. } if m.contains("would overlap")),
                "{e:?}"
            );
            let e = eccentric_reducer(300.0, 50.0, h, 0.0).unwrap_err();
            assert!(
                matches!(&e, UemError::NotDevelopable { reason: m, .. } if m.contains("would overlap")),
                "{e:?}"
            );
        }
        // just inside the limit both still develop; just beyond both refuse
        let h_at = |turn: f64| 250.0 * (1.0 - turn * turn).sqrt() / turn; // sin(alpha) = turn
        assert!(
            cone_frustum(300.0, 50.0, h_at(0.9985)).is_ok()
                && eccentric_reducer(300.0, 50.0, h_at(0.9985), 0.0).is_ok()
        );
        assert!(
            cone_frustum(300.0, 50.0, h_at(0.9995)).is_err()
                && eccentric_reducer(300.0, 50.0, h_at(0.9995), 0.0).is_err()
        );
    }

    /// Validation defect D3: the representation estimate near tangency. It is an estimate (three interior
    /// samples per segment); this measures it against a dense sampling of the exact curve.
    #[test]
    fn d3_representation_estimate_near_tangency_is_close_to_the_dense_truth() {
        for (r, mid_only_ratio) in [(150.0 * (1.0 - 1.1e-6), 1.21), (150.0 * (1.0 - 1e-5), 1.04)] {
            let b = Branch {
                big_r: 150.0,
                r,
                beta: 45f64.to_radians(),
                delta: 0.0,
            };
            let p = branch_stub(150.0, r, r, 45.0, 0.0, 600.0).unwrap();
            // dense truth: 64 samples per segment against the chord
            let (n, m) = (p.quantity("polyline_segments").unwrap() as usize, 64);
            let dphi = TAU / (n * m) as f64;
            let dense: Vec<P2> = (0..=n * m)
                .map(|i| P2::new(r * i as f64 * dphi, b.s(i as f64 * dphi)))
                .collect();
            let mut truth: f64 = 0.0;
            for seg in 0..n {
                let (a, c) = (dense[seg * m], dense[(seg + 1) * m]);
                let (dx, dy) = (c.x - a.x, c.y - a.y);
                for k in 1..m {
                    let q = dense[seg * m + k];
                    truth = truth.max(((q.x - a.x) * dy - (q.y - a.y) * dx).abs() / dx.hypot(dy));
                }
            }
            let ratio = truth / p.eps_rep;
            assert!(
                ratio >= 1.0 - 1e-9,
                "an estimate from fewer samples cannot exceed the dense value"
            );
            assert!(
                ratio < 1.08,
                "estimate {} against dense {truth}: ratio {ratio} (the midpoint-only estimate was off by {mid_only_ratio})",
                p.eps_rep
            );
        }
    }

    /// Dense truth: largest distance from the exact curve to the polyline, `m` samples per segment.
    fn dense_deviation(pts: &[P2], exact: &dyn Fn(f64) -> P2, m: usize) -> f64 {
        let n = pts.len() - 1;
        let mut worst: f64 = 0.0;
        for seg in 0..n {
            for k in 1..m {
                let t = (seg as f64 + k as f64 / m as f64) / n as f64;
                worst = worst.max(seg_distance(exact(t), pts[seg], pts[seg + 1]));
            }
        }
        worst
    }

    fn polyline<'a>(p: &'a Pattern, name: &str) -> &'a [P2] {
        match &p
            .outline
            .iter()
            .chain(p.holes.iter())
            .find(|c| c.name == name)
            .unwrap()
            .geom
        {
            Geom::Polyline { pts, .. } => pts,
            _ => panic!("{name} is not a polyline"),
        }
    }

    /// Review RB-004: the polyline density follows a stated maximum chord error, the achieved error is
    /// measured against the exact curve and recorded, and a part that cannot be represented within the
    /// limit is refused. Before the fix every curve had 720 (or 360) segments whatever its size: 0.03 mm
    /// for an ordinary joint of radius 1500 and 0.21 mm for the reviewer's near-tangent case.
    #[test]
    fn rb004_polyline_density_follows_the_stated_maximum_chord_error() {
        let segs = |p: &Pattern| p.quantity("polyline_segments").unwrap() as usize;
        assert_eq!(MAX_CHORD_ERROR_MM, 0.01);
        // an ordinary part needs no refinement, and says which limit applied
        let small = branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap();
        assert!(segs(&small) == SEGMENTS && small.eps_rep <= MAX_CHORD_ERROR_MM);
        assert_eq!(
            small.quantity("chord_error_limit"),
            Some(MAX_CHORD_ERROR_MM)
        );
        assert!(
            small.eps_rep_basis.contains("0.01 mm")
                && small.eps_rep_basis.contains("implementation limit")
        );
        // the same shape twenty times larger had 0.026 mm at 720 segments: refined until the limit holds
        let b = Branch {
            big_r: 3000.0,
            r: 2000.0,
            beta: 60f64.to_radians(),
            delta: 0.0,
        };
        let coarse = {
            let all: Vec<P2> = (0..=SUB * SEGMENTS)
                .map(|i| {
                    let phi = i as f64 * TAU / (SUB * SEGMENTS) as f64;
                    P2::new(2000.0 * phi, b.s(phi))
                })
                .collect();
            chord_deviation(&all, SUB)
        };
        assert!(
            coarse > 2.0 * MAX_CHORD_ERROR_MM,
            "the unrefined polyline is {coarse} mm off: this is what the fix removes"
        );
        let large = branch_stub(3000.0, 2000.0, 2000.0, 60.0, 0.0, 8000.0).unwrap();
        assert!(
            segs(&large) > SEGMENTS && large.eps_rep <= MAX_CHORD_ERROR_MM,
            "{} segments, {} mm",
            segs(&large),
            large.eps_rep
        );
        let header = header_with_opening(&b, 3000.0, 24000.0, 12000.0).unwrap();
        assert!(segs(&header) > SEGMENTS && header.eps_rep <= MAX_CHORD_ERROR_MM);
        // the recorded figure is the truth: a 64-sample measurement against the exact curve agrees with it
        let truth = dense_deviation(
            polyline(&large, "saddle_cut"),
            &|t| P2::new(2000.0 * TAU * t, b.s(TAU * t)),
            64,
        );
        assert!(
            truth <= MAX_CHORD_ERROR_MM
                && truth <= large.eps_rep * 1.01
                && truth >= large.eps_rep * 0.9,
            "recorded {} against dense {truth}",
            large.eps_rep
        );
        // the reviewer's case (R 1000, delta -300, beta 15 deg): 0.21 mm at gap 1e-3 and 2.41 mm at gap 1.1e-6
        for gap in [1e-3, 1e-4] {
            let r = 1000.0 * (1.0 - gap) - 300.0;
            let near = branch_stub(1000.0, r, r, 15.0, -300.0, 9000.0).unwrap();
            assert!(
                near.eps_rep <= MAX_CHORD_ERROR_MM && segs(&near) > 4 * SEGMENTS,
                "gap {gap}: {} mm with {} segments",
                near.eps_rep,
                segs(&near)
            );
            let g = Branch {
                big_r: 1000.0,
                r,
                beta: 15f64.to_radians(),
                delta: -300.0,
            };
            let truth = dense_deviation(
                polyline(&near, "saddle_cut"),
                &|t| P2::new(r * TAU * t, g.s(TAU * t)),
                48,
            );
            assert!(
                truth <= MAX_CHORD_ERROR_MM * 1.001,
                "gap {gap}: dense truth {truth}"
            );
        }
        // an oblique cone is refined the same way (it had 360 segments whatever its size)
        let ecc = eccentric_reducer(2000.0, 1000.0, 3000.0, 1000.0).unwrap();
        assert!(
            segs(&ecc) > CONE_SEGMENTS && ecc.eps_rep <= MAX_CHORD_ERROR_MM,
            "{} segments, {} mm",
            segs(&ecc),
            ecc.eps_rep
        );
        // what cannot be represented within the limit with the largest density is refused, not drawn coarser
        let r = 1e6 * (1.0 - 1.1e-6);
        let e = branch_stub(1e6, r, r, 15.0, 0.0, 1e7).unwrap_err();
        assert!(
            matches!(&e, UemError::OutOfRange { param: "chord_error", reason, .. } if reason.contains("0.01 mm") && reason.contains("implementation limit")),
            "{e:?}"
        );
        // vertices stay uniform in the angle whatever the density (phi = 0, 90, 180, 270 deg are vertices,
        // which puts a vertex on each tangent parameter, Spec 02 §4.4)
        let cut = polyline(&large, "saddle_cut");
        let n = segs(&large);
        for (i, q) in cut.iter().enumerate().step_by(n / 8) {
            assert!((q.x - 2000.0 * TAU * i as f64 / n as f64).abs() < 1e-9);
        }
    }

    /// Review RB-005 / validation D4: eps_num of a branch is an a-priori bound that does not depend on
    /// s_end and covers the errors measured against high-precision arithmetic. The earlier figure,
    /// 16·eps·(R + s_end)/sin(beta), was exceeded 1.3 to 6 times next to tangency.
    #[test]
    fn rb005_eps_num_of_a_branch_is_an_a_priori_bound() {
        // (r, beta, s_end, error measured against 50-digit arithmetic by reviewer B / validation D4, in mm)
        for (r, beta, s_end, measured) in [
            (149.99978, 90.0, 151.0, 2.0e-12),
            (149.9997, 90.0, 160.0, 1.4e-12),
            (149.999835, 45.0, 600.0, 1.6e-12),
            (149.9985, 45.0, 600.0, 1.2e-12),
            (149.9997, 90.0, 400.0, 1.4e-11), // the largest error validation D4 reported at R 150
        ] {
            let p = branch_stub(150.0, r, r, beta, 0.0, s_end).unwrap();
            let b = Branch {
                big_r: 150.0,
                r,
                beta: f64::to_radians(beta),
                delta: 0.0,
            };
            let old = 16.0 * f64::EPSILON * (150.0 + s_end) / b.beta.sin();
            assert!(
                p.eps_num >= s_error_bound(&b) && p.eps_num >= measured,
                "r {r}: stated {} against measured {measured}",
                p.eps_num
            );
            if s_end < 200.0 {
                assert!(
                    old < measured,
                    "the earlier estimate ({old}) was below the measured error ({measured}): this is defect D4"
                );
            }
            let longer = branch_stub(150.0, r, r, beta, 0.0, s_end * 10.0).unwrap();
            assert_eq!(
                longer.eps_num, p.eps_num,
                "the bound does not grow with s_end"
            );
            assert!(
                p.eps_num_basis.contains("A-priori bound")
                    && p.eps_num_basis.contains("not interval-certified")
            );
        }
        // the bound is the formula of the derivation, term by term
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 20.0,
        };
        let (u, t) = (f64::EPSILON / 2.0, f64::EPSILON);
        let root = (150.0f64 * 150.0 - 120.0 * 120.0).sqrt();
        let want = ((2.0 * u * 150.0 * 150.0 + 150.0 * 100.0 * (t + u)) / root
            + 1.5 * u * 150.0
            + 100.0 * (2.0 * t + 2.0 * u)
            + 250.0 * (t + 2.0 * u))
            / b.beta.sin();
        assert!((s_error_bound(&b) - want).abs() <= 1e-12 * want);
        // at the refusal limit the cancellation term dominates: about 3.5e3·u·R / sin(beta)
        let b = Branch {
            big_r: 150.0,
            r: 150.0 * (1.0 - 1.01e-6),
            beta: 90f64.to_radians(),
            delta: 0.0,
        };
        assert!(
            s_error_bound(&b) > 5.0e-11 && s_error_bound(&b) < 7.0e-11,
            "{}",
            s_error_bound(&b)
        );
        // the header opening carries the same bound through x and the arcsine
        let h = header_with_opening(&b, 150.0, 2000.0, 1000.0).unwrap();
        assert!(
            h.eps_num >= s_error_bound(&b) * 0.0 && h.eps_num > 1e-11 && h.eps_num < 1e-9,
            "{}",
            h.eps_num
        );
    }

    /// The developed angle of an oblique cone whose apex lies close to its rim (validation D4, second
    /// case): the fixed 5-point rule did not resolve the integrand and its own estimate did not say so.
    #[test]
    fn d4_flat_oblique_reducer_is_integrated_to_its_stated_error() {
        // The case of validation report section 5, D4. Reference: mpmath at 40 digits with the interval split
        // where r1 = a·cos(theta): 1.27928855381558597808 (computed for this fix pass; the value printed in
        // the report, 1.2792885578, was itself 4e-9 off). The fixed rule gave 1.2792885609.
        const REFERENCE: f64 = 1.279_288_553_815_586;
        let (r1, r2, h, e) = (
            5.90942068969116,
            2.061326520132905,
            0.024581416189856813,
            12.239738052541922,
        );
        let g = ObliqueCone { r1, r2, h, e };
        let fixed = (0..720)
            .map(|i| {
                gauss(
                    &|t| g.dpsi(t),
                    i as f64 * TAU / 720.0,
                    (i + 1) as f64 * TAU / 720.0,
                    1,
                )
                .0
            })
            .sum::<f64>();
        let (table, est) = oblique_cone_table(&g, 720);
        let adaptive = table[720].2;
        assert!(
            (fixed - REFERENCE).abs() > 1e-9,
            "the fixed rule is off by {}",
            (fixed - REFERENCE).abs()
        );
        assert!((adaptive - REFERENCE).abs() < 1e-14, "{adaptive}");
        assert!(
            est < 1e-11 && (adaptive - REFERENCE).abs() <= est + 1e-14,
            "estimate {est}"
        );
        // and the pattern states an error that covers it
        let p = eccentric_reducer(r1, r2, h, e).unwrap();
        assert!(
            (p.quantity("developed_angle").unwrap() - REFERENCE).abs() * 30.0 <= p.eps_num,
            "{}",
            p.eps_num
        );
        // no cancellation in rho² next to the rim
        let near = ObliqueCone {
            r1: 100.0,
            r2: 50.0,
            h: 1e-3,
            e: 50.0 - 1e-9,
        };
        let (a, hh) = near.apex();
        assert!((near.rho(0.0) - (100.0 - a).hypot(hh)).abs() <= 1e-15 * near.rho(0.0));
    }
}
