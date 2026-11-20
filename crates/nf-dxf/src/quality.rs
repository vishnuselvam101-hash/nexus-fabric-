//! DXF quality gate: an error-detection pass over a file AS WRITTEN AND
//! RE-READ. It is given nothing but the text of the file. It does not know the
//! part, the request or the entities the writer intended; it asks whether the
//! file is a well-formed cutting file of the kind this product writes:
//!
//! | rule | what fails it |
//! |---|---|
//! | `readable` | the file does not parse, or holds an entity type this build does not write |
//! | `units_declared` | header does not declare AC1009, decimal units, degrees counter-clockwise from +x; no millimetre statement |
//! | `finite_numbers` | a coordinate, radius or angle that is not a finite number; a radius that is not positive |
//! | `wrong_layer` | cut geometry off the CUT_OUTER / CUT_INNER layers, text off the annotation layer, an inner profile on the outer layer |
//! | `zero_length` | a line, polyline segment or arc of zero length; a polyline with fewer than two vertices |
//! | `duplicate_or_overlapping` | the same entity twice, collinear segments that overlap, arcs of one circle that overlap |
//! | `open_contour` | an end point not joined to exactly one other end point; an inner profile not flagged closed; not exactly one outer profile |
//! | `self_intersection` | a profile that crosses itself |
//! | `inner_inside_outer` | an inner profile that crosses the outer one, lies outside it, or overlaps another inner profile |
//! | `orientation` | outer profile not counter-clockwise, inner profile not clockwise (Spec 02 §0.3) |
//! | `extents` | `$EXTMIN` / `$EXTMAX` differ from the extents of the entities in the file |
//! | `release_marking` | the NOT RELEASED marking is missing, or annotation lies on or above the geometry |
//!
//! Any finding makes the file INVALID. LIMITATION: binary64 arithmetic with
//! stated tolerances, not exact predicates; contacts closer than the tolerance
//! are not decided rigorously. CAM-system acceptance has not been tested.

use crate::{Entity, MARK_LAYER, MARK_TEXT, UNITS_TEXT, arc_box, read, read_header};

pub const OUTER_LAYER: &str = "CUT_OUTER";
pub const INNER_LAYER: &str = "CUT_INNER";

pub const RULES: [&str; 12] = [
    "readable",
    "units_declared",
    "finite_numbers",
    "wrong_layer",
    "zero_length",
    "duplicate_or_overlapping",
    "open_contour",
    "self_intersection",
    "inner_inside_outer",
    "orientation",
    "extents",
    "release_marking",
];

/// Chords used to flatten one arc for the crossing tests.
const ARC_CHORDS: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct QualityFinding {
    pub rule: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QualityReport {
    /// Cut entities read from the file.
    pub entities: usize,
    pub annotations: usize,
    pub outer_contours: usize,
    pub inner_contours: usize,
    /// Straight segments the crossing and overlap tests ran on (arcs flattened).
    pub segments_tested: usize,
    /// Tolerance used to decide that two points are the same point (mm).
    pub join_tolerance: f64,
    pub findings: Vec<QualityFinding>,
    pub pass: bool,
}

impl QualityReport {
    pub fn failed_rules(&self) -> Vec<&'static str> {
        let mut r: Vec<&'static str> = self.findings.iter().map(|f| f.rule).collect();
        r.sort();
        r.dedup();
        r
    }
}

type Pt = (f64, f64);

fn d(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn orient(a: Pt, b: Pt, c: Pt) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn proper_cross(a: Pt, b: Pt, c: Pt, e: Pt) -> bool {
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, e),
        orient(c, e, a),
        orient(c, e, b),
    );
    o1 != 0.0
        && o2 != 0.0
        && o3 != 0.0
        && o4 != 0.0
        && (o1 > 0.0) != (o2 > 0.0)
        && (o3 > 0.0) != (o4 > 0.0)
}

fn sweep_of(start_deg: f64, end_deg: f64) -> f64 {
    let s = (end_deg - start_deg) % 360.0;
    if s < 0.0 { s + 360.0 } else { s }
}

fn arc_point(c: Pt, r: f64, deg: f64) -> Pt {
    (
        c.0 + r * deg.to_radians().cos(),
        c.1 + r * deg.to_radians().sin(),
    )
}

/// The points an entity is drawn through, in its own direction (arcs flattened).
fn flatten(e: &Entity) -> Vec<Pt> {
    match e {
        Entity::Line { a, b, .. } => vec![*a, *b],
        Entity::Arc {
            c,
            r,
            start_deg,
            end_deg,
            ..
        } => {
            let sweep = sweep_of(*start_deg, *end_deg);
            (0..=ARC_CHORDS)
                .map(|i| arc_point(*c, *r, start_deg + sweep * i as f64 / ARC_CHORDS as f64))
                .collect()
        }
        Entity::Polyline { pts, closed, .. } => {
            let mut v = pts.clone();
            if *closed && let Some(first) = pts.first() {
                v.push(*first);
            }
            v
        }
        Entity::Text { .. } => vec![],
    }
}

fn is_closed_entity(e: &Entity) -> bool {
    matches!(e, Entity::Polyline { closed: true, .. })
}

/// One straight piece of the flattened drawing.
#[derive(Clone, Copy)]
struct Seg {
    a: Pt,
    b: Pt,
    /// entity index and position inside the entity
    ent: usize,
    k: usize,
    /// number of pieces of its entity, and whether the entity is a closed loop
    of: usize,
    looped: bool,
    from_arc: bool,
}

/// Calls `f(i, j)` once for every pair of segments whose bounding boxes share a
/// cell of a uniform grid (about one segment per cell); segments whose box
/// covers very many cells are paired with every other segment.
fn candidate_pairs(segs: &[Seg], mut f: impl FnMut(usize, usize)) {
    let m = segs.len();
    if m < 2 {
        return;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in segs {
        for p in [s.a, s.b] {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
    }
    let (w, h) = (x1 - x0, y1 - y0);
    if m < 64 {
        for i in 0..m {
            for j in i + 1..m {
                f(i, j);
            }
        }
        return;
    }
    let aspect = if w > 0.0 && h > 0.0 { w / h } else { 1.0 };
    let nx = (((m as f64) * aspect).sqrt().ceil() as usize).clamp(1, 4096);
    let ny = (((m as f64) / aspect).sqrt().ceil() as usize).clamp(1, 4096);
    let cx = |x: f64| -> usize {
        if w > 0.0 {
            (((x - x0) / w * nx as f64) as usize).min(nx - 1)
        } else {
            0
        }
    };
    let cy = |y: f64| -> usize {
        if h > 0.0 {
            (((y - y0) / h * ny as f64) as usize).min(ny - 1)
        } else {
            0
        }
    };
    let limit = 4 * (nx + ny);
    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); nx * ny];
    let mut boxes = Vec::with_capacity(m);
    let mut long: Vec<usize> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        let b = (
            cx(s.a.0.min(s.b.0)),
            cy(s.a.1.min(s.b.1)),
            cx(s.a.0.max(s.b.0)),
            cy(s.a.1.max(s.b.1)),
        );
        boxes.push(b);
        if (b.2 - b.0 + 1) * (b.3 - b.1 + 1) > limit {
            long.push(i);
        } else {
            for gx in b.0..=b.2 {
                for gy in b.1..=b.3 {
                    grid[gy * nx + gx].push(i as u32);
                }
            }
        }
    }
    for (n, &i) in long.iter().enumerate() {
        for j in 0..m {
            // two long segments meet once, in the turn of the earlier one
            if j == i || long[..n].contains(&j) {
                continue;
            }
            f(i.min(j), i.max(j));
        }
    }
    for (cell, members) in grid.iter().enumerate() {
        let (gx, gy) = (cell % nx, cell / nx);
        for (p, &i) in members.iter().enumerate() {
            for &j in &members[p + 1..] {
                let (i, j) = (i as usize, j as usize);
                let (bi, bj) = (boxes[i], boxes[j]);
                // examined once: in the lowest-left cell the two boxes share
                if bi.0.max(bj.0) == gx && bi.1.max(bj.1) == gy {
                    f(i.min(j), i.max(j));
                }
            }
        }
    }
}

/// True when the two pieces belong to different open entities and meet at an end point of both: the
/// place where the chain joins them, which is not a crossing.
fn joint_at(s: &Seg, t: &Seg, tol: f64) -> bool {
    if s.ent == t.ent || s.looped || t.looped {
        return false;
    }
    let ends = |g: &Seg| -> Vec<Pt> {
        let mut v = Vec::with_capacity(2);
        if g.k == 0 {
            v.push(g.a);
        }
        if g.k + 1 == g.of {
            v.push(g.b);
        }
        v
    };
    let (es, et) = (ends(s), ends(t));
    es.iter().any(|p| et.iter().any(|q| d(*p, *q) <= tol))
}

/// Length over which two segments run on top of each other (0 if they are not collinear within `tol`).
fn overlap_length(s: &Seg, t: &Seg, tol: f64) -> f64 {
    let len = d(s.a, s.b);
    if len <= 0.0 {
        return 0.0;
    }
    let u = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
    let off = |p: Pt| ((p.0 - s.a.0) * u.1 - (p.1 - s.a.1) * u.0).abs();
    if off(t.a) > tol || off(t.b) > tol {
        return 0.0;
    }
    let along = |p: Pt| (p.0 - s.a.0) * u.0 + (p.1 - s.a.1) * u.1;
    let (t0, t1) = (along(t.a).min(along(t.b)), along(t.a).max(along(t.b)));
    (t1.min(len) - t0.max(0.0)).max(0.0)
}

fn inside(poly: &[Pt], p: Pt) -> bool {
    let mut odd = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

fn signed_area(poly: &[Pt]) -> f64 {
    let n = poly.len();
    0.5 * (0..n)
        .map(|i| poly[i].0 * poly[(i + 1) % n].1 - poly[(i + 1) % n].0 * poly[i].1)
        .sum::<f64>()
}

/// A closed profile found in the file: its points in traversal order (closing point not repeated),
/// its layer, and whether the traversal follows the direction in which the file wrote its lines.
struct Contour {
    layer: String,
    pts: Vec<Pt>,
    /// `Some(true)`: traversed as written; `Some(false)`: against it; `None`: only arcs, no direction in the file.
    as_written: Option<bool>,
}

/// Run the gate on the text of a DXF file.
pub fn quality_gate(text: &str) -> QualityReport {
    let mut findings: Vec<QualityFinding> = Vec::new();
    let mut add = |rule: &'static str, detail: String| {
        // a damaged file may fail one rule thousands of times: keep the first few of each
        if findings.iter().filter(|f| f.rule == rule).count() < 8 {
            findings.push(QualityFinding { rule, detail });
        }
    };
    let empty = |findings: Vec<QualityFinding>| QualityReport {
        entities: 0,
        annotations: 0,
        outer_contours: 0,
        inner_contours: 0,
        segments_tested: 0,
        join_tolerance: 0.0,
        findings,
        pass: false,
    };
    // ---- readable
    let (all, unknown) = match read(text) {
        Ok(x) => x,
        Err(e) => {
            add("readable", format!("the file cannot be read: {e}"));
            return empty(findings);
        }
    };
    for u in &unknown {
        add(
            "readable",
            format!("entity type {u} is not one this build writes"),
        );
    }
    let (notes, geometry): (Vec<Entity>, Vec<Entity>) =
        all.into_iter().partition(|e| e.is_annotation());

    // ---- units_declared
    let header = read_header(text).unwrap_or_default();
    let var = |name: &str, code: i32, want: &str| {
        header
            .get(name)
            .is_some_and(|v| v.iter().any(|(c, x)| *c == code && x == want))
    };
    for (name, code, want, what) in [
        ("$ACADVER", 1, "AC1009", "format AC1009"),
        ("$LUNITS", 70, "2", "decimal linear units"),
        ("$AUNITS", 70, "0", "angles in decimal degrees"),
        ("$ANGDIR", 70, "0", "angles counter-clockwise"),
        ("$ANGBASE", 50, "0.0", "angles measured from +x"),
    ] {
        if !var(name, code, want) {
            add(
                "units_declared",
                format!("the header does not declare {what} ({name} = {want})"),
            );
        }
    }
    let on_mark = |want: &str| {
        notes.iter().any(
            |n| matches!(n, Entity::Text { layer, text, .. } if layer == MARK_LAYER && text == want),
        )
    };
    if !on_mark(UNITS_TEXT) {
        add(
            "units_declared",
            format!("the drawing does not state its unit (\"{UNITS_TEXT}\")"),
        );
    }

    // ---- finite_numbers
    let mut numbers_ok = true;
    for (i, e) in geometry.iter().chain(notes.iter()).enumerate() {
        let (vals, radius): (Vec<f64>, Option<f64>) = match e {
            Entity::Line { a, b, .. } => (vec![a.0, a.1, b.0, b.1], None),
            Entity::Arc {
                c,
                r,
                start_deg,
                end_deg,
                ..
            } => (vec![c.0, c.1, *r, *start_deg, *end_deg], Some(*r)),
            Entity::Polyline { pts, .. } => (pts.iter().flat_map(|p| [p.0, p.1]).collect(), None),
            Entity::Text { at, height, .. } => (vec![at.0, at.1, *height], Some(*height)),
        };
        if vals.iter().any(|v| !v.is_finite()) {
            numbers_ok = false;
            add(
                "finite_numbers",
                format!("entity {} holds a number that is not finite", i + 1),
            );
        }
        if radius.is_some_and(|r| r.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)) {
            numbers_ok = false;
            add(
                "finite_numbers",
                format!(
                    "entity {} has a radius or height that is not positive",
                    i + 1
                ),
            );
        }
    }
    if !numbers_ok {
        let mut r = empty(findings);
        r.entities = geometry.len();
        r.annotations = notes.len();
        return r;
    }

    // ---- wrong_layer
    for (i, g) in geometry.iter().enumerate() {
        if g.layer() != OUTER_LAYER && g.layer() != INNER_LAYER {
            add(
                "wrong_layer",
                format!(
                    "cut entity {} is on layer '{}', not on {OUTER_LAYER} or {INNER_LAYER}",
                    i + 1,
                    g.layer()
                ),
            );
        }
    }
    for n in &notes {
        if n.layer() != MARK_LAYER {
            add(
                "wrong_layer",
                format!(
                    "annotation text is on layer '{}', not on {MARK_LAYER}",
                    n.layer()
                ),
            );
        }
    }

    let scale = geometry
        .iter()
        .flat_map(|e| e.points())
        .fold(0.0f64, |m, p| m.max(p.0.abs()).max(p.1.abs()));
    // two points are the same point within this distance: arc end points are computed from degrees,
    // which costs a few units in the last place of the largest coordinate
    let tol = 1e-9f64.max(64.0 * f64::EPSILON * scale);

    // ---- zero_length
    for (i, e) in geometry.iter().enumerate() {
        match e {
            Entity::Line { a, b, .. } if d(*a, *b) <= tol => {
                add(
                    "zero_length",
                    format!("line {} has zero length at ({}, {})", i + 1, a.0, a.1),
                );
            }
            Entity::Arc {
                r,
                start_deg,
                end_deg,
                ..
            } if r * sweep_of(*start_deg, *end_deg).to_radians() <= tol => {
                add("zero_length", format!("arc {} has zero length", i + 1));
            }
            Entity::Polyline { pts, closed, .. } => {
                if pts.len() < 2 {
                    add(
                        "zero_length",
                        format!("polyline {} has fewer than two vertices", i + 1),
                    );
                }
                let ring = flatten(e);
                if let Some(k) = ring.windows(2).position(|w| d(w[0], w[1]) <= tol) {
                    add(
                        "zero_length",
                        format!(
                            "polyline {} has a segment of zero length at vertex {} ({}, {}){}",
                            i + 1,
                            k + 1,
                            ring[k].0,
                            ring[k].1,
                            if *closed && k + 2 == ring.len() {
                                ": the closing vertex repeats the first although the polyline is flagged closed"
                            } else {
                                ""
                            }
                        ),
                    );
                }
            }
            _ => {}
        }
    }

    // ---- flatten every cut entity
    let mut segs: Vec<Seg> = Vec::new();
    for (i, e) in geometry.iter().enumerate() {
        let pts = flatten(e);
        let of = pts.len().saturating_sub(1);
        for (k, w) in pts.windows(2).enumerate() {
            segs.push(Seg {
                a: w[0],
                b: w[1],
                ent: i,
                k,
                of,
                looped: is_closed_entity(e),
                from_arc: matches!(e, Entity::Arc { .. }),
            });
        }
    }

    // ---- duplicate_or_overlapping (lines and polyline segments), and crossings
    let mut crossings: Vec<(usize, usize)> = Vec::new(); // (entity, entity)
    {
        let mut overlaps: Vec<(usize, usize, f64)> = Vec::new();
        candidate_pairs(&segs, |i, j| {
            let (s, t) = (&segs[i], &segs[j]);
            if !s.from_arc && !t.from_arc {
                let o = overlap_length(s, t, tol);
                if o > 16.0 * tol {
                    overlaps.push((s.ent, t.ent, o));
                    return;
                }
            }
            // Pieces that are consecutive along the profile (neighbours along one entity, or the two ends
            // where the chain joins two entities) may touch at their common point and nothing else. Any
            // other contact, even a single shared point, is a crossing: a profile that touches itself is
            // not a simple profile (a rose curve passes through its centre four times).
            let neighbours = s.ent == t.ent
                && (s.k.abs_diff(t.k) <= 1 || (s.looped && s.k.abs_diff(t.k) == s.of - 1));
            if neighbours || joint_at(s, t, tol) {
                return;
            }
            let touching = [s.a, s.b]
                .iter()
                .any(|p| d(*p, t.a) <= tol || d(*p, t.b) <= tol);
            if touching || proper_cross(s.a, s.b, t.a, t.b) {
                crossings.push((s.ent, t.ent));
            }
        });
        for (a, b, o) in overlaps {
            add(
                "duplicate_or_overlapping",
                if a == b {
                    format!(
                        "entity {} runs back over itself for {o} mm (a segment drawn twice)",
                        a + 1
                    )
                } else {
                    format!("entities {} and {} overlap for {o} mm", a + 1, b + 1)
                },
            );
        }
    }
    // arcs of one circle whose sweeps overlap
    let arcs: Vec<(usize, Pt, f64, f64, f64)> = geometry
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Entity::Arc {
                c,
                r,
                start_deg,
                end_deg,
                ..
            } => Some((i, *c, *r, *start_deg, sweep_of(*start_deg, *end_deg))),
            _ => None,
        })
        .collect();
    for (n, a) in arcs.iter().enumerate() {
        for b in &arcs[n + 1..] {
            if d(a.1, b.1) > tol || (a.2 - b.2).abs() > tol {
                continue;
            }
            // start of b relative to a, and the reverse: an overlap of positive length in either
            let rel = |from: f64, to: f64| (to - from).rem_euclid(360.0);
            let over = (a.4 - rel(a.3, b.3))
                .min(b.4)
                .max((b.4 - rel(b.3, a.3)).min(a.4));
            if over.to_radians() * a.2 > 16.0 * tol {
                add(
                    "duplicate_or_overlapping",
                    format!(
                        "arcs {} and {} lie on the same circle and overlap for {over} degrees",
                        a.0 + 1,
                        b.0 + 1
                    ),
                );
            }
        }
    }

    // ---- open_contour: join end points, layer by layer
    let mut contours: Vec<Contour> = Vec::new();
    let mut open_found = false;
    for layer in [OUTER_LAYER, INNER_LAYER] {
        let on_layer: Vec<usize> = (0..geometry.len())
            .filter(|i| geometry[*i].layer() == layer)
            .collect();
        // closed polylines are profiles of their own
        for &i in &on_layer {
            if is_closed_entity(&geometry[i]) {
                let mut pts = flatten(&geometry[i]);
                pts.pop();
                contours.push(Contour {
                    layer: layer.into(),
                    pts,
                    as_written: Some(true),
                });
            } else if layer == INNER_LAYER {
                open_found = true;
                add(
                    "open_contour",
                    format!(
                        "inner profile entity {} is not flagged as a closed polyline",
                        i + 1
                    ),
                );
            }
        }
        // the rest are chained by their end points
        let open: Vec<usize> = on_layer
            .iter()
            .copied()
            .filter(|i| !is_closed_entity(&geometry[*i]))
            .collect();
        let ends: Vec<(usize, Vec<Pt>)> =
            open.iter().map(|&i| (i, flatten(&geometry[i]))).collect();
        let ends: Vec<(usize, Vec<Pt>)> = ends.into_iter().filter(|(_, p)| p.len() >= 2).collect();
        // partner[(entity slot, end)] = (entity slot, end)
        let end_point = |slot: usize, end: usize| -> Pt {
            let p = &ends[slot].1;
            if end == 0 { p[0] } else { p[p.len() - 1] }
        };
        let mut partner: Vec<[Option<(usize, usize)>; 2]> = vec![[None, None]; ends.len()];
        let mut chain_ok = true;
        for s in 0..ends.len() {
            for e in [0usize, 1] {
                let p = end_point(s, e);
                let near: Vec<(usize, usize)> = (0..ends.len())
                    .flat_map(|t| [(t, 0), (t, 1)])
                    .filter(|&(t, f)| (t, f) != (s, e) && d(end_point(t, f), p) <= tol)
                    .collect();
                match near.as_slice() {
                    [one] => partner[s][e] = Some(*one),
                    [] => {
                        chain_ok = false;
                        add(
                            "open_contour",
                            format!(
                                "the end of entity {} at ({}, {}) on {layer} is not joined to another entity: the profile is open",
                                ends[s].0 + 1,
                                p.0,
                                p.1
                            ),
                        );
                    }
                    _ => {
                        chain_ok = false;
                        add(
                            "open_contour",
                            format!(
                                "{} entities meet at ({}, {}) on {layer}: a profile must not branch",
                                near.len() + 1,
                                p.0,
                                p.1
                            ),
                        );
                    }
                }
            }
        }
        open_found |= !chain_ok;
        if chain_ok {
            let mut used = vec![false; ends.len()];
            for start in 0..ends.len() {
                if used[start] {
                    continue;
                }
                let (mut slot, mut enter) = (start, 0usize);
                let mut pts: Vec<Pt> = Vec::new();
                let mut direction: Option<bool> = None;
                loop {
                    used[slot] = true;
                    let mut p = ends[slot].1.clone();
                    if enter == 1 {
                        p.reverse();
                    }
                    // the file gives a direction to lines and polylines, not to arcs
                    if direction.is_none() && !matches!(geometry[ends[slot].0], Entity::Arc { .. })
                    {
                        direction = Some(enter == 0);
                    }
                    p.pop(); // the joint belongs to the next entity
                    pts.extend(p);
                    let Some((next, next_end)) = partner[slot][1 - enter] else {
                        break;
                    };
                    if next == start {
                        break;
                    }
                    if used[next] {
                        break;
                    }
                    slot = next;
                    enter = next_end;
                }
                contours.push(Contour {
                    layer: layer.into(),
                    pts,
                    as_written: direction,
                });
            }
        }
    }
    let outer_n = contours.iter().filter(|c| c.layer == OUTER_LAYER).count();
    let inner_n = contours.iter().filter(|c| c.layer == INNER_LAYER).count();
    if !open_found && outer_n != 1 {
        add(
            "open_contour",
            format!(
                "the file holds {outer_n} closed outer profiles on {OUTER_LAYER}; exactly one is expected"
            ),
        );
    }

    // ---- self_intersection / inner_inside_outer from the crossings found above
    let layer_of = |ent: usize| geometry[ent].layer();
    for (a, b) in &crossings {
        if (layer_of(*a) == layer_of(*b) && layer_of(*a) == OUTER_LAYER)
            || (a == b && layer_of(*a) == INNER_LAYER)
        {
            add(
                "self_intersection",
                format!(
                    "entities {} and {} of one profile cross each other",
                    a + 1,
                    b + 1
                ),
            );
        } else if layer_of(*a) == layer_of(*b) {
            add(
                "inner_inside_outer",
                format!("inner profiles {} and {} cross each other", a + 1, b + 1),
            );
        } else {
            add(
                "inner_inside_outer",
                format!(
                    "an inner profile crosses the outer profile (entities {} and {})",
                    a + 1,
                    b + 1
                ),
            );
        }
    }
    // containment and orientation need whole profiles
    if let (true, Some(outer)) = (
        outer_n == 1,
        contours.iter().find(|c| c.layer == OUTER_LAYER),
    ) {
        let area = signed_area(&outer.pts);
        match outer.as_written {
            Some(as_written) => {
                let written = if as_written { area } else { -area };
                if written.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                    add(
                        "orientation",
                        "the outer profile is not counter-clockwise in the direction its entities are written".into(),
                    );
                }
            }
            None => {
                if area == 0.0 {
                    add("orientation", "the outer profile encloses no area".into());
                }
            }
        }
        let inners: Vec<&Contour> = contours.iter().filter(|c| c.layer == INNER_LAYER).collect();
        for (n, c) in inners.iter().enumerate() {
            if signed_area(&c.pts).partial_cmp(&0.0) != Some(std::cmp::Ordering::Less) {
                add(
                    "orientation",
                    format!("inner profile {} is not clockwise", n + 1),
                );
            }
            // a sample of its vertices must lie inside the outer profile (crossings were tested above)
            let step = (c.pts.len() / 64).max(1);
            if c.pts.iter().step_by(step).any(|p| !inside(&outer.pts, *p)) {
                add(
                    "inner_inside_outer",
                    format!("inner profile {} lies outside the outer profile", n + 1),
                );
            }
            for (m, other) in inners.iter().enumerate() {
                if m != n && other.pts.first().is_some_and(|p| inside(&c.pts, *p)) {
                    add(
                        "inner_inside_outer",
                        format!(
                            "inner profile {} lies inside inner profile {}",
                            m + 1,
                            n + 1
                        ),
                    );
                }
            }
        }
    }

    // ---- extents: the header against the box of everything in the file
    let mut lowest_geometry = f64::MAX;
    let mut boxed: Vec<Pt> = Vec::new();
    for e in &geometry {
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
        for p in pts {
            lowest_geometry = lowest_geometry.min(p.1);
            boxed.push(p);
        }
    }
    for n in &notes {
        if let Entity::Text {
            at, height, text, ..
        } = n
        {
            boxed.push(*at);
            boxed.push((at.0 + height * text.len() as f64, at.1 + height));
        }
    }
    let (bx0, by0, bx1, by1) = boxed
        .iter()
        .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| {
            (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
        });
    let header_xy = |name: &str| -> Option<Pt> {
        let v = header.get(name)?;
        let get = |code: i32| {
            v.iter()
                .find(|(c, _)| *c == code)
                .and_then(|(_, x)| x.parse::<f64>().ok())
        };
        Some((get(10)?, get(20)?))
    };
    match (header_xy("$EXTMIN"), header_xy("$EXTMAX")) {
        (Some(lo), Some(hi)) => {
            let t = 1e-8f64.max(1e-9 * (bx1 - bx0).abs().max((by1 - by0).abs()));
            let off = (lo.0 - bx0)
                .abs()
                .max((lo.1 - by0).abs())
                .max((hi.0 - bx1).abs())
                .max((hi.1 - by1).abs());
            if off.partial_cmp(&t) != Some(std::cmp::Ordering::Less) {
                add(
                    "extents",
                    format!(
                        "$EXTMIN / $EXTMAX ({}, {}) .. ({}, {}) differ from the extents of the entities ({bx0}, {by0}) .. ({bx1}, {by1}) by {off} mm",
                        lo.0, lo.1, hi.0, hi.1
                    ),
                );
            }
        }
        _ => add(
            "extents",
            "the header does not give $EXTMIN and $EXTMAX".into(),
        ),
    }

    // ---- release_marking
    if !on_mark(MARK_TEXT) {
        add(
            "release_marking",
            "the NOT RELEASED marking is missing from the annotation layer".into(),
        );
    }
    if !geometry.is_empty()
        && notes.iter().any(
            |n| matches!(n, Entity::Text { at, height, .. } if at.1 + height > lowest_geometry),
        )
    {
        add(
            "release_marking",
            "annotation text lies on or above the cut geometry; it must lie below it".into(),
        );
    }

    QualityReport {
        entities: geometry.len(),
        annotations: notes.len(),
        outer_contours: outer_n,
        inner_contours: inner_n,
        segments_tested: segs.len(),
        join_tolerance: tol,
        pass: findings.is_empty(),
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{write, write_annotated};

    fn line(layer: &str, a: Pt, b: Pt) -> Entity {
        Entity::Line {
            layer: layer.into(),
            a,
            b,
        }
    }

    /// A 100 x 60 plate, counter-clockwise, with a clockwise diamond opening.
    fn plate() -> Vec<Entity> {
        vec![
            line(OUTER_LAYER, (0.0, 0.0), (100.0, 0.0)),
            line(OUTER_LAYER, (100.0, 0.0), (100.0, 60.0)),
            line(OUTER_LAYER, (100.0, 60.0), (0.0, 60.0)),
            line(OUTER_LAYER, (0.0, 60.0), (0.0, 0.0)),
            Entity::Polyline {
                layer: INNER_LAYER.into(),
                pts: vec![(50.0, 40.0), (60.0, 30.0), (50.0, 20.0), (40.0, 30.0)],
                closed: true,
            },
        ]
    }

    /// CONE-01 sector: two arcs and two straight seam edges.
    fn sector() -> Vec<Entity> {
        let (l1, l2, phi) = (
            632.455532033676f64,
            316.227766016838f64,
            113.84199576606166f64,
        );
        let at = |r: f64, deg: f64| arc_point((0.0, 0.0), r, deg);
        vec![
            Entity::Arc {
                layer: OUTER_LAYER.into(),
                c: (0.0, 0.0),
                r: l1,
                start_deg: 0.0,
                end_deg: phi,
            },
            line(OUTER_LAYER, at(l1, phi), at(l2, phi)),
            Entity::Arc {
                layer: OUTER_LAYER.into(),
                c: (0.0, 0.0),
                r: l2,
                start_deg: 0.0,
                end_deg: phi,
            },
            line(OUTER_LAYER, at(l2, 0.0), at(l1, 0.0)),
        ]
    }

    fn rules(text: &str) -> Vec<&'static str> {
        quality_gate(text).failed_rules()
    }

    /// The gate must name `rule` for the corrupted file, and the uncorrupted file must pass.
    fn must_fail(what: &str, text: &str, rule: &str) {
        let r = quality_gate(text);
        assert!(
            !r.pass && r.failed_rules().contains(&rule),
            "{what}: expected rule {rule}, got {:?}",
            r.findings
        );
    }

    #[test]
    fn well_formed_files_pass_every_rule() {
        for (name, e) in [("plate", plate()), ("sector", sector())] {
            let notes = vec![
                "MARKING SIDE: OUTSIDE".to_string(),
                "SEAM EDGES: ...".to_string(),
            ];
            for text in [write(&e), write_annotated(&e, &notes)] {
                let r = quality_gate(&text);
                assert!(r.pass, "{name}: {:?}", r.findings);
                assert_eq!(
                    (r.entities, r.outer_contours),
                    (e.len(), 1),
                    "{name}: {r:?}"
                );
            }
        }
        assert_eq!(quality_gate(&write(&plate())).inner_contours, 1);
        assert_eq!(RULES.len(), 12);
    }

    #[test]
    fn rule_readable_truncated_file_and_foreign_entities() {
        let t = write(&plate());
        must_fail("truncated", &t[..t.len() - 20], "readable");
        must_fail("no entities section", "  0\nEOF\n", "readable");
        must_fail(
            "a circle",
            &t.replace(
                "  0\nENDSEC\n  0\nEOF",
                "  0\nCIRCLE\n  8\nCUT_OUTER\n 10\n5.0\n 20\n5.0\n 40\n1.0\n  0\nENDSEC\n  0\nEOF",
            ),
            "readable",
        );
    }

    #[test]
    fn rule_units_declared_missing_or_wrong_units() {
        let t = write(&plate());
        must_fail(
            "inches",
            &t.replace("$LUNITS\n 70\n2", "$LUNITS\n 70\n4"),
            "units_declared",
        );
        must_fail(
            "clockwise angles",
            &t.replace("$ANGDIR\n 70\n0", "$ANGDIR\n 70\n1"),
            "units_declared",
        );
        must_fail(
            "radians",
            &t.replace("$AUNITS\n 70\n0", "$AUNITS\n 70\n3"),
            "units_declared",
        );
        must_fail(
            "no unit statement",
            &t.replace(UNITS_TEXT, "UNITS: INCHES"),
            "units_declared",
        );
        must_fail(
            "no header variable",
            &t.replace("  9\n$LUNITS\n 70\n2\n", ""),
            "units_declared",
        );
    }

    #[test]
    fn rule_finite_numbers_nan_infinity_and_bad_radius() {
        let t = write(&plate());
        must_fail("NaN", &t.replacen("100.0", "NaN", 1), "finite_numbers");
        must_fail(
            "infinity",
            &t.replacen("100.0", "1e999", 1),
            "finite_numbers",
        );
        let mut s = sector();
        if let Entity::Arc { r, .. } = &mut s[0] {
            *r = -5.0;
        }
        must_fail("negative radius", &write(&s), "finite_numbers");
    }

    #[test]
    fn rule_wrong_layer_geometry_and_text_on_the_wrong_layer() {
        let t = write(&plate());
        must_fail(
            "cut line on layer 0",
            &t.replacen("LINE\n  8\nCUT_OUTER", "LINE\n  8\n0", 1),
            "wrong_layer",
        );
        must_fail(
            "cut line on the annotation layer",
            &t.replacen(
                "LINE\n  8\nCUT_OUTER",
                &format!("LINE\n  8\n{MARK_LAYER}"),
                1,
            ),
            "wrong_layer",
        );
        must_fail(
            "text on a cut layer",
            &t.replacen(
                &format!("TEXT\n  8\n{MARK_LAYER}"),
                "TEXT\n  8\nCUT_OUTER",
                1,
            ),
            "wrong_layer",
        );
        // the opening written on the OUTER layer: two outer profiles
        must_fail(
            "opening on the outer layer",
            &t.replace("CUT_INNER", "CUT_OUTER"),
            "open_contour",
        );
    }

    #[test]
    fn rule_zero_length_segments() {
        let mut e = plate();
        e.push(line(OUTER_LAYER, (10.0, 10.0), (10.0, 10.0)));
        must_fail("zero-length line", &write(&e), "zero_length");
        let mut e = plate();
        if let Entity::Polyline { pts, .. } = &mut e[4] {
            pts.insert(1, (50.0, 40.0)); // a vertex twice
        }
        must_fail("repeated vertex", &write(&e), "zero_length");
        // flagged closed AND the first vertex repeated at the end
        let mut e = plate();
        if let Entity::Polyline { pts, .. } = &mut e[4] {
            pts.push((50.0, 40.0));
        }
        let r = quality_gate(&write(&e));
        assert!(
            r.findings.iter().any(|f| f.rule == "zero_length"
                && f.detail.contains("closing vertex repeats the first")),
            "{:?}",
            r.findings
        );
        let mut s = sector();
        if let Entity::Arc { end_deg, .. } = &mut s[0] {
            *end_deg = 0.0;
        }
        must_fail("zero-length arc", &write(&s), "zero_length");
    }

    #[test]
    fn rule_duplicate_or_overlapping_entities() {
        let mut e = plate();
        e.push(e[0].clone());
        must_fail(
            "the same line twice",
            &write(&e),
            "duplicate_or_overlapping",
        );
        let mut e = plate();
        e.push(line(OUTER_LAYER, (80.0, 0.0), (20.0, 0.0))); // lies on the bottom edge, reversed, shorter
        must_fail("collinear overlap", &write(&e), "duplicate_or_overlapping");
        let mut s = sector();
        s.push(Entity::Arc {
            layer: OUTER_LAYER.into(),
            c: (0.0, 0.0),
            r: 632.455532033676,
            start_deg: 50.0,
            end_deg: 130.0,
        });
        must_fail("overlapping arcs", &write(&s), "duplicate_or_overlapping");
        let mut e = plate();
        if let Entity::Polyline { pts, .. } = &mut e[4] {
            // the polyline runs back over its own first segment
            *pts = vec![
                (50.0, 40.0),
                (60.0, 30.0),
                (55.0, 35.0),
                (60.0, 30.0),
                (50.0, 20.0),
                (40.0, 30.0),
            ];
        }
        must_fail(
            "segment drawn twice",
            &write(&e),
            "duplicate_or_overlapping",
        );
        // an edge that only touches its neighbours is not an overlap
        assert!(!rules(&write(&plate())).contains(&"duplicate_or_overlapping"));
    }

    #[test]
    fn rule_open_contour_gaps_missing_edges_and_unflagged_openings() {
        let mut e = plate();
        e.remove(2);
        must_fail("an edge is missing", &write(&e), "open_contour");
        let mut e = plate();
        e[1] = line(OUTER_LAYER, (100.0, 0.0), (100.0, 59.99));
        must_fail("a gap of 0.01 mm", &write(&e), "open_contour");
        let mut e = plate();
        e[1] = line(OUTER_LAYER, (100.0, 0.0), (100.0, 60.0 - 1e-6));
        must_fail(
            "a gap of one nanometre... times a thousand",
            &write(&e),
            "open_contour",
        );
        // the opening as earlier builds wrote it: first vertex repeated, flag 70 = 0
        let mut e = plate();
        if let Entity::Polyline { pts, closed, .. } = &mut e[4] {
            pts.push((50.0, 40.0));
            *closed = false;
        }
        let r = quality_gate(&write(&e));
        assert!(
            r.findings.iter().any(|f| f.rule == "open_contour"
                && f.detail.contains("not flagged as a closed polyline")),
            "{:?}",
            r.findings
        );
        // a third edge meeting a corner
        let mut e = plate();
        e.push(line(OUTER_LAYER, (0.0, 0.0), (30.0, 30.0)));
        let r = quality_gate(&write(&e));
        assert!(
            r.findings.iter().any(|f| f.rule == "open_contour"
                && (f.detail.contains("must not branch") || f.detail.contains("not joined"))),
            "{:?}",
            r.findings
        );
        // no outer profile at all
        let e: Vec<Entity> = plate().into_iter().skip(4).collect();
        must_fail("no outer profile", &write(&e), "open_contour");
        // arcs joined to lines within rounding are closed
        assert!(quality_gate(&write(&sector())).pass);
    }

    #[test]
    fn rule_self_intersection() {
        // bow tie: two corners exchanged
        let e = vec![
            line(OUTER_LAYER, (0.0, 0.0), (100.0, 60.0)),
            line(OUTER_LAYER, (100.0, 60.0), (100.0, 0.0)),
            line(OUTER_LAYER, (100.0, 0.0), (0.0, 60.0)),
            line(OUTER_LAYER, (0.0, 60.0), (0.0, 0.0)),
        ];
        must_fail("bow tie", &write(&e), "self_intersection");
        // an opening that crosses itself
        let mut e = plate();
        if let Entity::Polyline { pts, .. } = &mut e[4] {
            *pts = vec![(40.0, 40.0), (60.0, 20.0), (60.0, 40.0), (40.0, 20.0)];
        }
        must_fail("figure-of-eight opening", &write(&e), "self_intersection");
        // a dense polyline that loops through itself, far from any vertex
        let mut pts: Vec<Pt> = (0..4000)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / 4000.0;
                (
                    300.0 + 100.0 * (2.0 * t).cos() * t.cos(),
                    300.0 + 100.0 * (2.0 * t).cos() * t.sin(),
                )
            })
            .collect();
        pts.reverse();
        let e = vec![
            line(OUTER_LAYER, (0.0, 0.0), (600.0, 0.0)),
            line(OUTER_LAYER, (600.0, 0.0), (600.0, 600.0)),
            line(OUTER_LAYER, (600.0, 600.0), (0.0, 600.0)),
            line(OUTER_LAYER, (0.0, 600.0), (0.0, 0.0)),
            Entity::Polyline {
                layer: INNER_LAYER.into(),
                pts,
                closed: true,
            },
        ];
        must_fail("rose curve", &write(&e), "self_intersection");
    }

    #[test]
    fn rule_inner_inside_outer() {
        let shifted = |dx: f64, dy: f64| -> String {
            let mut e = plate();
            if let Entity::Polyline { pts, .. } = &mut e[4] {
                pts.iter_mut().for_each(|p| *p = (p.0 + dx, p.1 + dy));
            }
            write(&e)
        };
        must_fail(
            "opening wholly outside",
            &shifted(200.0, 0.0),
            "inner_inside_outer",
        );
        must_fail(
            "opening across the edge",
            &shifted(45.0, 0.0),
            "inner_inside_outer",
        );
        must_fail(
            "opening across the top edge",
            &shifted(0.0, 25.0),
            "inner_inside_outer",
        );
        assert!(quality_gate(&shifted(30.0, 5.0)).pass, "still inside");
        // an opening inside another opening
        let mut e = plate();
        e.push(Entity::Polyline {
            layer: INNER_LAYER.into(),
            pts: vec![(50.0, 32.0), (52.0, 30.0), (50.0, 28.0), (48.0, 30.0)],
            closed: true,
        });
        must_fail("nested openings", &write(&e), "inner_inside_outer");
        // two openings that cross each other
        let mut e = plate();
        e.push(Entity::Polyline {
            layer: INNER_LAYER.into(),
            pts: vec![(58.0, 40.0), (68.0, 30.0), (58.0, 20.0), (48.0, 30.0)],
            closed: true,
        });
        must_fail("crossing openings", &write(&e), "inner_inside_outer");
    }

    #[test]
    fn rule_orientation() {
        // the same plate written clockwise
        let e = vec![
            line(OUTER_LAYER, (0.0, 0.0), (0.0, 60.0)),
            line(OUTER_LAYER, (0.0, 60.0), (100.0, 60.0)),
            line(OUTER_LAYER, (100.0, 60.0), (100.0, 0.0)),
            line(OUTER_LAYER, (100.0, 0.0), (0.0, 0.0)),
        ];
        must_fail("clockwise outer profile", &write(&e), "orientation");
        let mut e = plate();
        if let Entity::Polyline { pts, .. } = &mut e[4] {
            pts.reverse();
        }
        must_fail("counter-clockwise opening", &write(&e), "orientation");
    }

    #[test]
    fn rule_extents() {
        let t = write(&plate());
        let wrong = t.replacen("$EXTMAX\n 10\n", "$EXTMAX\n 10\n1", 1);
        assert_ne!(wrong, t);
        must_fail("header extents changed", &wrong, "extents");
        must_fail(
            "header extents missing",
            &t.replace("$EXTMIN", "$EXTMINX"),
            "extents",
        );
        // the geometry moved 1000 mm along x while the header stays. (Moving one corner does not change the
        // extents: the annotation box, which the header also covers, decides the largest x.)
        let shifted: Vec<Entity> = plate()
            .into_iter()
            .map(|e| match e {
                Entity::Line { layer, a, b } => Entity::Line {
                    layer,
                    a: (a.0 + 1000.0, a.1),
                    b: (b.0 + 1000.0, b.1),
                },
                Entity::Polyline { layer, pts, closed } => Entity::Polyline {
                    layer,
                    pts: pts.into_iter().map(|p| (p.0 + 1000.0, p.1)).collect(),
                    closed,
                },
                other => other,
            })
            .collect();
        let end = "  0\nENDSEC\n"; // the first ENDSEC closes the header
        let body = write(&shifted);
        let moved = format!(
            "{}{}",
            &t[..t.find(end).unwrap()],
            &body[body.find(end).unwrap()..]
        );
        must_fail("geometry moved, header kept", &moved, "extents");
        assert!(
            !rules(&moved).contains(&"open_contour"),
            "the profile itself is still closed"
        );
    }

    #[test]
    fn rule_release_marking() {
        let t = write(&plate());
        must_fail(
            "marking removed",
            &t.replace(MARK_TEXT, "RELEASED FOR MANUFACTURING"),
            "release_marking",
        );
        // the annotation moved up onto the part
        let i = t.find("  0\nTEXT").unwrap();
        let moved = format!("{}{}", &t[..i], t[i..].replacen(" 20\n-", " 20\n", 1));
        assert_ne!(moved, t);
        must_fail("annotation on the part", &moved, "release_marking");
    }

    #[test]
    fn every_rule_has_a_corrupted_file_that_trips_it_and_only_listed_rules_exist() {
        // a file that is wrong in several ways names all of them, and nothing outside RULES
        let mut e = plate();
        e.remove(2);
        e.push(e[0].clone());
        let t = write(&e)
            .replace("$LUNITS\n 70\n2", "$LUNITS\n 70\n4")
            .replace(MARK_TEXT, "x");
        let r = quality_gate(&t);
        for rule in [
            "units_declared",
            "duplicate_or_overlapping",
            "open_contour",
            "release_marking",
        ] {
            assert!(r.failed_rules().contains(&rule), "{rule}: {:?}", r.findings);
        }
        assert!(r.findings.iter().all(|f| RULES.contains(&f.rule)));
    }

    #[test]
    fn a_dense_profile_is_checked_in_reasonable_time() {
        let n = 46_080;
        let mut pts: Vec<Pt> = (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / n as f64;
                (5000.0 + 900.0 * t.cos(), 3000.0 + 700.0 * t.sin())
            })
            .collect();
        pts.reverse(); // clockwise
        let e = vec![
            line(OUTER_LAYER, (0.0, 0.0), (10000.0, 0.0)),
            line(OUTER_LAYER, (10000.0, 0.0), (10000.0, 6000.0)),
            line(OUTER_LAYER, (10000.0, 6000.0), (0.0, 6000.0)),
            line(OUTER_LAYER, (0.0, 6000.0), (0.0, 0.0)),
            Entity::Polyline {
                layer: INNER_LAYER.into(),
                pts,
                closed: true,
            },
        ];
        let started = std::time::Instant::now();
        let r = quality_gate(&write(&e));
        assert!(r.pass, "{:?}", r.findings);
        assert_eq!(r.segments_tested, n + 4);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
    }
}
