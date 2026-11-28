//! POST /api/orders/{oid}/nest. Nests the certified, unheld part outlines of the current computed result on the
//! stock sheet and returns each sheet checked by nf-nest's independent checker, with its NOT RELEASED sheet DXF.
//! Stock size, kerf, part gap, edge margin and rotations are named rules, UNVERIFIED until a factory confirms
//! them. Nothing here approves or releases: the output is a plan for a person to check.
//! Only a person may nest (admin or engineer; the AI operator is refused with 403 and the refusal is audited), the
//! order must be computed and carry a "checked" mark, and the request is audited.

use super::*;
use nf_nest::check::check;
use nf_nest::dxf_out::{DxfFormat, sheet_dxf};
use nf_nest::outline::outline_from_dxf;
use nf_nest::{PartIn, Rules, nest, utilisation};

/// Sheets per response: a plan with more sheets is refused, not cut short.
const MAX_NEST_SHEETS: usize = 50;

pub(super) async fn nest_order(
    State(app): Ctx,
    headers: HeaderMap,
    Path(oid): Path<i64>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "nest order")?;
    if u.kind == "agent" {
        record_alone(
            &app,
            &u.username,
            "nest_refused",
            &format!("order {oid}: the AI operator may not nest (403)"),
        )?;
        return Err(fail_code(
            StatusCode::FORBIDDEN,
            "agent_cannot_nest",
            &[],
            "the AI operator cannot nest; a person does",
        ));
    }
    let res = nest_do(&app, &headers, &u, oid, &raw);
    settle(&app, &u, oid, "nest", String::new(), res).await
}

fn rule_num(body: &Value, key: &str, default: f64) -> Result<f64, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite())
            .ok_or_else(|| unprocessable("nest_rules", &format!("'{key}' must be a number"))),
    }
}

fn nest_do(app: &App, headers: &HeaderMap, u: &User, oid: i64, raw: &[u8]) -> Api {
    let body: Value = if raw.is_empty() {
        json!({})
    } else {
        request_json(headers, raw)?
    };
    let d = Rules::unverified_default();
    let rotations: Vec<u32> = match body.get("rotations") {
        None | Some(Value::Null) => d.rotations.clone(),
        Some(v) => v
            .as_array()
            .ok_or_else(|| unprocessable("nest_rules", "'rotations' must be a list of degrees"))?
            .iter()
            .map(|x| {
                x.as_u64()
                    .map(|n| n as u32)
                    .ok_or_else(|| unprocessable("nest_rules", "rotation steps are whole degrees"))
            })
            .collect::<Result<_, _>>()?,
    };
    let rules = Rules {
        sheet_w: rule_num(&body, "sheet_w", d.sheet_w)?,
        sheet_h: rule_num(&body, "sheet_h", d.sheet_h)?,
        kerf_mm: rule_num(&body, "kerf_mm", d.kerf_mm)?,
        part_gap_mm: rule_num(&body, "part_gap_mm", d.part_gap_mm)?,
        edge_mm: rule_num(&body, "edge_mm", d.edge_mm)?,
        rotations,
    };
    rules
        .validate()
        .map_err(|e| unprocessable(e.code, &e.msg))?;

    let db = app.db();
    let result_json: Option<String> = db
        .query_row(
            "SELECT result_json FROM order_results WHERE order_id = ? ORDER BY id DESC LIMIT 1",
            params![oid],
            |r| r.get(0),
        )
        .optional()?;
    let Some(result_json) = result_json else {
        return Err(unprocessable(
            "order_not_computed",
            "compute the order before nesting",
        ));
    };
    let result: Value = serde_json::from_str(&result_json).unwrap_or(Value::Null);
    {
        let files: Vec<&str> = result["parts"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter_map(|p| p["dxf"].as_str())
            .collect();
        super::refuse_edited_order(&db, oid, &files)?;
    }
    let checked = decision_view(&db, oid, None)?
        .iter()
        .rev()
        .any(|d| d["kind"] == "mark_checked" && d["effective"] == true);
    if !checked {
        return Err(unprocessable(
            "order_not_checked",
            "a person must mark the order as checked before nesting",
        ));
    }

    // certified and unheld: the DXF is allowed by the gate, has no hold, and is one closed outline
    let mut inputs: Vec<PartIn> = Vec::new();
    let mut excluded: Vec<Value> = Vec::new();
    for p in result["parts"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let id = format!("{}", p["no"].as_str().unwrap_or("?"));
        // a quantity wider than 32 bits is clamped, never wrapped: the copies cap refuses it
        let qty = u32::try_from(p["qty"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
        let blocked = p["dxf_blocked_by"]
            .as_array()
            .is_some_and(|a| !a.is_empty());
        let reason = if p["state"] != "computed" {
            Some("not computed or held")
        } else if p["dxf_allowed"] != true || blocked {
            Some("DXF held by the gate")
        } else if p["dxf"].as_str().is_none() {
            Some("no certified DXF outline on this result")
        } else if qty == 0 {
            Some("no quantity")
        } else {
            None
        };
        if let Some(reason) = reason {
            excluded.push(json!({ "part": id, "reason": reason }));
            continue;
        }
        match outline_from_dxf(p["dxf"].as_str().unwrap_or_default()) {
            Ok(outline) => inputs.push(PartIn { id, outline, qty }),
            Err(e) => excluded.push(json!({ "part": id, "reason": e })),
        }
    }
    if inputs.is_empty() {
        return ok(
            json!({ "status": "no_parts", "rules": rules_json(&rules), "sheets": [], "excluded": excluded }),
        );
    }
    let planned = nest(&inputs, &rules).map_err(|e| unprocessable(e.code, &e.msg))?;
    if planned.sheets.len() > MAX_NEST_SHEETS {
        return Err(unprocessable(
            "nest_limit",
            &format!(
                "the plan needs {} sheets; at most {MAX_NEST_SHEETS} per request",
                planned.sheets.len()
            ),
        ));
    }
    let checked_nest = check(&inputs, &rules, planned).map_err(|errs| {
        // a failed check is never written out as a sheet
        unprocessable("nest_check_failed", &errs.join("; "))
    })?;
    let (per, total) = utilisation(checked_nest.nest());
    let mut sheets = Vec::new();
    for (i, s) in checked_nest.nest().sheets.iter().enumerate() {
        let dxf = sheet_dxf(&checked_nest, i, DxfFormat::R2000Mtext)
            .ok_or_else(|| unprocessable("nest_internal", "a sheet is missing"))?;
        // R2000 output is ASCII (Unicode as \U+ escapes), so it is valid UTF-8 for the JSON response
        let dxf = String::from_utf8_lossy(&dxf).into_owned();
        let placed: Vec<Value> = s
            .placed
            .iter()
            .map(|q| json!({ "part": q.id, "copy": q.copy + 1, "rot": q.rot, "pose": [q.pose.0, q.pose.1] }))
            .collect();
        sheets.push(
            json!({ "index": i + 1, "utilisation_pct": per[i], "placed": placed, "dxf": dxf }),
        );
    }
    drop(db);
    record_alone(
        app,
        &u.username,
        "nest",
        &format!(
            "order {oid}: {} sheet(s), {} part kind(s), utilisation {total:.1} % (rules UNVERIFIED)",
            sheets.len(),
            inputs.len()
        ),
    )?;
    ok(json!({
        "status": "checked",
        "engine": checked_nest.nest().engine,
        "not_released": true,
        "rules": rules_json(&rules),
        "utilisation_pct": total,
        "sheets": sheets,
        "excluded": excluded,
    }))
}

fn rules_json(r: &Rules) -> Value {
    json!({
        "sheet_w": r.sheet_w, "sheet_h": r.sheet_h, "kerf_mm": r.kerf_mm, "part_gap_mm": r.part_gap_mm,
        "edge_mm": r.edge_mm, "rotations": r.rotations,
        "status": "UNVERIFIED",
        "rule_ids": [nf_nest::RULE_STOCK, nf_nest::RULE_KERF, nf_nest::RULE_GAP, nf_nest::RULE_EDGE, nf_nest::RULE_ROT],
    })
}
