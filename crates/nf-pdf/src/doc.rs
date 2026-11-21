//! Document structure: cross-reference tables and streams, indirect objects,
//! object streams, stream decoding, and reconstruction of damaged files.
//!
//! When the cross-reference data cannot be used, the file is scanned for
//! `N G obj` headers and the trailer is taken from the last dictionary that
//! carries /Root. Every object load counts against the object limit and checks
//! the wall-clock deadline; every array and dictionary item counts against the item
//! limits in [`Doc::parse`].

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::filters::decode_chain;
use crate::lexer::{obj_from_tok, parse_obj, uint, Budget, Dict, Lexer, Name, Obj, Stream, Tok};
use crate::limits::{limit, malformed, Deadline, Fault, Limits, CHECK_EVERY};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Free,
    Offset(usize),
    InStream { stm: u32, index: u32 },
}

struct ObjStm {
    data: Rc<Vec<u8>>,
    first: usize,
    members: Vec<(u32, usize)>,
}

/// An open document. Objects are parsed on demand and cached.
pub struct Doc<'a> {
    pub data: &'a [u8],
    pub lim: Limits,
    pub dl: Deadline,
    pub trailer: Dict,
    /// Bytes materialised from streams so far (checked against `Limits::max_decoded_bytes`).
    pub decoded: usize,
    /// Item budget for objects of the document structure. Objects are cached for the whole
    /// conversion, so the count is never reset, except after an object that was only inspected
    /// during reconstruction and dropped.
    pub parse: Budget,
    objects_parsed: usize,
    entries: BTreeMap<u32, Entry>,
    cache: HashMap<u32, Rc<Obj>>,
    objstm: HashMap<u32, Rc<ObjStm>>,
    streams: HashMap<u32, Rc<Vec<u8>>>,
    resolving: Vec<u32>,
    scan: Option<BTreeMap<u32, usize>>,
}

fn is_kw(t: &Tok, kw: &[u8]) -> bool {
    matches!(t, Tok::Kw(k) if *k == kw)
}

fn read_be(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b))
}

/// Position of the first occurrence of `needle` at or after `from`.
pub fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let tail = hay.get(from..)?;
    tail.windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + from)
}

/// The `N G` pair that ends just before the `obj` keyword at `pos`, if there is one.
fn header_before(d: &[u8], pos: usize) -> Option<(u32, usize)> {
    let ws = |c: u8| matches!(c, 0 | 9 | 10 | 12 | 13 | 32);
    let mut p = pos;
    let skip_ws = |p: &mut usize| -> bool {
        let start = *p;
        while *p > 0 && d.get(*p - 1).is_some_and(|&c| ws(c)) {
            *p -= 1;
        }
        *p < start
    };
    let digits = |p: &mut usize| -> usize {
        let start = *p;
        while *p > 0 && d.get(*p - 1).is_some_and(|c| c.is_ascii_digit()) {
            *p -= 1;
        }
        start - *p
    };
    if !skip_ws(&mut p) {
        return None;
    }
    let gen_end = p;
    if digits(&mut p) == 0 {
        return None;
    }
    let gen = uint(scan_digits(d.get(p..gen_end)?))?;
    if !skip_ws(&mut p) {
        return None;
    }
    let num_end = p;
    if digits(&mut p) == 0 {
        return None;
    }
    let num = uint(scan_digits(d.get(p..num_end)?))?;
    (gen <= 65_535).then_some((num, p))
}

fn scan_digits(s: &[u8]) -> f64 {
    s.iter().fold(0f64, |acc, &c| {
        acc * 10.0 + f64::from(c.saturating_sub(b'0'))
    })
}

impl<'a> Doc<'a> {
    /// Opens a document: cross-reference data first, reconstruction if that fails.
    pub fn open(data: &'a [u8], lim: Limits, dl: Deadline) -> Result<Doc<'a>, Fault> {
        let parse = Budget::new(&lim, dl);
        let mut doc = Doc {
            data,
            lim,
            dl,
            trailer: Dict::default(),
            decoded: 0,
            parse,
            objects_parsed: 0,
            entries: BTreeMap::new(),
            cache: HashMap::new(),
            objstm: HashMap::new(),
            streams: HashMap::new(),
            resolving: Vec::new(),
            scan: None,
        };
        let usable = match doc.read_xref_chain() {
            Ok(()) => doc.trailer.get(b"Root").is_some(),
            Err(Fault::Malformed(_)) | Err(Fault::Unsupported(_)) => false,
            Err(other) => return Err(other),
        };
        if !usable {
            doc.entries.clear();
            doc.trailer = Dict::default();
            doc.cache.clear();
            doc.objstm.clear();
            doc.streams.clear();
            doc.reconstruct()?;
        }
        if doc.trailer.get(b"Encrypt").is_some() {
            return Err(Fault::Encrypted);
        }
        if doc.trailer.get(b"Root").is_none() {
            return malformed("no document catalogue (/Root) found");
        }
        Ok(doc)
    }

    fn count_object(&mut self) -> Result<(), Fault> {
        self.objects_parsed += 1;
        if self.objects_parsed > self.lim.max_objects {
            return limit("objects", "more objects than the limit allows");
        }
        self.dl.check()
    }

    /// Follows the /Prev chain from the last `startxref`, newest section first.
    fn read_xref_chain(&mut self) -> Result<(), Fault> {
        let tail_start = self.data.len().saturating_sub(4096);
        let tail = self.data.get(tail_start..).unwrap_or(&[]);
        let Some(kw) = tail.windows(9).rposition(|w| w == b"startxref") else {
            return malformed("no startxref keyword");
        };
        let mut lx = Lexer::new(tail, kw + 9);
        let mut off = match lx.next()? {
            Tok::Num(v) => uint(v).map(|x| x as usize),
            _ => None,
        }
        .ok_or_else(|| Fault::Malformed("startxref offset is not a number".into()))?;
        let mut seen = BTreeSet::new();
        let mut first = true;
        for _ in 0..self.lim.max_xref_sections {
            if !seen.insert(off) {
                break;
            }
            let section = self.read_xref_section(off)?;
            if first {
                self.trailer = section.clone();
                first = false;
            } else {
                for (k, v) in section.0.iter() {
                    if self.trailer.get(k).is_none() {
                        self.trailer.0.push((k.clone(), v.clone()));
                    }
                }
            }
            match section.get(b"Prev").and_then(Obj::num).and_then(uint) {
                Some(p) => off = p as usize,
                None => return Ok(()),
            }
        }
        limit("xref_sections", "too many cross-reference sections")
    }

    fn read_xref_section(&mut self, off: usize) -> Result<Dict, Fault> {
        if self.data.get(off..).is_some_and(|s| s.starts_with(b"xref")) {
            self.read_table(off)
        } else {
            self.read_xref_stream(off)
        }
    }

    fn read_table(&mut self, off: usize) -> Result<Dict, Fault> {
        let mut lx = Lexer::new(self.data, off);
        lx.next()?;
        let mut table: Vec<(u32, Entry)> = Vec::new();
        loop {
            let t = lx.next()?;
            if is_kw(&t, b"trailer") {
                break;
            }
            let (first, count) = match (t, lx.next()?) {
                (Tok::Num(a), Tok::Num(b)) => match (uint(a), uint(b)) {
                    (Some(a), Some(b)) => (a, b),
                    _ => return malformed("bad cross-reference subsection header"),
                },
                _ => return malformed("bad cross-reference subsection header"),
            };
            for i in 0..count {
                if table.len() > self.lim.max_objects {
                    return limit("objects", "cross-reference table is too large");
                }
                if i.is_multiple_of(4096) {
                    self.dl.check()?;
                }
                let (o, g, k) = (lx.next()?, lx.next()?, lx.next()?);
                let (Tok::Num(o), Tok::Num(_), Tok::Kw(kind)) = (o, g, k) else {
                    return malformed("bad cross-reference entry");
                };
                let Some(num) = first.checked_add(i) else {
                    return malformed("cross-reference object number overflows");
                };
                let entry = if kind == b"n".as_slice() {
                    match uint(o) {
                        Some(o) => Entry::Offset(o as usize),
                        None => return malformed("bad cross-reference offset"),
                    }
                } else if kind == b"f".as_slice() {
                    Entry::Free
                } else {
                    return malformed("bad cross-reference entry type");
                };
                table.push((num, entry));
            }
        }
        let trailer = match parse_obj(&mut lx, &mut self.parse)? {
            Obj::Dict(d) => d,
            _ => return malformed("trailer is not a dictionary"),
        };
        // Hybrid files: entries in the referenced stream take precedence over the table.
        if let Some(xs) = trailer.get(b"XRefStm").and_then(Obj::num).and_then(uint) {
            self.read_xref_stream(xs as usize)?;
        }
        for (num, entry) in table {
            self.entries.entry(num).or_insert(entry);
        }
        Ok(trailer)
    }

    fn read_xref_stream(&mut self, off: usize) -> Result<Dict, Fault> {
        let (_, obj) = self.parse_indirect_at(off)?;
        let Obj::Stream(s) = obj else {
            return malformed("cross-reference stream expected");
        };
        let w_items = match s.dict.get(b"W").and_then(Obj::arr) {
            Some(a) if a.len() == 3 => a.to_vec(),
            _ => return malformed("cross-reference stream has no valid /W"),
        };
        let mut w = [0usize; 3];
        for (slot, item) in w.iter_mut().zip(w_items.iter()) {
            match item.num().and_then(uint) {
                Some(v) if v <= 8 => *slot = v as usize,
                _ => return malformed("bad /W in cross-reference stream"),
            }
        }
        let size = s
            .dict
            .get(b"Size")
            .and_then(Obj::num)
            .and_then(uint)
            .unwrap_or(0);
        let index: Vec<(u32, u32)> = match s.dict.get(b"Index").and_then(Obj::arr) {
            Some(a) => a
                .chunks(2)
                .filter_map(|p| match p {
                    [x, y] => Some((x.num().and_then(uint)?, y.num().and_then(uint)?)),
                    _ => None,
                })
                .collect(),
            None => vec![(0, size)],
        };
        let row = w[0] + w[1] + w[2];
        if row == 0 {
            return malformed("cross-reference stream rows are empty");
        }
        let data = self.stream_data(&s, None)?;
        let mut pos = 0usize;
        for (start, count) in index {
            for i in 0..count {
                if i.is_multiple_of(4096) {
                    self.dl.check()?;
                }
                let Some(rec) = data.get(pos..pos + row) else {
                    return malformed("cross-reference stream is truncated");
                };
                pos += row;
                let kind = if w[0] == 0 {
                    1
                } else {
                    read_be(rec.get(..w[0]).unwrap_or(&[]))
                };
                let f2 = read_be(rec.get(w[0]..w[0] + w[1]).unwrap_or(&[]));
                let f3 = read_be(rec.get(w[0] + w[1]..row).unwrap_or(&[]));
                let Some(num) = start.checked_add(i) else {
                    return malformed("cross-reference object number overflows");
                };
                let entry = match kind {
                    0 => Entry::Free,
                    1 => Entry::Offset(usize::try_from(f2).unwrap_or(usize::MAX)),
                    2 => Entry::InStream {
                        stm: u32::try_from(f2).unwrap_or(u32::MAX),
                        index: u32::try_from(f3).unwrap_or(u32::MAX),
                    },
                    _ => continue,
                };
                if self.entries.len() > self.lim.max_objects {
                    return limit("objects", "cross-reference stream is too large");
                }
                self.entries.entry(num).or_insert(entry);
            }
        }
        Ok(s.dict.clone())
    }

    /// Parses an indirect object at `off`, including stream data.
    fn parse_indirect_at(&mut self, off: usize) -> Result<(u32, Obj), Fault> {
        self.count_object()?;
        let mut lx = Lexer::new(self.data, off);
        let num = match lx.next()? {
            Tok::Num(v) => uint(v),
            _ => None,
        }
        .ok_or_else(|| Fault::Malformed("object header expected".into()))?;
        if !matches!(lx.next()?, Tok::Num(_)) || !is_kw(&lx.next()?, b"obj") {
            return malformed("object header expected");
        }
        let t = lx.next()?;
        let value = obj_from_tok(&mut lx, t, 0, &mut self.parse)?;
        if let Obj::Dict(d) = &value {
            let t2 = lx.next()?;
            if is_kw(&t2, b"stream") {
                let mut p = lx.pos;
                if self.data.get(p) == Some(&b'\r') {
                    p += 1;
                }
                if self.data.get(p) == Some(&b'\n') {
                    p += 1;
                }
                let len = self.stream_length(d, p)?;
                return Ok((
                    num,
                    Obj::Stream(Stream {
                        dict: d.clone(),
                        start: p,
                        len,
                    }),
                ));
            }
        }
        Ok((num, value))
    }

    fn stream_length(&mut self, d: &Dict, p: usize) -> Result<usize, Fault> {
        let declared = match d.get(b"Length") {
            Some(Obj::Num(v)) => uint(*v),
            Some(Obj::Ref(n, _)) => {
                let o = self.get(*n)?;
                o.num().and_then(uint)
            }
            _ => None,
        };
        if let Some(len) = declared.map(|x| x as usize) {
            if let Some(end) = p.checked_add(len) {
                let mut q = end;
                while self
                    .data
                    .get(q)
                    .is_some_and(|&c| matches!(c, 0 | 9 | 10 | 12 | 13 | 32))
                {
                    q += 1;
                }
                if self.data.get(q..q + 9).is_some_and(|s| s == b"endstream") {
                    return Ok(len);
                }
            }
        }
        // Declared length is wrong: take everything up to the end keyword, minus one EOL.
        let Some(e) = find(self.data, p, b"endstream") else {
            return malformed("stream without endstream");
        };
        let mut len = e.saturating_sub(p);
        let tail2 = len
            .checked_sub(2)
            .and_then(|s| self.data.get(p + s..p + len));
        if tail2.is_some_and(|s| s == b"\r\n") {
            len -= 2;
        } else if len >= 1 && matches!(self.data.get(p + len - 1), Some(b'\n') | Some(b'\r')) {
            len -= 1;
        }
        Ok(len)
    }

    /// Object `num`, parsed on first use and cached. Missing or free objects are Null.
    pub fn get(&mut self, num: u32) -> Result<Rc<Obj>, Fault> {
        if let Some(o) = self.cache.get(&num) {
            return Ok(o.clone());
        }
        if self.resolving.contains(&num) {
            return malformed("circular reference between objects");
        }
        self.resolving.push(num);
        let loaded = self.load(num);
        self.resolving.pop();
        let obj = Rc::new(loaded?);
        self.cache.insert(num, obj.clone());
        Ok(obj)
    }

    fn load(&mut self, num: u32) -> Result<Obj, Fault> {
        match self.entries.get(&num).copied() {
            Some(Entry::Offset(off)) => match self.parse_indirect_at(off) {
                Ok((n, obj)) if n == num => Ok(obj),
                Ok(_) | Err(Fault::Malformed(_)) => self.load_by_scan(num),
                Err(e) => Err(e),
            },
            Some(Entry::InStream { stm, index }) => self.load_in_stream(stm, index, num),
            Some(Entry::Free) => Ok(Obj::Null),
            None => self.load_by_scan(num),
        }
    }

    fn load_by_scan(&mut self, num: u32) -> Result<Obj, Fault> {
        self.ensure_scan()?;
        let Some(off) = self.scanned(num) else {
            return Ok(Obj::Null);
        };
        match self.parse_indirect_at(off) {
            Ok((n, obj)) if n == num => Ok(obj),
            Ok(_) | Err(Fault::Malformed(_)) => Ok(Obj::Null),
            Err(e) => Err(e),
        }
    }

    /// Scans the whole file once for `N G obj` headers (last definition wins).
    fn ensure_scan(&mut self) -> Result<(), Fault> {
        if self.scan.is_some() {
            return Ok(());
        }
        let mut map = BTreeMap::new();
        let mut i = 0usize;
        let mut rounds = 0u64;
        while let Some(pos) = find(self.data, i, b"obj") {
            if rounds.is_multiple_of(CHECK_EVERY) {
                self.dl.check()?;
            }
            rounds += 1;
            if let Some((num, start)) = header_before(self.data, pos) {
                if map.len() > self.lim.max_objects {
                    return limit("objects", "too many object headers in the file");
                }
                map.insert(num, start);
            }
            i = pos + 3;
        }
        self.scan = Some(map);
        Ok(())
    }

    fn scanned(&self, num: u32) -> Option<usize> {
        self.scan.as_ref().and_then(|m| m.get(&num).copied())
    }

    fn load_in_stream(&mut self, stm: u32, index: u32, num: u32) -> Result<Obj, Fault> {
        self.count_object()?;
        let os = self.objstm(stm)?;
        let Some(&(objnum, off)) = os.members.get(index as usize) else {
            return Ok(Obj::Null);
        };
        if objnum != num {
            return Ok(Obj::Null);
        }
        let start = os.first.saturating_add(off);
        let mut lx = Lexer::new(os.data.as_slice(), start);
        let t = lx.next()?;
        obj_from_tok(&mut lx, t, 0, &mut self.parse)
    }

    fn objstm(&mut self, stm: u32) -> Result<Rc<ObjStm>, Fault> {
        if let Some(o) = self.objstm.get(&stm) {
            return Ok(o.clone());
        }
        let obj = self.get(stm)?;
        let Obj::Stream(s) = &*obj else {
            return malformed("object stream is not a stream");
        };
        let s = s.clone();
        let n = s
            .dict
            .get(b"N")
            .and_then(Obj::num)
            .and_then(uint)
            .ok_or_else(|| Fault::Malformed("object stream without /N".into()))?;
        let first = s
            .dict
            .get(b"First")
            .and_then(Obj::num)
            .and_then(uint)
            .ok_or_else(|| Fault::Malformed("object stream without /First".into()))?;
        if n as usize > self.lim.max_objects {
            return limit("objects", "object stream holds too many objects");
        }
        let data = self.stream_data(&s, Some(stm))?;
        let mut lx = Lexer::new(data.as_slice(), 0);
        let mut members = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let (a, b) = (lx.next()?, lx.next()?);
            match (a, b) {
                (Tok::Num(x), Tok::Num(y)) => match (uint(x), uint(y)) {
                    (Some(x), Some(y)) => members.push((x, y as usize)),
                    _ => return malformed("bad object stream header"),
                },
                _ => return malformed("bad object stream header"),
            }
        }
        let rc = Rc::new(ObjStm {
            data,
            first: first as usize,
            members,
        });
        self.objstm.insert(stm, rc.clone());
        Ok(rc)
    }

    /// Decoded data of a stream (cached when its object number is known).
    pub fn stream_data(&mut self, s: &Stream, num: Option<u32>) -> Result<Rc<Vec<u8>>, Fault> {
        if let Some(n) = num {
            if let Some(c) = self.streams.get(&n) {
                return Ok(c.clone());
            }
        }
        let end = s
            .start
            .checked_add(s.len)
            .ok_or_else(|| Fault::Malformed("stream range overflows".into()))?;
        let raw = self
            .data
            .get(s.start..end)
            .ok_or_else(|| Fault::Malformed("stream data lies outside the file".into()))?;
        let stages = self.filter_stages(&s.dict)?;
        let out = if stages.is_empty() {
            if self.decoded.saturating_add(raw.len()) > self.lim.max_decoded_bytes {
                return limit(
                    "decoded_bytes",
                    "decoded stream data exceeds the total budget",
                );
            }
            self.decoded += raw.len();
            raw.to_vec()
        } else {
            decode_chain(raw, &stages, &self.lim, &mut self.decoded, &self.dl)?
        };
        let rc = Rc::new(out);
        if let Some(n) = num {
            self.streams.insert(n, rc.clone());
        }
        Ok(rc)
    }

    fn filter_stages(&mut self, dict: &Dict) -> Result<Vec<(Name, Dict)>, Fault> {
        let names: Vec<Name> = match self.dict_entry(dict, b"Filter")? {
            None => Vec::new(),
            Some(Obj::Name(n)) => vec![n],
            Some(Obj::Arr(items)) => {
                let mut v = Vec::new();
                for item in items {
                    match self.resolve(&item)? {
                        Obj::Name(n) => v.push(n),
                        _ => return malformed("bad /Filter entry"),
                    }
                }
                v
            }
            Some(_) => return malformed("bad /Filter"),
        };
        let parms: Vec<Dict> = match self.dict_entry(dict, b"DecodeParms")? {
            Some(Obj::Dict(d)) => vec![d],
            Some(Obj::Arr(items)) => items
                .into_iter()
                .map(|p| match p {
                    Obj::Dict(d) => d,
                    _ => Dict::default(),
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(names
            .into_iter()
            .enumerate()
            .map(|(i, n)| (n, parms.get(i).cloned().unwrap_or_default()))
            .collect())
    }

    /// Dictionary entry with any indirect reference resolved.
    pub fn dict_entry(&mut self, d: &Dict, key: &[u8]) -> Result<Option<Obj>, Fault> {
        match d.get(key) {
            None => Ok(None),
            Some(v) => Ok(Some(self.resolve(v)?)),
        }
    }

    /// `o` itself, or the object it refers to.
    pub fn resolve(&mut self, o: &Obj) -> Result<Obj, Fault> {
        match o {
            Obj::Ref(n, _) => Ok((*self.get(*n)?).clone()),
            other => Ok(other.clone()),
        }
    }

    /// Rebuilds the object map and the trailer from the raw file contents.
    fn reconstruct(&mut self) -> Result<(), Fault> {
        self.ensure_scan()?;
        let offsets: Vec<(u32, usize)> = self
            .scan
            .as_ref()
            .map(|m| m.iter().map(|(&n, &o)| (n, o)).collect())
            .unwrap_or_default();
        for &(num, off) in &offsets {
            self.entries.insert(num, Entry::Offset(off));
        }
        let mut by_offset: Vec<(usize, u32)> = offsets.iter().map(|&(n, o)| (o, n)).collect();
        by_offset.sort_unstable();
        let mut best: Option<(usize, Dict)> = None;
        let mut next = find(self.data, 0, b"trailer");
        while let Some(pos) = next {
            self.dl.check()?;
            let at = pos + 7;
            next = find(self.data, at, b"trailer");
            // A candidate is read only up to the next 'trailer' keyword, so the candidate parses
            // never overlap and the search stays linear in the file size.
            let window = self
                .data
                .get(..next.unwrap_or(self.data.len()))
                .unwrap_or(self.data);
            let mut lx = Lexer::new(window, at);
            let kept = self.parse.items;
            let candidate = parse_obj(&mut lx, &mut self.parse);
            self.parse.items = kept;
            if let Ok(Obj::Dict(d)) = candidate {
                if d.get(b"Root").is_some() {
                    best = Some((pos, d));
                }
            }
        }
        for (off, num) in by_offset {
            self.dl.check()?;
            // The object is only inspected here and dropped; an object used later is parsed
            // again and counted then.
            let kept = self.parse.items;
            let parsed = self.parse_indirect_at(off);
            self.parse.items = kept;
            let Ok((n, obj)) = parsed else {
                continue;
            };
            if n != num {
                continue;
            }
            let is_objstm = matches!(&obj, Obj::Stream(s)
                if s.dict.get(b"Type").and_then(Obj::name).is_some_and(|t| t == b"ObjStm"));
            if is_objstm {
                if let Ok(os) = self.objstm(num) {
                    for (i, (member, _)) in os.members.iter().enumerate() {
                        let index = u32::try_from(i).unwrap_or(u32::MAX);
                        self.entries
                            .entry(*member)
                            .or_insert(Entry::InStream { stm: num, index });
                    }
                }
            }
            if let Some(d) = obj.dict() {
                if d.get(b"Root").is_some() && best.as_ref().is_none_or(|(p, _)| off >= *p) {
                    best = Some((off, d.clone()));
                }
            }
        }
        match best {
            Some((_, d)) => {
                self.trailer = d;
                Ok(())
            }
            None => malformed("no trailer with /Root and no cross-reference data"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn header_before_reads_number_and_generation() {
        let d = b"xx\n12 0 obj\n<<>>";
        let pos = find(d, 0, b"obj").unwrap_or(0);
        assert_eq!(header_before(d, pos), Some((12, 3)));
        assert_eq!(header_before(b"endobj", 3), None);
    }
}
