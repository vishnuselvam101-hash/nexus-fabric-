//! PDF to DXF inside the product (work plan task 3.1; review C findings RC-05, RC-06, RC-06-N1).
//!
//! The server never parses PDF data. A conversion launches the separate program `nf-pdf2dxf`
//! (crate `nf-pdf`) as a child process:
//! - no inherited environment (on Windows only `SystemRoot`), standard input closed;
//! - a fresh, owner-only working directory under the data directory (`convert-tmp/`), holding the
//!   decrypted PDF for the time of the conversion only, removed afterwards and at start-up;
//! - a kill timeout of its own ([`DEFAULT_TIMEOUT`]), longer than the converter's 90 s worker
//!   limit, applied by the blocking task itself so that it holds even when the request is dropped;
//! - one conversion at a time (the slot is held by the blocking task, not by the request).
//!
//! The program isolates its worker process itself before the worker reads any input: on Linux
//! resource limits, Landlock and a seccomp allow-list with no socket and no file opened by name
//! (`docs/backend/PDF_TO_DXF.md`, section Isolation). The server ENFORCES that this happened: a
//! result is accepted only when the program's report states the isolation that its platform
//! requires (Linux: the seccomp allow-list and the resource limits; Windows: the Job Object).
//! The service unit's own `RestrictNamespaces=true` makes `unshare -n` unavailable, so no
//! network namespace is added; the seccomp allow-list refuses every socket instead.
//!
//! A DXF is accepted only on exit 0, with the report's status `converted` and the report's
//! output hash equal to the SHA-256 of the DXF file (RC-06-N1). Exit 2 or 3 is a refusal: its
//! code, limit and message are passed on unchanged. Anything else (start failure, crash,
//! internal error, isolation not confirmed, the server's timeout) is an infrastructure problem.
//!
//! The result is "Converted from PDF - not verified - not released". It is stored as two
//! documents of the project (the DXF and the JSON report, encrypted like every document), with
//! an append-only record and an audit entry. It is never a flat pattern and is never attached
//! to a calculation: calculations take their geometry from the request only.

use super::*;
use std::io::Read;
use std::path::Path as FsPath;
use std::process::{Command, Stdio};

/// The label every conversion carries, in the API, the interface and the file name.
pub(crate) const LABEL: &str = "Converted from PDF - not verified - not released";

/// Implementation limit: the server stops the converter after this time. Longer than the
/// converter's own worker limit (90 s), shorter than the default request time limit (120 s).
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(110);

/// Implementation limits for reading the converter's files back (its own limits: DXF 64 MiB,
/// report 16 MiB) and its messages.
const MAX_DXF_BYTES: u64 = 64 << 20;
const MAX_REPORT_BYTES: u64 = 16 << 20;
const MAX_STDERR_BYTES: u64 = 256 << 10;

/// Largest page number the server passes on (the converter refuses pages past the end).
const MAX_PAGE: u64 = 100_000;

/// Directory under the data directory for the working directories of conversions.
pub(crate) const WORK_DIR: &str = "convert-tmp";

/// Name of the converter program.
pub(crate) fn program_name() -> &'static str {
    if cfg!(windows) {
        "nf-pdf2dxf.exe"
    } else {
        "nf-pdf2dxf"
    }
}

/// The converter: `NF_PDF2DXF` if set, otherwise the program beside this server's executable.
pub(crate) fn default_converter() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NF_PDF2DXF") {
        return Some(PathBuf::from(p));
    }
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(program_name()))
}

/// Working directories of conversions that never finished are removed at start-up.
pub(crate) fn remove_stale_work(data_dir: &FsPath) {
    let _ = std::fs::remove_dir_all(data_dir.join(WORK_DIR));
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ScaleArg {
    /// Millimetres per PDF point.
    MmPerPoint(f64),
    /// Drawing scale 1:N.
    Ratio(f64),
}

#[derive(Clone, Debug)]
pub(crate) struct ConvRequest {
    pub page: u64,
    pub scale: ScaleArg,
    pub tolerance_mm: Option<f64>,
    pub include_fill: bool,
    pub include_text: bool,
}

impl ConvRequest {
    fn args(&self) -> Vec<String> {
        let mut a = vec!["--page".to_string(), self.page.to_string()];
        match self.scale {
            ScaleArg::MmPerPoint(x) => {
                a.push("--mm-per-point".into());
                a.push(x.to_string());
            }
            ScaleArg::Ratio(n) => {
                a.push("--scale".into());
                a.push(format!("1:{n}"));
            }
        }
        if let Some(t) = self.tolerance_mm {
            a.push("--tolerance".into());
            a.push(t.to_string());
        }
        if !self.include_fill {
            a.push("--no-fill".into());
        }
        if !self.include_text {
            a.push("--no-text".into());
        }
        a
    }

    fn scale_json(&self) -> Value {
        match self.scale {
            ScaleArg::MmPerPoint(x) => json!({ "kind": "mm_per_point", "value": x }),
            ScaleArg::Ratio(n) => json!({ "kind": "ratio", "value": n }),
        }
    }
}

/// What a conversion ended with.
#[derive(Debug)]
pub(crate) enum Outcome {
    Converted {
        dxf: Vec<u8>,
        report: Vec<u8>,
        report_json: Value,
    },
    /// Exit 2 (refused) or 3 (limit exceeded): the converter's own code, limit and message.
    Refused {
        status: String,
        code: String,
        limit: Option<String>,
        message: String,
        report: Option<Vec<u8>>,
    },
    /// The conversion could not be run; says nothing about the document.
    Unavailable(String),
}

fn read_bounded(path: &FsPath, max: u64) -> Result<Vec<u8>, String> {
    let f =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", file_name(path)))?;
    let mut buf = Vec::new();
    f.take(max + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("cannot read {}: {e}", file_name(path)))?;
    if buf.len() as u64 > max {
        return Err(format!("{} is larger than {max} bytes", file_name(path)));
    }
    Ok(buf)
}

fn file_name(p: &FsPath) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A fresh owner-only directory for one conversion.
fn make_work_dir(data_dir: &FsPath) -> Result<PathBuf, String> {
    let root = data_dir.join(WORK_DIR);
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(&root)
        .map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let dir = root.join(format!("c-{}", hex::encode(crypto::random_bytes::<8>())));
    b.recursive(false);
    b.create(&dir)
        .map_err(|e| format!("cannot create a working directory: {e}"))?;
    Ok(dir)
}

fn write_private(path: &FsPath, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(bytes)?;
    f.flush()
}

/// The isolation that the report must state before a DXF is accepted on this platform.
fn isolation_confirmed(report: &Value) -> Result<(), String> {
    let iso = &report["isolation"];
    #[cfg(target_os = "linux")]
    {
        let seccomp = iso["seccomp"].as_str().unwrap_or("");
        if !seccomp.starts_with("allow-list enforced") || !iso["resource_limits"].is_string() {
            return Err(format!(
                "the converter's report does not confirm its isolation (seccomp allow-list and resource limits): {iso}"
            ));
        }
    }
    #[cfg(windows)]
    {
        if !iso["job_object"].is_string() {
            return Err(format!(
                "the converter's report does not confirm its Job Object: {iso}"
            ));
        }
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        if !iso["resource_limits"].is_string() {
            return Err(format!(
                "the converter's report does not confirm its resource limits: {iso}"
            ));
        }
    }
    Ok(())
}

/// The converter's one-line JSON message on standard error (the last such line).
fn message_line(stderr: &str) -> Option<Value> {
    stderr
        .lines()
        .rev()
        .filter(|l| l.trim_start().starts_with('{'))
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
}

/// Runs the converter once. Blocking: call it from `spawn_blocking`.
pub(crate) fn run_converter(
    exe: &FsPath,
    timeout: Duration,
    data_dir: &FsPath,
    pdf: &[u8],
    req: &ConvRequest,
) -> Outcome {
    let dir = match make_work_dir(data_dir) {
        Ok(d) => d,
        Err(e) => return Outcome::Unavailable(e),
    };
    let out = run_in(exe, timeout, &dir, pdf, req);
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn run_in(exe: &FsPath, timeout: Duration, dir: &FsPath, pdf: &[u8], req: &ConvRequest) -> Outcome {
    let input = dir.join("in.pdf");
    let output = dir.join("out.dxf");
    let report = dir.join("report.json");
    if let Err(e) = write_private(&input, pdf) {
        return Outcome::Unavailable(format!("cannot write the working copy of the PDF: {e}"));
    }
    let mut cmd = Command::new(exe);
    cmd.arg("--in")
        .arg(&input)
        .arg("--out")
        .arg(&output)
        .arg("--report")
        .arg(&report)
        .args(req.args())
        .env_clear()
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let (true, Some(root)) = (cfg!(windows), std::env::var_os("SystemRoot")) {
        cmd.env("SystemRoot", root);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Outcome::Unavailable(format!(
                "the converter {} could not be started: {e}",
                exe.display()
            ));
        }
    };
    let reader = child.stderr.take().map(|pipe| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.take(MAX_STDERR_BYTES).read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    });
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Ok(st),
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!(
                    "the converter did not finish within {} s and was stopped",
                    timeout.as_secs_f32()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("cannot wait for the converter: {e}"));
            }
        }
    };
    // After a kill the pipe may stay open in a grandchild for a moment; the reader is bounded
    // and is not waited for after a timeout.
    let stderr = match (&status, reader) {
        (Ok(_), Some(r)) => r.join().unwrap_or_default(),
        _ => String::new(),
    };
    let status = match status {
        Ok(s) => s,
        Err(e) => return Outcome::Unavailable(e),
    };
    let line = message_line(&stderr);
    match status.code() {
        Some(0) => accept(&output, &report),
        Some(2) | Some(3) => {
            let Some(v) = line else {
                return Outcome::Unavailable(format!(
                    "the converter refused without a readable message (exit {status})"
                ));
            };
            let code = v["code"].as_str().unwrap_or("").to_string();
            if code.is_empty() {
                return Outcome::Unavailable("the converter refused without a code".into());
            }
            Outcome::Refused {
                status: v["status"].as_str().unwrap_or("refused").to_string(),
                code,
                limit: v["limit"].as_str().map(String::from),
                message: v["message"].as_str().unwrap_or("").to_string(),
                report: read_bounded(&report, MAX_REPORT_BYTES).ok(),
            }
        }
        _ => {
            let detail = line
                .map(|v| {
                    format!(
                        "{}: {}",
                        v["code"].as_str().unwrap_or("?"),
                        v["message"].as_str().unwrap_or("")
                    )
                })
                .unwrap_or_else(|| clip(stderr.trim(), 500));
            Outcome::Unavailable(format!("the converter failed ({status}): {detail}"))
        }
    }
}

/// Exit 0: the DXF is accepted only with a report that says `converted`, states the isolation
/// and carries the DXF's own hash.
fn accept(output: &FsPath, report: &FsPath) -> Outcome {
    let rep = match read_bounded(report, MAX_REPORT_BYTES) {
        Ok(r) => r,
        Err(e) => return Outcome::Unavailable(format!("the converter's report: {e}")),
    };
    let dxf = match read_bounded(output, MAX_DXF_BYTES) {
        Ok(d) => d,
        Err(e) => return Outcome::Unavailable(format!("the converter's DXF: {e}")),
    };
    let v: Value = match serde_json::from_slice(&rep) {
        Ok(v) => v,
        Err(e) => return Outcome::Unavailable(format!("the converter's report is not JSON: {e}")),
    };
    if v["status"] != "converted" {
        return Outcome::Unavailable(format!("exit 0 but the report says status {}", v["status"]));
    }
    if dxf.is_empty() || v["output"]["dxf_sha256"].as_str() != Some(&nf_core::sha256_hex(&dxf)) {
        return Outcome::Unavailable(
            "the DXF does not match the hash in the converter's report".into(),
        );
    }
    if let Err(e) = isolation_confirmed(&v) {
        return Outcome::Unavailable(e);
    }
    Outcome::Converted {
        dxf,
        report: rep,
        report_json: v,
    }
}

// ---------------------------------------------------------------- the request

/// A refusal of the request by the server itself, before the converter runs (same codes as
/// the converter where the rule is the same).
fn refuse(code: &str, msg: &str) -> ApiError {
    fail_json(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": msg, "code": code, "message": msg, "status": "refused", "limit": null }),
    )
}

fn positive_number(body: &Value, key: &str) -> Option<f64> {
    body.get(key).and_then(Value::as_f64)
}

/// Reads the conversion request. The scale is required; nothing is guessed.
pub(crate) fn parse_request(body: &Value) -> Result<ConvRequest, ApiError> {
    let page = match body.get("page") {
        None | Some(Value::Null) => 1,
        Some(v) => match v.as_u64() {
            Some(n) if (1..=MAX_PAGE).contains(&n) => n,
            _ => {
                return Err(refuse(
                    "invalid_page",
                    &format!("page must be a whole number from 1 to {MAX_PAGE}"),
                ));
            }
        },
    };
    let kind = body.get("scale_kind").and_then(Value::as_str);
    let value = body.get("scale_value");
    let scale = match (kind, value) {
        (None, None)
        | (None, Some(Value::Null))
        | (Some(_), None)
        | (Some(_), Some(Value::Null)) => {
            return Err(refuse(
                "scale_required",
                "no scale given: give millimetres per PDF point or a drawing scale 1:N. A PDF has no trustworthy real-world scale, so none is guessed.",
            ));
        }
        (Some("mm_per_point"), Some(_)) => match positive_number(body, "scale_value") {
            Some(x) => ScaleArg::MmPerPoint(x),
            None => return Err(refuse("invalid_scale", "the scale must be a number")),
        },
        (Some("ratio"), Some(_)) => match positive_number(body, "scale_value") {
            Some(n) => ScaleArg::Ratio(n),
            None => return Err(refuse("invalid_scale", "the scale must be a number")),
        },
        _ => {
            return Err(refuse(
                "invalid_scale",
                "scale_kind must be mm_per_point or ratio",
            ));
        }
    };
    let tolerance_mm = match body.get("tolerance_mm") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_f64() {
            Some(t) => Some(t),
            None => {
                return Err(refuse(
                    "invalid_tolerance",
                    "the tolerance must be a number",
                ));
            }
        },
    };
    let flag = |k: &str| -> Result<bool, ApiError> {
        match body.get(k) {
            None | Some(Value::Null) => Ok(true),
            Some(Value::Bool(b)) => Ok(*b),
            Some(_) => Err(refuse(
                "invalid_arguments",
                &format!("{k} must be true or false"),
            )),
        }
    };
    Ok(ConvRequest {
        page,
        scale,
        tolerance_mm,
        include_fill: flag("include_fill")?,
        include_text: flag("include_text")?,
    })
}

fn stem(filename: &str) -> String {
    let base = filename.rsplit_once('.').map_or(filename, |(s, _)| s);
    let s: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    if s.is_empty() { "document".into() } else { s }
}

/// A new immutable snapshot of the project's documents, linked to its predecessor (as an upload makes).
pub(crate) fn snapshot(app: &App, tx: &Connection, project: i64) -> Result<String, ApiError> {
    let members: Vec<String> = q_all(
        tx,
        "SELECT record_id FROM documents WHERE project_id = ? ORDER BY record_id",
        params![project],
        |r| r.get(0),
    )?;
    let prev: Option<String> = tx.query_row("SELECT id FROM records WHERE project_id = ? AND kind = 'snapshot' ORDER BY rowid DESC LIMIT 1", params![project], |r| r.get(0)).optional()?;
    let mut rels: Vec<nf_core::Rel> = members.iter().map(|m| rel("contains", m)).collect();
    if let Some(p) = &prev {
        rels.push(rel("predecessor", p));
    }
    let snap = Record::new(
        Family::Source,
        "snapshot",
        json!({ "project": project, "artifact_count": members.len() }),
        rels,
        None,
    );
    app.put_record(tx, project, &snap)?;
    Ok(snap.id)
}

/// The columns of a conversion as the API shows them.
/// The documents of a conversion point to it (`documents.conversion_id`); nothing is updated.
pub(crate) const CONV_COLS: &str = "c.id, c.project_id, c.source_document, c.page, c.scale_kind, c.scale_value, c.tolerance_mm, c.include_fill, c.include_text, c.status, c.code, c.limit_name, c.message, c.converter_version, c.source_sha256, (SELECT d.id FROM documents d WHERE d.conversion_id = c.id AND d.detected_type = 'dxf'), (SELECT d.id FROM documents d WHERE d.conversion_id = c.id AND d.detected_type = 'pdf_conversion_report'), c.dxf_sha256, c.report_sha256, c.created_by, c.created_at";

pub(crate) fn conv_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?, "project": r.get::<_, i64>(1)?, "source_document": r.get::<_, i64>(2)?,
        "page": r.get::<_, i64>(3)?, "scale": { "kind": r.get::<_, String>(4)?, "value": r.get::<_, f64>(5)? },
        "tolerance_mm": r.get::<_, Option<f64>>(6)?, "include_fill": r.get::<_, bool>(7)?, "include_text": r.get::<_, bool>(8)?,
        "status": r.get::<_, String>(9)?, "code": r.get::<_, Option<String>>(10)?, "limit": r.get::<_, Option<String>>(11)?,
        "message": r.get::<_, Option<String>>(12)?, "converter_version": r.get::<_, Option<String>>(13)?,
        "source_sha256": r.get::<_, String>(14)?, "dxf_document": r.get::<_, Option<i64>>(15)?,
        "report_document": r.get::<_, Option<i64>>(16)?, "dxf_sha256": r.get::<_, Option<String>>(17)?,
        "report_sha256": r.get::<_, Option<String>>(18)?, "created_by": r.get::<_, String>(19)?,
        "created_at": r.get::<_, i64>(20)?, "label": LABEL, "release_state": "NOT RELEASED",
        "flat_pattern": false,
    }))
}

/// POST /api/documents/{id}/pdf-to-dxf
pub(crate) async fn convert(
    State(app): Ctx,
    headers: HeaderMap,
    Path(doc): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let (u, project, filename, sha, record_id) = {
        let db = app.db();
        let u = auth_db(&db, &app, &headers)?;
        require(&u, &["admin", "engineer"])?;
        let row: Option<(i64, String, String, String, String)> = db
            .query_row(
                "SELECT project_id, filename, detected_type, sha256, record_id FROM documents WHERE id = ?",
                params![doc],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((project, filename, detected, sha, record_id)) = row else {
            return Err(not_found());
        };
        project_guard(&app, &db, &u, project, "convert PDF")?;
        if detected != "pdf" {
            return Err(refuse(
                "not_a_pdf",
                "only a document whose content was detected as PDF can be converted",
            ));
        }
        (u, project, filename, sha, record_id)
    };
    let req = parse_request(&body)?;
    let Some(exe) = app.pdf2dxf_exe.clone() else {
        return Err(unavailable(
            &app,
            &u,
            doc,
            "no converter program is configured",
        ));
    };
    if !exe.is_file() {
        return Err(unavailable(
            &app,
            &u,
            doc,
            &format!("the converter program {} is not installed", exe.display()),
        ));
    }
    let Ok(slot) = app.convert_slots.clone().try_acquire_owned() else {
        return Err(fail_json(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "a conversion is already running; retry shortly", "retry": true }),
        ));
    };
    let pdf = match storage::load_blob(&app.data_dir, &app.key, &sha) {
        Ok(b) => b,
        Err(problem) => {
            eprintln!(
                "nf-server: document #{doc} ({sha}) cannot be read for conversion: {problem}"
            );
            return Err(fail(
                StatusCode::CONFLICT,
                "the stored PDF failed its integrity check and is withheld; nothing was converted",
            ));
        }
    };
    let outcome = {
        let (timeout, data_dir, r) = (app.convert_timeout, app.data_dir.clone(), req.clone());
        tokio::task::spawn_blocking(move || {
            // The slot belongs to the child process, not to the request.
            let _slot = slot;
            run_converter(&exe, timeout, &data_dir, &pdf, &r)
        })
        .await
        .unwrap_or_else(|e| Outcome::Unavailable(format!("conversion task failed: {e}")))
    };
    record(
        &app, &u, project, doc, &filename, &sha, &record_id, &req, outcome,
    )
}

fn unavailable(app: &App, u: &User, doc: i64, problem: &str) -> ApiError {
    eprintln!("nf-server: conversion of document #{doc} could not be run: {problem}");
    let db = app.db();
    if let Err(e) = app.audit(
        &db,
        &u.username,
        "pdf_conversion_unavailable",
        &format!("document #{doc}: {problem}"),
    ) {
        eprintln!("nf-server: could not audit: {e}");
    }
    fail_json(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({ "error": format!("the conversion could not be run ({problem}). This is an infrastructure problem, not a finding about the document. Nothing was stored."), "infrastructure": true }),
    )
}

#[allow(clippy::too_many_arguments)]
fn record(
    app: &App,
    u: &User,
    project: i64,
    doc: i64,
    filename: &str,
    source_sha: &str,
    source_record: &str,
    req: &ConvRequest,
    outcome: Outcome,
) -> Api {
    let (status, code, limit, message, dxf, report, report_json) = match outcome {
        Outcome::Unavailable(problem) => return Err(unavailable(app, u, doc, &problem)),
        Outcome::Converted {
            dxf,
            report,
            report_json,
        } => (
            "converted".to_string(),
            None,
            None,
            None,
            Some(dxf),
            Some(report),
            Some(report_json),
        ),
        Outcome::Refused {
            status,
            code,
            limit,
            message,
            report,
        } => {
            let rj = report
                .as_deref()
                .and_then(|r| serde_json::from_slice::<Value>(r).ok());
            (status, Some(code), limit, Some(message), None, report, rj)
        }
    };
    let converter_version = report_json
        .as_ref()
        .and_then(|v| v["converter_version"].as_str().map(String::from));
    let dxf_sha = match &dxf {
        Some(d) => Some(app.store_blob(d)?),
        None => None,
    };
    let report_sha = match &report {
        Some(r) => Some(app.store_blob(r)?),
        None => None,
    };
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    project_guard(app, &tx, u, project, "convert PDF")?;
    let rec = Record::new(
        Family::Activity,
        "pdf_conversion",
        json!({
            "source_document": doc, "source_sha256": source_sha, "page": req.page, "scale": req.scale_json(),
            "tolerance_mm": req.tolerance_mm, "include_fill": req.include_fill, "include_text": req.include_text,
            "status": status, "code": code, "limit": limit, "message": message,
            "converter_version": converter_version, "dxf_sha256": dxf_sha, "report_sha256": report_sha,
            "label": LABEL, "release_state": "NOT RELEASED", "flat_pattern": false, "entered_by": u.username,
        }),
        vec![rel("derived_from", source_record)],
        None,
    );
    app.put_record(&tx, project, &rec)?;
    let scale_value = match req.scale {
        ScaleArg::MmPerPoint(x) | ScaleArg::Ratio(x) => x,
    };
    let scale_kind = match req.scale {
        ScaleArg::MmPerPoint(_) => "mm_per_point",
        ScaleArg::Ratio(_) => "ratio",
    };
    tx.execute(
        "INSERT INTO pdf_conversions(project_id, source_document, page, scale_kind, scale_value, tolerance_mm, include_fill, include_text, status, code, limit_name, message, converter_version, source_sha256, dxf_sha256, report_sha256, record_id, created_by, created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        params![project, doc, req.page as i64, scale_kind, scale_value, req.tolerance_mm, req.include_fill, req.include_text, status, code, limit, message, converter_version, source_sha, dxf_sha, report_sha, rec.id, u.username, now() as i64],
    )?;
    let id = tx.last_insert_rowid();
    let converted = matches!(
        (&dxf, &dxf_sha, &report, &report_sha),
        (Some(_), Some(_), Some(_), Some(_))
    );
    if let (Some(d), Some(ds), Some(r), Some(rs)) = (&dxf, &dxf_sha, &report, &report_sha) {
        let base = format!("{}.p{}", stem(filename), req.page);
        for (name, dtype, bytes, sha) in [
            (
                format!("NOT-RELEASED_from-pdf_{base}.dxf"),
                "dxf",
                d.len(),
                ds,
            ),
            (
                format!("NOT-RELEASED_from-pdf_{base}.report.json"),
                "pdf_conversion_report",
                r.len(),
                rs,
            ),
        ] {
            tx.execute(
                "INSERT INTO documents(project_id, filename, detected_type, size, sha256, extension_mismatch, record_id, created_by, created_at, conversion_id) VALUES (?,?,?,?,?,0,?,?,?,?)",
                params![project, name, dtype, bytes as i64, sha, rec.id, u.username, now() as i64, id],
            )?;
        }
        snapshot(app, &tx, project)?;
    }
    app.fault("convert.before_commit")?;
    let detail = match (&code, &limit) {
        (None, _) => format!(
            "conversion #{id}: document #{doc} page {} scale {scale_kind} {scale_value} -> DXF {} (not released)",
            req.page,
            dxf_sha.as_deref().unwrap_or("")
        ),
        (Some(c), Some(l)) => format!(
            "conversion #{id}: document #{doc} page {}: {status} {c} ({l})",
            req.page
        ),
        (Some(c), None) => format!(
            "conversion #{id}: document #{doc} page {}: {status} {c}",
            req.page
        ),
    };
    let action = if converted {
        "pdf_converted"
    } else {
        "pdf_conversion_refused"
    };
    app.audit(&tx, &u.username, action, &detail)?;
    let row = tx.query_row(
        &format!("SELECT {CONV_COLS} FROM pdf_conversions c WHERE c.id = ?"),
        params![id],
        conv_row,
    )?;
    tx.commit()?;
    let mut answer = json!({ "conversion": row, "report": report_json });
    if code.is_some() {
        answer["error"] = json!(message.clone().unwrap_or_default());
        answer["code"] = json!(code);
        answer["limit"] = json!(limit);
        answer["message"] = json!(message);
        answer["status"] = json!(status);
        return Err(fail_json(StatusCode::UNPROCESSABLE_ENTITY, answer));
    }
    ok(answer)
}

/// The conversion and the project it belongs to, if the user may see it.
fn conv_guard(app: &App, db: &Connection, u: &User, id: i64) -> Result<Value, ApiError> {
    let row = db
        .query_row(
            &format!("SELECT {CONV_COLS} FROM pdf_conversions c WHERE c.id = ?"),
            params![id],
            conv_row,
        )
        .optional()?;
    let Some(row) = row else {
        return Err(not_found());
    };
    project_guard(
        app,
        db,
        u,
        row["project"].as_i64().unwrap_or(-1),
        "read PDF conversion",
    )?;
    Ok(row)
}

fn load(app: &App, sha: &str) -> Result<Vec<u8>, ApiError> {
    storage::load_blob(&app.data_dir, &app.key, sha).map_err(|problem| {
        eprintln!("nf-server: stored file {sha} of a PDF conversion: {problem}");
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the stored file failed its integrity check and is withheld; convert the PDF again",
        )
    })
}

/// GET /api/pdf-conversions/{id}: the conversion and its report.
pub(crate) async fn get_conversion(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let row = conv_guard(&app, &db, &u, id)?;
    let report = match row["report_sha256"].as_str() {
        Some(s) => serde_json::from_slice::<Value>(&load(&app, s)?).unwrap_or(Value::Null),
        None => Value::Null,
    };
    ok(json!({ "conversion": row, "report": report }))
}

/// GET /api/pdf-conversions/{id}/{file}: `dxf` or `report`, as a download.
pub(crate) async fn get_file(
    State(app): Ctx,
    headers: HeaderMap,
    Path((id, file)): Path<(i64, String)>,
) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    let row = conv_guard(&app, &db, &u, id)?;
    let (key, ctype, ext) = match file.as_str() {
        "dxf" => ("dxf_sha256", "application/dxf", "dxf"),
        "report" => ("report_sha256", "application/json", "report.json"),
        _ => return Err(not_found()),
    };
    let Some(sha) = row[key].as_str().map(String::from) else {
        return Err(fail(
            StatusCode::NOT_FOUND,
            "no such file: the conversion was refused and produced no DXF",
        ));
    };
    let bytes = load(&app, &sha)?;
    app.audit(
        &db,
        &u.username,
        "pdf_conversion_download",
        &format!("conversion #{id} {file} {sha}"),
    )?;
    let name = format!(
        "NOT-RELEASED_from-pdf_conversion{id}_p{}_{}.{ext}",
        row["page"],
        &sha[..8]
    );
    Ok((
        [
            (header::CONTENT_TYPE, ctype.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}
