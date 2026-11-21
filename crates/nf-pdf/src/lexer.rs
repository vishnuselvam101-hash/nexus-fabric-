//! PDF lexical analysis and object parsing over byte slices.
//!
//! Nothing here indexes untrusted data unchecked: every read goes through
//! `get`, and nesting is bounded by the caller's depth limit.

use crate::limits::{malformed, Fault};

pub type Name = Vec<u8>;

/// A dictionary: key/value pairs in file order. A duplicate key resolves to the first value.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Dict(pub Vec<(Name, Obj)>);

impl Dict {
    pub fn get(&self, key: &[u8]) -> Option<&Obj> {
        self.0
            .iter()
            .find(|(k, _)| k.as_slice() == key)
            .map(|(_, v)| v)
    }
}

/// A stream object: its dictionary and the byte range of its raw data in the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Stream {
    pub dict: Dict,
    pub start: usize,
    pub len: usize,
}

/// A PDF object. Integers and reals share `Num`; object numbers are checked on use.
#[derive(Clone, Debug, PartialEq)]
pub enum Obj {
    Null,
    Bool(bool),
    Num(f64),
    Name(Name),
    Str(Vec<u8>),
    Arr(Vec<Obj>),
    Dict(Dict),
    Stream(Stream),
    Ref(u32, u16),
}

impl Obj {
    pub fn num(&self) -> Option<f64> {
        match self {
            Obj::Num(v) => Some(*v),
            _ => None,
        }
    }
    pub fn name(&self) -> Option<&[u8]> {
        match self {
            Obj::Name(n) => Some(n.as_slice()),
            _ => None,
        }
    }
    pub fn arr(&self) -> Option<&[Obj]> {
        match self {
            Obj::Arr(a) => Some(a.as_slice()),
            _ => None,
        }
    }
    /// The dictionary of a dictionary or of a stream.
    pub fn dict(&self) -> Option<&Dict> {
        match self {
            Obj::Dict(d) => Some(d),
            Obj::Stream(s) => Some(&s.dict),
            _ => None,
        }
    }
}

/// `v` as an unsigned 32-bit integer, if it is one.
pub fn uint(v: f64) -> Option<u32> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&v) {
        Some(v as u32)
    } else {
        None
    }
}

pub fn is_ws(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}

pub fn is_delim(c: u8) -> bool {
    matches!(
        c,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Value of a numeric token made only of `+ - . 0-9`. Digits beyond 40 are ignored
/// so that absurdly long tokens stay finite and cheap.
pub fn scan_number(s: &[u8]) -> Option<f64> {
    let mut i = 0usize;
    let mut neg = false;
    match s.first() {
        Some(b'-') => {
            neg = true;
            i = 1;
        }
        Some(b'+') => i = 1,
        _ => {}
    }
    let mut value = 0f64;
    let mut scale = 1f64;
    let mut seen_dot = false;
    let mut kept = 0usize;
    let mut any = false;
    while let Some(&c) = s.get(i) {
        match c {
            b'0'..=b'9' => {
                any = true;
                if kept < 40 {
                    value = value * 10.0 + f64::from(c - b'0');
                    kept += 1;
                    if seen_dot {
                        scale *= 10.0;
                    }
                }
            }
            b'.' => {
                if seen_dot {
                    break;
                }
                seen_dot = true;
            }
            _ => break,
        }
        i += 1;
    }
    if !any {
        return None;
    }
    let v = value / scale;
    Some(if neg { -v } else { v })
}

/// One lexical token. `Kw` borrows from the input.
#[derive(Debug, PartialEq)]
pub enum Tok<'a> {
    Num(f64),
    Name(Name),
    Str(Vec<u8>),
    ArrOpen,
    ArrClose,
    DictOpen,
    DictClose,
    Kw(&'a [u8]),
    Eof,
}

pub struct Lexer<'a> {
    pub d: &'a [u8],
    pub pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(d: &'a [u8], pos: usize) -> Self {
        Lexer { d, pos }
    }

    fn peek(&self) -> Option<u8> {
        self.d.get(self.pos).copied()
    }

    fn peek_at(&self, off: usize) -> Option<u8> {
        self.d.get(self.pos.checked_add(off)?).copied()
    }

    fn slice(&self, start: usize, end: usize) -> &'a [u8] {
        self.d.get(start..end).unwrap_or(&[])
    }

    /// Skips white space and comments.
    pub fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if is_ws(c) {
                self.pos += 1;
            } else if c == b'%' {
                while let Some(c) = self.peek() {
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    pub fn next(&mut self) -> Result<Tok<'a>, Fault> {
        self.skip_ws();
        let Some(c) = self.peek() else {
            return Ok(Tok::Eof);
        };
        let start = self.pos;
        match c {
            b'[' => {
                self.pos += 1;
                Ok(Tok::ArrOpen)
            }
            b']' => {
                self.pos += 1;
                Ok(Tok::ArrClose)
            }
            b'{' | b'}' | b')' => {
                self.pos += 1;
                Ok(Tok::Kw(self.slice(start, self.pos)))
            }
            b'<' => {
                if self.peek_at(1) == Some(b'<') {
                    self.pos += 2;
                    Ok(Tok::DictOpen)
                } else {
                    Ok(Tok::Str(self.hex_string()))
                }
            }
            b'>' => {
                if self.peek_at(1) == Some(b'>') {
                    self.pos += 2;
                    Ok(Tok::DictClose)
                } else {
                    self.pos += 1;
                    Ok(Tok::Kw(self.slice(start, self.pos)))
                }
            }
            b'(' => Ok(Tok::Str(self.lit_string())),
            b'/' => Ok(Tok::Name(self.name())),
            _ => {
                while let Some(c) = self.peek() {
                    if is_ws(c) || is_delim(c) {
                        break;
                    }
                    self.pos += 1;
                }
                let run = self.slice(start, self.pos);
                if matches!(c, b'0'..=b'9' | b'+' | b'-' | b'.') {
                    if let Some(v) = scan_number(run) {
                        return Ok(Tok::Num(v));
                    }
                }
                Ok(Tok::Kw(run))
            }
        }
    }

    fn lit_string(&mut self) -> Vec<u8> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut depth = 1usize;
        while let Some(c) = self.peek() {
            self.pos += 1;
            match c {
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                b'\\' => {
                    let Some(n) = self.peek() else { break };
                    self.pos += 1;
                    match n {
                        b'n' => out.push(10),
                        b'r' => out.push(13),
                        b't' => out.push(9),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = u32::from(n - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xFF) as u8);
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        out
    }

    fn hex_string(&mut self) -> Vec<u8> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut hi: Option<u8> = None;
        while let Some(c) = self.peek() {
            self.pos += 1;
            if c == b'>' {
                break;
            }
            if let Some(v) = hexval(c) {
                match hi {
                    None => hi = Some(v),
                    Some(h) => {
                        out.push((h << 4) | v);
                        hi = None;
                    }
                }
            }
        }
        if let Some(h) = hi {
            out.push(h << 4);
        }
        out
    }

    fn name(&mut self) -> Name {
        self.pos += 1;
        let mut out = Vec::new();
        while let Some(c) = self.peek() {
            if is_ws(c) || is_delim(c) {
                break;
            }
            self.pos += 1;
            if c == b'#' {
                if let (Some(h), Some(l)) = (
                    self.peek().and_then(hexval),
                    self.peek_at(1).and_then(hexval),
                ) {
                    out.push((h << 4) | l);
                    self.pos += 2;
                    continue;
                }
            }
            out.push(c);
        }
        out
    }
}

/// Parses one direct object starting at the lexer's position.
pub fn parse_obj(lx: &mut Lexer, max_depth: usize) -> Result<Obj, Fault> {
    let t = lx.next()?;
    obj_from_tok(lx, t, 0, max_depth)
}

/// Builds an object from a token already read. `depth` counts open arrays and dictionaries.
pub fn obj_from_tok(lx: &mut Lexer, t: Tok, depth: usize, max_depth: usize) -> Result<Obj, Fault> {
    match t {
        Tok::Num(v) => {
            // "n g R" is an indirect reference; anything else leaves the lexer where it was.
            if let Some(n) = uint(v) {
                let save = lx.pos;
                if let Ok(Tok::Num(g)) = lx.next() {
                    if let Some(g16) = uint(g).filter(|g| *g <= 65_535) {
                        if let Ok(Tok::Kw(k)) = lx.next() {
                            if k == b"R".as_slice() {
                                return Ok(Obj::Ref(n, g16 as u16));
                            }
                        }
                    }
                }
                lx.pos = save;
            }
            Ok(Obj::Num(v))
        }
        Tok::Name(n) => Ok(Obj::Name(n)),
        Tok::Str(s) => Ok(Obj::Str(s)),
        Tok::ArrOpen => {
            if depth >= max_depth {
                return Err(Fault::Limit("nesting", "object nesting too deep".into()));
            }
            let mut items = Vec::new();
            loop {
                match lx.next()? {
                    Tok::ArrClose => break,
                    Tok::Eof => return malformed("unterminated array"),
                    other => items.push(obj_from_tok(lx, other, depth + 1, max_depth)?),
                }
            }
            Ok(Obj::Arr(items))
        }
        Tok::DictOpen => {
            if depth >= max_depth {
                return Err(Fault::Limit("nesting", "object nesting too deep".into()));
            }
            let mut dict = Dict::default();
            loop {
                match lx.next()? {
                    Tok::DictClose => break,
                    Tok::Name(key) => {
                        let vt = lx.next()?;
                        if vt == Tok::DictClose {
                            break;
                        }
                        let v = obj_from_tok(lx, vt, depth + 1, max_depth)?;
                        dict.0.push((key, v));
                    }
                    Tok::Eof => return malformed("unterminated dictionary"),
                    _ => return malformed("dictionary key is not a name"),
                }
            }
            Ok(Obj::Dict(dict))
        }
        Tok::Kw(k) => match k {
            b"true" => Ok(Obj::Bool(true)),
            b"false" => Ok(Obj::Bool(false)),
            b"null" => Ok(Obj::Null),
            _ => malformed(format!(
                "unexpected keyword '{}' where an object was expected",
                String::from_utf8_lossy(k.get(..k.len().min(16)).unwrap_or(k))
            )),
        },
        Tok::ArrClose | Tok::DictClose => malformed("unbalanced closing delimiter"),
        Tok::Eof => malformed("unexpected end of data"),
    }
}
