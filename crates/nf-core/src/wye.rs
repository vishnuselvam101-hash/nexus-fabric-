//! NFJ-1 S1-L9b: a wye (Y-branch): one main pipe ends at the intersection of three centre lines and splits into two legs.
//! Every piece (main, leg 1, leg 2, and for a narrower branch the two cones and the two branch pipes) with its cut curves,
//! its development, its marks, its area and its weld lengths. Pure functions, no I/O, no engine state.
//!
//! Status (stated, not hidden): NOT reachable from `job::run`; nothing here can be served as a cut file; `ENGINE_VERSION`
//! is unchanged. Every result is `certified: false` with its `method`, named `rules` (all UNVERIFIED, none confirmed by a
//! factory) and the `holds` the caller must turn into a withheld DXF.
//!
//! Geometry, derived from first principles (tests/wye_closed_forms.rs is independent of this file; ZK `wyeModel` and
//! `spec_yfork.md` are a comparison, not the source):
//!
//! * Frame: the intersection is the origin, the legs run along unit vectors `u0 = (-1,0,0)` (main), `u1 = (cos b1, sin b1, 0)`,
//!   `u2 = (cos b2, -sin b2, 0)`; the plane of the Y is `xy`. A generator of leg `i` is `P(s, th) = s u_i + rho_i(s) (cos th
//!   e1_i + sin th ez)`, `th = 0` towards the first neighbour of the leg on the plane of the Y, `th = 90 deg` towards `+z`.
//! * Common sphere: the outer surface of every leg touches the sphere of radius `Rs = D/2` about the origin (a cylinder
//!   is the half angle `psi = 0`, a cone has `rho(s) = (Rs - s sin psi) / cos psi`: its generator is at distance `Rs` from
//!   the origin). With `g_i(P) = (P.u_i - Rs sin psi_i) / cos psi_i` the surface of leg `i` satisfies `|P|^2 - Rs^2 =
//!   g_i(P)^2`, so two legs meet only in the planes `g_i = g_j` and `g_i = -g_j`; the retained one is `g_i = g_j`.
//!   Leg `i` is kept where `g_i >= g_j` for both neighbours; `g_i - g_j` is AFFINE in `s` on a generator, so the cut is
//!   one linear solve, no root finding. The slope `A` and the solution `s_ij = B / A` are affine / Mobius in `x = cos th`
//!   only (every axis lies in the plane `z = 0`), hence monotone on each half circle: `A > 0` for all `th` is exactly
//!   `A(-1) > 0 and A(1) > 0`, and the shortest / longest generator are at `x = +-1` or at the crossing of the two
//!   neighbours' cuts, which is a quadratic in `x` solved in closed form (no sampled maximum).
//! * Crotch point: `g_0 = g_1 = g_2` (two linear equations in `x, y`) on the main surface `y^2 + z^2 = Rs^2`. For cones it
//!   lies ahead of the intersection (`x > 0`): the end of the main pipe protrudes (ZK example 4: 64.96 mm).
//! * Thickness: the wall `t` is measured along the surface normal. The surface offset by `o` along the inward normal is
//!   the cone of the same half angle with `rho_o(s) = rho(s) - o / cos psi`, tangent to the sphere of radius `Rs - o`. ZK's
//!   `(Rs - s sin psi - o) / cos psi` IS that cone, so ZK's wye already follows the normal rule (rule `WYE_THICKNESS_NORMAL`);
//!   a radial reading `rho - o` has no ZK parity and is only a named alternative (not implemented).
//! * The developed (neutral) surface of a cone leg is a fan about the apex `s_a = (Rs - o) / sin psi`; slant distance
//!   `l(s) = |s_a - s| / cos psi`, fan angle `theta * |sin psi|`. A cone whose small mouth lies beyond the apex has no
//!   positive radius there and is refused (`wye_cone_mouth_inside_apex`); a branch wider than the main (`psi < 0`, apex behind
//!   the intersection) is built (fan with the same orientation rule) but carries the hold `wye_branch_wider_than_main`.
//!
//! Numerics: cut, extremes, marks and the crotch are closed form. Areas are adaptive Simpson with a-posteriori error,
//! split at the kinks (result refused above `WYE_AREA_REL_TOL`). Outlines are sampled polylines (720 points per turn plus
//! every kink); `eps_rep` is an ESTIMATE of the chord error, not a bound.

use crate::shapes::{Basis, ShapeRefusal, adaptive_simpson, mass_kg, refuse};
use crate::tee::{mod_p, shoelace};
use std::f64::consts::PI;

pub const WYE_METHOD: &str = "common_sphere_planar_joints+affine_cut_closed_form+analytic_extremes+adaptive_simpson_areas+sampled_outlines; certified:false";
/// Marking points round each piece (ZK parity): `theta = k pi / 8`.
pub const WYE_MARKS: usize = 16;
/// Outline samples per turn (ZK parity); every kink is added as an extra sample.
pub const WYE_OUTLINE_SAMPLES: usize = 720;
/// Samples per turn of the dense output invariants and of the weld length.
pub const WYE_DENSE_SAMPLES: usize = 2880;
/// `A` (slope of `g_i - g_j` along a generator) must exceed this, else the generator does not leave the neighbour.
pub const WYE_A_MIN: f64 = 1e-9;
/// IMPLEMENTATION LIMIT (UNVERIFIED as a fabrication value): branch diameter within this of `D` is taken equal (ZK 0.5 mm).
pub const WYE_SAME_DIAMETER_TOL: f64 = 0.5;
/// Hold when the angle between the legs is outside 30..90 deg (AWWA C208 4.1.10; UNVERIFIED for the factory).
pub const WYE_AWWA_MIN_DEG: f64 = 30.0;
pub const WYE_AWWA_MAX_DEG: f64 = 90.0;
/// Relative error estimate above which an area is refused.
pub const WYE_AREA_REL_TOL: f64 = 1e-7;
/// Output invariant: the largest jump between two neighbouring dense samples of a generator length, relative to the
/// range of that length (a discontinuity or a garbage value breaks it).
pub const WYE_JUMP_REL: f64 = 0.05;
/// Output invariant: residual of a cut point on the surface of the neighbour that cuts it, in mm per mm of `Rs`.
pub const WYE_ON_SURFACE_TOL: f64 = 1e-9;
pub const WYE_DEFAULT_SEAMS: u32 = 1;
/// UNVERIFIED (see rule `WYE_JOINT_PLANE_TOL`): hold threshold (mm) of the difference between the two joint-plane rules
/// under the ZK rule.
pub const WYE_JOINT_PLANE_TOL: f64 = 0.5;
const MAX_MM: f64 = crate::shapes::MAX_MM;
const MIN_MM: f64 = crate::shapes::MIN_MM;
/// First seam: `-45 deg` from the plane-of-Y line of the first neighbour towards the `-z` crotch point (ZK `ths`).
const SEAM0: f64 = -PI / 4.0;

/// (name, value, kind, status)
pub const WYE_NAMED_RULES: [(&str, &str, &str, &str); 17] = [
    (
        "WYE_CONSTRUCTION_COMMON_SPHERE",
        "every leg's outer surface (cylinder or cone) touches the sphere of radius D/2 about the intersection of the three centre lines; legs are joined in the planes g_i = g_j; a wye with a branch narrower than the main needs the cone legs (cylinders alone leave the crotch open)",
        "rule (construction, from Atkins / Escher Wyss / FastSHAPES)",
        "UNVERIFIED",
    ),
    (
        "WYE_THICKNESS_NORMAL",
        "DEFAULT (and the ZK formula): the plate thickness of a cone leg is measured along the surface normal; the developed surface is the cone offset by the basis along the inward normal (radius rho - o / cos psi at the same axial position)",
        "rule (basis)",
        "UNVERIFIED",
    ),
    (
        "WYE_THICKNESS_RADIAL",
        "ALTERNATIVE, named only, NOT implemented: thickness taken radially (rho - o); ZK has no such reading for the wye",
        "rule (basis)",
        "UNVERIFIED",
    ),
    (
        "WYE_JOINT_PLANES_AMPLITUDE_SPHERE",
        "DEFAULT: the joint planes of a plate come from the sphere of its own cut-amplitude surface (radius Rs - ao), so two plates cut on their offset surfaces meet (valid because WYE_CONE_THICKNESS_IS_MAIN: every offset surface is tangent to that sphere)",
        "rule (cut construction)",
        "UNVERIFIED",
    ),
    (
        "WYE_JOINT_PLANES_OUTER_SPHERE",
        "ZK parity (selected with `wye_with_rule(.., WyeJointRule::OuterSphere)`): the joint planes come from the OUTER sphere even when the amplitude surface is offset, so plates cut on offset surfaces meet only on the outer-sphere planes; the difference to the default is result.join_plane_delta_max and, above WYE_JOINT_PLANE_TOL, a hold",
        "rule (cut construction)",
        "UNVERIFIED",
    ),
    (
        "WYE_JOINT_PLANE_TOL",
        "0.5 mm: the largest difference between the two joint-plane rules that is accepted without a hold under the ZK rule; chosen as the same order as a plate root gap and as WYE_SAME_DIAMETER_TOL, not taken from a standard or a factory",
        "tolerance",
        "UNVERIFIED",
    ),
    (
        "WYE_CUT_MAP_AXIAL",
        "the cut found on the amplitude surface is carried to the developed (neutral) surface along the same generator at the same axial position; carrying it along the surface normal instead would move a cone cut axially by |o - ao| sin(psi) (result.cut_map_axial_shift_max)",
        "rule (development)",
        "UNVERIFIED",
    ),
    (
        "WYE_BASIS_DEFAULT",
        "circumference measured at the plate mid surface, cut amplitude at the outer surface (the mitred-bend convention, observed on a partner bend drawing; not seen on a wye drawing)",
        "rule (basis)",
        "UNVERIFIED",
    ),
    (
        "WYE_SEAM_ANGLE",
        "first longitudinal seam 45 deg from the plane-of-Y line of the first neighbour towards the -z crotch point; more seams equally spaced; a cone leg is one fan seamed there",
        "rule (seam)",
        "UNVERIFIED",
    ),
    (
        "WYE_SEAM_COUNT_DEFAULT",
        "one longitudinal seam per pipe leg when none is given; a cone leg is never split",
        "rule (seam)",
        "UNVERIFIED",
    ),
    (
        "WYE_ALLOWANCE_NONE",
        "no extra length is added to any leg (no kerf, no finishing allowance)",
        "rule (allowance)",
        "UNVERIFIED",
    ),
    (
        "WYE_LENGTH_FROM_INTERSECTION",
        "L, H and H2 are measured from the intersection of the centre lines along the leg's centre line (AWWA F and Hy); a drawing that dimensions otherwise must be converted by the reader",
        "rule (dimensioning)",
        "UNVERIFIED",
    ),
    (
        "WYE_CONE_THICKNESS_IS_MAIN",
        "the cone legs have the main's plate thickness; the branch pipes beyond them have the branch thickness",
        "rule (thickness)",
        "UNVERIFIED",
    ),
    (
        "WYE_SAME_DIAMETER_TOL",
        "a branch diameter within 0.5 mm of D is the same diameter (no cone); the difference is reported as a question",
        "rule (tolerance)",
        "UNVERIFIED",
    ),
    (
        "WYE_ANGLE_RANGE_AWWA",
        "an angle between the legs outside 30..90 deg is built but withheld (hold) until confirmed (AWWA C208 4.1.10: below 30 deg a wye plus a bend)",
        "rule (range)",
        "UNVERIFIED",
    ),
    (
        "WYE_FAN_ORIENTATION",
        "fan development: the angle increases with theta from the seam generator placed on +y; the same orientation for a narrowing and a widening cone (ZK mirrors the widening one)",
        "rule (development)",
        "UNVERIFIED",
    ),
    (
        "WYE_NO_CROTCH_PLATE",
        "no crotch plate or reinforcement is generated (other / dims only)",
        "rule (scope)",
        "UNVERIFIED",
    ),
];

pub const WYE_REFUSAL_CODES: [&str; 22] = [
    "wye_value_not_finite",
    "wye_value_not_positive",
    "wye_value_too_large",
    "wye_angle_out_of_range",
    "wye_wall_too_thick",
    "wye_split_out_of_range",
    "wye_cone_length_missing",
    "wye_branch_t_missing",
    "wye_cone_not_buildable",
    "wye_cone_too_steep",
    "wye_cone_mouth_inside_apex",
    "wye_cut_inside_apex",
    "wye_end_inside_cut",
    "wye_tube_length_not_positive",
    "wye_crotch_not_found",
    "wye_cut_out_of_range",
    "wye_cut_not_continuous",
    "wye_cut_not_on_neighbour",
    "wye_area_not_converged",
    "wye_result_not_finite",
    "wye_weld_not_consistent",
    "wye_surface_offset_invalid",
];

// ---------------------------------------------------------------- spec and result

/// Where the joint planes of a plate come from (rules `WYE_JOINT_PLANES_AMPLITUDE_SPHERE` / `WYE_JOINT_PLANES_OUTER_SPHERE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WyeJointRule {
    /// Default: the sphere of the plate's amplitude surface (radius `Rs - ao`).
    AmplitudeSphere,
    /// ZK parity: the outer sphere (radius `Rs`) for every amplitude.
    OuterSphere,
}

impl WyeJointRule {
    pub fn name(self) -> &'static str {
        match self {
            WyeJointRule::AmplitudeSphere => "WYE_JOINT_PLANES_AMPLITUDE_SPHERE",
            WyeJointRule::OuterSphere => "WYE_JOINT_PLANES_OUTER_SPHERE",
        }
    }
    fn radius(self, rs: f64, ao: f64) -> f64 {
        match self {
            WyeJointRule::AmplitudeSphere => rs - ao,
            WyeJointRule::OuterSphere => rs,
        }
    }
    fn other(self) -> WyeJointRule {
        match self {
            WyeJointRule::AmplitudeSphere => WyeJointRule::OuterSphere,
            WyeJointRule::OuterSphere => WyeJointRule::AmplitudeSphere,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WyeSpec {
    /// Main outer diameter and wall.
    pub d: f64,
    pub t: f64,
    /// Swing of leg 1 and leg 2 from the straight continuation of the main (deg, each 0..180, sum = the angle between the
    /// legs, strictly between 0 and 180). A swing of 0 is a straight leg (hold `wye_leg_straight`).
    pub beta1_deg: f64,
    pub beta2_deg: f64,
    /// From the intersection along the centre line to the end of the main, leg 1, leg 2.
    pub l: f64,
    pub h1: f64,
    pub h2: f64,
    /// Branch outer diameter / wall. `branch_d` differing from `d` by more than 0.5 mm makes the legs cones (then
    /// `cone_s` and `branch_t` are required).
    pub branch_d: Option<f64>,
    pub branch_t: Option<f64>,
    /// From the intersection to the small mouth of the cone (the joint with the branch pipe).
    pub cone_s: Option<f64>,
    /// Longitudinal seams of every PIPE piece (1..=6); `None`: 1.
    pub split: Option<u32>,
    /// Surface whose circumference is developed, and surface at which the cuts are measured.
    pub basis: Basis,
    pub amp: Basis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WyeKind {
    Pipe,
    Cone,
    Tube,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WyeMark {
    pub i: usize,
    /// Generator length from the end (small mouth for a cone) to the cut.
    pub len: f64,
    /// Arc length along the end circle (neutral surface) from the seam.
    pub u: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WyePiece {
    pub kind: WyeKind,
    /// 0 main, 1 leg 1, 2 leg 2.
    pub leg: usize,
    pub t: f64,
    /// Plates (longitudinal seams); a cone is 1.
    pub n: u32,
    /// Circumference of the end circle (neutral surface) and width of one plate.
    pub circ: f64,
    pub plate_w: f64,
    /// Bounding box of one plate outline.
    pub w: f64,
    pub h: f64,
    /// Neutral-surface area of the whole piece (adaptive quadrature), its error estimate, mass.
    pub area: f64,
    pub area_err: f64,
    pub mass_kg: f64,
    pub shortest: f64,
    pub longest: f64,
    pub marks: Vec<WyeMark>,
    /// Cones: half angle, full fan angle (deg), slant distance apex to the small mouth (neutral surface, signed length
    /// `|l|`), axial position of the end. Zero for pipes.
    pub psi_deg: f64,
    pub fan_deg: f64,
    pub apex: f64,
    pub s_end: f64,
    pub seams_deg: Vec<f64>,
    /// One outline polygon per plate, each at its own origin (`x` along the circumference, `y` from the end).
    pub plates: Vec<Vec<[f64; 2]>>,
    /// ESTIMATE (not a bound) of the distance between the outline polygon and the exact curve: chord sagitta of the cut.
    pub eps_rep: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WyeHold {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WyeResult {
    pub pieces: Vec<WyePiece>,
    /// Crotch point (outer surface) and half angle of the cones (deg, 0 without cones; negative when the branch is wider).
    pub crotch: [f64; 3],
    pub psi_deg: f64,
    pub cone: bool,
    /// Weld lengths on the outer surface: main-leg 1, main-leg 2, leg 1-leg 2 (mm).
    pub welds: [f64; 3],
    pub total_mass_kg: f64,
    /// Name of the joint-plane rule used.
    pub joint_rule: &'static str,
    /// Largest change (mm, sampled at 144 angles) of any cut between the two joint-plane rules, all three legs (0 for an
    /// outer amplitude and for cylinders, whose joint planes do not depend on the sphere).
    pub join_plane_delta_max: f64,
    /// Largest radial distance (mm, sampled) between a cut point (on the plate's amplitude surface) and the amplitude
    /// surface of the neighbour that cuts it, under the rule used: ~0 when the plates meet (default rule, equal walls); the
    /// step between plates of different wall (branch_t != t) or, under the ZK rule, the fit-up gap.
    pub fit_gap_max: f64,
    /// Largest axial shift (mm) of a cone cut if the cut were carried from the amplitude to the developed surface along the
    /// surface normal instead of along the generator (rule `WYE_CUT_MAP_AXIAL`): `|o - ao| |sin psi|`.
    pub cut_map_axial_shift_max: f64,
    pub holds: Vec<WyeHold>,
    pub questions: Vec<String>,
    pub certified: bool,
    pub method: &'static str,
    pub rules: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

impl WyeResult {
    pub fn has_hold(&self, code: &str) -> bool {
        self.holds.iter().any(|h| h.code == code)
    }
}

/// One self-check of `verify_wye`.
#[derive(Debug, Clone, PartialEq)]
pub struct WyeCheck {
    pub name: &'static str,
    pub value: f64,
    pub bound: f64,
    pub ok: bool,
}

// ---------------------------------------------------------------- validation

fn num(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "wye_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v.abs() > MAX_MM {
        return Err(refuse(
            "wye_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

fn dim(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    num(name, v)?;
    if v <= 0.0 || v < MIN_MM {
        return Err(refuse(
            "wye_value_not_positive",
            name,
            format!("{name} must be greater than 0 (got {v})"),
        ));
    }
    Ok(v)
}

fn swing(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "wye_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if !(0.0..180.0).contains(&v) {
        return Err(refuse(
            "wye_angle_out_of_range",
            name,
            format!("{name} must be from 0 up to (not including) 180 deg (got {v})"),
        ));
    }
    Ok(v)
}

// ---------------------------------------------------------------- model

type V3 = [f64; 3];

fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[derive(Debug, Clone)]
struct Leg {
    u: V3,
    e1: V3,
    psi: f64,
    nb: [usize; 2],
}

#[derive(Debug, Clone)]
struct Model {
    rs: f64,
    legs: [Leg; 3],
    crotch: V3,
}

/// `g_i(P)` of the common-sphere construction for the sphere radius `rp`.
fn g_of(l: &Leg, rp: f64, p: &V3) -> f64 {
    (dot(p, &l.u) - rp * l.psi.sin()) / l.psi.cos()
}

/// A line `s = (n0 + n1 x) / (a0 + a1 x)`, `x = cos th`: the cut of leg `i` by neighbour `j`.
#[derive(Debug, Clone, Copy)]
struct Joint {
    n0: f64,
    n1: f64,
    a0: f64,
    a1: f64,
}

impl Joint {
    fn a(&self, x: f64) -> f64 {
        self.a0 + self.a1 * x
    }
    fn s(&self, x: f64) -> f64 {
        (self.n0 + self.n1 * x) / self.a(x)
    }
}

fn build_model(rs: f64, b1: f64, b2: f64, psi: f64) -> Result<Model, ShapeRefusal> {
    let u = [
        [-1.0, 0.0, 0.0],
        [b1.cos(), b1.sin(), 0.0],
        [b2.cos(), -b2.sin(), 0.0],
    ];
    let nb = [[1usize, 2usize], [0, 2], [0, 1]];
    let psis = [0.0, psi, psi];
    let mut legs = Vec::with_capacity(3);
    for i in 0..3 {
        // e1: in the plane of the Y, square to the centre line, on the side of the first neighbour (a neighbour exactly
        // opposite: on the side away from the second one)
        let e = [-u[i][1], u[i][0], 0.0];
        let a1 = dot(&e, &u[nb[i][0]]);
        let a2 = dot(&e, &u[nb[i][1]]);
        let flip = a1 < -1e-12 || (a1.abs() <= 1e-12 && a2 > 0.0);
        let e1 = if flip { [-e[0], -e[1], 0.0] } else { e };
        legs.push(Leg {
            u: u[i],
            e1,
            psi: psis[i],
            nb: nb[i],
        });
    }
    let legs: [Leg; 3] = [legs[0].clone(), legs[1].clone(), legs[2].clone()];
    // crotch: g0 = g1 = g2 are two equations in (x, y); z from the main surface y^2 + z^2 = rs^2
    let lin = |j: usize| {
        // g0 - gj = c0 + cx x + cy y
        let (l0, lj) = (&legs[0], &legs[j]);
        let k0 = -rs * l0.psi.sin() / l0.psi.cos() + rs * lj.psi.sin() / lj.psi.cos();
        (
            k0,
            l0.u[0] / l0.psi.cos() - lj.u[0] / lj.psi.cos(),
            l0.u[1] / l0.psi.cos() - lj.u[1] / lj.psi.cos(),
        )
    };
    let (k1, ax1, ay1) = lin(1);
    let (k2, ax2, ay2) = lin(2);
    let det = ax1 * ay2 - ay1 * ax2;
    if det.abs() < 1e-12 {
        return Err(refuse(
            "wye_crotch_not_found",
            "beta1_deg",
            "the three joint planes do not meet in one line (degenerate leg angles)",
        ));
    }
    let x = (-k1 * ay2 + k2 * ay1) / det;
    let y = (-ax1 * k2 + ax2 * k1) / det;
    let z2 = rs * rs - y * y;
    if z2.is_nan() || z2 < 0.0 || !x.is_finite() || !y.is_finite() {
        return Err(refuse(
            "wye_crotch_not_found",
            "beta1_deg",
            "the three joint planes meet outside the main pipe surface (no crotch point)",
        ));
    }
    Ok(Model {
        rs,
        legs,
        crotch: [x, y, z2.sqrt()],
    })
}

impl Model {
    /// The joint of leg `i` with its neighbour number `k` (0 or 1): cut on the surface offset `ao` by the planes of the
    /// sphere of radius `rp`. `G(s, x)` is evaluated from the vector definition, never from a closed form.
    fn joint(&self, i: usize, k: usize, ao: f64, rp: f64) -> Joint {
        let li = &self.legs[i];
        let lj = &self.legs[li.nb[k]];
        let g = |s: f64, x: f64| {
            let rho = (self.rs - s * li.psi.sin() - ao) / li.psi.cos();
            let p = [
                s * li.u[0] + rho * x * li.e1[0],
                s * li.u[1] + rho * x * li.e1[1],
                0.0,
            ];
            g_of(li, rp, &p) - g_of(lj, rp, &p)
        };
        let (g0m, g0p, g1m, g1p) = (g(0.0, -1.0), g(0.0, 1.0), g(1.0, -1.0), g(1.0, 1.0));
        let (am, ap) = (g1m - g0m, g1p - g0p);
        Joint {
            n0: -(g0p + g0m) / 2.0,
            n1: -(g0p - g0m) / 2.0,
            a0: (ap + am) / 2.0,
            a1: (ap - am) / 2.0,
        }
    }
}

/// Cut of one leg on the surface offset `ao`: the two joints, with the extremes in closed form.
struct LegCut {
    j: [Joint; 2],
    /// Smallest and largest cut position (mm along the axis) and the angles (rad, 0..2pi) of the kink and of the ends.
    cmin: f64,
    cmax: f64,
    breaks: Vec<f64>,
}

impl LegCut {
    fn new(m: &Model, i: usize, ao: f64, rp: f64) -> Result<LegCut, ShapeRefusal> {
        let j = [m.joint(i, 0, ao, rp), m.joint(i, 1, ao, rp)];
        for q in &j {
            // A is affine in x: positive on [-1, 1] exactly when positive at both ends
            if !(q.a(-1.0) > WYE_A_MIN && q.a(1.0) > WYE_A_MIN) {
                return Err(refuse(
                    "wye_cone_too_steep",
                    "branch_d",
                    format!(
                        "leg {i} does not leave its neighbour (slope {:.3e} / {:.3e}): the cones are steeper than half the angle between the legs",
                        q.a(-1.0),
                        q.a(1.0)
                    ),
                ));
            }
        }
        let mut xs = vec![-1.0, 1.0];
        // crossing of the two cuts: (p0 + p1 x)(w0 + w1 x) = (r0 + r1 x)(q0 + q1 x)
        let (p, q) = (&j[0], &j[1]);
        let qa = p.n1 * q.a1 - q.n1 * p.a1;
        let qb = p.n0 * q.a1 + p.n1 * q.a0 - q.n0 * p.a1 - q.n1 * p.a0;
        let qc = p.n0 * q.a0 - q.n0 * p.a0;
        let scale = qa.abs().max(qb.abs()).max(qc.abs());
        if scale > 0.0 {
            let mut roots = Vec::new();
            if qa.abs() <= 1e-14 * scale {
                if qb.abs() > 1e-14 * scale {
                    roots.push(-qc / qb);
                }
            } else {
                let disc = qb * qb - 4.0 * qa * qc;
                if disc >= 0.0 {
                    let sq = disc.sqrt();
                    // numerically stable pair
                    let qq = -0.5 * (qb + qb.signum() * sq);
                    if qq != 0.0 {
                        roots.push(qq / qa);
                        roots.push(qc / qq);
                    } else {
                        roots.push(0.0);
                    }
                }
            }
            for r in roots {
                if r.is_finite() && r > -1.0 && r < 1.0 {
                    xs.push(r);
                }
            }
        }
        let cut_at = |x: f64| j[0].s(x).max(j[1].s(x));
        let mut cmin = f64::INFINITY;
        let mut cmax = f64::NEG_INFINITY;
        let mut breaks = Vec::new();
        for &x in &xs {
            let c = cut_at(x);
            cmin = cmin.min(c);
            cmax = cmax.max(c);
            let th = x.clamp(-1.0, 1.0).acos();
            breaks.push(th);
            if th > 0.0 && th < PI {
                breaks.push(2.0 * PI - th);
            }
        }
        breaks.sort_by(|a, b| a.partial_cmp(b).unwrap());
        breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-14);
        if !cmin.is_finite() || !cmax.is_finite() {
            return Err(refuse(
                "wye_result_not_finite",
                "beta1_deg",
                format!("leg {i}: the cut is not finite"),
            ));
        }
        Ok(LegCut {
            j,
            cmin,
            cmax,
            breaks,
        })
    }

    fn cut(&self, th: f64) -> f64 {
        let x = th.cos();
        self.j[0].s(x).max(self.j[1].s(x))
    }

    /// 0 or 1: the neighbour that cuts at `th`.
    fn who(&self, th: f64) -> usize {
        let x = th.cos();
        usize::from(self.j[1].s(x) > self.j[0].s(x))
    }
}

/// Angles (rad) of the outline samples of the range `[a, b]`: `n` uniform steps plus every kink inside.
fn nodes(a: f64, b: f64, n: usize, breaks: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = (0..=n).map(|k| a + (b - a) * k as f64 / n as f64).collect();
    for turn in -1..=2 {
        for &t in breaks {
            let th = t + 2.0 * PI * f64::from(turn);
            if th > a && th < b {
                v.push(th);
            }
        }
    }
    v.sort_by(|p, q| p.partial_cmp(q).unwrap());
    v.dedup_by(|p, q| (*p - *q).abs() < 1e-12);
    v
}

/// Integral of `f` over `[a, b]`, split at the kinks. Returns (value, error estimate).
fn integrate(f: &dyn Fn(f64) -> f64, a: f64, b: f64, breaks: &[f64]) -> (f64, f64) {
    let pts = nodes(a, b, 1, breaks);
    let (mut v, mut e) = (0.0, 0.0);
    for w in pts.windows(2) {
        let (x, y) = adaptive_simpson(f, w[0], w[1]);
        v += x;
        e += y;
    }
    (v, e)
}

// ---------------------------------------------------------------- pieces

struct LegPlan {
    kind: WyeKind,
    leg: usize,
    t: f64,
    s_end: f64,
}

fn finite_all(v: &[f64]) -> bool {
    v.iter().all(|x| x.is_finite())
}

#[derive(Debug, Clone, Copy)]
struct Setup {
    cone: bool,
    psi: f64,
    sr: f64,
    r_branch: f64,
}

fn thick(name: &'static str, t: f64, r: f64) -> Result<(), ShapeRefusal> {
    if t >= r {
        return Err(refuse(
            "wye_wall_too_thick",
            name,
            format!("{name} {t} is not less than the radius {r}"),
        ));
    }
    Ok(())
}

/// Offset of a basis for the wall `t`.
fn off(b: Basis, t: f64) -> f64 {
    match b {
        Basis::Outer => 0.0,
        Basis::Mid => t / 2.0,
        Basis::Inner => t,
    }
}

/// All the checks that do not need the geometry; returns the cone data.
fn check_spec(s: &WyeSpec) -> Result<Setup, ShapeRefusal> {
    dim("d", s.d)?;
    dim("t", s.t)?;
    swing("beta1_deg", s.beta1_deg)?;
    swing("beta2_deg", s.beta2_deg)?;
    dim("l", s.l)?;
    dim("h1", s.h1)?;
    dim("h2", s.h2)?;
    for (n, v) in [
        ("branch_d", s.branch_d),
        ("branch_t", s.branch_t),
        ("cone_s", s.cone_s),
    ] {
        if let Some(v) = v {
            dim(n, v)?;
        }
    }
    let delta = s.beta1_deg + s.beta2_deg;
    if !(delta > 0.0 && delta < 180.0) {
        return Err(refuse(
            "wye_angle_out_of_range",
            "beta1_deg",
            format!(
                "the angle between the legs must be greater than 0 and less than 180 deg (got {delta})"
            ),
        ));
    }
    if let Some(n) = s.split
        && !(1..=crate::shapes::MAX_CYL_SEAMS).contains(&n)
    {
        return Err(refuse(
            "wye_split_out_of_range",
            "split",
            format!(
                "seams must be 1..={} (got {n})",
                crate::shapes::MAX_CYL_SEAMS
            ),
        ));
    }
    let rs = s.d / 2.0;
    thick("t", s.t, rs)?;
    let cone = s
        .branch_d
        .is_some_and(|b| (b - s.d).abs() > WYE_SAME_DIAMETER_TOL);
    if !cone {
        if let Some(bt) = s.branch_t {
            thick("branch_t", bt, rs)?;
        }
        return Ok(Setup {
            cone: false,
            psi: 0.0,
            sr: 0.0,
            r_branch: rs,
        });
    }
    let r = s.branch_d.unwrap_or(s.d) / 2.0;
    let Some(sr) = s.cone_s else {
        return Err(refuse(
            "wye_cone_length_missing",
            "cone_s",
            "a branch of another diameter needs cone_s (the cone length): a cone or a reducer after the wye cannot be chosen here",
        ));
    };
    let Some(bt) = s.branch_t else {
        return Err(refuse(
            "wye_branch_t_missing",
            "branch_t",
            "a branch of another diameter needs the branch wall branch_t",
        ));
    };
    thick("branch_t", bt, r)?;
    let hy = sr.hypot(r);
    if hy.is_nan() || rs >= hy {
        return Err(refuse(
            "wye_cone_not_buildable",
            "cone_s",
            format!(
                "the small mouth (axial {sr}, radius {r}) lies inside the common sphere (radius {rs}): no cone touches the sphere through it"
            ),
        ));
    }
    for (n, h) in [("h1", s.h1), ("h2", s.h2)] {
        if h <= sr {
            return Err(refuse(
                "wye_tube_length_not_positive",
                n,
                format!("{n} {h} does not reach past the small mouth of the cone ({sr})"),
            ));
        }
    }
    let psi = (rs / hy).asin() - r.atan2(sr);
    Ok(Setup {
        cone: true,
        psi,
        sr,
        r_branch: r,
    })
}

/// Rule and running diagnostics shared by the leg pieces of one wye.
struct LegCtx {
    rule: WyeJointRule,
    /// Cut-amplitude offset of each leg's plate.
    aos: [f64; 3],
    delta: f64,
    gap: f64,
    shift: f64,
}

/// Everything about one leg's pipe or cone piece.
fn leg_piece(
    m: &Model,
    s: &WyeSpec,
    plan: &LegPlan,
    seams_n: u32,
    ctx: &mut LegCtx,
) -> Result<WyePiece, ShapeRefusal> {
    let i = plan.leg;
    let psi = m.legs[i].psi;
    let (cp, sp) = (psi.cos(), psi.sin());
    let cone = plan.kind == WyeKind::Cone;
    let o = off(s.basis, plan.t);
    let ao = off(s.amp, plan.t);
    let rs = m.rs;
    let lc = LegCut::new(m, i, ao, ctx.rule.radius(rs, ao))?;
    // surface offsets must stay positive over the whole leg
    for (name, off_v) in [("basis", o), ("amp", ao)] {
        for sv in [lc.cmin.min(plan.s_end), lc.cmax.max(plan.s_end)] {
            if (rs - sv * sp - off_v) / cp <= 0.0 {
                return Err(refuse(
                    if cone && sv == plan.s_end {
                        "wye_cone_mouth_inside_apex"
                    } else {
                        "wye_cut_inside_apex"
                    },
                    "t",
                    format!(
                        "leg {i}: the {name} surface has no positive radius at axial position {sv:.3} (the wall is too thick for the cone, or the mouth is beyond the apex)"
                    ),
                ));
            }
        }
    }
    if cone && (psi.abs() < 1e-12) {
        return Err(refuse(
            "wye_cone_not_buildable",
            "branch_d",
            "a cone leg with no taper",
        ));
    }
    let room = plan.s_end - lc.cmax;
    if room.is_nan() || room <= 1e-9 {
        return Err(refuse(
            "wye_end_inside_cut",
            if i == 0 { "l" } else { "h1" },
            format!(
                "leg {i}: the end (axial {}) is not beyond the deepest cut (axial {:.4})",
                plan.s_end, lc.cmax
            ),
        ));
    }
    let len = |th: f64| (plan.s_end - lc.cut(th)) / cp;
    let r_end = (rs - plan.s_end * sp - o) / cp;
    let circ = 2.0 * PI * r_end;
    // closed-form extremes of the generator length
    let longest = (plan.s_end - lc.cmin) / cp;
    let shortest = (plan.s_end - lc.cmax) / cp;
    let marks: Vec<WyeMark> = (0..WYE_MARKS)
        .map(|k| {
            let th = PI / 8.0 * k as f64;
            WyeMark {
                i: k,
                len: len(th),
                u: r_end * mod_p(th - SEAM0, 2.0 * PI),
            }
        })
        .collect();
    // output invariants: finite, > 0, inside [shortest, longest], continuous, on the neighbour surface
    check_cut_invariants(m, &lc, i, ao, plan.s_end, cp, (shortest, longest))?;
    let (plates, area, area_err, w, h, n, eps_rep);
    if cone {
        let sab = (rs - o) / sp;
        let ell = |sv: f64| (sab - sv).abs() / cp;
        let le = ell(plan.s_end);
        let ang = sp.abs();
        let pol = |th: f64, sv: f64| {
            let a = (th - SEAM0) * ang;
            [ell(sv) * a.sin(), ell(sv) * a.cos()]
        };
        let th_nodes = nodes(SEAM0, SEAM0 + 2.0 * PI, WYE_OUTLINE_SAMPLES, &lc.breaks);
        let mut small: Vec<[f64; 2]> = th_nodes.iter().map(|&t| pol(t, plan.s_end)).collect();
        let top: Vec<[f64; 2]> = th_nodes.iter().map(|&t| pol(t, lc.cut(t))).collect();
        small.extend(top.into_iter().rev());
        let f = |th: f64| {
            let lcut = ell(lc.cut(th));
            (lcut * lcut - le * le).abs()
        };
        let (v, e) = integrate(&f, SEAM0, SEAM0 + 2.0 * PI, &lc.breaks);
        area = 0.5 * ang * v;
        area_err = 0.5 * ang * e;
        let bb = extent(&small);
        w = bb[2] - bb[0];
        h = bb[3] - bb[1];
        plates = vec![small];
        n = 1;
        eps_rep = sagitta(&len);
    } else {
        let rb = rs - o;
        let nn = seams_n;
        let wp = 2.0 * PI * rb / f64::from(nn);
        let mut pl = Vec::new();
        let (mut ar, mut er) = (0.0, 0.0);
        let (mut wmax, mut hmax) = (0.0f64, 0.0f64);
        for j in 0..nn {
            let a0 = SEAM0 + 2.0 * PI * f64::from(j) / f64::from(nn);
            let a1 = a0 + 2.0 * PI / f64::from(nn);
            let per = (WYE_OUTLINE_SAMPLES as f64 / f64::from(nn)).ceil() as usize;
            let th_nodes = nodes(a0, a1, per, &lc.breaks);
            let mut poly = vec![[0.0, 0.0], [wp, 0.0]];
            for &t in th_nodes.iter().rev() {
                poly.push([rb * (t - a0), len(t)]);
            }
            let bb = extent(&poly);
            wmax = wmax.max(bb[2] - bb[0]);
            hmax = hmax.max(bb[3] - bb[1]);
            let (v, e) = integrate(&len, a0, a1, &lc.breaks);
            ar += rb * v;
            er += rb * e;
            pl.push(poly);
        }
        plates = pl;
        area = ar;
        area_err = er;
        w = wmax;
        h = hmax;
        n = nn;
        eps_rep = sagitta(&len);
    }
    if !(area.is_finite() && area > 0.0) || area_err > WYE_AREA_REL_TOL * area {
        return Err(refuse(
            "wye_area_not_converged",
            "d",
            format!("leg {i}: area {area} (error estimate {area_err}) not trustworthy"),
        ));
    }
    // difference between the two joint-plane rules (every leg: the main's joints with a cone also move)
    if ao > 0.0 {
        let alt = LegCut::new(m, i, ao, ctx.rule.other().radius(rs, ao))?;
        for k in 0..145 {
            let th = 2.0 * PI * f64::from(k) / 144.0;
            ctx.delta = ctx.delta.max((lc.cut(th) - alt.cut(th)).abs());
        }
    }
    // fit-up gap on the amplitude surfaces: at the marks, the kinks and 144 more angles
    let mut ths: Vec<f64> = (0..144).map(|k| 2.0 * PI * f64::from(k) / 144.0).collect();
    ths.extend((0..WYE_MARKS).map(|k| PI / 8.0 * k as f64));
    ths.extend(lc.breaks.iter().copied());
    for th in ths {
        ctx.gap = ctx.gap.max(fit_gap(m, i, &lc, ao, &ctx.aos, th));
    }
    if cone {
        ctx.shift = ctx.shift.max((o - ao).abs() * sp.abs());
    }
    let seams_deg = (0..n)
        .map(|j| mod_p(-45.0 + 360.0 * f64::from(j) / f64::from(n), 360.0))
        .collect();
    let (psi_deg, fan_deg, apex) = if cone {
        let sab = (rs - o) / sp;
        (
            psi.to_degrees(),
            360.0 * sp.abs(),
            (sab - plan.s_end).abs() / cp,
        )
    } else {
        (0.0, 0.0, 0.0)
    };
    let piece = WyePiece {
        kind: plan.kind,
        leg: i,
        t: plan.t,
        n,
        circ,
        plate_w: circ / f64::from(n),
        w,
        h,
        area,
        area_err,
        mass_kg: mass_kg(area, plan.t),
        shortest,
        longest,
        marks,
        psi_deg,
        fan_deg,
        apex,
        s_end: plan.s_end,
        seams_deg,
        plates,
        eps_rep,
    };
    let nums = [
        piece.circ,
        piece.w,
        piece.h,
        piece.area,
        piece.mass_kg,
        piece.shortest,
        piece.longest,
        piece.apex,
        piece.fan_deg,
    ];
    if !finite_all(&nums)
        || piece
            .plates
            .iter()
            .any(|p| p.iter().any(|q| !finite_all(q)))
    {
        return Err(refuse(
            "wye_result_not_finite",
            "d",
            format!("leg {i}: a result is not finite"),
        ));
    }
    Ok(piece)
}

fn extent(pts: &[[f64; 2]]) -> [f64; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in pts {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    }
    b
}

/// Chord sagitta of the generator length at the outline step (ESTIMATE of the outline error along the generator, mm).
fn sagitta(len: &dyn Fn(f64) -> f64) -> f64 {
    let dth = 2.0 * PI / WYE_OUTLINE_SAMPLES as f64;
    let mut worst = 0.0f64;
    for k in 0..WYE_OUTLINE_SAMPLES {
        let a = dth * k as f64;
        let mid = len(a + dth / 2.0);
        let chord = 0.5 * (len(a) + len(a + dth));
        worst = worst.max((mid - chord).abs());
    }
    worst
}

fn check_cut_invariants(
    m: &Model,
    lc: &LegCut,
    i: usize,
    ao: f64,
    s_end: f64,
    cp: f64,
    (shortest, longest): (f64, f64),
) -> Result<(), ShapeRefusal> {
    let nd = WYE_DENSE_SAMPLES;
    let range = longest - shortest;
    let tol = 1e-9 * (1.0 + longest.abs());
    let mut prev: Option<f64> = None;
    let mut worst_jump = 0.0f64;
    for k in 0..=nd {
        let th = 2.0 * PI * k as f64 / nd as f64;
        let c = lc.cut(th);
        let len = (s_end - c) / cp;
        if !len.is_finite() || len <= 0.0 || len < shortest - tol || len > longest + tol {
            return Err(refuse(
                "wye_cut_out_of_range",
                "d",
                format!(
                    "leg {i}: generator length {len} at {:.3} deg is outside (0, {longest}] or its closed-form range [{shortest}, {longest}]",
                    th.to_degrees()
                ),
            ));
        }
        if let Some(p) = prev {
            worst_jump = worst_jump.max((len - p).abs());
        }
        prev = Some(len);
    }
    if worst_jump > WYE_JUMP_REL * range + 1e-6 {
        return Err(refuse(
            "wye_cut_not_continuous",
            "d",
            format!(
                "leg {i}: generator length jumps by {worst_jump:.4} between neighbouring samples (range {range:.4})"
            ),
        ));
    }
    // the cut point of the outer surface lies on the neighbour that cuts it (marks and the kinks)
    if ao == 0.0 {
        let mut ths: Vec<f64> = (0..WYE_MARKS).map(|k| PI / 8.0 * k as f64).collect();
        ths.extend(lc.breaks.iter().copied());
        for th in ths {
            let r = surface_residual(m, i, lc, th);
            if r.is_nan() || r.abs() > WYE_ON_SURFACE_TOL * m.rs.max(1.0) {
                return Err(refuse(
                    "wye_cut_not_on_neighbour",
                    "d",
                    format!(
                        "leg {i}: the cut point at {:.3} deg is {r:.3e} mm off the neighbour's surface",
                        th.to_degrees()
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Distance (signed, radial) of the cut point of leg `i` at `th` (outer surface) from the surface of the neighbour that
/// cuts it there: the 3D point and the neighbour's axis distance and radius, from vectors.
fn surface_residual(m: &Model, i: usize, lc: &LegCut, th: f64) -> f64 {
    let li = &m.legs[i];
    let s = lc.cut(th);
    let rho = (m.rs - s * li.psi.sin()) / li.psi.cos();
    let p = [
        s * li.u[0] + rho * th.cos() * li.e1[0],
        s * li.u[1] + rho * th.cos() * li.e1[1],
        rho * th.sin(),
    ];
    let lj = &m.legs[li.nb[lc.who(th)]];
    let sj = dot(&p, &lj.u);
    let perp = (dot(&p, &p) - sj * sj).max(0.0).sqrt();
    perp - (m.rs - sj * lj.psi.sin()) / lj.psi.cos()
}

/// Radial distance of the cut point of leg `i` at `th` (on its surface offset `ao`) from the surface of the neighbour that
/// cuts it there, offset by that neighbour's own amplitude `aos[j]`: from vectors.
fn fit_gap(m: &Model, i: usize, lc: &LegCut, ao: f64, aos: &[f64; 3], th: f64) -> f64 {
    let li = &m.legs[i];
    let s = lc.cut(th);
    let rho = (m.rs - s * li.psi.sin() - ao) / li.psi.cos();
    let p = [
        s * li.u[0] + rho * th.cos() * li.e1[0],
        s * li.u[1] + rho * th.cos() * li.e1[1],
        rho * th.sin(),
    ];
    let j = li.nb[lc.who(th)];
    let lj = &m.legs[j];
    let sj = dot(&p, &lj.u);
    let perp = (dot(&p, &p) - sj * sj).max(0.0).sqrt();
    (perp - (m.rs - sj * lj.psi.sin() - aos[j]) / lj.psi.cos()).abs()
}

/// Weld length (outer surface, 3D polyline) of the joint of leg `i` with leg `j`, seen from leg `i`.
fn weld_from(m: &Model, i: usize, j: usize) -> Result<f64, ShapeRefusal> {
    let lc = LegCut::new(m, i, 0.0, m.rs)?;
    let li = &m.legs[i];
    let k = usize::from(li.nb[1] == j);
    let pt = |th: f64| {
        let s = lc.cut(th);
        let rho = (m.rs - s * li.psi.sin()) / li.psi.cos();
        [
            s * li.u[0] + rho * th.cos() * li.e1[0],
            s * li.u[1] + rho * th.cos() * li.e1[1],
            rho * th.sin(),
        ]
    };
    let ths = nodes(0.0, 2.0 * PI, WYE_DENSE_SAMPLES, &lc.breaks);
    let mut total = 0.0;
    for w in ths.windows(2) {
        let mid = 0.5 * (w[0] + w[1]);
        if lc.who(mid) == k {
            let (a, b) = (pt(w[0]), pt(w[1]));
            total += ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        }
    }
    Ok(total)
}

// ---------------------------------------------------------------- the shape

/// Build every piece of the wye. Errors are coded refusals (`wye_*`); holds and questions are in the result.
pub fn wye(s: &WyeSpec) -> Result<WyeResult, ShapeRefusal> {
    wye_with_rule(s, WyeJointRule::AmplitudeSphere)
}

/// As `wye`, with the joint-plane rule chosen. `OuterSphere` is the ZK reading (UNVERIFIED); a difference to the default
/// above `WYE_JOINT_PLANE_TOL` is then a hold (`wye_joint_plane_delta_over_tol`) that blocks the DXF.
pub fn wye_with_rule(s: &WyeSpec, rule: WyeJointRule) -> Result<WyeResult, ShapeRefusal> {
    let su = check_spec(s)?;
    let rs = s.d / 2.0;
    let (b1, b2) = (s.beta1_deg.to_radians(), s.beta2_deg.to_radians());
    let m = build_model(rs, b1, b2, su.psi)?;
    let bt = s.branch_t.unwrap_or(s.t);
    let seams_n = s.split.unwrap_or(WYE_DEFAULT_SEAMS);
    let mut plans = vec![LegPlan {
        kind: WyeKind::Pipe,
        leg: 0,
        t: s.t,
        s_end: s.l,
    }];
    if su.cone {
        for (leg, _h) in [(1usize, s.h1), (2, s.h2)] {
            plans.push(LegPlan {
                kind: WyeKind::Cone,
                leg,
                t: s.t,
                s_end: su.sr,
            });
        }
    } else {
        plans.push(LegPlan {
            kind: WyeKind::Pipe,
            leg: 1,
            t: bt,
            s_end: s.h1,
        });
        plans.push(LegPlan {
            kind: WyeKind::Pipe,
            leg: 2,
            t: bt,
            s_end: s.h2,
        });
    }
    let mut aos = [0.0f64; 3];
    for p in &plans {
        aos[p.leg] = off(s.amp, p.t);
    }
    let mut ctx = LegCtx {
        rule,
        aos,
        delta: 0.0,
        gap: 0.0,
        shift: 0.0,
    };
    let mut pieces = Vec::new();
    for p in &plans {
        pieces.push(leg_piece(&m, s, p, seams_n, &mut ctx)?);
    }
    let LegCtx {
        delta, gap, shift, ..
    } = ctx;
    if su.cone {
        // branch pipes beyond the cones
        let r = su.r_branch;
        let o = off(s.basis, bt);
        for (leg, h) in [(1usize, s.h1), (2, s.h2)] {
            let rt = r - o;
            if rt.is_nan() || rt <= 0.0 {
                return Err(refuse(
                    "wye_surface_offset_invalid",
                    "branch_t",
                    "the branch pipe basis surface has no positive radius",
                ));
            }
            let len = h - su.sr;
            let nn = seams_n;
            let circ = 2.0 * PI * rt;
            let wp = circ / f64::from(nn);
            let area = circ * len;
            let plate = vec![[0.0, 0.0], [wp, 0.0], [wp, len], [0.0, len]];
            pieces.push(WyePiece {
                kind: WyeKind::Tube,
                leg,
                t: bt,
                n: nn,
                circ,
                plate_w: wp,
                w: wp,
                h: len,
                area,
                area_err: 0.0,
                mass_kg: mass_kg(area, bt),
                shortest: len,
                longest: len,
                marks: Vec::new(),
                psi_deg: 0.0,
                fan_deg: 0.0,
                apex: 0.0,
                s_end: h,
                seams_deg: (0..nn)
                    .map(|j| mod_p(-45.0 + 360.0 * f64::from(j) / f64::from(nn), 360.0))
                    .collect(),
                plates: vec![plate; nn as usize],
                eps_rep: 0.0,
            });
        }
    }
    let welds = [
        weld_from(&m, 0, 1)?,
        weld_from(&m, 0, 2)?,
        weld_from(&m, 1, 2)?,
    ];
    // the two sides of a joint must give one length
    let back = [
        weld_from(&m, 1, 0)?,
        weld_from(&m, 2, 0)?,
        weld_from(&m, 2, 1)?,
    ];
    for k in 0..3 {
        let dw = (welds[k] - back[k]).abs();
        if dw.is_nan() || dw > 1e-5 * welds[k].abs().max(1.0) {
            return Err(refuse(
                "wye_weld_not_consistent",
                "d",
                format!(
                    "joint {k}: weld length {} from one side, {} from the other",
                    welds[k], back[k]
                ),
            ));
        }
    }
    let total_mass_kg: f64 = pieces.iter().map(|p| p.mass_kg).sum();
    if !finite_all(&[
        total_mass_kg,
        welds[0],
        welds[1],
        welds[2],
        delta,
        gap,
        shift,
    ]) || !finite_all(&m.crotch)
    {
        return Err(refuse(
            "wye_result_not_finite",
            "d",
            "a total is not finite",
        ));
    }
    let (mut holds, mut questions) = (Vec::new(), Vec::new());
    let ang = s.beta1_deg + s.beta2_deg;
    if !(WYE_AWWA_MIN_DEG..=WYE_AWWA_MAX_DEG).contains(&ang) {
        holds.push(WyeHold {
            code: "wye_angle_outside_awwa_range",
            message: format!(
                "angle between the legs {ang} deg is outside 30..90 deg: confirm (AWWA C208 4.1.10)"
            ),
        });
    }
    if rule == WyeJointRule::OuterSphere && delta > WYE_JOINT_PLANE_TOL {
        holds.push(WyeHold {
            code: "wye_joint_plane_delta_over_tol",
            message: format!(
                "joint planes from the outer sphere (ZK rule) move a cut by {delta:.3} mm against the amplitude-surface planes (tolerance {WYE_JOINT_PLANE_TOL} mm, UNVERIFIED): plates cut on their offset surfaces do not meet (gap {gap:.3} mm); confirm the rule or use the cut amplitude 'outer'"
            ),
        });
    }
    if gap > 1e-6 && !(rule == WyeJointRule::OuterSphere && delta > WYE_JOINT_PLANE_TOL) {
        questions.push(format!(
            "plates cut on their amplitude surfaces differ by {gap:.3} mm at the joints (plates of different wall, or the ZK joint-plane rule): confirm the fit-up"
        ));
    }
    if s.beta1_deg < 1e-9 || s.beta2_deg < 1e-9 {
        holds.push(WyeHold {
            code: "wye_leg_straight",
            message: "one leg continues the main straight: confirm whether this is a lateral (tee with branch angle) instead".into(),
        });
    }
    if su.cone && su.psi < 0.0 {
        holds.push(WyeHold {
            code: "wye_branch_wider_than_main",
            message: "the branch is wider than the main: built from the common-sphere formulas, no worked example exists; confirm".into(),
        });
    }
    if let Some(bd) = s.branch_d
        && !su.cone
    {
        if (bd - s.d).abs() > 0.0 {
            questions.push(format!(
                "branch_d {bd} differs from d {} by {:.3} mm (within {WYE_SAME_DIAMETER_TOL} mm): taken as the same diameter",
                s.d,
                (bd - s.d).abs()
            ));
        }
        if s.cone_s.is_some() {
            holds.push(WyeHold {
                code: "wye_input_unused",
                message: "cone_s is given but the branch has the main's diameter: no cone is built, cone_s is not used; confirm".into(),
            });
        }
    }
    if !su.cone && s.cone_s.is_some() && s.branch_d.is_none() {
        holds.push(WyeHold {
            code: "wye_input_unused",
            message:
                "cone_s is given without branch_d: no cone is built, cone_s is not used; confirm"
                    .into(),
        });
    }
    if m.crotch[0] > 1e-9 {
        questions.push(format!(
            "the end of the main protrudes {:.3} mm ahead of the intersection at the crotch point (cones): confirm whether L is measured to the intersection or to the tip",
            m.crotch[0]
        ));
    }
    questions.push("lengths are measured from the intersection along the centre lines (AWWA F, Hy): confirm against the drawing".into());
    Ok(WyeResult {
        pieces,
        crotch: m.crotch,
        psi_deg: su.psi.to_degrees(),
        cone: su.cone,
        welds,
        total_mass_kg,
        joint_rule: rule.name(),
        join_plane_delta_max: delta,
        fit_gap_max: gap,
        cut_map_axial_shift_max: shift,
        holds,
        questions,
        certified: false,
        method: WYE_METHOD,
        rules: &WYE_NAMED_RULES,
    })
}

// ---------------------------------------------------------------- verify

/// Independent self-checks of a result, from the vector definition of the geometry (not from the formulas that built it):
/// every cut point lies on the surface of the neighbour that cuts it and on the right side of the other; the crotch point
/// lies on the three surfaces; each cone is tangent to the common sphere; the two sides of each weld agree; the exact area
/// agrees with the outline polygon; the total mass is the sum of the pieces. A check outside its bound is `ok: false`.
pub fn verify_wye(s: &WyeSpec, r: &WyeResult) -> Result<Vec<WyeCheck>, ShapeRefusal> {
    let su = check_spec(s)?;
    let rs = s.d / 2.0;
    let m = build_model(
        rs,
        s.beta1_deg.to_radians(),
        s.beta2_deg.to_radians(),
        su.psi,
    )?;
    let mut out = Vec::new();
    let scale = rs.max(1.0);
    // cut points on the neighbour and on the correct side of the other
    let (mut on_n, mut side) = (0.0f64, 0.0f64);
    for i in 0..3 {
        let lc = LegCut::new(&m, i, 0.0, rs)?;
        for k in 0..360 {
            let th = 2.0 * PI * f64::from(k) / 360.0;
            on_n = on_n.max(surface_residual(&m, i, &lc, th).abs());
            let li = &m.legs[i];
            let sv = lc.cut(th);
            let rho = (rs - sv * li.psi.sin()) / li.psi.cos();
            let p = [
                sv * li.u[0] + rho * th.cos() * li.e1[0],
                sv * li.u[1] + rho * th.cos() * li.e1[1],
                rho * th.sin(),
            ];
            for &j in &li.nb {
                let gap = g_of(li, rs, &p) - g_of(&m.legs[j], rs, &p);
                side = side.max(-gap);
            }
        }
    }
    out.push(WyeCheck {
        name: "cut_on_neighbour_surface",
        value: on_n,
        bound: WYE_ON_SURFACE_TOL * scale,
        ok: on_n <= WYE_ON_SURFACE_TOL * scale,
    });
    out.push(WyeCheck {
        name: "cut_on_retained_side",
        value: side.max(0.0),
        bound: 1e-9 * scale,
        ok: side <= 1e-9 * scale,
    });
    // crotch on all three surfaces
    let c = r.crotch;
    let mut worst = 0.0f64;
    for l in &m.legs {
        let sj = dot(&c, &l.u);
        let perp = (dot(&c, &c) - sj * sj).max(0.0).sqrt();
        worst = worst.max((perp - (rs - sj * l.psi.sin()) / l.psi.cos()).abs());
    }
    out.push(WyeCheck {
        name: "crotch_on_three_surfaces",
        value: worst,
        bound: 1e-9 * scale,
        ok: worst <= 1e-9 * scale,
    });
    // cone tangent to the sphere: distance of the generator from the origin is rs
    let psi = m.legs[1].psi;
    let rho = |sv: f64| (rs - sv * psi.sin()) / psi.cos();
    let (s1, s2) = (0.0, 100.0);
    let (r1, r2) = (rho(s1), rho(s2));
    let dist = (s1 * r2 - s2 * r1).abs() / (s2 - s1).hypot(r2 - r1);
    out.push(WyeCheck {
        name: "cone_tangent_to_common_sphere",
        value: (dist - rs).abs(),
        bound: 1e-9 * scale,
        ok: (dist - rs).abs() <= 1e-9 * scale,
    });
    // weld agreement
    let mut wd = 0.0f64;
    for (k, (i, j)) in [(0usize, 1usize), (0, 2), (1, 2)].into_iter().enumerate() {
        let a = weld_from(&m, i, j)?;
        let b = weld_from(&m, j, i)?;
        wd = wd
            .max((a - b).abs() / a.abs().max(1.0))
            .max((a - r.welds[k]).abs() / a.abs().max(1.0));
    }
    out.push(WyeCheck {
        name: "weld_length_both_sides",
        value: wd,
        bound: 1e-5,
        ok: wd <= 1e-5,
    });
    // exact area against the outline polygon (chord error only)
    let mut ad = 0.0f64;
    for p in &r.pieces {
        let poly: f64 = p.plates.iter().map(|q| shoelace(q)).sum();
        ad = ad.max((poly - p.area).abs() / p.area);
    }
    out.push(WyeCheck {
        name: "area_vs_outline_polygon",
        value: ad,
        bound: 2e-4,
        ok: ad <= 2e-4,
    });
    let sum: f64 = r.pieces.iter().map(|p| p.mass_kg).sum();
    out.push(WyeCheck {
        name: "total_mass_is_sum",
        value: (sum - r.total_mass_kg).abs(),
        bound: 1e-9 * sum.max(1.0),
        ok: (sum - r.total_mass_kg).abs() <= 1e-9 * sum.max(1.0),
    });
    Ok(out)
}

// ---------------------------------------------------------------- probes for the independent tests

/// Read-only probes of the internal model for tests (the tests recompute everything from vectors; these only expose the
/// numbers under test).
pub mod probe {
    use super::*;

    /// Cut position (axial, mm) of leg `leg` at angle `th` (rad) on the surface offset `ao`, joint planes of the outer
    /// sphere (`own_planes = false`) or of the sphere `Rs - ao` (`true`).
    pub fn cut(s: &WyeSpec, leg: usize, th: f64, ao: f64, own_planes: bool) -> Option<f64> {
        let su = check_spec(s).ok()?;
        let rs = s.d / 2.0;
        let m = build_model(
            rs,
            s.beta1_deg.to_radians(),
            s.beta2_deg.to_radians(),
            su.psi,
        )
        .ok()?;
        let rp = if own_planes { rs - ao } else { rs };
        Some(LegCut::new(&m, leg, ao, rp).ok()?.cut(th))
    }

    /// (cut min, cut max) in closed form, default joint-plane rule (sphere `Rs - ao`).
    pub fn extremes(s: &WyeSpec, leg: usize, ao: f64) -> Option<(f64, f64)> {
        let su = check_spec(s).ok()?;
        let rs = s.d / 2.0;
        let m = build_model(
            rs,
            s.beta1_deg.to_radians(),
            s.beta2_deg.to_radians(),
            su.psi,
        )
        .ok()?;
        let lc = LegCut::new(&m, leg, ao, rs - ao).ok()?;
        Some((lc.cmin, lc.cmax))
    }

    /// Half angle of the cone legs (rad, 0 for a wye of one diameter), crotch point.
    pub fn model(s: &WyeSpec) -> Option<(f64, [f64; 3])> {
        let su = check_spec(s).ok()?;
        let m = build_model(
            s.d / 2.0,
            s.beta1_deg.to_radians(),
            s.beta2_deg.to_radians(),
            su.psi,
        )
        .ok()?;
        Some((su.psi, m.crotch))
    }

    /// Leg direction and the in-plane square direction `e1` of leg `leg`.
    pub fn frame(s: &WyeSpec, leg: usize) -> Option<([f64; 3], [f64; 3])> {
        let su = check_spec(s).ok()?;
        let m = build_model(
            s.d / 2.0,
            s.beta1_deg.to_radians(),
            s.beta2_deg.to_radians(),
            su.psi,
        )
        .ok()?;
        Some((m.legs[leg].u, m.legs[leg].e1))
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn model() -> (Model, f64) {
        (
            build_model(
                508.0,
                std::f64::consts::FRAC_PI_4,
                std::f64::consts::FRAC_PI_4,
                0.0,
            )
            .unwrap(),
            508.0,
        )
    }

    /// The output invariants fire on corrupted data (they are the guard against a silently wrong cut, L9a lesson).
    #[test]
    fn invariants_refuse_corrupted_cuts() {
        let (m, rs) = model();
        let lc = LegCut::new(&m, 1, 0.0, rs).unwrap();
        let (short, long) = (900.0 - lc.cmax, 900.0 - lc.cmin);
        assert!(check_cut_invariants(&m, &lc, 1, 0.0, 900.0, 1.0, (short, long)).is_ok());
        // a claimed range that the cut leaves
        let e =
            check_cut_invariants(&m, &lc, 1, 0.0, 900.0, 1.0, (short + 10.0, long)).unwrap_err();
        assert_eq!(e.code, "wye_cut_out_of_range");
        // a leg end inside the cut: length <= 0
        let e = check_cut_invariants(&m, &lc, 1, 0.0, 100.0, 1.0, (short, long)).unwrap_err();
        assert_eq!(e.code, "wye_cut_out_of_range");
        // a cut that is not on the neighbour surface (plane shifted by 5 mm)
        let mut bad = LegCut::new(&m, 1, 0.0, rs).unwrap();
        bad.j[0].n0 += 5.0 * bad.j[0].a0;
        bad.j[1].n0 += 5.0 * bad.j[1].a0;
        let vals: Vec<f64> = (0..5000)
            .map(|k| bad.cut(2.0 * PI * f64::from(k) / 5000.0))
            .collect();
        let (lo, hi) = (
            vals.iter().cloned().fold(f64::INFINITY, f64::min),
            vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        let e = check_cut_invariants(&m, &bad, 1, 0.0, 900.0, 1.0, (900.0 - hi, 900.0 - lo))
            .unwrap_err();
        assert_eq!(e.code, "wye_cut_not_on_neighbour");
        // a cut with a pole near x = -1 (slope almost 0 there): steep but finite; continuity guard fires
        let mut steep = LegCut::new(&m, 1, 0.0, rs).unwrap();
        steep.j[0] = Joint {
            n0: 1.0,
            n1: 0.0,
            a0: 1.000_001e-3,
            a1: 1e-3,
        };
        steep.j[1] = steep.j[0];
        let (smin, smax) = (steep.j[0].s(1.0), steep.j[0].s(-1.0));
        let e = check_cut_invariants(&m, &steep, 1, 0.0, 3.0e9, 1.0, (3.0e9 - smax, 3.0e9 - smin))
            .unwrap_err();
        assert_eq!(e.code, "wye_cut_not_continuous");
    }

    /// Exact periodic wrap: the cut at theta and theta + 2 pi are the same value, at the seam angle and at 0 / pi.
    #[test]
    fn cut_is_periodic_and_even() {
        let (m, rs) = model();
        for i in 0..3 {
            let lc = LegCut::new(&m, i, 0.0, rs).unwrap();
            for k in 0..64 {
                let th = 2.0 * PI * f64::from(k) / 64.0;
                assert!((lc.cut(th) - lc.cut(th + 2.0 * PI)).abs() < 1e-9);
                assert!((lc.cut(th) - lc.cut(-th)).abs() < 1e-12);
            }
        }
    }
}
