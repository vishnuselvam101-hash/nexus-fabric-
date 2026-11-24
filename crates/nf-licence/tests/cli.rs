//! CLI round-trip tests (plan case 11): issue -> inspect, keygen file mode
//! 0600, refusal paths. Date parsing is unit-tested inline in src/main.rs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Development key, replaced by the owner's key before any sale.
/// Matches the development entry in `nf_licence::VERIFY_KEYS`.
const DEV_TEST_SEED: [u8; 32] = [
    65, 41, 102, 160, 89, 98, 20, 92, 203, 222, 5, 44, 127, 163, 111, 170, 247, 81, 158, 210, 113,
    97, 16, 248, 97, 227, 46, 47, 218, 120, 156, 231,
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nf-licence"))
        .args(args)
        .output()
        .unwrap()
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nf-licence-cli-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_seed(path: &Path, seed: &[u8; 32]) {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    use std::io::Write;
    opts.open(path).unwrap().write_all(seed).unwrap();
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8(o.stderr.clone()).unwrap()
}

#[test]
fn keygen_creates_owner_only_seed_and_never_prints_it() {
    let dir = temp_dir("keygen");
    let seed_path = dir.join("seed.key");
    let out = run(&["keygen", "--out", seed_path.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("public key: "));
    assert!(text.contains("key_id: "));
    let seed = std::fs::read(&seed_path).unwrap();
    assert_eq!(seed.len(), 32);
    // The seed itself is never printed.
    assert!(!text.contains(&hex::encode(&seed)));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&seed_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "seed file mode {:o}", mode & 0o777);
    }
    // An existing seed file is never overwritten.
    let again = run(&["keygen", "--out", seed_path.to_str().unwrap()]);
    assert!(!again.status.success());
    assert_eq!(std::fs::read(&seed_path).unwrap(), seed);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn pubkey_matches_keygen() {
    let dir = temp_dir("pubkey");
    let seed_path = dir.join("seed.key");
    let gen = run(&["keygen", "--out", seed_path.to_str().unwrap()]);
    assert!(gen.status.success(), "{}", stderr(&gen));
    let pub_out = run(&["pubkey", "--key-file", seed_path.to_str().unwrap()]);
    assert!(pub_out.status.success(), "{}", stderr(&pub_out));
    for line in stdout(&pub_out).lines() {
        assert!(stdout(&gen).contains(line), "missing {line}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn issue_inspect_round_trip() {
    let dir = temp_dir("roundtrip");
    let seed_path = dir.join("dev.key");
    write_seed(&seed_path, &DEV_TEST_SEED);
    let lic_path = dir.join("licence.nflic");
    let out = run(&[
        "issue",
        "--key-file",
        seed_path.to_str().unwrap(),
        "--licence-id",
        "LIC-CLI-0001",
        "--customer",
        "Chuo-Sogo Test",
        "--seats",
        "3",
        "--edition",
        "standard",
        "--valid-from",
        "2020-01-01",
        "--valid-until",
        "2099-12-31",
        "--issuer",
        "Chuo-Sogo",
        "--out",
        lic_path.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        !stdout(&out).contains("warning:"),
        "dev key must be compiled in"
    );
    // The written file verifies through the library against the compiled keys.
    let bytes = std::fs::read(&lic_path).unwrap();
    let payload = nf_licence::verify_with_keys(&bytes, nf_licence::VERIFY_KEYS).unwrap();
    assert_eq!(payload.licence_id, "LIC-CLI-0001");
    assert_eq!(payload.seats, 3);
    assert_eq!(payload.key_id, "39bd3a8a");
    // inspect prints the fields and the current state (valid: window covers now).
    let insp = run(&["inspect", lic_path.to_str().unwrap()]);
    assert!(insp.status.success(), "{}", stderr(&insp));
    let text = stdout(&insp);
    assert!(text.contains("licence_id: LIC-CLI-0001"));
    assert!(text.contains("customer: Chuo-Sogo Test"));
    assert!(text.contains("seats: 3"));
    assert!(text.contains("edition: standard"));
    assert!(text.contains("issuer: Chuo-Sogo"));
    assert!(text.contains("key_id: 39bd3a8a"));
    assert!(text.contains("state: valid"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn inspect_rejects_foreign_key() {
    let dir = temp_dir("foreign");
    let seed_path = dir.join("other.key");
    let gen = run(&["keygen", "--out", seed_path.to_str().unwrap()]);
    assert!(gen.status.success(), "{}", stderr(&gen));
    let lic_path = dir.join("licence.nflic");
    let out = run(&[
        "issue",
        "--key-file",
        seed_path.to_str().unwrap(),
        "--licence-id",
        "LIC-OTHER",
        "--customer",
        "Other",
        "--seats",
        "1",
        "--valid-from",
        "2020-01-01",
        "--valid-until",
        "2099-12-31",
        "--out",
        lic_path.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("warning:"));
    let insp = run(&["inspect", lic_path.to_str().unwrap()]);
    assert!(!insp.status.success());
    assert!(stderr(&insp).contains("not a valid licence"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn inspect_rejects_tampered_file() {
    let dir = temp_dir("tamper");
    let seed_path = dir.join("dev.key");
    write_seed(&seed_path, &DEV_TEST_SEED);
    let lic_path = dir.join("licence.nflic");
    let out = run(&[
        "issue",
        "--key-file",
        seed_path.to_str().unwrap(),
        "--licence-id",
        "LIC-TAMPER",
        "--customer",
        "Original",
        "--seats",
        "1",
        "--valid-from",
        "2020-01-01",
        "--valid-until",
        "2099-12-31",
        "--out",
        lic_path.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = std::fs::read_to_string(&lic_path).unwrap();
    std::fs::write(&lic_path, text.replace("Original", "Modified")).unwrap();
    let insp = run(&["inspect", lic_path.to_str().unwrap()]);
    assert!(!insp.status.success());
    assert!(stderr(&insp).contains("not a valid licence"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn key_paths_inside_data_folder_refused() {
    let dir = temp_dir("datafolder");
    std::fs::write(dir.join(".installation-marker"), b"test").unwrap();
    let seed_path = dir.join("dev.key");
    write_seed(&seed_path, &DEV_TEST_SEED);
    let out = run(&["pubkey", "--key-file", seed_path.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("data folder"));
    let gen = run(&["keygen", "--out", dir.join("new.key").to_str().unwrap()]);
    assert!(!gen.status.success());
    assert!(stderr(&gen).contains("data folder"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn issue_rejects_invalid_fields() {
    let dir = temp_dir("badfields");
    let seed_path = dir.join("dev.key");
    write_seed(&seed_path, &DEV_TEST_SEED);
    let lic_path = dir.join("l.nflic");
    let base = [
        "issue",
        "--key-file",
        seed_path.to_str().unwrap(),
        "--licence-id",
        "L",
        "--customer",
        "C",
        "--seats",
        "1",
        "--valid-from",
        "2026-01-01",
        "--valid-until",
        "2026-02-01",
        "--out",
        lic_path.to_str().unwrap(),
    ];
    // seats 0 (value at index 8).
    let mut args = base.to_vec();
    args[8] = "0";
    let out = run(&args);
    assert!(!out.status.success());
    // valid_until <= valid_from (value at index 12).
    let mut args = base.to_vec();
    args[12] = "2026-01-01";
    let out = run(&args);
    assert!(!out.status.success());
    // Bad date is a usage error (exit 2); value at index 10.
    let mut args = base.to_vec();
    args[10] = "2026-02-30";
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2));
    assert!(!lic_path.exists(), "no file written on refusal");
    std::fs::remove_dir_all(&dir).unwrap();
}
