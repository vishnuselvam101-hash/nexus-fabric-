//! Nesting for NEXUS FABRIC: sheet nesting of certified part outlines (bottom-left fill), a 1D pipe cut list,
//! an independent checker that must pass before any sheet DXF is written, and the sheet DXF writer.
//!
//! Nothing here approves or releases a result. Every sheet DXF carries the marking NOT RELEASED. Stock sizes,
//! kerf, part gap and edge margin are named rules marked UNVERIFIED until a person of the factory confirms them.
//! The crate is not a dependency of nf-digest.
//!
//! Engine choice: jagua-rs 2.0.0 (MPL-2.0, rust-version 1.90) was assessed and not embedded in this version; the
//! bottom-left fill is the v1 placer. Its only workspace dependency is nf-dxf (read-only, for part outlines).

pub mod check;
pub mod dxf_out;
pub mod geom;
pub mod outline;
pub mod pipes;

#[cfg(test)]
mod tests;

use geom::P;

/// Rule names as they appear in the output. All are UNVERIFIED until a factory confirms them.
pub const RULE_STOCK: &str = "NEST_STOCK_SIZE";
pub const RULE_KERF: &str = "NEST_KERF_MM";
pub const RULE_GAP: &str = "NEST_PART_GAP_MM";
pub const RULE_EDGE: &str = "NEST_EDGE_MM";
pub const RULE_ROT: &str = "NEST_ROTATION_STEPS";
pub const MAX_PARTS: usize = 500;

#[derive(Clone, Debug, PartialEq)]
pub struct NestError(pub String);

impl std::fmt::Display for NestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Sheet and spacing rules. UNVERIFIED defaults; the caller states the values it used.
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub sheet_w: f64,
    pub sheet_h: f64,
    /// Width of the cut (kerf) in mm.
    pub kerf_mm: f64,
    /// Extra clear distance between two parts, beyond the kerf, in mm.
    pub part_gap_mm: f64,
    /// Margin kept from the sheet edge, in mm.
    pub edge_mm: f64,
    /// Allowed rotations in degrees; a subset of 0, 90, 180, 270.
    pub rotations: Vec<u32>,
}

impl Rules {
    /// UNVERIFIED default: a 1500 x 3000 mm sheet, 0.2 mm kerf, 2 mm part gap, 5 mm edge, 0 and 90 degrees.
    pub fn unverified_default() -> Rules {
        Rules {
            sheet_w: 1500.0,
            sheet_h: 3000.0,
            kerf_mm: 0.2,
            part_gap_mm: 2.0,
            edge_mm: 5.0,
            rotations: vec![0, 90],
        }
    }

    /// Clearance between two parts: kerf plus part gap.
    pub fn clearance(&self) -> f64 {
        self.kerf_mm + self.part_gap_mm
    }

    pub fn validate(&self) -> Result<(), NestError> {
        let bad = |m: &str| {
            Err(NestError(format!(
                "{m} (rule {RULE_STOCK}/{RULE_KERF}/{RULE_GAP}/{RULE_EDGE}, UNVERIFIED)"
            )))
        };
        if !(self.sheet_w > 0.0
            && self.sheet_h > 0.0
            && self.sheet_w <= 6000.0
            && self.sheet_h <= 6000.0)
        {
            return bad("sheet size must be above 0 and at most 6000 mm");
        }
        if !(self.kerf_mm >= 0.0 && self.kerf_mm <= 2.0)
            || !(self.part_gap_mm >= 0.0 && self.part_gap_mm <= 50.0)
        {
            return bad("kerf must be 0 to 2 mm and part gap 0 to 50 mm");
        }
        if self.clearance() <= 0.0 {
            return bad("kerf plus part gap must be above 0 mm");
        }
        if !(self.edge_mm >= 0.0 && self.edge_mm <= 100.0) {
            return bad("edge margin must be 0 to 100 mm");
        }
        if self.rotations.is_empty()
            || self
                .rotations
                .iter()
                .any(|r| ![0, 90, 180, 270].contains(r))
        {
            return Err(NestError(format!(
                "rotation steps must be a non-empty subset of 0, 90, 180, 270 ({RULE_ROT})"
            )));
        }
        Ok(())
    }
}

/// One part to nest: a certified outline and how many copies are needed.
#[derive(Clone, Debug, PartialEq)]
pub struct PartIn {
    pub id: String,
    pub outline: Vec<P>,
    pub qty: u32,
}

/// One placed copy. `pts` is the placed outline; `at` is the bounding-box corner after rotation.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub id: String,
    pub copy: u32,
    pub rot: u32,
    pub at: P,
    pub pts: Vec<P>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Sheet {
    pub placed: Vec<Placed>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Nest {
    pub rules: Rules,
    pub parts: Vec<PartIn>,
    pub sheets: Vec<Sheet>,
}

/// Utilisation: part area over sheet area, per sheet and over all sheets, in percent.
pub fn utilisation(n: &Nest) -> (Vec<f64>, f64) {
    let sheet_area = n.rules.sheet_w * n.rules.sheet_h;
    let per: Vec<f64> = n
        .sheets
        .iter()
        .map(|s| s.placed.iter().map(|p| geom::area(&p.pts)).sum::<f64>() / sheet_area * 100.0)
        .collect();
    let total: f64 = n
        .sheets
        .iter()
        .map(|s| s.placed.iter().map(|p| geom::area(&p.pts)).sum::<f64>())
        .sum();
    let all = if n.sheets.is_empty() {
        0.0
    } else {
        total / (sheet_area * n.sheets.len() as f64) * 100.0
    };
    (per, all)
}

/// Bottom-left fill. Parts are placed largest first; each copy goes to the lowest, then leftmost, feasible
/// position over all allowed rotations on the first sheet that takes it. Candidate positions are the sheet
/// corner and the corners next to placed parts. Every placement is re-tested with the full outline clearance.
pub fn nest(parts: &[PartIn], rules: &Rules) -> Result<Nest, NestError> {
    rules.validate()?;
    if parts.len() > MAX_PARTS {
        return Err(NestError(format!(
            "at most {MAX_PARTS} part kinds per nest"
        )));
    }
    let clr = rules.clearance();
    let edge = rules.edge_mm;
    let mut prepared: Vec<(usize, u32, Vec<P>)> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let norm = geom::normalise(&p.outline);
        if norm.len() < 3 || geom::area(&norm) <= 0.0 {
            return Err(NestError(format!("part {} has no usable outline", p.id)));
        }
        for c in 0..p.qty {
            prepared.push((i, c, norm.clone()));
        }
    }
    prepared.sort_by(|a, b| {
        geom::area(&b.2)
            .total_cmp(&geom::area(&a.2))
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });

    let mut sheets: Vec<Sheet> = Vec::new();
    for (i, copy, norm) in prepared {
        let mut done = false;
        for si in 0..=sheets.len() {
            if si == sheets.len() {
                sheets.push(Sheet::default());
            }
            if let Some(p) = best_position(&sheets[si], &norm, rules, clr, edge) {
                sheets[si].placed.push(Placed {
                    id: parts[i].id.clone(),
                    copy,
                    rot: p.0,
                    at: p.1,
                    pts: geom::place(&norm, p.0, p.1),
                });
                done = true;
                break;
            }
            if si + 1 == sheets.len() && sheets[si].placed.is_empty() {
                return Err(NestError(format!(
                    "part {} does not fit the stock sheet (rule {RULE_STOCK}, UNVERIFIED)",
                    parts[i].id
                )));
            }
        }
        if !done {
            return Err(NestError("internal: a part was not placed".into()));
        }
    }
    Ok(Nest {
        rules: rules.clone(),
        parts: parts.to_vec(),
        sheets,
    })
}

/// Lowest then leftmost feasible (rotation, bounding-box corner) on one sheet, or None.
fn best_position(
    sheet: &Sheet,
    norm: &[P],
    rules: &Rules,
    clr: f64,
    edge: f64,
) -> Option<(u32, P)> {
    let mut cands: Vec<P> = vec![(edge, edge)];
    for p in &sheet.placed {
        let (x0, y0, x1, y1) = geom::bbox(&p.pts);
        cands.push((x1 + clr, y0));
        cands.push((x0, y1 + clr));
        cands.push((x1 + clr, edge));
        cands.push((edge, y1 + clr));
    }
    cands.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    let mut best: Option<(u32, P)> = None;
    for &rot in &rules.rotations {
        let r = geom::rotate(norm, rot);
        let (x0, y0, x1, y1) = geom::bbox(&r);
        let (w, h) = (x1 - x0, y1 - y0);
        for &c in &cands {
            if c.0 + w > rules.sheet_w - edge + geom::EPS
                || c.1 + h > rules.sheet_h - edge + geom::EPS
            {
                continue;
            }
            if let Some((_, b)) = best {
                if (c.1, c.0) >= (b.1, b.0) {
                    break;
                }
            }
            let shape = geom::place(norm, rot, c);
            if sheet
                .placed
                .iter()
                .all(|p| geom::clear(&shape, &p.pts, clr))
            {
                best = Some((rot, c));
                break;
            }
        }
    }
    best
}
