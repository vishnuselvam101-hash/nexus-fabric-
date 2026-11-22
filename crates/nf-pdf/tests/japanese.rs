//! Japanese drawings (work plan 5J.3): Type0 fonts with ToUnicode, full-width digits and dimension
//! symbols, vertical writing. Every PDF is generated here; no font file is used. A glyph whose
//! text is unknown is refused, never guessed.

mod common;

use common::*;
use nf_pdf::{convert, Converted};

const MEDIA: [f64; 4] = [0.0, 0.0, 200.0, 200.0];

/// A ToUnicode CMap with the given bfchar / bfrange sections.
fn cmap(sections: &str) -> Vec<u8> {
    format!(
        "/CIDInit /ProcSet findresource begin\n\
         1 begincodespacerange <0000> <FFFF> endcodespacerange\n{sections}\nend end"
    )
    .into_bytes()
}

/// A page whose font F1 is a Type0 font with a dummy CIDFontType2 descendant. `encoding` returns
/// the /Encoding value (a name, or a reference to an embedded CMap made with `pdf`).
fn cid_page(
    content: &[u8],
    encoding: impl FnOnce(&mut Pdf) -> String,
    to_unicode: Option<&[u8]>,
) -> Vec<u8> {
    page_with_resources(content, MEDIA, |pdf| {
        let enc = encoding(pdf);
        let cid = pdf.add(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Dummy \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 2 >> /DW 1000 >>",
        );
        let tu = match to_unicode {
            Some(body) => format!(" /ToUnicode {} 0 R", pdf.stream("", body, false)),
            None => String::new(),
        };
        let font = pdf.add(format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /Dummy /Encoding {enc} \
             /DescendantFonts [{cid} 0 R]{tu} >>"
        ));
        format!("/Font << /F1 {font} 0 R >>")
    })
}

/// One text object showing the given hex string in F1. A short line is drawn first: a page with
/// no vector path is refused, so the text alone would not reach the text handling.
fn show(hex: &str) -> Vec<u8> {
    format!("0 0 m 5 0 l S BT /F1 10 Tf 10 100 Td <{hex}> Tj ET").into_bytes()
}

fn identity_h(_: &mut Pdf) -> String {
    "/Identity-H".into()
}

fn convert_mm(bytes: &[u8]) -> Converted {
    convert(bytes, &options(1.0)).expect("text problems are skipped, not refused")
}

fn text_of(c: &Converted) -> Vec<String> {
    texts(&c.dxf, "PDF_TEXT")
        .into_iter()
        .map(|(_, _, t)| t)
        .collect()
}

fn skipped(c: &Converted, reason: &str) -> u64 {
    c.report
        .stats
        .text_skipped
        .get(reason)
        .copied()
        .unwrap_or(0)
}

fn counted(c: &Converted, key: &str) -> u64 {
    c.report.stats.counts.get(key).copied().unwrap_or(0)
}

#[test]
fn identity_h_two_byte_codes_decode_through_tounicode() {
    let map = cmap("2 beginbfchar <0001> <FF11> <0002> <FF12> endbfchar");
    let bytes = cid_page(&show("00010002"), identity_h, Some(&map));
    let c = convert_mm(&bytes);
    // Full-width 1 and 2 are read as the digits 1 and 2.
    assert_eq!(text_of(&c), ["12"]);
    assert_eq!(counted(&c, "text_chars_normalised"), 2);
}

#[test]
fn full_width_dimension_text_is_read_as_ascii_and_symbols_are_counted() {
    // 1 R, 2 full-width digits, 3 full stop, 4 minus sign, 5 diameter sign, 6 degree sign.
    let map = cmap(
        "10 beginbfchar \
         <0001> <FF32> <0002> <FF11> <0003> <FF12> <0004> <FF0E> <0005> <FF15> \
         <0006> <2212> <0007> <FF12> <0008> <03C6> <0009> <00D8> <000A> <00B0> <000B> <FF0B> \
         endbfchar",
    );
    let content = [
        show("00010002000300040005"),
        show("000800020005"),
        show("00060003"),
        show("0009"),
    ]
    .concat();
    let c = convert_mm(&cid_page(&content, identity_h, Some(&map)));
    // Each show is one TEXT entity: R12.5, then phi 1 5, then minus 2, then Ø.
    assert_eq!(text_of(&c), ["R12.5", "?15", "-2", "?"]);
    // R 1 2 . 5 (five), 1 and 5 (two), minus and 2 (two).
    assert_eq!(counted(&c, "text_chars_normalised"), 9);
    assert_eq!(counted(&c, "text_chars_drawing_symbol"), 2);
}

#[test]
fn an_unmapped_code_inside_a_number_refuses_the_whole_string() {
    let map = cmap("2 beginbfchar <0001> <FF11> <0003> <FF13> endbfchar");
    let c = convert_mm(&cid_page(&show("000100020003"), identity_h, Some(&map)));
    assert!(text_of(&c).is_empty(), "a guessed digit was written");
    assert_eq!(skipped(&c, "undecodable_encoding"), 1);
}

#[test]
fn an_unreadable_bfrange_entry_is_unmapped_not_an_empty_string() {
    // The middle code of the range maps to a lone surrogate, which is not text.
    let map = cmap("1 beginbfrange <0001> <0003> [<FF11> <D800> <FF13>] endbfrange");
    let c = convert_mm(&cid_page(&show("000100020003"), identity_h, Some(&map)));
    assert!(
        text_of(&c).is_empty(),
        "the unreadable entry was dropped: {:?}",
        text_of(&c)
    );
    assert_eq!(skipped(&c, "undecodable_encoding"), 1);
}

#[test]
fn an_odd_trailing_byte_is_not_looked_up_as_a_two_byte_code() {
    // The single byte 0x02 must not match the two-byte code 0x0002.
    let map = cmap("2 beginbfchar <0001> <0031> <0002> <0035> endbfchar");
    let c = convert_mm(&cid_page(&show("000102"), identity_h, Some(&map)));
    assert!(text_of(&c).is_empty(), "an odd byte was read as a code");
    assert_eq!(skipped(&c, "undecodable_encoding"), 1);
}

#[test]
fn identity_v_vertical_text_is_skipped_and_never_merged_into_a_number() {
    let map = cmap("2 beginbfchar <0001> <0031> <0002> <0032> endbfchar");
    let vertical = |_: &mut Pdf| "/Identity-V".to_string();
    let c = convert_mm(&cid_page(&show("00010002"), vertical, Some(&map)));
    assert!(text_of(&c).is_empty());
    assert_eq!(skipped(&c, "vertical_writing"), 1);
}

#[test]
fn an_embedded_vertical_cmap_is_skipped_as_predefined() {
    let map = cmap("2 beginbfchar <0001> <0031> <0002> <0032> endbfchar");
    let embedded = |pdf: &mut Pdf| {
        let n = pdf.stream(
            "/Type /CMap /CMapName /Test-V /WMode 1",
            b"/CIDInit /ProcSet findresource begin end",
            false,
        );
        format!("{n} 0 R")
    };
    let c = convert_mm(&cid_page(&show("00010002"), embedded, Some(&map)));
    assert!(text_of(&c).is_empty());
    assert_eq!(skipped(&c, "predefined_cmap"), 1);
}

#[test]
fn identity_h_without_tounicode_is_skipped() {
    let c = convert_mm(&cid_page(&show("00010002"), identity_h, None));
    assert!(text_of(&c).is_empty());
    assert_eq!(skipped(&c, "no_tounicode"), 1);
}
