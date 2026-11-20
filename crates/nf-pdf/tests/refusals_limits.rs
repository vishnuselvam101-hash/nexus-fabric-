//! Every refusal code, and every named limit. Limits are lowered where the default
//! would need a large fixture; the compression-ratio and form-depth cases use defaults.

mod common;

use std::time::Duration;

use common::*;
use nf_pdf::{convert, FailureKind, Limits, Options, Scale};

fn refused(bytes: &[u8], opts: &Options) -> (&'static str, String) {
    let f = convert_err(bytes, opts);
    assert_eq!(f.kind, FailureKind::Refused, "{}", f.message);
    assert_eq!(f.exit_code(), 2);
    (f.code, f.message)
}

fn limited(bytes: &[u8], opts: &Options) -> &'static str {
    let f = convert_err(bytes, opts);
    assert_eq!(
        f.kind,
        FailureKind::LimitExceeded,
        "{}: {}",
        f.code,
        f.message
    );
    assert_eq!(f.exit_code(), 3);
    assert_eq!(f.code, "limit_exceeded");
    f.limit.expect("a limit name")
}

fn opts_with(limits: impl FnOnce(&mut Limits)) -> Options {
    let mut o = options(1.0);
    limits(&mut o.limits);
    o
}

/// A page whose content stream has the given dictionary entries and data, unfiltered.
fn page_with_content(content_dict: &str, data: &[u8]) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog = pdf.reserve();
    let pages = pdf.reserve();
    let page = pdf.reserve();
    let content = pdf.stream(content_dict, data, false);
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
    pdf.classic(catalog)
}

fn image_page(extra_content: &str) -> Vec<u8> {
    let content = format!("q 200 0 0 200 0 0 cm /Im1 Do Q {extra_content}");
    page_with_resources(content.as_bytes(), [0.0, 0.0, 200.0, 200.0], |pdf| {
        let img = pdf.stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            &[0u8],
            false,
        );
        format!("/XObject << /Im1 {img} 0 R >>")
    })
}

#[test]
fn refuses_encrypted_files() {
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
    pdf.trailer_extra = "/Encrypt << /Filter /Standard /V 1 >>".into();
    let (code, _) = refused(&pdf.classic(catalog), &options(1.0));
    assert_eq!(code, "encrypted");
}

#[test]
fn refuses_malformed_structure() {
    let (code, msg) = refused(b"%PDF-1.4\nthis is not a document", &options(1.0));
    assert_eq!(code, "malformed");
    assert!(msg.starts_with("the PDF structure cannot be read"));
}

#[test]
fn refuses_unsupported_stream_filters() {
    let bytes = page_with_content("/Filter /DCTDecode", b"\xFF\xD8\xFF\xD9");
    let (code, msg) = refused(&bytes, &options(1.0));
    assert_eq!(code, "unsupported_filter");
    assert!(msg.contains("DCTDecode"), "{msg}");
}

#[test]
fn refuses_page_out_of_range() {
    let mut o = options(1.0);
    o.page = 2;
    let (code, msg) = refused(&page_pdf(&PageSpec::new(b"0 0 m 1 1 l S")), &o);
    assert_eq!(code, "page_out_of_range");
    assert!(msg.contains("the document has 1 page"), "{msg}");
}

#[test]
fn refuses_scanned_raster_pages() {
    let (code, msg) = refused(&image_page(""), &options(1.0));
    assert_eq!(code, "raster_page");
    assert!(msg.contains("cannot be converted reliably"), "{msg}");
    // A few paths do not make a drawing: still a scan.
    let (code, _) = refused(&image_page("0 0 m 1 1 l S 0 0 m 2 2 l S"), &options(1.0));
    assert_eq!(code, "raster_page");
}

#[test]
fn an_image_beside_many_paths_is_not_refused() {
    let mut paths = String::new();
    for i in 0..12 {
        paths.push_str(&format!("{i} 0 m {i} 5 l S "));
    }
    let c = convert(&image_page(&paths), &options(1.0)).expect("converts");
    assert_eq!(c.report.stats.get("images"), 1);
}

#[test]
fn refuses_pages_without_vector_paths() {
    let (code, _) = refused(&page_pdf(&PageSpec::new(b"BT ET")), &options(1.0));
    assert_eq!(code, "no_vector_paths");
    let mut o = options(1.0);
    o.include_fill = false;
    let (code, msg) = refused(&page_pdf(&PageSpec::new(b"0 0 m 5 0 l 5 5 l h f")), &o);
    assert_eq!(code, "no_vector_paths");
    assert!(msg.contains("--no-fill"), "{msg}");
}

#[test]
fn refuses_invalid_options() {
    let pdf = page_pdf(&PageSpec::new(b"0 0 m 1 1 l S"));
    for mm in [0.0, -1.0, f64::NAN, f64::INFINITY, 2e6] {
        let (code, _) = refused(&pdf, &Options::new(Scale::MmPerPoint(mm)));
        assert_eq!(code, "invalid_scale", "mm per point {mm}");
    }
    let (code, _) = refused(&pdf, &Options::new(Scale::Ratio(-10.0)));
    assert_eq!(code, "invalid_scale");
    for tol in [0.0, -0.1, 1000.0, f64::NAN] {
        let mut o = options(1.0);
        o.tolerance_mm = tol;
        let (code, _) = refused(&pdf, &o);
        assert_eq!(code, "invalid_tolerance", "tolerance {tol}");
    }
    let mut o = options(1.0);
    o.page = 0;
    let (code, _) = refused(&pdf, &o);
    assert_eq!(code, "invalid_page");
}

#[test]
fn limit_input_bytes() {
    let pdf = page_pdf(&PageSpec::new(b"0 0 m 1 1 l S"));
    assert_eq!(
        limited(&pdf, &opts_with(|l| l.max_input_bytes = 10)),
        "input_bytes"
    );
}

#[test]
fn limit_decoded_bytes_total_budget() {
    let content = "0 0 m 1 1 l S\n".repeat(400);
    let mut spec = PageSpec::new(content.as_bytes());
    spec.flate = true;
    let bytes = page_pdf(&spec);
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_decoded_bytes = 1000)),
        "decoded_bytes"
    );
}

#[test]
fn limit_compression_ratio_with_default_limits() {
    // 8 MiB of zeros compresses to a few kilobytes: far beyond 100:1 and above the 4 MiB check.
    let zeros = vec![0u8; 8 << 20];
    let bytes = page_pdf(&PageSpec {
        flate: true,
        ..PageSpec::new(&zeros)
    });
    assert_eq!(limited(&bytes, &options(1.0)), "compression_ratio");
}

#[test]
fn limit_objects() {
    let bytes = page_pdf(&PageSpec::new(b"0 0 m 1 1 l S"));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_objects = 2)),
        "objects"
    );
}

#[test]
fn limit_operators() {
    let bytes = page_pdf(&PageSpec::new(b"0 0 m 1 1 l 2 2 l 3 3 l S"));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_operators = 3)),
        "operators"
    );
}

#[test]
fn limit_path_segments() {
    let bytes = page_pdf(&PageSpec::new(b"0 0 m 1 1 l 2 2 l 3 3 l 4 4 l 5 5 l S"));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_path_segments = 2)),
        "path_segments"
    );
}

#[test]
fn limit_graphics_nesting() {
    let bytes = page_pdf(&PageSpec::new(b"q q q Q Q Q 0 0 m 1 1 l S"));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_graphics_depth = 2)),
        "graphics_depth"
    );
}

#[test]
fn limit_object_nesting_in_operands() {
    let bytes = page_pdf(&PageSpec::new(b"[[[1]]] TJ 0 0 m 1 1 l S"));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_nesting = 2)),
        "nesting"
    );
}

#[test]
fn limit_form_depth_on_a_self_referencing_form() {
    // A form whose content invokes itself must stop at the depth limit, with default limits.
    let mut pdf = Pdf::new();
    let catalog = pdf.reserve();
    let pages = pdf.reserve();
    let page = pdf.reserve();
    let form = pdf.reserve();
    let body = b"/Fm1 Do";
    pdf.set(
        form,
        format!(
            "<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /XObject << /Fm1 {form} 0 R >> >> /Length {} >>\nstream\n{}\nendstream",
            body.len(),
            String::from_utf8_lossy(body)
        ),
    );
    let content = pdf.stream("", b"/Fm1 Do 0 0 m 1 1 l S", false);
    pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
    pdf.set(
        pages,
        format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>"),
    );
    pdf.set(
        page,
        format!(
            "<< /Type /Page /Parent {pages} 0 R /MediaBox [0 0 200 200] /Contents {content} 0 R /Resources << /XObject << /Fm1 {form} 0 R >> >> >>"
        ),
    );
    assert_eq!(limited(&pdf.classic(catalog), &options(1.0)), "form_depth");
}

#[test]
fn limit_text_items() {
    let bytes = page_pdf(&PageSpec::new(
        b"BT 0 0 Td (a) Tj (b) Tj (c) Tj ET 0 0 m 1 1 l S",
    ));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_text_items = 2)),
        "text_items"
    );
}

#[test]
fn limit_output_bytes() {
    let bytes = page_pdf(&PageSpec::new(b"0 0 m 1 1 l S"));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_output_bytes = 100)),
        "output_bytes"
    );
}

#[test]
fn limit_xref_sections() {
    let bytes = page_pdf(&PageSpec::new(b"0 0 m 1 1 l S"));
    assert_eq!(
        limited(&bytes, &opts_with(|l| l.max_xref_sections = 0)),
        "xref_sections"
    );
}

#[test]
fn limit_wall_clock_ends_in_a_refusal_not_a_hang() {
    let bytes = page_pdf(&PageSpec::new(b"0 0 m 1 1 l S"));
    let mut o = options(1.0);
    o.time_limit = Duration::ZERO;
    assert_eq!(limited(&bytes, &o), "wall_clock");
}

#[test]
fn pathological_nesting_ends_quickly_with_default_limits() {
    let content = "[".repeat(200_000);
    let started = std::time::Instant::now();
    let bytes = page_pdf(&PageSpec::new(content.as_bytes()));
    let f = convert_err(&bytes, &options(1.0));
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(f.kind == FailureKind::Refused || f.kind == FailureKind::LimitExceeded);
}
