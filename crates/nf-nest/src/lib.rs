//! Nesting for NEXUS FABRIC: sheet nesting of certified part outlines (bottom-left fill), a 1D pipe cut list,
//! an independent checker that must pass before any sheet DXF is written, and the sheet DXF writer.
//!
//! Nothing here approves or releases a result. Every sheet DXF carries the marking NOT RELEASED. Stock sizes,
//! kerf, part gap and edge margin are named rules marked UNVERIFIED until a person of the factory confirms them.
//! The crate is not a dependency of nf-digest.
//!
//! Engines: jagua-rs 2.0.0 (MPL-2.0, unmodified, pinned) decides each placement; the bottom-left fill is the fallback
//! when jagua-rs cannot run for an input. Both results must pass the checker. Other dependencies: nf-dxf (read
//! the part outlines and write the CAM DXF), encoding_rs (Shift_JIS for the R12 hand-off). See NOTICES.

pub mod check;
pub mod dxf_out;
pub mod geom;
pub mod jagua;
pub mod outline;
pub mod pipes;

#[cfg(test)]
mod tests;

use geom::P;
use std::time::{Duration, Instant};

/// Rule names as they appear in the output. All are UNVERIFIED until a factory confirms them.
pub const RULE_STOCK: &str = "NEST_STOCK_SIZE";
pub const RULE_KERF: &str = "NEST_KERF_MM";
pub const RULE_GAP: &str = "NEST_PART_GAP_MM";
pub const RULE_EDGE: &str = "NEST_EDGE_MM";
pub const RULE_ROT: &str = "NEST_ROTATION_STEPS";
pub const MAX_PARTS: usize = 500;
/// Most copies placed in one nest (the sum of every part's quantity). Placement is quadratic in the copies.
pub const MAX_COPIES: u64 = 10_000;
/// Wall-time budget of one nest. Past it the nest is refused with code `nest_timeout`; there is no partial result.
pub const NEST_BUDGET: Duration = Duration::from_secs(20);

/// A refusal: a stable code (for the client) and a message that names the rule, part or limit it broke.
#[derive(Clone, Debug, PartialEq)]
pub struct NestError {
    pub code: &'static str,
    pub msg: String,
}

impl NestError {
    /// A refusal with the generic code `nest_refused`.
    pub fn new(msg: impl Into<String>) -> NestError {
        NestError::coded("nest_refused", msg)
    }

    pub fn coded(code: &'static str, msg: impl Into<String>) -> NestError {
        NestError {
            code,
            msg: msg.into(),
        }
    }
}

impl std::fmt::Display for NestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

/// Fails with `nest_timeout` once the deadline has passed.
pub(crate) fn out_of_time(deadline: Instant) -> Result<(), NestError> {
    if Instant::now() >= deadline {
        Err(NestError::coded(
            "nest_timeout",
            format!(
                "the nest ran past its {} s budget; fewer parts or copies per nest",
                NEST_BUDGET.as_secs()
            ),
        ))
    } else {
        Ok(())
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
            Err(NestError::new(format!(
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
            return Err(NestError::new(format!(
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

/// One placed copy. `pose` is the centroid position on the sheet; `pts` is the placed outline.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub id: String,
    pub copy: u32,
    pub rot: u32,
    pub pose: P,
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
    /// The engine that placed the parts: jagua-rs, or the bottom-left fill when jagua-rs could not run.
    pub engine: String,
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

/// Each copy with its centred outline, largest first (the placement order of both engines).
pub(crate) fn prepare(parts: &[PartIn]) -> Result<Vec<(usize, u32, Vec<P>)>, NestError> {
    let mut out: Vec<(usize, u32, Vec<P>)> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let c = geom::centre(&p.outline);
        if c.len() < 3 || geom::area(&c) <= 0.0 {
            return Err(NestError::coded(
                "nest_outline",
                format!("part {} has no usable outline", p.id),
            ));
        }
        if !check::outline_is_simple(&c) {
            return Err(NestError::coded(
                "nest_outline",
                format!("part {} outline crosses itself; it cannot be nested", p.id),
            ));
        }
        for k in 0..p.qty {
            out.push((i, k, c.clone()));
        }
    }
    out.sort_by(|a, b| {
        geom::area(&b.2)
            .total_cmp(&geom::area(&a.2))
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });
    Ok(out)
}

/// Nest with jagua-rs within `NEST_BUDGET`. Refused with a code when the input is over a limit or runs out of time.
/// If jagua-rs cannot run for these inputs (an engine error, or a panic inside it), the bottom-left fill is used and
/// `engine` says why. Either result must still pass `check::check`.
pub fn nest(parts: &[PartIn], rules: &Rules) -> Result<Nest, NestError> {
    nest_within(parts, rules, NEST_BUDGET)
}

/// `nest` with its own wall-time budget.
pub fn nest_within(parts: &[PartIn], rules: &Rules, budget: Duration) -> Result<Nest, NestError> {
    nest_with_engine(parts, rules, Instant::now() + budget, jagua::nest)
}

pub(crate) type Engine =
    fn(&[PartIn], &Rules, &[(usize, u32, Vec<P>)], Instant) -> Result<Nest, NestError>;

/// The nest with a given primary engine. Split out so that the fallback (an engine error or panic) can be tested.
pub(crate) fn nest_with_engine(
    parts: &[PartIn],
    rules: &Rules,
    deadline: Instant,
    engine: Engine,
) -> Result<Nest, NestError> {
    rules.validate()?;
    if parts.len() > MAX_PARTS {
        return Err(NestError::coded(
            "nest_parts",
            format!("at most {MAX_PARTS} part kinds per nest"),
        ));
    }
    let copies: u64 = parts.iter().map(|p| u64::from(p.qty)).sum();
    if copies > MAX_COPIES {
        return Err(NestError::coded(
            "nest_too_many_copies",
            format!("the nest asks for {copies} copies; at most {MAX_COPIES} per nest"),
        ));
    }
    let prepared = prepare(parts)?;
    out_of_time(deadline)?;
    let why = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine(parts, rules, &prepared, deadline)
    })) {
        Ok(Ok(n)) => return Ok(n),
        Ok(Err(e)) if e.code == "nest_timeout" => return Err(e),
        Ok(Err(e)) => format!("jagua-rs not used: {}", e.msg),
        Err(_) => "jagua-rs not used: it panicked".to_string(),
    };
    let mut n = blf(parts, rules, &prepared, deadline)?;
    n.engine = format!("bottom-left fill ({why})");
    Ok(n)
}

/// The candidate corners of a sheet, lowest then leftmost first: the sheet corner and the corners beside placed parts.
pub(crate) fn corners(sheet: &Sheet, clr: f64, edge: f64) -> Vec<P> {
    let mut cands: Vec<P> = vec![(edge, edge)];
    for p in &sheet.placed {
        let (x0, y0, x1, y1) = geom::bbox(&p.pts);
        cands.push((x1 + clr, y0));
        cands.push((x0, y1 + clr));
        cands.push((x1 + clr, edge));
        cands.push((edge, y1 + clr));
    }
    cands.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    cands
}

/// True when `a` is lower on the sheet than `b`, or level and further left.
pub(crate) fn lower_left(a: P, b: P) -> bool {
    a.1 < b.1 - geom::EPS || ((a.1 - b.1).abs() <= geom::EPS && a.0 < b.0 - geom::EPS)
}

/// Bottom-left fill (the fallback engine). Every placement is re-tested with the full outline clearance.
pub(crate) fn blf(
    parts: &[PartIn],
    rules: &Rules,
    prepared: &[(usize, u32, Vec<P>)],
    deadline: Instant,
) -> Result<Nest, NestError> {
    let clr = rules.clearance();
    let edge = rules.edge_mm;
    let mut sheets: Vec<Sheet> = Vec::new();
    for (i, copy, centred) in prepared {
        out_of_time(deadline)?;
        let mut done = false;
        for si in 0..=sheets.len() {
            if si == sheets.len() {
                sheets.push(Sheet::default());
            }
            if let Some((rot, t, pts)) = blf_position(&sheets[si], centred, rules, clr, edge) {
                sheets[si].placed.push(Placed {
                    id: parts[*i].id.clone(),
                    copy: *copy,
                    rot,
                    pose: t,
                    pts,
                });
                done = true;
                break;
            }
            if si + 1 == sheets.len() && sheets[si].placed.is_empty() {
                return Err(NestError::coded(
                    "nest_fit",
                    format!(
                        "part {} does not fit the stock sheet (rule {RULE_STOCK}, UNVERIFIED)",
                        parts[*i].id
                    ),
                ));
            }
        }
        if !done {
            return Err(NestError::coded(
                "nest_internal",
                "internal: a part was not placed",
            ));
        }
    }
    Ok(Nest {
        rules: rules.clone(),
        parts: parts.to_vec(),
        sheets,
        engine: "bottom-left fill".into(),
    })
}

/// Lowest then leftmost feasible (rotation, pose, outline) on one sheet, or None.
fn blf_position(
    sheet: &Sheet,
    centred: &[P],
    rules: &Rules,
    clr: f64,
    edge: f64,
) -> Option<(u32, P, Vec<P>)> {
    let cands = corners(sheet, clr, edge);
    let mut best: Option<(u32, P, Vec<P>, P)> = None;
    for &rot in &rules.rotations {
        let r = geom::rotate(centred, rot);
        let (x0, y0, x1, y1) = geom::bbox(&r);
        let (w, h) = (x1 - x0, y1 - y0);
        for &c in &cands {
            if c.0 + w > rules.sheet_w - edge + geom::EPS
                || c.1 + h > rules.sheet_h - edge + geom::EPS
            {
                continue;
            }
            if let Some((_, _, _, bc)) = &best {
                if !lower_left(c, *bc) {
                    break;
                }
            }
            let (t, pts) = geom::place_corner(centred, rot, c);
            if sheet.placed.iter().all(|p| geom::clear(&pts, &p.pts, clr)) {
                best = Some((rot, t, pts, c));
                break;
            }
        }
    }
    best.map(|(rot, t, pts, _)| (rot, t, pts))
}
