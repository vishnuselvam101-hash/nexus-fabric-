//! `$DWGCODEPAGE` (ANSI_932, ANSI_1252), UTF-8 from AC1021, and the TEXT/MTEXT escapes, from fixtures
//! whose strings are written as bytes.

mod common;

use nf_cadio::{CadError, Document, Drawing, Entity, Reason};

/// "切断 SS400 又は STW400" in Shift_JIS (code page 932), written by Python's cp932 codec.
const SJIS_LINE: &[u8] =
    b"\x90\xD8\x92\x66\x20\x53\x53\x34\x30\x30\x20\x96\x94\x82\xCD\x20\x53\x54\x57\x34\x30\x30";
/// The same string in UTF-8.
const UTF8_LINE: &[u8] = b"\xE5\x88\x87\xE6\x96\xAD\x20\x53\x53\x34\x30\x30\x20\xE5\x8F\x88\xE3\x81\xAF\x20\x53\x54\x57\x34\x30\x30";
const TEXT: &str = "切断 SS400 又は STW400";

/// A file with one TEXT entity whose string is `marker` in the template, replaced by `bytes`.
fn text_file(version: &str, codepage: Option<&str>, bytes: &[u8]) -> Vec<u8> {
    let mut header = common::header(version, Some(4));
    if let Some(cp) = codepage {
        header.push_str(&format!("9\n$DWGCODEPAGE\n3\n{cp}\n"));
    }
    let body = common::section(
        "ENTITIES",
        "0\nTEXT\n8\nNOTE\n10\n0\n20\n0\n40\n2\n1\n@TXT@\n",
    );
    common::with_bytes(&common::dxf(&header, &body), "@TXT@", bytes)
}

fn drawing_of(bytes: &[u8]) -> Result<Drawing, CadError> {
    Document::parse(bytes)?.drawing()
}

fn text_of(drawing: &Drawing) -> (String, String) {
    match &drawing.entities[0] {
        Entity::Text { raw, plain, .. } => (raw.clone(), plain.clone()),
        other => panic!("expected TEXT, got {other:?}"),
    }
}

#[test]
fn ansi_932_japanese_text_decodes_exactly() {
    let mut bytes = text_file("AC1009", Some("ANSI_932"), SJIS_LINE);
    let drawing = drawing_of(&bytes).expect("Shift_JIS text");
    assert_eq!(text_of(&drawing), (TEXT.to_string(), TEXT.to_string()));
    // UTF-8 bytes read in the Shift_JIS code page do not give the same text.
    bytes = text_file("AC1009", Some("ANSI_932"), UTF8_LINE);
    let got = drawing_of(&bytes).ok().map(|d| text_of(&d).0);
    assert_ne!(got.as_deref(), Some(TEXT));
}

#[test]
fn ansi_1252_latin_text_decodes_exactly() {
    let bytes = text_file("AC1009", Some("ANSI_1252"), b"Caf\xE9 \x80");
    let drawing = drawing_of(&bytes).expect("Windows-1252 text");
    assert_eq!(text_of(&drawing).0, "Café €");
}

#[test]
fn utf8_is_used_from_ac1021_whatever_the_code_page() {
    let bytes = text_file("AC1021", Some("ANSI_932"), UTF8_LINE);
    let drawing = drawing_of(&bytes).expect("UTF-8 text");
    assert_eq!(text_of(&drawing).0, TEXT);
    let bytes = text_file("AC1021", None, UTF8_LINE);
    assert_eq!(text_of(&drawing_of(&bytes).unwrap()).0, TEXT);
}

#[test]
fn bytes_invalid_in_the_file_encoding_are_refused() {
    let bytes = text_file("AC1021", None, b"\xFF\xFE");
    let err = drawing_of(&bytes).expect_err("not UTF-8");
    assert_eq!(err.reason, Reason::InvalidText);
    let bytes = text_file("AC1009", Some("ANSI_932"), b"\x81\r");
    let err = drawing_of(&bytes).expect_err("lone Shift_JIS lead byte");
    assert_eq!(err.reason, Reason::InvalidText);
}

#[test]
fn unknown_code_page_and_missing_code_page_refuse_non_ascii_text() {
    let bytes = text_file("AC1009", Some("ANSI_936"), SJIS_LINE);
    let err = drawing_of(&bytes).expect_err("code page not decoded");
    assert_eq!(
        err.reason,
        Reason::UnknownEncoding {
            codepage: "ANSI_936".into()
        }
    );
    let bytes = text_file("AC1009", None, b"Caf\xE9");
    let err = drawing_of(&bytes).expect_err("no code page");
    assert_eq!(err.reason, Reason::NoEncoding);
    // ASCII text needs no code page.
    let bytes = text_file("AC1009", None, b"PART 1");
    assert_eq!(text_of(&drawing_of(&bytes).unwrap()).0, "PART 1");
}

#[test]
fn unicode_escape_decodes_to_the_character() {
    let bytes = text_file("AC1009", Some("ANSI_932"), b"\\U+5207\\U+65AD");
    let drawing = drawing_of(&bytes).expect("escaped text");
    assert_eq!(
        text_of(&drawing),
        ("\\U+5207\\U+65AD".into(), "切断".into())
    );
}

#[test]
fn two_byte_escape_decodes_in_the_file_code_page() {
    // \M+1 then four hex digits: the Shift_JIS pair 0x9266 is the character 断.
    let bytes = text_file("AC1009", Some("ANSI_932"), b"\\M+19266");
    let drawing = drawing_of(&bytes).expect("two-byte escape");
    assert_eq!(text_of(&drawing).1, "断");
}

#[test]
fn malformed_escapes_are_kept_as_written() {
    let bytes = text_file("AC1009", Some("ANSI_932"), b"\\U+ZZZZ \\M+2");
    let drawing = drawing_of(&bytes).expect("kept literally");
    assert_eq!(text_of(&drawing).1, "\\U+ZZZZ \\M+2");
}
