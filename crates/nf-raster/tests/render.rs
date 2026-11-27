//! Rendering of the generated fixtures (tests/fixtures/make_fixtures.py): sizes, content,
//! cross-checks between encodings, refusals, no panic on damaged input, time.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use std::time::Instant;

use common::{decode_png, fixture, tiny_pdf};
use nf_raster::{info, render, FailureKind, Options, PageKind};

fn gray_opts() -> Options {
    Options {
        gray: true,
        ..Options::default()
    }
}

fn go(name: &str) -> (nf_raster::Rendered, common::Png) {
    let r = render(&fixture(name), &gray_opts())
        .unwrap_or_else(|f| panic!("{name}: {} {}", f.code, f.message));
    let p = decode_png(&r.png);
    (r, p)
}

#[test]
fn every_scanned_fixture_renders_non_blank_at_the_expected_size() {
    // (file, expected width, expected height) at 200 dpi; sizes are exact: the generated pages are
    // an integer number of pixels at 200 dpi.
    let cases = [
        ("scan_dct_gray.pdf", 2351, 1662),
        ("scan_dct_rgb.pdf", 2351, 1662),
        ("scan_dct_cmyk.pdf", 2351, 1662),
        ("scan_g4.pdf", 2351, 1662),
        ("scan_g3_1d.pdf", 2351, 1662),
        ("scan_flate_gray.pdf", 2351, 1662),
        ("scan_flate_1bit.pdf", 2351, 1662),
        ("scan_indexed_4bit.pdf", 2351, 1662),
        ("scan_imagemask.pdf", 2351, 1662),
        ("scan_multi.pdf", 2351, 1662),
        ("scan_jbig2_arith.pdf", 1176, 831),
        ("scan_jbig2_mmr.pdf", 1176, 831),
        ("scan_g4_small.pdf", 1176, 831),
        ("scan_rot90_crop.pdf", 1662, 2351),
        ("scan_rot180.pdf", 2351, 1662),
        ("scan_a3_g4.pdf", 3307, 2339),
        ("scan_a3_dct.pdf", 3307, 2339),
        ("vector_sample.pdf", 4898, 3463),
    ];
    for (name, w, h) in cases {
        let (r, p) = go(name);
        assert!(
            (p.w as i64 - w).abs() <= 1 && (p.h as i64 - h).abs() <= 1,
            "{name}: {}x{} expected {w}x{h}",
            p.w,
            p.h
        );
        assert_eq!(
            (r.width as usize, r.height as usize),
            (p.w, p.h),
            "{name}: report size"
        );
        assert_eq!(p.dpi, 200, "{name}: pHYs");
        let dark = p.dark();
        let total = p.w * p.h;
        assert!(
            dark > total / 1000 && dark < total / 3,
            "{name}: {dark} dark of {total}"
        );
        assert_eq!(r.page_count, 1, "{name}");
    }
}

#[test]
fn lossless_encodings_of_one_bitmap_give_identical_pixels() {
    let (_, flate) = go("scan_flate_1bit.pdf");
    for name in ["scan_g4.pdf", "scan_g3_1d.pdf", "scan_imagemask.pdf"] {
        let (_, p) = go(name);
        assert!(
            p.px == flate.px,
            "{name} differs from the Flate 1-bit rendering"
        );
    }
    let (_, jb) = go("scan_jbig2_arith.pdf");
    let (_, jm) = go("scan_jbig2_mmr.pdf");
    let (_, g4) = go("scan_g4_small.pdf");
    assert!(
        jb.px == g4.px,
        "JBIG2 arithmetic differs from CCITT G4 of the same bitmap"
    );
    assert!(
        jm.px == g4.px,
        "JBIG2 MMR differs from CCITT G4 of the same bitmap"
    );
    // the small bitmap is the top-left crop of the full one
    for y in 0..g4.h {
        assert!(
            g4.px[y * g4.w..(y + 1) * g4.w] == flate.px[y * flate.w..y * flate.w + g4.w],
            "row {y}"
        );
    }
}

#[test]
fn lossy_and_palette_encodings_stay_close_to_the_gray_original() {
    let (_, gray) = go("scan_flate_gray.pdf");
    for (name, tol) in [
        ("scan_dct_gray.pdf", 2.0),
        ("scan_dct_rgb.pdf", 3.0),
        ("scan_indexed_4bit.pdf", 8.0),
    ] {
        let (_, p) = go(name);
        let d = p.mean_abs_diff(&gray);
        assert!(d < tol, "{name}: mean difference {d}");
    }
    let (_, multi) = go("scan_multi.pdf");
    assert!(multi.mean_abs_diff(&gray) < 6.0, "composited page");
}

#[test]
fn rotation_and_boxes_are_applied() {
    let (_, up) = go("scan_dct_gray.pdf");
    let (_, r180) = go("scan_rot180.pdf");
    // a 180 degree rotation is the same picture turned over
    let n = up.px.len();
    let flipped: Vec<u8> = up.px.iter().rev().copied().collect();
    assert_eq!(n, r180.px.len());
    let d: u64 = flipped
        .iter()
        .zip(&r180.px)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    assert!((d as f64 / n as f64) < 1.0, "180 degrees");
    // /Rotate 90 with an offset MediaBox and a CropBox: the cropped page, turned a quarter
    let (r, r90) = go("scan_rot90_crop.pdf");
    assert_eq!((r.rotate, r90.w, r90.h), (90, 1662, 2351));
    // clockwise quarter turn of the upright picture
    let mut worst = 0u64;
    for y in (0..r90.h).step_by(97) {
        for x in (0..r90.w).step_by(89) {
            // pixel (x, y) of the turned page comes from (y, up.h - 1 - x) of the upright one
            let src = up.px[(up.h - 1 - x) * up.w + y];
            worst += u64::from(src.abs_diff(r90.px[y * r90.w + x]) > 64);
        }
    }
    assert!(
        worst < 40,
        "rotated content differs at {worst} sampled pixels"
    );
}

#[test]
fn colour_output_when_not_gray() {
    let r = render(&fixture("scan_dct_rgb.pdf"), &Options::default()).unwrap();
    let p = decode_png(&r.png);
    assert_eq!(p.ch, 3);
    assert!(
        p.px.chunks(3).any(|c| c[0] != c[2]),
        "the colour tint of the fixture is kept"
    );
    let r = render(&fixture("scan_g4.pdf"), &Options::default()).unwrap();
    assert_eq!(decode_png(&r.png).ch, 1, "a bilevel page stays one channel");
}

#[test]
fn vector_pages_are_drawn_from_lines() {
    let (r, p) = go("vector_sample.pdf");
    assert_eq!(r.kind, PageKind::Vector);
    assert!(r.lines > 50 && r.images == 0);
    // the sheet border is on the page: dark pixels near the first and last rows
    let row_dark = |y: usize| {
        p.px[y * p.w..(y + 1) * p.w]
            .iter()
            .filter(|&&v| v < 128)
            .count()
    };
    assert!((0..80).any(|y| row_dark(y) > p.w / 2), "top border");
    assert!(
        (p.h - 80..p.h).any(|y| row_dark(y) > p.w / 2),
        "bottom border"
    );
}

#[test]
fn info_reports_pages_and_kinds() {
    let o = Options::default();
    let i = info(&fixture("scan_multi.pdf"), &o).unwrap();
    assert_eq!((i.page_count, i.pages.len()), (1, 1));
    let p = &i.pages[0];
    assert_eq!((p.kind, p.images), (PageKind::Scanned, 2));
    assert!((p.width_pt - 846.36).abs() < 0.01 && (p.height_pt - 598.32).abs() < 0.01);
    let v = info(&fixture("vector_sample.pdf"), &o).unwrap();
    assert_eq!(v.pages[0].kind, PageKind::Vector);
    let r = info(&fixture("scan_rot90_crop.pdf"), &o).unwrap();
    assert_eq!(
        (r.pages[0].rotate, r.pages[0].width_pt.round() as i64),
        (90, 598)
    );
    let j = i.to_json(&[("seccomp", "x".to_string())]);
    assert!(j.contains("\"kind\": \"scanned\"") && j.contains("\"page_count\": 1"));
}

#[test]
fn the_report_names_filters_and_warnings() {
    let (r, _) = go("scan_jbig2_arith.pdf");
    assert_eq!(r.filters, vec![("jbig2", 1)]);
    let (r, _) = go("scan_multi.pdf");
    assert_eq!(r.filters, vec![("ccitt", 1), ("dct", 1)]);
    assert!(r
        .to_json("00", &[])
        .contains("\"image_filters\": {\"ccitt\": 1, \"dct\": 1}"));
}

fn refusal(name: &str) -> (FailureKind, &'static str, Option<&'static str>) {
    match render(&fixture(name), &gray_opts()) {
        Ok(_) => panic!("{name} was rendered"),
        Err(f) => (f.kind, f.code, f.limit),
    }
}

#[test]
fn malformed_and_hostile_files_give_coded_refusals() {
    use FailureKind::{LimitExceeded as L, Refused as R};
    assert_eq!(refusal("bad_garbage.pdf"), (R, "malformed", None));
    assert_eq!(refusal("bad_truncated.pdf"), (R, "malformed", None));
    assert_eq!(refusal("bad_page_loop.pdf"), (R, "malformed", None));
    assert_eq!(refusal("bad_encrypted.pdf"), (R, "encrypted", None));
    assert_eq!(
        refusal("bad_jpeg_data.pdf"),
        (R, "image_decode_failed", None)
    );
    assert_eq!(refusal("bad_jpx.pdf"), (R, "unsupported_filter", None));
    assert_eq!(
        refusal("bad_huge_dims.pdf"),
        (L, "limit_exceeded", Some("image_pixels"))
    );
    // 200 MiB of zeros in a few hundred KB: stopped by the expansion-ratio guard
    assert_eq!(
        refusal("bad_bomb.pdf"),
        (L, "limit_exceeded", Some("compression_ratio"))
    );
}

#[test]
fn bad_arguments_are_refused() {
    let pdf = fixture("scan_g4_small.pdf");
    let code = |o: Options| render(&pdf, &o).err().map(|f| f.code);
    assert_eq!(
        code(Options {
            page: 0,
            ..Options::default()
        }),
        Some("invalid_page")
    );
    assert_eq!(
        code(Options {
            page: 2,
            ..Options::default()
        }),
        Some("page_out_of_range")
    );
    assert_eq!(
        code(Options {
            dpi: 10,
            ..Options::default()
        }),
        Some("invalid_dpi")
    );
    assert_eq!(
        code(Options {
            dpi: 5000,
            ..Options::default()
        }),
        Some("invalid_dpi")
    );
}

#[test]
fn output_size_is_bounded() {
    let mut o = Options::default();
    o.limits.max_canvas_bytes = 1_000_000;
    let f = render(&fixture("scan_g4.pdf"), &o).err().unwrap();
    assert_eq!((f.code, f.limit), ("limit_exceeded", Some("output_pixels")));
    let mut o = Options::default();
    o.limits.max_image_pixels = 1_000_000;
    let f = render(&fixture("scan_g4.pdf"), &o).err().unwrap();
    assert_eq!((f.code, f.limit), ("limit_exceeded", Some("image_pixels")));
}

/// Deterministic xorshift for the damage tests.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn damaged_files_never_panic() {
    let low = Options {
        dpi: 40,
        gray: true,
        ..Options::default()
    };
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    // NF_RASTER_FUZZ_ROUNDS raises the number of mutations per file for a longer run.
    let rounds: usize = std::env::var("NF_RASTER_FUZZ_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let mut outcomes = [0usize; 2];
    for name in [
        "scan_g4_small.pdf",
        "scan_jbig2_arith.pdf",
        "scan_jbig2_mmr.pdf",
        "bad_jpx.pdf",
        "scan_g3_1d.pdf",
        "scan_dct_cmyk.pdf",
        "scan_indexed_4bit.pdf",
    ] {
        let good = fixture(name);
        let good = if good.len() > 12_000 {
            good[..12_000].to_vec()
        } else {
            good
        };
        for round in 0..rounds {
            let mut d = good.clone();
            match round % 3 {
                0 => {
                    let n = 1 + (rng.next() % 8) as usize;
                    for _ in 0..n {
                        let at = (rng.next() as usize) % d.len();
                        d[at] ^= 1 << (rng.next() % 8);
                    }
                }
                1 => d.truncate((rng.next() as usize) % d.len()),
                _ => {
                    let at = (rng.next() as usize) % d.len();
                    let len = ((rng.next() as usize) % 64).min(d.len() - at);
                    for b in &mut d[at..at + len] {
                        *b = rng.next() as u8;
                    }
                }
            }
            let r = std::panic::catch_unwind(|| render(&d, &low).is_ok());
            match r {
                Ok(ok) => outcomes[usize::from(ok)] += 1,
                Err(_) => panic!("{name}: panic on mutation {round}"),
            }
        }
    }
    eprintln!(
        "damaged inputs: {} refused, {} rendered",
        outcomes[0], outcomes[1]
    );
    assert!(outcomes[0] > 100, "most damage must be noticed");
}

#[test]
fn raw_garbage_never_panics() {
    let mut rng = Rng(12345);
    for len in [0usize, 1, 7, 64, 1000, 5000] {
        for _ in 0..40 {
            let d: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            assert!(std::panic::catch_unwind(|| render(&d, &Options::default()).is_err()).unwrap());
        }
    }
    let mut d = b"%PDF-1.4\n".to_vec();
    d.extend((0..3000).map(|_| rng.next() as u8));
    assert!(std::panic::catch_unwind(|| render(&d, &Options::default()).is_err()).unwrap());
}

#[test]
fn a3_at_200_dpi_renders_in_under_three_seconds() {
    for name in ["scan_a3_g4.pdf", "scan_a3_dct.pdf"] {
        let pdf = fixture(name);
        let start = Instant::now();
        let r = render(&pdf, &Options::default()).unwrap();
        let t = start.elapsed();
        eprintln!(
            "A3 200 dpi {name}: {}x{} in {:?} (render time reported {} ms)",
            r.width, r.height, t, r.elapsed_ms
        );
        assert!(t.as_secs_f64() < 3.0, "{name}: {t:?}");
    }
}

#[test]
fn inline_images_are_painted_in_place() {
    // 2 x 2 checkerboard (rows top first: black white / white black) over the whole 100 x 100 pt page
    let pdf = tiny_pdf(
        [0.0, 0.0, 100.0, 100.0],
        "q 100 0 0 100 0 0 cm BI /W 2 /H 2 /BPC 8 /CS /G /F /AHx ID 00FF FF00> EI Q",
    );
    let o = Options {
        dpi: 72,
        gray: true,
        ..Options::default()
    };
    let r = render(&pdf, &o).unwrap();
    let p = decode_png(&r.png);
    assert_eq!((p.w, p.h), (100, 100));
    let at = |x: usize, y: usize| p.px[y * p.w + x];
    assert_eq!(
        (at(10, 10), at(90, 10), at(10, 90), at(90, 90)),
        (0, 255, 255, 0)
    );
    assert_eq!(r.images, 1);
}

#[test]
fn lines_are_drawn_where_the_page_says() {
    // a 4 pt wide horizontal bar at y = 50 (PDF y up), full width
    let pdf = tiny_pdf([0.0, 0.0, 100.0, 100.0], "4 w 0 50 m 100 50 l S");
    let o = Options {
        dpi: 72,
        gray: true,
        ..Options::default()
    };
    let p = decode_png(&render(&pdf, &o).unwrap().png);
    let col: Vec<u8> = (0..p.h).map(|y| p.px[y * p.w + 50]).collect();
    assert!(
        col[49] < 30 && col[50] < 30,
        "the bar covers rows 48..52 of the image (y flipped)"
    );
    assert!(col[40] == 255 && col[60] == 255);
}

#[test]
fn an_enormous_page_is_refused_not_allocated() {
    let pdf = tiny_pdf([0.0, 0.0, 200000.0, 200000.0], "0 0 m 10 10 l S");
    let f = render(&pdf, &Options::default()).err().unwrap();
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("output_pixels")
        )
    );
}

#[test]
fn decoded_images_of_a_page_are_bounded_together() {
    let mut o = gray_opts();
    o.limits.max_total_image_bytes = 1_000_000;
    let f = render(&fixture("scan_multi.pdf"), &o).err().unwrap();
    assert_eq!(
        (f.code, f.limit),
        ("limit_exceeded", Some("image_bytes_total"))
    );
}

/// A one-page PDF whose content draws `/Im0` (a stream with `dict` extras and `data`).
fn image_pdf(media: f64, dict: &str, data: &[u8], content: &str) -> Vec<u8> {
    let mut v = format!(
        "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {media} {media}] /Contents 4 0 R /Resources << /XObject << /Im0 5 0 R >> >> >>\nendobj\n4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n5 0 obj\n<< /Type /XObject /Subtype /Image {dict} /Length {} >>\nstream\n",
        content.len(),
        content,
        data.len()
    )
    .into_bytes();
    v.extend_from_slice(data);
    v.extend_from_slice(b"\nendstream\nendobj\ntrailer\n<< /Root 1 0 R /Size 6 >>\n%%EOF\n");
    v
}

fn failure_of(pdf: &[u8], o: &Options) -> nf_raster::Failure {
    match render(pdf, o) {
        Ok(_) => panic!("the render must fail"),
        Err(f) => *f,
    }
}

#[test]
fn a_pathological_stroke_ends_with_a_coded_work_limit() {
    // 300 strokes of a 100 pt wide line across a 1000 pt page at 200 dpi: about 1e11 pixel visits
    let content = "100 w 0 0 m 1000 1000 l S\n".repeat(300);
    let pdf = tiny_pdf([0.0, 0.0, 1000.0, 1000.0], &content);
    let mut o = gray_opts();
    o.limits.max_draw_work = 20_000_000;
    let t = Instant::now();
    let f = failure_of(&pdf, &o);
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("draw_work")
        )
    );
    assert!(t.elapsed().as_secs_f64() < 20.0, "{:?}", t.elapsed());
}

#[test]
fn a_pathological_stroke_ends_at_the_time_limit_inside_the_stroke() {
    let content = "100 w 0 0 m 1000 1000 l S\n".repeat(300);
    let pdf = tiny_pdf([0.0, 0.0, 1000.0, 1000.0], &content);
    let mut o = gray_opts();
    o.time_limit = std::time::Duration::from_millis(100);
    let t = Instant::now();
    let f = failure_of(&pdf, &o);
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("wall_clock")
        )
    );
    assert!(t.elapsed().as_secs_f64() < 20.0, "{:?}", t.elapsed());
}

#[test]
fn a_huge_placement_count_ends_with_a_coded_limit() {
    // one 300 x 300 image placed 4000 times over the whole page
    let content = "q 500 0 0 500 0 0 cm /Im0 Do Q\n".repeat(4000);
    let pdf = image_pdf(
        500.0,
        "/Width 300 /Height 300 /ColorSpace /DeviceGray /BitsPerComponent 8",
        &vec![128u8; 300 * 300],
        &content,
    );
    let mut o = gray_opts();
    o.dpi = 72;
    o.limits.max_draw_work = 20_000_000;
    let t = Instant::now();
    let f = failure_of(&pdf, &o);
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("draw_work")
        )
    );
    assert!(t.elapsed().as_secs_f64() < 20.0, "{:?}", t.elapsed());
}

#[test]
fn wide_one_pixel_inline_images_end_with_a_coded_limit_in_time() {
    // 1,500,000 x 128 px canvas, 4096 inline 1 x 1 images each covering rows 1..63
    let one = "q 1500000 0 0 62 0 65 cm BI /W 1 /H 1 /CS /G /BPC 8 ID \x70 EI Q\n";
    let pdf = tiny_pdf([0.0, 0.0, 1_500_000.0, 128.0], &one.repeat(4096));
    let mut o = gray_opts();
    o.dpi = 72;
    let t = Instant::now();
    let f = failure_of(&pdf, &o);
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("draw_work")
        )
    );
    assert!(t.elapsed().as_secs_f64() < 60.0, "{:?}", t.elapsed());
}

#[test]
fn a_reduced_image_placed_many_times_is_reduced_once() {
    // 2000 x 2000 source drawn at 100 x 100 pixels, 40 times: the reduction (4e6 reads) is
    // charged once, so a budget of 20M passes; 40 reductions would need 1.6e8.
    let content = "q 100 0 0 100 0 0 cm /Im0 Do Q\n".repeat(40);
    let pdf = image_pdf(
        100.0,
        "/Width 2000 /Height 2000 /ColorSpace /DeviceGray /BitsPerComponent 8",
        &vec![100u8; 2000 * 2000],
        &content,
    );
    let mut o = gray_opts();
    o.dpi = 72;
    o.limits.max_draw_work = 20_000_000;
    render(&pdf, &o).expect("one reduction is charged once");
}

#[test]
fn an_unsupported_bits_per_component_is_refused() {
    let pdf = image_pdf(
        100.0,
        "/Width 4 /Height 4 /ColorSpace /DeviceGray /BitsPerComponent 3",
        &[0x55; 16],
        "q 100 0 0 100 0 0 cm /Im0 Do Q",
    );
    let f = failure_of(&pdf, &gray_opts());
    assert_eq!((f.kind, f.code), (FailureKind::Refused, "unsupported_bpc"));
}

#[test]
fn an_image_mask_in_a_jpeg_is_refused_with_a_code() {
    let jpeg = fixture("scan_dct_gray.pdf");
    let pdf = image_pdf(
        100.0,
        "/Width 4 /Height 4 /ImageMask true /Filter /DCTDecode",
        &jpeg[..64],
        "q 100 0 0 100 0 0 cm /Im0 Do Q",
    );
    let f = failure_of(&pdf, &gray_opts());
    assert_eq!(
        (f.kind, f.code),
        (FailureKind::Refused, "unsupported_filter")
    );
}

#[test]
fn an_indexed_decode_array_inverts_the_palette_order() {
    // two entries: 0 = black, 1 = white; /Decode [1 0] swaps them
    let dict = |d: &str| {
        format!("/Width 2 /Height 2 /ColorSpace [/Indexed /DeviceGray 1 <00FF>] /BitsPerComponent 1 {d}")
    };
    let px = |d: &str| {
        let pdf = image_pdf(
            100.0,
            &dict(d),
            &[0b0100_0000, 0b0100_0000],
            "q 100 0 0 100 0 0 cm /Im0 Do Q",
        );
        let o = Options {
            dpi: 72,
            gray: true,
            ..Options::default()
        };
        let p = decode_png(&render(&pdf, &o).unwrap().png);
        (p.px[10 * p.w + 10], p.px[10 * p.w + 90])
    };
    assert_eq!(px(""), (0, 255));
    assert_eq!(px("/Decode [1 0]"), (255, 0));
}

#[test]
fn a_jbig2_region_over_the_pixel_limit_is_a_coded_limit() {
    let mut j: Vec<u8> = Vec::new();
    // page information segment: 8 x 8
    j.extend_from_slice(&[0, 0, 0, 0, 48, 0, 1, 0, 0, 0, 19]);
    j.extend_from_slice(&[0, 0, 0, 8, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    // immediate generic region of 2^31 x 2^31 pixels
    j.extend_from_slice(&[0, 0, 0, 1, 38, 0, 1, 0, 0, 0, 18]);
    j.extend_from_slice(&[0x80, 0, 0, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let pdf = image_pdf(
        100.0,
        "/Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /JBIG2Decode",
        &j,
        "q 100 0 0 100 0 0 cm /Im0 Do Q",
    );
    let f = failure_of(&pdf, &gray_opts());
    assert_eq!(
        (f.kind, f.code, f.limit),
        (
            FailureKind::LimitExceeded,
            "limit_exceeded",
            Some("image_pixels")
        )
    );
}

fn jbig2_pdf(j: &[u8]) -> Vec<u8> {
    image_pdf(
        100.0,
        "/Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /JBIG2Decode",
        j,
        "q 100 0 0 100 0 0 cm /Im0 Do Q",
    )
}

#[test]
fn a_jbig2_region_of_unknown_length_is_checked_too() {
    let mut j: Vec<u8> = Vec::new();
    j.extend_from_slice(&[0, 0, 0, 0, 48, 0, 1, 0, 0, 0, 19]);
    j.extend_from_slice(&[0, 0, 0, 8, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    // immediate generic region, data length 0xFFFFFFFF, 2^31 x 2^31 pixels
    j.extend_from_slice(&[0, 0, 0, 1, 38, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF]);
    j.extend_from_slice(&[0x80, 0, 0, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let f = failure_of(&jbig2_pdf(&j), &gray_opts());
    assert_eq!(f.limit, Some("image_pixels"));
}

#[test]
fn a_jbig2_page_information_segment_over_the_limit_is_checked() {
    let mut j: Vec<u8> = Vec::new();
    j.extend_from_slice(&[0, 0, 0, 0, 48, 0, 1, 0, 0, 0, 19]);
    j.extend_from_slice(&[
        0x80, 0, 0, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    let f = failure_of(&jbig2_pdf(&j), &gray_opts());
    assert_eq!(f.limit, Some("image_pixels"));
}

#[test]
fn a_canvas_over_the_limit_is_refused_before_images_are_decoded() {
    // the image would fail to decode (bad bpc); the size limit must be reported first
    let pdf = image_pdf(
        2000.0,
        "/Width 4 /Height 4 /ColorSpace /DeviceGray /BitsPerComponent 3",
        &[0; 16],
        "q 2000 0 0 2000 0 0 cm /Im0 Do Q",
    );
    let mut o = gray_opts();
    o.limits.max_canvas_bytes = 1_000_000;
    let f = failure_of(&pdf, &o);
    assert_eq!(f.limit, Some("output_pixels"));
}
