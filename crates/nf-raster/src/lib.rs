//! Page images from PDF pages: scanned pages (a page-covering image: DCT, CCITT G3/G4, JBIG2,
//! Flate and the other plain filters), several images composited, MediaBox/CropBox and
//! /Rotate handled, and vector-only pages line-rendered as a fallback (no fonts, no fills,
//! no clipping: lines only).
//!
//! The output is a PNG (8-bit gray or RGB) of the upright page at the requested resolution.
//! It is an image of the paper, not a measurement: the page's size in points is reported so
//! that the caller can state the scale.
//!
//! Threat model. The library parses untrusted bytes. Every failure is a [`Failure`] (a coded
//! refusal or a limit), never a panic, and nothing is produced on failure. Run it as a
//! separate process with no network and resource limits: the program `nf-raster` does this
//! (see `docs/backend/RASTER.md`).

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod decode;
mod draw;
mod png;
pub mod report;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use nf_pdf::raster::{Fault, ImageSrc, Item, Limits, PdfDoc, Scene};
use sha2::{Digest, Sha256};

pub use report::{Info, PageKind, PageSummary, Rendered};

/// Version of the program, written into every report.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Resolution range accepted (dots per inch).
pub const MIN_DPI: u32 = 30;
pub const MAX_DPI: u32 = 1200;
/// Default resolution.
pub const DEFAULT_DPI: u32 = 200;
/// A page with a page-covering image and at most this many painted paths is a scanned page.
pub const SCAN_MAX_PATHS: u64 = 10;

/// Every bound the renderer adds to the PDF parser's own [`Limits`]. Implementation limits of
/// this program (memory and time), not engineering values.
#[derive(Clone, Debug)]
pub struct RasterLimits {
    pub pdf: Limits,
    /// Pixel visits of all drawing on one page (line stamps, image reads and writes).
    pub max_draw_work: u64,
    /// Pixels of one decoded image (width times height).
    pub max_image_pixels: u64,
    /// Bytes of the output canvas (pixels times channels).
    pub max_canvas_bytes: u64,
    /// Bytes of all decoded images of one page together.
    pub max_total_image_bytes: u64,
    /// Pages whose kind `info` works out one by one.
    pub max_info_pages: u32,
}

impl Default for RasterLimits {
    fn default() -> Self {
        RasterLimits {
            pdf: Limits::default(),
            max_draw_work: 1_500_000_000,
            max_image_pixels: 100_000_000,
            max_canvas_bytes: 200_000_000,
            max_total_image_bytes: 800_000_000,
            max_info_pages: 2_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Page number from 1 (render only).
    pub page: u32,
    pub dpi: u32,
    /// Gray output (one channel); otherwise RGB when the page has colour images.
    pub gray: bool,
    pub limits: RasterLimits,
    pub time_limit: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            page: 1,
            dpi: DEFAULT_DPI,
            gray: false,
            limits: RasterLimits::default(),
            time_limit: Duration::from_secs(60),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// The input cannot be rendered (exit code 2).
    Refused,
    /// A limit was exceeded (exit code 3).
    LimitExceeded,
}

#[derive(Debug)]
pub struct Failure {
    pub kind: FailureKind,
    /// Machine-readable code, for example "encrypted", "malformed", "unsupported_filter".
    pub code: &'static str,
    pub limit: Option<&'static str>,
    pub message: String,
    pub page_count: Option<u32>,
}

impl Failure {
    pub fn exit_code(&self) -> i32 {
        match self.kind {
            FailureKind::Refused => 2,
            FailureKind::LimitExceeded => 3,
        }
    }
}

fn to_failure(f: Fault, page_count: Option<u32>) -> Box<Failure> {
    let (kind, code, limit, message) = match f {
        Fault::Malformed(m) => (
            FailureKind::Refused,
            "malformed",
            None,
            format!("the PDF structure cannot be read: {m}"),
        ),
        Fault::Unsupported(m) => {
            let code = if m.starts_with("colour space") {
                "unsupported_colorspace"
            } else {
                "unsupported_filter"
            };
            (
                FailureKind::Refused,
                code,
                None,
                format!("{m} is not supported"),
            )
        }
        Fault::Encrypted => (
            FailureKind::Refused,
            "encrypted",
            None,
            "the PDF is encrypted; encrypted files are not supported".to_string(),
        ),
        Fault::Refused(code, m) => (FailureKind::Refused, code, None, m),
        Fault::Limit(name, m) => (FailureKind::LimitExceeded, "limit_exceeded", Some(name), m),
        Fault::Timeout => (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("wall_clock"),
            "the rendering ran out of time".to_string(),
        ),
    };
    Box::new(Failure {
        kind,
        code,
        limit,
        message,
        page_count,
    })
}

fn refuse(code: &'static str, message: &str) -> Box<Failure> {
    Box::new(Failure {
        kind: FailureKind::Refused,
        code,
        limit: None,
        message: message.to_string(),
        page_count: None,
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn check_options(o: &Options) -> Result<(), Box<Failure>> {
    if o.page == 0 {
        return Err(refuse("invalid_page", "page numbers start at 1"));
    }
    if !(MIN_DPI..=MAX_DPI).contains(&o.dpi) {
        return Err(refuse(
            "invalid_dpi",
            &format!("the resolution must be between {MIN_DPI} and {MAX_DPI} dpi"),
        ));
    }
    Ok(())
}

fn classify(scene: &Scene) -> (PageKind, u64, u64) {
    let (w, h) = (scene.width, scene.height);
    let area = w * h;
    let mut images = 0u64;
    let mut covering = false;
    let mut lines = 0u64;
    for item in &scene.items {
        match item {
            Item::Line { .. } => lines += 1,
            Item::Image { ctm, .. } => {
                images += 1;
                let pts = [
                    (ctm.e, ctm.f),
                    (ctm.a + ctm.e, ctm.b + ctm.f),
                    (ctm.c + ctm.e, ctm.d + ctm.f),
                    (ctm.a + ctm.c + ctm.e, ctm.b + ctm.d + ctm.f),
                ];
                let lo = |f: fn(&(f64, f64)) -> f64| pts.iter().map(f).fold(f64::MAX, f64::min);
                let hi = |f: fn(&(f64, f64)) -> f64| pts.iter().map(f).fold(f64::MIN, f64::max);
                let (x0, x1) = (lo(|p| p.0).max(0.0), hi(|p| p.0).min(w));
                let (y0, y1) = (lo(|p| p.1).max(0.0), hi(|p| p.1).min(h));
                if x1 > x0 && y1 > y0 && (x1 - x0) * (y1 - y0) >= 0.5 * area {
                    covering = true;
                }
            }
        }
    }
    let kind = if images == 0 && lines == 0 {
        PageKind::Blank
    } else if images == 0 {
        PageKind::Vector
    } else if covering && scene.painted <= SCAN_MAX_PATHS {
        PageKind::Scanned
    } else {
        PageKind::Mixed
    };
    (kind, images, lines)
}

/// Page count and, for every page, its size and kind (scanned, vector, mixed, blank).
pub fn info(input: &[u8], opts: &Options) -> Result<Info, Box<Failure>> {
    let start = Instant::now();
    let mut doc = PdfDoc::open(input, opts.limits.pdf.clone(), opts.time_limit)
        .map_err(|f| to_failure(f, None))?;
    let count = doc.page_count().map_err(|f| to_failure(f, None))?;
    let mut pages = Vec::new();
    for n in 1..=count.min(opts.limits.max_info_pages) {
        match doc.scene(n, 1.0) {
            Ok(scene) => {
                let (kind, images, lines) = classify(&scene);
                pages.push(PageSummary {
                    page: n,
                    width_pt: scene.width_pt,
                    height_pt: scene.height_pt,
                    rotate: scene.rotate,
                    kind,
                    images,
                    lines,
                    error: None,
                });
            }
            Err(Fault::Timeout) => {
                pages.push(PageSummary::unknown(n, "time"));
                break;
            }
            Err(f) => {
                let fail = to_failure(f, Some(count));
                pages.push(PageSummary::unknown(n, fail.code));
            }
        }
    }
    Ok(Info {
        sha256: sha256_hex(input),
        input_bytes: input.len(),
        page_count: count,
        pages,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

/// Renders page `opts.page` at `opts.dpi`.
pub fn render(input: &[u8], opts: &Options) -> Result<Rendered, Box<Failure>> {
    check_options(opts)?;
    let start = Instant::now();
    let mut doc = PdfDoc::open(input, opts.limits.pdf.clone(), opts.time_limit)
        .map_err(|f| to_failure(f, None))?;
    let ppp = f64::from(opts.dpi) / 72.0;
    let scene = doc.scene(opts.page, ppp).map_err(|f| to_failure(f, None))?;
    let count = scene.page_count;
    let fail = |f: Fault| to_failure(f, Some(count));
    let (kind, images, lines) = classify(&scene);
    let (wpx, hpx) = (scene.width.round(), scene.height.round());
    if !(wpx >= 1.0 && hpx >= 1.0 && wpx.is_finite() && hpx.is_finite()) {
        return Err(fail(Fault::Malformed("the page has no usable size".into())));
    }
    if wpx > f64::from(u32::MAX >> 4) || hpx > f64::from(u32::MAX >> 4) {
        return Err(fail(Fault::Limit(
            "output_pixels",
            "the page image is too large".into(),
        )));
    }
    let (w, h) = (wpx as u32, hpx as u32);
    // Early: even a one-channel canvas must fit, before anything is decoded.
    if u64::from(w) * u64::from(h) > opts.limits.max_canvas_bytes {
        return Err(fail(Fault::Limit(
            "output_pixels",
            "the page image would be larger than the limit allows; lower the resolution".into(),
        )));
    }
    let mut stats = Warn::default();
    for (k, (n, m)) in &scene.stats.warnings {
        stats.add(k, *n, m);
    }
    // Colour only when asked for and some image has colour; decided after decoding.
    let any_colour_possible = !opts.gray;
    let mut decoded: BTreeMap<usize, Option<decode::Pixmap>> = BTreeMap::new();
    // First pass: decode every image placement (cached per source), to know the channels.
    let mut warnings = decode::Warnings::default();
    let max_px = opts.limits.max_image_pixels;
    let mut ch = 1usize;
    let mut next_key = 0usize;
    let mut total_image_bytes = 0u64;
    let mut keys: Vec<usize> = Vec::new();
    let mut by_obj: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut filters: BTreeMap<&'static str, u64> = BTreeMap::new();
    for item in &scene.items {
        let Item::Image { src, .. } = item else {
            keys.push(usize::MAX);
            continue;
        };
        let ident = match src {
            ImageSrc::Xobject(s, _) => Some((s.start, s.len)),
            ImageSrc::Inline(..) => None,
        };
        let key = match ident.and_then(|i| by_obj.get(&i).copied()) {
            Some(k) => k,
            None => {
                let k = next_key;
                next_key += 1;
                if let Some(i) = ident {
                    by_obj.insert(i, k);
                }
                let raw = doc.load_image(src, max_px).map_err(fail)?;
                let label = match &raw.filter {
                    nf_pdf::raster::ImageFilter::Raw => "raw",
                    nf_pdf::raster::ImageFilter::Dct => "dct",
                    nf_pdf::raster::ImageFilter::Ccitt(_) => "ccitt",
                    nf_pdf::raster::ImageFilter::Jbig2 { .. } => "jbig2",
                    nf_pdf::raster::ImageFilter::Jpx => "jpx",
                };
                *filters.entry(label).or_insert(0) += 1;
                let pix =
                    decode::decode(raw, &doc.deadline(), max_px, &mut warnings).map_err(fail)?;
                if pix.ch == 3 && !pix.mask && any_colour_possible {
                    ch = 3;
                }
                total_image_bytes += pix.data.len() as u64;
                if total_image_bytes > opts.limits.max_total_image_bytes {
                    return Err(fail(Fault::Limit(
                        "image_bytes_total",
                        "the decoded images of the page together are larger than the limit allows"
                            .into(),
                    )));
                }
                decoded.insert(k, Some(pix));
                k
            }
        };
        keys.push(key);
    }
    for w in warnings.0 {
        stats.add(w, 1, warning_text(w));
    }
    let bytes = u64::from(w) * u64::from(h) * ch as u64;
    if bytes > opts.limits.max_canvas_bytes {
        return Err(fail(Fault::Limit(
            "output_pixels",
            "the page image would be larger than the limit allows; lower the resolution".into(),
        )));
    }
    let mut canvas = draw::Canvas::new(w, h, ch, opts.limits.max_draw_work);
    let dl = doc.deadline();
    for (idx, item) in scene.items.iter().enumerate() {
        if idx % 1024 == 0 {
            dl.check().map_err(fail)?;
        }
        match item {
            Item::Line {
                pts,
                closed,
                width,
                gray,
            } => canvas
                .polyline(pts, *closed, *width, *gray, &dl)
                .map_err(fail)?,
            Item::Image { ctm, fill, .. } => {
                let key = keys.get(idx).copied().unwrap_or(usize::MAX);
                if let Some(Some(pix)) = decoded.get(&key) {
                    canvas.image(key, pix, ctm, *fill, &dl).map_err(fail)?;
                }
            }
        }
    }
    let png = png::encode(w, h, ch, opts.dpi, &canvas.data).ok_or_else(|| {
        fail(Fault::Malformed(
            "the page image could not be encoded".into(),
        ))
    })?;
    Ok(Rendered {
        png,
        width: w,
        height: h,
        channels: ch,
        dpi: opts.dpi,
        page: opts.page,
        page_count: count,
        width_pt: scene.width_pt,
        height_pt: scene.height_pt,
        expected_width: scene.width,
        expected_height: scene.height,
        rotate: scene.rotate,
        kind,
        images,
        lines,
        filters: filters.into_iter().collect(),
        warnings: stats.0.into_iter().map(|(k, (n, m))| (k, n, m)).collect(),
        sha256: sha256_hex(input),
        input_bytes: input.len(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

fn warning_text(code: &str) -> &'static str {
    match code {
        "image_truncated" => "an image holds less data than its size says; the rest is left white",
        "image_alpha_ignored" => "an image soft mask or colour-key mask was not applied",
        _ => "see the code",
    }
}

#[derive(Default)]
struct Warn(BTreeMap<&'static str, (u64, &'static str)>);

impl Warn {
    fn add(&mut self, code: &'static str, n: u64, msg: &'static str) {
        let e = self.0.entry(code).or_insert((0, msg));
        e.0 = e.0.saturating_add(n);
    }
}
