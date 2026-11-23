//! Independence of the verifier from the development engine (work plan task 1.5, RC-15).
//!
//! Two guards:
//! 1. The non-test code of the verifier's modules (`verify`, `independent`, `curve_bound`, `geometry`)
//!    contains no import from and no call into `develop`: the code that checks a development must not
//!    share code with the code that produces it. Test-only imports are exempt (tests may compare the
//!    two implementations against each other).
//! 2. The verifier's re-derived engine-error bounds (`verify::engine_*_error_bound`) agree BIT FOR BIT
//!    with the engine's own bounds (`develop::*_error_bound`) on seeded parameter sets, including
//!    near-tangent, shallow and obtuse geometries. The two copies are independent code restating the
//!    same first-order analysis of the same documented evaluation sequence (Spec 02 eq. 4.3 and the
//!    header chart); if either drifts, the verification tolerances and the reported eps_num would
//!    silently disagree. Derivation: docs/backend/TOLERANCE_POLICY.md sections 8.2 and 11.

use uem::develop;
use uem::geometry::Branch;
use uem::verify::{engine_opening_error_bound, engine_s_error_bound, engine_saddle_error_bound};

const GUARDED: &[(&str, &str)] = &[
    ("verify.rs", include_str!("../src/verify.rs")),
    ("independent.rs", include_str!("../src/independent.rs")),
    ("curve_bound.rs", include_str!("../src/curve_bound.rs")),
    ("geometry.rs", include_str!("../src/geometry.rs")),
];

/// Everything before the file's `#[cfg(test)] mod tests` item. A file without that marker is taken
/// whole; a file that has `#[cfg(test)]` but a different marker fails the guard, so a refactor of the
/// test module cannot silently exempt the whole file.
fn non_test_part(src: &str) -> &str {
    if src.contains("#[cfg(test)]") {
        let marker = "#[cfg(test)]\nmod tests";
        let i = src
            .find(marker)
            .expect("test module marker changed; update verifier_independence.rs");
        &src[..i]
    } else {
        src
    }
}

fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `develop` as a whole word (not inside `developed`, `geodesic_development`, ...).
fn word_develop(s: &str) -> bool {
    let b = s.as_bytes();
    for i in 0..b.len().saturating_sub(6) {
        if &b[i..i + 7] == b"develop"
            && (i == 0 || !is_word_byte(b[i - 1]))
            && (i + 7 == b.len() || !is_word_byte(b[i + 7]))
        {
            return true;
        }
    }
    false
}

#[test]
fn verifier_code_does_not_import_develop() {
    for (name, src) in GUARDED {
        for (i, line) in non_test_part(src).lines().enumerate() {
            let import = line.trim_start().starts_with("use ") && word_develop(line);
            assert!(
                !line.contains("develop::") && !line.contains("crate::develop") && !import,
                "{name}:{}: verifier code references the development engine: {line}",
                i + 1
            );
        }
    }
}

/// xorshift64* generator with a fixed seed: the same parameter sets on every run
/// (the generator of tests/property_bounds.rs).
struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }
}

/// Branch geometry drawn like tests/property_bounds.rs: near tangency, shallow and obtuse angles
/// often; |delta| + r < R so the square root of the bound stays real.
fn random_branch(rng: &mut Rng) -> Branch {
    let big_r = 10f64.powf(rng.range(0.0, 3.0));
    let r = if rng.unit() < 0.4 {
        big_r * (1.0 - 10f64.powf(-rng.range(2.0, 5.5)))
    } else {
        big_r * rng.range(0.02, 0.98)
    };
    let beta_deg = match rng.unit() {
        x if x < 0.3 => 10f64.powf(rng.range(-2.5, 0.0)),
        x if x < 0.6 => 180.0 - 10f64.powf(rng.range(-2.5, 0.0)),
        _ => rng.range(5.0, 175.0),
    };
    let room = big_r * (1.0 - 1e-6) - r;
    let delta = if rng.unit() < 0.5 {
        0.0
    } else {
        0.999 * room * rng.range(-1.0, 1.0)
    };
    Branch {
        big_r,
        r,
        beta: beta_deg.to_radians(),
        delta,
    }
}

fn assert_same(what: &str, b: &Branch, verifier: f64, engine: f64) {
    assert_eq!(
        verifier.to_bits(),
        engine.to_bits(),
        "{what} disagrees: verifier {verifier:e} vs engine {engine:e} for \
         R={} r={} beta_deg={} delta={}",
        b.big_r,
        b.r,
        b.beta.to_degrees(),
        b.delta
    );
}

#[test]
fn verifier_bounds_match_engine_bounds_bit_for_bit() {
    let mut rng = Rng(0x4E46_5243_3135_0001);
    for _ in 0..2000 {
        let b = random_branch(&mut rng);
        let r_dev = b.r * rng.range(0.9, 1.1);
        let r_dev_header = b.big_r * rng.range(0.9, 1.1);
        let station = rng.range(-2.0, 2.0) * b.big_r;
        assert_same(
            "s_error_bound",
            &b,
            engine_s_error_bound(&b),
            develop::s_error_bound(&b),
        );
        assert_same(
            "saddle_error_bound",
            &b,
            engine_saddle_error_bound(&b, r_dev),
            develop::saddle_error_bound(&b, r_dev),
        );
        assert_same(
            "opening_error_bound",
            &b,
            engine_opening_error_bound(&b, r_dev_header, station),
            develop::opening_error_bound(&b, r_dev_header, station),
        );
    }
}
