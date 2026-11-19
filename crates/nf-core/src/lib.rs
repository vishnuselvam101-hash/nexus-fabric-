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
//!   `release_state` cannot return it, whatever the gate table says, because
//!   engineer approval (G16) is not implemented;
//! - typed dependency edges, propagation of a change along them and scrutiny
//!   obligations for everything affected (`deps`, provisional rule table);
//! - an engine identifier derived from the engine sources at build time and
//!   recorded with every result.
//!
//! Not implemented (see docs/IMPLEMENTATION_STATUS.md): scheduler/rounds,
//! exploration, investigations, coverage ledger, lanes/overlays, quarantine,
//! engineer approval.

pub mod deps;
pub mod job;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Identifier of the engine that computes results: a release number that is
/// raised by hand with every change of algorithm or output, plus a fingerprint
/// of the uem, nf-dxf and nf-core sources taken at build time (build.rs), so
/// that a result of one build can never be mistaken for a result of another.
/// It is recorded in every result and in every activity record.
///
/// 0.3.0 (19 Nov 2026): branch patterns are drawn from the OUTSIDE like header
/// plates (validation defect D1; up to 0.2.0 they were the mirror image), and
/// every pattern declares its marking side. Results of 0.2.0 and earlier are
/// results of another engine: REJECTED with gate G21, "recompute".
pub const ENGINE_VERSION: &str = concat!("nf-engine 0.3.0+src.", env!("NF_ENGINE_SRC_HASH"));

/// Engine identifiers known to have produced wrong or unidentifiable results.
/// Stored results carrying one of these (or any identifier other than the
/// running engine's) are not trusted: they must be recomputed.
pub const SUPERSEDED_ENGINES: [(&str, &str); 1] = [(
    "nf-core 0.1.0 / uem 0.1.0",
    "This identifier was used unchanged by every build up to 2026-11-19, including builds before the header-outline fix (docs/LEARNING_AND_STRESS_TESTING.md, Defect 002: header plate outline and branch opening in different frames). A result carrying it cannot be told apart from one computed with that defect.",
)];

pub fn superseded_engine_reason(engine: &str) -> Option<&'static str> {
    SUPERSEDED_ENGINES
        .iter()
        .find(|(id, _)| *id == engine)
        .map(|(_, why)| *why)
}

/// Only results computed by exactly the running engine are current.
pub fn engine_is_current(engine: &str) -> bool {
    engine == ENGINE_VERSION && superseded_engine_reason(engine).is_none()
}

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
    pub fn new(
        family: Family,
        kind: &str,
        payload: Value,
        relations: Vec<Rel>,
        activity: Option<String>,
    ) -> Record {
        let body = json!({ "family": family, "kind": kind, "payload": payload, "relations": relations, "activity": activity });
        let id = sha256_hex(canonical(&body).as_bytes());
        Record {
            id,
            family,
            kind: kind.to_string(),
            payload,
            relations,
            activity,
        }
    }

    /// Recompute the identity from content (used by audits and on load).
    pub fn verify_id(&self) -> bool {
        Record::new(
            self.family,
            &self.kind,
            self.payload.clone(),
            self.relations.clone(),
            self.activity.clone(),
        )
        .id == self.id
    }
}

pub fn rel(name: &str, to: &str) -> Rel {
    Rel {
        rel: name.to_string(),
        to: to.to_string(),
    }
}

/// Smallest set containing `roots` and closed under relations (and producing
/// activities). Records missing from `records` are reported, never ignored.
pub fn closure(
    records: &BTreeMap<String, Record>,
    roots: &[String],
) -> (BTreeSet<String>, Vec<String>) {
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
    let joined: String = ids
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\n");
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
    /// Defined by Spec 02 §10.3 and kept so that the name stays reserved.
    /// NOT REACHABLE in this build: no function produces it.
    Approved,
    ReviewRequired,
    Rejected,
    /// The engine refused the request and computed nothing. Not a result:
    /// there is nothing to review, correct, replay or download.
    Refused,
}

/// Gates every computed result must carry. A stored gate table that lacks one
/// of them is incomplete, and an incomplete table is a failed one.
pub const REQUIRED_GATES: [&str; 12] = [
    "G01", "G02", "G05", "G06", "G08", "G10", "G11", "G14", "G15", "G16", "G18", "G19",
];

/// Release state from gate results (Spec 02 §10.3). A model confidence score
/// is not an input to this function, and cannot be.
///
/// Fail closed: an empty gate table is REJECTED (no evidence is not good
/// evidence), and so is any critical FAIL. Spec 02 §10.3 would give APPROVED
/// when every gate passes; engineer approval is not implemented in this
/// build, so that case is REVIEW_REQUIRED here and APPROVED cannot be returned.
pub fn release_state(gates: &[Gate]) -> ReleaseState {
    if gates.is_empty()
        || gates
            .iter()
            .any(|g| g.critical && g.result == GateResult::Fail)
    {
        ReleaseState::Rejected
    } else {
        ReleaseState::ReviewRequired
    }
}

/// Complete a gate table against `REQUIRED_GATES`: every required gate that is
/// absent is added as a critical FAIL. Used wherever a table comes from
/// storage rather than from the computation that produced it.
pub fn with_required_gates(mut gates: Vec<Gate>) -> Vec<Gate> {
    for id in REQUIRED_GATES {
        if !gates.iter().any(|g| g.id == id) {
            gates.push(Gate { id: id.into(), name: "missing_gate".into(), critical: true, result: GateResult::Fail, detail: format!("Gate {id} is missing from the stored result. An incomplete gate table is treated as failed (integrity).") });
        }
    }
    gates
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
        let b = Record::new(
            Family::Claim,
            "b",
            json!(2),
            vec![rel("computed_from", &a.id)],
            None,
        );
        let c = Record::new(
            Family::Claim,
            "c",
            json!(3),
            vec![rel("computed_from", &b.id), rel("x", "deadbeef")],
            None,
        );
        let unrelated = Record::new(Family::Claim, "u", json!(4), vec![], None);
        let mut m = BTreeMap::new();
        for r in [&a, &b, &c, &unrelated] {
            m.insert(r.id.clone(), r.clone());
        }
        let (ids, missing) = closure(&m, std::slice::from_ref(&c.id));
        assert!(ids.contains(&a.id) && ids.contains(&b.id) && !ids.contains(&unrelated.id));
        assert_eq!(missing, vec!["deadbeef".to_string()]);
        // unrelated additions do not change the closure hash
        let h1 = closure_hash(&ids);
        let more = Record::new(Family::Claim, "v", json!(5), vec![], None);
        m.insert(more.id.clone(), more);
        assert_eq!(
            closure_hash(&closure(&m, std::slice::from_ref(&c.id)).0),
            h1
        );
    }

    #[test]
    fn release_state_rules() {
        let g = |critical, result| Gate {
            id: "G".into(),
            name: "n".into(),
            critical,
            result,
            detail: String::new(),
        };
        assert_eq!(
            release_state(&[g(true, GateResult::Pass), g(true, GateResult::Fail)]),
            ReleaseState::Rejected
        );
        assert_eq!(
            release_state(&[g(true, GateResult::Pass), g(false, GateResult::Fail)]),
            ReleaseState::ReviewRequired
        );
        assert_eq!(
            release_state(&[
                g(true, GateResult::Pass),
                g(true, GateResult::NotImplemented)
            ]),
            ReleaseState::ReviewRequired
        );
        assert_eq!(
            release_state(&[g(true, GateResult::Pass), g(true, GateResult::Inconclusive)]),
            ReleaseState::ReviewRequired
        );
        // BE-002: this assertion used to expect APPROVED. APPROVED must be unreachable in this build
        // (G16, engineer approval, is not implemented), and "no gates" must fail closed.
        assert_eq!(
            release_state(&[g(true, GateResult::Pass)]),
            ReleaseState::ReviewRequired
        );
        assert_eq!(
            release_state(&[]),
            ReleaseState::Rejected,
            "an empty gate table is no evidence"
        );
    }

    /// BE-002: no gate table, however favourable or malformed, yields APPROVED.
    #[test]
    fn approved_is_unreachable_for_every_gate_table() {
        use GateResult::*;
        let all = [
            Pass,
            Fail,
            Inconclusive,
            NotImplemented,
            NotRun,
            NotApplicable,
        ];
        let g = |id: &str, critical, result| Gate {
            id: id.into(),
            name: "n".into(),
            critical,
            result,
            detail: String::new(),
        };
        let mut tables: Vec<Vec<Gate>> = vec![
            vec![],
            REQUIRED_GATES.iter().map(|id| g(id, true, Pass)).collect(),
            REQUIRED_GATES
                .iter()
                .map(|id| g(id, false, NotApplicable))
                .collect(),
        ];
        for a in all {
            for b in all {
                for critical in [true, false] {
                    tables.push(vec![g("G10", critical, a), g("G16", critical, b)]);
                }
            }
        }
        for t in &tables {
            assert_ne!(release_state(t), ReleaseState::Approved, "{t:?}");
            assert_ne!(
                release_state(&with_required_gates(t.clone())),
                ReleaseState::Approved
            );
        }
        // a table that lacks required gates is rejected once completed
        assert_eq!(
            release_state(&with_required_gates(vec![g("G20", true, Pass)])),
            ReleaseState::Rejected
        );
        assert_eq!(
            release_state(&with_required_gates(vec![])),
            ReleaseState::Rejected
        );
        let full: Vec<Gate> = REQUIRED_GATES.iter().map(|id| g(id, true, Pass)).collect();
        assert_eq!(
            with_required_gates(full.clone()),
            full,
            "a complete table is left as it is"
        );
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use super::*;

    /// Records are stored as JSON text. Identity must survive storage exactly,
    /// which requires exact float round-tripping (serde_json `float_roundtrip`).
    #[test]
    fn records_survive_json_storage_bit_exactly() {
        let asm = job::Request::HeaderWithBranch {
            header_radius: 150.0,
            header_dev_radius: 150.0,
            header_length: 1200.0,
            branch_station: 600.0,
            branch_radius: 100.0,
            branch_dev_radius: 100.0,
            beta_deg: 60.0,
            delta: 12.5,
            s_end: 400.0,
            surface_reference: "outside".into(),
        };
        let mut all = job::run(
            &job::Request::EccentricReducer {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
                e: 100.0,
            },
            "t",
        )
        .records;
        all.extend(job::run(&asm, "t").records);
        for r in &all {
            let text = serde_json::to_string(r).unwrap();
            let back: Record = serde_json::from_str(&text).unwrap();
            assert!(
                back.verify_id(),
                "record {} ({}) changed identity after a JSON round trip",
                r.kind,
                r.id
            );
        }
    }
}
