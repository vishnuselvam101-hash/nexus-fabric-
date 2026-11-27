//! NFJ-1 S1-L9a: a round branch standing on a cone (concentric or eccentric reducer / bellmouth), perpendicular to the
//! cone axis: the branch cut, the hole in the developed cone, the position of the longitudinal seams, the cone pieces.
//! Pure functions, no I/O, no engine state.
//!
//! Status (stated, not hidden): NOT reachable from `job::run`; nothing here can be served as a cut file;
//! `ENGINE_VERSION` is unchanged. Every result is `certified: false` with its `method`, named `rules` (all UNVERIFIED,
//! none confirmed by a factory) and the `holds` the caller must turn into a withheld DXF.
//!
//! Geometry (ZK `cbSetup` ... `coneBranchPieces`, re-derived; tests/conebranch_closed_forms.rs is independent of it):
//! cone axis `x`, the mouth `Da` at `x = 0`, the mouth `Db` at `x = L`; every section `x = const` is a circle of outer
//! radius `Ro(x) = Ra + k x` with centre `C(x) = cs x (cos f, sin f)` (cs = (Ra - Rb)/L for an eccentric cone, the flat
//! side at azimuth `f` is the tangent line `x`-parallel; cs = 0 for a concentric one). The branch axis is perpendicular to
//! the cone axis, at azimuth `dir`, offset `e` (positive towards increasing azimuth), crossing the cone axis plane at
//! `x0`. A wall line of the branch (radius `rho`, angle `th`, `th = 0` towards `+x`) stays in the plane `x = x0 +
//! rho cos(th)` and meets the section circle there: the contact is closed form.
//!
//! The cone plate thickness is taken radially (a section circle of radius `Ro - d`), NOT along the surface normal: rule
//! `CONEBRANCH_THICKNESS_RADIAL`, UNVERIFIED (ZK spec 6). The normal thickness of a plate that is `T` radially is
//! `T cos(half angle)`; the effect is quantified in tests/conebranch_closed_forms.rs and is below 0.5 mm for the
//! reference examples, 1.6 mm at 30 deg half angle.
//!
//! Numerics: the eccentric development angle `psi(a)` is the arc length of the unit generator direction on the unit
//! sphere, integrated by adaptive Simpson into a table (error estimate returned, a result above `FAN_REL_TOL` is
//! refused). Branch area and hole area are adaptive Simpson with an a-posteriori estimate. Hole and cut polylines are
//! sampled; `eps_rep` is an ESTIMATE of the polyline-to-curve distance, not a bound.

use crate::shapes::{Basis, ConeSpec, ShapeRefusal, adaptive_simpson, cone_split, mass_kg, refuse};
use crate::tee::{Mount, mod_p, shoelace};
use std::f64::consts::PI;

pub const CONEBRANCH_METHOD: &str = "closed_form_contact_curves+analytic_interval_hole+sphere_arclength_fan_table+adaptive_simpson_areas+searched_seams; sampled polylines; certified:false";
/// Polyline sizes (ZK parity) and marking points round the branch.
pub const CB_CUT_SAMPLES: usize = 720;
pub const CB_HOLE_SAMPLES: usize = 720;
pub const CB_WELD_SAMPLES: usize = 720;
pub const CB_MARKS: usize = 16;
/// Cells of the development-angle table (eccentric cone) and the relative error above which it is refused.
pub const FAN_CELLS: usize = 4096;
pub const FAN_REL_TOL: f64 = 1e-9;
/// Relative acceptance of the area quadrature estimates.
pub const CB_AREA_REL_TOL: f64 = 1e-9;
/// The hole-area integrand has kinks where the active root of the hole interval changes: a looser acceptance.
pub const CB_HOLE_AREA_REL_TOL: f64 = 1e-6;
/// RULE `CONEBRANCH_SEAM_GAP_T`, UNVERIFIED (ZK parity): clear distance seam to hole / weld line, in cone plate thicknesses.
pub const CB_SEAM_GAP_T: f64 = 5.0;
/// Pieces the cone is cut into when the caller states none (named rule).
pub const CB_DEFAULT_SPLIT: u32 = 2;
pub const CB_MAX_SPLIT: u32 = 12;
pub const CB_X_QUESTION: &str =
    "branch position along the cone not on the drawing: placed at half the length, confirm";
pub const CB_FLAT_QUESTION: &str = "flat side of the eccentric cone not on the drawing: taken as the top (0 deg), confirm; the hole and the cut depend on it";

const MAX_MM: f64 = crate::shapes::MAX_MM;
const MIN_MM: f64 = crate::shapes::MIN_MM;

/// (name, value, kind, status)
pub const CONEBRANCH_NAMED_RULES: [(&str, &str, &str, &str); 11] = [
    (
        "CONEBRANCH_THICKNESS_RADIAL",
        "the cone plate thickness is taken radially (section circle Ro - d), not along the surface normal; the developed surface is the radial mid surface Ra - T/2; the branch contact surface is the outer (set-on) or radial-T inner (set-in) circle",
        "rule (basis)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_HOLE_NORMAL_CUT",
        "the hole in the cone is cut square to the plate (along the surface normal) so that the branch bore (set-on: inner radius, set-in: outer radius) touches the plate nowhere across its thickness; it reaches (T/2) sin(half angle) further than a hole cut at the inner surface only",
        "rule (hole construction)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_BRANCH_CUT_SURFACE",
        "branch cut at the highest point where the branch wall meets the cone outer (set-on) or radial-T inner (set-in) surface",
        "rule (cut construction)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_MOUNT_DEFAULT",
        "set-on (branch rests on the cone)",
        "rule (joint type; a drawing note wins)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_SEAM_RULE",
        "n seams at equal azimuth steps from the top (concentric) or from the flat side (eccentric); when a seam would cross the hole or come closer than the gap to the hole or the weld line, all seams are turned together so that the middle of the hole/weld azimuth span is the middle of one piece",
        "rule (seam position)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_SEAM_GAP_T",
        "5 x cone plate thickness (clear distance seam to hole edge and to weld line, measured in the development)",
        "rule (clear distance)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_BRANCH_SEAM_SIDE",
        "the branch longitudinal seam lies on the side (th = 90 deg); mark 4 faces the x = 0 mouth (concentric: the Da mouth; eccentric: always the larger mouth), mark 12 the other mouth",
        "rule (branch seam, marking)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_H_FROM_AXIS",
        "H is measured along the branch axis from the cone axis (concentric) or from the Da mouth centre line (eccentric) to the branch end",
        "rule (dimension reading)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_E_SIGN",
        "offset positive towards increasing azimuth (azimuth seen from the Da mouth: 0 top, 90 right); a sign without a side is resolved by the caller (tee::signed_offset), never abs()'d here",
        "rule (sign)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_X_FLAT_DEFAULT",
        "branch position missing: half the cone length, with a question; eccentric flat side missing: the top (0 deg), with a question and a hold",
        "rule (fallback)",
        "UNVERIFIED",
    ),
    (
        "CONEBRANCH_LIMITS",
        "2..=12 pieces, one branch, perpendicular branch only, 720 point polylines",
        "implementation limit",
        "UNVERIFIED",
    ),
];

pub const CONEBRANCH_REFUSAL_CODES: [&str; 16] = [
    "conebranch_value_not_finite",
    "conebranch_value_not_positive",
    "conebranch_value_too_large",
    "conebranch_not_conical",
    "conebranch_wall_too_thick",
    "conebranch_branch_not_perpendicular",
    "conebranch_split_out_of_range",
    "conebranch_branch_not_within_cone",
    "conebranch_hole_not_through_plate",
    "conebranch_hole_beyond_mouth",
    "conebranch_branch_end_below_cut",
    "conebranch_cut_not_found",
    "conebranch_fan_not_converged",
    "conebranch_area_not_converged",
    "conebranch_seam_not_placeable",
    "conebranch_result_not_finite",
];

// ---------------------------------------------------------------- spec and result

#[derive(Debug, Clone, PartialEq)]
pub struct ConeBranchSpec {
    /// Outer diameters at the two mouths: `da` is the mouth the branch position `x` is measured from.
    pub da: f64,
    pub db: f64,
    /// Axial length and plate thickness of the cone.
    pub l: f64,
    pub t: f64,
    /// Eccentric cone (two parallel circles with a common tangent line, the flat side).
    pub ecc: bool,
    /// Azimuth (deg, seen from the `da` mouth, 0 top, 90 right) of the flat side. `None` for an eccentric cone:
    /// taken as 0 with a question and the hold `conebranch_flat_side_unconfirmed`. Ignored (not dropped: must be `None`
    /// or any finite value) for a concentric cone.
    pub flat_deg: Option<f64>,
    /// Branch outer diameter and wall.
    pub d: f64,
    pub tb: f64,
    /// Distance from the `da` mouth to the branch centre line. `None`: half the length (question).
    pub x: Option<f64>,
    pub dir_deg: f64,
    /// Signed offset of the branch axis, positive towards increasing azimuth.
    pub e: f64,
    /// Branch length from the reference line to the branch end.
    pub h: f64,
    /// Sharp angle between the axes; `None` or 90 only (a branch that is not perpendicular is refused).
    pub angle_deg: Option<f64>,
    pub mount: Mount,
    /// Pieces the cone is cut into (2..=12); `None`: 2.
    pub split: Option<u32>,
    /// Clear distance seam to hole (mm); `None`: `CB_SEAM_GAP_T` x t.
    pub seam_gap: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CbMark {
    pub i: usize,
    pub u: f64,
    pub len: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CbBranchPiece {
    /// Circumference at the branch mid surface.
    pub circ: f64,
    /// 17 marks (`i = 0..=16`), mark 4 faces the `x = 0` mouth (see `d4`).
    pub marks: Vec<CbMark>,
    /// Drawn cut line (u, length), `CB_CUT_SAMPLES + 1` points.
    pub cut: Vec<[f64; 2]>,
    pub cut_length: f64,
    pub shortest: f64,
    pub longest: f64,
    pub net_area: f64,
    pub area_err: f64,
    pub mass_kg: f64,
    /// Outer diameter of the mouth that mark 4 / mark 12 face.
    pub d4: f64,
    pub d12: f64,
    /// Azimuth (deg, Da view) the seam lies at.
    pub seam_az_deg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CbHoleMark {
    pub i: usize,
    pub x: f64,
    pub y: f64,
    /// Axial position from the `da` mouth and azimuth (deg, Da view) of the mark.
    pub ax: f64,
    pub az_deg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CbConePiece {
    pub label: String,
    pub outline: Vec<[f64; 2]>,
    /// Hole polygon in the piece frame (the first piece only).
    pub hole: Option<Vec<[f64; 2]>>,
    pub copies: u32,
    pub w: f64,
    pub h: f64,
    /// Piece area with the hole subtracted, per copy, and its mass.
    pub net_area: f64,
    pub mass_kg: f64,
    /// Closed-form chord and height of a concentric piece (bit-identical to `shapes::cone_split`).
    pub chord: Option<f64>,
    pub height: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CbHold {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CbHoleInfo {
    pub x_min: f64,
    pub x_max: f64,
    /// Azimuth span (deg) and its start (deg, Da view, 0..360).
    pub az_start_deg: f64,
    pub az_span_deg: f64,
    pub bbox: [f64; 4],
    pub box_w: f64,
    pub box_h: f64,
    /// Area of the hole polygon (ZK parity) and of the exact hole (adaptive quadrature, estimate).
    pub area_poly: f64,
    pub area: f64,
    pub area_err: f64,
    /// Axial extent of the hole when cut at the inner surface only (rule alternative), for the record.
    pub len_inner_only: Option<f64>,
    pub len_normal_cut: f64,
    pub marks: Vec<CbHoleMark>,
    pub perimeter: f64,
    /// ESTIMATE (not a bound) of the distance between the polygon and the exact hole outline.
    pub eps_rep: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CbSeamInfo {
    /// Seam azimuths (deg, Da view) after the rule / the turn, in piece order.
    pub deg: Vec<f64>,
    pub rule_deg: Vec<f64>,
    /// The rule seams hit the hole or the weld gap (so the seams were turned).
    pub hit: bool,
    /// Turned angle (deg, Da view, in (-180/n, 180/n]).
    pub moved_deg: f64,
    /// Seams still cross or come closer than the gap after the turn.
    pub cross: bool,
    pub gap: f64,
    /// Smallest distance seam to hole edge and seam to weld line, in the development (after the turn).
    pub seam_hole: f64,
    pub seam_weld: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConeBranchResult {
    pub branch: CbBranchPiece,
    pub hole: CbHoleInfo,
    pub seams: CbSeamInfo,
    pub pieces: Vec<CbConePiece>,
    pub n: u32,
    /// Branch position used (from the `da` mouth): the stated one, or half the length when none was stated.
    pub x_used: f64,
    /// Developed fan: apex distance of the large and small mouth (mid surface) and the full development angle (rad).
    pub fan_total_angle: f64,
    pub fan_err: f64,
    pub cone_area: f64,
    pub cone_area_err: f64,
    /// Whole cone mass with the hole subtracted.
    pub cone_mass_kg: f64,
    pub holds: Vec<CbHold>,
    pub questions: Vec<&'static str>,
    pub certified: bool,
    pub method: &'static str,
    pub rules: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

impl ConeBranchResult {
    pub fn has_hold(&self, code: &str) -> bool {
        self.holds.iter().any(|h| h.code == code)
    }
}

// ---------------------------------------------------------------- validation

fn num(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "conebranch_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v.abs() > MAX_MM {
        return Err(refuse(
            "conebranch_value_too_large",
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
            "conebranch_value_not_positive",
            name,
            format!("{name} must be greater than 0 (got {v})"),
        ));
    }
    Ok(v)
}

fn ang(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "conebranch_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v.abs() > 1e6 {
        return Err(refuse(
            "conebranch_value_too_large",
            name,
            format!("{name} exceeds 1e6 deg (got {v})"),
        ));
    }
    Ok(v)
}

fn wrap_a(a: f64, c: f64) -> f64 {
    a - 2.0 * PI * ((a - c + PI) / (2.0 * PI)).floor()
}

// ---------------------------------------------------------------- internal model (ZK cbSetup)

/// Internal frame: for an eccentric cone written from the smaller mouth the model is mirrored so that the larger mouth
/// is at `x = 0` and the flat side at azimuth 0 (ZK `cbSetup` `flip`); `cb_az` / `cb_x` map back.
#[derive(Debug, Clone)]
struct Model {
    ra: f64,
    rb: f64,
    l: f64,
    t: f64,
    k: f64,
    cs: f64,
    ecc: bool,
    flip: bool,
    f: f64,
    ro: f64,
    ri: f64,
    rm: f64,
    x0: f64,
    dir: f64,
    e: f64,
    h: f64,
    rh: f64,
    dc: f64,
    tb: f64,
}

fn setup(s: &ConeBranchSpec) -> Result<(Model, Vec<&'static str>), ShapeRefusal> {
    let da = dim("da", s.da)?;
    let db = dim("db", s.db)?;
    let l = dim("l", s.l)?;
    let t = dim("t", s.t)?;
    let d = dim("d", s.d)?;
    let tb = dim("tb", s.tb)?;
    num("e", s.e)?;
    num("h", s.h)?;
    ang("dir_deg", s.dir_deg)?;
    let mut questions = Vec::new();
    let x = match s.x {
        Some(x) => num("x", x)?,
        None => {
            questions.push(CB_X_QUESTION);
            l / 2.0
        }
    };
    let flat = match (s.ecc, s.flat_deg) {
        (_, Some(f)) => ang("flat_deg", f)?,
        (true, None) => {
            questions.push(CB_FLAT_QUESTION);
            0.0
        }
        (false, None) => 0.0,
    };
    if let Some(a) = s.angle_deg {
        ang("angle_deg", a)?;
        if (a - 90.0).abs() > 1e-9 {
            return Err(refuse(
                "conebranch_branch_not_perpendicular",
                "angle_deg",
                format!(
                    "a branch on a cone must be perpendicular to the cone axis (got {a} deg); an oblique branch is not built"
                ),
            ));
        }
    }
    if let Some(g) = s.seam_gap {
        num("seam_gap", g)?;
    }
    let (ra0, rb0) = (da / 2.0, db / 2.0);
    if (rb0 - ra0).abs() / l <= 1e-9 {
        return Err(refuse(
            "conebranch_not_conical",
            "db",
            "da and db are equal: this is a cylinder, not a cone",
        ));
    }
    if t >= ra0.min(rb0) {
        return Err(refuse(
            "conebranch_wall_too_thick",
            "t",
            "the cone plate is as thick as the radius of one mouth",
        ));
    }
    let ro = d / 2.0;
    if tb >= ro {
        return Err(refuse(
            "conebranch_wall_too_thick",
            "tb",
            "the branch wall is as thick as the branch radius",
        ));
    }
    let ecc = s.ecc;
    let flip = ecc && da < db;
    let f = if ecc { flat * PI / 180.0 } else { 0.0 };
    let mut dr = s.dir_deg * PI / 180.0 - f;
    let mut e = s.e;
    let mut x0 = x;
    let mut h = s.h;
    let dd = (da - db) / 2.0;
    if flip {
        e = -(e + dd * dr.sin());
        h -= dd * dr.cos();
        x0 = l - x0;
        dr = -dr;
    }
    let (ra, rb) = if flip { (rb0, ra0) } else { (ra0, rb0) };
    let inn = s.mount == Mount::SetIn;
    let m = Model {
        ra,
        rb,
        l,
        t,
        k: (rb - ra) / l,
        cs: if ecc { (ra - rb) / l } else { 0.0 },
        ecc,
        flip,
        f,
        ro,
        ri: ro - tb,
        rm: ro - tb / 2.0,
        x0,
        dir: dr,
        e,
        h,
        rh: if inn { ro } else { ro - tb },
        dc: if inn { t } else { 0.0 },
        tb,
    };
    Ok((m, questions))
}

impl Model {
    fn az(&self, a: f64) -> f64 {
        if self.flip { self.f - a } else { self.f + a }
    }
    fn ax(&self, x: f64) -> f64 {
        if self.flip { self.l - x } else { x }
    }
    /// Axial position `x0 + rho cos(th)` of a wall line and its lateral offset.
    fn hit(&self, rho: f64, th: f64, d: f64) -> Option<f64> {
        let x = self.x0 + rho * th.cos();
        let l = self.e + rho * th.sin();
        let cx = self.cs * x;
        let r = self.ra + self.k * x - d;
        let cv = -cx * self.dir.sin();
        let g = r * r - (l - cv) * (l - cv);
        if g < 0.0 {
            None
        } else {
            Some(cx * self.dir.cos() + g.sqrt())
        }
    }

    /// Highest contact of the branch wall (`ri..=ro`) with the section circle at depth `d`, closed form: candidates
    /// are the wall ends, the zeros of `g` and the stationary points of `t(rho) = a0 + a1 rho + sqrt(g)`.
    fn cut(&self, th: f64, d: f64) -> Option<f64> {
        let (c, s) = (th.cos(), th.sin());
        let (sf, cf) = (-self.dir.sin(), self.dir.cos());
        let a0 = self.cs * cf * self.x0;
        let a1 = self.cs * cf * c;
        let w0 = self.e - self.cs * sf * self.x0;
        let w1 = s - self.cs * sf * c;
        let q0 = self.ra + self.k * self.x0 - d;
        let q1 = self.k * c;
        let g0 = q0 * q0 - w0 * w0;
        let g1 = 2.0 * (q0 * q1 - w0 * w1);
        let g2 = q1 * q1 - w1 * w1;
        let (lo, hi) = (self.ri, self.ro);
        let roots = |a2: f64, a1_: f64, a0_: f64| -> Vec<f64> {
            if a2.abs() < 1e-18 {
                return if a1_.abs() < 1e-18 {
                    vec![]
                } else {
                    vec![-a0_ / a1_]
                };
            }
            let dd = a1_ * a1_ - 4.0 * a2 * a0_;
            if dd < 0.0 {
                return vec![];
            }
            let q = dd.sqrt();
            vec![(-a1_ - q) / (2.0 * a2), (-a1_ + q) / (2.0 * a2)]
        };
        let mut cand = vec![lo, hi];
        cand.extend(roots(g2, g1, g0));
        if a1.abs() < 1e-15 {
            if g2.abs() > 1e-18 {
                cand.push(-g1 / (2.0 * g2));
            }
        } else {
            for r in roots(
                4.0 * g2 * g2 - 4.0 * a1 * a1 * g2,
                4.0 * g1 * g2 - 4.0 * a1 * a1 * g1,
                g1 * g1 - 4.0 * a1 * a1 * g0,
            ) {
                if (g1 + 2.0 * g2 * r) * a1 <= 1e-12 {
                    cand.push(r);
                }
            }
        }
        let mut best: Option<f64> = None;
        for r in cand {
            if !r.is_finite() || r < lo - 1e-12 || r > hi + 1e-12 {
                continue;
            }
            let r = r.clamp(lo, hi);
            let gv = g0 + g1 * r + g2 * r * r;
            if gv < -1e-7 {
                continue;
            }
            let tv = a0 + a1 * r + gv.max(0.0).sqrt();
            best = Some(best.map_or(tv, |b: f64| b.max(tv)));
        }
        best
    }

    /// Axial interval `[lo, hi]` (development surface `x`) of the hole at azimuth `a` (internal frame): the set of `x`
    /// for which some point of the plate (along the surface normal, `|w| <= T/2`) lies inside the bore.
    fn iv(&self, a: f64) -> Option<[f64; 2]> {
        let s_ = (a - self.dir).sin();
        let kap = self.k + self.cs * a.cos();
        let nn = (1.0 + kap * kap).sqrt();
        let p1 = -self.cs * self.dir.sin() + self.k * s_;
        let v0 = (self.ra - self.t / 2.0) * s_;
        let (rh, t) = (self.rh, self.t);
        let mut los: Vec<f64> = vec![];
        let mut his: Vec<f64> = vec![];
        for w in [-t / 2.0, t / 2.0] {
            let cx = self.x0 + w * kap / nn;
            let q = v0 + w * s_ / nn - self.e;
            let a2 = 1.0 + p1 * p1;
            let a1 = 2.0 * (-cx + p1 * q);
            let a0 = cx * cx + q * q - rh * rh;
            let d = a1 * a1 - 4.0 * a2 * a0;
            if d < 0.0 {
                continue;
            }
            let r = d.sqrt();
            los.push((-a1 - r) / (2.0 * a2));
            his.push((-a1 + r) / (2.0 * a2));
        }
        let det = (s_ + p1 * kap) / nn;
        if det.abs() > 1e-14 {
            let (r1x, r1y) = (s_ / nn / det, kap / nn / det);
            let n1 = r1x.hypot(r1y);
            for sg in [-1.0f64, 1.0] {
                let qx = self.x0 + sg * rh * r1x / n1;
                let qy = self.e - v0 + sg * rh * r1y / n1;
                let w = (-p1 * qx + qy) / det;
                if (-t / 2.0 - 1e-12..=t / 2.0 + 1e-12).contains(&w) {
                    let v = r1x * qx + r1y * qy;
                    if sg < 0.0 { los.push(v) } else { his.push(v) }
                }
            }
        }
        if los.is_empty() || his.is_empty() {
            return None;
        }
        let lo = los.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = his.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Some([lo, hi])
    }

    /// Same, cut at the inner (radial T) surface only (the tee rule; kept for the record).
    fn iv_inner(&self, a: f64) -> Option<[f64; 2]> {
        let sa = (a - self.dir).sin();
        let p0 = (self.ra - self.t) * sa;
        let p1 = -self.cs * self.dir.sin() + self.k * sa;
        let a2 = 1.0 + p1 * p1;
        let a1 = 2.0 * (-self.x0 + p1 * (p0 - self.e));
        let a0 = self.x0 * self.x0 + (p0 - self.e).powi(2) - self.rh * self.rh;
        let d = a1 * a1 - 4.0 * a2 * a0;
        if d < 0.0 {
            None
        } else {
            Some([(-a1 - d.sqrt()) / (2.0 * a2), (-a1 + d.sqrt()) / (2.0 * a2)])
        }
    }

    fn weld(&self, m: usize) -> Vec<[f64; 2]> {
        let mut out = vec![];
        for i in 0..m {
            let th = 2.0 * PI * i as f64 / m as f64;
            let Some(t) = self.hit(self.ro, th, 0.0) else {
                continue;
            };
            let x = self.x0 + self.ro * th.cos();
            let l = self.e + self.ro * th.sin();
            out.push([
                x,
                (t * self.dir.sin() + l * self.dir.cos())
                    .atan2(t * self.dir.cos() - l * self.dir.sin() - self.cs * x),
            ]);
        }
        out
    }
}

// ---------------------------------------------------------------- hole ring

struct HoleRing {
    alo: f64,
    ahi: f64,
    am: f64,
    ring: Vec<[f64; 2]>,
    xmin: f64,
    xmax: f64,
}

enum HoleFail {
    NotThrough,
}

fn edge(m: &Model, ac: f64, sg: f64, iv: &dyn Fn(&Model, f64) -> Option<[f64; 2]>) -> Option<f64> {
    let step = 1e-3;
    let mut ain = ac;
    let mut k = 0usize;
    while iv(m, ain + sg * step).is_some() {
        ain += sg * step;
        k += 1;
        if k as f64 > PI / step {
            return None; // hole all round the cone
        }
    }
    let mut aout = ain + sg * step;
    for _ in 0..80 {
        let mid = (ain + aout) / 2.0;
        if iv(m, mid).is_some() {
            ain = mid;
        } else {
            aout = mid;
        }
    }
    Some(ain)
}

fn hole_ring(m: &Model, n: usize) -> Result<HoleRing, HoleFail> {
    let tc = m.hit(0.0, 0.0, m.t).ok_or(HoleFail::NotThrough)?;
    let y = tc * m.dir.cos() - m.e * m.dir.sin();
    let z = tc * m.dir.sin() + m.e * m.dir.cos();
    let ac = z.atan2(y - m.cs * m.x0);
    let iv = |mm: &Model, a: f64| mm.iv(a);
    iv(m, ac).ok_or(HoleFail::NotThrough)?;
    let alo = edge(m, ac, -1.0, &iv).ok_or(HoleFail::NotThrough)?;
    let ahi = edge(m, ac, 1.0, &iv).ok_or(HoleFail::NotThrough)?;
    let (am, ah) = ((alo + ahi) / 2.0, (ahi - alo) / 2.0);
    let (mut lo, mut hi) = (vec![], vec![]);
    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..=n {
        let a = am - ah * (PI * i as f64 / n as f64).cos();
        let Some(q) = m.iv(a) else { continue };
        lo.push([q[0], a]);
        hi.push([q[1], a]);
        xmin = xmin.min(q[0]);
        xmax = xmax.max(q[1]);
    }
    if lo.is_empty() {
        return Err(HoleFail::NotThrough);
    }
    hi.reverse();
    lo.extend(hi);
    Ok(HoleRing {
        alo,
        ahi,
        am,
        ring: lo,
        xmin,
        xmax,
    })
}

// ---------------------------------------------------------------- the development (fan)

/// Development of the radial mid surface (`Ra - T/2`) of the cone about its apex: a point `(x, a)` goes to
/// `lam(x) ell(a) (sin psi(a), cos psi(a))`.
struct Fan {
    r0: f64,
    k: f64,
    ecc: bool,
    // concentric
    sg: f64,
    l0: f64,
    // eccentric
    xa: f64,
    table: Vec<f64>,
    total: f64,
    err: f64,
}

impl Fan {
    fn lam(&self, x: f64) -> f64 {
        (self.r0 + self.k * x) / self.r0
    }
    fn ell(&self, a: f64) -> f64 {
        if self.ecc {
            (self.xa * self.xa + 4.0 * self.r0 * self.r0 * (a / 2.0).sin().powi(2)).sqrt()
        } else {
            self.l0
        }
    }
    /// d psi / d a (eccentric): `r0 sqrt(xa^2 + 4 r0^2 sin^4(a/2)) / ell^2`, cancellation free.
    fn dpsi(&self, a: f64) -> f64 {
        let s = (a / 2.0).sin();
        self.r0 * (self.xa * self.xa + 4.0 * self.r0 * self.r0 * s.powi(4)).sqrt()
            / (self.xa * self.xa + 4.0 * self.r0 * self.r0 * s * s)
    }
    fn psi(&self, a: f64) -> f64 {
        if !self.ecc {
            return a * self.sg;
        }
        let t = a / (2.0 * PI);
        let kk = t.floor();
        let p = (t - kk) * FAN_CELLS as f64;
        let j = (p.floor() as usize).min(FAN_CELLS - 1);
        let u = p - j as f64;
        // cubic Hermite on the cell with the exact derivative at both ends
        let h = 2.0 * PI / FAN_CELLS as f64;
        let (y0, y1) = (self.table[j], self.table[j + 1]);
        let (d0, d1) = (self.dpsi(2.0 * PI * j as f64 / FAN_CELLS as f64) * h, {
            self.dpsi(2.0 * PI * (j + 1) as f64 / FAN_CELLS as f64) * h
        });
        let (u2, u3) = (u * u, u * u * u);
        let v = (2.0 * u3 - 3.0 * u2 + 1.0) * y0
            + (u3 - 2.0 * u2 + u) * d0
            + (-2.0 * u3 + 3.0 * u2) * y1
            + (u3 - u2) * d1;
        kk * self.total + v
    }
    fn pt(&self, x: f64, a: f64) -> [f64; 2] {
        let rr = self.lam(x) * self.ell(a);
        let ps = self.psi(a);
        [rr * ps.sin(), rr * ps.cos()]
    }
}

fn fan(m: &Model) -> Result<Fan, ShapeRefusal> {
    let r0 = m.ra - m.t / 2.0;
    let xa = -r0 / m.k;
    if !m.ecc {
        let sg = m.k.abs() / (1.0 + m.k * m.k).sqrt();
        return Ok(Fan {
            r0,
            k: m.k,
            ecc: false,
            sg,
            l0: r0 / sg,
            xa,
            table: vec![],
            total: 2.0 * PI * sg,
            err: 0.0,
        });
    }
    let mut f = Fan {
        r0,
        k: m.k,
        ecc: true,
        sg: 0.0,
        l0: 0.0,
        xa,
        table: vec![0.0],
        total: 0.0,
        err: 0.0,
    };
    let (mut acc, mut err) = (0.0, 0.0);
    for i in 0..FAN_CELLS {
        let (a0, a1) = (
            2.0 * PI * i as f64 / FAN_CELLS as f64,
            2.0 * PI * (i + 1) as f64 / FAN_CELLS as f64,
        );
        let (v, e) = adaptive_simpson(&|a| f.dpsi(a), a0, a1);
        acc += v;
        err += e;
        f.table.push(acc);
    }
    f.total = acc;
    // Hermite interpolation probe: midpoint of 64 cells against the integral from the cell start
    let mut herm = 0.0f64;
    for j in (0..FAN_CELLS).step_by(FAN_CELLS / 64) {
        let a0 = 2.0 * PI * j as f64 / FAN_CELLS as f64;
        let am = a0 + PI / FAN_CELLS as f64;
        let (v, _) = adaptive_simpson(&|a| f.dpsi(a), a0, am);
        herm = herm.max((f.psi(am) - (f.table[j] + v)).abs());
    }
    f.err = err + herm;
    if !(f.total.is_finite() && f.err.is_finite() && f.err <= FAN_REL_TOL * f.total) {
        return Err(refuse(
            "conebranch_fan_not_converged",
            "l",
            format!(
                "the development angle of the eccentric cone did not converge to {FAN_REL_TOL} relative (estimate {:e}); refused rather than returned",
                f.err / f.total
            ),
        ));
    }
    Ok(f)
}

// ---------------------------------------------------------------- seams

struct SeamOut {
    rule: Vec<(f64, bool, f64, f64)>,
    fin: Vec<(f64, bool, f64, f64)>,
    ss: Vec<f64>,
    hit: bool,
    moved: f64,
    cross: bool,
    kc: f64,
}

fn seams(m: &Model, fan: &Fan, hh: &HoleRing, wl: &[[f64; 2]], n: u32, gap: f64) -> SeamOut {
    let step = 2.0 * PI / f64::from(n);
    let hp: Vec<[f64; 2]> = hh.ring.iter().map(|q| fan.pt(q[0], q[1])).collect();
    let wp: Vec<[f64; 2]> = wl.iter().map(|q| fan.pt(q[0], q[1])).collect();
    let aw: Vec<f64> = wl.iter().map(|q| wrap_a(q[1], hh.am)).collect();
    let a_lo = aw.iter().cloned().fold(hh.alo, f64::min);
    let a_hi = aw.iter().cloned().fold(hh.ahi, f64::max);
    let kc = (a_lo + a_hi) / 2.0;
    let dist = |pts: &[[f64; 2]], a_s: f64| -> f64 {
        let ps = fan.psi(a_s);
        let (ux, uy) = (ps.sin(), ps.cos());
        let l0 = fan.lam(0.0) * fan.ell(a_s);
        let l1 = fan.lam(m.l) * fan.ell(a_s);
        let (lo, hi) = (l0.min(l1), l0.max(l1));
        let mut mn = f64::INFINITY;
        for p in pts {
            let pr = (p[0] * ux + p[1] * uy).clamp(lo, hi);
            mn = mn.min((p[0] - pr * ux).hypot(p[1] - pr * uy));
        }
        mn
    };
    let check = |ss: &[f64]| -> Vec<(f64, bool, f64, f64)> {
        ss.iter()
            .map(|&a_s| {
                let w = wrap_a(a_s, hh.am);
                let cross = hh.alo < w && w < hh.ahi;
                (
                    a_s,
                    cross,
                    if cross { 0.0 } else { dist(&hp, a_s) },
                    dist(&wp, a_s),
                )
            })
            .collect()
    };
    let hits = |r: &[(f64, bool, f64, f64)]| r.iter().any(|q| q.1 || q.2 < gap || q.3 < gap);
    let rule: Vec<f64> = (0..n).map(|j| step * f64::from(j)).collect();
    let r0 = check(&rule);
    let hit = hits(&r0);
    let (mut ss, mut r1, mut moved) = (rule.clone(), r0.clone(), 0.0);
    if hit {
        ss = (0..n)
            .map(|k| kc - PI / f64::from(n) + step * f64::from(k))
            .collect();
        r1 = check(&ss);
        moved = mod_p(ss[0] - rule[0] + step / 2.0, step) - step / 2.0;
    }
    let cross = hits(&r1);
    SeamOut {
        rule: r0,
        fin: r1,
        ss,
        hit,
        moved,
        cross,
        kc,
    }
}

// ---------------------------------------------------------------- frames

/// Concentric: the piece centre generator (azimuth `ac`) is +Y, apex at the origin (ZK `cbFrameConc`).
fn frame_conc(fan: &Fan, ac: f64) -> impl Fn([f64; 2]) -> [f64; 2] {
    let p = fan.psi(ac);
    let (c, s) = (p.cos(), p.sin());
    move |q: [f64; 2]| [q[0] * c - q[1] * s, q[0] * s + q[1] * c]
}

/// Eccentric: the point of the `x = 0` mouth on generator `a0` is the origin, that generator is +X, increasing
/// azimuth goes to -Y (ZK `cbFrameEcc`, the frame of `eccStrip`).
fn frame_ecc(fan: &Fan, m: &Model, a0: f64) -> impl Fn([f64; 2]) -> [f64; 2] {
    let o = fan.pt(0.0, a0);
    let e = fan.pt(m.l, a0);
    let ang = (e[1] - o[1]).atan2(e[0] - o[0]);
    let (c, s) = ((-ang).cos(), (-ang).sin());
    let raw = move |p: [f64; 2]| {
        let (x, y) = (p[0] - o[0], p[1] - o[1]);
        [x * c - y * s, x * s + y * c]
    };
    let sg = if raw(fan.pt(m.l / 2.0, a0 + 1e-3))[1] > 0.0 {
        -1.0
    } else {
        1.0
    };
    move |p: [f64; 2]| {
        let q = raw(p);
        [q[0], sg * q[1]]
    }
}

fn extent(pts: &[[f64; 2]]) -> [f64; 4] {
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    [x0, y0, x1, y1]
}

fn point_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let u = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - u * dx).hypot(p[1] - a[1] - u * dy)
}

fn polyline_len(p: &[[f64; 2]]) -> f64 {
    p.windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum()
}

fn d360(m: &Model, a: f64) -> f64 {
    mod_p(m.az(a) * 180.0 / PI, 360.0)
}

fn finite_all(v: &[f64]) -> bool {
    v.iter().all(|x| x.is_finite())
}

// ---------------------------------------------------------------- branch piece

fn branch_piece(m: &Model) -> Result<CbBranchPiece, ShapeRefusal> {
    let d = m.dc;
    let w = 2.0 * PI * m.rm;
    let len_at = |th: f64| -> Result<f64, ShapeRefusal> {
        let q = m.cut(th, d).filter(|q| q.is_finite()).ok_or_else(|| {
            refuse(
                "conebranch_cut_not_found",
                "d",
                "the branch wall does not meet the cone surface at every angle",
            )
        })?;
        Ok(m.h - q)
    };
    let mut cut = Vec::with_capacity(CB_CUT_SAMPLES + 1);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..=CB_CUT_SAMPLES {
        let l = len_at(PI / 2.0 + 2.0 * PI * i as f64 / CB_CUT_SAMPLES as f64)?;
        cut.push([w * i as f64 / CB_CUT_SAMPLES as f64, l]);
        lo = lo.min(l);
        hi = hi.max(l);
    }
    let mut marks = vec![];
    for k in 0..=CB_MARKS {
        let len = len_at(PI / 2.0 + 2.0 * PI * k as f64 / CB_MARKS as f64)?;
        marks.push(CbMark {
            i: k,
            u: w * k as f64 / CB_MARKS as f64,
            len,
        });
    }
    if lo <= 0.0 {
        return Err(refuse(
            "conebranch_branch_end_below_cut",
            "h",
            format!(
                "the branch end (h) is not beyond the highest contact with the cone (shortest length {lo:.3} mm)"
            ),
        ));
    }
    // exact extremes: golden-section on each sampled bracket of a local minimum / maximum
    let (shortest, longest) = refine_extremes(m, d, &cut, w);
    let h = m.h;
    let (v, err) = adaptive_simpson(
        &|th| m.cut(th, d).map_or(f64::NAN, |q| h - q),
        PI / 2.0,
        PI / 2.0 + 2.0 * PI,
    );
    let area = m.rm * v;
    let area_err = m.rm * err;
    if !(area.is_finite()
        && area > 0.0
        && area_err.is_finite()
        && area_err <= CB_AREA_REL_TOL * area)
    {
        return Err(refuse(
            "conebranch_area_not_converged",
            "d",
            format!(
                "the branch area did not converge to {CB_AREA_REL_TOL} relative; refused rather than returned"
            ),
        ));
    }
    let cut_length = polyline_len(&cut);
    let az0 = mod_p(m.az(m.dir + PI / 2.0) * 180.0 / PI, 360.0);
    Ok(CbBranchPiece {
        circ: w,
        marks,
        cut,
        cut_length,
        shortest,
        longest,
        net_area: area,
        area_err,
        mass_kg: mass_kg(area, m.tb),
        d4: 2.0 * m.ra,
        d12: 2.0 * m.rb,
        seam_az_deg: az0,
    })
}

fn refine_extremes(m: &Model, d: f64, cut: &[[f64; 2]], w: f64) -> (f64, f64) {
    let f = |u: f64| -> f64 {
        let th = PI / 2.0 + 2.0 * PI * u / w;
        m.cut(th, d).map_or(f64::NAN, |q| m.h - q)
    };
    let n = cut.len() - 1;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in cut {
        lo = lo.min(c[1]);
        hi = hi.max(c[1]);
    }
    for sign in [1.0f64, -1.0] {
        let mut best = if sign > 0.0 { lo } else { hi };
        for i in 0..=n {
            let (p, c, q) = (
                cut[(i + n - 1) % n][1] * sign,
                cut[i][1] * sign,
                cut[(i + 1) % n][1] * sign,
            );
            if c <= p && c <= q {
                let (mut a, mut b) = (cut[i][0] - w / n as f64, cut[i][0] + w / n as f64);
                for _ in 0..60 {
                    let (c1, c2) = (a + 0.381_966 * (b - a), a + 0.618_034 * (b - a));
                    if f(c1) * sign <= f(c2) * sign {
                        b = c2;
                    } else {
                        a = c1;
                    }
                }
                let v = f((a + b) / 2.0);
                if v.is_finite() && v * sign < best * sign {
                    best = v;
                }
            }
        }
        if sign > 0.0 {
            lo = best;
        } else {
            hi = best;
        }
    }
    (lo, hi)
}

// ---------------------------------------------------------------- the whole cone side

/// Branch cut and cone pieces of a branch on a cone. The feasibility of the cone (fit, hole, mouth) is decided once:
/// a cone that cannot be built refuses the whole result, the branch is never returned alone (DESIGN L2).
pub fn cone_branch(s: &ConeBranchSpec) -> Result<ConeBranchResult, ShapeRefusal> {
    let n = match s.split {
        None => CB_DEFAULT_SPLIT,
        Some(n) if (2..=CB_MAX_SPLIT).contains(&n) => n,
        Some(n) => {
            return Err(refuse(
                "conebranch_split_out_of_range",
                "split",
                format!("split must be 2..={CB_MAX_SPLIT} (got {n})"),
            ));
        }
    };
    let (m, questions) = setup(s)?;
    // 1. the cone: fit, hole, mouth
    for i in 0..360 {
        let th = 2.0 * PI * f64::from(i) / 360.0;
        let x = m.x0 + m.rh * th.cos();
        if (m.e + m.rh * th.sin() + m.cs * x * m.dir.sin()).abs() > m.ra + m.k * x - m.dc + 1e-9 {
            return Err(refuse(
                "conebranch_branch_not_within_cone",
                "d",
                "the branch does not fit on the cone (branch diameter and offset against the cone diameter at the branch)",
            ));
        }
    }
    let hh = hole_ring(&m, CB_HOLE_SAMPLES).map_err(|HoleFail::NotThrough| {
        refuse(
            "conebranch_hole_not_through_plate",
            "d",
            "the branch hole does not pass through the cone plate (check the cone and branch dimensions)",
        )
    })?;
    if hh.xmin < -1e-6 || hh.xmax > m.l + 1e-6 {
        return Err(refuse(
            "conebranch_hole_beyond_mouth",
            "x",
            "the branch hole runs past a mouth of the cone (check the distance from the mouth to the branch centre and the cone length)",
        ));
    }
    // 2. the branch (blocked by a cone that is not buildable: the checks above ran first)
    let branch = branch_piece(&m)?;
    // 3. the fan, weld line, seams
    let fan = fan(&m)?;
    let weld = m.weld(CB_WELD_SAMPLES);
    let gap = s.seam_gap.unwrap_or(CB_SEAM_GAP_T * m.t);
    let sm = seams(&m, &fan, &hh, &weld, n, gap);
    let step = 2.0 * PI / f64::from(n);
    let kc = sm.kc;
    let a0 = sm
        .ss
        .iter()
        .map(|&sa| kc - mod_p(kc - sa, 2.0 * PI))
        .fold(f64::NEG_INFINITY, f64::max);
    // 4. hole polygon and marks in the frame of the first piece
    let tr: Box<dyn Fn([f64; 2]) -> [f64; 2]> = if m.ecc {
        Box::new(frame_ecc(&fan, &m, a0))
    } else {
        Box::new(frame_conc(&fan, a0 + step / 2.0))
    };
    let hole: Vec<[f64; 2]> = hh.ring.iter().map(|q| tr(fan.pt(q[0], q[1]))).collect();
    let mut hm = vec![];
    for k in 0..16usize {
        let th = 2.0 * PI * k as f64 / 16.0;
        let Some(tt) = m.hit(m.rh, th, m.t) else {
            continue;
        };
        let x = m.x0 + m.rh * th.cos();
        let l = m.e + m.rh * th.sin();
        let a = wrap_a(
            (tt * m.dir.sin() + l * m.dir.cos())
                .atan2(tt * m.dir.cos() - l * m.dir.sin() - m.cs * x),
            hh.am,
        )
        .clamp(hh.alo, hh.ahi);
        let iv = m.iv(a).unwrap_or([x, x]);
        let cq = th.cos();
        let xx = if cq > 1e-9 {
            iv[1]
        } else if cq < -1e-9 {
            iv[0]
        } else {
            (iv[0] + iv[1]) / 2.0
        };
        let p = tr(fan.pt(xx, a));
        hm.push(CbHoleMark {
            i: k,
            x: p[0],
            y: p[1],
            ax: m.ax(xx),
            az_deg: d360(&m, a),
        });
    }
    let bb = extent(&hole);
    let area_poly = shoelace(&hole);
    // exact hole area: integral over the azimuth span (cosine substitution removes the square-root ends)
    let (am, ah) = (hh.am, (hh.ahi - hh.alo) / 2.0);
    let lam = |x: f64| fan.lam(x);
    let integrand = |ph: f64| {
        let a = am - ah * ph.cos();
        let Some(q) = m.iv(a) else { return 0.0 };
        let dpsi = if fan.ecc { fan.dpsi(a) } else { fan.sg };
        let e = fan.ell(a);
        0.5 * e * e * (lam(q[1]).powi(2) - lam(q[0]).powi(2)).abs() * dpsi * ah * ph.sin()
    };
    let (area_h, area_h_err) = adaptive_simpson(&integrand, 0.0, PI);
    let iv_inner_len = {
        let ac = hh.am;
        m.iv_inner(ac).map(|v| v[1] - v[0])
    };
    let len_normal = m.iv(hh.am).map_or(0.0, |v| v[1] - v[0]);
    let perimeter = polyline_len(&hole) + {
        let (f, l) = (hole[0], hole[hole.len() - 1]);
        (f[0] - l[0]).hypot(f[1] - l[1])
    };
    // eps_rep: exact curve points at the mid azimuth of each polygon edge against that edge (an estimate, not a bound)
    let mut eps_rep = 0.0f64;
    {
        let mut nh = hh.ring.len() / 2;
        nh = nh.max(1);
        for side in 0..2 {
            for i in 0..nh.saturating_sub(1) {
                let idx = |j: usize| if side == 0 { j } else { hh.ring.len() - 1 - j };
                let (p, q) = (hh.ring[idx(i)], hh.ring[idx(i + 1)]);
                let am_ = (p[1] + q[1]) / 2.0;
                if let Some(v) = m.iv(am_) {
                    let xx = if side == 0 { v[0] } else { v[1] };
                    let exact = tr(fan.pt(xx, am_));
                    let (pa, qa) = (tr(fan.pt(p[0], p[1])), tr(fan.pt(q[0], q[1])));
                    eps_rep = eps_rep.max(point_segment(exact, pa, qa));
                }
            }
        }
    }
    let hole_info = CbHoleInfo {
        x_min: m.ax(hh.xmin).min(m.ax(hh.xmax)),
        x_max: m.ax(hh.xmin).max(m.ax(hh.xmax)),
        az_start_deg: d360(&m, if m.flip { hh.ahi } else { hh.alo }),
        az_span_deg: (hh.ahi - hh.alo) * 180.0 / PI,
        bbox: bb,
        box_w: bb[2] - bb[0],
        box_h: bb[3] - bb[1],
        area_poly,
        area: area_h,
        area_err: area_h_err,
        len_inner_only: iv_inner_len,
        len_normal_cut: len_normal,
        marks: hm,
        perimeter,
        eps_rep,
    };
    if !(area_h.is_finite() && area_h > 0.0 && area_h_err <= CB_HOLE_AREA_REL_TOL * area_h) {
        return Err(refuse(
            "conebranch_area_not_converged",
            "d",
            "the hole area did not converge; refused rather than returned",
        ));
    }
    // 5. cone pieces
    let r0 = fan.r0;
    let lam_l = fan.lam(m.l);
    let (cone_area, cone_area_err, pieces) = if !m.ecc {
        let cs = cone_split(ConeSpec {
            d1: s.da.min(s.db),
            d2: s.da.max(s.db),
            l: m.l,
            t: m.t,
            basis: Basis::Mid,
            split: n,
        })?;
        let half = cs.piece_angle;
        let mut out = vec![];
        let mut o = vec![];
        let mut inn = vec![];
        for i in 0..=240 {
            let a = -half / 2.0 + half * f64::from(i) / 240.0;
            o.push([cs.rl * a.sin(), cs.rl * a.cos()]);
            inn.push([cs.rs * a.sin(), cs.rs * a.cos()]);
        }
        inn.reverse();
        o.extend(inn);
        let o0 = o[0];
        o.push(o0);
        let ex = extent(&o);
        out.push(CbConePiece {
            label: "1/n (branch hole)".into(),
            outline: o.clone(),
            hole: Some(hole.clone()),
            copies: 1,
            w: ex[2] - ex[0],
            h: ex[3] - ex[1],
            net_area: cs.piece_area - area_h,
            mass_kg: mass_kg(cs.piece_area - area_h, m.t),
            chord: Some(cs.chord),
            height: Some(cs.height),
        });
        out.push(CbConePiece {
            label: "other pieces (same shape)".into(),
            outline: o,
            hole: None,
            copies: n - 1,
            w: ex[2] - ex[0],
            h: ex[3] - ex[1],
            net_area: cs.piece_area,
            mass_kg: mass_kg(cs.piece_area, m.t),
            chord: Some(cs.chord),
            height: Some(cs.height),
        });
        (cs.piece_area * f64::from(n), 0.0, out)
    } else {
        // area element: 1/2 (1 - lam_L^2) ell^2 dpsi = 1/2 (1 - lam_L^2) r0 sqrt(xa^2 + 4 r0^2 sin^4(a/2))
        let dens = |a: f64| {
            let sn = (a / 2.0).sin();
            0.5 * (1.0 - lam_l * lam_l) * r0 * (fan.xa * fan.xa + 4.0 * r0 * r0 * sn.powi(4)).sqrt()
        };
        let mut out: Vec<CbConePiece> = vec![];
        let (mut tot, mut tot_err) = (0.0, 0.0);
        let mirror = n == 2 && sm.moved.abs() < 1e-9;
        for j in 0..n {
            let (aj, aj1) = (a0 + step * f64::from(j), a0 + step * f64::from(j + 1));
            let (area, err) = adaptive_simpson(&dens, aj, aj1);
            tot += area;
            tot_err += err;
            let (outline, ex) = if j == 1 && mirror {
                let pc = &out[0];
                let o: Vec<[f64; 2]> = pc.outline.iter().map(|q| [q[0], -q[1]]).collect();
                let e = extent(&o);
                (o, e)
            } else {
                let trj = frame_ecc(&fan, &m, aj);
                let ns = (1440 / n as usize).max(180);
                let mut e0 = vec![];
                let mut e1 = vec![];
                for i in 0..=ns {
                    let a = aj + (aj1 - aj) * i as f64 / ns as f64;
                    e0.push(trj(fan.pt(0.0, a)));
                    e1.push(trj(fan.pt(m.l, a)));
                }
                e1.reverse();
                e0.extend(e1);
                let f0 = e0[0];
                e0.push(f0);
                let e = extent(&e0);
                (e0, e)
            };
            out.push(CbConePiece {
                label: format!("{}/{}", j + 1, n),
                outline,
                hole: if j == 0 { Some(hole.clone()) } else { None },
                copies: 1,
                w: ex[2] - ex[0],
                h: ex[3] - ex[1],
                net_area: area - if j == 0 { area_h } else { 0.0 },
                mass_kg: mass_kg(area - if j == 0 { area_h } else { 0.0 }, m.t),
                chord: None,
                height: None,
            });
        }
        (tot, tot_err, out)
    };
    let cone_mass = mass_kg(cone_area - area_h, m.t);
    // 6. holds, questions
    let mut holds = vec![];
    if sm.cross {
        holds.push(CbHold {
            code: "conebranch_seam_crosses_hole",
            message: format!(
                "with {n} pieces a seam still crosses the branch hole or comes closer than {gap:.1} mm to it or to the weld line after turning the seams; use fewer pieces or fix the seam position"
            ),
        });
    }
    if s.ecc && s.flat_deg.is_none() {
        holds.push(CbHold {
            code: "conebranch_flat_side_unconfirmed",
            message: CB_FLAT_QUESTION.to_string(),
        });
    }
    let fin = &sm.fin;
    let seam_hole = fin.iter().map(|q| q.2).fold(f64::INFINITY, f64::min);
    let seam_weld = fin.iter().map(|q| q.3).fold(f64::INFINITY, f64::min);
    let sdeg: Vec<f64> = sm.ss.iter().map(|&a| d360(&m, a)).collect();
    let rdeg: Vec<f64> = sm.rule.iter().map(|q| d360(&m, q.0)).collect();
    let mut vals = vec![
        branch.net_area,
        branch.mass_kg,
        hole_info.area,
        cone_mass,
        seam_hole,
        seam_weld,
        fan.total,
    ];
    vals.extend(hole_info.marks.iter().flat_map(|q| [q.x, q.y]));
    vals.extend(branch.marks.iter().map(|q| q.len));
    if !finite_all(&vals) {
        return Err(refuse(
            "conebranch_result_not_finite",
            "d",
            "a result value is not finite; refused",
        ));
    }
    let seam_info = CbSeamInfo {
        deg: sdeg,
        rule_deg: rdeg,
        hit: sm.hit,
        moved_deg: (if m.flip { -1.0 } else { 1.0 }) * sm.moved * 180.0 / PI,
        cross: sm.cross,
        gap,
        seam_hole,
        seam_weld,
    };
    Ok(ConeBranchResult {
        branch,
        hole: hole_info,
        seams: seam_info,
        pieces,
        n,
        x_used: s.x.unwrap_or(s.l / 2.0),
        fan_total_angle: fan.total,
        fan_err: fan.err,
        cone_area,
        cone_area_err,
        cone_mass_kg: cone_mass,
        holds,
        questions,
        certified: false,
        method: CONEBRANCH_METHOD,
        rules: &CONEBRANCH_NAMED_RULES,
    })
}

/// Internal views for the independent tests (not part of the engine surface).
#[doc(hidden)]
pub mod probe {
    use super::*;

    /// Highest contact of the branch wall with the cone surface at angle `th` (ZK `cbCut`), by the closed form.
    pub fn cut_t(s: &ConeBranchSpec, th: f64) -> Option<f64> {
        let (m, _) = setup(s).ok()?;
        m.cut(th, m.dc)
    }
    /// Hole interval `[lo, hi]` of the development surface at the internal azimuth `a`.
    pub fn hole_iv(s: &ConeBranchSpec, a: f64) -> Option<[f64; 2]> {
        let (m, _) = setup(s).ok()?;
        m.iv(a)
    }
    /// The development of the cone (built once; the eccentric one holds a table).
    pub struct FanProbe(Fan);
    impl FanProbe {
        /// Development point of `(x, a)` (internal frame).
        pub fn pt(&self, x: f64, a: f64) -> [f64; 2] {
            self.0.pt(x, a)
        }
        /// Development angle of the generator at azimuth `a`.
        pub fn psi(&self, a: f64) -> f64 {
            self.0.psi(a)
        }
        /// Stated error of the development angle table.
        pub fn err(&self) -> f64 {
            self.0.err
        }
    }
    pub fn fan_of(s: &ConeBranchSpec) -> Option<FanProbe> {
        let (m, _) = setup(s).ok()?;
        Some(FanProbe(fan(&m).ok()?))
    }
    /// The internal model: (ra, rb, k, cs, flip, x0, dir, e, h, rh, dc, f).
    pub fn model(s: &ConeBranchSpec) -> Option<[f64; 12]> {
        let (m, _) = setup(s).ok()?;
        Some([
            m.ra,
            m.rb,
            m.k,
            m.cs,
            if m.flip { 1.0 } else { 0.0 },
            m.x0,
            m.dir,
            m.e,
            m.h,
            m.rh,
            m.dc,
            m.f,
        ])
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn ex1() -> ConeBranchSpec {
        ConeBranchSpec {
            da: 1016.0,
            db: 812.8,
            l: 812.8,
            t: 12.0,
            ecc: false,
            flat_deg: None,
            d: 406.4,
            tb: 9.0,
            x: Some(406.4),
            dir_deg: 0.0,
            e: 0.0,
            h: 800.0,
            angle_deg: None,
            mount: Mount::SetOn,
            split: Some(2),
            seam_gap: None,
        }
    }

    #[test]
    fn example_1_branch_marks() {
        let r = cone_branch(&ex1()).unwrap();
        assert!((r.branch.marks[4].len - 317.40).abs() < 0.01);
        assert!(!r.certified);
    }
}
