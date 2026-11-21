//! Content-stream interpreter: graphics state, paths and painting, Form XObjects,
//! images, and operator dispatch. Text operators are in `text.rs`.
//!
//! Coordinates are converted to output millimetres as soon as they are written:
//! the current transformation matrix already contains the page transform, so
//! every path point and every text origin is final when it is stored.

use std::collections::HashMap;
use std::rc::Rc;

use crate::doc::{find, Doc};
use crate::dxf::{self, Body};
use crate::font::Font;
use crate::geom::{self, IDENTITY, M};
use crate::lexer::{is_delim, is_ws, obj_from_tok, Budget, Dict, Lexer, Obj, Stream, Tok};
use crate::limits::{limit, malformed, Fault};
use crate::report::Stats;

/// Options that change what is written.
#[derive(Clone, Copy, Debug)]
pub struct RunOpts {
    /// Chord tolerance for curve flattening, millimetres.
    pub tol_mm: f64,
    pub fill: bool,
    pub text: bool,
}

#[derive(Clone)]
pub struct GState {
    pub ctm: M,
    pub dash: Vec<f64>,
    pub phase: f64,
    pub font: Option<Rc<Font>>,
    pub size: f64,
    pub tc: f64,
    pub tw: f64,
    pub th: f64,
    pub tl: f64,
    pub rise: f64,
    pub mode: i64,
}

impl GState {
    fn new(ctm: M) -> Self {
        GState {
            ctm,
            dash: Vec::new(),
            phase: 0.0,
            font: None,
            size: 0.0,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            rise: 0.0,
            mode: 0,
        }
    }
}

/// Text object state. Not part of the graphics state.
#[derive(Clone, Copy, Debug)]
pub struct TextObj {
    pub tm: M,
    pub tlm: M,
    /// False once a glyph advance is unknown: following origins cannot be placed.
    pub known: bool,
}

#[derive(Clone, Debug)]
pub struct Sub {
    pub pts: Vec<(f64, f64)>,
    pub closed: bool,
}

#[derive(Default)]
struct Path {
    subs: Vec<Sub>,
    cur: Option<(f64, f64)>,
    start: (f64, f64),
}

/// Operands of the last operators, as numbers. Returns `None` on any mismatch.
pub fn nums<const N: usize>(args: &[Obj]) -> Option<[f64; N]> {
    let start = args.len().checked_sub(N)?;
    let tail = args.get(start..)?;
    let mut out = [0.0; N];
    for (slot, o) in out.iter_mut().zip(tail) {
        *slot = o.num()?;
    }
    Some(out)
}

/// Removes consecutive points closer than a nanometre.
fn dedupe(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for &p in pts {
        match out.last() {
            Some(&q) if (p.0 - q.0).hypot(p.1 - q.1) < 1e-6 => {}
            _ => out.push(p),
        }
    }
    out
}

pub const W_MALFORMED: (&str, &str) = (
    "malformed_operator",
    "operators with missing or invalid operands were ignored",
);

/// What a run produced, without the borrow of the document.
pub struct Parts {
    pub out: Body,
    pub stats: Stats,
    pub images: Vec<[f64; 4]>,
    pub painted: u64,
    pub segments: u64,
}

pub struct Interp<'d, 'a> {
    pub doc: &'d mut Doc<'a>,
    pub opt: RunOpts,
    pub out: Body,
    pub stats: Stats,
    pub gs: GState,
    stack: Vec<GState>,
    path: Path,
    pub txt: TextObj,
    pub res: Dict,
    pub fonts: HashMap<u32, Rc<Font>>,
    /// Bounding boxes (millimetres) of image placements.
    pub images: Vec<[f64; 4]>,
    /// Paint operations that had geometry.
    pub painted: u64,
    pub segments: u64,
    pub ops: u64,
    pub form_depth: usize,
}

impl<'d, 'a> Interp<'d, 'a> {
    pub fn new(doc: &'d mut Doc<'a>, opt: RunOpts, out_limit: usize, ctm: M, res: Dict) -> Self {
        Interp {
            doc,
            opt,
            out: Body::new(out_limit),
            stats: Stats::default(),
            gs: GState::new(ctm),
            stack: Vec::new(),
            path: Path::default(),
            txt: TextObj {
                tm: IDENTITY,
                tlm: IDENTITY,
                known: true,
            },
            res,
            fonts: HashMap::new(),
            images: Vec::new(),
            painted: 0,
            segments: 0,
            ops: 0,
            form_depth: 0,
        }
    }

    /// Ends the run and returns what it produced.
    pub fn finish(self) -> Parts {
        Parts {
            out: self.out,
            stats: self.stats,
            images: self.images,
            painted: self.painted,
            segments: self.segments,
        }
    }

    fn bad(&mut self) {
        self.stats.warn(W_MALFORMED.0, W_MALFORMED.1);
    }

    fn add_segments(&mut self, n: usize) -> Result<(), Fault> {
        self.segments = self.segments.saturating_add(n as u64);
        self.stats.count("path_segments", n as u64);
        if self.segments > self.doc.lim.max_path_segments {
            return limit("path_segments", "more path segments than the limit allows");
        }
        Ok(())
    }

    /// Runs one content stream with the current resources.
    pub fn run(&mut self, data: &[u8]) -> Result<(), Fault> {
        let mut lx = Lexer::new(data, 0);
        let mut operands: Vec<Obj> = Vec::new();
        // Items of the operands waiting for one operator; released when the operator runs.
        let mut pending = Budget::new(&self.doc.lim, self.doc.dl);
        let max_operands = self.doc.lim.max_operands;
        loop {
            let t = lx.next()?;
            let operand = match t {
                Tok::Eof => break,
                Tok::Kw(b"true") => Obj::Bool(true),
                Tok::Kw(b"false") => Obj::Bool(false),
                Tok::Kw(b"null") => Obj::Null,
                Tok::Kw(op) => {
                    self.ops += 1;
                    if self.ops > self.doc.lim.max_operators {
                        return limit("operators", "more content operators than the limit allows");
                    }
                    if self.ops.is_multiple_of(1024) {
                        self.doc.dl.check()?;
                    }
                    self.op(op, &operands, &mut lx)?;
                    operands.clear();
                    pending.items = 0;
                    continue;
                }
                other => obj_from_tok(&mut lx, other, 0, &mut pending)?,
            };
            if operands.len() >= max_operands {
                return limit(
                    "operands",
                    "more operands are waiting for an operator than the limit allows",
                );
            }
            operands.push(operand);
        }
        Ok(())
    }

    fn op(&mut self, name: &[u8], args: &[Obj], lx: &mut Lexer) -> Result<(), Fault> {
        match name {
            b"q" => self.save(),
            b"Q" => {
                self.restore();
                Ok(())
            }
            b"cm" => {
                match nums::<6>(args).and_then(|m| M::from_slice(&m)) {
                    Some(m) if m.is_finite() => self.gs.ctm = m.then(self.gs.ctm),
                    _ => self.bad(),
                }
                Ok(())
            }
            b"d" => {
                self.dash_op(args);
                Ok(())
            }
            b"gs" => self.ext_gstate(args),
            b"m" => {
                self.move_to(args);
                Ok(())
            }
            b"l" => self.line_to(args),
            b"c" => self.curve(args, 'c'),
            b"v" => self.curve(args, 'v'),
            b"y" => self.curve(args, 'y'),
            b"h" => {
                if let Some(s) = self.path.subs.last_mut() {
                    s.closed = true;
                    self.path.cur = Some(self.path.start);
                }
                Ok(())
            }
            b"re" => self.rect(args),
            b"S" => self.paint(true, false, false),
            b"s" => self.paint(true, false, true),
            b"f" | b"F" | b"f*" => self.paint(false, true, false),
            b"B" | b"B*" => self.paint(true, true, false),
            b"b" | b"b*" => self.paint(true, true, true),
            b"n" => {
                self.path = Path::default();
                Ok(())
            }
            b"W" | b"W*" => {
                self.stats.count("clip_operators", 1);
                self.stats.warn(
                    "clipping_not_applied",
                    "clipping paths are not applied; geometry outside clip regions is included",
                );
                Ok(())
            }
            b"Do" => self.do_xobject(args),
            b"BI" => self.inline_image(lx),
            b"BDC" => {
                if args.first().and_then(Obj::name).is_some_and(|n| n == b"OC") {
                    self.stats.warn(
                        "optional_content_ignored",
                        "optional content is ignored: all marked content is drawn",
                    );
                }
                Ok(())
            }
            b"sh" => {
                self.stats.count("shadings", 1);
                self.stats
                    .warn("shading_not_converted", "shadings are not converted");
                Ok(())
            }
            b"scn" | b"SCN" => {
                if args.last().and_then(Obj::name).is_some() {
                    self.stats.count("pattern_fills", 1);
                    self.stats.warn(
                        "pattern_fill_outline_only",
                        "pattern-filled paths are converted as outlines; pattern content is not converted",
                    );
                }
                Ok(())
            }
            b"BMC" | b"EMC" | b"DP" | b"MP" | b"BX" | b"EX" | b"sc" | b"SC" | b"g" | b"G"
            | b"rg" | b"RG" | b"k" | b"K" | b"cs" | b"CS" | b"w" | b"J" | b"j" | b"M" | b"i"
            | b"ri" | b"d0" | b"d1" => Ok(()),
            _ => {
                if self.text_op(name, args)? {
                    return Ok(());
                }
                self.stats.count("unknown_operators", 1);
                self.stats.warn(
                    "unknown_operator",
                    "unknown content-stream operators were ignored",
                );
                Ok(())
            }
        }
    }

    fn save(&mut self) -> Result<(), Fault> {
        if self.stack.len() >= self.doc.lim.max_graphics_depth {
            return limit(
                "graphics_depth",
                "q nesting is deeper than the limit allows",
            );
        }
        self.stack.push(self.gs.clone());
        Ok(())
    }

    fn restore(&mut self) {
        match self.stack.pop() {
            Some(g) => self.gs = g,
            None => self
                .stats
                .warn("unbalanced_restore", "Q without a matching q was ignored"),
        }
    }

    fn dash_op(&mut self, args: &[Obj]) {
        let (arr, phase) = match args {
            [Obj::Arr(a), p] => (a.clone(), p.num()),
            _ => {
                self.bad();
                return;
            }
        };
        let values: Option<Vec<f64>> = arr.iter().map(|o| o.num()).collect();
        match (values, phase) {
            (Some(v), Some(p)) if p.is_finite() && v.iter().all(|x| x.is_finite() && *x >= 0.0) => {
                if v.iter().sum::<f64>() > 0.0 {
                    self.gs.dash = v;
                    self.gs.phase = p;
                } else {
                    self.gs.dash.clear();
                    self.gs.phase = 0.0;
                }
            }
            _ => {
                self.gs.dash.clear();
                self.gs.phase = 0.0;
                self.stats.warn(
                    "invalid_dash",
                    "invalid dash patterns were treated as solid",
                );
            }
        }
    }

    fn ext_gstate(&mut self, args: &[Obj]) -> Result<(), Fault> {
        let Some(name) = args.last().and_then(Obj::name) else {
            self.bad();
            return Ok(());
        };
        let Some(gs) = self.resource(b"ExtGState", name)? else {
            self.stats.warn(
                "ext_gstate_missing",
                "a graphics state resource was not found",
            );
            return Ok(());
        };
        let Some(d) = gs.dict().cloned() else {
            return Ok(());
        };
        if let Some(Obj::Arr(pair)) = self.doc.dict_entry(&d, b"D")? {
            if let [arr, phase] = pair.as_slice() {
                let arr = self.doc.resolve(arr)?;
                let phase = self.doc.resolve(phase)?;
                self.dash_op(&[arr, phase]);
            }
        }
        Ok(())
    }

    /// Resolved resource entry `name` in the sub-dictionary `category`.
    fn resource(&mut self, category: &[u8], name: &[u8]) -> Result<Option<Obj>, Fault> {
        let Some(cat) = self.doc.dict_entry(&self.res, category)? else {
            return Ok(None);
        };
        let Some(cat) = cat.dict().cloned() else {
            return Ok(None);
        };
        self.doc.dict_entry(&cat, name)
    }

    fn to_mm(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let p = self.gs.ctm.apply(x, y);
        (p.0.is_finite() && p.1.is_finite()).then_some(p)
    }

    fn move_to(&mut self, args: &[Obj]) {
        let Some([x, y]) = nums::<2>(args) else {
            self.bad();
            return;
        };
        let Some(p) = self.to_mm(x, y) else {
            self.bad();
            return;
        };
        self.path.subs.push(Sub {
            pts: vec![p],
            closed: false,
        });
        self.path.cur = Some(p);
        self.path.start = p;
    }

    /// The subpath that a segment is appended to; a closed subpath is continued from its start.
    fn open_sub(&mut self) -> bool {
        let start = self.path.start;
        let reopen = matches!(self.path.subs.last(), Some(s) if s.closed);
        if reopen {
            self.path.subs.push(Sub {
                pts: vec![start],
                closed: false,
            });
        }
        !self.path.subs.is_empty()
    }

    fn line_to(&mut self, args: &[Obj]) -> Result<(), Fault> {
        let (Some([x, y]), Some(_)) = (nums::<2>(args), self.path.cur) else {
            self.bad();
            return Ok(());
        };
        let Some(p) = self.to_mm(x, y) else {
            self.bad();
            return Ok(());
        };
        if !self.open_sub() {
            self.bad();
            return Ok(());
        }
        if let Some(s) = self.path.subs.last_mut() {
            s.pts.push(p);
        }
        self.path.cur = Some(p);
        self.add_segments(1)
    }

    fn curve(&mut self, args: &[Obj], kind: char) -> Result<(), Fault> {
        let Some(cur) = self.path.cur else {
            self.bad();
            return Ok(());
        };
        // Control points are converted to millimetres as they are read. For 'v' the first
        // control point is the current point; for 'y' the last one is the end point.
        let p0 = cur;
        let (q1, q2, q3) = match (kind, nums::<6>(args), nums::<4>(args)) {
            ('c', Some(v), _) => (
                self.to_mm(v[0], v[1]),
                self.to_mm(v[2], v[3]),
                self.to_mm(v[4], v[5]),
            ),
            ('v', _, Some(v)) => (Some(p0), self.to_mm(v[0], v[1]), self.to_mm(v[2], v[3])),
            ('y', _, Some(v)) => (
                self.to_mm(v[0], v[1]),
                self.to_mm(v[2], v[3]),
                self.to_mm(v[2], v[3]),
            ),
            _ => {
                self.bad();
                return Ok(());
            }
        };
        let (Some(q1), Some(q2), Some(q3)) = (q1, q2, q3) else {
            self.bad();
            return Ok(());
        };
        if !self.open_sub() {
            self.bad();
            return Ok(());
        }
        let mut pts = Vec::new();
        let n = geom::flatten_cubic([p0, q1, q2, q3], self.opt.tol_mm, &mut pts)?;
        self.add_segments(n)?;
        if let Some(s) = self.path.subs.last_mut() {
            s.pts.extend(pts);
        }
        self.path.cur = Some(q3);
        Ok(())
    }

    fn rect(&mut self, args: &[Obj]) -> Result<(), Fault> {
        let Some([x, y, w, h]) = nums::<4>(args) else {
            self.bad();
            return Ok(());
        };
        let corners = [
            self.to_mm(x, y),
            self.to_mm(x + w, y),
            self.to_mm(x + w, y + h),
            self.to_mm(x, y + h),
        ];
        let Some(pts) = corners.into_iter().collect::<Option<Vec<_>>>() else {
            self.bad();
            return Ok(());
        };
        let Some(&first) = pts.first() else {
            return Ok(());
        };
        self.path.subs.push(Sub { pts, closed: true });
        self.path.cur = Some(first);
        self.path.start = first;
        self.add_segments(4)
    }

    fn paint(&mut self, stroke: bool, fill: bool, close: bool) -> Result<(), Fault> {
        let mut subs = std::mem::take(&mut self.path.subs);
        self.path = Path::default();
        if close {
            for s in subs.iter_mut() {
                s.closed = true;
            }
        }
        let with_geometry = subs.iter().any(|s| s.pts.len() >= 2);
        if !with_geometry {
            self.stats.count("degenerate_subpaths", subs.len() as u64);
            return Ok(());
        }
        self.painted += 1;
        let scale = self.gs.ctm.length_scale();
        let dash_mm: Vec<f64> = self.gs.dash.iter().map(|d| d * scale).collect();
        let phase_mm = self.gs.phase * scale;
        for sub in &subs {
            let mut pts = dedupe(&sub.pts);
            if pts.len() < 2 {
                self.stats.count("degenerate_subpaths", 1);
                continue;
            }
            if fill {
                self.stats.count("fill_subpaths", 1);
                if self.opt.fill {
                    let closed = pts.len() >= 3;
                    self.out.polyline(dxf::LAYER_FILL, &pts, closed)?;
                }
            }
            if stroke {
                let mut closed = sub.closed;
                if closed && pts.len() >= 3 && pts.first() == pts.last() {
                    pts.pop();
                }
                if closed && pts.len() < 3 {
                    closed = false;
                }
                if dash_mm.is_empty() {
                    self.stats.count("stroke_subpaths", 1);
                    self.out.polyline(dxf::LAYER_STROKE, &pts, closed)?;
                } else {
                    self.stats.count("dashed_subpaths", 1);
                    let remaining = self.doc.lim.max_path_segments.saturating_sub(self.segments);
                    let pieces = geom::dash(&pts, closed, &dash_mm, phase_mm, remaining)?;
                    self.add_segments(pieces.len())?;
                    for piece in pieces {
                        let piece = dedupe(&piece);
                        if piece.len() >= 2 {
                            self.stats.count("dash_pieces", 1);
                            self.out.polyline(dxf::LAYER_DASHED, &piece, false)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn do_xobject(&mut self, args: &[Obj]) -> Result<(), Fault> {
        let Some(name) = args.last().and_then(Obj::name) else {
            self.bad();
            return Ok(());
        };
        let Some(cat) = self.doc.dict_entry(&self.res, b"XObject")? else {
            return Ok(());
        };
        let Some(cat) = cat.dict().cloned() else {
            return Ok(());
        };
        let (num, obj) = match cat.get(name) {
            Some(Obj::Ref(n, _)) => {
                let n = *n;
                (Some(n), self.doc.get(n)?)
            }
            Some(other) => (None, Rc::new(other.clone())),
            None => {
                self.stats
                    .warn("xobject_missing", "an XObject resource was not found");
                return Ok(());
            }
        };
        let Obj::Stream(s) = &*obj else {
            self.stats.warn(
                "xobject_ignored",
                "XObjects that are not streams were ignored",
            );
            return Ok(());
        };
        match s.dict.get(b"Subtype").and_then(Obj::name) {
            Some(b"Image") => {
                self.stats.count("images", 1);
                self.record_image();
                Ok(())
            }
            Some(b"Form") => self.form(s, num),
            _ => {
                self.stats.warn(
                    "xobject_ignored",
                    "XObjects other than images and forms were ignored",
                );
                Ok(())
            }
        }
    }

    fn record_image(&mut self) {
        let c = self.gs.ctm;
        let pts = [
            c.apply(0.0, 0.0),
            c.apply(1.0, 0.0),
            c.apply(0.0, 1.0),
            c.apply(1.0, 1.0),
        ];
        let minx = pts.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let maxx = pts.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let miny = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let maxy = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        if [minx, miny, maxx, maxy].iter().all(|v| v.is_finite()) {
            self.images.push([minx, miny, maxx, maxy]);
        }
    }

    /// Form XObject: its matrix is applied, its resources replace the current ones, and
    /// its operators run inside an implicit q/Q.
    fn form(&mut self, s: &Stream, num: Option<u32>) -> Result<(), Fault> {
        if self.form_depth >= self.doc.lim.max_form_depth {
            return limit(
                "form_depth",
                "Form XObjects are nested deeper than the limit allows",
            );
        }
        self.stats.count("forms", 1);
        let data = self.doc.stream_data(s, num)?;
        let matrix = match self.doc.dict_entry(&s.dict, b"Matrix")? {
            Some(Obj::Arr(v)) => {
                let vals: Option<Vec<f64>> = v.iter().map(|o| o.num()).collect();
                vals.and_then(|v| M::from_slice(&v)).unwrap_or(IDENTITY)
            }
            _ => IDENTITY,
        };
        let res = match self.doc.dict_entry(&s.dict, b"Resources")? {
            Some(Obj::Dict(d)) => d,
            _ => self.res.clone(),
        };
        let saved_gs = self.gs.clone();
        let saved_res = std::mem::replace(&mut self.res, res);
        let depth = self.stack.len();
        self.gs.ctm = matrix.then(self.gs.ctm);
        self.form_depth += 1;
        let result = self.run(&data);
        self.form_depth -= 1;
        self.stack.truncate(depth);
        self.res = saved_res;
        self.gs = saved_gs;
        result
    }

    /// Inline image: the dictionary runs to ID; the data runs to a whitespace-delimited EI.
    fn inline_image(&mut self, lx: &mut Lexer) -> Result<(), Fault> {
        let mut seen = 0usize;
        loop {
            match lx.next()? {
                Tok::Kw(b"ID") => break,
                Tok::Eof => return malformed("inline image without ID"),
                _ => {}
            }
            seen += 1;
            if seen > 4096 {
                return malformed("inline image dictionary is too long");
            }
        }
        self.stats.count("images", 1);
        self.record_image();
        let data = lx.d;
        let mut p = lx.pos;
        if data.get(p).is_some_and(|&c| is_ws(c)) {
            p += 1;
        }
        loop {
            let Some(q) = find(data, p, b"EI") else {
                return malformed("inline image without EI");
            };
            let before_ok = q == 0 || data.get(q - 1).is_some_and(|&c| is_ws(c));
            let after_ok = match data.get(q + 2) {
                None => true,
                Some(&c) => is_ws(c) || is_delim(c),
            };
            if before_ok && after_ok {
                lx.pos = q + 2;
                return Ok(());
            }
            p = q + 1;
        }
    }
}
