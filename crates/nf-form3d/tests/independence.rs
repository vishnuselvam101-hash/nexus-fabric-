//! Independence guard (modelled on uem's verifier_independence.rs): nf-form3d is a library over uem only. Its code
//! must not import the engine layers (nf-core, nf-dxf), the exporters, the CAD and digest crates or the server, and
//! its manifest must depend on uem alone. Test code is exempt from the source check.

const SRC: &str = include_str!("../src/lib.rs");
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
    for (no, line) in SRC.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for name in FORBIDDEN {
            assert!(
                !code.contains(&format!("{name}::")) && !code.contains(&format!("{name};")),
                "src/lib.rs line {} uses the forbidden crate {name}: {line}",
                no + 1
            );
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
