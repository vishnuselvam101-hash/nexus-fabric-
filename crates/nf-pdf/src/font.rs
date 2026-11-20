//! Fonts for text output.
//!
//! Text is decoded only where the mapping is reliable: WinAnsi (full table),
//! StandardEncoding and MacRomanEncoding (their ASCII ranges), /Differences
//! naming ASCII or uniXXXX glyphs, and ToUnicode CMaps. Anything else gets a skip
//! reason and is counted, never guessed. Type 3 fonts, predefined CMaps, vertical
//! writing and symbolic fonts without a usable map are skipped.

use std::collections::BTreeMap;

use crate::doc::Doc;
use crate::lexer::{Dict, Lexer, Obj, Tok};
use crate::limits::Fault;

/// Simple (one byte per code) or composite (two bytes per code, Identity-H).
enum Kind {
    Simple,
    Composite,
}

enum RangeDst {
    /// Last character of the string increases by one for each code.
    Base(String),
    /// One string per code.
    List(Vec<String>),
}

#[derive(Default)]
struct UniMap {
    singles: BTreeMap<u32, String>,
    ranges: Vec<(u32, u32, RangeDst)>,
}

impl UniMap {
    fn lookup(&self, code: u32) -> Option<String> {
        if let Some(s) = self.singles.get(&code) {
            return Some(s.clone());
        }
        for (lo, hi, dst) in &self.ranges {
            if (*lo..=*hi).contains(&code) {
                let k = code - lo;
                return match dst {
                    RangeDst::Base(s) => {
                        let mut chars: Vec<char> = s.chars().collect();
                        let last = chars.last_mut()?;
                        let next = char::from_u32(u32::from(*last).checked_add(k)?)?;
                        *last = next;
                        Some(chars.into_iter().collect())
                    }
                    RangeDst::List(v) => v.get(k as usize).cloned(),
                };
            }
        }
        None
    }
}

pub struct Font {
    kind: Kind,
    skip: Option<&'static str>,
    table: Vec<Option<char>>,
    uni: Option<UniMap>,
    first: u32,
    widths: Option<Vec<f64>>,
    missing: f64,
    cid_widths: Vec<(u32, u32, f64)>,
    default_width: f64,
}

/// Why text in this font is not emitted, or `None` when it may be emitted.
impl Font {
    /// A font whose text is never emitted, for the given reason.
    pub fn unusable(reason: &'static str) -> Font {
        Font {
            kind: Kind::Simple,
            skip: Some(reason),
            table: Vec::new(),
            uni: None,
            first: 0,
            widths: None,
            missing: 0.0,
            cid_widths: Vec::new(),
            default_width: 1000.0,
        }
    }

    pub fn skip_reason(&self) -> Option<&'static str> {
        self.skip
    }

    /// Codes of a string as (code, byte length).
    pub fn codes(&self, s: &[u8]) -> Vec<(u32, usize)> {
        match self.kind {
            Kind::Simple => s.iter().map(|&b| (u32::from(b), 1)).collect(),
            Kind::Composite => s
                .chunks(2)
                .map(|c| match c {
                    [hi, lo] => ((u32::from(*hi) << 8) | u32::from(*lo), 2),
                    [one] => (u32::from(*one), 1),
                    _ => (0, 0),
                })
                .collect(),
        }
    }

    /// Unicode text of a string, or `None` when any code cannot be decoded reliably.
    pub fn decode(&self, s: &[u8]) -> Option<String> {
        let mut out = String::new();
        for (code, len) in self.codes(s) {
            if let Some(text) = self.uni.as_ref().and_then(|u| u.lookup(code)) {
                out.push_str(&text);
                continue;
            }
            if len != 1 {
                return None;
            }
            let c = self.table.get(code as usize).copied().flatten()?;
            out.push(c);
        }
        Some(out)
    }

    /// Glyph advance in thousandths of text space, or `None` when it is not known.
    pub fn advance(&self, code: u32) -> Option<f64> {
        match self.kind {
            Kind::Simple => {
                let w = self.widths.as_ref()?;
                let idx = code.checked_sub(self.first).map(|i| i as usize);
                Some(match idx.and_then(|i| w.get(i)) {
                    Some(v) => *v,
                    None => self.missing,
                })
            }
            Kind::Composite => {
                let hit = self
                    .cid_widths
                    .iter()
                    .find(|(lo, hi, _)| (*lo..=*hi).contains(&code))
                    .map(|(_, _, w)| *w);
                Some(hit.unwrap_or(self.default_width))
            }
        }
    }

    /// Loads a font dictionary. Unusable fonts load with a skip reason.
    pub fn load(doc: &mut Doc, dict: &Dict) -> Result<Font, Fault> {
        let subtype = doc
            .dict_entry(dict, b"Subtype")?
            .and_then(|o| o.name().map(|n| n.to_vec()))
            .unwrap_or_default();
        let base_font = doc
            .dict_entry(dict, b"BaseFont")?
            .and_then(|o| o.name().map(|n| n.to_vec()))
            .unwrap_or_default();
        let uni = load_to_unicode(doc, dict)?;
        let mut font = Font {
            kind: Kind::Simple,
            skip: None,
            table: vec![None; 256],
            uni,
            first: 0,
            widths: None,
            missing: 0.0,
            cid_widths: Vec::new(),
            default_width: 1000.0,
        };
        match subtype.as_slice() {
            b"Type3" => {
                font.skip = Some("type3_font");
                return Ok(font);
            }
            b"Type0" => return load_composite(doc, dict, font),
            _ => {}
        }
        // Simple fonts.
        if let Some(Obj::Arr(items)) = doc.dict_entry(dict, b"Widths")? {
            font.first = doc
                .dict_entry(dict, b"FirstChar")?
                .and_then(|o| o.num())
                .map_or(0, |v| v.max(0.0) as u32);
            let mut w = Vec::with_capacity(items.len());
            for item in &items {
                w.push(doc.resolve(item)?.num().unwrap_or(0.0));
            }
            font.widths = Some(w);
        }
        if let Some(desc) = doc.dict_entry(dict, b"FontDescriptor")? {
            if let Some(d) = desc.dict() {
                font.missing = doc
                    .dict_entry(d, b"MissingWidth")?
                    .and_then(|o| o.num())
                    .unwrap_or(0.0);
            }
        }
        let base = encoding_base(doc, dict, &base_font)?;
        let mut table: Vec<Option<char>> =
            (0..256u32).map(|c| base.and_then(|f| f(c as u8))).collect();
        if let Some(Obj::Dict(enc)) = doc.dict_entry(dict, b"Encoding")? {
            if let Some(Obj::Arr(diffs)) = doc.dict_entry(&enc, b"Differences")? {
                let mut code = 0usize;
                for item in &diffs {
                    match doc.resolve(item)? {
                        Obj::Num(v) => code = v.max(0.0) as usize,
                        Obj::Name(n) => {
                            if let (Some(slot), Some(ch)) = (table.get_mut(code), glyph_char(&n)) {
                                *slot = Some(ch);
                            }
                            code = code.saturating_add(1);
                        }
                        _ => {}
                    }
                }
            }
        }
        if symbolic_without_map(&base_font, base.is_some(), font.uni.is_some()) {
            font.skip = Some("symbolic_font");
        }
        font.table = table;
        Ok(font)
    }
}

fn symbolic_without_map(base_font: &[u8], has_base: bool, has_uni: bool) -> bool {
    let symbolic = base_font.starts_with(b"Symbol") || base_font.starts_with(b"ZapfDingbats");
    symbolic && !has_base && !has_uni
}

type Encoder = fn(u8) -> Option<char>;

/// Base encoding named by /Encoding (or /BaseEncoding), or the StandardEncoding default
/// for the non-symbolic standard fonts.
fn encoding_base(doc: &mut Doc, dict: &Dict, base_font: &[u8]) -> Result<Option<Encoder>, Fault> {
    let named = match doc.dict_entry(dict, b"Encoding")? {
        Some(Obj::Name(n)) => Some(n),
        Some(Obj::Dict(d)) => match doc.dict_entry(&d, b"BaseEncoding")? {
            Some(Obj::Name(n)) => Some(n),
            _ => None,
        },
        _ => None,
    };
    Ok(match named.as_deref() {
        Some(b"WinAnsiEncoding") => Some(win_ansi as Encoder),
        Some(b"MacRomanEncoding") => Some(mac_roman as Encoder),
        Some(b"StandardEncoding") => Some(standard as Encoder),
        Some(_) => None,
        None => {
            let standard_face = [&b"Helvetica"[..], &b"Times"[..], &b"Courier"[..]]
                .iter()
                .any(|p| base_font.starts_with(p));
            if standard_face {
                Some(standard as Encoder)
            } else {
                None
            }
        }
    })
}

fn load_to_unicode(doc: &mut Doc, dict: &Dict) -> Result<Option<UniMap>, Fault> {
    let Some(Obj::Stream(s)) = doc.dict_entry(dict, b"ToUnicode")? else {
        return Ok(None);
    };
    let bytes = match doc.stream_data(&s, None) {
        Ok(b) => b,
        // A ToUnicode map that cannot be decoded only removes the text mapping.
        Err(Fault::Malformed(_)) | Err(Fault::Unsupported(_)) => return Ok(None),
        Err(other) => return Err(other),
    };
    Ok(Some(parse_cmap(&bytes)))
}

/// Parses the bfchar and bfrange sections of a ToUnicode CMap. Unparseable
/// entries are ignored; the result is only used where a code is found.
fn parse_cmap(data: &[u8]) -> UniMap {
    let mut map = UniMap::default();
    let mut lx = Lexer::new(data, 0);
    let mut budget = 200_000usize;
    while let Ok(t) = lx.next() {
        if t == Tok::Eof || budget == 0 {
            break;
        }
        match t {
            Tok::Kw(b"beginbfchar") => {
                while let Ok(a) = lx.next() {
                    if matches!(a, Tok::Kw(b"endbfchar") | Tok::Eof) {
                        break;
                    }
                    let Ok(b) = lx.next() else { break };
                    budget = budget.saturating_sub(1);
                    if let (Tok::Str(src), Tok::Str(dst)) = (a, b) {
                        if let (Some(code), Some(text)) = (be_code(&src), utf16be(&dst)) {
                            map.singles.insert(code, text);
                        }
                    }
                }
            }
            Tok::Kw(b"beginbfrange") => {
                while let Ok(lo) = lx.next() {
                    if matches!(lo, Tok::Kw(b"endbfrange") | Tok::Eof) {
                        break;
                    }
                    let (Ok(hi), Ok(dst)) = (lx.next(), lx.next()) else {
                        break;
                    };
                    budget = budget.saturating_sub(1);
                    let (Tok::Str(lo), Tok::Str(hi)) = (lo, hi) else {
                        continue;
                    };
                    let (Some(lo), Some(hi)) = (be_code(&lo), be_code(&hi)) else {
                        continue;
                    };
                    match dst {
                        Tok::Str(d) => {
                            if let Some(text) = utf16be(&d) {
                                map.ranges.push((lo, hi, RangeDst::Base(text)));
                            }
                        }
                        Tok::ArrOpen => {
                            let mut list = Vec::new();
                            while let Ok(item) = lx.next() {
                                match item {
                                    Tok::Str(d) => list.push(utf16be(&d).unwrap_or_default()),
                                    Tok::ArrClose | Tok::Eof => break,
                                    _ => {}
                                }
                            }
                            map.ranges.push((lo, hi, RangeDst::List(list)));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    map
}

fn be_code(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || bytes.len() > 4 {
        return None;
    }
    Some(bytes.iter().fold(0u32, |acc, &b| (acc << 8) | u32::from(b)))
}

fn utf16be(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|c| match c {
            [h, l] => (u16::from(*h) << 8) | u16::from(*l),
            _ => 0,
        })
        .collect();
    String::from_utf16(&units).ok()
}

fn load_composite(doc: &mut Doc, dict: &Dict, mut font: Font) -> Result<Font, Fault> {
    font.kind = Kind::Composite;
    match doc.dict_entry(dict, b"Encoding")? {
        Some(Obj::Name(n)) if n == b"Identity-H" => {}
        Some(Obj::Name(n)) if n == b"Identity-V" => {
            font.skip = Some("vertical_writing");
            return Ok(font);
        }
        _ => {
            font.skip = Some("predefined_cmap");
            return Ok(font);
        }
    }
    if font.uni.is_none() {
        font.skip = Some("no_tounicode");
        return Ok(font);
    }
    let desc = match doc.dict_entry(dict, b"DescendantFonts")? {
        Some(Obj::Arr(items)) => match items.first() {
            Some(first) => doc.resolve(first)?,
            None => Obj::Null,
        },
        _ => Obj::Null,
    };
    let Some(cid) = desc.dict().cloned() else {
        font.skip = Some("malformed_font");
        return Ok(font);
    };
    if let Some(dw) = doc.dict_entry(&cid, b"DW")?.and_then(|o| o.num()) {
        font.default_width = dw;
    }
    if let Some(Obj::Arr(w)) = doc.dict_entry(&cid, b"W")? {
        font.cid_widths = parse_cid_widths(doc, &w)?;
    }
    Ok(font)
}

/// /W array: `c [w1 w2 ...]` or `cfirst clast w`.
fn parse_cid_widths(doc: &mut Doc, items: &[Obj]) -> Result<Vec<(u32, u32, f64)>, Fault> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(item) = items.get(i) {
        let first = doc.resolve(item)?.num().and_then(crate::lexer::uint);
        let Some(first) = first else {
            i += 1;
            continue;
        };
        match items.get(i + 1).cloned() {
            Some(Obj::Arr(list)) => {
                for (k, w) in list.iter().enumerate() {
                    let v = doc.resolve(w)?.num().unwrap_or(0.0);
                    let code = first.saturating_add(k as u32);
                    out.push((code, code, v));
                }
                i += 2;
            }
            Some(other) => {
                let last = doc.resolve(&other)?.num().and_then(crate::lexer::uint);
                let w = match items.get(i + 2) {
                    Some(o) => doc.resolve(o)?.num().unwrap_or(0.0),
                    None => 0.0,
                };
                if let Some(last) = last {
                    out.push((first, last, w));
                }
                i += 3;
            }
            None => break,
        }
        if out.len() > 100_000 {
            break;
        }
    }
    Ok(out)
}

fn win_ansi(code: u8) -> Option<char> {
    match code {
        0x20..=0x7E => Some(char::from(code)),
        0x80 => Some('\u{20AC}'),
        0x82 => Some('\u{201A}'),
        0x83 => Some('\u{0192}'),
        0x84 => Some('\u{201E}'),
        0x85 => Some('\u{2026}'),
        0x86 => Some('\u{2020}'),
        0x87 => Some('\u{2021}'),
        0x88 => Some('\u{02C6}'),
        0x89 => Some('\u{2030}'),
        0x8A => Some('\u{0160}'),
        0x8B => Some('\u{2039}'),
        0x8C => Some('\u{0152}'),
        0x8E => Some('\u{017D}'),
        0x91 => Some('\u{2018}'),
        0x92 => Some('\u{2019}'),
        0x93 => Some('\u{201C}'),
        0x94 => Some('\u{201D}'),
        0x95 => Some('\u{2022}'),
        0x96 => Some('\u{2013}'),
        0x97 => Some('\u{2014}'),
        0x98 => Some('\u{02DC}'),
        0x99 => Some('\u{2122}'),
        0x9A => Some('\u{0161}'),
        0x9B => Some('\u{203A}'),
        0x9C => Some('\u{0153}'),
        0x9E => Some('\u{017E}'),
        0x9F => Some('\u{0178}'),
        0xA0..=0xFF => Some(char::from(code)),
        _ => None,
    }
}

fn standard(code: u8) -> Option<char> {
    match code {
        0x27 => Some('\u{2019}'),
        0x60 => Some('\u{2018}'),
        0x20..=0x7E => Some(char::from(code)),
        _ => None,
    }
}

fn mac_roman(code: u8) -> Option<char> {
    match code {
        0x20..=0x7E => Some(char::from(code)),
        _ => None,
    }
}

const NAMED_GLYPHS: &[(&str, char)] = &[
    ("space", ' '),
    ("exclam", '!'),
    ("quotedbl", '"'),
    ("numbersign", '#'),
    ("dollar", '$'),
    ("percent", '%'),
    ("ampersand", '&'),
    ("quotesingle", '\''),
    ("parenleft", '('),
    ("parenright", ')'),
    ("asterisk", '*'),
    ("plus", '+'),
    ("comma", ','),
    ("hyphen", '-'),
    ("period", '.'),
    ("slash", '/'),
    ("zero", '0'),
    ("one", '1'),
    ("two", '2'),
    ("three", '3'),
    ("four", '4'),
    ("five", '5'),
    ("six", '6'),
    ("seven", '7'),
    ("eight", '8'),
    ("nine", '9'),
    ("colon", ':'),
    ("semicolon", ';'),
    ("less", '<'),
    ("equal", '='),
    ("greater", '>'),
    ("question", '?'),
    ("at", '@'),
    ("bracketleft", '['),
    ("backslash", '\\'),
    ("bracketright", ']'),
    ("asciicircum", '^'),
    ("underscore", '_'),
    ("grave", '`'),
    ("braceleft", '{'),
    ("bar", '|'),
    ("braceright", '}'),
    ("asciitilde", '~'),
    ("quoteleft", '\u{2018}'),
    ("quoteright", '\u{2019}'),
    ("endash", '\u{2013}'),
    ("emdash", '\u{2014}'),
    ("bullet", '\u{2022}'),
    ("ellipsis", '\u{2026}'),
    ("Euro", '\u{20AC}'),
];

/// Character for a glyph name: a single ASCII letter or digit, a name from
/// NAMED_GLYPHS, or uniXXXX / uXXXX. Anything else is unknown.
fn glyph_char(name: &[u8]) -> Option<char> {
    if let [c] = name {
        return if c.is_ascii_alphanumeric() {
            Some(char::from(*c))
        } else {
            None
        };
    }
    let text = std::str::from_utf8(name).ok()?;
    if let Some((_, c)) = NAMED_GLYPHS.iter().find(|(n, _)| *n == text) {
        return Some(*c);
    }
    let hex = text
        .strip_prefix("uni")
        .filter(|h| h.len() == 4)
        .or_else(|| {
            text.strip_prefix('u')
                .filter(|h| (4..=6).contains(&h.len()))
        })?;
    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn glyph_names_resolve() {
        assert_eq!(glyph_char(b"A"), Some('A'));
        assert_eq!(glyph_char(b"space"), Some(' '));
        assert_eq!(glyph_char(b"uni00B0"), Some('\u{B0}'));
        assert_eq!(glyph_char(b"notaglyph"), None);
    }

    #[test]
    fn win_ansi_table_covers_latin1_and_cp1252_gaps() {
        assert_eq!(win_ansi(0x41), Some('A'));
        assert_eq!(win_ansi(0x80), Some('\u{20AC}'));
        assert_eq!(win_ansi(0x81), None);
        assert_eq!(win_ansi(0xE9), Some('\u{E9}'));
    }

    #[test]
    fn cmap_bfchar_and_bfrange_parse() {
        let cmap = b"1 begincodespacerange <0000> <FFFF> endcodespacerange \
            1 beginbfchar <0001> <0041> endbfchar \
            1 beginbfrange <0010> <0012> <0061> endbfrange";
        let map = parse_cmap(cmap);
        assert_eq!(map.lookup(1).as_deref(), Some("A"));
        assert_eq!(map.lookup(0x12).as_deref(), Some("c"));
        assert_eq!(map.lookup(0x20), None);
    }
}
