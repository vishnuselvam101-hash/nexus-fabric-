//! Test fixtures: PDF bytes written by the test code itself, and helpers that
//! convert them and re-read the DXF with the nf-dxf reader.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use nf_pdf::{convert, Converted, Failure, Options, Scale};

pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

/// Raw PDF objects. Object `n` is element `n - 1`; `None` marks a member of an object stream.
#[derive(Default)]
pub struct Pdf {
    objs: Vec<Option<Vec<u8>>>,
    compressed: BTreeMap<u32, (u32, u32)>,
    /// Extra entries for the trailer dictionary (for example /Encrypt).
    pub trailer_extra: String,
}

impl Pdf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reserve(&mut self) -> u32 {
        self.objs.push(None);
        self.objs.len() as u32
    }

    pub fn set(&mut self, num: u32, body: impl AsRef<[u8]>) {
        self.objs[(num - 1) as usize] = Some(body.as_ref().to_vec());
    }

    pub fn add(&mut self, body: impl AsRef<[u8]>) -> u32 {
        let n = self.reserve();
        self.set(n, body);
        n
    }

    /// A stream object. `dict` holds extra entries; `flate` compresses the data.
    pub fn stream(&mut self, dict: &str, data: &[u8], flate: bool) -> u32 {
        let (payload, filter) = if flate {
            (zlib(data), "/Filter /FlateDecode ")
        } else {
            (data.to_vec(), "")
        };
        let mut body =
            format!("<< {filter}{dict} /Length {} >>\nstream\n", payload.len()).into_bytes();
        body.extend_from_slice(&payload);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    /// Moves the given objects into one Flate-compressed object stream.
    pub fn objstm(&mut self, members: &[u32]) -> u32 {
        let mut header = String::new();
        let mut body = Vec::new();
        for (idx, &m) in members.iter().enumerate() {
            let b = self.objs[(m - 1) as usize].take().expect("member body");
            header.push_str(&format!("{} {} ", m, body.len()));
            body.extend_from_slice(&b);
            body.push(b'\n');
            self.compressed.insert(m, (0, idx as u32));
        }
        let first = header.len();
        let mut data = header.into_bytes();
        data.extend_from_slice(&body);
        let stm = self.stream(
            &format!("/Type /ObjStm /N {} /First {}", members.len(), first),
            &data,
            true,
        );
        for m in members {
            if let Some(e) = self.compressed.get_mut(m) {
                e.0 = stm;
            }
        }
        stm
    }

    /// File with a classic cross-reference table (no object streams allowed).
    pub fn classic(&self, root: u32) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in self.objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body.as_ref().expect("classic files hold plain objects"));
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", self.objs.len() + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {root} 0 R {} >>\nstartxref\n{xref}\n%%EOF\n",
                self.objs.len() + 1,
                self.trailer_extra
            )
            .as_bytes(),
        );
        out
    }

    /// File with a cross-reference stream; members of object streams are type-2 entries.
    pub fn with_xref_stream(&self, root: u32) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets: BTreeMap<u32, u32> = BTreeMap::new();
        for (i, body) in self.objs.iter().enumerate() {
            let num = i as u32 + 1;
            if let Some(b) = body {
                offsets.insert(num, out.len() as u32);
                out.extend_from_slice(format!("{num} 0 obj\n").as_bytes());
                out.extend_from_slice(b);
                out.extend_from_slice(b"\nendobj\n");
            }
        }
        let xref_num = self.objs.len() as u32 + 1;
        let size = xref_num + 1;
        let xref_off = out.len() as u32;
        let mut rows = Vec::new();
        for num in 0..size {
            if let Some(off) = offsets.get(&num) {
                rows.push(1u8);
                rows.extend_from_slice(&off.to_be_bytes());
                rows.extend_from_slice(&[0, 0]);
            } else if let Some((stm, idx)) = self.compressed.get(&num) {
                rows.push(2u8);
                rows.extend_from_slice(&stm.to_be_bytes());
                rows.extend_from_slice(&(*idx as u16).to_be_bytes());
            } else if num == xref_num {
                rows.push(1u8);
                rows.extend_from_slice(&xref_off.to_be_bytes());
                rows.extend_from_slice(&[0, 0]);
            } else {
                rows.extend_from_slice(&[0; 7]);
            }
        }
        let mut obj = format!(
            "{xref_num} 0 obj\n<< /Type /XRef /Size {size} /W [1 4 2] /Root {root} 0 R {} /Length {} >>\nstream\n",
            self.trailer_extra,
            rows.len()
        )
        .into_bytes();
        obj.extend_from_slice(&rows);
        obj.extend_from_slice(b"\nendstream\nendobj\n");
        out.extend_from_slice(&obj);
        out.extend_from_slice(format!("startxref\n{xref_off}\n%%EOF\n").as_bytes());
        out
    }
}

/// One page with a content stream and the usual catalogue and page tree.
pub struct PageSpec<'a> {
    pub content: &'a [u8],
    pub media: [f64; 4],
    pub crop: Option<[f64; 4]>,
    pub rotate: u32,
    /// Entries of the /Resources dictionary, written as they are.
    pub resources: String,
    /// Extra page entries, written as they are.
    pub extra: String,
    pub flate: bool,
    pub user_unit: Option<f64>,
}

impl<'a> PageSpec<'a> {
    pub fn new(content: &'a [u8]) -> Self {
        PageSpec {
            content,
            media: [0.0, 0.0, 200.0, 200.0],
            crop: None,
            rotate: 0,
            resources: String::new(),
            extra: String::new(),
            flate: false,
            user_unit: None,
        }
    }
}

fn fmt_box(b: [f64; 4]) -> String {
    format!("{} {} {} {}", b[0], b[1], b[2], b[3])
}

/// The PDF file for one page.
pub fn page_pdf(spec: &PageSpec) -> Vec<u8> {
    build_page(spec).classic(1)
}

/// The same page, with a cross-reference stream.
pub fn page_pdf_xref_stream(spec: &PageSpec) -> Vec<u8> {
    build_page(spec).with_xref_stream(1)
}

/// The same page, with the catalogue and page object in an object stream.
pub fn page_pdf_objstm(spec: &PageSpec) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let (catalog, pages, page, content) = page_objects(&mut pdf, spec);
    let _ = (pages, content);
    pdf.objstm(&[catalog, pages, page]);
    pdf.with_xref_stream(catalog)
}

fn page_objects(pdf: &mut Pdf, spec: &PageSpec) -> (u32, u32, u32, u32) {
    let catalog = pdf.reserve();
    let pages = pdf.reserve();
    let page = pdf.reserve();
    let content = pdf.stream("", spec.content, spec.flate);
    pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
    pdf.set(
        pages,
        format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>"),
    );
    let mut p = format!(
        "<< /Type /Page /Parent {pages} 0 R /MediaBox [{}] /Contents {content} 0 R /Resources << {} >>",
        fmt_box(spec.media),
        spec.resources
    );
    if let Some(c) = spec.crop {
        p.push_str(&format!(" /CropBox [{}]", fmt_box(c)));
    }
    if spec.rotate != 0 {
        p.push_str(&format!(" /Rotate {}", spec.rotate));
    }
    if let Some(u) = spec.user_unit {
        p.push_str(&format!(" /UserUnit {u}"));
    }
    p.push_str(&format!(" {} >>", spec.extra));
    pdf.set(page, p);
    (catalog, pages, page, content)
}

fn build_page(spec: &PageSpec) -> Pdf {
    let mut pdf = Pdf::new();
    let (catalog, _, _, _) = page_objects(&mut pdf, spec);
    let _ = catalog;
    pdf
}

/// Adds a stream object usable as a Form XObject or image resource.
pub fn form_xobject(
    pdf: &mut Pdf,
    bbox: [f64; 4],
    matrix: [f64; 6],
    content: &[u8],
    resources: &str,
) -> u32 {
    let dict = format!(
        "/Type /XObject /Subtype /Form /BBox [{}] /Matrix [{} {} {} {} {} {}] /Resources << {resources} >>",
        fmt_box(bbox),
        matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
    );
    pdf.stream(&dict, content, false)
}

/// A page whose resource objects are created by `resources`, which returns the body of the
/// /Resources dictionary.
pub fn page_with_resources(
    content: &[u8],
    media: [f64; 4],
    resources: impl FnOnce(&mut Pdf) -> String,
) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog = pdf.reserve();
    let pages = pdf.reserve();
    let page = pdf.reserve();
    let res = resources(&mut pdf);
    let content_num = pdf.stream("", content, false);
    pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
    pdf.set(
        pages,
        format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>"),
    );
    pdf.set(
        page,
        format!(
            "<< /Type /Page /Parent {pages} 0 R /MediaBox [{}] /Contents {content_num} 0 R /Resources << {res} >> >>",
            fmt_box(media)
        ),
    );
    pdf.classic(catalog)
}

pub fn options(mm_per_point: f64) -> Options {
    Options::new(Scale::MmPerPoint(mm_per_point))
}

pub fn convert_ok(bytes: &[u8], mm_per_point: f64) -> Converted {
    convert(bytes, &options(mm_per_point)).expect("conversion should succeed")
}

pub fn convert_err(bytes: &[u8], opts: &Options) -> Box<Failure> {
    convert(bytes, opts).expect_err("conversion should be refused")
}

/// Entities of a DXF file, re-read with the nf-dxf reader.
pub fn reread(dxf: &[u8]) -> Vec<nf_dxf::Entity> {
    let text = std::str::from_utf8(dxf).expect("the DXF is ASCII");
    nf_dxf::read(text).expect("nf-dxf can re-read the output").0
}

/// Polylines on `layer`: points and closed flag.
pub fn polylines(dxf: &[u8], layer: &str) -> Vec<(Vec<(f64, f64)>, bool)> {
    reread(dxf)
        .into_iter()
        .filter_map(|e| match e {
            nf_dxf::Entity::Polyline {
                layer: l,
                pts,
                closed,
            } if l == layer => Some((pts, closed)),
            _ => None,
        })
        .collect()
}

/// TEXT entities on `layer`: insertion point, height and text.
pub fn texts(dxf: &[u8], layer: &str) -> Vec<((f64, f64), f64, String)> {
    reread(dxf)
        .into_iter()
        .filter_map(|e| match e {
            nf_dxf::Entity::Text {
                layer: l,
                at,
                height,
                text,
            } if l == layer => Some((at, height, text)),
            _ => None,
        })
        .collect()
}

pub fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 2e-6
}

pub fn assert_pt(p: (f64, f64), want: (f64, f64)) {
    assert!(
        close(p.0, want.0) && close(p.1, want.1),
        "point {p:?} is not {want:?}"
    );
}

/// A valid one-page file of exactly `total` bytes: an unreferenced stream object pads it.
pub fn page_of_exact_size(total: usize) -> Vec<u8> {
    let build = |pad: usize| {
        let mut pdf = Pdf::new();
        let catalog = pdf.reserve();
        let pages = pdf.reserve();
        let page = pdf.reserve();
        let content = pdf.stream("", b"0 0 m 10 10 l S", false);
        let _padding = pdf.stream("", &vec![b' '; pad], false);
        pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
        pdf.set(
            pages,
            format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>"),
        );
        pdf.set(
            page,
            format!("<< /Type /Page /Parent {pages} 0 R /MediaBox [0 0 200 200] /Contents {content} 0 R >>"),
        );
        pdf.classic(catalog)
    };
    let mut pad = total.saturating_sub(build(0).len());
    for _ in 0..8 {
        let bytes = build(pad);
        if bytes.len() == total {
            return bytes;
        }
        pad = (pad + total).saturating_sub(bytes.len());
    }
    panic!("no padding gives {total} bytes");
}
