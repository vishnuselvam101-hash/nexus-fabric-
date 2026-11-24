//! Membrane (trust zone Z0): admission checks on uploaded bytes.
//! Type is decided from content, never from the file name. Nothing is parsed
//! or interpreted here.

pub const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, PartialEq)]
pub enum Admission {
    Accepted { detected_type: &'static str },
    Rejected { reason: &'static str },
}

pub fn admit(bytes: &[u8]) -> Admission {
    if bytes.is_empty() {
        return Admission::Rejected { reason: "empty" };
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Admission::Rejected {
            reason: "too_large",
        };
    }
    let head = &bytes[..bytes.len().min(2048)];
    let t = if head.starts_with(b"%PDF-") {
        "pdf"
    } else if head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        "png"
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpeg"
    } else if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        "tiff"
    } else if head.starts_with(b"ISO-10303-21") {
        "step"
    } else if looks_like_ascii_dxf(head) {
        "dxf"
    } else {
        return Admission::Rejected {
            reason: "unsupported_format",
        };
    };
    Admission::Accepted { detected_type: t }
}

fn looks_like_ascii_dxf(head: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(head) else {
        return false;
    };
    let mut lines = text.lines().map(str::trim);
    let first = lines.next().unwrap_or("");
    (first == "0" || first == "999") && text.contains("SECTION")
}

/// Declared extension disagrees with detected content (recorded, not fatal).
pub fn extension_mismatch(filename: &str, detected: &str) -> bool {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let ok: &[&str] = match detected {
        "pdf" => &["pdf"],
        "png" => &["png"],
        "jpeg" => &["jpg", "jpeg"],
        "tiff" => &["tif", "tiff"],
        "step" => &["step", "stp", "p21"],
        "dxf" => &["dxf"],
        _ => &[],
    };
    !ok.contains(&ext.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_by_content() {
        assert_eq!(
            admit(b"%PDF-1.7\n..."),
            Admission::Accepted {
                detected_type: "pdf"
            }
        );
        assert_eq!(
            admit(b"  0\nSECTION\n  2\nHEADER\n"),
            Admission::Accepted {
                detected_type: "dxf"
            }
        );
        assert_eq!(
            admit(b"ISO-10303-21;\nHEADER;"),
            Admission::Accepted {
                detected_type: "step"
            }
        );
        assert_eq!(
            admit(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]),
            Admission::Accepted {
                detected_type: "jpeg"
            }
        );
        assert_eq!(admit(b""), Admission::Rejected { reason: "empty" });
        assert_eq!(
            admit(b"MZ\x90\x00 executable"),
            Admission::Rejected {
                reason: "unsupported_format"
            }
        );
        assert_eq!(
            admit(b"PK\x03\x04 zip"),
            Admission::Rejected {
                reason: "unsupported_format"
            }
        );
        assert!(extension_mismatch("drawing.pdf", "png") && !extension_mismatch("a.JPG", "jpeg"));
    }
}
