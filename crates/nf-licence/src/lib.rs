//! Offline licence format, verification and state machine for NEXUS FABRIC.
//!
//! A licence is a small JSON envelope `{"payload":{...},"signature":"<128 hex>"}`.
//! The signed bytes are the compact serde_json serialization of the payload struct
//! (fixed field order, UTF-8, no newline); the verifier re-serializes the parsed
//! payload and checks the Ed25519 signature over exactly those bytes, so any edit
//! or reformatting of the signed content fails. Pure Ed25519, no prehash.
//!
//! The state machine is a pure function with time injected: nothing here reads a
//! clock, a file or a database. Grace and evaluation lengths are code constants,
//! not file fields. Nothing is ever deleted or locked; a licence only decides
//! whether NEW work is refused, and only when enforcement is switched on.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Largest accepted licence file (bytes).
pub const MAX_FILE_BYTES: usize = 8 * 1024;
/// One day in seconds.
pub const DAY: u64 = 86_400;
/// Full-function grace period after `valid_until`.
pub const GRACE: u64 = 14 * DAY;
/// Evaluation length from first run when no licence was ever verified.
pub const EVALUATION: u64 = 30 * DAY;
/// A clock reading this many seconds behind `last_seen` is not a rollback.
const ROLLBACK_TOLERANCE: u64 = 60;

/// Public keys compiled into the program, keyed by `key_id`. Rotation: a new
/// release appends a new key; old keys stay until all their monthly licences
/// expire; removing a key in a later release revokes it.
///
/// The key below is a development key, replaced by the owner's key before any
/// sale. Its private seed exists only in test code (`DEV_TEST_SEED`).
pub static VERIFY_KEYS: &[(&str, [u8; 32])] = &[(
    "39bd3a8a",
    [
        0x3e, 0x16, 0x40, 0xca, 0x26, 0x76, 0x7c, 0xa5, 0x7d, 0x98, 0x0b, 0x23, 0x4c, 0x12, 0x38,
        0x2f, 0x2a, 0xc4, 0xfa, 0xd7, 0x56, 0x49, 0x0e, 0xbd, 0xa4, 0x24, 0xb0, 0xd4, 0x1b, 0xe9,
        0xb5, 0xd2,
    ],
)];

/// Which product tier a licence covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    Pilot,
    Standard,
    Site,
}

/// The signed content of a licence. Field order is the canonical byte order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub v: u32,
    pub licence_id: String,
    pub customer: String,
    pub seats: u32,
    pub edition: Edition,
    pub valid_from: u64,
    pub valid_until: u64,
    pub issued_at: u64,
    pub issuer: String,
    pub key_id: String,
}

/// The licence file: payload plus hex-encoded Ed25519 signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub payload: Payload,
    pub signature: String,
}

/// Why a licence file was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidReason {
    /// File larger than `MAX_FILE_BYTES`.
    Oversize,
    /// Not the envelope JSON, unknown field, bad signature encoding, wrong version.
    Malformed,
    /// `key_id` is not in the compiled key registry.
    UnknownKey,
    /// Signature does not verify over the canonical payload bytes.
    BadSignature,
    /// Structurally signed but semantically invalid (seats 0, empty validity window).
    InvalidFields,
}

impl std::fmt::Display for InvalidReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            InvalidReason::Oversize => "licence file is too large",
            InvalidReason::Malformed => "licence file is malformed",
            InvalidReason::UnknownKey => "licence key_id is not a known issuer key",
            InvalidReason::BadSignature => "licence signature does not verify",
            InvalidReason::InvalidFields => "licence fields are invalid",
        };
        f.write_str(s)
    }
}

impl std::error::Error for InvalidReason {}

/// The exact bytes that are signed: compact JSON of the payload, struct field
/// order, no newline. CLI and verifier share this function, so the bytes are
/// identical by construction.
pub fn canonical_bytes(payload: &Payload) -> Result<Vec<u8>, String> {
    serde_json::to_vec(payload).map_err(|e| format!("cannot serialize licence payload: {e}"))
}

/// `key_id` = first 8 hex characters of SHA-256 of the public key.
pub fn key_id(public_key: &[u8; 32]) -> String {
    let hash = Sha256::digest(public_key);
    hex::encode(&hash[..4])
}

/// The public key that belongs to a 32-byte signing seed.
pub fn public_key_from_seed(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

/// Sign a payload with a 32-byte seed and return the envelope. The caller sets
/// `key_id` (the CLI derives it from the seed); the payload is signed as given.
pub fn sign(payload: &Payload, seed: &[u8; 32]) -> Result<Envelope, String> {
    let bytes = canonical_bytes(payload)?;
    let signature = SigningKey::from_bytes(seed).sign(&bytes);
    Ok(Envelope {
        payload: payload.clone(),
        signature: hex::encode(signature.to_bytes()),
    })
}

/// Verify a licence file against a key registry and return the signed payload.
/// The server passes `VERIFY_KEYS`; tests pass their own registry.
pub fn verify_with_keys(bytes: &[u8], keys: &[(&str, [u8; 32])]) -> Result<Payload, InvalidReason> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(InvalidReason::Oversize);
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| InvalidReason::Malformed)?;
    let sig_bytes: [u8; 64] = hex::decode(&envelope.signature)
        .map_err(|_| InvalidReason::Malformed)?
        .try_into()
        .map_err(|_| InvalidReason::Malformed)?;
    if envelope.payload.v != 1 {
        return Err(InvalidReason::Malformed);
    }
    let key = keys
        .iter()
        .find(|(id, _)| *id == envelope.payload.key_id)
        .map(|(_, k)| k)
        .ok_or(InvalidReason::UnknownKey)?;
    let verifying = VerifyingKey::from_bytes(key).map_err(|_| InvalidReason::UnknownKey)?;
    let canonical = canonical_bytes(&envelope.payload).map_err(|_| InvalidReason::Malformed)?;
    verifying
        .verify(&canonical, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| InvalidReason::BadSignature)?;
    if envelope.payload.seats == 0 || envelope.payload.valid_until <= envelope.payload.valid_from {
        return Err(InvalidReason::InvalidFields);
    }
    Ok(envelope.payload)
}

/// The licence state. `ReadOnly` refuses new work (when enforcement is on);
/// every state keeps all existing jobs, records and files readable/exportable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// No verified licence: full function for `EVALUATION` from first run.
    /// `note` carries "not yet valid" for a licence whose window has not opened.
    Evaluation {
        days_left: u64,
        note: Option<&'static str>,
    },
    /// Inside the paid window.
    Valid,
    /// Past `valid_until`, full function with a banner until `grace_until`.
    Grace { grace_until: u64 },
    /// New work refused; reason is a stable snake_case identifier.
    ReadOnly { reason: &'static str },
}

/// Evaluation result: the true state plus the clock-rollback flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub state: State,
    /// `now` was more than `ROLLBACK_TOLERANCE` behind `last_seen`.
    pub clock_rollback: bool,
}

impl Status {
    /// Whether new work must be refused. `enforce` is the owner's release-gate
    /// switch: while it is false nothing is ever blocked, whatever the state.
    /// The state itself is always computed truthfully by `evaluate`.
    pub fn blocks_new_work(&self, enforce: bool) -> bool {
        enforce && matches!(self.state, State::ReadOnly { .. })
    }
}

/// Pure licence state machine; time is injected, nothing is read or written.
///
/// - `payload`: the verified file payload, if the file verified this run.
/// - `now`: current unix seconds. `last_seen`: the persisted monotonic high
///   water mark; effective time is `max(now, last_seen)`, so a rolled-back
///   clock can never improve the state nor extend grace.
/// - `eval_start`: persisted first-run time of the evaluation (None = starts now).
/// - `last_verified`: persisted payload of the last licence that ever verified.
///   An invalid or removed file falls back to it — never to a fresh trial.
pub fn evaluate(
    payload: Option<&Payload>,
    now: u64,
    last_seen: u64,
    eval_start: Option<u64>,
    last_verified: Option<&Payload>,
) -> Status {
    let clock_rollback = last_seen > now.saturating_add(ROLLBACK_TOLERANCE);
    let t = now.max(last_seen);
    let state = match payload.or(last_verified) {
        Some(p) => licensed_state(p, t, eval_start),
        None => evaluation_state(t, eval_start),
    };
    Status {
        state,
        clock_rollback,
    }
}

fn eval_days_left(t: u64, eval_start: Option<u64>) -> u64 {
    let start = eval_start.unwrap_or(t);
    EVALUATION
        .saturating_sub(t.saturating_sub(start))
        .div_ceil(DAY)
}

fn evaluation_state(t: u64, eval_start: Option<u64>) -> State {
    let start = eval_start.unwrap_or(t);
    if t.saturating_sub(start) < EVALUATION {
        State::Evaluation {
            days_left: eval_days_left(t, eval_start),
            note: None,
        }
    } else {
        State::ReadOnly {
            reason: "evaluation_expired",
        }
    }
}

fn licensed_state(p: &Payload, t: u64, eval_start: Option<u64>) -> State {
    if t < p.valid_from {
        // A licence that is not yet valid never locks: evaluation continues.
        return State::Evaluation {
            days_left: eval_days_left(t, eval_start),
            note: Some("not yet valid"),
        };
    }
    let grace_until = p.valid_until.saturating_add(GRACE);
    if t < p.valid_until {
        State::Valid
    } else if t < grace_until {
        State::Grace { grace_until }
    } else {
        State::ReadOnly {
            reason: "licence_expired",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Development key, replaced by the owner's key before any sale.
    /// Matches the development entry in `VERIFY_KEYS`.
    const DEV_TEST_SEED: [u8; 32] = [
        65, 41, 102, 160, 89, 98, 20, 92, 203, 222, 5, 44, 127, 163, 111, 170, 247, 81, 158, 210,
        113, 97, 16, 248, 97, 227, 46, 47, 218, 120, 156, 231,
    ];

    const T0: u64 = 1_000_000_000;

    fn dev_payload() -> Payload {
        Payload {
            v: 1,
            licence_id: "LIC-TEST-0001".to_string(),
            customer: "Test Customer".to_string(),
            seats: 2,
            edition: Edition::Standard,
            valid_from: T0,
            valid_until: T0 + 30 * DAY,
            issued_at: T0 - DAY,
            issuer: "Chuo-Sogo".to_string(),
            key_id: key_id(&public_key_from_seed(&DEV_TEST_SEED)),
        }
    }

    fn signed_bytes(payload: &Payload, seed: &[u8; 32]) -> Vec<u8> {
        let envelope = sign(payload, seed).unwrap();
        serde_json::to_vec(&envelope).unwrap()
    }

    // (1) sign/verify round trip, against the compiled registry and a custom one.
    #[test]
    fn sign_verify_round_trip() {
        let payload = dev_payload();
        let bytes = signed_bytes(&payload, &DEV_TEST_SEED);
        assert_eq!(verify_with_keys(&bytes, VERIFY_KEYS), Ok(payload.clone()));
        let custom: &[(&str, [u8; 32])] =
            &[(&payload.key_id, public_key_from_seed(&DEV_TEST_SEED))];
        assert_eq!(verify_with_keys(&bytes, custom), Ok(payload));
    }

    // (2) tampered payload rejected.
    #[test]
    fn tampered_payload_rejected() {
        let bytes = signed_bytes(&dev_payload(), &DEV_TEST_SEED);
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["payload"]["customer"] = serde_json::Value::String("Someone Else".to_string());
        let tampered = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            verify_with_keys(&tampered, VERIFY_KEYS),
            Err(InvalidReason::BadSignature)
        );
    }

    // (3) signature from a different seed rejected even with a known key_id.
    #[test]
    fn wrong_key_rejected() {
        let bytes = signed_bytes(&dev_payload(), &[7u8; 32]);
        assert_eq!(
            verify_with_keys(&bytes, VERIFY_KEYS),
            Err(InvalidReason::BadSignature)
        );
    }

    // (4) unknown key_id rejected.
    #[test]
    fn unknown_key_id_rejected() {
        let mut payload = dev_payload();
        payload.key_id = "deadbeef".to_string();
        let bytes = signed_bytes(&payload, &DEV_TEST_SEED);
        assert_eq!(
            verify_with_keys(&bytes, VERIFY_KEYS),
            Err(InvalidReason::UnknownKey)
        );
    }

    // (5) malformed JSON, unknown field, oversize, bad signature encoding, wrong version.
    #[test]
    fn malformed_rejected() {
        assert_eq!(
            verify_with_keys(b"not json", VERIFY_KEYS),
            Err(InvalidReason::Malformed)
        );
        let bytes = signed_bytes(&dev_payload(), &DEV_TEST_SEED);
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["payload"]["extra"] = serde_json::Value::Number(1.into());
        assert_eq!(
            verify_with_keys(&serde_json::to_vec(&value).unwrap(), VERIFY_KEYS),
            Err(InvalidReason::Malformed)
        );
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["extra"] = serde_json::Value::Number(1.into());
        assert_eq!(
            verify_with_keys(&serde_json::to_vec(&value).unwrap(), VERIFY_KEYS),
            Err(InvalidReason::Malformed)
        );
        assert_eq!(
            verify_with_keys(&vec![b' '; MAX_FILE_BYTES + 1], VERIFY_KEYS),
            Err(InvalidReason::Oversize)
        );
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["signature"] = serde_json::Value::String("abcd".to_string());
        assert_eq!(
            verify_with_keys(&serde_json::to_vec(&value).unwrap(), VERIFY_KEYS),
            Err(InvalidReason::Malformed)
        );
        let mut payload = dev_payload();
        payload.v = 2;
        assert_eq!(
            verify_with_keys(&signed_bytes(&payload, &DEV_TEST_SEED), VERIFY_KEYS),
            Err(InvalidReason::Malformed)
        );
    }

    // (6) canonical bytes stable regardless of input key order.
    #[test]
    fn canonical_bytes_stable_under_key_reorder() {
        let payload = dev_payload();
        let bytes = signed_bytes(&payload, &DEV_TEST_SEED);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        // Rebuild the payload object with the keys in reverse order.
        let obj = value["payload"].as_object().unwrap();
        let mut reversed = serde_json::Map::new();
        for (k, v) in obj.iter().rev() {
            reversed.insert(k.clone(), v.clone());
        }
        let mut reordered = serde_json::Map::new();
        reordered.insert("signature".to_string(), value["signature"].clone());
        reordered.insert("payload".to_string(), serde_json::Value::Object(reversed));
        let reordered_bytes = serde_json::to_vec(&serde_json::Value::Object(reordered)).unwrap();
        assert_eq!(
            verify_with_keys(&reordered_bytes, VERIFY_KEYS),
            Ok(payload.clone())
        );
        let parsed: Envelope = serde_json::from_slice(&reordered_bytes).unwrap();
        assert_eq!(
            canonical_bytes(&parsed.payload).unwrap(),
            canonical_bytes(&payload).unwrap()
        );
    }

    // (7) states: not-yet-valid, valid, grace, exact grace boundary, expired.
    #[test]
    fn licence_states() {
        let p = dev_payload();
        let grace_until = p.valid_until + GRACE;
        let at = |t: u64| evaluate(Some(&p), t, 0, None, None).state;
        assert_eq!(
            at(p.valid_from - 1),
            State::Evaluation {
                days_left: 30,
                note: Some("not yet valid")
            }
        );
        assert_eq!(at(p.valid_from), State::Valid);
        assert_eq!(at(p.valid_until - 1), State::Valid);
        assert_eq!(at(p.valid_until), State::Grace { grace_until });
        assert_eq!(at(grace_until - 1), State::Grace { grace_until });
        assert_eq!(
            at(grace_until),
            State::ReadOnly {
                reason: "licence_expired"
            }
        );
        assert_eq!(
            at(grace_until + DAY),
            State::ReadOnly {
                reason: "licence_expired"
            }
        );
    }

    // (8) rollback: effective time is last_seen, grace not extended, flag set.
    #[test]
    fn clock_rollback_never_improves_state() {
        let p = dev_payload();
        let grace_until = p.valid_until + GRACE;
        // Clock rolled back from the read-only boundary into the grace window:
        // the state stays read-only and the rollback flag is set.
        let s = evaluate(Some(&p), p.valid_until + DAY, grace_until, Some(0), None);
        assert_eq!(
            s.state,
            State::ReadOnly {
                reason: "licence_expired"
            }
        );
        assert!(s.clock_rollback);
        // Grace end is fixed by the payload, never stretched by the clock.
        let s = evaluate(Some(&p), grace_until - 100, grace_until - 10, None, None);
        assert_eq!(s.state, State::Grace { grace_until });
        assert!(s.clock_rollback);
        // Tolerance: 60 s behind is not a rollback, 61 s is.
        let s = evaluate(Some(&p), T0, T0 + 60, None, None);
        assert!(!s.clock_rollback);
        let s = evaluate(Some(&p), T0, T0 + 61, None, None);
        assert!(s.clock_rollback);
    }

    // (9) evaluation day 0 / day 30; removal after a verified licence never
    // grants a fresh trial.
    #[test]
    fn evaluation_and_fallback() {
        let s = evaluate(None, T0, 0, Some(T0), None);
        assert_eq!(
            s.state,
            State::Evaluation {
                days_left: 30,
                note: None
            }
        );
        assert!(!s.clock_rollback);
        // No persisted start: evaluation begins now.
        assert_eq!(
            evaluate(None, T0, 0, None, None).state,
            State::Evaluation {
                days_left: 30,
                note: None
            }
        );
        assert_eq!(
            evaluate(None, T0 + EVALUATION - 1, 0, Some(T0), None).state,
            State::Evaluation {
                days_left: 1,
                note: None
            }
        );
        assert_eq!(
            evaluate(None, T0 + EVALUATION, 0, Some(T0), None).state,
            State::ReadOnly {
                reason: "evaluation_expired"
            }
        );
        // File removed after an expired licence: read-only, not a fresh trial.
        let p = dev_payload();
        let t = p.valid_until + GRACE + DAY;
        assert_eq!(
            evaluate(None, t, 0, None, Some(&p)).state,
            State::ReadOnly {
                reason: "licence_expired"
            }
        );
        // File removed mid-window: the persisted payload keeps its own state.
        assert_eq!(
            evaluate(None, p.valid_from + DAY, 0, None, Some(&p)).state,
            State::Valid
        );
    }

    // (10) seats = 0 and empty/inverted validity window rejected.
    #[test]
    fn invalid_fields_rejected() {
        let mut payload = dev_payload();
        payload.seats = 0;
        assert_eq!(
            verify_with_keys(&signed_bytes(&payload, &DEV_TEST_SEED), VERIFY_KEYS),
            Err(InvalidReason::InvalidFields)
        );
        let mut payload = dev_payload();
        payload.valid_until = payload.valid_from;
        assert_eq!(
            verify_with_keys(&signed_bytes(&payload, &DEV_TEST_SEED), VERIFY_KEYS),
            Err(InvalidReason::InvalidFields)
        );
        let mut payload = dev_payload();
        payload.valid_until = payload.valid_from - 1;
        assert_eq!(
            verify_with_keys(&signed_bytes(&payload, &DEV_TEST_SEED), VERIFY_KEYS),
            Err(InvalidReason::InvalidFields)
        );
    }

    // Owner override: enforcement off blocks nothing; on, only ReadOnly blocks.
    #[test]
    fn blocks_new_work_respects_enforce() {
        let p = dev_payload();
        let read_only = evaluate(Some(&p), p.valid_until + GRACE, 0, None, None);
        assert!(!read_only.blocks_new_work(false));
        assert!(read_only.blocks_new_work(true));
        for t in [p.valid_from - 1, p.valid_from, p.valid_until + DAY] {
            let s = evaluate(Some(&p), t, 0, None, None);
            assert!(!s.blocks_new_work(true));
            assert!(!s.blocks_new_work(false));
        }
        let expired_eval = evaluate(None, T0 + EVALUATION, 0, Some(T0), None);
        assert!(!expired_eval.blocks_new_work(false));
        assert!(expired_eval.blocks_new_work(true));
    }
}
