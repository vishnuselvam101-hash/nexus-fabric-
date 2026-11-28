//! Minimal PNG writer: 8-bit gray or RGB, filter "Up", deflate through `flate2` (pure Rust).

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

/// Dots per inch recorded in the pHYs chunk.
pub fn encode(
    width: u32,
    height: u32,
    channels: usize,
    dpi: u32,
    pixels: &[u8],
) -> Option<Vec<u8>> {
    let color_type = match channels {
        1 => 0u8,
        3 => 2u8,
        _ => return None,
    };
    let stride = (width as usize).checked_mul(channels)?;
    if pixels.len() != stride.checked_mul(height as usize)? || stride == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(pixels.len() / 8 + 256);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, color_type, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    let ppm = (f64::from(dpi) / 0.0254).round() as u32;
    let mut phys = Vec::with_capacity(9);
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.push(1);
    chunk(&mut out, b"pHYs", &phys);
    let mut z = ZlibEncoder::new(
        Vec::with_capacity(pixels.len() / 8 + 64),
        Compression::new(4),
    );
    let mut prev: Option<&[u8]> = None;
    let mut line = vec![0u8; stride + 1];
    for row in pixels.chunks_exact(stride) {
        if let Some((tag, body)) = line.split_first_mut() {
            *tag = 2;
            match prev {
                Some(p) => {
                    for ((o, a), b) in body.iter_mut().zip(row).zip(p) {
                        *o = a.wrapping_sub(*b);
                    }
                }
                None => body.copy_from_slice(row),
            }
        }
        z.write_all(&line).ok()?;
        prev = Some(row);
    }
    let packed = z.finish().ok()?;
    for part in packed.chunks(1 << 16) {
        chunk(&mut out, b"IDAT", part);
    }
    chunk(&mut out, b"IEND", &[]);
    Some(out)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut h = crc32fast::Hasher::new();
    out.extend_from_slice(kind);
    h.update(kind);
    out.extend_from_slice(data);
    h.update(data);
    out.extend_from_slice(&h.finalize().to_be_bytes());
}
