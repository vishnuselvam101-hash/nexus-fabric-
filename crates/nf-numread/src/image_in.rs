//! PNG input and the small amount of image arithmetic the reader needs (pure safe Rust).

use crate::Error;

/// Largest accepted side in pixels (an A0 sheet at 600 dpi is about 28 000 px; anything above this
/// is refused, not downscaled, so a huge file cannot exhaust memory).
pub const MAX_SIDE: usize = 20_000;
/// Largest accepted pixel count.
pub const MAX_PIXELS: usize = 120_000_000;

/// Largest decoder working size (width x height x samples x bytes per sample, before the 16-bit
/// strip): 512 MiB. A 16-bit RGBA page is refused above 64 million pixels, 8-bit RGBA at the full
/// 120 million pixels still passes.
pub const MAX_DECODE_BYTES: usize = 512 * 1024 * 1024;

/// An 8-bit grey image, row-major.
#[derive(Clone, Debug)]
pub struct Gray {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Gray {
    pub fn new(w: usize, h: usize, data: Vec<u8>) -> Result<Gray, Error> {
        check_size(w, h)?;
        if data.len() != w * h {
            return Err(Error::Image(format!(
                "pixel buffer has {} bytes, expected {}",
                data.len(),
                w * h
            )));
        }
        Ok(Gray { w, h, data })
    }

    fn at(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.w + x]
    }

    /// Copy of the rectangle [x0,x1) x [y0,y1), clamped to the image; None if it is empty.
    pub fn crop(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> Option<Gray> {
        let (x1, y1) = (x1.min(self.w), y1.min(self.h));
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let mut data = Vec::with_capacity(w * h);
        for y in y0..y1 {
            data.extend_from_slice(&self.data[y * self.w + x0..y * self.w + x1]);
        }
        Some(Gray { w, h, data })
    }

    /// Bounding box [x0,y0,x1,y1] of the dark pixels (Otsu threshold of the whole crop) in columns
    /// x0..x1; None when there is no ink there.
    pub fn ink_bounds(&self, x0: usize, x1: usize) -> Option<[f32; 4]> {
        let t = self.otsu();
        let x1 = x1.min(self.w);
        if x0 >= x1 {
            return None;
        }
        let (mut minx, mut maxx, mut miny, mut maxy) = (usize::MAX, 0usize, usize::MAX, 0usize);
        for y in 0..self.h {
            for x in x0..x1 {
                if self.at(x, y) <= t {
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    miny = miny.min(y);
                    maxy = maxy.max(y);
                }
            }
        }
        (minx != usize::MAX).then_some([
            minx as f32,
            miny as f32,
            (maxx + 1) as f32,
            (maxy + 1) as f32,
        ])
    }

    fn otsu(&self) -> u8 {
        let mut hist = [0u64; 256];
        for &v in &self.data {
            hist[usize::from(v)] += 1;
        }
        let total = self.data.len() as f64;
        let sum_all: f64 = hist
            .iter()
            .enumerate()
            .map(|(i, &c)| i as f64 * c as f64)
            .sum();
        let (mut wb, mut sum_b, mut best, mut thr) = (0f64, 0f64, -1f64, 128u8);
        for (t, &c) in hist.iter().enumerate() {
            wb += c as f64;
            if wb == 0.0 {
                continue;
            }
            let wf = total - wb;
            if wf <= 0.0 {
                break;
            }
            sum_b += t as f64 * c as f64;
            let (mb, mf) = (sum_b / wb, (sum_all - sum_b) / wf);
            let var = wb * wf * (mb - mf) * (mb - mf);
            if var > best {
                best = var;
                thr = t as u8;
            }
        }
        thr
    }

    /// Rotate by 90 degrees clockwise (`cw`) or counter-clockwise.
    pub fn rotate90(&self, cw: bool) -> Gray {
        let (nw, nh) = (self.h, self.w);
        let mut data = vec![0u8; nw * nh];
        for y in 0..self.h {
            for x in 0..self.w {
                let (nx, ny) = if cw {
                    (self.h - 1 - y, x)
                } else {
                    (y, self.w - 1 - x)
                };
                data[ny * nw + nx] = self.at(x, y);
            }
        }
        Gray { w: nw, h: nh, data }
    }

    /// Triangle-filter resample to `dw` x `dh` (support grows when shrinking, so thin lines are
    /// averaged instead of skipped). Deterministic: plain f32 arithmetic in a fixed order.
    pub fn resize(&self, dw: usize, dh: usize) -> Gray {
        let dw = dw.max(1);
        let dh = dh.max(1);
        if dw == self.w && dh == self.h {
            return self.clone();
        }
        let wx = weights(self.w, dw);
        let wy = weights(self.h, dh);
        // horizontal pass: self.h rows x dw columns
        let mut mid = vec![0f32; self.h * dw];
        for y in 0..self.h {
            let row = &self.data[y * self.w..(y + 1) * self.w];
            for (x, (start, ws)) in wx.iter().enumerate() {
                let mut acc = 0f32;
                for (k, wt) in ws.iter().enumerate() {
                    acc += f32::from(row[start + k]) * wt;
                }
                mid[y * dw + x] = acc;
            }
        }
        let mut data = vec![0u8; dw * dh];
        for (y, (start, ws)) in wy.iter().enumerate() {
            for x in 0..dw {
                let mut acc = 0f32;
                for (k, wt) in ws.iter().enumerate() {
                    acc += mid[(start + k) * dw + x] * wt;
                }
                data[y * dw + x] = acc.round().clamp(0.0, 255.0) as u8;
            }
        }
        Gray { w: dw, h: dh, data }
    }
}

/// For each destination index: first source index and the normalised weights.
fn weights(src: usize, dst: usize) -> Vec<(usize, Vec<f32>)> {
    let scale = src as f32 / dst as f32;
    let support = scale.max(1.0);
    (0..dst)
        .map(|i| {
            let centre = (i as f32 + 0.5) * scale;
            let lo = ((centre - support).floor().max(0.0)) as usize;
            let hi = (((centre + support).ceil()) as usize).min(src);
            let lo = lo.min(src.saturating_sub(1));
            let hi = hi.max(lo + 1);
            let mut ws: Vec<f32> = (lo..hi)
                .map(|j| (1.0 - ((j as f32 + 0.5 - centre) / support).abs()).max(0.0))
                .collect();
            let sum: f32 = ws.iter().sum();
            if sum <= 0.0 {
                ws.iter_mut().for_each(|w| *w = 1.0 / (hi - lo) as f32);
            } else {
                ws.iter_mut().for_each(|w| *w /= sum);
            }
            (lo, ws)
        })
        .collect()
}

pub fn check_size(w: usize, h: usize) -> Result<(), Error> {
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE || w.saturating_mul(h) > MAX_PIXELS {
        return Err(Error::Image(format!(
            "image size {w}x{h} is outside the accepted range"
        )));
    }
    Ok(())
}

/// Refuse a picture whose raw decode size (samples and bytes per sample counted) is above
/// `MAX_DECODE_BYTES`; `bytes_per_sample` is 1 for depths up to 8 bits (rounded up), 2 for 16.
pub fn check_decode_bytes(
    w: usize,
    h: usize,
    samples: usize,
    bytes_per_sample: usize,
) -> Result<(), Error> {
    let need = w
        .saturating_mul(h)
        .saturating_mul(samples)
        .saturating_mul(bytes_per_sample.max(1));
    if need > MAX_DECODE_BYTES {
        return Err(Error::Image(format!(
            "image {w}x{h} needs {need} bytes to decode, limit is {MAX_DECODE_BYTES}"
        )));
    }
    Ok(())
}

/// Decode a PNG of any colour type / depth into 8-bit grey (transparent pixels count as white).
pub fn decode_png(bytes: &[u8]) -> Result<Gray, Error> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    dec.set_limits(png::Limits {
        bytes: MAX_DECODE_BYTES,
    });
    let mut reader = dec
        .read_info()
        .map_err(|e| Error::Image(format!("PNG: {e}")))?;
    let (w, h) = {
        let i = reader.info();
        (i.width as usize, i.height as usize)
    };
    check_size(w, h)?;
    {
        let i = reader.info();
        let samples = if i.color_type == png::ColorType::Indexed {
            4
        } else {
            i.color_type.samples()
        };
        check_decode_bytes(w, h, samples, i.bit_depth as usize / 8)?;
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| Error::Image("PNG: output size overflow".into()))?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| Error::Image(format!("PNG: {e}")))?;
    let (w, h) = (info.width as usize, info.height as usize);
    check_size(w, h)?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(Error::Image("PNG: unsupported bit depth".into()));
    }
    let ch = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(Error::Image("PNG: palette not expanded".into())),
    };
    let line = info.line_size;
    let mut data = Vec::with_capacity(w * h);
    for y in 0..h {
        let row = buf
            .get(y * line..y * line + w * ch)
            .ok_or_else(|| Error::Image("PNG: short pixel data".into()))?;
        for px in row.chunks_exact(ch) {
            let (g, a) = match ch {
                1 => (u32::from(px[0]), 255u32),
                2 => (u32::from(px[0]), u32::from(px[1])),
                3 => (
                    (u32::from(px[0]) * 299 + u32::from(px[1]) * 587 + u32::from(px[2]) * 114)
                        / 1000,
                    255,
                ),
                _ => (
                    (u32::from(px[0]) * 299 + u32::from(px[1]) * 587 + u32::from(px[2]) * 114)
                        / 1000,
                    u32::from(px[3]),
                ),
            };
            data.push(((g * a + 255 * (255 - a)) / 255) as u8);
        }
    }
    Ok(Gray { w, h, data })
}

#[cfg(test)]
mod decode_limit_tests {
    use super::*;

    #[test]
    fn sixteen_bit_rgba_is_limited_tighter_than_eight_bit() {
        // 100 million pixels: 8-bit RGBA (400 MB) passes, 16-bit RGBA (800 MB) is refused
        assert!(check_decode_bytes(10_000, 10_000, 4, 1).is_ok());
        assert!(check_decode_bytes(10_000, 10_000, 4, 2).is_err());
        assert!(check_decode_bytes(8_000, 8_000, 4, 2).is_ok());
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        let mut crc = 0xffff_ffffu32;
        for &b in kind.iter().chain(data) {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&(!crc).to_be_bytes());
    }

    /// A header-only 16-bit RGBA PNG of 100 million pixels is refused by decode_png itself.
    #[test]
    fn decode_png_refuses_huge_sixteen_bit_rgba_header() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&10_000u32.to_be_bytes());
        ihdr.extend_from_slice(&10_000u32.to_be_bytes());
        ihdr.extend_from_slice(&[16, 6, 0, 0, 0]);
        chunk(&mut png, b"IHDR", &ihdr);
        chunk(&mut png, b"IDAT", &[]);
        let e = decode_png(&png).expect_err("must be refused");
        assert!(e.to_string().contains("needs"), "{e}");
    }
}
