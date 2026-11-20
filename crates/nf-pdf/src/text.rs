//! Text operators: positioning, glyph advances, and TEXT output for strings whose
//! encoding decodes reliably. Text is never converted to outline geometry, and
//! dimension text is never read as a number or a scale.

use std::rc::Rc;

use crate::dxf;
use crate::font::Font;
use crate::geom::{IDENTITY, M};
use crate::interp::{nums, Interp, TextObj, W_MALFORMED};
use crate::lexer::{Dict, Obj};
use crate::limits::{limit, Fault};

impl<'d, 'a> Interp<'d, 'a> {
    /// Handles one text operator. Returns false when `name` is not a text operator.
    pub fn text_op(&mut self, name: &[u8], args: &[Obj]) -> Result<bool, Fault> {
        match name {
            b"BT" => {
                self.txt = TextObj {
                    tm: IDENTITY,
                    tlm: IDENTITY,
                    known: true,
                };
            }
            b"ET" => {}
            b"Tc" => self.set_param(nums::<1>(args), |g, v| g.tc = v),
            b"Tw" => self.set_param(nums::<1>(args), |g, v| g.tw = v),
            b"Tz" => self.set_param(nums::<1>(args), |g, v| g.th = v / 100.0),
            b"TL" => self.set_param(nums::<1>(args), |g, v| g.tl = v),
            b"Ts" => self.set_param(nums::<1>(args), |g, v| g.rise = v),
            b"Tr" => match nums::<1>(args) {
                Some([v]) if v.fract() == 0.0 && v.abs() < 1e6 => self.gs.mode = v as i64,
                _ => self.bad_operands(),
            },
            b"Tf" => self.set_font(args)?,
            b"Td" => match nums::<2>(args) {
                Some([x, y]) => self.next_line(x, y),
                None => self.bad_operands(),
            },
            b"TD" => match nums::<2>(args) {
                Some([x, y]) => {
                    self.gs.tl = -y;
                    self.next_line(x, y);
                }
                None => self.bad_operands(),
            },
            b"Tm" => match nums::<6>(args).and_then(|m| M::from_slice(&m)) {
                Some(m) => {
                    self.txt.tm = m;
                    self.txt.tlm = m;
                    self.txt.known = true;
                }
                None => self.bad_operands(),
            },
            b"T*" => {
                let tl = self.gs.tl;
                self.next_line(0.0, -tl);
            }
            b"Tj" => match args.last() {
                Some(Obj::Str(s)) => self.show(s)?,
                _ => self.bad_operands(),
            },
            b"TJ" => match args.last() {
                Some(Obj::Arr(items)) => {
                    for item in items {
                        match item {
                            Obj::Str(s) => self.show(s)?,
                            Obj::Num(n) => self.adjust(*n),
                            _ => {}
                        }
                    }
                }
                _ => self.bad_operands(),
            },
            b"'" => {
                let tl = self.gs.tl;
                self.next_line(0.0, -tl);
                match args.last() {
                    Some(Obj::Str(s)) => self.show(s)?,
                    _ => self.bad_operands(),
                }
            }
            b"\"" => match args {
                [aw, ac, Obj::Str(s)] => match (aw.num(), ac.num()) {
                    (Some(aw), Some(ac)) => {
                        self.gs.tw = aw;
                        self.gs.tc = ac;
                        let tl = self.gs.tl;
                        self.next_line(0.0, -tl);
                        self.show(s)?;
                    }
                    _ => self.bad_operands(),
                },
                _ => self.bad_operands(),
            },
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn bad_operands(&mut self) {
        self.stats.warn(W_MALFORMED.0, W_MALFORMED.1);
    }

    fn set_param(
        &mut self,
        v: Option<[f64; 1]>,
        set: impl FnOnce(&mut crate::interp::GState, f64),
    ) {
        match v {
            Some([x]) if x.is_finite() => set(&mut self.gs, x),
            _ => self.bad_operands(),
        }
    }

    /// T*, Td and the line-starting operators: the line matrix moves, and so does the text matrix.
    fn next_line(&mut self, x: f64, y: f64) {
        self.txt.tlm = M::translate(x, y).then(self.txt.tlm);
        self.txt.tm = self.txt.tlm;
        self.txt.known = true;
    }

    /// Tf: the font resource and size.
    fn set_font(&mut self, args: &[Obj]) -> Result<(), Fault> {
        let (Some(name), Some(size)) = (
            args.first().and_then(Obj::name),
            args.get(1).and_then(Obj::num),
        ) else {
            self.bad_operands();
            return Ok(());
        };
        let name = name.to_vec();
        self.gs.font = self.load_font(&name)?;
        self.gs.size = size;
        Ok(())
    }

    fn load_font(&mut self, name: &[u8]) -> Result<Option<Rc<Font>>, Fault> {
        let Some(cat) = self.doc.dict_entry(&self.res, b"Font")? else {
            self.stats
                .warn("font_missing", "a font resource was not found");
            return Ok(None);
        };
        let Some(cat) = cat.dict().cloned() else {
            return Ok(None);
        };
        let (num, obj) = match cat.get(name) {
            None => {
                self.stats
                    .warn("font_missing", "a font resource was not found");
                return Ok(None);
            }
            Some(Obj::Ref(n, _)) => {
                let n = *n;
                (Some(n), (*self.doc.get(n)?).clone())
            }
            Some(other) => (None, other.clone()),
        };
        if let Some(n) = num {
            if let Some(f) = self.fonts.get(&n) {
                return Ok(Some(f.clone()));
            }
        }
        let font = match obj.dict() {
            Some(d) => {
                let d: Dict = d.clone();
                match Font::load(self.doc, &d) {
                    Ok(f) => Rc::new(f),
                    Err(Fault::Malformed(_)) | Err(Fault::Unsupported(_)) => {
                        Rc::new(Font::unusable("malformed_font"))
                    }
                    Err(other) => return Err(other),
                }
            }
            None => Rc::new(Font::unusable("malformed_font")),
        };
        if let Some(n) = num {
            self.fonts.insert(n, font.clone());
        }
        Ok(Some(font))
    }

    /// Text rendering matrix: font size and horizontal scaling, rise, text matrix, CTM.
    fn text_matrix(&self) -> M {
        let g = &self.gs;
        M {
            a: g.size * g.th,
            b: 0.0,
            c: 0.0,
            d: g.size,
            e: 0.0,
            f: g.rise,
        }
        .then(self.txt.tm)
        .then(g.ctm)
    }

    /// Shows one string: emits a TEXT entity when it can be placed and decoded, then advances.
    fn show(&mut self, s: &[u8]) -> Result<(), Fault> {
        self.stats.count("text_items", 1);
        if self.stats.get("text_items") > self.doc.lim.max_text_items {
            return limit("text_items", "more text items than the limit allows");
        }
        if s.is_empty() {
            return Ok(());
        }
        let font = self.gs.font.clone();
        match font.as_deref() {
            None => self.stats.skip_text("no_font"),
            Some(f) => {
                let reason = if let Some(r) = f.skip_reason() {
                    Some(r)
                } else if !self.opt.text {
                    Some("text_not_requested")
                } else if self.gs.mode == 3 || self.gs.mode == 7 {
                    Some("invisible_text")
                } else if !self.txt.known {
                    Some("position_unknown")
                } else {
                    None
                };
                match reason {
                    Some(r) => self.stats.skip_text(r),
                    None => match f.decode(s) {
                        None => self.stats.skip_text("undecodable_encoding"),
                        Some(text) => {
                            let trm = self.text_matrix();
                            self.emit(&text, trm)?;
                        }
                    },
                }
            }
        }
        self.advance(s, font.as_deref());
        Ok(())
    }

    /// Moves the text matrix past a string, using the glyph widths. An unknown width
    /// makes the position of later strings in this text object unknown.
    fn advance(&mut self, s: &[u8], font: Option<&Font>) {
        let Some(f) = font else {
            self.txt.known = false;
            return;
        };
        if f.skip_reason().is_some() {
            self.txt.known = false;
            return;
        }
        for (code, len) in f.codes(s) {
            let Some(w) = f.advance(code) else {
                self.txt.known = false;
                return;
            };
            let mut tx = w / 1000.0 * self.gs.size + self.gs.tc;
            if len == 1 && code == 32 {
                tx += self.gs.tw;
            }
            tx *= self.gs.th;
            self.txt.tm = M::translate(tx, 0.0).then(self.txt.tm);
        }
    }

    /// A TJ number: a move in the opposite direction, in thousandths of text size.
    fn adjust(&mut self, n: f64) {
        if self.txt.known && n.is_finite() {
            let tx = -n / 1000.0 * self.gs.size * self.gs.th;
            self.txt.tm = M::translate(tx, 0.0).then(self.txt.tm);
        }
    }

    fn emit(&mut self, text: &str, trm: M) -> Result<(), Fault> {
        let height = trm.c.hypot(trm.d);
        if !height.is_finite() || height <= 1e-9 {
            self.stats.skip_text("degenerate_size");
            return Ok(());
        }
        let det = trm.a * trm.d - trm.b * trm.c;
        let sign = if det < 0.0 { -1.0 } else { 1.0 };
        let width = sign * trm.a.hypot(trm.b) / height;
        let rotation = trm.b.atan2(trm.a).to_degrees();
        let mut clean = String::with_capacity(text.len());
        let mut replaced = 0u64;
        for ch in text.chars() {
            if (' '..='~').contains(&ch) && ch != '%' && ch != '^' {
                clean.push(ch);
            } else {
                clean.push('?');
                replaced += 1;
            }
        }
        if replaced > 0 {
            self.stats.count("text_chars_replaced", replaced);
            self.stats.warn(
                "text_chars_replaced",
                "characters that the DXF TEXT form does not carry were written as '?'",
            );
        }
        if clean.trim().is_empty() {
            return Ok(());
        }
        self.out.text(
            dxf::LAYER_TEXT,
            (trm.e, trm.f),
            height,
            rotation,
            width,
            &clean,
        )?;
        self.stats.count("text_written", 1);
        Ok(())
    }
}
