//! NFJ-1 S1-L5b: closed-form development geometry for the zumen-kirizu (ZK) shapes disc/ring, plate, rib, concentric
//! and eccentric cone split into n pieces, and the split cylinder. Pure functions, no I/O, no engine state.
//!
//! Status (stated, not hidden): these functions are NOT reachable from `job::run`. No `Request` variant, no `Pattern`,
//! no DXF and no AMOEBA digest is produced from them yet, so nothing computed here can be served as a cut file. A shape
//! becomes servable only when a later lane wires it into the engine with its own verify function and certified
//! `eps_rep` (owner rule of 24 Nov 2026: a shape whose DXF cannot be certified stays withheld).
//!
//! Exactness: ring, plate (lines and true circles), cylinder and the concentric cone are exact closed forms. The
//! eccentric cone angle and area use composite Simpson quadrature of a smooth integrand (error bound stated at
//! `ECC_QUAD_PANELS`), not interval arithmetic: that part is NOT certified.
//!
//! Every value is either from the request, derived by a named formula, or a named rule. The only rule constant here
//! is `STEEL_DENSITY_KG_MM3` (7850 kg/m3, the nominal handbook value ZK uses, `RHO` in shared.js): a named rule
//! labelled UNVERIFIED for the factory's material; it is not read from a drawing.

use std::f64::consts::PI;

/// UNVERIFIED rule: nominal carbon-steel density 7.85e-6 kg/mm3 (same value as ZK `RHO`).
pub const STEEL_DENSITY_KG_MM3: f64 = 7.85e-6;
/// Largest value accepted for any dimension (mm); same implementation limit as `job::MAX_MAGNITUDE_MM`.
pub const MAX_MM: f64 = crate::job::MAX_MAGNITUDE_MM;
/// Smallest positive dimension accepted (mm); same as `job::MIN_DIMENSION_MM`.
pub const MIN_MM: f64 = crate::job::MIN_DIMENSION_MM;
/// Implementation limit: a ring or cone is split into 1..=12 pieces (ZK `splitN`/stock search maximum).
pub const MAX_SPLIT: u32 = 12;
/// Implementation limit: a cylinder has 1..=6 longitudinal seams (ZK `seamsN`).
pub const MAX_CYL_SEAMS: u32 = 6;
/// Composite Simpson panels for the eccentric cone integrals. The integrands are analytic on [0, 2 pi] with
/// bounded derivatives of the size of the radii; the Simpson error is below 1e-9 of the result for every
/// accepted cone (checked against an independent chord construction in the tests). Not an interval bound.
pub const ECC_QUAD_PANELS: usize = 4096;

/// Stable refusal codes of this module (documented in docs/backend/REFUSAL_CODES.md when wired into the engine).
pub const SHAPE_REFUSAL_CODES: [&str; 12] = [
    "shape_value_not_finite",
    "shape_value_not_positive",
    "shape_value_too_large",
    "ring_id_not_below_od",
    "shape_split_out_of_range",
    "cone_not_conical",
    "shape_wall_too_thick",
    "plate_taper_offset_missing",
    "plate_chamfer_too_long",
    "plate_chamfer_corner_invalid",
    "plate_hole_invalid",
    "rib_geometry_invalid",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeRefusal {
    pub code: &'static str,
    pub fields: Vec<&'static str>,
    pub message: String,
}

fn refuse(code: &'static str, field: &'static str, message: impl Into<String>) -> ShapeRefusal {
    ShapeRefusal {
        code,
        fields: vec![field],
        message: message.into(),
    }
}

/// A finite dimension in (MIN_MM, MAX_MM].
fn dim(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "shape_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v <= 0.0 || v < MIN_MM {
        return Err(refuse(
            "shape_value_not_positive",
            name,
            format!("{name} must be greater than 0 (got {v})"),
        ));
    }
    if v > MAX_MM {
        return Err(refuse(
            "shape_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

/// A finite value in [0, MAX_MM] (zero allowed).
fn dim0(name: &'static str, v: f64) -> Result<f64, ShapeRefusal> {
    if !v.is_finite() {
        return Err(refuse(
            "shape_value_not_finite",
            name,
            format!("{name} is not a finite number"),
        ));
    }
    if v < 0.0 {
        return Err(refuse(
            "shape_value_not_positive",
            name,
            format!("{name} must not be negative (got {v})"),
        ));
    }
    if v > MAX_MM {
        return Err(refuse(
            "shape_value_too_large",
            name,
            format!("{name} exceeds the limit {MAX_MM} mm (got {v})"),
        ));
    }
    Ok(v)
}

fn split_n(name: &'static str, n: u32, max: u32) -> Result<u32, ShapeRefusal> {
    if (1..=max).contains(&n) {
        Ok(n)
    } else {
        Err(refuse(
            "shape_split_out_of_range",
            name,
            format!("{name} must be 1..={max} (got {n})"),
        ))
    }
}

/// Mass in kg of `area_mm2` of plate `t` mm thick.
pub fn mass_kg(area_mm2: f64, t: f64) -> f64 {
    area_mm2 * t * STEEL_DENSITY_KG_MM3
}

// ---------------------------------------------------------------- ring / disc

#[derive(Debug, Clone, Copy)]
pub struct RingSpec {
    pub od: f64,
    /// 0 = disc (flange cover): no hole.
    pub id: f64,
    pub t: f64,
    pub split: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RingPiece {
    /// Start and end angle (rad) of the annular sector this piece covers.
    pub a0: f64,
    pub a1: f64,
    pub area: f64,
    /// True only for an unsplit ring with ID > 0 (the inner circle is a hole, not part of the outline).
    pub has_hole: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RingResult {
    pub pieces: Vec<RingPiece>,
    pub area_total: f64,
    pub mass_kg: f64,
}

/// Ring or disc, optionally cut into `split` equal annular sectors. Exact: area = pi/4 (OD^2 - ID^2) (a polygon
/// drawn with chords would be smaller by up to the chord error; the drawn curve is a true arc).
pub fn ring(s: RingSpec) -> Result<RingResult, ShapeRefusal> {
    let od = dim("OD", s.od)?;
    let id = dim0("ID", s.id)?;
    let t = dim("t", s.t)?;
    let n = split_n("split", s.split, MAX_SPLIT)?;
    if id >= od {
        return Err(refuse(
            "ring_id_not_below_od",
            "ID",
            format!("ID {id} must be smaller than OD {od}"),
        ));
    }
    let total = PI / 4.0 * (od * od - id * id);
    let nf = f64::from(n);
    let pieces = (0..n)
        .map(|k| RingPiece {
            a0: 2.0 * PI * f64::from(k) / nf,
            a1: 2.0 * PI * f64::from(k + 1) / nf,
            area: total / nf,
            has_hole: n == 1 && id > 0.0,
        })
        .collect();
    Ok(RingResult {
        pieces,
        area_total: total,
        mass_kg: mass_kg(total, t),
    })
}

// ---------------------------------------------------------------- cylinder

#[derive(Debug, Clone, Copy)]
pub struct CylSpec {
    pub d: f64,
    pub t: f64,
    pub l: f64,
    /// Number of longitudinal seams = number of equal plates (1 = one plate rolled).
    pub seams: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CylResult {
    /// pi (D - t): circumference at the plate mid surface (named rule: circumference at mid, DESIGN 2.4).
    pub circ: f64,
    pub plates: u32,
    pub plate_w: f64,
    pub plate_h: f64,
    pub area_total: f64,
    pub mass_kg: f64,
}

pub fn cylinder(s: CylSpec) -> Result<CylResult, ShapeRefusal> {
    let d = dim("D", s.d)?;
    let t = dim("t", s.t)?;
    let l = dim("L", s.l)?;
    let n = split_n("split", s.seams, MAX_CYL_SEAMS)?;
    if t >= d / 2.0 {
        return Err(refuse(
            "shape_wall_too_thick",
            "t",
            format!("t {t} must be below D/2 = {}", d / 2.0),
        ));
    }
    let circ = PI * (d - t);
    Ok(CylResult {
        circ,
        plates: n,
        plate_w: circ / f64::from(n),
        plate_h: l,
        area_total: circ * l,
        mass_kg: mass_kg(circ * l, t),
    })
}

// ---------------------------------------------------------------- stock fit

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stock {
    pub w: f64,
    pub l: f64,
}

/// Index of the first stock sheet that holds a w x h piece in 0 or 90 degrees with `margin` mm free on every edge.
/// ZK `fitStock` (shared.js:841). Non-finite sizes never fit.
pub fn fit_stock(w: f64, h: f64, stock: &[Stock], margin: f64) -> Option<usize> {
    if !(w.is_finite() && h.is_finite() && margin.is_finite()) {
        return None;
    }
    let (a, b) = (w.min(h), w.max(h));
    stock.iter().position(|s| {
        a <= s.w.min(s.l) - 2.0 * margin + 1e-9 && b <= s.w.max(s.l) - 2.0 * margin + 1e-9
    })
}

/// Fewest longitudinal seams (starting at `from`, at most `MAX_CYL_SEAMS`) for which every equal plate fits some
/// stock sheet. None when no seam count up to the limit fits. Used by the hold "pipe does not fit the plate".
pub fn min_cyl_seams(
    d: f64,
    t: f64,
    l: f64,
    from: u32,
    stock: &[Stock],
    margin: f64,
) -> Result<Option<u32>, ShapeRefusal> {
    for k in from.max(1)..=MAX_CYL_SEAMS {
        let c = cylinder(CylSpec { d, t, l, seams: k })?;
        if fit_stock(c.plate_w, c.plate_h, stock, margin).is_some() {
            return Ok(Some(k));
        }
    }
    cylinder(CylSpec { d, t, l, seams: 1 })?;
    Ok(None)
}

// ---------------------------------------------------------------- concentric cone split

/// Which surface the entered diameters are measured on; the cone is developed at that surface shifted to the
/// plate mid surface (named rule, DESIGN 2.4: `mid` default). `Outer`: diameters are outer; `Inner`: inner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    Outer,
    Mid,
    Inner,
}

impl Basis {
    /// Radial offset from the entered (outer) radius to the surface that is developed.
    fn offset(self, t: f64) -> f64 {
        match self {
            Basis::Outer => 0.0,
            Basis::Mid => t / 2.0,
            Basis::Inner => t,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConeSpec {
    pub d1: f64,
    pub d2: f64,
    pub l: f64,
    pub t: f64,
    pub basis: Basis,
    pub split: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConeSplit {
    /// Slant radii of the large and small arc (distance from the development apex).
    pub rl: f64,
    pub rs: f64,
    /// Development angle of one piece (rad) = 2 pi sin(alpha) / n.
    pub piece_angle: f64,
    /// Chord of the large arc and height of the piece (ZK `summary` w and h for a flare).
    pub chord: f64,
    pub height: f64,
    pub piece_area: f64,
    pub n: u32,
    pub mass_kg: f64,
}

/// Concentric cone cut into n identical annular sectors (n >= 2; ZK default 2). Exact closed form:
/// slant s = hypot(L, rl - rs), sin a = (rl - rs)/s, Rl = rl/sin a, Rs = rs/sin a, piece angle = 2 pi sin a / n.
pub fn cone_split(c: ConeSpec) -> Result<ConeSplit, ShapeRefusal> {
    let d1 = dim("D1", c.d1)?;
    let d2 = dim("D2", c.d2)?;
    let l = dim("L", c.l)?;
    let t = dim("t", c.t)?;
    let n = split_n("split", c.split, MAX_SPLIT)?.max(2);
    let o = c.basis.offset(t);
    let (r1, r2) = (d1 / 2.0 - o, d2 / 2.0 - o);
    if r1 <= 0.0 || r2 <= 0.0 {
        return Err(refuse(
            "shape_wall_too_thick",
            "t",
            "the plate is thicker than the radius of one mouth",
        ));
    }
    let (rs, rl) = if r1 <= r2 { (r1, r2) } else { (r2, r1) };
    let sl = l.hypot(rl - rs);
    let sin_a = (rl - rs) / sl;
    if sin_a < 1e-9 {
        return Err(refuse(
            "cone_not_conical",
            "D2",
            "D1 and D2 are equal: this is a cylinder, not a cone",
        ));
    }
    let (big, small) = (rl / sin_a, rs / sin_a);
    let ang = 2.0 * PI * sin_a / f64::from(n);
    let area = 0.5 * ang * (big * big - small * small);
    Ok(ConeSplit {
        rl: big,
        rs: small,
        piece_angle: ang,
        chord: 2.0 * big * (ang / 2.0).sin(),
        height: big - small * (ang / 2.0).cos(),
        piece_area: area,
        n,
        mass_kg: mass_kg(area * f64::from(n), t),
    })
}

// ---------------------------------------------------------------- eccentric cone split

#[derive(Debug, Clone, PartialEq)]
pub struct EccPiece {
    /// Azimuth range (rad) on the large mouth, from the flat side.
    pub a0: f64,
    pub a1: f64,
    /// Angle between the first and last generator of the piece in the development (rad).
    pub dev_angle: f64,
    pub area: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EccSplit {
    pub pieces: Vec<EccPiece>,
    pub area_total: f64,
    pub mass_kg: f64,
}

/// Eccentric (oblique) cone of two parallel circles with a common tangent line ("flat side"), cut into n pieces by
/// generators at equal azimuth steps from the flat side (n = 2 gives the two halves). The development maps azimuth
/// a to polar angle phi(a) about the apex with d phi/da = sqrt(r1^2 - rho'^2)/rho, rho the distance apex to the
/// large-mouth point (exact generator mapping; arc length of the mouth circle is preserved). Area of a piece is
/// (1 - k^2)/2 * integral rho^2 d phi, k = r2/r1. NOT certified (quadrature, see `ECC_QUAD_PANELS`).
pub fn ecc_cone_split(c: ConeSpec) -> Result<EccSplit, ShapeRefusal> {
    let d1 = dim("D1", c.d1)?;
    let d2 = dim("D2", c.d2)?;
    let l = dim("L", c.l)?;
    let t = dim("t", c.t)?;
    let n = split_n("split", c.split, MAX_SPLIT)?.max(2);
    let o = c.basis.offset(t);
    let (ra, rb) = (d1 / 2.0 - o, d2 / 2.0 - o);
    if ra <= 0.0 || rb <= 0.0 {
        return Err(refuse(
            "shape_wall_too_thick",
            "t",
            "the plate is thicker than the radius of one mouth",
        ));
    }
    // large mouth first (ZK flips by naming; the development is the same)
    let (r1, r2) = if ra >= rb { (ra, rb) } else { (rb, ra) };
    if (r1 - r2).abs() < 1e-9 * r1 {
        return Err(refuse(
            "cone_not_conical",
            "D2",
            "D1 and D2 are equal: this is a cylinder, not a cone",
        ));
    }
    let k = r2 / r1;
    // homothety centre (apex): x = L/(1-k), y = (r1-r2)/(1-k); circle 1 at x = 0 centred on the axis
    let ay = (r1 - r2) / (1.0 - k);
    let ax = l / (1.0 - k);
    let rho2 = |a: f64| ax * ax + ay * ay - 2.0 * ay * r1 * a.cos() + r1 * r1;
    let dphi = |a: f64| {
        let rho = rho2(a).sqrt();
        let drho = ay * r1 * a.sin() / rho;
        (r1 * r1 - drho * drho).max(0.0).sqrt() / rho
    };
    let darea = |a: f64| 0.5 * (1.0 - k * k) * rho2(a) * dphi(a);
    let simpson = |f: &dyn Fn(f64) -> f64, a0: f64, a1: f64| {
        let m = ECC_QUAD_PANELS;
        let h = (a1 - a0) / m as f64;
        let mut s = f(a0) + f(a1);
        for i in 1..m {
            s += f(a0 + h * i as f64) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        s * h / 3.0
    };
    let nf = f64::from(n);
    let pieces: Vec<EccPiece> = (0..n)
        .map(|j| {
            let (a0, a1) = (
                2.0 * PI * f64::from(j) / nf,
                2.0 * PI * f64::from(j + 1) / nf,
            );
            // ZK cuts a 2-split at pi (flat side to far side): the same azimuth rule for n = 2
            EccPiece {
                a0,
                a1,
                dev_angle: simpson(&dphi, a0, a1),
                area: simpson(&darea, a0, a1),
            }
        })
        .collect();
    let area_total: f64 = pieces.iter().map(|p| p.area).sum();
    Ok(EccSplit {
        mass_kg: mass_kg(area_total, t),
        area_total,
        pieces,
    })
}

// ---------------------------------------------------------------- plate

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleGrid {
    pub d: f64,
    pub x: f64,
    pub y: f64,
    pub nx: u32,
    pub ny: u32,
    pub px: f64,
    pub py: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateSpec {
    pub w: f64,
    pub h: f64,
    /// Top edge length: None = rectangle; 0 = triangle; otherwise trapezoid.
    pub w2: Option<f64>,
    /// Offset of the top edge's left end from the bottom edge's left end (None only allowed for a rectangle or a
    /// triangle; a triangle without it gets 0, a right angle at the lower left, and says so in `defaulted_off`).
    pub off: Option<f64>,
    pub t: f64,
    /// Chamfer C (equal length along both edges from the corner). None / 0 = none.
    pub c: Option<f64>,
    /// 0 = every corner, 1.. = that corner only (1 lower left, 2 lower right, 3 upper right, 4 upper left;
    /// a triangle has 3: apex).
    pub c_at: u32,
    pub hole: Option<HoleGrid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlateKind {
    Rectangle,
    Trapezoid,
    Triangle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plate {
    pub kind: PlateKind,
    /// Corners before chamfering (counter-clockwise from the lower left).
    pub corners: Vec<[f64; 2]>,
    /// Outline vertices after chamfering (counter-clockwise, open: the last joins the first).
    pub outline: Vec<[f64; 2]>,
    pub hole_centres: Vec<[f64; 2]>,
    pub hole_d: f64,
    pub defaulted_off: bool,
    pub outline_area: f64,
    /// Area after the holes (exact circles).
    pub area: f64,
    pub perimeter: f64,
    /// Bounding box of the outline (ZK summary w and h) and of the corners (the blank the plate is cut from).
    pub w: f64,
    pub h: f64,
    pub blank_w: f64,
    pub blank_h: f64,
    pub mass_kg: f64,
    /// Mass of the blank rectangle round the unchamfered corners.
    pub blank_kg: f64,
}

fn shoelace_signed(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

fn bbox(p: &[[f64; 2]]) -> (f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in p {
        x0 = x0.min(q[0]);
        x1 = x1.max(q[0]);
        y0 = y0.min(q[1]);
        y1 = y1.max(q[1]);
    }
    (x1 - x0, y1 - y0)
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Distance from point q to the closed polygon's edges.
pub fn edge_distance(q: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    let mut m = f64::INFINITY;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let l2 = (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
        let s = if l2 > 0.0 {
            (((q[0] - a[0]) * (b[0] - a[0]) + (q[1] - a[1]) * (b[1] - a[1])) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        m = m.min((q[0] - a[0] - s * (b[0] - a[0])).hypot(q[1] - a[1] - s * (b[1] - a[1])));
    }
    m
}

/// Even-odd point in polygon.
pub fn inside_polygon(q: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    let mut c = false;
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

/// Corners of the plate before chamfers, counter-clockwise from the lower left (ZK `plateCorners`).
pub fn plate_corners(w: f64, h: f64, w2: Option<f64>, off: Option<f64>) -> Vec<[f64; 2]> {
    let top = w2.unwrap_or(w);
    let o = if w2.is_none() {
        0.0
    } else {
        off.unwrap_or(0.0)
    };
    if top == 0.0 {
        vec![[0.0, 0.0], [w, 0.0], [o, h]]
    } else {
        vec![[0.0, 0.0], [w, 0.0], [o + top, h], [o, h]]
    }
}

/// Plate with straight edges, C chamfers and a grid of round holes. Exact: lines and true circles; the area is the
/// shoelace area of the straight outline minus n x pi d^2/4 (ZK draws each hole as a 360-gon, 0.0035 % smaller).
pub fn plate(s: PlateSpec) -> Result<Plate, ShapeRefusal> {
    let w = dim("plate_W", s.w)?;
    let h = dim("plate_H", s.h)?;
    let t = dim("t", s.t)?;
    let w2 = match s.w2 {
        Some(v) => Some(dim0("plate_W2", v)?),
        None => None,
    };
    let off = match s.off {
        Some(v) => {
            if !v.is_finite() || v.abs() > MAX_MM {
                return Err(refuse(
                    "shape_value_not_finite",
                    "plate_off",
                    "plate_off is not a finite number within the limit",
                ));
            }
            Some(v)
        }
        None => None,
    };
    let tri = w2 == Some(0.0);
    let trap = w2.is_some_and(|v| v != 0.0 && (v - w).abs() > 1e-9);
    if trap && off.is_none() {
        return Err(refuse(
            "plate_taper_offset_missing",
            "plate_off",
            "a trapezoid needs the offset of its top edge (0 = square at the left, (W - W2)/2 = symmetric)",
        ));
    }
    let defaulted_off = tri && off.is_none();
    let off_eff = if defaulted_off { Some(0.0) } else { off };
    let kind = if tri {
        PlateKind::Triangle
    } else if trap || off_eff.is_some_and(|o| o != 0.0 && w2.is_some()) {
        PlateKind::Trapezoid
    } else {
        PlateKind::Rectangle
    };
    let corners = plate_corners(w, h, w2, off_eff);
    let nc = corners.len();
    let outline_area_raw = shoelace_signed(&corners);
    if outline_area_raw.abs() < MIN_MM {
        return Err(refuse(
            "shape_value_not_positive",
            "plate_W2",
            "the outline has no area",
        ));
    }
    // chamfers
    let c = match s.c {
        Some(v) if v != 0.0 => Some(dim("plate_C", v)?),
        _ => None,
    };
    let mut raw: Vec<[f64; 2]> = Vec::with_capacity(nc * 2);
    if let Some(c) = c {
        if s.c_at as usize > nc {
            return Err(refuse(
                "plate_chamfer_corner_invalid",
                "plate_C_at",
                format!("corner {} does not exist (0..={nc})", s.c_at),
            ));
        }
        let on = |i: usize| s.c_at == 0 || s.c_at as usize == i + 1;
        for i in 0..nc {
            let j = (i + 1) % nc;
            if (if on(i) { c } else { 0.0 }) + (if on(j) { c } else { 0.0 })
                > dist(corners[i], corners[j]) + 1e-6
            {
                return Err(refuse(
                    "plate_chamfer_too_long",
                    "plate_C",
                    format!("C{c} is longer than the edge it sits on, or two chamfers overlap"),
                ));
            }
        }
        for (i, p) in corners.iter().enumerate() {
            if !on(i) {
                raw.push(*p);
                continue;
            }
            let a = corners[(i + nc - 1) % nc];
            let b = corners[(i + 1) % nc];
            let (la, lb) = (dist(a, *p), dist(b, *p));
            raw.push([p[0] + c * (a[0] - p[0]) / la, p[1] + c * (a[1] - p[1]) / la]);
            raw.push([p[0] + c * (b[0] - p[0]) / lb, p[1] + c * (b[1] - p[1]) / lb]);
        }
    } else {
        raw.extend_from_slice(&corners);
    }
    // drop zero-length edges (a chamfer equal to the whole edge)
    let mut outline: Vec<[f64; 2]> = Vec::new();
    for i in 0..raw.len() {
        let p = raw[(i + raw.len() - 1) % raw.len()];
        if dist(raw[i], p) > 1e-9 {
            outline.push(raw[i]);
        }
    }
    if outline.len() < 3 {
        return Err(refuse(
            "plate_chamfer_too_long",
            "plate_C",
            "the chamfers leave no outline",
        ));
    }
    // orientation: counter-clockwise (the lower-left start with W > 0, H > 0 gives it unless off makes it clockwise)
    if shoelace_signed(&outline) < 0.0 {
        outline.reverse();
    }
    let outline_area = shoelace_signed(&outline);
    // holes
    let mut centres = Vec::new();
    let mut hole_d = 0.0;
    if let Some(g) = s.hole {
        let d = dim("hole_d", g.d)?;
        if !(g.x.is_finite() && g.y.is_finite() && g.px.is_finite() && g.py.is_finite())
            || g.nx == 0
            || g.ny == 0
        {
            return Err(refuse(
                "plate_hole_invalid",
                "hole_x",
                "hole position, pitch and counts must be finite and the counts at least 1",
            ));
        }
        if u64::from(g.nx) * u64::from(g.ny) > 10_000 || g.px.abs() > MAX_MM || g.py.abs() > MAX_MM
        {
            return Err(refuse(
                "plate_hole_invalid",
                "hole_nx",
                "too many holes or pitch too large",
            ));
        }
        hole_d = d;
        for j in 0..g.ny {
            for i in 0..g.nx {
                centres.push([g.x + f64::from(i) * g.px, g.y + f64::from(j) * g.py]);
            }
        }
    }
    let area = outline_area - centres.len() as f64 * PI * hole_d * hole_d / 4.0;
    let perimeter: f64 = (0..outline.len())
        .map(|i| dist(outline[i], outline[(i + 1) % outline.len()]))
        .sum();
    let (bw, bh) = bbox(&outline);
    let (kw, kh) = bbox(&corners);
    Ok(Plate {
        kind,
        corners,
        outline,
        hole_centres: centres,
        hole_d,
        defaulted_off,
        outline_area,
        area,
        perimeter,
        w: bw,
        h: bh,
        blank_w: kw,
        blank_h: kh,
        mass_kg: mass_kg(area, t),
        blank_kg: mass_kg(kw * kh, t),
    })
}

/// Holes that do not lie fully inside the outline (centre outside, or closer to an edge than d/2).
pub fn holes_outside(p: &Plate) -> Vec<[f64; 2]> {
    p.hole_centres
        .iter()
        .copied()
        .filter(|q| {
            !inside_polygon(*q, &p.outline) || edge_distance(*q, &p.outline) < p.hole_d / 2.0 - 1e-6
        })
        .collect()
}

// ---------------------------------------------------------------- rib along the main pipe's outer circle

#[derive(Debug, Clone, Copy)]
pub struct RibSpec {
    /// Outer radius of the main pipe (the rib edge follows this circle).
    pub ro: f64,
    /// Signed distance from the rib's base edge to the main pipe's centre line, along the rib height.
    pub vc: f64,
    pub b: f64,
    pub h: f64,
    /// Width of the flat top (0 = pointed).
    pub top: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rib {
    pub area: f64,
    /// x of the upper corner on the main-pipe circle (ZK `info.xC`).
    pub x_c: f64,
    /// x of the base corner on the main-pipe circle (ZK `info.u0`).
    pub u0: f64,
}

/// Rib bounded by the base edge (y = 0, 0..b), a straight edge to the top, the flat top and the arc of the main pipe.
/// Exact: area = h (b + xa(h) + top)/2 - integral_0^h xa(v) dv, with xa(v) = sqrt(Ro^2 - (v - vc)^2) - u0 and the
/// integral in closed form (circular segment).
pub fn rib(s: RibSpec) -> Result<Rib, ShapeRefusal> {
    let ro = dim("main_D", s.ro * 2.0)? / 2.0;
    let b = dim("rib_b", s.b)?;
    let h = dim("rib_h", s.h)?;
    let top = dim0("rib_top", s.top)?;
    if !s.vc.is_finite() || s.vc.abs() > MAX_MM {
        return Err(refuse(
            "shape_value_not_finite",
            "rib_vc",
            "rib position is not a finite number within the limit",
        ));
    }
    // the whole height must lie on the circle: |v - vc| <= Ro for v in [0, h]
    if s.vc.abs() >= ro || (h - s.vc).abs() >= ro {
        return Err(refuse(
            "rib_geometry_invalid",
            "rib_h",
            "the rib does not meet the main pipe's outer circle over its whole height",
        ));
    }
    let u0 = (ro * ro - s.vc * s.vc).sqrt();
    let xa = |v: f64| (ro * ro - (v - s.vc) * (v - s.vc)).max(0.0).sqrt() - u0;
    // F(w) = antiderivative of sqrt(Ro^2 - w^2)
    let big_f = |w: f64| {
        0.5 * (w * (ro * ro - w * w).max(0.0).sqrt() + ro * ro * (w / ro).clamp(-1.0, 1.0).asin())
    };
    let integral = big_f(h - s.vc) - big_f(-s.vc) - u0 * h;
    let x_c = xa(h);
    let area = h * (b + x_c + top) / 2.0 - integral;
    Ok(Rib { area, x_c, u0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() <= tol, "{a} vs {b} (tol {tol})");
    }

    #[test]
    fn disc_mass_is_the_exact_circle_area() {
        let r = ring(RingSpec {
            od: 800.0,
            id: 0.0,
            t: 16.0,
            split: 1,
        })
        .unwrap();
        close(r.area_total, PI * 400.0 * 400.0, 1e-6);
        close(r.mass_kg, 63.13, 0.01);
        assert!(!r.pieces[0].has_hole);
    }

    #[test]
    fn ring_split_pieces_add_up_and_reject_bad_input() {
        let r = ring(RingSpec {
            od: 1000.0,
            id: 400.0,
            t: 10.0,
            split: 3,
        })
        .unwrap();
        close(
            r.pieces.iter().map(|p| p.area).sum::<f64>(),
            PI / 4.0 * (1e6 - 16e4),
            1e-6,
        );
        assert!(r.pieces.iter().all(|p| !p.has_hole));
        assert_eq!(
            ring(RingSpec {
                od: 100.0,
                id: 100.0,
                t: 1.0,
                split: 1
            })
            .unwrap_err()
            .code,
            "ring_id_not_below_od"
        );
        assert_eq!(
            ring(RingSpec {
                od: f64::NAN,
                id: 0.0,
                t: 1.0,
                split: 1
            })
            .unwrap_err()
            .code,
            "shape_value_not_finite"
        );
        assert_eq!(
            ring(RingSpec {
                od: 100.0,
                id: 0.0,
                t: 1.0,
                split: 0
            })
            .unwrap_err()
            .code,
            "shape_split_out_of_range"
        );
    }

    #[test]
    fn cone_split_matches_verify_cone_split_values() {
        // verify_cone_split.py values (ZK test_build.mjs:100), t -> 0
        for (n, chord, h) in [
            (2, 1808.39, 785.33),
            (3, 1233.8, 701.18),
            (4, 932.82, 671.27),
        ] {
            let c = cone_split(ConeSpec {
                d1: 800.0,
                d2: 1200.0,
                l: 600.0,
                t: 0.001,
                basis: Basis::Mid,
                split: n,
            })
            .unwrap();
            close(c.chord, chord, 0.05);
            close(c.height, h, 0.05);
        }
    }

    #[test]
    fn cone_split_second_formulation_lateral_area() {
        // independent: lateral area of the frustum pi (r1 + r2) slant = n pieces x piece area
        let (d1, d2, l, t) = (800.0, 1200.0, 600.0, 10.0);
        for n in 2..=6 {
            let c = cone_split(ConeSpec {
                d1,
                d2,
                l,
                t,
                basis: Basis::Mid,
                split: n,
            })
            .unwrap();
            let (r1, r2) = (d1 / 2.0 - t / 2.0, d2 / 2.0 - t / 2.0);
            let lateral = PI * (r1 + r2) * l.hypot(r2 - r1);
            close(c.piece_area * f64::from(n), lateral, 1e-6 * lateral);
        }
        assert_eq!(
            cone_split(ConeSpec {
                d1: 500.0,
                d2: 500.0,
                l: 100.0,
                t: 5.0,
                basis: Basis::Mid,
                split: 2
            })
            .unwrap_err()
            .code,
            "cone_not_conical"
        );
    }

    /// ZK's own construction (chord triangulation of the strip, eccStrip) as an independent second formulation.
    fn zk_strip_angle(r1: f64, r2: f64, l: f64, a0: f64, a1: f64, nn: usize) -> f64 {
        let c2 = r1 - r2;
        let p1: Vec<[f64; 3]> = (0..=nn)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f64 / nn as f64;
                [0.0, r1 * a.cos(), r1 * a.sin()]
            })
            .collect();
        let p2: Vec<[f64; 3]> = (0..=nn)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f64 / nn as f64;
                [l, c2 + r2 * a.cos(), r2 * a.sin()]
            })
            .collect();
        let d = |u: [f64; 3], v: [f64; 3]| {
            ((u[0] - v[0]).powi(2) + (u[1] - v[1]).powi(2) + (u[2] - v[2]).powi(2)).sqrt()
        };
        let third = |p: [f64; 2], q: [f64; 2], lp: f64, lq: f64| {
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            let dd = dx.hypot(dy);
            let aa = (lp * lp - lq * lq + dd * dd) / (2.0 * dd);
            let hh = (lp * lp - aa * aa).max(0.0).sqrt();
            let (ux, uy) = (dx / dd, dy / dd);
            [p[0] + aa * ux + hh * uy, p[1] + aa * uy - hh * ux]
        };
        let mut a = vec![[0.0, 0.0]];
        let mut b = vec![[d(p1[0], p2[0]), 0.0]];
        for i in 0..nn {
            let na = third(a[i], b[i], d(p1[i], p1[i + 1]), d(p2[i], p1[i + 1]));
            a.push(na);
            let nb = third(na, b[i], d(p1[i + 1], p2[i + 1]), d(p2[i], p2[i + 1]));
            b.push(nb);
        }
        let u = [b[0][0] - a[0][0], b[0][1] - a[0][1]];
        let w = [b[nn][0] - a[nn][0], b[nn][1] - a[nn][1]];
        ((u[0] * w[0] + u[1] * w[1]) / u[0].hypot(u[1]) / w[0].hypot(w[1]))
            .acos()
            .to_degrees()
    }

    #[test]
    fn ecc_split_angles_match_verify_values_and_the_chord_construction() {
        let spec = |n| ConeSpec {
            d1: 1200.0,
            d2: 800.0,
            l: 700.0,
            t: 0.001,
            basis: Basis::Mid,
            split: n,
        };
        for (n, want) in [
            (2, vec![47.005]),
            (3, vec![32.101, 29.808, 32.101]),
            (4, vec![24.538, 22.467, 22.467, 24.538]),
        ] {
            let e = ecc_cone_split(spec(n)).unwrap();
            for (p, w) in e.pieces.iter().zip(&want) {
                close(p.dev_angle.to_degrees(), *w, 0.01);
                let a = zk_strip_angle(600.0 - 0.0005, 400.0 - 0.0005, 700.0, p.a0, p.a1, 2000);
                close(p.dev_angle.to_degrees(), a, 0.005);
            }
        }
    }

    #[test]
    fn ecc_split_total_area_does_not_depend_on_n_and_matches_the_cylinder_limit() {
        let mk = |n, d2| {
            ecc_cone_split(ConeSpec {
                d1: 1219.2,
                d2,
                l: 1200.0,
                t: 12.0,
                basis: Basis::Mid,
                split: n,
            })
            .unwrap()
        };
        let (a2, a3, a5) = (
            mk(2, 914.4).area_total,
            mk(3, 914.4).area_total,
            mk(5, 914.4).area_total,
        );
        close(a2, a3, 1e-9 * a2);
        close(a2, a5, 1e-9 * a2);
        // limit D2 -> D1: the oblique cone becomes a cylinder of circumference pi (D - t) and height L
        let near = mk(2, 1219.2 - 1e-3).area_total;
        close(near, PI * (1219.2 - 12.0) * 1200.0, 1e-5 * near);
    }

    #[test]
    fn plate_closed_forms() {
        // rectangle 300x80 with C10 on all corners: 24000 - 4 x 10^2/2 = 23800; perimeter 2(300+80) - 8 x 10 + 4 x 10 sqrt 2
        let p = plate(PlateSpec {
            w: 300.0,
            h: 80.0,
            w2: None,
            off: None,
            t: 12.0,
            c: Some(10.0),
            c_at: 0,
            hole: None,
        })
        .unwrap();
        close(p.area, 23800.0, 1e-9);
        close(
            p.perimeter,
            2.0 * (300.0 + 80.0) - 8.0 * 10.0 + 4.0 * 10.0 * 2f64.sqrt(),
            1e-9,
        );
        close(p.mass_kg, 23800.0 * 12.0 * 7.85e-6, 1e-12);
        // trapezoid 300 / 150, off 0, hole d22: (300+150)/2 x 200 - pi 11^2
        let t = plate(PlateSpec {
            w: 300.0,
            h: 200.0,
            w2: Some(150.0),
            off: Some(0.0),
            t: 9.0,
            c: None,
            c_at: 0,
            hole: Some(HoleGrid {
                d: 22.0,
                x: 60.0,
                y: 50.0,
                nx: 1,
                ny: 1,
                px: 0.0,
                py: 0.0,
            }),
        })
        .unwrap();
        close(t.area, 45000.0 - PI * 121.0, 1e-9);
        assert_eq!(t.kind, PlateKind::Trapezoid);
        // triangle 200x150, C15 at the right angle (corner 1): 15000 - 15^2/2; at the acute corner 2: sin of that angle
        let tri = |at| {
            plate(PlateSpec {
                w: 200.0,
                h: 150.0,
                w2: Some(0.0),
                off: Some(0.0),
                t: 9.0,
                c: Some(15.0),
                c_at: at,
                hole: None,
            })
            .unwrap()
        };
        close(tri(1).area, 15000.0 - 112.5, 1e-9);
        let acute = (150.0f64 / 250.0).asin();
        close(tri(2).area, 15000.0 - 0.5 * 225.0 * acute.sin(), 1e-9);
        close(tri(2).w, 188.0, 1e-9);
        // 2 x 2 holes
        let g = plate(PlateSpec {
            w: 400.0,
            h: 400.0,
            w2: None,
            off: None,
            t: 16.0,
            c: None,
            c_at: 0,
            hole: Some(HoleGrid {
                d: 24.0,
                x: 50.0,
                y: 50.0,
                nx: 2,
                ny: 2,
                px: 300.0,
                py: 300.0,
            }),
        })
        .unwrap();
        close(g.area, 160000.0 - 4.0 * PI * 144.0, 1e-9);
        assert!(holes_outside(&g).is_empty());
    }

    #[test]
    fn plate_refusals_are_coded_and_never_nan() {
        let base = PlateSpec {
            w: 300.0,
            h: 80.0,
            w2: None,
            off: None,
            t: 9.0,
            c: None,
            c_at: 0,
            hole: None,
        };
        assert_eq!(
            plate(PlateSpec {
                w2: Some(150.0),
                ..base
            })
            .unwrap_err()
            .code,
            "plate_taper_offset_missing"
        );
        assert_eq!(
            plate(PlateSpec {
                c: Some(10.0),
                c_at: 9,
                ..base
            })
            .unwrap_err()
            .code,
            "plate_chamfer_corner_invalid"
        );
        assert_eq!(
            plate(PlateSpec {
                h: 15.0,
                c: Some(10.0),
                c_at: 0,
                ..base
            })
            .unwrap_err()
            .code,
            "plate_chamfer_too_long"
        );
        assert_eq!(
            plate(PlateSpec { w: -1.0, ..base }).unwrap_err().code,
            "shape_value_not_positive"
        );
        assert_eq!(
            plate(PlateSpec {
                h: f64::INFINITY,
                ..base
            })
            .unwrap_err()
            .code,
            "shape_value_not_finite"
        );
        assert_eq!(
            plate(PlateSpec { t: 1e12, ..base }).unwrap_err().code,
            "shape_value_too_large"
        );
    }

    #[test]
    fn rib_closed_form_matches_a_fine_polyline() {
        // independent: shoelace area of the 4000-segment polygon
        let s = RibSpec {
            ro: 159.25,
            vc: 159.25 - 89.1,
            b: 60.0,
            h: 50.0,
            top: 0.0,
        };
        let r = rib(s).unwrap();
        let u0 = (s.ro * s.ro - s.vc * s.vc).sqrt();
        let xa = |v: f64| (s.ro * s.ro - (v - s.vc).powi(2)).sqrt() - u0;
        let mut pts = vec![[0.0, 0.0], [s.b, 0.0]];
        for k in 0..=4000 {
            let v = s.h * (1.0 - k as f64 / 4000.0);
            pts.push([xa(v), v]);
        }
        close(r.area, shoelace_signed(&pts).abs(), 1e-3);
        close(r.x_c, xa(s.h), 1e-12);
    }

    #[test]
    fn stock_fit_orientation_and_min_seams() {
        let s = [Stock {
            w: 2438.0,
            l: 6096.0,
        }];
        assert_eq!(fit_stock(4124.283, 1500.0, &s, 0.0), Some(0));
        assert_eq!(fit_stock(2749.522, 2500.0, &s, 0.0), None);
        assert_eq!(
            fit_stock(
                2062.142,
                2500.0,
                &[Stock {
                    w: 6096.0,
                    l: 2438.0
                }],
                0.0
            ),
            Some(0)
        );
        assert_eq!(fit_stock(f64::NAN, 1.0, &s, 0.0), None);
        assert_eq!(
            min_cyl_seams(2641.6, 16.0, 1500.0, 1, &s, 0.0).unwrap(),
            Some(2)
        );
        for (w, k) in [(1524.0, 6), (1829.0, 5), (2438.0, 4)] {
            assert_eq!(
                min_cyl_seams(2641.6, 16.0, 2500.0, 1, &[Stock { w, l: 6096.0 }], 0.0).unwrap(),
                Some(k)
            );
        }
        assert_eq!(
            min_cyl_seams(2641.6, 16.0, 7000.0, 1, &s, 0.0).unwrap(),
            None
        );
    }

    #[test]
    fn cylinder_closed_form() {
        let c = cylinder(CylSpec {
            d: 2641.6,
            t: 16.0,
            l: 1500.0,
            seams: 2,
        })
        .unwrap();
        close(c.circ, PI * 2625.6, 1e-9);
        close(c.plate_w, PI * 2625.6 / 2.0, 1e-9);
        close(c.mass_kg, PI * 2625.6 * 1500.0 * 16.0 * 7.85e-6, 1e-9);
        assert_eq!(
            cylinder(CylSpec {
                d: 100.0,
                t: 60.0,
                l: 10.0,
                seams: 1
            })
            .unwrap_err()
            .code,
            "shape_wall_too_thick"
        );
        assert_eq!(
            cylinder(CylSpec {
                d: 100.0,
                t: 1.0,
                l: 10.0,
                seams: 7
            })
            .unwrap_err()
            .code,
            "shape_split_out_of_range"
        );
    }
}
