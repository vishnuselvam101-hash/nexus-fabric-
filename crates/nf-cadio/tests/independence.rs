//! nf-cadio shares no code with nf-dxf or nf-digest: no `use` or path reference in its sources, and
//! no runtime dependency on either. The only allowed use is a dev-dependency that writes test fixtures
//! (tests/nf_dxf_fixtures.rs).

use std::fs;
use std::path::Path;

/// Crate paths in code. A doc mention with hyphens is not code and is not matched.
const FORBIDDEN_IN_CODE: &[&str] = &["nf_dxf", "nf_digest"];

/// Package names in the manifest's runtime dependency section.
const FORBIDDEN_IN_MANIFEST: &[&str] = &["nf-dxf", "nf-digest"];

fn read_sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            read_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let name = path.display().to_string();
            out.push((name, fs::read_to_string(&path).expect("read source")));
        }
    }
}

#[test]
fn sources_do_not_reference_nf_dxf_or_nf_digest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    read_sources(&root.join("src"), &mut sources);
    assert!(!sources.is_empty());
    for (name, text) in &sources {
        for word in FORBIDDEN_IN_CODE {
            assert!(!text.contains(word), "{name} references {word}");
        }
    }
}

#[test]
fn runtime_dependencies_do_not_include_nf_dxf_or_nf_digest() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read Cargo.toml");
    let runtime = manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|rest| rest.split("[dev-dependencies]").next())
        .expect("[dependencies] section");
    for word in FORBIDDEN_IN_MANIFEST {
        assert!(!runtime.contains(word), "runtime dependency on {word}");
    }
}

#[test]
fn only_the_fixture_test_uses_nf_dxf() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    for entry in fs::read_dir(&root).expect("read tests") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).expect("read test");
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name == "independence.rs" {
                continue; // this file holds the patterns it searches for
            }
            let uses = text.contains("use nf_dxf") || text.contains("nf_dxf::");
            assert_eq!(uses, name == "nf_dxf_fixtures.rs", "{name}");
        }
    }
}
