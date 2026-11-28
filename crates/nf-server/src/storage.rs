//! Storage start-up and the blob store.
//!
//! - `open_storage`: database, schema, versioned migrations, storage key.
//!   Fails with a clear message instead of guessing: a missing or unreadable
//!   key file beside existing data never produces a new key (BE-010).
//! - Blobs: written to a temporary file, fsynced, renamed, directory fsynced;
//!   verified (decrypt + hash) on every read and before an existing file is
//!   trusted by a new write (BE-006).

use crate::crypto;
use rusqlite::{Connection, OptionalExtension, params};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 7: `order_results` (the supersede chain of order computes, NFJ-1 L10), on top of 6.
/// 6: NFJ-1 order tables and users.kind, on top of 5 (AUTH-1 recovery/must_change columns). Rollback to an older build
/// is a restore of the pre-update backup: an older program refuses a newer schema (see the check below) and does not
/// undo the migration.
pub const SCHEMA_VERSION: i64 = 7;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY, username TEXT UNIQUE NOT NULL, role TEXT NOT NULL, salt TEXT NOT NULL, hash TEXT NOT NULL, active INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS sessions(token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL, expires_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS projects(id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS documents(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, filename TEXT NOT NULL, detected_type TEXT NOT NULL, size INTEGER NOT NULL, sha256 TEXT NOT NULL, extension_mismatch INTEGER NOT NULL, record_id TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS records(id TEXT PRIMARY KEY, project_id INTEGER NOT NULL, family TEXT NOT NULL, kind TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS calculations(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, family TEXT NOT NULL, state TEXT NOT NULL, closure_hash TEXT NOT NULL, result_json TEXT NOT NULL, replay_json TEXT, dxf_sha256 TEXT, supersedes INTEGER, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS obligations(id INTEGER PRIMARY KEY, origin_calc INTEGER NOT NULL, target_calc INTEGER NOT NULL, record_id TEXT NOT NULL, kind TEXT NOT NULL, label TEXT NOT NULL, direction TEXT NOT NULL, via TEXT NOT NULL, required_check TEXT NOT NULL, state TEXT NOT NULL, note TEXT NOT NULL, resolved_by TEXT, resolved_at INTEGER, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY, at INTEGER NOT NULL, username TEXT NOT NULL, action TEXT NOT NULL, detail TEXT NOT NULL, prev TEXT, hash TEXT);
CREATE TABLE IF NOT EXISTS lessons(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, kind TEXT NOT NULL, family TEXT NOT NULL, parameter TEXT NOT NULL, wrong_value REAL, right_value REAL, detail TEXT NOT NULL, source_calc INTEGER NOT NULL, record_id TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS advisories(id INTEGER PRIMARY KEY, calc_id INTEGER NOT NULL, lesson_id INTEGER NOT NULL, message TEXT NOT NULL, state TEXT NOT NULL, note TEXT NOT NULL, resolved_by TEXT, resolved_at INTEGER);
CREATE TABLE IF NOT EXISTS project_members(project_id INTEGER NOT NULL, user_id INTEGER NOT NULL, PRIMARY KEY(project_id, user_id));
CREATE TABLE IF NOT EXISTS login_failures(username TEXT NOT NULL, at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS login_clients(username TEXT NOT NULL, client TEXT NOT NULL, at INTEGER NOT NULL, PRIMARY KEY(username, client));
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS pdf_conversions(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, source_document INTEGER NOT NULL, page INTEGER NOT NULL, scale_kind TEXT NOT NULL, scale_value REAL NOT NULL, tolerance_mm REAL, include_fill INTEGER NOT NULL, include_text INTEGER NOT NULL, status TEXT NOT NULL, code TEXT, limit_name TEXT, message TEXT, converter_version TEXT, source_sha256 TEXT NOT NULL, dxf_sha256 TEXT, report_sha256 TEXT, record_id TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS amoeba_digests(id INTEGER PRIMARY KEY, calc_id INTEGER NOT NULL, part TEXT NOT NULL, dxf_sha256 TEXT NOT NULL, inputs_sha256 TEXT NOT NULL, engine TEXT NOT NULL, verdict TEXT NOT NULL, checks_json TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(calc_id, part));
CREATE TABLE IF NOT EXISTS orders(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, name TEXT NOT NULL, state TEXT NOT NULL, rules_json TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS order_files(id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL REFERENCES orders(id), document_id INTEGER NOT NULL REFERENCES documents(id), ord INTEGER NOT NULL, UNIQUE(order_id, ord));
CREATE TABLE IF NOT EXISTS order_pages(id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL REFERENCES orders(id), file_id INTEGER NOT NULL REFERENCES order_files(id), page_no INTEGER NOT NULL, file_page INTEGER NOT NULL, png_sha256 TEXT, width INTEGER NOT NULL DEFAULT 0, height INTEGER NOT NULL DEFAULT 0, kind TEXT NOT NULL, read_state TEXT NOT NULL, skipped INTEGER NOT NULL DEFAULT 0, skipped_by TEXT, skipped_at INTEGER, UNIQUE(order_id, page_no));
CREATE TABLE IF NOT EXISTS order_parts(id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL REFERENCES orders(id), page_id INTEGER REFERENCES order_pages(id), no TEXT NOT NULL, name TEXT NOT NULL, shape TEXT NOT NULL, attach_part TEXT, qty INTEGER NOT NULL, table_rows_json TEXT NOT NULL, group_key TEXT, state TEXT NOT NULL, UNIQUE(order_id, no));
CREATE TABLE IF NOT EXISTS order_values(id INTEGER PRIMARY KEY, part_id INTEGER NOT NULL REFERENCES order_parts(id), key TEXT NOT NULL, value REAL, text TEXT NOT NULL, bbox_json TEXT, source TEXT NOT NULL, state TEXT NOT NULL, was REAL, proposed_value REAL, proposed_by TEXT NOT NULL, edited_by TEXT, edited_at INTEGER, UNIQUE(part_id, key));
CREATE TABLE IF NOT EXISTS order_results(id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL REFERENCES orders(id), input_hash TEXT NOT NULL, engine TEXT NOT NULL, state TEXT NOT NULL, supersedes INTEGER, result_json TEXT NOT NULL, counts_json TEXT NOT NULL, created_by TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS order_decisions(id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL REFERENCES orders(id), part_id INTEGER, value_id INTEGER, kind TEXT NOT NULL, flag TEXT, source TEXT NOT NULL, agent TEXT, reason TEXT NOT NULL, against_hash TEXT NOT NULL, prev_state TEXT, decided_by TEXT NOT NULL, decided_at INTEGER NOT NULL, undone_by TEXT, undone_at INTEGER, undo_note TEXT);
";

/// Columns added after the first schema. Each is added only if absent, and a
/// failure stops start-up (it used to be ignored).
const ADDED_COLUMNS: [(&str, &str, &str); 15] = [
    // RA-006: 0 for an attempt that was refused by the throttle (counted, but it does not open a slot)
    ("login_failures", "admitted", "INTEGER NOT NULL DEFAULT 1"),
    ("calculations", "supersedes", "INTEGER"),
    ("users", "active", "INTEGER NOT NULL DEFAULT 1"),
    ("audit", "prev", "TEXT"),
    ("audit", "hash", "TEXT"),
    // BE-011: the engine that computed the result, readable without parsing the result.
    ("calculations", "engine", "TEXT"),
    // BE-009: which engine build materialised the `state` column.
    ("calculations", "state_by", "TEXT"),
    // V-33: the client address a failed sign-in came from.
    ("login_failures", "client", "TEXT"),
    // S-7: the few values a job list needs, derived from result_json by SQL, and the byte length of the
    // result they were derived from. Lists read these instead of parsing every stored result.
    ("calculations", "core_json", "TEXT"),
    ("calculations", "result_bytes", "INTEGER"),
    // Task 3.1: the PDF conversion a document was produced by (NULL for an upload).
    ("documents", "conversion_id", "INTEGER"),
    // AUTH-1: the recovery code, only as a salted Argon2 hash (NULL: none yet); and the forced password change
    // after an administrator reset.
    ("users", "recovery_salt", "TEXT"),
    ("users", "recovery_hash", "TEXT"),
    ("users", "must_change", "INTEGER NOT NULL DEFAULT 0"),
    // NFJ-1: "person" or "agent" (the AI operator's service identity); decides the source of an order decision.
    ("users", "kind", "TEXT NOT NULL DEFAULT 'person'"),
];

/// Append-only at the database level (RC-20). Created at every start, so an older database gets them too.
const TRIGGERS: &str = "
CREATE TRIGGER IF NOT EXISTS audit_no_update BEFORE UPDATE ON audit BEGIN SELECT RAISE(ABORT, 'the audit log is append-only'); END;
CREATE TRIGGER IF NOT EXISTS audit_no_delete BEFORE DELETE ON audit BEGIN SELECT RAISE(ABORT, 'the audit log is append-only'); END;
CREATE TRIGGER IF NOT EXISTS records_no_update BEFORE UPDATE ON records BEGIN SELECT RAISE(ABORT, 'records are append-only'); END;
CREATE TRIGGER IF NOT EXISTS records_no_delete BEFORE DELETE ON records BEGIN SELECT RAISE(ABORT, 'records are append-only'); END;
CREATE TRIGGER IF NOT EXISTS amoeba_no_update BEFORE UPDATE ON amoeba_digests BEGIN SELECT RAISE(ABORT, 'AMOEBA snapshots are immutable'); END;
CREATE TRIGGER IF NOT EXISTS amoeba_no_delete BEFORE DELETE ON amoeba_digests BEGIN SELECT RAISE(ABORT, 'AMOEBA snapshots are immutable'); END;
CREATE TRIGGER IF NOT EXISTS order_results_no_delete BEFORE DELETE ON order_results BEGIN SELECT RAISE(ABORT, 'computed results are kept; a new compute supersedes them'); END;
CREATE TRIGGER IF NOT EXISTS order_decisions_no_delete BEFORE DELETE ON order_decisions BEGIN SELECT RAISE(ABORT, 'decisions are kept; undo marks them undone'); END;
";

const INDEXES: &str = "
CREATE INDEX IF NOT EXISTS ix_calc_project ON calculations(project_id);
CREATE INDEX IF NOT EXISTS ix_calc_supersedes ON calculations(supersedes);
CREATE INDEX IF NOT EXISTS ix_obl_target ON obligations(target_calc, state);
CREATE INDEX IF NOT EXISTS ix_adv_calc ON advisories(calc_id, state);
CREATE INDEX IF NOT EXISTS ix_lessons_project ON lessons(project_id, family);
CREATE INDEX IF NOT EXISTS ix_records_project ON records(project_id, family, kind);
CREATE INDEX IF NOT EXISTS ix_docs_project ON documents(project_id);
CREATE INDEX IF NOT EXISTS ix_docs_conversion ON documents(conversion_id);
CREATE INDEX IF NOT EXISTS ix_conv_project ON pdf_conversions(project_id);
CREATE INDEX IF NOT EXISTS ix_orders_project ON orders(project_id);
CREATE INDEX IF NOT EXISTS ix_order_pages_order ON order_pages(order_id, page_no);
CREATE INDEX IF NOT EXISTS ix_order_parts_order ON order_parts(order_id);
CREATE INDEX IF NOT EXISTS ix_order_results_order ON order_results(order_id, id);
CREATE INDEX IF NOT EXISTS ix_order_decisions_order ON order_decisions(order_id, value_id);
CREATE INDEX IF NOT EXISTS ix_fail_user ON login_failures(username, at);
CREATE INDEX IF NOT EXISTS ix_fail_client ON login_failures(client, at);
";

fn has_column(db: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut st = db.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = st
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(names.iter().any(|n| n == column))
}

pub fn meta_get(db: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    db.query_row("SELECT value FROM meta WHERE key = ?", params![key], |r| {
        r.get(0)
    })
    .optional()
}

pub fn meta_set(db: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    db.execute("INSERT INTO meta(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

/// Not secret and not reversible: lets start-up recognise the key this data was created with.
pub fn key_fingerprint(key: &[u8; 32]) -> String {
    let mut m = b"nexus-fabric storage key fingerprint v1\0".to_vec();
    m.extend_from_slice(key);
    nf_core::sha256_hex(&m)
}

/// HMAC-SHA256 (RFC 2104) with the storage key; hex.
pub fn hmac_hex(key: &[u8; 32], message: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let (mut ipad, mut opad) = ([0x36u8; 64], [0x5cu8; 64]);
    for (i, k) in key.iter().enumerate() {
        ipad[i] ^= k;
        opad[i] ^= k;
    }
    let inner = Sha256::new()
        .chain_update(ipad)
        .chain_update(message)
        .finalize();
    let outer = Sha256::new()
        .chain_update(opad)
        .chain_update(&inner[..])
        .finalize();
    hex::encode(&outer[..])
}

fn owner_only(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// Create a file that did not exist, owner-only from its first byte, and make it durable.
fn create_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    // Windows cannot open a directory as a file (access denied), and it does not need the directory flushed.
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        std::fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

/// File in the blob directory that says "an installation was initialised here".
pub const INSTALL_MARKER: &str = ".installation-marker";

fn rows_exist(db: &Connection) -> Result<bool, String> {
    for table in [
        "users",
        "calculations",
        "documents",
        "records",
        "projects",
        "audit",
    ] {
        let n: i64 = db
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .map_err(|e| format!("cannot read table {table}: {e}"))?;
        if n > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn data_exists(db: &Connection, data_dir: &Path) -> Result<bool, String> {
    Ok(rows_exist(db)? || first_blob(data_dir).is_some())
}

/// SQLite's own consistency check. `None` = no problem found. `quick` skips
/// the index-content comparison (used at start-up); the full check is run on demand.
pub fn integrity_check(db: &Connection, quick: bool) -> rusqlite::Result<Option<String>> {
    let mut st = db.prepare(if quick {
        "PRAGMA quick_check(20)"
    } else {
        "PRAGMA integrity_check(20)"
    })?;
    let lines = st
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(if lines.len() == 1 && lines[0] == "ok" {
        None
    } else {
        Some(lines.join("; "))
    })
}

fn some_blobs(data_dir: &Path, n: usize) -> Vec<PathBuf> {
    std::fs::read_dir(data_dir.join("blobs"))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "bin"))
                .take(n)
                .collect()
        })
        .unwrap_or_default()
}

fn first_blob(data_dir: &Path) -> Option<PathBuf> {
    some_blobs(data_dir, 1).into_iter().next()
}

/// Load the storage key. A new key is created only for an installation that
/// holds no data at all; in every other case a problem with the key file stops
/// start-up, because a fresh key would make every stored file unreadable.
fn load_key(db: &Connection, data_dir: &Path, key_path: &Path) -> Result<[u8; 32], String> {
    let has_data = data_exists(db, data_dir)?;
    let key: [u8; 32] = match std::fs::read(key_path) {
        Ok(bytes) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(key_path)
                    .map_err(|e| format!("cannot inspect key file {}: {e}", key_path.display()))?
                    .permissions()
                    .mode();
                if mode & 0o007 != 0 {
                    return Err(format!(
                        "storage key file {} is readable by every local user (mode {:o}). Refusing to start; restrict it (chmod 600, or 640 with a dedicated group) and start again.",
                        key_path.display(),
                        mode & 0o777
                    ));
                }
            }
            bytes.as_slice().try_into().map_err(|_| format!("storage key file {} has the wrong length ({} bytes, expected 32). Refusing to start; restore the original key file.", key_path.display(), bytes.len()))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if has_data {
                return Err(format!(
                    "storage key file {} is MISSING, but the data directory {} already holds data. Refusing to start: a new key would make every stored document and DXF unreadable. Restore the key file, or point NF_KEY_FILE at it. No key was created.",
                    key_path.display(),
                    data_dir.display()
                ));
            }
            let k: [u8; 32] = crypto::random_bytes();
            if let Some(dir) = key_path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| {
                    format!(
                        "cannot create the directory of the key file {}: {e}",
                        key_path.display()
                    )
                })?;
            }
            create_private(key_path, &k).map_err(|e| {
                format!("cannot create storage key file {}: {e}", key_path.display())
            })?;
            k
        }
        Err(e) => {
            return Err(format!(
                "storage key file {} cannot be read ({e}). Refusing to start; no key was created.",
                key_path.display()
            ));
        }
    };
    let fp = key_fingerprint(&key);
    match meta_get(db, "key_fingerprint").map_err(|e| e.to_string())? {
        Some(stored) if stored != fp => Err(format!(
            "storage key file {} is not the key this data directory was created with (fingerprint mismatch). Refusing to start; restore the original key file.",
            key_path.display()
        )),
        Some(_) => Ok(key),
        None => {
            // A database from before fingerprints: prove the key against a stored blob, if there is one.
            // (A single torn blob must not block start-up, so several are tried.)
            let blobs = some_blobs(data_dir, 20);
            let opens = |p: &PathBuf| {
                std::fs::read(p)
                    .ok()
                    .is_some_and(|sealed| crypto::open(&key, &sealed).is_ok())
            };
            if !blobs.is_empty() && !blobs.iter().any(opens) {
                return Err(format!(
                    "storage key file {} does not decrypt the stored files in {}. Refusing to start; restore the original key file.",
                    key_path.display(),
                    data_dir.display()
                ));
            }
            meta_set(db, "key_fingerprint", &fp).map_err(|e| e.to_string())?;
            Ok(key)
        }
    }
}

/// Open (or initialise) the data directory. Every failure is returned with a
/// message an operator can act on; nothing is created to paper over it.
pub fn open_storage(data_dir: &Path, key_path: &Path) -> Result<(Connection, [u8; 32]), String> {
    std::fs::create_dir_all(data_dir.join("blobs"))
        .map_err(|e| format!("cannot create data directory {}: {e}", data_dir.display()))?;
    owner_only(data_dir, 0o700)
        .map_err(|e| format!("cannot restrict data directory {}: {e}", data_dir.display()))?;
    let db_path = data_dir.join("nexus-fabric.sqlite");
    let db = Connection::open(&db_path)
        .map_err(|e| format!("cannot open database {}: {e}", db_path.display()))?;
    owner_only(&db_path, 0o600)
        .map_err(|e| format!("cannot restrict database file {}: {e}", db_path.display()))?;
    db.busy_timeout(std::time::Duration::from_millis(2000))
        .map_err(|e| format!("cannot set the database busy timeout: {e}"))?;
    let mode: String = db.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0)).map_err(|e| format!("database {} is not a usable database or is damaged ({e}). Refusing to start; restore it from a backup.", db_path.display()))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(format!("database journal mode is '{mode}', expected WAL"));
    }
    db.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
        .map_err(|e| format!("cannot configure the database: {e}"))?;
    let found: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| format!("database {} is damaged (cannot read the schema version: {e}). Refusing to start; restore it from a backup.", db_path.display()))?;
    if found > SCHEMA_VERSION {
        return Err(format!(
            "database schema version {found} is newer than this build understands ({SCHEMA_VERSION}). Refusing to start with an older program."
        ));
    }
    // S-2: a damaged database must not be served. (The full check is available on demand in /api/audit/verify.)
    let check = integrity_check(&db, true).map_err(|e| format!("database {} is damaged and cannot be checked ({e}). Refusing to start; restore it from a backup.", db_path.display()))?;
    if let Some(problem) = check {
        return Err(format!(
            "database {} failed its integrity check ({problem}). Refusing to start; restore it from a backup.",
            db_path.display()
        ));
    }
    db.execute_batch(SCHEMA).map_err(|e| {
        format!(
            "database {} cannot be initialised or is damaged ({e}). Refusing to start.",
            db_path.display()
        )
    })?;
    // RC-20: the audit log and the records are append-only inside the database too. This stops code bugs and
    // accidental edits; it does not stop a writer of the file (who can drop a trigger). The audit chain check
    // (main.rs, App::audit) and the anchor are the detection for that.
    db.execute_batch(TRIGGERS).map_err(|e| {
        format!(
            "database {} cannot be initialised ({e}). Refusing to start.",
            db_path.display()
        )
    })?;
    for (table, column, decl) in ADDED_COLUMNS {
        if !has_column(&db, table, column)
            .map_err(|e| format!("cannot inspect table {table}: {e}"))?
        {
            db.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
                [],
            )
            .map_err(|e| format!("migration failed (add {table}.{column}): {e}"))?;
        }
    }
    db.execute_batch(INDEXES)
        .map_err(|e| format!("cannot create indexes: {e}"))?;
    // BE-011: record the engine per result. The stored engineering result itself is not rewritten.
    db.execute("UPDATE calculations SET engine = json_extract(result_json, '$.engine') WHERE engine IS NULL AND json_valid(result_json)", []).map_err(|e| format!("migration failed (engine per result): {e}"))?;
    // S-2: the database and the data directory must belong together. At first initialisation a random
    // installation id is written into both the database and a marker file beside the stored files. A
    // database WITHOUT that id beside a marker or stored files is a database that was lost, truncated or
    // replaced; starting would offer first-run setup on top of someone's data.
    let marker = data_dir.join("blobs").join(INSTALL_MARKER);
    let marker_id = match std::fs::read_to_string(&marker) {
        Ok(t) => Some(t.lines().next().unwrap_or("").trim().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(format!(
                "cannot read the installation marker {}: {e}",
                marker.display()
            ));
        }
    };
    let db_id = meta_get(&db, "installation_id").map_err(|e| e.to_string())?;
    let lost = |what: &str| {
        format!(
            "the database {} does not belong to the installation in {} ({what}). The database was lost, truncated or replaced. Refusing to start in first-run mode. Restore the database from a backup. Only if you really want a new, empty installation: move the old data directory away and start with a fresh one.",
            db_path.display(),
            data_dir.display()
        )
    };
    let has_key_fingerprint = meta_get(&db, "key_fingerprint")
        .map_err(|e| e.to_string())?
        .is_some();
    let installation_id = match (db_id, marker_id) {
        (Some(d), Some(m)) if d == m => d,
        (Some(_), Some(_)) => {
            return Err(lost(
                "its installation id differs from the installation marker",
            ));
        }
        // RA-014: a database restored from before the marker carries no installation id. Its key fingerprint
        // is checked against the key file by `load_key` below, so the database is accepted only with its own key.
        (None, Some(m)) if has_key_fingerprint => {
            meta_set(&db, "installation_id", &m).map_err(|e| e.to_string())?;
            m
        }
        (None, Some(_)) => {
            return Err(lost(
                "installation marker present, but the database carries no installation id",
            ));
        }
        // marker missing: a database from before markers, or a marker deleted by hand; the database is the authority
        (Some(d), None) => d,
        (None, None) if !rows_exist(&db)? && first_blob(data_dir).is_some() => {
            return Err(lost("stored files present, but the database is empty"));
        }
        (None, None) => {
            let id = hex::encode(crypto::random_bytes::<16>());
            meta_set(&db, "installation_id", &id).map_err(|e| e.to_string())?;
            id
        }
    };
    let key = load_key(&db, data_dir, key_path)?;
    if !marker.exists() {
        create_private(&marker, format!("{installation_id}\nNEXUS FABRIC data directory. Do not delete: this file ties the stored files to their database.\n").as_bytes()).map_err(|e| format!("cannot write the installation marker {}: {e}", marker.display()))?;
    }
    db.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| format!("cannot record the schema version: {e}"))?;
    Ok((db, key))
}

// ---------------------------------------------------------------- blobs

#[derive(Debug)]
pub enum BlobError {
    Missing,
    /// Present but torn, truncated, altered or sealed with another key.
    Corrupt(String),
    Io(String),
}

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobError::Missing => write!(f, "stored file is missing"),
            BlobError::Corrupt(why) => write!(f, "stored file failed its integrity check ({why})"),
            BlobError::Io(why) => write!(f, "stored file could not be read ({why})"),
        }
    }
}

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn blob_path(data_dir: &Path, sha: &str) -> PathBuf {
    data_dir.join("blobs").join(format!("{sha}.bin"))
}

/// Read and verify: decrypt, then compare the SHA-256 of the plaintext with the name.
pub fn load_blob(data_dir: &Path, key: &[u8; 32], sha: &str) -> Result<Vec<u8>, BlobError> {
    if !is_sha256_hex(sha) {
        return Err(BlobError::Corrupt(
            "reference is not a SHA-256 value".into(),
        ));
    }
    let sealed = match std::fs::read(blob_path(data_dir, sha)) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(BlobError::Missing),
        Err(e) => return Err(BlobError::Io(e.to_string())),
    };
    let plain = crypto::open(key, &sealed).map_err(BlobError::Corrupt)?;
    if nf_core::sha256_hex(&plain) != sha {
        return Err(BlobError::Corrupt("content does not match its hash".into()));
    }
    Ok(plain)
}

/// Store content under its hash. Atomic and durable: temporary file in the
/// same directory, fsync, rename over the final name, fsync of the directory.
/// A file already present is trusted only if it verifies; a damaged one is
/// replaced (returns `true` in the second position when that happened).
pub fn store_blob(
    data_dir: &Path,
    key: &[u8; 32],
    bytes: &[u8],
) -> Result<(String, bool), StoreError> {
    let sha = nf_core::sha256_hex(bytes);
    let path = blob_path(data_dir, &sha);
    let repaired = match load_blob(data_dir, key, &sha) {
        Ok(_) => return Ok((sha, false)),
        Err(BlobError::Missing) => false,
        Err(_) => true,
    };
    let dir = data_dir.join("blobs");
    let tmp = dir.join(format!(
        ".tmp-{}-{}",
        &sha[..16],
        hex::encode(crypto::random_bytes::<8>())
    ));
    let sealed = crypto::seal(key, bytes).map_err(StoreError::Failed)?;
    let write = || -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(&sealed)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &path)?;
        // Windows cannot open a directory as a file (access denied), so the directory flush is Unix only
        // (as in `create_private`). Before this change every blob store failed on Windows with os error 5.
        #[cfg(unix)]
        std::fs::File::open(&dir)?.sync_all()?;
        Ok(())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp);
        return Err(if is_out_of_space(&e) {
            StoreError::Full(format!("could not store file {sha}: {e}"))
        } else {
            StoreError::Failed(format!("could not store file {sha}: {e}"))
        });
    }
    Ok((sha, repaired))
}

/// Why a file could not be stored. A full disk is told apart from other failures (RA-010): the client gets
/// 507 for it, not a generic 500.
#[derive(Debug)]
pub enum StoreError {
    Full(String),
    Failed(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Full(m) | StoreError::Failed(m) => f.write_str(m),
        }
    }
}

/// The disk is full (ENOSPC, or the Windows equivalent): nothing was stored (RA-010).
pub fn is_out_of_space(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(28) | Some(112))
}

/// Temporary files left by a write that never finished are not blobs; remove them at start-up.
pub fn remove_stale_temporaries(data_dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(data_dir.join("blobs")) {
        for p in rd.flatten().map(|e| e.path()) {
            if p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(".tmp-"))
            {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}
