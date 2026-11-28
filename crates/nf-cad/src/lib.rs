//! NF CAD phase 1: a read-only view of a DXF. Opens a file through `nf-cadio`, lists layers and entities,
//! measures (distance, angle, arc radius, polyline length and area), picks entities at a canvas point, and
//! re-runs NF's DXF quality gate on the text. Nothing here edits geometry.
//!
//! Independence: this crate is not a dependency of `nf-digest`, and it never edits `uem`, `nf-dxf` or
//! `nf-core`. `nf-dxf` is used only to call its quality gate.

pub mod edit;
pub mod geom;
pub mod read;
pub mod tools;

use geom::{Geom, P};
use nf_cadio::{Document, Limits};
use read::{Entity, Layer};
use std::collections::HashMap;

/// Caps for the viewer. Above these the file is refused with a reason, never truncated.
pub const MAX_BYTES: usize = 8 << 20;
pub const MAX_ENTITIES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Stable code: `too_large`, `too_many_entities` or `malformed`.
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drawing {
    /// `$ACADVER` as written, or "unknown".
    pub version: String,
    /// `$INSUNITS` name; "unknown" when missing or 0 (never guessed).
    pub units: &'static str,
    pub layers: Vec<Layer>,
    pub entities: Vec<Entity>,
    pub extents: Option<[f64; 4]>,
    pub warnings: Vec<String>,
}

/// Opens DXF bytes. Malformed, oversized or too-large files are refused.
pub fn open(bytes: &[u8]) -> Result<Drawing, Refusal> {
    if bytes.len() > MAX_BYTES {
        return Err(Refusal {
            code: "too_large",
            message: format!("file is {} bytes; the limit is {MAX_BYTES}", bytes.len()),
        });
    }
    let limits = Limits {
        max_bytes: MAX_BYTES,
        max_entities: MAX_ENTITIES,
        ..Limits::default()
    };
    let doc = Document::parse_with(bytes, &limits).map_err(|e| Refusal {
        code: match e.reason {
            nf_cadio::Reason::TooManyEntities { .. } => "too_many_entities",
            nf_cadio::Reason::TooLarge { .. } => "too_large",
            _ => "malformed",
        },
        message: format!("line {}: {}", e.line, e.reason),
    })?;
    let version = match doc.variable("$ACADVER") {
        Some(nf_cadio::HeaderValue::Text(t)) => t.trim().to_string(),
        _ => "unknown".to_string(),
    };
    let mut warnings = Vec::new();
    let empty = Vec::new();
    let tables = doc.section("TABLES").map(|s| &s.pairs).unwrap_or(&empty);
    let mut layers = read::layers(tables);
    let mut index: HashMap<String, usize> = layers
        .iter()
        .enumerate()
        .map(|(i, l)| (l.name.clone(), i))
        .collect();
    let entities_pairs = match doc.section("ENTITIES") {
        Some(s) => s.pairs.as_slice(),
        None => {
            warnings.push("no ENTITIES section".into());
            &[]
        }
    };
    let block_pairs = doc
        .section("BLOCKS")
        .map(|s| s.pairs.as_slice())
        .unwrap_or(&[]);
    let blocks = read::blocks(block_pairs);
    let (entities, expand_warnings) = read::entities(entities_pairs, &blocks, MAX_ENTITIES)
        .map_err(|(code, message)| Refusal { code, message })?;
    warnings.extend(expand_warnings);
    for e in &entities {
        if !index.contains_key(&e.layer) {
            index.insert(e.layer.clone(), layers.len());
            layers.push(Layer {
                name: e.layer.clone(),
                colour: 7,
                on: true,
                frozen: false,
                count: 0,
            });
        }
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for e in &entities {
        *counts.entry(e.layer.as_str()).or_insert(0) += 1;
    }
    for l in layers.iter_mut() {
        l.count = counts.get(l.name.as_str()).copied().unwrap_or(0);
    }
    let extents = entities
        .iter()
        .filter_map(|e| geom::extents(&e.geom))
        .reduce(|a, b| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]
        });
    let unsupported = read::unread(&entities);
    if unsupported > 0 {
        warnings.push(format!(
            "{unsupported} entities of a type phase 1 does not measure (listed, not read)"
        ));
    }
    Ok(Drawing {
        version,
        units: doc.units().as_str(),
        layers,
        entities,
        extents,
        warnings,
    })
}

impl Drawing {
    pub fn entity(&self, id: &str) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Ids of the entities on `layer`, in file order.
    pub fn on_layer(&self, layer: &str) -> Vec<&str> {
        self.entities
            .iter()
            .filter(|e| e.layer == layer)
            .map(|e| e.id.as_str())
            .collect()
    }

    /// The entity nearest to `p` within `tol` (drawing units), ties broken by file order.
    pub fn pick(&self, p: P, tol: f64) -> Option<&Entity> {
        let mut best: Option<(f64, &Entity)> = None;
        for e in &self.entities {
            let d = geom::pick_distance(&e.geom, p);
            if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, e));
            }
        }
        best.map(|(_, e)| e)
    }
}

/// The measurements phase 1 offers. Each names the entities it came from.
#[derive(Debug, Clone, PartialEq)]
pub enum Measure {
    /// Distance between two points.
    Distance { from: P, to: P },
    /// dx, dy and the direction of a -> b.
    Vector { from: P, to: P },
    /// Angle between two lines (unsigned, 0..=90).
    LineAngle { a: [P; 2], b: [P; 2] },
    /// Angle at a vertex between two rays (0..=180).
    VertexAngle { at: P, a: P, b: P },
    /// Arc radius and its included sweep in degrees.
    ArcRadius { entity: String },
    /// Length of a line, arc, circle, or polyline (bulge segments included).
    Length { entity: String },
    /// Area of a circle or closed polyline (bulge segments included).
    Area { entity: String },
}

/// The value of a measurement, with its unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub value: f64,
    pub unit: &'static str,
    /// Entity ids the value came from.
    pub from: Vec<String>,
}

impl Drawing {
    /// Measures. Every value is in drawing units (`units`, or "unknown"); angles are degrees.
    pub fn measure(&self, m: &Measure) -> Result<Vec<Value>, String> {
        let u = "drawing units";
        let ent = |id: &str| self.entity(id).ok_or_else(|| format!("no entity '{id}'"));
        let one = |v: f64, unit: &'static str, from: Vec<String>| {
            Ok(vec![Value {
                value: v,
                unit,
                from,
            }])
        };
        match m {
            Measure::Distance { from, to } => one(geom::dist(*from, *to), u, vec![]),
            Measure::Vector { from, to } => Ok(vec![
                Value {
                    value: to[0] - from[0],
                    unit: u,
                    from: vec![],
                },
                Value {
                    value: to[1] - from[1],
                    unit: u,
                    from: vec![],
                },
                Value {
                    value: geom::angle_deg(*from, *to),
                    unit: "deg",
                    from: vec![],
                },
            ]),
            Measure::LineAngle { a, b } => {
                one(geom::line_angle_deg(a[0], a[1], b[0], b[1]), "deg", vec![])
            }
            Measure::VertexAngle { at, a, b } => {
                one(geom::vertex_angle_deg(*at, *a, *b), "deg", vec![])
            }
            Measure::ArcRadius { entity } => match &ent(entity)?.geom {
                Geom::Arc { r, start, end, .. } => Ok(vec![
                    Value {
                        value: *r,
                        unit: u,
                        from: vec![entity.clone()],
                    },
                    Value {
                        value: geom::sweep_deg(*start, *end),
                        unit: "deg",
                        from: vec![entity.clone()],
                    },
                ]),
                Geom::Circle { r, .. } => one(*r, u, vec![entity.clone()]),
                _ => Err(format!("'{entity}' is not an arc or circle")),
            },
            Measure::Length { entity } => {
                let v = match &ent(entity)?.geom {
                    Geom::Line { a, b } => geom::dist(*a, *b),
                    Geom::Arc { r, start, end, .. } => geom::arc_length(*r, *start, *end),
                    Geom::Circle { r, .. } => geom::circle_length(*r),
                    Geom::Poly { v, closed } => geom::poly_length(v, *closed),
                    _ => return Err(format!("'{entity}' has no length")),
                };
                one(v, u, vec![entity.clone()])
            }
            Measure::Area { entity } => {
                let v = match &ent(entity)?.geom {
                    Geom::Circle { r, .. } => geom::circle_area(*r),
                    Geom::Poly { v, closed: true } => geom::poly_area(v),
                    Geom::Poly { .. } => return Err(format!("'{entity}' is open: no area")),
                    _ => return Err(format!("'{entity}' has no area")),
                };
                one(v, "square drawing units", vec![entity.clone()])
            }
        }
    }
}

/// One rule of NF's quality gate, as it applies to the file.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RuleCheck {
    pub rule: &'static str,
    pub pass: bool,
    pub findings: Vec<String>,
}

/// Re-runs NF's DXF quality gate (`nf_dxf::quality::quality_gate`, all 14 rules) on the file text. Run without
/// the job's declared joint or conventions, so a file that is not an NF output fails the rules that need them:
/// the failures are reported as they are, never filtered out.
pub fn quality_checks(bytes: &[u8]) -> Vec<RuleCheck> {
    let text = String::from_utf8_lossy(bytes);
    let report = nf_dxf::quality::quality_gate(&text);
    nf_dxf::quality::RULES
        .iter()
        .map(|rule| {
            let findings: Vec<String> = report
                .findings
                .iter()
                .filter(|f| f.rule == *rule)
                .map(|f| f.detail.clone())
                .collect();
            RuleCheck {
                rule,
                pass: findings.is_empty(),
                findings,
            }
        })
        .collect()
}

/// JSON for one entity: its id, type, layer, handle, geometry and warnings. Units are the drawing's.
pub fn entity_json(e: &Entity) -> serde_json::Value {
    use serde_json::json;
    let geom = match &e.geom {
        Geom::Point { at } => json!({ "type": "point", "at": at }),
        Geom::Line { a, b } => json!({ "type": "line", "a": a, "b": b }),
        Geom::Circle { c, r } => json!({ "type": "circle", "c": c, "r": r }),
        Geom::Arc { c, r, start, end } => {
            json!({ "type": "arc", "c": c, "r": r, "start": start, "end": end })
        }
        Geom::Poly { v, closed } => json!({
            "type": "poly",
            "closed": closed,
            "v": v.iter().map(|x| json!([x.p[0], x.p[1], x.bulge])).collect::<Vec<_>>(),
        }),
        Geom::Text { at, height, text } => {
            json!({ "type": "text", "at": at, "height": height, "text": text })
        }
        Geom::Unsupported => json!({ "type": "unsupported" }),
    };
    json!({
        "id": e.id,
        "kind": e.kind,
        "handle": e.handle,
        "layer": e.layer,
        "geom": geom,
        "extents": geom::extents(&e.geom),
        "parent": e.parent,
        "record": e.record,
        "warnings": e.warnings,
    })
}

/// Parses "x,y" into a point; None when malformed or not finite.
pub fn parse_point(s: &str) -> Option<P> {
    let (x, y) = s.split_once(',')?;
    let (x, y) = (read::num(x)?, read::num(y)?);
    Some([x, y])
}

/// Builds a measurement from named query arguments: `kind` plus its points and entity ids.
pub fn measure_from_query(q: &dyn Fn(&str) -> Option<String>) -> Result<Measure, String> {
    let pt = |k: &str| -> Result<P, String> {
        q(k).as_deref()
            .and_then(parse_point)
            .ok_or_else(|| format!("'{k}' must be x,y"))
    };
    let ent = || q("entity").ok_or_else(|| "'entity' is missing".to_string());
    let kind = q("kind").ok_or("'kind' is missing")?;
    Ok(match kind.as_str() {
        "distance" => Measure::Distance {
            from: pt("from")?,
            to: pt("to")?,
        },
        "vector" => Measure::Vector {
            from: pt("from")?,
            to: pt("to")?,
        },
        "line_angle" => Measure::LineAngle {
            a: [pt("a1")?, pt("a2")?],
            b: [pt("b1")?, pt("b2")?],
        },
        "vertex_angle" => Measure::VertexAngle {
            at: pt("at")?,
            a: pt("a")?,
            b: pt("b")?,
        },
        "arc_radius" => Measure::ArcRadius { entity: ent()? },
        "length" => Measure::Length { entity: ent()? },
        "area" => Measure::Area { entity: ent()? },
        other => return Err(format!("unknown measurement '{other}'")),
    })
}
