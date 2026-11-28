//! The `nf-raster` program: outputs, refusals, report/hash agreement and the isolation of its
//! worker process.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use std::cell::RefCell;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use common::{decode_png, fixture_path};
use sha2::{Digest, Sha256};

struct Tmp(PathBuf);
impl Tmp {
    fn new(tag: &str) -> Tmp {
        let p = std::env::temp_dir().join(format!("nf-raster-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
    fn p(&self, n: &str) -> PathBuf {
        self.0.join(n)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    run_with_ledger(args)
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn hex(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

// A private ledger root per test thread, created root-owned in `nf-sandbox`. Each test's crops
// are counted together even when the input is copied to another path.
thread_local! {
    static LEDGER_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn ledger_root() -> PathBuf {
    LEDGER_ROOT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if let Some(p) = slot.as_ref() {
            return p.clone();
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p =
            std::env::temp_dir().join(format!("nf-raster-ledger-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).unwrap();
        *slot = Some(p.clone());
        p
    })
}

fn run_with_ledger(args: &[&str]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nf-raster"));
    c.args(args)
        .env("NF_CROP_LEDGER_ROOT", ledger_root())
        .env("HOME", std::env::temp_dir())
        .stdin(Stdio::null());
    c.output().unwrap()
}

fn run_with_root(args: &[&str], root: &Path) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nf-raster"));
    c.args(args)
        .env("NF_CROP_LEDGER_ROOT", root)
        .env("HOME", std::env::temp_dir())
        .stdin(Stdio::null());
    c.output().unwrap()
}

/// The program treats the input's directory as the job directory; put each fixture there.
fn input(t: &Tmp, name: &str) -> PathBuf {
    let p = t.p(name);
    // The cap test needs two distinct input paths for the same source PDF.
    let source = if name.starts_with("cap") {
        "scan_g4_small.pdf"
    } else if name == "oob.pdf" {
        "scan_g4.pdf"
    } else {
        name
    };
    std::fs::copy(fixture_path(source), &p).unwrap();
    p
}

#[test]
fn page_writes_a_png_and_a_report_that_matches_it() {
    let t = Tmp::new("page");
    let (png, rep) = (t.p("p.png"), t.p("p.json"));
    let pdf = input(&t, "scan_g4_small.pdf");
    let o = run(&[
        "page",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&png),
        "--report",
        s(&rep),
        "--dpi",
        "100",
        "--gray",
    ]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let bytes = std::fs::read(&png).unwrap();
    let p = decode_png(&bytes);
    assert_eq!((p.w, p.h, p.ch, p.dpi), (588, 416, 1, 100));
    let report = std::fs::read_to_string(&rep).unwrap();
    assert!(report.contains("\"status\": \"rendered\""));
    assert!(
        report.contains(&format!("\"png_sha256\": \"{}\"", hex(&bytes))),
        "{report}"
    );
    assert!(report.contains(&format!(
        "\"sha256\": \"{}\"",
        hex(&std::fs::read(&pdf).unwrap())
    )));
    assert!(
        report.contains("\"seccomp\": \"allow-list enforced"),
        "{report}"
    );
    assert!(report.contains("\"resource_limits\""));
    assert!(!t.p("p.png.partial").exists());
}

#[test]
fn info_writes_json() {
    let t = Tmp::new("info");
    let out = t.p("i.json");
    let pdf = input(&t, "scan_multi.pdf");
    let o = run(&["info", "--in", s(&pdf), "--out", s(&out)]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let j = std::fs::read_to_string(&out).unwrap();
    assert!(
        j.contains("\"page_count\": 1") && j.contains("\"kind\": \"scanned\""),
        "{j}"
    );
}

#[test]
fn refusals_exit_2_leave_no_png_and_say_why() {
    let t = Tmp::new("refuse");
    for (name, code) in [
        ("bad_garbage.pdf", "malformed"),
        ("bad_encrypted.pdf", "encrypted"),
        ("bad_jpx.pdf", "unsupported_filter"),
        ("bad_jpeg_data.pdf", "image_decode_failed"),
    ] {
        let (png, rep) = (t.p("r.png"), t.p("r.json"));
        // a file from an earlier run must not survive
        std::fs::write(&png, b"old").unwrap();
        let pdf = input(&t, name);
        let o = run(&[
            "page",
            "1",
            "--in",
            s(&pdf),
            "--out",
            s(&png),
            "--report",
            s(&rep),
        ]);
        assert_eq!(
            o.status.code(),
            Some(2),
            "{name}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(
            err.contains(&format!("\"code\": \"{code}\"")),
            "{name}: {err}"
        );
        assert!(
            !png.exists() && !t.p("r.png.partial").exists(),
            "{name}: output left behind"
        );
        let report = std::fs::read_to_string(&rep).unwrap();
        assert!(
            report.contains("\"status\": \"refused\"") && report.contains(code),
            "{name}: {report}"
        );
    }
}

#[test]
fn limits_exit_3() {
    let t = Tmp::new("limit");
    for (name, limit) in [
        ("bad_bomb.pdf", "compression_ratio"),
        ("bad_huge_dims.pdf", "image_pixels"),
    ] {
        let pdf = input(&t, name);
        let o = run(&[
            "page",
            "1",
            "--in",
            s(&pdf),
            "--out",
            s(&t.p("l.png")),
            "--report",
            s(&t.p("l.json")),
        ]);
        assert_eq!(o.status.code(), Some(3), "{name}");
        assert!(
            String::from_utf8_lossy(&o.stderr).contains(&format!("\"limit\": \"{limit}\"")),
            "{name}"
        );
        assert!(!t.p("l.png").exists());
    }
}

#[test]
fn bad_arguments_exit_2() {
    let t = Tmp::new("args");
    let pdf = input(&t, "scan_g4_small.pdf");
    let (png, rep) = (t.p("a.png"), t.p("a.json"));
    for (args, code) in [
        (
            vec![
                "page",
                "--in",
                s(&pdf),
                "--out",
                s(&png),
                "--report",
                s(&rep),
            ],
            "invalid_page",
        ),
        (
            vec![
                "page",
                "0",
                "--in",
                s(&pdf),
                "--out",
                s(&png),
                "--report",
                s(&rep),
            ],
            "invalid_page",
        ),
        (
            vec![
                "page",
                "1",
                "--in",
                s(&pdf),
                "--out",
                s(&png),
                "--report",
                s(&rep),
                "--dpi",
                "9",
            ],
            "invalid_dpi",
        ),
        (
            vec!["page", "1", "--in", s(&pdf), "--out", s(&png)],
            "invalid_arguments",
        ),
        (
            vec![
                "page",
                "1",
                "--in",
                "/nonexistent/x.pdf",
                "--out",
                s(&png),
                "--report",
                s(&rep),
            ],
            "input_unreadable",
        ),
        (vec!["frobnicate"], "invalid_arguments"),
        (
            vec![
                "page",
                "3",
                "--in",
                s(&pdf),
                "--out",
                s(&png),
                "--report",
                s(&rep),
            ],
            "page_out_of_range",
        ),
    ] {
        let o = run(&args);
        assert_eq!(
            o.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(
            String::from_utf8_lossy(&o.stderr).contains(code),
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
}

/// The worker tries forbidden operations after each isolation step; every one must be refused.
#[cfg(target_os = "linux")]
#[test]
fn the_worker_is_isolated() {
    let t = Tmp::new("iso");
    let pdf = input(&t, "scan_dct_gray.pdf");
    let o = run(&[
        "page",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&t.p("i.png")),
        "--report",
        s(&t.p("i.json")),
        "--dpi",
        "50",
        "--isolation-probe",
    ]);
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert_eq!(o.status.code(), Some(0), "{err}");
    for want in [
        "seccomp: socket inet tcp: refused (EPERM)",
        "seccomp: socket unix: refused (EPERM)",
        "seccomp: read the input by name: refused (EPERM)",
        "seccomp: write the output by name: refused (EPERM)",
        "seccomp: create a file beside the output: refused (EPERM)",
        "seccomp: run /bin/true: refused (EPERM)",
        "rlimit NOFILE: 16/16",
        "rlimit AS: 3221225472/3221225472",
    ] {
        assert!(err.contains(want), "missing {want:?} in\n{err}");
    }
    assert!(!err.contains("ALLOWED"), "{err}");
    let report = std::fs::read_to_string(t.p("i.json")).unwrap();
    assert!(report.contains("\"seccomp\"") && report.contains("\"landlock\""));
    // the page still rendered under the filter (JPEG, no threads, no files)
    assert!(t.p("i.png").exists());
}

/// A program ended from outside leaves no output under its name and an `incomplete` report.
#[cfg(target_os = "linux")]
#[test]
fn a_killed_program_leaves_an_incomplete_report() {
    use std::io::Read;
    use std::process::Stdio;
    let t = Tmp::new("kill");
    // A FIFO as the input blocks the worker (it reads standard input) until we kill the parent.
    let fifo = t.p("in.fifo");
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let (png, rep) = (t.p("k.png"), t.p("k.json"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_nf-raster"))
        .args([
            "page",
            "1",
            "--in",
            s(&fifo),
            "--out",
            s(&png),
            "--report",
            s(&rep),
        ])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // opening the FIFO for writing unblocks the parent's open; keep the write end open
    let writer = std::fs::OpenOptions::new().write(true).open(&fifo).unwrap();
    for _ in 0..200 {
        if std::fs::read_to_string(&rep).is_ok_and(|r| r.contains("incomplete")) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    child.kill().unwrap();
    let _ = child.wait();
    drop(writer);
    let mut err = String::new();
    let _ = child.stderr.take().unwrap().read_to_string(&mut err);
    assert!(!png.exists(), "no PNG under its name");
    let report = std::fs::read_to_string(&rep).unwrap();
    assert!(report.contains("\"status\": \"incomplete\""), "{report}");
}

#[test]
fn crop_writes_an_exact_pixel_box_in_the_sandboxed_worker() {
    let t = Tmp::new("crop");
    let (png, rep) = (t.p("c.png"), t.p("c.json"));
    let pdf = input(&t, "scan_g4_small.pdf");
    // A box within one pixel of page edges is clamped; the fractional edges are snapped out.
    let o = run(&[
        "crop",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&png),
        "--report",
        s(&rep),
        "--dpi",
        "100",
        "--gray",
        "--bbox",
        "100.2,50.3,199.7,149.8",
    ]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let p = decode_png(&std::fs::read(&png).unwrap());
    assert_eq!((p.w, p.h, p.ch, p.dpi), (100, 100, 1, 100));
    let report = std::fs::read_to_string(&rep).unwrap();
    assert!(report.contains("\"command\": \"crop\""), "{report}");
    assert!(
        report.contains("\"crop_px\": [100, 50, 200, 150]"),
        "{report}"
    );
    assert!(
        report.contains("\"seccomp\": \"allow-list enforced"),
        "{report}"
    );
    assert!(!t.p("c.png.partial").exists());
}

#[test]
fn crop_refuses_an_out_of_page_box_and_the_42nd_crop_of_a_page() {
    let t = Tmp::new("crop-limits");
    let pdf = input(&t, "cap.pdf");
    let oob = input(&t, "oob.pdf");
    let (png, rep) = (t.p("l.png"), t.p("l.json"));
    let o = run(&[
        "crop",
        "1",
        "--in",
        s(&oob),
        "--out",
        s(&png),
        "--report",
        s(&rep),
        "--dpi",
        "100",
        "--gray",
        "--bbox",
        "0,0,10000,10000",
    ]);
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(!png.exists());
    let line = String::from_utf8_lossy(&o.stderr);
    assert!(line.contains("box_outside_page"), "{line}");
    for n in 1..=40 {
        let o = run(&[
            "crop",
            "1",
            "--in",
            s(&pdf),
            "--out",
            s(&png),
            "--report",
            s(&rep),
            "--dpi",
            "100",
            "--gray",
            "--bbox",
            "10,10,20,20",
        ]);
        assert_eq!(
            o.status.code(),
            Some(0),
            "crop {n}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    let o = run(&[
        "crop",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&png),
        "--report",
        s(&rep),
        "--dpi",
        "100",
        "--gray",
        "--bbox",
        "10,10,20,20",
    ]);
    assert_eq!(
        o.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("crops_per_page"));
    // Changing the report or output name cannot start a fresh count for the same input and page.
    let other = run(&[
        "crop",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&t.p("other.png")),
        "--report",
        s(&t.p("other.json")),
        "--dpi",
        "100",
        "--gray",
        "--bbox",
        "10,10,20,20",
    ]);
    assert_eq!(
        other.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&other.stderr)
    );

    let escaped = run(&[
        "crop",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&Path::new("/tmp").join("nf-raster-escape.png")),
        "--report",
        s(&rep),
        "--bbox",
        "10,10,20,20",
    ]);
    assert_eq!(
        escaped.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&escaped.stderr)
    );
    assert!(String::from_utf8_lossy(&escaped.stderr).contains("path_outside_job"));
    assert!(!Path::new("/tmp/nf-raster-escape.png").exists());
}

#[test]
fn a_copy_of_the_same_pdf_cannot_start_a_fresh_crop_count() {
    let t = Tmp::new("crop-copy");
    let pdf = input(&t, "cap.pdf");
    let copy = input(&t, "cap-copy.pdf");
    let (png, rep) = (t.p("c.png"), t.p("c.json"));
    let crop = |p: &Path| {
        run(&[
            "crop",
            "1",
            "--in",
            s(p),
            "--out",
            s(&png),
            "--report",
            s(&rep),
            "--dpi",
            "100",
            "--gray",
            "--bbox",
            "10,10,20,20",
        ])
    };
    for _ in 1..=40 {
        assert_eq!(crop(&pdf).status.code(), Some(0));
    }
    let o = crop(&copy);
    assert_eq!(
        o.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("crops_per_page"));
}

#[cfg(unix)]
#[test]
fn crop_ledger_accepts_the_service_owned_dir_and_refuses_redirects() {
    let t = Tmp::new("crop-ledger-owner");
    let pdf = input(&t, "scan_g4_small.pdf");
    let (png, rep) = (t.p("c.png"), t.p("c.json"));
    let args = [
        "crop",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&png),
        "--report",
        s(&rep),
        "--dpi",
        "100",
        "--gray",
        "--bbox",
        "10,10,20,20",
    ];
    // The child has the same effective uid as this test. The pure owner test covers a non-root uid.
    let allowed = t.p("ledger-allowed");
    std::fs::create_dir_all(&allowed).unwrap();
    std::fs::set_permissions(&allowed, std::fs::Permissions::from_mode(0o700)).unwrap();
    let o = run_with_root(&args, &allowed);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );

    let loose = t.p("ledger-loose");
    std::fs::create_dir_all(&loose).unwrap();
    std::fs::set_permissions(&loose, std::fs::Permissions::from_mode(0o707)).unwrap();
    let o = run_with_root(&args, &loose);
    assert_eq!(o.status.code(), Some(2));
    let line = String::from_utf8_lossy(&o.stderr);
    assert!(line.contains("ledger_unreadable"), "{line}");

    let link = t.p("ledger-link");
    std::os::unix::fs::symlink(&allowed, &link).unwrap();
    let o = run_with_root(&args, &link);
    assert_eq!(o.status.code(), Some(2));
    let line = String::from_utf8_lossy(&o.stderr);
    assert!(line.contains("ledger_unreadable"), "{line}");
}

#[test]
fn a_direct_worker_crop_is_refused_without_a_parent_charge() {
    let t = Tmp::new("crop-worker");
    let pdf = input(&t, "scan_g4_small.pdf");
    let o = Command::new(env!("CARGO_BIN_EXE_nf-raster"))
        .args([
            "--worker",
            "1",
            "crop",
            "1",
            "--in",
            s(&pdf),
            "--out",
            s(&t.p("w.png")),
            "--report",
            s(&t.p("w.json")),
            "--dpi",
            "100",
            "--gray",
            "--bbox",
            "10,10,20,20",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(o.status.success(), "{:?}", o.status);
    let answer = String::from_utf8_lossy(&o.stdout);
    assert!(answer.contains("crop_not_charged"), "{answer}");
    assert!(!t.p("w.png").exists());
}

#[test]
fn a_spent_crop_charge_cannot_be_replayed_by_the_worker() {
    let t = Tmp::new("crop-replay");
    let pdf = input(&t, "scan_g4_small.pdf");
    let crop = run(&[
        "crop",
        "1",
        "--in",
        s(&pdf),
        "--out",
        s(&t.p("c.png")),
        "--report",
        s(&t.p("c.json")),
        "--dpi",
        "100",
        "--gray",
        "--bbox",
        "10,10,20,20",
    ]);
    assert_eq!(
        crop.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&crop.stderr)
    );
    // The parent's proof is forgeable only with the trusted root's secret; even a valid proof must
    // not authorize a second render once its one-use charge has been spent.
    let secret = std::fs::read(ledger_root().join(".key")).unwrap();
    let sha = hex(&std::fs::read(&pdf).unwrap());
    let mut material = secret.clone();
    material.extend_from_slice(format!("\0crop\0{sha}\0{}\0{}\0", 1, 1).as_bytes());
    material.extend_from_slice(&secret);
    let charge = format!("{}:{sha}:1:1", hex(&material));
    let replay = Command::new(env!("CARGO_BIN_EXE_nf-raster"))
        .args([
            "--worker",
            "1",
            "crop",
            "1",
            "--in",
            s(&pdf),
            "--out",
            s(&t.p("w.png")),
            "--report",
            s(&t.p("w.json")),
            "--dpi",
            "100",
            "--gray",
            "--bbox",
            "10,10,20,20",
        ])
        .env("NF_CROP_LEDGER_ROOT", ledger_root())
        .env("NF_CROP_CHARGE", charge)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(replay.status.success(), "{:?}", replay.status);
    let answer = String::from_utf8_lossy(&replay.stdout);
    assert!(answer.contains("crop_not_charged"), "{answer}");
    assert!(!t.p("w.png").exists());
}
