//! Round trip through the nf-dxf reader, determinism, the report, and the command line.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use nf_pdf::{convert, Options, Scale};

fn sample_page() -> Vec<u8> {
    let content = b"10 10 m 60 10 l 60 40 l S 0 0 m 10 0 10 10 0 10 c s 5 5 20 15 re f \
        [4 2] 0 d 0 50 m 40 50 l S BT /F1 9 Tf 10 80 Td (PART 7) Tj ET";
    page_with_resources(content, [0.0, 0.0, 200.0, 200.0], |pdf| {
        let f = pdf.add(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        );
        format!("/Font << /F1 {f} 0 R >>")
    })
}

#[test]
fn same_input_gives_byte_identical_dxf_and_report() {
    let pdf = sample_page();
    let a = convert(&pdf, &options(0.5)).expect("converts");
    let b = convert(&pdf, &options(0.5)).expect("converts");
    assert_eq!(a.dxf, b.dxf);
    assert_eq!(a.report.to_json(), b.report.to_json());
}

#[test]
fn dxf_re_reads_with_the_same_geometry() {
    let c = convert_ok(&sample_page(), 1.0);
    let stroke = polylines(&c.dxf, "PDF_STROKE");
    assert_eq!(
        stroke.len(),
        2,
        "two stroked subpaths: the open L and the rectangle-less curve path"
    );
    assert_pt(stroke[0].0[0], (10.0, 10.0));
    assert_pt(stroke[0].0[1], (60.0, 10.0));
    assert_pt(stroke[0].0[2], (60.0, 40.0));
    assert!(!stroke[0].1);
    // The 'c' curve closed with 's': closed flag set, first and last vertices equal in the file.
    assert!(stroke[1].1, "s closes the subpath");
    let fill = polylines(&c.dxf, "PDF_FILL");
    assert_eq!(fill.len(), 1);
    assert!(fill[0].1);
    let dashed = polylines(&c.dxf, "PDF_DASHED");
    // [4 2] on a 40-unit line: on-pieces 0-4, 6-10, ..., 36-40.
    assert_eq!(
        dashed.len(),
        7,
        "[4 2] on a 40-unit line gives seven on-pieces"
    );
    assert_pt(dashed[0].0[0], (0.0, 50.0));
    assert_pt(dashed[0].0[1], (4.0, 50.0));
    assert_pt(dashed[6].0[1], (40.0, 50.0));
    let text = texts(&c.dxf, "PDF_TEXT");
    assert_eq!(text.len(), 1);
    assert_eq!(text[0].2, "PART 7");
    assert!(close(text[0].1, 9.0));
}

#[test]
fn header_comments_and_release_marking_are_written() {
    let c = convert_ok(&sample_page(), 0.5);
    let head = std::str::from_utf8(&c.dxf).expect("ascii");
    for key in [
        "999\nNF-SOURCE: pdf\n",
        "999\nNF-SOURCE-SHA256: ",
        "999\nNF-PAGE: 1\n",
        "999\nNF-SCALE-MM-PER-POINT: 0.500000000\n",
        "999\nNF-UNITS: mm\n",
        "999\nNF-CHORD-TOLERANCE-MM: 0.010000\n",
        "999\nNF-CONVERTER-VERSION: ",
    ] {
        assert!(head.contains(key), "missing {key:?}");
    }
    assert!(
        head.starts_with("999\n"),
        "comments come before the first section"
    );
    let marks = texts(&c.dxf, "NF_NOT_RELEASED");
    assert_eq!(marks.len(), 2);
    assert_eq!(
        marks[0].2,
        "CONVERTED FROM PDF - NOT VERIFIED - NOT RELEASED"
    );
    assert!(marks[1].2.contains("MM PER PDF POINT"));
    assert!(head.contains("AC1009"));
}

#[test]
fn report_states_counts_bounds_and_notes() {
    let c = convert_ok(&sample_page(), 1.0);
    let json = c.report.to_json();
    for needle in [
        "\"status\": \"converted\"",
        "\"converter_version\": \"0.1.0\"",
        "\"dxf_sha256\": \"",
        "\"bbox_mm\": [",
        "\"layers\": [\"PDF_STROKE\"",
        "\"stroke_subpaths\": ",
        "\"dash_pieces\": ",
        "\"text_skipped_by_reason\": {",
        "\"notes\": [",
    ] {
        assert!(json.contains(needle), "report lacks {needle}");
    }
    assert!(json.is_ascii());
    let bbox = c.report.bbox_mm.expect("geometry extent");
    assert!(close(bbox[0], 0.0) && close(bbox[2], 60.0), "{bbox:?}");
}

#[test]
fn report_describes_refusals() {
    let mut o = Options::new(Scale::MmPerPoint(1.0));
    o.page = 9;
    let f = convert(&sample_page(), &o).expect_err("out of range");
    let json = f.report.to_json();
    assert!(json.contains("\"status\": \"refused\""));
    assert!(json.contains("\"code\": \"page_out_of_range\""));
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nf-pdf2dxf")
}

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).expect("work dir");
    dir
}

fn run_cli(dir: &Path, input: &Path, extra: &[&str]) -> (i32, String) {
    let out = dir.join("out.dxf");
    let report = dir.join("out.json");
    let _ = std::fs::remove_file(&out);
    let mut cmd = Command::new(bin());
    cmd.arg("--in")
        .arg(input)
        .arg("--out")
        .arg(&out)
        .arg("--report")
        .arg(&report)
        .args(extra)
        .env_clear();
    let result = cmd.output().expect("the binary runs");
    let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
    (result.status.code().unwrap_or(-1), stderr)
}

#[test]
fn cli_converts_and_writes_dxf_and_report() {
    let dir = work_dir("cli_ok");
    let input = dir.join("in.pdf");
    std::fs::write(&input, sample_page()).expect("write input");
    let (code, stderr) = run_cli(&dir, &input, &["--mm-per-point", "1.0"]);
    assert_eq!(code, 0, "{stderr}");
    let dxf = std::fs::read(dir.join("out.dxf")).expect("dxf written");
    let lib = convert_ok(&sample_page(), 1.0);
    assert_eq!(
        dxf, lib.dxf,
        "the program and the library agree byte for byte"
    );
    let json = std::fs::read_to_string(dir.join("out.json")).expect("report written");
    assert!(json.contains("\"status\": \"converted\""));
}

#[test]
fn cli_scale_is_required_and_not_guessed() {
    let dir = work_dir("cli_scale");
    let input = dir.join("in.pdf");
    std::fs::write(&input, sample_page()).expect("write input");
    let (code, stderr) = run_cli(&dir, &input, &[]);
    assert_eq!(code, 2);
    assert!(stderr.contains("\"code\": \"scale_required\""), "{stderr}");
    assert!(!dir.join("out.dxf").exists(), "no DXF after a refusal");
    let (code, stderr) = run_cli(&dir, &input, &["--mm-per-point", "1", "--scale", "1:10"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("scale_ambiguous"), "{stderr}");
    let (code, _) = run_cli(&dir, &input, &["--scale", "10"]);
    assert_eq!(code, 2);
}

#[test]
fn cli_refusal_writes_report_and_no_dxf() {
    let dir = work_dir("cli_refused");
    let input = dir.join("in.pdf");
    let mut pdf = Pdf::new();
    let catalog = pdf.reserve();
    let pages = pdf.reserve();
    let page = pdf.reserve();
    let content = pdf.stream("", b"0 0 m 1 1 l S", false);
    pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
    pdf.set(
        pages,
        format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>"),
    );
    pdf.set(
        page,
        format!(
            "<< /Type /Page /Parent {pages} 0 R /MediaBox [0 0 200 200] /Contents {content} 0 R >>"
        ),
    );
    pdf.trailer_extra = "/Encrypt << /Filter /Standard >>".into();
    std::fs::write(&input, pdf.classic(catalog)).expect("write input");
    let (code, stderr) = run_cli(&dir, &input, &["--mm-per-point", "1"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("\"code\": \"encrypted\""), "{stderr}");
    assert!(!dir.join("out.dxf").exists());
    let json = std::fs::read_to_string(dir.join("out.json")).expect("report written");
    assert!(json.contains("\"status\": \"refused\""));
}

#[test]
fn cli_limit_exceeded_exits_with_three() {
    let dir = work_dir("cli_limit");
    let input = dir.join("in.pdf");
    let content = "[".repeat(100_000);
    std::fs::write(&input, page_pdf(&PageSpec::new(content.as_bytes()))).expect("write input");
    let (code, stderr) = run_cli(&dir, &input, &["--mm-per-point", "1"]);
    assert_eq!(code, 3, "{stderr}");
    assert!(
        stderr.contains("\"status\": \"limit_exceeded\""),
        "{stderr}"
    );
    assert!(!dir.join("out.dxf").exists());
}

#[test]
fn cli_rejects_missing_arguments() {
    let output = Command::new(bin())
        .arg("--in")
        .arg("x.pdf")
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid_arguments"));
}

// ---- Work plan 3.1: --no-text, and the input limit at its real value ----

#[test]
fn no_text_option_writes_no_text_and_counts_it_as_not_requested() {
    let mut o = options(1.0);
    o.include_text = false;
    let c = convert(&sample_page(), &o).expect("converts");
    assert!(texts(&c.dxf, "PDF_TEXT").is_empty(), "no TEXT on PDF_TEXT");
    assert_eq!(
        polylines(&c.dxf, "PDF_STROKE").len(),
        2,
        "geometry is unchanged"
    );
    assert_eq!(
        c.report.stats.text_skipped.get("text_not_requested"),
        Some(&1)
    );
    let json = c.report.to_json();
    assert!(json.contains("\"include_text\": false"), "{json}");
    // The release marking is not text of the drawing and stays.
    assert_eq!(texts(&c.dxf, "NF_NOT_RELEASED").len(), 2);
}

#[test]
fn cli_no_text_flag_removes_text_entities() {
    let dir = work_dir("cli_no_text");
    let input = dir.join("in.pdf");
    std::fs::write(&input, sample_page()).expect("write input");
    let (code, stderr) = run_cli(&dir, &input, &["--mm-per-point", "1.0", "--no-text"]);
    assert_eq!(code, 0, "{stderr}");
    let dxf = std::fs::read(dir.join("out.dxf")).expect("dxf written");
    assert!(
        texts(&dxf, "PDF_TEXT").is_empty(),
        "--no-text writes no PDF_TEXT"
    );
    let mut o = options(1.0);
    o.include_text = false;
    assert_eq!(dxf, convert(&sample_page(), &o).expect("converts").dxf);
    let json = std::fs::read_to_string(dir.join("out.json")).expect("report written");
    assert!(json.contains("\"include_text\": false"), "{json}");
    assert!(json.contains("\"text_not_requested\": 1"), "{json}");
    // Without the flag the same file has its text.
    let (code, _) = run_cli(&dir, &input, &["--mm-per-point", "1.0"]);
    assert_eq!(code, 0);
    let dxf = std::fs::read(dir.join("out.dxf")).expect("dxf written");
    assert_eq!(texts(&dxf, "PDF_TEXT").len(), 1);
}

/// A directory under the system temporary directory, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("nf-pdf-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_input_limit_is_64_mib_exactly() {
    let limit = nf_pdf::Limits::default().max_input_bytes;
    assert_eq!(limit, 64 << 20);
    let tmp = TempDir::new("input-limit");
    let input = tmp.0.join("in.pdf");
    std::fs::write(&input, page_of_exact_size(limit)).expect("write input");
    assert_eq!(std::fs::metadata(&input).expect("size").len(), limit as u64);
    let (code, stderr) = run_cli(&tmp.0, &input, &["--mm-per-point", "1"]);
    assert_eq!(code, 0, "a file of exactly 64 MiB converts: {stderr}");
    let json = std::fs::read_to_string(tmp.0.join("out.json")).expect("report written");
    assert!(json.contains(&format!("\"bytes\": {limit}")), "{json}");
    std::fs::write(&input, page_of_exact_size(limit + 1)).expect("write input");
    assert_eq!(
        std::fs::metadata(&input).expect("size").len(),
        limit as u64 + 1
    );
    let (code, stderr) = run_cli(&tmp.0, &input, &["--mm-per-point", "1"]);
    assert_eq!(code, 3, "{stderr}");
    assert!(stderr.contains("\"limit\": \"input_bytes\""), "{stderr}");
    assert!(!tmp.0.join("out.dxf").exists(), "no DXF after a limit");
}
