//! Page scenes for the page-image renderer (crate `nf-raster`): the lines and image placements
//! of one page in paint order, in output pixels, and the raw data of an image with the
//! non-image filters already removed.
//!
//! Nothing here decodes DCT, CCITT, JBIG2 or JPX data: those are returned as an
//! [`ImageFilter`] with their input bytes, and the caller decodes them in its own isolated
//! process. Everything that reads PDF structure goes through the same bounded parser as the
//! DXF converter ([`Limits`], [`Deadline`]).

use std::rc::Rc;
use std::time::Duration;

use crate::doc::Doc;
use crate::geom::M;
use crate::interp::{Interp, RunOpts};
use crate::lexer::{uint, Dict, Obj, Stream};
use crate::limits::{limit, malformed};
use crate::page;
use crate::report::Stats;

pub use crate::geom::M as Matrix;
pub use crate::limits::{Deadline, Fault, Limits};

/// Where an image placement gets its data.
#[derive(Clone, Debug)]
pub enum ImageSrc {
    /// An image XObject (its stream and, when indirect, its object number).
    Xobject(Stream, Option<u32>),
    /// An inline image: its dictionary (abbreviations expanded) and data.
    Inline(Rc<Dict>, Rc<Vec<u8>>),
}

/// One thing to draw. `Line` points and `Image` matrices are in output pixels, origin at the
/// lower left of the upright page, y up. An image maps the unit square to the page; row 0 of
/// the image is at y = 1.
#[derive(Clone, Debug)]
pub enum Item {
    Line {
        pts: Vec<(f64, f64)>,
        closed: bool,
        /// Line width in pixels (0: thinnest).
        width: f64,
        /// 0 black .. 255 white.
        gray: u8,
    },
    Image {
        ctm: M,
        src: ImageSrc,
        /// Fill gray at the time of painting (0 black .. 255 white), the colour of a stencil mask.
        fill: u8,
    },
}

/// A page ready to draw.
pub struct Scene {
    pub page_count: u32,
    /// Upright page size in pixels (not rounded).
    pub width: f64,
    pub height: f64,
    /// Page size in PDF points (user units times UserUnit).
    pub width_pt: f64,
    pub height_pt: f64,
    pub rotate: u32,
    pub items: Vec<Item>,
    /// Paint operations with geometry (strokes and fills).
    pub painted: u64,
    pub stats: Stats,
}

/// How an image's samples are coded.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageFilter {
    /// Raw samples (after Flate and the other plain filters), `bpc` bits per component.
    Raw,
    Dct,
    Ccitt(Ccitt),
    Jbig2 {
        globals: Option<Rc<Vec<u8>>>,
    },
    Jpx,
}

/// CCITTFaxDecode parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Ccitt {
    pub k: i64,
    pub columns: u32,
    pub rows: u32,
    pub black_is_1: bool,
    pub byte_align: bool,
    pub end_of_line: bool,
    pub end_of_block: bool,
}

/// A colour space, reduced to what the renderer converts.
#[derive(Clone, Debug, PartialEq)]
pub enum Color {
    Gray,
    Rgb,
    Cmyk,
    /// One component, 0 = no ink (white), 1 = full ink (Separation, one-colour DeviceN).
    Tint,
    Indexed {
        base: Box<Color>,
        hival: u32,
        table: Vec<u8>,
    },
}

impl Color {
    pub fn components(&self) -> usize {
        match self {
            Color::Gray | Color::Tint | Color::Indexed { .. } => 1,
            Color::Rgb => 3,
            Color::Cmyk => 4,
        }
    }
}

/// An image with its dictionary read and its non-image filters decoded.
#[derive(Clone, Debug)]
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    pub bpc: u32,
    pub color: Color,
    /// /Decode array (empty: default).
    pub decode: Vec<f64>,
    /// A stencil mask (paints the fill colour where the sample is 0 by default).
    pub mask: bool,
    pub filter: ImageFilter,
    pub data: Vec<u8>,
    /// The image has an /SMask or /Mask (not applied).
    pub has_alpha: bool,
}

/// An open document for rendering.
pub struct PdfDoc<'a> {
    doc: Doc<'a>,
}

fn rd_u32(doc: &mut Doc, d: &Dict, keys: &[&[u8]]) -> Result<Option<u32>, Fault> {
    for k in keys {
        if let Some(o) = doc.dict_entry(d, k)? {
            return Ok(o.num().and_then(uint));
        }
    }
    Ok(None)
}

impl<'a> PdfDoc<'a> {
    pub fn open(data: &'a [u8], lim: Limits, time: Duration) -> Result<Self, Fault> {
        if data.len() > lim.max_input_bytes {
            return Err(Fault::Limit(
                "input_bytes",
                "the input exceeds the size limit".into(),
            ));
        }
        Ok(PdfDoc {
            doc: Doc::open(data, lim, Deadline::after(time))?,
        })
    }

    pub fn deadline(&self) -> Deadline {
        self.doc.dl
    }

    pub fn page_count(&mut self) -> Result<u32, Fault> {
        Ok(page::select(&mut self.doc, 0)?.count)
    }

    /// The lines and images of page `n` (1-based), scaled by `px_per_pt` pixels per PDF point.
    pub fn scene(&mut self, n: u32, px_per_pt: f64) -> Result<Scene, Fault> {
        let sel = page::select(&mut self.doc, n)?;
        let Some(info) = sel.page else {
            return Err(Fault::Refused(
                "page_out_of_range",
                format!(
                    "page {n} was requested; the document has {} page(s)",
                    sel.count
                ),
            ));
        };
        let mut stats = Stats::default();
        let geo = page::geometry(&mut self.doc, &info, &mut stats)?;
        let (ctm, (w, h)) = page::transform(&geo, px_per_pt);
        let res = page::resources(&mut self.doc, &info)?;
        let content = page::contents(&mut self.doc, &info)?;
        let opts = RunOpts {
            tol_mm: 0.25,
            fill: true,
            text: false,
            capture: true,
        };
        let mut it = Interp::new(&mut self.doc, opts, 0, ctm, res);
        it.stats = std::mem::take(&mut stats);
        let outcome = it.run(&content);
        let items = std::mem::take(&mut it.items);
        let parts = it.finish();
        outcome?;
        let k = geo.user_unit;
        let (bw, bh) = (
            geo.visible[2] - geo.visible[0],
            geo.visible[3] - geo.visible[1],
        );
        let (wpt, hpt) = if geo.rotate == 90 || geo.rotate == 270 {
            (bh * k, bw * k)
        } else {
            (bw * k, bh * k)
        };
        Ok(Scene {
            page_count: sel.count,
            width: w,
            height: h,
            width_pt: wpt,
            height_pt: hpt,
            rotate: geo.rotate,
            items,
            painted: parts.painted,
            stats: parts.stats,
        })
    }

    /// Reads an image's dictionary and removes its plain filters (Flate, ASCII, RunLength).
    /// `max_pixels` bounds width times height.
    pub fn load_image(&mut self, src: &ImageSrc, max_pixels: u64) -> Result<RawImage, Fault> {
        let (dict, raw): (Dict, Vec<u8>) = match src {
            ImageSrc::Xobject(s, _) => {
                let end = s
                    .start
                    .checked_add(s.len)
                    .ok_or_else(|| Fault::Malformed("stream range overflows".into()))?;
                let raw =
                    self.doc.data.get(s.start..end).ok_or_else(|| {
                        Fault::Malformed("stream data lies outside the file".into())
                    })?;
                (s.dict.clone(), raw.to_vec())
            }
            ImageSrc::Inline(d, data) => ((**d).clone(), (**data).clone()),
        };
        let width = rd_u32(&mut self.doc, &dict, &[b"Width"])?.unwrap_or(0);
        let height = rd_u32(&mut self.doc, &dict, &[b"Height"])?.unwrap_or(0);
        if width == 0 || height == 0 {
            return malformed("image without a valid Width and Height");
        }
        if u64::from(width) * u64::from(height) > max_pixels {
            return limit(
                "image_pixels",
                "an image has more pixels than the limit allows",
            );
        }
        let mask = matches!(
            self.doc.dict_entry(&dict, b"ImageMask")?,
            Some(Obj::Bool(true))
        );
        let has_alpha = dict.get(b"SMask").is_some() || dict.get(b"Mask").is_some();
        let decode: Vec<f64> = match self.doc.dict_entry(&dict, b"Decode")? {
            Some(Obj::Arr(v)) => v.iter().filter_map(Obj::num).collect(),
            _ => Vec::new(),
        };
        let stages = self.doc.filter_stages(&dict)?;
        let image_at = stages.iter().position(|(n, _)| {
            matches!(
                n.as_slice(),
                b"DCTDecode" | b"DCT" | b"CCITTFaxDecode" | b"CCF" | b"JBIG2Decode" | b"JPXDecode"
            )
        });
        let split = image_at.unwrap_or(stages.len());
        let plain = stages.get(..split).unwrap_or(&[]);
        let data = if plain.is_empty() {
            if self.doc.decoded.saturating_add(raw.len()) > self.doc.lim.max_decoded_bytes {
                return limit(
                    "decoded_bytes",
                    "decoded stream data exceeds the total budget",
                );
            }
            self.doc.decoded += raw.len();
            raw
        } else {
            crate::filters::decode_chain(
                &raw,
                plain,
                &self.doc.lim,
                &mut self.doc.decoded,
                &self.doc.dl,
            )?
        };
        let mut filter = ImageFilter::Raw;
        if let Some(i) = image_at {
            if stages.len() > i + 1 {
                return Err(Fault::Unsupported(
                    "a filter that follows an image filter".into(),
                ));
            }
            let Some((name, parms)) = stages.get(i) else {
                return malformed("filter chain");
            };
            filter = match name.as_slice() {
                b"DCTDecode" | b"DCT" => ImageFilter::Dct,
                b"JPXDecode" => ImageFilter::Jpx,
                b"JBIG2Decode" => {
                    let globals = match parms.get(b"JBIG2Globals") {
                        Some(Obj::Ref(n, _)) => {
                            let n = *n;
                            match &*self.doc.get(n)? {
                                Obj::Stream(gs) => Some(self.doc.stream_data(gs, Some(n))?),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    ImageFilter::Jbig2 { globals }
                }
                _ => {
                    let k = match self.doc.dict_entry(parms, b"K")? {
                        Some(o) => o.num().unwrap_or(0.0) as i64,
                        None => 0,
                    };
                    let flag = |doc: &mut Doc, key: &[u8], default: bool| -> Result<bool, Fault> {
                        Ok(match doc.dict_entry(parms, key)? {
                            Some(Obj::Bool(b)) => b,
                            _ => default,
                        })
                    };
                    ImageFilter::Ccitt(Ccitt {
                        k,
                        columns: rd_u32(&mut self.doc, parms, &[b"Columns"])?.unwrap_or(1728),
                        rows: rd_u32(&mut self.doc, parms, &[b"Rows"])?.unwrap_or(0),
                        black_is_1: flag(&mut self.doc, b"BlackIs1", false)?,
                        byte_align: flag(&mut self.doc, b"EncodedByteAlign", false)?,
                        end_of_line: flag(&mut self.doc, b"EndOfLine", false)?,
                        end_of_block: flag(&mut self.doc, b"EndOfBlock", true)?,
                    })
                }
            };
        }
        let bpc = if mask {
            1
        } else {
            rd_u32(&mut self.doc, &dict, &[b"BitsPerComponent"])?.unwrap_or(8)
        };
        let color = if mask {
            Color::Gray
        } else {
            match self.doc.dict_entry(&dict, b"ColorSpace")? {
                Some(o) => self.color_space(&o, 0)?,
                None if matches!(filter, ImageFilter::Ccitt(_) | ImageFilter::Jbig2 { .. }) => {
                    Color::Gray
                }
                None if filter == ImageFilter::Dct => Color::Gray,
                None => return malformed("image without a ColorSpace"),
            }
        };
        Ok(RawImage {
            width,
            height,
            bpc,
            color,
            decode,
            mask,
            filter,
            data,
            has_alpha,
        })
    }

    fn color_space(&mut self, o: &Obj, depth: usize) -> Result<Color, Fault> {
        if depth > 4 {
            return malformed("colour spaces are nested too deeply");
        }
        let unsupported = |n: &[u8]| {
            Fault::Unsupported(format!(
                "colour space /{}",
                String::from_utf8_lossy(n.get(..n.len().min(32)).unwrap_or(n))
            ))
        };
        match self.doc.resolve(o)? {
            Obj::Name(n) => match n.as_slice() {
                b"DeviceGray" | b"G" | b"CalGray" => Ok(Color::Gray),
                b"DeviceRGB" | b"RGB" | b"CalRGB" => Ok(Color::Rgb),
                b"DeviceCMYK" | b"CMYK" => Ok(Color::Cmyk),
                other => Err(unsupported(other)),
            },
            Obj::Arr(items) => {
                let head = match items.first() {
                    Some(h) => self.doc.resolve(h)?,
                    None => return malformed("empty colour space array"),
                };
                let Some(name) = head.name().map(<[u8]>::to_vec) else {
                    return malformed("colour space array without a name");
                };
                match name.as_slice() {
                    b"DeviceGray" | b"G" | b"CalGray" => Ok(Color::Gray),
                    b"DeviceRGB" | b"RGB" | b"CalRGB" => Ok(Color::Rgb),
                    b"DeviceCMYK" | b"CMYK" => Ok(Color::Cmyk),
                    b"ICCBased" => {
                        let Some(stream) = items.get(1) else {
                            return malformed("ICCBased without a profile");
                        };
                        let obj = self.doc.resolve(stream)?;
                        let n = match obj.dict() {
                            Some(d) => rd_u32(&mut self.doc, d, &[b"N"])?.unwrap_or(0),
                            None => 0,
                        };
                        match n {
                            1 => Ok(Color::Gray),
                            3 => Ok(Color::Rgb),
                            4 => Ok(Color::Cmyk),
                            _ => malformed("ICCBased /N is not 1, 3 or 4"),
                        }
                    }
                    b"Indexed" | b"I" => {
                        let (Some(base), Some(hival), Some(lookup)) =
                            (items.get(1), items.get(2), items.get(3))
                        else {
                            return malformed("Indexed colour space needs four entries");
                        };
                        let base = self.color_space(base, depth + 1)?;
                        if matches!(base, Color::Indexed { .. }) {
                            return malformed("Indexed colour space with an Indexed base");
                        }
                        let hival = self.doc.resolve(hival)?.num().and_then(uint).unwrap_or(0);
                        if hival > 255 {
                            return malformed("Indexed hival is above 255");
                        }
                        let table = match self.doc.resolve(lookup)? {
                            Obj::Str(s) => s,
                            Obj::Stream(s) => self.doc.stream_data(&s, None)?.to_vec(),
                            _ => return malformed("Indexed lookup is not a string or stream"),
                        };
                        Ok(Color::Indexed {
                            base: Box::new(base),
                            hival,
                            table,
                        })
                    }
                    b"Separation" => Ok(Color::Tint),
                    b"DeviceN" => {
                        let n = match items.get(1) {
                            Some(names) => match self.doc.resolve(names)? {
                                Obj::Arr(a) => a.len(),
                                _ => 0,
                            },
                            None => 0,
                        };
                        if n == 1 {
                            Ok(Color::Tint)
                        } else {
                            Err(unsupported(b"DeviceN"))
                        }
                    }
                    other => Err(unsupported(other)),
                }
            }
            _ => malformed("colour space is not a name or an array"),
        }
    }
}
