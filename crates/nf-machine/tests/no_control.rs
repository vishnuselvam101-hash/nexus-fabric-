//! Build-time guard: this crate must never start, move or control a machine. It may only write files.
//! Fails if source code uses network, serial, process or vendor control APIs, or if a non-allowlisted dependency appears.
use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] = &[
    "std::net",
    "std::os::unix::net",
    "UnixStream",
    "UnixListener",
    "UnixDatagram",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "ToSocketAddrs",
    "std::process",
    "Command",
    "serialport",
    "SerialPort",
    "serial::",
    "/dev/tty",
    "tokio",
    "reqwest",
    "hyper",
    "ureq",
    "curl",
    "modbus",
    "opcua",
    "open62541",
    "focas",
    "FOCAS",
    "mtconnect",
    "MTConnect",
    "ssh",
    "ftp",
    "smb",
    "libloading",
    "extern \"C\"",
];
const ALLOWED_DEPS: &[&str] = &["encoding_rs", "serde", "serde_json", "sha2"];

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn crate_has_no_machine_control_or_network_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);
    assert!(!files.is_empty());
    let mut hits = Vec::new();
    for f in &files {
        let text = fs::read_to_string(f).unwrap();
        for (n, line) in text.lines().enumerate() {
            for tok in FORBIDDEN {
                if line.contains(tok) {
                    hits.push(format!("{}:{}: {}", f.display(), n + 1, tok));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "forbidden control/network code:\n{}",
        hits.join("\n")
    );

    // Every dependency table counts: [dependencies], [dev-dependencies], [build-dependencies], target-specific
    // tables and the table form [dependencies.NAME]. Each named dependency must be on the allowlist.
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let mut in_deps = false;
    for line in manifest.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if let Some(header) = l.strip_prefix('[') {
            let header = header.trim_end_matches(']').trim();
            if let Some(name) = dep_table_name(header) {
                assert!(
                    ALLOWED_DEPS.contains(&name),
                    "dependency not allowlisted: {name}"
                );
            }
            in_deps = is_dep_section(header);
            continue;
        }
        if in_deps {
            let name = l.split(['=', ' ', '.']).next().unwrap().trim_matches('"');
            assert!(
                ALLOWED_DEPS.contains(&name),
                "dependency not allowlisted: {name}"
            );
        }
    }
}

fn is_dep_section(h: &str) -> bool {
    ["dependencies", "dev-dependencies", "build-dependencies"].contains(&h)
        || (h.starts_with("target.")
            && ["dependencies", "dev-dependencies", "build-dependencies"]
                .iter()
                .any(|k| h.ends_with(&format!(".{k}"))))
}

/// NAME in [dependencies.NAME] or [target.CFG.dependencies.NAME] style headers.
fn dep_table_name(h: &str) -> Option<&str> {
    for k in ["dependencies.", "dev-dependencies.", "build-dependencies."] {
        if let Some(n) = h.strip_prefix(k) {
            return Some(n);
        }
    }
    if h.starts_with("target.") {
        for k in [
            ".dependencies.",
            ".dev-dependencies.",
            ".build-dependencies.",
        ] {
            if let Some(i) = h.find(k) {
                return Some(&h[i + k.len()..]);
            }
        }
    }
    None
}
