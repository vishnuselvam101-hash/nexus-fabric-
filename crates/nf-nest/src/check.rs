//! The independent nest checker. It re-derives every placed outline from the certified inputs (the part outlines
//! the caller passes in, never the list the nester returns) and the stored rotation and corner, then tests the result
//! with its own geometry (not the placer's functions). Only a nest that passes `check` can become a `CheckedNest`,
//! and only a `CheckedNest` can be written to DXF.

use crate::pipes::{PipeIn, PipePlan};
use crate::{Nest, NestError, PartIn, Rules};

const TOL: f64 = 1e-6;
/// Tolerance on the clearance, mm: the engine works in f32, so its placements can sit 1e-4 mm off the f64 outline.
const CLEARANCE_TOL: f64 = 1e-3;

/// Proof that a nest passed `check`. The field is private: no other code can build one.
#[derive(Debug)]
pub struct CheckedNest {
    nest: Nest,
}

impl CheckedNest {
    pub fn nest(&self) -> &Nest {
        &self.nest
    }
}

/// Proof that a pipe cut list passed `check_pipes`.
#[derive(Debug)]
pub struct CheckedPipes {
    plan: PipePlan,
}

impl CheckedPipes {
    pub fn plan(&self) -> &PipePlan {
        &self.plan
    }
}

/// Own rotation (not the placer's): 90 degree steps as coordinate swaps.
fn turn(p: (f64, f64), deg: u32) -> (f64, f64) {
    match deg {
        0 => p,
        90 => (-p.1, p.0),
        180 => (-p.0, -p.1),
        270 => (p.1, -p.0),
        _ => (f64::NAN, f64::NAN),
    }
}

/// Own area centroid (not the placer's), used to re-derive the centred outline.
fn own_centroid(p: &[(f64, f64)]) -> (f64, f64) {
    let n = p.len();
    let (mut a2, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x0, y0) = p[i];
        let (x1, y1) = p[(i + 1) % n];
        let k = x0 * y1 - x1 * y0;
        a2 += k;
        sx += (x0 + x1) * k;
        sy += (y0 + y1) * k;
    }
    (sx / (3.0 * a2), sy / (3.0 * a2))
}

/// The outline a placement must show: the input, centred on its centroid, turned by `rot` and moved to `t`.
fn expected_outline(input: &[(f64, f64)], rot: u32, t: (f64, f64)) -> Vec<(f64, f64)> {
    let c = own_centroid(input);
    input
        .iter()
        .map(|&(x, y)| turn((x - c.0, y - c.1), rot))
        .map(|p| (p.0 + t.0, p.1 + t.1))
        .collect()
}

fn same_outline(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    a.len() == b.len()
        && a.iter().all(|p| {
            b.iter()
                .any(|q| (p.0 - q.0).abs() < TOL && (p.1 - q.1).abs() < TOL)
        })
}

/// Winding-number point in polygon (independent of the placer's ray casting).
fn winding_inside(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut wn = 0i32;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let cr = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
        if a.1 <= p.1 {
            if b.1 > p.1 && cr > 0.0 {
                wn += 1;
            }
        } else if b.1 <= p.1 && cr < 0.0 {
            wn -= 1;
        }
    }
    wn != 0
}

fn point_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let (wx, wy) = (p.0 - a.0, p.1 - a.1);
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 0.0 {
        ((wx * vx + wy * vy) / len2).max(0.0).min(1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a.0 + t * vx - p.0, a.1 + t * vy - p.1);
    (cx * cx + cy * cy).sqrt()
}

fn orient(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let (d1, d2) = (orient(c, d, a), orient(c, d, b));
    let (d3, d4) = (orient(a, b, c), orient(a, b, d));
    (d1 * d2 < 0.0 && d3 * d4 < 0.0)
        || (d1.abs() < TOL && point_seg(a, c, d) < TOL)
        || (d2.abs() < TOL && point_seg(b, c, d) < TOL)
        || (d3.abs() < TOL && point_seg(c, a, b) < TOL)
        || (d4.abs() < TOL && point_seg(d, a, b) < TOL)
}

/// Smallest distance between two closed outlines; 0 when any edges cross.
fn outline_gap(a: &[(f64, f64)], b: &[(f64, f64)]) -> f64 {
    let mut best = f64::MAX;
    for i in 0..a.len() {
        let (p, q) = (a[i], a[(i + 1) % a.len()]);
        for j in 0..b.len() {
            let (r, s) = (b[j], b[(j + 1) % b.len()]);
            if segments_cross(p, q, r, s) {
                return 0.0;
            }
            best = best
                .min(point_seg(p, r, s))
                .min(point_seg(q, r, s))
                .min(point_seg(r, p, q))
                .min(point_seg(s, p, q));
        }
    }
    best
}

fn find<'a>(parts: &'a [PartIn], id: &str) -> Option<&'a PartIn> {
    parts.iter().find(|p| p.id == id)
}

/// True when the closed outline has no finite-coordinate fault and no two non-adjacent edges touch or cross.
pub fn outline_is_simple(p: &[(f64, f64)]) -> bool {
    let n = p.len();
    if n < 3 || p.iter().any(|q| !q.0.is_finite() || !q.1.is_finite()) {
        return false;
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if segments_cross(p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

/// Full nest check against the certified inputs and the rules asked for. Returns every violation found, or a
/// `CheckedNest`. The nest must have been made from exactly these inputs and rules: its own copy of either is not
/// trusted.
pub fn check(inputs: &[PartIn], rules: &Rules, n: Nest) -> Result<CheckedNest, Vec<String>> {
    let mut bad: Vec<String> = Vec::new();
    if let Err(NestError { msg, .. }) = rules.validate() {
        bad.push(msg);
        return Err(bad);
    }
    if n.rules != *rules {
        bad.push("the nest was made with other rules than the rules asked for".into());
    }
    if n.parts != inputs {
        bad.push("the nest's parts are not the certified input parts".into());
    }
    for p in inputs {
        if !outline_is_simple(&p.outline) {
            bad.push(format!("part {} outline crosses itself", p.id));
        }
    }
    let r = rules;
    let clr = r.clearance();

    // every part placed exactly once per copy
    for p in inputs {
        let mut copies: Vec<u32> = n
            .sheets
            .iter()
            .flat_map(|s| s.placed.iter().filter(|q| q.id == p.id).map(|q| q.copy))
            .collect();
        copies.sort_unstable();
        let want: Vec<u32> = (0..p.qty).collect();
        if copies != want {
            bad.push(format!(
                "part {} placed {} time(s), needs {} copy(ies) exactly once",
                p.id,
                copies.len(),
                p.qty
            ));
        }
    }
    for s in &n.sheets {
        for q in &s.placed {
            if find(inputs, &q.id).is_none() {
                bad.push(format!("placed part {} is not in the input", q.id));
            }
        }
    }

    for (si, s) in n.sheets.iter().enumerate() {
        for q in &s.placed {
            let Some(src) = find(inputs, &q.id) else {
                continue;
            };
            if !r.rotations.contains(&q.rot) {
                bad.push(format!(
                    "sheet {} part {} uses rotation {} not allowed",
                    si + 1,
                    q.id,
                    q.rot
                ));
            }
            let want = expected_outline(&src.outline, q.rot, q.pose);
            if !same_outline(&q.pts, &want) {
                bad.push(format!(
                    "sheet {} part {} does not match its input outline at its stored place",
                    si + 1,
                    q.id
                ));
            }
            for &(x, y) in &q.pts {
                if x < r.edge_mm - TOL
                    || y < r.edge_mm - TOL
                    || x > r.sheet_w - r.edge_mm + TOL
                    || y > r.sheet_h - r.edge_mm + TOL
                {
                    bad.push(format!(
                        "sheet {} part {} is outside the sheet or its edge margin",
                        si + 1,
                        q.id
                    ));
                    break;
                }
            }
        }
        for i in 0..s.placed.len() {
            for j in (i + 1)..s.placed.len() {
                let (a, b) = (&s.placed[i].pts, &s.placed[j].pts);
                let gap = outline_gap(a, b);
                let overlap = a.iter().any(|&p| winding_inside(p, b))
                    || b.iter().any(|&p| winding_inside(p, a));
                if overlap {
                    bad.push(format!(
                        "sheet {}: parts {} and {} overlap",
                        si + 1,
                        s.placed[i].id,
                        s.placed[j].id
                    ));
                } else if gap < clr - CLEARANCE_TOL {
                    bad.push(format!(
                        "sheet {}: parts {} and {} are {:.3} mm apart, need {:.3} mm (kerf + gap)",
                        si + 1,
                        s.placed[i].id,
                        s.placed[j].id,
                        gap,
                        clr
                    ));
                }
            }
        }
    }
    if bad.is_empty() {
        Ok(CheckedNest { nest: n })
    } else {
        Err(bad)
    }
}

/// Pipe cut list check: each bar's cuts plus the kerf between them fit the stock, every cut is used exactly once.
pub fn check_pipes(plan: PipePlan, items: &[PipeIn]) -> Result<CheckedPipes, Vec<String>> {
    let mut bad: Vec<String> = Vec::new();
    for it in items {
        let mut n = 0u32;
        for b in &plan.bars {
            n += b.cuts.iter().filter(|(id, _)| *id == it.id).count() as u32;
        }
        if n != it.qty {
            bad.push(format!(
                "pipe {} cut {} time(s), needs {}",
                it.id, n, it.qty
            ));
        }
        for b in &plan.bars {
            for (id, len) in &b.cuts {
                if id == &it.id && (*len - it.len_mm).abs() > TOL {
                    bad.push(format!(
                        "pipe {} cut at {len} mm, input is {} mm",
                        it.id, it.len_mm
                    ));
                }
            }
        }
    }
    for (bi, b) in plan.bars.iter().enumerate() {
        let sum: f64 = b.cuts.iter().map(|c| c.1).sum();
        let used = sum + plan.kerf_mm * (b.cuts.len().saturating_sub(1)) as f64;
        if used > plan.stock_mm + TOL {
            bad.push(format!(
                "bar {} needs {used:.3} mm, stock is {:.3} mm",
                bi + 1,
                plan.stock_mm
            ));
        }
    }
    if bad.is_empty() {
        Ok(CheckedPipes { plan })
    } else {
        Err(bad)
    }
}
