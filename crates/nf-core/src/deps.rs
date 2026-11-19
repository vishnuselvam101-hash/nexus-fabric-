//! Typed dependency propagation and scrutiny obligations (Dependency Model,
//! docs/architecture/amoeba/DEPENDENCY_MODEL.md).
//!
//! The *mechanism* is implemented here. The *rule table* (which check a change
//! requires) is PROVISIONAL: it is an AI-written proposal, evidence level EV-1,
//! and none of it has been reviewed by a fabrication engineer (0/56).

use crate::job::JobOutput;
use crate::{Family, GateResult, Record};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const RULE_TABLE_VERSION: &str = "provisional-0.1 (EV-1, unreviewed)";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Derived from the changed record: invalid until recomputed and re-verified.
    Downstream,
    /// Mates with something affected: must be re-checked for fit.
    Lateral,
    /// Shares a cause (e.g. the same convention) with the changed record: suspect.
    CommonCause,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeClass {
    Derivation,
    Interface,
    CommonCause,
}

fn edge_class(rel: &str) -> Option<EdgeClass> {
    match rel {
        "computed_from" | "developed_from" | "rendered_from" | "defined_by" | "pattern_of" => {
            Some(EdgeClass::Derivation)
        }
        "mates_at" | "interface_feature" | "positioned_by" => Some(EdgeClass::Interface),
        "convention_of" => Some(EdgeClass::CommonCause),
        _ => None, // evidence, activity inputs, declarations: not propagation edges
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affected {
    pub record_id: String,
    pub kind: String,
    pub label: String,
    pub direction: Direction,
    /// Edge type through which the record was reached.
    pub via: String,
    pub required_check: String,
    pub mandatory: bool,
}

fn label(r: &Record) -> String {
    let p = &r.payload;
    p.get("part")
        .or_else(|| p.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| r.kind.clone())
}

/// Provisional rule table: (record kind, direction) -> required check.
fn rule(kind: &str, dir: Direction) -> Option<&'static str> {
    Some(match (kind, dir) {
        ("intersection_curve", Direction::Downstream) => {
            "Recompute the intersection curve from the changed parameters"
        }
        ("pattern", Direction::Downstream) => {
            "Recompute the development and pass family verification and the profile check"
        }
        ("dxf_output", Direction::Downstream) => {
            "Regenerate the DXF and pass the independent re-import check"
        }
        ("interface", Direction::Lateral) => {
            "Pass the interface check: both parts must reproduce the same joint curve"
        }
        ("part", Direction::Lateral) => "Re-check the fit of this part at the joint",
        ("parameter", Direction::CommonCause) => {
            "Engineer to re-confirm this value: it was entered under the same convention as the corrected one"
        }
        _ => return None,
    })
}

/// Everything affected by a change to `changed`, in deterministic order.
pub fn propagate(records: &BTreeMap<String, Record>, changed: &str) -> Vec<Affected> {
    // reverse index: who depends on X, and through which edge
    let mut dependents: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for r in records.values() {
        for e in &r.relations {
            if edge_class(&e.rel).is_some() {
                dependents
                    .entry(e.to.as_str())
                    .or_default()
                    .push((r.id.as_str(), e.rel.as_str()));
            }
        }
    }
    let mut out: Vec<Affected> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::from([changed.to_string()]);
    let add = |out: &mut Vec<Affected>, r: &Record, dir: Direction, via: &str| {
        let Some(check) = rule(&r.kind, dir) else {
            return;
        };
        if !out
            .iter()
            .any(|a| a.record_id == r.id && a.direction == dir)
        {
            out.push(Affected {
                record_id: r.id.clone(),
                kind: r.kind.clone(),
                label: label(r),
                direction: dir,
                via: via.to_string(),
                required_check: check.to_string(),
                mandatory: true,
            });
        }
    };
    // Downstream: unbounded traversal over derivation edges. Interfaces touched on the way create lateral obligations.
    let mut queue: VecDeque<String> = VecDeque::from([changed.to_string()]);
    let mut touched_interfaces: BTreeSet<String> = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        for (dep, via) in dependents.get(id.as_str()).cloned().unwrap_or_default() {
            let Some(r) = records.get(dep) else { continue };
            if r.family != Family::Claim {
                continue;
            }
            match edge_class(via) {
                Some(EdgeClass::Derivation) if seen.insert(dep.to_string()) => {
                    add(&mut out, r, Direction::Downstream, via);
                    queue.push_back(dep.to_string());
                }
                Some(EdgeClass::Interface) => {
                    touched_interfaces.insert(dep.to_string());
                }
                _ => {}
            }
        }
    }
    // Lateral: each touched interface, and every part mating at it.
    for iid in &touched_interfaces {
        let r = &records[iid];
        add(&mut out, r, Direction::Lateral, "interface_feature");
        for e in r.relations.iter().filter(|e| e.rel == "mates_at") {
            if let Some(p) = records.get(&e.to) {
                add(&mut out, p, Direction::Lateral, "mates_at");
            }
        }
    }
    // Common cause: siblings under the same convention as the changed record.
    if let Some(ch) = records.get(changed) {
        for e in ch
            .relations
            .iter()
            .filter(|e| edge_class(&e.rel) == Some(EdgeClass::CommonCause))
        {
            for (sib, via) in dependents.get(e.to.as_str()).cloned().unwrap_or_default() {
                if sib != changed && via == "convention_of" {
                    add(&mut out, &records[sib], Direction::CommonCause, via);
                }
            }
        }
    }
    out.sort_by(|a, b| (a.direction, &a.kind, &a.label).cmp(&(b.direction, &b.kind, &b.label)));
    out
}

/// Can evidence from the recomputed job discharge this obligation?
/// Common-cause obligations are never discharged automatically.
pub fn discharge(a: &Affected, new: &JobOutput) -> (bool, String) {
    let gate_pass = |id: &str| {
        new.gates
            .iter()
            .any(|g| g.id == id && g.result == GateResult::Pass)
    };
    let scope_pass = |scope: &str| {
        let v: Vec<_> = new.checks.iter().filter(|c| c.scope == scope).collect();
        !v.is_empty() && v.iter().all(|c| c.verdict == "PASS")
    };
    let has_part = new.patterns.iter().any(|p| p.part == a.label);
    match (a.kind.as_str(), a.direction) {
        (_, Direction::CommonCause) => (false, "requires an engineer's decision".into()),
        ("pattern", _) if has_part && scope_pass(&a.label) && gate_pass("G11") => (
            true,
            format!(
                "recomputed; verification checks for '{}' and the profile check pass",
                a.label
            ),
        ),
        ("dxf_output", _) if has_part && gate_pass("G15") => (
            true,
            "regenerated; independent re-import check passes".into(),
        ),
        ("intersection_curve", _) if scope_pass("interface") => (
            true,
            "recomputed; both parts reproduce the same joint curve".into(),
        ),
        ("interface", _) | ("part", _) if scope_pass("interface") => (
            true,
            "interface check passes on the recomputed parts".into(),
        ),
        _ => (false, "no passing evidence in the recomputed job".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Request, run};

    fn assembly() -> Request {
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
            surface_reference: "outside".into(),
        }
    }

    fn graph(o: &JobOutput) -> BTreeMap<String, Record> {
        o.records
            .iter()
            .map(|r| (r.id.clone(), r.clone()))
            .collect()
    }

    fn param(o: &JobOutput, name: &str) -> String {
        o.records
            .iter()
            .find(|r| r.kind == "parameter" && r.payload["name"] == name)
            .unwrap()
            .id
            .clone()
    }

    fn has(v: &[Affected], kind: &str, label: &str, dir: Direction) -> bool {
        v.iter()
            .any(|a| a.kind == kind && a.label == label && a.direction == dir)
    }

    #[test]
    fn header_radius_change_reaches_both_parts_the_interface_and_sibling_radii() {
        let o = run(&assembly(), "t");
        let a = propagate(&graph(&o), &param(&o, "header_radius"));
        // downstream: the shared curve, both patterns, both cutting files
        assert!(has(
            &a,
            "intersection_curve",
            "intersection_curve",
            Direction::Downstream
        ));
        for part in ["header", "branch"] {
            assert!(
                has(&a, "pattern", part, Direction::Downstream),
                "pattern {part}"
            );
            assert!(
                has(&a, "dxf_output", part, Direction::Downstream),
                "dxf {part}"
            );
            assert!(has(&a, "part", part, Direction::Lateral), "fit of {part}");
        }
        assert!(has(
            &a,
            "interface",
            "header-branch joint",
            Direction::Lateral
        ));
        // common cause: the other radii entered under the same convention — not the lengths or angles
        let cc: Vec<&str> = a
            .iter()
            .filter(|x| x.direction == Direction::CommonCause)
            .map(|x| x.label.as_str())
            .collect();
        assert_eq!(
            cc,
            ["branch_dev_radius", "branch_radius", "header_dev_radius"]
        );
        assert!(a.iter().all(|x| x.mandatory));
        assert_eq!(
            a,
            propagate(&graph(&o), &param(&o, "header_radius")),
            "propagation must be deterministic"
        );
    }

    #[test]
    fn local_change_stays_local() {
        let o = run(&assembly(), "t");
        let a = propagate(&graph(&o), &param(&o, "header_length"));
        assert!(
            has(&a, "pattern", "header", Direction::Downstream)
                && has(&a, "dxf_output", "header", Direction::Downstream)
        );
        assert!(
            !a.iter()
                .any(|x| x.label == "branch" && x.direction == Direction::Downstream),
            "branch pattern does not depend on header length"
        );
        assert!(
            !a.iter().any(|x| x.direction == Direction::CommonCause),
            "a length is not under the radius convention"
        );
        let a = propagate(&graph(&o), &param(&o, "s_end"));
        assert!(
            has(&a, "pattern", "branch", Direction::Downstream)
                && !has(&a, "pattern", "header", Direction::Downstream)
        );
    }

    #[test]
    fn recomputation_discharges_everything_except_common_cause() {
        let old = run(&assembly(), "t");
        let affected = propagate(&graph(&old), &param(&old, "header_radius"));
        let new = run(&assembly().with_param("header_radius", 142.0).unwrap(), "t");
        for a in &affected {
            let (ok, _) = discharge(a, &new);
            assert_eq!(ok, a.direction != Direction::CommonCause, "{a:?}");
        }
        // if the corrected request cannot be developed, nothing is discharged
        let broken = run(&assembly().with_param("header_radius", 90.0).unwrap(), "t");
        assert!(affected.iter().all(|a| !discharge(a, &broken).0));
    }
}
