//! Minimal DXF writer and an *independent* reader used for re-import checks.
//!
//! Writer: ASCII DXF, AC1009 (R12) subset — LAYER table, LINE, ARC, POLYLINE.
//! R12 has no unit header variable: units (mm) are stated in a 999 comment
//! only. CAM-system acceptance has NOT been tested.
//!
//! The reader shares no code with the writer beyond the entity structs.

#[derive(Clone, Debug, PartialEq)]
pub enum Entity {
    Line { layer: String, a: (f64, f64), b: (f64, f64) },
    /// Counter-clockwise arc, angles in degrees (DXF convention).
    Arc { layer: String, c: (f64, f64), r: f64, start_deg: f64, end_deg: f64 },
    Polyline { layer: String, pts: Vec<(f64, f64)> },
}

impl Entity {
    pub fn layer(&self) -> &str {
        match self {
            Entity::Line { layer, .. } | Entity::Arc { layer, .. } | Entity::Polyline { layer, .. } => layer,
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
        }
    }
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

pub fn write(entities: &[Entity]) -> String {
    let mut o = String::new();
    let mut p = |code: i32, val: &str| {
        o.push_str(&format!("{code:>3}\n{val}\n"));
    };
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for e in entities {
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
    if entities.is_empty() {
        (minx, miny, maxx, maxy) = (0.0, 0.0, 0.0, 0.0);
    }
    p(999, "NEXUS FABRIC DXF - units: millimetres - not a released manufacturing file unless approved");
    p(0, "SECTION");
    p(2, "HEADER");
    p(9, "$ACADVER");
    p(1, "AC1009");
    p(9, "$EXTMIN");
    p(10, &num(minx));
    p(20, &num(miny));
    p(9, "$EXTMAX");
    p(10, &num(maxx));
    p(20, &num(maxy));
    p(0, "ENDSEC");
    let mut layers: Vec<&str> = entities.iter().map(|e| e.layer()).collect();
    layers.sort();
    layers.dedup();
    p(0, "SECTION");
    p(2, "TABLES");
    p(0, "TABLE");
    p(2, "LAYER");
    p(70, &layers.len().to_string());
    for l in &layers {
        p(0, "LAYER");
        p(2, l);
        p(70, "0");
        p(62, "7");
        p(6, "CONTINUOUS");
    }
    p(0, "ENDTAB");
    p(0, "ENDSEC");
    p(0, "SECTION");
    p(2, "ENTITIES");
    for e in entities {
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
        }
    }
    p(0, "ENDSEC");
    p(0, "EOF");
    o
}

/// Independent reader: parses group-code pairs and reconstructs the supported
/// entities. Unknown entity types are returned by name so that nothing is
/// silently ignored.
pub fn read(text: &str) -> Result<(Vec<Entity>, Vec<String>), String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() % 2 != 0 {
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
    pub entities_written: usize,
    pub entities_read: usize,
    pub unknown_entities: Vec<String>,
    /// Largest coordinate difference between intended and re-imported geometry.
    pub max_point_deviation: f64,
    /// Largest per-entity length difference.
    pub max_length_deviation: f64,
    pub layers_match: bool,
    pub pass: bool,
}

/// Compare intended entities with what an independent parse of `text` yields.
pub fn reimport_check(intended: &[Entity], text: &str, tol: f64) -> ReimportReport {
    let fail = |n: usize, u: Vec<String>| ReimportReport {
        entities_written: intended.len(),
        entities_read: n,
        unknown_entities: u,
        max_point_deviation: f64::INFINITY,
        max_length_deviation: f64::INFINITY,
        layers_match: false,
        pass: false,
    };
    let (got, unknown) = match read(text) {
        Ok(x) => x,
        Err(_) => return fail(0, vec![]),
    };
    if got.len() != intended.len() {
        return fail(got.len(), unknown);
    }
    let (mut dp, mut dl): (f64, f64) = (0.0, 0.0);
    let mut layers_match = true;
    for (a, b) in intended.iter().zip(&got) {
        if a.layer() != b.layer() || std::mem::discriminant(a) != std::mem::discriminant(b) {
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
    }
    let pass = unknown.is_empty() && layers_match && dp <= tol && dl <= tol;
    ReimportReport { entities_written: intended.len(), entities_read: got.len(), unknown_entities: unknown, max_point_deviation: dp, max_length_deviation: dl, layers_match, pass }
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
}
