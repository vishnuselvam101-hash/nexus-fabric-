//! Mutation and generation fuzzer for the converter.
//!
//! Deterministic: a fixed seed, so a failing case number reproduces. Every case runs
//! under `catch_unwind` and is timed. The release default is 200,000 cases; debug
//! builds run fewer. `NF_PDF_FUZZ_CASES` overrides the count.
//!
//! Generators: (1) byte mutations of valid files (bit flips, byte edits, token
//! insertion, deletion, duplication, truncation, splices from other seeds); (2)
//! random content streams built from operator and operand tokens, including huge
//! and NaN-like numbers; (3) deep nesting of arrays, dictionaries and q.

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use common::*;
use nf_pdf::{convert, FailureKind, Options, Scale};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    fn chance(&mut self, per_mille: u64) -> bool {
        self.next() % 1000 < per_mille
    }
}

const OPS: &[&str] = &[
    "m", "l", "c", "v", "y", "re", "h", "S", "s", "f", "f*", "F", "B", "B*", "b", "b*", "n", "W",
    "W*", "q", "Q", "cm", "d", "gs", "BT", "ET", "Tf", "Td", "TD", "Tm", "T*", "Tj", "TJ", "'",
    "\"", "Tc", "Tw", "Tz", "TL", "Tr", "Ts", "Do", "BI", "ID", "EI", "BDC", "BMC", "EMC", "sh",
    "scn", "SCN", "g", "rg", "k", "w", "J", "j", "M", "i", "d0", "d1", "BX", "EX", "xyz", "0", "-",
    "+",
];

const TOKENS: &[&str] = &[
    "(abc)",
    "(a\\)b)",
    "(\\101\\)",
    "<4142>",
    "<41 4",
    "[1 2 (x)]",
    "[[[1]]]",
    "<< /K 1 >>",
    "/F1",
    "/Fm1",
    "/Im1",
    "/GS1",
    "/Pattern",
    "/OC",
    "true",
    "false",
    "null",
    "1e308",
    "-1e308",
    "NaN",
    "inf",
    "-0",
    "0.0000000001",
    "99999999999999999999999999999999999999999",
    "--1",
    "+.5",
    "1.2.3",
    ".",
    "%comment\n",
    "{",
    "}",
    "\\",
    "R",
    "1 0 R",
    "endobj",
    "stream",
    "\u{0}",
    "\u{7f}",
    "\u{e9}",
];

fn number(rng: &mut Rng) -> String {
    match rng.below(10) {
        0 => format!("{}", rng.below(300)),
        1 => format!("{}.{}", rng.below(100), rng.below(1000)),
        2 => "1e308".into(),
        3 => "-99999999999999999999999999999".into(),
        4 => "-0".into(),
        5 => "NaN".into(),
        6 => format!("{}", (rng.next() % 1_000_000_000) as f64 / 7.0),
        7 => format!("-{}.{}", rng.below(50), rng.below(10_000)),
        _ => format!("{}", rng.below(20) as i64 - 10),
    }
}

/// A content stream of random operators and operands.
fn random_content(rng: &mut Rng) -> Vec<u8> {
    let n = 1 + rng.below(90);
    let mut out = String::new();
    for _ in 0..n {
        match rng.below(10) {
            0..=4 => {
                out.push_str(&number(rng));
                out.push(' ');
            }
            5..=7 => {
                out.push_str(OPS[rng.below(OPS.len())]);
                out.push('\n');
            }
            8 => {
                out.push_str(TOKENS[rng.below(TOKENS.len())]);
                out.push(' ');
            }
            _ => {
                out.push_str(&format!("{} {} ", number(rng), number(rng)));
            }
        }
    }
    out.into_bytes()
}

/// Page with resources the content may refer to.
fn fuzz_page(content: &[u8], flate: bool) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog = pdf.reserve();
    let pages = pdf.reserve();
    let page = pdf.reserve();
    let font = pdf.add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding /FirstChar 32 /LastChar 126 /Widths [278 278 355] >>");
    let form = form_xobject(
        &mut pdf,
        [0.0, 0.0, 100.0, 100.0],
        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        b"0 0 m 5 5 l S",
        "",
    );
    let image = pdf.stream(
        "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
        &[0u8],
        false,
    );
    let gs = pdf.add("<< /Type /ExtGState /D [[2 2] 0] >>");
    let content_num = pdf.stream("", content, flate);
    pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
    pdf.set(
        pages,
        format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>"),
    );
    pdf.set(
        page,
        format!(
            "<< /Type /Page /Parent {pages} 0 R /MediaBox [0 0 200 200] /Contents {content_num} 0 R /Resources << /Font << /F1 {font} 0 R >> /XObject << /Fm1 {form} 0 R /Im1 {image} 0 R >> /ExtGState << /GS1 {gs} 0 R >> >> >>"
        ),
    );
    pdf.classic(catalog)
}

fn seeds() -> Vec<Vec<u8>> {
    let mut out = vec![page_pdf(&PageSpec::new(b"10 20 m 110 20 l S"))];
    out.push(page_pdf(&PageSpec::new(
        b"0 0 m 0 100 100 100 100 0 c 10 10 40 30 re f [3 2] 0 d 0 50 m 90 50 l S",
    )));
    out.push(fuzz_page(
        b"BT /F1 12 Tf 10 20 Td [(A) -120 (B)] TJ ET 1 0 0 1 5 5 cm /Fm1 Do q /GS1 gs Q",
        false,
    ));
    out.push(fuzz_page(
        b"q 20 0 0 20 0 0 cm /Im1 Do Q 0 0 m 1 1 l S",
        true,
    ));
    let mut spec = PageSpec::new(b"0 0 m 3 4 l S");
    spec.flate = true;
    out.push(page_pdf(&spec));
    out.push(page_pdf_xref_stream(&PageSpec::new(b"5 5 m 60 5 l S")));
    out.push(page_pdf_objstm(&PageSpec::new(b"5 5 m 60 5 l S")));
    out
}

fn mutate(rng: &mut Rng, base: &[u8], other: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    let rounds = 1 + rng.below(6);
    for _ in 0..rounds {
        if v.is_empty() {
            v.push(b'%');
        }
        match rng.below(8) {
            0 => {
                let i = rng.below(v.len());
                if let Some(b) = v.get_mut(i) {
                    *b ^= 1 << rng.below(8);
                }
            }
            1 => {
                let i = rng.below(v.len());
                let specials = b"()<>[]{}/%0123456789-+.\x00\xff";
                if let Some(b) = v.get_mut(i) {
                    *b = specials[rng.below(specials.len())];
                }
            }
            2 => {
                let i = rng.below(v.len() + 1);
                let t = TOKENS[rng.below(TOKENS.len())].as_bytes().to_vec();
                v.splice(i..i, t);
            }
            3 => {
                let i = rng.below(v.len());
                let n = 1 + rng.below(64);
                let end = (i + n).min(v.len());
                v.drain(i..end);
            }
            4 => {
                let i = rng.below(v.len());
                let n = 1 + rng.below(128);
                let end = (i + n).min(v.len());
                let chunk = v.get(i..end).unwrap_or(&[]).to_vec();
                let at = rng.below(v.len() + 1);
                v.splice(at..at, chunk);
            }
            5 => {
                let keep = rng.below(v.len() + 1);
                v.truncate(keep);
            }
            6 => {
                if !other.is_empty() {
                    let a = rng.below(other.len());
                    let n = 1 + rng.below(256);
                    let chunk = other
                        .get(a..(a + n).min(other.len()))
                        .unwrap_or(&[])
                        .to_vec();
                    let at = rng.below(v.len() + 1);
                    v.splice(at..at, chunk);
                }
            }
            _ => {
                let i = rng.below(v.len());
                let needle = b"obj";
                if let Some(pos) = v
                    .get(i..)
                    .and_then(|s| s.windows(3).position(|w| w == needle))
                {
                    v.drain(i + pos..(i + pos + 3).min(v.len()));
                }
            }
        }
    }
    v
}

/// Nested structures of a given depth.
fn deep_nesting(rng: &mut Rng) -> Vec<u8> {
    let depth = 1 + rng.below(3000);
    let (open, close): (&str, &str) = match rng.below(4) {
        0 => ("[", "]"),
        1 => ("<< /K ", " >>"),
        2 => ("q ", "Q "),
        _ => ("(", ")"),
    };
    let mut s = open.repeat(depth);
    s.push_str(&close.repeat(depth));
    s.push_str(" 0 0 m 1 1 l S");
    s.into_bytes()
}

struct Stats {
    cases: usize,
    converted: usize,
    refused: usize,
    limit: usize,
    panics: usize,
    slow: usize,
    slowest: Duration,
    slowest_case: usize,
    codes: std::collections::BTreeMap<&'static str, usize>,
}

fn default_cases() -> usize {
    if let Some(n) = std::env::var("NF_PDF_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return n;
    }
    if cfg!(debug_assertions) {
        3_000
    } else {
        200_000
    }
}

#[test]
fn fuzz_never_panics_and_every_case_is_bounded() {
    let cases = default_cases();
    let bound = if cfg!(debug_assertions) {
        Duration::from_secs(5)
    } else {
        Duration::from_secs(1)
    };
    let seeds = seeds();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut st = Stats {
        cases: 0,
        converted: 0,
        refused: 0,
        limit: 0,
        panics: 0,
        slow: 0,
        slowest: Duration::ZERO,
        slowest_case: 0,
        codes: Default::default(),
    };
    let mut opts = Options::new(Scale::MmPerPoint(1.0));
    opts.time_limit = Duration::from_secs(2);
    let mut worst_panic_case = None;
    for case in 0..cases {
        let bytes = match case % 10 {
            0..=4 => {
                let base = &seeds[rng.below(seeds.len())];
                let other = &seeds[rng.below(seeds.len())];
                mutate(&mut rng, base, other)
            }
            5..=7 => fuzz_page(&random_content(&mut rng), rng.chance(300)),
            8 => fuzz_page(&deep_nesting(&mut rng), false),
            _ => {
                let base = &seeds[rng.below(seeds.len())];
                let mut m = base.clone();
                m.truncate(rng.below(base.len() + 1));
                m
            }
        };
        let started = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| {
            convert(&bytes, &opts)
                .map(|_| ())
                .map_err(|f| (f.kind, f.code))
        }));
        let took = started.elapsed();
        st.cases += 1;
        if took > st.slowest {
            st.slowest = took;
            st.slowest_case = case;
        }
        if took > bound {
            st.slow += 1;
        }
        match result {
            Err(_) => {
                st.panics += 1;
                worst_panic_case.get_or_insert(case);
            }
            Ok(Ok(())) => st.converted += 1,
            Ok(Err((kind, code))) => {
                *st.codes.entry(code).or_insert(0) += 1;
                match kind {
                    FailureKind::Refused => st.refused += 1,
                    FailureKind::LimitExceeded => st.limit += 1,
                }
            }
        }
    }
    println!(
        "fuzz: cases={} converted={} refused={} limit_exceeded={} panics={} over_bound={} slowest={:.3} ms (case {}) bound={} ms",
        st.cases,
        st.converted,
        st.refused,
        st.limit,
        st.panics,
        st.slow,
        st.slowest.as_secs_f64() * 1e3,
        st.slowest_case,
        bound.as_millis()
    );
    println!("fuzz: outcome codes {:?}", st.codes);
    assert_eq!(
        st.panics, 0,
        "a case panicked; first case number {:?}",
        worst_panic_case
    );
    assert_eq!(st.slow, 0, "cases over the per-case bound");
}
