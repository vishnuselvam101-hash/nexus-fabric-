//! Entities and layers from nf-cadio's raw section pairs. Anything missing or not finite is recorded as a
//! warning on the entity and the entity is kept as `Unsupported`; nothing here panics on input.
//!
//! Blocks: INSERT is listed as an entity and its block contents are expanded beneath it (children carry
//! `parent` = the INSERT's id, with the transform applied). DIMENSION's anonymous block is expanded the same way.
//! Caps: total entities (`limit`), nesting depth (`MAX_DEPTH`) and cycles (a block that contains itself) refuse
//! the whole file; nothing is truncated.

use crate::geom::{self, Affine, Geom, Vtx, CHORD_TOL, MAX_CURVE_POINTS, P};
use nf_cadio::Pair;
use std::collections::HashMap;

/// Nesting depth of INSERT / DIMENSION blocks above which the file is refused.
pub const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    /// Stable id for this file: "E0", "E1", ... in expansion order.
    pub id: String,
    /// The entity type as written (LINE, CIRCLE, LWPOLYLINE, INSERT, ...).
    pub kind: String,
    /// Group 5 handle when the file has one.
    pub handle: Option<String>,
    pub layer: String,
    pub geom: Geom,
    pub warnings: Vec<String>,
    /// Index of the entity's group 0 record in the ENTITIES section (top-level entities only). Edits address
    /// entities through this index.
    pub record: Option<usize>,
    /// The id of the INSERT or DIMENSION this entity was expanded from (None for top-level entities).
    pub parent: Option<String>,
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
pub(crate) struct Rec<'a> {
    pub kind: &'a str,
    pub body: &'a [Pair],
}

pub(crate) fn records(pairs: &[Pair]) -> Vec<Rec<'_>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].code == 0 {
            let start = i + 1;
            let mut j = start;
            while j < pairs.len() && pairs[j].code != 0 {
                j += 1;
            }
            out.push(Rec {
                kind: pairs[i].value.trim(),
                body: &pairs[start..j],
            });
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn get(body: &[Pair], code: u16) -> Option<&str> {
    body.iter()
        .find(|p| p.code == code)
        .map(|p| p.value.as_str())
}

fn getf(body: &[Pair], code: u16) -> Option<f64> {
    get(body, code).and_then(num)
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

/// One entity record of a section, with its polyline vertices gathered (POLYLINE) and its position.
pub(crate) struct Primary<'a> {
    pub kind: &'a str,
    pub body: &'a [Pair],
    pub verts: Vec<Vtx>,
    pub record: usize,
    pub warnings: Vec<String>,
}

/// The entity records of a section, in order. POLYLINE swallows its VERTEX records and SEQEND; stray VERTEX and
/// SEQEND records are skipped. `record` is the index among all group 0 records of the section.
pub(crate) fn primaries<'a>(recs: &[Rec<'a>]) -> Vec<Primary<'a>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < recs.len() {
        let r = &recs[i];
        let idx = i;
        i += 1;
        if r.kind == "POLYLINE" {
            let mut warn = Vec::new();
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
                        Some(p) => v.push(Vtx {
                            p,
                            bulge: getf(recs[i].body, 42).unwrap_or(0.0),
                        }),
                        None => warn.push("polyline vertex without coordinates".into()),
                    }
                }
                i += 1;
            }
            if closed_by_seqend {
                i += 1;
            } else {
                warn.push("POLYLINE without SEQEND: the records after it to the end of the section were consumed, not read".into());
            }
            out.push(Primary {
                kind: "POLYLINE",
                body: r.body,
                verts: v,
                record: idx,
                warnings: warn,
            });
        } else if r.kind == "SEQEND"
            || r.kind == "VERTEX"
            || r.kind == "BLOCK"
            || r.kind == "ENDBLK"
        {
            continue;
        } else {
            out.push(Primary {
                kind: r.kind,
                body: r.body,
                verts: Vec::new(),
                record: idx,
                warnings: Vec::new(),
            });
        }
    }
    out
}

/// The geometry of one primary record, in its own coordinates. Empty means: listed, not read (Unsupported).
fn geometry(p: &Primary<'_>, warn: &mut Vec<String>) -> Vec<Geom> {
    let body = p.body;
    let one = |g: Option<Geom>| g.map(|g| vec![g]).unwrap_or_default();
    match p.kind {
        "POINT" => one(coord(body, 10, 20, warn).map(|at| Geom::Point { at })),
        "LINE" => match (coord(body, 10, 20, warn), coord(body, 11, 21, warn)) {
            (Some(a), Some(b)) => vec![Geom::Line { a, b }],
            _ => vec![],
        },
        "CIRCLE" => match (coord(body, 10, 20, warn), scalar(body, 40, warn)) {
            (Some(c), Some(r)) if r > 0.0 => vec![Geom::Circle { c, r }],
            (Some(_), Some(_)) => {
                warn.push("radius is not positive".into());
                vec![]
            }
            _ => vec![],
        },
        "ARC" => match (
            coord(body, 10, 20, warn),
            scalar(body, 40, warn),
            scalar(body, 50, warn),
            scalar(body, 51, warn),
        ) {
            (Some(c), Some(r), Some(start), Some(end)) if r > 0.0 => {
                vec![Geom::Arc { c, r, start, end }]
            }
            (Some(_), Some(_), Some(_), Some(_)) => {
                warn.push("radius is not positive".into());
                vec![]
            }
            _ => vec![],
        },
        "LWPOLYLINE" => {
            let flags = getf(body, 70).unwrap_or(0.0) as i64;
            let mut v: Vec<Vtx> = Vec::new();
            for q in body {
                match q.code {
                    10 => v.push(Vtx {
                        p: [num(&q.value).unwrap_or(f64::NAN), 0.0],
                        bulge: 0.0,
                    }),
                    20 => match v.last_mut() {
                        Some(last) => last.p[1] = num(&q.value).unwrap_or(f64::NAN),
                        None => warn.push("vertex Y before X".into()),
                    },
                    42 => {
                        if let Some(last) = v.last_mut() {
                            last.bulge = num(&q.value).unwrap_or(0.0);
                        }
                    }
                    _ => {}
                }
            }
            one(poly(v, flags & 1 == 1, warn))
        }
        "POLYLINE" => {
            let flags = getf(body, 70).unwrap_or(0.0) as i64;
            one(poly(p.verts.clone(), flags & 1 == 1, warn))
        }
        "TEXT" | "MTEXT" => match coord(body, 10, 20, warn) {
            Some(at) => {
                let text = if p.kind == "MTEXT" {
                    let mut s: String = body
                        .iter()
                        .filter(|q| q.code == 3)
                        .map(|q| q.value.as_str())
                        .collect();
                    s.push_str(get(body, 1).unwrap_or(""));
                    s
                } else {
                    get(body, 1).unwrap_or("").to_string()
                };
                let height = getf(body, 40).unwrap_or(0.0).max(0.0);
                vec![Geom::Text { at, height, text }]
            }
            None => vec![],
        },
        "SPLINE" => spline(body, warn).map(|g| vec![g]).unwrap_or_default(),
        "ELLIPSE" => ellipse(body, warn).map(|g| vec![g]).unwrap_or_default(),
        "HATCH" => hatch(body, warn),
        "DIMENSION" => match coord(body, 11, 21, warn) {
            Some(at) => {
                let measured = getf(body, 42);
                let fmt = measured.map(fmt_value).unwrap_or_else(|| "?".into());
                let text = match get(body, 1).map(str::trim) {
                    Some(t) if !t.is_empty() => t.replace("<>", &fmt),
                    _ => fmt,
                };
                vec![Geom::Text {
                    at,
                    height: 0.0,
                    text,
                }]
            }
            None => vec![],
        },
        _ => vec![],
    }
}

fn fmt_value(v: f64) -> String {
    let s = format!("{v:.4}");
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" {
        "0".into()
    } else {
        t.into()
    }
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

/// Samples a curve to a polyline with the chord tolerance, or warns and returns None.
fn sampled(
    f: &dyn Fn(f64) -> P,
    t0: f64,
    t1: f64,
    warn: &mut Vec<String>,
    what: &str,
) -> Option<Vec<P>> {
    match geom::sample_curve(f, t0, t1, CHORD_TOL, MAX_CURVE_POINTS) {
        Ok(pts) => {
            warn.push(format!(
                "{what} evaluated to a polyline, chord tolerance {CHORD_TOL} drawing units"
            ));
            Some(pts)
        }
        Err(e) => {
            warn.push(format!("{what} not evaluated: {e}"));
            None
        }
    }
}

fn to_poly(pts: Vec<P>, closed: bool) -> Option<Geom> {
    let mut pts = pts;
    if closed && pts.len() > 1 && geom::dist(pts[0], pts[pts.len() - 1]) < 1e-9 {
        pts.pop();
    }
    if pts.len() < 2 {
        return None;
    }
    Some(Geom::Poly {
        v: pts.into_iter().map(|p| Vtx { p, bulge: 0.0 }).collect(),
        closed,
    })
}

/// SPLINE: NURBS evaluated by de Boor (weights from group 41, knots from 40). A fit-point-only spline is drawn
/// through its fit points, with a warning, because its curve is not stored.
fn spline(body: &[Pair], warn: &mut Vec<String>) -> Option<Geom> {
    let flags = getf(body, 70).unwrap_or(0.0) as i64;
    let closed = flags & 1 == 1;
    let deg = getf(body, 71).unwrap_or(3.0) as usize;
    let mut knots = Vec::new();
    let mut weights = Vec::new();
    let mut ctrl: Vec<P> = Vec::new();
    let mut fit: Vec<P> = Vec::new();
    for q in body {
        match q.code {
            40 => knots.push(num(&q.value)),
            41 => weights.push(num(&q.value)),
            10 => ctrl.push([num(&q.value).unwrap_or(f64::NAN), 0.0]),
            20 => match ctrl.last_mut() {
                Some(l) => l[1] = num(&q.value).unwrap_or(f64::NAN),
                None => warn.push("spline control Y before X".into()),
            },
            11 => fit.push([num(&q.value).unwrap_or(f64::NAN), 0.0]),
            21 => match fit.last_mut() {
                Some(l) => l[1] = num(&q.value).unwrap_or(f64::NAN),
                None => warn.push("spline fit Y before X".into()),
            },
            _ => {}
        }
    }
    if ctrl.is_empty() && fit.len() >= 2 {
        warn.push(
            "spline stored by fit points: drawn through its fit points, not its exact curve".into(),
        );
        let pts = fit;
        if pts.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
            warn.push("spline fit point is not finite".into());
            return None;
        }
        return to_poly(pts, closed);
    }
    if ctrl.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
        warn.push("spline control point is not finite".into());
        return None;
    }
    let knots: Vec<f64> = match knots.into_iter().collect::<Option<Vec<f64>>>() {
        Some(k) => k,
        None => {
            warn.push("spline knot is not finite".into());
            return None;
        }
    };
    let n = ctrl.len();
    if n <= deg || deg == 0 || deg > 9 || knots.len() != n + deg + 1 {
        warn.push("spline knot vector or degree is inconsistent: not evaluated".into());
        return None;
    }
    let w: Vec<f64> = if weights.len() == n {
        match weights.into_iter().collect::<Option<Vec<f64>>>() {
            Some(w) if w.iter().all(|x| *x > 0.0) => w,
            _ => {
                warn.push("spline weight is not positive: not evaluated".into());
                return None;
            }
        }
    } else {
        vec![1.0; n]
    };
    if knots.windows(2).any(|k| k[1] < k[0]) {
        warn.push("spline knots are not non-decreasing: not evaluated".into());
        return None;
    }
    let (t0, t1) = (knots[deg], knots[n]);
    let f = |t: f64| geom::nurbs_at(deg, &knots, &ctrl, &w, t);
    let pts = sampled(&f, t0, t1, warn, "spline")?;
    to_poly(pts, closed)
}

/// ELLIPSE: centre, major axis vector from the centre, minor/major ratio, parametric start/end in radians.
fn ellipse(body: &[Pair], warn: &mut Vec<String>) -> Option<Geom> {
    let c = coord(body, 10, 20, warn)?;
    let a = coord(body, 11, 21, warn)?;
    let ratio = scalar(body, 40, warn)?;
    let start = getf(body, 41).unwrap_or(0.0);
    let mut end = getf(body, 42).unwrap_or(std::f64::consts::TAU);
    if !(ratio > 0.0 && ratio <= 1.0) || geom::dist([0.0, 0.0], a) == 0.0 {
        warn.push("ellipse axis or ratio is degenerate: not evaluated".into());
        return None;
    }
    if end <= start {
        end += std::f64::consts::TAU;
    }
    let full = end - start >= std::f64::consts::TAU - 1e-12;
    let f = |t: f64| geom::ellipse_at(c, a, ratio, t);
    let pts = sampled(
        &f,
        start,
        if full {
            start + std::f64::consts::TAU
        } else {
            end
        },
        warn,
        "ellipse",
    )?;
    to_poly(pts, full)
}

/// HATCH boundaries (read-only): one Geom per loop. Polyline loops are exact; edge loops made of lines and arcs
/// are sampled. A loop with an ellipse or spline edge is listed, not read.
fn hatch(body: &[Pair], warn: &mut Vec<String>) -> Vec<Geom> {
    let loops = getf(body, 91).unwrap_or(0.0).max(0.0) as usize;
    let mut i = body
        .iter()
        .position(|q| q.code == 91)
        .map_or(body.len(), |p| p + 1);
    let mut out = Vec::new();
    let next = |i: &mut usize, code: u16| -> Option<&str> {
        while *i < body.len() && body[*i].code != code {
            *i += 1;
        }
        let v = body.get(*i).map(|q| q.value.as_str());
        *i += 1;
        v
    };
    for k in 0..loops {
        let Some(flags) = next(&mut i, 92).and_then(num) else {
            warn.push("hatch boundary without its type flags: rest not read".into());
            break;
        };
        let flags = flags as i64;
        let mut pts: Vec<P> = Vec::new();
        let mut verts: Vec<Vtx> = Vec::new();
        let mut closed = true;
        let mut ok = true;
        if flags & 2 != 0 {
            let bulge_flag = next(&mut i, 72).and_then(num).unwrap_or(0.0) as i64 != 0;
            closed = next(&mut i, 73).and_then(num).unwrap_or(1.0) as i64 != 0;
            let n = next(&mut i, 93).and_then(num).unwrap_or(0.0).max(0.0) as usize;
            for _ in 0..n {
                let (Some(x), Some(y)) = (
                    next(&mut i, 10).and_then(num),
                    next(&mut i, 20).and_then(num),
                ) else {
                    ok = false;
                    break;
                };
                let mut b = 0.0;
                if bulge_flag {
                    b = next(&mut i, 42).and_then(num).unwrap_or(0.0);
                }
                verts.push(Vtx {
                    p: [x, y],
                    bulge: b,
                });
            }
        } else {
            let n = next(&mut i, 93).and_then(num).unwrap_or(0.0).max(0.0) as usize;
            for _ in 0..n {
                let ty = next(&mut i, 72).and_then(num).unwrap_or(0.0) as i64;
                match ty {
                    1 => {
                        let a = (
                            next(&mut i, 10).and_then(num),
                            next(&mut i, 20).and_then(num),
                        );
                        let b = (
                            next(&mut i, 11).and_then(num),
                            next(&mut i, 21).and_then(num),
                        );
                        match (a, b) {
                            ((Some(ax), Some(ay)), (Some(bx), Some(by))) => {
                                pts.push([ax, ay]);
                                pts.push([bx, by]);
                            }
                            _ => ok = false,
                        }
                    }
                    2 => {
                        let c = (
                            next(&mut i, 10).and_then(num),
                            next(&mut i, 20).and_then(num),
                        );
                        let r = next(&mut i, 40).and_then(num);
                        let s = next(&mut i, 50).and_then(num);
                        let e = next(&mut i, 51).and_then(num);
                        let ccw = next(&mut i, 73).and_then(num).unwrap_or(1.0) as i64 != 0;
                        match (c, r, s, e) {
                            ((Some(cx), Some(cy)), Some(r), Some(s), Some(e)) if r > 0.0 => {
                                let (s, e) = if ccw { (s, e) } else { (e, s) };
                                let f = |t: f64| geom::point_at([cx, cy], r, t);
                                let sweep_end = if e <= s { e + 360.0 } else { e };
                                match geom::sample_curve(
                                    &|t| f(t),
                                    s.to_radians(),
                                    sweep_end.to_radians(),
                                    CHORD_TOL,
                                    MAX_CURVE_POINTS,
                                ) {
                                    Ok(v) => pts.extend(v),
                                    Err(_) => ok = false,
                                }
                            }
                            _ => ok = false,
                        }
                    }
                    _ => {
                        warn.push(format!(
                            "hatch boundary {} has an ellipse or spline edge: listed, not read",
                            k + 1
                        ));
                        ok = false;
                        break;
                    }
                }
            }
        }
        if !ok {
            warn.push(format!("hatch boundary {} could not be read", k + 1));
            continue;
        }
        if flags & 2 != 0 {
            if verts.len() < 2 {
                warn.push(format!(
                    "hatch boundary {} has fewer than two vertices",
                    k + 1
                ));
                continue;
            }
            out.push(Geom::Poly { v: verts, closed });
        } else if let Some(g) = to_poly(pts, true) {
            out.push(g);
        } else {
            warn.push(format!("hatch boundary {} has no points", k + 1));
        }
    }
    if out.is_empty() && loops == 0 {
        warn.push("hatch without boundaries".into());
    }
    out
}

/// Reads LAYER records of the TABLES section.
pub fn layers(tables: &[Pair]) -> Vec<Layer> {
    records(tables)
        .into_iter()
        .filter(|r| r.kind == "LAYER")
        .filter_map(|r| {
            let name = get(r.body, 2)?.trim().to_string();
            let raw = get(r.body, 62)
                .and_then(|s| s.trim().parse::<i64>().ok())
                .unwrap_or(7);
            let flags = get(r.body, 70)
                .and_then(|s| s.trim().parse::<i64>().ok())
                .unwrap_or(0);
            Some(Layer {
                name,
                colour: raw.abs(),
                on: raw >= 0,
                frozen: flags & 1 == 1,
                count: 0,
            })
        })
        .collect()
}

/// Why a file is refused: a stable code and a message.
pub type Refuse = (&'static str, String);

/// A block definition: its base point and its entity records (from BLOCKS).
pub struct Block<'a> {
    base: P,
    recs: Vec<Rec<'a>>,
}

/// The BLOCKS section, by name. Blocks are read in file order; a later block with the same name is ignored.
pub fn blocks(pairs: &[Pair]) -> HashMap<String, Block<'_>> {
    let recs = records(pairs);
    let mut out: HashMap<String, Block<'_>> = HashMap::new();
    let mut cur: Option<(String, P, Vec<Rec<'_>>)> = None;
    for r in recs {
        match r.kind {
            "BLOCK" => {
                if let Some((n, b, v)) = cur.take() {
                    out.entry(n).or_insert(Block { base: b, recs: v });
                }
                let name = get(r.body, 2).unwrap_or("").trim().to_string();
                let base = [
                    getf(r.body, 10).unwrap_or(0.0),
                    getf(r.body, 20).unwrap_or(0.0),
                ];
                cur = Some((name, base, Vec::new()));
            }
            "ENDBLK" => {
                if let Some((n, b, v)) = cur.take() {
                    out.entry(n).or_insert(Block { base: b, recs: v });
                }
            }
            _ => {
                if let Some((_, _, v)) = cur.as_mut() {
                    v.push(r);
                }
            }
        }
    }
    if let Some((n, b, v)) = cur.take() {
        out.entry(n).or_insert(Block { base: b, recs: v });
    }
    out
}

/// The transform an INSERT applies to its block: base point out, scale, array offset, rotation, insertion point.
fn insert_transform(
    body: &[Pair],
    base: P,
    col: usize,
    row: usize,
    warn: &mut Vec<String>,
) -> Affine {
    let ins = [getf(body, 10).unwrap_or(0.0), getf(body, 20).unwrap_or(0.0)];
    let sx = getf(body, 41).unwrap_or(1.0);
    let sy = getf(body, 42).unwrap_or(1.0);
    let rot = getf(body, 50).unwrap_or(0.0);
    let cs = getf(body, 44).unwrap_or(0.0);
    let rs = getf(body, 45).unwrap_or(0.0);
    if sx == 0.0 || sy == 0.0 {
        warn.push("zero scale: block collapsed".into());
    }
    Affine::translate(-base[0], -base[1])
        .then(&Affine::scale(sx, sy))
        .then(&Affine::translate(col as f64 * cs, row as f64 * rs))
        .then(&Affine::rotate(rot))
        .then(&Affine::translate(ins[0], ins[1]))
}

/// Maps a geometry through `xf`. Similarity maps keep circles and arcs exact; anything else is sampled.
fn place(g: &Geom, xf: &Affine, warn: &mut Vec<String>) -> Geom {
    let det_neg = xf.det() < 0.0;
    let s = xf.is_similarity();
    match g {
        Geom::Point { at } => Geom::Point { at: xf.apply(*at) },
        Geom::Line { a, b } => Geom::Line {
            a: xf.apply(*a),
            b: xf.apply(*b),
        },
        Geom::Text { at, height, text } => {
            let k = xf.det().abs().sqrt();
            Geom::Text {
                at: xf.apply(*at),
                height: height * k,
                text: text.clone(),
            }
        }
        Geom::Circle { c, r } => match s {
            Some(k) => Geom::Circle {
                c: xf.apply(*c),
                r: r * k,
            },
            None => sample_geom(g, xf, warn),
        },
        Geom::Arc { c, r, start, end } => match s {
            Some(k) => {
                let dir = |deg: f64| {
                    let (sn, cs) = deg.to_radians().sin_cos();
                    xf.linear([cs, sn])
                };
                let ang = |v: P| v[1].atan2(v[0]).to_degrees().rem_euclid(360.0);
                let (a, b) = (ang(dir(*start)), ang(dir(*end)));
                let (ns, ne) = if det_neg { (b, a) } else { (a, b) };
                Geom::Arc {
                    c: xf.apply(*c),
                    r: r * k,
                    start: ns,
                    end: ne,
                }
            }
            None => sample_geom(g, xf, warn),
        },
        Geom::Poly { v, closed } => match s {
            Some(_) => Geom::Poly {
                v: v.iter()
                    .map(|x| Vtx {
                        p: xf.apply(x.p),
                        bulge: if det_neg { -x.bulge } else { x.bulge },
                    })
                    .collect(),
                closed: *closed,
            },
            None => sample_geom(g, xf, warn),
        },
        Geom::Unsupported => Geom::Unsupported,
    }
}

/// Non-similarity placement: the curve is sampled in its own frame, then the points are mapped.
fn sample_geom(g: &Geom, xf: &Affine, warn: &mut Vec<String>) -> Geom {
    if !warn.iter().any(|w| w.starts_with("non-uniform")) {
        warn.push("non-uniform scale in a block: circles, arcs and polylines with bulges are evaluated to polylines".into());
    }
    let pts: Vec<P> = match g {
        Geom::Circle { c, r } => (0..=64)
            .map(|k| geom::point_at(*c, *r, k as f64 * 360.0 / 64.0))
            .collect(),
        Geom::Arc { c, r, start, end } => {
            let sw = geom::sweep_deg(*start, *end);
            let steps = ((sw / 2.0).ceil() as usize).max(2);
            (0..=steps)
                .map(|k| geom::point_at(*c, *r, start + sw * k as f64 / steps as f64))
                .collect()
        }
        Geom::Poly { v, closed } => {
            let n = v.len();
            let segs = if *closed { n } else { n.saturating_sub(1) };
            let mut out = Vec::new();
            for i in 0..segs {
                let (a, b) = (v[i].p, v[(i + 1) % n].p);
                out.push(a);
                if v[i].bulge.abs() > 1e-12 {
                    for k in 1..16 {
                        out.push(bulge_pt(a, b, v[i].bulge, k as f64 / 16.0));
                    }
                }
            }
            if !*closed {
                out.push(v[n - 1].p);
            }
            out
        }
        _ => vec![],
    };
    let mapped: Vec<P> = pts.into_iter().map(|p| xf.apply(p)).collect();
    let closed = matches!(g, Geom::Circle { .. }) || matches!(g, Geom::Poly { closed: true, .. });
    to_poly(mapped, closed).unwrap_or(Geom::Unsupported)
}

fn bulge_pt(a: P, b: P, bulge: f64, t: f64) -> P {
    let theta = 4.0 * bulge.atan();
    let chord = geom::dist(a, b);
    if theta.abs() < 1e-12 || chord == 0.0 {
        return [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    }
    let r = chord / (2.0 * (theta / 2.0).sin().abs());
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let left = [-(b[1] - a[1]) / chord, (b[0] - a[0]) / chord];
    let off = r * (theta / 2.0).cos() * theta.signum();
    let c = [mid[0] + left[0] * off, mid[1] + left[1] * off];
    let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
    let ang = a0 + theta * t;
    [c[0] + r * ang.cos(), c[1] + r * ang.sin()]
}

/// Builds the ENTITIES of a file (top-level records, with INSERT and DIMENSION blocks expanded beneath them).
/// `limit` caps the total number of entities, expanded ones included.
pub fn entities(
    pairs: &[Pair],
    blocks: &HashMap<String, Block<'_>>,
    limit: usize,
) -> Result<(Vec<Entity>, Vec<String>), Refuse> {
    let recs = records(pairs);
    let prims = primaries(&recs);
    let mut ex = Expander {
        blocks,
        out: Vec::new(),
        limit,
        warnings: Vec::new(),
    };
    for p in &prims {
        let mut warn = p.warnings.clone();
        let geoms = geometry(p, &mut warn);
        let layer = get(p.body, 8).unwrap_or("0").to_string();
        let handle = get(p.body, 5)
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty());
        if p.kind == "INSERT" || p.kind == "DIMENSION" {
            let name = get(p.body, 2).unwrap_or("").trim().to_string();
            let geoms = if geoms.is_empty() {
                vec![Geom::Unsupported]
            } else {
                geoms
            };
            let first = ex.push(
                p.kind,
                handle.clone(),
                &layer,
                geoms.into_iter().next().unwrap(),
                warn,
                Some(p.record),
                None,
            )?;
            if p.kind == "INSERT" {
                ex.expand_insert(&first, p, &name, &layer, 0, &mut Vec::new())?;
            } else if !name.is_empty() {
                let xf = Affine::identity();
                ex.expand_block(&first, &name, &xf, &layer, 1, &mut Vec::new())?;
            }
            continue;
        }
        if geoms.is_empty() {
            ex.push(
                p.kind,
                handle,
                &layer,
                Geom::Unsupported,
                warn,
                Some(p.record),
                None,
            )?;
            continue;
        }
        for (k, g) in geoms.into_iter().enumerate() {
            let w = if k == 0 { warn.clone() } else { Vec::new() };
            ex.push(p.kind, handle.clone(), &layer, g, w, Some(p.record), None)?;
        }
    }
    Ok((ex.out, ex.warnings))
}

struct Expander<'b, 'a> {
    blocks: &'b HashMap<String, Block<'a>>,
    out: Vec<Entity>,
    limit: usize,
    warnings: Vec<String>,
}

impl<'b, 'a> Expander<'b, 'a> {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        kind: &str,
        handle: Option<String>,
        layer: &str,
        geom: Geom,
        warnings: Vec<String>,
        record: Option<usize>,
        parent: Option<String>,
    ) -> Result<String, Refuse> {
        if self.out.len() >= self.limit {
            return Err((
                "too_many_entities",
                format!(
                    "more than {} entities (blocks expanded included)",
                    self.limit
                ),
            ));
        }
        let id = format!("E{}", self.out.len());
        self.out.push(Entity {
            id: id.clone(),
            kind: kind.to_string(),
            handle,
            layer: layer.to_string(),
            geom,
            warnings,
            record,
            parent,
        });
        Ok(id)
    }

    /// Expands the INSERT `p` (already pushed as `owner`).
    fn expand_insert(
        &mut self,
        owner: &str,
        p: &Primary<'_>,
        name: &str,
        layer: &str,
        depth: usize,
        path: &mut Vec<String>,
    ) -> Result<(), Refuse> {
        let Some(block) = self.blocks.get(name) else {
            self.warnings.push(format!(
                "INSERT of unknown block '{name}': listed, not expanded"
            ));
            return Ok(());
        };
        let cols = getf(p.body, 70).unwrap_or(1.0).max(1.0) as usize;
        let rows = getf(p.body, 71).unwrap_or(1.0).max(1.0) as usize;
        if cols.saturating_mul(rows) > self.limit {
            return Err((
                "too_many_entities",
                format!(
                    "INSERT '{name}' array of {cols}x{rows} exceeds {} entities",
                    self.limit
                ),
            ));
        }
        let base = block.base;
        for j in 0..rows {
            for i in 0..cols {
                let mut warn = Vec::new();
                let xf = insert_transform(p.body, base, i, j, &mut warn);
                self.warnings
                    .extend(warn.into_iter().map(|w| format!("INSERT '{name}': {w}")));
                self.expand_block(owner, name, &xf, layer, depth + 1, path)?;
            }
        }
        Ok(())
    }

    /// Expands block `name` under `owner` with the transform `xf` (block coordinates to the drawing).
    fn expand_block(
        &mut self,
        owner: &str,
        name: &str,
        xf: &Affine,
        layer: &str,
        depth: usize,
        path: &mut Vec<String>,
    ) -> Result<(), Refuse> {
        if path.iter().any(|n| n == name) {
            return Err(("block_cycle", format!("block '{name}' contains itself")));
        }
        if depth > MAX_DEPTH {
            return Err((
                "block_too_deep",
                format!("blocks nested deeper than {MAX_DEPTH} levels"),
            ));
        }
        let Some(block) = self.blocks.get(name) else {
            return Ok(());
        };
        let prims = primaries(&block.recs);
        path.push(name.to_string());
        for p in &prims {
            let mut warn = p.warnings.clone();
            let geoms = geometry(p, &mut warn);
            let own_layer = get(p.body, 8).unwrap_or("0");
            let lay = if own_layer == "0" {
                layer.to_string()
            } else {
                own_layer.to_string()
            };
            let handle = get(p.body, 5)
                .map(|h| h.trim().to_string())
                .filter(|h| !h.is_empty());
            if p.kind == "INSERT" {
                let child_name = get(p.body, 2).unwrap_or("").trim().to_string();
                let id = self.push(
                    "INSERT",
                    handle,
                    &lay,
                    Geom::Unsupported,
                    warn,
                    None,
                    Some(owner.to_string()),
                )?;
                // the nested INSERT's own transform is applied before the outer one
                let nested_name = child_name.clone();
                let Some(nb) = self.blocks.get(&nested_name) else {
                    self.warnings.push(format!(
                        "INSERT of unknown block '{nested_name}': listed, not expanded"
                    ));
                    continue;
                };
                let cols = getf(p.body, 70).unwrap_or(1.0).max(1.0) as usize;
                let rows = getf(p.body, 71).unwrap_or(1.0).max(1.0) as usize;
                if cols.saturating_mul(rows) > self.limit {
                    return Err((
                        "too_many_entities",
                        format!(
                            "INSERT '{nested_name}' array exceeds {} entities",
                            self.limit
                        ),
                    ));
                }
                let base = nb.base;
                for j in 0..rows {
                    for i in 0..cols {
                        let mut w = Vec::new();
                        let inner = insert_transform(p.body, base, i, j, &mut w);
                        let total = inner.then(xf);
                        self.expand_block(&id, &nested_name, &total, &lay, depth + 1, path)?;
                    }
                }
                continue;
            }
            let geoms = if geoms.is_empty() {
                vec![Geom::Unsupported]
            } else {
                geoms
            };
            for (k, g) in geoms.iter().enumerate() {
                let placed = place(g, xf, &mut warn);
                let w = if k == 0 { warn.clone() } else { Vec::new() };
                self.push(
                    p.kind,
                    handle.clone(),
                    &lay,
                    placed,
                    w,
                    None,
                    Some(owner.to_string()),
                )?;
            }
        }
        path.pop();
        Ok(())
    }
}

/// How many entities are listed but not read (Unsupported geometry).
pub fn unread(entities: &[Entity]) -> usize {
    entities
        .iter()
        .filter(|e| matches!(e.geom, Geom::Unsupported))
        .count()
}
