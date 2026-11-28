//! nf-numread: find every number on a drawing page and where it is.
//!
//! Offline, CPU only, no training: PP-OCRv4 text detection and recognition (Apache-2.0) run on the
//! pure Rust `rten` ONNX runtime. The page picture goes in; a list of numbers comes out, each with the
//! text as printed (prefix mark and unit mark included), the parsed value, a kind hint from the prefix,
//! its box in page pixels and a confidence. The reader only proposes: a person clicks the number to
//! fill a slot and confirms it; nothing here approves a value.
//!
//! No unsafe code in this crate (the runtime dependency has its own, as any numeric runtime does).
#![forbid(unsafe_code)]

use nf_core::reading::{same_number, text_contains_number, CheckCtx, ItemCheck, ItemIn, OcrCheck};

mod det;
pub mod image_in;
mod rec;
pub mod text;

use image_in::Gray;
use rten::Model;
use sha2::{Digest, Sha256};
pub use text::Kind;

use std::path::{Path, PathBuf};

/// Model file names inside the model directory (see models/MODELS.md).
pub const DET_FILE: &str = "ch_PP-OCRv4_det_infer.onnx";
pub const REC_FILE: &str = "ch_PP-OCRv4_rec_infer.onnx";
pub const DICT_FILE: &str = "ppocr_keys_v1.txt";

/// SHA-256 of the model files (see models/MODELS.md); checked every time a reader is built.
pub const DET_SHA256: &str = "d2a7720d45a54257208b1e13e36a8479894cb74155a5efe29462512d42f49da9";
pub const REC_SHA256: &str = "48fc40f24f6d2a207a2b1091d3437eb3cc3eb6b676dc3ef9c37384005483683b";
pub const DICT_SHA256: &str = "28b2362ad4ab2dc38769aa72feb535e3a9ddb3fd2a7585a05920e6393b1dc7f7";

/// Environment variable naming the model directory.
pub const MODEL_DIR_ENV: &str = "NF_NUMREAD_MODELS";

/// Largest model file read (the real ones are 4.7 and 10.9 MB); a bigger file is refused before it
/// is read into memory or hashed.
pub const MAX_MODEL_BYTES: u64 = 64 * 1024 * 1024;
/// Largest dictionary file read (the real one is 26 KB).
pub const MAX_DICT_BYTES: u64 = 1024 * 1024;

/// A reading whose mean character probability is below this is flagged `low_confidence`.
/// Chosen at about the 4th percentile of the sample page items (SPIKE.md); the UI must ask a person
/// to confirm such a number.
pub const LOW_CONFIDENCE_BELOW: f32 = 0.85;

/// The configured model directory: `$NF_NUMREAD_MODELS`, else the crate's own `models/` directory.
pub fn default_model_dir() -> PathBuf {
    match std::env::var_os(MODEL_DIR_ENV) {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("models"),
    }
}

/// Read a model file (at most `max` bytes) and refuse it unless its SHA-256 is the pinned one.
fn read_pinned(dir: &Path, name: &str, want: &str, max: u64) -> Result<Vec<u8>, Error> {
    use std::io::Read;
    let path = dir.join(name);
    let cannot = |e: std::io::Error| {
        Error::Model(format!(
            "cannot read {} ({e}); run crates/nf-numread/scripts/fetch-models.sh or set {MODEL_DIR_ENV}",
            path.display()
        ))
    };
    let file = std::fs::File::open(&path).map_err(cannot)?;
    let too_big = || {
        Error::Model(format!(
            "{} is too large (limit {max} bytes); not read",
            path.display()
        ))
    };
    if file.metadata().map_err(cannot)?.len() > max {
        return Err(too_big());
    }
    // the limit is also enforced while reading (the file may grow after the size check)
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes).map_err(cannot)?;
    if bytes.len() as u64 > max {
        return Err(too_big());
    }
    let got = hex(&bytes);
    if got != want {
        return Err(Error::Model(format!(
            "{} does not match its pinned SHA-256 (got {got}, want {want})",
            path.display()
        )));
    }
    Ok(bytes)
}

#[derive(Debug)]
pub enum Error {
    /// The picture is not a readable PNG or its size is outside the accepted range.
    Image(String),
    /// A model file failed its hash check, did not load, or returned something unexpected.
    Model(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Image(m) => write!(f, "image: {m}"),
            Error::Model(m) => write!(f, "model: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Tunable numbers; the defaults are the measured ones (SPIKE.md).
#[derive(Clone, Debug)]
pub struct Params {
    /// Long side of the picture handed to the detector.
    pub det_side: usize,
    /// Detector probability above which a pixel is text.
    pub bin_thresh: f32,
    /// Minimum mean probability of a detected region; regions below it are not reported at all
    /// (they are not flagged, they do not exist in the output).
    pub box_thresh: f32,
    /// Region growth factor (DB "unclip").
    pub unclip: f32,
    /// Smallest region, in detector pixels.
    pub min_pixels: u32,
    /// Height the recogniser sees, and the widest crop it is given.
    pub rec_height: usize,
    pub rec_max_width: usize,
    /// Also try tall regions rotated by 90 degrees (vertical dimension text).
    pub vertical: bool,
    /// A tall region must beat the upright reading by this much to be read rotated.
    pub vertical_margin: f32,
    /// A digit-led token gets a diameter sign when the recogniser put at least this much probability
    /// on a phi-like class just before it (0 switches the repair off).
    pub phi_thresh: f32,
    /// Upper bound on regions per page (time and memory are bounded for any input).
    pub max_regions: usize,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            det_side: 1536,
            bin_thresh: 0.3,
            box_thresh: 0.6,
            unclip: 1.5,
            min_pixels: 3,
            rec_height: 48,
            rec_max_width: 3200,
            vertical: true,
            vertical_margin: 0.05,
            phi_thresh: 0.05,
            max_regions: 3000,
        }
    }
}

/// One number found on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberItem {
    /// The number as printed, cleaned of stray dimension-line strokes (prefix and unit marks kept).
    pub text: String,
    /// What the recogniser returned when `text` had to be repaired.
    pub raw: Option<String>,
    /// Parsed numeric value (None for a date or a fraction such as 1/1).
    pub value: Option<f64>,
    pub kind: Kind,
    /// [x0, y0, x1, y1] in pixels of the page picture.
    pub bbox: [f32; 4],
    /// Mean character probability, 0..1 (of the characters the recogniser returned; an inferred
    /// diameter sign is not counted).
    pub confidence: f32,
    /// The text differs from what the recogniser returned (`raw` holds that).
    pub repaired: bool,
    /// The diameter sign was not read but put in front because the recogniser showed a phi-like
    /// glyph there (`Params::phi_thresh`).
    pub phi_inferred: bool,
    /// More than one reading of the written form (comma that is not a thousands group, ratio/time).
    pub ambiguous: bool,
    /// `confidence` is below `LOW_CONFIDENCE_BELOW`.
    pub low_confidence: bool,
}

pub struct Reader {
    det: Model,
    rec: Model,
    dict: Vec<String>,
    params: Params,
}

fn hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

impl Reader {
    /// Models from the configured directory (`default_model_dir`), default parameters.
    pub fn new() -> Result<Reader, Error> {
        Reader::with_params(Params::default())
    }

    pub fn with_params(params: Params) -> Result<Reader, Error> {
        Reader::from_dir(&default_model_dir(), params)
    }

    /// Load the three model files from `dir`; any file whose SHA-256 differs from the pinned value
    /// is refused before it is parsed.
    pub fn from_dir(dir: &Path, params: Params) -> Result<Reader, Error> {
        let det_bytes = read_pinned(dir, DET_FILE, DET_SHA256, MAX_MODEL_BYTES)?;
        let rec_bytes = read_pinned(dir, REC_FILE, REC_SHA256, MAX_MODEL_BYTES)?;
        let dict_bytes = read_pinned(dir, DICT_FILE, DICT_SHA256, MAX_DICT_BYTES)?;
        let dict_text = String::from_utf8(dict_bytes)
            .map_err(|_| Error::Model("dictionary is not UTF-8".into()))?;
        let det = Model::load(det_bytes).map_err(|e| Error::Model(format!("detector: {e}")))?;
        let rec = Model::load(rec_bytes).map_err(|e| Error::Model(format!("recogniser: {e}")))?;
        // CTC index 0 is the blank; the last index is the space the model was trained with.
        let mut dict = vec![String::new()];
        dict.extend(dict_text.lines().map(str::to_string));
        dict.push(" ".to_string());
        Ok(Reader {
            det,
            rec,
            dict,
            params,
        })
    }

    pub fn read_png(&self, png: &[u8]) -> Result<Vec<NumberItem>, Error> {
        self.read_gray(&image_in::decode_png(png)?)
    }

    /// Every number on the page, ordered top to bottom then left to right (by box corner).
    pub fn read_gray(&self, g: &Gray) -> Result<Vec<NumberItem>, Error> {
        image_in::check_size(g.w, g.h)?;
        if g.w < 16 || g.h < 16 {
            return Ok(Vec::new());
        }
        let p = &self.params;
        let boxes = det::detect(&self.det, g, p)?;
        let mut items = Vec::new();
        for b in boxes.iter().take(p.max_regions) {
            let (x0, y0) = (
                b[0].floor().max(0.0) as usize,
                b[1].floor().max(0.0) as usize,
            );
            let (x1, y1) = (b[2].ceil() as usize, b[3].ceil() as usize);
            let Some(crop) = g.crop(x0, y0, x1, y1) else {
                continue;
            };
            if crop.w < 3 || crop.h < 3 {
                continue;
            }
            let (bx0, by0, bx1, by1) = (
                x0 as f32,
                y0 as f32,
                (x0 + crop.w) as f32,
                (y0 + crop.h) as f32,
            );
            let mut best = (
                Orient::Horizontal,
                crop.clone(),
                rec::recognise(&self.rec, &self.dict, &crop, p)?,
            );
            if p.vertical && crop.h as f32 >= 1.5 * crop.w as f32 {
                let mut top = best.2.mean_prob() + p.vertical_margin;
                for (o, cw) in [(Orient::VerticalUp, true), (Orient::VerticalDown, false)] {
                    let turned = crop.rotate90(cw);
                    let line = rec::recognise(&self.rec, &self.dict, &turned, p)?;
                    if line.mean_prob() > top {
                        top = line.mean_prob();
                        best = (o, turned, line);
                    }
                }
            }
            let (orient, upright, line) = best;
            let pitch = line.pitch();
            if line.steps == 0 {
                continue;
            }
            for mut tok in text::tokens(&line.chars) {
                // raw and confidence are taken before any repair or inference
                let raw = tok.text();
                let confidence = tok.conf();
                let phi_inferred = p.phi_thresh > 0.0 && add_phi(&mut tok, &line, p.phi_thresh);
                let Some(parsed) = text::parse(&tok.text()) else {
                    continue;
                };
                let repaired = phi_inferred || parsed.text != raw;
                let (first, last) = (tok.chars[0].step, tok.chars[tok.chars.len() - 1].step);
                let ub = token_box(&upright, line.steps, first, last, pitch);
                let bbox = to_page(orient, [bx0, by0, bx1, by1], ub);
                items.push(NumberItem {
                    raw: repaired.then_some(raw),
                    text: parsed.text,
                    value: parsed.value,
                    kind: parsed.kind,
                    bbox,
                    confidence,
                    repaired,
                    phi_inferred,
                    ambiguous: parsed.ambiguous,
                    low_confidence: confidence < LOW_CONFIDENCE_BELOW,
                });
            }
        }
        items.sort_by(|a, b| {
            a.bbox[1]
                .total_cmp(&b.bbox[1])
                .then(a.bbox[0].total_cmp(&b.bbox[0]))
                .then(a.text.cmp(&b.text))
        });
        Ok(items)
    }

    /// Read a known box directly, without running the page detector again. The detector can miss a
    /// small, already located crop; the recogniser reads the crop at its own height.
    fn read_box_text(&self, crop: &Gray) -> Result<Vec<(String, Option<f64>)>, Error> {
        let p = &self.params;
        let upright = rec::recognise(&self.rec, &self.dict, crop, p)?;
        let mut line = upright;
        if p.vertical && crop.h as f32 >= 1.5 * crop.w as f32 {
            for cw in [true, false] {
                let turned = crop.rotate90(cw);
                let candidate = rec::recognise(&self.rec, &self.dict, &turned, p)?;
                if candidate.mean_prob() > line.mean_prob() + p.vertical_margin {
                    line = candidate;
                }
            }
        }
        let mut out = Vec::new();
        if line.steps == 0 {
            return Ok(out);
        }
        for mut tok in text::tokens(&line.chars) {
            if p.phi_thresh > 0.0 {
                add_phi(&mut tok, &line, p.phi_thresh);
            }
            if let Some(parsed) = text::parse(&tok.text()) {
                out.push((parsed.text, parsed.value));
            }
        }
        Ok(out)
    }

    /// Second read of one proposed value's box: re-read only that part of the page and compare every
    /// number found with the proposed value. The result is a verdict (proposal only) from
    /// `nf-core::reading`; no value is accepted or confirmed here. `input_sha256` is the hash of the
    /// exact grey pixels handed to the second read, and `model_id` says which models made it.
    pub fn verify_box(
        &self,
        page: &[u8],
        bbox: [f32; 4],
        key: &str,
        text: &str,
        value: f64,
        source: &str,
    ) -> Result<BoxVerification, Error> {
        let g = image_in::decode_png(page)?;
        let ctx = CheckCtx {
            page: Some((g.w as f64, g.h as f64)),
            require_box: true,
        };
        let b64 = bbox.map(f64::from);
        let box_ok = matches!(
            nf_core::reading::check_box(Some(&b64), ctx.page),
            nf_core::reading::BoxCheck::Ok
        );
        let mut model_error = None;
        let mut input_sha256 = String::new();
        let ocr: OcrCheck = if box_ok {
            let x0 = (bbox[0].floor().max(0.0) as usize).min(g.w);
            let y0 = (bbox[1].floor().max(0.0) as usize).min(g.h);
            let x1 = (bbox[2].ceil().max(0.0) as usize).min(g.w);
            let y1 = (bbox[3].ceil().max(0.0) as usize).min(g.h);
            match g.crop(x0, y0, x1, y1) {
                Some(crop) => {
                    input_sha256 = hex(&crop.data);
                    match self.read_box_text(&crop) {
                        Ok(tokens) => {
                            let read: Vec<String> = tokens.iter().map(|(t, _)| t.clone()).collect();
                            let agrees = tokens.iter().any(|(t, v)| {
                                v.is_some_and(|n| same_number(n, value))
                                    || text_contains_number(t, value)
                            });
                            if agrees {
                                OcrCheck::Agrees { read }
                            } else {
                                OcrCheck::Disagrees { read }
                            }
                        }
                        Err(e) => {
                            model_error = Some(e.to_string());
                            OcrCheck::Unavailable("ocr_failed".into())
                        }
                    }
                }
                None => OcrCheck::Unavailable("ocr_failed".into()),
            }
        } else {
            OcrCheck::Unavailable("ocr_bad_box".into())
        };
        let check: ItemCheck = nf_core::reading::check_item(
            &ItemIn {
                key,
                value: Some(value),
                text,
                bbox: Some(&b64),
                source,
            },
            &ctx,
            ocr,
        );
        Ok(BoxVerification {
            check,
            model_id: MODEL_ID,
            input_sha256,
            page_sha256: hex(page),
            page: (g.w, g.h),
            model_error,
        })
    }
}

/// Identity of the models whose second read produced a verdict.
pub const MODEL_ID: &str = "nf-numread R0 / PP-OCRv4 (det+rec)";

/// The verdict of one second read, with the provenance a proposal must carry. The model only
/// proposes: the person still confirms (and the verdict never grants authority).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxVerification {
    pub check: ItemCheck,
    pub model_id: &'static str,
    /// SHA-256 of the grey crop pixels handed to the second read (empty when no crop was made).
    pub input_sha256: String,
    /// SHA-256 of the page image the crop came from; binds the verdict to those exact pixels.
    pub page_sha256: String,
    /// Size of the page image the crop came from, in pixels.
    pub page: (usize, usize),
    /// Model error text when the second read could not be made.
    pub model_error: Option<String>,
}

impl BoxVerification {
    pub fn to_json(&self) -> String {
        let mut check = self.check.to_json();
        if let Some(obj) = check.as_object_mut() {
            obj.insert("model_id".into(), serde_json::json!(self.model_id));
            obj.insert("input_sha256".into(), serde_json::json!(self.input_sha256));
            obj.insert("page_sha256".into(), serde_json::json!(self.page_sha256));
            obj.insert("page".into(), serde_json::json!([self.page.0, self.page.1]));
            if let Some(e) = &self.model_error {
                obj.insert("model_error".into(), serde_json::json!(e));
            }
        }
        serde_json::to_string_pretty(&check).unwrap_or_default()
    }
}
/// phi-like glyph in the few steps before a bare number, put the sign back.
/// Returns true when a sign was inserted.
fn add_phi(tok: &mut text::Tok, line: &rec::Line, thresh: f32) -> bool {
    let first = tok.chars[0].step;
    if !tok.chars[0]
        .glyph
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
    {
        return false;
    }
    let prev_end = line
        .chars
        .iter()
        .rev()
        .find(|c| c.step < first)
        .map_or(0, |c| c.step + 1);
    let from = prev_end.max(first.saturating_sub(4));
    let mut best = (0usize, 0f32);
    for t in from..first {
        if line.phi[t] > best.1 {
            best = (t, line.phi[t]);
        }
    }
    if best.1 >= thresh {
        tok.chars.insert(
            0,
            rec::Ch {
                glyph: "φ".to_string(),
                step: best.0,
                prob: best.1,
            },
        );
        return true;
    }
    false
}

#[derive(Clone, Copy, Debug)]
enum Orient {
    Horizontal,
    /// Read after a clockwise turn: the text runs from the bottom of the region to the top.
    VerticalUp,
    /// Read after a counter-clockwise turn: the text runs from the top to the bottom.
    VerticalDown,
}

/// Box, in the upright crop, of the characters at CTC steps first..=last: the step window (widened by
/// half a character pitch, since a CTC character fires near its middle) snapped to the ink inside it.
fn token_box(upright: &Gray, steps: usize, first: usize, last: usize, pitch: f32) -> [f32; 4] {
    let (w, h) = (upright.w as f32, upright.h as f32);
    let per = w / steps as f32;
    let x0 = ((first as f32 - 0.5 * pitch) * per).clamp(0.0, w);
    let x1 = (((last as f32 + 0.5 * pitch) + 0.5) * per).clamp(0.0, w);
    match upright.ink_bounds(x0.floor() as usize, x1.ceil() as usize) {
        Some(b) => b,
        None => [x0, 0.0, x1.max(x0), h],
    }
}

/// Map a box of the upright (possibly turned) crop back to page pixels; `r` is the region on the page.
fn to_page(o: Orient, r: [f32; 4], u: [f32; 4]) -> [f32; 4] {
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    match o {
        Orient::Horizontal => [r[0] + u[0], r[1] + u[1], r[0] + u[2], r[1] + u[3]],
        Orient::VerticalUp => [r[0] + u[1], r[1] + h - u[2], r[0] + u[3], r[1] + h - u[0]],
        Orient::VerticalDown => [r[0] + w - u[3], r[1] + u[0], r[0] + w - u[1], r[1] + u[2]],
    }
}

fn r1(x: f32) -> f64 {
    (f64::from(x) * 10.0).round() / 10.0
}

/// The CLI document: {"image":{width,height},"items":[{text,value,kind_hint,box,confidence,repaired,phi_inferred,ambiguous,low_confidence[,raw]}]}.
pub fn to_json(items: &[NumberItem], width: usize, height: usize) -> String {
    use serde_json::{json, Value};
    let list: Vec<Value> = items
        .iter()
        .map(|i| {
            let mut v = json!({
                "text": i.text,
                "value": i.value,
                "kind_hint": i.kind.name(),
                "box": [r1(i.bbox[0]), r1(i.bbox[1]), r1(i.bbox[2]), r1(i.bbox[3])],
                "confidence": (f64::from(i.confidence) * 1000.0).round() / 1000.0,
            });
            v["repaired"] = json!(i.repaired);
            v["phi_inferred"] = json!(i.phi_inferred);
            v["ambiguous"] = json!(i.ambiguous);
            v["low_confidence"] = json!(i.low_confidence);
            if let Some(r) = &i.raw {
                v["raw"] = json!(r);
            }
            v
        })
        .collect();
    let doc = json!({
        "schema": "nf-numread/1",
        "image": { "width": width, "height": height },
        "items": list,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rec::{Ch, Line};

    fn ch(g: &str, step: usize, prob: f32) -> Ch {
        Ch {
            glyph: g.into(),
            step,
            prob,
        }
    }

    #[test]
    fn inferred_phi_is_flagged_and_raw_is_the_unrepaired_text() {
        let line = Line {
            chars: vec![ch("1", 6, 0.9), ch("2", 8, 0.9)],
            steps: 12,
            phi: vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        let mut tok = text::Tok {
            chars: line.chars.clone(),
        };
        let raw = tok.text();
        let conf = tok.conf();
        assert!(add_phi(&mut tok, &line, 0.05));
        assert_eq!(raw, "12");
        assert_eq!(tok.text(), "φ12");
        let parsed = text::parse(&tok.text()).unwrap();
        assert_eq!(parsed.kind, Kind::Diameter);
        // the confidence taken before insertion is not dragged down by the inserted glyph
        assert!(conf > 0.89);
        // no phi-like step: nothing inserted
        let mut tok2 = text::Tok {
            chars: line.chars.clone(),
        };
        let quiet = Line {
            phi: vec![0.0; 12],
            ..line
        };
        assert!(!add_phi(&mut tok2, &quiet, 0.05));
        assert_eq!(tok2.text(), "12");
    }

    #[test]
    fn low_confidence_flag_in_item_and_json() {
        let mk = |confidence: f32| NumberItem {
            text: "12.5".into(),
            raw: None,
            value: Some(12.5),
            kind: Kind::Number,
            bbox: [0.0, 0.0, 10.0, 10.0],
            confidence,
            repaired: false,
            phi_inferred: false,
            ambiguous: false,
            low_confidence: confidence < LOW_CONFIDENCE_BELOW,
        };
        let (lo, hi) = (mk(LOW_CONFIDENCE_BELOW - 0.01), mk(LOW_CONFIDENCE_BELOW));
        assert!(lo.low_confidence && !hi.low_confidence);
        let doc: serde_json::Value =
            serde_json::from_str(&to_json(&[lo, hi], 100, 100)).expect("json");
        assert_eq!(doc["items"][0]["low_confidence"], true);
        assert_eq!(doc["items"][1]["low_confidence"], false);
        for k in ["repaired", "phi_inferred", "ambiguous"] {
            assert_eq!(doc["items"][0][k], false, "{k}");
        }
    }
}
