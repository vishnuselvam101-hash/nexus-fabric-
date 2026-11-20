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
    ENGINE_VERSION, Family, Gate, GateResult, Record, ReleaseState, closure, closure_hash, rel,
    sha256_hex,
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
    },
    ConeFrustum {
        r1: f64,
        r2: f64,
        h: f64,
    },
    EccentricReducer {
        r1: f64,
        r2: f64,
        h: f64,
        e: f64,
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
    /// Parse a request and refuse fields this build does not know (BE-016):
    /// a silently ignored "thickness" would let an engineer believe it was applied.
    pub fn parse_strict(v: &Value) -> Result<Request, String> {
        let req: Request =
            serde_json::from_value(v.clone()).map_err(|e| format!("invalid request: {e}"))?;
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
            return Err("invalid request: missing field `surface_reference`".into());
        }
        let known = serde_json::to_value(&req).map_err(|e| e.to_string())?;
        let unknown: Vec<&str> = v
            .as_object()
            .map(|o| {
                o.keys()
                    .filter(|k| known.get(k.as_str()).is_none())
                    .map(|k| k.as_str())
                    .collect()
            })
            .unwrap_or_default();
        if !unknown.is_empty() {
            return Err(format!(
                "invalid request: unknown field(s) {} for family '{}'. Nothing is ignored silently; remove them or use a family that has them",
                unknown.join(", "),
                known["family"].as_str().unwrap_or("")
            ));
        }
        Ok(req)
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
    /// Lines and polylines: the exact vertices that are cut. Arcs: points on
    /// the arc joined by chords, FOR DISPLAY ONLY (the DXF carries the exact arc).
    pub points: Vec<[f64; 2]>,
    /// False for arcs: `points` is then an approximation of the curve.
    #[serde(default)]
    pub points_exact: bool,
    /// Largest distance between the chords through `points` and the exact
    /// curve (mm); 0 when `points_exact`. Exact value for a circular arc.
    #[serde(default)]
    pub points_max_deviation: f64,
    pub length: f64,
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
    /// ESTIMATE of the numerical error (mm), not a bound.
    pub eps_num: f64,
    /// ESTIMATE of the polyline representation error (mm), not a bound.
    pub eps_rep: f64,
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobOutput {
    pub engine: String,
    pub request: Request,
    pub state: ReleaseState,
    /// Set exactly when the engine refused the request (state REFUSED, no pattern).
    pub outcome: Option<String>,
    /// The parameter the refusal is about, where the engine names one.
    #[serde(default)]
    pub refused_field: Option<String>,
    pub gates: Vec<Gate>,
    pub patterns: Vec<PatternView>,
    pub checks: Vec<CheckView>,
    pub closure_hash: String,
    pub closure_size: usize,
    pub work_units: u64,
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
    let outer = p.outline.iter().map(|c| curve_entity(c, false));
    let inner = p.holes.iter().map(|c| curve_entity(c, true));
    outer.chain(inner).collect()
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

/// Chords used to show an arc in the JSON result. Display only.
const ARC_DISPLAY_CHORDS: usize = 96;

/// The annotation line every DXF of this build carries about its marking side.
pub fn marking_side_note(side: uem::ViewSide) -> String {
    format!(
        "MARKING SIDE: PATTERN DRAWN AS SEEN FROM THE {} OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED",
        side.code().to_uppercase()
    )
}

const MARKING_SIDE_SCRUTINY: &str = "Marking side: every pattern of this job is drawn as seen from the OUTSIDE of the formed part (pattern normal toward the outside). Spec 02 §0.3 requires a declared marking side but does not choose one; this side follows from the frames the specification writes down (§4.1, §4.4) and is a PROVISIONAL convention declared by the software. It has not been reviewed by a fabrication engineer. Marking a pattern on the other face mirrors the part (misfit of tens of millimetres for an offset, oblique branch). An engineer must confirm the marking side before use.";

fn curve_view(c: &uem::Curve) -> CurveView {
    CurveView {
        name: c.name.to_string(),
        layer: layer_name(c.layer).to_string(),
        kind: match c.geom {
            Geom::Line { .. } => "line",
            Geom::Arc { .. } => "arc",
            Geom::Polyline { .. } => "polyline",
        }
        .to_string(),
        points: profile::flatten(std::slice::from_ref(c), ARC_DISPLAY_CHORDS)
            .iter()
            .map(|q| [q.x, q.y])
            .collect(),
        points_exact: !matches!(c.geom, Geom::Arc { .. }),
        points_max_deviation: match c.geom {
            // sagitta of one chord: r·(1 − cos(sweep / 2n)), exact for a circle (validation defect D7)
            Geom::Arc { r, a0, a1, .. } => {
                r * (1.0 - ((a1 - a0).abs() / (2.0 * ARC_DISPLAY_CHORDS as f64)).cos())
            }
            _ => 0.0,
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
    /// `deps` are the typed edges from the pattern to what it was developed from.
    fn part(
        &mut self,
        part: &str,
        pattern: &Pattern,
        deps: Vec<crate::Rel>,
        checks: Vec<Check>,
    ) -> (String, String) {
        let inputs: Vec<&str> = deps.iter().map(|d| d.to.as_str()).collect();
        let a_dev = self.act("build.develop", "Z2", &inputs, 1500);
        let entities = to_entities(pattern);
        let notes = vec![marking_side_note(pattern.marking_side)];
        let text = nf_dxf::write_annotated(&entities, &notes);
        let dxf_sha = sha256_hex(text.as_bytes());
        let pv = PatternView {
            part: part.to_string(),
            family: pattern.family.to_string(),
            class: pattern.class.code().to_string(),
            marking_side: pattern.marking_side.code().to_string(),
            eps_num: pattern.eps_num,
            eps_rep: pattern.eps_rep,
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
                    "eps_num and eps_rep are estimates, not bounds: independent validation measured up to 6 times eps_num and 1.2 times the earlier eps_rep near tangency.".to_string(),
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
            json!({ "part": part, "sha256": dxf_sha, "bytes": text.len(), "entities": entities.len(), "format": "DXF AC1009 subset", "units": "mm: $LUNITS=2 (decimal), $AUNITS=0, $ANGDIR=0 in the header; the unit is stated in a comment and a TEXT annotation (R12 has no unit-of-measure variable)", "marking": { "layer": nf_dxf::MARK_LAYER, "text": nf_dxf::MARK_TEXT, "marking_side": notes[0] } }),
            vec![rel("rendered_from", &pattern_id)],
            Some(a_dxf),
        ));
        let a_re = self.act("verify.reimport", "Z3", &[&dxf_id, &pattern_id], 200);
        let re = nf_dxf::reimport_check_annotated(&entities, &text, 1e-8, &notes);
        self.sides.push(pattern.marking_side);
        self.reimport_ok &= re.pass;
        let re_id = self.push(Record::new(
            Family::Assessment,
            "reimport_report",
            json!({ "part": part, "pass": re.pass, "entities_written": re.entities_written, "entities_read": re.entities_read, "unknown_entities": re.unknown_entities, "max_point_deviation": re.max_point_deviation, "max_length_deviation": re.max_length_deviation, "layers_match": re.layers_match, "annotations_read": re.annotations_read, "not_released_marking": re.not_released_marking, "units_declared": re.units_declared, "extents_match": re.extents_match, "marking_side_stated": re.required_notes_present }),
            vec![rel("verifies", &dxf_id)],
            Some(a_re),
        ));
        // DXF quality gate (nf-dxf quality, review item 5): the file as written and re-read must pass it.
        // A file that does not pass is never offered: it is withheld from the job's DXF list and G15 fails.
        let a_q = self.act("verify.dxf_quality", "Z3", &[&dxf_id], 300);
        let quality = nf_dxf::quality::quality_gate(&text);
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

    /// `refusal` = (reason, parameter) when the engine refused the request.
    fn finish(
        mut self,
        req: &Request,
        mut gates: Vec<Gate>,
        refusal: Option<(String, Option<String>)>,
    ) -> JobOutput {
        let (outcome, refused_field) = match refusal {
            Some((why, field)) => (Some(why), field),
            None => (None, None),
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
            gates,
            patterns: self.patterns,
            checks: self.checks,
            closure_hash: closure_hash(&ids),
            closure_size: ids.len(),
            work_units: self.wu,
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
            if has_polyline {
                "eps_num and eps_rep are ESTIMATES, not bounds, and not interval-certified. Independent validation measured true errors up to 6 times eps_num near tangency; eps_rep is the largest deviation at three interior samples per polyline segment."
            } else {
                "eps_num is an ESTIMATE, not a bound, and not interval-certified."
            },
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
            "Algorithms are evidence level EV-1 (AI-generated, not independently reviewed) and not production-qualified.",
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
        json!({ "outcome": format!("{e:?}"), "message": e.to_string(), "critical": critical }),
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
    b.finish(req, gates, Some((e.to_string(), field)))
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
                UemError::OutOfRange {
                    param: "surface_reference",
                    reason: format!("must be one of {SURFACE_REFERENCES:?}"),
                },
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
                        UemError::OutOfRange {
                            param: "branch_dev_radius",
                            reason,
                        },
                    );
                }
                Ok(note) => scrutiny = note,
            }
        }
    }
    match r {
        Err(e) => err_output(req, b, &rq, e),
        Ok((p, checks)) => {
            b.part(part, &p, vec![rel("developed_from", &rq)], checks);
            let sref = match req {
                Request::BranchStub {
                    surface_reference, ..
                } => Some(surface_reference.as_str()),
                _ => None,
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
                UemError::OutOfRange {
                    param,
                    reason: format!("must be one of {SURFACE_REFERENCES:?}"),
                },
            );
        }
    }
    // RB-001: the joint type is declared or absent. A set-on joint is only the Spec 02 §4.4 pairing: the
    // branch cut at its bore (inside surface) on the header outside surface. Any other declaration is refused,
    // because the opening of a branch cut at its outside diameter would be as wide as the branch: it would
    // pass through the hole.
    match joint {
        None => {}
        Some("set_on") => {
            if hsref != SET_ON_HEADER_SURFACE || bsref != SET_ON_BRANCH_SURFACE {
                return err_output(
                    req,
                    b,
                    &rq,
                    UemError::OutOfRange {
                        param: "joint",
                        reason: format!(
                            "a set-on joint (Spec 02 section 4.4) needs the branch INSIDE surface on the header OUTSIDE surface: branch_surface_reference \"{SET_ON_BRANCH_SURFACE}\" and header_surface_reference \"{SET_ON_HEADER_SURFACE}\". Declared: header \"{hsref}\", branch \"{bsref}\". A branch whose outside surface is the contact would have its opening cut at the outside diameter, so it would pass through the hole; this build has no wall thickness from which the bore could be derived"
                        ),
                    },
                );
            }
        }
        Some(other) => {
            return err_output(
                req,
                b,
                &rq,
                UemError::OutOfRange {
                    param: "joint",
                    reason: format!("the joint type must be absent or \"set_on\", not \"{other}\""),
                },
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
                    return err_output(
                        req,
                        b,
                        &rq,
                        UemError::OutOfRange {
                            param: dev_name,
                            reason,
                        },
                    );
                }
                Ok(Some(note)) => scrutiny.push(note),
                Ok(None) => {}
            }
        }
    }
    // One claim per parameter, so that a change to one of them can be traced.
    let conv = b.push(Record::new(Family::Claim, "convention", json!({ "name": "surface_reference", "value": { "header": hsref, "branch": bsref }, "scope": "header contact radius and its development radius: header; branch contact radius and its development radius: branch", "joint": joint.unwrap_or("not declared"), "axes": { "provenance": "human_entered", "authority": "assumed" } }), vec![rel("declared_in", &rq)], None));
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
        "single contact curve (joint type not declared): the branch end cut and the header opening are cut on one curve of the entered radii"
    };
    let interface = b.push(Record::new(
        Family::Claim,
        "interface",
        json!({ "name": "header-branch joint", "joint": joint_label, "declared": joint.unwrap_or("not declared"), "header_surface": hsref, "branch_surface": bsref, "constraint": "branch end cut and header opening are the same intersection curve" }),
        vec![rel("mates_at", &part_h), rel("mates_at", &part_b), rel("interface_feature", &curve), rel("positioned_by", &p_st)],
        None,
    ));

    let hc = verify::verify_header_opening(&header_pat, &geo, hdev, hlen, station);
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
    );
    let bc = verify::verify_branch(&branch_pat, &geo, bdev, s_end);
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
    );

    // Interface constraint, evaluated from the two finished patterns: the fold-back check, and the independent
    // 3D check of the joint (review RB-001, RB-006), each part against the other part's own geometry.
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
    if let Some(g) = gates.iter_mut().find(|g| g.id == "G02") {
        g.detail.push_str(" Contact and development radii were checked against the declared surface of each part (Spec 02 §6.1): the direction is possible; no wall thickness is modelled, so its size is not checked.");
        if joint.is_none() {
            g.detail.push_str(" Joint type not declared: the saddle and the opening are cut on one curve of the entered contact radii, a zero-thickness idealisation that is not called set-on. A set-on branch is cut at its bore: declare joint set_on with header_surface_reference outside and branch_surface_reference inside.");
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
///
/// Equality under OUTSIDE or INSIDE means zero wall thickness. The
/// specification does not forbid an idealised zero-thickness surface, so it is
/// accepted with a scrutiny note, not refused. No thickness, k-factor or
/// plausible thickness range is assumed anywhere: the specification gives none
/// (MATH-16), so the size of the difference is not judged.
///
/// Ok(None) = consistent; Ok(Some(note)) = accepted, engineer must look; Err = refused.
pub fn radius_consistency(
    sref: &str,
    contact_name: &str,
    contact: f64,
    dev_name: &str,
    dev: f64,
) -> Result<Option<String>, String> {
    let same = (dev - contact).abs() <= SAME_RADIUS_REL * dev.abs().max(contact.abs());
    let zero_t = |side: &str| {
        Some(format!(
            "{dev_name} = {dev} equals the {side} contact radius {contact_name} = {contact}: that is a wall of zero thickness (an idealised surface). Thickness is not modelled; an engineer must confirm the radii before use."
        ))
    };
    match sref {
        "outside" if same => Ok(zero_t("outside")),
        "outside" if dev > contact => Err(format!(
            "inconsistent with surface_reference 'outside': {dev_name} = {dev} is larger than the outside contact radius {contact_name} = {contact}. The development (reference) surface lies inside the wall (Spec 02 §6.1: r_dev = r_i + k·t, 0 < k <= 0.5), so it cannot be larger than the outside radius; these values would mean a negative wall thickness. Correct the radii or the declared surface"
        )),
        "outside" => Ok(None),
        "inside" if same => Ok(zero_t("inside")),
        "inside" if dev < contact => Err(format!(
            "inconsistent with surface_reference 'inside': {dev_name} = {dev} is smaller than the inside contact radius {contact_name} = {contact}. The development (reference) surface lies inside the wall (Spec 02 §6.1: r_dev = r_i + k·t, 0 < k <= 0.5), so it cannot be smaller than the inside radius; these values would mean a negative wall thickness. Correct the radii or the declared surface"
        )),
        "inside" => Ok(None),
        "neutral" if same => Ok(None),
        "neutral" => Err(format!(
            "inconsistent with surface_reference 'neutral': the contact radius {contact_name} = {contact} is declared to be on the reference surface itself, so {dev_name} must equal it, but it is {dev}. Correct the radii or the declared surface"
        )),
        _ => Ok(Some(format!(
            "The relation between {contact_name} = {contact} and {dev_name} = {dev} could not be checked because the surface reference is unknown."
        ))),
    }
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
            return Err(UemError::OutOfRange {
                param: name,
                reason: format!(
                    "magnitude {x:e} mm exceeds {MAX_MAGNITUDE_MM:e} mm, the largest size this build has been exercised with (implementation limit, not an engineering rule)"
                ),
            });
        }
        if x > 0.0 && x < MIN_DIMENSION_MM && !SIGNED_PARAMS.contains(&name) {
            return Err(UemError::OutOfRange {
                param: name,
                reason: format!(
                    "{x:e} mm is below {MIN_DIMENSION_MM:e} mm, the smallest dimension this build has been exercised with (implementation limit, not an engineering rule)"
                ),
            });
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
    match req.clone() {
        Request::CylinderShell { r_dev, length } => single(
            req,
            entered_by,
            "shell",
            develop::cylinder_shell(r_dev, length).map(|p| {
                let c = verify::verify_cylinder(&p, r_dev, length);
                (p, c)
            }),
        ),
        Request::ConeFrustum { r1, r2, h } => single(
            req,
            entered_by,
            "reducer",
            develop::cone_frustum(r1, r2, h).map(|p| {
                let c = verify::verify_cone(&p, r1, r2, h);
                (p, c)
            }),
        ),
        Request::EccentricReducer { r1, r2, h, e } => single(
            req,
            entered_by,
            "reducer",
            develop::eccentric_reducer(r1, r2, h, e).map(|p| {
                let c = verify::verify_eccentric(&p, r1, r2, h, e);
                (p, c)
            }),
        ),
        Request::BranchStub {
            header_radius,
            branch_radius,
            branch_dev_radius,
            beta_deg,
            delta,
            s_end,
            ..
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
            .map(|p| {
                let b = Branch {
                    big_r: header_radius,
                    r: branch_radius,
                    beta: beta_deg.to_radians(),
                    delta,
                };
                let c = verify::verify_branch(&p, &b, branch_dev_radius, s_end);
                (p, c)
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
            },
            Request::ConeFrustum {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
            },
            Request::EccentricReducer {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
                e: 100.0,
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
            assert!(o.checks.iter().all(|c| c.verdict == "PASS"), "{r:?}");
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
            assert!(pass("G10") && pass("G11") && pass("G15"), "{r:?}");
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
                    h: 300.5
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
        let o = run(
            &Request::ConeFrustum {
                r1: 200.0,
                r2: 199.99,
                h: 300.0,
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
    }

    /// BE-003: every refusal has the explicit REFUSED state, a reason, and nothing that looks like a result.
    #[test]
    fn be003_refusals_have_their_own_state() {
        let cases = [
            with_radii("outside", -150.0, 150.0, 100.0, 100.0),
            with_radii("outside", 150.0, 150.0, 400.0, 400.0),
            with_radii("whatever", 150.0, 150.0, 100.0, 100.0),
            Request::ConeFrustum {
                r1: 200.0,
                r2: 199.99,
                h: 300.0,
            },
            Request::ConeFrustum {
                r1: f64::NAN,
                r2: 1.0,
                h: 1.0,
            },
            Request::EccentricReducer {
                r1: 100.0,
                r2: 100.0,
                h: 300.0,
                e: 50.0,
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
            },
            Request::CylinderShell {
                r_dev: 1e-300,
                length: 10.0,
            },
            Request::CylinderShell {
                r_dev: 100.0,
                length: 1e9,
            },
            Request::ConeFrustum {
                r1: 200.0,
                r2: 1e-7,
                h: 300.0,
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
                    length: 1e-2
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
        assert!(
            e.contains("thickness")
                && e.contains("surface_reference")
                && e.contains("unknown field"),
            "{e}"
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

    /// D1: a job whose parts are not all drawn from the declared side cannot stay reviewable.
    #[test]
    fn d1_parts_from_different_sides_fail_the_ambiguity_gate() {
        let mut b = Builder::new();
        b.sides = vec![uem::ViewSide::Outside, uem::ViewSide::Inside];
        let p = develop::cylinder_shell(100.0, 250.0).unwrap();
        let rq = b.push(request_claim(&reqs()[0], "t"));
        let checks = verify::verify_cylinder(&p, 100.0, 250.0);
        b.part("shell", &p, vec![rel("developed_from", &rq)], checks);
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

    /// D7: arc points in the JSON are labelled as a display approximation with their true deviation.
    #[test]
    fn d7_arc_points_are_labelled_as_an_approximation() {
        let o = run(
            &Request::ConeFrustum {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
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
        assert!(
            (arc.points_max_deviation - measured).abs() < 1e-9 && (measured - 0.034).abs() < 1e-3,
            "stated {} measured {measured}",
            arc.points_max_deviation
        );
        for c in p.curves.iter().filter(|c| c.kind != "arc") {
            assert!(c.points_exact && c.points_max_deviation == 0.0);
        }
        let poly = run(&reqs()[3], "t");
        assert!(poly.patterns[0].curves.iter().all(|c| c.points_exact));
        // D3 / D4: nothing calls the estimates bounds
        let g08 = |o: &JobOutput| {
            o.gates
                .iter()
                .find(|g| g.id == "G08")
                .unwrap()
                .detail
                .clone()
        };
        assert!(
            g08(&o).contains("ESTIMATE, not a bound")
                && g08(&poly).contains("ESTIMATES, not bounds")
        );
        assert!(
            poly.patterns[0]
                .assumptions
                .iter()
                .any(|a| a.contains("estimates, not bounds"))
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
            joint.starts_with("single contact curve") && !joint.contains("set-on"),
            "{joint}"
        );
        assert!(g02_detail(&o).contains("Joint type not declared"));
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
                .starts_with("single contact curve")
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
        b.part("broken", &pattern, vec![], vec![]);
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
}
