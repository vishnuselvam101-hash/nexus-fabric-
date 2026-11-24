//! AMOEBA digest in the server. Every DXF a job produces is checked by nf-digest against the job's own
//! reference, and the check is kept as an immutable snapshot (the DXF hash, the inputs hash, the engine,
//! every check record). Gate G23 reads the snapshots; a DXF is served only when every check of its
//! snapshot PASSes. The digest decides nothing by itself: a FAIL or INCONCLUSIVE only holds the file.

use nf_core::job::{CurveView, JobOutput, PatternView};
use nf_digest::{Check, RefCurve, Reference, Report};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

/// Gate of the digest (the next free id after G22).
pub const GATE_ID: &str = "G23";

fn curve(c: &CurveView) -> RefCurve {
    RefCurve {
        points: c.points.clone(),
        length: c.length,
        max_dev: c.points_max_deviation,
        is_arc: c.kind == "arc",
    }
}

/// The reference one part is checked against: the pattern's outline and openings as the engine declares them.
pub fn reference(part: &str, pattern: Option<&PatternView>) -> Reference {
    match pattern {
        Some(p) => Reference {
            part: part.to_string(),
            outer: p.curves.iter().map(curve).collect(),
            holes: p.holes.iter().map(curve).collect(),
        },
        None => Reference { part: part.to_string(), outer: Vec::new(), holes: Vec::new() },
    }
}

/// One digest of one DXF file, ready to be stored.
pub struct Digest {
    pub part: String,
    pub dxf_sha256: String,
    pub inputs_sha256: String,
    pub report: Report,
}

/// Digest every DXF of a job against its own reference. The inputs hash covers the request and the
/// pattern that the file was computed from.
pub fn digest_job(out: &JobOutput) -> Vec<Digest> {
    out.dxf
        .iter()
        .map(|(part, text)| {
            let pattern = out.patterns.iter().find(|p| &p.part == part);
            let reference = reference(part, pattern);
            let inputs = json!({ "request": &out.request, "pattern": pattern });
            Digest {
                part: part.clone(),
                dxf_sha256: nf_core::sha256_hex(text.as_bytes()),
                inputs_sha256: nf_core::sha256_hex(inputs.to_string().as_bytes()),
                report: nf_digest::check_text(text, &reference),
            }
        })
        .collect()
}

fn check_json(c: &Check) -> Value {
    json!({
        "name": c.name,
        "verdict": c.verdict.as_str(),
        "measured": c.measured,
        "expected": c.expected,
        "tolerance": c.tolerance,
        "detail": c.detail,
    })
}

fn report_json(r: &Report) -> Value {
    json!({
        "verdict": r.verdict().as_str(),
        "read_error": r.read_error,
        "checks": r.checks.iter().map(check_json).collect::<Vec<Value>>(),
    })
}

/// Store the snapshots of a job. Rows are never updated or deleted (append-only triggers).
pub fn store(db: &Connection, calc: i64, digests: &[Digest], engine: &str, at: i64) -> rusqlite::Result<()> {
    for d in digests {
        let checks = report_json(&d.report)["checks"].to_string();
        db.execute(
            "INSERT INTO amoeba_digests(calc_id, part, dxf_sha256, inputs_sha256, engine, verdict, checks_json, created_at) VALUES (?,?,?,?,?,?,?,?)",
            params![calc, d.part, d.dxf_sha256, d.inputs_sha256, engine, d.report.verdict().as_str(), checks, at],
        )?;
    }
    Ok(())
}

/// One stored snapshot.
pub struct Row {
    pub part: String,
    pub dxf_sha256: String,
    pub inputs_sha256: String,
    pub engine: String,
    pub verdict: String,
    pub checks: Value,
    pub created_at: i64,
}

pub fn rows(db: &Connection, calc: i64) -> rusqlite::Result<Vec<Row>> {
    let mut st = db.prepare(
        "SELECT part, dxf_sha256, inputs_sha256, engine, verdict, checks_json, created_at FROM amoeba_digests WHERE calc_id = ? ORDER BY part",
    )?;
    let mapped = st.query_map(params![calc], |r| {
        Ok(Row {
            part: r.get(0)?,
            dxf_sha256: r.get(1)?,
            inputs_sha256: r.get(2)?,
            engine: r.get(3)?,
            verdict: r.get(4)?,
            checks: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or(Value::Null),
            created_at: r.get(6)?,
        })
    })?;
    mapped.collect()
}

/// Gate G23 from the snapshots: FAIL if any file has a failing check, INCONCLUSIVE if any check could not
/// be judged or no snapshot exists, PASS only when every file passes every check.
pub fn gate(rows: &[Row]) -> (nf_core::GateResult, String) {
    use nf_core::GateResult as R;
    if rows.is_empty() {
        return (R::Inconclusive, "No AMOEBA digest is recorded for this result, so its DXF files cannot be confirmed.".into());
    }
    let failing: Vec<String> = rows
        .iter()
        .filter(|r| r.verdict == "FAIL")
        .map(|r| format!("{}: {}", r.part, failed_names(&r.checks, "FAIL").join(", ")))
        .collect();
    if !failing.is_empty() {
        return (R::Fail, format!("The AMOEBA digest found defects in the DXF file(s): {}. Such a file is never served.", failing.join("; ")));
    }
    let open: Vec<String> = rows
        .iter()
        .filter(|r| r.verdict != "PASS")
        .map(|r| format!("{}: {}", r.part, failed_names(&r.checks, "INCONCLUSIVE").join(", ")))
        .collect();
    if !open.is_empty() {
        return (R::Inconclusive, format!("The AMOEBA digest could not confirm every check of the DXF file(s): {}. The file is held until each check passes.", open.join("; ")));
    }
    (R::Pass, format!("The AMOEBA digest passes every check of all {} DXF file(s) (engine {}).", rows.len(), rows[0].engine))
}

fn failed_names(checks: &Value, verdict: &str) -> Vec<String> {
    checks
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["verdict"] == verdict)
                .filter_map(|c| c["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The snapshot of one part, when every check of it passed; otherwise the failing checks as JSON.
pub fn hold_for(rows: &[Row], part: &str) -> Result<(), (String, Value)> {
    match rows.iter().find(|r| r.part == part) {
        None => Err(("amoeba_digest_missing".into(), json!([]))),
        Some(r) if r.verdict == "PASS" => Ok(()),
        Some(r) => {
            let names: Vec<Value> = r
                .checks
                .as_array()
                .map(|a| a.iter().filter(|c| c["verdict"] != "PASS").cloned().collect())
                .unwrap_or_default();
            let code = if r.verdict == "FAIL" { "amoeba_digest_failed" } else { "amoeba_digest_inconclusive" };
            Err((code.into(), Value::Array(names)))
        }
    }
}

/// The stage view of a job: surround, engulf, digest per check, confirm or hold, export. Plain JSON, for polling.
pub fn stages(id: i64, state: &str, gates: &[nf_core::Gate], rows: &[Row]) -> Value {
    let digest_status = if rows.is_empty() {
        "PENDING".to_string()
    } else if rows.iter().any(|r| r.verdict == "FAIL") {
        "FAIL".into()
    } else if rows.iter().all(|r| r.verdict == "PASS") {
        "PASS".into()
    } else {
        "INCONCLUSIVE".into()
    };
    let all_pass = !rows.is_empty() && rows.iter().all(|r| r.verdict == "PASS");
    let mut hold: Vec<String> = gates
        .iter()
        .filter(|g| !matches!(g.result, nf_core::GateResult::Pass | nf_core::GateResult::NotApplicable))
        .map(|g| format!("{} {:?}", g.id, g.result))
        .collect();
    if state != "REVIEW_REQUIRED" {
        hold.push(format!("state {state}"));
    }
    let mut export_reasons = Vec::new();
    if !all_pass {
        export_reasons.push(format!("digest {digest_status}"));
    }
    if state != "REVIEW_REQUIRED" {
        export_reasons.push(format!("state {state}"));
    }
    let export = if export_reasons.is_empty() { "AVAILABLE" } else { "WITHHELD" };
    json!({
        "calculation": id,
        "state": state,
        "stages": [
            { "stage": "surround", "status": "DONE", "detail": format!("job computed; {} DXF file(s) produced", rows.len()) },
            { "stage": "engulf", "status": if rows.is_empty() { "PENDING" } else { "DONE" }, "snapshots": rows.iter().map(|r| json!({
                "part": r.part, "dxf_sha256": r.dxf_sha256, "inputs_sha256": r.inputs_sha256, "engine": r.engine, "created_at": r.created_at
            })).collect::<Vec<Value>>() },
            { "stage": "digest", "status": digest_status, "parts": rows.iter().map(|r| json!({
                "part": r.part, "verdict": r.verdict, "checks": r.checks
            })).collect::<Vec<Value>>() },
            { "stage": "confirm", "status": if hold.is_empty() { "CLEAR" } else { "HOLD" }, "reasons": hold },
            { "stage": "export", "status": export, "reasons": export_reasons },
        ],
    })
}
