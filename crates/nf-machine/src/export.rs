//! M2: export of certified parts as hand-off files into a folder the operator chose.
//! Person-only. All-or-nothing: every part is gated and converted before the first file is written;
//! each file is re-checked against its certificate right before it is written and read back after.
//! Refusals and successes are both written to the audit log.

use crate::audit::{AuditError, AuditLog};
use crate::dxf::{self, DxfError};
use crate::gate::{self, Certificate, Refusal};
use crate::profile::{MachineProfile, ProfileError, MAX_FILE_NAME};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// Manifests are never overwritten: a re-export gets the next version number.
const MAX_MANIFEST_VERSIONS: u32 = 9999;

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
    /// Loads the checked DXF as stored by the digest. Called twice: once to gate, once right before the write.
    pub load: Box<dyn Fn() -> Result<Vec<u8>, String>>,
    pub cert: Option<Certificate>,
}

impl PartSource {
    /// A part read from a file on disk.
    pub fn file(part_id: &str, path: PathBuf, cert: Option<Certificate>) -> Self {
        PartSource {
            part_id: part_id.into(),
            load: Box::new(move || std::fs::read(&path).map_err(|e| e.to_string())),
            cert,
        }
    }
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
    /// Replace existing regular files of the same names. Set only by a person's explicit choice.
    pub overwrite: bool,
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
    Refused {
        part: String,
        reason: Refusal,
    },
    Convert {
        part: String,
        error: DxfError,
    },
    Exists(String),
    Io(String),
    /// The audit entry could not be written; the export is not kept.
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

fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn io(e: std::io::Error) -> ExportError {
    ExportError::Io(e.to_string())
}

/// Whether anything (a file, a folder, a symlink, even a dangling one) sits at the path. Never follows links.
fn occupied(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// Writes one checked file. The temp file is created exclusively (never follows a planted link), read back,
/// and then either hard-linked to the destination (fails if it exists: no replace) or, with `overwrite`,
/// renamed over a regular file only.
fn write_checked(
    dir: &Path,
    name: &str,
    data: &[u8],
    overwrite: bool,
) -> Result<PathBuf, ExportError> {
    let dest = dir.join(name);
    let tmp = dir.join(format!(".{name}.tmp"));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => ExportError::Exists(tmp.display().to_string()),
            _ => io(e),
        })?;
    let staged = (|| -> Result<(), ExportError> {
        f.write_all(data).and_then(|_| f.sync_all()).map_err(io)?;
        drop(f);
        let back = fs::read(&tmp).map_err(io)?;
        if crate::sha256_hex(&back) != crate::sha256_hex(data) {
            return Err(ExportError::Io(format!(
                "read-back hash differs for {name}"
            )));
        }
        if overwrite {
            match fs::symlink_metadata(&dest) {
                Ok(m) if !m.is_file() => return Err(ExportError::Exists(name.into())),
                _ => {}
            }
            fs::rename(&tmp, &dest).map_err(io)?;
        } else {
            fs::hard_link(&tmp, &dest).map_err(|e| match e.kind() {
                ErrorKind::AlreadyExists => ExportError::Exists(name.into()),
                _ => io(e),
            })?;
            if let Err(e) = fs::remove_file(&tmp) {
                let _ = fs::remove_file(&dest);
                return Err(io(e));
            }
        }
        Ok(())
    })();
    if staged.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    staged.map(|_| dest)
}

/// Creates the next free manifest version for the job. Exclusive create: an existing manifest is never replaced.
fn write_manifest(dir: &Path, job: &str, text: &[u8]) -> Result<PathBuf, ExportError> {
    for v in 1..=MAX_MANIFEST_VERSIONS {
        let path = dir.join(format!("manifest-{job}-v{v}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(text).and_then(|_| f.sync_all()) {
                    let _ = fs::remove_file(&path);
                    return Err(io(e));
                }
                return Ok(path);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io(e)),
        }
    }
    Err(ExportError::Exists(format!("manifest-{job}")))
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

pub fn export(req: &ExportRequest, audit: &mut AuditLog) -> Result<ExportReport, ExportError> {
    let who = req.actor.id.as_str();
    // A refusal that cannot be audited is an error in its own right; it is never dropped silently.
    let refuse = |audit: &mut AuditLog, e: ExportError| -> ExportError {
        match audit.append(
            req.ts,
            who,
            "export_refused",
            json!({
                "job": req.job_id, "profile": req.profile.id, "reason": e.to_string(),
            }),
        ) {
            Ok(_) => e,
            Err(a) => ExportError::Audit(a),
        }
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
    let folder_ok = fs::symlink_metadata(req.out_dir).is_ok_and(|m| m.is_dir());
    if !folder_ok {
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
        if name.len() > MAX_FILE_NAME {
            return Err(refuse(
                audit,
                ExportError::Profile(ProfileError::BadFileName),
            ));
        }
        if names.contains(&name) {
            return Err(refuse(audit, ExportError::Exists(name)));
        }
        let dest = req.out_dir.join(&name);
        if occupied(&dest) {
            let plain_file = fs::symlink_metadata(&dest).is_ok_and(|m| m.is_file());
            if !(req.overwrite && plain_file) {
                return Err(refuse(audit, ExportError::Exists(name)));
            }
        }
        let bytes = (p.load)().map_err(|e| refuse(audit, ExportError::Io(e)))?;
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

    // Pass 2: re-check each source right before its write, write and read back, link into place.
    let mut written: Vec<PathBuf> = Vec::new();
    let mut records = Vec::new();
    let result = (|| -> Result<(), ExportError> {
        for (p, name) in req.parts.iter().zip(&names) {
            let bytes = (p.load)().map_err(ExportError::Io)?;
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
            let dest = write_checked(req.out_dir, name, &out, req.overwrite)?;
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
    let rollback = |written: &[PathBuf]| {
        for f in written {
            let _ = fs::remove_file(f);
        }
    };
    if let Err(e) = result {
        rollback(&written);
        return Err(refuse(audit, e));
    }

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
    let manifest_path = match write_manifest(req.out_dir, req.job_id, text.as_bytes()) {
        Ok(p) => p,
        Err(e) => {
            rollback(&written);
            return Err(refuse(audit, e));
        }
    };

    // The audit entry is the commit point: if it cannot be written, nothing of this export is kept.
    let entry = audit.append(req.ts, who, "export", json!({
        "job": req.job_id,
        "profile": { "id": req.profile.id, "version": req.profile.version },
        "engine_id": req.engine_id,
        "files": records.iter().map(|r| json!({
            "part": r.part_id, "file": r.file, "source_sha256": r.source_sha256, "sha256": r.out_sha256
        })).collect::<Vec<_>>(),
        "manifest": manifest_path.file_name().map(|n| n.to_string_lossy().into_owned()),
        "manifest_sha256": manifest_sha,
        "untested_profile": req.profile.tested_by.is_none(),
        "overwrite": req.overwrite,
    }));
    if let Err(e) = entry {
        rollback(&written);
        let _ = fs::remove_file(&manifest_path);
        return Err(ExportError::Audit(e));
    }

    Ok(ExportReport {
        files: records,
        manifest: manifest_path,
    })
}
