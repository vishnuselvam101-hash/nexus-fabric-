#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
use std::io::Read;
use std::path::PathBuf;

pub fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub struct Png {
    pub w: usize,
    pub h: usize,
    pub ch: usize,
    pub px: Vec<u8>,
    pub dpi: u32,
}

/// Decodes the PNGs this crate writes (8-bit gray or RGB, filter None or Up).
pub fn decode_png(b: &[u8]) -> Png {
    assert_eq!(&b[..8], b"\x89PNG\r\n\x1a\n");
    let mut at = 8;
    let (mut w, mut h, mut ch, mut dpi) = (0, 0, 0, 0);
    let mut idat = Vec::new();
    while at + 8 <= b.len() {
        let len = u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize;
        let kind = &b[at + 4..at + 8];
        let data = &b[at + 8..at + 8 + len];
        match kind {
            b"IHDR" => {
                w = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
                h = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
                assert_eq!(data[8], 8, "bit depth");
                ch = match data[9] {
                    0 => 1,
                    2 => 3,
                    t => panic!("colour type {t}"),
                };
            }
            b"pHYs" => {
                let ppm = u32::from_be_bytes(data[0..4].try_into().unwrap());
                dpi = (f64::from(ppm) * 0.0254).round() as u32;
            }
            b"IDAT" => idat.extend_from_slice(data),
            _ => {}
        }
        at += 12 + len;
    }
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&idat[..])
        .read_to_end(&mut raw)
        .unwrap();
    let stride = w * ch;
    assert_eq!(raw.len(), (stride + 1) * h);
    let mut px = vec![0u8; stride * h];
    for y in 0..h {
        let f = raw[y * (stride + 1)];
        for x in 0..stride {
            let v = raw[y * (stride + 1) + 1 + x];
            px[y * stride + x] = match f {
                0 => v,
                2 if y > 0 => v.wrapping_add(px[(y - 1) * stride + x]),
                2 => v,
                _ => panic!("filter {f}"),
            };
        }
    }
    Png { w, h, ch, px, dpi }
}

impl Png {
    pub fn dark(&self) -> usize {
        self.px.iter().filter(|&&v| v < 128).count() / self.ch
    }
    pub fn mean_abs_diff(&self, o: &Png) -> f64 {
        assert_eq!((self.w, self.h, self.ch), (o.w, o.h, o.ch));
        let s: u64 = self
            .px
            .iter()
            .zip(&o.px)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        s as f64 / self.px.len() as f64
    }
}

/// A one-page PDF from a media box and a content stream (no xref: the reader rebuilds it).
pub fn tiny_pdf(media: [f64; 4], content: &str) -> Vec<u8> {
    format!(
        "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [{} {} {} {}] /Contents 4 0 R /Resources << >> >>\nendobj\n4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\ntrailer\n<< /Root 1 0 R /Size 5 >>\n%%EOF\n",
        media[0], media[1], media[2], media[3], content.len(), content
    )
    .into_bytes()
}
