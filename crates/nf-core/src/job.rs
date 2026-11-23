//! Engineering jobs: deterministic passes from an engineer-entered request to
//! patterns, verification evidence, DXF files, re-import checks, gate
//! evaluation and release closure. A job is a pure function of its request:
//! running it again yields identical records and closure hash (replay audit).
//!
//! Single-part jobs record the request as one claim. The assembly job (header
//! with set-on branch) records every parameter as its own claim and links
//! records with typed dependency edges, so that `deps::propagate` can answer
//! "what is affected if this parameter changes?".

use crate::{
    ENGINE_VERSION, Family, Gate, GateResult, Record, Refusal, ReleaseState, closure, closure_hash,
    detail_map, rel, sha256_hex,
};
use nf_dxf::Entity;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uem::develop::{self, Branch};
use uem::verify::{self, Check, Verdict};
use uem::{Geom, Layer, Pattern, UemError, profile};

pub const SURFACE_REFERENCES: [&str; 4] = ["outside", "inside", "neutral", "unknown"];

fn unknown_surface() -> String {
    "unknown".into()
}

/// A `surface_reference` that is present must be a string (null is refused, as it always was); an absent one
/// is `None` through `#[serde(default)]`, which this function is not called for.
fn present_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum Request {
    CylinderShell {
        r_dev: f64,
        length: f64,
        /// Optional (work plan task 1.4): the wall surface the entered radius refers to, declared as for
        /// `branch_stub`. Absent: not declared, and the pattern and every check are exactly as before (the entered
        /// radius is the reference-surface radius, G06). `"unknown"`: the radius-dependent checks are INCONCLUSIVE.
        #[serde(
            default,
            deserialize_with = "present_string",
            skip_serializing_if = "Option::is_none"
        )]
        surface_reference: Option<String>,
    },
    ConeFrustum {
        r1: f64,
        r2: f64,
        h: f64,
        /// Optional (task 1.4): see `CylinderShell::surface_reference`.
        #[serde(
            default,
            deserialize_with = "present_string",
            skip_serializing_if = "Option::is_none"
        )]
        surface_reference: Option<String>,
    },
    EccentricReducer {
        r1: f64,
        r2: f64,
        h: f64,
        e: f64,
        /// Optional (task 1.4): see `CylinderShell::surface_reference`.
        #[serde(
            default,
            deserialize_with = "present_string",
            skip_serializing_if = "Option::is_none"
        )]
        surface_reference: Option<String>,
    },
    BranchStub {
        header_radius: f64,
        branch_radius: f64,
        branch_dev_radius: f64,
        beta_deg: f64,
        delta: f64,
        s_end: f64,
        /// Which wall surface the contact radii refer to. Optional for this family
        /// (requests written before it existed stay valid): absent means "unknown".
        #[serde(default = "unknown_surface")]
        surface_reference: String,
    },
    /// Assembly: header shell rolled from plate with one branch.
    HeaderWithBranch {
        header_radius: f64,
        header_dev_radius: f64,
        header_length: f64,
        branch_station: f64,
        branch_radius: f64,
        branch_dev_radius: f64,
        beta_deg: f64,
        delta: f64,
        s_end: f64,
        /// Which wall surface the entered contact radii refer to — a declared
        /// drawing-level convention, recorded and never inferred. Used for each
        /// part that does not declare its own surface below. It may be left out
        /// only when both part surfaces are given (BH-015), because then nothing
        /// needs it; `parse_strict` enforces that.
        #[serde(
            default,
            deserialize_with = "present_string",
            skip_serializing_if = "Option::is_none"
        )]
        surface_reference: Option<String>,
        /// Optional (review RB-001): the wall surface the HEADER contact radius refers to.
        /// Absent: `surface_reference`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        header_surface_reference: Option<String>,
        /// Optional (review RB-001): the wall surface the BRANCH contact radius refers to.
        /// Absent: `surface_reference`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch_surface_reference: Option<String>,
        /// Optional joint type. `"set_on"`: the set-on joint of Spec 02 §4.4, branch INSIDE
        /// surface on header OUTSIDE surface. The saddle on the branch and the opening in the
        /// header are then the same intersection curve, and the opening is the branch bore.
        /// Absent: the joint type is not declared; the same single contact curve is computed,
        /// labelled as such.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        joint: Option<String>,
    },
}

/// A request refusal with its code, the request fields it concerns, and its message.
fn coded(code: &str, fields: Vec<String>, message: impl Into<String>) -> Refusal {
    Refusal {
        code: code.to_string(),
        fields,
        message: message.into(),
        detail: None,
    }
}

/// The numeric and text fields of each family, as the request reads them (the same names as the form sends).
const FAMILY_FIELDS: [(&str, &[&str], &[&str]); 5] = [
    (
        "cylinder_shell",
        &["r_dev", "length"],
        &["surface_reference"],
    ),
    ("cone_frustum", &["r1", "r2", "h"], &["surface_reference"]),
    (
        "eccentric_reducer",
        &["r1", "r2", "h", "e"],
        &["surface_reference"],
    ),
    (
        "branch_stub",
        &[
            "header_radius",
            "branch_radius",
            "branch_dev_radius",
            "beta_deg",
            "delta",
            "s_end",
        ],
        &["surface_reference"],
    ),
    (
        "header_with_branch",
        &[
            "header_radius",
            "header_dev_radius",
            "header_length",
            "branch_station",
            "branch_radius",
            "branch_dev_radius",
            "beta_deg",
            "delta",
            "s_end",
        ],
        &[
            "surface_reference",
            "header_surface_reference",
            "branch_surface_reference",
            "joint",
        ],
    ),
];

/// The code of a request the parser refused, and the fields it names. serde's text names the missing field; a
/// wrong type is attributed to the fields of the family whose JSON value has the wrong type.
fn classify_parse_error(v: &Value, msg: &str) -> (&'static str, Vec<String>) {
    let Some(obj) = v.as_object() else {
        return (crate::refusal::REQUEST_NOT_OBJECT, vec![]);
    };
    let family = obj.get("family").and_then(Value::as_str);
    if family.is_none() || msg.contains("unknown variant") || msg.contains("missing field `family`")
    {
        return (crate::refusal::FAMILY_UNKNOWN, vec!["family".into()]);
    }
    if let Some(rest) = msg.split("missing field `").nth(1) {
        let name = rest.split('`').next().unwrap_or("").to_string();
        return (crate::refusal::FIELD_MISSING, vec![name]);
    }
    if msg.contains("invalid type") {
        let mut wrong = Vec::new();
        if let Some((_, nums, texts)) = FAMILY_FIELDS.iter().find(|(f, _, _)| Some(*f) == family) {
            for f in nums.iter() {
                if obj.get(*f).is_some_and(|x| !x.is_number()) {
                    wrong.push(f.to_string());
                }
            }
            for f in texts.iter() {
                // joint may be null (absent); surface_reference may not
                if obj
                    .get(*f)
                    .is_some_and(|x| !(x.is_string() || *f == "joint" && x.is_null()))
                {
                    wrong.push(f.to_string());
                }
            }
        }
        return (crate::refusal::FIELD_WRONG_TYPE, wrong);
    }
    (crate::refusal::REQUEST_MALFORMED, vec![])
}

impl Request {
    /// Names of numeric parameters that can be corrected individually.
    pub fn numeric_params(&self) -> Vec<(String, f64)> {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        v.as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, x)| x.as_f64().map(|f| (k.clone(), f)))
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Parse a request and refuse fields this build does not know (BE-016): a silently ignored "thickness" would
    /// let an engineer believe it was applied. The message is the text the API has always given; the refusal adds
    /// the stable code and the fields it concerns (docs/backend/REFUSAL_CODES.md).
    pub fn parse_strict(v: &Value) -> Result<Request, String> {
        Request::parse_refusal(v).map_err(|r| r.message)
    }

    /// As `parse_strict`, with the refusal in machine-readable form.
    pub fn parse_refusal(v: &Value) -> Result<Request, Refusal> {
        let req: Request = match serde_json::from_value(v.clone()) {
            Ok(r) => r,
            Err(e) => {
                let (code, fields) = classify_parse_error(v, &e.to_string());
                return Err(coded(code, fields, format!("invalid request: {e}")));
            }
        };
        // BH-015: the shared surface_reference is an absent-means-fallback only. It may be left out when both
        // part surfaces are given; otherwise a part would have no surface, and the request is refused as before.
        let shared_missing_without_both_parts = matches!(
            &req,
            Request::HeaderWithBranch {
                surface_reference: None,
                header_surface_reference,
                branch_surface_reference,
                ..
            } if header_surface_reference.is_none() || branch_surface_reference.is_none()
        );
        if shared_missing_without_both_parts {
            return Err(coded(
                crate::refusal::SURFACE_REFERENCE_REQUIRED,
                vec![
                    "surface_reference".into(),
                    "header_surface_reference".into(),
                    "branch_surface_reference".into(),
                ],
                "invalid request: missing field `surface_reference`",
            ));
        }
        let known = serde_json::to_value(&req)
            .map_err(|e| coded(crate::refusal::REQUEST_MALFORMED, vec![], e.to_string()))?;
        let unknown: Vec<String> = v
            .as_object()
            .map(|o| {
                o.keys()
                    .filter(|k| known.get(k.as_str()).is_none())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if !unknown.is_empty() {
            let message = format!(
                "invalid request: unknown field(s) {} for family '{}'. Nothing is ignored silently; remove them or use a family that has them",
                unknown.join(", "),
                known["family"].as_str().unwrap_or("")
            );
            return Err(coded(crate::refusal::FIELD_UNKNOWN, unknown, message));
        }
        Ok(req)
    }
    /// The surface declared for the families of task 1.4 (cylinder shell, cone, eccentric reducer); `None` when the
    /// field is absent, and for every other family (branch and header read their own declarations in `single` and
    /// `header_with_branch`).
    pub fn declared_surface(&self) -> Option<&str> {
        match self {
            Request::CylinderShell {
                surface_reference, ..
            }
            | Request::ConeFrustum {
                surface_reference, ..
            }
            | Request::EccentricReducer {
                surface_reference, ..
            } => surface_reference.as_deref(),
            Request::BranchStub { .. } | Request::HeaderWithBranch { .. } => None,
        }
    }
    pub fn family(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.get("family").and_then(|f| f.as_str().map(String::from)))
            .unwrap_or_default()
    }
    pub fn with_param(&self, name: &str, value: f64) -> Option<Request> {
        let mut v = serde_json::to_value(self).ok()?;
        let o = v.as_object_mut()?;
        if !o.get(name)?.is_number() {
            return None;
        }
        o.insert(name.to_string(), json!(value));
        serde_json::from_value(v).ok()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurveView {
    pub name: String,
    pub layer: String,
    pub kind: String,
    /// Lines and polylines: the exact vertices that are cut (the same vertices
    /// appear in the DXF; a polyline's chords deviate from the ideal curved
    /// edge by `eps_rep`). Arcs: points on the curve joined by chords FOR
    /// DISPLAY ONLY (the DXF carries the exact arc).
    pub points: Vec<[f64; 2]>,
    /// True only for straight lines, whose `points` are the exact curve. False
    /// for arcs and polylines (approximations of a curved edge).
    #[serde(default)]
    pub points_exact: bool,
    /// Largest distance between the chords through `points` and the exact
    /// curve (mm); 0 for lines. For a circular arc, the exact sagitta of the
    /// chords actually emitted (at most 0.01 mm unless the chord cap binds);
    /// for a polyline the engine's measured representation error `eps_rep` of
    /// that curve (a certified bound for the branch and the header; NOT certified for the eccentric reducer).
    #[serde(default)]
    pub points_max_deviation: f64,
    pub length: f64,
}

/// The stated status of the numerical errors (work plan task 1.2 review; TOLERANCE_POLICY section 8). Each is one
/// sentence, so the interface translates it sentence by sentence (web/i18n.js).
const EPS_NUM_BOUND: &str = "eps_num is an a-priori rounding bound, not interval-certified.";
const EPS_NUM_REDUCER: &str =
    "For an eccentric reducer its quadrature term is an ESTIMATE, not a bound.";
const EPS_REP_CERTIFIED: &str = "eps_rep is a certified upper bound of the chord error of the drawn polyline, eps_num included (interval arithmetic on the closed-form derivatives, docs/backend/TOLERANCE_POLICY.md section 8.1).";
const EPS_REP_NOT_CERTIFIED: &str = "eps_rep is NOT certified for this pattern (no bound could be proved, or its vertex term carries the quadrature estimate of the eccentric reducer): checks that would pass with it are INCONCLUSIVE (docs/backend/TOLERANCE_POLICY.md section 8.2).";
const EPS_REP_CERT_BRANCH: &str = "eps_rep is a certified upper bound (interval arithmetic, TOLERANCE_POLICY section 8) for a branch saddle and a header opening, where the pattern states it certified.";
const EPS_REP_NOT_CERT_REDUCER: &str = "For an eccentric reducer eps_rep is NOT certified: checks that would pass with it are INCONCLUSIVE; a FAIL stays FAIL.";

/// The assumption line of one pattern about its numerical errors.
fn eps_assumption(p: &uem::Pattern) -> String {
    if p.eps_rep > 0.0 && !p.eps_rep_certified {
        format!("{EPS_NUM_BOUND} {EPS_NUM_REDUCER} {EPS_REP_NOT_CERTIFIED}")
    } else if p.eps_rep > 0.0 {
        format!("{EPS_NUM_BOUND} {EPS_REP_CERTIFIED}")
    } else {
        EPS_NUM_BOUND.to_string()
    }
}

/// The G08 detail of a job: with polylines, the status of their representation errors as well.
fn g08_text_with_polylines() -> String {
    format!("{EPS_NUM_BOUND} {EPS_NUM_REDUCER} {EPS_REP_CERT_BRANCH} {EPS_REP_NOT_CERT_REDUCER}")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternView {
    pub part: String,
    pub family: String,
    pub class: String,
    /// Face of the formed part the pattern is seen from ("outside" / "inside"):
    /// the marking side of Spec 02 §0.3. A provisional convention of this build.
    #[serde(default)]
    pub marking_side: String,
    /// A-priori rounding bound of the numerical error (mm), not interval-certified; for the eccentric reducer it
    /// includes an ESTIMATE of the quadrature error (TOLERANCE_POLICY section 8.2).
    pub eps_num: f64,
    /// Representation error of the polyline (mm): a certified upper bound of the chord error where the pattern
    /// states it certified, otherwise NOT certified (TOLERANCE_POLICY section 8.1).
    pub eps_rep: f64,
    /// Task 1.1: true when 8·eps_num >= the tolerance of the part's 3D-intersection point check (agree + 4·bound).
    /// For the branch eps_num is that bound, so this is 4·eps_num >= agree; for the header eps_num is the larger of
    /// the plate's own estimate and the opening bound, so the flag can fire earlier on a long plate. In practice: a
    /// branch angle near 180 degrees, where the rounding of the angle itself dominates eps_num. The interface then
    /// shows eps_num prominently. Absent from the result when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub eps_num_governs: bool,
    pub quantities: BTreeMap<String, f64>,
    pub assumptions: Vec<String>,
    pub curves: Vec<CurveView>,
    pub holes: Vec<CurveView>,
    pub dxf_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckView {
    pub scope: String,
    pub name: String,
    pub method: String,
    pub independence: String,
    pub primary: f64,
    pub reference: f64,
    pub difference: f64,
    pub tolerance: f64,
    pub verdict: String,
    /// Why the verdict is INCONCLUSIVE, where a rule of this build sets it (task 1.4). Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobOutput {
    pub engine: String,
    pub request: Request,
    pub state: ReleaseState,
    /// Set exactly when the engine refused the request (state REFUSED, no pattern).
    pub outcome: Option<String>,
    /// The parameter the refusal is about, where the engine names one (kept as it always was).
    #[serde(default)]
    pub refused_field: Option<String>,
    /// The refusal in machine-readable form: code, request fields, message and the exact limit where the
    /// engine computes one (docs/backend/REFUSAL_CODES.md). Set exactly when `outcome` is.
    #[serde(default)]
    pub refusal: Option<Refusal>,
    pub gates: Vec<Gate>,
    pub patterns: Vec<PatternView>,
    pub checks: Vec<CheckView>,
    pub closure_hash: String,
    pub closure_size: usize,
    pub work_units: u64,
    /// Engine advisories of this job (for example the B2-001 development-radius advisory): accepted inputs an
    /// engineer must confirm. Separate from the lesson advisories the server adds. Also stated in G02.
    #[serde(default)]
    pub engine_advisories: Vec<String>,
    pub records: Vec<Record>,
    /// (part name, DXF text) — not serialised; stored as encrypted blobs.
    #[serde(skip)]
    pub dxf: Vec<(String, String)>,
}

fn layer_name(l: Layer) -> &'static str {
    match l {
        Layer::CutOuter => "CUT_OUTER",
        Layer::CutInner => "CUT_INNER",
        Layer::Reference => "REFERENCE",
    }
}

/// The DXF entities of a pattern: the outer profile, then the openings. An opening is a closed
/// profile: it is written with the closed flag (DXF polyline flag 70 bit 1) and WITHOUT repeating
/// its first vertex, which the closed flag implies (review RB-008). Its JSON points still carry
/// the repeated vertex, as they always did; the DXF and JSON agree vertex for vertex.
pub fn to_entities(p: &Pattern) -> Vec<Entity> {
    // RB-008: the outer profile is one closed polyline wherever R12 can hold it exactly (see outline_closed_polyline);
    // otherwise it stays one entity per curve, as before.
    let outer: Vec<Entity> = match outline_closed_polyline(&p.outline) {
        Some(pts) => vec![Entity::Polyline {
            layer: layer_name(Layer::CutOuter).to_string(),
            pts,
            closed: true,
        }],
        None => p.outline.iter().map(|c| curve_entity(c, false)).collect(),
    };
    let inner = p.holes.iter().map(|c| curve_entity(c, true));
    outer.into_iter().chain(inner).collect()
}

/// The entities as engine 0.4.4 wrote them: one entity per outline curve. Kept for the RB-008 test, which proves
/// the closed polyline of `to_entities` has the same vertices as these curves.
pub fn to_entities_per_curve(p: &Pattern) -> Vec<Entity> {
    let outer = p.outline.iter().map(|c| curve_entity(c, false));
    let inner = p.holes.iter().map(|c| curve_entity(c, true));
    outer.chain(inner).collect()
}

/// RB-008 (task 1.3): the outer profile as the vertices of ONE closed polyline (DXF polyline flag 70 bit 1), with
/// the same vertices as the profile's curves in the same order. Only where the format holds it exactly: every curve
/// a line or a polyline (an arc would need a vertex bulge, an encoding this build does not write, and chords would change
/// the cut), each curve
/// starting where the previous one ends, and the profile closing on its first vertex. The shared end points are
/// written once; the closing point is not repeated. None in every other case, and the profile stays as it was.
fn outline_closed_polyline(outline: &[uem::Curve]) -> Option<Vec<(f64, f64)>> {
    let mut pts: Vec<uem::P2> = Vec::new();
    for c in outline {
        let seg: Vec<uem::P2> = match &c.geom {
            Geom::Line { a, b } => vec![*a, *b],
            Geom::Polyline { pts, .. } => pts.clone(),
            Geom::Arc { .. } => return None,
        };
        let mut it = seg.into_iter();
        let first = it.next()?;
        match pts.last() {
            None => pts.push(first),
            Some(last) if *last == first => {}
            Some(_) => return None,
        }
        pts.extend(it);
    }
    if pts.len() < 3 || pts.first() != pts.last() {
        return None;
    }
    pts.pop();
    Some(pts.into_iter().map(|q| (q.x, q.y)).collect())
}

fn curve_entity(c: &uem::Curve, closed_profile: bool) -> Entity {
    let layer = layer_name(c.layer).to_string();
    match &c.geom {
        Geom::Line { a, b } => Entity::Line {
            layer,
            a: (a.x, a.y),
            b: (b.x, b.y),
        },
        Geom::Arc { c, r, a0, a1 } => Entity::Arc {
            layer,
            c: (c.x, c.y),
            r: *r,
            start_deg: a0.min(*a1).to_degrees(),
            end_deg: a0.max(*a1).to_degrees(),
        },
        Geom::Polyline { pts, .. } => {
            let mut v: Vec<(f64, f64)> = pts.iter().map(|q| (q.x, q.y)).collect();
            if closed_profile && v.len() >= 2 && v.first() == v.last() {
                v.pop();
            }
            Entity::Polyline {
                layer,
                pts: v,
                closed: closed_profile,
            }
        }
    }
}

/// Quantities that describe the polyline density. The result view lists only quantities with a unit that the
/// interface knows; these two are stated in the assumptions of the pattern instead (review RB-004).
const DENSITY_QUANTITIES: [&str; 2] = ["polyline_segments", "chord_error_limit"];

/// Sagitta tolerance of the arc points in the JSON result (mm, display only): the chords shown stay within this
/// distance of the arc (review B2-003).
const ARC_DISPLAY_TOL: f64 = 0.01;

/// Safety cap on the display chords of one arc. Where the tolerance needs more chords than this (very large
/// radii), the points stop at the cap and `points_max_deviation` states the true, larger deviation.
const ARC_DISPLAY_MAX_CHORDS: usize = 4096;

/// Display chords of an arc of radius `r` and sweep `sweep` (rad): the fewest n with r(1 - cos(sweep / 2n)) <= tol.
/// Since 1 - cos x <= x²/2, n = ceil((sweep/2)·sqrt(r / (2·tol))) meets the tolerance, capped at ARC_DISPLAY_MAX_CHORDS.
fn arc_display_chords(r: f64, sweep: f64) -> usize {
    let needed = (sweep.abs() / 2.0) * (r.abs() / (2.0 * ARC_DISPLAY_TOL)).sqrt();
    if needed.is_finite() {
        (needed.ceil() as usize).clamp(1, ARC_DISPLAY_MAX_CHORDS)
    } else {
        ARC_DISPLAY_MAX_CHORDS
    }
}

/// The annotation line every DXF of this build carries about its marking side.
pub fn marking_side_note(side: uem::ViewSide) -> String {
    nf_dxf::marking_side_line(side.code())
}

/// The request values a part's plate conventions state beyond its pattern. The caller that holds the request
/// sets it; a part of a family without an offset is `Plain`.
#[derive(Clone, Copy, Debug)]
pub enum Frame {
    /// Cylinder shell or cone frustum: no offset.
    Plain,
    /// Eccentric reducer: the offset e of its small end (mm, never negative).
    Reducer { e: f64 },
    /// Branch stub: the offset delta (mm), the lean beta (degrees) and the length s_end of the branch (mm).
    Branch {
        delta: f64,
        beta_deg: f64,
        s_end: f64,
    },
    /// Header plate of a header with one branch: the offset delta (mm) and the lean beta (degrees).
    Header { delta: f64, beta_deg: f64 },
}

fn frame_of(req: &Request) -> Frame {
    match req {
        Request::EccentricReducer { e, .. } => Frame::Reducer { e: *e },
        Request::BranchStub {
            delta,
            beta_deg,
            s_end,
            ..
        } => Frame::Branch {
            delta: *delta,
            beta_deg: *beta_deg,
            s_end: *s_end,
        },
        _ => Frame::Plain,
    }
}

const CONV_CONE_ANGLE: &str = "0 deg on seam_a (the +X axis), increasing counter-clockwise on the plate; on the part the angle about the axis increases counter-clockwise seen from the small end looking toward the large end";
const CONV_KERF: &str = "cut lines are the theoretical line on the development surface: kerf is NOT compensated; no bevel, root gap or weld allowance is included";
const CONV_STATUS: &str = "provisional convention of this build, declared by the software, not reviewed by a fabrication engineer";
const CONV_SIDE_SIGN: &str = "POSITIVE = THE BRANCH AXIS IS DISPLACED TO THE LEFT OF THE HEADER AXIS, FOR AN OBSERVER AT THE STATION-0 END LOOKING ALONG THE HEADER TOWARD INCREASING STATION WITH THE BRANCH POINTING UP";

/// The plate conventions of one part, as the job declares them (work plan task 1.3). Built from the pattern and the
/// request only; the DXF file, the result and the interface state these same facts, and the quality gate reads
/// them back. Values are in millimetres and degrees; `R_dev` is the developed radius of the part (circumference / 2 pi).
pub fn conventions_for(p: &Pattern, frame: Frame) -> nf_dxf::Conventions {
    use nf_dxf::{
        KEY_ANGLE_DATUM, KEY_BRANCH_ANGLE, KEY_END_EDGES, KEY_KERF, KEY_OFFSET_DELTA,
        KEY_OFFSET_SIGN, KEY_ORIGIN, KEY_PLATE_AXES, KEY_ROLLING_AXIS, KEY_SEAM_EDGES, KEY_STATUS,
        KEY_VIEW_SIDE,
    };
    use uem::develop::mm;
    let q = |name: &str| p.quantity(name).unwrap_or(f64::NAN);
    let (w, len) = (q("developed_width"), q("length"));
    let tau = std::f64::consts::TAU;
    let mut f: Vec<(&str, String)> = vec![(KEY_VIEW_SIDE, p.marking_side.code().to_string())];
    let offset = |f: &mut Vec<(&str, String)>, delta: f64, beta_deg: f64| {
        f.push((
            KEY_OFFSET_DELTA,
            format!(
                "delta = {} mm: the branch axis is displaced by delta from the header axis",
                mm(delta)
            ),
        ));
        f.push((KEY_OFFSET_SIGN, CONV_SIDE_SIGN.to_string()));
        f.push((KEY_BRANCH_ANGLE, format!("beta = {} deg, measured from the direction of increasing station to the branch axis; below 90 deg the branch leans toward increasing station", mm(beta_deg))));
    };
    match (p.family, frame) {
        ("header_shell_with_opening", Frame::Header { delta, beta_deg }) => {
            f.push((KEY_PLATE_AXES, format!("X = along the header axis (station), length {} mm; Y = around the header, developed circumference {} mm", mm(len), mm(w))));
            f.push((KEY_ROLLING_AXIS, "parallel to X: roll about an axis parallel to X; the drawn face becomes the outside of the header".into()));
            f.push((KEY_SEAM_EDGES, format!("seam_a and seam_b, the two edges parallel to X at Y = 0 and Y = {} mm; the seam is diametrically opposite the branch", mm(w))));
            f.push((KEY_END_EDGES, format!("bottom (X = 0, the station-0 end) and top (X = {} mm, the far end); station increases with X", mm(len))));
            f.push((KEY_ANGLE_DATUM, format!("Y = {} mm (half the circumference) is the generator facing the branch; Y increases toward the positive-delta side: counter-clockwise seen from the station-0 end looking toward increasing station", mm(0.5 * w))));
            f.push((
                KEY_ORIGIN,
                "lower left corner of the plate = the seam at the station-0 end (X = 0, Y = 0)"
                    .into(),
            ));
            offset(&mut f, delta, beta_deg);
        }
        ("cylinder_shell", _) => {
            f.push((KEY_PLATE_AXES, format!("X = around the shell, developed circumference {} mm; Y = along the shell axis, length {} mm", mm(w), mm(len))));
            f.push((KEY_ROLLING_AXIS, "parallel to Y: roll about an axis parallel to Y; the drawn face becomes the outside of the shell".into()));
            f.push((
                KEY_SEAM_EDGES,
                format!(
                    "seam_a and seam_b, the two edges parallel to Y at X = 0 and X = {} mm",
                    mm(w)
                ),
            ));
            f.push((
                KEY_END_EDGES,
                format!(
                    "bottom (Y = 0) and top (Y = {} mm): the two edges parallel to X",
                    mm(len)
                ),
            ));
            f.push((KEY_ANGLE_DATUM, format!("theta = X / R_dev, R_dev = {} mm: theta = 0 at the seam (X = 0), increasing with X; counter-clockwise seen from the end Y = {} mm looking toward Y = 0", mm(w / tau), mm(len))));
            f.push((
                KEY_ORIGIN,
                "lower left corner of the plate = the seam at the end Y = 0 (X = 0, Y = 0)".into(),
            ));
        }
        ("cone_frustum", _) => {
            let (l1, l2, phi) = (
                q("apex_radius_large"),
                q("apex_radius_small"),
                q("sector_angle"),
            );
            f.push((KEY_PLATE_AXES, "X along seam_a (the +X axis, angle 0); Y = 90 deg counter-clockwise from +X; distances in mm from the origin".into()));
            f.push((KEY_ROLLING_AXIS, "the cone axis, through the origin (the apex): roll so that the two straight edges meet; the drawn face becomes the outside of the cone".into()));
            f.push((
                KEY_SEAM_EDGES,
                format!(
                    "seam_a on the +X axis and seam_b at {} deg: the two straight edges",
                    mm(phi.to_degrees())
                ),
            ));
            f.push((KEY_END_EDGES, format!("outer arc (radius {} mm from the origin) = large end; inner arc (radius {} mm from the origin) = small end", mm(l1), mm(l2))));
            f.push((KEY_ANGLE_DATUM, CONV_CONE_ANGLE.into()));
            f.push((
                KEY_ORIGIN,
                "(0, 0) = the apex of the developed cone, not on the plate".into(),
            ));
        }
        ("eccentric_reducer", Frame::Reducer { e }) => {
            let psi = q("developed_angle");
            f.push((KEY_PLATE_AXES, "flat development of an oblique cone: X = along seam_a (the +X axis, angle 0); Y = 90 deg counter-clockwise from +X; distances in mm from the origin".into()));
            f.push((KEY_ROLLING_AXIS, "the cone axis, through the origin (the apex): roll so that the two straight edges meet; the drawn face becomes the outside of the cone".into()));
            f.push((KEY_SEAM_EDGES, format!("seam_a on the +X axis and seam_b at {} deg: the two straight edges; the seam is the generator on the side toward which the small end is offset", mm(psi.to_degrees()))));
            f.push((
                KEY_END_EDGES,
                "outer curve = large end; inner curve = small end".into(),
            ));
            f.push((KEY_ANGLE_DATUM, CONV_CONE_ANGLE.into()));
            f.push((
                KEY_ORIGIN,
                "(0, 0) = the apex of the developed oblique cone, not on the plate".into(),
            ));
            f.push((
                KEY_OFFSET_DELTA,
                format!("e = {} mm: the offset of the small-end axis", mm(e)),
            ));
            f.push((KEY_OFFSET_SIGN, "POSITIVE = THE SMALL END IS OFFSET TOWARD THE SEAM GENERATOR, THE SHORTEST GENERATOR; E = 0 IS A RIGHT CONE; THE OFFSET IS NEVER NEGATIVE".into()));
        }
        (
            "branch_stub",
            Frame::Branch {
                delta,
                beta_deg,
                s_end,
            },
        ) => {
            let s_min = q("s_min");
            f.push((KEY_PLATE_AXES, format!("X = around the branch, developed circumference {} mm; Y = along the branch axis, from the saddle cut (bottom) to the square end (top)", mm(w))));
            f.push((KEY_ROLLING_AXIS, "parallel to Y: roll about the branch axis; the drawn face becomes the outside of the branch".into()));
            f.push((
                KEY_SEAM_EDGES,
                format!(
                    "seam_a and seam_b, the two edges parallel to Y at X = 0 and X = {} mm",
                    mm(w)
                ),
            ));
            f.push((KEY_END_EDGES, format!("saddle cut (bottom edge, fits the header) and square end (top edge, Y = {} mm)", mm(s_end))));
            f.push((KEY_ANGLE_DATUM, format!("phi = X / R_dev, R_dev = {} mm: phi = 0 is the seam (generator on the positive-delta side); phi increases with X, counter-clockwise seen from the square end toward the header; phi = 90 deg away from the lean, 270 deg toward it", mm(w / tau))));
            f.push((KEY_ORIGIN, format!("X = 0 at the seam; Y = 0 is the level of the header axis, {} mm below the lowest point of the saddle cut (not on the plate)", mm(s_min))));
            offset(&mut f, delta, beta_deg);
        }
        _ => {}
    }
    f.push((KEY_KERF, CONV_KERF.into()));
    f.push((KEY_STATUS, CONV_STATUS.into()));
    // the canonical order of the statement: the order of nf_dxf::CONVENTION_KEYS
    f.sort_by_key(|(k, _)| {
        nf_dxf::CONVENTION_KEYS
            .iter()
            .position(|c| c == k)
            .unwrap_or(usize::MAX)
    });
    nf_dxf::Conventions {
        facts: f.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }
}

const MARKING_SIDE_SCRUTINY: &str = "Marking side: every pattern of this job is drawn as seen from the OUTSIDE of the formed part (pattern normal toward the outside). Spec 02 §0.3 requires a declared marking side but does not choose one; this side follows from the frames the specification writes down (§4.1, §4.4) and is a PROVISIONAL convention declared by the software. It has not been reviewed by a fabrication engineer. Marking a pattern on the other face mirrors the part (misfit of tens of millimetres for an offset, oblique branch). An engineer must confirm the marking side before use.";

fn curve_view(c: &uem::Curve) -> CurveView {
    // B2-003: the chord count of an arc follows from its radius and sweep, so the display stays within tolerance
    let chords = match c.geom {
        Geom::Arc { r, a0, a1, .. } => arc_display_chords(r, a1 - a0),
        _ => 1,
    };
    CurveView {
        name: c.name.to_string(),
        layer: layer_name(c.layer).to_string(),
        kind: match c.geom {
            Geom::Line { .. } => "line",
            Geom::Arc { .. } => "arc",
            Geom::Polyline { .. } => "polyline",
        }
        .to_string(),
        points: profile::flatten(std::slice::from_ref(c), chords)
            .iter()
            .map(|q| [q.x, q.y])
            .collect(),
        // B2-002: only straight lines are exact; a polyline approximates a curved edge
        points_exact: matches!(c.geom, Geom::Line { .. }),
        points_max_deviation: match c.geom {
            // sagitta of one chord of the chords actually emitted: r·(1 − cos(sweep / 2n)), exact for a circle
            // (validation defect D7; review B2-003)
            Geom::Arc { r, a0, a1, .. } => {
                r * (1.0 - ((a1 - a0).abs() / (2.0 * chords as f64)).cos())
            }
            // the curve's own measured representation error (develop.rs: per-curve eps_rep)
            Geom::Polyline { eps_rep, .. } => eps_rep,
            Geom::Line { .. } => 0.0,
        },
        length: profile::length(c),
    }
}

fn verdict_str(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "PASS",
        Verdict::Fail => "FAIL",
        Verdict::Inconclusive => "INCONCLUSIVE",
    }
}

fn check_view(scope: &str, c: &Check) -> CheckView {
    CheckView {
        scope: scope.into(),
        name: c.name.into(),
        method: c.method.into(),
        independence: c.independence.into(),
        primary: c.primary,
        reference: c.reference,
        difference: c.difference,
        tolerance: c.tolerance,
        verdict: verdict_str(c.verdict).into(),
        reason: c.reason.map(String::from),
    }
}

fn activity(kind: &str, zone: &str, inputs: &[&str], work_units: u64) -> Record {
    Record::new(
        Family::Activity,
        kind,
        json!({ "engine": ENGINE_VERSION, "zone": zone, "lane": "not_production_qualified", "work_units": work_units }),
        inputs.iter().map(|i| rel("input", i)).collect(),
        None,
    )
}

fn gate(id: &str, name: &str, critical: bool, result: GateResult, detail: &str) -> Gate {
    Gate {
        id: id.into(),
        name: name.into(),
        critical,
        result,
        detail: detail.into(),
    }
}

/// Accumulates records and per-part results while a job runs.
struct Builder {
    recs: Vec<Record>,
    wu: u64,
    patterns: Vec<PatternView>,
    checks: Vec<CheckView>,
    all_checks: Vec<Check>,
    dxf: Vec<(String, String)>,
    topo_ok: bool,
    reimport_ok: bool,
    has_polyline: bool,
    evaluated: Vec<String>,
    /// Declared marking side of every part, in order.
    sides: Vec<uem::ViewSide>,
    /// False once the DXF quality gate has refused a file of this job (such a file is never offered).
    quality_ok: bool,
    /// The parts whose DXF the quality gate refused, with the rules it failed.
    quality_notes: Vec<String>,
    /// Advisories (accepted, but an engineer must look): shown on the job itself and in G02.
    advisories: Vec<String>,
    /// Parts whose flat-pattern seam gap is below the resolution of the adjoining curves (Q2(b)).
    seam_unresolved: Vec<String>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            recs: vec![],
            wu: 0,
            patterns: vec![],
            checks: vec![],
            all_checks: vec![],
            dxf: vec![],
            topo_ok: true,
            reimport_ok: true,
            has_polyline: false,
            evaluated: vec![],
            sides: vec![],
            quality_ok: true,
            quality_notes: vec![],
            advisories: vec![],
            seam_unresolved: vec![],
        }
    }
    fn push(&mut self, r: Record) -> String {
        let id = r.id.clone();
        self.recs.push(r);
        id
    }
    fn act(&mut self, kind: &str, zone: &str, inputs: &[&str], wu: u64) -> String {
        self.wu += wu;
        self.push(activity(kind, zone, inputs, wu))
    }

    /// Record a verification evidence set about `subject`.
    fn evidence(
        &mut self,
        scope: &str,
        subject: &str,
        edge: &str,
        checks: Vec<Check>,
        wu: u64,
    ) -> String {
        let a = self.act("verify.family", "Z3", &[subject], wu);
        let views: Vec<CheckView> = checks.iter().map(|c| check_view(scope, c)).collect();
        let id = self.push(Record::new(
            Family::Assessment,
            "verification_evidence",
            json!({ "scope": scope, "checks": views }),
            vec![rel(edge, subject)],
            Some(a),
        ));
        self.checks.extend(views);
        self.all_checks.extend(checks);
        self.evaluated.push(id.clone());
        id
    }

    /// Pattern claim, profile report, DXF output and re-import report for one part.
    /// `deps` are the typed edges from the pattern to what it was developed from. `joint` is the joint the job
    /// declares for this part (header-with-branch parts only); the DXF states exactly that, and nothing else.
    fn part(
        &mut self,
        part: &str,
        pattern: &Pattern,
        deps: Vec<crate::Rel>,
        checks: Vec<Check>,
        joint: Option<&nf_dxf::JointStatement>,
        frame: Frame,
    ) -> (String, String) {
        let inputs: Vec<&str> = deps.iter().map(|d| d.to.as_str()).collect();
        let a_dev = self.act("build.develop", "Z2", &inputs, 1500);
        let entities = to_entities(pattern);
        let notes = vec![marking_side_note(pattern.marking_side)];
        // the joint lines are drawn at the size of the release marking, below the marking side
        let mut big = notes.clone();
        big.extend(joint.map(|j| j.texts()).unwrap_or_default());
        // task 1.3: the plate conventions, in the 999 block and as annotation lines below the geometry
        let conv = conventions_for(pattern, frame);
        let conv_complete = {
            let keys: Vec<&str> = conv.facts.iter().map(|(k, _)| k.as_str()).collect();
            uem::verify::convention_keys_complete(pattern.family, &keys)
                && conv
                    .facts
                    .iter()
                    .all(|(_, v)| !v.contains("NaN") && !v.contains("inf"))
        };
        let mut comments = joint.map(|j| j.comments()).unwrap_or_default();
        comments.extend(conv.comments());
        let text = nf_dxf::write_stated(&entities, &big, &conv.texts(), &comments);
        let dxf_sha = sha256_hex(text.as_bytes());
        let pv = PatternView {
            part: part.to_string(),
            family: pattern.family.to_string(),
            class: pattern.class.code().to_string(),
            marking_side: pattern.marking_side.code().to_string(),
            eps_num: pattern.eps_num,
            eps_rep: pattern.eps_rep,
            eps_num_governs: verify::eps_num_governs(pattern, &checks),
            quantities: pattern
                .quantities
                .iter()
                .filter(|(k, _)| !DENSITY_QUANTITIES.contains(k))
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            assumptions: pattern
                .assumptions
                .iter()
                .map(|s| s.to_string())
                .chain([
                    format!("Marking side: drawn as seen from the {} of the formed part. Provisional convention of this build (Spec 02 §0.3 requires a declared side and does not choose one); not reviewed by a fabrication engineer.", pattern.marking_side.code().to_uppercase()),
                    eps_assumption(pattern),
                ])
                .chain(pattern.quantity("polyline_segments").map(|n| format!("Polyline density: {n:.0} segments per curve, uniform in the angle; doubled from the base number until the measured chord error is within the chord error limit.")))
                .chain(pattern.quantity("chord_error_limit").map(|m| format!("Chord error limit: {m} mm, an implementation limit pending engineering review (docs/backend/TOLERANCE_POLICY.md); not an engineering tolerance.")))
                .collect(),
            curves: pattern.outline.iter().map(curve_view).collect(),
            holes: pattern.holes.iter().map(curve_view).collect(),
            dxf_sha256: dxf_sha.clone(),
        };
        let pattern_id = self.push(Record::new(Family::Claim, "pattern", json!({ "part": part, "pattern": pv, "axes": { "provenance": "computed", "authority": "calculated", "validation": "candidate" } }), deps, Some(a_dev)));
        self.evidence(part, &pattern_id, "verifies", checks, 6000);

        let a_prof = self.act("verify.profile", "Z3", &[&pattern_id], 800);
        let outer = profile::check(&pattern.outline, 1e-9);
        let holes_ok = pattern.holes.iter().all(|h| {
            let r = profile::check(std::slice::from_ref(h), 1e-9);
            r.closed && r.self_intersections == 0 && r.signed_area < 0.0
        });
        let ok =
            outer.closed && outer.self_intersections == 0 && outer.signed_area > 0.0 && holes_ok;
        self.topo_ok &= ok;
        // Q2(b): a seam gap below the resolution (cone: max(1e-9, 2 eps_num); reducer: max(1e-9, 2 eps_rep, 2 eps_num))
        // cannot be told from a closed seam, so the profile gate cannot say PASS.
        if let (Some(gap), Some(res)) = (
            pattern.quantity("seam_gap"),
            pattern.quantity("seam_gap_resolution"),
        ) && gap < res
        {
            self.seam_unresolved.push(format!(
                "{part}: the seam gap at the small end is {gap:e} mm, below {res:e} mm (the larger of 1e-9 mm and twice the representation and numerical errors of the adjoining curves)"
            ));
        }
        let prof_id = self.push(Record::new(
            Family::Assessment,
            "profile_report",
            json!({ "part": part, "pass": ok, "closed": outer.closed, "max_gap": outer.max_gap, "self_intersections": outer.self_intersections, "signed_area": outer.signed_area, "vertices": outer.vertices, "holes": pattern.holes.len(), "holes_valid": holes_ok }),
            vec![rel("verifies", &pattern_id)],
            Some(a_prof),
        ));

        let a_dxf = self.act("build.dxf", "Z2", &[&pattern_id], 200);
        let dxf_id = self.push(Record::new(
            Family::Claim,
            "dxf_output",
            json!({ "part": part, "sha256": dxf_sha, "bytes": text.len(), "entities": entities.len(), "format": "DXF AC1009 subset", "units": "mm: $LUNITS=2 (decimal), $AUNITS=0, $ANGDIR=0 in the header; the unit is stated by the NF-UNITS comment line, the comment block and a TEXT annotation (R12 has no unit-of-measure variable)", "marking": { "layer": nf_dxf::MARK_LAYER, "text": nf_dxf::MARK_TEXT, "marking_side": notes[0] }, "statement": { "comments": std::iter::once((nf_dxf::KEY_UNITS.to_string(), "mm".to_string())).chain(comments.clone()).collect::<Vec<(String, String)>>(), "texts": big[1..].to_vec(), "plate_conventions": conv.facts, "plate_convention_texts": conv.texts() } }),
            vec![rel("rendered_from", &pattern_id)],
            Some(a_dxf),
        ));
        let a_re = self.act("verify.reimport", "Z3", &[&dxf_id, &pattern_id], 200);
        let re = nf_dxf::reimport_check_stated(&entities, &text, 1e-8, &notes, joint);
        self.sides.push(pattern.marking_side);
        self.reimport_ok &= re.pass;
        let re_id = self.push(Record::new(
            Family::Assessment,
            "reimport_report",
            json!({ "part": part, "pass": re.pass, "entities_written": re.entities_written, "entities_read": re.entities_read, "unknown_entities": re.unknown_entities, "max_point_deviation": re.max_point_deviation, "max_length_deviation": re.max_length_deviation, "layers_match": re.layers_match, "annotations_read": re.annotations_read, "not_released_marking": re.not_released_marking, "units_declared": re.units_declared, "extents_match": re.extents_match, "marking_side_stated": re.required_notes_present, "joint_statement_matches": re.joint_statement_matches }),
            vec![rel("verifies", &dxf_id)],
            Some(a_re),
        ));
        // DXF quality gate (nf-dxf quality, review item 5): the file as written and re-read must pass it.
        // A file that does not pass is never offered: it is withheld from the job's DXF list and G15 fails.
        let a_q = self.act("verify.dxf_quality", "Z3", &[&dxf_id], 300);
        let mut quality = nf_dxf::quality::quality_gate_full(&text, joint, Some(&conv));
        if !conv_complete {
            // the job itself declares a convention set its family does not require (or a value that is not a number)
            quality.findings.push(nf_dxf::quality::QualityFinding {
                rule: "convention_statement",
                detail: "the plate conventions the job declares are not the complete set of this family, or hold a value that is not a number".into(),
            });
            quality.pass = false;
        }
        let q_id = self.push(Record::new(
            Family::Assessment,
            "dxf_quality_report",
            json!({ "part": part, "pass": quality.pass, "rules_failed": quality.failed_rules(), "findings": quality.findings.iter().map(|f| format!("{}: {}", f.rule, f.detail)).collect::<Vec<String>>(), "entities": quality.entities, "outer_contours": quality.outer_contours, "inner_contours": quality.inner_contours, "segments_tested": quality.segments_tested, "rules_checked": nf_dxf::quality::RULES }),
            vec![rel("verifies", &dxf_id)],
            Some(a_q),
        ));
        self.quality_ok &= quality.pass;
        if !quality.pass {
            self.quality_notes
                .push(format!("{part}: {}", quality.failed_rules().join(", ")));
        }
        self.has_polyline |= pattern.eps_rep > 0.0;
        self.patterns.push(pv);
        if quality.pass {
            self.dxf.push((part.to_string(), text));
        }
        self.evaluated
            .extend([pattern_id.clone(), dxf_id.clone(), prof_id, re_id, q_id]);
        (pattern_id, dxf_id)
    }

    /// `refusal` = (the refusal, the parameter it is about) when the engine refused the request.
    fn finish(
        mut self,
        req: &Request,
        mut gates: Vec<Gate>,
        refusal: Option<(Refusal, Option<String>)>,
    ) -> JobOutput {
        let (outcome, refused_field, refusal) = match refusal {
            Some((r, field)) => (Some(r.message.clone()), field, Some(r)),
            None => (None, None, None),
        };
        // D1: every part of one job must be drawn from the same, declared side. Anything else fails G02.
        if self.sides.iter().any(|s| *s != uem::MARKING_SIDE)
            && let Some(g) = gates.iter_mut().find(|g| g.id == "G02")
        {
            g.result = GateResult::Fail;
            g.detail = format!(
                "The parts of this job are not all drawn from the declared marking side ({:?}); marked on one face they would not fit. {}",
                self.sides, g.detail
            );
        }
        if !self.quality_ok
            && let Some(g) = gates.iter_mut().find(|g| g.id == "G15")
        {
            g.result = GateResult::Fail;
            g.detail = format!(
                "The DXF quality gate refused a file of this job, so it is not offered ({}). {}",
                self.quality_notes.join("; "),
                g.detail
            );
        }
        if !self.seam_unresolved.is_empty()
            && let Some(g) = gates
                .iter_mut()
                .find(|g| g.id == "G11" && g.result == GateResult::Pass)
        {
            g.result = GateResult::Inconclusive;
            g.detail = format!(
                "The seam cannot be told from a closed one: {}. {}",
                self.seam_unresolved.join("; "),
                g.detail
            );
        }
        // A refusal is not a result under review: it has its own state, end to end (BE-003).
        let state = if outcome.is_some() || self.patterns.is_empty() {
            ReleaseState::Refused
        } else {
            crate::release_state(&gates)
        };
        // gate ids are stable; keep the table in a fixed order
        gates.sort_by_key(|g| g.id.clone());
        let rels: Vec<crate::Rel> = self.evaluated.iter().map(|e| rel("evaluates", e)).collect();
        let gate_rec = Record::new(
            Family::Assessment,
            "gate_evaluation",
            json!({ "gates": gates, "state": state }),
            rels,
            None,
        );
        let root = gate_rec.id.clone();
        self.recs.push(gate_rec);
        let map: BTreeMap<String, Record> = self
            .recs
            .iter()
            .map(|r| (r.id.clone(), r.clone()))
            .collect();
        let (ids, missing) = closure(&map, &[root]);
        debug_assert!(missing.is_empty());
        JobOutput {
            engine: ENGINE_VERSION.into(),
            request: req.clone(),
            state,
            outcome,
            refused_field,
            refusal,
            gates,
            patterns: self.patterns,
            checks: self.checks,
            closure_hash: closure_hash(&ids),
            closure_size: ids.len(),
            work_units: self.wu,
            engine_advisories: self.advisories,
            records: self.recs,
            dxf: self.dxf,
        }
    }
}

/// Gate evaluation for this build. Gates the software cannot evaluate are
/// reported as NOT_IMPLEMENTED or INCONCLUSIVE — never as PASS. G18 and G19
/// depend on state that changes after the job (obligations, replay) and are
/// re-evaluated by the server when a result is read.
pub fn evaluate_gates(
    checks: &[Check],
    topo_ok: Option<bool>,
    reimport_ok: Option<bool>,
    has_polyline: bool,
    surface_reference: Option<&str>,
) -> Vec<Gate> {
    let ver = if checks.is_empty() {
        GateResult::NotRun
    } else if checks.iter().any(|c| c.verdict == Verdict::Fail) {
        GateResult::Fail
    } else if checks.iter().any(|c| c.verdict == Verdict::Inconclusive) {
        GateResult::Inconclusive
    } else {
        GateResult::Pass
    };
    let tri = |x: Option<bool>| match x {
        None => GateResult::NotRun,
        Some(true) => GateResult::Pass,
        Some(false) => GateResult::Fail,
    };
    let g08_detail: String = if has_polyline {
        g08_text_with_polylines()
    } else {
        EPS_NUM_BOUND.to_string()
    };
    let mut g = vec![
        gate(
            "G01",
            "provenance_complete",
            true,
            GateResult::Inconclusive,
            "Parameters were entered manually; they are not linked to confirmed dimensions on a source drawing (drawing intelligence is not implemented).",
        ),
        gate(
            "G05",
            "class_assigned",
            true,
            GateResult::Pass,
            "Development class assigned by the surface-type constructor. Class naming scheme is a proposal (Spec 02 §14), not adopted.",
        ),
        gate(
            "G06",
            "assumptions_backed",
            true,
            GateResult::Inconclusive,
            "No calibrated process data exists. Radii are used as reference-surface radii exactly as entered; thickness, allowances, weld gap and kerf are not modelled.",
        ),
        gate(
            "G08",
            "numeric_bounds",
            true,
            GateResult::Inconclusive,
            &g08_detail,
        ),
        gate(
            "G10",
            "verification",
            true,
            ver,
            "Independent-formulation checks (class I-F), including interface constraints where the job has several parts. No separately implemented verifier or external kernel cross-check exists yet.",
        ),
        gate(
            "G11",
            "profile_topology",
            true,
            tri(topo_ok),
            "Outer profiles closed, simple, counter-clockwise; openings closed, simple, clockwise (binary64 arithmetic, not exact predicates). Kerf-compensation ownership is not declared.",
        ),
        gate(
            "G14",
            "algorithm_status",
            true,
            GateResult::Inconclusive,
            "Algorithms are evidence level EV-1 (not independently reviewed) and not production-qualified.",
        ),
        gate(
            "G15",
            "output_integrity",
            true,
            tri(reimport_ok),
            "Every DXF is re-imported by a separate reader and compared with the intended geometry, and checked by the DXF quality gate (open contours, overlaps, self-intersection, orientation, extents, release marking); a file that fails is not offered. CAM acceptance not tested.",
        ),
        gate(
            "G16",
            "human_approval",
            true,
            GateResult::NotImplemented,
            "Engineer approval workflow is not implemented. This result cannot be released for manufacturing.",
        ),
        gate(
            "G18",
            "obligation_closure",
            true,
            GateResult::NotRun,
            "Evaluated when the result is read: depends on the current state of scrutiny obligations.",
        ),
        gate(
            "G19",
            "self_consistency",
            true,
            GateResult::NotRun,
            "Evaluated when the result is read: depends on replay audits. Audits A1-A4 are not implemented.",
        ),
    ];
    // G02: open ambiguities. The marking side is one on every job (validation defect D1); the surface the
    // entered radii refer to is another where the request has radii of two kinds.
    let mut detail = MARKING_SIDE_SCRUTINY.to_string();
    if let Some(sr) = surface_reference {
        detail.push(' ');
        detail.push_str(&if sr == "unknown" {
            "Surface reference of the entered radii is declared UNKNOWN: inside / outside / neutral must be established by an engineer before use.".to_string()
        } else {
            format!("Surface reference declared by the user as '{sr}'. The declaration is recorded, not checked against a drawing.")
        });
    }
    g.push(gate(
        "G02",
        "no_open_ambiguity",
        true,
        GateResult::Inconclusive,
        &detail,
    ));
    g
}

fn err_output(req: &Request, mut b: Builder, about: &str, e: UemError) -> JobOutput {
    let critical = !matches!(e, UemError::OutOfRange { .. });
    let a = b.act("build.develop", "Z2", &[about], 1500);
    let f = b.push(Record::new(
        Family::Finding,
        "engineering_outcome",
        json!({ "outcome": e.to_string(), "message": e.to_string(), "critical": critical, "code": e.code(), "fields": e.fields(), "detail": detail_map(e.detail()) }),
        vec![rel("about", about)],
        Some(a),
    ));
    b.evaluated.push(f);
    b.patterns.clear();
    b.dxf.clear();
    b.checks.clear();
    let gates = vec![gate(
        "G05",
        "class_assigned",
        true,
        if critical {
            GateResult::Fail
        } else {
            GateResult::Inconclusive
        },
        &format!("No pattern produced: {e}"),
    )];
    let field = match &e {
        UemError::OutOfRange { param, .. } => Some(param.to_string()),
        _ => None,
    };
    b.finish(req, gates, Some((Refusal::from_engine(&e), field)))
}

fn request_claim(req: &Request, entered_by: &str) -> Record {
    Record::new(
        Family::Claim,
        "development_request",
        json!({ "request": req, "entered_by": entered_by, "axes": { "provenance": "human_entered", "authority": "assumed", "reading": "n/a", "validation": "candidate" } }),
        vec![],
        None,
    )
}

fn single(
    req: &Request,
    entered_by: &str,
    part: &str,
    r: Result<(Pattern, Vec<Check>), UemError>,
) -> JobOutput {
    let mut b = Builder::new();
    let rq = b.push(request_claim(req, entered_by));
    // BE-001 for the single branch stub: the same rule as for the assembly, when a surface is declared.
    let mut scrutiny: Option<String> = None;
    if let Request::BranchStub {
        branch_radius,
        branch_dev_radius,
        surface_reference,
        ..
    } = req
    {
        if !SURFACE_REFERENCES.contains(&surface_reference.as_str()) {
            return err_output(
                req,
                b,
                &rq,
                uem::out_of_range(
                    crate::refusal::SURFACE_REFERENCE_INVALID,
                    "surface_reference",
                    vec!["surface_reference"],
                    format!("must be one of {SURFACE_REFERENCES:?}"),
                ),
            );
        }
        if branch_radius.is_finite()
            && *branch_radius > 0.0
            && branch_dev_radius.is_finite()
            && *branch_dev_radius > 0.0
        {
            match radius_consistency(
                surface_reference,
                "branch_radius",
                *branch_radius,
                "branch_dev_radius",
                *branch_dev_radius,
            ) {
                Err(reason) => {
                    return err_output(
                        req,
                        b,
                        &rq,
                        radius_error(reason, "branch_dev_radius", "branch_radius"),
                    );
                }
                Ok(note) => {
                    b.advisories.extend(note.iter().cloned());
                    scrutiny = note;
                }
            }
        }
    }
    match r {
        Err(e) => err_output(req, b, &rq, e),
        Ok((p, checks)) => {
            b.part(
                part,
                &p,
                vec![rel("developed_from", &rq)],
                checks,
                None,
                frame_of(req),
            );
            let sref = match req {
                Request::BranchStub {
                    surface_reference, ..
                } => Some(surface_reference.as_str()),
                _ => req.declared_surface(),
            };
            let mut gates = evaluate_gates(
                &b.all_checks,
                Some(b.topo_ok),
                Some(b.reimport_ok),
                b.has_polyline,
                sref,
            );
            if let (Some(g), Some(note)) = (gates.iter_mut().find(|g| g.id == "G02"), scrutiny) {
                g.detail.push(' ');
                g.detail.push_str(&note);
            }
            // task 1.4: an INCONCLUSIVE radius check says why, in the gate that holds it as well as in the check
            if b.all_checks.iter().any(|c| c.reason.is_some())
                && let Some(g) = gates.iter_mut().find(|g| g.id == "G10")
            {
                g.detail.push(' ');
                g.detail.push_str(verify::SURFACE_UNKNOWN_REASON);
            }
            b.finish(req, gates, None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// The set-on joint of Spec 02 §4.4 as the request declares it: branch INSIDE surface on header OUTSIDE surface.
pub const SET_ON_HEADER_SURFACE: &str = "outside";
pub const SET_ON_BRANCH_SURFACE: &str = "inside";

/// Header with one branch. `dims` = header radius, header development radius, header length, branch
/// station, branch radius, branch development radius, beta (deg), delta, s_end; `surfaces` = the wall
/// surface of the header radius and of the branch radius; `joint` = the declared joint type, if any.
fn header_with_branch(
    req: &Request,
    entered_by: &str,
    dims: [f64; 9],
    surfaces: (&str, &str),
    joint: Option<&str>,
) -> JobOutput {
    let [hr, hdev, hlen, station, br, bdev, beta_deg, delta, s_end] = dims;
    let (hsref, bsref) = surfaces;
    // the joint as the job declares it: the same words in the DXF, the records and the interface (BH-004)
    let js = nf_dxf::JointStatement::new(joint == Some("set_on"), hsref, bsref);
    let mut b = Builder::new();
    let rq = b.push(request_claim(req, entered_by));
    for (param, value) in [
        ("header_surface_reference", hsref),
        ("branch_surface_reference", bsref),
    ] {
        if !SURFACE_REFERENCES.contains(&value) {
            return err_output(
                req,
                b,
                &rq,
                uem::out_of_range(
                    crate::refusal::SURFACE_REFERENCE_INVALID,
                    param,
                    vec![param],
                    format!("must be one of {SURFACE_REFERENCES:?}"),
                ),
            );
        }
    }
    // RB-001: the joint type is declared or absent. A set-on joint is only the Spec 02 §4.4 pairing: the
    // branch cut at its bore (inside surface) on the header outside surface. Any other declaration is refused,
    // because the opening of a branch cut at its outside diameter would be as wide as the branch: it would
    // pass through the hole.
    match joint {
        None => {}
        Some("set_on") if hsref != SET_ON_HEADER_SURFACE || bsref != SET_ON_BRANCH_SURFACE => {
            return err_output(
                req,
                b,
                &rq,
                uem::out_of_range(
                    crate::refusal::JOINT_PAIRING_INVALID,
                    "joint",
                    vec![
                        "joint",
                        "header_surface_reference",
                        "branch_surface_reference",
                    ],
                    format!(
                        "a set-on joint (Spec 02 section 4.4) needs the branch INSIDE surface on the header OUTSIDE surface: branch_surface_reference \"{SET_ON_BRANCH_SURFACE}\" and header_surface_reference \"{SET_ON_HEADER_SURFACE}\". Declared: header \"{hsref}\", branch \"{bsref}\". A branch whose outside surface is the contact would have its opening cut at the outside diameter, so it would pass through the hole; this build has no wall thickness from which the bore could be derived"
                    ),
                ),
            );
        }
        // a set-on joint with the Spec 02 section 4.4 pairing
        Some("set_on") => {}
        Some(other) => {
            return err_output(
                req,
                b,
                &rq,
                uem::out_of_range(
                    crate::refusal::JOINT_TYPE_UNKNOWN,
                    "joint",
                    vec!["joint"],
                    format!("the joint type must be absent or \"set_on\", not \"{other}\""),
                ),
            );
        }
    }
    // BE-001: the declared surface of the contact radii must be geometrically possible for the entered
    // development radii. Checked only for radii the engine would accept at all (finite and > 0), so that
    // a negative radius is still reported as what it is. Each part is checked against its OWN surface.
    let mut scrutiny: Vec<String> = Vec::new();
    for (contact_name, contact, dev_name, dev, sref) in [
        ("header_radius", hr, "header_dev_radius", hdev, hsref),
        ("branch_radius", br, "branch_dev_radius", bdev, bsref),
    ] {
        if contact.is_finite() && contact > 0.0 && dev.is_finite() && dev > 0.0 {
            match radius_consistency(sref, contact_name, contact, dev_name, dev) {
                Err(reason) => {
                    return err_output(req, b, &rq, radius_error(reason, dev_name, contact_name));
                }
                Ok(Some(note)) => {
                    b.advisories.push(note.clone());
                    scrutiny.push(note);
                }
                Ok(None) => {}
            }
        }
    }
    // One claim per parameter, so that a change to one of them can be traced.
    let conv = b.push(Record::new(Family::Claim, "convention", json!({ "name": "surface_reference", "value": { "header": hsref, "branch": bsref }, "scope": "header contact radius and its development radius: header; branch contact radius and its development radius: branch", "joint": js.joint, "axes": { "provenance": "human_entered", "authority": "assumed" } }), vec![rel("declared_in", &rq)], None));
    let param = |b: &mut Builder, name: &str, value: f64, radius: bool| -> String {
        let mut rels = vec![rel("declared_in", &rq)];
        if radius {
            rels.push(rel("convention_of", &conv));
        }
        b.push(Record::new(Family::Claim, "parameter", json!({ "name": name, "value": value, "unit": if name == "beta_deg" { "deg" } else { "mm" }, "axes": { "provenance": "human_entered", "authority": "assumed", "validation": "candidate" } }), rels, None))
    };
    let p_hr = param(&mut b, "header_radius", hr, true);
    let p_hdev = param(&mut b, "header_dev_radius", hdev, true);
    let p_hlen = param(&mut b, "header_length", hlen, false);
    let p_st = param(&mut b, "branch_station", station, false);
    let p_br = param(&mut b, "branch_radius", br, true);
    let p_bdev = param(&mut b, "branch_dev_radius", bdev, true);
    let p_beta = param(&mut b, "beta_deg", beta_deg, false);
    let p_delta = param(&mut b, "delta", delta, false);
    let p_send = param(&mut b, "s_end", s_end, false);

    let part_h = b.push(Record::new(
        Family::Claim,
        "part",
        json!({ "name": "header" }),
        vec![
            rel("defined_by", &p_hr),
            rel("defined_by", &p_hdev),
            rel("defined_by", &p_hlen),
        ],
        None,
    ));
    let part_b = b.push(Record::new(
        Family::Claim,
        "part",
        json!({ "name": "branch" }),
        vec![
            rel("defined_by", &p_br),
            rel("defined_by", &p_bdev),
            rel("defined_by", &p_send),
        ],
        None,
    ));

    let geo = Branch {
        big_r: hr,
        r: br,
        beta: beta_deg.to_radians(),
        delta,
    };
    let mut branch_pat = match develop::branch_stub(hr, br, bdev, beta_deg, delta, s_end) {
        Ok(p) => p,
        Err(e) => return err_output(req, b, &rq, e),
    };
    let mut header_pat = match develop::header_with_opening(&geo, hdev, hlen, station) {
        Ok(p) => p,
        Err(e) => return err_output(req, b, &rq, e),
    };
    // Both parts of the joint are sampled with the same number of segments, the larger of the two that each
    // needs for its own chord error (review RB-004). Their vertices are then the same points of the intersection
    // curve, at the same branch azimuths, and the interface checks compare vertex with vertex. Without this a
    // part at 25.4 times the reference size was refused: one part was chord-sampled more finely than the other.
    let segments = |p: &Pattern| p.quantity("polyline_segments").unwrap_or(0.0) as usize;
    if segments(&branch_pat) != segments(&header_pat) {
        let n = segments(&branch_pat).max(segments(&header_pat));
        branch_pat = match develop::branch_stub_min(hr, br, bdev, beta_deg, delta, s_end, n) {
            Ok(p) => p,
            Err(e) => return err_output(req, b, &rq, e),
        };
        header_pat = match develop::header_with_opening_min(&geo, hdev, hlen, station, n) {
            Ok(p) => p,
            Err(e) => return err_output(req, b, &rq, e),
        };
    }
    // The checks of gate G10, on the final patterns: the header opening, the branch, the interface constraint
    // (fold-back) and the independent 3D check of the joint (review RB-001, RB-006), each part against the other
    // part's own geometry. Task 1.1: evaluated before anything of the joint is recorded, so that a branch angle the
    // verification cannot confirm is refused with its own code instead of a result that G10 then rejects.
    let hc = verify::verify_header_opening(&header_pat, &geo, hdev, hlen, station);
    let bc = verify::verify_branch(&branch_pat, &geo, bdev, s_end);
    let mut ic = verify::verify_interface(
        &header_pat,
        &geo,
        hdev,
        station,
        &branch_pat,
        &geo,
        bdev,
        s_end,
    );
    ic.extend(verify::verify_joint(
        &header_pat,
        &geo,
        hdev,
        station,
        &branch_pat,
        &geo,
        bdev,
        s_end,
    ));
    // task 1.4 (branch): each check is affected by the surface of the radius it uses. A check that uses both radii is
    // affected when either surface is unknown. A FAIL is never changed (`surface_unknown`).
    let either = if hsref == "unknown" || bsref == "unknown" {
        Some("unknown")
    } else {
        None
    };
    let hc = verify::surface_unknown(hc, Some(hsref), verify::HEADER_OPENING_HEADER_CHECKS);
    let hc = verify::surface_unknown(hc, either, verify::HEADER_OPENING_JOINT_CHECKS);
    let bc = verify::surface_unknown(bc, Some(hsref), verify::BRANCH_PART_HEADER_CHECKS);
    let bc = verify::surface_unknown(bc, Some(bsref), verify::BRANCH_PART_BRANCH_CHECKS);
    let bc = verify::surface_unknown(bc, either, verify::BRANCH_PART_JOINT_CHECKS);
    let ic = verify::surface_unknown(ic, either, verify::INTERFACE_JOINT_CHECKS);
    let all: Vec<Check> = hc.iter().chain(&bc).chain(&ic).cloned().collect();
    let cond = verify::header_with_branch_conditioning(&geo, hdev, hlen, station, bdev, s_end);
    if let Some(e) = verify::branch_angle_refusal(beta_deg, &cond, &all) {
        return err_output(req, b, &rq, e);
    }
    // The intersection curve is shared by both parts: it depends on both contact radii and the placement.
    let a_int = b.act(
        "build.intersect",
        "Z2",
        &[&p_hr, &p_br, &p_beta, &p_delta],
        700,
    );
    let curve = b.push(Record::new(
        Family::Claim,
        "intersection_curve",
        json!({ "between": ["header", "branch"], "s_min": branch_pat.quantity("s_min"), "s_max": branch_pat.quantity("s_max"), "formula": "Spec 02 eq. (4.3)" }),
        vec![rel("computed_from", &p_hr), rel("computed_from", &p_br), rel("computed_from", &p_beta), rel("computed_from", &p_delta)],
        Some(a_int),
    ));
    // RB-001: the joint is called set-on only when the request declares it, and then only as the Spec 02 §4.4
    // pairing. Without a declaration the same single curve is computed and it is not called set-on.
    let joint_label = if joint == Some("set_on") {
        "set-on (Spec 02 section 4.4: branch inside surface on header outside surface; the opening is the branch bore)"
    } else {
        "not declared (the branch end cut and the header opening are cut on one curve of the entered radii)"
    };
    let interface = b.push(Record::new(
        Family::Claim,
        "interface",
        json!({ "name": "header-branch joint", "joint": joint_label, "declared": js.joint, "header_surface": hsref, "branch_surface": bsref, "constraint": "branch end cut and header opening are the same intersection curve" }),
        vec![rel("mates_at", &part_h), rel("mates_at", &part_b), rel("interface_feature", &curve), rel("positioned_by", &p_st)],
        None,
    ));

    let (pat_h, _) = b.part(
        "header",
        &header_pat,
        vec![
            rel("developed_from", &curve),
            rel("developed_from", &p_hdev),
            rel("developed_from", &p_hlen),
            rel("developed_from", &p_st),
            rel("pattern_of", &part_h),
        ],
        hc,
        Some(&js),
        Frame::Header { delta, beta_deg },
    );
    let (pat_b, _) = b.part(
        "branch",
        &branch_pat,
        vec![
            rel("developed_from", &curve),
            rel("developed_from", &p_bdev),
            rel("developed_from", &p_send),
            rel("pattern_of", &part_b),
        ],
        bc,
        Some(&js),
        Frame::Branch {
            delta,
            beta_deg,
            s_end,
        },
    );

    // Interface constraint (`ic`, computed above from the two finished patterns).
    let a_if = b.act(
        "verify.interface",
        "Z3",
        &[&pat_h, &pat_b, &interface],
        3000,
    );
    let views: Vec<CheckView> = ic.iter().map(|c| check_view("interface", c)).collect();
    let ev = b.push(Record::new(
        Family::Assessment,
        "interface_evidence",
        json!({ "scope": "interface", "checks": views }),
        vec![
            rel("verifies", &interface),
            rel("uses", &pat_h),
            rel("uses", &pat_b),
        ],
        Some(a_if),
    ));
    b.checks.extend(views);
    b.all_checks.extend(ic);
    b.evaluated.extend([ev, interface]);

    let surface_note = if hsref == "unknown" || bsref == "unknown" {
        "unknown".to_string()
    } else if hsref == bsref {
        hsref.to_string()
    } else {
        format!("header {hsref}, branch {bsref}")
    };
    let mut gates = evaluate_gates(
        &b.all_checks,
        Some(b.topo_ok),
        Some(b.reimport_ok),
        b.has_polyline,
        Some(surface_note.as_str()),
    );
    // task 1.4: an INCONCLUSIVE radius check says why, in the gate that holds it as well
    if b.all_checks.iter().any(|c| c.reason.is_some())
        && let Some(g) = gates.iter_mut().find(|g| g.id == "G10")
    {
        g.detail.push(' ');
        g.detail.push_str(verify::SURFACE_UNKNOWN_REASON);
    }
    if let Some(g) = gates.iter_mut().find(|g| g.id == "G02") {
        g.detail.push_str(" Contact and development radii were checked against the declared surface of each part (Spec 02 §6.1): the direction is possible; no wall thickness is modelled, so its size is not checked beyond the half-radius advisory that may follow.");
        if joint.is_none() {
            g.detail.push_str(" Joint not declared: the saddle and the opening are cut on one curve of the entered contact radii, a zero-thickness idealisation. A set-on branch is cut at its bore: declare the joint as set-on (request value set_on) with header_surface_reference outside and branch_surface_reference inside.");
        }
        for note in &scrutiny {
            g.detail.push(' ');
            g.detail.push_str(note);
        }
    }
    b.finish(req, gates, None)
}

/// Relative difference below which two entered radii count as the same number
/// (binary64 noise only; this is not an engineering tolerance).
const SAME_RADIUS_REL: f64 = 1e-12;

/// BE-001. Geometric consistency of a contact radius, the wall surface it is
/// declared to lie on, and the development (reference-surface) radius.
///
/// Rule applied, and nothing more: Spec 02 §6.1 places the reference surface
/// inside the wall, r_dev = r_i + k·t with 0 < k <= 0.5 and thickness t >= 0,
/// and the outside radius is r_o = r_i + t. Hence r_i <= r_dev <= r_o, with
/// equality only for t = 0. Therefore:
///
///   * contact radius declared OUTSIDE: r_dev > r_contact is impossible (negative thickness) -> refused;
///   * contact radius declared INSIDE:  r_dev < r_contact is impossible -> refused;
///   * contact radius declared NEUTRAL: it IS the reference surface, so r_dev != r_contact is a contradiction -> refused;
///   * UNKNOWN: nothing can be checked -> scrutiny note (G02 stays INCONCLUSIVE).
///   * OUTSIDE with r_dev <= r_contact / 2: accepted with a scrutiny note (advisory, not a refusal).
///     With k = 0.5 the reference surface sits at (r_i + r_o) / 2 > r_o / 2 for any r_i > 0, so the
///     pair is consistent only for small k; the inner radius bounds t below (t >= 2·(r_dev − r_i))
///     but not above, so nothing further is checkable without r_o or t (an open item, section 10 G).
///
/// Equality under OUTSIDE or INSIDE means zero wall thickness. The
/// specification does not forbid an idealised zero-thickness surface, so it is
/// accepted with a scrutiny note, not refused. No thickness, k-factor or
/// plausible thickness range is assumed anywhere: the specification gives none
/// (MATH-16), so the size of the difference is not judged, except the half-radius advisory noted below for an outside development radius at most half the contact radius.
///
/// A radius that the declared surface contradicts: the rule code, the message, and the exact limit (the contact
/// radius) and the measured development radius.
#[derive(Clone, Debug, PartialEq)]
pub struct RadiusRefusal {
    pub code: &'static str,
    pub message: String,
    pub detail: Vec<(&'static str, f64)>,
}

/// Ok(None) = consistent; Ok(Some(note)) = accepted, engineer must look; Err = refused.
pub fn radius_consistency(
    sref: &str,
    contact_name: &str,
    contact: f64,
    dev_name: &str,
    dev: f64,
) -> Result<Option<String>, RadiusRefusal> {
    let same = (dev - contact).abs() <= SAME_RADIUS_REL * dev.abs().max(contact.abs());
    let zero_t = |side: &str| {
        Some(format!(
            "{dev_name} = {dev} equals the {side} contact radius {contact_name} = {contact}: that is a wall of zero thickness (an idealised surface). Thickness is not modelled; an engineer must confirm the radii before use."
        ))
    };
    // the limit of a development radius is the contact radius itself; both are the entered numbers
    let refuse = |code: &'static str, message: String| RadiusRefusal {
        code,
        message,
        detail: vec![("limit", contact), ("measured", dev)],
    };
    match sref {
        "outside" if same => Ok(zero_t("outside")),
        "outside" if dev > contact => Err(refuse(
            crate::refusal::RADIUS_DEV_EXCEEDS_OUTSIDE,
            format!(
                "inconsistent with surface_reference 'outside': {dev_name} = {dev} is larger than the outside contact radius {contact_name} = {contact}. The development (reference) surface lies inside the wall (Spec 02 §6.1: r_dev = r_i + k·t, 0 < k <= 0.5), so it cannot be larger than the outside radius; these values would mean a negative wall thickness. Correct the radii or the declared surface"
            ),
        )),
        "outside" if 2.0 * dev <= contact => Ok(Some(format!(
            "{dev_name} = {dev} is at most half the outside contact radius {contact_name} = {contact}: the reference surface r_dev = r_i + k*t, 0 < k <= 0.5 (Spec 02 §6.1) then needs an inner radius below {dev} and a wall thicker than {contact} - {dev}, and a neutral surface at mid-thickness (k = 0.5) is impossible; the radii are consistent only for k <= {k_max}. Thickness is not modelled and no k is assumed; an engineer must confirm the radii before use.",
            k_max = dev / contact
        ))),
        "outside" => Ok(None),
        "inside" if same => Ok(zero_t("inside")),
        "inside" if dev < contact => Err(refuse(
            crate::refusal::RADIUS_DEV_BELOW_INSIDE,
            format!(
                "inconsistent with surface_reference 'inside': {dev_name} = {dev} is smaller than the inside contact radius {contact_name} = {contact}. The development (reference) surface lies inside the wall (Spec 02 §6.1: r_dev = r_i + k·t, 0 < k <= 0.5), so it cannot be smaller than the inside radius; these values would mean a negative wall thickness. Correct the radii or the declared surface"
            ),
        )),
        "inside" => Ok(None),
        "neutral" if same => Ok(None),
        "neutral" => Err(refuse(
            crate::refusal::RADIUS_NEUTRAL_MISMATCH,
            format!(
                "inconsistent with surface_reference 'neutral': the contact radius {contact_name} = {contact} is declared to be on the reference surface itself, so {dev_name} must equal it, but it is {dev}. Correct the radii or the declared surface"
            ),
        )),
        _ => Ok(Some(format!(
            "The relation between {contact_name} = {contact} and {dev_name} = {dev} could not be checked because the surface reference is unknown."
        ))),
    }
}

/// The engine error of a refused radius: the field of the development radius is named first, the contact radius second.
fn radius_error(r: RadiusRefusal, dev_name: &'static str, contact_name: &'static str) -> UemError {
    uem::out_of_range_with(
        r.code,
        dev_name,
        vec![dev_name, contact_name],
        r.message,
        r.detail,
    )
}

/// Largest magnitude (mm) and smallest positive dimension (mm) this build
/// accepts. These are implementation limits, not engineering rules: they are
/// the range the stress campaign exercises (crates/nf-core/tests/stress.rs),
/// and Spec 02 §9 requires `OutOfRange` outside the range that was shown.
pub const MAX_MAGNITUDE_MM: f64 = 1e8;
pub const MIN_DIMENSION_MM: f64 = 1e-6;
const LENGTH_PARAMS: [&str; 14] = [
    "r_dev",
    "length",
    "r1",
    "r2",
    "h",
    "e",
    "header_radius",
    "header_dev_radius",
    "header_length",
    "branch_station",
    "branch_radius",
    "branch_dev_radius",
    "delta",
    "s_end",
];
/// Parameters that may legitimately be zero or negative (offsets and positions).
const SIGNED_PARAMS: [&str; 3] = ["e", "branch_station", "delta"];

/// BE-015: refuse magnitudes outside the range this build has been exercised in.
fn magnitude_check(req: &Request) -> Result<(), UemError> {
    let v = serde_json::to_value(req).unwrap_or(Value::Null);
    for name in LENGTH_PARAMS {
        let Some(raw) = v.get(name) else { continue };
        // a non-finite number serialises as null; the engine reports those itself
        let Some(x) = raw.as_f64() else { continue };
        if x.abs() > MAX_MAGNITUDE_MM {
            return Err(uem::out_of_range_with(
                crate::refusal::MAGNITUDE_ABOVE_LIMIT,
                name,
                vec![name],
                format!(
                    "magnitude {x:e} mm exceeds {MAX_MAGNITUDE_MM:e} mm, the largest size this build has been exercised with (implementation limit, not an engineering rule)"
                ),
                vec![("limit", MAX_MAGNITUDE_MM), ("measured", x.abs())],
            ));
        }
        if x > 0.0 && x < MIN_DIMENSION_MM && !SIGNED_PARAMS.contains(&name) {
            return Err(uem::out_of_range_with(
                crate::refusal::DIMENSION_BELOW_LIMIT,
                name,
                vec![name],
                format!(
                    "{x:e} mm is below {MIN_DIMENSION_MM:e} mm, the smallest dimension this build has been exercised with (implementation limit, not an engineering rule)"
                ),
                vec![("limit", MIN_DIMENSION_MM), ("measured", x)],
            ));
        }
    }
    Ok(())
}

/// Run a job. `entered_by` is recorded as human input data on the request.
pub fn run(req: &Request, entered_by: &str) -> JobOutput {
    if let Err(e) = magnitude_check(req) {
        let mut b = Builder::new();
        let rq = b.push(request_claim(req, entered_by));
        return err_output(req, b, &rq, e);
    }
    // task 1.4: a declared surface of the cylinder shell, cone or eccentric reducer must be one of the four words
    // (the branch families check theirs in `single` and `header_with_branch`, as before)
    if let Some(sref) = req.declared_surface()
        && !SURFACE_REFERENCES.contains(&sref)
    {
        let mut b = Builder::new();
        let rq = b.push(request_claim(req, entered_by));
        return err_output(
            req,
            b,
            &rq,
            uem::out_of_range(
                crate::refusal::SURFACE_REFERENCE_INVALID,
                "surface_reference",
                vec!["surface_reference"],
                format!("must be one of {SURFACE_REFERENCES:?}"),
            ),
        );
    }
    match req.clone() {
        Request::CylinderShell {
            r_dev,
            length,
            surface_reference,
        } => single(
            req,
            entered_by,
            "shell",
            develop::cylinder_shell(r_dev, length).map(|p| {
                let mut c = verify::verify_cylinder(&p, r_dev, length);
                // RC-09: the independent solid checks (3D end circles, sampled area) and the plate-axis
                // datum. The outline guard is already in `c`; keep one copy of it.
                let solid: Vec<_> = verify::verify_cylinder_solid(&p, r_dev, length)
                    .into_iter()
                    .filter(|s| !c.iter().any(|x| x.name == s.name))
                    .collect();
                c.extend(solid);
                (
                    p,
                    verify::surface_unknown(
                        c,
                        surface_reference.as_deref(),
                        verify::CYLINDER_RADIUS_CHECKS,
                    ),
                )
            }),
        ),
        Request::ConeFrustum {
            r1,
            r2,
            h,
            surface_reference,
        } => single(
            req,
            entered_by,
            "reducer",
            develop::cone_frustum(r1, r2, h).map(|p| {
                let c = verify::verify_cone(&p, r1, r2, h);
                (
                    p,
                    verify::surface_unknown(
                        c,
                        surface_reference.as_deref(),
                        verify::CONE_RADIUS_CHECKS,
                    ),
                )
            }),
        ),
        Request::EccentricReducer {
            r1,
            r2,
            h,
            e,
            surface_reference,
        } => single(
            req,
            entered_by,
            "reducer",
            develop::eccentric_reducer(r1, r2, h, e).map(|p| {
                let c = verify::verify_eccentric(&p, r1, r2, h, e);
                (
                    p,
                    verify::surface_unknown(
                        c,
                        surface_reference.as_deref(),
                        verify::ECCENTRIC_RADIUS_CHECKS,
                    ),
                )
            }),
        ),
        Request::BranchStub {
            header_radius,
            branch_radius,
            branch_dev_radius,
            beta_deg,
            delta,
            s_end,
            surface_reference,
        } => single(
            req,
            entered_by,
            "branch",
            develop::branch_stub(
                header_radius,
                branch_radius,
                branch_dev_radius,
                beta_deg,
                delta,
                s_end,
            )
            .and_then(|p| {
                let b = Branch {
                    big_r: header_radius,
                    r: branch_radius,
                    beta: beta_deg.to_radians(),
                    delta,
                };
                let c = verify::verify_branch(&p, &b, branch_dev_radius, s_end);
                // task 1.4 (branch): an `unknown` surface makes the radius checks that would PASS INCONCLUSIVE
                let c = verify::surface_unknown(
                    c,
                    Some(&surface_reference),
                    verify::BRANCH_STUB_RADIUS_CHECKS,
                );
                // task 1.1: a branch angle the verification cannot confirm is refused with its own code; the checks
                // read here are exactly those gate G10 reads (see `verify::branch_angle_refusal`)
                let cond = verify::branch_stub_conditioning(&b, branch_dev_radius, s_end);
                match verify::branch_angle_refusal(beta_deg, &cond, &c) {
                    Some(e) => Err(e),
                    None => Ok((p, c)),
                }
            }),
        ),
        Request::HeaderWithBranch {
            header_radius,
            header_dev_radius,
            header_length,
            branch_station,
            branch_radius,
            branch_dev_radius,
            beta_deg,
            delta,
            s_end,
            surface_reference,
            header_surface_reference,
            branch_surface_reference,
            joint,
        } => header_with_branch(
            req,
            entered_by,
            [
                header_radius,
                header_dev_radius,
                header_length,
                branch_station,
                branch_radius,
                branch_dev_radius,
                beta_deg,
                delta,
                s_end,
            ],
            // A part without its own surface takes the shared one. Both absent is refused by `parse_strict`; a
            // request built in code with both absent is read as "unknown", which never claims a surface.
            (
                header_surface_reference
                    .as_deref()
                    .or(surface_reference.as_deref())
                    .unwrap_or("unknown"),
                branch_surface_reference
                    .as_deref()
                    .or(surface_reference.as_deref())
                    .unwrap_or("unknown"),
            ),
            joint.as_deref(),
        ),
    }
}

/// Recompute from the request alone and compare closure hash and record ids.
pub fn replay_matches(
    req: &Request,
    entered_by: &str,
    expected_closure_hash: &str,
    expected_ids: &[String],
) -> (bool, String) {
    let again = run(req, entered_by);
    let ids: Vec<String> = again.records.iter().map(|r| r.id.clone()).collect();
    (
        again.closure_hash == expected_closure_hash && ids == expected_ids,
        again.closure_hash,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn assembly() -> Request {
        Request::HeaderWithBranch {
            header_radius: 150.0,
            header_dev_radius: 150.0,
            header_length: 1200.0,
            branch_station: 600.0,
            branch_radius: 100.0,
            branch_dev_radius: 100.0,
            beta_deg: 60.0,
            delta: 0.0,
            s_end: 400.0,
            surface_reference: Some("outside".into()),
            header_surface_reference: None,
            branch_surface_reference: None,
            joint: None,
        }
    }

    fn reqs() -> Vec<Request> {
        vec![
            Request::CylinderShell {
                r_dev: 100.0,
                length: 250.0,
                surface_reference: None,
            },
            Request::ConeFrustum {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
                surface_reference: None,
            },
            Request::EccentricReducer {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
                e: 100.0,
                surface_reference: None,
            },
            Request::BranchStub {
                header_radius: 150.0,
                branch_radius: 100.0,
                branch_dev_radius: 100.0,
                beta_deg: 60.0,
                delta: 0.0,
                s_end: 400.0,
                surface_reference: "unknown".into(),
            },
            assembly(),
        ]
    }

    #[test]
    fn jobs_run_verify_and_never_approve() {
        for r in reqs() {
            let o = run(&r, "tester");
            assert!(
                !o.patterns.is_empty() && o.dxf.len() == o.patterns.len(),
                "{r:?}"
            );
            // the eccentric reducer's circumference checks are INCONCLUSIVE by design (its eps_rep is not
            // certified); since RC-10 its drawn-area check is too (the chord-deficit allowance is eps_rep
            // times the perimeter, so rep_check downgrades it the same way)
            let reducer = o.patterns.iter().any(|p| p.family == "eccentric_reducer");
            // task 1.4 (engine 0.4.6): a branch stub with an `unknown` surface has its radius checks INCONCLUSIVE, with the reason
            let branch_unknown = matches!(&r, Request::BranchStub { surface_reference, .. } if surface_reference == "unknown");
            assert!(
                o.checks.iter().all(|c| c.verdict == "PASS"
                    || (reducer
                        && c.verdict == "INCONCLUSIVE"
                        && (c.name.ends_with("_equals_circumference") || c.name == "area"))
                    || (branch_unknown
                        && c.verdict == "INCONCLUSIVE"
                        && c.reason.as_deref() == Some(uem::verify::SURFACE_UNKNOWN_REASON))),
                "{r:?}"
            );
            assert_eq!(
                o.state,
                ReleaseState::ReviewRequired,
                "this build must never reach APPROVED"
            );
            assert!(
                o.gates
                    .iter()
                    .any(|g| g.id == "G16" && g.result == GateResult::NotImplemented)
            );
            assert!(o.records.iter().all(|x| x.verify_id()));
            let pass = |id: &str| {
                o.gates
                    .iter()
                    .any(|g| g.id == id && g.result == GateResult::Pass)
            };
            // G10 is INCONCLUSIVE for a reducer (its circumference and drawn-area checks, see above), PASS otherwise
            let g10 = o.gates.iter().find(|g| g.id == "G10").map(|g| g.result);
            let g10_want = if reducer || branch_unknown {
                GateResult::Inconclusive
            } else {
                GateResult::Pass
            };
            assert!(pass("G11") && pass("G15") && g10 == Some(g10_want), "{r:?}");
        }
    }

    #[test]
    fn assembly_has_two_parts_interface_evidence_and_opening() {
        let o = run(&assembly(), "t");
        assert_eq!(
            o.patterns
                .iter()
                .map(|p| p.part.as_str())
                .collect::<Vec<_>>(),
            ["header", "branch"]
        );
        assert_eq!(o.patterns[0].holes.len(), 1);
        assert_eq!(
            o.checks.iter().filter(|c| c.scope == "interface").count(),
            5 // three fold-back checks (the two 3D cross-checks, and since D1 the common marking side) and,
              // since review RB-001, the two independent 3D point checks of the joint (verify_joint)
        );
        assert!(o.dxf[0].1.contains("CUT_INNER"));
        assert!(o.gates.iter().any(|g| g.id == "G02"));
        let unknown = run(&assembly().with_param("delta", 0.0).unwrap(), "t");
        assert_eq!(unknown.closure_hash, o.closure_hash);
    }

    #[test]
    fn deterministic_replay() {
        for r in reqs() {
            let a = run(&r, "tester");
            let ids: Vec<String> = a.records.iter().map(|x| x.id.clone()).collect();
            assert!(
                replay_matches(&r, "tester", &a.closure_hash, &ids).0,
                "{r:?}"
            );
            assert_eq!(a.dxf, run(&r, "tester").dxf);
        }
        let a = run(&reqs()[1], "tester");
        assert_ne!(
            a.closure_hash,
            run(
                &Request::ConeFrustum {
                    r1: 200.0,
                    r2: 100.0,
                    h: 300.5,
                    surface_reference: None
                },
                "tester"
            )
            .closure_hash
        );
        assert_ne!(
            a.closure_hash,
            run(&reqs()[1], "someone else").closure_hash,
            "who entered the request is part of the record"
        );
    }

    #[test]
    fn unsupported_and_degenerate_inputs_give_no_dxf() {
        // B2-005: was (200, 199.99, 300), refused by sin(alpha) < 1e-3; refused now by the apex radius (1e8 mm)
        let o = run(
            &Request::ConeFrustum {
                r1: 1e6,
                r2: 1e6 - 1.0,
                h: 1e6,
                surface_reference: None,
            },
            "t",
        );
        assert!(o.dxf.is_empty() && o.patterns.is_empty());
        // BE-003: this used to expect REVIEW_REQUIRED. A refusal is not a result under review.
        assert_eq!(o.state, ReleaseState::Refused);
        assert!(o.records.iter().any(|r| r.family == Family::Finding));
        assert!(
            run(&assembly().with_param("branch_station", 10.0).unwrap(), "t")
                .dxf
                .is_empty(),
            "opening outside the shell"
        );
        let mut bad = assembly();
        if let Request::HeaderWithBranch {
            surface_reference, ..
        } = &mut bad
        {
            *surface_reference = Some("whatever".into());
        }
        assert!(run(&bad, "t").dxf.is_empty());
    }

    #[test]
    fn failed_verification_rejects() {
        let p = develop::cone_frustum(206.0, 100.0, 300.0).unwrap();
        let checks = verify::verify_cone(&p, 200.0, 100.0, 300.0);
        assert_eq!(
            crate::release_state(&evaluate_gates(
                &checks,
                Some(true),
                Some(true),
                false,
                None
            )),
            ReleaseState::Rejected
        );
        let good = verify::verify_cone(
            &develop::cone_frustum(200.0, 100.0, 300.0).unwrap(),
            200.0,
            100.0,
            300.0,
        );
        assert_eq!(
            crate::release_state(&evaluate_gates(&good, Some(true), Some(false), false, None)),
            ReleaseState::Rejected,
            "failed re-import must reject"
        );
        assert_eq!(
            crate::release_state(&evaluate_gates(&good, Some(false), Some(true), false, None)),
            ReleaseState::Rejected,
            "invalid profile must reject"
        );
    }

    fn with_radii(sref: &str, hr: f64, hdev: f64, br: f64, bdev: f64) -> Request {
        Request::HeaderWithBranch {
            header_radius: hr,
            header_dev_radius: hdev,
            header_length: 1200.0,
            branch_station: 600.0,
            branch_radius: br,
            branch_dev_radius: bdev,
            beta_deg: 60.0,
            delta: 0.0,
            s_end: 400.0,
            surface_reference: Some(sref.into()),
            header_surface_reference: None,
            branch_surface_reference: None,
            joint: None,
        }
    }

    /// BE-001: a declared surface that the radii contradict is refused; nothing is drawn.
    #[test]
    fn be001_radii_inconsistent_with_declared_surface_are_refused() {
        let refused = |r: Request, field: &str, word: &str| {
            let o = run(&r, "t");
            assert_eq!(o.state, ReleaseState::Refused, "{r:?}");
            assert!(o.patterns.is_empty() && o.dxf.is_empty(), "{r:?}");
            assert_eq!(o.refused_field.as_deref(), Some(field), "{r:?}");
            assert!(
                o.outcome
                    .as_deref()
                    .is_some_and(|m| m.contains(word) && m.contains("Spec 02")
                        || m.contains("reference surface itself")),
                "{:?}",
                o.outcome
            );
        };
        // the audit's case: outside 150 with development 160 (negative wall thickness)
        refused(
            with_radii("outside", 150.0, 160.0, 100.0, 110.0),
            "header_dev_radius",
            "negative wall thickness",
        );
        refused(
            with_radii("outside", 150.0, 146.0, 100.0, 100.5),
            "branch_dev_radius",
            "negative wall thickness",
        );
        refused(
            with_radii("inside", 146.0, 142.0, 96.0, 100.0),
            "header_dev_radius",
            "negative wall thickness",
        );
        refused(
            with_radii("inside", 142.0, 146.0, 100.0, 96.0),
            "branch_dev_radius",
            "negative wall thickness",
        );
        refused(
            with_radii("neutral", 150.0, 150.5, 100.0, 100.0),
            "header_dev_radius",
            "reference surface itself",
        );
        refused(
            with_radii("neutral", 150.0, 150.0, 100.0, 99.0),
            "branch_dev_radius",
            "reference surface itself",
        );
        // the unit rule itself
        assert!(radius_consistency("outside", "c", 150.0, "d", 160.0).is_err());
        assert!(radius_consistency("inside", "c", 150.0, "d", 140.0).is_err());
        assert!(radius_consistency("neutral", "c", 150.0, "d", 150.0 + 1e-6).is_err());
        assert_eq!(
            radius_consistency("outside", "c", 150.0, "d", 146.0),
            Ok(None)
        );
        assert_eq!(
            radius_consistency("inside", "c", 142.0, "d", 146.0),
            Ok(None)
        );
        assert_eq!(
            radius_consistency("neutral", "c", 150.0, "d", 150.0),
            Ok(None)
        );
        // review B2-001: OUTSIDE with development radius at most half the contact radius is an advisory, never a refusal
        let adv = radius_consistency("outside", "c", 50.0, "d", 0.5)
            .expect("advisory, not a refusal")
            .expect("advisory note");
        assert!(
            adv.contains("mid-thickness (k = 0.5) is impossible"),
            "{adv}"
        );
        assert!(matches!(
            radius_consistency("outside", "c", 100.0, "d", 50.0),
            Ok(Some(_))
        ));
        assert_eq!(
            radius_consistency("outside", "c", 100.0, "d", 50.0 + 1e-6),
            Ok(None)
        );
        // inside: documented as undecidable without r_o or t, so unchanged (no advisory, no refusal)
        assert_eq!(radius_consistency("inside", "c", 0.5, "d", 50.0), Ok(None));
    }

    /// BE-001: consistent declarations still compute; what cannot be decided is raised for scrutiny, not guessed.
    #[test]
    fn be001_consistent_radii_compute_and_undecidable_cases_raise_scrutiny() {
        let g02 = |r: Request| {
            let o = run(&r, "t");
            assert_eq!(o.state, ReleaseState::ReviewRequired, "{r:?}");
            assert_eq!(o.patterns.len(), 2);
            let g = o.gates.iter().find(|g| g.id == "G02").expect("G02").clone();
            assert_eq!(
                g.result,
                GateResult::Inconclusive,
                "the declaration is never a PASS"
            );
            g.detail
        };
        assert!(!g02(with_radii("outside", 150.0, 146.0, 100.0, 96.0)).contains("zero thickness"));
        assert!(!g02(with_radii("inside", 142.0, 146.0, 92.0, 96.0)).contains("zero thickness"));
        assert!(!g02(with_radii("neutral", 150.0, 150.0, 100.0, 100.0)).contains("zero thickness"));
        // equality under outside / inside: zero wall thickness is not forbidden by the specification, so it is flagged, not refused
        let d = g02(with_radii("outside", 150.0, 150.0, 100.0, 100.0));
        assert!(
            d.contains("header_dev_radius = 150")
                && d.contains("branch_dev_radius = 100")
                && d.matches("zero thickness").count() == 2,
            "{d}"
        );
        assert!(
            g02(with_radii("inside", 150.0, 150.0, 100.0, 104.0))
                .matches("zero thickness")
                .count()
                == 1
        );
        assert!(
            g02(with_radii("unknown", 150.0, 160.0, 100.0, 90.0)).contains("could not be checked")
        );
        // the single branch stub cannot declare a surface: open ambiguity, stated
        let stub = run(&reqs()[3], "t");
        assert!(stub.gates.iter().any(|g| g.id == "G02"
            && g.result == GateResult::Inconclusive
            && g.detail.contains("declared UNKNOWN")
            && g.detail.contains("could not be checked")));
        // a negative radius is still reported as a negative radius, not as an inconsistency
        let neg = run(&with_radii("outside", -150.0, 150.0, 100.0, 100.0), "t");
        assert_eq!(
            neg.refused_field.as_deref(),
            Some("header_radius"),
            "{:?}",
            neg.outcome
        );
        // review B2-001 advisory: a normal part does not carry it; the independent reproductions do
        assert!(!g02(with_radii("outside", 150.0, 146.0, 100.0, 96.0)).contains("mid-thickness"));
        let adv = "mid-thickness (k = 0.5) is impossible";
        // built from literal requests, not from with_radii, so the arithmetic is not shared with the rule
        let repro1 = Request::BranchStub {
            header_radius: 100.0,
            branch_radius: 50.0,
            branch_dev_radius: 0.5,
            beta_deg: 60.0,
            delta: 0.0,
            s_end: 200.0,
            surface_reference: "outside".into(),
        };
        let repro2 = Request::HeaderWithBranch {
            header_radius: 100.0,
            header_dev_radius: 0.5,
            header_length: 400.0,
            branch_station: 150.0,
            branch_radius: 50.0,
            branch_dev_radius: 50.0,
            beta_deg: 60.0,
            delta: 10.0,
            s_end: 200.0,
            surface_reference: Some("outside".into()),
            header_surface_reference: None,
            branch_surface_reference: None,
            joint: None,
        };
        for r in [repro1, repro2] {
            let o = run(&r, "t");
            assert_eq!(o.state, ReleaseState::ReviewRequired, "{r:?}");
            let d = o
                .gates
                .iter()
                .find(|g| g.id == "G02")
                .expect("G02")
                .detail
                .clone();
            assert!(d.contains(adv), "{r:?}: {d}");
            // Q1(i): the advisory is on the job itself (engine_advisories), not only in the G02 text
            assert!(
                o.engine_advisories.iter().any(|a| a.contains(adv)),
                "{r:?}: {:?}",
                o.engine_advisories
            );
            let json = serde_json::to_value(&o).expect("json");
            assert!(
                json["engine_advisories"].as_array().is_some_and(|a| a
                    .iter()
                    .any(|x| x.as_str().is_some_and(|t| t.contains(adv)))),
                "the advisory is part of the serialised job"
            );
        }
        // a part whose radii carry no advisory has none on the job
        let plain = run(&with_radii("outside", 150.0, 146.0, 100.0, 96.0), "t");
        assert!(
            plain.engine_advisories.is_empty(),
            "{:?}",
            plain.engine_advisories
        );
    }

    /// B2-005 boundary: a cone 1 % inside the 1e8 mm apex-radius limit is accepted AND its own independent area check passes
    /// (the area was (phi/2)(l1^2 - l2^2), which lost 3e-3 mm^2 to cancellation at this size and failed the 1e-3 mm^2 check).
    #[test]
    fn b2_005_cone_one_percent_inside_the_apex_limit_is_verified() {
        let h = 998.9994994993741; // r1 = 1e5, r2 = 99999: apex radius 9.99e7 mm
        let o = run(
            &Request::ConeFrustum {
                r1: 100000.0,
                r2: 99999.0,
                h,
                surface_reference: None,
            },
            "t",
        );
        assert_eq!(
            o.state,
            ReleaseState::ReviewRequired,
            "{:?}",
            o.checks
                .iter()
                .filter(|c| c.verdict != "PASS")
                .collect::<Vec<_>>()
        );
        assert!(o.checks.iter().all(|c| c.verdict == "PASS"));
        let area = o.patterns[0].quantities["area"];
        let s = h.hypot(1.0);
        let want = std::f64::consts::PI * (1e5 + 99999.0) * s;
        assert!((area - want).abs() <= 1e-3, "{area} vs {want}");
        // 1 % outside: refused
        let out = run(
            &Request::ConeFrustum {
                r1: 100000.0,
                r2: 99999.0,
                h: 1000.9995,
                surface_reference: None,
            },
            "t",
        );
        assert_eq!(out.state, ReleaseState::Refused);
    }

    /// Q2(b): every cone and reducer reports its flat-pattern seam gap and the gap below which the profile gate
    /// cannot say PASS (cone: max(1e-9, 2 eps_num); reducer: max(1e-9, 2 eps_rep, 2 eps_num)); a gap below that
    /// resolution makes G11 INCONCLUSIVE, never PASS.
    #[test]
    fn q2b_seam_gap_is_a_quantity_and_an_unresolved_seam_is_not_a_pass() {
        let g11 = |o: &JobOutput| o.gates.iter().find(|g| g.id == "G11").expect("G11").clone();
        // right cone: exact arcs and straight seam lines, gap l2 (2 pi - Phi) to the numerical resolution only
        let cone = run(
            &Request::ConeFrustum {
                r1: 300.0,
                r2: 50.0,
                h: 0.01,
                surface_reference: None,
            },
            "t",
        );
        let q = &cone.patterns[0].quantities;
        // cancellation-free: 1 - sin a = h^2 / (s (s + d)); the engine's own gap carries l2 * 2 pi * u of rounding
        let (d, hyp) = (250.0f64, 0.01f64.hypot(250.0));
        let gap = (50.0 * hyp / d) * 2.0 * std::f64::consts::PI * (0.01 * 0.01 / (hyp * (hyp + d)));
        assert!(
            (q["seam_gap"] - gap).abs() <= 1e-13,
            "{} vs {gap}",
            q["seam_gap"]
        );
        // resolution = max(1e-9 mm, 2 eps_num): at this size 2 eps_num is far below the floor
        let p0 = &cone.patterns[0];
        assert_eq!(
            q["seam_gap_resolution"],
            uem::develop::SEAM_RESOLUTION_MM.max(2.0 * p0.eps_num)
        );
        assert_eq!(g11(&cone).result, GateResult::Pass, "{}", g11(&cone).detail);
        // large cone (apex radius about 1e7 mm): 2 eps_num is about 1.5e-7 mm, far above 1e-9 mm. A seam gap of
        // 1e-7 mm (between the two) cannot be told from a closed seam: G11 INCONCLUSIVE, never PASS; a gap of
        // 3e-7 mm (twice 2 eps_num) is above the resolution: G11 PASS. (h from the mpmath tier.)
        let big = |h: f64| {
            run(
                &Request::ConeFrustum {
                    r1: 1.0e7,
                    r2: 9.0e6,
                    h,
                    surface_reference: None,
                },
                "t",
            )
        };
        let (below, above) = (big(0.05947080387175906), big(0.10300645387285069));
        for (o, want) in [(&below, 1e-7), (&above, 3e-7)] {
            let p = &o.patterns[0];
            let res = p.quantities["seam_gap_resolution"];
            assert_eq!(res, uem::develop::SEAM_RESOLUTION_MM.max(2.0 * p.eps_num));
            assert!(
                res > 1e-8,
                "the floor is derived from eps_num, not 1e-9: {res}"
            );
            assert!(
                (p.quantities["seam_gap"] - want).abs() <= 0.2 * want,
                "{}",
                p.quantities["seam_gap"]
            );
        }
        assert!(
            below.patterns[0].quantities["seam_gap"]
                < below.patterns[0].quantities["seam_gap_resolution"]
        );
        assert_eq!(
            g11(&below).result,
            GateResult::Inconclusive,
            "{}",
            g11(&below).detail
        );
        // the product may reject it on G15 (the DXF cannot represent the seam), never approve it
        assert!(below.state != ReleaseState::Approved, "{:?}", below.state);
        assert!(
            above.patterns[0].quantities["seam_gap"]
                > above.patterns[0].quantities["seam_gap_resolution"]
        );
        assert_eq!(
            g11(&above).result,
            GateResult::Pass,
            "{}",
            g11(&above).detail
        );
        // reducer, e = 0 (the same sector as a polyline): gap 3e-3 mm is below 2 eps_rep (~5.7e-3 mm)
        let h_small = 1.0925510388562152; // seam gap 3e-3 mm (mpmath)
        let h_large = 4.460487756959022; // seam gap 5e-2 mm (mpmath)
        let red = |h: f64| {
            run(
                &Request::EccentricReducer {
                    r1: 300.0,
                    r2: 50.0,
                    h,
                    e: 0.0,
                    surface_reference: None,
                },
                "t",
            )
        };
        let (small, large) = (red(h_small), red(h_large));
        for (o, want_gap) in [(&small, 3e-3), (&large, 5e-2)] {
            let p = &o.patterns[0];
            assert!(
                (p.quantities["seam_gap"] - want_gap).abs() <= 1e-3 * want_gap,
                "{} vs {want_gap}",
                p.quantities["seam_gap"]
            );
            assert!(
                (p.quantities["seam_gap_resolution"]
                    - (2.0 * p.eps_rep)
                        .max(2.0 * p.eps_num)
                        .max(uem::develop::SEAM_RESOLUTION_MM))
                .abs()
                    <= 1e-15,
                "the resolution is the largest of 1e-9, 2 eps_rep, 2 eps_num"
            );
        }
        let gs = g11(&small);
        assert_eq!(gs.result, GateResult::Inconclusive, "{}", gs.detail);
        assert!(
            gs.detail.contains("cannot be told from a closed one"),
            "{}",
            gs.detail
        );
        assert_eq!(small.state, ReleaseState::ReviewRequired);
        let gl = g11(&large);
        assert_eq!(gl.result, GateResult::Pass, "{}", gl.detail);
        // and the pattern is the same in both: only the verdict of the gate differs, the DXF is still produced
        assert!(!small.dxf.is_empty() && !large.dxf.is_empty());
    }

    /// BE-003: every refusal has the explicit REFUSED state, a reason, and nothing that looks like a result.
    #[test]
    fn be003_refusals_have_their_own_state() {
        let cases = [
            with_radii("outside", -150.0, 150.0, 100.0, 100.0),
            with_radii("outside", 150.0, 150.0, 400.0, 400.0),
            with_radii("whatever", 150.0, 150.0, 100.0, 100.0),
            // B2-005: was (200, 199.99, 300), refused by sin(alpha) < 1e-3; refused now by the apex radius
            Request::ConeFrustum {
                r1: 1e6,
                r2: 1e6 - 1.0,
                h: 1e6,
                surface_reference: None,
            },
            Request::ConeFrustum {
                r1: f64::NAN,
                r2: 1.0,
                h: 1.0,
                surface_reference: None,
            },
            Request::EccentricReducer {
                r1: 100.0,
                r2: 100.0,
                h: 300.0,
                e: 50.0,
                surface_reference: None,
            },
        ];
        for r in cases {
            let o = run(&r, "t");
            assert_eq!(o.state, ReleaseState::Refused, "{r:?}");
            assert!(
                o.outcome.is_some()
                    && o.patterns.is_empty()
                    && o.checks.is_empty()
                    && o.dxf.is_empty(),
                "{r:?}"
            );
            assert!(
                o.records
                    .iter()
                    .any(|x| x.kind == "gate_evaluation" && x.payload["state"] == "REFUSED")
            );
            assert!(o.records.iter().all(|x| x.verify_id()));
        }
        for r in reqs() {
            let o = run(&r, "t");
            assert!(
                o.outcome.is_none()
                    && o.refused_field.is_none()
                    && o.state == ReleaseState::ReviewRequired
            );
        }
    }

    /// BE-015: absurd magnitudes are refused instead of producing 301-digit coordinates.
    #[test]
    fn be015_magnitudes_outside_the_exercised_range_are_refused() {
        for r in [
            Request::CylinderShell {
                r_dev: 1e300,
                length: 1e300,
                surface_reference: None,
            },
            Request::CylinderShell {
                r_dev: 1e-300,
                length: 10.0,
                surface_reference: None,
            },
            Request::CylinderShell {
                r_dev: 100.0,
                length: 1e9,
                surface_reference: None,
            },
            Request::ConeFrustum {
                r1: 200.0,
                r2: 1e-7,
                h: 300.0,
                surface_reference: None,
            },
        ] {
            let o = run(&r, "t");
            assert_eq!(o.state, ReleaseState::Refused, "{r:?}");
            assert!(
                o.outcome
                    .as_deref()
                    .is_some_and(|m| m.contains("implementation limit")),
                "{:?}",
                o.outcome
            );
            assert!(o.refused_field.is_some());
        }
        assert_eq!(
            run(
                &Request::CylinderShell {
                    r_dev: 1e5,
                    length: 1e-2,
                    surface_reference: None
                },
                "t"
            )
            .state,
            ReleaseState::ReviewRequired
        );
    }

    /// BE-016: an unknown field is refused by name, never dropped.
    #[test]
    fn be016_unknown_fields_are_refused() {
        let e = Request::parse_strict(&json!({ "family": "cylinder_shell", "r_dev": 100, "length": 250, "thickness": 12, "surface_reference": "outside" })).unwrap_err();
        // task 1.4: surface_reference is a field of cylinder_shell now (it was refused here before); thickness still is not
        assert!(
            e.contains("thickness")
                && !e.contains("surface_reference")
                && e.contains("unknown field"),
            "{e}"
        );
        assert!(
            Request::parse_strict(&json!({ "family": "cylinder_shell", "r_dev": 100, "length": 250, "surface_reference": "outside" })).is_ok()
        );
        assert_eq!(
            Request::parse_strict(
                &json!({ "family": "cylinder_shell", "r_dev": 100, "length": 250 })
            )
            .unwrap(),
            reqs()[0]
        );
        assert_eq!(
            Request::parse_strict(&serde_json::to_value(assembly()).unwrap()).unwrap(),
            assembly()
        );
        assert!(Request::parse_strict(&json!({ "family": "teapot" })).is_err());
    }

    /// BH-015: the shared `surface_reference` may be left out when both part surfaces are given. Then the
    /// engine computes what it computes with the shared value given as well (the shared value is not used),
    /// and it is still required, with the same message as before, when a part surface is missing.
    #[test]
    fn bh015_part_surfaces_are_enough_and_the_shared_one_is_needed_only_when_a_part_is_missing() {
        let parts = json!({
            "family": "header_with_branch", "header_radius": 150.0, "header_dev_radius": 150.0,
            "header_length": 1200.0, "branch_station": 600.0, "branch_radius": 100.0, "branch_dev_radius": 100.0,
            "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0,
            "header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on"
        });
        let without = Request::parse_strict(&parts).expect("both part surfaces are given");
        let mut shared = parts.clone();
        shared["surface_reference"] = json!("outside");
        let with = Request::parse_strict(&shared).unwrap();
        let (a, b) = (run(&without, "t"), run(&with, "t"));
        assert_eq!(a.state, b.state);
        assert_eq!(a.outcome, b.outcome);
        assert_eq!(a.patterns.len(), 2, "both parts are computed");
        assert_eq!(
            serde_json::to_value(&a.patterns).unwrap(),
            serde_json::to_value(&b.patterns).unwrap()
        );
        assert_eq!(a.dxf, b.dxf);
        // the convention record names the part surfaces in both cases
        let conv = |o: &JobOutput| {
            o.records
                .iter()
                .find(|r| r.payload["name"] == "surface_reference")
                .map(|r| r.payload["value"].clone())
        };
        assert_eq!(
            conv(&a),
            Some(json!({ "header": "outside", "branch": "inside" }))
        );
        assert_eq!(conv(&a), conv(&b));
        // stored without the shared value, the request is read back as it was entered
        let stored = serde_json::to_value(&without).unwrap();
        assert!(stored.get("surface_reference").is_none());
        assert_eq!(serde_json::from_value::<Request>(stored).unwrap(), without);

        // a missing part surface is still refused, with the message the API has always given
        let refused = "invalid request: missing field `surface_reference`";
        let mut one_part = parts.clone();
        one_part
            .as_object_mut()
            .unwrap()
            .remove("branch_surface_reference");
        assert_eq!(Request::parse_strict(&one_part).unwrap_err(), refused);
        let mut no_parts = parts.clone();
        no_parts
            .as_object_mut()
            .unwrap()
            .remove("header_surface_reference");
        no_parts
            .as_object_mut()
            .unwrap()
            .remove("branch_surface_reference");
        assert_eq!(Request::parse_strict(&no_parts).unwrap_err(), refused);
        // an explicit null is refused as it always was
        let mut null = parts.clone();
        null["surface_reference"] = Value::Null;
        assert!(
            Request::parse_strict(&null)
                .unwrap_err()
                .contains("invalid type")
        );
        // a missing required number is still reported first, by name
        let mut no_radius = parts.clone();
        no_radius.as_object_mut().unwrap().remove("header_radius");
        no_radius
            .as_object_mut()
            .unwrap()
            .remove("surface_reference");
        assert!(
            Request::parse_strict(&no_radius)
                .unwrap_err()
                .contains("header_radius")
        );
    }

    /// DXF: every generated file carries the NOT RELEASED marking on a non-cutting layer and declares its units.
    #[test]
    fn every_dxf_is_marked_not_released_and_declares_units() {
        for r in reqs() {
            let o = run(&r, "t");
            for (part, text) in &o.dxf {
                let (all, unknown) = nf_dxf::read(text).unwrap();
                assert!(unknown.is_empty());
                assert!(all.iter().any(|e| matches!(e, Entity::Text { layer, text, .. } if layer == nf_dxf::MARK_LAYER && text == nf_dxf::MARK_TEXT)), "{part}");
                assert!(
                    all.iter()
                        .filter(|e| !e.is_annotation())
                        .all(|e| nf_dxf::is_cut_layer(e.layer())),
                    "{part}: only cut geometry on cut layers"
                );
                assert!(
                    all.iter()
                        .filter(|e| e.is_annotation())
                        .all(|e| !nf_dxf::is_cut_layer(e.layer())),
                    "{part}"
                );
                assert!(text.contains("$LUNITS") && text.contains(nf_dxf::UNITS_TEXT));
            }
            assert!(
                o.records
                    .iter()
                    .filter(|x| x.kind == "reimport_report")
                    .all(|x| x.payload["not_released_marking"] == true
                        && x.payload["units_declared"] == true
                        && x.payload["pass"] == true)
            );
        }
    }

    fn chiral_assembly() -> Request {
        Request::HeaderWithBranch {
            header_radius: 150.0,
            header_dev_radius: 150.0,
            header_length: 1200.0,
            branch_station: 500.0,
            branch_radius: 100.0,
            branch_dev_radius: 100.0,
            beta_deg: 60.0,
            delta: 20.0,
            s_end: 400.0,
            surface_reference: Some("outside".into()),
            header_surface_reference: None,
            branch_surface_reference: None,
            joint: None,
        }
    }

    /// Validation defect D1: every pattern, record, DXF and gate table states the marking side, the same for all parts.
    #[test]
    fn d1_marking_side_is_declared_everywhere_and_the_same_for_all_parts() {
        let mut all = reqs();
        all.push(chiral_assembly());
        for r in all {
            let o = run(&r, "t");
            assert_eq!(o.state, ReleaseState::ReviewRequired, "{r:?}");
            assert!(!o.patterns.is_empty());
            assert!(
                o.patterns.iter().all(|p| p.marking_side == "outside"),
                "{r:?}"
            );
            assert!(o.patterns.iter().all(|p| p.assumptions.iter().any(|a| {
                a.contains("Marking side: drawn as seen from the OUTSIDE")
                    && a.contains("Provisional")
            })));
            // mandatory scrutiny: G02 on EVERY job, never PASS, says provisional and unreviewed
            let g = o
                .gates
                .iter()
                .find(|g| g.id == "G02")
                .expect("G02 on every job");
            assert_eq!(g.result, GateResult::Inconclusive);
            assert!(
                g.critical
                    && g.detail.contains("PROVISIONAL convention")
                    && g.detail
                        .contains("not been reviewed by a fabrication engineer"),
                "{}",
                g.detail
            );
            for (part, text) in &o.dxf {
                let (ents, _) = nf_dxf::read(text).unwrap();
                let note = marking_side_note(uem::ViewSide::Outside);
                assert!(ents.iter().any(|e| matches!(e, Entity::Text { layer, text, .. } if layer == nf_dxf::MARK_LAYER && *text == note)), "{part}");
                assert!(note.contains("OUTSIDE") && note.contains("PROVISIONAL"));
            }
            assert!(
                o.records
                    .iter()
                    .filter(|x| x.kind == "pattern")
                    .all(|x| x.payload["pattern"]["marking_side"] == "outside")
            );
            assert!(
                o.records
                    .iter()
                    .filter(|x| x.kind == "reimport_report")
                    .all(|x| x.payload["marking_side_stated"] == true
                        && x.payload["extents_match"] == true)
            );
            assert!(
                o.records
                    .iter()
                    .filter(|x| x.kind == "dxf_output")
                    .all(|x| x.payload["marking"]["marking_side"]
                        .as_str()
                        .is_some_and(|m| m.contains("OUTSIDE")))
            );
        }
        // the assembly carries the cross-part check, and it passes for the chiral case
        let o = run(&chiral_assembly(), "t");
        let names: Vec<&str> = o
            .checks
            .iter()
            .filter(|c| c.scope == "interface")
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "parts_declare_the_same_marking_side",
                "branch_cut_lies_on_header_opening",
                "header_opening_lies_on_branch_cut",
                // review RB-001: the independent 3D checks of the joint, each part against the other's geometry
                "opening_on_intersection_of_branch_geometry",
                "saddle_on_intersection_of_header_geometry"
            ]
        );
        assert!(o.checks.iter().all(|c| c.verdict == "PASS"));
        assert!(o.checks.iter().any(|c| c.scope == "branch" && c.name == "saddle_handedness_matches_declared_side"));
        assert!(
            crate::REQUIRED_GATES.contains(&"G02"),
            "a stored result without the marking-side gate is incomplete"
        );
    }

    /// B2-002: a polyline is an approximation of a curved edge. It reports `points_exact: false` and the
    /// curve's own measured `eps_rep` as its deviation; only straight lines are exact with deviation 0.
    #[test]
    fn b2_002_polylines_are_labelled_approximate_with_their_eps_rep() {
        let max_chord = uem::develop::MAX_CHORD_ERROR_MM;
        // eccentric reducer (r1 = 100, r2 = 50, h = 150, e = 20): the large end and the small end are polylines
        let o = run(
            &Request::EccentricReducer {
                r1: 100.0,
                r2: 50.0,
                h: 150.0,
                e: 20.0,
                surface_reference: None,
            },
            "t",
        );
        let p = &o.patterns[0];
        let large = p.curves.iter().find(|c| c.name == "large_end").unwrap();
        let small = p.curves.iter().find(|c| c.name == "small_end").unwrap();
        for c in [large, small] {
            assert!(c.kind == "polyline" && !c.points_exact, "{}", c.name);
            assert!(c.points_max_deviation > 0.0 && c.points_max_deviation <= max_chord);
        }
        // large end: the pattern's eps_rep; small end: scaled by the homothety k = r2 / r1 (develop.rs)
        assert!((large.points_max_deviation - p.eps_rep).abs() < 1e-15);
        assert!((small.points_max_deviation - 0.5 * p.eps_rep).abs() < 1e-15);
        for c in p.curves.iter().filter(|c| c.kind == "line") {
            assert!(
                c.points_exact && c.points_max_deviation == 0.0,
                "{}",
                c.name
            );
        }

        // branch stub: saddle_cut is a polyline with the pattern's eps_rep; the three other edges are lines
        let b = run(&reqs()[3], "t");
        let bp = &b.patterns[0];
        let saddle = bp.curves.iter().find(|c| c.name == "saddle_cut").unwrap();
        assert!(saddle.kind == "polyline" && !saddle.points_exact);
        assert!(saddle.points_max_deviation > 0.0 && saddle.points_max_deviation <= max_chord);
        assert!((saddle.points_max_deviation - bp.eps_rep).abs() < 1e-15);
        assert_eq!(bp.curves.iter().filter(|c| c.kind == "line").count(), 3);

        // header with branch: the branch_opening is a polyline hole with the header's eps_rep
        let a = run(&assembly(), "t");
        let hp = a
            .patterns
            .iter()
            .find(|p| p.holes.iter().any(|h| h.name == "branch_opening"))
            .expect("header pattern with the branch opening");
        let opening = hp
            .holes
            .iter()
            .find(|h| h.name == "branch_opening")
            .unwrap();
        assert!(opening.kind == "polyline" && !opening.points_exact);
        assert!(opening.points_max_deviation > 0.0 && opening.points_max_deviation <= max_chord);
        assert!((opening.points_max_deviation - hp.eps_rep).abs() < 1e-15);
    }

    /// D1: a job whose parts are not all drawn from the declared side cannot stay reviewable.
    #[test]
    fn d1_parts_from_different_sides_fail_the_ambiguity_gate() {
        let mut b = Builder::new();
        b.sides = vec![uem::ViewSide::Outside, uem::ViewSide::Inside];
        let p = develop::cylinder_shell(100.0, 250.0).unwrap();
        let rq = b.push(request_claim(&reqs()[0], "t"));
        let checks = verify::verify_cylinder(&p, 100.0, 250.0);
        b.part(
            "shell",
            &p,
            vec![rel("developed_from", &rq)],
            checks,
            None,
            Frame::Plain,
        );
        let gates = evaluate_gates(&b.all_checks, Some(true), Some(true), false, None);
        let o = b.finish(&reqs()[0], gates, None);
        assert_eq!(o.state, ReleaseState::Rejected);
        assert!(o.gates.iter().any(|g| {
            g.id == "G02"
                && g.result == GateResult::Fail
                && g.detail
                    .contains("not all drawn from the declared marking side")
        }));
    }

    /// B2-003: the chords shown for an arc follow the sagitta tolerance; the deviation states the chords emitted.
    #[test]
    fn b2_003_arc_display_follows_the_sagitta_tolerance() {
        // the deviation of a displayed arc, measured from its points against the apex radius of the pattern
        fn measured_deviation(p: &PatternView, name: &str) -> (usize, f64, f64) {
            let arc = p.curves.iter().find(|c| c.name == name).unwrap();
            let l = p.quantities["apex_radius_large"];
            let dev = arc
                .points
                .windows(2)
                .map(|w| l - (0.5 * (w[0][0] + w[1][0])).hypot(0.5 * (w[0][1] + w[1][1])))
                .fold(0.0, f64::max);
            (arc.points.len() - 1, arc.points_max_deviation, dev)
        }
        // ordinary cone: the tolerance is met below the chord cap
        let o = run(
            &Request::ConeFrustum {
                r1: 10_000.0,
                r2: 5_000.0,
                h: 10_000.0,
                surface_reference: None,
            },
            "t",
        );
        let (n, stated, measured) = measured_deviation(&o.patterns[0], "large_end");
        assert!(n > 1 && n < super::ARC_DISPLAY_MAX_CHORDS, "{n} chords");
        assert!(
            stated <= 0.01 && (stated - measured).abs() < 1e-9,
            "{stated} {measured}"
        );
        // huge cone (B2-003 repro, apex radius 8.9e7 mm): the tolerance needs more chords than the cap, so the cap binds and the
        // stated deviation is the true sagitta of the 4096 chords shown (it was 23 946 mm with 96 chords)
        let o = run(
            &Request::ConeFrustum {
                // B2-005: was (1e8, 5e7, 1e8) = apex radius 2.2e8 mm, now above the 1e8 mm developed-size limit
                r1: 4e7,
                r2: 2e7,
                h: 4e7,
                surface_reference: None,
            },
            "t",
        );
        let (n, stated, measured) = measured_deviation(&o.patterns[0], "large_end");
        assert_eq!(n, super::ARC_DISPLAY_MAX_CHORDS);
        assert!(
            (stated - measured).abs() < 1e-6 * stated.max(1.0) && stated < 100.0,
            "stated {stated} measured {measured}"
        );
        // the chord count is the fewest that meets the tolerance (one chord fewer exceeds it)
        let r: f64 = 1e4;
        let sweep: f64 = 1.0;
        let need = super::arc_display_chords(r, sweep);
        let sag = |k: usize| r * (1.0 - (sweep / (2.0 * k as f64)).cos());
        assert!(sag(need) <= 0.01 && sag(need - 1) > 0.01, "need {need}");
    }

    /// B2-004: a header with an opening reports its gross and net plate area; net = gross - opening, and net < gross.
    /// A plain cylinder, which has no opening, keeps its `area`.
    #[test]
    fn b2_004_header_with_opening_reports_gross_and_net_area() {
        let a = run(&assembly(), "t");
        let hp = a
            .patterns
            .iter()
            .find(|p| p.holes.iter().any(|h| h.name == "branch_opening"))
            .expect("header pattern with the branch opening");
        let q = &hp.quantities;
        assert!(
            !q.contains_key("area"),
            "the header still reports a bare area"
        );
        let (gross, net, opening) = (q["gross_area"], q["net_area"], q["opening_area"]);
        // gross: developed circumference x length, both reported by the pattern
        assert!((gross - q["developed_width"] * q["length"]).abs() <= 1e-9 * gross);
        // opening: shoelace over the vertices of the hole as the JSON states them (independent of the engine sum)
        let h = hp
            .holes
            .iter()
            .find(|h| h.name == "branch_opening")
            .unwrap();
        let k = h.points.len();
        let shoelace: f64 = (0..k)
            .map(|i| {
                let (p0, p1) = (h.points[i], h.points[(i + 1) % k]);
                p0[0] * p1[1] - p1[0] * p0[1]
            })
            .sum::<f64>()
            * 0.5;
        assert!(
            (opening - shoelace.abs()).abs() <= 1e-9 * opening,
            "{opening} vs {shoelace}"
        );
        assert!((net - (gross - opening)).abs() <= 1e-9 * gross);
        assert!(net < gross && net > 0.0);

        let c = run(
            &Request::CylinderShell {
                r_dev: 100.0,
                length: 250.0,
                surface_reference: None,
            },
            "t",
        );
        let shell = &c.patterns[0].quantities;
        assert!(shell.contains_key("area") && !shell.contains_key("gross_area"));
        assert!(!shell.contains_key("net_area") && !shell.contains_key("opening_area"));
    }

    /// B2-007 (changed by task 1.2, and by its review): the eccentric reducer's eps_rep is NOT certified, because its
    /// vertex term carries the quadrature estimate; the G08 text and the assumption of the pattern say so.
    /// Review after task 1.2 (fail closed): a certified eps_rep coarser than the product limit `MAX_CHORD_ERROR_MM`
    /// (0.01 mm) must not let the checks whose tolerance uses it PASS. Branch stub R 150, r 148.5 (r/R = 0.99),
    /// beta 75 deg: certified eps_rep 0.010032 mm. Its saddle checks (handedness and declared delta side, tolerance
    /// 2·eps_rep) were PASS before the rule and are INCONCLUSIVE now. The tight near-tangent regression 011 (0.00964 mm)
    /// stays PASS.
    #[test]
    fn task_1_2_review_a_coarse_certified_bound_is_inconclusive_never_pass() {
        let v = json!({
            "family": "branch_stub",
            "header_radius": 150.0,
            "branch_radius": 148.5,
            "branch_dev_radius": 148.5,
            "beta_deg": 75.0,
            "delta": 0.0,
            "s_end": 400.0,
            "surface_reference": "outside"
        });
        let req = Request::parse_strict(&v).unwrap();
        let o = run(&req, "t");
        assert!(
            o.patterns
                .iter()
                .any(|p| p.eps_rep > uem::develop::MAX_CHORD_ERROR_MM),
            "the case must carry a certified bound above the limit"
        );
        for name in [
            "saddle_handedness_matches_declared_side",
            "saddle_on_the_declared_delta_side",
        ] {
            let c = o.checks.iter().find(|c| c.name == name).unwrap();
            assert_eq!(c.verdict, "INCONCLUSIVE", "{name}: {}", c.difference);
        }
        assert!(o.checks.iter().all(|c| c.verdict != "FAIL"));
    }

    #[test]
    fn b2_007_eps_rep_of_the_reducer_is_described_as_not_certified() {
        let o = run(
            &Request::EccentricReducer {
                r1: 100.0,
                r2: 50.0,
                h: 150.0,
                e: 20.0,
                surface_reference: None,
            },
            "t",
        );
        let g08 = o
            .gates
            .iter()
            .find(|g| g.id == "G08")
            .unwrap()
            .detail
            .clone();
        assert!(
            g08.contains("eps_num is an a-priori rounding bound")
                && g08.contains("For an eccentric reducer its quadrature term is an ESTIMATE")
                && g08.contains("For an eccentric reducer eps_rep is NOT certified")
                && !g08.contains("three interior")
                && !g08.contains("up to 6 times"),
            "{g08}"
        );
        let p = &o.patterns[0];
        assert!(p.eps_rep > 0.0);
        assert!(
            p.assumptions
                .iter()
                .any(|a| a.contains("eps_rep is NOT certified for this pattern"))
        );
        // the circumference checks of the reducer are INCONCLUSIVE, never PASS (rep_check)
        assert!(
            o.checks
                .iter()
                .filter(|c| c.name.ends_with("_equals_circumference"))
                .all(|c| c.verdict == "INCONCLUSIVE"),
            "{:?}",
            o.checks
        );
    }

    /// D7: arc points in the JSON are labelled as a display approximation with their true deviation.
    #[test]
    fn d7_arc_points_are_labelled_as_an_approximation() {
        let o = run(
            &Request::ConeFrustum {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
                surface_reference: None,
            },
            "t",
        ); // CONE-01
        let p = &o.patterns[0];
        let arc = p.curves.iter().find(|c| c.name == "large_end").unwrap();
        assert!(arc.kind == "arc" && !arc.points_exact);
        // measured: largest distance of chord midpoints from the circle of radius L1
        let l1 = p.quantities["apex_radius_large"];
        let measured = arc
            .points
            .windows(2)
            .map(|w| l1 - (0.5 * (w[0][0] + w[1][0])).hypot(0.5 * (w[0][1] + w[1][1])))
            .fold(0.0, f64::max);
        // B2-003: the chord count follows the 0.01 mm sagitta tolerance (was a fixed 96 chords, 0.034 mm here)
        assert!(
            (arc.points_max_deviation - measured).abs() < 1e-9
                && measured <= 0.01
                && (measured - 0.00996).abs() < 1e-4,
            "stated {} measured {measured}",
            arc.points_max_deviation
        );
        for c in p.curves.iter().filter(|c| c.kind != "arc") {
            assert!(c.points_exact && c.points_max_deviation == 0.0);
        }
        // B2-002: the branch's saddle_cut is a polyline, so it is an approximation (see b2_002_*);
        // only its straight seam and square-end edges are exact
        let poly = run(&reqs()[3], "t");
        assert!(
            poly.patterns[0]
                .curves
                .iter()
                .filter(|c| c.kind == "line")
                .all(|c| c.points_exact && c.points_max_deviation == 0.0)
        );
        // D3 / D4: nothing calls the estimates bounds
        let g08 = |o: &JobOutput| {
            o.gates
                .iter()
                .find(|g| g.id == "G08")
                .unwrap()
                .detail
                .clone()
        };
        // task 1.2 (review): eps_num is an a-priori bound (an estimate only for the reducer); eps_rep of the branch
        // is a certified bound; a cone without polylines states neither
        assert!(
            g08(&o).contains("eps_num is an a-priori rounding bound")
                && !g08(&o).contains("eps_rep")
                && g08(&poly).contains("eps_num is an a-priori rounding bound")
                && g08(&poly).contains("eps_rep is a certified upper bound")
        );
        assert!(
            poly.patterns[0]
                .assumptions
                .iter()
                .any(|a| a.contains("eps_rep is a certified upper bound of the chord error"))
        );
    }

    /// D2: an absurd magnitude in ANY length field is refused before anything is computed.
    #[test]
    fn d2_absurd_magnitude_in_any_field_is_refused_before_computation() {
        let mut n = 0;
        let mut all = reqs();
        all.push(chiral_assembly());
        for r in all {
            for (name, _) in r.numeric_params() {
                if name == "beta_deg" {
                    continue;
                }
                for v in [1e308, -1e308, 2e8] {
                    let o = run(&r.with_param(&name, v).unwrap(), "t");
                    assert_eq!(o.state, ReleaseState::Refused, "{name} = {v}");
                    assert_eq!(
                        o.refused_field.as_deref(),
                        Some(name.as_str()),
                        "{name} = {v}: {:?}",
                        o.outcome
                    );
                    assert!(
                        o.outcome
                            .as_deref()
                            .unwrap()
                            .contains("implementation limit")
                    );
                    // refused before computation: no development activity ran on a pattern, nothing non-finite is stored
                    assert!(o.patterns.is_empty() && o.checks.is_empty() && o.dxf.is_empty());
                    assert!(!serde_json::to_string(&o).unwrap().contains("null,\"eps"));
                    assert_eq!(o.records.iter().filter(|x| x.kind == "pattern").count(), 0);
                    n += 1;
                }
            }
        }
        assert!(
            n >= 3 * (2 + 3 + 4 + 5 + 8),
            "every length field of every family: {n}"
        );
    }

    /// BE-001 follow-up: the single branch stub can declare its surface, with the same rule as the assembly.
    #[test]
    fn be001_branch_stub_declares_its_surface() {
        let stub = |sref: &str, r: f64, dev: f64| Request::BranchStub {
            header_radius: 150.0,
            branch_radius: r,
            branch_dev_radius: dev,
            beta_deg: 60.0,
            delta: 0.0,
            s_end: 400.0,
            surface_reference: sref.into(),
        };
        for (sref, r, dev) in [
            ("outside", 100.0, 104.0),
            ("inside", 100.0, 96.0),
            ("neutral", 100.0, 100.5),
            ("whatever", 100.0, 100.0),
        ] {
            let o = run(&stub(sref, r, dev), "t");
            assert_eq!(o.state, ReleaseState::Refused, "{sref} {r}/{dev}");
        }
        assert_eq!(
            run(&stub("outside", 100.0, 104.0), "t")
                .refused_field
                .as_deref(),
            Some("branch_dev_radius")
        );
        for (sref, r, dev) in [
            ("outside", 100.0, 96.0),
            ("inside", 100.0, 104.0),
            ("neutral", 100.0, 100.0),
            ("unknown", 100.0, 110.0),
        ] {
            assert_eq!(
                run(&stub(sref, r, dev), "t").state,
                ReleaseState::ReviewRequired,
                "{sref} {r}/{dev}"
            );
        }
        let g = |o: &JobOutput| {
            o.gates
                .iter()
                .find(|g| g.id == "G02")
                .unwrap()
                .detail
                .clone()
        };
        assert!(g(&run(&stub("outside", 100.0, 100.0), "t")).contains("zero thickness"));
        // a request written before the field existed still parses, as "unknown"
        let old = Request::parse_strict(&json!({ "family": "branch_stub", "header_radius": 150, "branch_radius": 100, "branch_dev_radius": 100, "beta_deg": 60, "delta": 0, "s_end": 400 })).unwrap();
        assert_eq!(old, stub("unknown", 100.0, 100.0));
    }

    /// The set-on example of Spec 02 §4.4 (DN300 header, DN150 branch): header OUTSIDE radius 161.95 with
    /// development radius 156.795; branch INSIDE (bore) radius 77.095 with development radius 80.6.
    fn set_on_example(
        beta_deg: f64,
        delta: f64,
        joint: Option<&str>,
        hsref: Option<&str>,
        bsref: Option<&str>,
    ) -> Request {
        Request::HeaderWithBranch {
            header_radius: 161.95,
            header_dev_radius: 156.795,
            header_length: 1200.0,
            branch_station: 600.0,
            branch_radius: 77.095,
            branch_dev_radius: 80.6,
            beta_deg,
            delta,
            s_end: 400.0,
            surface_reference: Some("outside".into()),
            header_surface_reference: hsref.map(String::from),
            branch_surface_reference: bsref.map(String::from),
            joint: joint.map(String::from),
        }
    }

    fn interface_payload(o: &JobOutput) -> Value {
        o.records
            .iter()
            .find(|r| r.kind == "interface")
            .unwrap()
            .payload
            .clone()
    }

    /// Extent of the header opening along the header axis (x), from the pattern itself.
    fn opening_width_along_header(o: &JobOutput) -> f64 {
        let h = o.patterns.iter().find(|p| p.part == "header").unwrap();
        let q = |name: &str| {
            h.quantities
                .iter()
                .find(|(n, _)| n.as_str() == name)
                .unwrap()
                .1
        };
        q("opening_x_max") - q("opening_x_min")
    }

    fn g02_detail(o: &JobOutput) -> String {
        o.gates
            .iter()
            .find(|g| g.id == "G02")
            .unwrap()
            .detail
            .clone()
    }

    /// RB-001 (review B). The set-on joint of Spec 02 §4.4 is declared per part and computed: the branch is cut
    /// at its bore on the header outside surface, so the opening is as wide along the header axis as the bore
    /// (2 × 77.095 mm, not the outside diameter), and every interface check passes, including the independent
    /// 3D checks of the joint (uem verify::verify_joint, a formulation that shares no chart with develop).
    #[test]
    fn rb001_set_on_joint_is_declared_per_part_computed_and_independently_checked() {
        let o = run(
            &set_on_example(90.0, 0.0, Some("set_on"), Some("outside"), Some("inside")),
            "t",
        );
        assert_eq!(o.state, ReleaseState::ReviewRequired, "{:?}", o.outcome);
        assert_eq!(o.dxf.len(), 2, "both parts are offered");
        assert!(
            (opening_width_along_header(&o) - 2.0 * 77.095).abs() < 1e-9,
            "{}",
            opening_width_along_header(&o)
        );
        assert!(
            interface_payload(&o)["joint"]
                .as_str()
                .unwrap()
                .starts_with("set-on")
        );
        let interface: Vec<&CheckView> =
            o.checks.iter().filter(|c| c.scope == "interface").collect();
        for want in [
            "opening_on_intersection_of_branch_geometry",
            "saddle_on_intersection_of_header_geometry",
        ] {
            assert!(interface.iter().any(|c| c.name == want), "{want} missing");
        }
        assert!(
            interface.iter().all(|c| c.verdict == "PASS"),
            "{:?}",
            interface
                .iter()
                .filter(|c| c.verdict != "PASS")
                .map(|c| &c.name)
                .collect::<Vec<_>>()
        );
        // a chiral set-on joint (offset and oblique) passes as well
        let o = run(
            &set_on_example(60.0, 20.0, Some("set_on"), Some("outside"), Some("inside")),
            "t",
        );
        assert_eq!(o.state, ReleaseState::ReviewRequired, "{:?}", o.outcome);
        assert!(
            o.checks
                .iter()
                .filter(|c| c.scope == "interface")
                .all(|c| c.verdict == "PASS")
        );
    }

    /// RB-001: a set-on joint is refused unless the branch bore is its contact surface, and the field is named.
    /// A joint type that does not exist is refused too.
    #[test]
    fn rb001_set_on_is_refused_unless_the_branch_bore_is_the_contact_surface() {
        for (hs, bs) in [
            ("outside", "outside"),
            ("inside", "inside"),
            ("inside", "outside"),
        ] {
            let o = run(
                &set_on_example(90.0, 0.0, Some("set_on"), Some(hs), Some(bs)),
                "t",
            );
            assert_eq!(o.state, ReleaseState::Refused, "{hs}/{bs}");
            assert_eq!(o.refused_field.as_deref(), Some("joint"), "{hs}/{bs}");
            assert!(
                o.dxf.is_empty() && o.patterns.is_empty(),
                "{hs}/{bs}: nothing is offered"
            );
        }
        let o = run(
            &set_on_example(90.0, 0.0, Some("set_in"), Some("outside"), Some("inside")),
            "t",
        );
        assert_eq!(
            (o.state, o.refused_field.as_deref()),
            (ReleaseState::Refused, Some("joint"))
        );
        // the branch's own development radius is checked against the branch's own surface, under set-on too
        let mut bad = set_on_example(90.0, 0.0, Some("set_on"), Some("outside"), Some("inside"));
        if let Request::HeaderWithBranch {
            branch_dev_radius, ..
        } = &mut bad
        {
            *branch_dev_radius = 75.0; // below the inside radius 77.095: a negative wall
        }
        let o = run(&bad, "t");
        assert_eq!(
            (o.state, o.refused_field.as_deref()),
            (ReleaseState::Refused, Some("branch_dev_radius"))
        );
        // the bore pairing under the single 'outside' reference is refused as a negative wall of the branch
        let o = run(&set_on_example(90.0, 0.0, None, None, None), "t");
        assert_eq!(
            (o.state, o.refused_field.as_deref()),
            (ReleaseState::Refused, Some("branch_dev_radius"))
        );
    }

    /// RB-001: without a declared joint type the pairing is computed as one contact curve and is NOT called
    /// set-on. The default outside pair (both radii outside) keeps its geometry: its opening is the outside
    /// diameter, which is why it is labelled, not accepted as a set-on joint.
    #[test]
    fn rb001_undeclared_joint_is_one_contact_curve_and_is_not_called_set_on() {
        let od = Request::HeaderWithBranch {
            header_radius: 161.95,
            header_dev_radius: 156.795,
            header_length: 1200.0,
            branch_station: 600.0,
            branch_radius: 84.15,
            branch_dev_radius: 80.6,
            beta_deg: 90.0,
            delta: 0.0,
            s_end: 400.0,
            surface_reference: Some("outside".into()),
            header_surface_reference: None,
            branch_surface_reference: None,
            joint: None,
        };
        let o = run(&od, "t");
        assert_eq!(o.state, ReleaseState::ReviewRequired, "{:?}", o.outcome);
        assert!((opening_width_along_header(&o) - 2.0 * 84.15).abs() < 1e-9);
        let joint = interface_payload(&o)["joint"].as_str().unwrap().to_string();
        assert!(
            joint.starts_with("not declared") && !joint.contains("set-on"),
            "{joint}"
        );
        assert!(g02_detail(&o).contains("Joint not declared"));
        // the same pairing declared per part, without the joint type, is still one contact curve, not set-on
        let o = run(
            &set_on_example(90.0, 0.0, None, Some("outside"), Some("inside")),
            "t",
        );
        assert_eq!(o.state, ReleaseState::ReviewRequired, "{:?}", o.outcome);
        assert!(
            interface_payload(&o)["joint"]
                .as_str()
                .unwrap()
                .starts_with("not declared")
        );
    }

    /// DXF quality gate (nf-dxf quality, review item 5). A file the gate does not pass is never offered: it is
    /// withheld from the DXF list, gate G15 fails and the job is REJECTED. The pattern is deliberately broken:
    /// the opening crosses itself (a figure of eight). The re-import check passes, so the gate alone refuses it.
    #[test]
    fn dxf_quality_gate_refuses_a_broken_file_and_withholds_it() {
        let req = Request::CylinderShell {
            r_dev: 100.0,
            length: 250.0,
            surface_reference: None,
        };
        let mut pattern = uem::develop::cylinder_shell(100.0, 250.0).unwrap();
        pattern.holes = vec![uem::Curve {
            name: "branch_opening",
            layer: Layer::CutInner,
            geom: Geom::Polyline {
                pts: [
                    (40.0, 40.0),
                    (60.0, 20.0),
                    (60.0, 40.0),
                    (40.0, 20.0),
                    (40.0, 40.0),
                ]
                .iter()
                .map(|&(x, y)| uem::P2::new(x, y))
                .collect(),
                eps_rep: 0.0,
            },
        }];
        let mut b = Builder::new();
        b.part("broken", &pattern, vec![], vec![], None, Frame::Plain);
        assert!(
            !b.quality_ok && b.reimport_ok,
            "the gate refuses it; the re-import alone would not"
        );
        assert!(b.dxf.is_empty(), "the broken file is not offered");
        let report = b
            .recs
            .iter()
            .find(|r| r.kind == "dxf_quality_report")
            .unwrap();
        assert_eq!(report.payload["pass"], json!(false));
        assert!(
            report.payload["rules_failed"]
                .to_string()
                .contains("self_intersection"),
            "{}",
            report.payload
        );
        let gates = evaluate_gates(
            &b.all_checks,
            Some(b.topo_ok),
            Some(b.reimport_ok),
            b.has_polyline,
            Some("outside"),
        );
        let out = b.finish(&req, gates, None);
        assert_eq!(out.state, ReleaseState::Rejected);
        assert!(out.dxf.is_empty());
        assert!(out.gates.iter().any(|g| g.id == "G15"
            && g.result == GateResult::Fail
            && g.detail.contains("refused")));
        // a well-formed part passes the gate and is offered
        let good = run(&req, "t");
        assert_eq!(good.dxf.len(), 1);
        assert!(
            good.records
                .iter()
                .any(|r| r.kind == "dxf_quality_report" && r.payload["pass"] == json!(true))
        );
    }

    /// RC-09: the cylinder pipeline calls the independent solid verifier, so its 3D end-circle, sampled-area
    /// and plate-axis datum checks are part of the job (they were test-only before). A genuine shell passes them
    /// all; an `unknown` surface makes the radius-dependent solid checks INCONCLUSIVE like the other radius checks.
    #[test]
    fn rc09_cylinder_pipeline_runs_the_solid_and_datum_checks() {
        let req = Request::CylinderShell {
            r_dev: 100.0,
            length: 250.0,
            surface_reference: None,
        };
        let out = run(&req, "t");
        assert_eq!(out.outcome, None);
        for name in [
            "end_circle_3d_arc_length",
            "end_circles_3d_arc_length_equal",
            "area_equals_sampled_surface_area",
            "bottom_runs_along_x",
            "seam_a_runs_along_y",
        ] {
            let c = out.checks.iter().find(|c| c.name == name).expect(name);
            assert_eq!(c.verdict, "PASS", "{name}");
        }
        // one copy of the outline guard, not two
        assert_eq!(
            out.checks
                .iter()
                .filter(|c| c.name == "outline_counter_clockwise")
                .count(),
            1
        );
        let g10 = out.gates.iter().find(|g| g.id == "G10").unwrap();
        assert_eq!(serde_json::to_value(g10.result).unwrap(), json!("PASS"));
        // an unknown surface flips the radius-dependent solid checks too
        let unknown = run(
            &Request::CylinderShell {
                r_dev: 100.0,
                length: 250.0,
                surface_reference: Some("unknown".into()),
            },
            "t",
        );
        for name in [
            "end_circle_3d_arc_length",
            "end_circles_3d_arc_length_equal",
            "area_equals_sampled_surface_area",
        ] {
            let c = unknown.checks.iter().find(|c| c.name == name).expect(name);
            assert_eq!(c.verdict, "INCONCLUSIVE", "{name}");
        }
        // the datum is radius-free: it keeps its verdict under an unknown surface
        for name in ["bottom_runs_along_x", "seam_a_runs_along_y"] {
            let c = unknown.checks.iter().find(|c| c.name == name).expect(name);
            assert_eq!(c.verdict, "PASS", "{name}");
        }
    }
}

#[cfg(test)]
mod convention_withhold_tests {
    use super::*;

    /// Task 1.3 review (item 3): a part whose declared convention set is incomplete for its family, or holds a value
    /// that is not a number, fails the job's own convention check: the quality gate fails and no DXF is offered.
    #[test]
    fn an_incomplete_or_non_numeric_declaration_fails_the_gate_and_withholds_the_file() {
        let base = develop::cylinder_shell(100.0, 250.0).expect("shell");
        let checks = verify::verify_cylinder(&base, 100.0, 250.0);
        // a family with no declared key set
        let mut unknown = base.clone();
        unknown.family = "teapot";
        // a quantity the statement needs is missing, so its value is NaN
        let mut not_a_number = base.clone();
        not_a_number
            .quantities
            .retain(|(k, _)| *k != "developed_width");
        for (what, p) in [
            ("a family with no declared key set", unknown),
            ("a value that is not a number", not_a_number),
        ] {
            let mut b = Builder::new();
            let rq = b.push(request_claim(
                &Request::CylinderShell {
                    r_dev: 100.0,
                    length: 250.0,
                    surface_reference: None,
                },
                "t",
            ));
            b.part(
                "shell",
                &p,
                vec![rel("developed_from", &rq)],
                checks.clone(),
                None,
                Frame::Plain,
            );
            assert!(!b.quality_ok, "{what}: the gate must fail");
            assert!(b.dxf.is_empty(), "{what}: the file must be withheld");
            assert!(
                b.quality_notes
                    .iter()
                    .any(|n| n.contains("convention_statement")),
                "{what}: {:?}",
                b.quality_notes
            );
        }
    }
}
