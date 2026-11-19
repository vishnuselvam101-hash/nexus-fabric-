//! Minimal DXF writer and an *independent* reader used for re-import checks.
//!
//! Writer: ASCII DXF, AC1009 (R12) subset: LTYPE, LAYER and STYLE tables,
//! LINE, ARC, POLYLINE, and TEXT for annotation only.
//!
//! Units. R12 has no unit-of-measure header variable ($INSUNITS and
//! $MEASUREMENT were introduced by later formats and are deliberately not
//! written into an AC1009 file). What R12 does allow is written: $LUNITS = 2
//! (decimal linear units), $AUNITS = 0 (decimal degrees), $ANGBASE = 0 and
//! $ANGDIR = 0 (angles counter-clockwise from +x; the ARC entities depend on
//! this). The unit itself, millimetres, is stated in the leading 999 comment
//! and as a TEXT annotation inside the drawing.
//!
//! Release marking. Every file carries the annotation `MARK_TEXT` on the
//! non-cutting layer `MARK_LAYER`, placed below the geometry. This build can
//! not release anything, so the marking is unconditional. Cut geometry (the
//! LINE / ARC / POLYLINE entities and their layers) is written exactly as
//! before the marking existed.
//!
//! CAM-system acceptance has NOT been tested.
//!
//! The reader shares no code with the writer beyond the entity structs.

/// Layer that carries the annotation. Never a cutting layer.
pub const MARK_LAYER: &str = "NF_NOT_RELEASED";
pub const MARK_TEXT: &str = "NOT RELEASED - DEVELOPMENT BUILD - NOT VALIDATED - NOT FOR MANUFACTURING";
pub const UNITS_TEXT: &str = "UNITS: MILLIMETRES";

/// True for layers whose entities are cut paths.
pub fn is_cut_layer(layer: &str) -> bool {
    layer.starts_with("CUT")
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entity {
    Line { layer: String, a: (f64, f64), b: (f64, f64) },
    /// Counter-clockwise arc, angles in degrees (DXF convention).
    Arc { layer: String, c: (f64, f64), r: f64, start_deg: f64, end_deg: f64 },
    Polyline { layer: String, pts: Vec<(f64, f64)> },
    /// Annotation (single-line text, insertion point at its lower left). Never geometry.
    Text { layer: String, at: (f64, f64), height: f64, text: String },
}

impl Entity {
    pub fn layer(&self) -> &str {
        match self {
            Entity::Line { layer, .. } | Entity::Arc { layer, .. } | Entity::Polyline { layer, .. } | Entity::Text { layer, .. } => layer,
        }
    }
    pub fn length(&self) -> f64 {
        match self {
            Entity::Line { a, b, .. } => (a.0 - b.0).hypot(a.1 - b.1),
            Entity::Arc { r, start_deg, end_deg, .. } => {
                let mut sweep = end_deg - start_deg;
                if sweep < 0.0 {
                    sweep += 360.0;
                }
                r * sweep.to_radians()
            }
            Entity::Polyline { pts, .. } => pts.windows(2).map(|w| (w[0].0 - w[1].0).hypot(w[0].1 - w[1].1)).sum(),
            Entity::Text { .. } => 0.0,
        }
    }
    fn points(&self) -> Vec<(f64, f64)> {
        match self {
            Entity::Line { a, b, .. } => vec![*a, *b],
            Entity::Arc { c, r, start_deg, end_deg, .. } => vec![
                (c.0 + r * start_deg.to_radians().cos(), c.1 + r * start_deg.to_radians().sin()),
                (c.0 + r * end_deg.to_radians().cos(), c.1 + r * end_deg.to_radians().sin()),
            ],
            Entity::Polyline { pts, .. } => pts.clone(),
            Entity::Text { at, .. } => vec![*at],
        }
    }
    pub fn is_annotation(&self) -> bool {
        matches!(self, Entity::Text { .. })
    }
}

/// The annotation this build adds to every drawing: release marking and unit
/// statement, below the lower-left corner of the geometry, sized from its extent.
pub fn marking(minx: f64, miny: f64, maxx: f64, maxy: f64) -> Vec<Entity> {
    let span = (maxx - minx).max(maxy - miny);
    let h = if span.is_finite() && span > 0.0 { span / 50.0 } else { 1.0 };
    let (x, y) = (if minx.is_finite() { minx } else { 0.0 }, if miny.is_finite() { miny } else { 0.0 });
    vec![
        Entity::Text { layer: MARK_LAYER.into(), at: (x, y - 2.0 * h), height: h, text: MARK_TEXT.into() },
        Entity::Text { layer: MARK_LAYER.into(), at: (x, y - 4.0 * h), height: h, text: UNITS_TEXT.into() },
    ]
}

fn num(v: f64) -> String {
    // Shortest decimal that reads back to exactly the same binary64 value:
    // deterministic text and no precision loss. (A fixed number of decimals
    // was used before; for large-radius arcs the rounded angle moved the arc
    // end points by more than the re-import tolerance — regression 001.)
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{v}");
    if s.contains('.') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
}

/// Write the geometry `entities` (caller's cut geometry; any annotation among
/// them is ignored) followed by this build's release marking.
pub fn write(entities: &[Entity]) -> String {
    let geometry: Vec<&Entity> = entities.iter().filter(|e| !e.is_annotation()).collect();
    let mut o = String::new();
    let mut p = |code: i32, val: &str| {
        o.push_str(&format!("{code:>3}\n{val}\n"));
    };
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for e in &geometry {
        // extents from defining points (arcs: end points and centre +/- r, conservative)
        let mut pts = e.points();
        if let Entity::Arc { c, r, .. } = e {
            pts.push((c.0 - r, c.1 - r));
            pts.push((c.0 + r, c.1 + r));
        }
        for (x, y) in pts {
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        }
    }
    if geometry.is_empty() {
        (minx, miny, maxx, maxy) = (0.0, 0.0, 0.0, 0.0);
    }
    let notes = marking(minx, miny, maxx, maxy);
    // drawing extents include the annotation (text width estimated as one height per character)
    let (mut eminx, mut eminy, mut emaxx, mut emaxy) = (minx, miny, maxx, maxy);
    for n in &notes {
        if let Entity::Text { at, height, text, .. } = n {
            eminx = eminx.min(at.0);
            eminy = eminy.min(at.1);
            emaxx = emaxx.max(at.0 + height * text.len() as f64);
            emaxy = emaxy.max(at.1 + height);
        }
    }
    p(999, "NEXUS FABRIC DXF - units: millimetres - NOT RELEASED: development build, not validated, not for manufacturing");
    p(0, "SECTION");
    p(2, "HEADER");
    p(9, "$ACADVER");
    p(1, "AC1009");
    p(9, "$LUNITS");
    p(70, "2");
    p(9, "$AUNITS");
    p(70, "0");
    p(9, "$ANGBASE");
    p(50, "0.0");
    p(9, "$ANGDIR");
    p(70, "0");
    p(9, "$EXTMIN");
    p(10, &num(eminx));
    p(20, &num(eminy));
    p(9, "$EXTMAX");
    p(10, &num(emaxx));
    p(20, &num(emaxy));
    p(0, "ENDSEC");
    let mut layers: Vec<&str> = geometry.iter().map(|e| e.layer()).collect();
    layers.sort();
    layers.dedup();
    p(0, "SECTION");
    p(2, "TABLES");
    p(0, "TABLE");
    p(2, "LTYPE");
    p(70, "1");
    p(0, "LTYPE");
    p(2, "CONTINUOUS");
    p(70, "0");
    p(3, "Solid line");
    p(72, "65");
    p(73, "0");
    p(40, "0.0");
    p(0, "ENDTAB");
    p(0, "TABLE");
    p(2, "LAYER");
    p(70, &(layers.len() + 1).to_string());
    for l in &layers {
        p(0, "LAYER");
        p(2, l);
        p(70, "0");
        p(62, "7");
        p(6, "CONTINUOUS");
    }
    // annotation layer: its own colour (1 = red), never a cutting layer
    p(0, "LAYER");
    p(2, MARK_LAYER);
    p(70, "0");
    p(62, "1");
    p(6, "CONTINUOUS");
    p(0, "ENDTAB");
    p(0, "TABLE");
    p(2, "STYLE");
    p(70, "1");
    p(0, "STYLE");
    p(2, "STANDARD");
    p(70, "0");
    p(40, "0.0");
    p(41, "1.0");
    p(50, "0.0");
    p(71, "0");
    p(42, "1.0");
    p(3, "txt");
    p(4, "");
    p(0, "ENDTAB");
    p(0, "ENDSEC");
    p(0, "SECTION");
    p(2, "ENTITIES");
    for e in geometry.iter().copied().chain(notes.iter()) {
        match e {
            Entity::Line { layer, a, b } => {
                p(0, "LINE");
                p(8, layer);
                p(10, &num(a.0));
                p(20, &num(a.1));
                p(11, &num(b.0));
                p(21, &num(b.1));
            }
            Entity::Arc { layer, c, r, start_deg, end_deg } => {
                p(0, "ARC");
                p(8, layer);
                p(10, &num(c.0));
                p(20, &num(c.1));
                p(40, &num(*r));
                p(50, &num(*start_deg));
                p(51, &num(*end_deg));
            }
            Entity::Polyline { layer, pts } => {
                p(0, "POLYLINE");
                p(8, layer);
                p(66, "1");
                p(70, "0");
                for q in pts {
                    p(0, "VERTEX");
                    p(8, layer);
                    p(10, &num(q.0));
                    p(20, &num(q.1));
                }
                p(0, "SEQEND");
            }
            Entity::Text { layer, at, height, text } => {
                p(0, "TEXT");
                p(8, layer);
                p(10, &num(at.0));
                p(20, &num(at.1));
                p(40, &num(*height));
                p(1, text);
                p(7, "STANDARD");
            }
        }
    }
    p(0, "ENDSEC");
    p(0, "EOF");
    o
}

/// Independent header reader: every `$VARIABLE` of the HEADER section with the
/// group-code pairs that follow it.
pub fn read_header(text: &str) -> Result<std::collections::BTreeMap<String, Vec<(i32, String)>>, String> {
    let lines: Vec<&str> = text.lines().collect();
    if !lines.len().is_multiple_of(2) {
        return Err("odd number of lines: group code without value".into());
    }
    let mut vars = std::collections::BTreeMap::new();
    let (mut in_header, mut current): (bool, Option<String>) = (false, None);
    for ch in lines.chunks(2) {
        let code: i32 = ch[0].trim().parse().map_err(|_| format!("bad group code '{}'", ch[0]))?;
        let val = ch[1].trim();
        match (code, val) {
            (2, "HEADER") => in_header = true,
            (0, "ENDSEC") if in_header => return Ok(vars),
            (9, name) if in_header => {
                current = Some(name.to_string());
                vars.insert(name.to_string(), Vec::new());
            }
            _ if in_header => {
                if let Some(v) = current.as_ref().and_then(|c| vars.get_mut(c)) {
                    v.push((code, val.to_string()));
                }
            }
            _ => {}
        }
    }
    Err("no complete HEADER section".into())
}

/// Independent reader: parses group-code pairs and reconstructs the supported
/// entities. Unknown entity types are returned by name so that nothing is
/// silently ignored.
pub fn read(text: &str) -> Result<(Vec<Entity>, Vec<String>), String> {
    let lines: Vec<&str> = text.lines().collect();
    if !lines.len().is_multiple_of(2) {
        return Err("odd number of lines: group code without value".into());
    }
    let mut pairs = Vec::with_capacity(lines.len() / 2);
    for ch in lines.chunks(2) {
        let code: i32 = ch[0].trim().parse().map_err(|_| format!("bad group code '{}'", ch[0]))?;
        pairs.push((code, ch[1].trim()));
    }
    let start = pairs.iter().position(|&(c, v)| c == 2 && v == "ENTITIES").ok_or("no ENTITIES section")?;
    let mut ents = Vec::new();
    let mut unknown = Vec::new();
    let mut i = start + 1;
    let f = |v: &str| v.parse::<f64>().map_err(|_| format!("bad number '{v}'"));
    let mut saw_eof = false;
    while i < pairs.len() {
        let (code, val) = pairs[i];
        if code != 0 {
            i += 1;
            continue;
        }
        // collect this entity's own pairs
        let mut j = i + 1;
        while j < pairs.len() && pairs[j].0 != 0 {
            j += 1;
        }
        let body = &pairs[i + 1..j];
        let get = |c: i32| body.iter().find(|p| p.0 == c).map(|p| p.1);
        let layer = get(8).unwrap_or("0").to_string();
        match val {
            "LINE" => ents.push(Entity::Line {
                layer,
                a: (f(get(10).ok_or("LINE without 10")?)?, f(get(20).ok_or("LINE without 20")?)?),
                b: (f(get(11).ok_or("LINE without 11")?)?, f(get(21).ok_or("LINE without 21")?)?),
            }),
            "ARC" => ents.push(Entity::Arc {
                layer,
                c: (f(get(10).ok_or("ARC without 10")?)?, f(get(20).ok_or("ARC without 20")?)?),
                r: f(get(40).ok_or("ARC without 40")?)?,
                start_deg: f(get(50).ok_or("ARC without 50")?)?,
                end_deg: f(get(51).ok_or("ARC without 51")?)?,
            }),
            "POLYLINE" => {
                let mut pts = Vec::new();
                let mut k = j;
                let mut closed = false;
                while k < pairs.len() {
                    if pairs[k].0 == 0 && pairs[k].1 == "VERTEX" {
                        let mut m = k + 1;
                        while m < pairs.len() && pairs[m].0 != 0 {
                            m += 1;
                        }
                        let vb = &pairs[k + 1..m];
                        let g = |c: i32| vb.iter().find(|p| p.0 == c).map(|p| p.1);
                        pts.push((f(g(10).ok_or("VERTEX without 10")?)?, f(g(20).ok_or("VERTEX without 20")?)?));
                        k = m;
                    } else if pairs[k].0 == 0 && pairs[k].1 == "SEQEND" {
                        closed = true;
                        k += 1;
                        break;
                    } else {
                        break;
                    }
                }
                if !closed {
                    return Err("POLYLINE without SEQEND".into());
                }
                ents.push(Entity::Polyline { layer, pts });
                j = k;
            }
            "TEXT" => ents.push(Entity::Text {
                layer,
                at: (f(get(10).ok_or("TEXT without 10")?)?, f(get(20).ok_or("TEXT without 20")?)?),
                height: f(get(40).ok_or("TEXT without 40")?)?,
                text: get(1).ok_or("TEXT without 1")?.to_string(),
            }),
            "ENDSEC" => {}
            "EOF" => {
                saw_eof = true;
            }
            other => unknown.push(other.to_string()),
        }
        i = j;
    }
    if !saw_eof {
        return Err("missing EOF".into());
    }
    Ok((ents, unknown))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReimportReport {
    /// Geometry entities (annotation is counted separately).
    pub entities_written: usize,
    pub entities_read: usize,
    pub annotations_read: usize,
    pub unknown_entities: Vec<String>,
    /// Largest coordinate difference between intended and re-imported geometry.
    pub max_point_deviation: f64,
    /// Largest per-entity length difference.
    pub max_length_deviation: f64,
    pub layers_match: bool,
    /// The NOT RELEASED marking is present, on the annotation layer only, clear
    /// of the geometry, and no annotation sits on a cutting layer.
    pub not_released_marking: bool,
    /// Header declares decimal units and counter-clockwise degrees, and the
    /// drawing states millimetres.
    pub units_declared: bool,
    pub pass: bool,
}

/// Compare intended geometry with what an independent parse of `text` yields,
/// and confirm the release marking and unit declarations of the file.
pub fn reimport_check(intended: &[Entity], text: &str, tol: f64) -> ReimportReport {
    let intended: Vec<&Entity> = intended.iter().filter(|e| !e.is_annotation()).collect();
    let fail = |n: usize, u: Vec<String>| ReimportReport {
        entities_written: intended.len(),
        entities_read: n,
        annotations_read: 0,
        unknown_entities: u,
        max_point_deviation: f64::INFINITY,
        max_length_deviation: f64::INFINITY,
        layers_match: false,
        not_released_marking: false,
        units_declared: false,
        pass: false,
    };
    let (all, unknown) = match read(text) {
        Ok(x) => x,
        Err(_) => return fail(0, vec![]),
    };
    let (notes, got): (Vec<Entity>, Vec<Entity>) = all.into_iter().partition(|e| e.is_annotation());
    if got.len() != intended.len() {
        return fail(got.len(), unknown);
    }
    let (mut dp, mut dl): (f64, f64) = (0.0, 0.0);
    let mut layers_match = true;
    let mut lowest = f64::INFINITY;
    for (a, b) in intended.iter().zip(&got) {
        if a.layer() != b.layer() || std::mem::discriminant(*a) != std::mem::discriminant(b) {
            layers_match = false;
        }
        let (pa, pb) = (a.points(), b.points());
        if pa.len() != pb.len() {
            return fail(got.len(), unknown);
        }
        for (x, y) in pa.iter().zip(&pb) {
            dp = dp.max((x.0 - y.0).abs()).max((x.1 - y.1).abs());
        }
        dl = dl.max((a.length() - b.length()).abs());
        let mut ys: Vec<f64> = pb.iter().map(|q| q.1).collect();
        if let Entity::Arc { c, r, .. } = b {
            ys.push(c.1 - r);
        }
        lowest = ys.into_iter().fold(lowest, f64::min);
    }
    // Marking: exactly on the annotation layer, nothing else there, no text on a cut layer, below the geometry.
    let text_of = |e: &Entity| match e {
        Entity::Text { text, .. } => text.clone(),
        _ => String::new(),
    };
    let on_mark = |want: &str| notes.iter().any(|n| n.layer() == MARK_LAYER && text_of(n) == want);
    let notes_clear = notes.iter().all(|n| n.layer() == MARK_LAYER && !is_cut_layer(n.layer()) && matches!(n, Entity::Text { at, height, .. } if got.is_empty() || at.1 + height <= lowest));
    let geometry_clear = got.iter().all(|g| g.layer() != MARK_LAYER);
    let not_released_marking = on_mark(MARK_TEXT) && notes_clear && geometry_clear;
    let header = read_header(text).unwrap_or_default();
    let var = |name: &str, code: i32, want: &str| header.get(name).is_some_and(|v| v.iter().any(|(c, x)| *c == code && x == want));
    let units_declared = var("$ACADVER", 1, "AC1009") && var("$LUNITS", 70, "2") && var("$AUNITS", 70, "0") && var("$ANGDIR", 70, "0") && var("$ANGBASE", 50, "0.0") && on_mark(UNITS_TEXT);
    let pass = unknown.is_empty() && layers_match && dp <= tol && dl <= tol && not_released_marking && units_declared;
    ReimportReport { entities_written: intended.len(), entities_read: got.len(), annotations_read: notes.len(), unknown_entities: unknown, max_point_deviation: dp, max_length_deviation: dl, layers_match, not_released_marking, units_declared, pass }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Entity> {
        vec![
            Entity::Line { layer: "CUT_OUTER".into(), a: (0.0, 0.0), b: (628.318530718, 0.0) },
            Entity::Arc { layer: "CUT_OUTER".into(), c: (0.0, 0.0), r: 632.4555320337, start_deg: 0.0, end_deg: 113.8419957661 },
            Entity::Polyline { layer: "CUT_OUTER".into(), pts: vec![(1.0, 2.0), (3.5, -4.25), (1e3, 1e-3)] },
        ]
    }

    #[test]
    fn round_trip() {
        let e = sample();
        let t = write(&e);
        let r = reimport_check(&e, &t, 1e-8);
        assert!(r.pass, "{r:?}");
        assert_eq!(write(&e), t, "writer must be deterministic");
    }

    #[test]
    fn large_radius_arcs_survive_exactly() {
        // Regression 001: 9 fixed decimals moved the end of this arc by about 1e-5 mm.
        let e = vec![Entity::Arc { layer: "CUT_OUTER".into(), c: (0.0, 0.0), r: 987654.3210987654, start_deg: 0.0, end_deg: 12.345678901234567 }, Entity::Line { layer: "CUT_OUTER".into(), a: (1e-9, -0.0), b: (123456789.12345679, 1.0 / 3.0) }];
        let r = reimport_check(&e, &write(&e), 0.0);
        assert!(r.pass && r.max_point_deviation == 0.0 && r.max_length_deviation == 0.0, "{r:?}");
    }

    #[test]
    fn detects_corruption_truncation_and_foreign_entities() {
        let e = sample();
        let t = write(&e);
        assert!(!reimport_check(&e, &t.replace("632.4555320337", "632.4555321337"), 1e-8).pass);
        assert!(!reimport_check(&e, &t[..t.len() - 12], 1e-8).pass);
        assert!(!reimport_check(&e, &t.replace("  0\nEOF", "  0\nCIRCLE\n  8\nX\n  0\nEOF"), 1e-8).pass);
        assert!(!reimport_check(&e, &t.replacen("CUT_OUTER\n 10\n0.0", "NO_CUT\n 10\n0.0", 1), 1e-8).pass);
        assert!(read("garbage").is_err());
    }

    /// The marking is annotation on its own layer; the cut entities are written
    /// exactly as before it existed (same group codes, same exact numbers).
    #[test]
    fn not_released_marking_and_units_without_touching_cut_geometry() {
        let e = vec![Entity::Line { layer: "CUT_OUTER".into(), a: (0.0, 0.0), b: (1.0 / 3.0, 250.0) }, Entity::Polyline { layer: "CUT_INNER".into(), pts: vec![(1.0, 2.0), (3.5, 4.25), (1.0, 2.0)] }];
        let t = write(&e);
        let cut = "  0\nLINE\n  8\nCUT_OUTER\n 10\n0.0\n 20\n0.0\n 11\n0.3333333333333333\n 21\n250.0\n  0\nPOLYLINE\n  8\nCUT_INNER\n 66\n1\n 70\n0\n  0\nVERTEX\n  8\nCUT_INNER\n 10\n1.0\n 20\n2.0\n  0\nVERTEX\n  8\nCUT_INNER\n 10\n3.5\n 20\n4.25\n  0\nVERTEX\n  8\nCUT_INNER\n 10\n1.0\n 20\n2.0\n  0\nSEQEND\n";
        assert!(t.contains(&format!("  2\nENTITIES\n{cut}  0\nTEXT\n")), "cut geometry must be written unchanged and before any annotation");
        let (all, unknown) = read(&t).unwrap();
        assert!(unknown.is_empty());
        let notes: Vec<&Entity> = all.iter().filter(|x| x.is_annotation()).collect();
        assert_eq!(notes.len(), 2);
        assert!(notes.iter().all(|n| n.layer() == MARK_LAYER && !is_cut_layer(n.layer())));
        assert!(matches!(notes[0], Entity::Text { text, at, height, .. } if text == MARK_TEXT && at.1 + height < 0.0), "marking lies below the geometry");
        assert!(all.iter().filter(|x| !x.is_annotation()).all(|g| is_cut_layer(g.layer())));
        let h = read_header(&t).unwrap();
        assert_eq!(h["$LUNITS"], vec![(70, "2".to_string())]);
        assert_eq!(h["$AUNITS"], vec![(70, "0".to_string())]);
        assert_eq!(h["$ANGDIR"], vec![(70, "0".to_string())]);
        assert!(!h.contains_key("$INSUNITS"), "not an R12 variable");
        assert!(t.contains("units: millimetres") && t.contains(UNITS_TEXT));
        let r = reimport_check(&e, &t, 0.0);
        assert!(r.pass && r.not_released_marking && r.units_declared && r.annotations_read == 2 && r.entities_read == 2 && r.max_point_deviation == 0.0, "{r:?}");
        // an empty drawing still carries the marking
        assert!(reimport_check(&[], &write(&[]), 0.0).pass);
    }

    #[test]
    fn missing_moved_or_altered_marking_and_units_fail_the_check() {
        let e = sample();
        let t = write(&e);
        let no_mark = t.replace(MARK_TEXT, "RELEASED");
        assert!(!reimport_check(&e, &no_mark, 1e-8).not_released_marking && !reimport_check(&e, &no_mark, 1e-8).pass);
        let on_cut = t.replace(&format!("TEXT\n  8\n{MARK_LAYER}"), "TEXT\n  8\nCUT_OUTER");
        assert!(!reimport_check(&e, &on_cut, 1e-8).pass, "annotation on a cutting layer");
        let geometry_on_mark = t.replacen("LINE\n  8\nCUT_OUTER", &format!("LINE\n  8\n{MARK_LAYER}"), 1);
        assert!(!reimport_check(&e, &geometry_on_mark, 1e-8).pass, "geometry on the annotation layer");
        let inches = t.replace("$LUNITS\n 70\n2", "$LUNITS\n 70\n4");
        assert!(!reimport_check(&e, &inches, 1e-8).units_declared && !reimport_check(&e, &inches, 1e-8).pass);
        let clockwise = t.replace("$ANGDIR\n 70\n0", "$ANGDIR\n 70\n1");
        assert!(!reimport_check(&e, &clockwise, 1e-8).pass);
        let no_units = t.replace(UNITS_TEXT, "UNITS: INCHES");
        assert!(!reimport_check(&e, &no_units, 1e-8).pass);
        let stripped: String = { let i = t.find("  0\nTEXT").unwrap(); let j = t.rfind("  0\nENDSEC").unwrap(); format!("{}{}", &t[..i], &t[j..]) };
        assert!(read(&stripped).is_ok() && !reimport_check(&e, &stripped, 1e-8).pass, "marking removed");
    }
}
