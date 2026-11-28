//! NFJ-1 S1-L7b: reinforcing pad (collar plate) round a tee branch, perpendicular or lateral, with the split of the pad
//! into 2..=12 pieces. Pure functions, no I/O, no engine state.
//!
//! Status (stated, not hidden): `certified: true` only when `uem::certify` proves the 0.05 mm chord bound for the drawn
//! outline and no hold applies (TOLERANCE_POLICY section 8.9). Otherwise `certified: false` with its `method` and named
//! `rules` (all UNVERIFIED, none confirmed by a factory). `ENGINE_VERSION` is 0.6.0 since L11.
//!
//! Frame (ZK `padPiece` / `offsetRing` / `padSplit`, re-derived; tests are independent): the pad lies on the main outer
//! surface (radius `Ro`), is rolled from a plate of thickness `t` and is developed at its mid surface `Rp = Ro + t/2`.
//! Plate coordinates are (`u` = `Rp` x azimuth from the branch direction, `x` = axial position from the intersection
//! of the branch and main axes). The hole is the line where the branch outer surface meets the pad: the perpendicular
//! hole follows `Ro` (thin) or, when the offset is larger than the branch radius or the branch is lateral, is cut so
//! that the branch touches the pad nowhere across its thickness (`Ro..Rt`, rule `PAD_HOLE_THROUGH_THICKNESS`). The
//! outline is either a constant width `B` outside the hole (corners of the hole that open outwards are rounded with
//! arcs of radius `B`) or the circle of the plan view `Dp` (rule `PAD_OUTLINE_DP_CENTRE`).
//!
//! Exactness: the hole is a 360 point polyline (thin) or the `lat_hole` ring (thick); `eps_rep` is an ESTIMATE of the
//! polyline-to-curve distance, not a bound. Split rays hit the polylines exactly (closed-form segment intersection).

use crate::shapes::{ShapeRefusal, mass_kg, refuse};
use crate::tee::{bbox, check_angle, clamp1, lat_cs, lat_hole, mod_p, shoelace};
use std::f64::consts::PI;
use uem::P2;
use uem::certify::{CurveEnclosure, L11_MAX_CHORD_ERROR_MM, certified_rep, clamp};
use uem::interval::Iv;

pub const PAD_METHOD: &str = "analytic hole (360 point polyline or lat_hole ring), constant-width offset with arcs, ray split on the polylines; areas by shoelace; L11 certified polyline bounds where derivable";
pub const PAD_POLY_SAMPLES: usize = 360;
pub const PAD_MAX_SPLIT: u32 = 12;
/// Direction change (deg) above which a hole vertex is a corner; arc step (deg) of a rounded corner.
pub const PAD_CORNER_DEG: f64 = 4.0;
pub const PAD_ARC_STEP_DEG: f64 = 2.0;

/// (name, value, kind, status)
pub const PAD_NAMED_RULES: [(&str, &str, &str, &str); 5] = [
    (
        "PAD_HOLE_THROUGH_THICKNESS",
        "a lateral pad, or a pad whose branch offset is larger than the branch outer radius, has its hole cut so that the branch touches the pad nowhere across the pad thickness (outermost envelope between the main surface and the pad surface)",
        "rule (hole construction)",
        "UNVERIFIED",
    ),
    (
        "PAD_OUTLINE_WIDTH",
        "outline = hole offset outwards by the width B on the developed pad; outward corners rounded with arcs of radius B",
        "rule (outline construction)",
        "UNVERIFIED",
    ),
    (
        "PAD_OUTLINE_DP_CENTRE",
        "plan-view circle Dp: centre where the branch axis crosses the main outer surface, matched on the pad outer surface (a lateral pad's hole is long, so a circle can leave a narrow rim; a hole that leaves the circle is refused)",
        "rule (outline construction)",
        "UNVERIFIED",
    ),
    (
        "PAD_SPLIT_RAYS",
        "split lines are rays from the branch centre on the developed pad at beta0 + 360 k / n; beta0 = 180/n deg when n and n/2 are even, 90 deg for a two-way split along the main axis, else 0; the pad is not split unless the person says so (no default split)",
        "rule (split construction)",
        "UNVERIFIED",
    ),
    (
        "PAD_LIMITS",
        "2..=12 pieces, 360 point polylines",
        "implementation limit",
        "UNVERIFIED",
    ),
];

/// Stable refusal codes of this module (the angle codes are the `tee_*` ones of `tee::check_angle`).
pub const PAD_REFUSAL_CODES: [&str; 9] = [
    "pad_value_not_finite",
    "pad_value_not_positive",
    "pad_value_too_large",
    "pad_branch_beyond_main",
    "pad_hole_off_main",
    "pad_outline_not_around_hole",
    "pad_split_out_of_range",
    "pad_split_not_possible",
    "pad_result_not_finite",
];

const MAX_MM: f64 = crate::shapes::MAX_MM;
const MIN_MM: f64 = crate::shapes::MIN_MM;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PadOutline {
    /// Constant width `B` outside the hole (mm).
    Width(f64),
    /// Circle of diameter `Dp` of the plan view (mm).
    Circle(f64),
}

/// Direction of the split line of a two-way split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitCut {
    /// The split line is perpendicular to the main axis (ZK `cross`).
    Cross,
    /// The split line runs along the main axis (ZK `along`).
    Along,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadSpec {
    /// Main outer diameter.
    pub main_d: f64,
    /// Pad plate thickness.
    pub t: f64,
    /// Branch outer diameter.
    pub d: f64,
    /// Signed offset of the branch centre line from the main centre line.
    pub e: f64,
    /// Sharp branch angle (deg): `None` or 90 perpendicular; below 90 lateral.
    pub angle_deg: Option<f64>,
    pub outline: PadOutline,
    /// Number of pieces: `None` or 1 = not split; 2..=12.
    pub split: Option<u32>,
    pub cut: SplitCut,
}

/// One piece of the developed pad: closed polyline (first point repeated at the end). `copies` is 2 for the
/// perpendicular two-way cross split, where the two pieces are the same shape mirrored (one is returned).
#[derive(Debug, Clone, PartialEq)]
pub struct PadPiece {
    pub pts: Vec<[f64; 2]>,
    pub copies: u32,
    pub w: f64,
    pub h: f64,
    pub area: f64,
    pub mass_kg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PadResult {
    /// Exact definitions of the drawn hole and outline; empty unless the representation is certified.
    pub exact: Vec<crate::tee::CurveDecl>,
    /// Outline polyline (closed, first point repeated) and the hole ring (open). Same number of points.
    pub outline: Vec<[f64; 2]>,
    pub hole: Vec<[f64; 2]>,
    /// Bounding box of the outline, and of the hole.
    pub w: f64,
    pub h: f64,
    pub hole_w: f64,
    pub hole_h: f64,
    pub net_area: f64,
    /// Mass of the unsplit pad.
    pub mass_kg: f64,
    pub thick_hole: bool,
    pub split: u32,
    /// "none", "cross", "along" (two-way) or "even" (3..=12).
    pub cut: &'static str,
    pub pieces: Vec<PadPiece>,
    /// Split line lengths (hole edge to outline) and the smallest angle (deg) between a split line and the main axis
    /// measured from the axis (ZK `seamAngle`: 90 = perpendicular to the main axis).
    pub seams: Vec<f64>,
    pub seam_angle_deg: f64,
    /// (hole end, outline end) of every split line, and the split centre.
    pub cuts: Vec<[[f64; 2]; 2]>,
    pub center: [f64; 2],
    /// Sum of the piece masses (copies counted): equals `mass_kg` (checked by tests, not forced).
    pub pieces_mass_kg: f64,
    /// Sum over the pieces of bounding box area x copies x t x density (mass of the plate blanks).
    pub gross_box_mass_kg: f64,
    /// ESTIMATE of the polyline-to-curve distance of the hole and outline (mid-edge sagitta); not a bound.
    pub eps_rep: f64,
    pub eps_rep_certified: bool,
    pub certified: bool,
    pub method: &'static str,
    pub rules: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

fn dim(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "pad_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v <= 0.0 || v < MIN_MM {
        return Err(refuse(
            "pad_value_not_positive",
            name,
            format!("{name} must be greater than 0 (got {v})"),
        ));
    }
    if v > MAX_MM {
        return Err(refuse(
            "pad_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

// ---------------------------------------------------------------- offset ring

/// A closed ring (open list, last point does not return to the first) moved outwards by `b` on the developed pad.
/// Returns (hole side, outline side) with the same number of points: an outward corner (turn above
/// `PAD_CORNER_DEG`) becomes an arc of radius `b` of `ceil(turn / 2 deg) + 1` points, and the hole side repeats the
/// corner point for each, so a 3D mesh can join hole and outline point by point. ZK `offsetRing`.
fn offset_ring(inn: &[[f64; 2]], b: f64) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let m = inn.len();
    let mut area = 0.0;
    for i in 0..m {
        let (q0, q1) = (inn[i], inn[(i + 1) % m]);
        area += q0[0] * q1[1] - q1[0] * q0[1];
    }
    let sg = if area > 0.0 { 1.0 } else { -1.0 };
    let (bw, bh) = bbox(inn);
    let eps = 1e-6 * bw.max(bh).max(1.0);
    let th = PAD_CORNER_DEG * PI / 180.0;
    let dist = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]);
    let near = |i: usize, dir: i64| -> [f64; 2] {
        for j in 1..m {
            let k = (i as i64 + dir * j as i64).rem_euclid(m as i64) as usize;
            if dist(inn[k], inn[i]) > eps {
                return inn[k];
            }
        }
        inn[i]
    };
    let nrm = |a: [f64; 2], c: [f64; 2]| -> [f64; 2] {
        let (tx, ty) = (c[0] - a[0], c[1] - a[1]);
        let l = tx.hypot(ty);
        let l = if l == 0.0 { 1.0 } else { l };
        [sg * ty / l, -sg * tx / l]
    };
    let dup = |i: usize| dist(inn[(i + m - 1) % m], inn[i]) <= eps;
    let mut s0 = 0;
    while s0 + 1 < m && dup(s0) {
        s0 += 1;
    }
    let (mut ins, mut out): (Vec<[f64; 2]>, Vec<[f64; 2]>) = (vec![], vec![]);
    for c0 in 0..m {
        let i = (s0 + c0) % m;
        if c0 > 0 && dup(i) {
            let last = *out.last().unwrap_or(&inn[i]);
            out.push(last);
            ins.push(inn[i]);
            continue;
        }
        let (p, a, c) = (inn[i], near(i, -1), near(i, 1));
        let (n1, n2) = (nrm(a, p), nrm(p, c));
        let turn = (n1[0] * n2[1] - n1[1] * n2[0]).atan2(n1[0] * n2[0] + n1[1] * n2[1]);
        let dot = n1[0] * n2[0] + n1[1] * n2[1];
        if turn.abs() <= th || dot < -0.999 {
            let nc = nrm(a, c);
            out.push([p[0] + b * nc[0], p[1] + b * nc[1]]);
            ins.push(p);
            continue;
        }
        if sg * turn < 0.0 {
            out.push([
                p[0] + b * (n1[0] + n2[0]) / (1.0 + dot),
                p[1] + b * (n1[1] + n2[1]) / (1.0 + dot),
            ]);
            ins.push(p);
            continue;
        }
        let steps = (turn.abs() / (PAD_ARC_STEP_DEG * PI / 180.0))
            .ceil()
            .max(1.0) as usize;
        for k in 0..=steps {
            let w = turn * k as f64 / steps as f64;
            let (cw, sw) = (w.cos(), w.sin());
            out.push([
                p[0] + b * (n1[0] * cw - n1[1] * sw),
                p[1] + b * (n1[0] * sw + n1[1] * cw),
            ]);
            ins.push(p);
        }
    }
    (ins, out)
}

// ---------------------------------------------------------------- split

#[derive(Debug, Clone, Copy)]
struct Hit {
    i: usize,
    s: f64,
    p: [f64; 2],
}

fn cw_poly(p: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut s = 0.0;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    if s > 0.0 {
        p.iter().rev().copied().collect()
    } else {
        p.to_vec()
    }
}

/// Where the ray from `c` in direction `b` meets the closed polygon: exactly one hit, else `None`.
fn ray_hit(poly: &[[f64; 2]], c: [f64; 2], b: f64) -> Option<Hit> {
    let (dx, dy) = (b.cos(), b.sin());
    let n = poly.len();
    let mut hits: Vec<Hit> = vec![];
    for i in 0..n {
        let (a, q) = (poly[i], poly[(i + 1) % n]);
        let (ex, ey) = (q[0] - a[0], q[1] - a[1]);
        let den = dy * ex - dx * ey;
        if den.abs() < 1e-15 {
            continue;
        }
        let (rx, ry) = (a[0] - c[0], a[1] - c[1]);
        let t = (ry * ex - rx * ey) / den;
        let mut s = (dx * ry - dy * rx) / den;
        if t <= 1e-9 || !(-1e-9..=1.0 + 1e-9).contains(&s) {
            continue;
        }
        s = s.clamp(0.0, 1.0);
        let p = [a[0] + s * ex, a[1] + s * ey];
        if hits
            .iter()
            .any(|h| (h.p[0] - p[0]).hypot(h.p[1] - p[1]) < 1e-6)
        {
            continue;
        }
        hits.push(if s >= 1.0 - 1e-9 {
            Hit {
                i: (i + 1) % n,
                s: 0.0,
                p,
            }
        } else {
            Hit { i, s, p }
        });
    }
    if hits.len() == 1 { Some(hits[0]) } else { None }
}

/// Walk the clockwise polygon from hit `h0` to hit `h1` with increasing vertex index.
fn walk_to(poly: &[[f64; 2]], h0: Hit, h1: Hit) -> Vec<[f64; 2]> {
    let n = poly.len();
    let mut pts = vec![h0.p];
    if !(h1.i == h0.i && h1.s >= h0.s) {
        let mut j = (h0.i + 1) % n;
        loop {
            pts.push(poly[j]);
            if j == h1.i {
                break;
            }
            j = (j + 1) % n;
        }
    }
    if h1.s > 1e-12 {
        pts.push(h1.p);
    }
    pts
}

struct SplitOut {
    pieces: Vec<Vec<[f64; 2]>>,
    cuts: Vec<[[f64; 2]; 2]>,
    seams: Vec<f64>,
    seam_angle: f64,
}

fn pad_split(
    inn: &[[f64; 2]],
    out: &[[f64; 2]],
    c: [f64; 2],
    n: u32,
    cut: SplitCut,
) -> Option<SplitOut> {
    let b0 = if n == 2 && cut == SplitCut::Along {
        PI / 2.0
    } else if n.is_multiple_of(2) && (n / 2).is_multiple_of(2) {
        PI / f64::from(n)
    } else {
        0.0
    };
    let bs: Vec<f64> = (0..n)
        .map(|k| b0 + 2.0 * PI * f64::from(k) / f64::from(n))
        .collect();
    let (ip, op) = (cw_poly(inn), cw_poly(out));
    let hi: Vec<Option<Hit>> = bs.iter().map(|b| ray_hit(&ip, c, *b)).collect();
    let ho: Vec<Option<Hit>> = bs.iter().map(|b| ray_hit(&op, c, *b)).collect();
    if hi.iter().chain(ho.iter()).any(|h| h.is_none()) {
        return None;
    }
    let (hi, ho): (Vec<Hit>, Vec<Hit>) = (
        hi.into_iter().flatten().collect(),
        ho.into_iter().flatten().collect(),
    );
    let nn = n as usize;
    let mut pieces = vec![];
    for k in 0..nn {
        let k1 = (k + 1) % nn;
        let mut p: Vec<[f64; 2]> = walk_to(&op, ho[k1], ho[k]).into_iter().rev().collect();
        p.extend(walk_to(&ip, hi[k1], hi[k]));
        p.push(p[0]);
        pieces.push(p);
    }
    let cuts = (0..nn).map(|j| [hi[j].p, ho[j].p]).collect();
    let seams = (0..nn)
        .map(|j| (ho[j].p[0] - hi[j].p[0]).hypot(ho[j].p[1] - hi[j].p[1]))
        .collect();
    let seam_angle = bs
        .iter()
        .map(|b| (mod_p(*b, PI) * 180.0 / PI - 90.0).abs())
        .fold(f64::INFINITY, f64::min);
    Some(SplitOut {
        pieces,
        cuts,
        seams,
        seam_angle,
    })
}

// ---------------------------------------------------------------- pad

fn piece_of(pts: Vec<[f64; 2]>, copies: u32, t: f64) -> PadPiece {
    let (w, h) = bbox(&pts);
    let area = shoelace(&pts);
    PadPiece {
        w,
        h,
        area,
        mass_kg: mass_kg(area, t),
        copies,
        pts,
    }
}

/// Enclosure of the thin pad hole `(Rp·asin((e + ro·sin θ)/Ro), ro·cos θ)`; the first coordinate is already
/// a plate distance, so distances are measured in millimetres.
struct PadHoleCurve {
    rp: f64,
    ro_main: f64,
    e: f64,
    ro: f64,
}

impl CurveEnclosure for PadHoleCurve {
    fn enclose(&self, t: Iv) -> Option<(Iv, Iv)> {
        let arg = clamp(
            Iv::pt(self.e)
                .add(Iv::pt(self.ro).mul(t.sin()))
                .div(Iv::pt(self.ro_main))?,
            -1.0,
            1.0,
        );
        Some((
            Iv::pt(self.rp).mul(arg.asin()?),
            Iv::pt(self.ro).mul(t.cos()),
        ))
    }
}

/// Enclosure of the plan-view circle outline developed on the pad mid surface.
struct PadCircleCurve {
    rp: f64,
    rt: f64,
    e: f64,
    rad: f64,
    xs: f64,
}

impl CurveEnclosure for PadCircleCurve {
    fn enclose(&self, t: Iv) -> Option<(Iv, Iv)> {
        let arg = clamp(
            Iv::pt(self.e)
                .add(Iv::pt(self.rad).mul(t.sin()))
                .div(Iv::pt(self.rt))?,
            -1.0,
            1.0,
        );
        Some((
            Iv::pt(self.rp).mul(arg.asin()?),
            Iv::pt(self.xs).add(Iv::pt(self.rad).mul(t.cos())),
        ))
    }
}

fn params(n: usize) -> Vec<f64> {
    (0..=n).map(|k| 2.0 * PI * k as f64 / n as f64).collect()
}

fn closed(points: &[[f64; 2]]) -> Vec<P2> {
    let mut v: Vec<P2> = points.iter().map(|q| P2::new(q[0], q[1])).collect();
    if let Some(first) = v.first().copied() {
        v.push(first);
    }
    v
}

/// Developed pad with its hole, outline and (optionally) its split. See the module comment for the rules.
pub fn pad(s: &PadSpec) -> Result<PadResult, ShapeRefusal> {
    dim("main_d", s.main_d)?;
    dim("t", s.t)?;
    dim("d", s.d)?;
    if !s.e.is_finite() || s.e.abs() > MAX_MM {
        return Err(refuse(
            "pad_value_not_finite",
            "e",
            format!(
                "e must be a finite number within the limit {MAX_MM} mm (got {})",
                s.e
            ),
        ));
    }
    check_angle(s.angle_deg)?;
    match s.outline {
        PadOutline::Width(b) => {
            dim("outline.width", b)?;
        }
        PadOutline::Circle(dp) => {
            dim("outline.dp", dp)?;
        }
    }
    let split = match s.split {
        None | Some(1) => 1,
        Some(n) if (2..=PAD_MAX_SPLIT).contains(&n) => n,
        Some(n) => {
            return Err(refuse(
                "pad_split_out_of_range",
                "split",
                format!(
                    "a pad is split into 2..={PAD_MAX_SPLIT} pieces (got {n}); it is not split by itself"
                ),
            ));
        }
    };
    let (ro_main, ro) = (s.main_d / 2.0, s.d / 2.0);
    let (rp, rt) = (ro_main + s.t / 2.0, ro_main + s.t);
    let e = s.e;
    if e.abs() + ro > ro_main + 1e-6 {
        return Err(refuse(
            "pad_branch_beyond_main",
            "e",
            format!(
                "the branch (outer radius {ro}, offset {}) reaches beyond the main outer radius {ro_main}",
                e.abs()
            ),
        ));
    }
    let ca_sa = lat_cs(s.angle_deg);
    let lateral = ca_sa.0 != 0.0;
    let thick = lateral || e.abs() > ro;
    let (mut inn, mut out): (Vec<[f64; 2]>, Vec<[f64; 2]>) = (vec![], vec![]);
    let mut eps_hole = 0.0f64;
    let mut eps_certified = false;
    if thick {
        let Some(hp) = lat_hole(ro_main, rt, ro, e, ca_sa) else {
            return Err(refuse(
                "pad_hole_off_main",
                "d",
                "the branch does not pass through the pad: no hole",
            ));
        };
        inn = hp.ring.iter().map(|q| [rp * q[0], q[1]]).collect();
        eps_hole = hp.eps;
    } else {
        let f = |th: f64| {
            [
                rp * clamp1((e + ro * th.sin()) / ro_main).asin(),
                ro * th.cos(),
            ]
        };
        for i in 0..PAD_POLY_SAMPLES {
            inn.push(f(2.0 * PI * i as f64 / PAD_POLY_SAMPLES as f64));
        }
        let curve = PadHoleCurve { rp, ro_main, e, ro };
        if let Some(bound) = certified_rep(
            &curve,
            &closed(&inn),
            &params(PAD_POLY_SAMPLES),
            L11_MAX_CHORD_ERROR_MM,
        ) {
            eps_hole = bound;
            eps_certified = true;
        } else {
            let h = 2.0 * PI / PAD_POLY_SAMPLES as f64;
            for i in 0..PAD_POLY_SAMPLES {
                let (p, q, m) = (
                    f(h * i as f64),
                    f(h * (i as f64 + 1.0)),
                    f(h * (i as f64 + 0.5)),
                );
                let c = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
                eps_hole = eps_hole.max((m[0] - c[0]).hypot(m[1] - c[1]));
            }
        }
    }
    let xs = if lateral {
        (ro_main * ro_main - e * e).max(0.0).sqrt() * ca_sa.0 / ca_sa.1
    } else {
        0.0
    };
    let mut eps = eps_hole;
    match s.outline {
        PadOutline::Circle(dp) => {
            if thick {
                let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
                for q in &inn {
                    y0 = y0.min(q[1]);
                    y1 = y1.max(q[1]);
                }
                let uc = rp * clamp1(e / ro_main).asin();
                let xc = (y0 + y1) / 2.0;
                for q in &inn {
                    let th = (q[0] - uc).atan2(q[1] - xc);
                    out.push([
                        rp * clamp1((e + dp / 2.0 * th.sin()) / rt).asin(),
                        xs + dp / 2.0 * th.cos(),
                    ]);
                }
            } else {
                for i in 0..PAD_POLY_SAMPLES {
                    let th = 2.0 * PI * i as f64 / PAD_POLY_SAMPLES as f64;
                    out.push([
                        rp * clamp1((e + dp / 2.0 * th.sin()) / rt).asin(),
                        dp / 2.0 * th.cos(),
                    ]);
                }
                let curve = PadCircleCurve {
                    rp,
                    rt,
                    e,
                    rad: dp / 2.0,
                    xs: 0.0,
                };
                if let Some(bound) = certified_rep(
                    &curve,
                    &closed(&out),
                    &params(PAD_POLY_SAMPLES),
                    L11_MAX_CHORD_ERROR_MM,
                ) {
                    eps = eps.max(bound);
                } else {
                    eps_certified = false;
                }
            }
        }
        PadOutline::Width(b) => {
            let (i2, o2) = offset_ring(&inn, b);
            inn = i2;
            out = o2;
            // The offset of a certified polyline by b has the same chord error plus the exact chord
            // error of each corner arc of radius b and at most PAD_ARC_STEP_DEG.
            let arc_bound = b * (1.0 - (PAD_ARC_STEP_DEG * PI / 360.0).cos()) + 1e-12;
            // The outline is the offset of the drawn hole polyline, so the hole's chord error and the
            // corner arc's own chord error add; taking the maximum would understate the total.
            eps = eps_hole + arc_bound;
            // The offset outline has no independent recompute in nf-digest: it is not certified, so no cut
            // file is served for it.
            eps_certified = false;
        }
    }
    // the hole must lie inside the outline (a plan-view circle can fail to enclose a long lateral hole)
    {
        let mut closed = out.clone();
        if let Some(f) = out.first() {
            closed.push(*f);
        }
        let wp = 2.0 * PI * rp;
        let inside = |q: [f64; 2]| {
            [-wp, 0.0, wp]
                .iter()
                .any(|sh| crate::tee::in_poly([q[0] + sh, q[1]], &closed))
        };
        if !inn.iter().all(|q| inside(*q)) {
            return Err(refuse(
                "pad_outline_not_around_hole",
                "outline",
                "the pad outline does not enclose the hole (widen the pad or give a larger diameter)",
            ));
        }
    }
    let (hole_w, hole_h) = bbox(&inn);
    let mut pts = out.clone();
    pts.push(out[0]);
    let (w, h) = bbox(&pts);
    let net_area = shoelace(&pts) - shoelace(&inn);
    let mass = mass_kg(net_area, s.t);
    if !(net_area.is_finite() && net_area > 0.0 && mass.is_finite()) {
        return Err(refuse(
            "pad_result_not_finite",
            "d",
            "pad area is not a finite positive number",
        ));
    }
    let exact = if eps_certified && !thick {
        let base = vec![
            ("main_d", s.main_d),
            ("t", s.t),
            ("branch_d", s.d),
            ("e", e),
        ];
        let mut v = vec![crate::tee::CurveDecl {
            on_hole: true,
            index: 0,
            kind: "pad_hole",
            params: base.clone(),
        }];
        if let PadOutline::Circle(dp) = s.outline {
            let mut params = base;
            params.push(("dp", dp));
            v.push(crate::tee::CurveDecl {
                on_hole: false,
                index: 0,
                kind: "pad_circle",
                params,
            });
        }
        v
    } else {
        Vec::new()
    };
    let mut res = PadResult {
        outline: pts,
        hole: inn.clone(),
        w,
        h,
        hole_w,
        hole_h,
        net_area,
        mass_kg: mass,
        thick_hole: thick,
        split,
        cut: "none",
        pieces: vec![],
        seams: vec![],
        seam_angle_deg: 90.0,
        cuts: vec![],
        center: [rp * clamp1(e / ro_main).asin(), xs],
        pieces_mass_kg: mass,
        gross_box_mass_kg: mass_kg(w * h, s.t),
        exact,
        eps_rep: eps,
        eps_rep_certified: eps_certified,
        certified: eps_certified && eps <= L11_MAX_CHORD_ERROR_MM,
        method: PAD_METHOD,
        rules: &PAD_NAMED_RULES,
    };
    if split > 1 {
        let Some(sp) = pad_split(&inn, &out, res.center, split, s.cut) else {
            return Err(refuse(
                "pad_split_not_possible",
                "split",
                format!(
                    "a split line misses or crosses the hole or the outline more than once: a {split}-way split cannot be cut here"
                ),
            ));
        };
        let kind = if split == 2 {
            if s.cut == SplitCut::Along {
                "along"
            } else {
                "cross"
            }
        } else {
            "even"
        };
        // the perpendicular two-way cross split gives two pieces of one shape (the second turned over): one is returned
        let pieces: Vec<PadPiece> = if kind == "cross" && !lateral {
            vec![piece_of(sp.pieces[0].clone(), 2, s.t)]
        } else {
            sp.pieces
                .iter()
                .map(|p| piece_of(p.clone(), 1, s.t))
                .collect()
        };
        res.pieces_mass_kg = pieces.iter().map(|p| p.mass_kg * f64::from(p.copies)).sum();
        res.gross_box_mass_kg = pieces
            .iter()
            .map(|p| mass_kg(p.w * p.h, s.t) * f64::from(p.copies))
            .sum();
        res.pieces = pieces;
        res.cut = kind;
        res.seams = sp.seams;
        res.seam_angle_deg = sp.seam_angle;
        res.cuts = sp.cuts;
    }
    let finite = res.w.is_finite()
        && res.h.is_finite()
        && res.pieces_mass_kg.is_finite()
        && res
            .outline
            .iter()
            .chain(res.hole.iter())
            .all(|p| p[0].is_finite() && p[1].is_finite())
        && res
            .pieces
            .iter()
            .all(|p| p.pts.iter().all(|q| q[0].is_finite() && q[1].is_finite()));
    if !finite {
        return Err(refuse(
            "pad_result_not_finite",
            "d",
            "a pad quantity is not finite",
        ));
    }
    Ok(res)
}
