//! AMOEBA digest: an independent checker of a DXF file against the job that produced it.
//!
//! The digest reads the bytes of a DXF file itself (its own reader, no shared code with nf-dxf) and
//! re-measures it: structure, units and statements, geometry (closed contours, crossings, duplicates,
//! tiny segments, gaps), and dimensions (extents, lengths, area, arc radii, holes) against the reference
//! that the job declares. Every check gives a record with PASS, FAIL or INCONCLUSIVE and the values it
//! measured. A FAIL is a finding. An INCONCLUSIVE means the file could not be judged on that point.
//!
//! The digest decides nothing by itself: the server keeps a DXF back unless every check PASSes.
//! Tolerance (E4 v3): tol = max(1e-9 x M, min(0.001 mm, 1e-6 x M)), M = the part's bounding-box diagonal, for
//! positions, lengths, radii, joins and segments alike. Every finding that uses it reports the measured value and tol in mm.

pub mod geom;
pub mod reader;

use geom::{
    Piece, bbox_of, contours, crossings, diagonal, duplicates, inside, loop_area, pieces, tolerance,
};
use reader::{Ent, Parsed};

/// The marking statement every file carries (the same statement is required by policy, not taken from the writer).
pub const NOT_RELEASED: &str =
    "NOT RELEASED - DEVELOPMENT BUILD - NOT VALIDATED - NOT FOR MANUFACTURING";
/// Layer of the marking annotation: never a cutting layer.
pub const MARK_LAYER: &str = "NF_NOT_RELEASED";
/// Cutting layers of the outer profile and of the openings.
pub const OUTER_LAYER: &str = "CUT_OUTER";
pub const INNER_LAYER: &str = "CUT_INNER";
/// Millimetres in the $INSUNITS code of DXF.
const INSUNITS_MM: i64 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Inconclusive,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Inconclusive => "INCONCLUSIVE",
        }
    }
}

/// One check: its verdict, the values measured, and the reason in words.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub name: &'static str,
    pub verdict: Verdict,
    pub measured: Option<f64>,
    pub expected: Option<f64>,
    pub tolerance: Option<f64>,
    pub detail: String,
}

/// One curve of the job's reference (a pattern's outline or opening): its points in order, its declared
/// length and the largest distance of its points from the ideal curve (0 for a straight line).
#[derive(Clone, Debug, PartialEq)]
pub struct RefCurve {
    pub points: Vec<[f64; 2]>,
    pub length: f64,
    pub max_dev: f64,
    pub is_arc: bool,
    /// One of the two seam edges the engine declares for the part (a straight edge). The seam pair may coincide in the
    /// file (near-360 degree sectors): the two edges are then read as the two sides of one closed outline.
    pub seam: bool,
}

/// What the job declares for one part: the reference the file is checked against.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    pub part: String,
    pub outer: Vec<RefCurve>,
    pub holes: Vec<RefCurve>,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub part: String,
    pub checks: Vec<Check>,
    /// Set when the bytes cannot be read as DXF at all: every check is then INCONCLUSIVE.
    pub read_error: Option<String>,
    pub entities: usize,
    pub contours: usize,
    pub layers: usize,
}

impl Report {
    /// FAIL if any check fails; otherwise INCONCLUSIVE if any is inconclusive; PASS only when every check passes.
    pub fn verdict(&self) -> Verdict {
        if self.checks.iter().any(|c| c.verdict == Verdict::Fail) {
            Verdict::Fail
        } else if self.checks.is_empty()
            || self
                .checks
                .iter()
                .any(|c| c.verdict == Verdict::Inconclusive)
        {
            Verdict::Inconclusive
        } else {
            Verdict::Pass
        }
    }

    fn unreadable(part: &str, why: String) -> Report {
        let names = [
            "sections",
            "units",
            "numbers_finite",
            "layers",
            "entity_types",
            "not_released",
            "nf_metadata",
            "contours_closed",
            "seam_gaps",
            "self_intersection",
            "duplicates",
            "tiny_segments",
            "outer_profiles",
            "hole_count",
            "hole_position",
            "holes_inside_outline",
            "extents",
            "outline_length",
            "hole_length",
            "area",
            "arc_radii",
        ];
        Report {
            part: part.to_string(),
            checks: names
                .iter()
                .map(|n| Check {
                    name: n,
                    verdict: Verdict::Inconclusive,
                    measured: None,
                    expected: None,
                    tolerance: None,
                    detail: format!("the file cannot be read: {why}"),
                })
                .collect(),
            read_error: Some(why),
            entities: 0,
            contours: 0,
            layers: 0,
        }
    }
}

fn check(
    name: &'static str,
    verdict: Verdict,
    measured: Option<f64>,
    expected: Option<f64>,
    tolerance: Option<f64>,
    detail: String,
) -> Check {
    Check {
        name,
        verdict,
        measured,
        expected,
        tolerance,
        detail,
    }
}

fn pass_if(name: &'static str, ok: bool, detail: String) -> Check {
    check(
        name,
        if ok { Verdict::Pass } else { Verdict::Fail },
        None,
        None,
        None,
        detail,
    )
}

/// Check the DXF bytes. Bytes that are not UTF-8 text, or that are not a DXF group structure, give an
/// INCONCLUSIVE report; a readable file is judged check by check.
pub fn check_bytes(bytes: &[u8], reference: &Reference) -> Report {
    match std::str::from_utf8(bytes) {
        Ok(t) => check_text(t, reference),
        Err(_) => Report::unreadable(&reference.part, "the bytes are not UTF-8 text".into()),
    }
}

pub fn check_text(text: &str, reference: &Reference) -> Report {
    let parsed = match reader::parse(text) {
        Ok(p) => p,
        Err(why) => return Report::unreadable(&reference.part, why),
    };
    // the digest tolerance follows the part: the declared outline's diagonal, else the file's own geometry
    let declared = reference
        .outer
        .iter()
        .chain(reference.holes.iter())
        .flat_map(|c| c.points.iter().copied());
    let mut diag = diagonal(declared);
    if diag == 0.0 {
        diag = diagonal(parsed.ents.iter().flat_map(ent_points));
    }
    let tol = tolerance(diag);
    let (mut geo, empty) = pieces(&parsed.ents, tol);
    mark_seams(&mut geo, reference, tol);
    let all_contours = if parsed.nonfinite.is_empty() {
        contours(&geo, tol)
    } else {
        Vec::new()
    };
    let mut checks = vec![
        sections(&parsed),
        units(&parsed),
        numbers(&parsed),
        layers(&parsed),
        entity_types(&parsed.ents),
        not_released(&parsed),
        nf_metadata(&parsed),
    ];
    let finite = parsed.nonfinite.is_empty();
    let geometry_skip = |name: &'static str| {
        check(
            name,
            Verdict::Inconclusive,
            None,
            None,
            None,
            "coordinates are not finite; geometry is not measured".into(),
        )
    };
    if finite {
        checks.extend(geometry(&geo, &all_contours, empty, reference, tol));
    } else {
        for n in [
            "contours_closed",
            "seam_gaps",
            "self_intersection",
            "duplicates",
            "tiny_segments",
            "outer_profiles",
            "hole_count",
            "hole_position",
            "holes_inside_outline",
            "extents",
            "outline_length",
            "hole_length",
            "area",
            "arc_radii",
        ] {
            checks.push(geometry_skip(n));
        }
    }
    let contour_count = all_contours.len();
    Report {
        part: reference.part.clone(),
        checks,
        read_error: None,
        entities: parsed.ents.len(),
        contours: contour_count,
        layers: parsed.layers.len(),
    }
}

/// Points that bound an entity: its ends or vertices, and the corners of an arc's full circle.
/// Marks the segments of the file that are the declared seam pair: each declared seam (a curve with `seam` set) marks
/// the first segment whose two ends are its two ends within `tol`, in either direction. A straight edge is one segment;
/// a polyline outline (an eccentric reducer) carries its seam edges as segments of the same closed polyline. Only marked
/// segments may coincide with each other (geom::duplicates); an undeclared coincident edge is still a finding.
fn mark_seams(geo: &mut [Piece], r: &Reference, tol: f64) {
    let near = |a: reader::P, b: reader::P| (a[0] - b[0]).hypot(a[1] - b[1]) <= tol;
    let mut declared: Vec<[reader::P; 2]> = r
        .outer
        .iter()
        .chain(r.holes.iter())
        .filter(|c| c.seam && c.points.len() == 2)
        .map(|c| [c.points[0], c.points[1]])
        .collect();
    for p in geo.iter_mut().filter(|p| p.arc.is_none()) {
        let n = p.pts.len();
        let mut ends: Vec<(reader::P, reader::P)> =
            p.pts.windows(2).map(|w| (w[0], w[1])).collect();
        if p.closed && n >= 2 {
            ends.push((p.pts[n - 1], p.pts[0]));
        }
        for (k, (a, b)) in ends.into_iter().enumerate() {
            let found = declared
                .iter()
                .position(|d| (near(a, d[0]) && near(b, d[1])) || (near(a, d[1]) && near(b, d[0])));
            if let (Some(i), Some(flag)) = (found, p.seam_segs.get_mut(k)) {
                declared.remove(i);
                *flag = true;
            }
        }
    }
}

/// Distance between two points (mm).
fn step(a: reader::P, b: reader::P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn ent_points(e: &Ent) -> Vec<reader::P> {
    match e {
        Ent::Line { a, b, .. } => vec![*a, *b],
        Ent::Poly { pts, .. } => pts.clone(),
        Ent::Arc { c, r, .. } => vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] + r]],
        Ent::Text { .. } | Ent::Other { .. } => Vec::new(),
    }
}

fn sections(p: &Parsed) -> Check {
    let need = ["HEADER", "TABLES", "ENTITIES"];
    let missing: Vec<&str> = need
        .iter()
        .copied()
        .filter(|n| !p.sections.iter().any(|s| s == n))
        .collect();
    let ok = missing.is_empty() && p.eof && p.endsec == p.sections.len();
    let detail = if ok {
        format!("sections {} closed; EOF present", p.sections.join(", "))
    } else {
        format!(
            "missing sections: [{}]; sections {} ENDSEC {}; EOF {}",
            missing.join(", "),
            p.sections.len(),
            p.endsec,
            if p.eof { "present" } else { "missing" }
        )
    };
    pass_if("sections", ok, detail)
}

fn units(p: &Parsed) -> Check {
    let stated = p
        .comments
        .iter()
        .find_map(|c| c.strip_prefix("NF-UNITS:").map(|v| v.trim().to_string()));
    let texts_bad: Vec<String> = p
        .ents
        .iter()
        .filter_map(|e| match e {
            Ent::Text { text, .. }
                if text.trim().starts_with("UNITS:") && text.trim() != "UNITS: MILLIMETRES" =>
            {
                Some(text.trim().to_string())
            }
            _ => None,
        })
        .collect();
    if let Some(v) = p.insunits.filter(|v| *v != INSUNITS_MM) {
        return check(
            "units",
            Verdict::Fail,
            Some(v as f64),
            Some(INSUNITS_MM as f64),
            None,
            format!("$INSUNITS is {v}; millimetres require {INSUNITS_MM}"),
        );
    }
    if stated.as_deref().is_some_and(|v| v != "mm") {
        return check(
            "units",
            Verdict::Fail,
            None,
            None,
            None,
            format!(
                "NF-UNITS states '{}'; the file must be in millimetres",
                stated.clone().unwrap_or_default()
            ),
        );
    }
    if !texts_bad.is_empty() {
        return check(
            "units",
            Verdict::Fail,
            None,
            None,
            None,
            format!("annotation contradicts the units: {}", texts_bad.join("; ")),
        );
    }
    match stated {
        None => check(
            "units",
            Verdict::Fail,
            None,
            None,
            None,
            "the file states no unit (no $INSUNITS and no NF-UNITS statement)".into(),
        ),
        Some(_) => check(
            "units",
            Verdict::Pass,
            None,
            None,
            None,
            "millimetres (NF-UNITS: mm)".into(),
        ),
    }
}

fn numbers(p: &Parsed) -> Check {
    let n = p.nonfinite.len();
    pass_if(
        "numbers_finite",
        n == 0,
        if n == 0 {
            "every coordinate is finite".into()
        } else {
            format!(
                "{n} non-finite value(s): {}",
                p.nonfinite
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
    )
}

fn layers(p: &Parsed) -> Check {
    let declared = |l: &str| p.layers.iter().any(|d| d == l);
    let mut bad: Vec<String> = Vec::new();
    for e in &p.ents {
        let layer = e.layer();
        if !declared(layer) {
            bad.push(format!("undeclared layer {layer}"));
            continue;
        }
        match e {
            Ent::Text { .. } if layer != MARK_LAYER => bad.push(format!("annotation on {layer}")),
            Ent::Line { .. } | Ent::Arc { .. } | Ent::Poly { .. } if layer == MARK_LAYER => {
                bad.push("cut geometry on the marking layer".into())
            }
            Ent::Line { .. } | Ent::Arc { .. } | Ent::Poly { .. } if !layer.starts_with("CUT") => {
                bad.push(format!("geometry on non-cutting layer {layer}"))
            }
            _ => {}
        }
    }
    bad.sort();
    bad.dedup();
    let detail = if bad.is_empty() {
        format!(
            "{} layer(s) declared; cut geometry on CUT layers only",
            p.layers.len()
        )
    } else {
        bad.join("; ")
    };
    pass_if("layers", bad.is_empty(), detail)
}

fn entity_types(ents: &[Ent]) -> Check {
    let mut kinds: Vec<String> = ents
        .iter()
        .filter_map(|e| match e {
            Ent::Other { kind, .. } => Some(kind.clone()),
            _ => None,
        })
        .collect();
    kinds.sort();
    kinds.dedup();
    if kinds.is_empty() {
        check(
            "entity_types",
            Verdict::Pass,
            None,
            None,
            None,
            "only LINE, ARC, POLYLINE and TEXT entities".into(),
        )
    } else {
        check(
            "entity_types",
            Verdict::Inconclusive,
            None,
            None,
            None,
            format!(
                "entity kinds not checked by the digest: {}",
                kinds.join(", ")
            ),
        )
    }
}

fn not_released(p: &Parsed) -> Check {
    let on_mark = p.ents.iter().any(|e| matches!(e, Ent::Text { layer, text, .. } if layer == MARK_LAYER && text.trim() == NOT_RELEASED));
    let in_comment = p.comments.iter().any(|c| c.contains("NOT RELEASED"));
    let ok = on_mark && in_comment;
    pass_if(
        "not_released",
        ok,
        format!(
            "marking annotation {}; statement in the comment block {}",
            if on_mark { "present" } else { "MISSING" },
            if in_comment { "present" } else { "MISSING" }
        ),
    )
}

fn nf_metadata(p: &Parsed) -> Check {
    let mut bad = Vec::new();
    for c in &p.comments {
        if let Some((key, value)) = c.split_once(": ") {
            if key.starts_with("NF-") && value.trim().is_empty() {
                bad.push(format!("{key} has no value"));
            }
            if key == "NF-JOINT" && value.trim() != "set-on" && value.trim() != "not declared" {
                bad.push(format!("NF-JOINT states '{}'", value.trim()));
            }
        }
    }
    if !p.comments.iter().any(|c| c == "NF-UNITS: mm") {
        bad.push("NF-UNITS: mm missing".into());
    }
    pass_if(
        "nf_metadata",
        bad.is_empty(),
        if bad.is_empty() {
            "NF metadata complete and consistent".into()
        } else {
            bad.join("; ")
        },
    )
}

fn geometry(
    geo: &[Piece],
    all: &[geom::Contour],
    empty: usize,
    r: &Reference,
    tol: f64,
) -> Vec<Check> {
    let cut: Vec<&Piece> = geo.iter().filter(|p| p.layer.starts_with("CUT")).collect();
    let closed_outer: Vec<&geom::Contour> = all
        .iter()
        .filter(|c| c.closed && c.layer == OUTER_LAYER)
        .collect();
    let closed_inner: Vec<&geom::Contour> = all
        .iter()
        .filter(|c| c.closed && c.layer == INNER_LAYER)
        .collect();
    let mut out = Vec::new();

    // contours closed, seam gaps, branching
    let open: Vec<&geom::Contour> = all.iter().filter(|c| !c.closed).collect();
    let max_gap = open.iter().map(|c| c.gap).fold(0.0f64, f64::max);
    let branching: usize = open.iter().map(|c| c.branch_joints).sum();
    out.push(check(
        "contours_closed",
        if open.is_empty() {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        Some(open.len() as f64),
        Some(0.0),
        Some(tol),
        format!(
            "{} open contour(s), {} branching joint(s); every contour must close within {tol:.3e} mm",
            open.len(),
            branching
        ),
    ));
    out.push(check(
        "seam_gaps",
        if open.is_empty() {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        Some(max_gap),
        Some(0.0),
        Some(tol),
        format!("largest gap between free ends {max_gap:.6} mm; tolerance {tol:.3e} mm"),
    ));

    // crossings
    let (cross_n, cross_inconclusive) = crossings(&cut, tol);
    out.push(if cross_inconclusive {
        check("self_intersection", Verdict::Inconclusive, None, Some(0.0), None, "too many segments for the crossing test (or an arc needs more chords than the test samples)".into())
    } else {
        check("self_intersection", if cross_n == 0 { Verdict::Pass } else { Verdict::Fail }, Some(cross_n as f64), Some(0.0), None, format!("{cross_n} crossing pair(s)"))
    });

    // A reference that declares a straight or polyline feature shorter than the tolerance (a micro-scale branch or
    // saddle): the file's features of that size cannot be told apart from the declared ones, so a finding about them
    // is INCONCLUSIVE, not FAIL (0.4.12). The file is then held, and the job itself is not refused.
    let fine = r.outer.iter().chain(r.holes.iter()).any(|c| {
        !c.is_arc
            && c.points
                .windows(2)
                .any(|w| step(w[0], w[1]) > 0.0 && step(w[0], w[1]) <= tol)
    });
    let finding = |bad: bool| {
        if !bad {
            Verdict::Pass
        } else if fine {
            Verdict::Inconclusive
        } else {
            Verdict::Fail
        }
    };
    let below = if fine {
        "; the reference declares features shorter than the tolerance, so these cannot be told apart".to_string()
    } else {
        String::new()
    };

    // duplicates
    let dup = duplicates(&cut, tol);
    out.push(check(
        "duplicates",
        finding(dup > 0),
        Some(dup as f64),
        Some(0.0),
        Some(tol),
        format!("{dup} duplicated piece(s); tolerance {tol:.3e} mm{below}"),
    ));

    // tiny segments
    let tiny: usize = geo.iter().map(|p| p.tiny).sum::<usize>() + empty;
    let min_seg = geo.iter().map(|p| p.min_seg).fold(f64::INFINITY, f64::min);
    out.push(check(
        "tiny_segments",
        finding(tiny > 0),
        Some(tiny as f64),
        Some(0.0),
        Some(tol),
        format!(
            "{tiny} segment(s) shorter than {tol:.3e} mm or empty; shortest {}{below}",
            if min_seg.is_finite() {
                format!("{min_seg:.6} mm")
            } else {
                "none".into()
            }
        ),
    ));

    // structure: one outer profile, openings count and inside the outline
    let n_outer = closed_outer.len();
    out.push(check(
        "outer_profiles",
        if n_outer == 1 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        Some(n_outer as f64),
        Some(1.0),
        None,
        format!("{n_outer} closed outer profile(s); one is expected per part"),
    ));
    let n_holes = closed_inner.len();
    out.push(check(
        "hole_count",
        if n_holes == r.holes.len() {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        Some(n_holes as f64),
        Some(r.holes.len() as f64),
        None,
        format!(
            "{n_holes} closed opening(s) in the file; the job declares {}",
            r.holes.len()
        ),
    ));
    out.push(hole_position_check(&closed_inner, r, tol, fine));
    let outline: Option<&geom::Contour> = if n_outer == 1 {
        closed_outer.first().copied()
    } else {
        None
    };
    out.push(match outline {
        None => check(
            "holes_inside_outline",
            Verdict::Inconclusive,
            None,
            None,
            None,
            "no single closed outer profile to test against".into(),
        ),
        Some(o) => {
            let outside = closed_inner
                .iter()
                .filter(|h| h.samples.iter().any(|p| !inside(&o.samples, *p)))
                .count();
            check(
                "holes_inside_outline",
                if outside == 0 {
                    Verdict::Pass
                } else {
                    Verdict::Fail
                },
                Some(outside as f64),
                Some(0.0),
                None,
                format!("{outside} opening(s) not wholly inside the outline"),
            )
        }
    });

    // dimensions
    out.push(extents_check(&cut, r, tol));
    let outer_len: f64 = geo
        .iter()
        .filter(|p| p.layer == OUTER_LAYER)
        .map(|p| p.length)
        .sum();
    let hole_len: f64 = geo
        .iter()
        .filter(|p| p.layer == INNER_LAYER)
        .map(|p| p.length)
        .sum();
    let ref_outer_len: f64 = r.outer.iter().map(|c| c.length).sum();
    let ref_hole_len: f64 = r.holes.iter().map(|c| c.length).sum();
    let n_outer_curves = r.outer.len().max(1) as f64;
    let n_hole_curves = r.holes.len().max(1) as f64;
    out.push(length_check(
        "outline_length",
        outer_len,
        ref_outer_len,
        tol * n_outer_curves,
    ));
    out.push(length_check(
        "hole_length",
        hole_len,
        ref_hole_len,
        tol * n_hole_curves,
    ));
    out.push(area_check(&closed_outer, &closed_inner, r, tol));
    out.push(radii_check(&cut, r, tol));
    out
}

/// Centre of the bounding box of a point list (mm); None when there are no points.
fn bbox_centre(pts: &[[f64; 2]]) -> Option<[f64; 2]> {
    bbox_of(pts.iter().copied()).map(|b| [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])
}

fn at_text(p: Option<[f64; 2]>) -> String {
    match p {
        Some(p) => format!("{:.4}, {:.4}", p[0], p[1]),
        None => "no points".into(),
    }
}

/// Each drawn opening sits at exactly one declared hole, and each declared hole has exactly one drawn opening there
/// (one-to-one, by position, within the tolerance the extents use). The position of an opening is the centre of its
/// bounding box. A reference feature below the tolerance makes a finding INCONCLUSIVE (`fine`), not FAIL.
fn hole_position_check(drawn: &[&geom::Contour], r: &Reference, tol: f64, fine: bool) -> Check {
    let arc_dev = r
        .outer
        .iter()
        .chain(r.holes.iter())
        .map(|c| c.max_dev)
        .fold(0.0f64, f64::max);
    let pos_tol = tol + arc_dev;
    let at: Vec<Option<[f64; 2]>> = drawn.iter().map(|h| bbox_centre(&h.samples)).collect();
    let decl: Vec<Option<[f64; 2]>> = r.holes.iter().map(|h| bbox_centre(&h.points)).collect();
    let near = |a: Option<[f64; 2]>, b: Option<[f64; 2]>| match (a, b) {
        (Some(a), Some(b)) => (a[0] - b[0]).hypot(a[1] - b[1]) <= pos_tol,
        _ => false,
    };
    let mut bad = Vec::new();
    for (i, a) in at.iter().enumerate() {
        match decl.iter().filter(|b| near(*a, **b)).count() {
            1 => {}
            0 => bad.push(format!(
                "opening {} at ({}) matches no declared hole",
                i + 1,
                at_text(*a)
            )),
            _ => bad.push(format!(
                "opening {} at ({}) matches more than one declared hole",
                i + 1,
                at_text(*a)
            )),
        }
    }
    for (j, b) in decl.iter().enumerate() {
        match at.iter().filter(|a| near(**a, *b)).count() {
            1 => {}
            0 => bad.push(format!(
                "declared hole {} at ({}) has no opening at its position",
                j + 1,
                at_text(*b)
            )),
            _ => bad.push(format!(
                "declared hole {} at ({}) has more than one opening at its position",
                j + 1,
                at_text(*b)
            )),
        }
    }
    let verdict = if bad.is_empty() {
        Verdict::Pass
    } else if fine {
        Verdict::Inconclusive
    } else {
        Verdict::Fail
    };
    let detail = if bad.is_empty() {
        format!(
            "{} opening(s) each at one declared hole position, one-to-one; tolerance {pos_tol:.3e} mm",
            drawn.len()
        )
    } else {
        format!(
            "{}; tolerance {pos_tol:.3e} mm{}",
            bad.join("; "),
            if fine {
                "; the reference declares features shorter than the tolerance, so these cannot be told apart"
            } else {
                ""
            }
        )
    };
    check(
        "hole_position",
        verdict,
        Some(bad.len() as f64),
        Some(0.0),
        Some(pos_tol),
        detail,
    )
}

fn length_check(name: &'static str, measured: f64, expected: f64, tol: f64) -> Check {
    let diff = (measured - expected).abs();
    let ok = diff <= tol + 1e-9 * expected.abs();
    check(
        name,
        if ok { Verdict::Pass } else { Verdict::Fail },
        Some(measured),
        Some(expected),
        Some(tol),
        format!(
            "length {measured:.6} mm against {expected:.6} mm (difference {diff:.6}; tolerance {tol:.3e} mm)"
        ),
    )
}

fn extents_check(cut: &[&Piece], r: &Reference, tol: f64) -> Check {
    let dxf = bbox_of(cut.iter().flat_map(|p| {
        let b = p.bbox();
        [[b[0], b[1]], [b[2], b[3]]]
    }));
    let refb = bbox_of(
        r.outer
            .iter()
            .chain(r.holes.iter())
            .flat_map(|c| c.points.iter().copied()),
    );
    let arc_dev = r
        .outer
        .iter()
        .chain(r.holes.iter())
        .map(|c| c.max_dev)
        .fold(0.0f64, f64::max);
    match (dxf, refb) {
        (Some(d), Some(f)) => {
            let diff = (0..4).map(|i| (d[i] - f[i]).abs()).fold(0.0f64, f64::max);
            let tol = tol + arc_dev;
            check(
                "extents",
                if diff <= tol {
                    Verdict::Pass
                } else {
                    Verdict::Fail
                },
                Some(diff),
                Some(0.0),
                Some(tol),
                format!(
                    "bounding box [{:.4}, {:.4}, {:.4}, {:.4}] against the job's [{:.4}, {:.4}, {:.4}, {:.4}]",
                    d[0], d[1], d[2], d[3], f[0], f[1], f[2], f[3]
                ),
            )
        }
        (None, None) => check(
            "extents",
            Verdict::Pass,
            Some(0.0),
            Some(0.0),
            Some(tol),
            "no geometry, none declared".into(),
        ),
        _ => check(
            "extents",
            Verdict::Fail,
            None,
            None,
            None,
            "geometry and declared outline do not both exist".into(),
        ),
    }
}

/// Loop of a reference: its curves joined end to end (either direction) into one point sequence.
fn reference_loop(curves: &[RefCurve], tol: f64) -> Option<Vec<[f64; 2]>> {
    let first = curves.first()?;
    let mut used = vec![false; curves.len()];
    used[0] = true;
    let mut pts = first.points.clone();
    let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol;
    loop {
        let end = *pts.last()?;
        let next = curves.iter().enumerate().find(|(i, c)| {
            !used[*i]
                && c.points
                    .first()
                    .zip(c.points.last())
                    .is_some_and(|(f, l)| close(end, *f) || close(end, *l))
        });
        match next {
            Some((i, c)) => {
                used[i] = true;
                if close(end, c.points[0]) {
                    pts.extend(c.points.iter().skip(1).copied());
                } else {
                    pts.extend(c.points.iter().rev().skip(1).copied());
                }
            }
            None => break,
        }
    }
    if used.iter().all(|u| *u) && (curves.len() == 1 || close(*pts.last()?, pts[0])) {
        Some(pts)
    } else {
        None
    }
}

fn area_check(
    closed_outer: &[&geom::Contour],
    closed_inner: &[&geom::Contour],
    r: &Reference,
    tol: f64,
) -> Check {
    let perim: f64 = r.outer.iter().chain(r.holes.iter()).map(|c| c.length).sum();
    let arc_loss: f64 = r
        .outer
        .iter()
        .chain(r.holes.iter())
        .map(|c| c.max_dev * c.length)
        .sum();
    let join_tol = tol;
    let tol = tol * perim + arc_loss;
    let measured: f64 = closed_outer.iter().map(|c| c.area).sum::<f64>()
        - closed_inner.iter().map(|c| c.area).sum::<f64>();
    let Some(outer_pts) = reference_loop(&r.outer, join_tol) else {
        return check(
            "area",
            Verdict::Inconclusive,
            Some(measured),
            None,
            Some(tol),
            "the declared outline does not form one closed loop; area is not compared".into(),
        );
    };
    let expected =
        loop_area(&outer_pts) - r.holes.iter().map(|h| loop_area(&h.points)).sum::<f64>();
    let diff = (measured - expected).abs();
    check(
        "area",
        if diff <= tol {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        Some(measured),
        Some(expected),
        Some(tol),
        format!(
            "area {measured:.4} mm² against {expected:.4} mm² (difference {diff:.6}; tolerance {tol:.3e} mm)"
        ),
    )
}

fn radii_check(cut: &[&Piece], r: &Reference, tol: f64) -> Check {
    // A measured arc matches a declared arc when every declared point of that arc lies on the measured circle.
    // (Fitting a radius through three nearly collinear points is ill-conditioned for very large radii.)
    let arcs: Vec<geom::Arc> = cut.iter().filter_map(|p| p.arc).collect();
    let declared: Vec<&RefCurve> = r
        .outer
        .iter()
        .chain(r.holes.iter())
        .filter(|c| c.is_arc && c.points.len() >= 3)
        .collect();
    let unmatched = arcs
        .iter()
        .filter(|a| {
            !declared.iter().any(|c| {
                c.points.iter().all(|p| {
                    ((p[0] - a.c[0]).hypot(p[1] - a.c[1]) - a.r.abs()).abs() <= tol + c.max_dev
                })
            })
        })
        .count();
    let detail = format!(
        "{} arc(s) in the file, {} declared arc(s); {unmatched} arc(s) whose circle does not pass through a declared arc",
        arcs.len(),
        declared.len()
    );
    check(
        "arc_radii",
        if unmatched == 0 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        Some(unmatched as f64),
        Some(0.0),
        Some(tol),
        detail,
    )
}

#[cfg(test)]
mod tests;
