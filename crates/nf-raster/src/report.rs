//! Results and their JSON (fixed key order, ASCII only).

use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    /// A page-covering image and almost no vector paths.
    Scanned,
    /// Vector paths only.
    Vector,
    /// Images and vector paths.
    Mixed,
    /// Nothing painted.
    Blank,
    /// The page could not be examined (see `error`).
    Unknown,
}

impl PageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PageKind::Scanned => "scanned",
            PageKind::Vector => "vector",
            PageKind::Mixed => "mixed",
            PageKind::Blank => "blank",
            PageKind::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PageSummary {
    pub page: u32,
    pub width_pt: f64,
    pub height_pt: f64,
    pub rotate: u32,
    pub kind: PageKind,
    pub images: u64,
    pub lines: u64,
    pub error: Option<&'static str>,
}

impl PageSummary {
    pub fn unknown(page: u32, code: &'static str) -> Self {
        PageSummary {
            page,
            width_pt: 0.0,
            height_pt: 0.0,
            rotate: 0,
            kind: PageKind::Unknown,
            images: 0,
            lines: 0,
            error: Some(code),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Info {
    pub sha256: String,
    pub input_bytes: usize,
    pub page_count: u32,
    pub pages: Vec<PageSummary>,
    pub elapsed_ms: u64,
}

#[derive(Debug)]
pub struct Rendered {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: usize,
    pub dpi: u32,
    pub page: u32,
    pub page_count: u32,
    pub width_pt: f64,
    pub height_pt: f64,
    /// Upright page size in pixels before rounding.
    pub expected_width: f64,
    pub expected_height: f64,
    pub rotate: u32,
    pub kind: PageKind,
    pub images: u64,
    pub lines: u64,
    pub filters: Vec<(&'static str, u64)>,
    pub warnings: Vec<(&'static str, u64, &'static str)>,
    pub sha256: String,
    pub input_bytes: usize,
    pub elapsed_ms: u64,
}

pub fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (' '..='~').contains(&c) => o.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(o, "\\u{unit:04x}");
                }
            }
        }
    }
    o.push('"');
    o
}

fn isolation_json(iso: &[(&'static str, String)]) -> String {
    let items: Vec<String> = iso
        .iter()
        .map(|(k, v)| format!("{}: {}", json_str(k), json_str(v)))
        .collect();
    format!("{{{}}}", items.join(", "))
}

impl Info {
    pub fn to_json(&self, iso: &[(&'static str, String)]) -> String {
        let pages: Vec<String> = self
            .pages
            .iter()
            .map(|p| {
                format!(
                    "    {{\"page\": {}, \"kind\": {}, \"width_pt\": {:.3}, \"height_pt\": {:.3}, \"rotate\": {}, \"images\": {}, \"lines\": {}, \"error\": {}}}",
                    p.page,
                    json_str(p.kind.as_str()),
                    p.width_pt,
                    p.height_pt,
                    p.rotate,
                    p.images,
                    p.lines,
                    p.error.map_or("null".to_string(), json_str)
                )
            })
            .collect();
        format!(
            "{{\n  \"tool\": \"nf-raster\",\n  \"version\": {},\n  \"command\": \"info\",\n  \"status\": \"ok\",\n  \"sha256\": {},\n  \"input_bytes\": {},\n  \"page_count\": {},\n  \"elapsed_ms\": {},\n  \"pages\": [\n{}\n  ],\n  \"isolation\": {}\n}}\n",
            json_str(crate::VERSION),
            json_str(&self.sha256),
            self.input_bytes,
            self.page_count,
            self.elapsed_ms,
            pages.join(",\n"),
            isolation_json(iso)
        )
    }
}

impl Rendered {
    pub fn to_json(&self, png_sha256: &str, iso: &[(&'static str, String)]) -> String {
        let filters: Vec<String> = self
            .filters
            .iter()
            .map(|(k, n)| format!("{}: {}", json_str(k), n))
            .collect();
        let warnings: Vec<String> = self
            .warnings
            .iter()
            .map(|(k, n, m)| {
                format!(
                    "{{\"code\": {}, \"count\": {}, \"message\": {}}}",
                    json_str(k),
                    n,
                    json_str(m)
                )
            })
            .collect();
        format!(
            "{{\n  \"tool\": \"nf-raster\",\n  \"version\": {},\n  \"command\": \"page\",\n  \"status\": \"rendered\",\n  \"sha256\": {},\n  \"input_bytes\": {},\n  \"page\": {},\n  \"page_count\": {},\n  \"kind\": {},\n  \"dpi\": {},\n  \"width_px\": {},\n  \"height_px\": {},\n  \"expected_width_px\": {:.3},\n  \"expected_height_px\": {:.3},\n  \"channels\": {},\n  \"page_width_pt\": {:.3},\n  \"page_height_pt\": {:.3},\n  \"rotate\": {},\n  \"images\": {},\n  \"lines\": {},\n  \"image_filters\": {{{}}},\n  \"warnings\": [{}],\n  \"elapsed_ms\": {},\n  \"png_bytes\": {},\n  \"png_sha256\": {},\n  \"isolation\": {}\n}}\n",
            json_str(crate::VERSION),
            json_str(&self.sha256),
            self.input_bytes,
            self.page,
            self.page_count,
            json_str(self.kind.as_str()),
            self.dpi,
            self.width,
            self.height,
            self.expected_width,
            self.expected_height,
            self.channels,
            self.width_pt,
            self.height_pt,
            self.rotate,
            self.images,
            self.lines,
            filters.join(", "),
            warnings.join(", "),
            self.elapsed_ms,
            self.png.len(),
            json_str(png_sha256),
            isolation_json(iso)
        )
    }
}
