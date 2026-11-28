//! The send gate. Pure function: the caller supplies the certificate and the bytes it has just read.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CertVerdict {
    Pass,
    Fail,
    Inconclusive,
}

/// What the digest certified for one part: the file hash it checked and the engine that produced it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certificate {
    pub snapshot_id: String,
    pub part_id: String,
    pub engine_id: String,
    pub sha256: String,
    pub verdict: CertVerdict,
    pub held: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoCertificate,
    NotCertified(CertVerdict),
    Held,
    PartMismatch { expected: String, certified: String },
    EngineStale { current: String, certified: String },
    HashMismatch { certified: String, actual: String },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::NoCertificate => write!(f, "no digest certificate for this part"),
            Refusal::NotCertified(v) => write!(f, "digest verdict is {v:?}, not Pass"),
            Refusal::Held => write!(f, "part is held"),
            Refusal::PartMismatch {
                expected,
                certified,
            } => write!(f, "certificate is for part {certified}, not {expected}"),
            Refusal::EngineStale { current, certified } => write!(
                f,
                "engine {current} differs from certified engine {certified}"
            ),
            Refusal::HashMismatch { certified, actual } => {
                write!(f, "file hash {actual} differs from certified {certified}")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// Only a certified (Pass), unheld part whose bytes match the certificate passes.
pub fn check(
    cert: Option<&Certificate>,
    part_id: &str,
    engine_id: &str,
    bytes: &[u8],
) -> Result<(), Refusal> {
    let c = cert.ok_or(Refusal::NoCertificate)?;
    if c.verdict != CertVerdict::Pass {
        return Err(Refusal::NotCertified(c.verdict));
    }
    if c.held {
        return Err(Refusal::Held);
    }
    if c.part_id != part_id {
        return Err(Refusal::PartMismatch {
            expected: part_id.to_string(),
            certified: c.part_id.clone(),
        });
    }
    if c.engine_id != engine_id {
        return Err(Refusal::EngineStale {
            current: engine_id.to_string(),
            certified: c.engine_id.clone(),
        });
    }
    let actual = crate::sha256_hex(bytes);
    if actual != c.sha256 {
        return Err(Refusal::HashMismatch {
            certified: c.sha256.clone(),
            actual,
        });
    }
    Ok(())
}
