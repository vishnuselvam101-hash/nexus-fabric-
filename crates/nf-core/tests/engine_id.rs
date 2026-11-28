//! BE-011: the engine identifier must change whenever engine source changes,
//! and results of superseded engines must be identifiable.
include!("../engine_fingerprint.rs");

#[test]
fn engine_id_is_derived_from_the_engine_sources_and_changes_with_them() {
    let crates_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let files = engine_sources(&crates_dir).unwrap();
    assert!(
        files.iter().any(|(p, _)| p == "uem/src/develop.rs")
            && files.iter().any(|(p, _)| p == "nf-dxf/src/lib.rs")
            && files.iter().any(|(p, _)| p == "nf-core/src/job.rs")
    );
    let now = source_fingerprint(&files);
    assert!(
        nf_core::ENGINE_VERSION.starts_with("nf-engine 0.6.0+src."),
        "L11 certified representation bounds require an engine version bump: {}",
        nf_core::ENGINE_VERSION
    );
    assert!(
        nf_core::ENGINE_VERSION.ends_with(&now),
        "the embedded engine id {} must carry the fingerprint {now} of the sources it was built from",
        nf_core::ENGINE_VERSION
    );
    // one changed byte in a uem source file gives a different identifier
    let mut changed = files.clone();
    let i = changed
        .iter()
        .position(|(p, _)| p == "uem/src/develop.rs")
        .unwrap();
    changed[i].1.push(b' ');
    assert_ne!(source_fingerprint(&changed), now);
    // so does a renamed or added file
    let mut added = files.clone();
    added.push(("uem/src/new.rs".into(), vec![]));
    assert_ne!(source_fingerprint(&added), now);
}

#[test]
fn engine_id_differs_from_every_superseded_engine_and_those_are_flagged() {
    assert_ne!(
        nf_core::ENGINE_VERSION,
        "nf-core 0.1.0 / uem 0.1.0",
        "the identifier used before the header-outline fix must never be reused"
    );
    assert!(
        nf_core::superseded_engine_reason("nf-core 0.1.0 / uem 0.1.0")
            .is_some_and(|r| r.contains("Defect 002"))
    );
    assert!(
        nf_core::SUPERSEDED_ENGINES
            .iter()
            .all(|(id, _)| *id != nf_core::ENGINE_VERSION)
    );
    assert!(
        !nf_core::engine_is_current("nf-core 0.1.0 / uem 0.1.0")
            && !nf_core::engine_is_current("")
            && nf_core::engine_is_current(nf_core::ENGINE_VERSION)
    );
    let out = nf_core::job::run(
        &nf_core::job::Request::CylinderShell {
            r_dev: 100.0,
            length: 250.0,
            surface_reference: None,
        },
        "t",
    );
    assert_eq!(
        out.engine,
        nf_core::ENGINE_VERSION,
        "every result records the engine that computed it"
    );
}
