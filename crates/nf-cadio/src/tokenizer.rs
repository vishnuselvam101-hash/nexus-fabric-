//! Group code and value pairs of an ASCII DXF file.

use crate::error::{CadError, Reason};

/// Highest group code in DXF (extended data uses 1000 to 1071).
pub(crate) const MAX_GROUP_CODE: u16 = 1071;

/// One group code and its value. `value` is the value line without its line terminator. Bytes outside
/// ASCII are kept one char per byte (Latin-1), so the original bytes can be recovered for a later
/// encoding step (`$DWGCODEPAGE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub code: u16,
    pub value: String,
    /// 1-based line of the group code (the value is on the next line).
    pub line: usize,
}

/// Calls `visit` with every pair of `input`, in order. Accepts CRLF and LF line ends, and codes padded
/// with spaces or tabs. Reads one line at a time: no table of lines is built, and the pair count is
/// checked before each pair is handed on, so `visit` never sees more than `max_pairs` pairs.
pub(crate) fn for_each_pair(
    input: &[u8],
    max_pairs: usize,
    mut visit: impl FnMut(Pair) -> Result<(), CadError>,
) -> Result<(), CadError> {
    if let Some(pos) = input.iter().position(|&b| b == 0) {
        let line = input[..pos].iter().filter(|&&b| b == b'\n').count() + 1;
        return Err(CadError::at(line, Reason::NulByte));
    }
    // Blank lines at the end of the file are not data. Keep the input up to the end of the last line
    // that has a non-blank byte.
    let Some(last) = input.iter().rposition(|b| !b.is_ascii_whitespace()) else {
        return Ok(());
    };
    let end = input[last..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(input.len(), |off| last + off);
    let text = &input[..end];
    let line_count = text.iter().filter(|&&b| b == b'\n').count() + 1;
    if line_count % 2 == 1 {
        return Err(CadError::at(line_count, Reason::TruncatedPair));
    }
    let mut lines = text
        .split(|&b| b == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line));
    let mut count = 0usize;
    let mut line = 1usize;
    while let Some(code_line) = lines.next() {
        let Some(value_line) = lines.next() else {
            return Err(CadError::at(line, Reason::TruncatedPair));
        };
        count += 1;
        if count > max_pairs {
            return Err(CadError::at(
                line,
                Reason::TooManyPairs { limit: max_pairs },
            ));
        }
        let code = parse_code(code_line).map_err(|reason| CadError::at(line, reason))?;
        visit(Pair {
            code,
            value: latin1(value_line),
            line,
        })?;
        line += 2;
    }
    Ok(())
}

fn parse_code(line: &[u8]) -> Result<u16, Reason> {
    let text = line.trim_ascii();
    let digits = !text.is_empty() && text.len() <= 4 && text.iter().all(u8::is_ascii_digit);
    let code = if digits {
        std::str::from_utf8(text)
            .ok()
            .and_then(|t| t.parse::<u16>().ok())
    } else {
        None
    };
    match code {
        Some(code) if code <= MAX_GROUP_CODE => Ok(code),
        _ => Err(Reason::InvalidGroupCode {
            text: latin1(&line[..line.len().min(32)]),
        }),
    }
}

/// Bytes as chars, one per byte. Never fails.
pub(crate) fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}
