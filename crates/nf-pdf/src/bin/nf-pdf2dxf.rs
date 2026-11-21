//! nf-pdf2dxf: converts one page of a vector PDF to DXF R12.
//!
//! nf-pdf2dxf --in <pdf> --out <dxf> --report <json> [--page N]
//!            (--mm-per-point X | --scale 1:N) [--tolerance mm] [--no-fill] [--no-text]
//!
//! Exit codes: 0 converted; 2 refused (code and message as JSON on stderr);
//! 3 limit exceeded (JSON on stderr); 70 internal error. The report JSON is written
//! for every outcome. The DXF is written only on success.
//!
//! Order of work: read the input, create the output and report files, apply resource
//! limits to this process (Unix), then parse. Nothing else is opened after the limits.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use nf_pdf::{convert, Converted, Failure, FailureKind, Limits, Options, Scale};

const USAGE: &str = "usage: nf-pdf2dxf --in <pdf> --out <dxf> --report <json> [--page N] \
(--mm-per-point X | --scale 1:N) [--tolerance mm] [--no-fill] [--no-text]";

struct Args {
    input: PathBuf,
    output: PathBuf,
    report: PathBuf,
    options: Options,
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

/// Writes a one-line refusal as JSON on stderr and exits with `code`.
fn exit_with(code: i32, status: &str, reason: &str, limit: Option<&str>, message: &str) -> ! {
    let limit_part = match limit {
        Some(l) => format!(", \"limit\": {}", json_escape(l)),
        None => String::new(),
    };
    eprintln!(
        "{{\"status\": {}, \"code\": {}{}, \"message\": {}}}",
        json_escape(status),
        json_escape(reason),
        limit_part,
        json_escape(message)
    );
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
    }
}

/// Resource limits on this process. Unix only; elsewhere the parent must apply limits.
#[cfg(unix)]
fn apply_resource_limits() -> Result<(), String> {
    use rustix::process::{setrlimit, Resource, Rlimit};
    let set = |res: Resource, v: u64| -> Result<(), String> {
        setrlimit(
            res,
            Rlimit {
                current: Some(v),
                maximum: Some(v),
            },
        )
        .map_err(|e| format!("{res:?}: {e}"))
    };
    set(Resource::As, 2 << 30)?;
    set(Resource::Cpu, 60)?;
    set(Resource::Fsize, 96 << 20)?;
    set(Resource::Nofile, 16)?;
    set(Resource::Core, 0)?;
    Ok(())
}

#[cfg(not(unix))]
fn apply_resource_limits() -> Result<(), String> {
    Ok(())
}

fn remove_if_present(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn write_report(file: &mut File, text: &str) {
    if file.write_all(text.as_bytes()).is_err() || file.flush().is_err() {
        exit_with(
            70,
            "internal_error",
            "report_write_failed",
            None,
            "the report could not be written",
        );
    }
}

fn finish_failure(args: &Args, mut report_file: File, failure: &Failure) -> ! {
    write_report(&mut report_file, &failure.report.to_json());
    remove_if_present(&args.output);
    let (code, status) = match failure.kind {
        FailureKind::Refused => (2, "refused"),
        FailureKind::LimitExceeded => (3, "limit_exceeded"),
    };
    exit_with(code, status, failure.code, failure.limit, &failure.message)
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let args = parse_args(&argv);

    let mut input_file = File::open(&args.input)
        .unwrap_or_else(|e| invalid("input_unreadable", &format!("cannot open the input: {e}")));
    let max_input = args.options.limits.max_input_bytes as u64;
    let mut data = Vec::new();
    let mut limited = (&mut input_file).take(max_input + 1);
    if limited.read_to_end(&mut data).is_err() {
        invalid("input_unreadable", "the input could not be read");
    }
    drop(input_file);

    let mut out_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args.output)
        .unwrap_or_else(|e| {
            invalid(
                "output_unwritable",
                &format!("cannot create the output: {e}"),
            )
        });
    let report_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args.report)
        .unwrap_or_else(|e| {
            invalid(
                "output_unwritable",
                &format!("cannot create the report: {e}"),
            )
        });

    if let Err(e) = apply_resource_limits() {
        remove_if_present(&args.output);
        exit_with(
            70,
            "internal_error",
            "resource_limits",
            None,
            &format!("resource limits could not be applied: {e}"),
        );
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| convert(&data, &args.options)));
    match outcome {
        Ok(Ok(Converted { dxf, report })) => {
            let mut report_file = report_file;
            if out_file.write_all(&dxf).is_err() || out_file.flush().is_err() {
                remove_if_present(&args.output);
                exit_with(
                    70,
                    "internal_error",
                    "output_write_failed",
                    None,
                    "the DXF could not be written",
                );
            }
            drop(out_file);
            write_report(&mut report_file, &report.to_json());
            std::process::exit(0);
        }
        Ok(Err(failure)) => {
            drop(out_file);
            finish_failure(&args, report_file, &failure)
        }
        Err(_) => {
            drop(out_file);
            remove_if_present(&args.output);
            exit_with(
                70,
                "internal_error",
                "internal_error",
                None,
                "the converter stopped unexpectedly",
            );
        }
    }
}
