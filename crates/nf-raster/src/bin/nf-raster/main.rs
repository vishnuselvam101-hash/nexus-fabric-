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

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
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
/// The trusted ledger root when the installer has not configured one. It is never inside a job
/// directory: a caller cannot start a fresh count by copying the same PDF somewhere else.
const DEFAULT_CROP_ROOT: &str = "/var/lib/nexus-fabric/crop-ledger";
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

/// Adds `O_NOFOLLOW` where the platform has it. Every ledger and output open uses this so a
/// symlink swapped in after a path check cannot redirect a write.
#[cfg(unix)]
fn no_follow(opts: &mut OpenOptions) {
    opts.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn no_follow(_opts: &mut OpenOptions) {}

/// A directory is fit for the ledger only when it is owned by root or by the current effective
/// user, and neither group nor others can write into it. The shipped service runs as `nexusfabric`
/// and owns `/var/lib/nexus-fabric`; a caller outside the service cannot own that directory and
/// cannot choose a fresh ledger by setting the environment.
#[cfg(unix)]
fn trusted_dir_parts(is_dir: bool, uid: u32, euid: u32, mode: u32) -> bool {
    is_dir && (uid == 0 || uid == euid) && mode & 0o022 == 0
}

#[cfg(unix)]
fn trusted_dir(md: &fs::Metadata) -> bool {
    trusted_dir_parts(
        md.is_dir(),
        md.uid(),
        rustix::process::geteuid().as_raw(),
        md.mode(),
    )
}

#[cfg(not(unix))]
fn trusted_dir(md: &fs::Metadata) -> bool {
    md.is_dir()
}

/// The proof key is fit only when owned by root or by the current effective user, with no group or
/// other access bits and no owner-execute bit. The file is opened with O_NOFOLLOW below, so a
/// symlink cannot redirect this check to a key prepared elsewhere.
#[cfg(unix)]
fn trusted_key_parts(is_file: bool, uid: u32, euid: u32, mode: u32) -> bool {
    is_file && (uid == 0 || uid == euid) && mode & 0o177 == 0
}

#[cfg(unix)]
fn trusted_key(md: &fs::Metadata) -> bool {
    trusted_key_parts(
        md.is_file(),
        md.uid(),
        rustix::process::geteuid().as_raw(),
        md.mode(),
    )
}

#[cfg(not(unix))]
fn trusted_key(md: &fs::Metadata) -> bool {
    md.is_file()
}

fn ledger_root() -> PathBuf {
    match std::env::var_os("NF_CROP_LEDGER_ROOT") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(DEFAULT_CROP_ROOT),
    }
}

/// Opens the ledger root, creating it on first use, and refuses a root that is not trusted.
fn open_ledger_root() -> Result<PathBuf, Outcome> {
    let configured = ledger_root();
    // A symlink could redirect the count to a directory the caller prepared elsewhere. Refuse it
    // before following the link; the per-crop files are also opened with O_NOFOLLOW.
    if fs::symlink_metadata(&configured).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop count directory must not be a symbolic link",
        ));
    }
    let exists = configured.exists();
    fs::create_dir_all(&configured).map_err(|_| {
        failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop count directory could not be created",
        )
    })?;
    let root = fs::canonicalize(&configured).map_err(|_| {
        failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop count directory could not be opened",
        )
    })?;
    #[cfg(unix)]
    if !exists {
        let _ = fs::set_permissions(&root, fs::Permissions::from_mode(0o700));
    }
    let ok = fs::metadata(&root).is_ok_and(|m| trusted_dir(&m));
    if !ok {
        return Err(failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop count directory is not controlled by the server",
        ));
    }
    Ok(root)
}

/// The input is the trust anchor: its directory is the job directory. Outputs may not be named
/// outside it, through an absolute path, `..` or a symbolic link.
fn job_dir(input: &Path) -> (PathBuf, PathBuf) {
    let real = fs::canonicalize(input)
        .unwrap_or_else(|e| invalid("input_unreadable", &format!("cannot open the input: {e}")));
    let root = real
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .to_path_buf();
    (real, root)
}

fn confined_path(path: &Path, root: &Path, what: &str) -> PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let real_parent = fs::canonicalize(parent).unwrap_or_else(|e| {
        invalid(
            "output_unwritable",
            &format!("cannot use the {what} directory: {e}"),
        )
    });
    if real_parent != root {
        invalid(
            "path_outside_job",
            &format!("the {what} must be inside the input's directory"),
        );
    }
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        invalid(
            "path_outside_job",
            &format!("the {what} must not be a symbolic link"),
        );
    }
    let name = path.file_name().unwrap_or_else(|| {
        invalid(
            "invalid_arguments",
            &format!("the {what} needs a file name"),
        )
    });
    real_parent.join(name)
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
    let (input, root) = job_dir(&input);
    let output = confined_path(&output, &root, "output");
    let report = confined_path(&report, &root, "report");
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
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    no_follow(&mut opts);
    opts.open(path).unwrap_or_else(|e| {
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
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        no_follow(&mut opts);
        let mut f = opts.open(&tmp)?;
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

/// Opens an existing file without following a final symlink.
fn open_no_follow(path: &Path, what: &str) -> Result<File, Outcome> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    no_follow(&mut opts);
    opts.open(path).map_err(|_| {
        failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            &format!("the {what} could not be opened safely"),
        )
    })
}

/// Opens a lock file, creating it in the same no-follow step. A symlink swapped in between a path
/// check and the open is refused by the kernel.
fn open_lock(path: &Path) -> Result<File, Outcome> {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true);
    no_follow(&mut opts);
    opts.open(path).map_err(|_| {
        failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop lock could not be opened safely",
        )
    })
}

/// The counter is keyed by the bytes of the canonical input and the page, and lives in the trusted
/// root. Copying the same PDF to another job directory cannot start a fresh count; changing
/// `--report` or an output name cannot either. Each record is the page number and its count (two
/// little-endian u32).
fn crop_paths(root: &Path, content_sha: &str, page: u32) -> (PathBuf, PathBuf) {
    let key = crop_key(content_sha, page);
    (
        root.join(format!(".nf-raster-crop-{key}.ledger")),
        root.join(format!(".nf-raster-crop-{key}.lock")),
    )
}

fn crop_key(content_sha: &str, page: u32) -> String {
    sha256_hex(format!("{content_sha}\0{page}").as_bytes())
}

/// A charge is a one-use file in the trusted root, named by the exact content, page and count that
/// the parent charged. The worker consumes it before it renders; a replay has no file to consume.
fn crop_token(root: &Path, content_sha: &str, page: u32, count: u32) -> PathBuf {
    root.join(format!(
        ".nf-raster-crop-{}.{count}.charge",
        crop_key(content_sha, page)
    ))
}

/// The SHA-256 of the exact input bytes. This is the identity the ledger counts, not a caller-chosen
/// path or file name.
fn input_sha(input: &Path) -> Result<String, Outcome> {
    let md = fs::metadata(input).map_err(|_| {
        failed(
            2,
            "refused",
            "input_unreadable",
            None,
            "the input could not be read",
        )
    })?;
    if md.len() > MAX_INPUT_BYTES {
        return Err(failed(
            3,
            "limit_exceeded",
            "limit_exceeded",
            Some("input_bytes"),
            "the input exceeds the size limit",
        ));
    }
    let bytes = fs::read(input).map_err(|_| {
        failed(
            2,
            "refused",
            "input_unreadable",
            None,
            "the input could not be read",
        )
    })?;
    Ok(sha256_hex(&bytes))
}

/// Charge one crop to a page before any rendering. The lock is held across the read, the check and
/// the atomic write, so concurrent runs cannot both claim the last slot. An interrupted run leaves
/// either the old count or the new one, never a half record. The returned count names a one-time
/// charge that the worker must spend before it renders.
fn charge_crop(input: &Path, page: u32) -> Result<(u32, String), Outcome> {
    let root = open_ledger_root()?;
    let content_sha = input_sha(input)?;
    let (ledger, lock_path) = crop_paths(&root, &content_sha, page);
    let lock = open_lock(&lock_path)?;
    lock.lock().map_err(|_| {
        failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop count could not be locked",
        )
    })?;
    let old = match open_no_follow(&ledger, "crop ledger") {
        Ok(f) => {
            let mut bytes = Vec::new();
            use std::io::Read;
            f.take(64 * 1024 + 1).read_to_end(&mut bytes).map_err(|_| {
                failed(
                    2,
                    "refused",
                    "ledger_unreadable",
                    None,
                    "the crop count could not be read",
                )
            })?;
            bytes
        }
        Err(outcome) if outcome.code == 2 => Vec::new(),
        Err(outcome) => return Err(outcome),
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
    let (pairs, _) = old.as_chunks::<8>();
    for &[a, b, c, d, e, f, g, h] in pairs {
        records.push((
            u32::from_le_bytes([a, b, c, d]),
            u32::from_le_bytes([e, f, g, h]),
        ));
    }
    let count = match records.iter_mut().find(|(p, _)| *p == page) {
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
    let mut bytes = Vec::with_capacity(records.len() * 8);
    for (p, c) in records {
        bytes.extend_from_slice(&p.to_le_bytes());
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    if !write_whole(&ledger, &bytes) {
        return Err(failed(
            70,
            "internal_error",
            "ledger_write_failed",
            None,
            "the crop count could not be written",
        ));
    }
    // The worker spends this exact count exactly once. A proof cannot be replayed for a second
    // render because the charge file is removed under the same lock while the worker is unsandboxed.
    if !write_whole(&crop_token(&root, &content_sha, page, count), b"1") {
        return Err(failed(
            70,
            "internal_error",
            "ledger_write_failed",
            None,
            "the crop charge could not be recorded",
        ));
    }
    Ok((count, content_sha))
}

/// Spend the one-time charge for this crop. The same lock the parent used guards the remove, so
/// two workers holding the same proof cannot both render. This runs before the worker isolates
/// itself; after isolation it has no file access.
fn consume_crop_charge(
    root: &Path,
    content_sha: &str,
    page: u32,
    count: u32,
) -> Result<(), Outcome> {
    let (_, lock_path) = crop_paths(root, content_sha, page);
    let lock = open_lock(&lock_path)?;
    lock.lock().map_err(|_| {
        failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop charge could not be locked",
        )
    })?;
    let token = crop_token(root, content_sha, page, count);
    let f = open_no_follow(&token, "crop charge")?;
    if !f.metadata().is_ok_and(|m| m.is_file()) {
        return Err(failed(
            2,
            "refused",
            "crop_not_charged",
            None,
            "the crop charge is not valid",
        ));
    }
    std::fs::remove_file(&token).map_err(|_| {
        failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop charge could not be spent",
        )
    })
}

/// The install secret lets the parent mint a proof that a crop was charged. It lives in the
/// trusted root and is never sent to a caller; the worker reads it directly before it isolates
/// itself, so a direct `--worker crop` without the parent's proof is refused.
fn crop_secret(root: &Path) -> Result<Vec<u8>, Outcome> {
    let path = root.join(".key");
    if let Ok(f) = open_no_follow(&path, "crop key") {
        let meta = f.metadata().ok();
        let mut key = Vec::new();
        let trusted_key = meta.as_ref().is_some_and(trusted_key);
        if f.take(33).read_to_end(&mut key).is_ok() && key.len() == 32 && trusted_key {
            return Ok(key);
        }
        return Err(failed(
            2,
            "refused",
            "ledger_unreadable",
            None,
            "the crop proof key is damaged",
        ));
    }
    let mut bytes = [0u8; 32];
    let random = File::open("/dev/urandom").map_err(|_| {
        failed(
            70,
            "internal_error",
            "ledger_write_failed",
            None,
            "the crop proof key could not be generated",
        )
    })?;
    use std::io::Read;
    let mut random = random;
    if random.read_exact(&mut bytes).is_err() {
        return Err(failed(
            70,
            "internal_error",
            "ledger_write_failed",
            None,
            "the crop proof key could not be generated",
        ));
    }
    if !write_whole(&path, &bytes) {
        return Err(failed(
            70,
            "internal_error",
            "ledger_write_failed",
            None,
            "the crop proof key could not be written",
        ));
    }
    #[cfg(unix)]
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    Ok(bytes.to_vec())
}

/// A domain-separated digest of the install secret and the charge. The parent creates it; the
/// worker derives the same digest from the root's secret and the value in its environment.
fn crop_proof(secret: &[u8], content_sha: &str, page: u32, count: u32) -> String {
    let mut material = Vec::with_capacity(secret.len() * 2 + 128);
    material.extend_from_slice(secret);
    material.extend_from_slice(format!("\0crop\0{content_sha}\0{page}\0{count}\0").as_bytes());
    material.extend_from_slice(secret);
    sha256_hex(&material)
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
    // A crop is charged now, before its render starts: the 41st request does no work at all.
    let mut crop_env = None;
    if args.command == Command_::Crop {
        match charge_crop(&args.input, args.options.page) {
            Ok((count, content_sha)) => {
                let charge = match open_ledger_root().and_then(|root| {
                    crop_secret(&root).map(|secret| {
                        (
                            format!(
                                "{}:{content_sha}:{}:{count}",
                                crop_proof(&secret, &content_sha, args.options.page, count),
                                args.options.page
                            ),
                            root,
                        )
                    })
                }) {
                    Ok(charge) => charge,
                    Err(outcome) => {
                        remove_if_present(&out_partial);
                        let _ = write_whole(&args.report, &outcome.report);
                        eprintln!("{}", outcome.line);
                        std::process::exit(outcome.code);
                    }
                };
                crop_env = Some(charge);
            }
            Err(outcome) => {
                remove_if_present(&out_partial);
                let _ = write_whole(&args.report, &outcome.report);
                eprintln!("{}", outcome.line);
                std::process::exit(outcome.code);
            }
        }
    }
    let max_out = MAX_OUTPUT_BYTES;

    let outcome = run_worker(argv, input, max_out, crop_env);

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
fn run_worker(
    argv: &[String],
    input: File,
    max_out: usize,
    crop_env: Option<(String, PathBuf)>,
) -> Outcome {
    let mut cmd = Command::new(worker_program());
    cmd.arg(WORKER_FLAG)
        .arg(std::process::id().to_string())
        .args(argv.iter().skip(1))
        .env_clear();
    if let Some((charge, root)) = crop_env {
        cmd.env("NF_CROP_CHARGE", charge)
            .env("NF_CROP_LEDGER_ROOT", root);
    }
    cmd.stdin(Stdio::from(input))
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

/// The parent passes a proof that this crop was charged in the trusted ledger. A direct worker
/// call has no such proof and is refused before it renders anything. The secret is read from the
/// trusted root while the worker still has file access, before isolation.
fn worker_crop_charge(
    args: &Args,
) -> Result<String, (u8, &'static str, &'static str, &'static str)> {
    let charge = std::env::var("NF_CROP_CHARGE").map_err(|_| {
        (
            2,
            "refused",
            "crop_not_charged",
            "a crop must be charged by the parent before rendering",
        )
    })?;
    let parts: Vec<&str> = charge.split(':').collect();
    let (Some(proof), Some(content_sha), Some(page), Some(count)) =
        (parts.first(), parts.get(1), parts.get(2), parts.get(3))
    else {
        return Err((
            2,
            "refused",
            "crop_not_charged",
            "the crop charge is not valid",
        ));
    };
    let (Ok(page), Ok(count)) = (page.parse::<u32>(), count.parse::<u32>()) else {
        return Err((
            2,
            "refused",
            "crop_not_charged",
            "the crop charge is not valid",
        ));
    };
    if page != args.options.page || count == 0 || count > MAX_CROPS_PER_PAGE {
        return Err((
            if count > MAX_CROPS_PER_PAGE { 3 } else { 2 },
            if count > MAX_CROPS_PER_PAGE {
                "limit_exceeded"
            } else {
                "refused"
            },
            if count > MAX_CROPS_PER_PAGE {
                "crops_per_page"
            } else {
                "crop_not_charged"
            },
            "the crop charge does not authorize this render",
        ));
    }
    let root = open_ledger_root().map_err(|_| {
        (
            2,
            "refused",
            "crop_not_charged",
            "the crop charge is not trusted",
        )
    })?;
    let secret = crop_secret(&root).map_err(|_| {
        (
            2,
            "refused",
            "crop_not_charged",
            "the crop charge is not trusted",
        )
    })?;
    if crop_proof(&secret, content_sha, page, count) != *proof {
        return Err((
            2,
            "refused",
            "crop_not_charged",
            "the crop charge is not valid",
        ));
    }
    // The proof is only a claim: the one-use charge must still exist, and spending it here is what
    // stops a replay from rendering the same page again without another count.
    consume_crop_charge(&root, content_sha, page, count).map_err(|_| {
        (
            2,
            "refused",
            "crop_not_charged",
            "the crop charge has already been spent",
        )
    })?;
    Ok((*content_sha).to_string())
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

    // The proof is read from the trusted root before the worker locks itself down. A direct worker
    // invocation cannot read that root and has no proof from a parent.
    let crop_content_sha = if args.command == Command_::Crop {
        match worker_crop_charge(&args) {
            Ok(sha) => Some(sha),
            Err((code, status, reason, message)) => send_failure(code, status, reason, message),
        }
    } else {
        None
    };

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
    // The bytes just read must be the ones the parent hashed when it charged the crop.
    if let Some(charged_sha) = crop_content_sha.as_deref() {
        if sha256_hex(&data) != charged_sha {
            send_failure(
                2,
                "refused",
                "crop_changed",
                "the crop input changed after it was charged",
            );
        }
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

    #[cfg(unix)]
    #[test]
    fn crop_ledger_owner_check_accepts_root_or_current_user_only() {
        let euid = rustix::process::geteuid().as_raw();
        let other = if euid == 1001 { 1002 } else { 1001 };
        assert!(trusted_dir_parts(true, 0, euid, 0o700), "root-owned");
        assert!(trusted_dir_parts(true, euid, euid, 0o700), "service-owned");
        assert!(trusted_dir_parts(true, euid, euid, 0o750), "readable group");
        assert!(!trusted_dir_parts(true, other, euid, 0o700), "another user");
        assert!(!trusted_dir_parts(true, 0, euid, 0o770), "group write");
        assert!(!trusted_dir_parts(true, euid, euid, 0o707), "other write");
        assert!(
            !trusted_dir_parts(false, euid, euid, 0o700),
            "not a directory"
        );
    }

    #[cfg(unix)]
    #[test]
    fn crop_key_owner_check_accepts_root_or_current_user_only() {
        let euid = rustix::process::geteuid().as_raw();
        let other = if euid == 1001 { 1002 } else { 1001 };
        assert!(trusted_key_parts(true, 0, euid, 0o600), "root-owned");
        assert!(trusted_key_parts(true, euid, euid, 0o600), "service-owned");
        assert!(trusted_key_parts(true, euid, euid, 0o400), "read-only key");
        assert!(!trusted_key_parts(true, other, euid, 0o600), "another user");
        assert!(!trusted_key_parts(true, 0, euid, 0o660), "group write");
        assert!(!trusted_key_parts(true, euid, euid, 0o606), "other write");
        assert!(!trusted_key_parts(true, euid, euid, 0o700), "owner execute");
        assert!(!trusted_key_parts(false, euid, euid, 0o600), "not a file");
    }
}
