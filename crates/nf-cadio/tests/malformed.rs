//! Malformed and over-limit input is refused with a reason and a line number. Nothing panics.

mod common;

use nf_cadio::{CadError, Document, Limits, Reason};

fn refuse(text: &[u8]) -> CadError {
    match Document::parse(text) {
        Ok(_) => panic!("expected a refusal for {:?}", String::from_utf8_lossy(text)),
        Err(e) => e,
    }
}

fn refuse_with(text: &[u8], limits: &Limits) -> CadError {
    match Document::parse_with(text, limits) {
        Ok(_) => panic!("expected a refusal under {limits:?}"),
        Err(e) => e,
    }
}

#[test]
fn every_truncation_before_eof_is_refused() {
    let full = common::r12_sample();
    let eof_at = full.rfind("  0\r\nEOF").expect("sample ends with EOF");
    for cut in 0..eof_at + 8 {
        let prefix = &full.as_bytes()[..cut];
        assert!(
            Document::parse(prefix).is_err(),
            "prefix of {cut} bytes was accepted"
        );
    }
    // The whole file, and the file without its last CRLF, are complete.
    assert!(Document::parse(full.as_bytes()).is_ok());
    assert!(Document::parse(&full.as_bytes()[..full.len() - 2]).is_ok());
}

#[test]
fn odd_number_of_lines_is_a_truncated_pair() {
    let e = refuse(b"  0\nSECTION\n  2\n");
    assert!(matches!(e.reason, Reason::TruncatedPair), "{e:?}");
    assert_eq!(e.line, 3);
}

#[test]
fn non_numeric_group_code_is_refused_with_its_line() {
    let e = refuse(b"  0\nSECTION\nX2\nHEADER\n  0\nENDSEC\n  0\nEOF\n");
    assert_eq!(e.line, 3);
    assert!(matches!(e.reason, Reason::InvalidGroupCode { .. }), "{e:?}");
}

#[test]
fn out_of_range_and_signed_codes_are_refused() {
    for bad in ["1072", "-1", "+5", "12345", "", "   ", "1.5", "0x10"] {
        let text = format!("  0\nEOF\n{bad}\nx\n");
        let e = refuse(text.as_bytes());
        assert_eq!(e.line, 3, "code {bad:?}: {e:?}");
        assert!(
            matches!(e.reason, Reason::InvalidGroupCode { .. }),
            "code {bad:?}: {e:?}"
        );
    }
}

#[test]
fn long_bad_code_line_is_truncated_in_the_message() {
    let junk = "Z".repeat(10_000);
    let text = format!("{junk}\nvalue\n  0\nEOF\n");
    let e = refuse(text.as_bytes());
    match e.reason {
        Reason::InvalidGroupCode { text } => assert_eq!(text.len(), 32),
        other => panic!("{other:?}"),
    }
}

#[test]
fn nul_byte_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nHEADER\n\0\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::NulByte), "{e:?}");
    assert_eq!(e.line, 5);
}

#[test]
fn missing_eof_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nHEADER\n  0\nENDSEC\n");
    assert!(matches!(e.reason, Reason::MissingEof), "{e:?}");
}

#[test]
fn empty_input_is_refused_not_panicking() {
    let e = refuse(b"");
    assert!(matches!(e.reason, Reason::MissingEof), "{e:?}");
    assert_eq!(e.line, 0);
}

#[test]
fn section_not_closed_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 7);
}

#[test]
fn nested_section_is_refused() {
    let e =
        refuse(b"  0\nSECTION\n  2\nHEADER\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 5);
}

#[test]
fn section_without_name_is_refused() {
    let e = refuse(b"  0\nSECTION\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 1);
}

#[test]
fn data_after_eof_is_refused() {
    let e = refuse(b"  0\nEOF\n  0\nSECTION\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 3);
}

#[test]
fn record_outside_a_section_is_refused() {
    let e = refuse(b"  0\nLINE\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 1);
}

#[test]
fn header_value_without_variable_name_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nHEADER\n 70\n4\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 5);
}

#[test]
fn header_name_without_dollar_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nHEADER\n  9\nINSUNITS\n 70\n4\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 5);
}

#[test]
fn non_numeric_insunits_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\nmm\n  0\nENDSEC\n  0\nEOF\n");
    assert!(
        matches!(e.reason, Reason::InvalidValue { code: 70, .. }),
        "{e:?}"
    );
    assert_eq!(e.line, 7);
}

#[test]
fn non_finite_coordinates_are_refused() {
    for bad in ["NaN", "inf", "-infinity", "1e999"] {
        let text = format!(
            "  0\nSECTION\n  2\nHEADER\n  9\n$EXTMIN\n 10\n{bad}\n 20\n0\n  0\nENDSEC\n  0\nEOF\n"
        );
        let e = refuse(text.as_bytes());
        assert!(
            matches!(e.reason, Reason::InvalidValue { code: 10, .. }),
            "{bad}: {e:?}"
        );
    }
}

#[test]
fn huge_integer_is_refused_not_overflowed() {
    let e = refuse(b"  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n99999999999999999999999\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::InvalidValue { .. }), "{e:?}");
}

#[test]
fn huge_declared_table_count_is_refused() {
    let text = "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n 70\n4000000000\n  0\nENDTAB\n  0\nENDSEC\n  0\nEOF\n";
    let e = refuse(text.as_bytes());
    assert!(
        matches!(e.reason, Reason::DeclaredCountTooLarge { .. }),
        "{e:?}"
    );
    assert_eq!(e.line, 9);
}

#[test]
fn negative_declared_table_count_is_refused() {
    let text = "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n 70\n-3\n  0\nENDTAB\n  0\nENDSEC\n  0\nEOF\n";
    let e = refuse(text.as_bytes());
    assert!(
        matches!(e.reason, Reason::InvalidValue { code: 70, .. }),
        "{e:?}"
    );
}

#[test]
fn entity_count_over_limit_is_refused_at_the_record() {
    let mut text = String::from("  0\nSECTION\n  2\nENTITIES\n");
    for _ in 0..5 {
        text.push_str("  0\nLINE\n");
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    let limits = Limits {
        max_entities: 4,
        ..Limits::default()
    };
    let e = refuse_with(text.as_bytes(), &limits);
    assert!(
        matches!(e.reason, Reason::TooManyEntities { limit: 4 }),
        "{e:?}"
    );
    assert_eq!(e.line, 5 + 2 * 4);
}

#[test]
fn entity_count_at_limit_is_accepted() {
    let mut text = String::from("  0\nSECTION\n  2\nENTITIES\n");
    for _ in 0..4 {
        text.push_str("  0\nLINE\n");
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    let limits = Limits {
        max_entities: 4,
        ..Limits::default()
    };
    assert!(Document::parse_with(text.as_bytes(), &limits).is_ok());
}

#[test]
fn byte_limit_is_refused_before_reading() {
    let text = common::r12_sample();
    let limits = Limits {
        max_bytes: 10,
        ..Limits::default()
    };
    let e = refuse_with(text.as_bytes(), &limits);
    assert_eq!(e.line, 0);
    assert!(
        matches!(e.reason, Reason::TooLarge { limit: 10, .. }),
        "{e:?}"
    );
}

#[test]
fn extended_data_nesting_over_limit_is_refused() {
    let limits = Limits {
        max_depth: 2,
        ..Limits::default()
    };
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n1002\n{\n1002\n{\n1002\n{\n1002\n}\n1002\n}\n1002\n}\n  0\nENDSEC\n  0\nEOF\n";
    let e = refuse_with(text.as_bytes(), &limits);
    assert!(matches!(e.reason, Reason::TooDeep { limit: 2 }), "{e:?}");
    assert_eq!(e.line, 11);
}

#[test]
fn deep_nesting_default_limit_is_refused_without_stack_growth() {
    let mut text = String::from("  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n");
    for _ in 0..100_000 {
        text.push_str("1002\n{\n");
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    let e = refuse(text.as_bytes());
    assert!(matches!(e.reason, Reason::TooDeep { .. }), "{e:?}");
}

#[test]
fn unmatched_closing_brace_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n1002\n}\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::UnbalancedBraces), "{e:?}");
    assert_eq!(e.line, 7);
}

#[test]
fn unclosed_brace_at_record_boundary_is_refused() {
    // The next record (line 9) starts while the brace is still open.
    let e = refuse(
        b"  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n1002\n{\n  0\nLINE\n  0\nENDSEC\n  0\nEOF\n",
    );
    assert!(matches!(e.reason, Reason::UnbalancedBraces), "{e:?}");
    assert_eq!(e.line, 9);
}

#[test]
fn unclosed_brace_at_section_end_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n1002\n{\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::UnbalancedBraces), "{e:?}");
}

#[test]
fn extended_data_group_other_than_braces_is_refused() {
    let e = refuse(b"  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n1002\nAPP\n  0\nENDSEC\n  0\nEOF\n");
    assert!(matches!(e.reason, Reason::Structure { .. }), "{e:?}");
    assert_eq!(e.line, 7);
}

#[test]
fn refusal_display_names_the_line() {
    let e = refuse(b"  0\nSECTION\nX2\nHEADER\n");
    let shown = e.to_string();
    assert!(shown.contains("line 3"), "{shown}");
}
