//! Geometry: golden coordinates in millimetres, transforms, rotation, crop boxes,
//! Form XObjects, dashes, fill layer, text placement, Bezier flattening bounds, and
//! the container variants (compressed streams, xref streams, object streams).

mod common;

use common::*;
use nf_pdf::{convert, Options, Scale};

/// Millimetres per PDF point at 1:10 (25.4 / 72 * 10).
const K10: f64 = 25.4 / 72.0 * 10.0;

fn stroke_points(dxf: &[u8]) -> Vec<(f64, f64)> {
    let pl = polylines(dxf, "PDF_STROKE");
    assert_eq!(pl.len(), 1, "one stroked polyline expected");
    pl[0].0.clone()
}

#[test]
fn golden_line_at_scale_1_to_10() {
    let pdf = page_pdf(&PageSpec::new(b"10 20 m 110 20 l S"));
    let c = convert(&pdf, &Options::new(Scale::Ratio(10.0))).expect("converts");
    let (pts, closed) = polylines(&c.dxf, "PDF_STROKE").remove(0);
    assert!(!closed);
    assert_pt(pts[0], (10.0 * K10, 20.0 * K10));
    assert_pt(pts[1], (110.0 * K10, 20.0 * K10));
    assert!(close(c.report.mm_per_point, K10));
    assert_eq!(c.report.stats.get("path_segments"), 1);
}

#[test]
fn golden_line_with_mm_per_point_and_ratio_agree() {
    let pdf = page_pdf(&PageSpec::new(b"0 0 m 50 0 l S"));
    let a = convert(&pdf, &Options::new(Scale::Ratio(100.0))).expect("ok");
    let b = convert(&pdf, &Options::new(Scale::MmPerPoint(25.4 / 72.0 * 100.0))).expect("ok");
    // The note text names the scale as given, so compare the geometry rather than the bytes.
    assert_eq!(
        polylines(&a.dxf, "PDF_STROKE"),
        polylines(&b.dxf, "PDF_STROKE")
    );
}

#[test]
fn rectangle_is_a_closed_polyline() {
    let pdf = page_pdf(&PageSpec::new(b"10 10 50 30 re S"));
    let pts = stroke_points(&convert_ok(&pdf, 1.0).dxf);
    assert_eq!(pts.len(), 4);
    assert_pt(pts[0], (10.0, 10.0));
    assert_pt(pts[2], (60.0, 40.0));
    let pl = polylines(&convert_ok(&pdf, 1.0).dxf, "PDF_STROKE");
    assert!(pl[0].1, "a rectangle is closed");
}

#[test]
fn closepath_with_s_closes_the_subpath() {
    let pdf = page_pdf(&PageSpec::new(b"0 0 m 10 0 l 10 10 l s"));
    let (pts, closed) = polylines(&convert_ok(&pdf, 1.0).dxf, "PDF_STROKE").remove(0);
    assert_eq!(pts.len(), 3);
    assert!(closed);
}

#[test]
fn bezier_flattening_stays_within_the_chord_bound() {
    // Exact cubic: P0 (0,0), P1 (0,100), P2 (100,100), P3 (100,0).
    let p = [(0.0, 0.0), (0.0, 100.0), (100.0, 100.0), (100.0, 0.0)];
    let pdf = page_pdf(&PageSpec::new(b"0 0 m 0 100 100 100 100 0 c S"));
    for tol in [0.01, 0.05, 0.5] {
        let mut opts = Options::new(Scale::MmPerPoint(1.0));
        opts.tolerance_mm = tol;
        let c = convert(&pdf, &opts).expect("converts");
        let (pts, _) = polylines(&c.dxf, "PDF_STROKE").remove(0);
        let mut worst = 0.0f64;
        for i in 0..=4000 {
            let q = exact_cubic(p, f64::from(i) / 4000.0);
            worst = worst.max(dist_to_polyline(q, &pts));
        }
        eprintln!(
            "tolerance {tol} mm: measured worst chord error {worst:.6} mm, {} vertices",
            pts.len()
        );
        assert!(worst <= tol + 1e-5, "tolerance {tol}: measured {worst}");
    }
}

fn exact_cubic(p: [(f64, f64); 4], t: f64) -> (f64, f64) {
    let s = 1.0 - t;
    let (w0, w1, w2, w3) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
    (
        w0 * p[0].0 + w1 * p[1].0 + w2 * p[2].0 + w3 * p[3].0,
        w0 * p[0].1 + w1 * p[1].1 + w2 * p[2].1 + w3 * p[3].1,
    )
}

fn dist_to_polyline(q: (f64, f64), poly: &[(f64, f64)]) -> f64 {
    let mut best = f64::MAX;
    for w in poly.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let l2 = vx * vx + vy * vy;
        let t = if l2 > 0.0 {
            (((q.0 - a.0) * vx + (q.1 - a.1) * vy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        best = best.min((q.0 - (a.0 + vx * t)).hypot(q.1 - (a.1 + vy * t)));
    }
    best
}

#[test]
fn nested_transforms_are_applied_exactly() {
    // CTM after both cm: translate(5,5) applied first, then scale 2.
    let pdf = page_pdf(&PageSpec::new(
        b"q 2 0 0 2 0 0 cm 1 0 0 1 5 5 cm 0 0 m 1 1 l S Q",
    ));
    let pts = stroke_points(&convert_ok(&pdf, 1.0).dxf);
    assert_pt(pts[0], (10.0, 10.0));
    assert_pt(pts[1], (12.0, 12.0));
}

#[test]
fn rotation_maps_points_to_the_upright_page() {
    // Visible page 200 x 100 points; the point (10, 20) and (30, 20) are the test points.
    for (rotate, want_a, want_b, size) in [
        (0, (10.0, 20.0), (30.0, 20.0), (200.0, 100.0)),
        (90, (20.0, 190.0), (20.0, 170.0), (100.0, 200.0)),
        (180, (190.0, 80.0), (170.0, 80.0), (200.0, 100.0)),
        (270, (80.0, 10.0), (80.0, 30.0), (100.0, 200.0)),
    ] {
        let mut spec = PageSpec::new(b"10 20 m 30 20 l S");
        spec.media = [0.0, 0.0, 200.0, 100.0];
        spec.rotate = rotate;
        let c = convert_ok(&page_pdf(&spec), 1.0);
        let pts = stroke_points(&c.dxf);
        assert_pt(pts[0], want_a);
        assert_pt(pts[1], want_b);
        assert_eq!(c.report.page_size_mm, Some(size), "rotation {rotate}");
    }
}

#[test]
fn media_box_origin_and_crop_box_offset() {
    let mut spec = PageSpec::new(b"50 60 m 60 60 l S");
    spec.media = [0.0, 0.0, 300.0, 300.0];
    spec.crop = Some([50.0, 60.0, 250.0, 260.0]);
    let c = convert_ok(&page_pdf(&spec), 1.0);
    let pts = stroke_points(&c.dxf);
    assert_pt(pts[0], (0.0, 0.0));
    assert_pt(pts[1], (10.0, 0.0));
    assert_eq!(c.report.page_size_mm, Some((200.0, 200.0)));
}

#[test]
fn media_box_with_nonzero_origin_is_normalised() {
    let mut spec = PageSpec::new(b"110 120 m 120 120 l S");
    spec.media = [100.0, 100.0, 300.0, 300.0];
    let pts = stroke_points(&convert_ok(&page_pdf(&spec), 1.0).dxf);
    assert_pt(pts[0], (10.0, 20.0));
}

#[test]
fn user_unit_scales_user_space() {
    let mut spec = PageSpec::new(b"10 20 m 30 20 l S");
    spec.user_unit = Some(2.0);
    let pts = stroke_points(&convert_ok(&page_pdf(&spec), 1.0).dxf);
    assert_pt(pts[0], (20.0, 40.0));
    assert_pt(pts[1], (60.0, 40.0));
}

#[test]
fn form_xobject_matrix_and_outer_transform_compose() {
    let form = |pdf: &mut Pdf| {
        let f = form_xobject(
            pdf,
            [0.0, 0.0, 200.0, 200.0],
            [1.0, 0.0, 0.0, 1.0, 100.0, 0.0],
            b"0 0 m 10 0 l S",
            "",
        );
        format!("/XObject << /Fm1 {f} 0 R >>")
    };
    let pdf = page_with_resources(
        b"q 1 0 0 1 0 50 cm /Fm1 Do Q",
        [0.0, 0.0, 200.0, 200.0],
        form,
    );
    let c = convert_ok(&pdf, 1.0);
    let pts = stroke_points(&c.dxf);
    assert_pt(pts[0], (100.0, 50.0));
    assert_pt(pts[1], (110.0, 50.0));
    assert_eq!(c.report.stats.get("forms"), 1);
}

#[test]
fn dashed_stroke_goes_to_dash_layer_as_pieces() {
    let pdf = page_pdf(&PageSpec::new(b"[3 2] 0 d 0 0 m 10 0 l S"));
    let c = convert_ok(&pdf, 1.0);
    assert!(polylines(&c.dxf, "PDF_STROKE").is_empty());
    let pieces = polylines(&c.dxf, "PDF_DASHED");
    assert_eq!(pieces.len(), 2);
    assert_pt(pieces[0].0[0], (0.0, 0.0));
    assert_pt(pieces[0].0[1], (3.0, 0.0));
    assert_pt(pieces[1].0[0], (5.0, 0.0));
    assert_pt(pieces[1].0[1], (8.0, 0.0));
}

#[test]
fn filled_paths_go_to_fill_layer_and_can_be_switched_off() {
    let content = b"0 0 m 10 0 l 10 10 l h f 0 0 m 1 1 l S";
    let pdf = page_pdf(&PageSpec::new(content));
    let c = convert_ok(&pdf, 1.0);
    let fill = polylines(&c.dxf, "PDF_FILL");
    assert_eq!(fill.len(), 1);
    assert!(fill[0].1 && fill[0].0.len() == 3);
    let mut opts = Options::new(Scale::MmPerPoint(1.0));
    opts.include_fill = false;
    let off = convert(&pdf, &opts).expect("still converts: the stroke remains");
    assert!(polylines(&off.dxf, "PDF_FILL").is_empty());
    assert_eq!(polylines(&off.dxf, "PDF_STROKE").len(), 1);
}

#[test]
fn text_in_winansi_is_placed_at_its_origin_with_font_size_height() {
    let content = b"0 0 m 1 1 l S BT /F1 12 Tf 10 20 Td (Hello) Tj ET";
    let pdf = page_with_resources(content, [0.0, 0.0, 200.0, 200.0], |pdf| {
        let f = pdf.add(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        );
        format!("/Font << /F1 {f} 0 R >>")
    });
    let c = convert_ok(&pdf, 1.0);
    let t = texts(&c.dxf, "PDF_TEXT");
    assert_eq!(t.len(), 1);
    assert_pt(t[0].0, (10.0, 20.0));
    assert!(close(t[0].1, 12.0));
    assert_eq!(t[0].2, "Hello");
}

#[test]
fn text_advances_use_widths_and_tj_adjustments() {
    let content = b"0 0 m 1 1 l S BT /F1 12 Tf 10 20 Td [(A) -1000 (B)] TJ ET";
    let pdf = page_with_resources(content, [0.0, 0.0, 200.0, 200.0], |pdf| {
        let f = pdf.add(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding /FirstChar 65 /LastChar 66 /Widths [722 667] >>",
        );
        format!("/Font << /F1 {f} 0 R >>")
    });
    let t = texts(&convert_ok(&pdf, 1.0).dxf, "PDF_TEXT");
    assert_eq!(t.len(), 2);
    assert_pt(t[0].0, (10.0, 20.0));
    // A advances 0.722 * 12; the TJ value -1000 moves right by 12.
    assert_pt(t[1].0, (10.0 + 0.722 * 12.0 + 12.0, 20.0));
    assert_eq!(t[1].2, "B");
}

#[test]
fn text_after_unknown_advance_is_skipped_and_counted() {
    // Helvetica without /Widths: the advance is unknown, so the second string cannot be placed.
    let content = b"0 0 m 1 1 l S BT /F1 12 Tf 10 20 Td (A) Tj (B) Tj ET";
    let pdf = page_with_resources(content, [0.0, 0.0, 200.0, 200.0], |pdf| {
        let f = pdf.add(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        );
        format!("/Font << /F1 {f} 0 R >>")
    });
    let c = convert_ok(&pdf, 1.0);
    assert_eq!(texts(&c.dxf, "PDF_TEXT").len(), 1);
    assert_eq!(
        c.report.stats.text_skipped.get("position_unknown"),
        Some(&1)
    );
}

#[test]
fn non_ascii_text_is_replaced_and_reported() {
    let content = b"0 0 m 1 1 l S BT /F1 12 Tf 10 20 Td (caf\\351) Tj ET";
    let pdf = page_with_resources(content, [0.0, 0.0, 200.0, 200.0], |pdf| {
        let f = pdf.add(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        );
        format!("/Font << /F1 {f} 0 R >>")
    });
    let c = convert_ok(&pdf, 1.0);
    let t = texts(&c.dxf, "PDF_TEXT");
    assert_eq!(t[0].2, "caf?");
    assert!(c.report.stats.warnings.contains_key("text_chars_replaced"));
}

#[test]
fn compressed_content_gives_the_same_geometry() {
    let plain = convert_ok(&page_pdf(&PageSpec::new(b"10 20 m 110 20 l S")), 1.0);
    let mut spec = PageSpec::new(b"10 20 m 110 20 l S");
    spec.flate = true;
    let packed = convert_ok(&page_pdf(&spec), 1.0);
    // The header carries the input's SHA-256, which differs; the geometry must not.
    assert_eq!(
        polylines(&plain.dxf, "PDF_STROKE"),
        polylines(&packed.dxf, "PDF_STROKE")
    );
}

#[test]
fn xref_stream_and_object_stream_files_give_the_same_geometry() {
    let spec = PageSpec::new(b"10 20 m 110 20 l S");
    let plain = convert_ok(&page_pdf(&spec), 1.0);
    let xs = convert_ok(&page_pdf_xref_stream(&spec), 1.0);
    let os = convert_ok(&page_pdf_objstm(&spec), 1.0);
    assert_eq!(
        polylines(&plain.dxf, "PDF_STROKE"),
        polylines(&xs.dxf, "PDF_STROKE"),
        "xref-stream file"
    );
    assert_eq!(
        polylines(&plain.dxf, "PDF_STROKE"),
        polylines(&os.dxf, "PDF_STROKE"),
        "object-stream file"
    );
}

#[test]
fn image_and_paths_convert_paths_and_count_the_image() {
    let content = b"q 20 0 0 20 150 150 cm /Im1 Do Q 0 0 m 10 0 l S";
    let pdf = page_with_resources(content, [0.0, 0.0, 200.0, 200.0], |pdf| {
        let img = pdf.stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            &[0u8],
            false,
        );
        format!("/XObject << /Im1 {img} 0 R >>")
    });
    let c = convert_ok(&pdf, 1.0);
    assert_eq!(stroke_points(&c.dxf).len(), 2);
    assert_eq!(c.report.stats.get("images"), 1);
}

#[test]
fn unbalanced_restore_and_unknown_operators_are_warnings_not_refusals() {
    let pdf = page_pdf(&PageSpec::new(b"Q 0 0 m 5 0 l S zz 1 2 3 foo"));
    let c = convert_ok(&pdf, 1.0);
    assert_eq!(stroke_points(&c.dxf).len(), 2);
    assert!(c.report.stats.warnings.contains_key("unbalanced_restore"));
    assert!(c.report.stats.warnings.contains_key("unknown_operator"));
}
