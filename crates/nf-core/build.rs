// Derives the engine identifier from the engine sources (BE-011): a result
// stored by one build can be told apart from a result of any other build.
include!("engine_fingerprint.rs");

fn main() {
    let crates_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("crates directory").to_path_buf();
    for krate in ["uem", "nf-dxf", "nf-core"] {
        println!("cargo:rerun-if-changed={}", crates_dir.join(krate).join("src").display());
    }
    println!("cargo:rerun-if-changed=engine_fingerprint.rs");
    let files = engine_sources(&crates_dir).expect("engine sources must be readable at build time");
    println!("cargo:rustc-env=NF_ENGINE_SRC_HASH={}", source_fingerprint(&files));
}
