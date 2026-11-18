//! The development job: one deterministic pass from an engineer-entered
//! request to pattern, verification evidence, DXF, re-import check, gate
//! evaluation and release closure. Pure function of the request: running it
//! twice yields identical records and the same closure hash (replay audit).

use crate::{closure, closure_hash, rel, sha256_hex, Family, Gate, GateResult, Record, ReleaseState, ENGINE_VERSION};
use nf_dxf::Entity;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uem::develop::{self, Branch};
use uem::verify::{self, Check, Verdict};
use uem::{profile, Geom, Layer, Pattern, UemError};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum Request {
    CylinderShell { r_dev: f64, length: f64 },
    ConeFrustum { r1: f64, r2: f64, h: f64 },
    EccentricReducer { r1: f64, r2: f64, h: f64, e: f64 },
    BranchStub { header_radius: f64, branch_radius: f64, branch_dev_radius: f64, beta_deg: f64, delta: f64, s_end: f64 },
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
    pub family: String,
    pub class: String,
    pub eps_num: f64,
    pub eps_rep: f64,
    pub quantities: BTreeMap<String, f64>,
    pub assumptions: Vec<String>,
    pub curves: Vec<CurveView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckView {
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
    pub pattern: Option<PatternView>,
    pub checks: Vec<CheckView>,
    pub profile: Option<Value>,
    pub reimport: Option<Value>,
    pub closure_hash: String,
    pub closure_size: usize,
    pub work_units: u64,
    pub records: Vec<Record>,
    #[serde(skip)]
    pub dxf: Option<String>,
}

fn layer_name(l: Layer) -> &'static str {
    match l {
        Layer::CutOuter => "CUT_OUTER",
        Layer::Reference => "REFERENCE",
    }
}

pub fn to_entities(p: &Pattern) -> Vec<Entity> {
    p.outline
        .iter()
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

fn view(p: &Pattern) -> PatternView {
    PatternView {
        family: p.family.to_string(),
        class: p.class.code().to_string(),
        eps_num: p.eps_num,
        eps_rep: p.eps_rep,
        quantities: p.quantities.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        assumptions: p.assumptions.iter().map(|s| s.to_string()).collect(),
        curves: p
            .outline
            .iter()
            .map(|c| CurveView {
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
            })
            .collect(),
    }
}

fn check_view(c: &Check) -> CheckView {
    CheckView {
        name: c.name.into(),
        method: c.method.into(),
        independence: c.independence.into(),
        primary: c.primary,
        reference: c.reference,
        difference: c.difference,
        tolerance: c.tolerance,
        verdict: match c.verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Inconclusive => "INCONCLUSIVE",
        }
        .into(),
    }
}

fn develop_and_verify(req: &Request) -> Result<(Pattern, Vec<Check>), UemError> {
    Ok(match *req {
        Request::CylinderShell { r_dev, length } => {
            let p = develop::cylinder_shell(r_dev, length)?;
            let c = verify::verify_cylinder(&p, r_dev, length);
            (p, c)
        }
        Request::ConeFrustum { r1, r2, h } => {
            let p = develop::cone_frustum(r1, r2, h)?;
            let c = verify::verify_cone(&p, r1, r2, h);
            (p, c)
        }
        Request::EccentricReducer { r1, r2, h, e } => {
            let p = develop::eccentric_reducer(r1, r2, h, e)?;
            let c = verify::verify_eccentric(&p, r1, r2, h, e);
            (p, c)
        }
        Request::BranchStub { header_radius, branch_radius, branch_dev_radius, beta_deg, delta, s_end } => {
            let p = develop::branch_stub(header_radius, branch_radius, branch_dev_radius, beta_deg, delta, s_end)?;
            let b = Branch { big_r: header_radius, r: branch_radius, beta: beta_deg.to_radians(), delta };
            let c = verify::verify_branch(&p, &b, branch_dev_radius, s_end);
            (p, c)
        }
    })
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
    Gate { id: id.into(), name: name.into(), critical, result, detail: detail.into() }
}

/// Gate evaluation for this build. Gates that the software cannot evaluate
/// are reported as NOT_IMPLEMENTED — never as PASS.
pub fn evaluate_gates(checks: &[Check], prof: Option<&profile::ProfileReport>, reimport_pass: Option<bool>, has_polyline: bool) -> Vec<Gate> {
    let ver = if checks.is_empty() {
        GateResult::NotRun
    } else if checks.iter().any(|c| c.verdict == Verdict::Fail) {
        GateResult::Fail
    } else if checks.iter().any(|c| c.verdict == Verdict::Inconclusive) {
        GateResult::Inconclusive
    } else {
        GateResult::Pass
    };
    let topo = match prof {
        None => GateResult::NotRun,
        Some(p) if p.closed && p.self_intersections == 0 && p.signed_area > 0.0 => GateResult::Pass,
        Some(_) => GateResult::Fail,
    };
    let out = match reimport_pass {
        None => GateResult::NotRun,
        Some(true) => GateResult::Pass,
        Some(false) => GateResult::Fail,
    };
    vec![
        gate("G01", "provenance_complete", true, GateResult::Inconclusive, "Parameters were entered manually; they are not linked to confirmed dimensions on a source drawing (drawing intelligence is not implemented)."),
        gate("G05", "class_assigned", true, GateResult::Pass, "Development class assigned by the surface-type constructor. Class naming scheme is a proposal (Spec 02 §14), not adopted."),
        gate("G06", "assumptions_backed", true, GateResult::Inconclusive, "No calibrated process data exists. Radii are used as reference-surface radii exactly as entered; thickness, allowances, weld gap and kerf are not modelled."),
        gate("G08", "numeric_bounds", true, GateResult::Inconclusive, if has_polyline { "Numerical and curve-representation errors are estimated, not interval-certified." } else { "Numerical error is estimated, not interval-certified." }),
        gate("G10", "verification", true, ver, "Independent-formulation checks (class I-F). No separately implemented verifier or external kernel cross-check exists yet."),
        gate("G11", "profile_topology", true, topo, "Closed, non-self-intersecting, counter-clockwise outline (binary64 arithmetic, not exact predicates). Kerf-compensation ownership is not declared."),
        gate("G14", "algorithm_status", true, GateResult::Inconclusive, "Algorithms are evidence level EV-1 (AI-generated, not independently reviewed) and not production-qualified."),
        gate("G15", "output_integrity", true, out, "DXF re-imported by the independent reader and compared with the intended geometry. CAM acceptance not tested."),
        gate("G18", "obligation_closure", true, GateResult::NotImplemented, "Dependency propagation and scrutiny obligations are not implemented."),
        gate("G19", "self_consistency", true, GateResult::NotRun, "Replay audit not yet run for this result. Independent-instance replay and audits A1-A4 are not implemented."),
        gate("G16", "human_approval", true, GateResult::NotImplemented, "Engineer approval workflow is not implemented. This result cannot be released for manufacturing."),
    ]
}

/// Run the job. `entered_by` is recorded as human input data on the request.
pub fn run(req: &Request, entered_by: &str) -> JobOutput {
    let mut recs: Vec<Record> = Vec::new();
    let mut wu: u64 = 0;
    let request = Record::new(
        Family::Claim,
        "development_request",
        json!({ "request": req, "entered_by": entered_by,
                "axes": { "provenance": "human_entered", "authority": "assumed", "reading": "n/a", "validation": "candidate" } }),
        vec![],
        None,
    );
    recs.push(request.clone());

    let a_dev = activity("build.develop", "Z2", &[&request.id], 1500);
    wu += 1500;
    recs.push(a_dev.clone());

    let (pattern, checks) = match develop_and_verify(req) {
        Ok(x) => x,
        Err(e) => {
            let critical = !matches!(e, UemError::OutOfRange { .. });
            let finding = Record::new(
                Family::Finding,
                "engineering_outcome",
                json!({ "outcome": format!("{e:?}"), "message": e.to_string(), "critical": critical }),
                vec![rel("about", &request.id)],
                Some(a_dev.id.clone()),
            );
            recs.push(finding.clone());
            let gates = vec![gate(
                "G05",
                "class_assigned",
                true,
                if critical { GateResult::Fail } else { GateResult::Inconclusive },
                &format!("No pattern produced: {e}"),
            )];
            let state = crate::release_state(&gates);
            let gate_rec = Record::new(Family::Assessment, "gate_evaluation", json!({ "gates": gates, "state": state }), vec![rel("evaluates", &finding.id)], None);
            recs.push(gate_rec.clone());
            let map: BTreeMap<String, Record> = recs.iter().map(|r| (r.id.clone(), r.clone())).collect();
            let (ids, _) = closure(&map, &[gate_rec.id.clone()]);
            return JobOutput {
                engine: ENGINE_VERSION.into(),
                request: req.clone(),
                state,
                outcome: Some(e.to_string()),
                gates,
                pattern: None,
                checks: vec![],
                profile: None,
                reimport: None,
                closure_hash: closure_hash(&ids),
                closure_size: ids.len(),
                work_units: wu,
                records: recs,
                dxf: None,
            };
        }
    };

    let pv = view(&pattern);
    let pattern_rec = Record::new(
        Family::Claim,
        "pattern",
        json!({ "pattern": pv, "axes": { "provenance": "computed", "authority": "calculated", "validation": "candidate" } }),
        vec![rel("developed_from", &request.id)],
        Some(a_dev.id.clone()),
    );
    recs.push(pattern_rec.clone());

    let a_ver = activity("verify.family", "Z3", &[&request.id, &pattern_rec.id], 6000);
    wu += 6000;
    recs.push(a_ver.clone());
    let cviews: Vec<CheckView> = checks.iter().map(check_view).collect();
    let evidence = Record::new(Family::Assessment, "verification_evidence", json!({ "checks": cviews }), vec![rel("verifies", &pattern_rec.id)], Some(a_ver.id.clone()));
    recs.push(evidence.clone());

    let a_prof = activity("verify.profile", "Z3", &[&pattern_rec.id], 800);
    wu += 800;
    recs.push(a_prof.clone());
    let prof = profile::check(&pattern.outline, 1e-9);
    let prof_json = json!({ "closed": prof.closed, "max_gap": prof.max_gap, "self_intersections": prof.self_intersections, "signed_area": prof.signed_area, "vertices": prof.vertices });
    let prof_rec = Record::new(Family::Assessment, "profile_report", prof_json.clone(), vec![rel("verifies", &pattern_rec.id)], Some(a_prof.id.clone()));
    recs.push(prof_rec.clone());

    let a_dxf = activity("build.dxf", "Z2", &[&pattern_rec.id], 200);
    wu += 200;
    recs.push(a_dxf.clone());
    let entities = to_entities(&pattern);
    let text = nf_dxf::write(&entities);
    let dxf_rec = Record::new(
        Family::Claim,
        "dxf_output",
        json!({ "sha256": sha256_hex(text.as_bytes()), "bytes": text.len(), "entities": entities.len(), "format": "DXF AC1009 subset", "units": "mm (comment only)" }),
        vec![rel("rendered_from", &pattern_rec.id)],
        Some(a_dxf.id.clone()),
    );
    recs.push(dxf_rec.clone());

    let a_re = activity("verify.reimport", "Z3", &[&dxf_rec.id, &pattern_rec.id], 200);
    wu += 200;
    recs.push(a_re.clone());
    let re = nf_dxf::reimport_check(&entities, &text, 1e-8);
    let re_json = json!({ "pass": re.pass, "entities_written": re.entities_written, "entities_read": re.entities_read, "unknown_entities": re.unknown_entities,
                          "max_point_deviation": re.max_point_deviation, "max_length_deviation": re.max_length_deviation, "layers_match": re.layers_match });
    let re_rec = Record::new(Family::Assessment, "reimport_report", re_json.clone(), vec![rel("verifies", &dxf_rec.id)], Some(a_re.id.clone()));
    recs.push(re_rec.clone());

    let gates = evaluate_gates(&checks, Some(&prof), Some(re.pass), pattern.eps_rep > 0.0);
    let state = crate::release_state(&gates);
    let gate_rec = Record::new(
        Family::Assessment,
        "gate_evaluation",
        json!({ "gates": gates, "state": state }),
        vec![rel("evaluates", &dxf_rec.id), rel("evaluates", &pattern_rec.id), rel("uses", &evidence.id), rel("uses", &prof_rec.id), rel("uses", &re_rec.id)],
        None,
    );
    recs.push(gate_rec.clone());

    let map: BTreeMap<String, Record> = recs.iter().map(|r| (r.id.clone(), r.clone())).collect();
    let (ids, missing) = closure(&map, &[gate_rec.id.clone()]);
    debug_assert!(missing.is_empty());

    JobOutput {
        engine: ENGINE_VERSION.into(),
        request: req.clone(),
        state,
        outcome: None,
        gates,
        pattern: Some(pv),
        checks: cviews,
        profile: Some(prof_json),
        reimport: Some(re_json),
        closure_hash: closure_hash(&ids),
        closure_size: ids.len(),
        work_units: wu,
        records: recs,
        dxf: Some(text),
    }
}

/// Same-process replay audit: recompute from the request alone and compare
/// the closure hash and every record identity. (An *independent execution
/// instance*, as required before manufacturing release, is not implemented.)
pub fn replay_matches(req: &Request, entered_by: &str, expected_closure_hash: &str, expected_ids: &[String]) -> (bool, String) {
    let again = run(req, entered_by);
    let ids: Vec<String> = again.records.iter().map(|r| r.id.clone()).collect();
    let ok = again.closure_hash == expected_closure_hash && ids == expected_ids;
    (ok, again.closure_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reqs() -> Vec<Request> {
        vec![
            Request::CylinderShell { r_dev: 100.0, length: 250.0 },
            Request::ConeFrustum { r1: 200.0, r2: 100.0, h: 300.0 },
            Request::EccentricReducer { r1: 200.0, r2: 100.0, h: 300.0, e: 100.0 },
            Request::BranchStub { header_radius: 150.0, branch_radius: 100.0, branch_dev_radius: 100.0, beta_deg: 60.0, delta: 0.0, s_end: 400.0 },
        ]
    }

    #[test]
    fn jobs_run_verify_and_never_approve() {
        for r in reqs() {
            let o = run(&r, "tester");
            assert!(o.pattern.is_some() && o.dxf.is_some(), "{r:?}");
            assert!(o.checks.iter().all(|c| c.verdict == "PASS"), "{r:?}");
            assert_eq!(o.state, ReleaseState::ReviewRequired, "this build must never reach APPROVED");
            assert!(o.gates.iter().any(|g| g.id == "G16" && g.result == GateResult::NotImplemented));
            assert!(o.records.iter().all(|x| x.verify_id()));
            let pass = |id: &str| o.gates.iter().any(|g| g.id == id && g.result == GateResult::Pass);
            assert!(pass("G10") && pass("G11") && pass("G15"), "{r:?}");
        }
    }

    #[test]
    fn deterministic_replay() {
        for r in reqs() {
            let a = run(&r, "tester");
            let ids: Vec<String> = a.records.iter().map(|x| x.id.clone()).collect();
            let (ok, _) = replay_matches(&r, "tester", &a.closure_hash, &ids);
            assert!(ok, "{r:?}");
            assert_eq!(a.dxf, run(&r, "tester").dxf);
        }
        let a = run(&reqs()[1], "tester");
        let b = run(&Request::ConeFrustum { r1: 200.0, r2: 100.0, h: 300.5 }, "tester");
        assert_ne!(a.closure_hash, b.closure_hash);
        assert_ne!(a.closure_hash, run(&reqs()[1], "someone else").closure_hash, "who entered the request is part of the record");
    }

    #[test]
    fn unsupported_and_degenerate_inputs_give_no_dxf() {
        let o = run(&Request::ConeFrustum { r1: 200.0, r2: 199.99, h: 300.0 }, "t");
        assert!(o.dxf.is_none() && o.pattern.is_none());
        assert_eq!(o.state, ReleaseState::ReviewRequired);
        assert!(o.records.iter().any(|r| r.family == Family::Finding));
        let o = run(&Request::BranchStub { header_radius: 150.0, branch_radius: 100.0, branch_dev_radius: 100.0, beta_deg: 90.0, delta: 80.0, s_end: 400.0 }, "t");
        assert!(o.dxf.is_none());
    }

    #[test]
    fn failed_verification_rejects() {
        let p = develop::cone_frustum(206.0, 100.0, 300.0).unwrap();
        let checks = verify::verify_cone(&p, 200.0, 100.0, 300.0);
        let prof = profile::check(&p.outline, 1e-9);
        let gates = evaluate_gates(&checks, Some(&prof), Some(true), false);
        assert_eq!(crate::release_state(&gates), ReleaseState::Rejected);
        let gates = evaluate_gates(&verify::verify_cone(&develop::cone_frustum(200.0, 100.0, 300.0).unwrap(), 200.0, 100.0, 300.0), Some(&prof), Some(false), false);
        assert_eq!(crate::release_state(&gates), ReleaseState::Rejected, "failed re-import must reject");
    }
}
