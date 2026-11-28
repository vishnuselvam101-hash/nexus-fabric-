//! NFJ-1 S1-L9b: fixed-seed randomized property test of `nf_core::wye` against an independent reference written in
//! tests/wye_ref (3D half-solid piercing for cylinder wyes, surface-meeting scan for cone wyes, plane bisection when the amplitude
//! surface is offset), plus a fuzz of every field (no panic, no NaN, always a coded `wye_*` refusal). Specs include symmetric
//! legs, one straight leg, angles at the ends of the range, narrowing and widening cones, cones at the steepness limit and at the
//! apex limit, thick walls, a branch diameter at the 0.5 mm tolerance and lengths at the deepest cut.
mod wye_ref;

use nf_core::shapes::Basis;
use nf_core::wye::{self, WyeKind, WyeSpec, verify_wye};
use std::f64::consts::PI;
use wye_ref::*;

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

fn off(b: Basis, t: f64) -> f64 {
    match b {
        Basis::Outer => 0.0,
        Basis::Mid => t / 2.0,
        Basis::Inner => t,
    }
}

struct Gen {
    r: Lcg,
}

impl Gen {
    fn basis(&mut self) -> Basis {
        self.r
            .pick(&[Basis::Outer, Basis::Mid, Basis::Mid, Basis::Inner])
    }

    fn cone_spec(&mut self, narrowing: bool, near_limit: bool) -> Option<WyeSpec> {
        let d = self.r.uni(300.0, 3000.0);
        let rs = d / 2.0;
        let (b1, b2) = (self.r.uni(4.0, 75.0), self.r.uni(4.0, 75.0));
        let half = (b1 + b2) / 2.0;
        let (psi_deg, r) = if narrowing {
            let f = if near_limit {
                self.r.pick(&[0.9, 0.97, 0.995, 0.9995, 1.0005, 1.02])
            } else {
                self.r.uni(0.05, 1.1)
            };
            (f * half, rs * self.r.uni(0.3, 0.95))
        } else {
            (-self.r.uni(0.02, 0.9) * half, rs * self.r.uni(1.05, 1.8))
        };
        let psi = psi_deg.to_radians();
        let sr = (rs - r * psi.cos()) / psi.sin();
        if !(sr > 0.0 && sr.is_finite()) {
            return None;
        }
        let t = d * self.r.uni(0.005, 0.06);
        let bt = (2.0 * r) * self.r.uni(0.005, 0.06);
        Some(WyeSpec {
            d,
            t,
            beta1_deg: b1,
            beta2_deg: b2,
            l: rs * self.r.uni(1.0, 8.0),
            h1: sr + rs * self.r.uni(0.3, 3.0),
            h2: sr + rs * self.r.uni(0.3, 3.0),
            branch_d: Some(2.0 * r),
            branch_t: Some(bt),
            cone_s: Some(sr),
            split: Some(self.r.pick(&[1, 1, 2, 3])),
            basis: self.basis(),
            amp: self.basis(),
        })
    }

    fn pipe_spec(&mut self, symmetric: bool, shape: u32) -> WyeSpec {
        let d = self.r.uni(200.0, 3000.0);
        let rs = d / 2.0;
        let (b1, b2) = match shape {
            // 0 random, 1 symmetric, 2 near a straight leg, 3 angle at the top of the range, 4 small angle
            1 => {
                let b = self.r.uni(5.0, 80.0);
                (b, b)
            }
            2 => (self.r.uni(5.0, 80.0), self.r.pick(&[1e-6, 1e-3, 0.5, 2.0])),
            3 => (self.r.uni(60.0, 89.9), 89.99 - 0.0),
            4 => (self.r.uni(3.0, 10.0), self.r.uni(3.0, 10.0)),
            _ => (self.r.uni(3.0, 85.0), self.r.uni(3.0, 85.0)),
        };
        let b2 = if shape == 3 {
            self.r.uni(60.0, 89.9)
        } else {
            b2
        };
        let delta = (b1 + b2).to_radians();
        let deep = rs / (delta / 2.0).tan();
        let h1 = (deep + rs * 4.0) * self.r.uni(0.9, 1.6);
        let h2 = if symmetric {
            h1
        } else {
            (deep + rs * 4.0) * self.r.uni(0.9, 1.6)
        };
        WyeSpec {
            d,
            t: d * self.r.uni(0.004, 0.3),
            beta1_deg: b1,
            beta2_deg: b2,
            l: rs * self.r.uni(0.5, 9.0) * (1.0 + (b1.max(b2) / 45.0)),
            h1,
            h2,
            branch_d: None,
            branch_t: if self.r.next() < 0.3 {
                Some(d * self.r.uni(0.004, 0.1))
            } else {
                None
            },
            cone_s: None,
            split: Some(self.r.pick(&[1, 1, 2, 4, 6])),
            basis: self.basis(),
            amp: self.basis(),
        }
    }
}

/// Independent retained-side predicate: the point lies on the kept side of the plane of BOTH neighbours (`g_i >= g_j`, planes
/// of the sphere of the amplitude surface `Rs - ao`) and ON the plane of one of them (`g_i = g_j`, 1e-6 mm): a cut on the
/// wrong plane (`g_i = -g_j`) or on the wrong side of the right one fails.
fn retained_side(rf: &Ref, leg: usize, pt: &V3, ao: f64) -> bool {
    let rp = rf.rs - ao;
    let gi = rf.g(leg, rp, pt);
    let kept = NB[leg]
        .iter()
        .all(|&j| gi >= rf.g(j, rp, pt) - 1e-9 * rf.rs);
    let on = NB[leg].iter().any(|&j| (gi - rf.g(j, rp, pt)).abs() < 1e-6);
    kept && on
}

/// Negative control: the predicate rejects a cut point moved back into a neighbour (the wrong side of the plane), for a
/// grid of cone and cylinder wyes. Its `on the plane` part also rejects the engine's own point once it is moved out along the
/// generator, so the predicate here is only used for the point ON the cut and the rejection of the point behind it.
#[test]
fn retained_side_predicate_detects_the_wrong_side() {
    let mut g = Gen {
        r: Lcg(SEED ^ 0x5151),
    };
    let (mut rejected, mut tried) = (0, 0);
    for k in 0..40 {
        let s = if k % 2 == 0 {
            g.pipe_spec(false, 0)
        } else {
            match g.cone_spec(true, false) {
                Some(s) => s,
                None => continue,
            }
        };
        let Ok(r) = wye::wye(&s) else { continue };
        let rf = Ref::new(s.d / 2.0, s.beta1_deg, s.beta2_deg, r.psi_deg.to_radians());
        for leg in 0..3 {
            let ao = if leg == 0 || r.cone {
                off(s.amp, s.t)
            } else {
                off(s.amp, s.branch_t.unwrap_or(s.t))
            };
            for th in [0.3, 1.2, 2.5, 4.0, 5.5] {
                let e = wye::probe::cut(&s, leg, th, ao, true).unwrap();
                assert!(retained_side(&rf, leg, &rf.point(leg, e, th, ao), ao));
                tried += 1;
                if !retained_side(&rf, leg, &rf.point(leg, e - 25.0, th, ao), ao) {
                    rejected += 1;
                }
            }
        }
    }
    assert!(
        tried > 100 && rejected * 10 >= tried * 9,
        "rejected {rejected} of {tried}"
    );
}

struct Tally {
    built: usize,
    refused: usize,
    codes: std::collections::BTreeMap<&'static str, usize>,
    worst_cut: f64,
    worst_ao: f64,
    compared: usize,
    verified: usize,
    cone_built: usize,
    ao_cases: usize,
    grazing_skipped: usize,
    retained_checked: usize,
    worst_gap: f64,
}

fn check_spec(s: &WyeSpec, rng: &mut Lcg, t: &mut Tally) {
    let res = std::panic::catch_unwind(|| wye::wye(s)).unwrap_or_else(|_| panic!("panic on {s:?}"));
    match res {
        Err(e) => {
            assert!(e.code.starts_with("wye_"), "uncoded refusal {e:?}");
            t.refused += 1;
            *t.codes.entry(e.code).or_insert(0) += 1;
            // a geometric refusal must be true: the deepest cut reaches the leg end (checked with the independent reference)
            if e.code == "wye_end_inside_cut" && s.branch_d.is_none() && s.amp == Basis::Outer {
                let psi = 0.0;
                let rf = Ref::new(s.d / 2.0, s.beta1_deg, s.beta2_deg, psi);
                let ends = [s.l, s.h1, s.h2];
                let mut deepest_over = false;
                for (i, &send) in ends.iter().enumerate() {
                    let hi = send + 80.0 * rf.rs;
                    for k in 0..48 {
                        let th = 2.0 * PI * k as f64 / 48.0 + 0.0123;
                        if let Some(c) = rf.pierce_h(i, th, 0.0, 0.0, hi.min(60.0 * rf.rs), true)
                            && c >= send - 1e-6
                        {
                            deepest_over = true;
                        }
                    }
                    // exact generators 0 and 180 deg
                    for th in [0.0, PI] {
                        if let Some(c) = rf.pierce_h(i, th, 0.0, 0.0, hi.min(60.0 * rf.rs), true)
                            && c >= send - 1e-6
                        {
                            deepest_over = true;
                        }
                    }
                }
                assert!(
                    deepest_over,
                    "refused wye_end_inside_cut but the reference cut stays inside the leg: {s:?}"
                );
            }
        }
        Ok(r) => {
            t.built += 1;
            // outputs
            for p in &r.pieces {
                assert!(
                    p.area.is_finite() && p.area > 0.0 && p.mass_kg > 0.0,
                    "{s:?}"
                );
                assert!(p.shortest > 0.0 && p.longest >= p.shortest, "{s:?}");
                for m in &p.marks {
                    assert!(
                        m.len.is_finite()
                            && m.len >= p.shortest - 1e-6
                            && m.len <= p.longest + 1e-6
                    );
                }
            }
            assert!(r.welds.iter().all(|w| w.is_finite() && *w > 0.0));
            assert!(!r.certified && r.method.contains("certified:false"));
            let psi = r.psi_deg.to_radians();
            if r.cone {
                t.cone_built += 1;
            }
            let rf = Ref::new(s.d / 2.0, s.beta1_deg, s.beta2_deg, psi);
            let bt = s.branch_t.unwrap_or(s.t);
            for p in r.pieces.iter().filter(|p| p.kind != WyeKind::Tube) {
                let tp = if p.leg == 0 || r.cone { s.t } else { bt };
                let ao = off(s.amp, tp);
                if ao > 0.0 {
                    t.ao_cases += 1;
                }
                let eng = |th: f64| wye::probe::cut(s, p.leg, th, ao, true).unwrap();
                let (cmin, cmax) = wye::probe::extremes(s, p.leg, ao).unwrap();
                let hi = (p.s_end).min(cmax + 1.5 * rf.rs + 0.5 * cmax.abs());
                let mut ths: Vec<f64> = (0..16).map(|k| PI / 8.0 * k as f64).collect();
                ths.extend((0..16).map(|_| rng.uni(0.0, 2.0 * PI)));
                ths.extend([1e-7, PI + 1e-7, 2.0 * PI - 1e-7]);
                for th0 in ths {
                    // crotch angles touch tangentially for cylinders: read 0.0123 rad away
                    let th = if (th0.sin().abs() - 1.0).abs() < 1e-6 {
                        th0 + 0.0123
                    } else {
                        th0
                    };
                    let e = eng(th);
                    assert!(
                        e >= cmin - 1e-9 && e <= cmax + 1e-9,
                        "cut outside closed-form range"
                    );
                    // retained side (independent): the cut point lies on the kept side of the plane of BOTH neighbours
                    // (g_i >= g_j, the planes of the amplitude sphere); on the plane of the neighbour that cuts it
                    let pt = rf.point(p.leg, e, th, ao);
                    assert!(
                        retained_side(&rf, p.leg, &pt, ao),
                        "retained side violated: leg {} th {th} on {s:?}",
                        p.leg
                    );
                    let mut gap = f64::INFINITY;
                    for &j in &NB[p.leg] {
                        let (sj, perp) = rf.axis_coords(j, &pt);
                        gap = gap.min((perp - rf.rho(j, sj, ao)).abs());
                    }
                    t.retained_checked += 1;
                    // plates of one wall cut on their own offset surfaces meet (the default joint-plane rule)
                    if p.t == s.t && (r.cone || s.branch_t.is_none_or(|b| b == s.t)) {
                        t.worst_gap = t.worst_gap.max(gap);
                        assert!(
                            gap < 1e-6,
                            "plates do not meet: gap {gap} mm, leg {} th {th} on {s:?}",
                            p.leg
                        );
                    }
                    // within 0.1 rad of the crotch angles (90 / 270 deg) the cylinder surfaces meet at a grazing angle: the surface scans are
                    // ill-conditioned there (error ~ 1e-5 mm at 0.1 rad, measured), so the plane bisection is the reference there
                    // a leg within 0.05 deg of straight makes two cylinders nearly coaxial: the same ill-conditioning (the lateral limit
                    // is checked against its own closed form in tests/wye_closed_forms.rs)
                    let near_crotch =
                        th.sin().abs() > (0.1f64).cos() || s.beta1_deg.min(s.beta2_deg) < 0.05;
                    if ao == 0.0 && !near_crotch && r.cone && rf.grazing(p.leg, th, 0.0, hi) {
                        t.grazing_skipped += 1;
                        // not compared with a scan, but the engine's point must still lie on a neighbour surface (front nappe)
                        let pt = rf.point(p.leg, e, th, 0.0);
                        let on = NB[p.leg].iter().any(|&j| {
                            let (sj, perp) = rf.axis_coords(j, &pt);
                            let rho = rf.rho(j, sj, 0.0);
                            rho > 0.0 && (perp - rho).abs() < 1e-6 * rf.rs
                        });
                        assert!(
                            on,
                            "grazing sample: the engine's cut point is on no neighbour surface: {s:?} leg {} th {th}",
                            p.leg
                        );
                        continue;
                    }
                    let reference = if ao > 0.0 || near_crotch {
                        Some(rf.plane_cut(p.leg, th, ao, rf.rs - ao))
                    } else if !r.cone {
                        rf.pierce_h(p.leg, th, 0.0, 0.0, hi, true)
                    } else {
                        rf.cone_cut_from_surfaces(p.leg, th, hi)
                    };
                    let rc = reference.unwrap_or_else(|| {
                        panic!("reference finds no cut: leg {} th {th} on {s:?}", p.leg)
                    });
                    let err = (rc - e).abs();
                    if ao > 0.0 {
                        t.worst_ao = t.worst_ao.max(err);
                    } else {
                        t.worst_cut = t.worst_cut.max(err);
                    }
                    t.compared += 1;
                    assert!(
                        err < 1e-6 * (1.0 + rc.abs()),
                        "cut differs by {err}: leg {} th {th} engine {e} reference {rc} on {s:?}",
                        p.leg
                    );
                }
            }
            for c in verify_wye(s, &r).unwrap() {
                assert!(c.ok, "verify failed {c:?} on {s:?}");
            }
            t.verified += 1;
        }
    }
}

#[test]
fn property_against_the_independent_reference() {
    let mut g = Gen { r: Lcg(SEED) };
    let mut rng = Lcg(SEED ^ 0xDEAD_BEEF);
    let mut t = Tally {
        built: 0,
        refused: 0,
        codes: Default::default(),
        worst_cut: 0.0,
        worst_ao: 0.0,
        compared: 0,
        verified: 0,
        cone_built: 0,
        ao_cases: 0,
        grazing_skipped: 0,
        retained_checked: 0,
        worst_gap: 0.0,
    };
    let mut specs: Vec<WyeSpec> = Vec::new();
    for _ in 0..100 {
        specs.push(g.pipe_spec(false, 0));
    }
    for _ in 0..80 {
        specs.push(g.pipe_spec(true, 1));
    }
    for _ in 0..40 {
        specs.push(g.pipe_spec(false, 2));
    }
    for _ in 0..30 {
        specs.push(g.pipe_spec(false, 3));
    }
    for _ in 0..30 {
        specs.push(g.pipe_spec(true, 4));
    }
    let mut want_cones = 0;
    while want_cones < 130 {
        if let Some(s) = g.cone_spec(true, false) {
            specs.push(s);
            want_cones += 1;
        }
    }
    while want_cones < 190 {
        if let Some(s) = g.cone_spec(false, false) {
            specs.push(s);
            want_cones += 1;
        }
    }
    while want_cones < 250 {
        if let Some(s) = g.cone_spec(true, true) {
            specs.push(s);
            want_cones += 1;
        }
    }
    // thick walls
    for _ in 0..30 {
        let mut s = g.pipe_spec(false, 0);
        s.t = s.d * g.r.uni(0.3, 0.499);
        specs.push(s);
    }
    // branch diameter at the 0.5 mm tolerance: equal diameter, no cone
    for k in 0..20 {
        let mut s = g.pipe_spec(false, 1);
        s.branch_d = Some(s.d + [0.0, 0.49, -0.49, 0.5, 0.51, -0.51][k % 6]);
        s.cone_s = if s.branch_d.unwrap() != s.d && (s.branch_d.unwrap() - s.d).abs() > 0.5 {
            Some(s.d * 1.5)
        } else {
            None
        };
        s.branch_t = Some(s.t);
        s.h1 += s.d;
        s.h2 += s.d;
        specs.push(s);
    }
    // lengths right at the deepest cut (accept or refuse, never wrong): L at cmax +- 0.5 %
    for k in 0..40 {
        let mut s = g.pipe_spec(k % 2 == 0, 0);
        s.amp = Basis::Outer;
        if let Some((_, cmax)) = wye::probe::extremes(
            &WyeSpec {
                l: 1e6,
                ..s.clone()
            },
            0,
            0.0,
        ) {
            s.l = cmax * (1.0 + [-0.005, 0.005][k % 2]) + if cmax <= 0.0 { 1.0 } else { 0.0 };
        }
        specs.push(s);
    }
    assert!(specs.len() >= 500, "{} specs", specs.len());
    for s in &specs {
        check_spec(s, &mut rng, &mut t);
    }
    println!(
        "wye property: {} specs, {} built ({} with cone legs, {} piece checks with an offset amplitude), {} refused {:?}; {} cut comparisons; worst cut error outer {:.3e} mm, offset amp {:.3e} mm; verify_wye ok on {}; {} cone samples not scanned (a double root the scan cannot see; the engine's point is checked on a neighbour surface instead); retained-side check (g_i >= g_j, independent) on {} samples including all {} skipped ones, worst plate gap {:.3e} mm (equal walls)",
        specs.len(),
        t.built,
        t.cone_built,
        t.ao_cases,
        t.refused,
        t.codes,
        t.compared,
        t.worst_cut,
        t.worst_ao,
        t.verified,
        t.grazing_skipped,
        t.retained_checked,
        t.grazing_skipped,
        t.worst_gap
    );
    assert!(t.built >= 300 && t.cone_built >= 60 && t.refused >= 20);
    assert!(t.worst_cut < 1e-6 && t.worst_ao < 1e-6);
}

fn base() -> WyeSpec {
    WyeSpec {
        d: 1016.0,
        t: 12.0,
        beta1_deg: 30.0,
        beta2_deg: 30.0,
        l: 500.0,
        h1: 1500.0,
        h2: 1500.0,
        branch_d: Some(610.0),
        branch_t: Some(9.0),
        cone_s: Some(900.0),
        split: Some(1),
        basis: Basis::Mid,
        amp: Basis::Outer,
    }
}

/// Every numeric field set to null (None), 0, negative, huge, tiny, NaN, infinity: no panic, no NaN in an accepted result, always a
/// coded wye_* refusal.
#[test]
fn fuzz_every_field() {
    let values = [
        0.0,
        -5.0,
        1e12,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e-9,
        1e300,
    ];
    type Setter = fn(&mut WyeSpec, f64);
    let setters: Vec<(&str, Setter)> = vec![
        ("d", |s, v| s.d = v),
        ("t", |s, v| s.t = v),
        ("beta1", |s, v| s.beta1_deg = v),
        ("beta2", |s, v| s.beta2_deg = v),
        ("l", |s, v| s.l = v),
        ("h1", |s, v| s.h1 = v),
        ("h2", |s, v| s.h2 = v),
        ("branch_d", |s, v| s.branch_d = Some(v)),
        ("branch_t", |s, v| s.branch_t = Some(v)),
        ("cone_s", |s, v| s.cone_s = Some(v)),
        ("split", |s, v| s.split = Some(v as u32)),
    ];
    let mut refused = 0;
    for b in [
        base(),
        WyeSpec {
            branch_d: None,
            branch_t: None,
            cone_s: None,
            ..base()
        },
    ] {
        for (name, set) in &setters {
            for &v in &values {
                let mut s = b.clone();
                set(&mut s, v);
                let r = std::panic::catch_unwind(|| wye::wye(&s))
                    .unwrap_or_else(|_| panic!("panic: {name} = {v}"));
                match r {
                    Err(e) => {
                        assert!(e.code.starts_with("wye_"), "{name}={v}: {e:?}");
                        refused += 1;
                    }
                    Ok(res) => {
                        assert!(
                            res.pieces
                                .iter()
                                .all(|p| p.area.is_finite() && p.mass_kg.is_finite()),
                            "{name}={v}"
                        );
                    }
                }
            }
        }
    }
    assert!(refused > 100);
    // None for the optional fields: every combination of them
    for bd in [None, Some(610.0), Some(1016.0)] {
        for bt in [None, Some(9.0)] {
            for cs in [None, Some(900.0)] {
                let mut s = base();
                s.branch_d = bd;
                s.branch_t = bt;
                s.cone_s = cs;
                match wye::wye(&s) {
                    Err(e) => assert!(e.code.starts_with("wye_")),
                    Ok(r) => assert!(r.pieces.iter().all(|p| p.mass_kg.is_finite())),
                }
            }
        }
    }
}

/// Symmetric inputs (the class that hid an 840 mm error in L9a): beta1 = beta2, H1 = H2 and mirror angles. Leg 2 must be
/// leg 1 exactly, for cylinders and cones, at every sample angle, and swapping the two swings swaps the legs.
#[test]
fn symmetric_inputs_and_swap() {
    let mut r = Lcg(SEED ^ 0x1234);
    for k in 0..60 {
        let mut s = if k % 2 == 0 {
            base()
        } else {
            WyeSpec {
                branch_d: None,
                branch_t: None,
                cone_s: None,
                ..base()
            }
        };
        let b = r.uni(8.0, 70.0);
        s.beta1_deg = b;
        s.beta2_deg = b;
        s.h1 = s.h1.max(4000.0);
        s.h2 = s.h1;
        s.l = 2000.0;
        let Ok(res) = wye::wye(&s) else { continue };
        for th in (0..90).map(|i| PI * 2.0 * i as f64 / 90.0) {
            let (a, c) = (
                wye::probe::cut(&s, 1, th, 0.0, true).unwrap(),
                wye::probe::cut(&s, 2, th, 0.0, true).unwrap(),
            );
            assert!(
                (a - c).abs() < 1e-9 * (1.0 + a.abs()),
                "leg1 {a} leg2 {c} at {th}"
            );
        }
        assert!((res.pieces[1].area - res.pieces[2].area).abs() < 1e-9 * res.pieces[1].area);
        // swapping the swings (and the lengths) swaps the legs
        let mut sw = s.clone();
        sw.beta1_deg = 20.0;
        sw.beta2_deg = 45.0;
        sw.h1 = 4000.0;
        sw.h2 = 4500.0;
        let mut sw2 = sw.clone();
        sw2.beta1_deg = 45.0;
        sw2.beta2_deg = 20.0;
        sw2.h1 = 4500.0;
        sw2.h2 = 4000.0;
        if let (Ok(a), Ok(b)) = (wye::wye(&sw), wye::wye(&sw2)) {
            assert!((a.pieces[1].area - b.pieces[2].area).abs() < 1e-8 * a.pieces[1].area);
            assert!((a.pieces[2].area - b.pieces[1].area).abs() < 1e-8 * a.pieces[2].area);
            assert!((a.welds[2] - b.welds[2]).abs() < 1e-6 * a.welds[2]);
        }
    }
}
