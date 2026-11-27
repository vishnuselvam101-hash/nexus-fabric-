//! Page canvas: text with exact boxes, SVG body, item list. Text uses a monospace Latin face and a full-width CJK
//! face, so the advance of every glyph is known and a box is exact without rendering.

use crate::source::{Source, round_to};
use serde::Serialize;
use serde_json::{Value, json};

pub type B = [f64; 4];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    Iso,
    Jis,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Lang {
    En,
    Ja,
}
impl Style {
    pub fn name(self) -> &'static str {
        if self == Style::Iso { "iso" } else { "jis" }
    }
}
impl Lang {
    pub fn name(self) -> &'static str {
        if self == Lang::En { "en" } else { "ja" }
    }
}
#[derive(Clone, Copy, PartialEq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

pub const LATIN_ADV: f64 = 0.602;
pub const FONT: &str = "DejaVu Sans Mono, Noto Sans CJK JP";

pub fn glyph_adv(c: char) -> f64 {
    if (c as u32) >= 0x2E80 { 1.0 } else { LATIN_ADV }
}
pub fn text_w(s: &str, fs: f64) -> f64 {
    s.chars().map(glyph_adv).sum::<f64>() * fs
}
pub fn inter(a: B, b: B, pad: f64) -> bool {
    a[0] - pad < b[2] && b[0] - pad < a[2] && a[1] - pad < b[3] && b[1] - pad < a[3]
}
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[derive(Clone, Debug, Serialize)]
pub struct Tol {
    pub plus: f64,
    pub minus: f64,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub kind: &'static str,
    pub text: String,
    pub value: Value,
    pub unit: &'static str,
    pub tol: Option<Tol>,
    pub tol_source: Option<Source>,
    pub src: Option<Source>,
    pub part: String,
    pub decimals: Option<usize>,
    pub b: B,
}

pub struct Page {
    pub w: f64,
    pub h: f64,
    pub u: f64,
    pub style: Style,
    pub lang: Lang,
    pub body: String,
    pub items: Vec<Item>,
    pub dec: usize,
    pub gtol: char,
}

impl Page {
    pub fn new(w: f64, h: f64, style: Style, lang: Lang, dec: usize, gtol: char) -> Page {
        Page {
            w,
            h,
            u: w / 2000.0,
            style,
            lang,
            body: String::new(),
            items: vec![],
            dec,
            gtol,
        }
    }
    pub fn raw(&mut self, s: &str) {
        self.body.push_str(s);
        self.body.push('\n');
    }
    pub fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, sw: f64) {
        self.raw(&format!(
            "<line x1=\"{x0:.1}\" y1=\"{y0:.1}\" x2=\"{x1:.1}\" y2=\"{y1:.1}\" stroke=\"#000\" stroke-width=\"{sw:.2}\"/>"
        ));
    }
    pub fn rect(&mut self, b: B, sw: f64) {
        self.raw(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{sw:.2}\"/>",
            b[0], b[1], b[2] - b[0], b[3] - b[1]
        ));
    }
    pub fn poly(&mut self, pts: &[(f64, f64)], closed: bool, sw: f64, fill: &str) {
        let d: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
        let tag = if closed { "polygon" } else { "polyline" };
        self.raw(&format!(
            "<{tag} points=\"{}\" fill=\"{fill}\" stroke=\"#000\" stroke-width=\"{sw:.2}\" stroke-linejoin=\"round\"/>",
            d.join(" ")
        ));
    }
    /// Box of a string: x left, baseline y, font size.
    pub fn text_box(x: f64, base: f64, s: &str, fs: f64) -> B {
        [x, base - 0.85 * fs, x + text_w(s, fs), base + 0.2 * fs]
    }
    /// Inside the drawing frame and clear of every placed item.
    pub fn free(&self, b: B) -> bool {
        let m = 28.0 * self.u;
        if b[0] < m || b[1] < m || b[2] > self.w - m || b[3] > self.h - m {
            return false;
        }
        let pad = 3.0 * self.u;
        !self.items.iter().any(|i| inter(i.b, b, pad))
    }
    /// Place a text item. Returns its box, or `None` (nothing drawn, nothing recorded) when it would collide.
    #[allow(clippy::too_many_arguments)]
    pub fn emit(
        &mut self,
        mut it: Item,
        x: f64,
        base: f64,
        anchor: Anchor,
        fs: f64,
        bold: bool,
        bg: bool,
    ) -> Option<B> {
        let w = text_w(&it.text, fs);
        let xl = match anchor {
            Anchor::Start => x,
            Anchor::Middle => x - w / 2.0,
            Anchor::End => x - w,
        };
        let b = Page::text_box(xl, base, &it.text, fs);
        if !self.free(b) {
            return None;
        }
        if bg {
            self.raw(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#fff\"/>",
                b[0] - 2.0,
                b[1],
                b[2] - b[0] + 4.0,
                b[3] - b[1]
            ));
        }
        let lang = if self.lang == Lang::Ja {
            " xml:lang=\"ja\""
        } else {
            ""
        };
        self.raw(&format!(
            "<text x=\"{xl:.1}\" y=\"{base:.1}\" font-size=\"{fs:.1}\" font-family=\"{FONT}\"{lang}{} fill=\"#000\">{}</text>",
            if bold { " font-weight=\"bold\"" } else { "" },
            esc(&it.text)
        ));
        it.b = b;
        self.items.push(it);
        Some(b)
    }
    /// A printed word or phrase that carries no engine value.
    #[allow(clippy::too_many_arguments)]
    pub fn word(
        &mut self,
        kind: &'static str,
        s: &str,
        part: &str,
        what: &str,
        x: f64,
        base: f64,
        anchor: Anchor,
        fs: f64,
        bold: bool,
    ) -> Option<B> {
        let it = Item {
            kind,
            text: s.to_string(),
            value: json!(s),
            unit: "",
            tol: None,
            tol_source: None,
            src: Some(Source::Structure {
                what: what.to_string(),
            }),
            part: part.to_string(),
            decimals: None,
            b: [0.0; 4],
        };
        self.emit(it, x, base, anchor, fs, bold, false)
    }
    /// A printed number taken from the engine (or a structural integer), with optional prefix and tolerance.
    #[allow(clippy::too_many_arguments)]
    pub fn num(
        &mut self,
        kind: &'static str,
        part: &str,
        prefix: &str,
        v: f64,
        unit: &'static str,
        tol: Option<f64>,
        src: Source,
        x: f64,
        base: f64,
        anchor: Anchor,
        fs: f64,
        bg: bool,
    ) -> Option<B> {
        let dec = self.dec;
        let rv = round_to(v, dec);
        let mut text = format!("{prefix}{rv:.dec$}");
        if unit == "deg" {
            text.push('\u{00B0}');
        }
        let (tol_v, tol_source) = match tol {
            Some(t) => {
                text.push_str(&format!(" \u{00B1}{t}"));
                (
                    Some(Tol { plus: t, minus: -t }),
                    Some(Source::ToleranceTable {
                        class: self.gtol.to_string(),
                    }),
                )
            }
            None => (None, None),
        };
        let it = Item {
            kind,
            text,
            value: json!(rv),
            unit,
            tol: tol_v,
            tol_source,
            src: Some(src),
            part: part.to_string(),
            decimals: Some(dec),
            b: [0.0; 4],
        };
        self.emit(it, x, base, anchor, fs, false, bg)
    }
}
