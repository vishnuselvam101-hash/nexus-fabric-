//! Seeded random mutation of valid files, for 30 seconds in total. Every input must give a document or
//! a refusal; a panic fails the test. The generator is a fixed-seed xorshift, so a failure replays.

use std::time::{Duration, Instant};

use nf_cadio::{Document, Limits};

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

const TOKENS: &[&[u8]] = &[
    b"\r\n",
    b"\n",
    b" ",
    b"\t",
    b"0",
    b"-1",
    b"1002",
    b"{",
    b"}",
    b"70",
    b"9",
    b"10",
    b"$INSUNITS",
    b"ENDSEC",
    b"SECTION",
    b"EOF",
    b"TABLE",
    b"ENDBLK",
    b"99999999999999999999",
    b"NaN",
    b"1e999",
    b"\0",
    b"\xff",
    b"999",
    b"ENTITIES",
    b"HEADER",
    b"  0\r\n",
    b"  2\r\n",
    b"4000000000",
    b"\r",
];

fn mutate(rng: &mut Rng, base: &[u8]) -> Vec<u8> {
    let mut out = base.to_vec();
    for _ in 0..1 + rng.below(6) {
        match rng.below(5) {
            0 if !out.is_empty() => {
                let i = rng.below(out.len());
                out[i] = rng.next_u64() as u8;
            }
            1 => {
                let at = rng.below(out.len() + 1);
                let token = TOKENS[rng.below(TOKENS.len())];
                out.splice(at..at, token.iter().copied());
            }
            2 if !out.is_empty() => {
                let a = rng.below(out.len());
                let b = (a + rng.below(16)).min(out.len());
                out.drain(a..b);
            }
            3 => {
                let n = rng.below(out.len() + 1);
                out.truncate(n);
            }
            4 if !out.is_empty() => {
                let a = rng.below(out.len());
                let b = (a + rng.below(32)).min(out.len());
                let chunk = out[a..b].to_vec();
                let at = rng.below(out.len() + 1);
                out.splice(at..at, chunk);
            }
            _ => {}
        }
    }
    out
}

fn corpus() -> Vec<Vec<u8>> {
    let r12 = "  0\r\nSECTION\r\n  2\r\nHEADER\r\n  9\r\n$ACADVER\r\n  1\r\nAC1009\r\n  9\r\n$INSUNITS\r\n 70\r\n4\r\n  0\r\nENDSEC\r\n  0\r\nSECTION\r\n  2\r\nTABLES\r\n  0\r\nTABLE\r\n  2\r\nLAYER\r\n 70\r\n1\r\n  0\r\nLAYER\r\n  2\r\nCUT\r\n 70\r\n0\r\n  0\r\nENDTAB\r\n  0\r\nENDSEC\r\n  0\r\nSECTION\r\n  2\r\nENTITIES\r\n  0\r\nLINE\r\n  8\r\nCUT\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n 11\r\n10.0\r\n 21\r\n5.0\r\n  0\r\nTEXT\r\n  8\r\nNOTE\r\n 10\r\n1.0\r\n 20\r\n2.0\r\n 40\r\n3.5\r\n  1\r\nPART 1\r\n  0\r\nENDSEC\r\n  0\r\nEOF\r\n";
    let r2000 = "999\nnote\n  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$DWGCODEPAGE\n  3\nANSI_932\n  9\n$EXTMIN\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nLWPOLYLINE\n 90\n3\n 10\n0.0\n 20\n0.0\n 10\n5.0\n 20\n0.0\n 42\n0.5\n  1002\n{\n1000\nx\n  1002\n}\n  0\nENDSEC\n  0\nEOF\n";
    vec![r12.as_bytes().to_vec(), r2000.as_bytes().to_vec()]
}

#[test]
fn random_mutations_never_panic_for_30_seconds() {
    let corpus = corpus();
    let small = Limits {
        max_bytes: 512,
        max_pairs: 64,
        max_entities: 3,
        max_depth: 1,
    };
    let started = Instant::now();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut iterations, mut accepted, mut refused) = (0u64, 0u64, 0u64);
    while started.elapsed() < Duration::from_secs(30) {
        let base = &corpus[rng.below(corpus.len())];
        let input = mutate(&mut rng, base);
        let limits = if rng.below(4) == 0 {
            small
        } else {
            Limits::default()
        };
        match Document::parse_with(&input, &limits) {
            Ok(doc) => {
                accepted += 1;
                let _ = doc.units().as_str();
                let _ = doc.sections.len();
            }
            Err(e) => {
                refused += 1;
                let _ = e.to_string();
            }
        }
        iterations += 1;
    }
    eprintln!("fuzz: {iterations} inputs, {accepted} accepted, {refused} refused, no panic");
    assert!(started.elapsed() >= Duration::from_secs(30));
    assert!(iterations > 1_000, "only {iterations} inputs");
    assert!(
        accepted > 0 && refused > 0,
        "accepted {accepted}, refused {refused}"
    );
}
