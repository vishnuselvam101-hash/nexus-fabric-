//! From recogniser characters to number items: split, clean, parse, classify.

use crate::rec::Ch;

/// What the number most likely is, from its printed prefix or suffix. A hint for the UI only:
/// a person always confirms the slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Diameter,
    Radius,
    Thickness,
    Length,
    Thread,
    Eccentricity,
    Angle,
    Tolerance,
    Count,
    Number,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Diameter => "diameter",
            Kind::Radius => "radius",
            Kind::Thickness => "thickness",
            Kind::Length => "length",
            Kind::Thread => "thread",
            Kind::Eccentricity => "eccentricity",
            Kind::Angle => "angle",
            Kind::Tolerance => "tolerance",
            Kind::Count => "count",
            Kind::Number => "number",
        }
    }
}

/// A cleaned, recognised number token (without geometry).
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub text: String,
    pub value: Option<f64>,
    pub kind: Kind,
    /// True when the text differs from what the recogniser returned (junk stripped, decimal mark or
    /// phi repaired); the raw string is then kept next to it.
    pub repaired: bool,
    /// True when the written form has more than one reading (a comma that is not a thousands group,
    /// a ratio or time such as 1:2 or 12:30). A person must confirm such a number.
    pub ambiguous: bool,
}

/// A group of characters between two spaces (or after a split at a tolerance sign).
#[derive(Clone, Debug)]
pub struct Tok {
    pub chars: Vec<Ch>,
}

impl Tok {
    pub fn text(&self) -> String {
        self.chars.iter().map(|c| c.glyph.as_str()).collect()
    }
    pub fn conf(&self) -> f32 {
        if self.chars.is_empty() {
            0.0
        } else {
            self.chars.iter().map(|c| c.prob).sum::<f32>() / self.chars.len() as f32
        }
    }
}

/// Split a recognised line into tokens: at spaces, and in front of each tolerance sign; then glue the
/// pieces of a decimal number the recogniser separated ("1030." + "44").
pub fn tokens(chars: &[Ch]) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::new();
    let mut cur: Vec<Ch> = Vec::new();
    for c in chars {
        let g = c.glyph.as_str();
        if g == " " {
            if !cur.is_empty() {
                out.push(Tok {
                    chars: std::mem::take(&mut cur),
                });
            }
            continue;
        }
        if g == "±" && !cur.is_empty() {
            out.push(Tok {
                chars: std::mem::take(&mut cur),
            });
        }
        cur.push(c.clone());
    }
    if !cur.is_empty() {
        out.push(Tok { chars: cur });
    }
    let mut glued: Vec<Tok> = Vec::new();
    for t in out {
        if let Some(prev) = glued.last_mut() {
            let pt = prev.text();
            let tt = t.text();
            let a_open = pt.ends_with(['.', ','])
                && pt.chars().rev().nth(1).is_some_and(|c| c.is_ascii_digit());
            let b_digits = tt.chars().take(3).all(|c| c.is_ascii_digit()) && tt.len() <= 3;
            if a_open && b_digits {
                prev.chars.extend(t.chars);
                continue;
            }
        }
        glued.push(t);
    }
    glued
}

/// A leading + or - is kept as a sign only in front of a decimal number below this value.
pub const SIGN_KEPT_BELOW: f64 = 10.0;

const JUNK: &str = "→←↑↓↔↕—–-_|/\\~·.,:;'\"`<>()[]{}*^=+";

fn is_prefix(c: char) -> bool {
    matches!(
        c,
        'φ' | 'R' | 't' | 'T' | 'L' | 'M' | 'C' | 'e' | '×' | 'x' | '±'
    )
}

/// Value of a core that holds commas, same rule as `reading()` in nf-server orders: a comma is a
/// thousands separator only when the first group has 1-3 ASCII digits without a leading 0 and every
/// later group has exactly 3 digits ("1,500", "12,345,678", "12,345.6"). Any other comma token gives
/// one reading, a decimal comma, flagged ambiguous ("1,5", "0,500", "1,50", "1,5000"); when even that
/// is not a number ("1,2,3") there is no value, still flagged. Returns (text, value, ambiguous).
fn comma_reading(core: &str) -> (String, Option<f64>, bool) {
    let (int_part, frac) = match core.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (core, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let groups: Vec<&str> = int_part.split(',').collect();
    let grouped = groups.len() > 1
        && digits(groups[0])
        && groups[0].len() <= 3
        && !groups[0].starts_with('0')
        && groups[1..].iter().all(|g| g.len() == 3 && digits(g))
        && !frac.is_some_and(|f| f.contains(','));
    if grouped {
        let t = core.replace(',', "");
        let v = t.parse::<f64>().ok();
        (t, v, false)
    } else {
        let t = core.replace(',', ".");
        let v = t.parse::<f64>().ok();
        (t, v, true)
    }
}

/// Clean and classify one token; None when it is not a number in the sense of this reader
/// (an optional sign or prefix mark, a decimal number, an optional unit mark; or a date/fraction like 1/1).
pub fn parse(raw: &str) -> Option<Parsed> {
    let mut s: Vec<char> = raw
        .chars()
        .map(|c| match c {
            'Φ' | 'Ø' | 'ø' | '⌀' | '∅' | 'Ф' => 'φ',
            '＋' => '+',
            '−' => '-',
            '．' => '.',
            c => c,
        })
        .collect();
    // A leading + or - right before a digit is a sign, not a stroke: it is kept.
    let signed = |s: &[char]| {
        matches!(s.first(), Some('+' | '-')) && s.get(1).is_some_and(char::is_ascii_digit)
    };
    // junk (arrow heads, dimension-line strokes) glued to either end
    while s.first().is_some_and(|c| JUNK.contains(*c) && *c != '±') && s.len() > 1 && !signed(&s) {
        s.remove(0);
    }
    while s.last().is_some_and(|c| JUNK.contains(*c)) && s.len() > 1 {
        s.pop();
    }
    // phi often comes back as a lower-case p
    if s.len() > 1 && s[0] == 'p' && s[1].is_ascii_digit() {
        s[0] = 'φ';
    }
    // A sign is believed only on a small decimal (a tolerance such as -0.5, +0.2). In front of a
    // larger number or an integer it is far more often an arrow head or dimension-line stroke
    // (measured: "-2915.4", "+9"); it is then removed, and the number is flagged ambiguous.
    let mut stroke_sign = false;
    if signed(&s) {
        let body: String = s[1..]
            .iter()
            .take_while(|c| c.is_ascii_digit() || **c == '.')
            .collect();
        let small_decimal =
            body.contains('.') && body.parse::<f64>().is_ok_and(|v| v < SIGN_KEPT_BELOW);
        if !small_decimal {
            s.remove(0);
            stroke_sign = true;
        }
    }
    let sign = if signed(&s) { Some(s[0]) } else { None };
    let head_len = match (sign, s.first()) {
        (Some(_), _) => 1,
        (None, Some(c)) if is_prefix(*c) => 1,
        _ => 0,
    };
    let pre = if sign.is_none() && head_len == 1 {
        Some(s[0])
    } else {
        None
    };
    let mut head: Vec<char> = s[..head_len].to_vec();
    let mut rest = &s[head_len..];
    if pre.is_some() && rest.first() == Some(&'=') {
        head.push('=');
        rest = &rest[1..];
    }
    // number core: digits with the separators a recogniser may produce
    let span_end = rest
        .iter()
        .position(|c| !(c.is_ascii_digit() || matches!(c, '.' | ',' | ':' | ';')))
        .unwrap_or(rest.len());
    let span: Vec<char> = rest[..span_end].to_vec();
    let tail: String = rest[span_end..].iter().collect();
    let tail_ok = matches!(
        tail.as_str(),
        "" | "°" | "枚" | "本" | "個" | "mm" | "pcs" | "kg" | "%"
    );
    let ends_ok = span.first().is_some_and(char::is_ascii_digit)
        && span.last().is_some_and(char::is_ascii_digit);
    let mut ambiguous = stroke_sign;
    let mut forced: Option<Option<f64>> = None;
    let mut core: Vec<char> = span.clone();
    if ends_ok && span.iter().any(|c| matches!(c, ':' | ';')) {
        // ':' / ';' becomes a decimal point only when it clearly is one: the only separator, 3 or
        // more digits before it (so not a scale 1:2 or a time 12:30), 1-2 digits after it.
        let seps: Vec<usize> = span
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_ascii_digit())
            .map(|(i, _)| i)
            .collect();
        if let [i] = seps[..] {
            if matches!(span[i], ':' | ';') && i >= 3 && span.len() - i - 1 <= 2 {
                core[i] = '.';
            }
        }
    } else if ends_ok && span.contains(&',') {
        let (t, v, amb) = comma_reading(&span.iter().collect::<String>());
        core = t.chars().collect();
        ambiguous |= amb;
        forced = Some(v);
    }
    let core_s: String = core.iter().collect();
    let numeric_core = ends_ok
        && core.iter().all(|c| c.is_ascii_digit() || *c == '.')
        && core_s.matches('.').count() <= 1;
    let (kind, value) = if forced == Some(None) && tail_ok {
        // comma token that is not a number at all ("1,2,3"): reported without a value
        (Kind::Number, None)
    } else if (numeric_core || forced.is_some()) && tail_ok && forced != Some(None) {
        let mut value: f64 = match forced {
            Some(Some(v)) => v,
            _ => core_s.parse().ok()?,
        };
        if sign == Some('-') {
            value = -value;
        }
        let kind = match (pre, sign, tail.as_str()) {
            (_, Some(_), _) => Kind::Tolerance,
            (Some('φ'), _, _) => Kind::Diameter,
            (Some('R'), _, _) => Kind::Radius,
            (Some('t' | 'T'), _, _) => Kind::Thickness,
            (Some('L'), _, _) => Kind::Length,
            (Some('M'), _, _) => Kind::Thread,
            (Some('e'), _, _) => Kind::Eccentricity,
            (Some('±'), _, _) => Kind::Tolerance,
            (Some('×' | 'x'), _, _) => Kind::Count,
            (_, _, "°") => Kind::Angle,
            (_, _, "枚" | "本" | "個" | "pcs") => Kind::Count,
            _ => Kind::Number,
        };
        (kind, Some(value))
    } else if pre.is_none() && sign.is_none() && is_date_or_fraction(&s) {
        // 1/1, 2026-08-06, and the ratio/time forms 1:2, 12:30: no value, never a plain number
        ambiguous |= s.iter().any(|c| matches!(c, ':' | ';'));
        (Kind::Number, None)
    } else {
        return None;
    };
    let text: String = head
        .iter()
        .chain(core.iter())
        .chain(tail.chars().collect::<Vec<_>>().iter())
        .collect();
    // fall back to the cleaned token when the core was not touched (date forms)
    let text = if numeric_core || forced.is_some() {
        text
    } else {
        s.iter().collect()
    };
    Some(Parsed {
        repaired: text != raw,
        text,
        value,
        kind,
        ambiguous,
    })
}

/// 1/1, 2026-08-06, 12:30 (digits groups joined by one kind of separator).
fn is_date_or_fraction(s: &[char]) -> bool {
    let sep = match s.iter().find(|c| !c.is_ascii_digit()) {
        Some(c @ ('/' | '-' | ':' | ';' | '.')) => *c,
        _ => return false,
    };
    let groups: Vec<&[char]> = s.split(|c| *c == sep).collect();
    groups.len() >= 2
        && groups.len() <= 3
        && groups
            .iter()
            .all(|g| !g.is_empty() && g.iter().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Option<(String, Option<f64>, Kind)> {
        parse(s).map(|x| (x.text, x.value, x.kind))
    }

    #[test]
    fn prefixes_and_kinds() {
        assert_eq!(
            p("φ944.0"),
            Some(("φ944.0".into(), Some(944.0), Kind::Diameter))
        );
        assert_eq!(p("Φ944.0").unwrap().0, "φ944.0");
        assert_eq!(p("R59.5").unwrap().2, Kind::Radius);
        assert_eq!(p("t3.2").unwrap().2, Kind::Thickness);
        assert_eq!(p("t=3.2").unwrap().1, Some(3.2));
        assert_eq!(p("L421.3").unwrap().2, Kind::Length);
        assert_eq!(p("±0.8").unwrap().2, Kind::Tolerance);
        assert_eq!(p("126.0°").unwrap().2, Kind::Angle);
        assert_eq!(p("12枚").unwrap().2, Kind::Count);
        assert_eq!(p("2162.5").unwrap().2, Kind::Number);
    }

    #[test]
    fn repairs() {
        assert_eq!(p("→591.00").unwrap().0, "591.00");
        assert_eq!(p("-L2924.8").unwrap().0, "L2924.8");
        assert_eq!(p("1489.0→").unwrap().0, "1489.0");
        assert_eq!(p("405,00").unwrap().0, "405.00");
        assert_eq!(p("1467:12").unwrap().0, "1467.12");
        assert_eq!(p("p285.0").unwrap().0, "φ285.0");
        assert!(parse("p285.0").unwrap().repaired);
        assert!(!parse("285.0").unwrap().repaired);
    }

    #[test]
    fn non_numbers() {
        assert_eq!(p("SS400"), None);
        assert_eq!(p("Is0"), None);
        assert_eq!(p("NFR-CS-DD32710C"), None);
        assert_eq!(p("0405-f"), None);
        assert_eq!(p("DEVELOPMENT:"), None);
        assert_eq!(p("."), None);
        assert_eq!(p(""), None);
    }

    #[test]
    fn multiplier_and_piece_marks() {
        assert_eq!(p("12本").unwrap().2, Kind::Count);
        assert_eq!(p("4枚").unwrap().1, Some(4.0));
        assert_eq!(p("×2").unwrap().2, Kind::Count);
        assert_eq!(p("R59.5").unwrap().1, Some(59.5));
        assert_eq!(p("φ944.0").unwrap().1, Some(944.0));
    }

    #[test]
    fn dates_and_fractions() {
        assert_eq!(p("1/1").unwrap().1, None);
        assert_eq!(p("2026-08-06").unwrap().0, "2026-08-06");
    }

    /// (text, value, kind, ambiguous) of a token, or None.
    fn full(s: &str) -> Option<(String, Option<f64>, Kind, bool)> {
        parse(s).map(|x| (x.text, x.value, x.kind, x.ambiguous))
    }

    #[test]
    fn scale_and_time_are_never_a_plain_number() {
        // ratio and time: no value, flagged; never 1.2 or 12.30
        assert_eq!(full("1:2"), Some(("1:2".into(), None, Kind::Number, true)));
        assert_eq!(
            full("12:30"),
            Some(("12:30".into(), None, Kind::Number, true))
        );
        assert_eq!(full("1;2").unwrap().1, None);
        // a clear decimal misread is still repaired (and marked repaired)
        assert_eq!(full("1467:12").unwrap().1, Some(1467.12));
        assert!(parse("1467:12").unwrap().repaired);
        assert_eq!(full("2372;5").unwrap().1, Some(2372.5));
    }

    #[test]
    fn signs_are_kept() {
        for (raw, v, text) in [
            ("-0.5", -0.5, "-0.5"),
            ("+0.2", 0.2, "+0.2"),
            ("\u{2212}0.5", -0.5, "-0.5"),
        ] {
            let x = parse(raw).unwrap();
            assert_eq!(x.value, Some(v), "{raw}");
            assert_eq!(x.kind, Kind::Tolerance, "{raw}");
            assert_eq!(x.text, text);
        }
        // a stroke in front of a big number or an integer is not a sign: removed, flagged
        let st = parse("-2915.4").unwrap();
        assert_eq!(
            (st.text.as_str(), st.value, st.kind),
            ("2915.4", Some(2915.4), Kind::Number)
        );
        assert!(st.ambiguous && st.repaired);
        assert_eq!(parse("+9").unwrap().value, Some(9.0));
        assert!(parse("+9").unwrap().ambiguous);
        // a stroke in front of a letter is still junk
        assert_eq!(p("-L2924.8").unwrap().2, Kind::Length);
    }

    #[test]
    fn comma_rule_same_as_orders_reading() {
        // thousands groups: value, not ambiguous
        assert_eq!(
            full("1,500"),
            Some(("1500".into(), Some(1500.0), Kind::Number, false))
        );
        assert_eq!(
            full("12,345,678"),
            Some(("12345678".into(), Some(12345678.0), Kind::Number, false))
        );
        assert_eq!(full("12,345.6").unwrap().1, Some(12345.6));
        // everything else: one decimal reading, flagged ambiguous, never dropped
        assert_eq!(
            full("0,500"),
            Some(("0.500".into(), Some(0.5), Kind::Number, true))
        );
        assert_eq!(
            full("12,5"),
            Some(("12.5".into(), Some(12.5), Kind::Number, true))
        );
        assert_eq!(
            full("1,50"),
            Some(("1.50".into(), Some(1.5), Kind::Number, true))
        );
        assert_eq!(
            full("1,5000"),
            Some(("1.5000".into(), Some(1.5), Kind::Number, true))
        );
        assert!(full("405,00").unwrap().3);
        assert!(full("1234,567").unwrap().3);
        // not a number at all, still reported (no value), flagged
        assert_eq!(
            full("1,2,3"),
            Some(("1.2.3".into(), None, Kind::Number, true))
        );
        // no comma: never ambiguous
        assert!(!full("12.5").unwrap().3);
    }

    #[test]
    fn multiplier_is_a_count() {
        for t in ["\u{d7}2", "x2"] {
            let x = parse(t).unwrap();
            assert_eq!((x.kind, x.value), (Kind::Count, Some(2.0)), "{t}");
        }
    }
}
