//! Real vector PDFs written by ReportLab (see tests/fixtures/gen_reportlab.py).

mod common;

use common::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()))
}

#[test]
fn reportlab_lines_rectangle_and_dashes() {
    let c = convert_ok(&fixture("reportlab_lines.pdf"), 1.0);
    let stroke = polylines(&c.dxf, "PDF_STROKE");
    // The bottom line and the rectangle.
    assert!(
        stroke.iter().any(|(p, closed)| !closed
            && p.len() == 2
            && close(p[0].0, 10.0)
            && close(p[0].1, 10.0)
            && close(p[1].0, 290.0)
            && close(p[1].1, 10.0)),
        "bottom line not found: {stroke:?}"
    );
    assert!(
        stroke.iter().any(|(p, closed)| *closed && p.len() == 4),
        "rectangle not found"
    );
    // The dashed line becomes several pieces on PDF_DASHED.
    let dashed = polylines(&c.dxf, "PDF_DASHED");
    assert!(dashed.len() > 10, "dash pieces: {}", dashed.len());
    assert!(dashed
        .iter()
        .all(|(p, _)| p.iter().all(|q| close(q.1, 150.0))));
}

#[test]
fn reportlab_curves_are_flattened_within_tolerance() {
    let c = convert_ok(&fixture("reportlab_curves.pdf"), 1.0);
    let longest = polylines(&c.dxf, "PDF_STROKE")
        .into_iter()
        .chain(polylines(&c.dxf, "PDF_FILL"))
        .map(|(p, _)| p.len())
        .max()
        .unwrap_or(0);
    assert!(
        longest > 20,
        "the Bezier is flattened into many chords: {longest}"
    );
    assert!(c.report.stats.get("path_segments") > 20);
}

#[test]
fn reportlab_text_is_placed() {
    let c = convert_ok(&fixture("reportlab_text.pdf"), 1.0);
    let t = texts(&c.dxf, "PDF_TEXT");
    let part = t
        .iter()
        .find(|(_, _, s)| s == "PART 42")
        .expect("PART 42 is placed");
    assert_pt(part.0, (20.0, 100.0));
    assert!(close(part.1, 12.0));
    // The second string may be decoded or skipped; either way it is accounted for.
    let accounted = t.len() as u64 + c.report.stats.text_skipped.values().sum::<u64>();
    assert!(accounted >= 2, "text items are accounted: {accounted}");
}
