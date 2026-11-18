//! Engineering jobs: deterministic passes from an engineer-entered request to
//! patterns, verification evidence, DXF files, re-import checks, gate
//! evaluation and release closure. A job is a pure function of its request:
//! running it again yields identical records and closure hash (replay audit).
//!
//! Single-part jobs record the request as one claim. The assembly job (header
//! with set-on branch) records every parameter as its own claim and links
//! records with typed dependency edges, so that `deps::propagate` can answer
//! "what is affected if this parameter changes?".

use crate::{closure, closure_hash, rel, sha256_hex, Family, Gate, GateResult, Record, ReleaseState, ENGINE_VERSION};
use nf_dxf::Entity;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uem::develop::{self, Branch};
use uem::verify::{self, Check, Verdict};
use uem::{profile, Geom, Layer, Pattern, UemError};

pub const SURFACE_REFERENCES: [&str; 4] = ["outside", "inside", "neutral", "unknown"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum Request {
    CylinderShell { r_dev: f64, length: f64 },
    ConeFrustum { r1: f64, r2: f64, h: f64 },
    EccentricReducer { r1: f64, r2: f64, h: f64, e: f64 },
    BranchStub { header_radius: f64, branch_radius: f64, branch_dev_radius: f64, beta_deg: f64, delta: f64, s_end: f64 },
    /// Assembly: header shell rolled from plate with one set-on branch.
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
        /// drawing-level convention, recorded and never inferred.
        surface_reference: String,
    },
}

impl Request {
    /// Names of numeric parameters that can be corrected individually.
    pub fn numeric_params(&self) -> Vec<(String, f64)> {
        let v = serde_json::to_value(self).unwrap_or(Value::Null);
        v.as_object().map(|o| o.iter().filter_map(|(k, x)| x.as_f64().map(|f| (k.clone(), f))).collect()).unwrap_or_default()
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
    pub points: Vec<[f64; 2]>,
    pub length: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternView {
    pub part: String,
    pub family: String,
    pub class: String,
    pub eps_num: f64,
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
    pub outcome: Option<String>,
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

pub fn to_entities(p: &Pattern) -> Vec<Entity> {
    p.outline
        .iter()
        .chain(p.holes.iter())
        .map(|c| {
            let layer = layer_name(c.layer).to_string();
            match &c.geom {
                Geom::Line { a, b } => Entity::Line { layer, a: (a.x, a.y), b: (b.x, b.y) },
                Geom::Arc { c, r, a0, a1 } => Entity::Arc { layer, c: (c.x, c.y), r: *r, start_deg: a0.min(*a1).to_degrees(), end_deg: a0.max(*a1).to_degrees() },
                Geom::Polyline { pts, .. } => Entity::Polyline { layer, pts: pts.iter().map(|q| (q.x, q.y)).collect() },
            }
        })
        .collect()
}

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
        points: profile::flatten(std::slice::from_ref(c), 96).iter().map(|q| [q.x, q.y]).collect(),
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
    CheckView { scope: scope.into(), name: c.name.into(), method: c.method.into(), independence: c.independence.into(), primary: c.primary, reference: c.reference, difference: c.difference, tolerance: c.tolerance, verdict: verdict_str(c.verdict).into() }
}

fn activity(kind: &str, zone: &str, inputs: &[&str], work_units: u64) -> Record {
    Record::new(Family::Activity, kind, json!({ "engine": ENGINE_VERSION, "zone": zone, "lane": "not_production_qualified", "work_units": work_units }), inputs.iter().map(|i| rel("input", i)).collect(), None)
}

fn gate(id: &str, name: &str, critical: bool, result: GateResult, detail: &str) -> Gate {
    Gate { id: id.into(), name: name.into(), critical, result, detail: detail.into() }
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
}

impl Builder {
    fn new() -> Self {
        Builder { recs: vec![], wu: 0, patterns: vec![], checks: vec![], all_checks: vec![], dxf: vec![], topo_ok: true, reimport_ok: true, has_polyline: false, evaluated: vec![] }
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
    fn evidence(&mut self, scope: &str, subject: &str, edge: &str, checks: Vec<Check>, wu: u64) -> String {
        let a = self.act("verify.family", "Z3", &[subject], wu);
        let views: Vec<CheckView> = checks.iter().map(|c| check_view(scope, c)).collect();
        let id = self.push(Record::new(Family::Assessment, "verification_evidence", json!({ "scope": scope, "checks": views }), vec![rel(edge, subject)], Some(a)));
        self.checks.extend(views);
        self.all_checks.extend(checks);
        self.evaluated.push(id.clone());
        id
    }

    /// Pattern claim, profile report, DXF output and re-import report for one part.
    /// `deps` are the typed edges from the pattern to what it was developed from.
    fn part(&mut self, part: &str, pattern: &Pattern, deps: Vec<crate::Rel>, checks: Vec<Check>) -> (String, String) {
        let inputs: Vec<&str> = deps.iter().map(|d| d.to.as_str()).collect();
        let a_dev = self.act("build.develop", "Z2", &inputs, 1500);
        let entities = to_entities(pattern);
        let text = nf_dxf::write(&entities);
        let dxf_sha = sha256_hex(text.as_bytes());
        let pv = PatternView {
            part: part.to_string(),
            family: pattern.family.to_string(),
            class: pattern.class.code().to_string(),
            eps_num: pattern.eps_num,
            eps_rep: pattern.eps_rep,
            quantities: pattern.quantities.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            assumptions: pattern.assumptions.iter().map(|s| s.to_string()).collect(),
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
        let ok = outer.closed && outer.self_intersections == 0 && outer.signed_area > 0.0 && holes_ok;
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
            json!({ "part": part, "sha256": dxf_sha, "bytes": text.len(), "entities": entities.len(), "format": "DXF AC1009 subset", "units": "mm (comment only)" }),
            vec![rel("rendered_from", &pattern_id)],
            Some(a_dxf),
        ));
        let a_re = self.act("verify.reimport", "Z3", &[&dxf_id, &pattern_id], 200);
        let re = nf_dxf::reimport_check(&entities, &text, 1e-8);
        self.reimport_ok &= re.pass;
        let re_id = self.push(Record::new(
            Family::Assessment,
            "reimport_report",
            json!({ "part": part, "pass": re.pass, "entities_written": re.entities_written, "entities_read": re.entities_read, "unknown_entities": re.unknown_entities, "max_point_deviation": re.max_point_deviation, "max_length_deviation": re.max_length_deviation, "layers_match": re.layers_match }),
            vec![rel("verifies", &dxf_id)],
            Some(a_re),
        ));
        self.has_polyline |= pattern.eps_rep > 0.0;
        self.patterns.push(pv);
        self.dxf.push((part.to_string(), text));
        self.evaluated.extend([pattern_id.clone(), dxf_id.clone(), prof_id, re_id]);
        (pattern_id, dxf_id)
    }

    fn finish(mut self, req: &Request, mut gates: Vec<Gate>, outcome: Option<String>) -> JobOutput {
        let state = crate::release_state(&gates);
        // gate ids are stable; keep the table in a fixed order
        gates.sort_by_key(|g| g.id.clone());
        let rels: Vec<crate::Rel> = self.evaluated.iter().map(|e| rel("evaluates", e)).collect();
        let gate_rec = Record::new(Family::Assessment, "gate_evaluation", json!({ "gates": gates, "state": state }), rels, None);
        let root = gate_rec.id.clone();
        self.recs.push(gate_rec);
        let map: BTreeMap<String, Record> = self.recs.iter().map(|r| (r.id.clone(), r.clone())).collect();
        let (ids, missing) = closure(&map, &[root]);
        debug_assert!(missing.is_empty());
        JobOutput { engine: ENGINE_VERSION.into(), request: req.clone(), state, outcome, gates, patterns: self.patterns, checks: self.checks, closure_hash: closure_hash(&ids), closure_size: ids.len(), work_units: self.wu, records: self.recs, dxf: self.dxf }
    }
}

/// Gate evaluation for this build. Gates the software cannot evaluate are
/// reported as NOT_IMPLEMENTED or INCONCLUSIVE — never as PASS. G18 and G19
/// depend on state that changes after the job (obligations, replay) and are
/// re-evaluated by the server when a result is read.
pub fn evaluate_gates(checks: &[Check], topo_ok: Option<bool>, reimport_ok: Option<bool>, has_polyline: bool, surface_reference: Option<&str>) -> Vec<Gate> {
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
        gate("G01", "provenance_complete", true, GateResult::Inconclusive, "Parameters were entered manually; they are not linked to confirmed dimensions on a source drawing (drawing intelligence is not implemented)."),
        gate("G05", "class_assigned", true, GateResult::Pass, "Development class assigned by the surface-type constructor. Class naming scheme is a proposal (Spec 02 §14), not adopted."),
        gate("G06", "assumptions_backed", true, GateResult::Inconclusive, "No calibrated process data exists. Radii are used as reference-surface radii exactly as entered; thickness, allowances, weld gap and kerf are not modelled."),
        gate("G08", "numeric_bounds", true, GateResult::Inconclusive, if has_polyline { "Numerical and curve-representation errors are estimated, not interval-certified." } else { "Numerical error is estimated, not interval-certified." }),
        gate("G10", "verification", true, ver, "Independent-formulation checks (class I-F), including interface constraints where the job has several parts. No separately implemented verifier or external kernel cross-check exists yet."),
        gate("G11", "profile_topology", true, tri(topo_ok), "Outer profiles closed, simple, counter-clockwise; openings closed, simple, clockwise (binary64 arithmetic, not exact predicates). Kerf-compensation ownership is not declared."),
        gate("G14", "algorithm_status", true, GateResult::Inconclusive, "Algorithms are evidence level EV-1 (AI-generated, not independently reviewed) and not production-qualified."),
        gate("G15", "output_integrity", true, tri(reimport_ok), "Every DXF re-imported by the independent reader and compared with the intended geometry. CAM acceptance not tested."),
        gate("G16", "human_approval", true, GateResult::NotImplemented, "Engineer approval workflow is not implemented. This result cannot be released for manufacturing."),
        gate("G18", "obligation_closure", true, GateResult::NotRun, "Evaluated when the result is read: depends on the current state of scrutiny obligations."),
        gate("G19", "self_consistency", true, GateResult::NotRun, "Evaluated when the result is read: depends on replay audits. Audits A1-A4 are not implemented."),
    ];
    if let Some(sr) = surface_reference {
        let (res, detail) = if sr == "unknown" {
            (GateResult::Inconclusive, "Surface reference of the entered radii is declared UNKNOWN: inside / outside / neutral must be established by an engineer before use.".to_string())
        } else {
            (GateResult::Inconclusive, format!("Surface reference declared by the user as '{sr}'. The declaration is recorded, not checked against a drawing."))
        };
        g.push(gate("G02", "no_open_ambiguity", true, res, &detail));
    }
    g
}

fn err_output(req: &Request, mut b: Builder, about: &str, e: UemError) -> JobOutput {
    let critical = !matches!(e, UemError::OutOfRange { .. });
    let a = b.act("build.develop", "Z2", &[about], 1500);
    let f = b.push(Record::new(Family::Finding, "engineering_outcome", json!({ "outcome": format!("{e:?}"), "message": e.to_string(), "critical": critical }), vec![rel("about", about)], Some(a)));
    b.evaluated.push(f);
    b.patterns.clear();
    b.dxf.clear();
    b.checks.clear();
    let gates = vec![gate("G05", "class_assigned", true, if critical { GateResult::Fail } else { GateResult::Inconclusive }, &format!("No pattern produced: {e}"))];
    b.finish(req, gates, Some(e.to_string()))
}

fn request_claim(req: &Request, entered_by: &str) -> Record {
    Record::new(Family::Claim, "development_request", json!({ "request": req, "entered_by": entered_by, "axes": { "provenance": "human_entered", "authority": "assumed", "reading": "n/a", "validation": "candidate" } }), vec![], None)
}

fn single(req: &Request, entered_by: &str, part: &str, r: Result<(Pattern, Vec<Check>), UemError>) -> JobOutput {
    let mut b = Builder::new();
    let rq = b.push(request_claim(req, entered_by));
    match r {
        Err(e) => err_output(req, b, &rq, e),
        Ok((p, checks)) => {
            b.part(part, &p, vec![rel("developed_from", &rq)], checks);
            let gates = evaluate_gates(&b.all_checks, Some(b.topo_ok), Some(b.reimport_ok), b.has_polyline, None);
            b.finish(req, gates, None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn header_with_branch(req: &Request, entered_by: &str, hr: f64, hdev: f64, hlen: f64, station: f64, br: f64, bdev: f64, beta_deg: f64, delta: f64, s_end: f64, sref: &str) -> JobOutput {
    let mut b = Builder::new();
    let rq = b.push(request_claim(req, entered_by));
    if !SURFACE_REFERENCES.contains(&sref) {
        return err_output(req, b, &rq, UemError::OutOfRange { param: "surface_reference", reason: format!("must be one of {SURFACE_REFERENCES:?}") });
    }
    // One claim per parameter, so that a change to one of them can be traced.
    let conv = b.push(Record::new(Family::Claim, "convention", json!({ "name": "surface_reference", "value": sref, "scope": "all radii of this request", "axes": { "provenance": "human_entered", "authority": "assumed" } }), vec![rel("declared_in", &rq)], None));
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

    let part_h = b.push(Record::new(Family::Claim, "part", json!({ "name": "header" }), vec![rel("defined_by", &p_hr), rel("defined_by", &p_hdev), rel("defined_by", &p_hlen)], None));
    let part_b = b.push(Record::new(Family::Claim, "part", json!({ "name": "branch" }), vec![rel("defined_by", &p_br), rel("defined_by", &p_bdev), rel("defined_by", &p_send)], None));

    let geo = Branch { big_r: hr, r: br, beta: beta_deg.to_radians(), delta };
    let branch_pat = match develop::branch_stub(hr, br, bdev, beta_deg, delta, s_end) {
        Ok(p) => p,
        Err(e) => return err_output(req, b, &rq, e),
    };
    let header_pat = match develop::header_with_opening(&geo, hdev, hlen, station) {
        Ok(p) => p,
        Err(e) => return err_output(req, b, &rq, e),
    };
    // The intersection curve is shared by both parts: it depends on both contact radii and the placement.
    let a_int = b.act("build.intersect", "Z2", &[&p_hr, &p_br, &p_beta, &p_delta], 700);
    let curve = b.push(Record::new(
        Family::Claim,
        "intersection_curve",
        json!({ "between": ["header", "branch"], "s_min": branch_pat.quantity("s_min"), "s_max": branch_pat.quantity("s_max"), "formula": "Spec 02 eq. (4.3)" }),
        vec![rel("computed_from", &p_hr), rel("computed_from", &p_br), rel("computed_from", &p_beta), rel("computed_from", &p_delta)],
        Some(a_int),
    ));
    let interface = b.push(Record::new(
        Family::Claim,
        "interface",
        json!({ "name": "header-branch joint", "joint": "set-on (as entered)", "constraint": "branch end cut and header opening are the same intersection curve" }),
        vec![rel("mates_at", &part_h), rel("mates_at", &part_b), rel("interface_feature", &curve), rel("positioned_by", &p_st)],
        None,
    ));

    let hc = verify::verify_header_opening(&header_pat, &geo, hdev, hlen, station);
    let (pat_h, _) = b.part("header", &header_pat, vec![rel("developed_from", &curve), rel("developed_from", &p_hdev), rel("developed_from", &p_hlen), rel("developed_from", &p_st), rel("pattern_of", &part_h)], hc);
    let bc = verify::verify_branch(&branch_pat, &geo, bdev, s_end);
    let (pat_b, _) = b.part("branch", &branch_pat, vec![rel("developed_from", &curve), rel("developed_from", &p_bdev), rel("developed_from", &p_send), rel("pattern_of", &part_b)], bc);

    // Interface constraint, evaluated from the two finished patterns.
    let ic = verify::verify_interface(&header_pat, &geo, hdev, station, &branch_pat, &geo, bdev, s_end);
    let a_if = b.act("verify.interface", "Z3", &[&pat_h, &pat_b, &interface], 3000);
    let views: Vec<CheckView> = ic.iter().map(|c| check_view("interface", c)).collect();
    let ev = b.push(Record::new(Family::Assessment, "interface_evidence", json!({ "scope": "interface", "checks": views }), vec![rel("verifies", &interface), rel("uses", &pat_h), rel("uses", &pat_b)], Some(a_if)));
    b.checks.extend(views);
    b.all_checks.extend(ic);
    b.evaluated.extend([ev, interface]);

    let gates = evaluate_gates(&b.all_checks, Some(b.topo_ok), Some(b.reimport_ok), b.has_polyline, Some(sref));
    b.finish(req, gates, None)
}

/// Run a job. `entered_by` is recorded as human input data on the request.
pub fn run(req: &Request, entered_by: &str) -> JobOutput {
    match req.clone() {
        Request::CylinderShell { r_dev, length } => single(req, entered_by, "shell", develop::cylinder_shell(r_dev, length).map(|p| {
            let c = verify::verify_cylinder(&p, r_dev, length);
            (p, c)
        })),
        Request::ConeFrustum { r1, r2, h } => single(req, entered_by, "reducer", develop::cone_frustum(r1, r2, h).map(|p| {
            let c = verify::verify_cone(&p, r1, r2, h);
            (p, c)
        })),
        Request::EccentricReducer { r1, r2, h, e } => single(req, entered_by, "reducer", develop::eccentric_reducer(r1, r2, h, e).map(|p| {
            let c = verify::verify_eccentric(&p, r1, r2, h, e);
            (p, c)
        })),
        Request::BranchStub { header_radius, branch_radius, branch_dev_radius, beta_deg, delta, s_end } => single(req, entered_by, "branch", develop::branch_stub(header_radius, branch_radius, branch_dev_radius, beta_deg, delta, s_end).map(|p| {
            let b = Branch { big_r: header_radius, r: branch_radius, beta: beta_deg.to_radians(), delta };
            let c = verify::verify_branch(&p, &b, branch_dev_radius, s_end);
            (p, c)
        })),
        Request::HeaderWithBranch { header_radius, header_dev_radius, header_length, branch_station, branch_radius, branch_dev_radius, beta_deg, delta, s_end, surface_reference } => {
            header_with_branch(req, entered_by, header_radius, header_dev_radius, header_length, branch_station, branch_radius, branch_dev_radius, beta_deg, delta, s_end, &surface_reference)
        }
    }
}

/// Recompute from the request alone and compare closure hash and record ids.
pub fn replay_matches(req: &Request, entered_by: &str, expected_closure_hash: &str, expected_ids: &[String]) -> (bool, String) {
    let again = run(req, entered_by);
    let ids: Vec<String> = again.records.iter().map(|r| r.id.clone()).collect();
    (again.closure_hash == expected_closure_hash && ids == expected_ids, again.closure_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn assembly() -> Request {
        Request::HeaderWithBranch { header_radius: 150.0, header_dev_radius: 150.0, header_length: 1200.0, branch_station: 600.0, branch_radius: 100.0, branch_dev_radius: 100.0, beta_deg: 60.0, delta: 0.0, s_end: 400.0, surface_reference: "outside".into() }
    }

    fn reqs() -> Vec<Request> {
        vec![
            Request::CylinderShell { r_dev: 100.0, length: 250.0 },
            Request::ConeFrustum { r1: 200.0, r2: 100.0, h: 300.0 },
            Request::EccentricReducer { r1: 200.0, r2: 100.0, h: 300.0, e: 100.0 },
            Request::BranchStub { header_radius: 150.0, branch_radius: 100.0, branch_dev_radius: 100.0, beta_deg: 60.0, delta: 0.0, s_end: 400.0 },
            assembly(),
        ]
    }

    #[test]
    fn jobs_run_verify_and_never_approve() {
        for r in reqs() {
            let o = run(&r, "tester");
            assert!(!o.patterns.is_empty() && o.dxf.len() == o.patterns.len(), "{r:?}");
            assert!(o.checks.iter().all(|c| c.verdict == "PASS"), "{r:?}");
            assert_eq!(o.state, ReleaseState::ReviewRequired, "this build must never reach APPROVED");
            assert!(o.gates.iter().any(|g| g.id == "G16" && g.result == GateResult::NotImplemented));
            assert!(o.records.iter().all(|x| x.verify_id()));
            let pass = |id: &str| o.gates.iter().any(|g| g.id == id && g.result == GateResult::Pass);
            assert!(pass("G10") && pass("G11") && pass("G15"), "{r:?}");
        }
    }

    #[test]
    fn assembly_has_two_parts_interface_evidence_and_opening() {
        let o = run(&assembly(), "t");
        assert_eq!(o.patterns.iter().map(|p| p.part.as_str()).collect::<Vec<_>>(), ["header", "branch"]);
        assert_eq!(o.patterns[0].holes.len(), 1);
        assert_eq!(o.checks.iter().filter(|c| c.scope == "interface").count(), 2);
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
            assert!(replay_matches(&r, "tester", &a.closure_hash, &ids).0, "{r:?}");
            assert_eq!(a.dxf, run(&r, "tester").dxf);
        }
        let a = run(&reqs()[1], "tester");
        assert_ne!(a.closure_hash, run(&Request::ConeFrustum { r1: 200.0, r2: 100.0, h: 300.5 }, "tester").closure_hash);
        assert_ne!(a.closure_hash, run(&reqs()[1], "someone else").closure_hash, "who entered the request is part of the record");
    }

    #[test]
    fn unsupported_and_degenerate_inputs_give_no_dxf() {
        let o = run(&Request::ConeFrustum { r1: 200.0, r2: 199.99, h: 300.0 }, "t");
        assert!(o.dxf.is_empty() && o.patterns.is_empty());
        assert_eq!(o.state, ReleaseState::ReviewRequired);
        assert!(o.records.iter().any(|r| r.family == Family::Finding));
        assert!(run(&assembly().with_param("branch_station", 10.0).unwrap(), "t").dxf.is_empty(), "opening outside the shell");
        let mut bad = assembly();
        if let Request::HeaderWithBranch { surface_reference, .. } = &mut bad {
            *surface_reference = "whatever".into();
        }
        assert!(run(&bad, "t").dxf.is_empty());
    }

    #[test]
    fn failed_verification_rejects() {
        let p = develop::cone_frustum(206.0, 100.0, 300.0).unwrap();
        let checks = verify::verify_cone(&p, 200.0, 100.0, 300.0);
        assert_eq!(crate::release_state(&evaluate_gates(&checks, Some(true), Some(true), false, None)), ReleaseState::Rejected);
        let good = verify::verify_cone(&develop::cone_frustum(200.0, 100.0, 300.0).unwrap(), 200.0, 100.0, 300.0);
        assert_eq!(crate::release_state(&evaluate_gates(&good, Some(true), Some(false), false, None)), ReleaseState::Rejected, "failed re-import must reject");
        assert_eq!(crate::release_state(&evaluate_gates(&good, Some(false), Some(true), false, None)), ReleaseState::Rejected, "invalid profile must reject");
    }
}
