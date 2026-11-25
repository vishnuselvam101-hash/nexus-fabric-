//! Shared helpers for the nf-cadio tests.

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
