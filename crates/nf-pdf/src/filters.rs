//! Stream filters: FlateDecode (bounded inflate), ASCIIHexDecode, ASCII85Decode,
//! RunLengthDecode, and the PNG and TIFF predictors.
//!
//! The output of every filter is checked against the total decoded-bytes budget and
//! the compression-ratio guard while it is produced (inside each decoding loop), so a
//! bomb is refused before it occupies memory. Any other filter (LZW, DCT, JPX, JBIG2,
//! CCITT, Crypt) is reported as unsupported.

use crate::lexer::{is_ws, Dict, Obj};
use crate::limits::{Deadline, Fault, Limits, CHECK_EVERY};

/// The predictors check the deadline after this many bytes of rows.
const PREDICTOR_CHECK_BYTES: usize = 64 * 1024;

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
                apply_predictor(inflated, parms, dl)?
            }
            b"ASCIIHexDecode" | b"AHx" => ascii_hex(input, lim, *used, dl)?,
            b"ASCII85Decode" | b"A85" => ascii85(input, lim, *used, dl)?,
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

/// Initial capacity for a filter output: the expected size, but never more than the bytes
/// the decoded budget still allows (plus one, so that exceeding it is detected).
fn capacity(expected: usize, lim: &Limits, used: usize) -> usize {
    expected.min(lim.max_decoded_bytes.saturating_sub(used).saturating_add(1))
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

fn ascii_hex(input: &[u8], lim: &Limits, used: usize, dl: &Deadline) -> Result<Vec<u8>, Fault> {
    let mut out = Vec::with_capacity(capacity(input.len() / 2 + 1, lim, used));
    let mut hi: Option<u8> = None;
    for (i, &c) in input.iter().enumerate() {
        if (i as u64).is_multiple_of(CHECK_EVERY) {
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
                check_output(out.len(), input.len(), lim, used)?;
                hi = None;
            }
        }
    }
    if let Some(h) = hi {
        out.push(h << 4);
        check_output(out.len(), input.len(), lim, used)?;
    }
    Ok(out)
}

fn ascii85(input: &[u8], lim: &Limits, used: usize, dl: &Deadline) -> Result<Vec<u8>, Fault> {
    let expected = (input.len() / 5).saturating_mul(4).saturating_add(4);
    let mut out = Vec::with_capacity(capacity(expected, lim, used));
    let mut group = [0u8; 5];
    let mut n = 0usize;
    for (i, &c) in input.iter().enumerate() {
        if (i as u64).is_multiple_of(CHECK_EVERY) {
            dl.check()?;
        }
        if is_ws(c) {
            continue;
        }
        if c == b'~' {
            break;
        }
        if c == b'z' && n == 0 {
            // Four bytes from one input character: the budget is checked before the next one.
            out.extend_from_slice(&[0, 0, 0, 0]);
            check_output(out.len(), input.len(), lim, used)?;
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
            check_output(out.len(), input.len(), lim, used)?;
            n = 0;
        }
    }
    if n > 1 {
        for slot in group.iter_mut().skip(n) {
            *slot = 84;
        }
        let bytes = group_value(&group).to_be_bytes();
        out.extend_from_slice(bytes.get(..n - 1).unwrap_or(&[]));
        check_output(out.len(), input.len(), lim, used)?;
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
    let mut runs = 0u64;
    while let Some(&l) = input.get(i) {
        i += 1;
        if l == 128 {
            break;
        }
        // A run counter, not the input offset: offsets can step over every multiple.
        if runs.is_multiple_of(CHECK_EVERY) {
            dl.check()?;
        }
        runs += 1;
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
/// The deadline is checked after every [`PREDICTOR_CHECK_BYTES`] bytes of rows; the PNG
/// rows reuse two buffers instead of allocating per row.
fn apply_predictor(data: Vec<u8>, parms: &Dict, dl: &Deadline) -> Result<Vec<u8>, Fault> {
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
        let mut since_check = 0usize;
        for row in out.chunks_mut(row_len.max(1)) {
            since_check += row.len();
            if since_check >= PREDICTOR_CHECK_BYTES {
                since_check = 0;
                dl.check()?;
            }
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
    let mut row: Vec<u8> = Vec::with_capacity(row_len);
    let mut since_check = 0usize;
    for chunk in data.chunks(row_len + 1) {
        let (&kind, body) = match chunk.split_first() {
            Some(parts) => parts,
            None => break,
        };
        since_check += chunk.len();
        if since_check >= PREDICTOR_CHECK_BYTES {
            since_check = 0;
            dl.check()?;
        }
        row.clear();
        row.extend_from_slice(body);
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
        // The row just decoded becomes the row above, padded with zeros (a short last row).
        for (i, dst) in prev.iter_mut().enumerate() {
            *dst = row.get(i).copied().unwrap_or(0);
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

    fn expired() -> Deadline {
        Deadline::after(std::time::Duration::ZERO)
    }

    #[test]
    fn ascii_hex_and_ascii85_decode() {
        let dl = deadline();
        let lim = Limits::default();
        assert_eq!(
            ascii_hex(b"48 65 6c6c6f>", &lim, 0, &dl).unwrap_or_default(),
            b"Hello"
        );
        assert_eq!(
            ascii_hex(b"4>", &lim, 0, &dl).unwrap_or_default(),
            vec![0x40]
        );
        // "Man " in ASCII85 is "9jqo^".
        assert_eq!(
            ascii85(b"9jqo^~>", &lim, 0, &dl).unwrap_or_default(),
            b"Man "
        );
        assert_eq!(
            ascii85(b"z~>", &lim, 0, &dl).unwrap_or_default(),
            vec![0, 0, 0, 0]
        );
    }

    #[test]
    fn ascii85_and_ascii_hex_stop_at_the_budget_inside_the_loop() {
        let dl = deadline();
        let lim = Limits {
            max_decoded_bytes: 1000,
            ..Limits::default()
        };
        let budget = |r: Result<Vec<u8>, Fault>| matches!(r, Err(Fault::Limit("decoded_bytes", _)));
        // 'z' is four bytes per input character; the loop stops at the budget.
        let mut zs = vec![b'z'; 10_000];
        zs.extend_from_slice(b"~>");
        assert!(budget(ascii85(&zs, &lim, 0, &dl)));
        // Bytes already decoded by earlier stages count: 990 + 16 > 1000, 990 + 8 <= 1000.
        assert!(budget(ascii85(b"zzzz~>", &lim, 990, &dl)));
        assert_eq!(ascii85(b"zz~>", &lim, 990, &dl).map(|v| v.len()), Ok(8));
        let hex = vec![b'0'; 4_000];
        assert!(budget(ascii_hex(&hex, &lim, 0, &dl)));
        assert!(budget(ascii_hex(b"0000000000>", &lim, 996, &dl)));
        assert_eq!(
            ascii_hex(b"00000000>", &lim, 996, &dl).map(|v| v.len()),
            Ok(4)
        );
    }

    #[test]
    fn run_length_checks_the_deadline_on_a_run_counter() {
        // Repeat runs advance the offset by 2 from 1: an offset-based check never fired.
        let mut data = Vec::new();
        for _ in 0..10_000 {
            data.extend_from_slice(&[129, b'z']);
        }
        data.push(128);
        let r = run_length(&data, &Limits::default(), 0, &expired());
        assert_eq!(r, Err(Fault::Timeout));
    }

    #[test]
    fn predictors_check_the_deadline() {
        let mut png = Dict::default();
        png.0.push((b"Predictor".to_vec(), Obj::Num(12.0)));
        png.0.push((b"Columns".to_vec(), Obj::Num(1.0)));
        // 100,000 rows of one byte (type 0, value 0).
        let r = apply_predictor(vec![0u8; 200_000], &png, &expired());
        assert_eq!(r, Err(Fault::Timeout));
        let mut tiff = Dict::default();
        tiff.0.push((b"Predictor".to_vec(), Obj::Num(2.0)));
        tiff.0.push((b"Columns".to_vec(), Obj::Num(4.0)));
        let r = apply_predictor(vec![1u8; 200_000], &tiff, &expired());
        assert_eq!(r, Err(Fault::Timeout));
        // Below the check stride the predictors finish even with an expired deadline.
        assert!(apply_predictor(vec![0u8; 1_000], &png, &expired()).is_ok());
    }

    /// The PNG predictor as it was written before the row buffers were reused: one new
    /// vector per row. Kept here as the reference for the rewritten loop.
    fn png_reference(data: &[u8], row_len: usize, bpp: usize) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut prev = vec![0u8; row_len];
        for chunk in data.chunks(row_len + 1) {
            let (&kind, body) = chunk.split_first()?;
            let mut row = body.to_vec();
            for i in 0..row.len() {
                let left = if i >= bpp { row[i - bpp] } else { 0 };
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
                    _ => return None,
                };
                row[i] = row[i].wrapping_add(add);
            }
            out.extend_from_slice(&row);
            prev = vec![0u8; row_len];
            for (dst, src) in prev.iter_mut().zip(row.iter()) {
                *dst = *src;
            }
        }
        Some(out)
    }

    #[test]
    fn png_predictor_with_reused_rows_matches_the_reference() {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let shapes: [(usize, usize, usize); 5] =
            [(1, 8, 1), (3, 8, 7), (1, 16, 5), (4, 8, 33), (1, 1, 13)];
        for (colors, bpc, columns) in shapes {
            let row_len = (colors * bpc * columns).div_ceil(8);
            let bpp = (colors * bpc).div_ceil(8).max(1);
            for rows in [1usize, 2, 9] {
                // Every row type, random bytes, and a short last row.
                let mut data = Vec::new();
                for r in 0..rows {
                    data.push((r % 5) as u8);
                    for _ in 0..row_len {
                        data.push(next() as u8);
                    }
                }
                data.truncate(data.len() - row_len / 2);
                let mut parms = Dict::default();
                parms.0.push((b"Predictor".to_vec(), Obj::Num(15.0)));
                parms.0.push((b"Colors".to_vec(), Obj::Num(colors as f64)));
                parms
                    .0
                    .push((b"BitsPerComponent".to_vec(), Obj::Num(bpc as f64)));
                parms
                    .0
                    .push((b"Columns".to_vec(), Obj::Num(columns as f64)));
                let got = apply_predictor(data.clone(), &parms, &deadline());
                let want = png_reference(&data, row_len, bpp).ok_or(Fault::Timeout);
                assert_eq!(
                    got, want,
                    "colors {colors} bpc {bpc} columns {columns} rows {rows}"
                );
            }
        }
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
        let out = apply_predictor(vec![0, 1, 2, 2, 1, 1], &parms, &deadline());
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
