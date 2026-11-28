mod common;
use common::*;
use nf_machine::audit::AuditLog;
use nf_machine::*;
use std::fs;

fn person() -> Actor {
    Actor {
        id: "vishnu".into(),
        kind: ActorKind::Person,
    }
}

fn mk_req<'a>(
    p: &'a MachineProfile,
    actor: &'a Actor,
    licensed: bool,
    parts: &'a [PartSource],
    out: &'a std::path::Path,
    job: &'a str,
) -> ExportRequest<'a> {
    ExportRequest {
        job_id: job,
        profile: p,
        actor,
        licensed,
        engine_id: ENGINE,
        parts,
        out_dir: out,
        overwrite: false,
        ts: 1000,
    }
}

#[test]
fn export_writes_verified_files_and_audits_refusals() {
    let root = dir("export-ok");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    let out = root.join("out");
    fs::create_dir_all(&out).unwrap();
    let audit_path = root.join("audit.jsonl");
    let mut audit = AuditLog::open(&audit_path).unwrap();

    let body = dxf("ABC", "CUT_OUTER").into_bytes();
    let f = src.join("P1.dxf");
    fs::write(&f, &body).unwrap();
    let parts = vec![PartSource::file("P1", f.clone(), Some(cert("P1", &body)))];
    let p = profile(TextEncoding::ShiftJis, DxfVersion::R12);

    // agent may not export; licence required
    let agent = Actor {
        id: "amoeba".into(),
        kind: ActorKind::Agent,
    };
    assert!(matches!(
        export::export(&mk_req(&p, &agent, true, &parts, &out, "J1"), &mut audit),
        Err(ExportError::PersonOnly)
    ));
    assert!(matches!(
        export::export(
            &mk_req(&p, &person(), false, &parts, &out, "J1"),
            &mut audit
        ),
        Err(ExportError::LicenceRequired)
    ));
    assert!(!out.join("J1_P1_PLASMA-A.dxf").exists());

    let rep = export::export(&mk_req(&p, &person(), true, &parts, &out, "J1"), &mut audit).unwrap();
    assert_eq!(rep.files.len(), 1);
    let written = fs::read(out.join("J1_P1_PLASMA-A.dxf")).unwrap();
    assert_eq!(sha256_hex(&written), rep.files[0].out_sha256);
    assert!(rep.manifest.exists());

    // tamper the source after certification: nothing written for a second job
    let mut tampered = body.clone();
    tampered[0] = b'9';
    fs::write(&f, &tampered).unwrap();
    assert!(matches!(
        export::export(&mk_req(&p, &person(), true, &parts, &out, "J2"), &mut audit),
        Err(ExportError::Refused {
            reason: Refusal::HashMismatch { .. },
            ..
        })
    ));
    assert!(!out.join("J2_P1_PLASMA-A.dxf").exists());

    // every outcome audited, chain verifies, edit breaks it
    let entries = nf_machine::audit::read_all(&audit_path).unwrap();
    let actions: Vec<&str> = entries.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(
        actions,
        vec![
            "export_refused",
            "export_refused",
            "export",
            "export_refused"
        ]
    );
    assert!(AuditLog::open(&audit_path).is_ok());
    let text = fs::read_to_string(&audit_path)
        .unwrap()
        .replace("vishnu", "mallory");
    fs::write(&audit_path, text).unwrap();
    assert!(AuditLog::open(&audit_path).is_err());
}

#[test]
fn export_refuses_held_and_untouched_on_any_failure() {
    let root = dir("export-held");
    let out = root.join("out");
    fs::create_dir_all(&out).unwrap();
    let mut audit = AuditLog::open(&root.join("a.jsonl")).unwrap();
    let body = dxf("ABC", "CUT_OUTER").into_bytes();
    let f = root.join("P1.dxf");
    fs::write(&f, &body).unwrap();
    let f2 = root.join("P2.dxf");
    fs::write(&f2, &body).unwrap();
    let mut held = cert("P2", &body);
    held.held = true;
    let parts = vec![
        PartSource::file("P1", f, Some(cert("P1", &body))),
        PartSource::file("P2", f2, Some(held)),
    ];
    let p = profile(TextEncoding::Utf8, DxfVersion::R2000);
    let a = person();
    let req = ExportRequest {
        job_id: "J3",
        profile: &p,
        actor: &a,
        licensed: true,
        engine_id: ENGINE,
        parts: &parts,
        out_dir: &out,
        overwrite: false,
        ts: 5,
    };
    assert!(matches!(
        export::export(&req, &mut audit),
        Err(ExportError::Refused {
            reason: Refusal::Held,
            ..
        })
    ));
    // all-or-nothing: P1 was not written either
    assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
}
