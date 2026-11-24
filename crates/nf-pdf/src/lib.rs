//! Offline converter from vector PDF pages to DXF R12 (AC1009).
//!
//! The converter is deterministic and makes no network access. It reads one page of a
//! PDF, interprets its vector paths and (optionally) its text, and writes millimetres.
//! It never guesses a scale: the caller must give millimetres per PDF point, or a
//! drawing scale 1:N (one PDF point of paper is 25.4/72 mm, so `mm per point = 25.4/72 * N`).
//!
//! Threat model. The converter parses untrusted bytes. Every bound it enforces is in
//! [`Limits`]; every failure is a [`Failure`] (refusal or limit), never a panic, and
//! no output is produced on failure. It must be run as a separate process with no
//! network and resource limits; see `docs/backend/PDF_TO_DXF.md`.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod doc;
mod dxf;
mod filters;
mod font;
mod geom;
mod interp;
mod lexer;
mod limits;
mod page;
mod report;
mod text;

use std::time::Duration;

use sha2::{Digest, Sha256};

pub use limits::Limits;
pub use report::{Report, Stats};

use doc::Doc;
use dxf::{Header, LAYER_NOTE};
use limits::{Deadline, Fault};

/// Version of the converter, written into every DXF and report.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// A page with a page-covering image and at most this many painted paths is refused as raster.
pub const RASTER_MAX_PATHS: u64 = 10;
/// Default chord tolerance for curve flattening, millimetres.
pub const DEFAULT_TOLERANCE_MM: f64 = 0.01;
/// Points per inch (PDF user space is 1/72 inch by default).
pub const POINTS_PER_INCH: f64 = 72.0;
pub const MM_PER_INCH: f64 = 25.4;

/// How PDF units become millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    /// Millimetres of drawing per PDF point (1/72 inch of paper).
    MmPerPoint(f64),
    /// Drawing scale 1:N as printed on paper.
    Ratio(f64),
}

impl Scale {
    /// Millimetres per PDF point. A ratio 1:N gives `25.4 / 72 * N`.
    pub fn mm_per_point(self) -> f64 {
        match self {
            Scale::MmPerPoint(v) => v,
            Scale::Ratio(n) => MM_PER_INCH / POINTS_PER_INCH * n,
        }
    }
}

/// Settings for one conversion.
#[derive(Clone, Debug)]
pub struct Options {
    /// Page number, starting at 1.
    pub page: u32,
    pub scale: Scale,
    /// Chord tolerance for curve flattening, millimetres (0.000001 to 100).
    pub tolerance_mm: f64,
    /// Write filled-only paths on PDF_FILL. When false they are counted and not written.
    pub include_fill: bool,
    /// Write TEXT entities. When false text is counted as not requested.
    pub include_text: bool,
    pub limits: Limits,
    /// Wall-clock budget for the whole conversion.
    pub time_limit: Duration,
}

impl Options {
    pub fn new(scale: Scale) -> Self {
        Options {
            page: 1,
            scale,
            tolerance_mm: DEFAULT_TOLERANCE_MM,
            include_fill: true,
            include_text: true,
            limits: Limits::default(),
            time_limit: Duration::from_secs(30),
        }
    }
}

/// A successful conversion: the DXF bytes and the report.
#[derive(Debug)]
pub struct Converted {
    pub dxf: Vec<u8>,
    pub report: Report,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// The input cannot be converted (exit code 2).
    Refused,
    /// A limit was exceeded (exit code 3).
    LimitExceeded,
}

/// A failed conversion. No DXF is produced.
#[derive(Debug)]
pub struct Failure {
    pub kind: FailureKind,
    /// Machine-readable code, e.g. "encrypted", "raster_page", "limit_exceeded".
    pub code: &'static str,
    /// For `LimitExceeded`: the name of the limit.
    pub limit: Option<&'static str>,
    pub message: String,
    pub report: Report,
}

impl Failure {
    pub fn exit_code(&self) -> i32 {
        match self.kind {
            FailureKind::Refused => 2,
            FailureKind::LimitExceeded => 3,
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pdf_version(input: &[u8]) -> Option<String> {
    let head = input.get(..input.len().min(1024))?;
    let at = doc::find(head, 0, b"%PDF-")?;
    let v = head.get(at + 5..at + 8)?;
    let ok = matches!(v, [a, b'.', c] if a.is_ascii_digit() && c.is_ascii_digit());
    ok.then(|| String::from_utf8_lossy(v).into_owned())
}

fn blank_report(sha256: String, bytes: usize, opts: &Options) -> Report {
    let (scale_kind, ratio) = match opts.scale {
        Scale::MmPerPoint(_) => ("mm_per_point", None),
        Scale::Ratio(n) => ("ratio", Some(n)),
    };
    Report {
        status: "refused",
        code: None,
        limit: None,
        message: None,
        sha256,
        input_bytes: bytes,
        pdf_version: None,
        page: opts.page,
        page_count: None,
        mm_per_point: opts.scale.mm_per_point(),
        scale_kind,
        ratio,
        user_unit: 1.0,
        rotate: 0,
        visible_box_user: None,
        page_size_mm: None,
        tolerance_mm: opts.tolerance_mm,
        include_fill: opts.include_fill,
        include_text: opts.include_text,
        stats: Stats::default(),
        bbox_mm: None,
        dxf_bytes: 0,
        dxf_sha256: None,
        isolation: Vec::new(),
    }
}

fn check_options(opts: &Options) -> Result<(), Fault> {
    if opts.page == 0 {
        return Err(Fault::Refused(
            "invalid_page",
            "page numbers start at 1".into(),
        ));
    }
    let mm = opts.scale.mm_per_point();
    if !(mm.is_finite() && mm > 0.0 && mm <= 1e6) {
        return Err(Fault::Refused(
            "invalid_scale",
            "the scale must give between 0 and 1e6 millimetres per PDF point".into(),
        ));
    }
    let tol = opts.tolerance_mm;
    if !(tol.is_finite() && (1e-6..=100.0).contains(&tol)) {
        return Err(Fault::Refused(
            "invalid_tolerance",
            "the chord tolerance must be between 0.000001 and 100 millimetres".into(),
        ));
    }
    Ok(())
}

/// Converts one page. On failure nothing is produced except the report.
pub fn convert(input: &[u8], opts: &Options) -> Result<Converted, Box<Failure>> {
    let mut rep = blank_report(sha256_hex(input), input.len(), opts);
    let outcome = check_options(opts).and_then(|()| run(input, opts, &mut rep));
    match outcome {
        Ok(dxf) => {
            rep.status = "converted";
            rep.dxf_bytes = dxf.len();
            rep.dxf_sha256 = Some(sha256_hex(&dxf));
            Ok(Converted { dxf, report: rep })
        }
        Err(fault) => Err(Box::new(to_failure(fault, rep))),
    }
}

fn to_failure(fault: Fault, mut rep: Report) -> Failure {
    let (kind, code, limit, message) = match fault {
        Fault::Malformed(m) => (
            FailureKind::Refused,
            "malformed",
            None,
            format!("the PDF structure cannot be read: {m}"),
        ),
        Fault::Unsupported(m) => (
            FailureKind::Refused,
            "unsupported_filter",
            None,
            format!("{m} is not supported"),
        ),
        Fault::Encrypted => (
            FailureKind::Refused,
            "encrypted",
            None,
            "the PDF is encrypted; encrypted files are not supported".into(),
        ),
        Fault::Refused(code, m) => (FailureKind::Refused, code, None, m),
        Fault::Limit(name, m) => (FailureKind::LimitExceeded, "limit_exceeded", Some(name), m),
        Fault::Timeout => (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("wall_clock"),
            "the conversion ran out of time".into(),
        ),
    };
    rep.status = match kind {
        FailureKind::Refused => "refused",
        FailureKind::LimitExceeded => "limit_exceeded",
    };
    rep.code = Some(code);
    rep.limit = limit;
    rep.message = Some(message.clone());
    Failure {
        kind,
        code,
        limit,
        message,
        report: rep,
    }
}

/// The conversion pipeline. Fills `rep` as facts become known.
fn run(input: &[u8], opts: &Options, rep: &mut Report) -> Result<Vec<u8>, Fault> {
    if input.len() > opts.limits.max_input_bytes {
        return Err(Fault::Limit(
            "input_bytes",
            "the input exceeds the size limit".into(),
        ));
    }
    rep.pdf_version = pdf_version(input);
    let mut doc = Doc::open(input, opts.limits.clone(), Deadline::after(opts.time_limit))?;
    let sel = page::select(&mut doc, opts.page)?;
    rep.page_count = Some(sel.count);
    let Some(info) = sel.page else {
        return Err(Fault::Refused(
            "page_out_of_range",
            format!(
                "page {} was requested; the document has {} page(s)",
                opts.page, sel.count
            ),
        ));
    };
    let geo = page::geometry(&mut doc, &info, &mut rep.stats)?;
    rep.user_unit = geo.user_unit;
    rep.rotate = geo.rotate;
    rep.visible_box_user = Some(geo.visible);
    let mm = opts.scale.mm_per_point();
    let (ctm, page_mm) = page::transform(&geo, mm);
    rep.page_size_mm = Some(page_mm);
    let res = page::resources(&mut doc, &info)?;
    let content = page::contents(&mut doc, &info)?;
    let annots = page::annotation_count(&mut doc, &info)?;
    if annots > 0 {
        rep.stats.count("annotations", annots as u64);
        rep.stats
            .warn("annotations_not_converted", "annotations are not converted");
    }
    let run_opts = interp::RunOpts {
        tol_mm: opts.tolerance_mm,
        fill: opts.include_fill,
        text: opts.include_text,
    };
    let mut it = interp::Interp::new(&mut doc, run_opts, opts.limits.max_output_bytes, ctm, res);
    it.stats = std::mem::take(&mut rep.stats);
    let outcome = it.run(&content);
    let parts = it.finish();
    rep.stats = parts.stats;
    outcome?;

    let (w, h) = page_mm;
    let page_area = w * h;
    let covering = parts.images.iter().any(|b| {
        let (x0, y0) = (b[0].max(0.0), b[1].max(0.0));
        let (x1, y1) = (b[2].min(w), b[3].min(h));
        x1 > x0 && y1 > y0 && (x1 - x0) * (y1 - y0) >= 0.5 * page_area
    });
    if covering && parts.painted <= RASTER_MAX_PATHS {
        return Err(Fault::Refused(
            "raster_page",
            format!(
                "the page is a scanned or raster drawing: it has an image covering the page and {} vector path(s). Scanned drawings cannot be converted reliably and are not supported.",
                parts.painted
            ),
        ));
    }
    let mut body = parts.out;
    if body.polylines == 0 {
        let hint = if opts.include_fill {
            ""
        } else {
            " (filled-only paths were excluded by --no-fill)"
        };
        return Err(Fault::Refused(
            "no_vector_paths",
            format!("the page has no vector paths to convert{hint}"),
        ));
    }
    rep.bbox_mm = Some([body.min.0, body.min.1, body.max.0, body.max.1]);

    // The release-marking notes sit above the visible page, on the non-cutting layer.
    let note_h = (w.max(h) / 50.0).max(1e-3);
    let ratio_text = match opts.scale {
        Scale::Ratio(n) => format!("1:{n}"),
        Scale::MmPerPoint(_) => "GIVEN AS MM PER POINT".to_string(),
    };
    body.text(
        LAYER_NOTE,
        (0.0, h + 2.0 * note_h),
        note_h,
        0.0,
        1.0,
        "CONVERTED FROM PDF - NOT VERIFIED - NOT RELEASED",
    )?;
    body.text(
        LAYER_NOTE,
        (0.0, h + 3.5 * note_h),
        note_h,
        0.0,
        1.0,
        &format!(
            "SCALE {ratio_text} - {:.9} MM PER PDF POINT - UNITS: MILLIMETRES",
            mm
        ),
    )?;

    let skipped: u64 = rep.stats.text_skipped.values().sum();
    let header = Header {
        comments: vec![
            ("NF-SOURCE".into(), "pdf".into()),
            ("NF-SOURCE-SHA256".into(), rep.sha256.clone()),
            ("NF-PAGE".into(), opts.page.to_string()),
            ("NF-SCALE-MM-PER-POINT".into(), format!("{mm:.9}")),
            ("NF-UNITS".into(), "mm".into()),
            (
                "NF-CHORD-TOLERANCE-MM".into(),
                format!("{:.6}", opts.tolerance_mm),
            ),
            ("NF-CONVERTER-VERSION".into(), VERSION.into()),
            ("NF-USER-UNIT".into(), format!("{}", geo.user_unit)),
            ("NF-PATHS-PAINTED".into(), parts.painted.to_string()),
            ("NF-SEGMENTS".into(), parts.segments.to_string()),
            ("NF-TEXT-SKIPPED".into(), skipped.to_string()),
        ],
    };
    let text = dxf::assemble(&body, &header);
    if text.len() > opts.limits.max_output_bytes {
        return Err(Fault::Limit(
            "output_bytes",
            "the DXF output exceeds the size limit".into(),
        ));
    }
    Ok(text.into_bytes())
}
