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

const JUNK: &str = "→←↑↓↔↕—–-_|/\\~·.,:;'\"`<>()[]{}*^=+";

fn is_prefix(c: char) -> bool {
    matches!(c, 'φ' | 'R' | 't' | 'T' | 'L' | 'M' | 'C' | 'e' | '×' | '±')
}

/// Clean and classify one token; None when it is not a number in the sense of this reader
/// (an optional prefix mark, a decimal number, an optional unit mark; or a date/fraction like 1/1).
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
    // junk (arrow heads, dimension-line strokes) glued to either end
    while s.first().is_some_and(|c| JUNK.contains(*c) && *c != '±') && s.len() > 1 {
        s.remove(0);
    }
    while s.last().is_some_and(|c| JUNK.contains(*c)) && s.len() > 1 {
        s.pop();
    }
    // phi often comes back as a lower-case p
    if s.len() > 1 && s[0] == 'p' && s[1].is_ascii_digit() {
        s[0] = 'φ';
    }
    // decimal mark misread as , : ; between digits (one or two decimals), or leading "0,"
    for i in 1..s.len().saturating_sub(1) {
        if matches!(s[i], ',' | ':' | ';') && s[i - 1].is_ascii_digit() && s[i + 1].is_ascii_digit()
        {
            let after = s[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if s[i] != ',' || after <= 2 || (i == 1 && s[0] == '0') {
                s[i] = '.';
            }
        }
    }
    let text: String = s.iter().collect();
    let (pre, rest) = match s.first() {
        Some(c) if is_prefix(*c) => (Some(*c), &s[1..]),
        _ => (None, &s[..]),
    };
    let rest = if pre.is_some() && rest.first() == Some(&'=') {
        &rest[1..]
    } else {
        rest
    };
    // number core
    let digits_end = rest
        .iter()
        .position(|c| !(c.is_ascii_digit() || *c == '.'))
        .unwrap_or(rest.len());
    let core: String = rest[..digits_end].iter().collect();
    let tail: String = rest[digits_end..].iter().collect();
    let numeric_core = !core.is_empty()
        && core.chars().next().is_some_and(|c| c.is_ascii_digit())
        && core.chars().last().is_some_and(|c| c.is_ascii_digit())
        && core.matches('.').count() <= 1;
    let tail_ok = matches!(
        tail.as_str(),
        "" | "°" | "枚" | "本" | "個" | "mm" | "pcs" | "kg" | "%"
    );
    let (kind, value) = if numeric_core && tail_ok {
        let value: f64 = core.parse().ok()?;
        let kind = match (pre, tail.as_str()) {
            (Some('φ'), _) => Kind::Diameter,
            (Some('R'), _) => Kind::Radius,
            (Some('t' | 'T'), _) => Kind::Thickness,
            (Some('L'), _) => Kind::Length,
            (Some('M'), _) => Kind::Thread,
            (Some('e'), _) => Kind::Eccentricity,
            (Some('±'), _) => Kind::Tolerance,
            (_, "°") => Kind::Angle,
            (_, "枚" | "本" | "個" | "pcs") => Kind::Count,
            _ => Kind::Number,
        };
        (kind, Some(value))
    } else if pre.is_none() && is_date_or_fraction(&s) {
        (Kind::Number, None)
    } else {
        return None;
    };
    Some(Parsed {
        repaired: text != raw,
        text,
        value,
        kind,
    })
}

/// 1/1, 2026-08-06, 12:30 (digits groups joined by one kind of separator).
fn is_date_or_fraction(s: &[char]) -> bool {
    let sep = match s.iter().find(|c| !c.is_ascii_digit()) {
        Some(c @ ('/' | '-' | ':' | '.')) => *c,
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
        assert_eq!(p("×2").unwrap().2, Kind::Number);
        assert_eq!(p("R59.5").unwrap().1, Some(59.5));
        assert_eq!(p("φ944.0").unwrap().1, Some(944.0));
    }

    #[test]
    fn dates_and_fractions() {
        assert_eq!(p("1/1").unwrap().1, None);
        assert_eq!(p("2026-08-06").unwrap().0, "2026-08-06");
    }
}
