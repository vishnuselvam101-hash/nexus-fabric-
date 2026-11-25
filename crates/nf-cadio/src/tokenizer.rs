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

/// Splits `input` into pairs. Accepts CRLF and LF line ends, and codes padded with spaces or tabs.
pub(crate) fn tokenize(input: &[u8]) -> Result<Vec<Pair>, CadError> {
    if let Some(pos) = input.iter().position(|&b| b == 0) {
        let line = input[..pos].iter().filter(|&&b| b == b'\n').count() + 1;
        return Err(CadError::at(line, Reason::NulByte));
    }
    let mut lines: Vec<&[u8]> = input
        .split(|&b| b == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .collect();
    // A final LF leaves an empty last line; blank lines at the end of the file are not data.
    while lines
        .last()
        .is_some_and(|line| line.trim_ascii().is_empty())
    {
        lines.pop();
    }
    if lines.len() % 2 == 1 {
        return Err(CadError::at(lines.len(), Reason::TruncatedPair));
    }
    lines
        .chunks_exact(2)
        .enumerate()
        .map(|(i, pair)| {
            let line = 2 * i + 1;
            let code = parse_code(pair[0]).map_err(|reason| CadError::at(line, reason))?;
            Ok(Pair {
                code,
                value: latin1(pair[1]),
                line,
            })
        })
        .collect()
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
