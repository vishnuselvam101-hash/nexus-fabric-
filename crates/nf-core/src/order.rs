//! NFJ-1 S1-L5b: front half of the order builder for SINGLE parts (straight pipe, plate): which values are missing,
//! which are only stated, which make the part a HOLD, and the cross-check against the material table. Pure; no server
//! types. Port of the single-part branches of zumen-kirizu `build-core.mjs` (plateCheck :470, fillPlate :455,
//! parsePL :30, stock rule :1146, mass/size cross-check :1169-1195).
//!
//! A hold has a stable `code` (the Japanese sentence of ZK is not matched anywhere), a level, and the keys it
//! concerns. `Need` = the part is not computed or its DXF must be held for the engineer (HOLD-MISSING / HOLD-REVIEW,
//! DESIGN 2.6); `Info` = a default or source note; `Ok` = a cross-check passed. A PROPOSED value never reaches this
//! module: only confirmed values do (DESIGN 2.6). Like `shapes`, nothing here is reachable from `job::run` yet.

use crate::shapes::{self, CylSpec, HoleGrid, Plate, PlateSpec, Stock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Need,
    Info,
    Ok,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hold {
    pub level: Level,
    pub code: &'static str,
    pub keys: Vec<&'static str>,
}

/// UNVERIFIED rule (ZK): a computed mass agrees with the material table when within 0.5 % (or half the last printed
/// decimal). Not a fabrication value; the factory has not confirmed it.
pub const MASS_TABLE_REL_TOL: f64 = 0.005;
/// UNVERIFIED rule (ZK): a computed plate size agrees with the table size when each side is within 1 mm.
pub const SIZE_TABLE_TOL_MM: f64 = 1.0;

fn hold(level: Level, code: &'static str, keys: &[&'static str]) -> Hold {
    Hold {
        level,
        code,
        keys: keys.to_vec(),
    }
}

/// One row of the material table as read (text), with the numeric mass if it parsed.
#[derive(Debug, Clone, Default)]
pub struct TableRow {
    pub size: String,
    pub length: String,
    /// Mass in kg as written, and the number of decimals it was written with (rounding allowance of the check).
    pub mass: Option<f64>,
    pub mass_decimals: usize,
    pub qty: Option<f64>,
}

/// Confirmed plate values (mm, degrees where stated). `None` = not read. `*_stated` = the drawing shows text for
/// the value but no number could be read from it.
#[derive(Debug, Clone, Default)]
pub struct PlateParams {
    pub t: Option<f64>,
    pub plate_w: Option<f64>,
    pub plate_h: Option<f64>,
    pub plate_w2: Option<f64>,
    pub plate_off: Option<f64>,
    pub plate_c: Option<f64>,
    pub plate_c_stated: bool,
    pub plate_c_at: Option<f64>,
    pub hole_d: Option<f64>,
    pub hole_d_stated: bool,
    pub hole_x: Option<f64>,
    pub hole_y: Option<f64>,
    pub hole_n: Option<f64>,
    pub hole_nx: Option<f64>,
    pub hole_ny: Option<f64>,
    pub hole_px: Option<f64>,
    pub hole_py: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct PlatePiece {
    pub plate: Plate,
    pub qty: f64,
    pub kg: f64,
    pub gross_kg: f64,
}

#[derive(Debug, Clone, Default)]
pub struct PlatePart {
    pub holds: Vec<Hold>,
    /// None when the outline is not decided (part is not computed, no DXF).
    pub piece: Option<PlatePiece>,
    /// The spec the engine was given (after defaults and after dropping a chamfer or holes that were held).
    pub spec: Option<PlateSpec>,
}

impl PlatePart {
    pub fn count(&self, level: Level) -> usize {
        self.holds.iter().filter(|h| h.level == level).count()
    }
    pub fn has(&self, level: Level, code: &str) -> bool {
        self.holds
            .iter()
            .any(|h| h.level == level && h.code == code)
    }
}

/// Map full-width ASCII (U+FF01..U+FF5E) to ASCII and the ideographic space to a space: the part of NFKC the
/// material-table text needs. Uppercases.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c as u32 {
            0xFF01..=0xFF5E => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            0x3000 => ' ',
            _ => c,
        })
        .flat_map(char::to_uppercase)
        .collect()
}

/// Decimal numbers in a string, in order.
fn numbers(s: &str) -> Vec<f64> {
    let b: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let st = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i + 1 < b.len() && b[i] == '.' && b[i + 1].is_ascii_digit() {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if let Ok(v) = b[st..i].iter().collect::<String>().parse::<f64>() {
                out.push(v);
            }
        } else {
            i += 1;
        }
    }
    out
}

/// A number directly followed (blanks allowed) by the letter T that ends a word: "12t".
fn number_before_t(s: &str) -> Option<f64> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_digit() || b[i - 1] == '.')) {
            let st = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                i += 1;
            }
            let num: String = b[st..i].iter().collect();
            let mut j = i;
            while j < b.len() && b[j] == ' ' {
                j += 1;
            }
            if j < b.len()
                && b[j] == 'T'
                && !(j + 1 < b.len() && b[j + 1].is_alphanumeric())
                && let Ok(v) = num.parse::<f64>()
            {
                return Some(v);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Material-table plate size "PL-12x300x80", "12t x 300 x 80", "PL-16x800" + length "2700" -> (t, width, length).
/// The order of the numbers is thickness x width x length (UNVERIFIED rule, ZK `parsePL`): a number written with a
/// `t`, or equal to the drawn thickness, is taken as the thickness first. A phi sign means disc/ring: None.
pub fn parse_pl(
    size: &str,
    length: &str,
    t_known: Option<f64>,
) -> Option<(f64, Option<f64>, Option<f64>)> {
    let s = fold(size).replace('Ø', "Φ");
    if s.contains('Φ') || s.contains('φ') {
        return None;
    }
    let mut ns = numbers(&s);
    let ln = numbers(&fold(length)).first().copied();
    let t = if let Some(m) = number_before_t(&s) {
        let p = ns.iter().position(|x| *x == m)?;
        ns.remove(p);
        m
    } else if let Some(tk) = t_known.filter(|tk| ns.contains(tk)) {
        let p = ns.iter().position(|x| *x == tk)?;
        ns.remove(p);
        tk
    } else if !ns.is_empty() {
        ns.remove(0)
    } else {
        return None;
    };
    if ns.len() == 1
        && let Some(l) = ln
    {
        ns.push(l);
    }
    Some((t, ns.first().copied(), ns.get(1).copied()))
}

fn is_int(v: f64) -> bool {
    v.is_finite() && v.fract() == 0.0
}

/// Check one plate part, fill missing values from the material table row, and build the plate.
/// `qty` = quantity from the table (>= 1) else 1. `row` = the part's material-table row, if any.
pub fn check_plate(p: &PlateParams, row: Option<&TableRow>) -> PlatePart {
    let mut out = PlatePart::default();
    let mut p = p.clone();
    let mut from_table = false;
    // fill from the table (fillPlate)
    if let Some(r) = row
        && let Some((tt, w, h)) = parse_pl(&r.size, &r.length, p.t)
    {
        {
            match p.t {
                None => p.t = Some(tt),
                Some(t) if (t - tt).abs() > 0.05 => {
                    out.holds
                        .push(hold(Level::Need, "plate_t_vs_table", &["t"]))
                }
                _ => {}
            }
            if let (Some(w), Some(h)) = (w, h) {
                match (p.plate_w, p.plate_h) {
                    (None, None) => {
                        p.plate_w = Some(w);
                        p.plate_h = Some(h);
                        from_table = true;
                    }
                    (None, Some(ph)) => {
                        let o = if (ph - w).abs() <= 0.5 {
                            Some(h)
                        } else if (ph - h).abs() <= 0.5 {
                            Some(w)
                        } else {
                            None
                        };
                        if o.is_some() {
                            p.plate_w = o;
                            from_table = true;
                        }
                    }
                    (Some(pw), None) => {
                        let o = if (pw - w).abs() <= 0.5 {
                            Some(h)
                        } else if (pw - h).abs() <= 0.5 {
                            Some(w)
                        } else {
                            None
                        };
                        if o.is_some() {
                            p.plate_h = o;
                            from_table = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // required values
    let mut missing: Vec<&'static str> = Vec::new();
    if p.t.is_none() {
        missing.push("t");
    }
    if p.plate_w.is_none() {
        missing.push("plate_W");
    }
    if p.plate_h.is_none() {
        missing.push("plate_H");
    }
    if !missing.is_empty() {
        out.holds.push(hold(Level::Need, "missing", &missing));
        return out;
    }
    if from_table {
        out.holds.push(hold(
            Level::Info,
            "plate_from_table",
            &["plate_W", "plate_H"],
        ));
    }
    let (t, w, h) = (
        p.t.unwrap_or(0.0),
        p.plate_w.unwrap_or(0.0),
        p.plate_h.unwrap_or(0.0),
    );
    let (w2, off) = (p.plate_w2, p.plate_off);
    if !(w > 0.0 && h > 0.0 && t > 0.0)
        || w2.is_some_and(|v| v < 0.0)
        || ![w, h, t].iter().all(|v| v.is_finite())
    {
        out.holds.push(hold(
            Level::Need,
            "plate_bad",
            &["plate_W", "plate_H", "plate_W2"],
        ));
        return out;
    }
    let tri = w2 == Some(0.0);
    let trap = w2.is_some_and(|v| v != 0.0 && (v - w).abs() > 1e-9);
    if trap && off.is_none() {
        out.holds
            .push(hold(Level::Need, "plate_off", &["plate_off"]));
        return out;
    }
    if tri && off.is_none() {
        out.holds
            .push(hold(Level::Info, "plate_off_defaulted", &["plate_off"]));
    }
    let off_eff = if tri && off.is_none() { Some(0.0) } else { off };
    let nc: usize = if tri { 3 } else { 4 };
    let mut spec = PlateSpec {
        w,
        h,
        w2,
        off: off_eff,
        t,
        c: None,
        c_at: 0,
        hole: None,
    };
    // chamfer
    if p.plate_c.is_none() && (p.plate_c_at.is_some() || p.plate_c_stated) {
        out.holds.push(hold(Level::Need, "plate_C", &["plate_C"]));
    }
    if let Some(c) = p.plate_c.filter(|c| *c != 0.0) {
        match p.plate_c_at {
            None => out
                .holds
                .push(hold(Level::Need, "plate_C_at", &["plate_C_at"])),
            Some(at) if !(is_int(at) && at >= 0.0 && at <= nc as f64) => {
                out.holds
                    .push(hold(Level::Need, "plate_C_at", &["plate_C_at"]))
            }
            Some(at) => {
                let mut trial = spec;
                trial.c = Some(c);
                trial.c_at = at as u32;
                match shapes::plate_geometry(trial) {
                    Err(e) if e.code == "plate_chamfer_too_long" => {
                        out.holds
                            .push(hold(Level::Need, "plate_C_big", &["plate_C"]));
                        return out;
                    }
                    Err(_) => {
                        out.holds.push(hold(Level::Need, "plate_bad", &["plate_C"]));
                        return out;
                    }
                    Ok(_) => spec = trial,
                }
            }
        }
    }
    // holes
    let hole_keys: [(&str, Option<f64>); 7] = [
        ("hole_x", p.hole_x),
        ("hole_y", p.hole_y),
        ("hole_n", p.hole_n),
        ("hole_nx", p.hole_nx),
        ("hole_ny", p.hole_ny),
        ("hole_px", p.hole_px),
        ("hole_py", p.hole_py),
    ];
    if p.hole_d.is_none() && (p.hole_d_stated || hole_keys.iter().any(|(_, v)| v.is_some())) {
        out.holds
            .push(hold(Level::Need, "plate_hole_d", &["hole_d"]));
    }
    if let Some(d) = p.hole_d.filter(|d| *d > 0.0) {
        let (Some(x), Some(y)) = (p.hole_x, p.hole_y) else {
            out.holds
                .push(hold(Level::Need, "plate_hole_pos", &["hole_x", "hole_y"]));
            out.spec = Some(spec);
            out.piece = build_piece(spec, row, &mut out.holds);
            return out;
        };
        let (n, nx, ny, px, py) = (p.hole_n, p.hole_nx, p.hole_ny, p.hole_px, p.hole_py);
        let (ix, iy) = (nx.unwrap_or(1.0), ny.unwrap_or(1.0));
        let mut keys: Vec<&'static str> = Vec::new();
        fn add(keys: &mut Vec<&'static str>, k: &'static str) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        if n.is_some_and(|n| n > 1.0) && nx.is_none() && ny.is_none() {
            add(&mut keys, "hole_nx");
            add(&mut keys, "hole_ny");
        }
        if !(is_int(ix) && ix >= 1.0) {
            add(&mut keys, "hole_nx");
        }
        if !(is_int(iy) && iy >= 1.0) {
            add(&mut keys, "hole_ny");
        }
        if keys.is_empty() && n.is_some_and(|n| ix * iy != n) {
            add(&mut keys, "hole_nx");
            add(&mut keys, "hole_ny");
        }
        if ix > 1.0 && px.is_none() {
            add(&mut keys, "hole_px");
        }
        if iy > 1.0 && py.is_none() {
            add(&mut keys, "hole_py");
        }
        let grid = keys.is_empty();
        if !grid {
            out.holds.push(hold(Level::Need, "plate_hole_grid", &keys));
        }
        let (gx, gy) = if grid { (ix as u32, iy as u32) } else { (1, 1) };
        let g = HoleGrid {
            d,
            x,
            y,
            nx: gx,
            ny: gy,
            px: px.unwrap_or(0.0),
            py: py.unwrap_or(0.0),
        };
        let mut trial = spec;
        trial.hole = Some(g);
        match shapes::plate_geometry(trial) {
            Ok(pl) => {
                if !shapes::holes_outside(&pl).is_empty() {
                    out.holds
                        .push(hold(Level::Need, "plate_hole_out", &["hole_x", "hole_y"]));
                }
                if grid
                    && ((ix > 1.0 && px.unwrap_or(0.0) < d) || (iy > 1.0 && py.unwrap_or(0.0) < d))
                {
                    out.holds.push(hold(
                        Level::Need,
                        "plate_hole_overlap",
                        &["hole_px", "hole_py"],
                    ));
                }
                spec = trial;
            }
            Err(_) => out
                .holds
                .push(hold(Level::Need, "plate_hole_invalid", &["hole_d"])),
        }
    }
    out.spec = Some(spec);
    out.piece = build_piece(spec, row, &mut out.holds);
    if let (Some(r), Some(pc)) = (row, out.piece.as_ref()) {
        cross_check_plate(&p, r, pc, from_table, &mut out.holds);
    }
    out
}

fn build_piece(
    spec: PlateSpec,
    row: Option<&TableRow>,
    holds: &mut Vec<Hold>,
) -> Option<PlatePiece> {
    let qty = row.and_then(|r| r.qty).filter(|q| *q >= 1.0).unwrap_or(1.0);
    match shapes::plate_geometry(spec) {
        Ok(plate) => Some(PlatePiece {
            qty,
            kg: plate.mass_kg * qty,
            gross_kg: plate.blank_kg * qty,
            plate,
        }),
        Err(_) => {
            holds.push(hold(Level::Need, "plate_bad", &["plate_W", "plate_H"]));
            None
        }
    }
}

/// Mass and size against the material table (build-core.mjs:1169-1195). The mass is compared with the cut plate and
/// with the blank rectangle; the nearer one is used. Plates are light, so a difference inside the rounding of the
/// written mass (0.5 x 10^-decimals) is accepted besides the 0.5 % rule.
fn cross_check_plate(
    p: &PlateParams,
    r: &TableRow,
    pc: &PlatePiece,
    from_table: bool,
    holds: &mut Vec<Hold>,
) {
    if let Some(tm) = r.mass.filter(|m| *m > 0.0) {
        let (dn, dg) = (pc.kg - tm, pc.gross_kg - tm);
        let use_gross = dg.abs() < dn.abs();
        let d = if use_gross { dg } else { dn };
        let ok = d.abs() / tm < MASS_TABLE_REL_TOL
            || d.abs() <= 0.5 * 10f64.powi(-(r.mass_decimals as i32)) + 1e-9;
        holds.push(hold(
            if ok { Level::Ok } else { Level::Need },
            if use_gross {
                "mass_vs_table_blank"
            } else {
                "mass_vs_table_cut"
            },
            &["t"],
        ));
    }
    if !from_table && let Some((_, Some(a), Some(b))) = parse_pl(&r.size, &r.length, p.t) {
        {
            let mut x = [pc.plate.blank_w, pc.plate.blank_h];
            let mut y = [a, b];
            x.sort_by(f64::total_cmp);
            y.sort_by(f64::total_cmp);
            let ok = (x[0] - y[0]).abs() <= SIZE_TABLE_TOL_MM
                && (x[1] - y[1]).abs() <= SIZE_TABLE_TOL_MM;
            holds.push(hold(
                if ok { Level::Ok } else { Level::Need },
                "size_vs_table",
                &["plate_W", "plate_H"],
            ));
        }
    }
}

// ---------------------------------------------------------------- straight pipe

#[derive(Debug, Clone, Default)]
pub struct CylParams {
    pub d: Option<f64>,
    pub t: Option<f64>,
    pub l: Option<f64>,
    /// Longitudinal seams as read (integer expected).
    pub split: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CylPart {
    pub holds: Vec<Hold>,
    /// Seam count used (1 when not stated; stated values outside 1..=6 are clamped to the near end and held).
    pub seams: u32,
    pub plates: u32,
    pub plate_w: f64,
    pub plate_h: f64,
    pub kg: f64,
    pub circ: f64,
    /// Fewest seams (above the stated count) for which every plate fits the stock, when the stated count does not fit.
    pub fits_at: Option<u32>,
}

/// Straight pipe against the stock list: which seam count fits, and the holds of the stock rule
/// (build-core.mjs:1077 seam range, :1146-1165 stock fit). An empty `stock` list switches the stock rule off.
pub fn check_cylinder(p: &CylParams, stock: &[Stock], margin: f64) -> Option<CylPart> {
    let mut holds = Vec::new();
    let mut miss = Vec::new();
    for (k, v) in [("D", p.d), ("t", p.t), ("L", p.l)] {
        if v.is_none() {
            miss.push(k);
        }
    }
    if !miss.is_empty() {
        holds.push(hold(Level::Need, "missing", &miss));
        return Some(CylPart {
            holds,
            seams: 1,
            plates: 0,
            plate_w: 0.0,
            plate_h: 0.0,
            kg: 0.0,
            circ: 0.0,
            fits_at: None,
        });
    }
    let (d, t, l) = (p.d?, p.t?, p.l?);
    let seams = match p.split {
        None => 1,
        Some(s) if s.is_finite() => {
            let r = s.round();
            if !(is_int(s) && (1.0..=f64::from(shapes::MAX_CYL_SEAMS)).contains(&s)) {
                holds.push(hold(Level::Need, "split_seam", &["split"]));
            }
            r.clamp(1.0, f64::from(shapes::MAX_CYL_SEAMS)) as u32
        }
        Some(_) => {
            holds.push(hold(Level::Need, "split_seam", &["split"]));
            1
        }
    };
    let c = match shapes::cylinder(CylSpec { d, t, l, seams }) {
        Ok(c) => c,
        Err(_) => {
            holds.push(hold(Level::Need, "plate_bad", &["D", "t", "L"]));
            return Some(CylPart {
                holds,
                seams,
                plates: 0,
                plate_w: 0.0,
                plate_h: 0.0,
                kg: 0.0,
                circ: 0.0,
                fits_at: None,
            });
        }
    };
    let mut fits_at = None;
    if !stock.is_empty() {
        if shapes::fit_stock(c.plate_w, c.plate_h, stock, margin).is_none() {
            holds.push(hold(Level::Need, "stock_fit", &["split"]));
            match shapes::min_cyl_seams(d, t, l, seams + 1, stock, margin) {
                Ok(Some(k)) => {
                    fits_at = Some(k);
                    holds.push(hold(Level::Info, "stock_fit_more_seams_fit", &["split"]));
                }
                _ => {
                    let long =
                        stock.iter().map(|s| s.w.max(s.l)).fold(0.0, f64::max) - 2.0 * margin;
                    if l > long + 1e-9 {
                        holds.push(hold(Level::Need, "pipe_longer_than_stock", &["L"]));
                    }
                }
            }
        } else {
            holds.push(hold(Level::Info, "stock_checked", &[]));
        }
    }
    Some(CylPart {
        holds,
        seams,
        plates: c.plates,
        plate_w: c.plate_w,
        plate_h: c.plate_h,
        kg: c.mass_kg,
        circ: c.circ,
        fits_at,
    })
}

// ---------------------------------------------------------------- tee rib (rib on the main pipe)

/// Confirmed values of a tee rib part (mm). `main_d` and `branch_d` are the values of the tee group
/// (the group fill of the branch diameter is the tee lane's job, not done here).
#[derive(Debug, Clone, Default)]
pub struct RibParams {
    pub t: Option<f64>,
    pub main_d: Option<f64>,
    pub branch_d: Option<f64>,
    pub e: Option<f64>,
    pub rib_b: Option<f64>,
    pub rib_h: Option<f64>,
    pub rib_top: Option<f64>,
    /// The drawing shows a flat top but no width could be read.
    pub rib_top_stated: bool,
    /// Ribs along the main pipe's circle (<= 2) and triangular ribs along its axis.
    pub rib_n1: Option<f64>,
    pub rib_n2: Option<f64>,
    /// Sum of the material-table quantities of the part's rows, and the written mass (kg) with its decimals.
    pub table_qty: Option<f64>,
    pub table_mass: Option<f64>,
    pub table_mass_decimals: usize,
}

#[derive(Debug, Clone)]
pub enum RibGeo {
    Circle(shapes::RibSpec),
    Triangle(PlateSpec),
}

#[derive(Debug, Clone)]
pub struct RibOut {
    pub geo: RibGeo,
    /// Side label for a rib on an off-centre branch with two ribs.
    pub side: Option<&'static str>,
    pub n: f64,
    pub area: f64,
    pub kg: f64,
    pub gross_kg: f64,
}

#[derive(Debug, Clone, Default)]
pub struct RibPart {
    pub holds: Vec<Hold>,
    pub pieces: Vec<RibOut>,
}

impl RibPart {
    pub fn count(&self, level: Level) -> usize {
        self.holds.iter().filter(|h| h.level == level).count()
    }
    pub fn has(&self, level: Level, code: &str) -> bool {
        self.holds
            .iter()
            .any(|h| h.level == level && h.code == code)
    }
}

/// Tee rib front half (build-core.mjs ribGeos :45, rib checks :1003-1009, geometry check :354-366, mass :1166-1180).
pub fn check_tee_rib(p: &RibParams) -> RibPart {
    let mut out = RibPart::default();
    let mut miss = Vec::new();
    for (k, v) in [
        ("t", p.t),
        ("main_D", p.main_d),
        ("branch_D", p.branch_d),
        ("rib_b", p.rib_b),
        ("rib_h", p.rib_h),
    ] {
        if v.is_none() {
            miss.push(k);
        }
    }
    if !miss.is_empty() {
        out.holds.push(hold(Level::Need, "missing", &miss));
        return out;
    }
    let (t, md, bd, b, h) = (
        p.t.unwrap_or(0.0),
        p.main_d.unwrap_or(0.0),
        p.branch_d.unwrap_or(0.0),
        p.rib_b.unwrap_or(0.0),
        p.rib_h.unwrap_or(0.0),
    );
    // every stated number must be a finite, positive dimension (counts: whole numbers); anything else is a coded hold
    let bad_dim = [t, md, bd, b, h]
        .iter()
        .any(|v| !(v.is_finite() && *v > 0.0))
        || p.e.is_some_and(|v| !v.is_finite())
        || p.rib_top.is_some_and(|v| !(v.is_finite() && v >= 0.0))
        || [p.rib_n1, p.rib_n2]
            .iter()
            .flatten()
            .any(|v| !(is_int(*v) && *v >= 0.0));
    if bad_dim {
        out.holds.push(hold(
            Level::Need,
            "rib_bad_value",
            &[
                "t", "main_D", "branch_D", "rib_b", "rib_h", "rib_n1", "rib_n2",
            ],
        ));
        return out;
    }
    let (n1, n2) = (
        p.rib_n1.filter(|v| *v != 0.0),
        p.rib_n2.filter(|v| *v != 0.0),
    );
    if p.rib_n1.is_none() && p.rib_n2.is_none() {
        out.holds
            .push(hold(Level::Need, "rib_count", &["rib_n1", "rib_n2"]));
        return out;
    }
    if n1.is_some_and(|n| n > 2.0) {
        out.holds.push(hold(Level::Need, "rib_n1", &["rib_n1"]));
        return out;
    }
    if p.rib_top.is_none() && p.rib_top_stated {
        out.holds.push(hold(Level::Need, "rib_top", &["rib_top"]));
    }
    let e = p.e.unwrap_or(0.0).abs();
    if e > 1e-9 && n1 == Some(1.0) {
        out.holds.push(hold(Level::Info, "rib_side", &["e"]));
    }
    if let Some(q) = p.table_qty
        && q != n1.unwrap_or(0.0) + n2.unwrap_or(0.0)
    {
        out.holds
            .push(hold(Level::Need, "rib_qty", &["rib_n1", "rib_n2"]));
    }
    let (ro, rbr) = (md / 2.0, bd / 2.0);
    let top = p.rib_top.unwrap_or(0.0);
    let mut geos: Vec<(RibGeo, Option<&'static str>, f64)> = Vec::new();
    if let Some(n1) = n1 {
        let both = e > 1e-9 && n1 == 2.0;
        let mk = |vc: f64| RibGeo::Circle(shapes::RibSpec { ro, vc, b, h, top });
        geos.push((
            mk(e - rbr),
            if both { Some("center") } else { None },
            if both { 1.0 } else { n1 },
        ));
        if both {
            geos.push((mk(-(e + rbr)), Some("opposite"), 1.0));
        }
    }
    if let Some(n2) = n2 {
        geos.push((
            RibGeo::Triangle(PlateSpec {
                w: b,
                h,
                w2: Some(top),
                off: Some(0.0),
                t,
                c: None,
                c_at: 0,
                hole: None,
            }),
            None,
            n2,
        ));
    }
    for (g, side, n) in geos {
        match &g {
            RibGeo::Triangle(ps) => {
                if top >= b {
                    out.holds
                        .push(hold(Level::Need, "rib_geometry_top", &["rib_top"]));
                    return RibPart {
                        holds: out.holds,
                        pieces: Vec::new(),
                    };
                }
                match shapes::plate_geometry(*ps) {
                    Ok(pl) => out.pieces.push(RibOut {
                        geo: g.clone(),
                        side,
                        n,
                        area: pl.area,
                        kg: pl.mass_kg * n,
                        gross_kg: pl.blank_kg * n,
                    }),
                    Err(_) => {
                        out.holds
                            .push(hold(Level::Need, "rib_geometry_top", &["rib_b", "rib_h"]));
                        return RibPart {
                            holds: out.holds,
                            pieces: Vec::new(),
                        };
                    }
                }
            }
            RibGeo::Circle(rs) => {
                let tip_inside = (rs.ro * rs.ro - (rs.h - rs.vc).powi(2)).max(0.0).sqrt()
                    - (rs.ro * rs.ro - rs.vc * rs.vc).max(0.0).sqrt()
                    + rs.top
                    >= rs.b;
                let code = if rs.vc.abs() >= rs.ro {
                    Some("rib_not_on_pipe")
                } else if (rs.h - rs.vc).abs() > rs.ro {
                    Some("rib_taller_than_pipe")
                } else if tip_inside {
                    Some("rib_tip_inside_pipe")
                } else {
                    None
                };
                if let Some(code) = code {
                    out.holds
                        .push(hold(Level::Need, code, &["rib_b", "rib_h", "e"]));
                    return RibPart {
                        holds: out.holds,
                        pieces: Vec::new(),
                    };
                }
                match shapes::rib(*rs) {
                    Ok(r) => {
                        let hh = rs.h;
                        // blank = bounding box of the rib outline: x from the pipe-side corner to the far end
                        let xmax = rs.b.max(r.x_c + rs.top);
                        let xmin = 0.0f64.min(r.x_c);
                        out.pieces.push(RibOut {
                            geo: g.clone(),
                            side,
                            n,
                            area: r.area,
                            kg: mass_kg(r.area, t) * n,
                            gross_kg: mass_kg((xmax - xmin) * hh, t) * n,
                        });
                    }
                    Err(_) => {
                        out.holds.push(hold(
                            Level::Need,
                            "rib_not_on_pipe",
                            &["rib_b", "rib_h", "e"],
                        ));
                        return RibPart {
                            holds: out.holds,
                            pieces: Vec::new(),
                        };
                    }
                }
            }
        }
    }
    if let Some(tm) = p.table_mass.filter(|m| *m > 0.0) {
        let calc: f64 = out.pieces.iter().map(|x| x.kg).sum();
        let gross: f64 = out.pieces.iter().map(|x| x.gross_kg).sum();
        let (dn, dg) = (calc - tm, gross - tm);
        let use_gross = dg.abs() < dn.abs();
        let d = if use_gross { dg } else { dn };
        let ok = d.abs() / tm < MASS_TABLE_REL_TOL
            || d.abs() <= 0.5 * 10f64.powi(-(p.table_mass_decimals as i32)) + 1e-9;
        out.holds.push(hold(
            if ok { Level::Ok } else { Level::Need },
            "mass_vs_table",
            &["t"],
        ));
    }
    out
}

fn mass_kg(area: f64, t: f64) -> f64 {
    shapes::mass_kg(area, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pl_reads_the_table_writings_of_the_golden_cases() {
        assert_eq!(
            parse_pl("12t×300×80", "", None),
            Some((12.0, Some(300.0), Some(80.0)))
        );
        assert_eq!(
            parse_pl("PL-16×800", "2700", None),
            Some((16.0, Some(800.0), Some(2700.0)))
        );
        assert_eq!(
            parse_pl("ＰＬ－１２×３００×８０", "", None),
            Some((12.0, Some(300.0), Some(80.0)))
        );
        assert_eq!(parse_pl("PL-19×φ1900", "φ1600", None), None);
        assert_eq!(
            parse_pl("PL-16", "400", Some(16.0)),
            Some((16.0, None, None))
        );
        assert_eq!(parse_pl("", "", None), None);
    }

    #[test]
    fn plate_front_half_holds_are_coded() {
        let base = PlateParams {
            t: Some(9.0),
            plate_w: Some(300.0),
            plate_h: Some(200.0),
            ..Default::default()
        };
        let trap = check_plate(
            &PlateParams {
                plate_w2: Some(150.0),
                ..base.clone()
            },
            None,
        );
        assert!(trap.has(Level::Need, "plate_off") && trap.piece.is_none());
        let tri = check_plate(
            &PlateParams {
                plate_w: Some(200.0),
                plate_h: Some(150.0),
                plate_w2: Some(0.0),
                ..base.clone()
            },
            None,
        );
        assert!(tri.has(Level::Info, "plate_off_defaulted"));
        assert!((tri.piece.unwrap().plate.area - 15000.0).abs() < 1e-9);
        assert!(
            check_plate(
                &PlateParams {
                    plate_c: Some(10.0),
                    ..base.clone()
                },
                None
            )
            .has(Level::Need, "plate_C_at")
        );
        assert!(
            check_plate(
                &PlateParams {
                    plate_h: Some(15.0),
                    plate_c: Some(10.0),
                    plate_c_at: Some(0.0),
                    ..base.clone()
                },
                None
            )
            .has(Level::Need, "plate_C_big")
        );
        assert!(
            check_plate(
                &PlateParams {
                    hole_d: Some(22.0),
                    ..base.clone()
                },
                None
            )
            .has(Level::Need, "plate_hole_pos")
        );
        let holes =
            |n: Option<f64>, nx: Option<f64>, ny: Option<f64>, px: Option<f64>| PlateParams {
                t: Some(16.0),
                plate_w: Some(400.0),
                plate_h: Some(400.0),
                hole_d: Some(24.0),
                hole_n: n,
                hole_x: Some(50.0),
                hole_y: Some(50.0),
                hole_nx: nx,
                hole_ny: ny,
                hole_px: px,
                hole_py: px,
                ..Default::default()
            };
        assert!(
            check_plate(&holes(Some(4.0), None, None, None), None)
                .has(Level::Need, "plate_hole_grid")
        );
        assert!(
            check_plate(&holes(Some(6.0), Some(2.0), Some(2.0), Some(300.0)), None)
                .has(Level::Need, "plate_hole_grid")
        );
        assert!(
            check_plate(&holes(Some(2.0), Some(2.0), None, Some(20.0)), None)
                .has(Level::Need, "plate_hole_overlap")
        );
        let out = check_plate(
            &PlateParams {
                plate_w2: Some(150.0),
                plate_off: Some(0.0),
                hole_d: Some(22.0),
                hole_x: Some(280.0),
                hole_y: Some(150.0),
                ..base
            },
            None,
        );
        assert!(out.has(Level::Need, "plate_hole_out"));
        // non-finite never reaches a NaN piece
        let nan = check_plate(
            &PlateParams {
                t: Some(9.0),
                plate_w: Some(f64::NAN),
                plate_h: Some(10.0),
                ..Default::default()
            },
            None,
        );
        assert!(nan.piece.is_none() && nan.has(Level::Need, "plate_bad"));
    }

    #[test]
    fn cylinder_stock_rule_codes() {
        let s = [Stock {
            w: 2438.0,
            l: 6096.0,
        }];
        let mk = |l: f64, split: Option<f64>| CylParams {
            d: Some(2641.6),
            t: Some(16.0),
            l: Some(l),
            split,
        };
        let one = check_cylinder(&mk(1500.0, None), &s, 0.0).unwrap();
        assert!(
            one.holds
                .iter()
                .any(|h| h.code == "stock_fit" && h.level == Level::Need)
        );
        let two = check_cylinder(&mk(1500.0, Some(2.0)), &s, 0.0).unwrap();
        assert!(two.holds.iter().all(|h| h.level != Level::Need) && two.plates == 2);
        assert!(
            check_cylinder(&mk(7000.0, None), &s, 0.0)
                .unwrap()
                .holds
                .iter()
                .any(|h| h.code == "pipe_longer_than_stock")
        );
        assert!(
            check_cylinder(&mk(1500.0, Some(8.0)), &s, 0.0)
                .unwrap()
                .holds
                .iter()
                .any(|h| h.code == "split_seam")
        );
        assert!(
            !check_cylinder(&mk(1500.0, None), &[], 0.0)
                .unwrap()
                .holds
                .iter()
                .any(|h| h.code.starts_with("stock"))
        );
    }
}
