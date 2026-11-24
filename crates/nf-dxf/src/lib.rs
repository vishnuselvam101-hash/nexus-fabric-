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
//! this). The unit itself, millimetres, is stated three ways: the human line
//! of the leading 999 comment block, the key line `NF-UNITS: mm` in the same
//! block, and a TEXT annotation inside the drawing.
//!
//! Statements. A header-with-branch part also states its joint: the 999
//! comment lines `NF-JOINT`, `NF-HEADER-SURFACE` and `NF-BRANCH-SURFACE`, and
//! two TEXT annotations on the annotation layer. The joint is "set-on" or "not
//! declared", in every output. The key/value form and every key are documented
//! in docs/backend/DXF_METADATA.md. Other readers may ignore 999 comments; the
//! TEXT annotations carry the same facts visibly.
//!
//! Release marking. Every file carries the annotation `MARK_TEXT` on the
//! non-cutting layer `MARK_LAYER`, placed below the geometry. This build can
//! not release anything, so the marking is unconditional. Cut geometry (the
//! LINE / ARC / POLYLINE entities and their layers) is written exactly as
//! before the marking existed.
//!
//! CAM-system acceptance has NOT been tested.
//!
//! The reader shares no code with the writer beyond the entity structs. It was
//! written by the same author and lives in the same crate: "independent" here
//! means a separate implementation, not a separate owner (review RB-006).
//!
//! Quality gate. `quality::quality_gate` is an error-detection pass over a file
//! as written and re-read: open contours, duplicate or overlapping entities,
//! zero-length segments, self-intersections, inner profiles outside the outer
//! one, wrong layer, missing units, wrong extents. It looks only at the file.

pub mod quality;

/// Layer that carries the annotation. Never a cutting layer.
pub const MARK_LAYER: &str = "NF_NOT_RELEASED";
pub const MARK_TEXT: &str =
    "NOT RELEASED - DEVELOPMENT BUILD - NOT VALIDATED - NOT FOR MANUFACTURING";
pub const UNITS_TEXT: &str = "UNITS: MILLIMETRES";

/// Key of the units line of the 999 comment block; its value is `mm`.
pub const KEY_UNITS: &str = "NF-UNITS";
/// Keys of the joint statement (header-with-branch parts only).
pub const KEY_JOINT: &str = "NF-JOINT";
pub const KEY_HEADER_SURFACE: &str = "NF-HEADER-SURFACE";
pub const KEY_BRANCH_SURFACE: &str = "NF-BRANCH-SURFACE";
/// The only two joint states that any output may name (DXF, records, interface, docs).
pub const JOINT_SET_ON: &str = "set-on";
pub const JOINT_NOT_DECLARED: &str = "not declared";
/// TEXT annotations that belong to the joint statement. Any other annotation starting with one of these
/// prefixes is a contradiction and fails the quality gate.
pub const JOINT_TEXT_PREFIXES: [&str; 2] = ["JOINT:", "HEADER SURFACE:"];

/// Keys of the plate-convention statement (work plan task 1.3; docs/backend/DXF_METADATA.md, "Plate conventions").
/// A part states exactly the keys its family needs: the offset keys only for a part that has an offset.
pub const KEY_VIEW_SIDE: &str = "NF-VIEW-SIDE";
pub const KEY_PLATE_AXES: &str = "NF-PLATE-AXES";
pub const KEY_ROLLING_AXIS: &str = "NF-ROLLING-AXIS";
pub const KEY_SEAM_EDGES: &str = "NF-SEAM-EDGES";
pub const KEY_END_EDGES: &str = "NF-END-EDGES";
pub const KEY_ANGLE_DATUM: &str = "NF-ANGLE-DATUM";
pub const KEY_ORIGIN: &str = "NF-ORIGIN";
pub const KEY_KERF: &str = "NF-KERF";
pub const KEY_STATUS: &str = "NF-STATUS";
pub const KEY_OFFSET_DELTA: &str = "NF-OFFSET-DELTA";
pub const KEY_OFFSET_SIGN: &str = "NF-OFFSET-SIGN";
pub const KEY_BRANCH_ANGLE: &str = "NF-BRANCH-ANGLE";
/// Every plate-convention key this build writes.
pub const CONVENTION_KEYS: [&str; 12] = [
    KEY_VIEW_SIDE,
    KEY_PLATE_AXES,
    KEY_ROLLING_AXIS,
    KEY_SEAM_EDGES,
    KEY_END_EDGES,
    KEY_ANGLE_DATUM,
    KEY_ORIGIN,
    KEY_KERF,
    KEY_STATUS,
    KEY_OFFSET_DELTA,
    KEY_OFFSET_SIGN,
    KEY_BRANCH_ANGLE,
];
/// Longest value a group-999 comment line holds (DXF strings are at most 255 characters; kept below it).
pub const MAX_COMMENT_CHARS: usize = 240;

/// The plate conventions of one part: (key, value) in the order they are written. In the file each fact is
/// a 999 line `key: value` AND a TEXT annotation on the annotation layer, `LABEL: VALUE` in capitals.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Conventions {
    pub facts: Vec<(String, String)>,
}

impl Conventions {
    /// The 999 comment lines, as (key, value).
    pub fn comments(&self) -> Vec<(String, String)> {
        self.facts.clone()
    }

    /// The annotation lines (before splitting at `MAX_NOTE_CHARS`): the key without `NF-`, its dashes as
    /// spaces, then the value in capitals.
    pub fn texts(&self) -> Vec<String> {
        self.facts
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}: {}",
                    k.trim_start_matches("NF-").replace('-', " "),
                    v.to_uppercase()
                )
            })
            .collect()
    }
}

/// The annotation line that states the marking side, from the viewing side as the statement holds it
/// (`outside` or `inside`). The job writes its marking-side line with this same function.
pub fn marking_side_line(side: &str) -> String {
    format!(
        "MARKING SIDE: PATTERN DRAWN AS SEEN FROM THE {} OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED",
        side.to_uppercase()
    )
}

/// True for layers whose entities are cut paths.
pub fn is_cut_layer(layer: &str) -> bool {
    layer.starts_with("CUT")
}

/// The joint of a header-with-branch part as the job declares it, and the wall surfaces the opening was
/// cut for. Built from the request; written to the file; read back and compared by the re-import check and
/// by the quality gate. A file that states anything else is withheld.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JointStatement {
    /// `JOINT_SET_ON` or `JOINT_NOT_DECLARED`.
    pub joint: &'static str,
    pub header_surface: String,
    pub branch_surface: String,
}

impl JointStatement {
    /// `set_on`: the job declares the set-on joint (Spec 02 section 4.4). Otherwise the joint is not declared.
    pub fn new(set_on: bool, header_surface: &str, branch_surface: &str) -> JointStatement {
        JointStatement {
            joint: if set_on {
                JOINT_SET_ON
            } else {
                JOINT_NOT_DECLARED
            },
            header_surface: header_surface.to_string(),
            branch_surface: branch_surface.to_string(),
        }
    }

    /// The 999 comment lines, as (key, value).
    pub fn comments(&self) -> Vec<(String, String)> {
        vec![
            (KEY_JOINT.to_string(), self.joint.to_string()),
            (KEY_HEADER_SURFACE.to_string(), self.header_surface.clone()),
            (KEY_BRANCH_SURFACE.to_string(), self.branch_surface.clone()),
        ]
    }

    /// The TEXT annotations, in the order they are drawn. Capitals, as all annotation text of the drawing.
    pub fn texts(&self) -> Vec<String> {
        let joint = if self.joint == JOINT_SET_ON {
            "JOINT: SET-ON (SPEC 02 SECTION 4.4): BRANCH INSIDE SURFACE ON HEADER OUTSIDE SURFACE, OPENING IS THE BRANCH BORE"
        } else {
            "JOINT: NOT DECLARED: BRANCH END AND HEADER OPENING ARE CUT ON ONE CURVE OF THE ENTERED RADII"
        };
        vec![
            joint.to_string(),
            format!(
                "HEADER SURFACE: {} - BRANCH SURFACE: {}",
                self.header_surface.to_uppercase(),
                self.branch_surface.to_uppercase()
            ),
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entity {
    Line {
        layer: String,
        a: (f64, f64),
        b: (f64, f64),
    },
    /// Counter-clockwise arc, angles in degrees (DXF convention).
    Arc {
        layer: String,
        c: (f64, f64),
        r: f64,
        start_deg: f64,
        end_deg: f64,
    },
    /// `closed`: the polyline is a closed profile; the closing segment runs from
    /// the last vertex back to the first, which is NOT repeated (DXF flag 70 bit 1).
    Polyline {
        layer: String,
        pts: Vec<(f64, f64)>,
        closed: bool,
    },
    /// Annotation (single-line text, insertion point at its lower left). Never geometry.
    Text {
        layer: String,
        at: (f64, f64),
        height: f64,
        text: String,
    },
}

impl Entity {
    pub fn layer(&self) -> &str {
        match self {
            Entity::Line { layer, .. }
            | Entity::Arc { layer, .. }
            | Entity::Polyline { layer, .. }
            | Entity::Text { layer, .. } => layer,
        }
    }
    pub fn length(&self) -> f64 {
        match self {
            Entity::Line { a, b, .. } => (a.0 - b.0).hypot(a.1 - b.1),
            Entity::Arc {
                r,
                start_deg,
                end_deg,
                ..
            } => {
                let mut sweep = end_deg - start_deg;
                if sweep < 0.0 {
                    sweep += 360.0;
                }
                r * sweep.to_radians()
            }
            Entity::Polyline { pts, closed, .. } => {
                let open: f64 = pts
                    .windows(2)
                    .map(|w| (w[0].0 - w[1].0).hypot(w[0].1 - w[1].1))
                    .sum();
                match (closed, pts.first(), pts.last()) {
                    (true, Some(a), Some(b)) => open + (a.0 - b.0).hypot(a.1 - b.1),
                    _ => open,
                }
            }
            Entity::Text { .. } => 0.0,
        }
    }
    pub(crate) fn points(&self) -> Vec<(f64, f64)> {
        match self {
            Entity::Line { a, b, .. } => vec![*a, *b],
            Entity::Arc {
                c,
                r,
                start_deg,
                end_deg,
                ..
            } => vec![
                (
                    c.0 + r * start_deg.to_radians().cos(),
                    c.1 + r * start_deg.to_radians().sin(),
                ),
                (
                    c.0 + r * end_deg.to_radians().cos(),
                    c.1 + r * end_deg.to_radians().sin(),
                ),
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
    marking_with(minx, miny, maxx, maxy, &[])
}

/// Longest annotation line written (an R12 text string holds at most 255 characters).
pub const MAX_NOTE_CHARS: usize = 250;

/// The annotation lines as they are written: control characters replaced, and a
/// line longer than `MAX_NOTE_CHARS` split at a space into several lines.
/// Nothing is ever cut off.
pub fn annotation_lines(extra: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for line in extra {
        // one line of plain text: a line break would break the group-code structure of the file
        let clean: String = line
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .trim()
            .to_string();
        let mut rest = clean.as_str();
        while rest.chars().count() > MAX_NOTE_CHARS {
            let limit = rest
                .char_indices()
                .nth(MAX_NOTE_CHARS)
                .map(|(i, _)| i)
                .unwrap_or(rest.len());
            let cut = rest[..limit].rfind(' ').filter(|i| *i > 0).unwrap_or(limit);
            out.push(rest[..cut].trim().to_string());
            rest = rest[cut..].trim_start();
        }
        out.push(rest.to_string());
    }
    out
}

/// As `marking`, followed by further annotation lines supplied by the caller.
/// The first of them (the declared marking side of the pattern) is written in
/// the size of the release marking; the others (what the plate coordinates
/// mean: axes, seams, datums, kerf) in fine print below it, so that the
/// annotation block stays small against the part.
pub fn marking_with(minx: f64, miny: f64, maxx: f64, maxy: f64, extra: &[String]) -> Vec<Entity> {
    let (big, small) = extra.split_at(extra.len().min(1));
    marking_stated(minx, miny, maxx, maxy, big, small)
}

/// The annotation block: the release marking and the unit statement, then the `big` lines at the size of
/// the release marking (one line every two heights, starting below the marking side), then the `small`
/// lines in fine print below them. With one `big` line the block is exactly the one `marking_with` drew.
pub fn marking_stated(
    minx: f64,
    miny: f64,
    maxx: f64,
    maxy: f64,
    big: &[String],
    small: &[String],
) -> Vec<Entity> {
    let span = (maxx - minx).max(maxy - miny);
    let h = if span.is_finite() && span > 0.0 {
        span / 50.0
    } else {
        1.0
    };
    let (x, y) = (
        if minx.is_finite() { minx } else { 0.0 },
        if miny.is_finite() { miny } else { 0.0 },
    );
    let mut notes = vec![
        Entity::Text {
            layer: MARK_LAYER.into(),
            at: (x, y - 2.0 * h),
            height: h,
            text: MARK_TEXT.into(),
        },
        Entity::Text {
            layer: MARK_LAYER.into(),
            at: (x, y - 4.0 * h),
            height: h,
            text: UNITS_TEXT.into(),
        },
    ];
    let big = annotation_lines(big);
    for (i, line) in big.iter().enumerate() {
        notes.push(Entity::Text {
            layer: MARK_LAYER.into(),
            at: (x, y - 6.0 * h - 2.0 * h * i as f64),
            height: h,
            text: line.clone(),
        });
    }
    let fine = h / 10.0;
    let last_big = big.len().max(1) - 1;
    for (j, line) in annotation_lines(small).into_iter().enumerate() {
        notes.push(Entity::Text {
            layer: MARK_LAYER.into(),
            at: (
                x,
                y - 6.0 * h - 2.0 * h * last_big as f64 - 2.0 * fine * (j + 1) as f64,
            ),
            height: fine,
            text: line,
        });
    }
    notes
}

/// Exact bounding box of a counter-clockwise arc: its end points and every
/// axis direction (0, 90, 180, 270 degrees) that lies inside its sweep.
pub(crate) fn arc_box(c: (f64, f64), r: f64, start_deg: f64, end_deg: f64) -> [(f64, f64); 6] {
    let at = |deg: f64| {
        (
            c.0 + r * deg.to_radians().cos(),
            c.1 + r * deg.to_radians().sin(),
        )
    };
    let (a, b) = (at(start_deg), at(end_deg));
    let sweep = {
        let s = (end_deg - start_deg) % 360.0;
        if s < 0.0 { s + 360.0 } else { s }
    };
    let inside = |deg: f64| {
        let d = (deg - start_deg) % 360.0;
        (if d < 0.0 { d + 360.0 } else { d }) <= sweep
    };
    let axis = |deg: f64, p: (f64, f64)| if inside(deg) { p } else { a };
    [
        a,
        b,
        axis(0.0, (c.0 + r, c.1)),
        axis(90.0, (c.0, c.1 + r)),
        axis(180.0, (c.0 - r, c.1)),
        axis(270.0, (c.0, c.1 - r)),
    ]
}

/// Bounding box by a different method (dense sampling of arcs), used by the
/// round-trip check to test the extents the writer put into the header.
/// Returns (minx, miny, maxx, maxy, largest sampling error).
fn sampled_box(entities: &[Entity]) -> (f64, f64, f64, f64, f64) {
    const N: usize = 4096;
    let (mut b, mut err) = ((f64::MAX, f64::MAX, f64::MIN, f64::MIN), 0.0f64);
    let mut add = |x: f64, y: f64| b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
    for e in entities {
        match e {
            Entity::Arc {
                c,
                r,
                start_deg,
                end_deg,
                ..
            } => {
                let mut sweep = end_deg - start_deg;
                if sweep < 0.0 {
                    sweep += 360.0;
                }
                for i in 0..=N {
                    let a = (start_deg + sweep * i as f64 / N as f64).to_radians();
                    add(c.0 + r * a.cos(), c.1 + r * a.sin());
                }
                err = err.max(r.abs() * (1.0 - (sweep.to_radians() / (2.0 * N as f64)).cos()));
            }
            Entity::Text {
                at, height, text, ..
            } => {
                add(at.0, at.1);
                add(at.0 + height * text.len() as f64, at.1 + height);
            }
            other => other.points().into_iter().for_each(|(x, y)| add(x, y)),
        }
    }
    (b.0, b.1, b.2, b.3, err)
}

fn num(v: f64) -> String {
    // Shortest decimal that reads back to exactly the same binary64 value:
    // deterministic text and no precision loss. (A fixed number of decimals
    // was used before; for large-radius arcs the rounded angle moved the arc
    // end points by more than the re-import tolerance — regression 001.)
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{v}");
    if s.contains('.') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// Write the geometry `entities` (caller's cut geometry; any annotation among
/// them is ignored) followed by this build's release marking.
pub fn write(entities: &[Entity]) -> String {
    write_annotated(entities, &[])
}

/// As `write`, with further annotation lines on the annotation layer (the first at the size of the release
/// marking, the others in fine print).
pub fn write_annotated(entities: &[Entity], extra_notes: &[String]) -> String {
    let (big, small) = extra_notes.split_at(extra_notes.len().min(1));
    write_stated(entities, big, small, &[])
}

/// A value written on one line: control characters become spaces and the ends are trimmed.
fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The file as NEXUS FABRIC writes every DXF: the geometry, the release marking and the units, then the
/// statement of the part. `big` and `small` are annotation lines (see `marking_stated`); `comments` are the
/// application-specific 999 lines `key: value` (see docs/backend/DXF_METADATA.md). The cut geometry is written
/// exactly as before any statement existed.
pub fn write_stated(
    entities: &[Entity],
    big: &[String],
    small: &[String],
    comments: &[(String, String)],
) -> String {
    let geometry: Vec<&Entity> = entities.iter().filter(|e| !e.is_annotation()).collect();
    let mut o = String::new();
    let mut p = |code: i32, val: &str| {
        o.push_str(&format!("{code:>3}\n{val}\n"));
    };
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for e in &geometry {
        // extents of the geometry itself: for an arc its end points and the axis points inside its sweep
        // (the box of the full circle was written before: validation defect D5)
        let pts = match e {
            Entity::Arc {
                c,
                r,
                start_deg,
                end_deg,
                ..
            } => arc_box(*c, *r, *start_deg, *end_deg).to_vec(),
            other => other.points(),
        };
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
    let notes = marking_stated(minx, miny, maxx, maxy, big, small);
    // drawing extents include the annotation (text width estimated as one height per character)
    let (mut eminx, mut eminy, mut emaxx, mut emaxy) = if geometry.is_empty() {
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN)
    } else {
        (minx, miny, maxx, maxy)
    };
    for n in &notes {
        if let Entity::Text {
            at, height, text, ..
        } = n
        {
            eminx = eminx.min(at.0);
            eminy = eminy.min(at.1);
            emaxx = emaxx.max(at.0 + height * text.len() as f64);
            emaxy = emaxy.max(at.1 + height);
        }
    }
    p(
        999,
        "NEXUS FABRIC DXF - units: millimetres - NOT RELEASED: development build, not validated, not for manufacturing",
    );
    p(999, &format!("{KEY_UNITS}: mm"));
    for (key, value) in comments {
        p(999, &format!("{key}: {}", one_line(value)));
    }
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
            Entity::Arc {
                layer,
                c,
                r,
                start_deg,
                end_deg,
            } => {
                p(0, "ARC");
                p(8, layer);
                p(10, &num(c.0));
                p(20, &num(c.1));
                p(40, &num(*r));
                p(50, &num(*start_deg));
                p(51, &num(*end_deg));
            }
            Entity::Polyline { layer, pts, closed } => {
                p(0, "POLYLINE");
                p(8, layer);
                p(66, "1");
                p(70, if *closed { "1" } else { "0" });
                for q in pts {
                    p(0, "VERTEX");
                    p(8, layer);
                    p(10, &num(q.0));
                    p(20, &num(q.1));
                }
                p(0, "SEQEND");
            }
            Entity::Text {
                layer,
                at,
                height,
                text,
            } => {
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

/// The group-999 comment lines of a file, in order, trimmed. Human text and application keys alike.
pub fn comment_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(2)
        .filter(|ch| ch.len() == 2 && ch[0].trim() == "999")
        .map(|ch| ch[1].trim().to_string())
        .collect()
}

/// The application keys of the file: every comment line that starts with `NF-`, split at its first ": " into
/// (key, value). A line without ": " has an empty value. Other comment lines are human text and are not listed.
pub fn statement_keys(text: &str) -> Vec<(String, String)> {
    comment_lines(text)
        .into_iter()
        .filter(|l| l.starts_with("NF-"))
        .map(|l| match l.split_once(": ") {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (l, String::new()),
        })
        .collect()
}

/// Application keys this build writes. A file that states any other `NF-` key is not one of its files.
pub const KNOWN_KEYS: [&str; 16] = [
    KEY_UNITS,
    KEY_JOINT,
    KEY_HEADER_SURFACE,
    KEY_BRANCH_SURFACE,
    KEY_VIEW_SIDE,
    KEY_PLATE_AXES,
    KEY_ROLLING_AXIS,
    KEY_SEAM_EDGES,
    KEY_END_EDGES,
    KEY_ANGLE_DATUM,
    KEY_ORIGIN,
    KEY_KERF,
    KEY_STATUS,
    KEY_OFFSET_DELTA,
    KEY_OFFSET_SIGN,
    KEY_BRANCH_ANGLE,
];

/// `NF-` keys that this build does not write.
pub fn unknown_statement_keys(text: &str) -> Vec<String> {
    statement_keys(text)
        .into_iter()
        .map(|(k, _)| k)
        .filter(|k| !KNOWN_KEYS.contains(&k.as_str()))
        .collect()
}

/// What the file states about the unit, compared with millimetres: `NF-UNITS: mm`, stated once. Empty when right.
pub fn units_problems(text: &str) -> Vec<String> {
    let values: Vec<String> = statement_keys(text)
        .into_iter()
        .filter(|(k, _)| k == KEY_UNITS)
        .map(|(_, v)| v)
        .collect();
    match values.as_slice() {
        [v] if v == "mm" => Vec::new(),
        [] => vec![format!("{KEY_UNITS}: mm is missing")],
        _ => vec![format!(
            "{KEY_UNITS} is stated as {} but the unit is mm",
            values.join(", ")
        )],
    }
}

/// Compare what the file states about its joint with what the job declares. `expect` None: the part is not a
/// header-with-branch part and must state no joint at all. Returns one entry per difference; empty when the file
/// states exactly the job's joint: the same three keys (each once, with the same value), and the same two
/// annotation lines (each once); no other joint line may appear.
pub fn joint_statement_problems(text: &str, expect: Option<&JointStatement>) -> Vec<String> {
    let mut out = Vec::new();
    let file_keys = statement_keys(text);
    let want_keys = expect.map(JointStatement::comments).unwrap_or_default();
    for key in [KEY_JOINT, KEY_HEADER_SURFACE, KEY_BRANCH_SURFACE] {
        let have: Vec<&str> = file_keys
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect();
        let want: Option<&str> = want_keys
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str());
        match (want, have.as_slice()) {
            (None, []) => {}
            (None, _) => out.push(format!(
                "{key} is stated as {} but the job declares no joint for this part",
                have.join(", ")
            )),
            (Some(w), []) => out.push(format!("{key}: {w} is missing")),
            (Some(w), [v]) if *v == w => {}
            (Some(w), _) => out.push(format!(
                "{key} is stated as {} but the job declares {w}",
                have.join(", ")
            )),
        }
    }
    let entities = match read(text) {
        Ok((e, _)) => e,
        Err(e) => {
            out.push(format!("the file cannot be read: {e}"));
            return out;
        }
    };
    let texts: Vec<&str> = entities
        .iter()
        .filter_map(|e| match e {
            Entity::Text { layer, text, .. } if layer == MARK_LAYER => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let want_texts = expect.map(JointStatement::texts).unwrap_or_default();
    for want in &want_texts {
        let n = texts.iter().filter(|t| **t == want.as_str()).count();
        if n != 1 {
            out.push(format!(
                "the statement line \"{want}\" is written {n} times; the job declares it once"
            ));
        }
    }
    for t in &texts {
        let joint_line = JOINT_TEXT_PREFIXES.iter().any(|p| t.starts_with(p));
        if joint_line && !want_texts.iter().any(|w| w == t) {
            out.push(format!(
                "the annotation \"{t}\" contradicts the joint the job declares"
            ));
        }
    }
    out
}

/// Compare the plate conventions the file states with the ones the job declares for this part. Empty when they
/// agree exactly: every declared key is stated once with the declared value (no more, no less), no other
/// convention key is stated, and the annotation layer carries exactly the declared annotation lines besides the
/// fixed marking, units, marking-side and joint lines. Missing, altered, repeated or extra statements are reported.
pub fn convention_statement_problems(text: &str, expect: &Conventions) -> Vec<String> {
    let mut out = Vec::new();
    let file_keys = statement_keys(text);
    for (key, want) in &expect.facts {
        let want = one_line(want);
        if want.chars().count() > MAX_COMMENT_CHARS {
            out.push(format!(
                "{key} is longer than {MAX_COMMENT_CHARS} characters"
            ));
        }
        let have: Vec<&str> = file_keys
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect();
        match have.as_slice() {
            [v] if *v == want => {}
            [] => out.push(format!("{key} is missing; the job declares: {want}")),
            _ => out.push(format!(
                "{key} is stated as {} but the job declares {want}",
                have.join(", ")
            )),
        }
    }
    for (key, _) in &file_keys {
        let convention = CONVENTION_KEYS.contains(&key.as_str());
        if convention && !expect.facts.iter().any(|(k, _)| k == key) {
            out.push(format!(
                "{key} is stated, but the job declares no {key} for this part"
            ));
        }
    }
    let entities = match read(text) {
        Ok((e, _)) => e,
        Err(e) => {
            out.push(format!("the file cannot be read: {e}"));
            return out;
        }
    };
    // The marking-side lines are left out of the multiset below, because they are compared with NF-VIEW-SIDE
    // by their own check (a file whose marking side contradicts its viewing side fails).
    let fixed = |t: &str| {
        t == MARK_TEXT
            || t == UNITS_TEXT
            || t.starts_with("MARKING SIDE:")
            || t.starts_with("HEADER SURFACE:")
            || JOINT_TEXT_PREFIXES.iter().any(|p| t.starts_with(p))
    };
    let marking_lines: Vec<String> = entities
        .iter()
        .filter_map(|e| match e {
            Entity::Text { layer, text, .. }
                if layer == MARK_LAYER && text.starts_with("MARKING SIDE:") =>
            {
                Some(text.clone())
            }
            _ => None,
        })
        .collect();
    match expect.facts.iter().find(|(k, _)| k == KEY_VIEW_SIDE) {
        None => out.push(format!(
            "{KEY_VIEW_SIDE} is not declared, so the marking side cannot be checked"
        )),
        Some((_, side)) => {
            let want = marking_side_line(side);
            let n = marking_lines.iter().filter(|t| **t == want).count();
            if n != 1 {
                out.push(format!(
                    "the marking side line \"{want}\" is written {n} times; the job declares it once"
                ));
            }
            for t in marking_lines.iter().filter(|t| **t != want) {
                out.push(format!(
                    "the marking side line \"{t}\" contradicts {KEY_VIEW_SIDE}: {side}"
                ));
            }
        }
    }
    let mut found: Vec<String> = entities
        .iter()
        .filter_map(|e| match e {
            Entity::Text { layer, text, .. } if layer == MARK_LAYER && !fixed(text) => {
                Some(text.clone())
            }
            _ => None,
        })
        .collect();
    let mut want = annotation_lines(&expect.texts());
    found.sort();
    want.sort();
    let count = |xs: &[String], x: &str| xs.iter().filter(|y| y.as_str() == x).count();
    for w in &want {
        let (n, m) = (count(&want, w), count(&found, w));
        if n != m {
            out.push(format!(
                "the statement line \"{w}\" is written {m} times; the job declares it {n} time(s)"
            ));
        }
    }
    for f in &found {
        if !want.contains(f) {
            out.push(format!(
                "the annotation \"{f}\" is not a statement the job declares for this part"
            ));
        }
    }
    out.dedup();
    out
}

/// Independent header reader: every `$VARIABLE` of the HEADER section with the
/// group-code pairs that follow it.
pub fn read_header(
    text: &str,
) -> Result<std::collections::BTreeMap<String, Vec<(i32, String)>>, String> {
    let lines: Vec<&str> = text.lines().collect();
    if !lines.len().is_multiple_of(2) {
        return Err("odd number of lines: group code without value".into());
    }
    let mut vars = std::collections::BTreeMap::new();
    let (mut in_header, mut current): (bool, Option<String>) = (false, None);
    for ch in lines.chunks(2) {
        let code: i32 = ch[0]
            .trim()
            .parse()
            .map_err(|_| format!("bad group code '{}'", ch[0]))?;
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
        let code: i32 = ch[0]
            .trim()
            .parse()
            .map_err(|_| format!("bad group code '{}'", ch[0]))?;
        pairs.push((code, ch[1].trim()));
    }
    let start = pairs
        .iter()
        .position(|&(c, v)| c == 2 && v == "ENTITIES")
        .ok_or("no ENTITIES section")?;
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
                a: (
                    f(get(10).ok_or("LINE without 10")?)?,
                    f(get(20).ok_or("LINE without 20")?)?,
                ),
                b: (
                    f(get(11).ok_or("LINE without 11")?)?,
                    f(get(21).ok_or("LINE without 21")?)?,
                ),
            }),
            "ARC" => ents.push(Entity::Arc {
                layer,
                c: (
                    f(get(10).ok_or("ARC without 10")?)?,
                    f(get(20).ok_or("ARC without 20")?)?,
                ),
                r: f(get(40).ok_or("ARC without 40")?)?,
                start_deg: f(get(50).ok_or("ARC without 50")?)?,
                end_deg: f(get(51).ok_or("ARC without 51")?)?,
            }),
            "POLYLINE" => {
                let flags: i64 = match get(70) {
                    Some(v) => v.parse().map_err(|_| format!("bad POLYLINE flags '{v}'"))?,
                    None => 0,
                };
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
                        pts.push((
                            f(g(10).ok_or("VERTEX without 10")?)?,
                            f(g(20).ok_or("VERTEX without 20")?)?,
                        ));
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
                ents.push(Entity::Polyline {
                    layer,
                    pts,
                    closed: flags & 1 == 1,
                });
                j = k;
            }
            "TEXT" => ents.push(Entity::Text {
                layer,
                at: (
                    f(get(10).ok_or("TEXT without 10")?)?,
                    f(get(20).ok_or("TEXT without 20")?)?,
                ),
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
    /// $EXTMIN / $EXTMAX of the header agree with the extents of the entities
    /// read back, computed by a different method (dense sampling of arcs).
    pub extents_match: bool,
    /// Every annotation line the caller required is present on the annotation layer.
    pub required_notes_present: bool,
    /// The file states its joint exactly as the job declares it (`joint_statement_problems`), and no other
    /// application key (`unknown_statement_keys`).
    pub joint_statement_matches: bool,
    pub pass: bool,
}

/// Compare intended geometry with what a separate parse of `text` yields, and
/// confirm the release marking and unit declarations of the file.
///
/// What this proves and what it does not (review RB-006): it proves that the
/// writer and the reader are faithful to each other, i.e. that the file holds
/// the entities the writer was given. It says NOTHING about whether those
/// entities are the right part: the comparison target is the product's own
/// intended geometry (this is why Defect 002 passed it). Whether the geometry
/// in the file is right is checked elsewhere, by verifying the geometry READ
/// BACK from the file against values recomputed from the inputs
/// (`nf_core::job`), and by `quality::quality_gate`, which looks only at the file.
pub fn reimport_check(intended: &[Entity], text: &str, tol: f64) -> ReimportReport {
    reimport_check_annotated(intended, text, tol, &[])
}

/// As `reimport_check`, and every line of `required_notes` must be present as
/// annotation on the annotation layer.
pub fn reimport_check_annotated(
    intended: &[Entity],
    text: &str,
    tol: f64,
    required_notes: &[String],
) -> ReimportReport {
    reimport_check_stated(intended, text, tol, required_notes, None)
}

/// As `reimport_check_annotated`, and the file must state the joint of `joint` exactly (None: no joint at all).
pub fn reimport_check_stated(
    intended: &[Entity],
    text: &str,
    tol: f64,
    required_notes: &[String],
    joint: Option<&JointStatement>,
) -> ReimportReport {
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
        extents_match: false,
        required_notes_present: false,
        joint_statement_matches: false,
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
    let geometry_box = sampled_box(&got);
    let lowest = geometry_box.1 - geometry_box.4;
    for (a, b) in intended.iter().zip(&got) {
        if a.layer() != b.layer() || std::mem::discriminant(*a) != std::mem::discriminant(b) {
            layers_match = false;
        }
        if let (Entity::Polyline { closed: ca, .. }, Entity::Polyline { closed: cb, .. }) = (a, b)
            && ca != cb
        {
            layers_match = false; // an open profile is not the closed profile that was intended
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
    // Marking: exactly on the annotation layer, nothing else there, no text on a cut layer, below the geometry.
    let text_of = |e: &Entity| match e {
        Entity::Text { text, .. } => text.clone(),
        _ => String::new(),
    };
    let on_mark = |want: &str| {
        notes
            .iter()
            .any(|n| n.layer() == MARK_LAYER && text_of(n) == want)
    };
    let notes_clear = notes.iter().all(|n| n.layer() == MARK_LAYER && !is_cut_layer(n.layer()) && matches!(n, Entity::Text { at, height, .. } if got.is_empty() || at.1 + height <= lowest));
    let geometry_clear = got.iter().all(|g| g.layer() != MARK_LAYER);
    let not_released_marking = on_mark(MARK_TEXT) && notes_clear && geometry_clear;
    let header = read_header(text).unwrap_or_default();
    let var = |name: &str, code: i32, want: &str| {
        header
            .get(name)
            .is_some_and(|v| v.iter().any(|(c, x)| *c == code && x == want))
    };
    let units_declared = var("$ACADVER", 1, "AC1009")
        && var("$LUNITS", 70, "2")
        && var("$AUNITS", 70, "0")
        && var("$ANGDIR", 70, "0")
        && var("$ANGBASE", 50, "0.0")
        && on_mark(UNITS_TEXT)
        && units_problems(text).is_empty();
    let required_notes_present = annotation_lines(required_notes)
        .iter()
        .all(|want| on_mark(want));
    let joint_statement_matches =
        joint_statement_problems(text, joint).is_empty() && unknown_statement_keys(text).is_empty();
    // D5: the header extents against the box of everything read back (geometry and annotation).
    let everything: Vec<Entity> = got.iter().cloned().chain(notes.iter().cloned()).collect();
    let (bx0, by0, bx1, by1, sample_err) = sampled_box(&everything);
    let header_xy = |name: &str| -> Option<(f64, f64)> {
        let v = header.get(name)?;
        let get = |code: i32| {
            v.iter()
                .find(|(c, _)| *c == code)
                .and_then(|(_, x)| x.parse::<f64>().ok())
        };
        Some((get(10)?, get(20)?))
    };
    let extents_match = match (header_xy("$EXTMIN"), header_xy("$EXTMAX")) {
        (Some(lo), Some(hi)) => {
            let scale = (bx1 - bx0).abs().max((by1 - by0).abs()).max(1.0);
            let t = tol.max(sample_err) + 1e-9 * scale;
            (lo.0 - bx0).abs() <= t
                && (lo.1 - by0).abs() <= t
                && (hi.0 - bx1).abs() <= t
                && (hi.1 - by1).abs() <= t
        }
        _ => false,
    };
    let pass = unknown.is_empty()
        && layers_match
        && dp <= tol
        && dl <= tol
        && not_released_marking
        && units_declared
        && extents_match
        && required_notes_present
        && joint_statement_matches;
    ReimportReport {
        entities_written: intended.len(),
        entities_read: got.len(),
        annotations_read: notes.len(),
        unknown_entities: unknown,
        max_point_deviation: dp,
        max_length_deviation: dl,
        layers_match,
        not_released_marking,
        units_declared,
        extents_match,
        required_notes_present,
        joint_statement_matches,
        pass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Entity> {
        vec![
            Entity::Line {
                layer: "CUT_OUTER".into(),
                a: (0.0, 0.0),
                b: (628.318530718, 0.0),
            },
            Entity::Arc {
                layer: "CUT_OUTER".into(),
                c: (0.0, 0.0),
                r: 632.4555320337,
                start_deg: 0.0,
                end_deg: 113.8419957661,
            },
            Entity::Polyline {
                layer: "CUT_OUTER".into(),
                pts: vec![(1.0, 2.0), (3.5, -4.25), (1e3, 1e-3)],
                closed: false,
            },
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
        let e = vec![
            Entity::Arc {
                layer: "CUT_OUTER".into(),
                c: (0.0, 0.0),
                r: 987654.3210987654,
                start_deg: 0.0,
                end_deg: 12.345678901234567,
            },
            Entity::Line {
                layer: "CUT_OUTER".into(),
                a: (1e-9, -0.0),
                b: (123456789.12345679, 1.0 / 3.0),
            },
        ];
        let r = reimport_check(&e, &write(&e), 0.0);
        assert!(
            r.pass && r.max_point_deviation == 0.0 && r.max_length_deviation == 0.0,
            "{r:?}"
        );
    }

    #[test]
    fn detects_corruption_truncation_and_foreign_entities() {
        let e = sample();
        let t = write(&e);
        assert!(!reimport_check(&e, &t.replace("632.4555320337", "632.4555321337"), 1e-8).pass);
        assert!(!reimport_check(&e, &t[..t.len() - 12], 1e-8).pass);
        assert!(
            !reimport_check(
                &e,
                &t.replace("  0\nEOF", "  0\nCIRCLE\n  8\nX\n  0\nEOF"),
                1e-8
            )
            .pass
        );
        assert!(
            !reimport_check(
                &e,
                &t.replacen("CUT_OUTER\n 10\n0.0", "NO_CUT\n 10\n0.0", 1),
                1e-8
            )
            .pass
        );
        assert!(read("garbage").is_err());
    }

    /// The marking is annotation on its own layer; the cut entities are written
    /// exactly as before it existed (same group codes, same exact numbers).
    #[test]
    fn not_released_marking_and_units_without_touching_cut_geometry() {
        let e = vec![
            Entity::Line {
                layer: "CUT_OUTER".into(),
                a: (0.0, 0.0),
                b: (1.0 / 3.0, 250.0),
            },
            Entity::Polyline {
                layer: "CUT_INNER".into(),
                pts: vec![(1.0, 2.0), (3.5, 4.25), (1.0, 2.0)],
                closed: false,
            },
        ];
        let t = write(&e);
        let cut = "  0\nLINE\n  8\nCUT_OUTER\n 10\n0.0\n 20\n0.0\n 11\n0.3333333333333333\n 21\n250.0\n  0\nPOLYLINE\n  8\nCUT_INNER\n 66\n1\n 70\n0\n  0\nVERTEX\n  8\nCUT_INNER\n 10\n1.0\n 20\n2.0\n  0\nVERTEX\n  8\nCUT_INNER\n 10\n3.5\n 20\n4.25\n  0\nVERTEX\n  8\nCUT_INNER\n 10\n1.0\n 20\n2.0\n  0\nSEQEND\n";
        assert!(
            t.contains(&format!("  2\nENTITIES\n{cut}  0\nTEXT\n")),
            "cut geometry must be written unchanged and before any annotation"
        );
        let (all, unknown) = read(&t).unwrap();
        assert!(unknown.is_empty());
        let notes: Vec<&Entity> = all.iter().filter(|x| x.is_annotation()).collect();
        assert_eq!(notes.len(), 2);
        assert!(
            notes
                .iter()
                .all(|n| n.layer() == MARK_LAYER && !is_cut_layer(n.layer()))
        );
        assert!(
            matches!(notes[0], Entity::Text { text, at, height, .. } if text == MARK_TEXT && at.1 + height < 0.0),
            "marking lies below the geometry"
        );
        assert!(
            all.iter()
                .filter(|x| !x.is_annotation())
                .all(|g| is_cut_layer(g.layer()))
        );
        let h = read_header(&t).unwrap();
        assert_eq!(h["$LUNITS"], vec![(70, "2".to_string())]);
        assert_eq!(h["$AUNITS"], vec![(70, "0".to_string())]);
        assert_eq!(h["$ANGDIR"], vec![(70, "0".to_string())]);
        assert!(!h.contains_key("$INSUNITS"), "not an R12 variable");
        assert!(t.contains("units: millimetres") && t.contains(UNITS_TEXT));
        let r = reimport_check(&e, &t, 0.0);
        assert!(
            r.pass
                && r.not_released_marking
                && r.units_declared
                && r.annotations_read == 2
                && r.entities_read == 2
                && r.max_point_deviation == 0.0,
            "{r:?}"
        );
        // an empty drawing still carries the marking
        assert!(reimport_check(&[], &write(&[]), 0.0).pass);
    }

    #[test]
    fn missing_moved_or_altered_marking_and_units_fail_the_check() {
        let e = sample();
        let t = write(&e);
        let no_mark = t.replace(MARK_TEXT, "RELEASED");
        assert!(
            !reimport_check(&e, &no_mark, 1e-8).not_released_marking
                && !reimport_check(&e, &no_mark, 1e-8).pass
        );
        let on_cut = t.replace(&format!("TEXT\n  8\n{MARK_LAYER}"), "TEXT\n  8\nCUT_OUTER");
        assert!(
            !reimport_check(&e, &on_cut, 1e-8).pass,
            "annotation on a cutting layer"
        );
        let geometry_on_mark = t.replacen(
            "LINE\n  8\nCUT_OUTER",
            &format!("LINE\n  8\n{MARK_LAYER}"),
            1,
        );
        assert!(
            !reimport_check(&e, &geometry_on_mark, 1e-8).pass,
            "geometry on the annotation layer"
        );
        let inches = t.replace("$LUNITS\n 70\n2", "$LUNITS\n 70\n4");
        assert!(
            !reimport_check(&e, &inches, 1e-8).units_declared
                && !reimport_check(&e, &inches, 1e-8).pass
        );
        let clockwise = t.replace("$ANGDIR\n 70\n0", "$ANGDIR\n 70\n1");
        assert!(!reimport_check(&e, &clockwise, 1e-8).pass);
        let no_units = t.replace(UNITS_TEXT, "UNITS: INCHES");
        assert!(!reimport_check(&e, &no_units, 1e-8).pass);
        let stripped: String = {
            let i = t.find("  0\nTEXT").unwrap();
            let j = t.rfind("  0\nENDSEC").unwrap();
            format!("{}{}", &t[..i], &t[j..])
        };
        assert!(
            read(&stripped).is_ok() && !reimport_check(&e, &stripped, 1e-8).pass,
            "marking removed"
        );
    }

    /// Validation defect D5: $EXTMIN / $EXTMAX are the extents of the sector, not of its full circles.
    #[test]
    fn d5_header_extents_are_those_of_the_geometry_and_are_checked() {
        // CONE-01: apex radii 632.4555 and 316.2278, sector 113.84 degrees
        let (l1, l2, phi) = (
            632.455532033676f64,
            316.227766016838f64,
            113.84199576606166f64,
        );
        let at = |r: f64, d: f64| (r * d.to_radians().cos(), r * d.to_radians().sin());
        let e = vec![
            Entity::Arc {
                layer: "CUT_OUTER".into(),
                c: (0.0, 0.0),
                r: l1,
                start_deg: 0.0,
                end_deg: phi,
            },
            Entity::Line {
                layer: "CUT_OUTER".into(),
                a: at(l1, phi),
                b: at(l2, phi),
            },
            Entity::Arc {
                layer: "CUT_OUTER".into(),
                c: (0.0, 0.0),
                r: l2,
                start_deg: 0.0,
                end_deg: phi,
            },
            Entity::Line {
                layer: "CUT_OUTER".into(),
                a: at(l2, 0.0),
                b: at(l1, 0.0),
            },
        ];
        let t = write(&e);
        let h = read_header(&t).unwrap();
        let xy = |name: &str| -> (f64, f64) {
            (h[name][0].1.parse().unwrap(), h[name][1].1.parse().unwrap())
        };
        let (lo, hi) = (xy("$EXTMIN"), xy("$EXTMAX"));
        // the geometry spans x = -255.5 .. 632.5 and y = 0 .. 632.5; the annotation lies below and may be wider
        assert!(
            (lo.0 - at(l1, phi).0).abs() < 1e-9 && (lo.0 + 255.6).abs() < 0.2,
            "{lo:?}"
        );
        assert!(
            (hi.1 - l1).abs() < 1e-9,
            "top of the large arc at 90 degrees: {hi:?}"
        );
        assert!(
            lo.1 < 0.0 && lo.1 > -0.2 * l1,
            "annotation just below y = 0, not the full circle: {lo:?}"
        );
        assert!(hi.0 >= l1 && lo.0 > -l1 + 300.0);
        let r = reimport_check(&e, &t, 1e-8);
        assert!(r.pass && r.extents_match, "{r:?}");
        // wrong extents (the old full-circle box, or anything else) fail the round trip
        let wrong = t.replacen(
            &format!("$EXTMIN\n 10\n{}", num(lo.0)),
            &format!("$EXTMIN\n 10\n{}", num(-l1)),
            1,
        );
        assert_ne!(wrong, t);
        let r = reimport_check(&e, &wrong, 1e-8);
        assert!(!r.extents_match && !r.pass, "{r:?}");
        // arcs through 180 and 270 degrees, and a sweep across 0
        for (s0, s1, want) in [
            (100.0, 200.0, (-10.0, -3.4202, -1.7365, 9.8481)),
            (250.0, 20.0, (-3.4202, -10.0, 10.0, 3.4202)),
            (10.0, 80.0, (1.7365, 1.7365, 9.8481, 9.8481)),
        ] {
            let b = arc_box((0.0, 0.0), 10.0, s0, s1);
            let got = b
                .iter()
                .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, p| {
                    (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1))
                });
            for (g, w) in [
                (got.0, want.0),
                (got.1, want.1),
                (got.2, want.2),
                (got.3, want.3),
            ] {
                assert!((g - w).abs() < 1e-3, "{s0}..{s1}: {got:?} against {want:?}");
            }
            let one = [Entity::Arc {
                layer: "CUT_OUTER".into(),
                c: (0.0, 0.0),
                r: 10.0,
                start_deg: s0,
                end_deg: s1,
            }];
            assert!(reimport_check(&one, &write(&one), 1e-8).pass);
        }
    }

    #[test]
    fn extra_annotation_lines_are_written_on_the_annotation_layer_and_required_by_the_check() {
        let e = sample();
        let notes = vec!["MARKING SIDE: OUTSIDE (PROVISIONAL)".to_string()];
        let t = write_annotated(&e, &notes);
        let (all, _) = read(&t).unwrap();
        assert!(all.iter().any(|x| matches!(x, Entity::Text { layer, text, .. } if layer == MARK_LAYER && text == &notes[0])));
        assert!(reimport_check_annotated(&e, &t, 1e-8, &notes).pass);
        assert!(
            !reimport_check_annotated(&e, &write(&e), 1e-8, &notes).pass,
            "a required line that is missing fails the check"
        );
        assert!(
            !reimport_check_annotated(
                &e,
                &t.replace("OUTSIDE (PROVISIONAL)", "INSIDE (PROVISIONAL)"),
                1e-8,
                &notes
            )
            .required_notes_present
        );
        // the cut geometry is byte-identical with and without the extra line
        let cut =
            |t: &str| t[t.find("ENTITIES").unwrap()..t.find("  0\nTEXT").unwrap()].to_string();
        assert_eq!(cut(&t), cut(&write(&e)));
    }
}
