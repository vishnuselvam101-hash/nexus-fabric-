//! Regressions found by the fuzzer. Each case must end in a refusal or a limit, quickly.

mod common;

use std::time::{Duration, Instant};

use common::*;
use nf_pdf::{convert, FailureKind};

#[test]
fn tiny_dash_pattern_on_a_long_path_is_bounded() {
    // A 1,000,000-unit line with a 2e-6 period would need 5e11 pieces: the dash walk must stop
    // at the path-segment budget, not grow the piece list without bound.
    let bytes = page_pdf(&PageSpec {
        media: [0.0, 0.0, 2000000.0, 200.0],
        ..PageSpec::new(b"[0.000001 0.000001] 0 d 0 0 m 1000000 0 l S")
    });
    let started = Instant::now();
    let f = convert(&bytes, &options(1.0)).expect_err("the dash walk is refused");
    assert_eq!(f.kind, FailureKind::LimitExceeded);
    assert_eq!(f.limit, Some("path_segments"));
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn zero_length_dash_entries_do_not_loop_forever() {
    let bytes = page_pdf(&PageSpec::new(b"[0 0 0 0] 0 d 0 0 m 5 0 l S"));
    // An all-zero pattern is treated as solid: the line is written, not refused.
    let c = convert(&bytes, &options(1.0)).expect("converts as a solid line");
    assert_eq!(polylines(&c.dxf, "PDF_STROKE").len(), 1);
}

#[test]
fn malformed_inline_image_dictionary_does_not_fail_the_conversion() {
    // The converter never reads inline image dictionaries; a stray `]` in one must not change
    // the outcome (it converted before the page-image renderer existed).
    let bytes = page_pdf(&PageSpec::new(
        b"q 10 0 0 10 0 0 cm BI /W 1 /H 1 /D [0 1 ] ] /BPC 8 ID \x00 EI Q 0 0 m 50 0 l S",
    ));
    let c = convert(&bytes, &options(1.0)).expect("converts as before");
    assert_eq!(polylines(&c.dxf, "PDF_STROKE").len(), 1);
}
