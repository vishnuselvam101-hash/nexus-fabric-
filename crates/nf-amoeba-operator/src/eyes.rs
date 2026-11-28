//! Eyes: turn a page picture into numbers with boxes. The only contract is `Eyes`.

use nf_core::assign::Reading;
use std::collections::BTreeMap;

pub struct PageRead {
    pub readings: Vec<Reading>,
    pub width: f32,
    pub height: f32,
}

pub trait Eyes {
    fn name(&self) -> String;
    fn read(&mut self, page: i64, png: &[u8]) -> Result<PageRead, String>;
}

/// Width and height from a PNG header.
pub fn png_size(b: &[u8]) -> Option<(f32, f32)> {
    if b.len() < 24 || &b[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
    Some((w as f32, h as f32))
}

/// R0: the offline number reader (nf-numread).
pub struct NumreadEyes {
    reader: nf_numread::Reader,
}

impl NumreadEyes {
    pub fn from_dir(dir: &std::path::Path) -> Result<NumreadEyes, String> {
        let reader = nf_numread::Reader::from_dir(dir, nf_numread::Params::default())
            .map_err(|e| format!("number reader: {e:?}"))?;
        Ok(NumreadEyes { reader })
    }
}

impl Eyes for NumreadEyes {
    fn name(&self) -> String {
        "nf-numread R0".into()
    }
    fn read(&mut self, page: i64, png: &[u8]) -> Result<PageRead, String> {
        let (width, height) = png_size(png).ok_or("the page picture is not a PNG")?;
        let items = self.reader.read_png(png).map_err(|e| format!("reading page {page}: {e:?}"))?;
        let mut readings = Vec::new();
        for it in items {
            let Some(value) = it.value else { continue };
            let mut flags = Vec::new();
            for (on, name) in [
                (it.repaired, "repaired"),
                (it.ambiguous, "ambiguous"),
                (it.phi_inferred, "phi_inferred"),
                (it.low_confidence, "low_confidence"),
            ] {
                if on {
                    flags.push(name.to_string());
                }
            }
            readings.push(Reading {
                id: readings.len(),
                text: it.text,
                value,
                page,
                bbox: it.bbox,
                confidence: it.confidence,
                flags,
                in_table: false,
            });
        }
        Ok(PageRead { readings, width, height })
    }
}

/// A reader that returns readings given beforehand (tests, and replaying a recorded read).
#[derive(Default)]
pub struct FixedEyes {
    pub pages: BTreeMap<i64, (Vec<Reading>, f32, f32)>,
}

impl Eyes for FixedEyes {
    fn name(&self) -> String {
        "fixed readings".into()
    }
    fn read(&mut self, page: i64, _png: &[u8]) -> Result<PageRead, String> {
        let (r, w, h) = self.pages.get(&page).ok_or("no readings for this page")?;
        Ok(PageRead { readings: r.clone(), width: *w, height: *h })
    }
}
