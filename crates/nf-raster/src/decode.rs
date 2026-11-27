//! Image decoding: DCT (JPEG), CCITT G3/G4, JBIG2 and raw samples to 8-bit gray or RGB pixels.
//!
//! Every decoder is a pure-Rust crate with `forbid(unsafe_code)` (CCITT, JBIG2) or with its
//! SIMD paths compiled out (`platform_independent`, JPEG). Sizes are checked before anything
//! large is allocated.

use nf_pdf::raster::{Ccitt, Color, Deadline, Fault, ImageFilter, RawImage};

/// Decoded pixels. For a stencil mask `data` is coverage (255 = paint) with one channel.
#[derive(Clone, Debug)]
pub struct Pixmap {
    pub w: u32,
    pub h: u32,
    pub ch: usize,
    pub data: Vec<u8>,
    pub mask: bool,
}

/// Warnings found while decoding (code only; the report carries the text).
#[derive(Default, Debug)]
pub struct Warnings(pub Vec<&'static str>);

fn corrupt<T>(what: &str) -> Result<T, Fault> {
    Err(Fault::Refused("image_decode_failed", what.to_string()))
}

/// Packed bit rows: `bpc` bits per sample, rows padded to whole bytes.
struct Packed {
    data: Vec<u8>,
    stride: usize,
    width: u32,
    height: u32,
    bpc: u32,
    /// Components per pixel.
    comps: usize,
}

/// Decodes `raw` to pixels. `max_pixels` bounds width times height.
pub fn decode(
    raw: &RawImage,
    dl: &Deadline,
    max_pixels: u64,
    warn: &mut Warnings,
) -> Result<Pixmap, Fault> {
    let mut color = raw.color.clone();
    let packed = match &raw.filter {
        ImageFilter::Raw => {
            let comps = if raw.mask { 1 } else { color.components() };
            let bits = (raw.width as usize)
                .checked_mul(comps)
                .and_then(|v| v.checked_mul(raw.bpc as usize))
                .ok_or_else(|| Fault::Malformed("image row size overflows".into()))?;
            Packed {
                data: raw.data.clone(),
                stride: bits.div_ceil(8),
                width: raw.width,
                height: raw.height,
                bpc: raw.bpc,
                comps,
            }
        }
        ImageFilter::Ccitt(p) => ccitt(raw, p, max_pixels, warn)?,
        ImageFilter::Jbig2 { globals } => {
            jbig2(raw, globals.as_deref().map(Vec::as_slice), max_pixels, warn)?
        }
        ImageFilter::Dct => {
            let (packed, c) = jpeg(raw, max_pixels)?;
            color = c;
            packed
        }
        ImageFilter::Jpx => {
            return Err(Fault::Unsupported("stream filter /JPXDecode".into()));
        }
    };
    if raw.has_alpha {
        warn.0.push("image_alpha_ignored");
    }
    to_pixmap(&packed, &color, raw, dl, warn)
}

fn ccitt(raw: &RawImage, p: &Ccitt, max_pixels: u64, warn: &mut Warnings) -> Result<Packed, Fault> {
    if p.columns == 0 || u64::from(p.columns) * u64::from(raw.height) > max_pixels {
        return Err(Fault::Limit(
            "image_pixels",
            "a CCITT image has more pixels than the limit allows".into(),
        ));
    }
    let stride = (p.columns as usize).div_ceil(8);
    let white: u8 = if p.black_is_1 { 0x00 } else { 0xFF };
    struct Rows {
        data: Vec<u8>,
        stride: usize,
        width: u32,
        rows: u32,
        height: u32,
        black_is_1: bool,
    }
    impl hayro_ccitt::Decoder for Rows {
        fn push_row(&mut self, changes: &[u32]) {
            if self.rows >= self.height {
                return;
            }
            let start = self.rows as usize * self.stride;
            self.rows += 1;
            let Some(row) = self.data.get_mut(start..start + self.stride) else {
                return;
            };
            for pair in changes.chunks(2) {
                let a = pair.first().copied().unwrap_or(0).min(self.width);
                let b = pair.get(1).copied().unwrap_or(self.width).min(self.width);
                for x in a..b {
                    if let Some(byte) = row.get_mut((x / 8) as usize) {
                        let m = 0x80u8 >> (x % 8);
                        if self.black_is_1 {
                            *byte |= m;
                        } else {
                            *byte &= !m;
                        }
                    }
                }
            }
        }
    }
    let total = stride
        .checked_mul(raw.height as usize)
        .ok_or_else(|| Fault::Malformed("image size overflows".into()))?;
    let mut sink = Rows {
        data: vec![white; total],
        stride,
        width: p.columns,
        rows: 0,
        height: raw.height,
        black_is_1: p.black_is_1,
    };
    let settings = hayro_ccitt::DecodeSettings {
        columns: p.columns,
        rows: raw.height,
        end_of_block: p.end_of_block,
        end_of_line: p.end_of_line,
        rows_are_byte_aligned: p.byte_align,
        encoding: match p.k {
            k if k < 0 => hayro_ccitt::EncodingMode::Group4,
            0 => hayro_ccitt::EncodingMode::Group3_1D,
            _ => hayro_ccitt::EncodingMode::Group3_2D,
        },
        strict: false,
    };
    let mut ctx = hayro_ccitt::DecoderContext::new(settings);
    let result = hayro_ccitt::decode(&raw.data, &mut sink, &mut ctx);
    if sink.rows == 0 {
        return corrupt(&format!(
            "CCITT data holds no rows{}",
            if result.is_err() {
                " (decoder error)"
            } else {
                ""
            }
        ));
    }
    if result.is_err() || sink.rows < raw.height {
        warn.0.push("image_truncated");
    }
    Ok(Packed {
        data: sink.data,
        stride,
        width: p.columns,
        height: raw.height,
        bpc: 1,
        comps: 1,
    })
}

fn jbig2(
    raw: &RawImage,
    globals: Option<&[u8]>,
    max_pixels: u64,
    warn: &mut Warnings,
) -> Result<Packed, Fault> {
    let settings = hayro_jbig2::DecodeSettings::default();
    let g = match globals {
        Some(d) => match hayro_jbig2::Globals::new(d, &settings) {
            Ok(g) => Some(g),
            Err(e) => return corrupt(&format!("JBIG2 globals: {e}")),
        },
        None => None,
    };
    let page = match hayro_jbig2::Page::new_embedded(&raw.data, g.as_ref(), &settings) {
        Ok(p) => p,
        Err(e) => return corrupt(&format!("JBIG2 data: {e}")),
    };
    let (w, h) = (page.width(), page.height());
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(Fault::Limit(
            "image_pixels",
            "a JBIG2 image has more pixels than the limit allows".into(),
        ));
    }
    let stride = (w as usize).div_ceil(8);
    struct Sink {
        data: Vec<u8>,
        stride: usize,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
    }
    impl Sink {
        fn clear_bit(&mut self) {
            if self.x < self.width && self.y < self.height {
                let at = self.y as usize * self.stride + (self.x / 8) as usize;
                if let Some(b) = self.data.get_mut(at) {
                    *b &= !(0x80u8 >> (self.x % 8));
                }
            }
        }
    }
    impl hayro_jbig2::Decoder for Sink {
        fn push_pixel(&mut self, black: bool) {
            if black {
                self.clear_bit();
            }
            self.x = self.x.saturating_add(1);
        }
        fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
            let n = chunk_count.saturating_mul(8);
            if !black {
                self.x = self.x.saturating_add(n);
                return;
            }
            for _ in 0..n {
                self.clear_bit();
                self.x = self.x.saturating_add(1);
            }
        }
        fn next_line(&mut self) {
            self.x = 0;
            self.y = self.y.saturating_add(1);
        }
    }
    let mut sink = Sink {
        // JBIG2 1 = black; the PDF filter output is 0 = black, so start white (all ones).
        data: vec![0xFF; stride * h as usize],
        stride,
        width: w,
        height: h,
        x: 0,
        y: 0,
    };
    if let Err(e) = page.decode(&mut sink) {
        return corrupt(&format!("JBIG2 decoding: {e}"));
    }
    if sink.y < h {
        warn.0.push("image_truncated");
    }
    Ok(Packed {
        data: sink.data,
        stride,
        width: w,
        height: h,
        bpc: 1,
        comps: 1,
    })
}

fn jpeg(raw: &RawImage, max_pixels: u64) -> Result<(Packed, Color), Fault> {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;
    let mut dec = zune_jpeg::JpegDecoder::new(ZCursor::new(raw.data.as_slice()));
    if let Err(e) = dec.decode_headers() {
        return corrupt(&format!("JPEG header: {e:?}"));
    }
    let (Some(info), Some(input)) = (dec.info(), dec.input_colorspace()) else {
        return corrupt("JPEG without a frame header");
    };
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(Fault::Limit(
            "image_pixels",
            "a JPEG image has more pixels than the limit allows".into(),
        ));
    }
    let (out_cs, color, comps) = match input {
        ColorSpace::Luma => (ColorSpace::Luma, Color::Gray, 1),
        ColorSpace::CMYK | ColorSpace::YCCK => (ColorSpace::CMYK, Color::Cmyk, 4),
        _ => (ColorSpace::RGB, Color::Rgb, 3),
    };
    dec.set_options(DecoderOptions::default().jpeg_set_out_colorspace(out_cs));
    let size = (w as usize) * (h as usize) * comps;
    let mut data = vec![0u8; size];
    if let Err(e) = dec.decode_into(&mut data) {
        return corrupt(&format!("JPEG data: {e:?}"));
    }
    Ok((
        Packed {
            data,
            stride: (w as usize) * comps,
            width: w,
            height: h,
            bpc: 8,
            comps,
        },
        color,
    ))
}

/// Unpacks samples to 0..=255 (indices stay raw for Indexed), applies /Decode, converts the
/// colour space.
fn to_pixmap(
    p: &Packed,
    color: &Color,
    raw: &RawImage,
    dl: &Deadline,
    warn: &mut Warnings,
) -> Result<Pixmap, Fault> {
    let indexed = matches!(color, Color::Indexed { .. });
    let (w, h) = (p.width as usize, p.height as usize);
    let out_ch = if raw.mask {
        1
    } else {
        match color {
            Color::Gray | Color::Tint => 1,
            Color::Rgb | Color::Cmyk => 3,
            Color::Indexed { base, .. } => {
                if matches!(**base, Color::Gray | Color::Tint) {
                    1
                } else {
                    3
                }
            }
        }
    };
    // Per-component decode tables (0..=255 in, 0..=255 out).
    let ncomp = p.comps.max(1);
    let luts: Vec<[u8; 256]> = (0..ncomp)
        .map(|c| {
            let (d0, d1) = match (indexed, raw.decode.get(2 * c), raw.decode.get(2 * c + 1)) {
                (false, Some(&a), Some(&b)) => (a, b),
                _ => (0.0, 1.0),
            };
            let mut t = [0u8; 256];
            for (i, slot) in t.iter_mut().enumerate() {
                *slot = if indexed {
                    i as u8
                } else {
                    let v = d0 + (i as f64 / 255.0) * (d1 - d0);
                    (v.clamp(0.0, 1.0) * 255.0).round() as u8
                };
            }
            t
        })
        .collect();
    let palette: Vec<u8> = match color {
        Color::Indexed { base, hival, table } => {
            let nb = base.components();
            let mut pal = Vec::new();
            for i in 0..=(*hival as usize) {
                let comps: Vec<u8> = (0..nb)
                    .map(|k| table.get(i * nb + k).copied().unwrap_or(0))
                    .collect();
                convert(base, &comps, out_ch, &mut pal);
            }
            pal
        }
        _ => Vec::new(),
    };
    let mut out = Vec::with_capacity(w.saturating_mul(h).saturating_mul(out_ch));
    let mut row_samples = vec![0u8; w * ncomp];
    let mut missing_rows = 0u32;
    for y in 0..h {
        if y % 256 == 0 {
            dl.check()?;
        }
        let row = p.data.get(y * p.stride..(y + 1) * p.stride);
        let Some(row) = row else {
            missing_rows += 1;
            // Missing data is paper: white (or no stencil coverage).
            out.extend(std::iter::repeat_n(
                if raw.mask { 0 } else { 255 },
                w * out_ch,
            ));
            continue;
        };
        unpack(row, p.bpc, &mut row_samples, indexed);
        if raw.mask {
            for &s in &row_samples {
                let v = luts
                    .first()
                    .map_or(s, |l| l.get(s as usize).copied().unwrap_or(s));
                out.push(if v < 128 { 255 } else { 0 });
            }
            continue;
        }
        match color {
            Color::Gray if raw.decode.is_empty() => out.extend_from_slice(&row_samples),
            Color::Indexed { .. } => {
                for &s in &row_samples {
                    let n = palette.len() / out_ch.max(1);
                    let at = (s as usize).min(n.saturating_sub(1)) * out_ch;
                    match palette.get(at..at + out_ch) {
                        Some(c) => out.extend_from_slice(c),
                        None => out.extend(std::iter::repeat_n(255u8, out_ch)),
                    }
                }
            }
            _ => {
                let mut comps = vec![0u8; ncomp];
                for px in row_samples.chunks_exact(ncomp) {
                    for ((d, &s), lut) in comps.iter_mut().zip(px).zip(&luts) {
                        *d = lut.get(s as usize).copied().unwrap_or(s);
                    }
                    convert(color, &comps, out_ch, &mut out);
                }
            }
        }
    }
    if missing_rows > 0 {
        if missing_rows as usize == h {
            return corrupt("the image holds no sample data");
        }
        warn.0.push("image_truncated");
    }
    Ok(Pixmap {
        w: p.width,
        h: p.height,
        ch: out_ch,
        data: out,
        mask: raw.mask,
    })
}

/// One pixel of colour `c` (already decoded) appended as `out_ch` channels.
fn convert(c: &Color, v: &[u8], out_ch: usize, out: &mut Vec<u8>) {
    let g = |i: usize| v.get(i).copied().unwrap_or(0);
    let (r, gg, b) = match c {
        Color::Gray => (g(0), g(0), g(0)),
        Color::Tint => (255 - g(0), 255 - g(0), 255 - g(0)),
        Color::Rgb => (g(0), g(1), g(2)),
        Color::Cmyk => {
            let k = 255 - u32::from(g(3));
            let f = |x: u8| ((255 - u32::from(x)) * k / 255) as u8;
            (f(g(0)), f(g(1)), f(g(2)))
        }
        Color::Indexed { .. } => (g(0), g(0), g(0)),
    };
    if out_ch == 1 {
        out.push(luma(r, gg, b));
    } else {
        out.extend_from_slice(&[r, gg, b]);
    }
}

pub fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 + 500) / 1000) as u8
}

/// Samples of one row at `bpc` bits, scaled to 0..=255 (raw values when `raw_values`).
fn unpack(row: &[u8], bpc: u32, out: &mut [u8], raw_values: bool) {
    let scale = |v: u32, max: u32| -> u8 {
        if raw_values {
            v as u8
        } else {
            (v * 255 / max) as u8
        }
    };
    match bpc {
        8 => {
            for (o, &s) in out.iter_mut().zip(row) {
                *o = s;
            }
        }
        16 => {
            for (o, s) in out.iter_mut().zip(row.iter().step_by(2)) {
                *o = if raw_values { 0 } else { *s };
            }
        }
        1 | 2 | 4 => {
            let per = 8 / bpc;
            let mask = (1u32 << bpc) - 1;
            let mut it = out.iter_mut();
            'rows: for &byte in row {
                for k in 0..per {
                    let shift = 8 - bpc * (k + 1);
                    let v = (u32::from(byte) >> shift) & mask;
                    match it.next() {
                        Some(o) => *o = scale(v, mask),
                        None => break 'rows,
                    }
                }
            }
            for o in it {
                *o = 0;
            }
        }
        _ => out.fill(0),
    }
}
