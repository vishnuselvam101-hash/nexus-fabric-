//! Reader for ASCII DXF group-code/value pairs. Written for the digest only: it shares no code with
//! nf-dxf (the writer and its reader), so a parsing defect there cannot hide itself here.
//! Supports the entities the NEXUS FABRIC writer emits (LINE, ARC, POLYLINE with VERTEX, TEXT) and
//! reports every other entity kind by name.

pub type P = [f64; 2];

#[derive(Clone, Debug, PartialEq)]
pub enum Ent {
    Line {
        layer: String,
        a: P,
        b: P,
    },
    Arc {
        layer: String,
        c: P,
        r: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Poly {
        layer: String,
        pts: Vec<P>,
        closed: bool,
    },
    Text {
        layer: String,
        text: String,
    },
    Other {
        kind: String,
        layer: String,
    },
}

impl Ent {
    pub fn layer(&self) -> &str {
        match self {
            Ent::Line { layer, .. }
            | Ent::Arc { layer, .. }
            | Ent::Poly { layer, .. }
            | Ent::Text { layer, .. }
            | Ent::Other { layer, .. } => layer,
        }
    }
}

#[derive(Debug, Default)]
pub struct Parsed {
    /// Group 999 comment lines, trimmed, in file order.
    pub comments: Vec<String>,
    /// Value of $INSUNITS, when the header states it.
    pub insunits: Option<i64>,
    /// Section names in file order.
    pub sections: Vec<String>,
    /// Layer names of the LAYER table.
    pub layers: Vec<String>,
    pub ents: Vec<Ent>,
    /// Descriptions of numeric values that are NaN or infinite.
    pub nonfinite: Vec<String>,
    /// Number of ENDSEC records seen.
    pub endsec: usize,
    /// The file ends with an EOF record.
    pub eof: bool,
}

/// One record of a section: its name (the entity or table-entry kind) and its pairs.
struct Rec<'a> {
    name: &'a str,
    body: Vec<(i32, &'a str)>,
}

/// Split the text into (code, value) pairs. A line that is not an integer group code, or an odd number
/// of lines, means the file cannot be read at all.
fn pairs(text: &str) -> Result<Vec<(i32, &str)>, String> {
    let mut lines: Vec<&str> = text.lines().collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if !lines.len().is_multiple_of(2) {
        return Err("the file ends in the middle of a group code/value pair".into());
    }
    let mut out = Vec::with_capacity(lines.len() / 2);
    for (i, ch) in lines.chunks(2).enumerate() {
        let code = ch[0]
            .trim()
            .parse::<i32>()
            .map_err(|_| format!("line {} is not an integer group code", 2 * i + 1))?;
        out.push((code, ch[1].trim_end_matches('\r')));
    }
    Ok(out)
}

fn num(v: &str, what: &str) -> Result<f64, String> {
    v.trim()
        .parse::<f64>()
        .map_err(|_| format!("{what} is not a number"))
}

/// Coordinates and sizes of an entity, read as numbers. Non-finite values are kept (and reported by the
/// caller) so that a file with NaN is a FAIL, not an unreadable file.
fn coord(rec: &Rec<'_>, code: i32, what: &str, nonfinite: &mut Vec<String>) -> Result<f64, String> {
    let v = rec
        .body
        .iter()
        .find(|(c, _)| *c == code)
        .ok_or_else(|| format!("{} entity has no {what}", rec.name))?
        .1;
    let x = num(v, what)?;
    if !x.is_finite() {
        nonfinite.push(format!("{} {what}", rec.name));
    }
    Ok(x)
}

fn text_of<'a>(rec: &Rec<'a>, code: i32) -> Option<&'a str> {
    rec.body.iter().find(|(c, _)| *c == code).map(|(_, v)| *v)
}

fn layer_of(rec: &Rec<'_>) -> String {
    text_of(rec, 8).unwrap_or("0").to_string()
}

fn point(rec: &Rec<'_>, x: i32, y: i32, nonfinite: &mut Vec<String>) -> Result<P, String> {
    Ok([
        coord(rec, x, "X", nonfinite)?,
        coord(rec, y, "Y", nonfinite)?,
    ])
}

pub fn parse(text: &str) -> Result<Parsed, String> {
    let pairs = pairs(text)?;
    let mut parsed = Parsed::default();
    // sections: (name, records); the first record of a section holds the pairs before its first entity
    let mut sections: Vec<(String, Vec<Rec>)> = Vec::new();
    let mut in_section = false;
    let mut expect_name = false;
    for (code, value) in pairs {
        if expect_name {
            expect_name = false;
            if code != 2 {
                return Err("SECTION is not followed by its name".into());
            }
            sections.push((
                value.trim().to_string(),
                vec![Rec {
                    name: "",
                    body: vec![],
                }],
            ));
            in_section = true;
            continue;
        }
        if code == 0 {
            match value.trim() {
                "SECTION" => {
                    expect_name = true;
                    continue;
                }
                "ENDSEC" => {
                    parsed.endsec += 1;
                    in_section = false;
                    continue;
                }
                "EOF" => {
                    parsed.eof = true;
                    in_section = false;
                    continue;
                }
                kind => {
                    if let Some((_, recs)) = sections.last_mut().filter(|_| in_section) {
                        recs.push(Rec {
                            name: kind,
                            body: vec![],
                        });
                    }
                    continue;
                }
            }
        }
        if code == 999 {
            parsed.comments.push(value.trim().to_string());
            continue;
        }
        if let Some((_, recs)) = sections.last_mut().filter(|_| in_section)
            && let Some(rec) = recs.last_mut()
        {
            rec.body.push((code, value));
        }
    }
    // the pairs of a section are kept in its records; numbers are checked where they are used
    for (name, recs) in &sections {
        parsed.sections.push(name.clone());
        match name.as_str() {
            "HEADER" => {
                let body = &recs[0].body;
                for (i, (c, v)) in body.iter().enumerate() {
                    if *c == 9
                        && v.trim() == "$INSUNITS"
                        && let Some((70, n)) = body.get(i + 1)
                    {
                        let n = n
                            .trim()
                            .parse::<i64>()
                            .map_err(|_| "$INSUNITS is not an integer".to_string())?;
                        parsed.insunits = Some(n);
                    }
                }
            }
            "TABLES" => {
                for rec in recs.iter().filter(|r| r.name == "LAYER") {
                    if let Some(n) = text_of(rec, 2) {
                        parsed.layers.push(n.trim().to_string());
                    }
                }
            }
            "ENTITIES" => {
                parsed.ents = entities(recs, &mut parsed.nonfinite)?;
            }
            _ => {}
        }
    }
    Ok(parsed)
}

fn entities(recs: &[Rec<'_>], nonfinite: &mut Vec<String>) -> Result<Vec<Ent>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < recs.len() {
        let rec = &recs[i];
        i += 1;
        match rec.name {
            "" => {}
            "LINE" => out.push(Ent::Line {
                layer: layer_of(rec),
                a: point(rec, 10, 20, nonfinite)?,
                b: point(rec, 11, 21, nonfinite)?,
            }),
            "ARC" => out.push(Ent::Arc {
                layer: layer_of(rec),
                c: point(rec, 10, 20, nonfinite)?,
                r: coord(rec, 40, "radius", nonfinite)?,
                start_deg: coord(rec, 50, "start angle", nonfinite)?,
                end_deg: coord(rec, 51, "end angle", nonfinite)?,
            }),
            "POLYLINE" => {
                let flags = match text_of(rec, 70) {
                    Some(v) => v
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| "POLYLINE flags are not an integer".to_string())?,
                    None => 0,
                };
                let mut pts = Vec::new();
                while i < recs.len() && recs[i].name != "SEQEND" {
                    if recs[i].name == "VERTEX" {
                        pts.push(point(&recs[i], 10, 20, nonfinite)?);
                    }
                    i += 1;
                }
                i += 1; // SEQEND
                out.push(Ent::Poly {
                    layer: layer_of(rec),
                    pts,
                    closed: flags & 1 == 1,
                });
            }
            "TEXT" => out.push(Ent::Text {
                layer: layer_of(rec),
                text: text_of(rec, 1).unwrap_or("").to_string(),
            }),
            kind => out.push(Ent::Other {
                kind: kind.to_string(),
                layer: layer_of(rec),
            }),
        }
    }
    Ok(out)
}
