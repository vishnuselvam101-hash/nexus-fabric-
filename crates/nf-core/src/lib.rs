//! NEXUS FABRIC core: AMOEBA record model (first subset) and the development job.
//!
//! Implemented here:
//! - immutable, content-addressed records in the seven families of the
//!   consolidated record model (identity = SHA-256 of canonical JSON; no
//!   wall-clock data in identities);
//! - typed relations between records and the release closure with its hash;
//! - a deterministic development job (develop → verify → profile → DXF →
//!   re-import → gates) whose every step is a recorded Activity with logical
//!   work units;
//! - release-state evaluation. `APPROVED` is not reachable in this build:
//!   engineer approval, obligation propagation and independent-instance replay
//!   are not implemented, and the gate evaluation says so explicitly.
//!
//! Not implemented (see docs/IMPLEMENTATION_STATUS.md): scheduler/rounds,
//! exploration, investigations, dependency obligations, coverage ledger,
//! lanes/overlays, quarantine.

pub mod deps;
pub mod job;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const ENGINE_VERSION: &str = "nf-core 0.1.0 / uem 0.1.0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Source,
    Claim,
    Relation,
    Activity,
    Finding,
    Assessment,
    Attestation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rel {
    /// Typed edge: this record depends on / refers to `to`.
    pub rel: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub family: Family,
    pub kind: String,
    pub payload: Value,
    pub relations: Vec<Rel>,
    /// Producing activity (record id), if any.
    pub activity: Option<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    hex::encode(&d[..])
}

/// Canonical serialisation: serde_json with sorted object keys (the default
/// map type). Floats use the shortest round-trip representation, which is
/// deterministic for identical binary64 values.
fn canonical(v: &Value) -> String {
    serde_json::to_string(v).expect("json")
}

impl Record {
    pub fn new(family: Family, kind: &str, payload: Value, relations: Vec<Rel>, activity: Option<String>) -> Record {
        let body = json!({ "family": family, "kind": kind, "payload": payload, "relations": relations, "activity": activity });
        let id = sha256_hex(canonical(&body).as_bytes());
        Record { id, family, kind: kind.to_string(), payload, relations, activity }
    }

    /// Recompute the identity from content (used by audits and on load).
    pub fn verify_id(&self) -> bool {
        Record::new(self.family, &self.kind, self.payload.clone(), self.relations.clone(), self.activity.clone()).id == self.id
    }
}

pub fn rel(name: &str, to: &str) -> Rel {
    Rel { rel: name.to_string(), to: to.to_string() }
}

/// Smallest set containing `roots` and closed under relations (and producing
/// activities). Records missing from `records` are reported, never ignored.
pub fn closure(records: &BTreeMap<String, Record>, roots: &[String]) -> (BTreeSet<String>, Vec<String>) {
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();
    let mut stack: Vec<String> = roots.to_vec();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        match records.get(&id) {
            None => missing.push(id),
            Some(r) => {
                for e in &r.relations {
                    stack.push(e.to.clone());
                }
                if let Some(a) = &r.activity {
                    stack.push(a.clone());
                }
            }
        }
    }
    (seen, missing)
}

/// Hash of a release closure: SHA-256 over the sorted record ids.
pub fn closure_hash(ids: &BTreeSet<String>) -> String {
    let joined: String = ids.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n");
    sha256_hex(joined.as_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateResult {
    Pass,
    Fail,
    Inconclusive,
    NotImplemented,
    NotRun,
    NotApplicable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    pub id: String,
    pub name: String,
    pub critical: bool,
    pub result: GateResult,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseState {
    Approved,
    ReviewRequired,
    Rejected,
}

/// Release state from gate results (Spec 02 §10.3). A model confidence score
/// is not an input to this function, and cannot be.
pub fn release_state(gates: &[Gate]) -> ReleaseState {
    if gates.iter().any(|g| g.critical && g.result == GateResult::Fail) {
        ReleaseState::Rejected
    } else if gates.iter().all(|g| matches!(g.result, GateResult::Pass | GateResult::NotApplicable)) {
        ReleaseState::Approved
    } else {
        ReleaseState::ReviewRequired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_identity_is_content_addressed() {
        let a = Record::new(Family::Claim, "x", json!({"b": 1, "a": 2.5}), vec![], None);
        let b = Record::new(Family::Claim, "x", json!({"a": 2.5, "b": 1}), vec![], None);
        assert_eq!(a.id, b.id, "key order must not matter");
        let c = Record::new(Family::Claim, "x", json!({"a": 2.5, "b": 2}), vec![], None);
        assert_ne!(a.id, c.id);
        assert!(a.verify_id());
        let mut tampered = a.clone();
        tampered.payload = json!({"a": 2.5, "b": 99});
        assert!(!tampered.verify_id());
    }

    #[test]
    fn closure_follows_relations_and_reports_missing() {
        let a = Record::new(Family::Claim, "a", json!(1), vec![], None);
        let b = Record::new(Family::Claim, "b", json!(2), vec![rel("computed_from", &a.id)], None);
        let c = Record::new(Family::Claim, "c", json!(3), vec![rel("computed_from", &b.id), rel("x", "deadbeef")], None);
        let unrelated = Record::new(Family::Claim, "u", json!(4), vec![], None);
        let mut m = BTreeMap::new();
        for r in [&a, &b, &c, &unrelated] {
            m.insert(r.id.clone(), r.clone());
        }
        let (ids, missing) = closure(&m, &[c.id.clone()]);
        assert!(ids.contains(&a.id) && ids.contains(&b.id) && !ids.contains(&unrelated.id));
        assert_eq!(missing, vec!["deadbeef".to_string()]);
        // unrelated additions do not change the closure hash
        let h1 = closure_hash(&ids);
        let more = Record::new(Family::Claim, "v", json!(5), vec![], None);
        m.insert(more.id.clone(), more);
        assert_eq!(closure_hash(&closure(&m, &[c.id.clone()]).0), h1);
    }

    #[test]
    fn release_state_rules() {
        let g = |critical, result| Gate { id: "G".into(), name: "n".into(), critical, result, detail: String::new() };
        assert_eq!(release_state(&[g(true, GateResult::Pass), g(true, GateResult::Fail)]), ReleaseState::Rejected);
        assert_eq!(release_state(&[g(true, GateResult::Pass), g(false, GateResult::Fail)]), ReleaseState::ReviewRequired);
        assert_eq!(release_state(&[g(true, GateResult::Pass), g(true, GateResult::NotImplemented)]), ReleaseState::ReviewRequired);
        assert_eq!(release_state(&[g(true, GateResult::Pass), g(true, GateResult::Inconclusive)]), ReleaseState::ReviewRequired);
        assert_eq!(release_state(&[g(true, GateResult::Pass)]), ReleaseState::Approved);
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use super::*;

    /// Records are stored as JSON text. Identity must survive storage exactly,
    /// which requires exact float round-tripping (serde_json `float_roundtrip`).
    #[test]
    fn records_survive_json_storage_bit_exactly() {
        let asm = job::Request::HeaderWithBranch { header_radius: 150.0, header_dev_radius: 150.0, header_length: 1200.0, branch_station: 600.0, branch_radius: 100.0, branch_dev_radius: 100.0, beta_deg: 60.0, delta: 12.5, s_end: 400.0, surface_reference: "outside".into() };
        let mut all = job::run(&job::Request::EccentricReducer { r1: 200.0, r2: 100.0, h: 300.0, e: 100.0 }, "t").records;
        all.extend(job::run(&asm, "t").records);
        for r in &all {
            let text = serde_json::to_string(r).unwrap();
            let back: Record = serde_json::from_str(&text).unwrap();
            assert!(back.verify_id(), "record {} ({}) changed identity after a JSON round trip", r.kind, r.id);
        }
    }
}
