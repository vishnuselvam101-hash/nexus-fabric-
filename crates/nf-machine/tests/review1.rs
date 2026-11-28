//! MACH-1 review round 1: profile and DXF hardening, export write safety, audit failure handling.
mod common;
use common::*;
use nf_machine::audit::AuditLog;
use nf_machine::dxf::{self, DxfError};
use nf_machine::sha256_hex;
use nf_machine::*;
use std::fs;

fn person() -> Actor {
    Actor {
        id: "vishnu".into(),
        kind: ActorKind::Person,
    }
}

fn req<'a>(
    p: &'a MachineProfile,
    a: &'a Actor,
    parts: &'a [PartSource],
    out: &'a std::path::Path,
    overwrite: bool,
) -> ExportRequest<'a> {
    ExportRequest {
        job_id: "J1",
        profile: p,
        actor: a,
        licensed: true,
        engine_id: ENGINE,
        parts,
        out_dir: out,
        overwrite,
        ts: 7,
    }
}

#[test]
fn profile_rejects_line_breaks_in_layers_and_bad_file_names() {
    let mut p = profile(TextEncoding::Utf8, DxfVersion::R12);
    p.layer_map.insert("CUT_OUTER".into(), "OUT\n0\nEOF".into());
    assert!(matches!(p.validate(), Err(ProfileError::ControlInLayer(_))));
    let mut p = profile(TextEncoding::Utf8, DxfVersion::R12);
    p.layer_map.insert("CUT_OUTER".into(), "OUT\r".into());
    assert!(matches!(p.validate(), Err(ProfileError::ControlInLayer(_))));

    let mut p = profile(TextEncoding::Utf8, DxfVersion::R12);
    p.file_name = "{part}:stream.dxf".into();
    assert_eq!(p.validate(), Err(ProfileError::BadFileName));
    p.file_name = format!("{{part}}{}.dxf", "x".repeat(120));
    assert_eq!(p.validate(), Err(ProfileError::BadFileName));
    p.file_name = format!("{{part}}{}.dxf", "x".repeat(100));
    assert!(p.validate().is_ok());
}

#[test]
fn dxf_refuses_injected_layer_missing_units_and_ambiguous_sjis() {
    // validate() is bypassed on purpose: convert() must still refuse a line break in a layer value
    let mut p = profile(TextEncoding::Utf8, DxfVersion::R12);
    p.layer_map.insert("CUT_OUTER".into(), "OUT\n0\nEOF".into());
    assert!(matches!(
        dxf::convert(dxf("x", "CUT_OUTER").as_bytes(), &p),
        Err(DxfError::Malformed(_))
    ));

    let p = profile(TextEncoding::Utf8, DxfVersion::R12);
    let no_units = dxf("x", "CUT_OUTER").replace("9\n$INSUNITS\n70\n4\n", "");
    assert_eq!(
        dxf::convert(no_units.as_bytes(), &p),
        Err(DxfError::MissingUnits)
    );

    let sj = profile(TextEncoding::ShiftJis, DxfVersion::R12);
    assert!(matches!(
        dxf::convert(dxf("\u{a5}", "CUT_OUTER").as_bytes(), &sj),
        Err(DxfError::Unencodable('\u{a5}'))
    ));
    assert!(matches!(
        dxf::convert(dxf("\u{203e}", "CUT_OUTER").as_bytes(), &sj),
        Err(DxfError::Unencodable('\u{203e}'))
    ));
}

#[test]
fn r12_target_drops_r13_only_header_and_subclass_markers() {
    let src = dxf("x", "CUT_OUTER")
        .replace("0\nLINE\n", "0\nLINE\n100\nAcDbEntity\n")
        .replace("9\n$INSUNITS\n", "9\n$HANDSEED\n5\nFF\n9\n$INSUNITS\n");
    let r12 = dxf::convert(
        src.as_bytes(),
        &profile(TextEncoding::Utf8, DxfVersion::R12),
    )
    .unwrap();
    let s = String::from_utf8(r12).unwrap();
    assert!(!s.contains("AcDbEntity") && !s.contains("$HANDSEED"));
    assert!(s.contains("AC1009") && s.contains("$INSUNITS"));
    assert!(s.contains("LINE") && s.contains("OUTER"));

    // R2000 keeps markers and header variables when every entity carries its marker
    let marked = dxf("x", "CUT_OUTER")
        .replace("0\nLINE\n", "0\nLINE\n100\nAcDbEntity\n")
        .replace("0\nTEXT\n", "0\nTEXT\n100\nAcDbEntity\n")
        .replace("9\n$INSUNITS\n", "9\n$HANDSEED\n5\nFF\n9\n$INSUNITS\n");
    let r2000 = dxf::convert(
        marked.as_bytes(),
        &profile(TextEncoding::Utf8, DxfVersion::R2000),
    )
    .unwrap();
    let s = String::from_utf8(r2000).unwrap();
    assert!(s.contains("AcDbEntity") && s.contains("$HANDSEED") && s.contains("AC1015"));

    // R2000 is refused for R12-style input: an entity without its subclass marker
    assert!(matches!(
        dxf::convert(
            dxf("x", "CUT_OUTER").as_bytes(),
            &profile(TextEncoding::Utf8, DxfVersion::R2000)
        ),
        Err(DxfError::NeedsSubclassMarkers(_))
    ));
}

#[test]
fn engine_units_statement_adds_insunits_and_foreign_file_without_units_is_refused() {
    // NF's own DXF: no $INSUNITS, but the 999 units statement says mm
    let engine = dxf("x", "CUT_OUTER")
        .replace("9\n$INSUNITS\n70\n4\n", "")
        .replace("0\nHEADER\n", "")
        .replace("2\nHEADER\n", "2\nHEADER\n999\nNF-UNITS: mm\n");
    let p = profile(TextEncoding::Utf8, DxfVersion::R12);
    let out = String::from_utf8(dxf::convert(engine.as_bytes(), &p).unwrap()).unwrap();
    assert!(out.contains("9\r\n$INSUNITS\r\n70\r\n4\r\n"));
    assert!(out.contains("NF-UNITS: mm") && out.contains("LINE"));

    // a foreign file without units is refused, and so is a units statement that is not mm
    let foreign = dxf("x", "CUT_OUTER").replace("9\n$INSUNITS\n70\n4\n", "");
    assert_eq!(
        dxf::convert(foreign.as_bytes(), &p),
        Err(DxfError::MissingUnits)
    );
    let inch = engine.replace("NF-UNITS: mm", "NF-UNITS: inch");
    assert_eq!(
        dxf::convert(inch.as_bytes(), &p),
        Err(DxfError::MissingUnits)
    );
}

#[test]
fn export_never_follows_links_or_replaces_without_a_person_flag() {
    let root = dir("review1-links");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    let out = root.join("out");
    fs::create_dir_all(&out).unwrap();
    let body = dxf("ABC", "CUT_OUTER").into_bytes();
    let f = src.join("P1.dxf");
    fs::write(&f, &body).unwrap();
    let parts = vec![PartSource::file("P1", f.clone(), Some(cert("P1", &body)))];
    let p = profile(TextEncoding::Utf8, DxfVersion::R12);
    let a = person();
    let mut audit = AuditLog::open(&root.join("a.jsonl")).unwrap();
    let name = "J1_P1_PLASMA-A.dxf";

    // a planted link at the temp name: nothing outside may be written
    let victim = root.join("victim.txt");
    fs::write(&victim, "keep").unwrap();
    std::os::unix::fs::symlink(&victim, out.join(format!(".{name}.tmp"))).unwrap();
    assert!(export::export(&req(&p, &a, &parts, &out, false), &mut audit).is_err());
    assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
    assert!(!out.join(name).exists());
    fs::remove_file(out.join(format!(".{name}.tmp"))).unwrap();

    // a dangling link at the destination counts as existing
    std::os::unix::fs::symlink(root.join("nowhere"), out.join(name)).unwrap();
    assert!(matches!(
        export::export(&req(&p, &a, &parts, &out, true), &mut audit),
        Err(ExportError::Exists(_))
    ));
    fs::remove_file(out.join(name)).unwrap();

    // an existing regular file is kept unless a person asked to overwrite
    fs::write(out.join(name), "old").unwrap();
    assert!(matches!(
        export::export(&req(&p, &a, &parts, &out, false), &mut audit),
        Err(ExportError::Exists(_))
    ));
    assert_eq!(fs::read_to_string(out.join(name)).unwrap(), "old");
    export::export(&req(&p, &a, &parts, &out, true), &mut audit).unwrap();
    assert_eq!(
        sha256_hex(&fs::read(out.join(name)).unwrap()),
        sha256_hex(&dxf::convert(&body, &p).unwrap())
    );
}

#[test]
fn manifest_is_versioned_and_never_overwritten() {
    let root = dir("review1-manifest");
    let out = root.join("out");
    fs::create_dir_all(&out).unwrap();
    let body = dxf("ABC", "CUT_OUTER").into_bytes();
    let f = root.join("P1.dxf");
    fs::write(&f, &body).unwrap();
    let parts = vec![PartSource::file("P1", f, Some(cert("P1", &body)))];
    let p = profile(TextEncoding::Utf8, DxfVersion::R12);
    let a = person();
    let mut audit = AuditLog::open(&root.join("a.jsonl")).unwrap();
    fs::write(out.join("manifest-J1-v1.json"), "earlier").unwrap();
    let rep = export::export(&req(&p, &a, &parts, &out, false), &mut audit).unwrap();
    assert_eq!(rep.manifest.file_name().unwrap(), "manifest-J1-v2.json");
    assert_eq!(
        fs::read_to_string(out.join("manifest-J1-v1.json")).unwrap(),
        "earlier"
    );
}

#[test]
fn audit_failure_removes_files_and_refusal_audit_failure_is_an_error() {
    let root = dir("review1-audit");
    let out = root.join("out");
    fs::create_dir_all(&out).unwrap();
    let body = dxf("ABC", "CUT_OUTER").into_bytes();
    let f = root.join("P1.dxf");
    fs::write(&f, &body).unwrap();
    let parts = vec![PartSource::file("P1", f, Some(cert("P1", &body)))];
    let p = profile(TextEncoding::Utf8, DxfVersion::R12);
    let a = person();

    // the audit path becomes a folder after open: every append fails
    let apath = root.join("audit.jsonl");
    fs::write(&apath, "").unwrap();
    let mut audit = AuditLog::open(&apath).unwrap();
    fs::remove_file(&apath).unwrap();
    fs::create_dir(&apath).unwrap();
    assert!(matches!(
        export::export(&req(&p, &a, &parts, &out, false), &mut audit),
        Err(ExportError::Audit(_))
    ));
    assert_eq!(
        fs::read_dir(&out).unwrap().count(),
        0,
        "no file may survive a failed audit"
    );

    // a refusal that cannot be audited is reported as an audit error, not as a silent refusal
    let mut no_licence = req(&p, &a, &parts, &out, false);
    no_licence.licensed = false;
    assert!(matches!(
        export::export(&no_licence, &mut audit),
        Err(ExportError::Audit(_))
    ));
}
