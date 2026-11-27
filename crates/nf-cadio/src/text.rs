//! Text in DXF files: the encoding of each string and the escapes of TEXT and MTEXT strings.
//!
//! Strings arrive from the tokenizer one char per byte. Here they are turned back into bytes and
//! decoded. Files from AC1021 (R2007) on are UTF-8. Older files use `$DWGCODEPAGE`. A code page that
//! is not listed is refused for non-ASCII text, never replaced.

use encoding_rs::{Encoding, SHIFT_JIS, WINDOWS_1252};

use crate::error::Reason;

/// The encoding of the strings in one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextEncoding {
    /// `$DWGCODEPAGE` ANSI_932: Shift_JIS, decoded with the WHATWG Shift_JIS table.
    Ansi932,
    /// `$DWGCODEPAGE` ANSI_1252: Windows-1252.
    Ansi1252,
    /// AC1021 and later: UTF-8, whatever `$DWGCODEPAGE` says.
    Utf8,
    /// A `$DWGCODEPAGE` this reader does not decode.
    Unsupported(String),
    /// No `$DWGCODEPAGE`, and the version is not AC1021 or later.
    Missing,
}

/// The encoding for a file with this `$ACADVER` and `$DWGCODEPAGE`.
pub(crate) fn encoding_for(version: Option<&str>, codepage: Option<&str>) -> TextEncoding {
    let utf8 = version
        .and_then(|v| v.trim().strip_prefix("AC"))
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| n >= 1021);
    if utf8 {
        return TextEncoding::Utf8;
    }
    match codepage.map(str::trim) {
        Some("ANSI_932") => TextEncoding::Ansi932,
        Some("ANSI_1252") => TextEncoding::Ansi1252,
        Some(other) => TextEncoding::Unsupported(other.to_string()),
        None => TextEncoding::Missing,
    }
}

/// Decodes one string as read by the tokenizer (one char per byte). ASCII is returned as it is.
pub(crate) fn decode(latin1: &str, enc: &TextEncoding) -> Result<String, Reason> {
    if latin1.is_ascii() {
        return Ok(latin1.to_string());
    }
    let bytes = latin1
        .chars()
        .map(|c| u8::try_from(u32::from(c)).map_err(|_| Reason::InvalidText))
        .collect::<Result<Vec<u8>, Reason>>()?;
    decode_bytes(&bytes, enc)
}

/// Decodes raw bytes in the file's encoding. Bytes that are not valid there are refused.
fn decode_bytes(bytes: &[u8], enc: &TextEncoding) -> Result<String, Reason> {
    match enc {
        TextEncoding::Utf8 => String::from_utf8(bytes.to_vec()).map_err(|_| Reason::InvalidText),
        TextEncoding::Ansi932 => from_codec(SHIFT_JIS, bytes),
        TextEncoding::Ansi1252 => from_codec(WINDOWS_1252, bytes),
        TextEncoding::Unsupported(name) => Err(Reason::UnknownEncoding {
            codepage: name.clone(),
        }),
        TextEncoding::Missing => Err(Reason::NoEncoding),
    }
}

fn from_codec(codec: &'static Encoding, bytes: &[u8]) -> Result<String, Reason> {
    codec
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|text| text.into_owned())
        .ok_or(Reason::InvalidText)
}

/// The plain text of a TEXT (`mtext` false) or MTEXT (`mtext` true) string.
///
/// `\U+XXXX` gives the Unicode character. `\M+1XXXX` gives the two-byte character XXXX in the file's
/// encoding. In MTEXT, `\P` is a new line, `\~` a space, `{` and `}` are group braces (dropped), and
/// formatting codes with a `;` end (`\H2.5;`, `\fArial|b0;`) are dropped. Other escapes, and escapes
/// with malformed arguments, are kept as written. The raw string is never changed.
pub(crate) fn plain(raw: &str, enc: &TextEncoding, mtext: bool) -> Result<String, Reason> {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c != '\\' {
            if mtext && (c == '{' || c == '}') {
                // group braces only; the text inside them is kept
            } else {
                out.push(c);
            }
            i += 1;
            continue;
        }
        if let Some((text, used)) = unicode_escape(&chars, i) {
            out.push(text);
            i += used;
            continue;
        }
        if let Some((code, used)) = two_byte_escape(&chars, i) {
            out.push_str(&decode_bytes(&code, enc)?);
            i += used;
            continue;
        }
        let next = chars.get(i + 1).copied();
        match next {
            Some('\\' | '{' | '}') => {
                out.extend(next);
                i += 2;
            }
            Some('P') if mtext => {
                out.push('\n');
                i += 2;
            }
            Some('~') if mtext => {
                out.push(' ');
                i += 2;
            }
            Some('L' | 'l' | 'O' | 'o' | 'K' | 'k') if mtext => i += 2,
            Some('H' | 'W' | 'C' | 'f' | 'F' | 'A' | 'Q' | 'T' | 'p') if mtext => {
                match chars[i + 2..].iter().take(32).position(|&d| d == ';') {
                    Some(end) => i += 2 + end + 1,
                    None => {
                        out.push('\\');
                        i += 1;
                    }
                }
            }
            _ => {
                out.push('\\');
                i += 1;
            }
        }
    }
    Ok(out)
}

/// `\U+XXXX` at `at`: the character and the number of chars used.
fn unicode_escape(chars: &[char], at: usize) -> Option<(char, usize)> {
    if chars.get(at + 1) != Some(&'U') || chars.get(at + 2) != Some(&'+') {
        return None;
    }
    let code = hex_at(chars, at + 3, 4)?;
    Some((char::from_u32(code)?, 7))
}

/// `\M+1XXXX` at `at`: the two bytes XXXX, and the number of chars used.
fn two_byte_escape(chars: &[char], at: usize) -> Option<([u8; 2], usize)> {
    if chars.get(at + 1) != Some(&'M')
        || chars.get(at + 2) != Some(&'+')
        || chars.get(at + 3) != Some(&'1')
    {
        return None;
    }
    let code = hex_at(chars, at + 4, 4)?;
    let hi = u8::try_from(code >> 8).ok()?;
    let lo = u8::try_from(code & 0xff).ok()?;
    Some(([hi, lo], 8))
}

/// `n` hex digits starting at `start`.
fn hex_at(chars: &[char], start: usize, n: usize) -> Option<u32> {
    let digits = chars.get(start..start.checked_add(n)?)?;
    digits
        .iter()
        .try_fold(0u32, |acc, d| Some(acc * 16 + d.to_digit(16)?))
}
