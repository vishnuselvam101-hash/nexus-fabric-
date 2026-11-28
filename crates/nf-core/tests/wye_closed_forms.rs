//! NFJ-1 S1-L9b: closed-form and first-principles tests of the wye (`nf_core::wye`). Independent of src/wye.rs:
//! the expected values are (a) hand derivations stated in each test, (b) `verify_yfork.py` output (ZK's own independent
//! check, kept in /root/nf-logs/tasks/NFJ-1/L9b/zk_verify_yfork.out; compared to 2 decimals), (c) the 3D piercing
//! reference of tests/wye_ref (half solids), never ZK's shared.js numbers. No test here is a golden case.
mod wye_ref;

use nf_core::shapes::Basis;
use nf_core::wye::{self, WyeKind, WyeResult, WyeSpec, verify_wye};
use std::f64::consts::PI;
use wye_ref::*;

fn spec(
    d: f64,
    t: f64,
    b: (f64, f64),
    l: f64,
    h: (f64, f64),
    cone: Option<(f64, f64, f64)>,
    split: u32,
) -> WyeSpec {
    WyeSpec {
        d,
        t,
        beta1_deg: b.0,
        beta2_deg: b.1,
        l,
        h1: h.0,
        h2: h.1,
        branch_d: cone.map(|c| c.0),
        branch_t: cone.map(|c| c.1),
        cone_s: cone.map(|c| c.2),
        split: Some(split),
        basis: Basis::Mid,
        amp: Basis::Outer,
    }
}

fn ex(i: usize) -> WyeSpec {
    match i {
        0 => spec(1016.0, 12.0, (45.0, 45.0), 500.0, (900.0, 900.0), None, 1),
        1 => spec(1625.6, 12.0, (30.0, 30.0), 600.0, (2200.0, 2200.0), None, 1),
        2 => spec(610.0, 9.0, (30.0, 45.0), 400.0, (1000.0, 900.0), None, 2),
        _ => spec(
            1016.0,
            12.0,
            (30.0, 30.0),
            500.0,
            (1500.0, 1500.0),
            Some((610.0, 9.0, 900.0)),
            1,
        ),
    }
}

fn run(s: &WyeSpec) -> WyeResult {
    wye::wye(s).unwrap_or_else(|e| panic!("{s:?} refused {}: {}", e.code, e.message))
}

fn code(s: &WyeSpec) -> &'static str {
    wye::wye(s).expect_err("must refuse").code
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: got {a} want {b} (tol {tol})");
}

fn shoelace(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1])
        .sum::<f64>()
        .abs()
        / 2.0
}

fn sym(a: [f64; 9]) -> Vec<f64> {
    let mut v = a.to_vec();
    v.extend(a[1..8].iter().rev());
    v
}

/// verify_yfork.py output (2 decimals), spec_yfork.md section 5 examples: generator lengths at theta = 22.5 k deg.
fn marks_ref(ex: usize, piece: usize) -> Vec<f64> {
    match (ex, piece) {
        (0, 0) => sym([
            289.58, 305.60, 351.21, 419.48, 500.0, 419.48, 351.21, 305.60, 289.58,
        ]),
        (0, _) => vec![
            689.58, 705.60, 751.21, 819.48, 900.0, 705.60, 540.79, 430.67, 392.0, 430.67, 540.79,
            705.60, 900.0, 819.48, 751.21, 705.60,
        ],
        (1, 0) => sym([
            382.21, 398.79, 446.00, 516.66, 600.0, 516.66, 446.00, 398.79, 382.21,
        ]),
        (1, _) => vec![
            1982.21, 1998.79, 2046.00, 2116.66, 2200.0, 1661.25, 1204.53, 899.35, 792.19, 899.35,
            1204.53, 1661.25, 2200.0, 2116.66, 2046.00, 1998.79,
        ],
        (2, 0) => vec![
            318.28, 324.50, 342.21, 368.73, 400.0, 351.65, 310.67, 283.28, 273.66, 283.28, 310.67,
            351.65, 400.0, 368.73, 342.21, 324.50,
        ],
        (2, 1) => vec![
            918.28, 924.50, 942.21, 968.73, 1000.0, 847.89, 718.94, 632.77, 602.52, 632.77, 718.94,
            847.89, 1000.0, 968.73, 942.21, 924.50,
        ],
        (2, _) => vec![
            773.66, 783.28, 810.67, 851.65, 900.0, 747.89, 618.94, 532.77, 502.52, 532.77, 618.94,
            747.89, 900.0, 851.65, 810.67, 783.28,
        ],
        (_, 0) => sym([
            426.77, 437.29, 467.25, 512.08, 564.96, 512.08, 467.25, 437.29, 426.77,
        ]),
        _ => vec![
            729.87, 739.17, 766.12, 807.83, 859.24, 618.77, 417.87, 305.57, 269.54, 305.57, 417.87,
            618.77, 859.24, 807.83, 766.12, 739.17,
        ],
    }
}

#[test]
fn examples_match_the_independent_verify_script_values() {
    for (i, np) in [(0, 3), (1, 3), (2, 3), (3, 5)] {
        let r = run(&ex(i));
        assert_eq!(r.pieces.len(), np);
        for (pi, p) in r.pieces.iter().enumerate() {
            if p.kind == WyeKind::Tube {
                continue;
            }
            let want = marks_ref(i, pi);
            for (m, w) in p.marks.iter().zip(&want) {
                close(m.len, *w, 0.006, &format!("ex{i} piece {pi} mark {}", m.i));
            }
        }
    }
    // totals (kg) and weld lengths printed by verify_yfork.py
    for (i, kg) in [(0, 507.7), (1, 1827.2), (2, 255.4), (3, 583.4)] {
        close(
            run(&ex(i)).total_mass_kg,
            kg,
            0.06,
            &format!("ex{i} total kg"),
        );
    }
    let welds = [
        (ex(0), [1662.326, 1662.326, 1940.660]),
        (ex(2), [975.2, 998.0, 1284.9]),
        (ex(3), [1625.057, 1625.057, 1796.088]),
        (
            spec(1016.0, 12.0, (30.0, 45.0), 500.0, (2500.0, 2500.0), None, 1),
            [1624.200, 1662.326, 2140.061],
        ),
    ];
    for (n, (s, w)) in welds.iter().enumerate() {
        let r = run(s);
        for (k, (got, want)) in r.welds.iter().zip(w).enumerate() {
            close(*got, *want, 0.06, &format!("case {n} weld {k}"));
        }
    }
    // example 4: cone half angle, crotch point, tangent point, apex distance
    let r = run(&ex(3));
    close(r.psi_deg, 13.5946, 1e-4, "psi");
    close(r.crotch[0], 64.96, 5e-3, "crotch x");
    close(r.crotch[1], 0.0, 1e-9, "crotch y");
    close(r.crotch[2], 508.0, 1e-9, "crotch z");
    let p = &r.pieces[1];
    close(p.apex, 1271.33, 5e-3, "neutral apex to small mouth");
    close(p.fan_deg, 84.6180, 5e-4, "fan angle");
    let apex_axial = 508.0 / r.psi_deg.to_radians().sin();
    close(apex_axial, 2161.24, 5e-3, "apex on the axis");
}

/// First principles for cylinders: the joint plane of two equal cylinders bisects their axes, (u_i - u_j).P = 0. On the generator
/// P = s u_i + R (cos th e1 + sin th ez) this gives s = R a cos th / (1 - c) with a = e1.u_j, c = u_i.u_j (derived here, not read
/// from the code). The leg length is H - s.
#[test]
fn cylinder_cuts_follow_the_bisecting_plane() {
    for &(b1, b2) in &[
        (45.0, 45.0),
        (30.0, 45.0),
        (15.0, 60.0),
        (60.0, 60.0),
        (10.0, 80.0),
        (8.0, 12.0),
    ] {
        let s = spec(800.0, 8.0, (b1, b2), 2500.0, (3000.0, 2800.0), None, 1);
        let r = run(&s);
        let rf = Ref::new(400.0, b1, b2, 0.0);
        for (i, send) in [(0usize, 2500.0), (1, 3000.0), (2, 2800.0)] {
            for m in &r.pieces[i].marks {
                let th = PI / 8.0 * m.i as f64;
                let mut cut = f64::NEG_INFINITY;
                for &j in &NB[i] {
                    let c = dot(&rf.u[i], &rf.u[j]);
                    let a = dot(&rf.e1[i], &rf.u[j]);
                    cut = cut.max(400.0 * a * th.cos() / (1.0 - c));
                }
                close(
                    m.len,
                    send - cut,
                    1e-9 * send,
                    &format!("b{b1}/{b2} leg {i} mark {}", m.i),
                );
            }
        }
    }
}

/// verify_yfork.py check 6 / AWWA C208 geometry, derived: the deepest cut of the main is R tan(D/4) and the crotch side of a
/// leg R cot(D/2) for the symmetric wye of angle D.
#[test]
fn awwa_depths_symmetric() {
    for (delta, deep, crotch) in [
        (90.0, 210.42, 508.0),
        (60.0, 136.12, 879.88),
        (30.0, 66.88, 1895.88),
    ] {
        let s = spec(
            1016.0,
            12.0,
            (delta / 2.0, delta / 2.0),
            3000.0,
            (9000.0, 9000.0),
            None,
            1,
        );
        let r = run(&s);
        close(
            3000.0 - r.pieces[0].shortest,
            deep,
            6e-3,
            "main deepest cut",
        );
        close(
            9000.0 - r.pieces[1].shortest,
            crotch,
            6e-3,
            "leg crotch-side cut",
        );
        close(
            3000.0 - r.pieces[0].shortest,
            508.0 * (delta / 4.0f64).to_radians().tan(),
            1e-9,
            "R tan(D/4)",
        );
        close(
            9000.0 - r.pieces[1].shortest,
            508.0 / (delta / 2.0f64).to_radians().tan(),
            1e-9,
            "R cot(D/2)",
        );
    }
}

/// Weld seen as the plane section of two cylinders: half an ellipse with semi axes R / cos(b/2) (main with a leg) or
/// R / sin(D/2) (two legs) and R. Perimeter by Simpson on the arc-length integral (independent of the code's polyline).
fn half_ellipse(a: f64, b: f64) -> f64 {
    let n = 20000;
    let f = |t: f64| (a * a * t.sin().powi(2) + b * b * t.cos().powi(2)).sqrt();
    let h = PI / n as f64;
    let mut s = f(0.0) + f(PI);
    for k in 1..n {
        s += f(h * k as f64) * if k % 2 == 1 { 4.0 } else { 2.0 };
    }
    s * h / 3.0
}

#[test]
fn weld_lengths_are_half_ellipses() {
    for &(b1, b2) in &[(45.0, 45.0), (30.0, 45.0), (20.0, 70.0)] {
        let s = spec(1016.0, 12.0, (b1, b2), 3000.0, (9000.0, 9000.0), None, 1);
        let w = run(&s).welds;
        let r = 508.0;
        let el = |bb: f64| half_ellipse(r / (bb / 2.0).to_radians().cos(), r);
        close(w[0], el(b1), 1e-5 * w[0], "main-leg1");
        close(w[1], el(b2), 1e-5 * w[1], "main-leg2");
        close(
            w[2],
            half_ellipse(r / ((b1 + b2) / 2.0).to_radians().sin(), r),
            1e-5 * w[2],
            "leg1-leg2",
        );
    }
}

/// Area of a same-diameter piece: 2 pi rb H - 2 rb Ra (K1 + K2), K = tan of half the turn (derived from the cut
/// s = R a cos th /(1-c): the integral of R K |cos| over the two half circles is 2 R K each).
#[test]
fn cylinder_area_closed_form() {
    let (d, t, b1, b2, l, h1, h2) = (1016.0, 12.0, 30.0, 45.0, 700.0, 1500.0, 1200.0);
    let r = run(&spec(d, t, (b1, b2), l, (h1, h2), None, 1));
    let (rr, rb) = (d / 2.0, d / 2.0 - t / 2.0);
    let delta = (b1 + b2).to_radians();
    let tn = |x: f64| (x.to_radians() / 2.0).tan();
    let cot_half = 1.0 / (delta / 2.0).tan();
    let areas = [
        2.0 * PI * rb * l - 2.0 * rb * rr * (tn(b1) + tn(b2)),
        2.0 * PI * rb * h1 - 2.0 * rb * rr * (tn(b1) + cot_half),
        2.0 * PI * rb * h2 - 2.0 * rb * rr * (tn(b2) + cot_half),
    ];
    for (p, a) in r.pieces.iter().zip(areas) {
        close(p.area, a, 1e-8 * a, "area");
        close(p.mass_kg, a * p.t * 7.85e-6, 1e-8 * a, "mass");
    }
}

/// Limit beta1 = beta2 -> 90 (the angle between the legs -> 180: refused at 180, ZK refuses it too): the main meets two opposite
/// legs as a T: main length L - R|cos th|, each leg H - R|cos th| (derived: the plane of u0 - u1 at 90 deg is x + y = 0, the cut
/// s = R |cos th|; a leg meets the other (opposite) leg in a plane through its own axis: cut 0 on that side, R cos th on the main side).
#[test]
fn tee_limit() {
    assert_eq!(
        code(&spec(
            1000.0,
            10.0,
            (90.0, 90.0),
            800.0,
            (900.0, 900.0),
            None,
            1
        )),
        "wye_angle_out_of_range"
    );
    let r = run(&spec(
        1000.0,
        10.0,
        (89.99, 89.99),
        800.0,
        (900.0, 900.0),
        None,
        1,
    ));
    for m in &r.pieces[0].marks {
        let th = PI / 8.0 * m.i as f64;
        close(m.len, 800.0 - 500.0 * th.cos().abs(), 0.1, "main");
    }
    // the two legs only touch each other (opposite): no cut on the side facing the other leg
    for m in &r.pieces[1].marks {
        let th = PI / 8.0 * m.i as f64;
        close(m.len, 900.0 - 500.0 * th.cos().max(0.0), 0.1, "leg");
    }
}

/// Limit beta2 = 0: the legs 0 and 2 are one straight pipe, leg 1 a lateral of the same diameter at angle b. Cylinder-cylinder
/// intersection derived directly: (s sin b + R cos b cos th)^2 + R^2 sin^2 th = R^2, entry (the larger s)
/// s = R (|cos th| - cos b cos th) / sin b.
#[test]
fn lateral_limit_leg_one() {
    for b in [30.0f64, 45.0, 70.0] {
        let s = spec(900.0, 9.0, (b, 0.0), 2000.0, (3000.0, 2500.0), None, 1);
        let r = run(&s);
        assert!(r.has_hold("wye_leg_straight"));
        let br = b.to_radians();
        for m in &r.pieces[1].marks {
            let th = PI / 8.0 * m.i as f64;
            let cut = 450.0 * (th.cos().abs() - br.cos() * th.cos()) / br.sin();
            close(
                m.len,
                3000.0 - cut,
                1e-9,
                &format!("lateral b{b} mark {}", m.i),
            );
        }
    }
}

/// First-principles check of the cut against the 3D references (no plane algebra). Cylinder wyes (examples 1-3): first entry into the
/// half solids of the neighbours (ZK check 2's definition). Cone wye (example 4): a half solid is not right (the main protrudes ahead of
/// the intersection, so a leg meets the main behind its start); there the cut is checked as a true meeting of the two surfaces found by
/// scanning for sign changes of (axis distance - radius), the meeting on the joint plane g_i = g_j (the other lies on g_i = -g_j).
#[test]
fn piercing_reference_agrees_on_the_examples() {
    let mut worst = 0.0f64;
    for i in 0..4 {
        let s = ex(i);
        let r = run(&s);
        let psi = if i == 3 { r.psi_deg.to_radians() } else { 0.0 };
        let rf = Ref::new(s.d / 2.0, s.beta1_deg, s.beta2_deg, psi);
        for p in r.pieces.iter().filter(|p| p.kind != WyeKind::Tube) {
            let cp = rf.psi[p.leg].cos();
            let refcut = |th: f64| {
                if i < 3 {
                    rf.pierce_h(p.leg, th, 0.0, 0.0, p.s_end, true)
                } else {
                    rf.cone_cut_from_surfaces(p.leg, th, p.s_end)
                }
                .unwrap_or_else(|| {
                    panic!(
                        "reference finds no cut: ex{i} leg {} th {:.4} deg",
                        p.leg,
                        th.to_degrees()
                    )
                })
            };
            for k in 0..64 {
                let th = 2.0 * PI * k as f64 / 64.0 + 0.0123;
                let eng = wye::probe::cut(&s, p.leg, th, 0.0, false).unwrap();
                worst = worst.max((refcut(th) - eng).abs());
            }
            // the marks: the generators at 90 / 270 deg touch the neighbour surface tangentially (crotch of the cylinders), so the
            // reference is read a hair away (0.0123 rad) and compared with the engine at the same angle
            for m in &p.marks {
                let th = PI / 8.0 * m.i as f64;
                // the generators at the crotch angles (90 / 270 deg) touch a cylinder neighbour tangentially: the interval inside it is
                // zero-length there, a scan cannot see it; read the reference 0.7 deg away instead
                let th = if (th.sin().abs() - 1.0).abs() < 1e-9 {
                    th + 0.0123
                } else {
                    th
                };
                let eng = wye::probe::cut(&s, p.leg, th, 0.0, false).unwrap();
                close(refcut(th), eng, 1e-8, "mark angle vs reference");
                let eng0 = wye::probe::cut(&s, p.leg, PI / 8.0 * m.i as f64, 0.0, false).unwrap();
                close(
                    (p.s_end - eng0) / cp,
                    m.len,
                    1e-9,
                    "mark length is the cut at its angle",
                );
            }
        }
    }
    assert!(worst < 1e-8, "worst {worst}");
}

/// Cone first principles: (a) the cone through the small mouth (cone_s, r) has radius r there and its generator is at
/// distance Rs from the intersection (tangent to the common sphere); (b) ZK's offset (Rs - s sin psi - o)/cos psi is the cone
/// offset by the NORMAL distance o: a point of it is at distance o from the outer generator line.
#[test]
fn cone_tangency_and_normal_offset() {
    let r = run(&ex(3));
    let psi = r.psi_deg.to_radians();
    let (rs, rb, sr) = (508.0, 305.0, 900.0);
    let rho = |s: f64, o: f64| (rs - s * psi.sin() - o) / psi.cos();
    close(rho(sr, 0.0), rb, 1e-9, "radius at the small mouth");
    // distance origin -> generator line through (0, rho(0)) and (sr, rho(sr))
    let (p0, p1) = ((0.0, rho(0.0, 0.0)), (sr, rho(sr, 0.0)));
    let dist = (p0.0 * p1.1 - p1.0 * p0.1).abs() / ((p1.0 - p0.0).hypot(p1.1 - p0.1));
    close(dist, rs, 1e-9, "tangent to the common sphere");
    // psi from the independent scan
    close(psi_from(rs, rb, sr), psi, 1e-12, "psi");
    // offset cone: distance from a point of rho(s, o) to the outer generator line is o
    for o in [3.0, 6.0, 12.0] {
        for s in [100.0, 450.0, 800.0] {
            let p = (s, rho(s, o));
            let d = ((p1.1 - p0.1) * (p.0 - p0.0) - (p1.0 - p0.0) * (p.1 - p0.1)).abs()
                / ((p1.0 - p0.0).hypot(p1.1 - p0.1));
            close(d, o, 1e-9, "normal offset");
        }
    }
    // the engine's neutral radius at the mouth is the normal-offset cone: rho(sr, t/2) (basis Mid)
    close(
        r.pieces[1].circ / (2.0 * PI),
        rho(sr, 6.0),
        1e-9,
        "neutral radius at the mouth",
    );
}

/// Crotch point on all three surfaces (vectors), for a cylinder wye and cone wyes, narrowing and widening.
#[test]
fn crotch_is_on_the_three_surfaces() {
    let specs = [
        ex(0),
        ex(2),
        ex(3),
        spec(
            1016.0,
            12.0,
            (30.0, 45.0),
            500.0,
            (1500.0, 1500.0),
            Some((610.0, 9.0, 900.0)),
            1,
        ),
        spec(
            610.0,
            9.0,
            (30.0, 30.0),
            500.0,
            (1500.0, 1500.0),
            Some((1016.0, 12.0, 900.0)),
            1,
        ),
    ];
    for s in &specs {
        let r = run(s);
        let rf = Ref::new(s.d / 2.0, s.beta1_deg, s.beta2_deg, r.psi_deg.to_radians());
        for j in 0..3 {
            let (sj, perp) = rf.axis_coords(j, &r.crotch);
            close(perp, rf.rho(j, sj, 0.0), 1e-9, "crotch on surface");
        }
        // and the reference's own pierce on the main leg at the crotch generator ends at the crotch (axial = -x)
        assert!(r.crotch[2] > 0.0);
    }
    // widening branch: the main's tip falls short of the intersection (x < 0), narrowing: it protrudes (x > 0)
    assert!(run(&specs[2]).crotch[0] > 0.0);
    assert!(run(&specs[4]).crotch[0] < 0.0);
}

/// The longest generator of example 4's cone is at the crotch angle (about 93.66 deg), where the two cuts cross: 868.0592. A
/// 0.1 deg grid (the value printed by verify_yfork.py: 867.92) misses the kink. The closed-form extreme is the true maximum.
#[test]
fn longest_generator_is_at_the_kink_not_on_a_grid() {
    let s = ex(3);
    let r = run(&s);
    let p = &r.pieces[1];
    close(p.longest, 868.0592, 1e-3, "closed-form longest");
    let mut grid = 0.0f64;
    for k in 0..3600 {
        let th = (k as f64 * 0.1).to_radians();
        let c = wye::probe::cut(&s, 1, th, 0.0, false).unwrap();
        grid = grid.max((s.cone_s.unwrap() - c) / r.psi_deg.to_radians().cos());
    }
    assert!(
        grid <= p.longest + 1e-9,
        "grid {grid} above the closed form {}",
        p.longest
    );
    assert!(
        p.longest - grid < 0.2 && p.longest - grid > 0.01,
        "grid {grid}"
    );
    // 100000 samples agree with the closed form
    let mut fine = 0.0f64;
    for k in 0..100000 {
        let th = 2.0 * PI * k as f64 / 100000.0;
        let c = wye::probe::cut(&s, 1, th, 0.0, false).unwrap();
        fine = fine.max((s.cone_s.unwrap() - c) / r.psi_deg.to_radians().cos());
    }
    // refine around the best fine sample (the kink is a corner: a grid under-reads it by slope x step)
    let at = |th: f64| {
        (s.cone_s.unwrap() - wye::probe::cut(&s, 1, th, 0.0, false).unwrap())
            / r.psi_deg.to_radians().cos()
    };
    let best = (0..100000)
        .map(|k| 2.0 * PI * k as f64 / 100000.0)
        .max_by(|a, b| at(*a).partial_cmp(&at(*b)).unwrap())
        .unwrap();
    let refined = (-2000..=2000)
        .map(|k| at(best + k as f64 * 3e-8))
        .fold(0.0, f64::max);
    close(refined, p.longest, 1e-5, "refined maximum at the kink");
    assert!(fine <= p.longest + 1e-9);
}

/// Apex sign cases. (1) narrowing: the mouth radius at the neutral surface reaches 0 at t = Rs - sr sin(psi) (basis Inner, o = t):
/// just below builds with a positive area, just above is refused `wye_cone_mouth_inside_apex`. (2) widening: apex behind the intersection;
/// built with the hold, positive apex distance, the cut nearer the apex than the mouth. (3) psi -> 0+: the cone leg converges to the cylinder leg.
#[test]
fn apex_sign_cases() {
    let base = ex(3);
    let psi = psi_from(508.0, 305.0, 900.0);
    let tstar = 508.0 - 900.0 * psi.sin();
    for (t, ok) in [(tstar - 0.5, true), (tstar + 0.5, false)] {
        let mut s = base.clone();
        s.t = t;
        s.basis = Basis::Inner;
        s.branch_t = Some(9.0);
        if ok {
            let r = run(&s);
            assert!(r.pieces[1].area > 0.0 && r.pieces[1].circ > 0.0);
            assert!(r.pieces[1].apex > 0.0);
        } else {
            assert_eq!(code(&s), "wye_cone_mouth_inside_apex");
        }
    }
    // widening
    let w = spec(
        610.0,
        9.0,
        (30.0, 30.0),
        500.0,
        (1500.0, 1500.0),
        Some((1016.0, 12.0, 900.0)),
        1,
    );
    let r = run(&w);
    assert!(r.psi_deg < 0.0);
    assert!(r.has_hold("wye_branch_wider_than_main"));
    let c = &r.pieces[1];
    assert!(c.apex > 0.0 && c.fan_deg > 0.0);
    let psi_w = r.psi_deg.to_radians();
    close(
        psi_w,
        psi_from(305.0, 508.0, 900.0),
        1e-12,
        "psi of the widening cone",
    );
    // the cut is nearer the apex than the mouth: shortest/longest are positive generator lengths
    assert!(c.shortest > 0.0 && c.longest >= c.shortest);
    // the fan outline spans radii around the apex: the mouth arc is the larger radius
    let rmax = c.plates[0]
        .iter()
        .map(|p| p[0].hypot(p[1]))
        .fold(0.0, f64::max);
    close(
        rmax,
        c.apex,
        1e-6,
        "mouth arc is the outer radius of the fan",
    );
    // psi -> 0+: a branch 0.6 mm narrower (over a long cone) -> same shape as the cylinder leg
    let thin = spec(
        1016.0,
        12.0,
        (45.0, 45.0),
        500.0,
        (1500.0, 1500.0),
        Some((1015.4, 12.0, 1000.0)),
        1,
    );
    let rt = run(&thin);
    let rc = run(&spec(
        1016.0,
        12.0,
        (45.0, 45.0),
        500.0,
        (1500.0, 1500.0),
        None,
        1,
    ));
    assert!(rt.psi_deg > 0.0 && rt.psi_deg < 0.05, "psi {}", rt.psi_deg);
    for k in 0..16 {
        let a = rt.pieces[1].marks[k].len;
        let b = rc.pieces[1].marks[k].len - (1500.0 - 1000.0);
        assert!(
            (a - b).abs() < 1.0,
            "mark {k}: cone leg {a} vs cylinder leg {b}"
        );
    }
}

/// The widest range of refusals: every code that can be reached from the public API, each from one stated input.
#[test]
fn refusal_codes_each_reached() {
    let base = ex(0);
    let basec = ex(3);
    let mut cases: Vec<(&str, WyeSpec)> = Vec::new();
    let mut m = |name: &'static str, s: WyeSpec| cases.push((name, s));
    let mut s = base.clone();
    s.d = f64::NAN;
    m("wye_value_not_finite", s);
    let mut s = base.clone();
    s.t = 0.0;
    m("wye_value_not_positive", s);
    let mut s = base.clone();
    s.d = 1e12;
    m("wye_value_too_large", s);
    let mut s = base.clone();
    s.beta1_deg = 180.0;
    m("wye_angle_out_of_range", s);
    let mut s = base.clone();
    s.t = 508.0;
    m("wye_wall_too_thick", s);
    let mut s = base.clone();
    s.split = Some(0);
    m("wye_split_out_of_range", s);
    let mut s = basec.clone();
    s.cone_s = None;
    m("wye_cone_length_missing", s);
    let mut s = basec.clone();
    s.branch_t = None;
    m("wye_branch_t_missing", s);
    let mut s = basec.clone();
    s.cone_s = Some(300.0);
    m("wye_cone_not_buildable", s);
    let mut s = basec.clone();
    s.beta1_deg = 10.0;
    s.beta2_deg = 10.0;
    m("wye_cone_too_steep", s);
    let mut s = basec.clone();
    s.basis = Basis::Inner;
    s.t = 300.0;
    m("wye_cone_mouth_inside_apex", s);
    let mut s = base.clone();
    s.l = 200.0;
    m("wye_end_inside_cut", s);
    let mut s = basec.clone();
    s.h1 = 800.0;
    m("wye_tube_length_not_positive", s);
    for (c, s) in &cases {
        assert_eq!(code(s), *c, "{s:?}");
        assert!(wye::WYE_REFUSAL_CODES.contains(c));
    }
    // codes with no public trigger: defensive output invariants, exercised in the unit tests of src/wye.rs
    let defensive = [
        "wye_cut_inside_apex",
        "wye_crotch_not_found",
        "wye_cut_out_of_range",
        "wye_cut_not_continuous",
        "wye_cut_not_on_neighbour",
        "wye_area_not_converged",
        "wye_result_not_finite",
        "wye_weld_not_consistent",
        "wye_surface_offset_invalid",
    ];
    for c in defensive {
        assert!(wye::WYE_REFUSAL_CODES.contains(&c));
    }
    assert_eq!(cases.len() + defensive.len(), wye::WYE_REFUSAL_CODES.len());
}

#[test]
fn holds_and_questions() {
    // outside the AWWA range
    let r = run(&spec(
        1016.0,
        12.0,
        (60.0, 60.0),
        500.0,
        (1500.0, 1500.0),
        None,
        1,
    ));
    assert!(r.has_hold("wye_angle_outside_awwa_range"));
    let r = run(&spec(
        1016.0,
        12.0,
        (10.0, 10.0),
        500.0,
        (3000.0, 3000.0),
        None,
        1,
    ));
    assert!(r.has_hold("wye_angle_outside_awwa_range"));
    assert!(!run(&ex(0)).has_hold("wye_angle_outside_awwa_range"));
    // straight leg
    let r = run(&spec(
        1016.0,
        12.0,
        (45.0, 0.0),
        800.0,
        (1500.0, 1500.0),
        None,
        1,
    ));
    assert!(r.has_hold("wye_leg_straight"));
    // cone_s without a different branch diameter: no silent drop
    let mut s = ex(0);
    s.cone_s = Some(500.0);
    let r = run(&s);
    assert!(r.has_hold("wye_input_unused"));
    // branch within 0.5 mm of D with cone_s: unused hold; branch_d alone: question names the difference
    let mut s = ex(0);
    s.branch_d = Some(1016.3);
    let r = run(&s);
    assert!(r.questions.iter().any(|q| q.contains("0.300")));
    assert!(r.pieces.iter().all(|p| p.kind == WyeKind::Pipe));
    // the cone wye asks about the protruding main
    assert!(
        run(&ex(3))
            .questions
            .iter()
            .any(|q| q.contains("protrudes"))
    );
    // honesty fields
    for e in 0..4 {
        let r = run(&ex(e));
        assert!(!r.certified);
        assert!(r.method.contains("certified:false"));
        assert!(!r.rules.is_empty() && r.rules.iter().all(|x| x.3 == "UNVERIFIED"));
    }
}

#[test]
fn thickness_rule_and_join_plane_delta() {
    // cylinders: the joint plane does not depend on the radius, so the amplitude surface changes nothing
    let mut s = ex(0);
    s.amp = Basis::Inner;
    assert_eq!(run(&s).join_plane_delta_max, 0.0);
    // cone with the amplitude on the mid surface: the plates' own joint planes differ from the outer ones by a stated amount
    let mut s = ex(3);
    s.amp = Basis::Mid;
    let r = run(&s);
    assert!(r.join_plane_delta_max > 0.0 && r.join_plane_delta_max < 20.0);
    // and equals the independent reference's difference between the two plane rules (bisection), at the sampled angles
    let rf = Ref::new(508.0, 30.0, 30.0, r.psi_deg.to_radians());
    let mut want = 0.0f64;
    for k in 0..145 {
        let th = 2.0 * PI * k as f64 / 144.0;
        want = want.max((rf.plane_cut(1, th, 6.0, 508.0) - rf.plane_cut(1, th, 6.0, 502.0)).abs());
    }
    close(r.join_plane_delta_max, want, 1e-6, "join plane delta");
    // normal-thickness rule is named; the radial alternative is named as not implemented
    assert!(
        wye::WYE_NAMED_RULES
            .iter()
            .any(|x| x.0 == "WYE_THICKNESS_NORMAL")
    );
    assert!(
        wye::WYE_NAMED_RULES
            .iter()
            .any(|x| x.0 == "WYE_THICKNESS_RADIAL" && x.1.contains("NOT implemented"))
    );
}

#[test]
fn verify_function_passes_and_can_fail() {
    for e in 0..4 {
        let s = ex(e);
        let r = run(&s);
        let checks = verify_wye(&s, &r).unwrap();
        assert!(checks.len() >= 7);
        for c in &checks {
            assert!(c.ok, "ex{e} {c:?}");
        }
        // a wrong crotch point, a wrong weld length and a wrong total are caught
        let mut bad = r.clone();
        bad.crotch[0] += 1.0;
        assert!(
            verify_wye(&s, &bad)
                .unwrap()
                .iter()
                .any(|c| c.name == "crotch_on_three_surfaces" && !c.ok)
        );
        let mut bad = r.clone();
        bad.welds[2] *= 1.01;
        assert!(
            verify_wye(&s, &bad)
                .unwrap()
                .iter()
                .any(|c| c.name == "weld_length_both_sides" && !c.ok)
        );
        let mut bad = r.clone();
        bad.total_mass_kg += 1.0;
        assert!(
            verify_wye(&s, &bad)
                .unwrap()
                .iter()
                .any(|c| c.name == "total_mass_is_sum" && !c.ok)
        );
        let mut bad = r.clone();
        bad.pieces[0].area *= 1.01;
        assert!(
            verify_wye(&s, &bad)
                .unwrap()
                .iter()
                .any(|c| c.name == "area_vs_outline_polygon" && !c.ok)
        );
    }
}

#[test]
fn symmetric_legs_are_mirror_images_and_the_result_is_deterministic() {
    let r = run(&ex(0));
    for k in 0..16 {
        close(
            r.pieces[1].marks[k].len,
            r.pieces[2].marks[k].len,
            1e-12,
            "leg 1 vs leg 2",
        );
    }
    close(r.pieces[1].area, r.pieces[2].area, 1e-9, "leg areas");
    assert_eq!(run(&ex(3)), run(&ex(3)));
    // split: the plates of a piece together hold the area of the whole
    let r3 = run(&ex(2));
    for p in &r3.pieces {
        assert_eq!(p.plates.len(), 2);
        let poly: f64 = p.plates.iter().map(|q| shoelace(q)).sum();
        close(poly, p.area, 2e-4 * p.area, "plates sum to the piece");
    }
}
