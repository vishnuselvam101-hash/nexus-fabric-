//! Machine profiles and the hand-off export (nf-machine). Profiles are immutable and versioned.
//! Export is person-only, licence-gated and writes only into the configured export folders.
//! Certificates come from the server's own AMOEBA digest rows and stored result, never from the caller.
use super::*;
use nf_machine::audit::AuditLog;
use nf_machine::gate::Refusal;
use nf_machine::{
    Actor, ActorKind, CertVerdict, Certificate, ExportError, ExportRequest, MachineProfile,
    PartSource,
};

pub(crate) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route(
            "/api/machine/profiles",
            get(list_profiles).post(create_profile),
        )
        .route("/api/machine/profiles/validate", post(validate_profile))
        .route("/api/machine/folders", get(folders))
        .route("/api/calculations/{id}/export", post(export_calc))
        .route(
            "/machine",
            get(|| async {
                asset(
                    "text/html; charset=utf-8",
                    include_str!("../../../web/machine/index.html"),
                )
            }),
        )
        .route(
            "/machine/app.mjs",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../../../web/machine/app.mjs"),
                )
            }),
        )
}

fn writer(u: &User) -> Result<(), ApiError> {
    if u.kind != "person" {
        return Err(fail(
            StatusCode::FORBIDDEN,
            "an agent may propose an export but cannot write this",
        ));
    }
    require(u, &["admin", "engineer"])
}

async fn validate_profile(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Api {
    let db = app.db();
    let _u = auth_db(&db, &app, &headers)?;
    match MachineProfile::from_json(&body.to_string()) {
        Ok(p) => ok(json!({ "ok": true, "id": p.id, "version": p.version })),
        Err(e) => Err(fail_json(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": e.to_string() }),
        )),
    }
}

async fn list_profiles(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let _u = auth_db(&db, &app, &headers)?;
    let mut st = db.prepare("SELECT id, version, json, created_by, created_at FROM machine_profiles ORDER BY id, version")?;
    let rows = st.query_map([], |r| {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "version": r.get::<_, i64>(1)?,
            "profile": serde_json::from_str::<Value>(&r.get::<_, String>(2)?).unwrap_or(Value::Null),
            "created_by": r.get::<_, String>(3)?,
            "created_at": r.get::<_, i64>(4)?,
        }))
    })?;
    ok(json!(rows.collect::<rusqlite::Result<Vec<Value>>>()?))
}

async fn create_profile(State(app): Ctx, headers: HeaderMap, Json(body): Json<Value>) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    writer(&u)?;
    let profile = MachineProfile::from_json(&body.to_string())
        .map_err(|e| fail(StatusCode::BAD_REQUEST, &e.to_string()))?;
    let version: i64 = db.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM machine_profiles WHERE id = ?",
        params![profile.id],
        |r| r.get(0),
    )?;
    let mut stored = profile.clone();
    stored.version = version as u32;
    let text = stored.to_json();
    db.execute(
        "INSERT INTO machine_profiles(id, version, json, created_by, created_at) VALUES (?,?,?,?,?)",
        params![profile.id, version, text, u.username, now() as i64],
    )?;
    app.audit(
        &db,
        &u.username,
        "machine_profile_created",
        &format!("{} v{version}", profile.id),
    )?;
    ok(json!({ "id": profile.id, "version": version }))
}

async fn folders(State(app): Ctx, headers: HeaderMap) -> Api {
    let db = app.db();
    let _u = auth_db(&db, &app, &headers)?;
    let list: Vec<Value> = app
        .export_dirs
        .iter()
        .enumerate()
        .map(|(i, p)| json!({ "index": i, "path": p.display().to_string() }))
        .collect();
    ok(json!(list))
}

fn verdict_of(v: &str) -> CertVerdict {
    match v {
        "PASS" => CertVerdict::Pass,
        "FAIL" => CertVerdict::Fail,
        _ => CertVerdict::Inconclusive,
    }
}

/// True when any server-side hold applies to the part: the same holds that withhold its DXF download.
fn export_held(c: &Core, e: &Eval, rows: &[amoeba::Row], part: &str, sha: &str) -> bool {
    c.superseded_by.is_some()
        || e.refused()
        || e.state != REVIEW_REQUIRED
        || !e.integrity.is_empty()
        || !e.engine_current
        || c.records.as_ref().is_none_or(|r| !r.findings.is_empty())
        || c.records
            .as_ref()
            .is_none_or(|r| dxf_withheld(r).contains_key(part))
        || amoeba::hold_for(rows, part).is_err()
        || snapshot_mismatch(rows, sha, part)
        || export_hold(e).is_some()
}

/// Stable code for a refusal, for the panel's plain-words text.
fn export_code(e: &ExportError) -> &'static str {
    match e {
        ExportError::PersonOnly => "person_only",
        ExportError::LicenceRequired => "licence_required",
        ExportError::Profile(_) => "profile_invalid",
        ExportError::BadId(_) => "bad_id",
        ExportError::NotAFolder(_) => "folder_missing",
        ExportError::Refused { reason, .. } => match reason {
            Refusal::NoCertificate => "no_certificate",
            Refusal::NotCertified(_) => "not_certified",
            Refusal::Held => "held",
            Refusal::PartMismatch { .. } => "part_mismatch",
            Refusal::EngineStale { .. } => "engine_stale",
            Refusal::HashMismatch { .. } => "hash_mismatch",
        },
        ExportError::Convert { error, .. } => match error {
            nf_machine::dxf::DxfError::Malformed(_) => "malformed",
            nf_machine::dxf::DxfError::UnmappedLayer(_) => "unmapped_layer",
            nf_machine::dxf::DxfError::UnsupportedEntity(_) => "unsupported_entity",
            nf_machine::dxf::DxfError::UnitsMismatch(_) => "units_mismatch",
            nf_machine::dxf::DxfError::Unencodable(_) => "unencodable",
        },
        ExportError::Exists(_) => "exists",
        ExportError::Io(_) | ExportError::Audit(_) => "io",
    }
}

async fn export_calc(
    State(app): Ctx,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Api {
    let db = app.db();
    let u = auth_db(&db, &app, &headers)?;
    if u.kind != "person" {
        app.audit(
            &db,
            &u.username,
            "machine_export_refused",
            &format!("calculation #{id}: agent identity"),
        )?;
        return Err(fail(
            StatusCode::FORBIDDEN,
            "export is for a person; an agent may only propose it",
        ));
    }
    require(&u, &["admin", "engineer"])?;
    let (c, e) = calc_guard(&app, &db, &u, id, "machine export")?;
    let profile_id = body["profile"].as_str().unwrap_or_default().to_string();
    let version = body["version"].as_i64();
    let folder = body["folder"]
        .as_u64()
        .and_then(|i| app.export_dirs.get(i as usize).cloned())
        .ok_or_else(|| {
            fail(
                StatusCode::BAD_REQUEST,
                "choose an export folder from the list",
            )
        })?;
    let stored = match version {
        Some(v) => db
            .query_row(
                "SELECT json FROM machine_profiles WHERE id = ? AND version = ?",
                params![profile_id, v],
                |r| r.get::<_, String>(0),
            )
            .optional()?,
        None => db
            .query_row(
                "SELECT json FROM machine_profiles WHERE id = ? ORDER BY version DESC LIMIT 1",
                params![profile_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?,
    };
    let Some(text) = stored else {
        return Err(fail(StatusCode::NOT_FOUND, "no such machine profile"));
    };
    let profile = MachineProfile::from_json(&text)
        .map_err(|e| fail(StatusCode::BAD_REQUEST, &e.to_string()))?;

    let rows = amoeba::rows(&db, id)?;
    let Some(records) = c.records.as_ref() else {
        return Err(fail(
            StatusCode::CONFLICT,
            "nothing to export: this job has no verified DXF records",
        ));
    };
    let mut parts = Vec::new();
    for (part, sha) in &records.dxf {
        let held = export_held(&c, &e, &rows, part, sha);
        let cert = rows.iter().find(|r| &r.part == part).map(|r| Certificate {
            snapshot_id: format!("calc-{id}"),
            part_id: part.clone(),
            engine_id: r.engine.clone(),
            sha256: r.dxf_sha256.clone(),
            verdict: verdict_of(&r.verdict),
            held,
        });
        let (a, s) = (app.clone(), sha.clone());
        parts.push(PartSource {
            part_id: part.clone(),
            cert,
            load: Box::new(move || {
                storage::load_blob(&a.data_dir, &a.key, &s).map_err(|e| e.to_string())
            }),
        });
    }
    let licensed = {
        let lic = app.licence.lock().unwrap_or_else(|e| e.into_inner());
        licence::state_key(&licence::current(&lic, now()).state) != "read_only"
    };
    let mut log = AuditLog::open(&app.data_dir.join("machine-audit.jsonl"))
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    let actor = Actor {
        id: u.username.clone(),
        kind: ActorKind::Person,
    };
    let job = format!("calc-{id}");
    let req = ExportRequest {
        job_id: &job,
        profile: &profile,
        actor: &actor,
        licensed,
        engine_id: nf_core::ENGINE_VERSION,
        parts: &parts,
        out_dir: &folder,
        ts: now(),
    };
    match nf_machine::export::export(&req, &mut log) {
        Ok(rep) => {
            let files: Vec<Value> = rep.files.iter().map(|f| json!({ "part": f.part_id, "file": f.file, "source_sha256": f.source_sha256, "sha256": f.out_sha256 })).collect();
            app.audit(
                &db,
                &u.username,
                "machine_export",
                &format!(
                    "calculation #{id} profile {} v{} files {}",
                    profile.id,
                    profile.version,
                    files.len()
                ),
            )?;
            ok(
                json!({ "written": true, "job": job, "profile": profile.id, "version": profile.version, "files": files }),
            )
        }
        Err(err) => {
            let code = export_code(&err);
            app.audit(
                &db,
                &u.username,
                "machine_export_refused",
                &format!("calculation #{id} profile {}: {code}", profile.id),
            )?;
            let status = match err {
                ExportError::PersonOnly | ExportError::LicenceRequired => StatusCode::FORBIDDEN,
                ExportError::Refused { .. }
                | ExportError::Convert { .. }
                | ExportError::Exists(_) => StatusCode::CONFLICT,
                ExportError::Profile(_) | ExportError::BadId(_) | ExportError::NotAFolder(_) => {
                    StatusCode::BAD_REQUEST
                }
                ExportError::Io(_) | ExportError::Audit(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Err(fail_json(
                status,
                json!({ "written": false, "code": code, "error": err.to_string() }),
            ))
        }
    }
}
