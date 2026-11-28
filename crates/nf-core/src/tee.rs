//! NFJ-1 S1-L7a: tee (T-pipe) main with branches and the branch piece: set-on / set-in, offset `e`, direction `dir`,
//! several branches on one main, longitudinal seam rule. Pure functions, no I/O, no engine state.
//!
//! Status (stated, not hidden): NOT reachable from `job::run`. No `Request` variant, no `Pattern`, no DXF and no AMOEBA
//! digest is produced from it, so nothing computed here can be served as a cut file. `ENGINE_VERSION` is bumped by
//! L11 (0.6.0). Thin openings and perpendicular cuts carry certified bounds where they can be proved; a result is
//! `certified` only when every drawn hole is. It lists the named `rules` it applied (all UNVERIFIED, none confirmed
//! by a factory).
//!
//! Frame (ZK `shared.js` teeY / teeHoleA / branchPiece / teeMainPiece, re-derived here, tests are independent):
//! the main axis is `x`, azimuth `alpha` is 0 at the top, 90 right, 180 bottom, 270 left seen from the start end.
//! A branch stands perpendicular on the main; its centre line is `e` off the main centre line (positive towards
//! increasing azimuth) and at azimuth `dir`. `theta` runs round the branch (0 = along the main axis, 90 deg = the
//! side). Circumferences are measured at the plate mid surface (rule `TEE_CIRC_AT_MID`).
//!
//! Exactness: the contact heights, the cut lengths at any angle, the hole outline (as a function) and the
//! feasibility limits are exact closed forms. The drawn hole is a 360 point polyline and the branch cut a 360 point
//! polyline (ZK parity); `eps_rep` is an ESTIMATE of the polyline-to-curve distance (edge mid-point sagitta), not a
//! bound. The branch net area is an adaptive Simpson integral with its a-posteriori estimate (`area_err`, refused
//! above `AREA_REL_TOL`). The nearest distance between two holes or two welds is a coarse-then-fine search: an
//! estimate (`certified: false`), not interval-certified.
//!
//! L7b adds the lateral (angled) branch: `angle_deg` in (0, 90] is the sharp angle between the main axis and the branch
//! axis (90 or absent = the perpendicular branch, which takes the unchanged L7a code path). The lateral cut and hole use
//! the closed forms of `cut_tau` / `lat_span`; the hole through the main wall is the rule `TEE_LATERAL_HOLE_THROUGH_WALL`
//! (UNVERIFIED). A branch below 30 deg is built but held (`tee_lateral_angle_below_30`, blocks the DXF). Pads and their
//! split are in `tee_pad.rs`. Not here: the JIS table and the order layer (grouping, notes, holds from the drawing).

use crate::shapes::{ShapeRefusal, adaptive_simpson, mass_kg, refuse};
use std::f64::consts::PI;
use uem::P2;
use uem::certify::{
    CurveEnclosure, L11_MAX_CHORD_ERROR_MM, certified_rep, clamp, max0, sqrt_nonneg,
};
use uem::interval::Iv;

const MAX_MM: f64 = crate::shapes::MAX_MM;
const MIN_MM: f64 = crate::shapes::MIN_MM;
/// Relative acceptance of the branch-area quadrature estimate.
pub const AREA_REL_TOL: f64 = 1e-9;

/// Method tag carried by every result.
pub const TEE_METHOD: &str = "closed_form_contact_curves+sampled_polylines+adaptive_simpson_branch_area+searched_gaps+L11_interval_bound_on_certified_curves";
/// Hole and cut polyline sizes (ZK parity) and the number of marking points round a branch.
pub const TEE_POLY_SAMPLES: usize = 360;
pub const TEE_MARKS: usize = 16;
/// IMPLEMENTATION LIMITS (not fabrication rules, UNVERIFIED): branches per main, longitudinal seams per main.
pub const MAX_TEE_BRANCHES: usize = 16;
/// A lateral branch below this sharp angle is built but held (`tee_lateral_angle_below_30`).
pub const LATERAL_MIN_ANGLE_DEG: f64 = 30.0;
/// Polyline points round a lateral branch cut (the perpendicular branch keeps `TEE_POLY_SAMPLES`).
pub const TEE_LATERAL_POLY_SAMPLES: usize = 720;
pub const MAX_TEE_SEAMS: u32 = 12;
/// RULE `TEE_SEAM_FROM_BOTTOM_DEG`, UNVERIFIED (ZK parity): longitudinal seam 45 deg from the bottom (ground side).
pub const TEE_SEAM_DEG: f64 = 45.0;
/// RULE `TEE_SEAM_GAP_T`, UNVERIFIED (ZK parity): clear distance kept between a seam and a hole edge, in plate thicknesses.
pub const TEE_SEAM_GAP_T: f64 = 5.0;
/// Question text attached when a branch position was not on the drawing.
pub const TEE_X_QUESTION: &str =
    "branch position along the main not on the drawing: placed at half the length, confirm";

/// (name, value, kind, status)
pub const TEE_NAMED_RULES: [(&str, &str, &str, &str); 11] = [
    (
        "TEE_MOUNT_DEFAULT",
        "set-on (branch rests on the main)",
        "rule (joint type; a drawing note wins)",
        "UNVERIFIED",
    ),
    (
        "TEE_HOLE_SURFACE",
        "hole cut where the bore (set-on: inner radius, set-in: outer radius of the branch) meets the main INNER surface; a hole that the bore cannot keep on the inner surface (offset beyond the bore) is cut through the whole wall",
        "rule (hole construction)",
        "UNVERIFIED",
    ),
    (
        "TEE_BRANCH_CUT_SURFACE",
        "branch cut at the highest point where the branch wall meets the main outer (set-on) or inner (set-in) surface",
        "rule (cut construction)",
        "UNVERIFIED",
    ),
    (
        "TEE_CIRC_AT_MID",
        "circumferences measured at the plate mid surface",
        "rule (basis)",
        "UNVERIFIED",
    ),
    (
        "TEE_SEAM_FROM_BOTTOM_DEG",
        "45",
        "rule (seam azimuth, from the bottom)",
        "UNVERIFIED",
    ),
    (
        "TEE_SEAM_GAP_T",
        "5 x t",
        "rule (clear distance seam to hole)",
        "UNVERIFIED",
    ),
    (
        "TEE_BRANCH_X_DEFAULT",
        "hole centred on the main length (with a question)",
        "rule (position fallback)",
        "UNVERIFIED",
    ),
    (
        "TEE_E_SIGN",
        "offset positive towards increasing azimuth; a stated offset side must be perpendicular to the branch direction",
        "rule (sign)",
        "UNVERIFIED",
    ),
    (
        "TEE_LATERAL_HOLE_THROUGH_WALL",
        "a lateral branch (angle below 90 deg) cuts its hole so that the bore touches the main wall nowhere across the whole wall thickness (outermost envelope over the wall); the perpendicular hole rule is unchanged",
        "rule (hole construction)",
        "UNVERIFIED",
    ),
    (
        "TEE_LATERAL_MIN_ANGLE_DEG",
        "30 (a lateral branch below this is built but held for confirmation)",
        "rule (hold threshold)",
        "UNVERIFIED",
    ),
    (
        "TEE_LIMITS",
        "16 branches, 12 seams, 360 point polylines",
        "implementation limit",
        "UNVERIFIED",
    ),
];

/// Stable refusal codes of this module (documented in docs/backend/REFUSAL_CODES.md when wired into the engine).
pub const TEE_REFUSAL_CODES: [&str; 18] = [
    "tee_value_not_finite",
    "tee_value_not_positive",
    "tee_value_too_large",
    "tee_wall_too_thick",
    "tee_no_branch",
    "tee_branches_above_limit",
    "tee_seams_out_of_range",
    "tee_seam_angle_invalid",
    "tee_branch_angle_invalid",
    "tee_branch_too_large_to_insert",
    "tee_branch_bore_beyond_main",
    "tee_branch_end_inside_main",
    "tee_hole_off_pipe",
    "tee_hole_beyond_pipe_end",
    "tee_holes_overlap",
    "tee_offset_side_not_perpendicular",
    "tee_offset_negative_without_side",
    "tee_rib_on_lateral_unsupported",
];
/// Codes that are not in the list above but are raised by the shared numerics.
pub const TEE_EXTRA_REFUSAL_CODES: [&str; 2] = ["tee_area_not_converged", "tee_result_not_finite"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mount {
    /// Branch rests on the main (rule default).
    SetOn,
    /// Branch is inserted into the main.
    SetIn,
}

/// Where the seam angle is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeeSeamFrom {
    Bottom,
    Top,
}

/// One branch standing on the main.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchOnMain {
    pub name: String,
    pub d: f64,
    pub t: f64,
    /// Distance from the start end of the main to the branch centre line. `None`: half the main length (question).
    pub x: Option<f64>,
    /// Azimuth of the branch (deg): 0 top, 90 right, 180 bottom, 270 left.
    pub dir_deg: f64,
    /// Signed offset of the branch centre line from the main centre line (positive towards increasing azimuth).
    pub e: f64,
    /// Sharp angle between the axes (deg), 0 < angle <= 90. `None` or 90: perpendicular. Below 90: lateral. Above 90 is
    /// refused (give the sharp angle; 135 is 45).
    pub angle_deg: Option<f64>,
}

impl BranchOnMain {
    pub fn new(name: &str, d: f64, t: f64, x: Option<f64>, dir_deg: f64, e: f64) -> Self {
        BranchOnMain {
            name: name.to_string(),
            d,
            t,
            x,
            dir_deg,
            e,
            angle_deg: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeeMainSpec {
    pub d: f64,
    pub t: f64,
    pub l: f64,
    pub branches: Vec<BranchOnMain>,
    pub mount: Mount,
    pub seam_from: TeeSeamFrom,
    pub seam_deg: f64,
    /// The person put the seam on the mirrored side (right instead of left of the rule).
    pub seam_mirror: bool,
    /// Longitudinal seams (= plates the main is rolled from), 1..=`MAX_TEE_SEAMS`.
    pub seams: u32,
    /// Clear distance seam to hole; `None`: `TEE_SEAM_GAP_T` x t.
    pub gap: Option<f64>,
}

impl TeeMainSpec {
    /// Named-rule defaults: set-on, seam 45 deg from the bottom, one seam, gap 5 t.
    pub fn new(d: f64, t: f64, l: f64, branches: Vec<BranchOnMain>) -> Self {
        TeeMainSpec {
            d,
            t,
            l,
            branches,
            mount: Mount::SetOn,
            seam_from: TeeSeamFrom::Bottom,
            seam_deg: TEE_SEAM_DEG,
            seam_mirror: false,
            seams: 1,
            gap: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BranchPieceSpec {
    pub d: f64,
    pub t: f64,
    pub main_d: f64,
    pub main_t: f64,
    /// Length from the main centre line to the branch end along the branch axis.
    pub h: f64,
    pub e: f64,
    pub mount: Mount,
    pub angle_deg: Option<f64>,
}

// ---------------------------------------------------------------- validation helpers

fn refusal(
    code: &'static str,
    fields: Vec<&'static str>,
    message: impl Into<String>,
) -> ShapeRefusal {
    ShapeRefusal {
        code,
        fields,
        message: message.into(),
    }
}

/// A finite dimension in (MIN_MM, MAX_MM].
fn dim(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "tee_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v <= 0.0 || v < MIN_MM {
        return Err(refuse(
            "tee_value_not_positive",
            name,
            format!("{name} must be greater than 0 (got {v})"),
        ));
    }
    if v > MAX_MM {
        return Err(refuse(
            "tee_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

/// A finite signed value with magnitude at most MAX_MM.
fn signed(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "tee_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v.abs() > MAX_MM {
        return Err(refuse(
            "tee_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

fn wall(name_t: &'static str, name_d: &'static str, t: f64, d: f64) -> Result<(), ShapeRefusal> {
    if t >= d / 2.0 {
        return Err(refusal(
            "tee_wall_too_thick",
            vec![name_t, name_d],
            format!("{name_t} must be less than half of {name_d} (t {t}, d {d})"),
        ));
    }
    Ok(())
}

/// Branch angle: absent or 90 is the perpendicular branch; below 90 (and above 0) is lateral; anything else is
/// invalid (obtuse angles are refused, not converted).
pub(crate) fn check_angle(a: Option<f64>) -> Result<(), ShapeRefusal> {
    let Some(a) = a else { return Ok(()) };
    if !(a.is_finite() && a > 0.0 && a <= 90.0) {
        return Err(refuse(
            "tee_branch_angle_invalid",
            "angle_deg",
            format!("branch angle must be greater than 0 and at most 90 deg (got {a})"),
        ));
    }
    Ok(())
}

/// (cos, sin) of the sharp angle; (0, 1) for the perpendicular branch (ZK `latCS`).
pub(crate) fn lat_cs(a: Option<f64>) -> (f64, f64) {
    match a {
        Some(a) if a * PI / 180.0 < PI / 2.0 - 1e-12 => {
            let r = a * PI / 180.0;
            (r.cos(), r.sin())
        }
        _ => (0.0, 1.0),
    }
}

/// True when the angle makes a lateral (below 90 deg) branch.
pub fn is_lateral(a: Option<f64>) -> bool {
    lat_cs(a).0 != 0.0
}

/// Offset side stated on the drawing: a non-zero offset whose stated side is not perpendicular to the branch direction
/// is refused; otherwise the signed offset (positive towards increasing azimuth). ZK `signedE`. No side stated: a
/// non-negative offset is taken towards increasing azimuth (the rule, with a note for the caller to attach); a negative
/// offset without a side is refused (`tee_offset_negative_without_side`), never abs()'d.
pub fn signed_offset(e: f64, side_deg: Option<f64>, dir_deg: f64) -> Result<f64, ShapeRefusal> {
    let e = signed("e", e)?;
    signed("dir_deg", dir_deg)?;
    let Some(s) = side_deg else {
        if e < 0.0 {
            return Err(refuse(
                "tee_offset_negative_without_side",
                "e",
                format!(
                    "a negative offset ({e}) needs the side it lies on; a sign without a side is not used (give e_side_deg or a positive e)"
                ),
            ));
        }
        return Ok(e);
    };
    let e = e.abs();
    signed("e_side_deg", s)?;
    if e == 0.0 {
        return Ok(0.0);
    }
    let d = ((s - dir_deg) % 360.0 + 360.0) % 360.0;
    if (d - 90.0).abs() < 1.0 {
        Ok(e)
    } else if (d - 270.0).abs() < 1.0 {
        Ok(-e)
    } else {
        Err(refuse(
            "tee_offset_side_not_perpendicular",
            "e_side_deg",
            format!(
                "offset side {s} deg is not perpendicular to the branch direction {dir_deg} deg"
            ),
        ))
    }
}

// ---------------------------------------------------------------- closed forms

pub(crate) fn clamp1(v: f64) -> f64 {
    v.clamp(-1.0, 1.0)
}
pub(crate) fn mod_p(a: f64, m: f64) -> f64 {
    a - m * (a / m).floor()
}
fn wrap_a(a: f64, c: f64) -> f64 {
    a - 2.0 * PI * ((a - c + PI) / (2.0 * PI)).floor()
}

/// Height (above the main centre line) of the highest point where the branch wall (radii `ri..ro`) meets the main
/// surface of radius `rc`, at branch angle `th`, offset `e`. The wall crosses the surface at lateral position
/// `y = e + rho sin(th)` for rho in [ri, ro]; the height of the surface there is `sqrt(rc^2 - y^2)`, highest where
/// `|y|` is smallest (0 when the wall straddles the centre plane).
pub fn tee_y(rc: f64, ri: f64, ro: f64, e: f64, th: f64) -> f64 {
    let a = e + ri * th.sin();
    let b = e + ro * th.sin();
    let m = if a * b <= 0.0 {
        0.0
    } else {
        a.abs().min(b.abs())
    };
    (rc * rc - m * m).max(0.0).sqrt()
}

/// Position along the branch axis (from the intersection point of the axes) of the first point where the branch wall
/// (radii `ri..ro`) meets the main surface of radius `rc`, at angle `th` round the branch (0 = the sharp-angle side,
/// 90 deg = the side), offset `e`, sharp angle `alpha` (`None` or 90: the perpendicular `tee_y`, unchanged).
///
/// Closed form: `tau(th, rho) = (sqrt(rc^2 - z^2) + rho cos(th) cos(a)) / sin(a)` with `z = e + rho sin(th)` is concave
/// in `rho`, so its maximum over the wall is the clamped stationary point `z* = sign(k s) |k| rc / sqrt(s^2 + k^2)`
/// (`k = cos(th) cos(a)`, `s = sin(th)`), kept inside the wall and inside `|z| <= rc`.
pub fn cut_tau(rc: f64, ri: f64, ro: f64, e: f64, th: f64, alpha: Option<f64>) -> f64 {
    let (ca, sa) = lat_cs(alpha);
    if ca == 0.0 {
        return tee_y(rc, ri, ro, e, th);
    }
    let (s, c) = (th.sin(), th.cos());
    let k = c * ca;
    let (mut lo, mut hi) = (ri, ro);
    if s.abs() > 1e-15 {
        let (b1, b2) = ((-rc - e) / s, (rc - e) / s);
        lo = lo.max(b1.min(b2));
        hi = hi.min(b1.max(b2));
    }
    let rho = if s.abs() <= 1e-15 {
        if k > 0.0 { hi } else { lo }
    } else if lo > hi {
        (lo + hi) / 2.0
    } else {
        let zs = if k.abs() < 1e-15 {
            0.0
        } else {
            (if k * s > 0.0 { 1.0 } else { -1.0 }) * k.abs() * rc / (s * s + k * k).sqrt()
        };
        ((zs - e) / s).clamp(lo, hi)
    };
    let z = e + rho * s;
    ((rc * rc - z * z).max(0.0).sqrt() + rho * c * ca) / sa
}

/// Span (axial `x` from the intersection point) the bore `rh` leaves through the main wall (radii `r1..r2`) at
/// azimuth `phi` from the branch direction, for the sharp angle (cos `ca`, sin `sa`; (0, 1) = perpendicular). ZK
/// `latSpan`.
fn lat_span(r1: f64, r2: f64, rh: f64, e: f64, ca_sa: (f64, f64), phi: f64) -> Option<[f64; 2]> {
    let (ca, sa) = ca_sa;
    let (s, c) = (phi.sin(), phi.cos());
    let (mut lo, mut hi) = (r1, r2);
    let kk = c * ca;
    let d = (s * s + kk * kk).sqrt();
    if s.abs() > 1e-15 {
        let (b1, b2) = ((e - rh) / s, (e + rh) / s);
        lo = lo.max(b1.min(b2));
        hi = hi.min(b1.max(b2));
    } else if e.abs() > rh {
        return None;
    }
    if lo > hi + 1e-9 {
        return None;
    }
    hi = hi.max(lo);
    let at = |r: f64, sg: f64| {
        let u = r * s - e;
        (r * kk + sg * (rh * rh - u * u).max(0.0).sqrt()) / sa
    };
    let pick = |sg: f64| {
        if s.abs() <= 1e-15 {
            if sg * kk >= 0.0 { hi } else { lo }
        } else {
            ((sg * (if s > 0.0 { 1.0 } else { -1.0 }) * kk * rh / d + e) / s).clamp(lo, hi)
        }
    };
    Some([at(pick(-1.0), -1.0), at(pick(1.0), 1.0)])
}

pub(crate) struct ThickHole {
    pub(crate) ring: Vec<[f64; 2]>,
    pub(crate) p0: f64,
    pub(crate) p1: f64,
    pub(crate) xmin: f64,
    pub(crate) xmax: f64,
    pub(crate) edge: f64,
    pub(crate) edge_a: f64,
    pub(crate) r1: f64,
    pub(crate) r2: f64,
    pub(crate) rh: f64,
    pub(crate) e: f64,
    pub(crate) ca_sa: (f64, f64),
    /// ESTIMATE of the ring-to-curve distance: largest axial deviation of the exact outline at the mid angle of every
    /// ring edge from the edge's chord.
    pub(crate) eps: f64,
}

impl ThickHole {
    fn span(&self, q: f64) -> Option<[f64; 2]> {
        lat_span(self.r1, self.r2, self.rh, self.e, self.ca_sa, q)
    }
}

/// Hole through the whole main wall (offset larger than the bore, e.g. a drain branch on the invert). ZK `latHole`.
pub(crate) fn lat_hole(r1: f64, r2: f64, rh: f64, e: f64, ca_sa: (f64, f64)) -> Option<ThickHole> {
    let n = 180usize;
    let d0 = if e - rh < 0.0 { r1 } else { r2 };
    let d1 = if e + rh > 0.0 { r1 } else { r2 };
    let p0 = clamp1((e - rh) / d0).asin();
    let p1 = clamp1((e + rh) / d1).asin();
    let (mut lo, mut hi) = (Vec::new(), Vec::new());
    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..=n {
        let p = p0 + (p1 - p0) * (1.0 - (PI * i as f64 / n as f64).cos()) / 2.0;
        if let Some(sp) = lat_span(r1, r2, rh, e, ca_sa, p) {
            lo.push([p, sp[0]]);
            hi.push([p, sp[1]]);
            xmin = xmin.min(sp[0]);
            xmax = xmax.max(sp[1]);
        }
    }
    if hi.is_empty() {
        return None;
    }
    let nn = hi.len();
    let w0 = hi[0][1] - lo[0][1];
    let w1 = hi[nn - 1][1] - lo[nn - 1][1];
    let mut ring = lo.clone();
    for i in (0..nn).rev() {
        if !((i == nn - 1 && w1 < 1e-9) || (i == 0 && w0 < 1e-9)) {
            ring.push(hi[i]);
        }
    }
    let mut eps = 0.0f64;
    for i in 0..nn.saturating_sub(1) {
        let pm = 0.5 * (lo[i][0] + lo[i + 1][0]);
        if let Some(sp) = lat_span(r1, r2, rh, e, ca_sa, pm) {
            eps = eps
                .max((sp[0] - 0.5 * (lo[i][1] + lo[i + 1][1])).abs())
                .max((sp[1] - 0.5 * (hi[i][1] + hi[i + 1][1])).abs());
        }
    }
    let e0 = if p0.abs() >= PI / 2.0 - 1e-9 { w0 } else { 0.0 };
    let e1 = if p1.abs() >= PI / 2.0 - 1e-9 { w1 } else { 0.0 };
    Some(ThickHole {
        ring,
        p0,
        p1,
        xmin,
        xmax,
        edge: e0.max(e1),
        edge_a: if e1 >= e0 { p1 } else { p0 },
        r1,
        r2,
        rh,
        e,
        ca_sa,
        eps,
    })
}

fn ring_at(pts: &[[f64; 2]], th: f64) -> [f64; 2] {
    let n = pts.len();
    let t = mod_p(th / (2.0 * PI), 1.0) * n as f64;
    let i = (t.floor() as usize) % n;
    let f = t - t.floor();
    let (p, q) = (pts[i], pts[(i + 1) % n]);
    [p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f]
}

pub(crate) fn bbox(pts: &[[f64; 2]]) -> (f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (1e9f64, 1e9f64, -1e9f64, -1e9f64);
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    (x1 - x0, y1 - y0)
}

pub(crate) fn shoelace(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    let mut s = 0.0;
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        s += p[0] * q[1] - q[0] * p[1];
    }
    s.abs() / 2.0
}

/// Enclosure of the exact thin-hole outline `(dir + asin((e + rh sin θ)/ri), x0 + rh cos θ)`.
struct ThinHoleCurve {
    dir: f64,
    /// Left edge of the plate strip this hole is drawn in: `u(a) = rm·(a − left)`.
    left: f64,
    x0: f64,
    e: f64,
    rh: f64,
    ri: f64,
    /// Plate circumference radius; the first drawn coordinate is an azimuth in radians and must be scaled
    /// to millimetres before a distance to the drawn chord is measured.
    scale: f64,
}

impl CurveEnclosure for ThinHoleCurve {
    fn enclose(&self, t: Iv) -> Option<(Iv, Iv)> {
        let arg = clamp(
            Iv::pt(self.e)
                .add(Iv::pt(self.rh).mul(t.sin()))
                .div(Iv::pt(self.ri))?,
            -1.0,
            1.0,
        );
        let phi = arg.asin()?;
        let x = Iv::pt((self.dir - self.left) * self.scale).add(phi.mul(Iv::pt(self.scale)));
        let y = Iv::pt(self.x0).add(Iv::pt(self.rh).mul(t.cos()));
        Some((x, y))
    }
}

/// Enclosure of the perpendicular branch cut `(w(θ−π/2)/(2π), h − tee_y(θ))`.
struct PerpCutCurve {
    rc: f64,
    ri: f64,
    ro: f64,
    e: f64,
    w: f64,
    h: f64,
}

impl CurveEnclosure for PerpCutCurve {
    fn enclose(&self, t: Iv) -> Option<(Iv, Iv)> {
        let a = Iv::pt(self.e).add(Iv::pt(self.ri).mul(t.sin()));
        let b = Iv::pt(self.e).add(Iv::pt(self.ro).mul(t.sin()));
        let abs = |v: Iv| {
            if v.lo > 0.0 {
                v
            } else if v.hi < 0.0 {
                v.neg()
            } else {
                Iv::new(0.0, v.lo.abs().max(v.hi.abs()))
            }
        };
        let ma = abs(a);
        let mb = abs(b);
        // The exact wall height uses min(|a|, |b|), so both bounds meet: the lower bound is the smaller
        // lower bound and the upper bound is the smaller upper bound. A previous hull of the two absolute
        // enclosures added the upper bound of `max`, which is not an enclosure of `min`.
        let m = Iv::new(ma.lo.min(mb.lo), ma.hi.min(mb.hi));
        let inside = max0(Iv::pt(self.rc * self.rc).sub(m.mul(m)));
        let tau = sqrt_nonneg(inside)?;
        let x = Iv::pt(self.w)
            .mul(t.sub(Iv::pt(PI / 2.0)))
            .div(Iv::pt(2.0 * PI))?;
        Some((x, Iv::pt(self.h).sub(tau)))
    }
}

/// One branch hole on the main, internal (angles are absolute azimuths in rad).
struct HoleA {
    name: String,
    dir: f64,
    e: f64,
    x_given: bool,
    x0: f64,
    thick: bool,
    rh: f64,
    ro: f64,
    ri_main: f64,
    ro_main: f64,
    pts: Vec<[f64; 2]>,
    marks: Vec<(f64, f64)>,
    xmin: f64,
    xmax: f64,
    lo: f64,
    hi: f64,
    klo: f64,
    khi: f64,
    kc: f64,
    c: f64,
    edge: f64,
    edge_a: f64,
    eps_thick: f64,
    /// (cos, sin) of the sharp branch angle; (0, 1) for the perpendicular branch.
    ca_sa: (f64, f64),
    angle_deg: Option<f64>,
}

impl HoleA {
    /// The hole outline as a function of the angle round the branch (absolute azimuth, axial position).
    fn hp(&self, th: f64) -> [f64; 2] {
        if self.thick {
            ring_at(&self.pts, th)
        } else {
            [self.at(th), self.x0 + self.rh * th.cos()]
        }
    }
    fn at(&self, th: f64) -> f64 {
        self.dir + clamp1((self.e + self.rh * th.sin()) / self.ri_main).asin()
    }
    /// The line where the outer face of the branch meets the main outer face.
    fn wat(&self, th: f64) -> [f64; 2] {
        let q = clamp1((self.e + self.ro * th.sin()) / self.ro_main).asin();
        if self.ca_sa.0 == 0.0 {
            return [self.dir + q, self.x0 + self.ro * th.cos()];
        }
        let cot = self.ca_sa.0 / self.ca_sa.1;
        [
            self.dir + q,
            self.x0 + self.ro_main * q.cos() * cot + self.ro * th.cos() / self.ca_sa.1,
        ]
    }
}

fn hole_a(b: &BranchOnMain, ri: f64, ro_main: f64, mount: Mount, gg: f64, l: f64) -> Option<HoleA> {
    let ro = b.d / 2.0;
    let rh = if mount == Mount::SetIn { ro } else { ro - b.t };
    let e = b.e;
    let dir = b.dir_deg * PI / 180.0;
    let ca_sa = lat_cs(b.angle_deg);
    let thick = ca_sa.0 != 0.0 || e.abs() > rh;
    let hl = if thick {
        Some(lat_hole(ri, ro_main, rh, e, ca_sa)?)
    } else {
        None
    };
    let (dx0, dx1) = match &hl {
        Some(h) => (h.xmin, h.xmax),
        None => (-rh, rh),
    };
    let x0 = b.x.unwrap_or(l / 2.0 - (dx0 + dx1) / 2.0);
    let s0 = ((ri - e.abs()) / rh).max(-1.0);
    let mut marks = Vec::new();
    let (pts, xmin, xmax, edge, edge_a, hlo, hhi);
    if let Some(h) = &hl {
        pts = h
            .ring
            .iter()
            .map(|q| [dir + q[0], x0 + q[1]])
            .collect::<Vec<_>>();
        for k in 0..TEE_MARKS {
            let tq = 2.0 * PI * k as f64 / TEE_MARKS as f64;
            let cq = tq.cos();
            let p = clamp1((e + rh * tq.sin()) / ri).asin().max(h.p0).min(h.p1);
            let sp = h.span(p).unwrap_or([0.0, 0.0]);
            let x = if cq > 1e-9 {
                sp[1]
            } else if cq < -1e-9 {
                sp[0]
            } else {
                (sp[0] + sp[1]) / 2.0
            };
            marks.push((dir + p, x0 + x));
        }
        xmin = x0 + dx0;
        xmax = x0 + dx1;
        edge = h.edge;
        edge_a = dir + if h.edge_a >= 0.0 { PI / 2.0 } else { -PI / 2.0 };
        hlo = h.p0;
        hhi = h.p1;
    } else {
        let at = |th: f64| dir + clamp1((e + rh * th.sin()) / ri).asin();
        pts = (0..TEE_POLY_SAMPLES)
            .map(|k| {
                let th = 2.0 * PI * k as f64 / TEE_POLY_SAMPLES as f64;
                [at(th), x0 + rh * th.cos()]
            })
            .collect::<Vec<_>>();
        for k in 0..TEE_MARKS {
            let tk = 2.0 * PI * k as f64 / TEE_MARKS as f64;
            marks.push((at(tk), x0 + rh * tk.cos()));
        }
        xmin = x0 + dx0;
        xmax = x0 + dx1;
        edge = if s0 < 1.0 {
            2.0 * rh * (1.0 - s0 * s0).sqrt()
        } else {
            0.0
        };
        edge_a = dir + if e >= 0.0 { PI / 2.0 } else { -PI / 2.0 };
        hlo = clamp1((e - rh) / ri).asin();
        hhi = clamp1((e + rh) / ri).asin();
    }
    let lo_k = hlo.min(clamp1((e - ro) / ro_main).asin()) - gg;
    let hi_k = hhi.max(clamp1((e + ro) / ro_main).asin()) + gg;
    Some(HoleA {
        name: b.name.clone(),
        dir,
        e,
        x_given: b.x.is_some(),
        x0,
        thick,
        rh,
        ro,
        ri_main: ri,
        ro_main,
        pts,
        marks,
        xmin,
        xmax,
        lo: dir + hlo,
        hi: dir + hhi,
        klo: dir + lo_k,
        khi: dir + hi_k,
        kc: dir + (lo_k + hi_k) / 2.0,
        c: dir + clamp1(e / ri).asin(),
        edge,
        edge_a,
        eps_thick: hl.as_ref().map_or(0.0, |h| h.eps),
        ca_sa,
        angle_deg: b.angle_deg,
    })
}

pub(crate) fn in_poly(q: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        if (poly[i][1] > q[1]) != (poly[j][1] > q[1])
            && q[0]
                < (poly[j][0] - poly[i][0]) * (q[1] - poly[i][1]) / (poly[j][1] - poly[i][1])
                    + poly[i][0]
        {
            c = !c;
        }
        j = i;
    }
    c
}

fn wrap_overlap(pa: &[[f64; 2]], pb: &[[f64; 2]], wp: f64) -> bool {
    let in_w = |q: [f64; 2], poly: &[[f64; 2]]| {
        [-wp, 0.0, wp]
            .iter()
            .any(|s| in_poly([q[0] + s, q[1]], poly))
    };
    pa.iter().any(|q| in_w(*q, pb)) || pb.iter().any(|q| in_w(*q, pa))
}

/// Nearest distance between two closed outlines on a plate that wraps with period `wp`: coarse grid, then four
/// local refinements. An ESTIMATE (the global minimum is not proved).
fn wrap_gap(fa: &dyn Fn(f64) -> [f64; 2], fb: &dyn Fn(f64) -> [f64; 2], wp: f64) -> f64 {
    let n = 180usize;
    let mut h = 2.0 * PI / n as f64;
    let a: Vec<[f64; 2]> = (0..n).map(|i| fa(h * i as f64)).collect();
    let b: Vec<[f64; 2]> = (0..n).map(|i| fb(h * i as f64)).collect();
    let dist = |p: [f64; 2], q: [f64; 2]| {
        let mut du = mod_p(p[0] - q[0], wp);
        du = du.min(wp - du);
        du.hypot(p[1] - q[1])
    };
    let (mut best, mut ta, mut tb) = (f64::INFINITY, 0.0, 0.0);
    for (i, pa) in a.iter().enumerate() {
        for (j, pb) in b.iter().enumerate() {
            let d = dist(*pa, *pb);
            if d < best {
                best = d;
                ta = h * i as f64;
                tb = h * j as f64;
            }
        }
    }
    for _ in 0..4 {
        let (mut na, mut nb) = (ta, tb);
        for i in -10i32..=10 {
            for j in -10i32..=10 {
                let aa = ta + h * f64::from(i) / 10.0;
                let cc = tb + h * f64::from(j) / 10.0;
                let d2 = dist(fa(aa), fb(cc));
                if d2 < best {
                    best = d2;
                    na = aa;
                    nb = cc;
                }
            }
        }
        ta = na;
        tb = nb;
        h /= 10.0;
    }
    best
}

fn polyline_len(p: &[[f64; 2]], closed: bool) -> f64 {
    let mut s: f64 = p
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum();
    if closed && p.len() > 1 {
        let (a, b) = (p[p.len() - 1], p[0]);
        s += (b[0] - a[0]).hypot(b[1] - a[1]);
    }
    s
}

/// Estimate of the largest distance between the drawn polygon of a closed curve and the curve: mid-point sagitta of
/// every edge, from the curve function. An estimate, never a bound.
fn sagitta_estimate(f: &dyn Fn(f64) -> [f64; 2], n: usize) -> f64 {
    let h = 2.0 * PI / n as f64;
    let mut worst = 0.0f64;
    for i in 0..n {
        let (p, q) = (f(h * i as f64), f(h * (i as f64 + 1.0)));
        let m = f(h * (i as f64 + 0.5));
        let c = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
        worst = worst.max((m[0] - c[0]).hypot(m[1] - c[1]));
    }
    worst
}

// ---------------------------------------------------------------- results

/// One branch hole on the developed main (plate coordinates: `u` round, from the left seam of the plate that holds it).
#[derive(Debug, Clone, PartialEq)]
pub struct HoleOut {
    pub name: String,
    /// Plate index (0-based) holding the hole.
    pub plate: usize,
    /// Closed polygon in plate coordinates (`u`, axial `x`), without repeating the first vertex.
    pub pts: Vec<[f64; 2]>,
    pub hole_w: f64,
    pub hole_h: f64,
    /// Branch centre line round position (from the left seam of its plate) and axial position of the centre.
    pub hole_u: f64,
    pub hole_x: f64,
    pub xmin: f64,
    pub xmax: f64,
    /// Azimuth span (deg, from the top) the hole covers, start and end.
    pub from_deg: f64,
    pub to_deg: f64,
    pub dir_deg: f64,
    pub e: f64,
    /// Sharp branch angle (deg); 90 for the perpendicular branch.
    pub angle_deg: f64,
    /// True when the hole is cut through the whole wall (offset beyond the bore).
    pub thick: bool,
    /// Length of the hole edge that runs along the main's lateral line (invert) when the bore leaves the inner face.
    pub edge: f64,
    pub edge_deg: f64,
    /// Seam-keep zone (`u` of both ends) and whether a seam passes through it.
    pub keep: [f64; 2],
    pub cross_seam: bool,
    /// Distance seam to hole edge along the mid surface (negative when a seam crosses the hole), for the rule seam and
    /// for the mirrored one.
    pub seam_dist: f64,
    pub seam_dist_alt: f64,
    /// Marking points (u, x) of the 16 divisions round the branch.
    pub marks: Vec<[f64; 2]>,
    /// Area of the drawn polygon, and an ESTIMATE of the polygon-to-curve distance.
    pub area: f64,
    pub eps_rep: f64,
    pub eps_rep_certified: bool,
    pub x_given: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoleGap {
    pub a: usize,
    pub b: usize,
    /// Holes overlap (cut). Then `gap` is 0.
    pub cut: bool,
    /// Nearest distance between the two hole edges (mid surface) and between the two outer welds (main outer surface).
    pub gap: f64,
    pub weld: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Seam {
    pub deg: f64,
    pub arc: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plate {
    pub w: f64,
    pub h: f64,
    pub holes: Vec<usize>,
    pub gross_area: f64,
    pub net_area: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeeHold {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeeMainResult {
    /// Circumference at the mid surface and the width of one plate.
    pub circ: f64,
    pub plate_w: f64,
    pub seams_n: u32,
    pub l: f64,
    pub holes: Vec<HoleOut>,
    pub gaps: Vec<HoleGap>,
    pub plates: Vec<Plate>,
    pub seams: Vec<Seam>,
    /// Seam rotation (deg) applied to keep one hole off the seam (single branch only).
    pub moved_deg: f64,
    pub gap_mm: f64,
    pub cross_seam: bool,
    pub mirror: bool,
    pub net_area: f64,
    pub mass_kg: f64,
    /// Holds the caller must turn into a withheld DXF (never silently dropped).
    pub holds: Vec<TeeHold>,
    /// Questions for the engineer (branch position not on the drawing).
    pub questions: Vec<&'static str>,
    pub certified: bool,
    pub method: &'static str,
    pub rules: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

impl TeeMainResult {
    pub fn has_hold(&self, code: &str) -> bool {
        self.holds.iter().any(|h| h.code == code)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BranchMark {
    pub i: usize,
    pub u: f64,
    pub len: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BranchPieceResult {
    /// Circumference at the mid surface.
    pub circ: f64,
    pub marks: Vec<BranchMark>,
    /// Drawn cut line (u, length) from u = 0 to the circumference, `TEE_POLY_SAMPLES + 1` points.
    pub cut: Vec<[f64; 2]>,
    /// Closed outline polygon (bottom, then the cut line from right to left).
    pub outline: Vec<[f64; 2]>,
    /// Shortest and longest length of the exact cut curve (golden-section refined), and the sampled ones.
    pub shortest: f64,
    pub longest: f64,
    pub shortest_sampled: f64,
    pub longest_sampled: f64,
    pub extent_refine_gain: f64,
    /// Length of the drawn cut line.
    pub cut_length: f64,
    /// Certified bound (or estimate) of the distance from the exact cut curve to the drawn polyline.
    pub eps_rep: f64,
    pub eps_rep_certified: bool,
    /// Net area (adaptive Simpson), its estimate, and the area of the drawn polygon.
    pub net_area: f64,
    pub area_err: f64,
    pub area_drawn: f64,
    pub mass_kg: f64,
    /// Sharp branch angle (deg); 90 for the perpendicular branch.
    pub angle_deg: f64,
    /// Codes the caller must turn into a hold (never dropped): `tee_lateral_angle_below_30`.
    pub flags: Vec<&'static str>,
    pub certified: bool,
    pub method: &'static str,
    pub rules: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

// ---------------------------------------------------------------- branch piece

fn contact_radius(mount: Mount, ro_main: f64, ri_main: f64) -> f64 {
    if mount == Mount::SetIn {
        ri_main
    } else {
        ro_main
    }
}

/// Largest `cut_tau` round the branch (dense scan, then golden-section refinement of the best sample).
fn max_tau(rc: f64, ri: f64, ro: f64, e: f64, alpha: Option<f64>) -> f64 {
    let n = 7200usize;
    let f = |th: f64| cut_tau(rc, ri, ro, e, th, alpha);
    let hstep = 2.0 * PI / n as f64;
    let (mut best, mut ib) = (f64::NEG_INFINITY, 0usize);
    for i in 0..n {
        let v = f(hstep * i as f64);
        if v > best {
            best = v;
            ib = i;
        }
    }
    let c = hstep * ib as f64;
    let (mut a, mut b) = (c - hstep, c + hstep);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut x1, mut x2) = (b - g * (b - a), a + g * (b - a));
    let (mut f1, mut f2) = (f(x1), f(x2));
    for _ in 0..80 {
        if f1 > f2 {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = f(x1);
        } else {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = f(x2);
        }
    }
    best.max(f(0.5 * (a + b)))
}

#[allow(clippy::too_many_arguments)]
fn check_branch_fits(
    d: f64,
    t: f64,
    e: f64,
    main_d: f64,
    main_t: f64,
    h: Option<f64>,
    mount: Mount,
    angle: Option<f64>,
) -> Result<(), ShapeRefusal> {
    let (ro_main, ri_main) = (main_d / 2.0, main_d / 2.0 - main_t);
    let (ro, ri) = (d / 2.0, d / 2.0 - t);
    if mount == Mount::SetIn && ro + e.abs() > ri_main {
        return Err(refusal(
            "tee_branch_too_large_to_insert",
            vec!["d", "e", "main_d", "main_t"],
            format!(
                "branch outer radius {ro} plus offset {} exceeds the main inner radius {ri_main}: it cannot be inserted (a branch of the same diameter is set on)",
                e.abs()
            ),
        ));
    }
    // same +1e-6 as validate_main and ZK build-core (a hole at the wall is on the main)
    if ri + e.abs() > ro_main + 1e-6 {
        return Err(refusal(
            "tee_branch_bore_beyond_main",
            vec!["d", "t", "e", "main_d"],
            format!(
                "branch inner radius {ri} plus offset {} exceeds the main outer radius {ro_main}",
                e.abs()
            ),
        ));
    }
    if let Some(h) = h {
        let rc = contact_radius(mount, ro_main, ri_main);
        let y_top = if is_lateral(angle) {
            max_tau(rc, ri, ro, e, angle)
        } else {
            let e = e.abs();
            if e <= ro {
                rc
            } else {
                (rc * rc - (e - ro) * (e - ro)).max(0.0).sqrt()
            }
        };
        if h <= y_top {
            return Err(refusal(
                "tee_branch_end_inside_main",
                vec!["h"],
                format!(
                    "branch end {h} mm from the main centre line is not beyond where the branch meets the main ({y_top} mm)"
                ),
            ));
        }
    }
    Ok(())
}

/// Branch piece: cut round the branch where its wall meets the main (set-on: main outer surface, set-in: main inner
/// surface), 16 marks (0 = side of positive offset, 4 = along the main axis, 8 = side of negative offset, 12 = along
/// the other end of the main), the cut line, extremes, area and mass.
pub fn branch_piece(s: &BranchPieceSpec) -> Result<BranchPieceResult, ShapeRefusal> {
    dim("d", s.d)?;
    dim("t", s.t)?;
    dim("main_d", s.main_d)?;
    dim("main_t", s.main_t)?;
    dim("h", s.h)?;
    signed("e", s.e)?;
    check_angle(s.angle_deg)?;
    wall("t", "d", s.t, s.d)?;
    wall("main_t", "main_d", s.main_t, s.main_d)?;
    check_branch_fits(
        s.d,
        s.t,
        s.e,
        s.main_d,
        s.main_t,
        Some(s.h),
        s.mount,
        s.angle_deg,
    )?;
    let (ro_main, ri_main) = (s.main_d / 2.0, s.main_d / 2.0 - s.main_t);
    let rc = contact_radius(s.mount, ro_main, ri_main);
    let (ro, ri) = (s.d / 2.0, s.d / 2.0 - s.t);
    let rm = ro - s.t / 2.0;
    let w = 2.0 * PI * rm;
    let th0 = PI / 2.0;
    let len_at = |th: f64| s.h - cut_tau(rc, ri, ro, s.e, th, s.angle_deg);
    let poly_n = if is_lateral(s.angle_deg) {
        TEE_LATERAL_POLY_SAMPLES
    } else {
        TEE_POLY_SAMPLES
    };
    let len_k = |k: usize, n: usize| len_at(th0 + 2.0 * PI * k as f64 / n as f64);
    let mut cut = Vec::with_capacity(poly_n + 1);
    let (mut lo_s, mut hi_s) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..=poly_n {
        let l = len_k(i, poly_n);
        cut.push([w * i as f64 / poly_n as f64, l]);
        lo_s = lo_s.min(l);
        hi_s = hi_s.max(l);
    }
    let marks: Vec<BranchMark> = (0..=TEE_MARKS)
        .map(|k| BranchMark {
            i: k,
            u: w * k as f64 / TEE_MARKS as f64,
            len: len_k(k, TEE_MARKS),
        })
        .collect();
    // exact extremes: dense scan then golden-section refinement of the best sample, both ends
    let dense = 7200usize;
    let vals: Vec<f64> = (0..dense).map(|i| len_k(i, dense)).collect();
    let (mut imin, mut imax) = (0usize, 0usize);
    for i in 0..dense {
        if vals[i] < vals[imin] {
            imin = i;
        }
        if vals[i] > vals[imax] {
            imax = i;
        }
    }
    let refine = |i: usize, want_min: bool| -> f64 {
        let h = 2.0 * PI / dense as f64;
        let c = th0 + h * i as f64;
        let (mut a, mut b) = (c - h, c + h);
        let g = (5f64.sqrt() - 1.0) / 2.0;
        let f = |x: f64| if want_min { len_at(x) } else { -len_at(x) };
        let (mut x1, mut x2) = (b - g * (b - a), a + g * (b - a));
        let (mut f1, mut f2) = (f(x1), f(x2));
        for _ in 0..80 {
            if f1 < f2 {
                b = x2;
                x2 = x1;
                f2 = f1;
                x1 = b - g * (b - a);
                f1 = f(x1);
            } else {
                a = x1;
                x1 = x2;
                f1 = f2;
                x2 = a + g * (b - a);
                f2 = f(x2);
            }
        }
        let v = len_at((a + b) / 2.0);
        if want_min {
            v.min(vals[i])
        } else {
            v.max(vals[i])
        }
    };
    let shortest = refine(imin, true).min(lo_s);
    let longest = refine(imax, false).max(hi_s);
    let gain = (lo_s - shortest).abs().max((hi_s - longest).abs());
    // area: integral of the length over the developed width (u = rm * theta), adaptive Simpson with its estimate
    let (int, err) = adaptive_simpson(&len_at, th0, th0 + 2.0 * PI);
    let net_area = rm * int;
    let area_err = rm * err;
    if !(net_area.is_finite() && net_area > 0.0) {
        return Err(refuse(
            "tee_result_not_finite",
            "d",
            "branch area is not a finite positive number",
        ));
    }
    if area_err > AREA_REL_TOL * net_area {
        return Err(refuse(
            "tee_area_not_converged",
            "d",
            format!(
                "branch area estimate {area_err:e} exceeds the acceptance {:e} of the value",
                AREA_REL_TOL
            ),
        ));
    }
    let mut outline = vec![[0.0, 0.0], [w, 0.0]];
    outline.extend(cut.iter().rev().copied());
    let area_drawn = shoelace(&outline);
    let mass = mass_kg(net_area, s.t);
    let cut_certified = if is_lateral(s.angle_deg) {
        None
    } else {
        let pts: Vec<P2> = cut.iter().map(|q| P2::new(q[0], q[1])).collect();
        let params: Vec<f64> = (0..=poly_n)
            .map(|k| th0 + 2.0 * PI * k as f64 / poly_n as f64)
            .collect();
        let curve = PerpCutCurve {
            rc,
            ri,
            ro,
            e: s.e,
            w,
            h: s.h,
        };
        certified_rep(&curve, &pts, &params, L11_MAX_CHORD_ERROR_MM)
    };
    let cut_eps = cut_certified.unwrap_or_else(|| {
        sagitta_estimate(&|th| [w * (th - th0) / (2.0 * PI), len_at(th)], poly_n)
    });
    let res = BranchPieceResult {
        circ: w,
        marks,
        cut_length: polyline_len(&cut, false),
        cut,
        outline,
        shortest,
        longest,
        shortest_sampled: lo_s,
        longest_sampled: hi_s,
        extent_refine_gain: gain,
        eps_rep: cut_eps,
        eps_rep_certified: cut_certified.is_some(),
        net_area,
        area_err,
        area_drawn,
        mass_kg: mass,
        angle_deg: s.angle_deg.unwrap_or(90.0),
        flags: if s.angle_deg.is_some_and(|a| a < LATERAL_MIN_ANGLE_DEG) {
            vec!["tee_lateral_angle_below_30"]
        } else {
            vec![]
        },
        certified: cut_certified.is_some(),
        method: TEE_METHOD,
        rules: &TEE_NAMED_RULES,
    };
    if ![res.shortest, res.longest, res.cut_length, res.mass_kg]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(refuse(
            "tee_result_not_finite",
            "d",
            "a branch piece quantity is not finite",
        ));
    }
    Ok(res)
}

// ---------------------------------------------------------------- tee main

fn validate_main(s: &TeeMainSpec) -> Result<(), ShapeRefusal> {
    dim("d", s.d)?;
    dim("t", s.t)?;
    dim("l", s.l)?;
    wall("t", "d", s.t, s.d)?;
    if s.branches.is_empty() {
        return Err(refuse(
            "tee_no_branch",
            "branches",
            "a tee main needs at least one branch",
        ));
    }
    if s.branches.len() > MAX_TEE_BRANCHES {
        return Err(refuse(
            "tee_branches_above_limit",
            "branches",
            format!("at most {MAX_TEE_BRANCHES} branches per main"),
        ));
    }
    if !(1..=MAX_TEE_SEAMS).contains(&s.seams) {
        return Err(refuse(
            "tee_seams_out_of_range",
            "seams",
            format!("seams must be 1..={MAX_TEE_SEAMS} (got {})", s.seams),
        ));
    }
    if !s.seam_deg.is_finite() || !(0.0..=180.0).contains(&s.seam_deg) {
        return Err(refuse(
            "tee_seam_angle_invalid",
            "seam_deg",
            format!("seam angle must be 0..=180 deg (got {})", s.seam_deg),
        ));
    }
    if let Some(g) = s.gap {
        signed("gap", g)?;
        if g < 0.0 {
            return Err(refuse(
                "tee_value_not_positive",
                "gap",
                "gap must not be negative",
            ));
        }
    }
    for b in &s.branches {
        dim("branch.d", b.d)?;
        dim("branch.t", b.t)?;
        signed("branch.e", b.e)?;
        signed("branch.dir_deg", b.dir_deg)?;
        if let Some(x) = b.x {
            signed("branch.x", x)?;
        }
        check_angle(b.angle_deg)?;
        wall("branch.t", "branch.d", b.t, b.d)?;
        // the hole must keep inside the main (ZK `teeFeasible`); checked before the insert limit so that a hole that
        // cannot be on the main at all is reported as such
        let rh = if s.mount == Mount::SetIn {
            b.d / 2.0
        } else {
            b.d / 2.0 - b.t
        };
        if b.e.abs() + rh > s.d / 2.0 + 1e-6 {
            return Err(refuse(
                "tee_hole_off_pipe",
                "branch.e",
                format!(
                    "hole of branch {} does not fit on the main (|e| + bore radius exceeds the main outer radius)",
                    b.name
                ),
            ));
        }
        check_branch_fits(b.d, b.t, b.e, s.d, s.t, None, s.mount, b.angle_deg)?;
    }
    Ok(())
}

/// Tee main: the plate(s) the main is rolled from, with one hole per branch, the seam rule, hole gaps and holds.
pub fn tee_main(s: &TeeMainSpec) -> Result<TeeMainResult, ShapeRefusal> {
    validate_main(s)?;
    let ro = s.d / 2.0;
    let ri = ro - s.t;
    let rm = ro - s.t / 2.0;
    let w = 2.0 * PI * rm;
    let sa = s.seam_deg;
    let mut ps = (if s.seam_from == TeeSeamFrom::Top {
        sa
    } else {
        180.0 - sa
    }) * PI
        / 180.0;
    if s.seam_mirror {
        ps = -ps;
    }
    let n = s.seams as usize;
    let gap = s.gap.unwrap_or(TEE_SEAM_GAP_T * s.t);
    let step = 2.0 * PI / n as f64;
    let gg = gap / rm;
    let mut hs: Vec<HoleA> = Vec::new();
    for b in &s.branches {
        match hole_a(b, ri, ro, s.mount, gg, s.l) {
            Some(h) => hs.push(h),
            None => {
                return Err(refuse(
                    "tee_hole_off_pipe",
                    "branch.e",
                    format!(
                        "the bore of branch {} does not pass through the wall of the main",
                        b.name
                    ),
                ));
            }
        }
    }
    for h in &hs {
        if h.xmin < 0.0 || h.xmax > s.l {
            return Err(refusal(
                "tee_hole_beyond_pipe_end",
                vec!["branch.x", "l"],
                format!(
                    "hole of branch {} spans x = {:.3} .. {:.3} and does not lie inside the main length {}",
                    h.name, h.xmin, h.xmax, s.l
                ),
            ));
        }
    }
    let rule_of = |p: f64| -> Vec<f64> { (0..n).map(|k| -p + step * k as f64).collect() };
    let rule = rule_of(ps);
    let hit_by = |h: &HoleA, ss: &[f64]| {
        ss.iter().any(|a| {
            let wv = wrap_a(*a, h.kc);
            h.klo < wv && wv < h.khi
        })
    };
    let h0 = &hs[0];
    let hit = hs.iter().any(|h| hit_by(h, &rule));
    let mut seams = rule.clone();
    let mut moved = 0.0;
    let cross;
    if hs.len() == 1 {
        cross = hit && h0.khi - h0.klo > step;
        if hit && !cross {
            seams = (0..n)
                .map(|k| h0.kc - PI / n as f64 + step * k as f64)
                .collect();
            moved = mod_p(seams[0] - rule[0] + step / 2.0, step) - step / 2.0;
        }
    } else {
        cross = hit;
    }
    let ah = h0.kc - mod_p(h0.kc - seams[0], step);
    let pw = w / n as f64;
    let seam_dist = |h: &HoleA, ss: &[f64]| -> f64 {
        ss.iter()
            .map(|sm| {
                let wv = wrap_a(*sm, (h.lo + h.hi) / 2.0);
                if h.lo < wv && wv < h.hi {
                    -rm * (wv - h.lo).min(h.hi - wv)
                } else {
                    let d = if wv < h.lo { h.lo - wv } else { wv - h.hi };
                    rm * d.min(2.0 * PI - (h.hi - h.lo) - d)
                }
            })
            .fold(f64::INFINITY, f64::min)
    };
    let alt = rule_of(-ps);
    let deg = |a: f64| {
        let x = mod_p(a * 180.0 / PI, 360.0);
        if x > 180.0 + 1e-9 { x - 360.0 } else { x }
    };
    let mut holes = Vec::new();
    for h in &hs {
        let off = mod_p(h.kc - ah, 2.0 * PI);
        let jj = (n - 1).min((off / step).floor() as usize);
        let left = ah + step * jj as f64;
        let reff = ah + off;
        let u = |a: f64| rm * (wrap_a(a, reff) - left);
        let pts: Vec<[f64; 2]> = h.pts.iter().map(|q| [u(q[0]), q[1]]).collect();
        let (hw, hh) = bbox(&pts);
        let area = shoelace(&pts);
        let fh = |th: f64| {
            let q = h.hp(th);
            [u(q[0]), q[1]]
        };
        let certified = if h.thick {
            None
        } else {
            // Use the same points that are written to the pattern, including the strip offset `left`.
            // A previous version enclosed `rm·azimuth` but compared it with points shifted by `rm·left`;
            // that failed closed away from a zero seam and was not a proof of the drawn outline.
            let mut drawn: Vec<P2> = pts.iter().map(|q| P2::new(q[0], q[1])).collect();
            if let Some(first) = drawn.first().copied() {
                drawn.push(first);
            }
            let params: Vec<f64> = (0..=TEE_POLY_SAMPLES)
                .map(|k| 2.0 * PI * k as f64 / TEE_POLY_SAMPLES as f64)
                .collect();
            let curve = ThinHoleCurve {
                dir: h.dir,
                left,
                x0: h.x0,
                e: h.e,
                rh: h.rh,
                ri: h.ri_main,
                scale: rm,
            };
            certified_rep(&curve, &drawn, &params, L11_MAX_CHORD_ERROR_MM)
        };
        let eps = match certified {
            Some(v) => v,
            None => {
                let _ = &fh;
                if h.thick {
                    h.eps_thick
                } else {
                    sagitta_estimate(&fh, TEE_POLY_SAMPLES)
                }
            }
        };
        let from = deg(h.lo);
        holes.push(HoleOut {
            name: h.name.clone(),
            plate: jj,
            marks: h.marks.iter().map(|m| [u(m.0), m.1]).collect(),
            hole_w: hw,
            hole_h: hh,
            hole_u: u(h.c),
            hole_x: h.x0,
            xmin: h.xmin,
            xmax: h.xmax,
            from_deg: from,
            to_deg: from + (h.hi - h.lo) * 180.0 / PI,
            dir_deg: h.dir * 180.0 / PI,
            e: h.e,
            angle_deg: h.angle_deg.unwrap_or(90.0),
            thick: h.thick,
            edge: h.edge,
            edge_deg: mod_p(h.edge_a * 180.0 / PI, 360.0),
            keep: [u(h.klo), u(h.khi)],
            cross_seam: hit_by(h, &seams),
            seam_dist: seam_dist(h, &seams),
            seam_dist_alt: seam_dist(h, &alt),
            area,
            eps_rep: eps,
            eps_rep_certified: certified.is_some(),
            x_given: h.x_given,
            pts,
        });
    }
    let mut gaps = Vec::new();
    for a1 in 0..hs.len() {
        for a2 in a1 + 1..hs.len() {
            let (ha, hb) = (&hs[a1], &hs[a2]);
            let pa: Vec<[f64; 2]> = ha.pts.iter().map(|q| [rm * q[0], q[1]]).collect();
            let pb: Vec<[f64; 2]> = hb.pts.iter().map(|q| [rm * q[0], q[1]]).collect();
            let wm = 2.0 * PI * rm;
            let cut = wrap_overlap(&pa, &pb, wm);
            let (ha_h, hb_h) = (
                |th: f64| {
                    let q = ha.hp(th);
                    [rm * q[0], q[1]]
                },
                |th: f64| {
                    let q = hb.hp(th);
                    [rm * q[0], q[1]]
                },
            );
            let (ha_w, hb_w) = (
                |th: f64| {
                    let q = ha.wat(th);
                    [ro * q[0], q[1]]
                },
                |th: f64| {
                    let q = hb.wat(th);
                    [ro * q[0], q[1]]
                },
            );
            gaps.push(HoleGap {
                a: a1,
                b: a2,
                cut,
                gap: if cut { 0.0 } else { wrap_gap(&ha_h, &hb_h, wm) },
                weld: wrap_gap(&ha_w, &hb_w, 2.0 * PI * ro),
            });
        }
    }
    if let Some(g) = gaps.iter().find(|g| g.cut) {
        return Err(refuse(
            "tee_holes_overlap",
            "branches",
            format!(
                "the holes of branches {} and {} overlap (check position, direction and diameter)",
                holes[g.a].name, holes[g.b].name
            ),
        ));
    }
    let seams_out: Vec<Seam> = (0..n)
        .map(|j| {
            let a = wrap_a(ah + step * j as f64, 0.0);
            Seam {
                deg: a * 180.0 / PI,
                arc: rm * a,
            }
        })
        .collect();
    let mut plates: Vec<Plate> = (0..n)
        .map(|_| Plate {
            w: pw,
            h: s.l,
            holes: vec![],
            gross_area: pw * s.l,
            net_area: pw * s.l,
        })
        .collect();
    for (i, h) in holes.iter().enumerate() {
        plates[h.plate].holes.push(i);
        plates[h.plate].net_area -= h.area;
    }
    let net_area: f64 = plates.iter().map(|p| p.net_area).sum();
    let mut holds = Vec::new();
    if cross {
        holds.push(TeeHold {
            code: "tee_seam_crosses_hole",
            message: "no seam position keeps every hole (with the clear distance) off the longitudinal seams; the person must choose a seam count or side".into(),
        });
    }
    for b in &s.branches {
        if b.angle_deg.is_some_and(|a| a < LATERAL_MIN_ANGLE_DEG) {
            holds.push(TeeHold {
                code: "tee_lateral_angle_below_30",
                message: format!(
                    "branch {} stands at less than {LATERAL_MIN_ANGLE_DEG} deg to the main: built, but the person must confirm the angle before a cut file is served",
                    b.name
                ),
            });
        }
    }
    let questions: Vec<&'static str> = if holes.iter().any(|h| !h.x_given) {
        vec![TEE_X_QUESTION]
    } else {
        vec![]
    };
    let res = TeeMainResult {
        circ: w,
        plate_w: pw,
        seams_n: s.seams,
        l: s.l,
        gaps,
        plates,
        seams: seams_out,
        moved_deg: moved * 180.0 / PI,
        gap_mm: gap,
        cross_seam: cross,
        mirror: s.seam_mirror,
        net_area,
        mass_kg: mass_kg(net_area, s.t),
        holds,
        questions,
        certified: !holes.is_empty() && holes.iter().all(|h| h.eps_rep_certified),
        method: TEE_METHOD,
        rules: &TEE_NAMED_RULES,
        holes,
    };
    let finite = res.mass_kg.is_finite()
        && res.holes.iter().all(|h| {
            h.hole_u.is_finite()
                && h.hole_w.is_finite()
                && h.hole_h.is_finite()
                && h.seam_dist.is_finite()
                && h.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite())
        })
        && res
            .gaps
            .iter()
            .all(|g| g.gap.is_finite() && g.weld.is_finite());
    if !finite {
        return Err(refuse(
            "tee_result_not_finite",
            "branches",
            "a tee main quantity is not finite",
        ));
    }
    Ok(res)
}

/// A rib stands on the branch only when the branch is perpendicular to the main. A rib on a lateral branch is not
/// built (ZK says so too): it is refused with a code, never dropped. Call it with the branch angle of the rib's branch.
pub fn rib_on_branch(angle_deg: Option<f64>) -> Result<(), ShapeRefusal> {
    check_angle(angle_deg)?;
    if is_lateral(angle_deg) {
        return Err(refuse(
            "tee_rib_on_lateral_unsupported",
            "angle_deg",
            format!(
                "a rib on a lateral branch ({} deg) is not built; ribs stand on perpendicular branches only",
                angle_deg.unwrap_or(90.0)
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod branch_fit_tests {
    use super::*;

    // 610 x 9 branch on a 1016 x 12 main: bore radius 296, main outer radius 508, so e = 212 puts the bore on the wall
    #[test]
    fn branch_bore_on_the_wall_is_accepted_within_1e_6_and_refused_beyond() {
        let fit = |e: f64| check_branch_fits(610.0, 9.0, e, 1016.0, 12.0, None, Mount::SetOn, None);
        assert!(
            fit(212.0 + 0.5e-6).is_ok(),
            "just inside the tolerance is accepted"
        );
        assert!(fit(212.0).is_ok());
        let err = fit(212.0 + 2e-6).expect_err("just outside the tolerance is refused");
        assert_eq!(err.code, "tee_branch_bore_beyond_main");
    }
}
