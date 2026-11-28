//! Person-only edits of a DXF. `apply` takes the bytes of one version and a list of operations and returns the
//! bytes of a NEW version; the input is never changed. Only the records an operation touches are rewritten; every
//! other line is copied as it was. The output carries the marker `999 EDITED IN NF CAD - NOT A VERIFIED NF PATTERN`
//! as its first pair, so a file that has been edited stays identifiable after a download and re-upload.
//!
//! Support matrix (anything else is refused with `unsupported_op`, never guessed):
//!   move, rotate, mirror, scale (uniform, about a pivot): POINT, LINE, CIRCLE, ARC, LWPOLYLINE, POLYLINE, SPLINE
//!   move, rotate, scale only: TEXT, INSERT (mirror would need the block's own orientation)
//!   delete: any top-level entity; set layer: any top-level entity; add: LINE, CIRCLE, ARC, LWPOLYLINE.
//!
//! Entities are addressed by the id `open()` gave them in the input version. Only top-level entities can be
//! edited: an entity expanded from a block is edited through its INSERT (or DIMENSION).

use crate::geom::{Affine, P};
use crate::open;
use serde::{Deserialize, Serialize};

pub const MARK_CODE: &str = "999";
pub const MARK_TEXT: &str = "EDITED IN NF CAD - NOT A VERIFIED NF PATTERN";
const MARK_PREFIX: &str = "EDITED IN NF CAD";

/// True when the bytes carry the edit marker anywhere. Conservative on purpose: a certification gate refuses on it.
pub fn is_edited(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes).contains(MARK_PREFIX)
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditError {
    /// Stable code: `unsupported_op`, `unknown_entity`, `bad_request`, `layer_missing`, `malformed`.
    pub code: &'static str,
    pub message: String,
}

fn err(code: &'static str, message: impl Into<String>) -> EditError {
    EditError { code, message: message.into() }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Move { ids: Vec<String>, dx: f64, dy: f64 },
    Rotate { ids: Vec<String>, cx: f64, cy: f64, deg: f64 },
    Mirror { ids: Vec<String>, a: P, b: P },
    Scale { ids: Vec<String>, cx: f64, cy: f64, factor: f64 },
    Delete { ids: Vec<String> },
    SetLayer { ids: Vec<String>, layer: String },
    Add { entity: NewEntity },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NewEntity {
    Line { a: P, b: P, layer: String },
    Circle { c: P, r: f64, layer: String },
    /// Counter-clockwise from `start` to `end` (degrees).
    Arc { c: P, r: f64, start: f64, end: f64, layer: String },
    Polyline { v: Vec<P>, closed: bool, layer: String },
}

/// One raw DXF pair as written in the file (code and value text are kept verbatim).
#[derive(Debug, Clone, PartialEq)]
struct RawPair {
    code: String,
    value: String,
}

fn pair(code: &str, value: impl Into<String>) -> RawPair {
    RawPair { code: code.to_string(), value: value.into() }
}

fn fmt(v: f64) -> String {
    format!("{v}")
}

fn code_of(p: &RawPair) -> Option<u16> {
    p.code.trim().parse().ok()
}

fn num_of(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Splits the whole file into pairs. Returns the CRLF flag and the pairs; a file that does not pair up is malformed.
fn split(bytes: &[u8]) -> Result<(bool, Vec<RawPair>), EditError> {
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let crlf = text.contains("\r\n");
    let mut lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.len() % 2 != 0 {
        return Err(err("malformed", "odd number of lines: not a DXF pair file"));
    }
    let pairs = lines.chunks(2).map(|c| RawPair { code: c[0].trim().to_string(), value: c[1].to_string() }).collect();
    Ok((crlf, pairs))
}

fn join(pairs: &[RawPair], crlf: bool) -> Vec<u8> {
    let nl = if crlf { "\r\n" } else { "\n" };
    let mut s = String::new();
    for p in pairs {
        s.push_str(&p.code);
        s.push_str(nl);
        s.push_str(&p.value);
        s.push_str(nl);
    }
    // Latin-1 round trip: every char came from one byte
    s.chars().map(|c| c as u32 as u8).collect()
}

/// Index of the first pair of the ENTITIES section's content, and the index of its ENDSEC.
fn entities_range(pairs: &[RawPair]) -> Result<(usize, usize), EditError> {
    for i in 0..pairs.len().saturating_sub(1) {
        if pairs[i].code == "0" && pairs[i].value.trim() == "SECTION" && pairs[i + 1].code == "2" && pairs[i + 1].value.trim() == "ENTITIES" {
            let start = i + 2;
            let end = (start..pairs.len()).find(|&j| pairs[j].code == "0" && pairs[j].value.trim() == "ENDSEC");
            return end.map(|e| (start, e)).ok_or_else(|| err("malformed", "ENTITIES section has no ENDSEC"));
        }
    }
    Ok((pairs.len(), pairs.len()))
}

fn table_layers(pairs: &[RawPair]) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..pairs.len().saturating_sub(1) {
        if pairs[i].code == "0" && pairs[i].value.trim() == "SECTION" && pairs[i + 1].value.trim() == "TABLES" {
            let mut j = i + 2;
            while j + 1 < pairs.len() && !(pairs[j].code == "0" && pairs[j].value.trim() == "ENDSEC") {
                if pairs[j].code == "0" && pairs[j].value.trim() == "LAYER" {
                    let mut k = j + 1;
                    while k < pairs.len() && pairs[k].code != "0" {
                        if pairs[k].code == "2" {
                            out.push(pairs[k].value.trim().to_string());
                            break;
                        }
                        k += 1;
                    }
                }
                j += 1;
            }
        }
    }
    out
}

fn check_layer(name: &str, layers: &[String]) -> Result<(), EditError> {
    if name.is_empty() || name.len() > 255 || name != name.trim() || !name.is_ascii() || name.contains(['\n', '\r']) {
        return Err(err("bad_request", "layer names are 1 to 255 ASCII characters, no line breaks, no surrounding spaces"));
    }
    if !layers.iter().any(|l| l == name) {
        return Err(err("layer_missing", format!("layer '{name}' is not in the drawing's table")));
    }
    Ok(())
}

/// One operation's transform of the drawing: what happens to points, to angles and to lengths.
#[derive(Clone, Copy)]
enum Xf {
    Move(f64, f64),
    Rotate(P, f64),
    Mirror(P, P),
    Scale(P, f64),
}

impl Xf {
    fn affine(&self) -> Affine {
        match *self {
            Xf::Move(dx, dy) => Affine::translate(dx, dy),
            Xf::Rotate(c, d) => Affine::translate(-c[0], -c[1]).then(&Affine::rotate(d)).then(&Affine::translate(c[0], c[1])),
            Xf::Mirror(a, b) => Affine::mirror(a, b),
            Xf::Scale(c, f) => Affine::translate(-c[0], -c[1]).then(&Affine::scale(f, f)).then(&Affine::translate(c[0], c[1])),
        }
    }
    fn len_factor(&self) -> f64 {
        match *self {
            Xf::Scale(_, f) => f,
            _ => 1.0,
        }
    }
    /// Angle (degrees) after the transform: rotation adds, mirror reflects, others keep it.
    fn angle(&self, deg: f64) -> f64 {
        let d = match *self {
            Xf::Rotate(_, r) => deg + r,
            Xf::Mirror(a, b) => {
                let phi = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
                2.0 * phi - deg
            }
            _ => deg,
        };
        d.rem_euclid(360.0)
    }
    fn mirrors(&self) -> bool {
        matches!(self, Xf::Mirror(..))
    }
}

fn unsupported(kind: &str, what: &str) -> EditError {
    err("unsupported_op", format!("{what} is not supported for {kind}"))
}

/// Transforms one record's pairs in place. `kind` is the record's type; `rest` is the record's other records
/// (POLYLINE vertices), handled by the caller.
fn transform_pairs(kind: &str, pairs: &mut [RawPair], xf: Xf) -> Result<(), EditError> {
    let is_text_or_insert = matches!(kind, "TEXT" | "INSERT");
    if is_text_or_insert && xf.mirrors() {
        return Err(unsupported(kind, "mirror"));
    }
    let scalars: &[u16] = match kind {
        "CIRCLE" | "ARC" => &[40],
        "TEXT" => &[40],
        "INSERT" => &[41, 42],
        _ => &[],
    };
    let angles: &[u16] = match kind {
        "ARC" => &[50, 51],
        "TEXT" | "INSERT" => &[50],
        _ => &[],
    };
    let bulge_kinds = matches!(kind, "LWPOLYLINE" | "VERTEX");
    let mut i = 0;
    while i < pairs.len() {
        let c = code_of(&pairs[i]);
        // coordinate pairs: 10/20, 11/21
        if matches!(c, Some(10) | Some(11)) && i + 1 < pairs.len() && code_of(&pairs[i + 1]) == c.map(|x| x + 10) {
            let (x, y) = (num_of(&pairs[i].value), num_of(&pairs[i + 1].value));
            match (x, y) {
                (Some(x), Some(y)) => {
                    let p = xf.affine().apply([x, y]);
                    pairs[i].value = fmt(p[0]);
                    pairs[i + 1].value = fmt(p[1]);
                }
                _ => return Err(err("malformed", format!("{kind} has a non-finite coordinate"))),
            }
            i += 2;
            continue;
        }
        if let Some(code) = c {
            if scalars.contains(&code) {
                if kind == "ARC" || kind == "CIRCLE" || kind == "TEXT" {
                    if code == 40 {
                        let v = num_of(&pairs[i].value).ok_or_else(|| err("malformed", "non-finite size"))?;
                        pairs[i].value = fmt(v * xf.len_factor());
                    }
                } else if kind == "INSERT" {
                    let v = num_of(&pairs[i].value).ok_or_else(|| err("malformed", "non-finite scale"))?;
                    pairs[i].value = fmt(v * xf.len_factor());
                }
            } else if angles.contains(&code) {
                let v = num_of(&pairs[i].value).ok_or_else(|| err("malformed", "non-finite angle"))?;
                pairs[i].value = fmt(xf.angle(v));
            } else if code == 42 && bulge_kinds && xf.mirrors() {
                let v = num_of(&pairs[i].value).unwrap_or(0.0);
                pairs[i].value = fmt(-v);
            }
        }
        i += 1;
    }
    // ARC: a reflected arc runs from the mirror of its old end to the mirror of its old start
    if kind == "ARC" && xf.mirrors() {
        let s = pairs.iter().position(|p| code_of(p) == Some(50));
        let e = pairs.iter().position(|p| code_of(p) == Some(51));
        if let (Some(s), Some(e)) = (s, e) {
            let (sv, ev) = (pairs[s].value.clone(), pairs[e].value.clone());
            pairs[s].value = ev;
            pairs[e].value = sv;
        }
    }
    Ok(())
}

/// Applies `ops` to `bytes` and returns the bytes of the new version. The input is not changed.
pub fn apply(bytes: &[u8], ops: &[Op]) -> Result<Vec<u8>, EditError> {
    let drawing = open(bytes).map_err(|r| err("malformed", r.message))?;
    let (crlf, pairs) = split(bytes)?;
    let layers = table_layers(&pairs);
    let (es, ee) = entities_range(&pairs)?;
    if es == pairs.len() {
        return Err(err("malformed", "no ENTITIES section"));
    }

    // Pre-ENTITIES (header and tables), the records of ENTITIES, and the rest of the file.
    let head: Vec<RawPair> = pairs[..es].to_vec();
    let tail: Vec<RawPair> = pairs[ee..].to_vec();
    let mut recs: Vec<Vec<RawPair>> = Vec::new();
    let mut i = es;
    while i < ee {
        let start = i;
        i += 1;
        while i < ee && pairs[i].code != "0" {
            i += 1;
        }
        recs.push(pairs[start..i].to_vec());
    }
    let mut deleted = vec![false; recs.len()];
    let kind_of = |r: &[RawPair]| r.first().map(|p| p.value.trim().to_string()).unwrap_or_default();

    // Maximum handle in the file: new entities take the next ones.
    let mut next_handle = pairs
        .iter()
        .filter(|p| p.code == "5")
        .filter_map(|p| u64::from_str_radix(p.value.trim(), 16).ok())
        .max()
        .unwrap_or(0);

    let mut new_recs: Vec<Vec<RawPair>> = Vec::new();

    let resolve = |id: &str| -> Result<usize, EditError> {
        let e = drawing.entity(id).ok_or_else(|| err("unknown_entity", format!("no entity '{id}' in this version")))?;
        if e.parent.is_some() {
            return Err(err("unknown_entity", format!("'{id}' is inside a block: edit its INSERT or DIMENSION instead")));
        }
        e.record.ok_or_else(|| err("unknown_entity", format!("'{id}' has no record")))
    };
    // The records of one entity: a POLYLINE is its header, its VERTEX records and its SEQEND.
    let span = |r: usize, recs: &[Vec<RawPair>]| -> Result<Vec<usize>, EditError> {
        let k = kind_of(&recs[r]);
        if k != "POLYLINE" {
            return Ok(vec![r]);
        }
        let mut out = vec![r];
        let mut j = r + 1;
        while j < recs.len() {
            let kj = kind_of(&recs[j]);
            out.push(j);
            if kj == "SEQEND" {
                return Ok(out);
            }
            if kj != "VERTEX" {
                break;
            }
            j += 1;
        }
        Err(err("unsupported_op", "a POLYLINE without SEQEND is not editable"))
    };

    let mut touched_ids: Vec<String> = Vec::new();
    for op in ops {
        match op {
            Op::Move { ids, dx, dy } => {
                for id in ids {
                    transform_entity(&drawing, &resolve, &span, &mut recs, &deleted, id, Xf::Move(*dx, *dy), &mut touched_ids)?;
                }
            }
            Op::Rotate { ids, cx, cy, deg } => {
                for id in ids {
                    transform_entity(&drawing, &resolve, &span, &mut recs, &deleted, id, Xf::Rotate([*cx, *cy], *deg), &mut touched_ids)?;
                }
            }
            Op::Mirror { ids, a, b } => {
                if a == b {
                    return Err(err("bad_request", "mirror axis needs two different points"));
                }
                for id in ids {
                    transform_entity(&drawing, &resolve, &span, &mut recs, &deleted, id, Xf::Mirror(*a, *b), &mut touched_ids)?;
                }
            }
            Op::Scale { ids, cx, cy, factor } => {
                if !(factor.is_finite() && *factor > 0.0) {
                    return Err(err("bad_request", "scale factor must be a positive number"));
                }
                for id in ids {
                    transform_entity(&drawing, &resolve, &span, &mut recs, &deleted, id, Xf::Scale([*cx, *cy], *factor), &mut touched_ids)?;
                }
            }
            Op::Delete { ids } => {
                for id in ids {
                    let r = resolve(id)?;
                    if deleted[r] {
                        return Err(err("unknown_entity", format!("'{id}' is already deleted in this edit")));
                    }
                    for j in span(r, &recs)? {
                        deleted[j] = true;
                    }
                }
            }
            Op::SetLayer { ids, layer } => {
                check_layer(layer, &layers)?;
                for id in ids {
                    let r = resolve(id)?;
                    if deleted[r] {
                        return Err(err("unknown_entity", format!("'{id}' is deleted in this edit")));
                    }
                    let rec = &mut recs[r];
                    match rec.iter().position(|p| p.code == "8") {
                        Some(k) => rec[k].value = layer.clone(),
                        None => rec.insert(1, pair("8", layer.clone())),
                    }
                }
            }
            Op::Add { entity } => {
                let (kind, layer, body) = new_body(entity, &layers)?;
                next_handle += 1;
                let mut r = vec![pair("0", kind), pair("8", layer), pair("5", format!("{next_handle:X}"))];
                r.extend(body);
                new_recs.push(r);
            }
        }
    }

    // Header: keep $HANDSEED ahead of every handle written.
    let mut head = head;
    if let Some(k) = head.iter().position(|p| p.code == "9" && p.value.trim() == "$HANDSEED") {
        if k + 1 < head.len() && head[k + 1].code == "5" {
            head[k + 1].value = format!("{:X}", next_handle + 1);
        }
    }

    let mut body: Vec<RawPair> = Vec::new();
    for (r, rec) in recs.iter().enumerate() {
        if !deleted[r] {
            body.extend(rec.iter().cloned());
        }
    }
    for r in &new_recs {
        body.extend(r.iter().cloned());
    }
    let mut out = Vec::with_capacity(head.len() + body.len() + tail.len() + 1);
    out.push(pair(MARK_CODE, MARK_TEXT));
    out.extend(head);
    out.extend(body);
    out.extend(tail);
    let bytes_out = join(&out, crlf);

    // The result must read back as a drawing; a version that does not is never returned.
    open(&bytes_out).map_err(|r| err("malformed", format!("the edited file does not read back: {}", r.message)))?;
    Ok(bytes_out)
}

#[allow(clippy::too_many_arguments)]
fn transform_entity(
    drawing: &crate::Drawing,
    resolve: &dyn Fn(&str) -> Result<usize, EditError>,
    span: &dyn Fn(usize, &[Vec<RawPair>]) -> Result<Vec<usize>, EditError>,
    recs: &mut [Vec<RawPair>],
    deleted: &[bool],
    id: &str,
    xf: Xf,
    touched: &mut Vec<String>,
) -> Result<(), EditError> {
    let r = resolve(id)?;
    if deleted[r] {
        return Err(err("unknown_entity", format!("'{id}' is deleted in this edit")));
    }
    let kind = drawing.entity(id).map(|e| e.kind.clone()).unwrap_or_default();
    let supported = match kind.as_str() {
        "POINT" | "LINE" | "CIRCLE" | "ARC" | "LWPOLYLINE" | "POLYLINE" | "SPLINE" => true,
        "TEXT" | "INSERT" => !xf.mirrors(),
        _ => false,
    };
    if !supported {
        return Err(unsupported(&kind, if xf.mirrors() { "mirror" } else { "this operation" }));
    }
    for j in span(r, recs)? {
        let k = recs[j].first().map(|p| p.value.trim().to_string()).unwrap_or_default();
        // a POLYLINE header holds an elevation point, not geometry: only its VERTEX records move
        match k.as_str() {
            "VERTEX" => transform_pairs("VERTEX", &mut recs[j][1..], xf)?,
            "POLYLINE" | "SEQEND" => {}
            _ => transform_pairs(&kind, &mut recs[j][1..], xf)?,
        }
    }
    touched.push(id.to_string());
    Ok(())
}

fn new_body(e: &NewEntity, layers: &[String]) -> Result<(&'static str, String, Vec<RawPair>), EditError> {
    let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
    let z = |c: P| vec![pair("10", fmt(c[0])), pair("20", fmt(c[1])), pair("30", "0")];
    match e {
        NewEntity::Line { a, b, layer } => {
            check_layer(layer, layers)?;
            if !finite(&[a[0], a[1], b[0], b[1]]) {
                return Err(err("bad_request", "line coordinates must be finite"));
            }
            let mut body = z(*a);
            body.push(pair("11", fmt(b[0])));
            body.push(pair("21", fmt(b[1])));
            body.push(pair("31", "0"));
            Ok(("LINE", layer.clone(), body))
        }
        NewEntity::Circle { c, r, layer } => {
            check_layer(layer, layers)?;
            if !(finite(&[c[0], c[1], *r]) && *r > 0.0) {
                return Err(err("bad_request", "circle needs a finite centre and a positive radius"));
            }
            let mut body = z(*c);
            body.push(pair("40", fmt(*r)));
            Ok(("CIRCLE", layer.clone(), body))
        }
        NewEntity::Arc { c, r, start, end, layer } => {
            check_layer(layer, layers)?;
            if !(finite(&[c[0], c[1], *r, *start, *end]) && *r > 0.0) {
                return Err(err("bad_request", "arc needs a finite centre, a positive radius and finite angles"));
            }
            let mut body = z(*c);
            body.push(pair("40", fmt(*r)));
            body.push(pair("50", fmt(start.rem_euclid(360.0))));
            body.push(pair("51", fmt(end.rem_euclid(360.0))));
            Ok(("ARC", layer.clone(), body))
        }
        NewEntity::Polyline { v, closed, layer } => {
            check_layer(layer, layers)?;
            if v.len() < 2 || v.len() > 10_000 || v.iter().any(|p| !finite(&[p[0], p[1]])) {
                return Err(err("bad_request", "polyline needs 2 to 10000 finite vertices"));
            }
            let mut body = vec![pair("90", v.len().to_string()), pair("70", if *closed { "1" } else { "0" })];
            for p in v {
                body.push(pair("10", fmt(p[0])));
                body.push(pair("20", fmt(p[1])));
            }
            Ok(("LWPOLYLINE", layer.clone(), body))
        }
    }
}

/// The entity ids an operation list names (for validation before anything is applied).
pub fn touched_ids(ops: &[Op]) -> Vec<String> {
    let mut out = Vec::new();
    for op in ops {
        match op {
            Op::Move { ids, .. } | Op::Rotate { ids, .. } | Op::Mirror { ids, .. } | Op::Scale { ids, .. } | Op::Delete { ids } | Op::SetLayer { ids, .. } => {
                out.extend(ids.iter().cloned())
            }
            Op::Add { .. } => {}
        }
    }
    out
}
