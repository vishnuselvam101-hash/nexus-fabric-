//! NFJ-1 S1-L6b: mitered (lobster-back) bend and reducing mitered bend: closed-form model, cut curves and developed
//! sheets. Pure functions, no I/O, no engine state.
//!
//! Status (stated, not hidden): NOT reachable from `job::run`. No `Request` variant, no `Pattern`, no DXF and no AMOEBA
//! digest is produced from it, so nothing computed here can be served as a cut file. `ENGINE_VERSION` is bumped by
//! L11 (0.6.0). A result is `certified` only where a rigorous bound of the drawn cut error is available.
//!
//! Geometry (spec docs/shapes/spec_miterbend.md for the equal-diameter bend, ZK spec_redbend.md for the reducing one):
//! the centre line is the polygon circumscribed about the arc of radius `R`; `joints` cut planes split it into
//! `joints + 1` pieces. A reducing bend has cylindrical end pieces and conical middle pieces that all circumscribe
//! spheres centred on the joint points, so every cut is a plane ellipse (AWWA C208 Figure 5 construction, UNVERIFIED as
//! the factory's construction). With `d_end` equal to `d` it reduces exactly to the plain bend.
//!
//! Exactness: tilt, joint angles, cut offsets, axis and generatrix lengths are exact closed forms. The developed
//! area is an adaptive Simpson integral with its a-posteriori estimate returned (`area_err`, refused above
//! `AREA_REL_TOL`); the sheet extents (width, height) are the extremes of the cut curves found by dense sampling and
//! golden-section refinement (`extent_refine_gain` is the amount refinement moved them, an estimate, not a bound).
//! Both are NOT interval-certified.
//!
//! Named rules (all UNVERIFIED, none confirmed by a factory) are listed in `BEND_NAMED_RULES` and echoed per result.

use crate::shapes::{Basis, ShapeRefusal, adaptive_simpson, mass_kg, refuse};
use std::f64::consts::PI;

/// Largest magnitude accepted for a dimension (mm) and smallest positive one: the engine limits.
const MAX_MM: f64 = crate::shapes::MAX_MM;
const MIN_MM: f64 = crate::shapes::MIN_MM;

/// RULE `ALLOW_PER_PIECE_MM`, UNVERIFIED (ZK parity, no public source): extra length added to every piece.
pub const ALLOW_PER_PIECE_MM: f64 = 4.0;
/// RULE `SEAM_FROM_HEEL_DEG`, UNVERIFIED (ZK parity): longitudinal seam 45 deg from the inner generatrix (heel).
pub const SEAM_FROM_HEEL_DEG: f64 = 45.0;
/// RULE `RED_BEND_MIN_DIFF_MM`, UNVERIFIED (ZK `isRedBend`): a mouth diameter difference at or below this is a plain bend.
pub const RED_BEND_MIN_DIFF_MM: f64 = 0.5;
/// Cone tilt (deg) above which a note is attached: AWWA C208 4.1.11 reducer length 4(DL-DS) converted; applying it to
/// a bend piece is an inference (UNVERIFIED).
pub const TILT_NOTE_DEG: f64 = 7.125;
/// IMPLEMENTATION LIMIT (not a fabrication rule, UNVERIFIED): at most this many joints and split pieces.
pub const MAX_BEND_JOINTS: u32 = 64;
pub const MAX_BEND_SPLIT: u32 = 12;
/// Fallback piece count table (largest angle included, pieces): MAFF 9.7.1(1)a, medium confidence; a fabricator snippet
/// gives 3/4/5 with 45/67.5 deg (conflict, low confidence). Used ONLY when the drawing gives no joint count.
pub const JOINT_TABLE: [(f64, u32); 3] = [(30.0, 2), (60.0, 3), (90.0, 4)];
/// Question attached to every result whose joint count came from the table.
pub const JOINTS_QUESTION: &str = "joints not on the drawing: confirm";
/// Relative acceptance of the developed-area quadrature estimate.
pub const AREA_REL_TOL: f64 = 1e-9;
/// Method tag carried by every result.
pub const BEND_METHOD: &str =
    "closed_form_cuts+adaptive_simpson_area+sampled_extents; L11 representation not certified";
const EXTENT_SAMPLES: usize = 1440;

/// (name, value, kind, status)
pub const BEND_NAMED_RULES: [(&str, &str, &str, &str); 9] = [
    (
        "bend_swing_radius",
        "basis|outer (default basis)",
        "rule (cut-curve swing radius)",
        "UNVERIFIED",
    ),
    (
        "ALLOW_PER_PIECE_MM",
        "4",
        "rule (extra length per piece)",
        "UNVERIFIED",
    ),
    (
        "SEAM_FROM_HEEL_DEG",
        "45",
        "rule (seam azimuth)",
        "UNVERIFIED",
    ),
    (
        "RED_BEND_MIN_DIFF_MM",
        "0.5",
        "rule (plain vs reducing)",
        "UNVERIFIED",
    ),
    (
        "TILT_NOTE_DEG",
        "7.125",
        "inference (C208 reducer length)",
        "UNVERIFIED",
    ),
    (
        "JOINT_TABLE",
        "30:2,60:3,90:4 pieces",
        "fallback only (drawing wins)",
        "UNVERIFIED",
    ),
    (
        "REDUCING_CONSTRUCTION",
        "all middle pieces one cone (AWWA C208 Fig. 5)",
        "rule (construction)",
        "UNVERIFIED",
    ),
    (
        "MAX_BEND_JOINTS/MAX_BEND_SPLIT",
        "64/12",
        "implementation limit",
        "UNVERIFIED",
    ),
    (
        "AREA_REL_TOL",
        "1e-9",
        "quadrature acceptance (not a fabrication value)",
        "implementation",
    ),
];

/// Stable refusal codes of this module (documented in docs/backend/REFUSAL_CODES.md when wired into the engine).
pub const BEND_REFUSAL_CODES: [&str; 16] = [
    "bend_value_not_finite",
    "bend_value_not_positive",
    "bend_value_too_large",
    "bend_wall_too_thick",
    "bend_angle_invalid",
    "bend_joints_invalid",
    "bend_joints_above_table",
    "bend_split_out_of_range",
    "bend_reducing_needs_two_joints",
    "bend_cone_too_steep",
    "bend_cut_degenerate",
    "bend_cuts_cross_at_heel",
    "bend_area_not_converged",
    "bend_result_not_finite",
    "bend_taper_piece_out_of_range",
    "bend_taper_piece_not_reducing",
];

/// Rule `bend_swing_radius`: radius of the cut-curve swing. `Basis` = the developed (basis) surface (default, consistent
/// with the width); `Outer` = D/2 (ZK behaviour; may be a fit-up convention for bevelled thick plate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwingRule {
    Basis,
    Outer,
}

/// Where the seam angle is measured from: the heel (inner generatrix, ZK ground side) or the top (outer generatrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamFrom {
    Heel,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointSource {
    /// Read from the drawing: wins.
    Drawing,
    /// Not on the drawing: from `JOINT_TABLE`; the result carries `JOINTS_QUESTION`.
    TableRule,
}

#[derive(Debug, Clone, Copy)]
pub struct BendSpec {
    pub d: f64,
    /// `None`, or within `RED_BEND_MIN_DIFF_MM` of `d`: plain bend.
    pub d_end: Option<f64>,
    pub t: f64,
    pub r: f64,
    pub angle_deg: f64,
    /// Joint count as read from the drawing (any f64: 0, 2.5, NaN are refused). `None`: table fallback.
    pub joints: Option<f64>,
    pub s1: f64,
    pub s2: f64,
    /// Piece number (start piece = 1) made the only cone. Refused (never ignored) unless the bend is reducing and the
    /// number is in 2..=joints.
    pub taper_piece: Option<u32>,
    pub basis: Basis,
    pub swing: SwingRule,
    pub allow: f64,
    pub seam_from: SeamFrom,
    pub seam_deg: f64,
    pub split: u32,
}

impl BendSpec {
    /// A spec with the named-rule defaults (basis mid, swing = basis, allow 4 mm, seam 45 deg from the heel, one sheet).
    pub fn new(
        d: f64,
        t: f64,
        r: f64,
        angle_deg: f64,
        joints: Option<f64>,
        s1: f64,
        s2: f64,
    ) -> Self {
        BendSpec {
            d,
            d_end: None,
            t,
            r,
            angle_deg,
            joints,
            s1,
            s2,
            taper_piece: None,
            basis: Basis::Mid,
            swing: SwingRule::Basis,
            allow: ALLOW_PER_PIECE_MM,
            seam_from: SeamFrom::Heel,
            seam_deg: SEAM_FROM_HEEL_DEG,
            split: 1,
        }
    }
}

/// Fallback piece count for an angle when the drawing gives none. Above the table: refused (no count is invented).
pub fn pieces_for_angle(angle_deg: f64, table: &[(f64, u32)]) -> Result<u32, ShapeRefusal> {
    if !angle_deg.is_finite() || angle_deg <= 0.0 {
        return Err(refuse(
            "bend_angle_invalid",
            "angle",
            "the angle must be a positive finite number",
        ));
    }
    table
        .iter()
        .find(|(upper, _)| angle_deg <= *upper)
        .map(|&(_, n)| n)
        .ok_or_else(|| {
            refuse(
                "bend_joints_above_table",
                "joints",
                format!("angle {angle_deg} is above the fallback table and the drawing gives no joint count"),
            )
        })
}

fn fin(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(refuse(
            "bend_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ))
    }
}

fn pos(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    fin(name, v)?;
    if v <= 0.0 || v < MIN_MM {
        return Err(refuse(
            "bend_value_not_positive",
            name,
            format!("{name} must be greater than 0 (got {v})"),
        ));
    }
    if v > MAX_MM {
        return Err(refuse(
            "bend_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

fn nonneg(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    fin(name, v)?;
    if v < 0.0 {
        return Err(refuse(
            "bend_value_not_positive",
            name,
            format!("{name} must not be negative (got {v})"),
        ));
    }
    if v > MAX_MM {
        return Err(refuse(
            "bend_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

/// A joint count from the drawing: finite integer in 1..=MAX_BEND_JOINTS.
fn drawing_joints(v: f64) -> Result<u32, ShapeRefusal> {
    if !v.is_finite() || v.fract() != 0.0 || v < 1.0 || v > f64::from(MAX_BEND_JOINTS) {
        return Err(refuse(
            "bend_joints_invalid",
            "joints",
            format!("the joint count must be an integer from 1 to {MAX_BEND_JOINTS} (got {v})"),
        ));
    }
    Ok(v as u32)
}

/// The joint count: the drawing wins; the table (pieces - 1) is the fallback and is labelled.
pub fn resolve_joints(
    drawing: Option<f64>,
    angle_deg: f64,
) -> Result<(u32, JointSource), ShapeRefusal> {
    match drawing {
        Some(v) => Ok((drawing_joints(v)?, JointSource::Drawing)),
        None => {
            let n = pieces_for_angle(angle_deg, &JOINT_TABLE)?;
            Ok((n - 1, JointSource::TableRule))
        }
    }
}

// ---------------------------------------------------------------- closed-form model

#[derive(Debug, Clone, PartialEq)]
pub struct JointModel {
    /// 1-based joint number.
    pub v: u32,
    /// Angles (rad) between the cut plane normal and the axes of the piece before / after.
    pub ph_a: f64,
    pub ph_b: f64,
    /// Radius of the sphere at this joint (swing surface) and the plane's distance from the joint point.
    pub r: f64,
    pub d: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PieceModel {
    /// 1 = start piece, joints + 1 = end piece.
    pub m: u32,
    /// Cone tilt (rad); 0 for cylinders; negative when the bend widens.
    pub th: f64,
    pub r: f64,
    pub rho0: f64,
    pub zlo: f64,
    pub zhi: f64,
    pub a_lo: f64,
    pub a_hi: f64,
    pub axis: f64,
    /// Generatrix lengths without allowance at the top (outer), heel (inner) and side.
    pub ten: f64,
    pub chi: f64,
    pub yo: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BendModel {
    pub rho: f64,
    /// `R tan(rho)`: tangent length of a joint.
    pub tr: f64,
    /// Centre-to-centre distance of two joints, `2 tr`.
    pub l: f64,
    /// Radial offset from the outer radius to the swing surface.
    pub ao: f64,
    pub rv: Vec<f64>,
    pub pieces: Vec<PieceModel>,
    pub joints: Vec<JointModel>,
    /// Pieces whose heel generatrix (no allowance) is not positive: front and back cuts cross.
    pub cross: Vec<u32>,
    pub reducing: bool,
    pub joints_n: u32,
}

/// Validated, resolved input.
#[derive(Debug, Clone, Copy)]
struct Valid {
    j: u32,
    d_end: f64,
    reducing: bool,
    taper: Option<u32>,
    source: JointSource,
}

fn validate(s: &BendSpec) -> Result<Valid, ShapeRefusal> {
    let (j, source) = resolve_joints(
        s.joints,
        if s.angle_deg.is_finite() {
            s.angle_deg
        } else {
            0.0
        },
    )?;
    let d = pos("D", s.d)?;
    let t = pos("t", s.t)?;
    let de = match s.d_end {
        Some(x) => pos("D_end", x)?,
        None => d,
    };
    pos("R", s.r)?;
    fin("angle", s.angle_deg)?;
    if !(s.angle_deg > 0.0 && s.angle_deg < 180.0) {
        return Err(refuse(
            "bend_angle_invalid",
            "angle",
            format!(
                "the bend angle must be above 0 and below 180 deg (got {})",
                s.angle_deg
            ),
        ));
    }
    nonneg("start_straight", s.s1)?;
    nonneg("end_straight", s.s2)?;
    nonneg("allow", s.allow)?;
    fin("seam", s.seam_deg)?;
    if t >= d.min(de) / 2.0 {
        return Err(refuse(
            "bend_wall_too_thick",
            "t",
            "the wall must be thinner than the radius of both mouths",
        ));
    }
    if !(1..=MAX_BEND_SPLIT).contains(&s.split) {
        return Err(refuse(
            "bend_split_out_of_range",
            "split",
            format!("split must be 1..={MAX_BEND_SPLIT} (got {})", s.split),
        ));
    }
    let reducing = (de - d).abs() > RED_BEND_MIN_DIFF_MM;
    if reducing && j < 2 {
        return Err(refuse(
            "bend_reducing_needs_two_joints",
            "joints",
            "a reducing bend with one joint has no middle piece to make a cone",
        ));
    }
    if let Some(k) = s.taper_piece {
        if !reducing {
            return Err(refuse(
                "bend_taper_piece_not_reducing",
                "taper_piece",
                format!(
                    "taper_piece {k} is given but the bend is not reducing: remove it or give D_end"
                ),
            ));
        }
        if !(2..=j).contains(&k) {
            return Err(refuse(
                "bend_taper_piece_out_of_range",
                "taper_piece",
                format!("taper_piece must be 2..={j} for {j} joints (got {k})"),
            ));
        }
    }
    Ok(Valid {
        j,
        d_end: if reducing { de } else { d },
        reducing,
        taper: s.taper_piece,
        source,
    })
}

fn swing_offset(s: &BendSpec) -> f64 {
    match s.swing {
        SwingRule::Basis => s.basis.offset(s.t),
        SwingRule::Outer => 0.0,
    }
}

fn model_of(s: &BendSpec, v: &Valid) -> Result<BendModel, ShapeRefusal> {
    let ao = swing_offset(s);
    let j = v.j as usize;
    let rho = (s.angle_deg / f64::from(v.j) / 2.0).to_radians();
    let tr = s.r * rho.tan();
    if !tr.is_finite() || tr > MAX_MM {
        return Err(refuse(
            "bend_value_too_large",
            "angle",
            "the tangent length of a joint exceeds the limit",
        ));
    }
    let l = 2.0 * tr;
    let r1 = s.d / 2.0 - ao;
    let rn = v.d_end / 2.0 - ao;
    let rv: Vec<f64> = (1..=j)
        .map(|k| {
            if j < 2 {
                r1
            } else if let Some(tp) = v.taper {
                if k < tp as usize { r1 } else { rn }
            } else {
                r1 + (rn - r1) * (k as f64 - 1.0) / (j as f64 - 1.0)
            }
        })
        .collect();
    // pieces: th, r, rho0
    let mut ps: Vec<PieceModel> = Vec::with_capacity(j + 1);
    for m in 1..=j + 1 {
        let sn = if m > 1 && m <= j {
            (rv[m - 2] - rv[m - 1]) / l
        } else {
            0.0
        };
        if sn.abs() >= 1.0 || !sn.is_finite() {
            return Err(refuse(
                "bend_cone_too_steep",
                "D_end",
                format!(
                    "the cone of piece {m} is too steep: the mouth difference is too large for the piece length"
                ),
            ));
        }
        let th = sn.asin();
        let r = rv[if m == 1 {
            0
        } else if m == j + 1 {
            j - 1
        } else {
            m - 2
        }];
        ps.push(PieceModel {
            m: m as u32,
            th,
            r,
            rho0: r / th.cos(),
            zlo: 0.0,
            zhi: 0.0,
            a_lo: 0.0,
            a_hi: 0.0,
            axis: 0.0,
            ten: 0.0,
            chi: 0.0,
            yo: 0.0,
        });
    }
    let c2 = (2.0 * rho).cos();
    let s2 = (2.0 * rho).sin();
    let mut js = Vec::with_capacity(j);
    for k in 1..=j {
        let (a, b) = (&ps[k - 1], &ps[k]);
        let (ca, cb) = (a.th.cos(), b.th.cos());
        let ph_a = (s2 * ca).atan2(cb + ca * c2);
        let d = rv[k - 1] * (a.th.tan() + b.th.tan())
            / (1.0 / (ca * ca) + 1.0 / (cb * cb) + 2.0 * c2 / (ca * cb)).sqrt();
        js.push(JointModel {
            v: k as u32,
            ph_a,
            ph_b: 2.0 * rho - ph_a,
            r: rv[k - 1],
            d,
        });
    }
    let mut cross = Vec::new();
    for p in ps.iter_mut() {
        let m = p.m as usize;
        let lo = if m > 1 { Some(&js[m - 2]) } else { None };
        let hi = if m <= j { Some(&js[m - 1]) } else { None };
        p.zlo = match lo {
            None => -(s.s1 + tr),
            Some(x) => x.d / x.ph_b.cos(),
        };
        p.zhi = match hi {
            None => tr + s.s2,
            Some(x) => (if m == 1 { 0.0 } else { l }) + x.d / x.ph_a.cos(),
        };
        p.a_lo = lo.map_or(0.0, |x| x.ph_b);
        p.a_hi = hi.map_or(0.0, |x| x.ph_a);
        let tt = p.th.tan();
        if (tt * p.a_lo.tan()).abs() >= 1.0 || (tt * p.a_hi.tan()).abs() >= 1.0 {
            return Err(refuse(
                "bend_cut_degenerate",
                "angle",
                format!(
                    "the cut curve of piece {m} is degenerate (tilt plus cut angle reach 90 deg)"
                ),
            ));
        }
        p.axis = p.zhi - p.zlo;
        let pc = p.clone();
        let gl = |psi: f64| (cut_z(&pc, psi, true) - cut_z(&pc, psi, false)) / pc.th.cos();
        p.ten = gl(0.0);
        p.chi = gl(PI);
        p.yo = gl(PI / 2.0);
        if p.chi.is_nan() || p.chi <= 0.0 {
            cross.push(p.m);
        }
    }
    Ok(BendModel {
        rho,
        tr,
        l,
        ao,
        rv,
        pieces: ps,
        joints: js,
        cross,
        reducing: v.reducing,
        joints_n: v.j,
    })
}

/// Axial position of the cut curve of a piece on the swing surface at azimuth `psi` (0 = outer generatrix).
/// `up`: the end (back) cut; otherwise the start (front) cut.
pub fn cut_z(p: &PieceModel, psi: f64, up: bool) -> f64 {
    let c = psi.cos();
    let tt = p.th.tan();
    if up {
        let a = p.a_hi.tan();
        (p.zhi + p.rho0 * a * c) / (1.0 + tt * a * c)
    } else {
        let b = p.a_lo.tan();
        (p.zlo - p.rho0 * b * c) / (1.0 - tt * b * c)
    }
}

/// The closed-form model of a bend. Pieces whose heel generatrix is not positive are listed in `cross` (the model is
/// still returned so the lengths can be inspected); `bend()` refuses them.
pub fn model(s: &BendSpec) -> Result<BendModel, ShapeRefusal> {
    let v = validate(s)?;
    model_of(s, &v)
}

// ---------------------------------------------------------------- developed sheets

#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// Bounding box of the developed outline (with allowance) and its area (with allowance).
    pub w: f64,
    pub h: f64,
    pub area: f64,
    pub area_err: f64,
    /// Straight edges at the two ends of the band (the generatrices cut at the seams).
    pub left_edge: f64,
    pub right_edge: f64,
    /// Azimuth (deg from the top) of the band's first edge.
    pub start_deg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PieceSheet {
    pub m: u32,
    pub cone: bool,
    pub th_deg: f64,
    /// Axis length without allowance, and centre length = axis + allow.
    pub axis: f64,
    pub center: f64,
    /// Generatrix lengths with allowance.
    pub ten: f64,
    pub chi: f64,
    pub yo: f64,
    /// Mouth outer diameters (perpendicular to the axis) at the start and end, and at mid-length of a middle piece.
    pub d_lo: f64,
    pub d_hi: f64,
    pub d_mid: Option<f64>,
    pub fan_deg: Option<f64>,
    pub seam_len: f64,
    pub bands: Vec<Band>,
    /// Net area (no allowance), net mass and the quadrature estimate.
    pub net_area: f64,
    pub net_mass_kg: f64,
    pub area_err: f64,
    /// Mass with allowance (sum of band areas).
    pub mass_with_allow_kg: f64,
    pub extent_refine_gain: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BendResult {
    pub model: BendModel,
    pub pieces: Vec<PieceSheet>,
    pub total_net_mass_kg: f64,
    pub total_axis: f64,
    pub joints_source: JointSource,
    /// Machine-readable: present when the joint count came from the table.
    pub question: Option<&'static str>,
    /// Mouth difference or tilt note: the largest cone tilt (deg) when it exceeds `TILT_NOTE_DEG`.
    pub tilt_note_deg: Option<f64>,
    /// Always false: area and extents are numerical with estimates, not interval-certified.
    pub certified: bool,
    pub method: &'static str,
    /// (rule name, applied value, status) for every named rule this result depends on.
    pub rules: Vec<(&'static str, String, &'static str)>,
}

struct Ctx<'a> {
    p: &'a PieceModel,
    rb0: f64,
    tt: f64,
    s: f64,
    c: f64,
    cone: bool,
    za: f64,
}

impl Ctx<'_> {
    fn rb(&self, z: f64) -> f64 {
        self.rb0 - z * self.tt
    }
    /// Point of the developed sheet for axial position `z` at azimuth `psi` of a band spanning from `a0` (mid `am`).
    fn xy(&self, z: f64, psi: f64, a0: f64, am: f64) -> [f64; 2] {
        let rb = self.rb(z);
        if !self.cone {
            return [rb * (psi - a0), z - self.p.zlo];
        }
        let sa = self.s.abs();
        let ell = rb / sa;
        let q = (psi - am) * sa;
        let sg = if self.p.th > 0.0 { 1.0 } else { -1.0 };
        [
            ell * q.sin(),
            (self.za - self.p.zlo) / self.c - sg * ell * q.cos(),
        ]
    }
    fn curve(&self, up: bool, h: f64, psi: f64, a0: f64, am: f64) -> [f64; 2] {
        let z = cut_z(self.p, psi, up);
        let z = if up { z + h * self.c } else { z - h * self.c };
        self.xy(z, psi, a0, am)
    }
}

fn golden_max(f: &dyn Fn(f64) -> f64, lo: f64, hi: f64) -> f64 {
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (lo, hi);
    let (mut x1, mut x2) = (b - g * (b - a), a + g * (b - a));
    let (mut f1, mut f2) = (f(x1), f(x2));
    for _ in 0..80 {
        if f1 < f2 {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = f(x2);
        } else {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = f(x1);
        }
    }
    f1.max(f2)
}

/// Extremes (xmin, xmax, ymin, ymax) of the outline of one band, and the largest amount refinement moved one of them.
fn extents(cx: &Ctx, h: f64, a0: f64, a1: f64) -> ([f64; 4], f64) {
    let am = 0.5 * (a0 + a1);
    let n = EXTENT_SAMPLES;
    let step = (a1 - a0) / n as f64;
    let at = |up: bool, i: usize| cx.curve(up, h, a0 + step * i as f64, a0, am);
    let mut ext = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut arg = [(0usize, false); 4];
    for &up in &[false, true] {
        for i in 0..=n {
            let p = at(up, i);
            if p[0] < ext[0] {
                ext[0] = p[0];
                arg[0] = (i, up);
            }
            if p[0] > ext[1] {
                ext[1] = p[0];
                arg[1] = (i, up);
            }
            if p[1] < ext[2] {
                ext[2] = p[1];
                arg[2] = (i, up);
            }
            if p[1] > ext[3] {
                ext[3] = p[1];
                arg[3] = (i, up);
            }
        }
    }
    let mut gain = 0.0f64;
    for k in 0..4 {
        let (i, up) = arg[k];
        let lo = a0 + step * (i.saturating_sub(1)) as f64;
        let hi = (a0 + step * (i + 1) as f64).min(a1);
        let sign = if k % 2 == 0 { -1.0 } else { 1.0 };
        let axis = k / 2;
        let f = |psi: f64| sign * cx.curve(up, h, psi, a0, am)[axis];
        let v = sign * golden_max(&f, lo, hi);
        let better = if sign > 0.0 { v > ext[k] } else { v < ext[k] };
        if better {
            gain = gain.max((v - ext[k]).abs());
            ext[k] = v;
        }
    }
    (ext, gain)
}

fn band_area(cx: &Ctx, h: f64, a0: f64, a1: f64) -> (f64, f64) {
    // |dA/dpsi| = (zhi' - zlo') (rho(zhi') + rho(zlo')) / (2 cos th): mean radius times slant length, exact for cone
    // and cylinder alike; no cancellation as the tilt tends to 0.
    let f = |psi: f64| {
        let zh = cut_z(cx.p, psi, true) + h * cx.c;
        let zl = cut_z(cx.p, psi, false) - h * cx.c;
        (zh - zl) * (cx.rb(zh) + cx.rb(zl)) / (2.0 * cx.c)
    };
    adaptive_simpson(&f, a0, a1)
}

fn make_sheet(s: &BendSpec, m: &BendModel, p: &PieceModel) -> Result<PieceSheet, ShapeRefusal> {
    let o = s.basis.offset(s.t);
    let h = s.allow / 2.0;
    let sn = p.th.sin();
    let cn = p.th.cos();
    let tt = p.th.tan();
    let cone = sn.abs() > 1e-15;
    let rb0 = p.rho0 - (o - m.ao);
    let ph = (match s.seam_from {
        SeamFrom::Top => s.seam_deg,
        SeamFrom::Heel => 180.0 - s.seam_deg,
    })
    .to_radians();
    let cx = Ctx {
        p,
        rb0,
        tt,
        s: sn,
        c: cn,
        cone,
        za: if cone { rb0 / tt } else { 0.0 },
    };
    let jn = m.joints_n;
    let d_at = |z: f64| 2.0 * (p.rho0 - z * tt + m.ao);
    let n = s.split;
    let mut bands = Vec::with_capacity(n as usize);
    let mut gain = 0.0f64;
    let mut area_err = 0.0f64;
    let mut area_sum = 0.0;
    for k in 0..n {
        let a0 = ph + 2.0 * PI * f64::from(k) / f64::from(n);
        let a1 = ph + 2.0 * PI * f64::from(k + 1) / f64::from(n);
        let am = 0.5 * (a0 + a1);
        let (ex, g) = extents(&cx, h, a0, a1);
        gain = gain.max(g);
        let (area, err) = band_area(&cx, h, a0, a1);
        if !area.is_finite() || err > AREA_REL_TOL * area.abs() {
            return Err(refuse(
                "bend_area_not_converged",
                "angle",
                "the developed area did not converge to its stated tolerance",
            ));
        }
        let dist = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        bands.push(Band {
            w: ex[1] - ex[0],
            h: ex[3] - ex[2],
            area,
            area_err: err,
            left_edge: dist(
                cx.curve(true, h, a0, a0, am),
                cx.curve(false, h, a0, a0, am),
            ),
            right_edge: dist(
                cx.curve(true, h, a1, a0, am),
                cx.curve(false, h, a1, a0, am),
            ),
            start_deg: (a0.to_degrees()).rem_euclid(360.0),
        });
        area_sum += area;
        area_err += err;
    }
    // net area (no allowance) of the whole piece in one sheet
    let (net, net_err) = band_area(&cx, 0.0, ph, ph + 2.0 * PI);
    if !net.is_finite() || net_err > AREA_REL_TOL * net.abs() {
        return Err(refuse(
            "bend_area_not_converged",
            "angle",
            "the net developed area did not converge to its stated tolerance",
        ));
    }
    let seam_len = (cut_z(p, ph, true) - cut_z(p, ph, false)) / cn + s.allow;
    let middle = p.m > 1 && p.m <= jn;
    let sheet = PieceSheet {
        m: p.m,
        cone,
        th_deg: p.th.to_degrees(),
        axis: p.axis,
        center: p.axis + s.allow,
        ten: p.ten + s.allow,
        chi: p.chi + s.allow,
        yo: p.yo + s.allow,
        d_lo: d_at(p.zlo),
        d_hi: d_at(p.zhi),
        d_mid: if middle { Some(d_at(m.l / 2.0)) } else { None },
        fan_deg: if cone { Some(360.0 * sn.abs()) } else { None },
        seam_len,
        bands,
        net_area: net,
        net_mass_kg: mass_kg(net, s.t),
        area_err: net_err.max(area_err),
        mass_with_allow_kg: mass_kg(area_sum, s.t),
        extent_refine_gain: gain,
    };
    Ok(sheet)
}

/// The mitered bend (plain or reducing): model, cut curves and developed sheets of every piece. Refuses (never panics,
/// never returns NaN) with a `bend_*` code.
pub fn bend(s: &BendSpec) -> Result<BendResult, ShapeRefusal> {
    let v = validate(s)?;
    let m = model_of(s, &v)?;
    if !m.cross.is_empty() {
        let list: Vec<String> = m
            .cross
            .iter()
            .map(|k| format!("piece {k} ({:.1} mm)", m.pieces[*k as usize - 1].chi))
            .collect();
        return Err(refuse(
            "bend_cuts_cross_at_heel",
            "R",
            format!(
                "the front and back cuts cross at the heel: {}; the bend radius is small against the diameter or there are too many joints",
                list.join(", ")
            ),
        ));
    }
    let mut pieces = Vec::with_capacity(m.pieces.len());
    for p in &m.pieces {
        pieces.push(make_sheet(s, &m, p)?);
    }
    let total_net_mass_kg: f64 = pieces.iter().map(|p| p.net_mass_kg).sum();
    let total_axis: f64 = pieces.iter().map(|p| p.axis).sum();
    let tilt = pieces.iter().map(|p| p.th_deg.abs()).fold(0.0, f64::max);
    let finite = total_net_mass_kg.is_finite()
        && total_axis.is_finite()
        && pieces.iter().all(|p| {
            [
                p.axis, p.ten, p.chi, p.yo, p.d_lo, p.d_hi, p.seam_len, p.net_area,
            ]
            .iter()
            .all(|x| x.is_finite())
                && p.bands.iter().all(|b| {
                    [b.w, b.h, b.area, b.left_edge, b.right_edge]
                        .iter()
                        .all(|x| x.is_finite())
                })
        });
    if !finite {
        return Err(refuse(
            "bend_result_not_finite",
            "R",
            "a computed length or area is not finite",
        ));
    }
    let mut rules = vec![
        (
            "bend_swing_radius",
            match s.swing {
                SwingRule::Basis => "basis".to_string(),
                SwingRule::Outer => "outer".to_string(),
            },
            "UNVERIFIED",
        ),
        ("ALLOW_PER_PIECE_MM", format!("{}", s.allow), "UNVERIFIED"),
        (
            "SEAM_FROM_HEEL_DEG",
            format!(
                "{} deg from the {}",
                s.seam_deg,
                if s.seam_from == SeamFrom::Heel {
                    "heel"
                } else {
                    "top"
                }
            ),
            "UNVERIFIED",
        ),
    ];
    if v.reducing {
        rules.push((
            "REDUCING_CONSTRUCTION",
            match v.taper {
                Some(k) => format!("only piece {k} is a cone"),
                None => "all middle pieces one cone".to_string(),
            },
            "UNVERIFIED",
        ));
    }
    if v.source == JointSource::TableRule {
        rules.push(("JOINT_TABLE", format!("{} joints", v.j), "UNVERIFIED"));
    }
    Ok(BendResult {
        model: m,
        pieces,
        total_net_mass_kg,
        total_axis,
        joints_source: v.source,
        question: (v.source == JointSource::TableRule).then_some(JOINTS_QUESTION),
        tilt_note_deg: (tilt > TILT_NOTE_DEG).then_some(tilt),
        certified: false,
        method: BEND_METHOD,
        rules,
    })
}
