//! The `nf-raster` program: outputs, refusals, report/hash agreement and the isolation of its
//! worker process.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    Command::new(env!("CARGO_BIN_EXE_nf-raster"))
        .args(args)
        .output()
        .unwrap()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn hex(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

#[test]
fn page_writes_a_png_and_a_report_that_matches_it() {
    let t = Tmp::new("page");
    let (png, rep) = (t.p("p.png"), t.p("p.json"));
    let pdf = fixture_path("scan_g4_small.pdf");
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
    let pdf = fixture_path("scan_multi.pdf");
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
        let pdf = fixture_path(name);
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
        let pdf = fixture_path(name);
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
    let pdf = fixture_path("scan_g4_small.pdf");
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
    let pdf = fixture_path("scan_dct_gray.pdf");
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
    let pdf = fixture_path("scan_g4_small.pdf");
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
    let pdf = fixture_path("scan_g4_small.pdf");
    let (png, rep) = (t.p("l.png"), t.p("l.json"));
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
}
