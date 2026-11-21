// Shared by build.rs and tests/engine_id.rs (via include!). Fingerprint of the
// engine sources: SHA-256 over (relative path, length, bytes) of every source
// file, in sorted path order. Any change to a file that can alter a computed
// result changes the engine identifier recorded with every result.

pub fn source_fingerprint(files: &[(String, Vec<u8>)]) -> String {
    use sha2::{Digest, Sha256};
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut h = Sha256::new();
    for (path, bytes) in sorted {
        h.update(path.as_bytes());
        h.update([0u8]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    let d = h.finalize();
    d[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// The engine source files, as (path relative to `crates/`, content).
pub fn engine_sources(crates_dir: &std::path::Path) -> std::io::Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for krate in ["uem", "nf-dxf", "nf-core"] {
        let dir = crates_dir.join(krate).join("src");
        let mut names: Vec<_> = std::fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?.into_iter().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rs")).collect();
        names.sort();
        for p in names {
            let rel = format!("{krate}/src/{}", p.file_name().and_then(|n| n.to_str()).unwrap_or_default());
            out.push((rel, std::fs::read(&p)?));
        }
    }
    Ok(out)
}
