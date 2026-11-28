//! nf-raster: page images (PNG) from scanned and vector PDF pages, and a page overview.
//!
//! nf-raster info --in <pdf> --out <json>
//! nf-raster page <N> --in <pdf> --out <png> --report <json> [--dpi 200] [--gray]
//! nf-raster crop <N> --in <pdf> --out <png> --report <json> --bbox x0,y0,x1,y1 [--dpi 200] [--gray]
//!
//! Exit codes: 0 done; 2 refused (code and message as JSON on stderr); 3 limit exceeded
//! (JSON on stderr); 70 internal error (JSON on stderr). For `page` the report file is written
//! for every outcome once the arguments are accepted; before the work starts it holds
//! `status: incomplete`. The PNG is written to `<out>.partial` and renamed to `<out>` only after
//! success; an earlier file of that name is removed at the start. `info` writes its JSON to
//! `--out` the same way.
//!
//! Two processes, as for nf-pdf2dxf. This program (the parent) checks the arguments, opens the
//! input, creates the output and report files, and starts a copy of itself as the worker: no
//! environment, the input file as standard input, a pipe as standard output. The worker isolates
//! itself before it reads any input (see `sandbox.rs`), renders, and returns the outcome through
//! the pipe. The parent never parses PDF data. `--isolation-probe` makes the worker attempt
//! forbidden operations after each isolation step and print one line each on stderr (tests).

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

use nf_raster::{info, render, sha256_hex, FailureKind, Options, MAX_DPI, MIN_DPI};

const USAGE: &str = "usage: nf-raster info --in <pdf> --out <json>\n       nf-raster page <N> --in <pdf> --out <png> --report <json> [--dpi 200] [--gray]\n       nf-raster crop <N> --in <pdf> --out <png> --report <json> --bbox x0,y0,x1,y1 [--dpi 200] [--gray]";

/// First argument of a worker process (not for direct use).
const WORKER_FLAG: &str = "--worker";
/// Implementation limit: the parent stops the worker after this wall-clock time.
const WORKER_WALL_LIMIT: Duration = Duration::from_secs(150);
/// Implementation limit: the largest report the parent accepts from the worker.
const MAX_REPORT_BYTES: usize = 16 << 20;
/// Implementation limit: the largest message line the parent accepts from the worker.
const MAX_LINE_BYTES: usize = 64 << 10;
/// Implementation limit: the largest PNG (or info JSON) the parent accepts from the worker.
const MAX_OUTPUT_BYTES: usize = 256 << 20;
/// Implementation limit: crops of one page, counted by the parent across runs (PZ-2).
const MAX_CROPS_PER_PAGE: u32 = 40;
/// Implementation limit: the largest input file.
const MAX_INPUT_BYTES: u64 = 64 << 20;
/// First bytes of the worker's answer on its standard output.
const FRAME_MAGIC: &[u8; 8] = b"NFRASTR1";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command_ {
    Info,
    Page,
    Crop,
}

struct Args {
    command: Command_,
    input: PathBuf,
    output: PathBuf,
    report: PathBuf,
    options: Options,
    probe: bool,
}

fn json_escape(s: &str) -> String {
    nf_raster::report::json_str(s)
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
    let mut it = argv.iter().skip(1);
    let command = match it.next().map(String::as_str) {
        Some("info") => Command_::Info,
        Some("page") => Command_::Page,
        Some("crop") => Command_::Crop,
        Some("--help") | Some("-h") => {
            println!("{USAGE}");
            std::process::exit(0);
        }
        _ => invalid(
            "invalid_arguments",
            &format!("a command is required\n{USAGE}"),
        ),
    };
    let mut input = None;
    let mut output = None;
    let mut report = None;
    let mut page: Option<u32> = None;
    let mut bbox: Option<[f32; 4]> = None;
    let mut dpi = nf_raster::DEFAULT_DPI;
    let mut gray = false;
    let mut probe = false;
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
            "--dpi" => {
                let v = value();
                dpi = v
                    .parse()
                    .unwrap_or_else(|_| invalid("invalid_dpi", "--dpi must be a whole number"));
                if !(MIN_DPI..=MAX_DPI).contains(&dpi) {
                    invalid(
                        "invalid_dpi",
                        &format!("--dpi must be between {MIN_DPI} and {MAX_DPI}"),
                    );
                }
            }
            "--bbox" => {
                let v = value();
                let parts: Vec<f32> = v
                    .split(',')
                    .map(|s| s.trim().parse::<f32>())
                    .collect::<Result<_, _>>()
                    .unwrap_or_else(|_| {
                        invalid("invalid_bbox", "--bbox must be x0,y0,x1,y1 in page pixels")
                    });
                bbox = parts.try_into().ok().filter(|b: &[f32; 4]| {
                    b.iter().all(|n| n.is_finite()) && b[0] < b[2] && b[1] < b[3]
                });
                if bbox.is_none() {
                    invalid("invalid_bbox", "--bbox must be x0,y0,x1,y1 in page pixels");
                }
            }
            "--gray" => gray = true,
            "--isolation-probe" => probe = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => {
                if matches!(command, Command_::Page | Command_::Crop)
                    && page.is_none()
                    && !other.starts_with('-')
                {
                    page = Some(other.parse().unwrap_or_else(|_| {
                        invalid("invalid_page", "the page must be a whole number from 1")
                    }));
                } else {
                    invalid(
                        "invalid_arguments",
                        &format!("unknown argument {other}\n{USAGE}"),
                    )
                }
            }
        }
    }
    let (Some(input), Some(output)) = (input, output) else {
        invalid(
            "invalid_arguments",
            &format!("--in and --out are required\n{USAGE}"),
        );
    };
    let report = match (command, report) {
        (_, Some(r)) => r,
        (Command_::Info, None) => {
            let mut name = output.as_os_str().to_os_string();
            name.push(".report");
            PathBuf::from(name)
        }
        (Command_::Page | Command_::Crop, None) => invalid(
            "invalid_arguments",
            &format!("--report is required for page and crop\n{USAGE}"),
        ),
    };
    if matches!(command, Command_::Page | Command_::Crop) && page.is_none() {
        invalid("invalid_page", "the page number is required: <command> <N>");
    }
    if command == Command_::Crop && bbox.is_none() {
        invalid("invalid_bbox", "--bbox is required for crop");
    }
    if command != Command_::Crop && bbox.is_some() {
        invalid("invalid_arguments", "--bbox is only valid for crop");
    }
    let options = Options {
        page: page.unwrap_or(1),
        dpi,
        gray,
        crop: bbox,
        ..Options::default()
    };
    Args {
        command,
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
    /// The PNG or info JSON (success only).
    dxf: Vec<u8>,
}

/// The report for an outcome that has no report from the converter: the same top-level keys
/// as a full report (tool, version, status, refusal), nothing else.
fn short_report(status: &str, reason: &str, limit: Option<&str>, message: &str) -> String {
    format!(
        "{{\n  \"tool\": \"nf-raster\",\n  \"version\": {},\n  \"status\": {},\n  \"refusal\": {{\"code\": {}, \"limit\": {}, \"message\": {}}}\n}}\n",
        json_escape(nf_raster::VERSION),
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

/// Counts live beside the report, so the cap follows the job's output directory and never
/// touches global state. Each record is the page number and its count (two little-endian u32).
fn ledger_path(report: &Path) -> PathBuf {
    let mut name = report.as_os_str().to_os_string();
    name.push(".crop-ledger");
    PathBuf::from(name)
}

/// Charge one crop to a page. The ledger is written atomically: an interrupted run never leaves
/// a half record. A concurrent writer can lose one increment; the limit is an implementation
/// guard, not a security decision, and the renderer's own pixel/bomb limits still apply.
fn charge_crop(report: &Path, page: u32) -> Result<(), Outcome> {
    let path = ledger_path(report);
    let old = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => {
            return Err(failed(
                2,
                "refused",
                "ledger_unreadable",
                None,
                "the crop count could not be read",
            ))
        }
    };
    if old.len() > 64 * 1024 || old.len() % 8 != 0 {
        return Err(failed(
            2,
            "refused",
            "ledger_invalid",
            None,
            "the crop count is damaged",
        ));
    }
    let mut records: Vec<(u32, u32)> = Vec::new();
    for pair in old.chunks_exact(8) {
        let n = |i: usize| -> Option<u32> {
            Some(u32::from_le_bytes(pair.get(i..i + 4)?.try_into().ok()?))
        };
        let (Some(p), Some(c)) = (n(0), n(4)) else {
            return Err(failed(
                2,
                "refused",
                "ledger_invalid",
                None,
                "the crop count is damaged",
            ));
        };
        records.push((p, c));
    }
    let entry = records.iter_mut().find(|(p, _)| *p == page);
    let count = match entry {
        Some((_, c)) => {
            if *c >= MAX_CROPS_PER_PAGE {
                return Err(failed(
                    3,
                    "limit_exceeded",
                    "limit_exceeded",
                    Some("crops_per_page"),
                    &format!("no more than {MAX_CROPS_PER_PAGE} crops of one page are allowed"),
                ));
            }
            *c += 1;
            *c
        }
        None => {
            records.push((page, 1));
            1
        }
    };
    let _ = count;
    let mut bytes = Vec::with_capacity(records.len() * 8);
    for (p, c) in records {
        bytes.extend_from_slice(&p.to_le_bytes());
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    if !write_whole(&path, &bytes) {
        return Err(failed(
            70,
            "internal_error",
            "ledger_write_failed",
            None,
            "the crop count could not be written",
        ));
    }
    Ok(())
}

/// The report written before the worker starts. If this program is ended from outside (a
/// signal, a kill by the caller's own timeout), the report says so and no output exists.
fn incomplete_report() -> String {
    short_report(
        "incomplete",
        "incomplete",
        None,
        "the rendering did not finish: the program was ended before it gave an answer. No output was written.",
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
    // An output left by an earlier run is never mistaken for this run's answer.
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
    let max_out = MAX_OUTPUT_BYTES;

    let outcome = run_worker(argv, input, max_out);

    if outcome.code == 0 {
        if args.command == Command_::Crop {
            if let Err(outcome) = charge_crop(&args.report, args.options.page) {
                remove_if_present(&out_partial);
                let _ = write_whole(&args.report, &outcome.report);
                eprintln!("{}", outcome.line);
                std::process::exit(outcome.code);
            }
        }
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
                "the output could not be written",
            );
        }
        // The report first: an output under its final name always has its full report beside it.
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
                    "the output could not be renamed to its final name",
                )
                .as_bytes(),
            );
            exit_with(
                70,
                "internal_error",
                "output_write_failed",
                None,
                "the output could not be renamed to its final name",
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
fn run_worker(argv: &[String], input: File, max_out: usize) -> Outcome {
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
    let cap = frame_cap(max_out);
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
        if let Some(outcome) = decode_frame(&buf, max_out) {
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

fn frame_cap(max_out: usize) -> usize {
    FRAME_MAGIC.len() + 1 + 3 * 8 + MAX_LINE_BYTES + MAX_REPORT_BYTES + max_out
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
fn decode_frame(buf: &[u8], max_out: usize) -> Option<Outcome> {
    let rest = buf.strip_prefix(FRAME_MAGIC.as_slice())?;
    let (&code, rest) = rest.split_first()?;
    let (line, rest) = take_block(rest, MAX_LINE_BYTES)?;
    let (report, rest) = take_block(rest, MAX_REPORT_BYTES)?;
    let (dxf, rest) = take_block(rest, max_out)?;
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

    let mut data = Vec::new();
    let read = std::io::stdin()
        .lock()
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut data);
    if let Err(e) = read {
        send_failure(
            2,
            "refused",
            "input_unreadable",
            &format!("the input could not be read: {e}"),
        );
    }
    if data.len() as u64 > MAX_INPUT_BYTES {
        let line = json_line(
            "limit_exceeded",
            "limit_exceeded",
            Some("input_bytes"),
            "the input exceeds the size limit",
        );
        let report = short_report(
            "limit_exceeded",
            "limit_exceeded",
            Some("input_bytes"),
            "the input exceeds the size limit",
        );
        send(3, &line, report.as_bytes(), b"");
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| match args.command {
        Command_::Info => info(&data, &args.options).map(|i| {
            let json = i.to_json(&isolation).into_bytes();
            (json.clone(), json)
        }),
        Command_::Page | Command_::Crop => render(&data, &args.options).map(|r| {
            let png_hash = sha256_hex(&r.png);
            let report = r.to_json(&png_hash, &isolation).into_bytes();
            (report, r.png)
        }),
    }));
    match outcome {
        Ok(Ok((report, payload))) => send(0, "", &report, &payload),
        Ok(Err(failure)) => {
            let status = match failure.kind {
                FailureKind::Refused => "refused",
                FailureKind::LimitExceeded => "limit_exceeded",
            };
            let line = json_line(status, failure.code, failure.limit, &failure.message);
            let report = short_report(status, failure.code, failure.limit, &failure.message);
            send(failure.exit_code() as u8, &line, report.as_bytes(), b"")
        }
        Err(_) => send_failure(
            70,
            "internal_error",
            "internal_error",
            "the renderer stopped unexpectedly",
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
        let max_out = MAX_OUTPUT_BYTES;
        let largest = FRAME_MAGIC.len() + 1 + 3 * 8 + MAX_LINE_BYTES + MAX_REPORT_BYTES + max_out;
        assert_eq!(frame_cap(max_out), largest);
    }
}
