//! Work plan task 1.2 (docs/backend/TOLERANCE_POLICY.md section 8): property tests of the certified representation
//! error `eps_rep`. For seeded random parameter sets per family (near-tangent, shallow and obtuse angles included) the
//! drawn polyline is compared with the exact curve, which is evaluated here from independently written closed forms
//! (not from the engine's bound code). Assertion: the measured chord error never exceeds the reported bound.
//! The largest ratio measured / bound is printed per family.
//!
//! Two tiers (work plan task 1.2 review, item 3). The FAST tier (`*_fast` tests, not ignored) runs 40 seeded
//! parameter sets per family in debug mode, so it is part of every `cargo test -p uem`. The FULL tier (2 000 sets
//! per family, `#[ignore]`) is run in release mode, as
//! `nf-sandbox cargo test --release -p uem --test property_bounds -- --include-ignored --nocapture`.
//! The 40-digit check of the same cases is `tests/bound_hp_check.py` (needs mpmath; NOT run on this host, see
//! TOLERANCE_POLICY 8.6).
//! The measured chord error is sampled OFF the bracket grid (item 2): at irrational fractions of each segment and at
//! the midpoints of the sixteenths, where the M·h²/8 term of the bound is actually attained.
//! The closed forms below restate the definitions of the curves (Spec 02 §4.3, §4.4); they are not the bound code.

use std::f64::consts::PI;
use uem::develop::{
    Branch, EPS_REP_UNCERTIFIED, branch_stub, eccentric_reducer, header_with_opening,
};
use uem::{Geom, P2, Pattern};

/// Parameter sets per family in the full tier (release, `--include-ignored`).
const CASES: usize = 2000;
/// Parameter sets per family in the fast tier (debug, every test run).
const FAST_CASES: usize = 40;
const TAU: f64 = 2.0 * PI;
/// Golden-ratio conjugate: the low-discrepancy sequence of the sample offsets is frac(1/2 + j·GOLD).
const GOLD: f64 = 0.618_033_988_749_894_9;

/// Offsets u in (0, 1) of the samples inside one segment, as fractions of its parameter width. The first 15 are
/// irrational (never a multiple of 1/16); the last 16 are the midpoints of the sixteenths, shifted by half a step
/// (odd multiples of 1/32), where the error of the linear interpolation is largest.
fn sample_offsets() -> Vec<f64> {
    let mut v: Vec<f64> = (1..=15).map(|j| (0.5 + j as f64 * GOLD).fract()).collect();
    v.extend((0..16).map(|k| (k as f64 + 0.5) / 16.0));
    v
}

/// xorshift64* generator with a fixed seed: the same parameter sets on every run.
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

/// Distance from q to the closed segment [a, b].
fn dist_segment(q: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((q.x - a.x) * dx + (q.y - a.y) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    q.dist(P2::new(a.x + t * dx, a.y + t * dy))
}

/// Branch geometry: header radius R, branch radius r, angle beta (deg), offset delta, drawn with |delta| + r < R.
/// Near tangency (r close to R), shallow (beta near 0) and obtuse (beta near 180) angles are drawn often.
fn random_branch(rng: &mut Rng) -> (f64, f64, f64, f64) {
    let big_r = 10f64.powf(rng.range(0.0, 3.0));
    let r = if rng.unit() < 0.4 {
        big_r * (1.0 - 10f64.powf(-rng.range(2.0, 5.5)))
    } else {
        big_r * rng.range(0.02, 0.98)
    };
    let beta = match rng.unit() {
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
    (big_r, r, beta, delta)
}

/// Largest axial extent |s| of the saddle: |s| <= (R + r |cos beta|) / sin beta (the bound used by the header).
fn axial_extent(big_r: f64, r: f64, beta_deg: f64) -> f64 {
    let b = beta_deg.to_radians();
    (big_r + r * b.cos().abs()) / b.sin()
}

/// Indices of the segments to measure: both ends and the knee at phi = 0 and phi = pi, plus random segments.
fn chosen_segments(rng: &mut Rng, n: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (0..30.min(n)).collect();
    v.extend(n.saturating_sub(30)..n);
    v.extend((n / 2).saturating_sub(30)..(n / 2 + 30).min(n));
    for _ in 0..60 {
        v.push((rng.unit() * n as f64) as usize % n);
    }
    v
}

/// Measured chord error of a polyline of `n` segments whose exact curve is `exact(phi)`, with the vertices
/// at phi_i = i·dphi. Sixteen samples per measured segment; returns the largest distance to the drawn chord.
fn measured_chord_error(
    drawn: &[P2],
    n: usize,
    segments: &[usize],
    exact: &dyn Fn(f64) -> P2,
) -> f64 {
    let dphi = TAU / n as f64;
    let offsets = sample_offsets();
    let mut worst: f64 = 0.0;
    for &i in segments {
        let (a, b) = (drawn[i], drawn[i + 1]);
        for &u in &offsets {
            let phi = (i as f64 + u) * dphi;
            worst = worst.max(dist_segment(exact(phi), a, b));
        }
    }
    worst
}

/// NF_BOUND_DUMP=<file>: append, for the first 400 certified cases of each family, the drawn vertices of 24 chosen
/// segments with the bounds, for the 40-digit check `tests/bound_hp_check.py` (not run unless the variable is set).
fn dump(
    family: &str,
    head: &str,
    n: usize,
    eps_rep: f64,
    eps_num: f64,
    segs: &[usize],
    drawn: &[P2],
) {
    if let Some(path) = std::env::var_os("NF_BOUND_DUMP") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        let mut line = format!("{family} {head} {n} {eps_rep:e} {eps_num:e}");
        for &i in segs.iter().take(24) {
            line.push_str(&format!(
                " {i} {:e} {:e} {:e} {:e}",
                drawn[i].x,
                drawn[i].y,
                drawn[i + 1].x,
                drawn[i + 1].y
            ));
        }
        writeln!(f, "{line}").unwrap();
    }
}

fn drawn_points(p: &Pattern, name: &str) -> Vec<P2> {
    p.outline
        .iter()
        .chain(p.holes.iter())
        .find(|c| c.name == name)
        .and_then(|c| match &c.geom {
            Geom::Polyline { pts, .. } => Some(pts.clone()),
            _ => None,
        })
        .expect("polyline curve")
}

#[derive(Default)]
struct Tally {
    accepted: usize,
    refused: usize,
    uncertified: usize,
    measured: usize,
    worst_ratio: f64,
}

fn check_ratio(t: &mut Tally, measured: f64, bound: f64, what: &str) {
    let ratio = measured / bound;
    t.worst_ratio = t.worst_ratio.max(ratio);
    assert!(
        measured <= bound + 1e-12 * (1.0 + bound),
        "{what}: measured chord error {measured:e} exceeds the certified bound {bound:e}"
    );
    t.measured += 1;
}

#[test]
fn stub_certified_chord_bound_covers_the_measured_error_fast() {
    stub_family(FAST_CASES);
}

#[test]
#[ignore = "full tier: run in release with --include-ignored (see the header)"]
fn stub_certified_chord_bound_covers_the_measured_error() {
    stub_family(CASES);
}

fn stub_family(cases: usize) {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut t = Tally::default();
    for _ in 0..cases {
        let (big_r, r, beta, delta) = random_branch(&mut rng);
        let s_end = 2.0 * axial_extent(big_r, r, beta) + big_r;
        let p = match branch_stub(big_r, r, r, beta, delta, s_end) {
            Ok(p) => p,
            Err(_) => {
                t.refused += 1;
                continue;
            }
        };
        t.accepted += 1;
        if !p.eps_rep_certified {
            t.uncertified += 1;
            continue;
        }
        let n = p.quantity("polyline_segments").unwrap() as usize;
        let drawn = drawn_points(&p, "saddle_cut");
        let b = beta.to_radians();
        let exact = |phi: f64| {
            let y = delta + r * phi.cos();
            let s = ((big_r * big_r - y * y).sqrt() - r * phi.sin() * b.cos()) / b.sin();
            P2::new(r * phi, s)
        };
        let segs = chosen_segments(&mut rng, n);
        let m = measured_chord_error(&drawn, n, &segs, &exact);
        if t.measured < 400 {
            dump(
                "stub",
                &format!("{big_r:e} {r:e} {beta:e} {delta:e}"),
                n,
                p.eps_rep,
                p.eps_num,
                &segs,
                &drawn,
            );
        }
        check_ratio(
            &mut t,
            m,
            p.eps_rep,
            &format!("stub R {big_r} r {r} beta {beta} delta {delta}"),
        );
    }
    eprintln!(
        "PROPERTY stub: {} sets, accepted {}, refused {}, certified-measured {}, uncertified {}, largest measured/bound {:.4}",
        cases, t.accepted, t.refused, t.measured, t.uncertified, t.worst_ratio
    );
    assert!(
        t.measured > cases / 2,
        "too few certified cases to be a test"
    );
}

#[test]
fn opening_certified_chord_bound_covers_the_measured_error_fast() {
    opening_family(FAST_CASES);
}

#[test]
#[ignore = "full tier: run in release with --include-ignored (see the header)"]
fn opening_certified_chord_bound_covers_the_measured_error() {
    opening_family(CASES);
}

fn opening_family(cases: usize) {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let mut t = Tally::default();
    for _ in 0..cases {
        let (big_r, r, beta, delta) = random_branch(&mut rng);
        let b = Branch {
            big_r,
            r,
            beta: beta.to_radians(),
            delta,
        };
        let ext = axial_extent(big_r, r, beta);
        let station = ext + r + big_r;
        let length = 2.0 * station;
        let p = match header_with_opening(&b, big_r, length, station) {
            Ok(p) => p,
            Err(_) => {
                t.refused += 1;
                continue;
            }
        };
        t.accepted += 1;
        if !p.eps_rep_certified {
            t.uncertified += 1;
            continue;
        }
        let n = p.quantity("polyline_segments").unwrap() as usize;
        let (sb, cb) = b.beta.sin_cos();
        let exact = |phi: f64| {
            let y = delta + r * phi.cos();
            let s = ((big_r * big_r - y * y).sqrt() - r * phi.sin() * cb) / sb;
            P2::new(
                station + s * cb - r * phi.sin() * sb,
                big_r * ((y / big_r).asin() + PI),
            )
        };
        // develop stores a clockwise loop, which may be the reverse of the parameter order: orient it first
        let mut drawn = drawn_points(&p, "branch_opening");
        let first = exact(TAU / n as f64);
        if drawn[1].dist(first) > drawn[n - 1].dist(first) {
            drawn.reverse();
        }
        let segs = chosen_segments(&mut rng, n);
        let m = measured_chord_error(&drawn, n, &segs, &exact);
        if t.measured < 400 {
            dump(
                "opening",
                &format!("{big_r:e} {r:e} {beta:e} {delta:e} {station:e}"),
                n,
                p.eps_rep,
                p.eps_num,
                &segs,
                &drawn,
            );
        }
        check_ratio(
            &mut t,
            m,
            p.eps_rep,
            &format!("opening R {big_r} r {r} beta {beta} delta {delta}"),
        );
    }
    eprintln!(
        "PROPERTY opening: {} sets, accepted {}, refused {}, certified-measured {}, uncertified {}, largest measured/bound {:.4}",
        cases, t.accepted, t.refused, t.measured, t.uncertified, t.worst_ratio
    );
    assert!(
        t.measured > cases / 2,
        "too few certified cases to be a test"
    );
}

/// Four-point Gauss–Legendre rule on [a, b] (the rule the reducer's psi is integrated with).
fn gauss4(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    const GL: [(f64, f64); 4] = [
        (-0.861_136_311_594_05, 0.347_854_845_137_45),
        (-0.339_981_043_584_86, 0.652_145_154_862_55),
        (0.339_981_043_584_86, 0.652_145_154_862_55),
        (0.861_136_311_594_05, 0.347_854_845_137_45),
    ];
    let mid = 0.5 * (a + b);
    let half = 0.5 * (b - a);
    GL.iter().map(|&(x, w)| w * f(mid + half * x)).sum::<f64>() * half
}

#[test]
fn reducer_certified_chord_bound_covers_the_measured_error_fast() {
    reducer_family(FAST_CASES);
}

#[test]
#[ignore = "full tier: run in release with --include-ignored (see the header)"]
fn reducer_certified_chord_bound_covers_the_measured_error() {
    reducer_family(CASES);
}

fn reducer_family(cases: usize) {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut t = Tally::default();
    for _ in 0..cases {
        let r1 = 10f64.powf(rng.range(0.0, 2.6));
        let r2 = r1 * rng.range(0.05, 0.95);
        let h = 10f64.powf(rng.range(-1.0, 2.5));
        let e = if rng.unit() < 0.3 {
            0.0
        } else {
            rng.range(0.0, 0.9) * (r1 - r2)
        };
        let p = match eccentric_reducer(r1, r2, h, e) {
            Ok(p) => p,
            Err(_) => {
                t.refused += 1;
                continue;
            }
        };
        t.accepted += 1;
        // The reducer's reported value is NOT certified (its vertex term carries the quadrature estimate, develop.rs
        // EPS_NUM_BASIS_CONE), so the certification flag cannot select these cases. The test checks the value
        // empirically instead; only a value whose chord bound could not be proved (no bound at all) is skipped.
        if p.eps_rep_basis == EPS_REP_UNCERTIFIED {
            t.uncertified += 1;
            continue;
        }
        // the exact large end, independently: rho(theta) from the apex, psi by a 4-point Gauss rule per panel
        let k = r1 / (r1 - r2);
        let (a, hh) = (e * k, h * k);
        let steps = 16 * p.quantity("polyline_segments").unwrap() as usize;
        let dth = TAU / steps as f64;
        let dpsi = |th: f64| {
            let x = r1 - a * th.cos();
            let rho2 = (r1 - a) * (r1 - a) + hh * hh + 2.0 * a * r1 * (1.0 - th.cos());
            r1 * (hh * hh + x * x).sqrt() / rho2
        };
        let mut psi = vec![0.0; steps + 1];
        for i in 0..steps {
            psi[i + 1] = psi[i] + gauss4(&dpsi, i as f64 * dth, (i + 1) as f64 * dth);
        }
        let rho =
            |th: f64| ((r1 - a) * (r1 - a) + hh * hh + 2.0 * a * r1 * (1.0 - th.cos())).sqrt();
        let drawn = match &p.outline[0].geom {
            Geom::Polyline { pts, .. } => pts.clone(),
            _ => panic!("large end is a polyline"),
        };
        // off-grid samples (item 2): theta = (i + u)·2pi/n, psi from the nearest grid point below it plus a partial
        // Gauss panel, so no sample lies on the grid of the integration
        let n = p.quantity("polyline_segments").unwrap() as usize;
        let segs = chosen_segments(&mut rng, n);
        let offsets = sample_offsets();
        let mut m: f64 = 0.0;
        for &i in &segs {
            let (u0, u1) = (drawn[i], drawn[i + 1]);
            for &u in &offsets {
                let theta = (i as f64 + u) * TAU / n as f64;
                let j = ((theta / dth).floor() as usize).min(steps);
                let psi_theta = psi[j] + gauss4(&dpsi, j as f64 * dth, theta);
                let q = P2::polar(rho(theta), psi_theta);
                m = m.max(dist_segment(q, u0, u1));
            }
        }
        check_ratio(
            &mut t,
            m,
            p.eps_rep,
            &format!("reducer r1 {r1} r2 {r2} h {h} e {e}"),
        );
    }
    eprintln!(
        "PROPERTY reducer: {} sets, accepted {}, refused {}, certified-measured {}, uncertified {}, largest measured/bound {:.4}",
        cases, t.accepted, t.refused, t.measured, t.uncertified, t.worst_ratio
    );
    assert!(
        t.measured > cases / 4,
        "too few certified cases to be a test"
    );
}
