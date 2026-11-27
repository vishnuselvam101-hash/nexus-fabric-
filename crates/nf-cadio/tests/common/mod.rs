//! Shared helpers for the nf-cadio tests.

#![allow(dead_code)]

/// Turns a test string written with LF line ends into CRLF.
pub fn crlf(s: &str) -> String {
    s.replace('\n', "\r\n")
}

/// A small hand-written R12 (AC1009) file in CRLF style: header with $INSUNITS, one LINE.
pub fn r12_sample() -> String {
    crlf(
        "  0\n\
         SECTION\n\
         \x20 2\n\
         HEADER\n\
         \x20 9\n\
         $ACADVER\n\
         \x20 1\n\
         AC1009\n\
         \x20 9\n\
         $INSUNITS\n\
         \x20 70\n\
         4\n\
         \x20 9\n\
         $EXTMIN\n\
         \x20 10\n\
         0.0\n\
         \x20 20\n\
         0.0\n\
         \x20 0\n\
         ENDSEC\n\
         \x20 0\n\
         SECTION\n\
         \x20 2\n\
         ENTITIES\n\
         \x20 0\n\
         LINE\n\
         \x20 8\n\
         CUT\n\
         \x20 10\n\
         0.0\n\
         \x20 20\n\
         0.0\n\
         \x20 11\n\
         10.0\n\
         \x20 21\n\
         5.0\n\
         \x20 0\n\
         ENDSEC\n\
         \x20 0\n\
         EOF\n",
    )
}

/// A DXF file as CRLF text: a HEADER section with `header` (code/value lines), then `sections`
/// (complete sections, see [`section`]).
pub fn dxf(header: &str, sections: &str) -> String {
    crlf(&format!(
        "0\nSECTION\n2\nHEADER\n{header}0\nENDSEC\n{sections}0\nEOF\n"
    ))
}

/// One complete section: `0 SECTION`, `2 name`, the body (code/value lines), `0 ENDSEC`.
pub fn section(name: &str, body: &str) -> String {
    format!("0\nSECTION\n2\n{name}\n{body}0\nENDSEC\n")
}

/// The HEADER variables for a file with the given version and units (`$INSUNITS`, or none for None).
pub fn header(version: &str, insunits: Option<i64>) -> String {
    let units = match insunits {
        Some(code) => format!("9\n$INSUNITS\n70\n{code}\n"),
        None => String::new(),
    };
    format!("9\n$ACADVER\n1\n{version}\n{units}")
}

/// Splits `template` (CRLF text, for example from [`dxf`]) at `marker` and puts `inserted` bytes between
/// the parts. For fixtures whose strings are not UTF-8, such as Shift_JIS.
pub fn with_bytes(template: &str, marker: &str, inserted: &[u8]) -> Vec<u8> {
    let (before, after) = template
        .split_once(marker)
        .expect("marker is in the template");
    let mut out = before.as_bytes().to_vec();
    out.extend_from_slice(inserted);
    out.extend_from_slice(after.as_bytes());
    out
}
