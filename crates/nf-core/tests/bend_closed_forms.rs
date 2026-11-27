//! NFJ-1 S1-L6b: closed-form and independent checks of `nf_core::bend` that run in CI (not only in a Python script).
//! Group A: the L6a spec values (docs/shapes/spec_miterbend.md, tests/golden/zumen/verify_miterbend.py): closed forms at
//! 90 deg with 1, 2, 3 joints, the t -> 0 limit, polygon versus arc, area = C c, and route B (3D plane geometry, no tan
//! formula). Group B: AWWA C208 Figure 5 closed forms for the reducing bend. Group C: named rules, joint count source,
//! refusals. Group D: fuzz (no panic, no NaN, every refusal coded).
//! These are independent expected values: none comes from the ZK extraction and none is a derived golden case.

use nf_core::bend::*;
use nf_core::shapes::Basis;
use std::f64::consts::PI;

fn plain(d: f64, t: f64, r: f64, a: f64, j: u32, s1: f64, s2: f64) -> BendSpec {
    BendSpec::new(d, t, r, a, Some(f64::from(j)), s1, s2)
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{what}: got {a}, want {b} (tol {tol})"
    );
}

fn rs_of(d: f64, t: f64, swing: SwingRule, basis: Basis) -> f64 {
    match swing {
        SwingRule::Outer => d / 2.0,
        SwingRule::Basis => match basis {
            Basis::Outer => d / 2.0,
            Basis::Mid => (d - t) / 2.0,
            Basis::Inner => d / 2.0 - t,
        },
    }
}

const CASES: [(f64, f64, f64, f64, u32, f64, f64); 6] = [
    (1016.0, 12.0, 1500.0, 90.0, 2, 300.0, 250.0),
    (2641.6, 16.0, 2600.0, 90.0, 3, 300.0, 300.0),
    (500.0, 8.0, 800.0, 60.0, 2, 100.0, 0.0),
    (300.0, 6.0, 450.0, 45.0, 1, 0.0, 120.0),
    (800.0, 10.0, 1200.0, 120.0, 4, 200.0, 200.0),
    (610.0, 9.5, 915.0, 30.0, 1, 50.0, 50.0),
];

// ------------------------------------------------------------------ A. L6a closed forms

#[test]
fn spec_closed_forms_every_piece_both_swing_rules() {
    for &(d, t, r, a, j, s1, s2) in &CASES {
        for swing in [SwingRule::Basis, SwingRule::Outer] {
            for basis in [Basis::Mid, Basis::Inner, Basis::Outer] {
                let mut s = plain(d, t, r, a, j, s1, s2);
                s.swing = swing;
                s.basis = basis;
                s.allow = 4.0;
                let res = bend(&s).unwrap();
                let th = a.to_radians() / (2.0 * f64::from(j));
                let tt = th.tan();
                let rs = rs_of(d, t, swing, basis);
                let rb = rs_of(d, t, SwingRule::Basis, basis);
                assert_eq!(res.pieces.len(), j as usize + 1);
                for (k, p) in res.pieces.iter().enumerate() {
                    let first = k == 0;
                    let last = k == j as usize;
                    // centre length (spec section 3)
                    let c = if first {
                        r * tt + s1
                    } else if last {
                        r * tt + s2
                    } else {
                        2.0 * r * tt
                    };
                    close(p.axis, c, 1e-7, "centre length");
                    close(p.center, c + 4.0, 1e-7, "centre + allow");
                    // generatrix lengths at the outer (0) and inner (pi) side and the swing
                    let cuts = f64::from(u8::from(!first) + u8::from(!last));
                    close(p.ten, c + 4.0 + rs * tt * cuts, 1e-7, "outer generatrix");
                    close(p.chi, c + 4.0 - rs * tt * cuts, 1e-7, "inner generatrix");
                    close(p.yo, c + 4.0, 1e-7, "side generatrix = centre + allow");
                    assert!(!p.cone && p.th_deg == 0.0, "equal diameters: cylinders");
                    // developed width and area = C c (the cosine terms integrate to zero)
                    let circ = 2.0 * PI * rb;
                    close(p.bands[0].w, circ, 1e-6, "developed width");
                    close(p.net_area, circ * c, 1e-9 * circ * c, "area = C c");
                    close(
                        p.net_mass_kg,
                        circ * c * t * 7.85e-6,
                        1e-9 * circ * c * t * 7.85e-6,
                        "mass",
                    );
                    // swing of the cut curves = rs tan(theta) per cut
                    close(
                        p.bands[0].h,
                        c + 4.0 + sheet_extra(rs, tt, first, last),
                        1e-6,
                        "sheet height",
                    );
                }
            }
        }
    }
}

/// Height of a sheet above `centre + allow`: the cut curves swing +/- rs tan(theta) at each cut; the extremes of the two
/// edges are at the outer and inner generatrix, so height = (c + allow) + |upper swing| + |lower swing|.
fn sheet_extra(rs: f64, tt: f64, first: bool, last: bool) -> f64 {
    rs * tt * (f64::from(u8::from(!first)) + f64::from(u8::from(!last)))
}

#[test]
fn closed_forms_90_degrees_one_two_three_joints() {
    let (d, t, r) = (1000.0, 10.0, 1500.0);
    let rs = d / 2.0;
    let mk = |j: u32| {
        let mut s = plain(d, t, r, 90.0, j, 0.0, 0.0);
        s.swing = SwingRule::Outer;
        s.allow = 0.0;
        bend(&s).unwrap()
    };
    // 2 joints: tan 22.5 = sqrt2 - 1; middle outer generatrix 2 (R + D/2)(sqrt2 - 1), inner 2 (R - D/2)(sqrt2 - 1)
    let r2 = mk(2);
    let k2 = 2f64.sqrt() - 1.0;
    close(r2.pieces[1].ten, 2.0 * (r + rs) * k2, 1e-9, "J2 outer");
    close(r2.pieces[1].chi, 2.0 * (r - rs) * k2, 1e-9, "J2 inner");
    close(
        r2.pieces[0].axis,
        r * k2,
        1e-9,
        "J2 end piece centre = R tan 22.5",
    );
    // 1 joint: tan 45 = 1: each piece centre length R, outer R + rs, inner R - rs
    let r1 = mk(1);
    close(r1.pieces[0].axis, r, 1e-9, "J1 centre");
    close(r1.pieces[0].ten, r + rs, 1e-9, "J1 outer");
    close(r1.pieces[0].chi, r - rs, 1e-9, "J1 inner");
    // 3 joints: tan 15 = 2 - sqrt3
    let r3 = mk(3);
    let k3 = 2.0 - 3f64.sqrt();
    close(r3.pieces[2].ten, 2.0 * (r + rs) * k3, 1e-9, "J3 outer");
    close(r3.pieces[1].axis, 2.0 * r * k3, 1e-9, "J3 centre");
}

#[test]
fn polygon_length_and_arc_convergence() {
    let (d, t, r, a) = (800.0, 8.0, 1200.0, 90.0);
    let mut prev = f64::INFINITY;
    for j in [1u32, 2, 4, 8, 16, 32, 64] {
        let mut s = plain(d, t, r, a, j, 100.0, 80.0);
        s.allow = 0.0;
        let res = bend(&s).unwrap();
        let poly =
            100.0 + 80.0 + 2.0 * f64::from(j) * r * (a.to_radians() / (2.0 * f64::from(j))).tan();
        close(res.total_axis, poly, 1e-6, "sum of centre lengths");
        let err = (poly - 180.0 - r * a.to_radians()).abs();
        assert!(
            err < prev,
            "circumscribed polygon approaches the arc from above"
        );
        prev = err;
    }
    assert!(prev < 1.0);
}

#[test]
fn t_to_zero_limit_all_bases_equal_oblique_cut() {
    let (d, r, a, j) = (900.0f64, 1100.0f64, 90.0f64, 2u32);
    let th = a.to_radians() / (2.0 * f64::from(j));
    let mut out = Vec::new();
    for basis in [Basis::Outer, Basis::Mid, Basis::Inner] {
        let mut s = plain(d, 1e-5, r, a, j, 0.0, 0.0);
        s.basis = basis;
        s.allow = 0.0;
        let res = bend(&s).unwrap();
        out.push((
            res.pieces[1].ten,
            res.pieces[1].chi,
            res.pieces[1].bands[0].w,
        ));
    }
    for o in &out[1..] {
        close(o.0, out[0].0, 1e-2, "bases coincide (ten)");
        close(o.1, out[0].1, 1e-2, "bases coincide (chi)");
        close(o.2, out[0].2, 1e-2, "bases coincide (width)");
    }
    // plain oblique cut of a cylinder of radius D/2: z = -x tan(theta) along the generatrix at azimuth 0 / pi
    close(
        out[0].0,
        2.0 * (r + d / 2.0) * th.tan(),
        1e-2,
        "oblique cut outer",
    );
    // A -> 0: flat cuts
    let mut s = plain(d, 10.0, r, 1e-7, 1, 100.0, 100.0);
    s.allow = 0.0;
    let res = bend(&s).unwrap();
    close(res.pieces[0].ten, res.pieces[0].chi, 1e-3, "flat cut");
}

// route B: 3D plane geometry, no tan formula for the cut ------------------------------------------------------------

type V2 = [f64; 2];

fn dot(a: V2, b: V2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// Generatrix length (no allowance) of piece `m` (1-based) at azimuth 0 (outer) or pi (inner) from the plane geometry:
/// vertices of the centre-line polygon, cut plane through each vertex with the bisector as normal, line-plane solve.
#[allow(clippy::too_many_arguments)]
fn route_b_len(r: f64, a_deg: f64, j: u32, s1: f64, s2: f64, rs: f64, m: u32, psi_pi: bool) -> f64 {
    let rho = a_deg.to_radians() / (2.0 * f64::from(j));
    let l = 2.0 * r * rho.tan();
    let mut c: Vec<V2> = vec![[s1 + r * rho.tan(), 0.0]];
    for v in 1..j {
        let b0 = 2.0 * rho * f64::from(v);
        let p = c[v as usize - 1];
        c.push([p[0] + l * b0.cos(), p[1] + l * b0.sin()]);
    }
    let dir = |k: u32| {
        let b = 2.0 * rho * f64::from(k - 1);
        [b.cos(), b.sin()]
    };
    let normal = |v: u32| {
        let (a, b) = (dir(v), dir(v + 1));
        let n = [a[0] + b[0], a[1] + b[1]];
        let nn = n[0].hypot(n[1]);
        [n[0] / nn, n[1] / nn]
    };
    let jn = j;
    let ez = dir(m);
    let b = 2.0 * rho * f64::from(m - 1);
    let sgn = if psi_pi { -1.0 } else { 1.0 };
    let ex = [b.sin() * sgn, -b.cos() * sgn];
    let cref = if m == 1 {
        c[0]
    } else if m == jn + 1 {
        c[jn as usize - 1]
    } else {
        c[m as usize - 2]
    };
    let solve = |v: u32| {
        let n = normal(v);
        let q = [
            cref[0] + rs * ex[0] - c[v as usize - 1][0],
            cref[1] + rs * ex[1] - c[v as usize - 1][1],
        ];
        -dot(q, n) / dot(ez, n)
    };
    let z_hi = if m == jn + 1 {
        r * rho.tan() + s2
    } else {
        solve(m)
    };
    let z_lo = if m == 1 {
        -(s1 + r * rho.tan())
    } else {
        solve(m - 1)
    };
    z_hi - z_lo
}

#[test]
fn route_b_plane_geometry_matches_closed_form() {
    for &(d, t, r, a, j, s1, s2) in &CASES {
        let mut s = plain(d, t, r, a, j, s1, s2);
        s.allow = 0.0;
        s.swing = SwingRule::Basis;
        let res = bend(&s).unwrap();
        let rs = (d - t) / 2.0;
        for p in &res.pieces {
            close(
                p.ten,
                route_b_len(r, a, j, s1, s2, rs, p.m, false),
                1e-7,
                "outer generatrix, plane geometry",
            );
            close(
                p.chi,
                route_b_len(r, a, j, s1, s2, rs, p.m, true),
                1e-7,
                "inner generatrix, plane geometry",
            );
        }
    }
}

// ------------------------------------------------------------------ B. reducing bend: AWWA C208 Figure 5

#[allow(clippy::too_many_arguments)]
fn red(d: f64, de: f64, t: f64, r: f64, a: f64, j: u32, s1: f64, s2: f64) -> BendSpec {
    let mut s = plain(d, t, r, a, j, s1, s2);
    s.d_end = Some(de);
    s.swing = SwingRule::Outer;
    s
}

#[test]
fn awwa_c208_closed_forms() {
    // ZK spec_redbend.md examples 1-3, checked against the C208 formulas (not against ZK output):
    // rho = A/(2J), sin(th) = (D1 - Dn) / (2 (n - 2) R tan rho) with n = 2J, tan(phi) = sin 2rho / (cos 2rho + cos th),
    // Z1 = r1 sin th / (cos 2rho + cos th), Zn = rn sin th / (cos 2rho + cos th), Y_x = r_x sin th / cos rho (cone-cone).
    for &(d, de, t, r, a, j, s1, s2) in &[
        (2641.6, 2438.4, 16.0, 2600.0, 90.0, 3u32, 300.0, 300.0),
        (1625.6, 1371.6, 12.0, 2400.0, 90.0, 4, 400.0, 400.0),
        (1524.0, 1625.6, 12.0, 1500.0, 45.0, 2, 500.0, 400.0),
    ] {
        let m = model(&red(d, de, t, r, a, j, s1, s2)).unwrap();
        let rho = a.to_radians() / (2.0 * f64::from(j));
        let n = 2.0 * f64::from(j);
        let sin_th = (d - de) / (2.0 * (n - 2.0) * r * rho.tan());
        let th = sin_th.asin();
        for p in &m.pieces[1..j as usize] {
            close(p.th.sin(), sin_th, 1e-12, "sin theta");
        }
        assert_eq!(m.pieces[0].th, 0.0);
        assert_eq!(m.pieces[j as usize].th, 0.0);
        let phi = ((2.0 * rho).sin() / ((2.0 * rho).cos() + th.cos())).atan();
        // end-piece / cone joint: angle between the plane normal and the cylinder axis
        close(m.joints[0].ph_a, phi, 1e-12, "phi at the first joint");
        close(
            m.joints[j as usize - 1].ph_b,
            phi,
            1e-12,
            "phi at the last joint",
        );
        let (r1, rn) = (d / 2.0, de / 2.0);
        close(
            m.pieces[0].zhi,
            r1 * th.sin() / ((2.0 * rho).cos() + th.cos()),
            1e-9,
            "Z1",
        );
        close(
            m.pieces[j as usize].zlo,
            rn * th.sin() / ((2.0 * rho).cos() + th.cos()),
            1e-9,
            "Zn",
        );
        // cone-cone joints: same tilt both sides -> phi = rho, d = r sin(th)/cos(rho)
        for v in 2..j as usize {
            let jt = &m.joints[v - 1];
            close(jt.ph_a, rho, 1e-12, "cone-cone phi");
            close(jt.d, jt.r * th.sin() / rho.cos(), 1e-9, "Y_x");
        }
        // sum of axis lengths: the centre-line polygon (+ the cylinder/cone offset (1-cos th)(1-cos 2rho), tiny)
        let poly = s1 + s2 + 2.0 * f64::from(j) * r * rho.tan();
        let sum: f64 = m.pieces.iter().map(|p| p.axis).sum();
        close(sum, poly, 0.01, "axis sum = centre line");
    }
}

#[test]
fn reducing_equals_plain_when_diameters_equal_and_is_continuous() {
    let base = plain(2641.6, 16.0, 2600.0, 90.0, 3, 300.0, 300.0);
    let p0 = bend(&base).unwrap();
    let mut s = base;
    s.d_end = Some(2641.6 - 0.4);
    let p1 = bend(&s).unwrap();
    assert_eq!(
        p0.pieces, p1.pieces,
        "difference <= 0.5 mm is a plain bend, bit for bit"
    );
    assert!(!p1.model.reducing);
    s.d_end = Some(2641.6 - 0.6);
    let p2 = bend(&s).unwrap();
    assert!(p2.model.reducing);
    for (a, b) in p0.pieces.iter().zip(&p2.pieces) {
        close(a.axis, b.axis, 0.2, "continuity axis");
        close(a.ten, b.ten, 0.5, "continuity generatrix");
        close(
            a.net_mass_kg,
            b.net_mass_kg,
            1e-3 * a.net_mass_kg,
            "continuity mass",
        );
    }
}

#[test]
fn bands_share_edges_and_add_up() {
    let mut s = red(2641.6, 2438.4, 16.0, 2600.0, 90.0, 3, 300.0, 300.0);
    let one = bend(&s).unwrap();
    s.split = 3;
    let three = bend(&s).unwrap();
    for (a, b) in one.pieces.iter().zip(&three.pieces) {
        let sum: f64 = b.bands.iter().map(|x| x.area).sum();
        close(
            sum,
            a.bands[0].area,
            1e-8 * sum,
            "bands add up to the whole sheet",
        );
        for k in 0..3 {
            close(
                b.bands[k].right_edge,
                b.bands[(k + 1) % 3].left_edge,
                1e-9,
                "shared seam edge",
            );
        }
        close(
            a.net_area,
            b.net_area,
            1e-9 * a.net_area,
            "net area independent of split",
        );
    }
}

/// Independent midpoint-rule surface integral (no adaptive Simpson): lateral area of the piece between its two cut
/// curves, rho(z)/cos(th) dz dpsi.
#[test]
fn cone_area_matches_independent_midpoint_integral() {
    let mut s = red(1625.6, 1371.6, 12.0, 2400.0, 90.0, 4, 400.0, 400.0);
    s.allow = 0.0;
    let m = model(&s).unwrap();
    let res = bend(&s).unwrap();
    for (p, sh) in m.pieces.iter().zip(&res.pieces) {
        let rb0 = p.rho0 - (s.t / 2.0 - m.ao);
        let n = 20000;
        let mut a = 0.0;
        for i in 0..n {
            let psi = 2.0 * PI * (f64::from(i) + 0.5) / f64::from(n);
            let (zl, zh) = (cut_z(p, psi, false), cut_z(p, psi, true));
            let tt = p.th.tan();
            // integral of rho(z)/cos th dz, rho(z) = rb0 - z tt
            a += (rb0 * (zh - zl) - 0.5 * tt * (zh * zh - zl * zl)) / p.th.cos();
        }
        a *= 2.0 * PI / f64::from(n);
        close(sh.net_area, a, 1e-7 * a, "net area, midpoint rule");
    }
}

// ------------------------------------------------------------------ C. named rules, joint source, refusals

#[test]
fn swing_rule_basis_versus_outer() {
    // D 1016 t 12 R 1500 90 deg 3 pieces mid: spec 829.3 / 1658.5 mm (basis) versus 831.7 / 1663.5 (outer), section 8
    let mut s = plain(1016.0, 12.0, 1500.0, 90.0, 2, 0.0, 0.0);
    s.allow = 0.0;
    let b = bend(&s).unwrap();
    s.swing = SwingRule::Outer;
    let o = bend(&s).unwrap();
    close(
        b.pieces[1].yo,
        2.0 * 1500.0 * (2f64.sqrt() - 1.0),
        1e-9,
        "centre",
    );
    close(
        b.pieces[1].ten,
        2.0 * (1500.0 + 502.0) * (2f64.sqrt() - 1.0),
        1e-9,
        "basis ten",
    );
    close(
        o.pieces[1].ten,
        2.0 * (1500.0 + 508.0) * (2f64.sqrt() - 1.0),
        1e-9,
        "outer ten",
    );
    assert!(
        o.pieces[1].ten - b.pieces[1].ten > 4.9,
        "the two rules differ by about 5 mm"
    );
    // both widths use the basis radius
    close(
        b.pieces[1].bands[0].w,
        o.pieces[1].bands[0].w,
        1e-9,
        "same width",
    );
    // default is basis and the rule is echoed with its status
    let d = bend(&plain(1016.0, 12.0, 1500.0, 90.0, 2, 0.0, 0.0)).unwrap();
    assert!(
        d.rules
            .iter()
            .any(|r| r.0 == "bend_swing_radius" && r.1 == "basis" && r.2 == "UNVERIFIED")
    );
    assert!(
        d.rules
            .iter()
            .any(|r| r.0 == "ALLOW_PER_PIECE_MM" && r.2 == "UNVERIFIED")
    );
    assert!(
        d.rules
            .iter()
            .any(|r| r.0 == "SEAM_FROM_HEEL_DEG" && r.2 == "UNVERIFIED")
    );
    assert!(!d.certified && d.method == BEND_METHOD);
    for (name, _, _, status) in BEND_NAMED_RULES {
        assert!(!name.is_empty() && !status.is_empty());
    }
}

#[test]
fn seam_and_allow_are_named_rules_with_effect() {
    let base = plain(1016.0, 12.0, 1500.0, 90.0, 2, 300.0, 300.0);
    let a = bend(&base).unwrap();
    let mut s = base;
    s.allow = 0.0;
    let b = bend(&s).unwrap();
    close(
        a.pieces[1].center - b.pieces[1].center,
        4.0,
        1e-9,
        "allow adds to the centre length",
    );
    close(
        a.pieces[1].net_area,
        b.pieces[1].net_area,
        1e-9 * a.pieces[1].net_area,
        "net area has no allowance",
    );
    // seam position moves where the sheet starts: the seam edge is the generatrix at the seam azimuth
    let mut s = base;
    s.seam_deg = 0.0;
    s.seam_from = SeamFrom::Top;
    let top = bend(&s).unwrap();
    close(
        top.pieces[1].seam_len,
        top.pieces[1].ten,
        1e-9,
        "seam at the top edge = top generatrix",
    );
    s.seam_from = SeamFrom::Heel;
    let heel = bend(&s).unwrap();
    close(
        heel.pieces[1].seam_len,
        heel.pieces[1].chi,
        1e-9,
        "seam at the heel = heel generatrix",
    );
}

#[test]
fn drawing_joint_count_wins_table_is_fallback() {
    assert_eq!(pieces_for_angle(30.0, &JOINT_TABLE).unwrap(), 2);
    assert_eq!(pieces_for_angle(30.0001, &JOINT_TABLE).unwrap(), 3);
    assert_eq!(pieces_for_angle(60.0, &JOINT_TABLE).unwrap(), 3);
    assert_eq!(pieces_for_angle(90.0, &JOINT_TABLE).unwrap(), 4);
    assert_eq!(
        pieces_for_angle(50.0, &[(45.0, 3), (67.5, 4), (90.0, 5)]).unwrap(),
        4
    );
    for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            pieces_for_angle(bad, &JOINT_TABLE).unwrap_err().code,
            "bend_angle_invalid"
        );
    }
    assert_eq!(
        pieces_for_angle(120.0, &JOINT_TABLE).unwrap_err().code,
        "bend_joints_above_table"
    );
    // the drawing wins over the table
    assert_eq!(
        resolve_joints(Some(5.0), 20.0).unwrap(),
        (5, JointSource::Drawing)
    );
    assert_eq!(
        resolve_joints(None, 20.0).unwrap(),
        (1, JointSource::TableRule)
    );
    let mut s = BendSpec::new(500.0, 8.0, 800.0, 45.0, None, 100.0, 100.0);
    let r = bend(&s).unwrap();
    assert_eq!(r.joints_source, JointSource::TableRule);
    assert_eq!(r.question, Some(JOINTS_QUESTION));
    assert_eq!(r.pieces.len(), 3 + 1 - 1);
    s.joints = Some(4.0);
    let r = bend(&s).unwrap();
    assert_eq!(
        (r.joints_source, r.question, r.pieces.len()),
        (JointSource::Drawing, None, 5)
    );
    // above 90 deg: allowed when the drawing gives the joints, refused (no count invented) when it does not
    let mut s = BendSpec::new(500.0, 8.0, 800.0, 120.0, Some(4.0), 100.0, 100.0);
    assert!(bend(&s).is_ok());
    s.joints = None;
    assert_eq!(bend(&s).unwrap_err().code, "bend_joints_above_table");
    s.joints = Some(4.0);
    s.angle_deg = 180.0;
    assert_eq!(bend(&s).unwrap_err().code, "bend_angle_invalid");
}

#[test]
fn coded_refusals_named_in_the_golden_and_fuzz_table() {
    let ok = red(1625.6, 1371.6, 12.0, 2400.0, 90.0, 4, 400.0, 400.0);
    let code = |f: &dyn Fn(&mut BendSpec)| {
        let mut s = ok;
        f(&mut s);
        bend(&s).unwrap_err().code
    };
    // H1, H2, M1 (same diameter), L4, M2, M3 of the ZK review fuzz table
    assert_eq!(code(&|s| s.joints = Some(0.0)), "bend_joints_invalid");
    assert_eq!(code(&|s| s.joints = Some(2.5)), "bend_joints_invalid");
    assert_eq!(
        code(&|s| {
            s.joints = Some(0.0);
            s.d_end = Some(s.d);
        }),
        "bend_joints_invalid"
    );
    assert_eq!(code(&|s| s.joints = Some(f64::NAN)), "bend_joints_invalid");
    assert_eq!(code(&|s| s.joints = Some(-3.0)), "bend_joints_invalid");
    assert_eq!(code(&|s| s.t = -5.0), "bend_value_not_positive");
    assert_eq!(code(&|s| s.d = 0.0), "bend_value_not_positive");
    assert_eq!(code(&|s| s.d_end = Some(0.0)), "bend_value_not_positive");
    assert_eq!(code(&|s| s.t = 700.0), "bend_wall_too_thick");
    assert_eq!(
        code(&|s| s.joints = Some(1.0)),
        "bend_reducing_needs_two_joints"
    );
    assert_eq!(
        code(&|s| {
            s.d_end = Some(600.0);
            s.r = 600.0;
            s.angle_deg = 30.0;
            s.joints = Some(2.0);
        }),
        "bend_cone_too_steep"
    );
    assert_eq!(
        code(&|s| {
            s.d_end = Some(1625.6);
            s.r = 800.0;
            s.joints = Some(3.0);
        }),
        "bend_cuts_cross_at_heel"
    );
    assert_eq!(code(&|s| s.split = 0), "bend_split_out_of_range");
    assert_eq!(code(&|s| s.allow = -1.0), "bend_value_not_positive");
    assert_eq!(code(&|s| s.seam_deg = f64::NAN), "bend_value_not_finite");
    // a taper_piece is never silently ignored: out of range on a reducing bend, or on a plain bend, is refused
    let mut s = ok;
    s.taper_piece = Some(1);
    assert_eq!(bend(&s).unwrap_err().code, "bend_taper_piece_out_of_range");
    s.taper_piece = Some(5);
    assert_eq!(bend(&s).unwrap_err().code, "bend_taper_piece_out_of_range");
    let mut p = plain(1016.0, 12.0, 1500.0, 90.0, 2, 300.0, 300.0);
    p.taper_piece = Some(3);
    assert_eq!(bend(&p).unwrap_err().code, "bend_taper_piece_not_reducing");
    // a valid taper builds: only piece 3 is a cone, and its tilt exceeds the note threshold
    s.taper_piece = Some(3);
    let r = bend(&s).unwrap();
    assert_eq!(r.pieces.iter().filter(|p| p.cone).count(), 1);
    assert!(
        r.tilt_note_deg.is_some_and(|t| t > TILT_NOTE_DEG),
        "tilt 7.64 deg exceeds 7.125"
    );
}

// ------------------------------------------------------------------ D. fuzz

const BAD: [f64; 12] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    0.0,
    -0.0,
    -1.0,
    1e-12,
    5e-324,
    1e8,
    1e8 + 1.0,
    1e300,
    f64::MAX,
];

fn check(s: &BendSpec) {
    match bend(s) {
        Ok(r) => {
            assert!(
                r.total_net_mass_kg.is_finite() && r.total_axis.is_finite(),
                "{s:?}"
            );
            for p in &r.pieces {
                for x in [
                    p.axis,
                    p.ten,
                    p.chi,
                    p.yo,
                    p.d_lo,
                    p.d_hi,
                    p.seam_len,
                    p.net_area,
                    p.net_mass_kg,
                    p.center,
                ] {
                    assert!(x.is_finite(), "{s:?}");
                }
                for b in &p.bands {
                    for x in [b.w, b.h, b.area, b.left_edge, b.right_edge] {
                        assert!(x.is_finite(), "{s:?}");
                    }
                }
            }
            assert!(!r.certified);
        }
        Err(e) => {
            assert!(
                BEND_REFUSAL_CODES.contains(&e.code),
                "uncoded refusal {} for {s:?}",
                e.code
            );
            assert!(!e.message.is_empty());
        }
    }
}

#[test]
fn fuzz_every_field_never_panics_never_nan() {
    let base = red(1625.6, 1371.6, 12.0, 2400.0, 90.0, 4, 400.0, 400.0);
    type Set = fn(&mut BendSpec, f64);
    let setters: [Set; 10] = [
        |s, v| s.d = v,
        |s, v| s.d_end = Some(v),
        |s, v| s.t = v,
        |s, v| s.r = v,
        |s, v| s.angle_deg = v,
        |s, v| s.joints = Some(v),
        |s, v| s.s1 = v,
        |s, v| s.s2 = v,
        |s, v| s.allow = v,
        |s, v| s.seam_deg = v,
    ];
    let mut n = 0;
    for set in setters {
        for v in BAD {
            let mut s = base;
            set(&mut s, v);
            check(&s);
            let mut s2 = plain(1016.0, 12.0, 1500.0, 90.0, 2, 300.0, 300.0);
            set(&mut s2, v);
            check(&s2);
            n += 2;
        }
    }
    for split in [0u32, 13, u32::MAX] {
        let mut s = base;
        s.split = split;
        check(&s);
    }
    for tp in [0u32, 1, 99, u32::MAX] {
        let mut s = base;
        s.taper_piece = Some(tp);
        check(&s);
    }
    assert!(n >= 240);
    // combination grid including tiny and huge dimensions
    let vals = [1e-3, 1.0, 50.0, 1000.0, 1e5, 1e7];
    let angles = [1e-6, 5.0, 45.0, 90.0, 179.0, 179.999];
    let js = [1.0, 2.0, 3.0, 64.0, 65.0];
    let mut combos = 0usize;
    for &d in &vals {
        for &t in &vals {
            for &r in &vals {
                for &a in &angles {
                    for &j in &js {
                        let mut s = plain(d, t, r, a, 1, 10.0, 10.0);
                        s.joints = Some(j);
                        s.d_end = if (d as u64).is_multiple_of(2) {
                            Some(d * 0.8)
                        } else {
                            None
                        };
                        check(&s);
                        combos += 1;
                    }
                }
            }
        }
    }
    assert!(combos > 6000);
}
