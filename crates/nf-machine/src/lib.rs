//! Machine profiles (M1) and the hand-off export (M2, M3).
//!
//! The crate writes files into a folder the operator chose. It has no network, serial or control code:
//! tests/no_control.rs fails the build if such code or dependency appears. A person starts every machine job.
//!
//! Send gate: a file is written only when its digest certificate is PASS, not held, for the same part, the
//! same engine id, and the SHA-256 of the file read right before writing matches the certificate.

pub mod audit;
pub mod dxf;
pub mod export;
pub mod gate;
pub mod profile;

pub use export::{propose, Actor, ActorKind, ExportError, ExportReport, ExportRequest, PartSource};
pub use gate::{CertVerdict, Certificate, Refusal};
pub use profile::{DxfVersion, MachineProfile, ProfileError, TextEncoding};

/// SHA-256 as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut s = String::with_capacity(64);
    for b in Sha256::digest(data) {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
