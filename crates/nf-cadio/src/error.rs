//! Typed refusals. Every malformed or over-limit input is refused with a reason and a line number.

use std::fmt;

/// Why an input was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// The input is longer than `Limits::max_bytes`. Reported at line 0, before any line is read.
    /// `bytes` is the input length; for a reader it is `limit + 1`, meaning "more than the limit".
    TooLarge { bytes: usize, limit: usize },
    /// The reader failed. Reported at line 0.
    ReadFailed { message: String },
    /// A NUL byte. DXF text never contains one.
    NulByte,
    /// A group code line that is not an integer from 0 to 1071.
    InvalidGroupCode { text: String },
    /// A group code with no value line: an odd number of lines, or the file was cut short.
    TruncatedPair,
    /// More group code and value pairs than `Limits::max_pairs`.
    TooManyPairs { limit: usize },
    /// The file does not end with a `0 EOF` record.
    MissingEof,
    /// A record after `0 EOF`.
    DataAfterEof,
    /// A record outside a section. Only `0 SECTION`, `0 EOF` and group 999 comments are allowed there.
    RecordOutsideSection,
    /// `0 SECTION` not followed by a group 2 name.
    SectionWithoutName,
    /// A section name with spaces or tabs around it. Names must match exactly, for example `ENTITIES`.
    SectionNameWhitespace,
    /// A section that reaches `0 SECTION` or `0 EOF`, or the end of the file, before `0 ENDSEC`.
    SectionNotClosed,
    /// A second HEADER section.
    DuplicateHeader,
    /// A record (group 0) inside the HEADER section.
    RecordInHeader,
    /// A header variable name that does not start with `$`.
    HeaderNameNotDollar,
    /// A header value that comes before any `9` variable name.
    HeaderValueWithoutName,
    /// A header variable set twice. The file would give two values for one variable.
    DuplicateHeaderVariable { name: String },
    /// Extended-data group 1002 that is neither `{` nor `}`.
    ExtendedDataGroup,
    /// A value that does not parse as the type its group code requires (integer or finite real).
    InvalidValue { code: u16, text: String },
    /// Extended-data `{` and `}` (group 1002) do not balance at a record or section boundary.
    UnbalancedBraces,
    /// Extended-data nesting is deeper than `Limits::max_depth`.
    TooDeep { limit: usize },
    /// More entity records (in ENTITIES and BLOCKS) than `Limits::max_entities`.
    TooManyEntities { limit: usize },
    /// A table declares more records than `Limits::max_entities`.
    DeclaredCountTooLarge { declared: i64, limit: usize },
    /// A value before the first record (group 0) of a section, other than a 999 comment.
    ExpectedRecord,
    /// A record lacks a group code it needs, for example a LINE without its start point.
    MissingField { entity: String, code: u16 },
    /// A VERTEX or SEQEND record with no POLYLINE before it.
    OrphanVertex,
    /// A POLYLINE that is not followed by its VERTEX records and a SEQEND.
    UnterminatedPolyline,
    /// Non-ASCII text in a code page this reader does not decode.
    UnknownEncoding { codepage: String },
    /// Non-ASCII text, but the file names no code page and is not UTF-8 (AC1021 or later).
    NoEncoding,
    /// Text whose bytes are not valid in the file's encoding.
    InvalidText,
}

/// A refusal: the reason and the 1-based line where it was found (0 when no line applies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadError {
    pub line: usize,
    pub reason: Reason,
}

impl CadError {
    pub(crate) fn at(line: usize, reason: Reason) -> CadError {
        CadError { line, reason }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::TooLarge { limit, .. } => {
                write!(f, "input is over the limit of {limit} bytes")
            }
            Reason::ReadFailed { message } => write!(f, "could not read the input: {message}"),
            Reason::NulByte => write!(f, "NUL byte in input"),
            Reason::InvalidGroupCode { text } => {
                write!(f, "group code is not an integer from 0 to 1071: {text:?}")
            }
            Reason::TruncatedPair => {
                write!(
                    f,
                    "group code without a value line (odd number of lines or cut short)"
                )
            }
            Reason::TooManyPairs { limit } => {
                write!(f, "more than {limit} group code and value pairs")
            }
            Reason::MissingEof => write!(f, "no 0 EOF record at the end"),
            Reason::DataAfterEof => write!(f, "data after 0 EOF"),
            Reason::RecordOutsideSection => write!(
                f,
                "record outside a section (expected 0 SECTION, 0 EOF or a 999 comment)"
            ),
            Reason::SectionWithoutName => write!(f, "SECTION without a name (group 2)"),
            Reason::SectionNameWhitespace => {
                write!(f, "section name has spaces or tabs around it")
            }
            Reason::SectionNotClosed => write!(f, "section not closed by 0 ENDSEC"),
            Reason::DuplicateHeader => write!(f, "second HEADER section"),
            Reason::RecordInHeader => write!(f, "record inside HEADER"),
            Reason::HeaderNameNotDollar => {
                write!(f, "header variable name must start with $")
            }
            Reason::HeaderValueWithoutName => {
                write!(f, "header value before any variable name")
            }
            Reason::DuplicateHeaderVariable { name } => {
                write!(f, "header variable {name} is set more than once")
            }
            Reason::ExtendedDataGroup => {
                write!(f, "extended-data group 1002 is neither {{ nor }}")
            }
            Reason::InvalidValue { code, text } => {
                write!(f, "value {text:?} is not valid for group code {code}")
            }
            Reason::UnbalancedBraces => write!(f, "extended-data braces do not balance"),
            Reason::TooDeep { limit } => {
                write!(
                    f,
                    "extended-data nesting is deeper than the limit of {limit}"
                )
            }
            Reason::TooManyEntities { limit } => {
                write!(f, "more than {limit} entity records")
            }
            Reason::DeclaredCountTooLarge { declared, limit } => {
                write!(
                    f,
                    "table declares {declared} records, over the limit of {limit}"
                )
            }
            Reason::ExpectedRecord => {
                write!(f, "value before the first record (group 0) of a section")
            }
            Reason::MissingField { entity, code } => {
                write!(f, "{entity} record has no group {code}")
            }
            Reason::OrphanVertex => write!(f, "VERTEX or SEQEND without a POLYLINE before it"),
            Reason::UnterminatedPolyline => {
                write!(f, "POLYLINE not followed by VERTEX records and SEQEND")
            }
            Reason::UnknownEncoding { codepage } => write!(
                f,
                "non-ASCII text in code page {codepage}, which this reader does not decode"
            ),
            Reason::NoEncoding => write!(
                f,
                "non-ASCII text, but the file names no code page and is not UTF-8 (AC1021 or later)"
            ),
            Reason::InvalidText => write!(f, "text is not valid in the file's code page"),
        }
    }
}

impl fmt::Display for CadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "refused: {}", self.reason)
        } else {
            write!(f, "refused at line {}: {}", self.line, self.reason)
        }
    }
}

impl std::error::Error for CadError {}
