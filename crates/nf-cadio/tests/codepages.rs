//! `$DWGCODEPAGE` (ANSI_932, ANSI_1252) and UTF-8 from AC1021, from fixtures whose layer names are
//! written as bytes. Non-ASCII text the file's encoding cannot give a refusal, never a replacement.

mod common;

use nf_cadio::{CadError, Document, Reason};

/// "切断 SS400 又は STW400" in Shift_JIS (code page 932), written by Python's cp932 codec.
const SJIS_NAME: &[u8] =
    b"\x90\xD8\x92\x66\x20\x53\x53\x34\x30\x30\x20\x96\x94\x82\xCD\x20\x53\x54\x57\x34\x30\x30";
/// The same string in UTF-8.
const UTF8_NAME: &[u8] = b"\xE5\x88\x87\xE6\x96\xAD\x20\x53\x53\x34\x30\x30\x20\xE5\x8F\x88\xE3\x81\xAF\x20\x53\x54\x57\x34\x30\x30";
const NAME: &str = "切断 SS400 又は STW400";

/// A file whose only LAYER is named by `bytes`.
fn layer_file(version: &str, codepage: Option<&str>, bytes: &[u8]) -> Vec<u8> {
    let mut header = common::header(version, Some(4));
    if let Some(cp) = codepage {
        header.push_str(&format!("9\n$DWGCODEPAGE\n3\n{cp}\n"));
    }
    let tables = common::section(
        "TABLES",
        "0\nTABLE\n2\nLAYER\n70\n1\n0\nLAYER\n2\n@NAME@\n62\n7\n0\nENDTAB\n",
    );
    common::with_bytes(&common::dxf(&header, &tables), "@NAME@", bytes)
}

fn layer_name(bytes: &[u8]) -> Result<String, CadError> {
    let doc = Document::parse(bytes)?;
    Ok(doc.drawing()?.layers[0].name.clone())
}

#[test]
fn ansi_932_japanese_name_decodes_exactly() {
    let bytes = layer_file("AC1009", Some("ANSI_932"), SJIS_NAME);
    assert_eq!(layer_name(&bytes).expect("Shift_JIS name"), NAME);
}

#[test]
fn ansi_1252_latin_name_decodes_exactly() {
    let bytes = layer_file("AC1009", Some("ANSI_1252"), b"Caf\xE9 \x80");
    assert_eq!(layer_name(&bytes).expect("Windows-1252 name"), "Café €");
}

#[test]
fn utf8_is_used_from_ac1021_whatever_the_code_page() {
    let bytes = layer_file("AC1021", Some("ANSI_932"), UTF8_NAME);
    assert_eq!(layer_name(&bytes).expect("UTF-8 name"), NAME);
    let bytes = layer_file("AC1021", None, UTF8_NAME);
    assert_eq!(layer_name(&bytes).expect("UTF-8 name"), NAME);
}

#[test]
fn bytes_invalid_in_the_file_encoding_are_refused() {
    let err = layer_name(&layer_file("AC1021", None, b"\xFF\xFE")).expect_err("not UTF-8");
    assert!(matches!(err.reason, Reason::InvalidText));
    // A lone Shift_JIS lead byte, followed by CR, is not a character.
    let err =
        layer_name(&layer_file("AC1009", Some("ANSI_932"), b"\x81\r")).expect_err("lead byte");
    assert!(matches!(err.reason, Reason::InvalidText));
}

#[test]
fn unknown_or_missing_code_page_refuses_non_ascii_names() {
    let err = layer_name(&layer_file("AC1009", Some("ANSI_936"), SJIS_NAME)).expect_err("936");
    assert_eq!(
        err.reason,
        Reason::UnknownEncoding {
            codepage: "ANSI_936".into()
        }
    );
    let err = layer_name(&layer_file("AC1009", None, b"Caf\xE9")).expect_err("no code page");
    assert!(matches!(err.reason, Reason::NoEncoding));
    assert_eq!(
        layer_name(&layer_file("AC1009", None, b"PART 1")).expect("ASCII needs no code page"),
        "PART 1"
    );
}
