//! Entities and layers from nf-cadio's raw section pairs. Anything missing or not finite is recorded as a
//! warning on the entity and the entity is kept as `Unsupported`; nothing here panics on input.

use crate::geom::{Geom, Vtx, P};
use nf_cadio::Pair;

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    /// Stable id for this file: "E0", "E1", ... in file order.
    pub id: String,
    /// The entity type as written (LINE, CIRCLE, LWPOLYLINE, ...).
    pub kind: String,
    /// Group 5 handle when the file has one.
    pub handle: Option<String>,
    pub layer: String,
    pub geom: Geom,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Layer {
    pub name: String,
    /// The colour index, absolute value (ACI).
    pub colour: i64,
    pub on: bool,
    pub frozen: bool,
    /// Number of entities on this layer.
    pub count: usize,
}

/// One group 0 record: its type and its body pairs (every pair after the 0 pair).
struct Rec<'a> {
    kind: &'a str,
    body: &'a [Pair],
}

fn records(pairs: &[Pair]) -> Vec<Rec<'_>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].code == 0 {
            let start = i + 1;
            let mut j = start;
            while j < pairs.len() && pairs[j].code != 0 {
                j += 1;
            }
            out.push(Rec { kind: pairs[i].value.trim(), body: &pairs[start..j] });
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn get(body: &[Pair], code: u16) -> Option<&str> {
    body.iter().find(|p| p.code == code).map(|p| p.value.as_str())
}

/// A finite number from a group value, or None.
pub(crate) fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn coord(body: &[Pair], x: u16, y: u16, warn: &mut Vec<String>) -> Option<P> {
    match (get(body, x).map(num), get(body, y).map(num)) {
        (Some(Some(a)), Some(Some(b))) => Some([a, b]),
        _ => {
            warn.push(format!("missing or non-finite coordinate (group {x}/{y})"));
            None
        }
    }
}

fn scalar(body: &[Pair], code: u16, warn: &mut Vec<String>) -> Option<f64> {
    match get(body, code).map(num) {
        Some(Some(v)) => Some(v),
        _ => {
            warn.push(format!("missing or non-finite value (group {code})"));
            None
        }
    }
}

/// Builds the geometry of one record. `None` means the record is kept as Unsupported.
fn geometry(kind: &str, body: &[Pair], warn: &mut Vec<String>) -> Geom {
    let g = match kind {
        "POINT" => coord(body, 10, 20, warn).map(|at| Geom::Point { at }),
        "LINE" => match (coord(body, 10, 20, warn), coord(body, 11, 21, warn)) {
            (Some(a), Some(b)) => Some(Geom::Line { a, b }),
            _ => None,
        },
        "CIRCLE" => match (coord(body, 10, 20, warn), scalar(body, 40, warn)) {
            (Some(c), Some(r)) if r > 0.0 => Some(Geom::Circle { c, r }),
            (Some(_), Some(_)) => {
                warn.push("radius is not positive".into());
                None
            }
            _ => None,
        },
        "ARC" => match (coord(body, 10, 20, warn), scalar(body, 40, warn), scalar(body, 50, warn), scalar(body, 51, warn)) {
            (Some(c), Some(r), Some(start), Some(end)) if r > 0.0 => {
                Some(Geom::Arc { c, r, start, end })
            }
            (Some(_), Some(_), Some(_), Some(_)) => {
                warn.push("radius is not positive".into());
                None
            }
            _ => None,
        },
        "LWPOLYLINE" => {
            let flags = get(body, 70).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
            let mut v: Vec<Vtx> = Vec::new();
            for p in body {
                match p.code {
                    10 => v.push(Vtx { p: [num(&p.value).unwrap_or(f64::NAN), 0.0], bulge: 0.0 }),
                    20 => match v.last_mut() {
                        Some(last) => last.p[1] = num(&p.value).unwrap_or(f64::NAN),
                        None => warn.push("vertex Y before X".into()),
                    },
                    42 => {
                        if let Some(last) = v.last_mut() {
                            last.bulge = num(&p.value).unwrap_or(0.0);
                        }
                    }
                    _ => {}
                }
            }
            poly(v, flags & 1 == 1, warn)
        }
        "TEXT" | "MTEXT" => match coord(body, 10, 20, warn) {
            Some(at) => {
                let text = if kind == "MTEXT" {
                    let mut s: String = body.iter().filter(|p| p.code == 3).map(|p| p.value.as_str()).collect();
                    s.push_str(get(body, 1).unwrap_or(""));
                    s
                } else {
                    get(body, 1).unwrap_or("").to_string()
                };
                let height = num(get(body, 40).unwrap_or("0")).unwrap_or(0.0).max(0.0);
                Some(Geom::Text { at, height, text })
            }
            None => None,
        },
        _ => None,
    };
    g.unwrap_or(Geom::Unsupported)
}

fn poly(v: Vec<Vtx>, closed: bool, warn: &mut Vec<String>) -> Option<Geom> {
    if v.iter().any(|x| !x.p[0].is_finite() || !x.p[1].is_finite()) {
        warn.push("polyline vertex is not finite".into());
        return None;
    }
    if v.len() < 2 {
        warn.push("polyline has fewer than two vertices".into());
        return None;
    }
    Some(Geom::Poly { v, closed })
}

/// Reads ENTITIES (and the POLYLINE VERTEX sequences inside it). `limit` caps the number of entities.
pub fn entities(pairs: &[Pair], limit: usize) -> Result<Vec<Entity>, String> {
    let recs = records(pairs);
    let mut out: Vec<Entity> = Vec::new();
    let mut i = 0;
    while i < recs.len() {
        let r = &recs[i];
        i += 1;
        if r.kind == "POLYLINE" {
            // header record, then VERTEX records, then SEQEND
            let mut warn = Vec::new();
            let flags = get(r.body, 70).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
            let mut v: Vec<Vtx> = Vec::new();
            let mut closed_by_seqend = false;
            while i < recs.len() {
                if recs[i].kind == "SEQEND" {
                    closed_by_seqend = true;
                    break;
                }
                if recs[i].kind == "VERTEX" {
                    let mut w = Vec::new();
                    match coord(recs[i].body, 10, 20, &mut w) {
                        Some(p) => {
                            let bulge = get(recs[i].body, 42).and_then(num).unwrap_or(0.0);
                            v.push(Vtx { p, bulge });
                        }
                        None => warn.push("polyline vertex without coordinates".into()),
                    }
                }
                i += 1;
            }
            if closed_by_seqend {
                i += 1; // SEQEND
            } else {
                warn.push("POLYLINE without SEQEND: the records after it to the end of ENTITIES were consumed, not read".into());
            }
            let geom = poly(v, flags & 1 == 1, &mut warn).unwrap_or(Geom::Unsupported);
            push(&mut out, limit, "POLYLINE", r.body, geom, warn)?;
            continue;
        }
        if r.kind == "SEQEND" || r.kind == "VERTEX" {
            continue;
        }
        let mut warn = Vec::new();
        let geom = geometry(r.kind, r.body, &mut warn);
        push(&mut out, limit, r.kind, r.body, geom, warn)?;
    }
    Ok(out)
}

fn push(out: &mut Vec<Entity>, limit: usize, kind: &str, body: &[Pair], geom: Geom, warnings: Vec<String>) -> Result<(), String> {
    if out.len() >= limit {
        return Err(format!("more than {limit} entities"));
    }
    let layer = get(body, 8).unwrap_or("0").to_string();
    let handle = get(body, 5).map(|h| h.trim().to_string()).filter(|h| !h.is_empty());
    out.push(Entity {
        id: format!("E{}", out.len()),
        kind: kind.to_string(),
        handle,
        layer,
        geom,
        warnings,
    });
    Ok(())
}

/// LAYER records of the TABLES section.
pub fn layers(tables: &[Pair]) -> Vec<Layer> {
    records(tables)
        .into_iter()
        .filter(|r| r.kind == "LAYER")
        .filter_map(|r| {
            let name = get(r.body, 2)?.trim().to_string();
            let raw = get(r.body, 62).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(7);
            let flags = get(r.body, 70).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
            Some(Layer { name, colour: raw.abs(), on: raw >= 0, frozen: flags & 1 == 1, count: 0 })
        })
        .collect()
}
