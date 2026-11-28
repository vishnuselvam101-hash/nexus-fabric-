//! M3: DXF-only hand-off. Rewrites an NF DXF into the profile's version, layer names and text encoding.
//! No geometry is changed. Anything the rules cannot express exactly is refused, not guessed.

use crate::profile::{is_clean, DxfVersion, MachineProfile, TextEncoding};
use encoding_rs::SHIFT_JIS;

/// Entities that R12 cannot hold; converting them would change geometry, so they are refused.
const NOT_IN_R12: &[&str] = &[
    "LWPOLYLINE",
    "SPLINE",
    "ELLIPSE",
    "MTEXT",
    "HATCH",
    "DIMENSION",
];

#[derive(Debug, PartialEq, Eq)]
pub enum DxfError {
    /// Input is not UTF-8 text or has an odd number of lines.
    Malformed(String),
    UnmappedLayer(String),
    UnsupportedEntity(String),
    UnitsMismatch(String),
    /// A character the target encoding cannot represent, or one that is ambiguous in it (U+00A5, U+203E).
    Unencodable(char),
    /// The source header has no $INSUNITS, so its units cannot be checked.
    MissingUnits,
}

impl std::fmt::Display for DxfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DxfError::Malformed(m) => write!(f, "malformed DXF: {m}"),
            DxfError::UnmappedLayer(l) => {
                write!(f, "layer '{l}' has no entry in the profile layer map")
            }
            DxfError::UnsupportedEntity(e) => write!(
                f,
                "entity {e} cannot be written in the profile's DXF version"
            ),
            DxfError::UnitsMismatch(u) => {
                write!(f, "$INSUNITS is {u}, profile needs millimetres (4)")
            }
            DxfError::Unencodable(c) => write!(
                f,
                "character U+{:04X} is not in the profile's text encoding or is ambiguous in it",
                *c as u32
            ),
            DxfError::MissingUnits => {
                write!(f, "header has no $INSUNITS; profile needs millimetres (4)")
            }
        }
    }
}

impl std::error::Error for DxfError {}

/// Convert the source bytes for the profile. Output is CRLF-terminated.
/// R12 targets drop what R13+ adds (subclass markers, code 100; the $HANDSEED header variable). Nothing
/// else is synthesised: an R2000 target keeps the source's entities and only its version and codepage change.
pub fn convert(src: &[u8], p: &MachineProfile) -> Result<Vec<u8>, DxfError> {
    let text = std::str::from_utf8(src).map_err(|_| DxfError::Malformed("not UTF-8".into()))?;
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let lines = if lines.last() == Some(&"") {
        &lines[..lines.len() - 1]
    } else {
        &lines[..]
    };
    if lines.len() % 2 != 0 {
        return Err(DxfError::Malformed(
            "odd number of group code / value lines".into(),
        ));
    }

    let mut out = String::with_capacity(text.len());
    let mut last_zero = String::new();
    let mut pending_var: Option<String> = None;
    let codepage = match p.text_encoding {
        TextEncoding::ShiftJis => Some("ANSI_932"),
        TextEncoding::Utf8 => None,
    };
    // Shift_JIS text is unreadable to a CAM that assumes ANSI_1252: declare the codepage if the header lacks it.
    let header_has_cp = lines.iter().any(|l| l.trim() == "$DWGCODEPAGE");
    let r12 = p.dxf_version == DxfVersion::R12;
    let mut saw_units = false;
    let mut skip_value = false;

    for pair in lines.chunks(2) {
        let code = pair[0].trim();
        if skip_value {
            skip_value = false;
            continue;
        }
        if r12 && code == "100" {
            continue;
        }
        let mut value = pair[1].to_string();
        let var_before = pending_var.clone();
        if code == "9" && pair[1].trim() == "$HANDSEED" && r12 {
            skip_value = true;
            continue;
        }
        match code {
            "0" => {
                last_zero = value.trim().to_string();
                if p.dxf_version == DxfVersion::R12 && NOT_IN_R12.contains(&last_zero.as_str()) {
                    return Err(DxfError::UnsupportedEntity(last_zero));
                }
            }
            "2" if last_zero == "LAYER" => value = map_layer(&value, p)?,
            "8" => value = map_layer(&value, p)?,
            "9" => {
                pending_var = Some(value.trim().to_string());
            }
            _ => {}
        }
        if code != "9" {
            if let Some(var) = pending_var.take() {
                match (var.as_str(), code) {
                    ("$ACADVER", "1") => value = p.dxf_version.acad_ver().to_string(),
                    ("$DWGCODEPAGE", "3") => {
                        if let Some(cp) = codepage {
                            value = cp.to_string();
                        }
                    }
                    ("$INSUNITS", "70") => {
                        if value.trim() != "4" {
                            return Err(DxfError::UnitsMismatch(value.trim().to_string()));
                        }
                        saw_units = true;
                    }
                    _ => {}
                }
            }
        }
        out.push_str(pair[0].trim());
        out.push_str("\r\n");
        out.push_str(&value);
        out.push_str("\r\n");
        if let (Some(cp), false, Some("$ACADVER"), "1") =
            (codepage, header_has_cp, var_before.as_deref(), code)
        {
            out.push_str("9\r\n$DWGCODEPAGE\r\n3\r\n");
            out.push_str(cp);
            out.push_str("\r\n");
        }
    }

    if !saw_units {
        return Err(DxfError::MissingUnits);
    }
    match p.text_encoding {
        TextEncoding::Utf8 => Ok(out.into_bytes()),
        TextEncoding::ShiftJis => encode_sjis(&out),
    }
}

fn map_layer(name: &str, p: &MachineProfile) -> Result<String, DxfError> {
    let key = name.trim_end();
    let value = p
        .layer_map
        .get(key)
        .ok_or_else(|| DxfError::UnmappedLayer(key.to_string()))?;
    // Re-checked here as well as in MachineProfile::validate: a line break in a value would inject DXF pairs.
    if !is_clean(value) {
        return Err(DxfError::Malformed(format!(
            "machine layer name for '{key}' has a control character or line break"
        )));
    }
    Ok(value.clone())
}

fn encode_sjis(s: &str) -> Result<Vec<u8>, DxfError> {
    // encoding_rs maps U+00A5 to 0x5C and U+203E to 0x7E (ASCII backslash and tilde): refused, not silently changed.
    if let Some(c) = s.chars().find(|c| matches!(c, '\u{a5}' | '\u{203e}')) {
        return Err(DxfError::Unencodable(c));
    }
    // encoding_rs substitutes HTML numeric references for unmappable characters; detect them and refuse.
    let (bytes, _, had_errors) = SHIFT_JIS.encode(s);
    if had_errors {
        let c = s
            .chars()
            .find(|c| !c.is_ascii() && !sjis_ok(*c))
            .unwrap_or('?');
        return Err(DxfError::Unencodable(c));
    }
    Ok(bytes.into_owned())
}

fn sjis_ok(c: char) -> bool {
    let mut buf = [0u8; 4];
    !SHIFT_JIS.encode(c.encode_utf8(&mut buf)).2
}
