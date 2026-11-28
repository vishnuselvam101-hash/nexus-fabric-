//! Append-only, hash-chained audit log (JSON lines). Each entry carries the hash of the previous entry,
//! so deleting or editing any entry breaks verification.

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub ts: u64,
    pub prev: String,
    pub actor: String,
    pub action: String,
    pub detail: serde_json::Value,
    pub hash: String,
}

#[derive(Serialize)]
struct Body<'a> {
    seq: u64,
    ts: u64,
    prev: &'a str,
    actor: &'a str,
    action: &'a str,
    detail: &'a serde_json::Value,
}

fn entry_hash(e: &Entry) -> String {
    let body = Body {
        seq: e.seq,
        ts: e.ts,
        prev: &e.prev,
        actor: &e.actor,
        action: &e.action,
        detail: &e.detail,
    };
    crate::sha256_hex(
        serde_json::to_string(&body)
            .expect("entry serialises")
            .as_bytes(),
    )
}

#[derive(Debug)]
pub enum AuditError {
    Io(String),
    Broken(u64),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::Io(e) => write!(f, "audit io: {e}"),
            AuditError::Broken(seq) => write!(f, "audit chain broken at entry {seq}"),
        }
    }
}

pub struct AuditLog {
    path: PathBuf,
    seq: u64,
    last: String,
}

impl AuditLog {
    /// Opens (or creates) the log and verifies the whole chain first.
    pub fn open(path: &Path) -> Result<Self, AuditError> {
        let mut log = AuditLog {
            path: path.to_path_buf(),
            seq: 0,
            last: "0".repeat(64),
        };
        for e in read_all(path)? {
            if e.seq != log.seq + 1 || e.prev != log.last || entry_hash(&e) != e.hash {
                return Err(AuditError::Broken(e.seq));
            }
            log.seq = e.seq;
            log.last = e.hash;
        }
        Ok(log)
    }

    pub fn append(
        &mut self,
        ts: u64,
        actor: &str,
        action: &str,
        detail: serde_json::Value,
    ) -> Result<Entry, AuditError> {
        let mut e = Entry {
            seq: self.seq + 1,
            ts,
            prev: self.last.clone(),
            actor: actor.into(),
            action: action.into(),
            detail,
            hash: String::new(),
        };
        e.hash = entry_hash(&e);
        let line = serde_json::to_string(&e).expect("entry serialises");
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|x| AuditError::Io(x.to_string()))?;
        writeln!(f, "{line}")
            .and_then(|_| f.sync_all())
            .map_err(|x| AuditError::Io(x.to_string()))?;
        self.seq = e.seq;
        self.last = e.hash.clone();
        Ok(e)
    }
}

/// Reads every entry; a missing file is an empty log.
pub fn read_all(path: &Path) -> Result<Vec<Entry>, AuditError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(AuditError::Io(e.to_string())),
    };
    text.lines()
        .map(|l| serde_json::from_str::<Entry>(l).map_err(|e| AuditError::Io(e.to_string())))
        .collect()
}
