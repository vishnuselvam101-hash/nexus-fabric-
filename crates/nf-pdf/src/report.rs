//! Counters, warnings and the JSON report. Output is deterministic: fixed key
//! order, sorted counters, and ASCII-only strings.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Counters and warnings gathered while converting.
#[derive(Default, Debug)]
pub struct Stats {
    pub counts: BTreeMap<&'static str, u64>,
    /// code -> (occurrences, message)
    pub warnings: BTreeMap<&'static str, (u64, &'static str)>,
    /// reason -> number of text items not emitted
    pub text_skipped: BTreeMap<&'static str, u64>,
}

impl Stats {
    pub fn count(&mut self, key: &'static str, n: u64) {
        let e = self.counts.entry(key).or_insert(0);
        *e = e.saturating_add(n);
    }

    pub fn warn(&mut self, code: &'static str, message: &'static str) {
        let e = self.warnings.entry(code).or_insert((0, message));
        e.0 = e.0.saturating_add(1);
    }

    pub fn skip_text(&mut self, reason: &'static str) {
        let e = self.text_skipped.entry(reason).or_insert(0);
        *e = e.saturating_add(1);
        self.count("text_items_skipped", 1);
    }

    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }
}

/// Everything the JSON report states about one run.
#[derive(Debug)]
pub struct Report {
    pub status: &'static str,
    pub code: Option<&'static str>,
    pub limit: Option<&'static str>,
    pub message: Option<String>,
    pub sha256: String,
    pub input_bytes: usize,
    pub pdf_version: Option<String>,
    pub page: u32,
    pub page_count: Option<u32>,
    pub mm_per_point: f64,
    pub scale_kind: &'static str,
    pub ratio: Option<f64>,
    pub user_unit: f64,
    pub rotate: u32,
    /// Visible box in user-space units (llx, lly, urx, ury).
    pub visible_box_user: Option<[f64; 4]>,
    pub page_size_mm: Option<(f64, f64)>,
    pub tolerance_mm: f64,
    pub include_fill: bool,
    pub include_text: bool,
    pub stats: Stats,
    pub bbox_mm: Option<[f64; 4]>,
    pub dxf_bytes: usize,
    pub dxf_sha256: Option<String>,
}

/// Facts the report states but the converter does not compute itself.
pub const NOTES: [&str; 6] = [
    "the DXF reproduces what is drawn in the PDF, in millimetres at the scale given",
    "a drawing is not necessarily to scale; the scale is the one supplied",
    "dimension text is not interpreted as geometry or as a scale",
    "line widths, colours, line caps and joins, fill rules and clipping are not represented",
    "the result is not a flat pattern and has not been checked against any manufacturing requirement",
    "optional content, annotations, shadings and patterns are not converted (see warnings)",
];

pub fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (' '..='~').contains(&c) => o.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(o, "\\u{:04x}", unit);
                }
            }
        }
    }
    o.push('"');
    o
}

fn jnum(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.6}")
    } else {
        "null".to_string()
    }
}

fn jopt_str(v: &Option<String>) -> String {
    match v {
        Some(s) => json_str(s),
        None => "null".to_string(),
    }
}

fn jbox(b: Option<[f64; 4]>) -> String {
    match b {
        Some([a, b1, c, d]) => format!("[{}, {}, {}, {}]", jnum(a), jnum(b1), jnum(c), jnum(d)),
        None => "null".to_string(),
    }
}

impl Report {
    pub fn to_json(&self) -> String {
        let s = &self.stats;
        let mut o = String::new();
        o.push_str("{\n");
        let _ = writeln!(o, "  \"tool\": \"nf-pdf2dxf\",");
        let _ = writeln!(o, "  \"converter_version\": {},", json_str(crate::VERSION));
        let _ = writeln!(o, "  \"status\": {},", json_str(self.status));
        let _ = writeln!(
            o,
            "  \"refusal\": {{\"code\": {}, \"limit\": {}, \"message\": {}}},",
            self.code.map_or("null".to_string(), json_str),
            self.limit.map_or("null".to_string(), json_str),
            jopt_str(&self.message)
        );
        let _ = writeln!(o, "  \"source\": {{\"sha256\": {}, \"bytes\": {}, \"pdf_version\": {}, \"page\": {}, \"page_count\": {}}},",
            json_str(&self.sha256),
            self.input_bytes,
            jopt_str(&self.pdf_version),
            self.page,
            self.page_count.map_or("null".to_string(), |n| n.to_string()));
        let _ = writeln!(o, "  \"scale\": {{\"kind\": {}, \"mm_per_point\": {}, \"ratio\": {}, \"user_unit\": {}, \"mm_per_user_unit\": {}}},",
            json_str(self.scale_kind),
            jnum(self.mm_per_point),
            self.ratio.map_or("null".to_string(), jnum),
            jnum(self.user_unit),
            jnum(self.mm_per_point * self.user_unit));
        let _ = writeln!(
            o,
            "  \"page\": {{\"rotate\": {}, \"visible_box_user\": {}, \"size_mm\": {}}},",
            self.rotate,
            jbox(self.visible_box_user),
            match self.page_size_mm {
                Some((w, h)) => format!("[{}, {}]", jnum(w), jnum(h)),
                None => "null".to_string(),
            }
        );
        let _ = writeln!(o, "  \"tolerance\": {{\"chord_mm\": {}, \"bound\": \"3m/(4n^2) <= chord_mm per cubic Bezier\"}},", jnum(self.tolerance_mm));
        let _ = writeln!(o, "  \"layers\": [\"PDF_STROKE\", \"PDF_DASHED\", \"PDF_FILL\", \"PDF_TEXT\", \"NF_NOT_RELEASED\"],");
        let _ = writeln!(
            o,
            "  \"options\": {{\"include_fill\": {}, \"include_text\": {}}},",
            self.include_fill, self.include_text
        );
        let _ = writeln!(o, "  \"counts\": {{");
        let counts: Vec<String> = s
            .counts
            .iter()
            .map(|(k, v)| format!("    {}: {}", json_str(k), v))
            .collect();
        o.push_str(&counts.join(",\n"));
        if !counts.is_empty() {
            o.push('\n');
        }
        let _ = writeln!(o, "  }},");
        let skipped: Vec<String> = s
            .text_skipped
            .iter()
            .map(|(k, v)| format!("{}: {}", json_str(k), v))
            .collect();
        let _ = writeln!(
            o,
            "  \"text_skipped_by_reason\": {{{}}},",
            skipped.join(", ")
        );
        let warnings: Vec<String> = s
            .warnings
            .iter()
            .map(|(code, (n, msg))| {
                format!(
                    "    {{\"code\": {}, \"count\": {}, \"message\": {}}}",
                    json_str(code),
                    n,
                    json_str(msg)
                )
            })
            .collect();
        let _ = writeln!(o, "  \"warnings\": [");
        o.push_str(&warnings.join(",\n"));
        if !warnings.is_empty() {
            o.push('\n');
        }
        let _ = writeln!(o, "  ],");
        let _ = writeln!(o, "  \"bbox_mm\": {},", jbox(self.bbox_mm));
        let _ = writeln!(
            o,
            "  \"output\": {{\"dxf_bytes\": {}, \"dxf_sha256\": {}}},",
            self.dxf_bytes,
            jopt_str(&self.dxf_sha256)
        );
        let notes: Vec<String> = NOTES.iter().map(|n| json_str(n)).collect();
        let _ = writeln!(o, "  \"notes\": [{}]", notes.join(", "));
        o.push_str("}\n");
        o
    }
}
