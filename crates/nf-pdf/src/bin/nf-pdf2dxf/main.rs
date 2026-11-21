//! nf-pdf2dxf: converts one page of a vector PDF to DXF R12.
//!
//! nf-pdf2dxf --in <pdf> --out <dxf> --report <json> [--page N]
//!            (--mm-per-point X | --scale 1:N) [--tolerance mm] [--no-fill] [--no-text]
//!
//! Exit codes: 0 converted; 2 refused (code and message as JSON on stderr);
//! 3 limit exceeded (JSON on stderr); 70 internal error (JSON on stderr). Once the
//! arguments are accepted and the files are open, the report file is written for every
//! outcome (when the converter gave no report, for example after an internal error, it holds
//! the tool, version, status and refusal only). Before the work starts the report holds
//! `status: incomplete`, so a program ended from outside leaves that, not an empty file. The
//! DXF is written to `<out>.partial` and renamed to `<out>` only after success: it exists
//! under its name only after exit 0 (an earlier file of that name is removed at the start).
//!
//! Two processes. This program (the parent) checks the arguments, opens the input, creates
//! the output and report files, and starts a copy of itself as the worker: no environment,
//! the input file as its standard input, a pipe as its standard output. The worker isolates
//! itself before it reads any input (see `sandbox.rs`), converts, and returns the outcome
//! through the pipe. The parent never parses PDF data. It writes the report and the DXF,
//! removes the partial output after any failure (including a worker that crashed, was stopped by a
//! limit, or ran past [`WORKER_WALL_LIMIT`]), and exits with the outcome's code.
//!
//! `--isolation-probe` (with the normal arguments) makes the worker also attempt forbidden
//! operations after each isolation step and write one line per attempt on stderr, before it
//! converts as usual. It exists for the tests of the isolation.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod sandbox;

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use nf_pdf::{convert, Converted, FailureKind, Limits, Options, Report, Scale};

const USAGE: &str = "usage: nf-pdf2dxf --in <pdf> --out <dxf> --report <json> [--page N] \
(--mm-per-point X | --scale 1:N) [--tolerance mm] [--no-fill] [--no-text]";

/// First argument of a worker process (not for direct use).
const WORKER_FLAG: &str = "--worker";
/// Implementation limit: the parent stops the worker after this wall-clock time. It covers
/// the 30 s conversion budget plus reading 64 MiB of input and returning 64 MiB of output.
const WORKER_WALL_LIMIT: Duration = Duration::from_secs(90);
/// Implementation limit: the largest report the parent accepts from the worker.
const MAX_REPORT_BYTES: usize = 16 << 20;
/// Implementation limit: the largest message line the parent accepts from the worker.
const MAX_LINE_BYTES: usize = 64 << 10;
/// First bytes of the worker's answer on its standard output.
const FRAME_MAGIC: &[u8; 8] = b"NFP2DXF1";

struct Args {
    input: PathBuf,
    output: PathBuf,
    report: PathBuf,
    options: Options,
    probe: bool,
}

fn json_escape(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (' '..='~').contains(&c) => o.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    o.push_str(&format!("\\u{:04x}", unit));
                }
            }
        }
    }
    o.push('"');
    o
}

/// The one-line JSON written on stderr for every outcome except success.
fn json_line(status: &str, reason: &str, limit: Option<&str>, message: &str) -> String {
    let limit_part = match limit {
        Some(l) => format!(", \"limit\": {}", json_escape(l)),
        None => String::new(),
    };
    format!(
        "{{\"status\": {}, \"code\": {}{}, \"message\": {}}}",
        json_escape(status),
        json_escape(reason),
        limit_part,
        json_escape(message)
    )
}

/// Writes a one-line refusal as JSON on stderr and exits with `code`.
fn exit_with(code: i32, status: &str, reason: &str, limit: Option<&str>, message: &str) -> ! {
    eprintln!("{}", json_line(status, reason, limit, message));
    std::process::exit(code);
}

fn invalid(code: &str, message: &str) -> ! {
    exit_with(2, "refused", code, None, message)
}

fn parse_args(argv: &[String]) -> Args {
    let mut input = None;
    let mut output = None;
    let mut report = None;
    let mut page = 1u32;
    let mut mm_per_point: Option<f64> = None;
    let mut scale: Option<f64> = None;
    let mut tolerance = nf_pdf::DEFAULT_TOLERANCE_MM;
    let mut include_fill = true;
    let mut include_text = true;
    let mut probe = false;
    let mut it = argv.iter().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || -> String {
            match it.next() {
                Some(v) => v.clone(),
                None => invalid(
                    "invalid_arguments",
                    &format!("{flag} needs a value\n{USAGE}"),
                ),
            }
        };
        match flag.as_str() {
            "--in" => input = Some(PathBuf::from(value())),
            "--out" => output = Some(PathBuf::from(value())),
            "--report" => report = Some(PathBuf::from(value())),
            "--page" => {
                let v = value();
                page = v
                    .parse()
                    .unwrap_or_else(|_| invalid("invalid_page", "--page must be a whole number"));
            }
            "--mm-per-point" => {
                let v = value();
                mm_per_point = Some(v.parse::<f64>().unwrap_or_else(|_| {
                    invalid("invalid_scale", "--mm-per-point must be a number")
                }));
            }
            "--scale" => {
                let v = value();
                let n = v
                    .strip_prefix("1:")
                    .and_then(|n| n.parse::<f64>().ok())
                    .unwrap_or_else(|| invalid("invalid_scale", "--scale must look like 1:10"));
                scale = Some(n);
            }
            "--tolerance" => {
                let v = value();
                tolerance = v.parse::<f64>().unwrap_or_else(|_| {
                    invalid("invalid_tolerance", "--tolerance must be a number")
                });
            }
            "--no-fill" => include_fill = false,
            "--no-text" => include_text = false,
            "--isolation-probe" => probe = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => invalid(
                "invalid_arguments",
                &format!("unknown argument {other}\n{USAGE}"),
            ),
        }
    }
    let (Some(input), Some(output), Some(report)) = (input, output, report) else {
        invalid(
            "invalid_arguments",
            &format!("--in, --out and --report are required\n{USAGE}"),
        );
    };
    let chosen = match (mm_per_point, scale) {
        (Some(_), Some(_)) => invalid(
            "scale_ambiguous",
            "give either --mm-per-point or --scale, not both",
        ),
        (Some(v), None) => Scale::MmPerPoint(v),
        (None, Some(n)) => Scale::Ratio(n),
        (None, None) => invalid(
            "scale_required",
            "no scale given: pass --mm-per-point X (millimetres per PDF point) or --scale 1:N. A PDF has no trustworthy real-world scale, so none is guessed.",
        ),
    };
    let mut options = Options::new(chosen);
    options.page = page;
    options.tolerance_mm = tolerance;
    options.include_fill = include_fill;
    options.include_text = include_text;
    options.limits = Limits::default();
    Args {
        input,
        output,
        report,
        options,
        probe,
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(String::as_str) == Some(WORKER_FLAG) {
        worker(&argv)
    } else {
        parent(&argv)
    }
}

// ---------------------------------------------------------------------------------------------
// Parent

/// What the parent does at the end.
struct Outcome {
    code: i32,
    /// The stderr line (empty on success).
    line: String,
    /// Report text.
    report: Vec<u8>,
    /// The DXF (success only).
    dxf: Vec<u8>,
}

/// The report for an outcome that has no report from the converter: the same top-level keys
/// as a full report (tool, version, status, refusal), nothing else.
fn short_report(status: &str, reason: &str, limit: Option<&str>, message: &str) -> String {
    format!(
        "{{\n  \"tool\": \"nf-pdf2dxf\",\n  \"converter_version\": {},\n  \"status\": {},\n  \"refusal\": {{\"code\": {}, \"limit\": {}, \"message\": {}}}\n}}\n",
        json_escape(nf_pdf::VERSION),
        json_escape(status),
        json_escape(reason),
        limit.map_or("null".to_string(), json_escape),
        json_escape(message)
    )
}

fn failed(code: i32, status: &str, reason: &str, limit: Option<&str>, message: &str) -> Outcome {
    Outcome {
        code,
        line: json_line(status, reason, limit, message),
        report: short_report(status, reason, limit, message).into_bytes(),
        dxf: Vec::new(),
    }
}

fn internal(reason: &str, message: &str) -> Outcome {
    failed(70, "internal_error", reason, None, message)
}

fn create(path: &Path, what: &str) -> File {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .unwrap_or_else(|e| {
            invalid(
                "output_unwritable",
                &format!("cannot create the {what}: {e}"),
            )
        })
}

/// `<path>.partial`: where a file is written before it is renamed to its final name.
fn partial(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".partial");
    PathBuf::from(name)
}

/// Writes `bytes` to `<path>.partial` and renames it to `path`, so that `path` holds either its
/// earlier content or all of `bytes`, never a part.
fn write_whole(path: &Path, bytes: &[u8]) -> bool {
    let tmp = partial(path);
    let written = (|| -> std::io::Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.flush()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        remove_if_present(&tmp);
    }
    written.is_ok()
}

/// The report written before the worker starts. If this program is ended from outside (a
/// signal, a kill by the caller's own timeout), the report says so and no DXF exists.
fn incomplete_report() -> String {
    short_report(
        "incomplete",
        "incomplete",
        None,
        "the conversion did not finish: the program was ended before it gave an answer. No DXF was written.",
    )
}

/// The program the parent starts as the worker: this same executable.
fn worker_program() -> PathBuf {
    // On Linux the link to the running image stays valid even if the file was replaced since.
    let own = Path::new("/proc/self/exe");
    if cfg!(target_os = "linux") && own.exists() {
        return own.to_path_buf();
    }
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from(own))
}

fn parent(argv: &[String]) -> ! {
    let args = parse_args(argv);
    let input = File::open(&args.input)
        .unwrap_or_else(|e| invalid("input_unreadable", &format!("cannot open the input: {e}")));
    // A DXF left by an earlier run is never mistaken for this run's answer.
    remove_if_present(&args.output);
    // The output is written to `<out>.partial` and renamed only after success (RC-06-N1): a
    // program ended by a signal leaves no file under the output's name. The partial file is
    // created now so that an unwritable location is refused before any work.
    let out_partial = partial(&args.output);
    drop(create(&out_partial, "output"));
    let mut report_file = create(&args.report, "report");
    let pre = report_file
        .write_all(incomplete_report().as_bytes())
        .is_ok()
        && report_file.flush().is_ok();
    drop(report_file);
    if !pre {
        remove_if_present(&out_partial);
        invalid("output_unwritable", "cannot write the report");
    }
    let max_dxf = args.options.limits.max_output_bytes;

    let outcome = run_worker(argv, input, max_dxf);

    if outcome.code == 0 {
        let mut out_file = create(&out_partial, "output");
        let written = out_file.write_all(&outcome.dxf).is_ok() && out_file.flush().is_ok();
        drop(out_file);
        if !written {
            remove_if_present(&out_partial);
            exit_with(
                70,
                "internal_error",
                "output_write_failed",
                None,
                "the DXF could not be written",
            );
        }
        // The report first: a DXF under its final name always has its full report beside it.
        if !write_whole(&args.report, &outcome.report) {
            remove_if_present(&out_partial);
            exit_with(
                70,
                "internal_error",
                "report_write_failed",
                None,
                "the report could not be written",
            );
        }
        if std::fs::rename(&out_partial, &args.output).is_err() {
            remove_if_present(&out_partial);
            let _ = write_whole(
                &args.report,
                short_report(
                    "internal_error",
                    "output_write_failed",
                    None,
                    "the DXF could not be renamed to its final name",
                )
                .as_bytes(),
            );
            exit_with(
                70,
                "internal_error",
                "output_write_failed",
                None,
                "the DXF could not be renamed to its final name",
            );
        }
        std::process::exit(0);
    }
    remove_if_present(&out_partial);
    let _ = write_whole(&args.report, &outcome.report);
    eprintln!("{}", outcome.line);
    std::process::exit(outcome.code);
}

fn remove_if_present(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Starts the worker, collects its answer, and stops it after [`WORKER_WALL_LIMIT`].
fn run_worker(argv: &[String], input: File, max_dxf: usize) -> Outcome {
    let mut cmd = Command::new(worker_program());
    cmd.arg(WORKER_FLAG)
        .arg(std::process::id().to_string())
        .args(argv.iter().skip(1))
        .env_clear()
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if cfg!(windows) {
        // Some Windows system libraries need this variable even in an empty environment.
        if let Some(root) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", root);
        }
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return internal(
                "worker_start_failed",
                &format!("the converter process could not be started: {e}"),
            )
        }
    };
    let Some(mut pipe) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return internal(
            "worker_start_failed",
            "the converter process has no output pipe",
        );
    };
    let cap = frame_cap(max_dxf);
    // Reads until the worker closes its output, or one byte past the cap (then the pipe is
    // closed and the worker's next write fails).
    let spawned = std::thread::Builder::new().spawn(move || {
        let mut buf = Vec::new();
        let ok = (&mut pipe)
            .take(cap as u64 + 1)
            .read_to_end(&mut buf)
            .is_ok();
        (ok, buf)
    });
    let reader = match spawned {
        Ok(r) => r,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return internal(
                "worker_failed",
                &format!("the converter's answer could not be collected: {e}"),
            );
        }
    };
    let started = Instant::now();
    let mut timed_out = false;
    let status: std::io::Result<ExitStatus> = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e);
            }
        }
        if started.elapsed() >= WORKER_WALL_LIMIT {
            let _ = child.kill();
            timed_out = true;
            break child.wait();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (read_ok, buf) = reader.join().unwrap_or((false, Vec::new()));
    if timed_out {
        return failed(
            3,
            "limit_exceeded",
            "limit_exceeded",
            Some("wall_clock"),
            &format!(
                "the converter process did not finish within {} s and was stopped",
                WORKER_WALL_LIMIT.as_secs()
            ),
        );
    }
    let status = match status {
        Ok(s) => s,
        Err(e) => {
            return internal(
                "worker_failed",
                &format!("the converter process could not be waited for: {e}"),
            )
        }
    };
    if status.success() && read_ok {
        if let Some(outcome) = decode_frame(&buf, max_dxf) {
            return outcome;
        }
        return internal(
            "worker_failed",
            "the converter process returned an answer that could not be read",
        );
    }
    worker_death(status)
}

/// The outcome when the worker ended without an answer.
fn worker_death(status: ExitStatus) -> Outcome {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            if sandbox::is_cpu_limit_signal(sig) {
                return failed(
                    3,
                    "limit_exceeded",
                    "limit_exceeded",
                    Some("cpu_time"),
                    &format!(
                        "the converter process used its CPU-time limit ({} s) and was stopped",
                        sandbox::CPU_SECONDS
                    ),
                );
            }
            return internal(
                "worker_failed",
                &format!(
                    "the converter process was stopped by signal {sig}{}",
                    sandbox::signal_hint(sig)
                ),
            );
        }
    }
    internal(
        "worker_failed",
        &format!(
            "the converter process ended without an answer ({})",
            match status.code() {
                Some(c) => format!("exit code {c}"),
                None => "no exit code".to_string(),
            }
        ),
    )
}

fn frame_cap(max_dxf: usize) -> usize {
    FRAME_MAGIC.len() + 1 + 3 * 8 + MAX_LINE_BYTES + MAX_REPORT_BYTES + max_dxf
}

fn take_block(b: &[u8], max: usize) -> Option<(&[u8], &[u8])> {
    let (len, rest) = b.split_at_checked(8)?;
    let n = usize::try_from(u64::from_le_bytes(len.try_into().ok()?)).ok()?;
    if n > max {
        return None;
    }
    rest.split_at_checked(n)
}

/// Reads the worker's answer: magic, exit code, then the message line, the report and the
/// DXF, each as a little-endian u64 length and its bytes. Anything else is rejected.
fn decode_frame(buf: &[u8], max_dxf: usize) -> Option<Outcome> {
    let rest = buf.strip_prefix(FRAME_MAGIC.as_slice())?;
    let (&code, rest) = rest.split_first()?;
    let (line, rest) = take_block(rest, MAX_LINE_BYTES)?;
    let (report, rest) = take_block(rest, MAX_REPORT_BYTES)?;
    let (dxf, rest) = take_block(rest, max_dxf)?;
    if !rest.is_empty() {
        return None;
    }
    let line = std::str::from_utf8(line).ok()?.to_string();
    if report.is_empty() {
        return None;
    }
    match code {
        0 if !dxf.is_empty() && line.is_empty() => {}
        2 | 3 | 70 if dxf.is_empty() && !line.is_empty() => {}
        _ => return None,
    }
    Some(Outcome {
        code: i32::from(code),
        line,
        report: report.to_vec(),
        dxf: dxf.to_vec(),
    })
}

// ---------------------------------------------------------------------------------------------
// Worker

/// Sends the answer to the parent and exits. Nothing else is written to standard output.
fn send(code: u8, line: &str, report: &[u8], dxf: &[u8]) -> ! {
    let mut out = std::io::stdout().lock();
    let mut write = || -> std::io::Result<()> {
        out.write_all(FRAME_MAGIC)?;
        out.write_all(&[code])?;
        for block in [line.as_bytes(), report, dxf] {
            out.write_all(&(block.len() as u64).to_le_bytes())?;
            out.write_all(block)?;
        }
        out.flush()
    };
    let ok = write().is_ok();
    std::process::exit(if ok { 0 } else { 71 });
}

/// Sends a failure that has no converter report.
fn send_failure(code: u8, status: &str, reason: &str, message: &str) -> ! {
    let line = json_line(status, reason, None, message);
    let report = short_report(status, reason, None, message);
    send(code, &line, report.as_bytes(), b"")
}

fn failure_code(kind: FailureKind) -> (u8, &'static str) {
    match kind {
        FailureKind::Refused => (2, "refused"),
        FailureKind::LimitExceeded => (3, "limit_exceeded"),
    }
}

fn report_json(mut report: Report, isolation: &[(&'static str, String)]) -> Vec<u8> {
    report.isolation = isolation.to_vec();
    report.to_json().into_bytes()
}

fn worker(argv: &[String]) -> ! {
    // argv: program, --worker, parent process id, then the program's own arguments.
    let parent_pid: Option<u32> = argv.get(2).and_then(|p| p.parse().ok());
    let own_args: Vec<String> = argv
        .iter()
        .take(1)
        .chain(argv.iter().skip(3))
        .cloned()
        .collect();
    let args = parse_args(&own_args);

    // Isolation comes first; the input is read only afterwards.
    // The output exists only as `<out>.partial` while the worker runs.
    let mut probe = sandbox::Probe::new(args.probe, &args.input, &partial(&args.output));
    let isolation = match sandbox::isolate(parent_pid, &mut probe) {
        Ok(summary) => summary,
        Err(e) => send_failure(
            70,
            "internal_error",
            "isolation_failed",
            &format!("the converter process could not isolate itself: {e}"),
        ),
    };

    let max_input = args.options.limits.max_input_bytes as u64;
    let mut data = Vec::new();
    let read = std::io::stdin()
        .lock()
        .take(max_input + 1)
        .read_to_end(&mut data);
    if let Err(e) = read {
        send_failure(
            2,
            "refused",
            "input_unreadable",
            &format!("the input could not be read: {e}"),
        );
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| convert(&data, &args.options)));
    match outcome {
        Ok(Ok(Converted { dxf, report })) => send(0, "", &report_json(report, &isolation), &dxf),
        Ok(Err(failure)) => {
            let (code, status) = failure_code(failure.kind);
            let line = json_line(status, failure.code, failure.limit, &failure.message);
            let report = report_json(failure.report, &isolation);
            send(code, &line, &report, b"")
        }
        Err(_) => send_failure(
            70,
            "internal_error",
            "internal_error",
            "the converter stopped unexpectedly",
        ),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
    use super::*;

    fn frame(code: u8, line: &[u8], report: &[u8], dxf: &[u8]) -> Vec<u8> {
        let mut f = FRAME_MAGIC.to_vec();
        f.push(code);
        for b in [line, report, dxf] {
            f.extend_from_slice(&(b.len() as u64).to_le_bytes());
            f.extend_from_slice(b);
        }
        f
    }

    #[test]
    fn answers_from_the_worker_are_checked_before_use() {
        let ok = decode_frame(&frame(0, b"", b"{}", b"DXF"), 100).expect("valid");
        assert_eq!((ok.code, ok.dxf.as_slice()), (0, b"DXF".as_slice()));
        let refused = decode_frame(&frame(2, b"{\"status\": \"refused\"}", b"{}", b""), 100);
        assert_eq!(refused.map(|o| o.code), Some(2));
        let mut bad_magic = frame(0, b"", b"", b"DXF");
        bad_magic[0] = b'X';
        assert!(decode_frame(&bad_magic, 100).is_none(), "magic");
        assert!(
            decode_frame(&frame(5, b"x", b"", b""), 100).is_none(),
            "unknown code"
        );
        assert!(
            decode_frame(&frame(2, b"x", b"", b"DXF"), 100).is_none(),
            "DXF on failure"
        );
        assert!(
            decode_frame(&frame(2, b"", b"", b""), 100).is_none(),
            "failure without a line"
        );
        assert!(
            decode_frame(&frame(0, b"", b"", b""), 100).is_none(),
            "success without DXF"
        );
        assert!(
            decode_frame(&frame(0, b"x", b"", b"DXF"), 100).is_none(),
            "line on success"
        );
        assert!(
            decode_frame(&frame(0, b"", b"", &[b'D'; 101]), 100).is_none(),
            "DXF over the limit"
        );
        assert!(
            decode_frame(&frame(2, &[0xff], b"{}", b""), 100).is_none(),
            "line not UTF-8"
        );
        assert!(
            decode_frame(&frame(2, b"x", b"", b""), 100).is_none(),
            "no report"
        );
        let mut trailing = frame(0, b"", b"", b"DXF");
        trailing.push(0);
        assert!(decode_frame(&trailing, 100).is_none(), "trailing bytes");
        let full = frame(0, b"", b"{}", b"DXF");
        for cut in 0..full.len() {
            assert!(decode_frame(&full[..cut], 100).is_none(), "cut at {cut}");
        }
        let mut huge = FRAME_MAGIC.to_vec();
        huge.push(2);
        huge.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_frame(&huge, 100).is_none(), "length beyond the data");
    }

    #[test]
    fn the_frame_cap_covers_the_largest_valid_answer() {
        let max_dxf = Limits::default().max_output_bytes;
        let largest = FRAME_MAGIC.len() + 1 + 3 * 8 + MAX_LINE_BYTES + MAX_REPORT_BYTES + max_dxf;
        assert_eq!(frame_cap(max_dxf), largest);
    }
}
