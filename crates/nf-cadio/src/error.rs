//! Typed refusals. Every malformed or over-limit input is refused with a reason and a line number.

use std::fmt;

/// Why an input was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The input is longer than `Limits::max_bytes`. Reported at line 0, before any line is read.
    TooLarge { bytes: usize, limit: usize },
    /// A NUL byte. DXF text never contains one.
    NulByte,
    /// A group code line that is not an integer from 0 to 1071.
    InvalidGroupCode { text: String },
    /// A group code with no value line: an odd number of lines, or the file was cut short.
    TruncatedPair,
    /// The file does not end with a `0 EOF` record.
    MissingEof,
    /// A record that is out of place. `detail` says what was expected.
    Structure { detail: &'static str },
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
            Reason::TooLarge { bytes, limit } => {
                write!(f, "input is {bytes} bytes, over the limit of {limit}")
            }
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
            Reason::MissingEof => write!(f, "no 0 EOF record at the end"),
            Reason::Structure { detail } => write!(f, "{detail}"),
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
