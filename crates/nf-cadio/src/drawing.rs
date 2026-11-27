//! The Drawing model: what the reader keeps from TABLES and ENTITIES, and the record helpers the
//! table and entity readers share.
//!
//! Units come only from `$INSUNITS`. A missing or zero value is `Units::Unknown`, and nothing else
//! sets the unit. Angles are in degrees, as in the file. Coordinates are in drawing units.

use std::collections::BTreeMap;

use crate::error::{CadError, Reason};
use crate::header::{int_value, real, HeaderValue};
use crate::text::{self, TextEncoding};
use crate::tokenizer::Pair;
use crate::units::Units;
use crate::{entities, tables, Document};

/// A drawing read from a DXF file.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawing {
    /// `$ACADVER`, for example `AC1009`, when the header has it.
    pub version: Option<String>,
    /// Units from `$INSUNITS`. `Units::Unknown` when the variable is missing or 0.
    pub units: Units,
    /// The LAYER table, in file order.
    pub layers: Vec<Layer>,
    /// The LTYPE table, in file order.
    pub line_types: Vec<LineType>,
    /// The STYLE table, in file order.
    pub text_styles: Vec<TextStyle>,
    /// The names of the BLOCK_RECORD table, in file order. Block contents are not read.
    pub blocks: Vec<String>,
    /// Entities of the ENTITIES section, in file order.
    pub entities: Vec<Entity>,
    /// Entity kinds this reader does not model, with how many of each were skipped.
    pub skipped: BTreeMap<String, usize>,
    /// Notes on what was read but not applied, for example an entity whose extrusion is not +Z.
    pub warnings: Vec<String>,
}

/// One layer of the LAYER table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    /// AutoCAD colour index (group 62, sign removed). 7 when the file gives none.
    pub color: i64,
    /// Linetype name (group 6), when the file gives one.
    pub line_type: Option<String>,
    /// False when group 62 is negative.
    pub on: bool,
    /// True when bit 1 of group 70 is set.
    pub frozen: bool,
}

/// One linetype of the LTYPE table.
#[derive(Debug, Clone, PartialEq)]
pub struct LineType {
    pub name: String,
    pub description: String,
    /// The dash, space and dot lengths (group 49), in file order.
    pub elements: Vec<f64>,
}

/// One text style of the STYLE table.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub name: String,
    pub font: String,
    /// Fixed text height (group 40). 0 means the height is set per text entity.
    pub height: f64,
    /// Width factor (group 41). 1 when the file gives none.
    pub width_factor: f64,
}

/// A point in drawing units. `z` is 0 when the file gives no group 30.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A polyline vertex. `bulge` is the arc bulge from this vertex to the next one (group 42): positive
/// is counter-clockwise, negative clockwise, 0 a straight segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub bulge: f64,
}

/// An entity of the ENTITIES section that this reader models.
#[derive(Debug, Clone, PartialEq)]
pub enum Entity {
    Line {
        layer: String,
        start: Point3,
        end: Point3,
        /// True when the extrusion direction (groups 210 to 230) is not +Z. See [`Entity::non_z_extrusion`].
        non_z_extrusion: bool,
    },
    Point {
        layer: String,
        at: Point3,
        non_z_extrusion: bool,
    },
    Circle {
        layer: String,
        center: Point3,
        radius: f64,
        non_z_extrusion: bool,
    },
    /// Counter-clockwise from `start_angle` to `end_angle`, in degrees.
    Arc {
        layer: String,
        center: Point3,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        non_z_extrusion: bool,
    },
    /// A LWPOLYLINE. `z` of every vertex is the elevation (group 38).
    LwPolyline {
        layer: String,
        closed: bool,
        vertices: Vec<Vertex>,
        non_z_extrusion: bool,
    },
    /// A POLYLINE with its VERTEX records. Polygon and polyface meshes are not modelled and are skipped.
    Polyline {
        layer: String,
        closed: bool,
        is_3d: bool,
        vertices: Vec<Vertex>,
        non_z_extrusion: bool,
    },
    Text {
        layer: String,
        at: Point3,
        height: f64,
        rotation: f64,
        /// The string as the file gives it, decoded, with escapes kept.
        raw: String,
        /// The string with `\U+` and `\M+` decoded.
        plain: String,
        non_z_extrusion: bool,
    },
    MText {
        layer: String,
        at: Point3,
        height: f64,
        /// The string as the file gives it (groups 3 and 1 joined), decoded, with escapes kept.
        raw: String,
        /// The text with escapes decoded and formatting codes removed; `\P` is a new line.
        plain: String,
        non_z_extrusion: bool,
    },
    /// The four corners in the order of the file (groups 10 to 13). The third and fourth corners
    /// are swapped in the drawing, as in the DXF format. A missing fourth corner is the third.
    Solid {
        layer: String,
        corners: [Point3; 4],
        non_z_extrusion: bool,
    },
}

impl Entity {
    /// True when the entity's extrusion direction (groups 210, 220, 230) is not +Z. Its coordinates
    /// are kept exactly as the file gives them; they are not mirrored or rotated into the drawing
    /// plane. Each such entity also has a note in [`Drawing::warnings`].
    pub fn non_z_extrusion(&self) -> bool {
        match self {
            Entity::Line {
                non_z_extrusion, ..
            }
            | Entity::Point {
                non_z_extrusion, ..
            }
            | Entity::Circle {
                non_z_extrusion, ..
            }
            | Entity::Arc {
                non_z_extrusion, ..
            }
            | Entity::LwPolyline {
                non_z_extrusion, ..
            }
            | Entity::Polyline {
                non_z_extrusion, ..
            }
            | Entity::Text {
                non_z_extrusion, ..
            }
            | Entity::MText {
                non_z_extrusion, ..
            }
            | Entity::Solid {
                non_z_extrusion, ..
            } => *non_z_extrusion,
        }
    }
}

/// Longest entity kind name kept as a key of `Drawing::skipped`.
const MAX_KIND_CHARS: usize = 40;

pub(crate) fn build(doc: &Document) -> Result<Drawing, CadError> {
    let version = match doc.variable("$ACADVER") {
        Some(HeaderValue::Text(v)) => Some(v.clone()),
        _ => None,
    };
    let codepage = match doc.variable("$DWGCODEPAGE") {
        Some(HeaderValue::Text(c)) => Some(c.as_str()),
        _ => None,
    };
    let enc = text::encoding_for(version.as_deref(), codepage);
    let mut drawing = Drawing {
        version,
        units: doc.units(),
        layers: Vec::new(),
        line_types: Vec::new(),
        text_styles: Vec::new(),
        blocks: Vec::new(),
        entities: Vec::new(),
        skipped: BTreeMap::new(),
        warnings: Vec::new(),
    };
    for section in doc.sections.iter().filter(|s| s.name == "TABLES") {
        tables::read(&section.pairs, &enc, &mut drawing)?;
    }
    for section in doc.sections.iter().filter(|s| s.name == "ENTITIES") {
        entities::read(&section.pairs, &enc, &mut drawing)?;
    }
    Ok(drawing)
}

/// Notes that an entity with an extrusion other than +Z was read as written.
pub(crate) fn warn_extrusion(drawing: &mut Drawing, kind: &str, line: usize) {
    drawing.warnings.push(format!(
        "{kind} at line {line}: extrusion is not +Z; coordinates kept as written, not transformed"
    ));
}

/// Counts one entity kind this reader does not model.
pub(crate) fn skip(drawing: &mut Drawing, kind: &str) {
    let key: String = kind.chars().take(MAX_KIND_CHARS).collect();
    *drawing.skipped.entry(key).or_insert(0) += 1;
}

/// One record: a group 0 pair and the pairs up to the next group 0.
pub(crate) struct Record<'a> {
    /// The entity or table-entry name in group 0, for example `LINE`.
    pub kind: &'a str,
    /// Line of the group 0 pair.
    pub line: usize,
    /// The pairs after group 0.
    pub body: &'a [Pair],
}

/// Splits a section body into records. Group 999 comments before the first record are allowed; any
/// other value before it is refused.
pub(crate) fn records(pairs: &[Pair]) -> Result<Vec<Record<'_>>, CadError> {
    if let Some(stray) = pairs
        .iter()
        .take_while(|p| p.code != 0)
        .find(|p| p.code != 999)
    {
        return Err(CadError::at(stray.line, Reason::ExpectedRecord));
    }
    let heads: Vec<usize> = pairs
        .iter()
        .enumerate()
        .filter(|(_, p)| p.code == 0)
        .map(|(i, _)| i)
        .collect();
    Ok(heads
        .iter()
        .enumerate()
        .map(|(k, &head)| {
            let end = heads.get(k + 1).copied().unwrap_or(pairs.len());
            let pair = &pairs[head];
            Record {
                kind: pair.value.as_str(),
                line: pair.line,
                body: &pairs[head + 1..end],
            }
        })
        .collect())
}

impl<'a> Record<'a> {
    /// The first pair with this group code.
    pub fn pair(&self, code: u16) -> Option<&'a Pair> {
        let body: &'a [Pair] = self.body;
        body.iter().find(|p| p.code == code)
    }

    /// The string of this group, decoded, or None when absent.
    pub fn text(&self, code: u16, enc: &TextEncoding) -> Result<Option<String>, CadError> {
        match self.pair(code) {
            None => Ok(None),
            Some(p) => text::decode(&p.value, enc)
                .map(Some)
                .map_err(|reason| CadError::at(p.line, reason)),
        }
    }

    /// The string of this group, or a refusal when absent.
    pub fn required_text(&self, code: u16, enc: &TextEncoding) -> Result<String, CadError> {
        self.text(code, enc)?.ok_or_else(|| self.missing(code))
    }

    /// The integer of this group, or None when absent.
    pub fn int(&self, code: u16) -> Result<Option<i64>, CadError> {
        self.pair(code).map(int_value).transpose()
    }

    /// The real number of this group, or None when absent.
    pub fn real(&self, code: u16) -> Result<Option<f64>, CadError> {
        self.pair(code).map(real).transpose()
    }

    /// The real number of this group, or a refusal when absent.
    pub fn required_real(&self, code: u16) -> Result<f64, CadError> {
        self.real(code)?.ok_or_else(|| self.missing(code))
    }

    /// The point whose coordinates are groups `code`, `code + 10` and `code + 20`. The z value is 0
    /// when group `code + 20` is absent. None when no coordinate is given; a refusal when only one of x
    /// and y is given.
    pub fn point(&self, code: u16) -> Result<Option<Point3>, CadError> {
        let x = self.real(code)?;
        let y = self.real(code + 10)?;
        let z = self.real(code + 20)?;
        match (x, y) {
            (Some(x), Some(y)) => Ok(Some(Point3 {
                x,
                y,
                z: z.unwrap_or(0.0),
            })),
            (None, None) => Ok(None),
            (Some(_), None) => Err(self.missing(code + 10)),
            (None, Some(_)) => Err(self.missing(code)),
        }
    }

    /// The point of [`Record::point`], or a refusal when absent.
    pub fn required_point(&self, code: u16) -> Result<Point3, CadError> {
        self.point(code)?.ok_or_else(|| self.missing(code))
    }

    /// A refusal that this record has no group `code`.
    pub fn missing(&self, code: u16) -> CadError {
        CadError::at(
            self.line,
            Reason::MissingField {
                entity: self.kind.chars().take(MAX_KIND_CHARS).collect(),
                code,
            },
        )
    }

    /// A refusal that the value of group `code` is not valid for that code.
    pub fn invalid(&self, code: u16) -> CadError {
        match self.pair(code) {
            Some(p) => CadError::at(
                p.line,
                Reason::InvalidValue {
                    code,
                    text: p.value.chars().take(32).collect(),
                },
            ),
            None => self.missing(code),
        }
    }
}
