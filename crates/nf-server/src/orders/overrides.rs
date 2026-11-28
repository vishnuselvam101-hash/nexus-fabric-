//! 6B: a person's hold override, built narrowly (founder pick 6B, DECISIONS.md "6B override allow-list").
//!
//! A person may let ONE part's DXF through past ONE review hold of the allow-list, with a typed reason. NF itself
//! never approves or releases anything: the override is a person's, recorded with name, time and reason, audited and
//! undoable, and the file it lets through carries `OVERRIDE_MARK` (a visible TEXT entity and a DXF comment). Refusals,
//! uncertified geometry, questions, never-wave kinds, mass and math holds and a failed digest stay absolute because the
//! gate subtracts only allow-list codes from what blocks the part and requires everything else to be clear.

use super::*;

/// The only hold codes a person may override.
pub(crate) const ALLOW_LIST: [&str; 4] = [
    "out_of_band",
    "source_inferred",
    "bend_center_vs_drawing",
    "tee_mismatch",
];
pub(crate) const MIN_REASON_CHARS: usize = 20;
pub(crate) const OVERRIDE_MARK: &str = "PERSON OVERRIDE — NOT VERIFIED BY NF";

/// A live override: not undone and made against the order as it stands now.
#[derive(Clone, Debug)]
pub(crate) struct ActiveOverride {
    pub id: i64,
    pub part: String,
    pub code: String,
    pub reason: String,
    pub by: String,
    pub at: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    Allowed,
    Blocked(Vec<String>),
}

/// What the gate reads of a part.
pub(crate) struct GateInput<'a> {
    pub blocked_by: &'a [String],
    pub certified: bool,
    pub refused: bool,
    pub has_dxf: bool,
}

pub(crate) fn gate_of(p: &oc::PartResult) -> GateInput<'_> {
    GateInput {
        blocked_by: &p.dxf_blocked_by,
        certified: p.certified,
        refused: p.refusal.is_some(),
        has_dxf: p.dxf.is_some(),
    }
}

/// Pure gate. `digest` is the independent digest verdict of this part's DXF (None while no digest exists: blocked).
pub(crate) fn override_gate(p: &GateInput<'_>, codes: &[String], digest: Option<&str>) -> Gate {
    let mut remaining: Vec<String> = p
        .blocked_by
        .iter()
        .filter(|c| !(ALLOW_LIST.contains(&c.as_str()) && codes.contains(c)))
        .cloned()
        .collect();
    if !p.certified {
        remaining.push("not_certified".into());
    }
    if p.refused {
        remaining.push("refused".into());
    }
    if !p.has_dxf {
        remaining.push("no_dxf".into());
    }
    if digest != Some("PASS") {
        remaining.push("digest_not_pass".into());
    }
    remaining.sort();
    remaining.dedup();
    if remaining.is_empty() {
        Gate::Allowed
    } else {
        Gate::Blocked(remaining)
    }
}

/// Stamp a DXF text: a 999 comment at the top and a visible TEXT entity (layer NF_OVERRIDE) at the end of ENTITIES.
pub(crate) fn mark_dxf(dxf: &str, notes: &[String]) -> String {
    let mut lines: Vec<&str> = dxf.lines().collect();
    let mut at = None;
    let mut in_entities = false;
    let mut i = 0;
    while i + 1 < lines.len() {
        let (c, v) = (lines[i].trim(), lines[i + 1].trim());
        if c == "2" && v == "ENTITIES" {
            in_entities = true;
        } else if in_entities && c == "0" && v == "ENDSEC" {
            at = Some(i);
            break;
        }
        i += 2;
    }
    let mut out = String::new();
    for l in std::iter::once(OVERRIDE_MARK).chain(notes.iter().map(String::as_str)) {
        out.push_str(&format!("999\n{}\n", l.replace(['\r', '\n'], " ")));
    }
    let text = format!(
        "0\nTEXT\n8\nNF_OVERRIDE\n10\n0.0\n20\n0.0\n30\n0.0\n40\n5.0\n1\n{OVERRIDE_MARK}\n"
    );
    for (n, l) in lines.drain(..).enumerate() {
        if Some(n) == at {
            out.push_str(&text);
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

fn person_only(u: &User) -> Result<(), ApiError> {
    if u.kind == "agent" {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "override_person_only",
            "the AI operator cannot override a hold; only a person can",
        ));
    }
    Ok(())
}

pub(super) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route(
            "/api/orders/{oid}/overrides",
            get(list_overrides).post(post_override),
        )
        .route("/api/orders/{oid}/overrides/{id}/undo", post(undo_override))
}

/// The overrides that are live for this compute input hash.
pub(super) fn active_overrides(
    db: &Connection,
    oid: i64,
    hash: &str,
) -> Result<Vec<ActiveOverride>, ApiError> {
    Ok(q_all(
        db,
        "SELECT id, part_no, code, reason, by_user, at FROM order_hold_overrides WHERE order_id = ? AND undone_at IS NULL AND against_hash = ? ORDER BY id",
        params![oid, hash],
        |r| {
            Ok(ActiveOverride {
                id: r.get(0)?,
                part: r.get(1)?,
                code: r.get(2)?,
                reason: r.get(3)?,
                by: r.get(4)?,
                at: r.get(5)?,
            })
        },
    )?)
}

async fn list_overrides(State(app): Ctx, headers: HeaderMap, Path(oid): Path<i64>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    guard_order(&app, &db, &u, oid, "list overrides")?;
    let (_, _, hash) = load_compute_input(&db, oid)?;
    let rows = q_all(
        &db,
        "SELECT id, part_no, code, reason, against_hash, by_user, at, undone_by, undone_at, undo_note FROM order_hold_overrides WHERE order_id = ? ORDER BY id",
        params![oid],
        |r| {
            let (h, undone): (String, Option<i64>) = (r.get(4)?, r.get(8)?);
            Ok(
                json!({ "id": r.get::<_, i64>(0)?, "part": r.get::<_, String>(1)?, "code": r.get::<_, String>(2)?,
                "reason": r.get::<_, String>(3)?, "by": r.get::<_, String>(5)?, "at": r.get::<_, i64>(6)?,
                "undone_by": r.get::<_, Option<String>>(7)?, "undone_at": undone, "undo_note": r.get::<_, Option<String>>(9)?,
                "active": undone.is_none() && h == hash, "stale": undone.is_none() && h != hash,
                "mark": OVERRIDE_MARK }),
            )
        },
    )?;
    ok(
        json!({ "allow_list": ALLOW_LIST, "min_reason_chars": MIN_REASON_CHARS, "mark": OVERRIDE_MARK, "overrides": rows }),
    )
}

async fn post_override(
    State(app): Ctx,
    headers: HeaderMap,
    Path(oid): Path<i64>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "override a hold")?;
    let res = post_override_do(&app, &headers, &u, oid, &raw).await;
    settle(&app, &u, oid, "override", body_text(&raw, "reason"), res).await
}

async fn post_override_do(
    app: &Arc<App>,
    headers: &HeaderMap,
    u: &User,
    oid: i64,
    raw: &[u8],
) -> Api {
    person_only(u)?;
    let body = request_json(headers, raw)?;
    let reason = text_field(&body, "reason", MAX_NOTE_CHARS)?.unwrap_or_default();
    let reason = reason.trim().to_string();
    if reason.chars().count() < MIN_REASON_CHARS {
        return Err(unprocessable(
            "override_reason",
            &format!("an override needs a typed reason of at least {MIN_REASON_CHARS} characters"),
        ));
    }
    let code = text_field(&body, "code", 64)?.unwrap_or_default();
    if !ALLOW_LIST.contains(&code.as_str()) {
        return Err(refuse_fields(
            StatusCode::CONFLICT,
            "override_not_allowed",
            &[&code],
            &format!(
                "{code} cannot be overridden; only {} can",
                ALLOW_LIST.join(", ")
            ),
        ));
    }
    let part = text_field(&body, "part", 40)?.unwrap_or_default();
    let (parts, rules, hash, ctx) = {
        let db = app.db();
        let (p, r, h) = load_compute_input(&db, oid)?;
        (p, r, h, load_review_context(&db, oid)?)
    };
    let res =
        tokio::task::spawn_blocking(move || rv::compute_order_with_context(&parts, &rules, &ctx))
            .await
            .map_err(|_| {
                fail(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the compute stopped unexpectedly",
                )
            })?;
    let Some(p) = res.parts.iter().find(|p| p.no == part) else {
        return Err(unprocessable("override_part", "the order has no such part"));
    };
    if !p.holds.iter().any(|h| h.level == "need" && h.code == code) {
        return Err(refuse(
            StatusCode::CONFLICT,
            "override_no_hold",
            &format!("part '{part}' has no open {code} hold to override"),
        ));
    }
    let mut db = app.db();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    still_writer(&tx, u)?;
    guard_order_tx(app, &tx, u, oid, "override a hold")?;
    let (_, _, hash2) = load_compute_input(&tx, oid)?;
    if hash2 != hash {
        return Err(refuse(
            StatusCode::CONFLICT,
            "order_changed",
            "the order changed while the override was checked; try again",
        ));
    }
    let dup: i64 = tx.query_row(
        "SELECT COUNT(*) FROM order_hold_overrides WHERE order_id = ? AND part_no = ? AND code = ? AND undone_at IS NULL AND against_hash = ?",
        params![oid, part, code, hash],
        |r| r.get(0),
    )?;
    if dup > 0 {
        return Err(refuse(
            StatusCode::CONFLICT,
            "override_exists",
            "this hold is overridden already; undo it first",
        ));
    }
    tx.execute(
        "INSERT INTO order_hold_overrides(order_id, part_no, code, reason, against_hash, by_user, at) VALUES (?,?,?,?,?,?,?)",
        params![oid, part, code, reason, hash, u.username, now() as i64],
    )?;
    let id = tx.last_insert_rowid();
    app.audit(
        &tx,
        &u.username,
        "order_override",
        &format!(
            "order #{oid} override #{id} of {code} on part '{part}' by {}: {}",
            u.username,
            clip(&reason, 300)
        ),
    )?;
    tx.commit()?;
    ok(
        json!({ "id": id, "part": part, "code": code, "by": u.username, "reason": reason, "mark": OVERRIDE_MARK }),
    )
}

async fn undo_override(
    State(app): Ctx,
    headers: HeaderMap,
    Path((oid, id)): Path<(i64, i64)>,
    raw: Bytes,
) -> Api {
    let u = write_access(&app, &headers, oid, "undo override")?;
    let res = (|| -> Api {
        person_only(&u)?;
        let body = if raw.is_empty() {
            json!({})
        } else {
            request_json(&headers, &raw)?
        };
        let note = text_field(&body, "note", MAX_NOTE_CHARS)?.unwrap_or_default();
        let mut db = app.db();
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        still_writer(&tx, &u)?;
        guard_order_tx(&app, &tx, &u, oid, "undo override")?;
        let row: Option<(String, String, bool)> = tx
            .query_row(
                "SELECT part_no, code, undone_at IS NOT NULL FROM order_hold_overrides WHERE id = ? AND order_id = ?",
                params![id, oid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((part, code, undone)) = row else {
            return Err(not_found());
        };
        if undone {
            return Err(refuse(
                StatusCode::CONFLICT,
                "already_undone",
                "this override was undone already",
            ));
        }
        tx.execute(
            "UPDATE order_hold_overrides SET undone_by = ?, undone_at = ?, undo_note = ? WHERE id = ?",
            params![u.username, now() as i64, clip(&note, 300), id],
        )?;
        app.audit(
            &tx,
            &u.username,
            "order_override_undo",
            &format!(
                "order #{oid} override #{id} of {code} on part '{part}' undone by {} {}",
                u.username,
                clip(&note, 200)
            ),
        )?;
        tx.commit()?;
        ok(json!({ "undone": id }))
    })();
    settle(&app, &u, oid, "override undo", body_text(&raw, "note"), res).await
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn allow_list_has_no_never_wave_code() {
        for c in ALLOW_LIST {
            assert!(!rv::is_never_wave(c), "{c}");
        }
    }

    fn input(blocked: &[&str], certified: bool) -> (Vec<String>, bool) {
        (blocked.iter().map(|s| s.to_string()).collect(), certified)
    }

    fn gate(blocked: &[&str], certified: bool, over: &[&str], digest: Option<&str>) -> Gate {
        let (b, c) = input(blocked, certified);
        let o: Vec<String> = over.iter().map(|s| s.to_string()).collect();
        override_gate(
            &GateInput {
                blocked_by: &b,
                certified: c,
                refused: false,
                has_dxf: true,
            },
            &o,
            digest,
        )
    }

    #[test]
    fn gate_lets_only_an_overridden_allow_list_hold_through() {
        assert_eq!(
            gate(
                &["source_inferred"],
                true,
                &["source_inferred"],
                Some("PASS")
            ),
            Gate::Allowed
        );
        assert_eq!(
            gate(
                &["source_inferred", "out_of_band"],
                true,
                &["source_inferred", "out_of_band"],
                Some("PASS")
            ),
            Gate::Allowed
        );
        // one hold not overridden, a digest that is not PASS or missing, uncertified, never-wave or non-list codes: blocked
        assert!(matches!(
            gate(
                &["source_inferred", "out_of_band"],
                true,
                &["source_inferred"],
                Some("PASS")
            ),
            Gate::Blocked(_)
        ));
        assert!(matches!(
            gate(
                &["source_inferred"],
                true,
                &["source_inferred"],
                Some("FAIL")
            ),
            Gate::Blocked(_)
        ));
        assert!(matches!(
            gate(&["source_inferred"], true, &["source_inferred"], None),
            Gate::Blocked(_)
        ));
        assert!(matches!(
            gate(
                &["source_inferred", "not_certified"],
                false,
                &["source_inferred"],
                Some("PASS")
            ),
            Gate::Blocked(_)
        ));
        assert!(matches!(
            gate(&["missing"], true, &["missing"], Some("PASS")),
            Gate::Blocked(_)
        ));
        assert!(matches!(
            gate(&["mass_total"], true, &["mass_total"], Some("PASS")),
            Gate::Blocked(_)
        ));
        let refused = GateInput {
            blocked_by: &["refused".to_string()],
            certified: true,
            refused: true,
            has_dxf: true,
        };
        assert!(matches!(
            override_gate(&refused, &["refused".into()], Some("PASS")),
            Gate::Blocked(_)
        ));
    }

    const DXF: &str = "0\nSECTION\n2\nHEADER\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nCUT\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn marking_is_a_comment_and_a_visible_text_inside_entities() {
        let m = mark_dxf(DXF, &["override #1 by Vishnu".to_string()]);
        assert!(m.starts_with(
            "999\nPERSON OVERRIDE — NOT VERIFIED BY NF\n999\noverride #1 by Vishnu\n"
        ));
        let ents = m.split("2\nENTITIES\n").nth(1).unwrap();
        let text_at = ents.find("0\nTEXT\n8\nNF_OVERRIDE\n").unwrap();
        let end_at = ents.find("0\nENDSEC\n").unwrap();
        assert!(ents.find("0\nLINE\n").unwrap() < text_at && text_at < end_at);
        assert!(ents[text_at..end_at].contains("1\nPERSON OVERRIDE — NOT VERIFIED BY NF\n"));
        assert!(m.ends_with("0\nEOF\n"));
    }
}
