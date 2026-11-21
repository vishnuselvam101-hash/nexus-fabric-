//! Isolation of the nf-pdf2dxf worker process on Linux (work plan 3.1, review finding RC-06).
//!
//! The program is run with `--isolation-probe`: after each isolation step the worker itself
//! attempts operations that the step must stop, and writes one line per attempt on stderr; then
//! it converts as usual. Each test below fails when its step is missing.
#![cfg(target_os = "linux")]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("work dir");
    dir
}

struct Run {
    code: i32,
    probe: Vec<String>,
    stderr: String,
    dir: PathBuf,
}

/// Runs the program with --isolation-probe on a small valid drawing.
fn probe_run(name: &str) -> Run {
    let dir = work_dir(name);
    let input = dir.join("in.pdf");
    std::fs::write(&input, page_pdf(&PageSpec::new(b"0 0 m 50 50 l S"))).expect("input");
    let out = Command::new(env!("CARGO_BIN_EXE_nf-pdf2dxf"))
        .arg("--in")
        .arg(&input)
        .arg("--out")
        .arg(dir.join("out.dxf"))
        .arg("--report")
        .arg(dir.join("out.json"))
        .args(["--mm-per-point", "1", "--isolation-probe"])
        .env_clear()
        .output()
        .expect("the program runs");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let probe = stderr
        .lines()
        .filter_map(|l| l.strip_prefix("nf-pdf2dxf probe: "))
        .map(str::to_string)
        .collect();
    Run {
        code: out.status.code().unwrap_or(-1),
        probe,
        stderr,
        dir,
    }
}

/// Asserts every expected probe line, and lists all that are missing at once.
fn expect_lines(run: &Run, expected: &[String]) {
    let missing: Vec<&String> = expected.iter().filter(|e| !run.probe.contains(e)).collect();
    assert!(
        missing.is_empty(),
        "missing probe results:\n  {}\nprobe output:\n  {}\nstderr:\n{}",
        missing
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  "),
        run.probe.join("\n  "),
        run.stderr
    );
}

fn hostname_line(stage: &str, refused: &str) -> String {
    if Path::new("/etc/hostname").is_file() {
        format!("{stage}: read /etc/hostname: refused ({refused})")
    } else {
        format!("{stage}: read /etc/hostname: absent on this system")
    }
}

#[test]
fn the_worker_has_its_resource_limits_before_reading_input() {
    let run = probe_run("iso_limits");
    expect_lines(
        &run,
        &[
            "limits: rlimit AS: 2147483648/2147483648".to_string(),
            "limits: rlimit CPU: 60/60".to_string(),
            "limits: rlimit FSIZE: 100663296/100663296".to_string(),
            "limits: rlimit NOFILE: 16/16".to_string(),
            "limits: rlimit CORE: 0/0".to_string(),
        ],
    );
}

#[test]
fn landlock_refuses_every_file_tcp_and_program_access() {
    let run = probe_run("iso_landlock");
    let status = run
        .probe
        .iter()
        .find(|l| l.starts_with("landlock: status: "))
        .cloned()
        .unwrap_or_default();
    assert!(
        status.contains("enforced (") || status.contains("enforced with the rights"),
        "this test needs a kernel with Landlock (Linux 5.13 or later, Landlock enabled; TCP rules from 6.7); status: {status:?}\n{}",
        run.stderr
    );
    expect_lines(
        &run,
        &[
            hostname_line("landlock", "EACCES"),
            "landlock: read the input by name: refused (EACCES)".to_string(),
            "landlock: write the output by name: refused (EACCES)".to_string(),
            "landlock: create a file beside the output: refused (EACCES)".to_string(),
            "landlock: tcp connect 127.0.0.1:9: refused (EACCES)".to_string(),
            "landlock: run /bin/true: refused (EACCES)".to_string(),
        ],
    );
}

#[test]
fn seccomp_refuses_sockets_files_and_new_programs() {
    let run = probe_run("iso_seccomp");
    expect_lines(
        &run,
        &[
            "seccomp: socket inet tcp: refused (EPERM)".to_string(),
            "seccomp: socket inet6 udp: refused (EPERM)".to_string(),
            "seccomp: socket unix: refused (EPERM)".to_string(),
            "seccomp: socketpair unix: refused (EPERM)".to_string(),
            hostname_line("seccomp", "EPERM"),
            "seccomp: read the input by name: refused (EPERM)".to_string(),
            "seccomp: write the output by name: refused (EPERM)".to_string(),
            "seccomp: create a file beside the output: refused (EPERM)".to_string(),
            "seccomp: run /bin/true: refused (EPERM)".to_string(),
        ],
    );
}

#[test]
fn an_isolated_worker_still_converts_and_the_report_says_how_it_was_isolated() {
    let run = probe_run("iso_convert");
    assert_eq!(run.code, 0, "{}", run.stderr);
    let dxf = std::fs::read(run.dir.join("out.dxf")).expect("DXF written");
    assert_eq!(polylines(&dxf, "PDF_STROKE").len(), 1);
    let report = std::fs::read_to_string(run.dir.join("out.json")).expect("report");
    for needle in [
        "\"isolation\": {\"platform\": \"linux\"",
        "\"resource_limits\": \"address space 2 GiB, CPU time 60 s",
        "\"landlock\": ",
        "\"seccomp\": \"allow-list enforced",
    ] {
        assert!(report.contains(needle), "report lacks {needle}: {report}");
    }
    // No probe file is left beside the output.
    let leftovers: Vec<_> = std::fs::read_dir(&run.dir)
        .expect("dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("probe"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

/// Review C, RC-06-N1: a program ended from outside (here SIGKILL while its worker waits for
/// input) leaves no file under the output's name and a report that says `incomplete`, never an
/// empty report; and its worker ends with it (PR_SET_PDEATHSIG).
#[test]
fn a_program_killed_from_outside_leaves_no_dxf_and_an_incomplete_report() {
    use std::io::Write;
    use std::time::{Duration, Instant};
    let dir = work_dir("killed_from_outside");
    let fifo = dir.join("in.fifo");
    let made = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "mkfifo");
    let out = dir.join("out.dxf");
    let report = dir.join("out.json");
    std::fs::write(&out, b"stale DXF of an earlier run").expect("stale output");
    let mut child = Command::new(env!("CARGO_BIN_EXE_nf-pdf2dxf"))
        .arg("--in")
        .arg(&fifo)
        .arg("--out")
        .arg(&out)
        .arg("--report")
        .arg(&report)
        .args(["--mm-per-point", "1"])
        .env_clear()
        .spawn()
        .expect("the program starts");
    // Opening the FIFO for writing lets the program's open() return; no byte is sent, so the
    // worker waits for input until the program is killed.
    let mut writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&fifo)
        .expect("fifo writer");
    let started = Instant::now();
    while !(report.exists() && dir.join("out.dxf.partial").exists()) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the program did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(300));
    child.kill().expect("kill");
    let _ = child.wait();
    assert!(!out.exists(), "a file under the output's name after a kill");
    let text = std::fs::read_to_string(&report).expect("report");
    assert!(
        text.contains("\"status\": \"incomplete\""),
        "report: {text}"
    );
    // The worker had our FIFO as its standard input; once it has ended, no reader is left and
    // a write fails with a broken pipe.
    let gone = Instant::now();
    loop {
        match writer.write_all(b"x") {
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => break,
            _ => {
                assert!(
                    gone.elapsed() < Duration::from_secs(10),
                    "the worker outlived its parent"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
