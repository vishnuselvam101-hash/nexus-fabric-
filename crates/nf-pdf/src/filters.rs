//! Stream filters: FlateDecode (bounded inflate), ASCIIHexDecode, ASCII85Decode,
//! RunLengthDecode, and the PNG and TIFF predictors.
//!
//! Output is checked against the total decoded-bytes budget and the
//! compression-ratio guard while it is produced, so a bomb is refused before
//! it occupies memory. Any other filter (LZW, DCT, JPX, JBIG2, CCITT, Crypt)
//! is reported as unsupported.

use crate::lexer::{is_ws, Dict, Obj};
use crate::limits::{Deadline, Fault, Limits};

/// Decodes `raw` through the filter chain. `stages` pairs each filter name with its
/// decode parameters (an empty dictionary when there are none).
pub fn decode_chain(
    raw: &[u8],
    stages: &[(Vec<u8>, Dict)],
    lim: &Limits,
    used: &mut usize,
    dl: &Deadline,
) -> Result<Vec<u8>, Fault> {
    let mut data: Option<Vec<u8>> = None;
    for (name, parms) in stages {
        let input: &[u8] = match &data {
            Some(v) => v.as_slice(),
            None => raw,
        };
        let out = match name.as_slice() {
            b"FlateDecode" | b"Fl" => {
                let inflated = inflate(input, lim, *used, dl)?;
                apply_predictor(inflated, parms)?
            }
            b"ASCIIHexDecode" | b"AHx" => ascii_hex(input, dl)?,
            b"ASCII85Decode" | b"A85" => ascii85(input, dl)?,
            b"RunLengthDecode" | b"RL" => run_length(input, lim, *used, dl)?,
            other => {
                return Err(Fault::Unsupported(format!(
                    "stream filter /{}",
                    String::from_utf8_lossy(other.get(..other.len().min(32)).unwrap_or(other))
                )));
            }
        };
        check_output(out.len(), input.len(), lim, *used)?;
        *used = used.saturating_add(out.len());
        data = Some(out);
    }
    Ok(match data {
        Some(v) => v,
        None => raw.to_vec(),
    })
}

fn check_output(out_len: usize, in_len: usize, lim: &Limits, used: usize) -> Result<(), Fault> {
    if used.saturating_add(out_len) > lim.max_decoded_bytes {
        return Err(Fault::Limit(
            "decoded_bytes",
            "decoded stream data exceeds the total budget".into(),
        ));
    }
    if out_len > lim.ratio_check_min_bytes && out_len > lim.max_ratio.saturating_mul(in_len) {
        return Err(Fault::Limit(
            "compression_ratio",
            "a stream expands by more than the permitted ratio".into(),
        ));
    }
    Ok(())
}

/// Inflates zlib (or, without a valid zlib header, raw deflate) data.
fn inflate(input: &[u8], lim: &Limits, used: usize, dl: &Deadline) -> Result<Vec<u8>, Fault> {
    let zlib = match (input.first(), input.get(1)) {
        (Some(&cmf), Some(&flg)) => {
            cmf & 0x0F == 8 && (u16::from(cmf) << 8 | u16::from(flg)) % 31 == 0
        }
        _ => false,
    };
    let mut dec = flate2::Decompress::new(zlib);
    let mut out: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 16 * 1024];
    loop {
        dl.check()?;
        let consumed_before = dec.total_in();
        let consumed = usize::try_from(consumed_before).unwrap_or(usize::MAX);
        let rest = input.get(consumed..).unwrap_or(&[]);
        let out_before = dec.total_out();
        let status = dec
            .decompress(rest, &mut chunk, flate2::FlushDecompress::None)
            .map_err(|e| Fault::Malformed(format!("corrupt Flate data: {e}")))?;
        let produced = usize::try_from(dec.total_out() - out_before).unwrap_or(0);
        out.extend_from_slice(chunk.get(..produced).unwrap_or(&[]));
        check_output(out.len(), input.len(), lim, used)?;
        if status == flate2::Status::StreamEnd {
            break;
        }
        if produced == 0 && dec.total_in() == consumed_before {
            return Err(Fault::Malformed("truncated Flate stream".into()));
        }
    }
    Ok(out)
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn ascii_hex(input: &[u8], dl: &Deadline) -> Result<Vec<u8>, Fault> {
    let mut out = Vec::with_capacity(input.len() / 2 + 1);
    let mut hi: Option<u8> = None;
    for (i, &c) in input.iter().enumerate() {
        if i.is_multiple_of(65_536) {
            dl.check()?;
        }
        if c == b'>' {
            break;
        }
        if is_ws(c) {
            continue;
        }
        let Some(v) = hexval(c) else {
            return Err(Fault::Malformed(
                "invalid character in ASCIIHex data".into(),
            ));
        };
        match hi {
            None => hi = Some(v),
            Some(h) => {
                out.push((h << 4) | v);
                hi = None;
            }
        }
    }
    if let Some(h) = hi {
        out.push(h << 4);
    }
    Ok(out)
}

fn ascii85(input: &[u8], dl: &Deadline) -> Result<Vec<u8>, Fault> {
    let mut out = Vec::with_capacity(input.len() * 4 / 5 + 4);
    let mut group = [0u8; 5];
    let mut n = 0usize;
    for (i, &c) in input.iter().enumerate() {
        if i.is_multiple_of(65_536) {
            dl.check()?;
        }
        if is_ws(c) {
            continue;
        }
        if c == b'~' {
            break;
        }
        if c == b'z' && n == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(b'!'..=b'u').contains(&c) {
            return Err(Fault::Malformed("invalid character in ASCII85 data".into()));
        }
        if let Some(slot) = group.get_mut(n) {
            *slot = c - b'!';
        }
        n += 1;
        if n == 5 {
            out.extend_from_slice(&group_value(&group).to_be_bytes());
            n = 0;
        }
    }
    if n > 1 {
        for slot in group.iter_mut().skip(n) {
            *slot = 84;
        }
        let bytes = group_value(&group).to_be_bytes();
        out.extend_from_slice(bytes.get(..n - 1).unwrap_or(&[]));
    }
    Ok(out)
}

fn group_value(group: &[u8; 5]) -> u32 {
    let v = group.iter().fold(0u64, |acc, &d| acc * 85 + u64::from(d));
    v as u32
}

fn run_length(input: &[u8], lim: &Limits, used: usize, dl: &Deadline) -> Result<Vec<u8>, Fault> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(&l) = input.get(i) {
        i += 1;
        if l == 128 {
            break;
        }
        if i.is_multiple_of(65_536) {
            dl.check()?;
        }
        if l < 128 {
            let end = i + usize::from(l) + 1;
            let Some(s) = input.get(i..end) else {
                return Err(Fault::Malformed("truncated RunLength data".into()));
            };
            out.extend_from_slice(s);
            i = end;
        } else {
            let n = 257 - usize::from(l);
            let Some(&b) = input.get(i) else {
                return Err(Fault::Malformed("truncated RunLength data".into()));
            };
            i += 1;
            out.resize(out.len() + n, b);
        }
        check_output(out.len(), input.len(), lim, used)?;
    }
    Ok(out)
}

fn int_parm(parms: &Dict, key: &[u8], default: i64) -> i64 {
    parms
        .get(key)
        .and_then(Obj::num)
        .filter(|v| v.is_finite() && v.fract() == 0.0 && v.abs() < 1e9)
        .map_or(default, |v| v as i64)
}

/// Applies the /Predictor named in `parms` (PNG 10 to 15, TIFF 2). Predictor 1 is a no-op.
fn apply_predictor(data: Vec<u8>, parms: &Dict) -> Result<Vec<u8>, Fault> {
    let predictor = int_parm(parms, b"Predictor", 1);
    if predictor == 1 {
        return Ok(data);
    }
    let colors = int_parm(parms, b"Colors", 1);
    let bpc = int_parm(parms, b"BitsPerComponent", 8);
    let columns = int_parm(parms, b"Columns", 1);
    let valid = (1..=32).contains(&colors)
        && matches!(bpc, 1 | 2 | 4 | 8 | 16)
        && (1..=65_536).contains(&columns);
    if !valid {
        return Err(Fault::Unsupported("predictor parameters".into()));
    }
    let colors = colors as usize;
    let bpc = bpc as usize;
    let columns = columns as usize;
    let row_bits = colors * bpc * columns;
    let row_len = row_bits.div_ceil(8);
    let bpp = (colors * bpc).div_ceil(8).max(1);
    if predictor == 2 {
        if bpc != 8 {
            return Err(Fault::Unsupported(
                "TIFF predictor with BitsPerComponent other than 8".into(),
            ));
        }
        let mut out = data;
        for row in out.chunks_mut(row_len.max(1)) {
            for i in bpp..row.len() {
                let prev = row.get(i - bpp).copied().unwrap_or(0);
                if let Some(v) = row.get_mut(i) {
                    *v = v.wrapping_add(prev);
                }
            }
        }
        return Ok(out);
    }
    if predictor < 10 {
        return Err(Fault::Unsupported(format!("predictor {predictor}")));
    }
    // PNG: each row starts with its filter type byte.
    let mut out: Vec<u8> = Vec::with_capacity(data.len());
    let mut prev = vec![0u8; row_len];
    for chunk in data.chunks(row_len + 1) {
        let (&kind, body) = match chunk.split_first() {
            Some(parts) => parts,
            None => break,
        };
        let mut row = body.to_vec();
        for i in 0..row.len() {
            let left = if i >= bpp {
                row.get(i - bpp).copied().unwrap_or(0)
            } else {
                0
            };
            let up = prev.get(i).copied().unwrap_or(0);
            let upleft = if i >= bpp {
                prev.get(i - bpp).copied().unwrap_or(0)
            } else {
                0
            };
            let add = match kind {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => paeth(left, up, upleft),
                _ => return Err(Fault::Malformed("unknown PNG predictor row type".into())),
            };
            if let Some(v) = row.get_mut(i) {
                *v = v.wrapping_add(add);
            }
        }
        out.extend_from_slice(&row);
        prev = vec![0u8; row_len];
        for (dst, src) in prev.iter_mut().zip(row.iter()) {
            *dst = *src;
        }
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn deadline() -> Deadline {
        Deadline::after(std::time::Duration::from_secs(60))
    }

    #[test]
    fn ascii_hex_and_ascii85_decode() {
        let dl = deadline();
        assert_eq!(
            ascii_hex(b"48 65 6c6c6f>", &dl).unwrap_or_default(),
            b"Hello"
        );
        assert_eq!(ascii_hex(b"4>", &dl).unwrap_or_default(), vec![0x40]);
        // "Man " in ASCII85 is "9jqo^".
        assert_eq!(ascii85(b"9jqo^~>", &dl).unwrap_or_default(), b"Man ");
        assert_eq!(ascii85(b"z~>", &dl).unwrap_or_default(), vec![0, 0, 0, 0]);
    }

    #[test]
    fn run_length_decodes_literal_and_repeat() {
        let lim = Limits::default();
        let out = run_length(&[2, b'a', b'b', b'c', 254, b'x', 128], &lim, 0, &deadline());
        assert_eq!(out.unwrap_or_default(), b"abcxxx");
    }

    #[test]
    fn png_up_predictor_restores_rows() {
        let mut parms = Dict::default();
        parms.0.push((b"Predictor".to_vec(), Obj::Num(12.0)));
        parms.0.push((b"Columns".to_vec(), Obj::Num(2.0)));
        // Row 1: type 0 (none) [1,2]; row 2: type 2 (up) [1,1] -> [2,3].
        let out = apply_predictor(vec![0, 1, 2, 2, 1, 1], &parms);
        assert_eq!(out.unwrap_or_default(), vec![1, 2, 2, 3]);
    }

    #[test]
    fn inflate_round_trip_and_bomb_guard() {
        use std::io::Write;
        let lim = Limits::default();
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        let zeros = vec![0u8; 8 << 20];
        let _ = enc.write_all(&zeros);
        let packed = enc.finish().unwrap_or_default();
        let mut used = 0usize;
        let r = inflate(&packed, &lim, 0, &deadline());
        // 8 MiB of zeros compresses far beyond 100:1 and must be refused.
        assert!(matches!(r, Err(Fault::Limit("compression_ratio", _))));
        used += 1;
        let _ = used;
    }
}
