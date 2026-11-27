//! A0 drawing scans at 300 dpi (9933 x 14043 px, 139.5 Mpx) render with the default limits, in
//! time; a scan just over the pixel limit is refused with its code; colour A0 is refused.
//! Timing tests run in release only (`cargo test --release`); debug builds skip them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use std::io::Write;
use std::time::Instant;

use common::decode_png;
use flate2::{write::ZlibEncoder, Compression};
use nf_raster::{render, FailureKind, Options};

/// A0 at 300 dpi: 9933 x 14043 px (the page is 2383.92 x 3370.32 pt).
const W: u32 = 9933;
const H: u32 = 14043;

/// Flate-compressed rows, written one at a time.
fn flate_rows(rows: impl Iterator<Item = Vec<u8>>) -> Vec<u8> {
    let mut z = ZlibEncoder::new(Vec::new(), Compression::fast());
    for r in rows {
        z.write_all(&r).unwrap();
    }
    z.finish().unwrap()
}

/// A one-page PDF drawing `/Im0` over the whole A0 page.
fn a0_pdf(dict: &str, data: &[u8]) -> Vec<u8> {
    let content = "q 2383.92 0 0 3370.32 0 0 cm /Im0 Do Q";
    let mut v = format!(
        "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 2383.92 3370.32] /Contents 4 0 R /Resources << /XObject << /Im0 5 0 R >> >> >>\nendobj\n4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n5 0 obj\n<< /Type /XObject /Subtype /Image {} /Length {} >>\nstream\n",
        content.len(),
        content,
        dict,
        data.len()
    )
    .into_bytes();
    v.extend_from_slice(data);
    v.extend_from_slice(b"\nendstream\nendobj\ntrailer\n<< /Root 1 0 R /Size 6 >>\n%%EOF\n");
    v
}

/// Width and height from the IHDR of a PNG this crate wrote.
fn png_size(png: &[u8]) -> (u32, u32) {
    (
        u32::from_be_bytes(png[16..20].try_into().unwrap()),
        u32::from_be_bytes(png[20..24].try_into().unwrap()),
    )
}

/// Every 50th row is a black drawing line on white paper; the rest is white.
fn scan_row(y: u32, bytes_per_row: usize, black: u8) -> Vec<u8> {
    if y.is_multiple_of(50) {
        vec![0u8; bytes_per_row]
    } else {
        vec![black; bytes_per_row]
    }
}

fn check_lines(png: &[u8]) {
    let p = decode_png(png);
    assert_eq!((p.w, p.h, p.ch), (W as usize, H as usize, 1));
    assert!(p.px[0] < 128, "row 0 is a drawing line");
    assert!(p.px[25 * p.w + 500] > 128, "row 25 is paper");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "about 10 s in release; run with --release")]
fn a0_one_bit_flate_scan_at_300_dpi_renders_in_time() {
    let data = flate_rows((0..H).map(|y| scan_row(y, (W as usize).div_ceil(8), 0xFF)));
    let pdf = a0_pdf(
        &format!("/Width {W} /Height {H} /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /FlateDecode"),
        &data,
    );
    let o = Options {
        gray: true,
        dpi: 300,
        ..Options::default()
    };
    let t = Instant::now();
    let r = render(&pdf, &o).unwrap_or_else(|f| panic!("{} {:?}", f.code, f.limit));
    let secs = t.elapsed().as_secs_f64();
    assert!(secs < 60.0, "A0 1-bit at 300 dpi took {secs:.1} s");
    assert_eq!(png_size(&r.png), (W, H));
    check_lines(&r.png);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "about 10 s in release; run with --release")]
fn a0_gray_flate_scan_at_300_dpi_renders_in_time() {
    let data = flate_rows((0..H).map(|y| scan_row(y, W as usize, 0xFF)));
    let pdf = a0_pdf(
        &format!("/Width {W} /Height {H} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode"),
        &data,
    );
    let o = Options {
        gray: true,
        dpi: 300,
        ..Options::default()
    };
    let t = Instant::now();
    let r = render(&pdf, &o).unwrap_or_else(|f| panic!("{} {:?}", f.code, f.limit));
    let secs = t.elapsed().as_secs_f64();
    assert!(secs < 60.0, "A0 gray at 300 dpi took {secs:.1} s");
    assert_eq!(png_size(&r.png), (W, H));
    check_lines(&r.png);
}

#[test]
fn a_scan_just_over_the_pixel_limit_is_refused() {
    // 14496 x 14496 = 210,134,016 px, above the 210 Mpx limit; the dictionary is refused
    // before any data is read, so the stream is a few bytes.
    let pdf = a0_pdf(
        "/Width 14496 /Height 14496 /ColorSpace /DeviceGray /BitsPerComponent 1",
        &[0u8; 16],
    );
    let o = Options {
        gray: true,
        ..Options::default()
    };
    let f = render(&pdf, &o).err().unwrap();
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("image_pixels")
        )
    );
}

#[test]
fn a_colour_a0_scan_at_300_dpi_is_refused_not_decoded() {
    // RGB at A0/300: the canvas would be 418 MB, over the 210 MB limit. Refused by a limit
    // code; the image is not decoded (the stream is a few bytes).
    let pdf = a0_pdf(
        &format!("/Width {W} /Height {H} /ColorSpace /DeviceRGB /BitsPerComponent 8"),
        &[0u8; 16],
    );
    let o = Options {
        gray: false,
        dpi: 300,
        ..Options::default()
    };
    let f = render(&pdf, &o).err().unwrap();
    assert_eq!(f.kind, FailureKind::LimitExceeded);
    assert_eq!(f.code, "limit_exceeded");
    assert_eq!(f.limit, Some("output_pixels"));
}

#[test]
fn a_colour_page_over_the_canvas_limit_is_refused_before_decoding() {
    // A small RGB image on the A0 page: the RGB canvas (418 MB) is over the limit, so it is
    // refused as output_pixels before the image is decoded.
    let data = vec![0u8; 3 * 4 * 4];
    let pdf = a0_pdf(
        "/Width 4 /Height 4 /ColorSpace /DeviceRGB /BitsPerComponent 8",
        &data,
    );
    let o = Options {
        gray: false,
        dpi: 300,
        ..Options::default()
    };
    let f = render(&pdf, &o).err().unwrap();
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("output_pixels")
        )
    );
}
