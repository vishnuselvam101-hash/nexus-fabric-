//! Independence guard (modelled on uem's verifier_independence.rs): nf-form3d is a library over uem only. Its code
//! must not import the engine layers (nf-core, nf-dxf), the exporters, the CAD and digest crates or the server, and
//! its manifest must depend on uem alone. Every source file of src/ is scanned. Test code is exempt from the scan.

use std::path::PathBuf;

const MANIFEST: &str = include_str!("../Cargo.toml");

const FORBIDDEN: &[&str] = &[
    "nf_core",
    "nf_dxf",
    "nf_export",
    "nf_cadio",
    "nf_cad",
    "nf_digest",
    "nf_nest",
    "nf_estimate",
    "nf_server",
    "nf_pdf",
];

#[test]
fn source_does_not_reach_forbidden_crates() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for path in &files {
        assert!(
            path.is_file() && path.extension().is_some_and(|x| x == "rs"),
            "unexpected entry in src/: {}",
            path.display()
        );
    }
    assert!(files.iter().any(|p| p.ends_with("lib.rs")));
    assert!(files.iter().any(|p| p.ends_with("triangulate.rs")));
    for path in &files {
        let src = std::fs::read_to_string(path).unwrap();
        for (no, line) in src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for name in FORBIDDEN {
                assert!(
                    !code.contains(&format!("{name}::")) && !code.contains(&format!("{name};")),
                    "{} line {} uses the forbidden crate {name}: {line}",
                    path.display(),
                    no + 1
                );
            }
        }
    }
}

#[test]
fn manifest_depends_on_uem_only() {
    let mut in_deps = false;
    let mut deps = Vec::new();
    for line in MANIFEST.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t == "[dependencies]";
            continue;
        }
        if in_deps && !t.is_empty() {
            deps.push(t.split('=').next().unwrap_or("").trim().to_string());
        }
    }
    assert_eq!(deps, vec!["uem".to_string()]);
}
