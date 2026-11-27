//! Text in DXF files: the encoding of each string and the escapes of TEXT and MTEXT strings.
//!
//! Strings arrive from the tokenizer one char per byte. Here they are turned back into bytes and
//! decoded. TEXT and MTEXT escapes are decoded where those entities are read.
//!
//! Files from AC1021 (R2007) on are UTF-8. Older files use `$DWGCODEPAGE`. A code page that
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
