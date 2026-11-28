//! AMOEBA digest in the server. Every DXF a job produces is checked by nf-digest against the job's own
//! reference, and the check is kept as an immutable snapshot (the DXF hash, the inputs hash, the engine,
//! every check record). Gate G23 reads the snapshots; a DXF is served only when every check of its
//! snapshot PASSes. The digest decides nothing by itself: a FAIL or INCONCLUSIVE only holds the file.

use nf_core::job::{CurveView, JobOutput, PatternView};
use nf_core::order_compute::{PartResult, PatternPiece};
use nf_digest::{Check, RefCurve, Reference, Report};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

/// Gate of the digest (the next free id after G22).
pub const GATE_ID: &str = "G23";

/// `PatternView` does not carry the engine's `eps_rep_certified` flag; its declared assumptions do. Read only
/// that statement, and call a bound uncertified when the statement is absent or says so (fail closed).
fn eps_rep_certified(p: &PatternView) -> bool {
    if p.eps_rep <= 0.0 {
        return false;
    }
    let certified = p
        .assumptions
        .iter()
        .any(|a| a.contains("eps_rep is a certified upper bound"));
    let not_certified = p
        .assumptions
        .iter()
        .any(|a| a.contains("eps_rep is NOT certified") || a.contains("NOT certified"));
    certified && !not_certified
}

fn curve(c: &CurveView, certified: bool) -> RefCurve {
    // Only a drawn polyline has a representation bound. An arc's points are display chords; the DXF carries
    // the exact arc, and a line is exact.
    let eps_rep = if c.kind == "polyline" && !c.points_exact && c.points_max_deviation > 0.0 {
        Some(c.points_max_deviation)
    } else {
        None
    };
    RefCurve {
        points: c.points.clone(),
        length: c.length,
        max_dev: c.points_max_deviation,
        is_arc: c.kind == "arc",
        // the engine's seam pair: the two straight edges named seam_a and seam_b (uem::develop)
        seam: c.kind == "line" && (c.name == "seam_a" || c.name == "seam_b"),
        eps_rep,
        eps_rep_certified: eps_rep.map(|_| certified),
    }
}

/// The reference one part is checked against: the pattern's outline and openings as the engine declares them.
pub fn reference(part: &str, pattern: Option<&PatternView>) -> Reference {
    match pattern {
        Some(p) => {
            let certified = eps_rep_certified(p);
            Reference {
                part: part.to_string(),
                outer: p.curves.iter().map(|c| curve(c, certified)).collect(),
                holes: p.holes.iter().map(|c| curve(c, certified)).collect(),
                eps_rep: (p.eps_rep > 0.0).then_some(p.eps_rep),
                // no drawn-polyline bound means the certification statement does not apply to this reference
                eps_rep_certified: (p.eps_rep > 0.0).then_some(certified),
            }
        }
        None => Reference {
            part: part.to_string(),
            outer: Vec::new(),
            holes: Vec::new(),
            eps_rep: None,
            eps_rep_certified: Some(false),
        },
    }
}

/// Reference of one cut piece of an `order_compute` result. The NFJ-1 shapes do not go through `job::run`; their
/// digest still has to use the engine's own certification statement, not a value copied from the file.
pub fn reference_for_piece(part: &PartResult, piece: &PatternPiece) -> Reference {
    let bound = part.eps_rep.filter(|v| v.is_finite() && *v > 0.0);
    let certified = part.certified && part.eps_rep_certified == Some(true);
    // every outline and opening is a closed profile in the DXF: the closing segment back to the first vertex is part
    // of the reference, as it is of the drawn polyline
    let polyline = |points: &[[f64; 2]]| {
        let mut points = points.to_vec();
        if points.len() >= 2 && points.first() != points.last() {
            points.push(points[0]);
        }
        let length = points
            .windows(2)
            .map(|w| ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt())
            .sum();
        RefCurve {
            points,
            length,
            max_dev: bound.unwrap_or(0.0),
            is_arc: false,
            seam: false,
            eps_rep: bound,
            eps_rep_certified: bound.map(|_| certified),
        }
    };
    let mut outer = Vec::new();
    if !piece.outline.is_empty() {
        outer.push(polyline(&piece.outline));
    }
    let mut holes: Vec<RefCurve> = piece
        .holes
        .iter()
        .filter(|h| !h.is_empty())
        .map(|h| polyline(h))
        .collect();
    for [cx, cy, r] in &piece.circles {
        if !r.is_finite() || *r <= 0.0 {
            continue;
        }
        let pts: Vec<[f64; 2]> = (0..=64)
            .map(|k| {
                let t = 2.0 * std::f64::consts::PI * k as f64 / 64.0;
                [cx + r * t.cos(), cy + r * t.sin()]
            })
            .collect();
        let length = 2.0 * std::f64::consts::PI * r;
        holes.push(RefCurve {
            points: pts,
            length,
            max_dev: 0.0,
            is_arc: true,
            seam: false,
            eps_rep: None,
            eps_rep_certified: None,
        });
    }
    Reference {
        part: part.no.clone(),
        outer,
        holes,
        eps_rep: bound,
        // the engine statement is carried even without a drawn-polyline bound: a withheld shape holds its DXF
        eps_rep_certified: Some(certified),
    }
}

/// Digest every order DXF against its own pattern piece before it is served. A part whose file is not a PASS
/// loses the file: `dxf` is cleared and the part is blocked with `digest_failed`.
pub fn gate_order_dxf(res: &mut nf_core::order_compute::OrderResult) {
    for p in res.parts.iter_mut() {
        let Some(text) = p.dxf.clone() else { continue };
        let passed = p.pattern.first().is_some_and(|pc| {
            let reference = reference_for_piece(p, pc);
            nf_digest::check_text(&text, &reference).verdict() == nf_digest::Verdict::Pass
        });
        if !passed {
            p.dxf = None;
            p.dxf_allowed = false;
            p.dxf_blocked_by.push("digest_failed".into());
            p.dxf_blocked_by.sort();
            p.dxf_blocked_by.dedup();
        }
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
pub fn store(
    db: &Connection,
    calc: i64,
    digests: &[Digest],
    engine: &str,
    at: i64,
) -> rusqlite::Result<()> {
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
        return (
            R::Inconclusive,
            "No AMOEBA digest is recorded for this result, so its DXF files cannot be confirmed."
                .into(),
        );
    }
    let failing: Vec<String> = rows
        .iter()
        .filter(|r| r.verdict == "FAIL")
        .map(|r| format!("{}: {}", r.part, check_lines(&r.checks, "FAIL").join("; ")))
        .collect();
    if !failing.is_empty() {
        return (
            R::Fail,
            format!(
                "The AMOEBA digest found defects in the DXF file(s): {}. Such a file is never served.",
                failing.join("; ")
            ),
        );
    }
    let open: Vec<String> = rows
        .iter()
        .filter(|r| r.verdict != "PASS")
        .map(|r| {
            format!(
                "{}: {}",
                r.part,
                check_lines(&r.checks, "INCONCLUSIVE").join("; ")
            )
        })
        .collect();
    if !open.is_empty() {
        return (
            R::Inconclusive,
            format!(
                "The AMOEBA digest could not confirm every check of the DXF file(s): {}. The file is held until each check passes.",
                open.join("; ")
            ),
        );
    }
    (
        R::Pass,
        format!(
            "The AMOEBA digest passes every check of all {} DXF file(s) (engine {}).",
            rows.len(),
            rows[0].engine
        ),
    )
}

/// One line per check with the given verdict: its name, measured value, tolerance (mm) and own detail.
fn check_lines(checks: &Value, verdict: &str) -> Vec<String> {
    let num = |v: &Value| {
        v.as_f64()
            .map_or_else(|| "n/a".to_string(), |x| x.to_string())
    };
    checks
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["verdict"] == verdict)
                .map(|c| {
                    format!(
                        "{} ({verdict}: measured {}, tolerance {} mm): {}",
                        c["name"].as_str().unwrap_or("?"),
                        num(&c["measured"]),
                        num(&c["tolerance"]),
                        c["detail"].as_str().unwrap_or("")
                    )
                })
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
                .map(|a| {
                    a.iter()
                        .filter(|c| c["verdict"] != "PASS")
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            let code = if r.verdict == "FAIL" {
                "amoeba_digest_failed"
            } else {
                "amoeba_digest_inconclusive"
            };
            Err((code.into(), Value::Array(names)))
        }
    }
}

/// The stage view of a job: surround, engulf, digest per check, confirm or hold, export. Plain JSON, for polling.
/// `holds` are the export holds of the export rule and of the owner rule (dxf_withheld); any hold withholds the export.
pub fn stages(
    id: i64,
    state: &str,
    gates: &[nf_core::Gate],
    rows: &[Row],
    holds: &[String],
) -> Value {
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
    let failing: Vec<String> = rows
        .iter()
        .flat_map(|r| {
            ["FAIL", "INCONCLUSIVE"]
                .into_iter()
                .flat_map(|v| check_lines(&r.checks, v))
                .map(|l| format!("{}: {l}", r.part))
                .collect::<Vec<String>>()
        })
        .collect();
    let mut hold: Vec<String> = gates
        .iter()
        .filter(|g| {
            !matches!(
                g.result,
                nf_core::GateResult::Pass | nf_core::GateResult::NotApplicable
            )
        })
        .map(|g| format!("{} {:?}", g.id, g.result))
        .collect();
    if state != "REVIEW_REQUIRED" {
        hold.push(format!("state {state}"));
    }
    let mut export_reasons = Vec::new();
    // a per-part hold already names the failing digest (`amoeba_digest_*`): the status line would repeat it
    let digest_named = holds.iter().any(|h| h.contains("amoeba_digest_"));
    if !all_pass && !digest_named {
        export_reasons.push(format!("digest {digest_status}"));
    }
    if state != "REVIEW_REQUIRED" {
        export_reasons.push(format!("state {state}"));
    }
    export_reasons.extend(holds.iter().map(|h| format!("held: {h}")));
    let export = if export_reasons.is_empty() {
        "AVAILABLE"
    } else {
        "WITHHELD"
    };
    json!({
        "calculation": id,
        "state": state,
        "stages": [
            { "stage": "surround", "status": "DONE", "detail": format!("job computed; {} DXF file(s) produced", rows.len()) },
            { "stage": "engulf", "status": if rows.is_empty() { "PENDING" } else { "DONE" }, "snapshots": rows.iter().map(|r| json!({
                "part": r.part, "dxf_sha256": r.dxf_sha256, "inputs_sha256": r.inputs_sha256, "engine": r.engine, "created_at": r.created_at
            })).collect::<Vec<Value>>() },
            { "stage": "digest", "status": digest_status, "failing": failing, "parts": rows.iter().map(|r| json!({
                "part": r.part, "verdict": r.verdict, "checks": r.checks
            })).collect::<Vec<Value>>() },
            { "stage": "confirm", "status": if hold.is_empty() { "CLEAR" } else { "HOLD" }, "reasons": hold },
            { "stage": "export", "status": export, "reasons": export_reasons },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nf_core::order_compute::{OrderRules, PartIn, compute_order};
    use std::collections::BTreeMap;

    fn curve_view(kind: &str, name: &str, dev: f64) -> CurveView {
        CurveView {
            name: name.into(),
            layer: "CUT_OUTER".into(),
            kind: kind.into(),
            points: vec![[0.0, 0.0], [1.0, 0.0]],
            points_exact: kind == "line",
            points_max_deviation: dev,
            length: 1.0,
        }
    }

    fn pattern_view(assumptions: Vec<String>) -> PatternView {
        PatternView {
            part: "P1".into(),
            family: "branch_stub".into(),
            class: "A".into(),
            marking_side: "outside".into(),
            eps_num: 1e-9,
            eps_rep: 0.01,
            eps_num_governs: false,
            quantities: BTreeMap::new(),
            assumptions,
            curves: vec![
                curve_view("polyline", "saddle_cut", 0.01),
                curve_view("line", "seam_a", 0.0),
            ],
            holes: vec![curve_view("arc", "opening", 0.002)],
            dxf_sha256: "sha".into(),
        }
    }

    #[test]
    fn the_reference_carries_the_declared_representation_bound() {
        let p = pattern_view(vec![
            "eps_rep is a certified upper bound of the chord error of the drawn polyline".into(),
        ]);
        let r = reference("P1", Some(&p));
        assert_eq!(r.eps_rep, Some(0.01));
        assert_eq!(r.eps_rep_certified, Some(true));
        assert_eq!(r.outer[0].eps_rep, Some(0.01));
        assert_eq!(r.outer[0].eps_rep_certified, Some(true));
        assert_eq!(r.outer[1].eps_rep, None);
        assert_eq!(r.outer[1].eps_rep_certified, None);
        // an arc's points are display chords, not a drawn polyline in the DXF
        assert_eq!(r.holes[0].eps_rep, None);
        assert_eq!(r.holes[0].eps_rep_certified, None);
    }

    #[test]
    fn a_pattern_without_a_certification_statement_is_uncertified() {
        let p = pattern_view(vec!["no bound statement".into()]);
        let r = reference("P1", Some(&p));
        assert_eq!(r.eps_rep, Some(0.01));
        assert_eq!(r.eps_rep_certified, Some(false));
        assert_eq!(r.outer[0].eps_rep_certified, Some(false));
    }

    #[test]
    fn a_pattern_that_says_not_certified_is_uncertified() {
        let p = pattern_view(vec!["eps_rep is NOT certified for this pattern".into()]);
        let r = reference("P1", Some(&p));
        assert_eq!(r.eps_rep_certified, Some(false));
        assert_eq!(r.outer[0].eps_rep_certified, Some(false));
    }

    #[test]
    fn a_certified_tee_dxf_digests_pass_and_a_tampered_vertex_is_not_served() {
        let tee = || {
            PartIn::new("T1", "tee_main")
                .v("D", 1016.0)
                .v("t", 12.0)
                .v("L", 2000.0)
                .v("branch_D", 508.0)
                .v("branch_t", 9.0)
                .v("branch_x", 1000.0)
                .v("dir", 0.0)
        };
        let mut res = compute_order(&[tee()], &OrderRules::default());
        let text = res.parts[0]
            .dxf
            .clone()
            .expect("certified unheld tee has a DXF");
        let pc = res.parts[0].pattern.first().expect("pattern").clone();
        let report = nf_digest::check_text(&text, &reference_for_piece(&res.parts[0], &pc));
        assert_eq!(
            report.verdict(),
            nf_digest::Verdict::Pass,
            "{:?}",
            report.checks
        );

        // one outline vertex moved by 0.1 mm: the digest must fail and the file must not be served
        let (mut ents, _) = nf_dxf::read(&text).expect("read back");
        let mut moved = false;
        for e in ents.iter_mut() {
            if let nf_dxf::Entity::Polyline { layer, pts, .. } = e
                && layer == "CUT_OUTER"
                && !moved
            {
                pts[1].0 += 0.1;
                moved = true;
            }
        }
        assert!(moved);
        let tampered = nf_dxf::write_stated(&ents, &[], &[], &[]);
        let bad = nf_digest::check_text(&tampered, &reference_for_piece(&res.parts[0], &pc));
        assert_eq!(bad.verdict(), nf_digest::Verdict::Fail, "{:?}", bad.checks);
        res.parts[0].dxf = Some(tampered);
        gate_order_dxf(&mut res);
        assert!(res.parts[0].dxf.is_none() && !res.parts[0].dxf_allowed);
        assert!(
            res.parts[0]
                .dxf_blocked_by
                .iter()
                .any(|c| c == "digest_failed")
        );
    }

    #[test]
    fn an_order_piece_reference_carries_the_engine_certification() {
        let parts = compute_order(
            &[
                PartIn::new("T1", "tee_main")
                    .v("D", 1016.0)
                    .v("t", 12.0)
                    .v("L", 2000.0)
                    .v("branch_D", 508.0)
                    .v("branch_t", 9.0)
                    .v("branch_x", 1000.0)
                    .v("dir", 0.0),
                PartIn::new("T1b", "tee_branch")
                    .v("D", 508.0)
                    .v("t", 9.0)
                    .v("H", 600.0)
                    .attached("T1"),
            ],
            &OrderRules::default(),
        )
        .parts;
        let p = &parts[0];
        let piece = p.pattern.first().expect("tee pattern");
        let r = reference_for_piece(p, piece);
        assert!(p.certified, "{}: {:?}", p.no, p.holds);
        assert_eq!(r.eps_rep, p.eps_rep);
        assert_eq!(r.eps_rep_certified, Some(true));
        assert!(!r.outer.is_empty());
        assert!(r.outer.iter().all(|c| c.eps_rep_certified == Some(true)));
    }

    #[test]
    fn a_withheld_order_piece_reference_is_uncertified() {
        let parts = compute_order(
            &[PartIn::new("B1", "miter_bend")
                .v("D", 1016.0)
                .v("t", 12.0)
                .v("R", 1500.0)
                .v("angle", 90.0)
                .v("joints", 2.0)
                .v("start_straight", 300.0)
                .v("end_straight", 300.0)],
            &OrderRules::default(),
        )
        .parts;
        let p = &parts[0];
        let piece = p.pattern.first().expect("bend pattern");
        let r = reference_for_piece(p, piece);
        assert!(!p.certified);
        assert_eq!(r.eps_rep_certified, Some(false));
    }

    #[test]
    fn a_failed_digest_is_named_once_in_the_export_reasons() {
        let rows = vec![Row {
            part: "P1".into(),
            dxf_sha256: "a".into(),
            inputs_sha256: "b".into(),
            engine: "e".into(),
            verdict: "FAIL".into(),
            checks: json!([]),
            created_at: 0,
        }];
        let holds = vec!["part P1: amoeba_digest_failed".to_string()];
        let view = stages(1, "REVIEW_REQUIRED", &[], &rows, &holds);
        let export = view["stages"]
            .as_array()
            .and_then(|s| s.iter().find(|s| s["stage"] == "export"))
            .expect("export stage");
        assert_eq!(
            export["reasons"],
            json!(["held: part P1: amoeba_digest_failed"])
        );
    }
}
