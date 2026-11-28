//! M2: export of certified parts as hand-off files into a folder the operator chose.
//! Person-only. All-or-nothing: every part is gated and converted before the first file is written;
//! each file is re-checked against its certificate right before it is written and read back after.
//! Refusals and successes are both written to the audit log.

use crate::audit::{AuditError, AuditLog};
use crate::dxf::{self, DxfError};
use crate::gate::{self, Certificate, Refusal};
use crate::profile::{MachineProfile, ProfileError};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorKind {
    Person,
    /// AMOEBA and other agents: may propose (see `propose`), never export.
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub id: String,
    pub kind: ActorKind,
}

pub struct PartSource {
    pub part_id: String,
    /// The checked DXF as stored by the digest.
    pub source: PathBuf,
    pub cert: Option<Certificate>,
}

pub struct ExportRequest<'a> {
    pub job_id: &'a str,
    pub profile: &'a MachineProfile,
    pub actor: &'a Actor,
    /// Set by the server after the licence check (nf-licence). Export is refused when false.
    pub licensed: bool,
    pub engine_id: &'a str,
    pub parts: &'a [PartSource],
    pub out_dir: &'a Path,
    pub ts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub part_id: String,
    pub file: String,
    pub source_sha256: String,
    pub out_sha256: String,
}

#[derive(Debug)]
pub struct ExportReport {
    pub files: Vec<FileRecord>,
    pub manifest: PathBuf,
}

#[derive(Debug)]
pub enum ExportError {
    PersonOnly,
    LicenceRequired,
    Profile(ProfileError),
    BadId(String),
    NotAFolder(String),
    Refused { part: String, reason: Refusal },
    Convert { part: String, error: DxfError },
    Exists(String),
    Io(String),
    Audit(AuditError),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::PersonOnly => write!(f, "export is for a person only"),
            ExportError::LicenceRequired => write!(f, "export needs a valid licence"),
            ExportError::Profile(e) => write!(f, "{e}"),
            ExportError::BadId(id) => write!(f, "id '{id}' must be letters, digits, '-' or '_'"),
            ExportError::NotAFolder(p) => write!(f, "not an existing folder: {p}"),
            ExportError::Refused { part, reason } => write!(f, "part {part} refused: {reason}"),
            ExportError::Convert { part, error } => write!(f, "part {part}: {error}"),
            ExportError::Exists(p) => write!(f, "file already exists, not overwritten: {p}"),
            ExportError::Io(e) => write!(f, "io: {e}"),
            ExportError::Audit(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ExportError {}

/// An agent's proposal. Writes nothing; a person must still run `export`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub job_id: String,
    pub profile_id: String,
    pub part_ids: Vec<String>,
    pub proposed_by: String,
}

pub fn propose(
    actor: &Actor,
    job_id: &str,
    profile: &MachineProfile,
    parts: &[PartSource],
) -> Proposal {
    Proposal {
        job_id: job_id.into(),
        profile_id: profile.id.clone(),
        part_ids: parts.iter().map(|p| p.part_id.clone()).collect(),
        proposed_by: actor.id.clone(),
    }
}

fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn export(req: &ExportRequest, audit: &mut AuditLog) -> Result<ExportReport, ExportError> {
    let who = req.actor.id.as_str();
    let refuse = |audit: &mut AuditLog, e: ExportError| -> ExportError {
        let _ = audit.append(
            req.ts,
            who,
            "export_refused",
            json!({
                "job": req.job_id, "profile": req.profile.id, "reason": e.to_string(),
            }),
        );
        e
    };

    if req.actor.kind != ActorKind::Person {
        return Err(refuse(audit, ExportError::PersonOnly));
    }
    if !req.licensed {
        return Err(refuse(audit, ExportError::LicenceRequired));
    }
    req.profile
        .validate()
        .map_err(|e| refuse(audit, ExportError::Profile(e)))?;
    if !safe_id(req.job_id) {
        return Err(refuse(audit, ExportError::BadId(req.job_id.into())));
    }
    if !req.out_dir.is_dir() {
        return Err(refuse(
            audit,
            ExportError::NotAFolder(req.out_dir.display().to_string()),
        ));
    }

    // Pass 1: gate, convert and check names for every part before any file is written.
    let mut names = Vec::new();
    for p in req.parts {
        if !safe_id(&p.part_id) {
            return Err(refuse(audit, ExportError::BadId(p.part_id.clone())));
        }
        let name = req.profile.file_name_for(req.job_id, &p.part_id);
        if names.contains(&name) {
            return Err(refuse(audit, ExportError::Exists(name)));
        }
        if req.out_dir.join(&name).exists() {
            return Err(refuse(audit, ExportError::Exists(name)));
        }
        let bytes =
            fs::read(&p.source).map_err(|e| refuse(audit, ExportError::Io(e.to_string())))?;
        gate::check(p.cert.as_ref(), &p.part_id, req.engine_id, &bytes).map_err(|r| {
            refuse(
                audit,
                ExportError::Refused {
                    part: p.part_id.clone(),
                    reason: r,
                },
            )
        })?;
        dxf::convert(&bytes, req.profile).map_err(|e| {
            refuse(
                audit,
                ExportError::Convert {
                    part: p.part_id.clone(),
                    error: e,
                },
            )
        })?;
        names.push(name);
    }

    // Pass 2: re-check each source right before its write, write via temp file, read back.
    let mut written: Vec<PathBuf> = Vec::new();
    let mut records = Vec::new();
    let result = (|| -> Result<(), ExportError> {
        for (p, name) in req.parts.iter().zip(&names) {
            let bytes = fs::read(&p.source).map_err(|e| ExportError::Io(e.to_string()))?;
            gate::check(p.cert.as_ref(), &p.part_id, req.engine_id, &bytes).map_err(|r| {
                ExportError::Refused {
                    part: p.part_id.clone(),
                    reason: r,
                }
            })?;
            let out = dxf::convert(&bytes, req.profile).map_err(|e| ExportError::Convert {
                part: p.part_id.clone(),
                error: e,
            })?;
            let out_sha = crate::sha256_hex(&out);
            let dest = req.out_dir.join(name);
            let tmp = req.out_dir.join(format!(".{name}.tmp"));
            fs::write(&tmp, &out).map_err(|e| ExportError::Io(e.to_string()))?;
            let back = fs::read(&tmp).map_err(|e| ExportError::Io(e.to_string()))?;
            if crate::sha256_hex(&back) != out_sha {
                let _ = fs::remove_file(&tmp);
                return Err(ExportError::Io(format!(
                    "read-back hash differs for {name}"
                )));
            }
            fs::rename(&tmp, &dest).map_err(|e| ExportError::Io(e.to_string()))?;
            written.push(dest);
            records.push(FileRecord {
                part_id: p.part_id.clone(),
                file: name.clone(),
                source_sha256: crate::sha256_hex(&bytes),
                out_sha256: out_sha,
            });
        }
        Ok(())
    })();
    if let Err(e) = result {
        for f in &written {
            let _ = fs::remove_file(f);
        }
        return Err(refuse(audit, e));
    }

    let manifest_path = req.out_dir.join(format!("manifest-{}.json", req.job_id));
    let manifest = json!({
        "job": req.job_id,
        "profile": { "id": req.profile.id, "version": req.profile.version },
        "operator": req.actor.id,
        "engine_id": req.engine_id,
        "ts": req.ts,
        "untested_profile": req.profile.tested_by.is_none(),
        "files": records.iter().map(|r| json!({
            "part": r.part_id, "file": r.file, "source_sha256": r.source_sha256, "sha256": r.out_sha256
        })).collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&manifest).expect("manifest serialises");
    let manifest_sha = crate::sha256_hex(text.as_bytes());
    if let Err(e) = fs::write(&manifest_path, text) {
        for f in &written {
            let _ = fs::remove_file(f);
        }
        let _ = fs::remove_file(&manifest_path);
        return Err(refuse(audit, ExportError::Io(e.to_string())));
    }

    audit
        .append(req.ts, who, "export", json!({
            "job": req.job_id,
            "profile": { "id": req.profile.id, "version": req.profile.version },
            "engine_id": req.engine_id,
            "files": records.iter().map(|r| json!({
                "part": r.part_id, "file": r.file, "source_sha256": r.source_sha256, "sha256": r.out_sha256
            })).collect::<Vec<_>>(),
            "manifest": manifest_path.file_name().map(|n| n.to_string_lossy().into_owned()),
            "manifest_sha256": manifest_sha,
            "untested_profile": req.profile.tested_by.is_none(),
        }))
        .map_err(ExportError::Audit)?;

    Ok(ExportReport {
        files: records,
        manifest: manifest_path,
    })
}
