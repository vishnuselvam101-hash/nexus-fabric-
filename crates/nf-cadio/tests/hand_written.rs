//! Hand-written DXF files in R12 and R2000 header styles, with CRLF and LF line ends.

mod common;

use nf_cadio::{Document, HeaderValue, Limits, Reason, Units};

fn names(doc: &Document) -> Vec<&str> {
    doc.sections.iter().map(|s| s.name.as_str()).collect()
}

#[test]
fn r12_crlf_sample_parses_sections_and_units() {
    let doc = Document::parse(common::r12_sample().as_bytes()).expect("valid R12 sample");
    assert_eq!(names(&doc), ["HEADER", "ENTITIES"]);
    assert_eq!(
        doc.variable("$ACADVER"),
        Some(&HeaderValue::Text("AC1009".to_string()))
    );
    assert_eq!(doc.variable("$INSUNITS"), Some(&HeaderValue::Int(4)));
    assert_eq!(doc.units(), Units::Millimeter);
    assert_eq!(doc.units().as_str(), "millimeter");
    assert_eq!(
        doc.variable("$EXTMIN"),
        Some(&HeaderValue::Point(vec![0.0, 0.0]))
    );
    let entities = doc.section("ENTITIES").expect("ENTITIES present");
    assert_eq!(entities.line, 21);
    let codes: Vec<(u16, &str)> = entities
        .pairs
        .iter()
        .map(|p| (p.code, p.value.as_str()))
        .collect();
    assert_eq!(codes[0], (0, "LINE"));
    assert_eq!(codes[1], (8, "CUT"));
}

#[test]
fn r2000_lf_file_with_codepage_units_and_raw_sections() {
    let text = r#"  0
SECTION
  2
HEADER
  9
$ACADVER
  1
AC1015
  9
$DWGCODEPAGE
  3
ANSI_1252
  9
$INSUNITS
 70
     6
  9
$EXTMIN
 10
0.0
 20
0.0
 30
0.0
  9
$EXTMAX
 10
120.5
 20
80.25
  0
ENDSEC
  0
SECTION
  2
CLASSES
  0
ENDSEC
  0
SECTION
  2
ENTITIES
  0
TEXT
  8
NOTES
 10
5.0
 20
5.0
 30
0.0
 40
2.5
  1
  Hello  world
  0
ENDSEC
  0
SECTION
  2
THUMBNAILIMAGE
 90
     4
 92
0
  0
ENDSEC
  0
EOF
"#;
    let doc = Document::parse(text.as_bytes()).expect("valid R2000 sample");
    assert_eq!(
        names(&doc),
        ["HEADER", "CLASSES", "ENTITIES", "THUMBNAILIMAGE"]
    );
    assert_eq!(doc.units(), Units::Meter);
    assert_eq!(
        doc.variable("$DWGCODEPAGE"),
        Some(&HeaderValue::Text("ANSI_1252".to_string()))
    );
    assert_eq!(
        doc.variable("$EXTMIN"),
        Some(&HeaderValue::Point(vec![0.0, 0.0, 0.0]))
    );
    assert_eq!(
        doc.variable("$EXTMAX"),
        Some(&HeaderValue::Point(vec![120.5, 80.25]))
    );
    // Leading spaces of a value line are part of the value; the code padding is not.
    let text_entity = doc.section("ENTITIES").unwrap();
    let hello = text_entity.pairs.iter().find(|p| p.code == 1).unwrap();
    assert_eq!(hello.value, "  Hello  world");
    // Unknown section kept raw, every pair in order.
    let thumb = doc.section("THUMBNAILIMAGE").unwrap();
    let raw: Vec<(u16, &str)> = thumb
        .pairs
        .iter()
        .map(|p| (p.code, p.value.as_str()))
        .collect();
    assert_eq!(raw, [(90, "     4"), (92, "0")]);
}

#[test]
fn padded_codes_tabs_and_crlf_mixed_with_lf() {
    let text = "\t  0\r\nSECTION\n   2   \r\nHEADER\n  9\r\n$INSUNITS\r\n 70 \r\n1\n  0\r\nENDSEC\n   0 \nEOF\n";
    let doc = Document::parse(text.as_bytes()).expect("padded codes are accepted");
    assert_eq!(doc.units(), Units::Inch);
    assert_eq!(doc.sections[0].name, "HEADER");
}

#[test]
fn leading_comments_and_blank_trailing_lines_are_accepted() {
    let text = "999\nnf comment: mm\n  0\nSECTION\n  2\nHEADER\n  0\nENDSEC\n  0\nEOF\n\n\n";
    let doc = Document::parse(text.as_bytes()).expect("comments are allowed");
    assert_eq!(doc.comments, ["nf comment: mm"]);
    assert_eq!(doc.sections.len(), 1);
}

#[test]
fn units_are_unknown_when_missing_or_zero() {
    let without = common::crlf(
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1009\n  0\nENDSEC\n  0\nEOF\n",
    );
    let doc = Document::parse(without.as_bytes()).unwrap();
    assert_eq!(doc.units(), Units::Unknown);
    assert_eq!(doc.units().as_str(), "unknown");

    let zero =
        common::crlf("  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n0\n  0\nENDSEC\n  0\nEOF\n");
    let doc = Document::parse(zero.as_bytes()).unwrap();
    assert_eq!(doc.units(), Units::Unknown);
}

#[test]
fn unrecognised_insunits_code_is_unknown_not_guessed() {
    let text = "  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n99\n  0\nENDSEC\n  0\nEOF\n";
    let doc = Document::parse(text.as_bytes()).unwrap();
    assert_eq!(doc.units(), Units::Unknown);
}

#[test]
fn duplicate_insunits_is_refused_not_resolved() {
    // $INSUNITS 4 (mm) then 1 (inch) would otherwise be resolved to one value without a word.
    let text = "  0\nSECTION\n  2\nHEADER\n  9\n$INSUNITS\n 70\n4\n  9\n$INSUNITS\n 70\n1\n  0\nENDSEC\n  0\nEOF\n";
    let e = Document::parse(text.as_bytes()).unwrap_err();
    assert_eq!(
        e.reason,
        Reason::DuplicateHeaderVariable {
            name: "$INSUNITS".to_string()
        }
    );
    assert_eq!(e.line, 9);
}

#[test]
fn file_without_sections_is_valid_when_it_ends_with_eof() {
    let doc = Document::parse(b"  0\nEOF\n").unwrap();
    assert!(doc.sections.is_empty());
    assert_eq!(doc.units(), Units::Unknown);
}

#[test]
fn default_limits_are_safe_and_nonzero() {
    let d = Limits::default();
    assert!(d.max_bytes >= 1 << 20);
    assert!(d.max_pairs >= 1_000_000);
    assert!(d.max_entities >= 100_000);
    assert!(d.max_depth >= 1);
}

#[test]
fn nested_extended_data_within_limit_is_accepted() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n1001\nAPP\n1002\n{\n1002\n{\n1002\n}\n1002\n}\n  0\nENDSEC\n  0\nEOF\n";
    let doc = Document::parse(text.as_bytes()).expect("balanced braces within depth");
    assert_eq!(doc.section("ENTITIES").unwrap().pairs.len(), 6);
}
