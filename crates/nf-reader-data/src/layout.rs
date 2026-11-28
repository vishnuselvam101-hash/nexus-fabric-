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
/// Does the segment (x0,y0)-(x1,y1) touch the box `b` grown by `pad`? (Liang-Barsky clip.)
pub fn seg_hits(s: B, b: B, pad: f64) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (s[2] - s[0], s[3] - s[1]);
    for (p, q) in [
        (-dx, s[0] - (b[0] - pad)),
        (dx, (b[2] + pad) - s[0]),
        (-dy, s[1] - (b[1] - pad)),
        (dy, (b[3] + pad) - s[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return false;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return false;
                }
                t1 = t1.min(r);
            }
        }
    }
    t0 <= t1
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
    /// The text elements alone (same markup as in `body`), for the text-only render the box test uses.
    pub text_body: String,
    pub items: Vec<Item>,
    pub dec: usize,
    pub gtol: char,
    /// Every drawn line segment (x0,y0,x1,y1) in page units: text is never placed across one.
    pub segs: Vec<B>,
    /// While set, a text item must also have a clear leader from its left edge to this point.
    pub leader: Option<(f64, f64)>,
    /// Printed text that is NOT in the answer key (negative examples): the strings, and their ink boxes.
    pub clutter: Vec<String>,
    pub cboxes: Vec<B>,
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
            text_body: String::new(),
            items: vec![],
            dec,
            gtol,
            segs: vec![],
            leader: None,
            clutter: vec![],
            cboxes: vec![],
        }
    }
    pub fn raw(&mut self, s: &str) {
        self.body.push_str(s);
        self.body.push('\n');
    }
    pub fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, sw: f64) {
        self.segs.push([x0, y0, x1, y1]);
        self.raw(&format!(
            "<line x1=\"{x0:.1}\" y1=\"{y0:.1}\" x2=\"{x1:.1}\" y2=\"{y1:.1}\" stroke=\"#000\" stroke-width=\"{sw:.2}\"/>"
        ));
    }
    /// Axis-aligned line drawn in pieces so that it never runs through a placed text box.
    pub fn dline(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, sw: f64) {
        let pad = 2.0 * self.u;
        let horiz = (y0 - y1).abs() < 1e-9;
        let vert = (x0 - x1).abs() < 1e-9;
        if !horiz && !vert {
            return self.line(x0, y0, x1, y1, sw);
        }
        let (a, b, c) = if horiz { (x0, x1, y0) } else { (y0, y1, x0) };
        let (lo, hi) = (a.min(b), a.max(b));
        let mut cuts: Vec<(f64, f64)> = vec![];
        for i in &self.items {
            let (bl, bh, cl, ch) = if horiz {
                (i.b[0], i.b[2], i.b[1], i.b[3])
            } else {
                (i.b[1], i.b[3], i.b[0], i.b[2])
            };
            if c > cl - pad && c < ch + pad && bh + pad > lo && bl - pad < hi {
                cuts.push((bl - pad, bh + pad));
            }
        }
        cuts.sort_by(|p, q| p.0.total_cmp(&q.0));
        let mut cur = lo;
        let mut pieces = vec![];
        for (cl, ch) in cuts {
            if cl > cur {
                pieces.push((cur, cl.min(hi)));
            }
            cur = cur.max(ch);
        }
        if cur < hi {
            pieces.push((cur, hi));
        }
        for (pl, ph) in pieces {
            if ph - pl < 1.5 * self.u {
                continue;
            }
            if horiz {
                self.line(pl, c, ph, c, sw);
            } else {
                self.line(c, pl, c, ph, sw);
            }
        }
    }
    pub fn rect(&mut self, b: B, sw: f64) {
        for s in [
            [b[0], b[1], b[2], b[1]],
            [b[2], b[1], b[2], b[3]],
            [b[2], b[3], b[0], b[3]],
            [b[0], b[3], b[0], b[1]],
        ] {
            self.segs.push(s);
        }
        self.raw(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{sw:.2}\"/>",
            b[0], b[1], b[2] - b[0], b[3] - b[1]
        ));
    }
    pub fn poly(&mut self, pts: &[(f64, f64)], closed: bool, sw: f64, fill: &str) {
        for i in 1..pts.len() {
            self.segs
                .push([pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1]);
        }
        if closed && pts.len() > 2 {
            let (a, z) = (pts[pts.len() - 1], pts[0]);
            self.segs.push([a.0, a.1, z.0, z.1]);
        }
        let d: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
        let tag = if closed { "polygon" } else { "polyline" };
        self.raw(&format!(
            "<{tag} points=\"{}\" fill=\"{fill}\" stroke=\"#000\" stroke-width=\"{sw:.2}\" stroke-linejoin=\"round\"/>",
            d.join(" ")
        ));
    }
    /// Ink box of a string: x left of the advance box, baseline y, font size. Trimmed to what the glyphs
    /// actually cover (cap height, descender only when a glyph has one, side bearings), measured on the
    /// rendered DejaVu Sans Mono / Noto Sans CJK JP text.
    pub fn text_box(x: f64, base: f64, s: &str, fs: f64) -> B {
        let cjk = s.chars().any(|c| (c as u32) >= 0x2E80);
        let desc = s.chars().any(|c| "gjpqy,;()[]\u{03C6}".contains(c));
        let top = if cjk {
            0.83
        } else if s.chars().any(|c| "bdfhklt".contains(c)) {
            0.77
        } else {
            0.74
        };
        let bot = if desc {
            0.21
        } else if s.contains('/') {
            0.1
        } else if cjk {
            0.08
        } else {
            0.02
        };
        let first = s.chars().next().unwrap_or(' ');
        let last = s.chars().last().unwrap_or(' ');
        let lb = match first {
            '1' => 0.12,
            '\u{65E5}' => 0.13,
            c if (c as u32) >= 0x2E80 => 0.065,
            _ => 0.05,
        };
        let rb = match last {
            '\u{3002}' | '\u{3001}' => 0.68,
            _ if s.chars().count() == 1 => 0.1,
            '1' => 0.06,
            '.' => 0.22,
            c if (c as u32) >= 0x2E80 => 0.065,
            _ => 0.04,
        };
        [
            x + lb * fs,
            base - top * fs,
            x + text_w(s, fs) - rb * fs,
            base + bot * fs,
        ]
    }
    /// Inside the drawing frame and clear of every placed item.
    pub fn free(&self, b: B) -> bool {
        let m = 28.0 * self.u;
        if b[0] < m || b[1] < m || b[2] > self.w - m || b[3] > self.h - m {
            return false;
        }
        let pad = 3.0 * self.u;
        if self.items.iter().any(|i| inter(i.b, b, pad)) {
            return false;
        }
        if self.cboxes.iter().any(|c| inter(*c, b, pad)) {
            return false;
        }
        let sp = 2.0 * self.u;
        if self.segs.iter().any(|s| seg_hits(*s, b, sp)) {
            return false;
        }
        if let Some(t) = self.leader {
            let ls = [b[0] - 4.0 * self.u, (b[1] + b[3]) / 2.0, t.0, t.1];
            if self.items.iter().any(|i| seg_hits(ls, i.b, sp)) {
                return false;
            }
        }
        true
    }
    /// Print text that stays in the image but is not an answer-key item (title-block fields, revision tables, grid
    /// references, numbered notes, part and drawing numbers). Same placement rules as an item; `false` when it collides.
    pub fn clutter_text(
        &mut self,
        s: &str,
        x: f64,
        base: f64,
        anchor: Anchor,
        fs: f64,
        bold: bool,
    ) -> bool {
        let w = text_w(s, fs);
        let xl = match anchor {
            Anchor::Start => x,
            Anchor::Middle => x - w / 2.0,
            Anchor::End => x - w,
        };
        let b = Page::text_box(xl, base, s, fs);
        if !self.free(b) {
            return false;
        }
        let lang = if self.lang == Lang::Ja {
            " xml:lang=\"ja\""
        } else {
            ""
        };
        let el = format!(
            "<text x=\"{xl:.1}\" y=\"{base:.1}\" font-size=\"{fs:.1}\" font-family=\"{FONT}\"{lang}{} fill=\"#000\">{}</text>",
            if bold { " font-weight=\"bold\"" } else { "" },
            esc(s)
        );
        self.text_body.push_str(&el);
        self.text_body.push('\n');
        self.raw(&el);
        self.clutter.push(s.to_string());
        self.cboxes.push(b);
        true
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
                xl - 2.0,
                base - 0.85 * fs,
                w + 4.0,
                1.05 * fs
            ));
        }
        let lang = if self.lang == Lang::Ja {
            " xml:lang=\"ja\""
        } else {
            ""
        };
        let el = format!(
            "<text x=\"{xl:.1}\" y=\"{base:.1}\" font-size=\"{fs:.1}\" font-family=\"{FONT}\"{lang}{} fill=\"#000\">{}</text>",
            if bold { " font-weight=\"bold\"" } else { "" },
            esc(&it.text)
        );
        self.text_body.push_str(&el);
        self.text_body.push('\n');
        self.raw(&el);
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
