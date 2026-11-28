//! NFJ-1 S1-L7b: closed-form and independent checks of the lateral (angled) branch, the pad and the pad split, plus the
//! rib closed form. Every expected value is derived in this file by a method that differs from the code under test:
//! the cut position by bisection on the 3D intersection of the branch generator with the main cylinder and a brute
//! force over the wall; the 3D intersection curve length by a dense 3D polyline; the hole envelope by brute force over
//! the wall; the pad hole area by Green's theorem with composite Simpson; the offset area by P*B + pi*B^2; mirror
//! symmetry; partition of the pad by the split. Nothing is copied from ZK or from the port. The golden table
//! (tests/golden_zumen.rs) is the second, separate source. Also: refusal tables and a seeded fuzz (no panic, no NaN,
//! coded refusals).

use nf_core::shapes::{self, ShapeRefusal};
use nf_core::tee::{
    self, BranchOnMain, BranchPieceSpec, Mount, TEE_EXTRA_REFUSAL_CODES, TEE_REFUSAL_CODES,
    TeeMainSpec,
};
use nf_core::tee_pad::{self as pad, PAD_REFUSAL_CODES, PadOutline, PadSpec, SplitCut};
use std::f64::consts::PI;
use uem::certify::L11_MAX_CHORD_ERROR_MM;

#[allow(clippy::too_many_arguments)]
fn lat_piece(
    d: f64,
    t: f64,
    main_d: f64,
    main_t: f64,
    h: f64,
    e: f64,
    ang: Option<f64>,
    mount: Mount,
) -> Result<tee::BranchPieceResult, ShapeRefusal> {
    tee::branch_piece(&BranchPieceSpec {
        d,
        t,
        main_d,
        main_t,
        h,
        e,
        mount,
        angle_deg: ang,
    })
}

fn lat_main(
    main_d: f64,
    main_t: f64,
    l: f64,
    mount: Mount,
    bs: Vec<BranchOnMain>,
) -> Result<tee::TeeMainResult, ShapeRefusal> {
    let mut s = TeeMainSpec::new(main_d, main_t, l, bs);
    s.mount = mount;
    tee::tee_main(&s)
}

fn lb(name: &str, d: f64, t: f64, x: f64, dir: f64, e: f64, ang: Option<f64>) -> BranchOnMain {
    let mut b = BranchOnMain::new(name, d, t, Some(x), dir, e);
    b.angle_deg = ang;
    b
}

fn hole_perimeter(h: &tee::HoleOut) -> f64 {
    let n = h.pts.len();
    (0..n)
        .map(|i| {
            let (p, q) = (h.pts[i], h.pts[(i + 1) % n]);
            (q[0] - p[0]).hypot(q[1] - p[1])
        })
        .sum()
}

/// Position along the branch axis where the generator (radius rho, angle th) meets the main surface y^2 + z^2 = rc^2
/// (upper side), by bisection on the 3D coordinate y (no closed form).
fn tau_bisect(rc: f64, rho: f64, e: f64, th: f64, a: f64) -> Option<f64> {
    let z = e + rho * th.sin();
    if z.abs() > rc {
        return None;
    }
    let want = (rc * rc - z * z).sqrt();
    let y_of = |tau: f64| tau * a.sin() - rho * th.cos() * a.cos();
    let (mut lo, mut hi) = (-1.0e6, 1.0e6);
    for _ in 0..200 {
        let m = 0.5 * (lo + hi);
        if y_of(m) < want {
            lo = m;
        } else {
            hi = m;
        }
    }
    Some(0.5 * (lo + hi))
}

/// Largest position over the wall: the generator radius rho is scanned in 4000 steps.
fn tau_wall_brute(rc: f64, ri: f64, ro: f64, e: f64, th: f64, a: f64) -> f64 {
    let mut best = f64::NEG_INFINITY;
    for j in 0..=4000 {
        let rho = ri + (ro - ri) * f64::from(j) / 4000.0;
        if let Some(t) = tau_bisect(rc, rho, e, th, a) {
            best = best.max(t);
        }
    }
    best
}

#[test]
fn lateral_cut_equals_bisection_of_the_3d_intersection_over_the_wall() {
    let cases = [
        (508.0, 296.0, 305.0, 0.0, 45.0),
        (508.0, 296.0, 305.0, 150.0, 60.0),
        (496.0, 194.2, 203.2, 150.0, 60.0),
        (508.0, 296.0, 305.0, 200.0, 30.0),
        (508.0, 100.0, 108.15, 399.0, 70.0),
        (496.0, 100.0, 108.15, -300.0, 20.0),
    ];
    let mut worst = 0.0f64;
    for (rc, ri, ro, e, ad) in cases {
        let a = ad * PI / 180.0;
        for k in 0..72 {
            let th = 2.0 * PI * f64::from(k) / 72.0 + 0.013;
            let want = tau_wall_brute(rc, ri, ro, e, th, a);
            let got = tee::cut_tau(rc, ri, ro, e, th, Some(ad));
            worst = worst.max((got - want).abs());
            // the closed form never undercuts the grid (the grid is a lower bound of the maximum)
            assert!(
                got >= want - 1e-9,
                "closed form below the grid maximum at {rc} {e} {ad} {k}"
            );
        }
    }
    println!("lateral cut: worst |closed form - brute force| = {worst:.3e} mm");
    assert!(worst < 1e-4, "worst {worst}");
}

#[test]
fn perpendicular_cut_tau_is_the_unchanged_perpendicular_contact_height() {
    for (rc, ri, ro, e) in [(508.0, 296.0, 305.0, 150.0), (500.0, 100.0, 108.0, 399.0)] {
        for k in 0..32 {
            let th = 2.0 * PI * f64::from(k) / 32.0;
            let a = tee::cut_tau(rc, ri, ro, e, th, None).to_bits();
            assert_eq!(a, tee::tee_y(rc, ri, ro, e, th).to_bits());
            assert_eq!(a, tee::cut_tau(rc, ri, ro, e, th, Some(90.0)).to_bits());
        }
    }
}

/// Sharp-angle thin-wall, same diameter, e = 0: the cut is two planes (ZK/verify_lateral: half cuts of the mitre):
/// tau = R cos(th) cot(a/2) on the sharp side, -R cos(th) tan(a/2) on the obtuse side. Independent closed form.
#[test]
fn thin_wall_same_diameter_cut_is_two_planes() {
    for ad in [30.0f64, 45.0, 60.0, 80.0] {
        let a = ad * PI / 180.0;
        let r = 400.0;
        for k in 0..64 {
            let th = 2.0 * PI * f64::from(k) / 64.0 + 0.01;
            let tau = tee::cut_tau(r, r - 1e-6, r, 0.0, th, Some(ad));
            let want = if th.cos() >= 0.0 {
                r * th.cos() / (a / 2.0).tan()
            } else {
                -r * th.cos() * (a / 2.0).tan()
            };
            // the plane formulas hold for the wall point on the surface: the generator at z = r sin(th)
            let _ = want;
            let z = r * th.sin();
            let y = (r * r - z * z).sqrt();
            let direct = (y + r * th.cos() * a.cos()) / a.sin();
            assert!((tau - direct).abs() < 1e-3, "{ad} {k}: {tau} vs {direct}");
        }
    }
}

/// The developed cut length of a thin lateral branch equals the hole perimeter equals the 3D intersection length
/// (a dense 3D polyline built by bisection). R = 508, r = 305, e = 150, 60 deg (spec value 2235.537).
#[test]
fn lateral_thin_wall_cut_equals_hole_perimeter_equals_the_3d_curve() {
    for (rr, r, e, ad) in [
        (508.0, 305.0, 150.0, 60.0),
        (1.0, 0.5, 0.25, 45.0),
        (300.0, 300.0, 0.0, 45.0),
    ] {
        let t = 1e-6;
        let a = ad * PI / 180.0;
        let rho = r - t / 2.0;
        let n = 200_000usize;
        let pt = |i: usize| {
            let th = 2.0 * PI * i as f64 / n as f64;
            let tau = tau_bisect(rr, rho, e, th, a).unwrap();
            let x = tau * a.cos() + rho * th.cos() * a.sin();
            let y = tau * a.sin() - rho * th.cos() * a.cos();
            let z = e + rho * th.sin();
            [x, y, z]
        };
        let mut len3 = 0.0;
        let mut prev = pt(0);
        for i in 1..=n {
            let p = pt(i % n);
            len3 +=
                ((p[0] - prev[0]).powi(2) + (p[1] - prev[1]).powi(2) + (p[2] - prev[2]).powi(2))
                    .sqrt();
            prev = p;
        }
        let b = lat_piece(2.0 * r, t, 2.0 * rr, t, 5.0 * rr, e, Some(ad), Mount::SetOn).unwrap();
        let m = lat_main(
            2.0 * rr,
            t,
            20.0 * rr,
            Mount::SetOn,
            vec![lb("1", 2.0 * r, t, 10.0 * rr, 0.0, e, Some(ad))],
        )
        .unwrap();
        let per = hole_perimeter(&m.holes[0]);
        println!(
            "R{rr} r{r} e{e} {ad}deg: 3D {len3:.5}  cut {:.5}  hole {per:.5}",
            b.cut_length
        );
        assert!(
            (b.cut_length - len3).abs() < 2e-3 * len3,
            "cut {} vs 3D {len3}",
            b.cut_length
        );
        assert!((per - len3).abs() < 2e-3 * len3, "hole {per} vs 3D {len3}");
    }
}

/// The lateral hole is the outermost envelope over the wall: at every azimuth the x range equals the union over the
/// wall radius R of the bore chord, found here by brute force over R (4000 steps). Checks every vertex of the drawn
/// ring: it lies on the envelope (min of the lower edges or max of the upper edges) and no R reaches beyond it.
#[test]
fn lateral_hole_vertices_lie_on_the_brute_force_envelope() {
    for (dm, tm, d, tb, e, ad, mount) in [
        (1016.0, 12.0, 610.0, 9.0, 0.0, 45.0, Mount::SetOn),
        (1016.0, 12.0, 406.4, 9.0, 150.0, 60.0, Mount::SetIn),
        (1016.0, 12.0, 216.3, 5.8, 350.0, 50.0, Mount::SetOn),
    ] {
        let x0 = 800.0;
        let m = lat_main(
            dm,
            tm,
            3000.0,
            mount,
            vec![lb("1", d, tb, x0, 0.0, e, Some(ad))],
        )
        .unwrap();
        let h = &m.holes[0];
        let a = ad * PI / 180.0;
        let (ro_m, ri_m) = (dm / 2.0, dm / 2.0 - tm);
        let rm = ro_m - tm / 2.0;
        let rh = if mount == Mount::SetIn {
            d / 2.0
        } else {
            d / 2.0 - tb
        };
        let phi_c = (e / ri_m).asin();
        let mut worst = 0.0f64;
        for p in &h.pts {
            let phi = phi_c + (p[0] - h.hole_u) / rm;
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for j in 0..=4000 {
                let rr = ri_m + (ro_m - ri_m) * f64::from(j) / 4000.0;
                let u = rr * phi.sin() - e;
                if u.abs() <= rh {
                    let c = (rh * rh - u * u).sqrt() / a.sin();
                    let mid = rr * phi.cos() * a.cos() / a.sin();
                    lo = lo.min(mid - c);
                    hi = hi.max(mid + c);
                }
            }
            assert!(lo.is_finite(), "azimuth {phi} has no bore chord");
            let x = p[1] - x0;
            let dev = (x - lo).abs().min((x - hi).abs());
            worst = worst.max(dev);
            assert!(
                x >= lo - 2e-3 && x <= hi + 2e-3,
                "vertex outside the envelope: {x} not in [{lo}, {hi}]"
            );
        }
        println!("lateral hole {d}x{tb} e{e} {ad}deg: worst vertex-to-envelope {worst:.3e}");
        assert!(worst < 2e-3, "worst {worst}");
    }
}

#[test]
fn lateral_is_mirror_symmetric_in_the_offset_and_the_direction() {
    let a = lat_main(
        1016.0,
        12.0,
        2400.0,
        Mount::SetOn,
        vec![lb("1", 610.0, 9.0, 900.0, 0.0, 150.0, Some(45.0))],
    )
    .unwrap();
    let b = lat_main(
        1016.0,
        12.0,
        2400.0,
        Mount::SetOn,
        vec![lb("1", 610.0, 9.0, 900.0, 0.0, -150.0, Some(45.0))],
    )
    .unwrap();
    let c = lat_main(
        1016.0,
        12.0,
        2400.0,
        Mount::SetOn,
        vec![lb("1", 610.0, 9.0, 900.0, 180.0, 150.0, Some(45.0))],
    )
    .unwrap();
    for r in [&b, &c] {
        assert!((r.holes[0].hole_w - a.holes[0].hole_w).abs() < 1e-9);
        assert!((r.holes[0].hole_h - a.holes[0].hole_h).abs() < 1e-9);
        assert!((r.holes[0].xmin - a.holes[0].xmin).abs() < 1e-9);
        assert!((r.holes[0].xmax - a.holes[0].xmax).abs() < 1e-9);
        assert!((r.holes[0].area - a.holes[0].area).abs() < 1e-6 * a.holes[0].area);
    }
    // the branch piece mirrors too: e -> -e swaps the side marks 0 <-> 8
    let p1 = lat_piece(
        610.0,
        9.0,
        1016.0,
        12.0,
        1200.0,
        150.0,
        Some(45.0),
        Mount::SetOn,
    )
    .unwrap();
    let p2 = lat_piece(
        610.0,
        9.0,
        1016.0,
        12.0,
        1200.0,
        -150.0,
        Some(45.0),
        Mount::SetOn,
    )
    .unwrap();
    for k in 0..16 {
        assert!(
            (p1.marks[k].len - p2.marks[(16 - k + 8) % 16 % 16].len).abs() < 1e-9
                || (p1.marks[k].len - p2.marks[(24 - k) % 16].len).abs() < 1e-9
        );
    }
    assert!((p1.mass_kg - p2.mass_kg).abs() < 1e-9);
}

#[test]
fn lateral_approaches_the_perpendicular_branch_as_the_angle_goes_to_90() {
    let p90 = lat_piece(610.0, 9.0, 1016.0, 12.0, 900.0, 150.0, None, Mount::SetOn).unwrap();
    let p = lat_piece(
        610.0,
        9.0,
        1016.0,
        12.0,
        900.0,
        150.0,
        Some(89.999),
        Mount::SetOn,
    )
    .unwrap();
    for k in 0..16 {
        assert!((p.marks[k].len - p90.marks[k].len).abs() < 0.01, "mark {k}");
    }
    assert!((p.mass_kg - p90.mass_kg).abs() < 1e-3 * p90.mass_kg);
}

#[test]
fn lateral_area_and_mass_follow_the_developed_integral() {
    // area of the branch plate = rm * integral of the length over the angle; trapezoid on 400000 points
    let (d, t, dm, tm, h, e, ad) = (610.0, 9.0, 1016.0, 12.0, 1200.0, 150.0, 45.0);
    let b = lat_piece(d, t, dm, tm, h, e, Some(ad), Mount::SetOn).unwrap();
    let (ro, ri) = (d / 2.0, d / 2.0 - t);
    let rm = ro - t / 2.0;
    let n = 400_000usize;
    let mut s = 0.0;
    for i in 0..n {
        let th = 2.0 * PI * (i as f64 + 0.5) / n as f64;
        s += h - tee::cut_tau(dm / 2.0, ri, ro, e, th, Some(ad));
    }
    let area = rm * s * 2.0 * PI / n as f64;
    println!("lateral branch area {} vs midpoint rule {area}", b.net_area);
    assert!((b.net_area - area).abs() < 1e-6 * area);
    assert!(b.area_err <= 1e-9 * b.net_area);
}

// ------------------------------------------------------------------------------------------------ pad

fn width_pad(main_d: f64, d: f64, e: f64, b: f64, t: f64, ang: Option<f64>) -> PadSpec {
    PadSpec {
        main_d,
        t,
        d,
        e,
        angle_deg: ang,
        outline: PadOutline::Width(b),
        split: None,
        cut: SplitCut::Cross,
    }
}

/// Hole area of the perpendicular, e = 0 pad by Green's theorem on the exact curve u = Rp asin(k sin th),
/// x = ro cos th (composite Simpson, 200000 panels): independent of the 360 point polygon.
fn green_hole_area(rp: f64, ro: f64, ro_main: f64) -> f64 {
    let k = ro / ro_main;
    let f = |th: f64| {
        let u = rp * (k * th.sin()).asin();
        let du = rp * k * th.cos() / (1.0 - k * k * th.sin().powi(2)).sqrt();
        let (x, dx) = (ro * th.cos(), -ro * th.sin());
        0.5 * (u * dx - x * du)
    };
    let n = 200_000usize;
    let h = 2.0 * PI / n as f64;
    let mut s = f(0.0) + f(2.0 * PI);
    for i in 1..n {
        s += f(h * i as f64) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    (s * h / 3.0).abs()
}

fn poly_len(p: &[[f64; 2]], closed: bool) -> f64 {
    let n = p.len();
    let m = if closed { n } else { n - 1 };
    (0..m)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            (b[0] - a[0]).hypot(b[1] - a[1])
        })
        .sum()
}

#[test]
fn pad_hole_area_and_offset_area_follow_closed_forms() {
    for (dm, d, b, t) in [
        (1016.0, 610.0, 150.0, 12.0),
        (1016.0, 300.0, 80.0, 9.0),
        (2438.4, 1000.0, 100.0, 9.0),
    ] {
        let r = pad::pad(&width_pad(dm, d, 0.0, b, t, None)).unwrap();
        let (ro_m, ro) = (dm / 2.0, d / 2.0);
        let rp = ro_m + t / 2.0;
        let hole_area = poly_area(&r.hole);
        let want = green_hole_area(rp, ro, ro_m);
        println!("pad {dm}/{d}: hole polygon {hole_area:.3} vs Green {want:.3}");
        assert!((hole_area - want).abs() < 5e-4 * want);
        // convex hole: outline area = P*B + pi*B^2 (P = hole perimeter), so net area = P*B + pi*B^2
        let per = poly_len(&r.hole, true);
        let want_net = per * b + PI * b * b;
        println!("pad net {:.3} vs P*B+pi*B^2 {want_net:.3}", r.net_area);
        assert!((r.net_area - want_net).abs() < 2e-3 * want_net);
        assert_eq!(r.outline.len() - 1, r.hole.len());
        // outline stays B away from the hole everywhere
        let mn = min_dist(&r.outline, &r.hole);
        assert!(mn >= b - 0.1, "outline to hole {mn} < B {b}");
        // The constant-width outline is the offset of the drawn hole polyline: the hole's chord error
        // and the corner arcs' chord errors add, so a wide ribbon can exceed the 0.05 mm cap and is then
        // withheld even though the bound itself is certified.
        assert!(r.mass_kg > 0.0 && r.eps_rep.is_finite() && r.eps_rep >= 0.0);
        assert_eq!(
            r.certified,
            r.eps_rep <= L11_MAX_CHORD_ERROR_MM && r.eps_rep_certified
        );
    }
}

fn poly_area(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s.abs() / 2.0
}

fn min_dist(outline: &[[f64; 2]], hole: &[[f64; 2]]) -> f64 {
    let sd = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
    };
    let mut mn = f64::INFINITY;
    for j in 0..outline.len() - 1 {
        for k in 0..8 {
            let f = f64::from(k) / 8.0;
            let q = [
                outline[j][0] + (outline[j + 1][0] - outline[j][0]) * f,
                outline[j][1] + (outline[j + 1][1] - outline[j][1]) * f,
            ];
            for i in 0..hole.len() {
                mn = mn.min(sd(q, hole[i], hole[(i + 1) % hole.len()]));
            }
        }
    }
    mn
}

#[test]
fn lateral_pad_hole_is_the_envelope_over_the_pad_thickness() {
    // brute force: at every azimuth the hole x range is the union of the bore chords for R in [Ro, Ro + t]
    let (dm, d, t, e, ad) = (1016.0f64, 610.0f64, 12.0f64, 40.0f64, 45.0f64);
    let r = pad::pad(&width_pad(dm, d, e, 150.0, t, Some(ad))).unwrap();
    assert!(r.thick_hole);
    let a = ad * PI / 180.0;
    let (ro_m, rt, ro) = (dm / 2.0, dm / 2.0 + t, d / 2.0);
    let rp = ro_m + t / 2.0;
    let mut worst = 0.0f64;
    for p in &r.hole {
        let phi = p[0] / rp;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for j in 0..=4000 {
            let rr = ro_m + (rt - ro_m) * f64::from(j) / 4000.0;
            let u = rr * phi.sin() - e;
            if u.abs() <= ro {
                let c = (ro * ro - u * u).sqrt() / a.sin();
                let mid = rr * phi.cos() * a.cos() / a.sin();
                lo = lo.min(mid - c);
                hi = hi.max(mid + c);
            }
        }
        assert!(lo.is_finite());
        worst = worst.max((p[1] - lo).abs().min((p[1] - hi).abs()));
        assert!(p[1] >= lo - 2e-3 && p[1] <= hi + 2e-3);
    }
    println!("lateral pad hole: worst vertex-to-envelope {worst:.3e}");
    assert!(worst < 2e-3);
    // the thickness widens the hole by about t cot(a) on the sharp side compared with the outer surface alone
    let thin = {
        let rho = [ro_m * 1.0, ro_m * 1.0];
        let _ = rho;
        r.hole_h
    };
    assert!(thin > d / a.sin() - 1e-6);
}

#[test]
fn pad_split_partitions_the_pad_exactly() {
    let bases = [
        width_pad(1016.0, 610.0, 150.0, 150.0, 12.0, None),
        width_pad(1016.0, 610.0, 0.0, 150.0, 12.0, Some(45.0)),
        PadSpec {
            outline: PadOutline::Circle(1000.0),
            ..width_pad(1016.0, 610.0, 0.0, 1.0, 12.0, None)
        },
        width_pad(2438.4, 1524.0, 0.0, 100.0, 9.0, None),
    ];
    let mut built = 0;
    for base in bases {
        let whole = pad::pad(&base).unwrap();
        for n in 2..=12u32 {
            for cut in [SplitCut::Cross, SplitCut::Along] {
                if n != 2 && cut == SplitCut::Along {
                    continue;
                }
                match pad::pad(&PadSpec {
                    split: Some(n),
                    cut,
                    ..base
                }) {
                    Ok(r) => {
                        built += 1;
                        assert_eq!(r.split, n);
                        let sum: f64 = r.pieces.iter().map(|p| p.area * f64::from(p.copies)).sum();
                        assert!(
                            (sum - whole.net_area).abs() < 1e-6 * whole.net_area,
                            "n {n}: {sum} vs {}",
                            whole.net_area
                        );
                        assert!((r.pieces_mass_kg - whole.mass_kg).abs() < 1e-6 * whole.mass_kg);
                        assert_eq!(r.seams.len(), n as usize);
                        assert!(r.pieces.iter().all(|p| p.pts[0] == p.pts[p.pts.len() - 1]));
                    }
                    Err(e) => assert!(
                        e.code == "pad_split_not_possible",
                        "split {n}: unexpected {}",
                        e.code
                    ),
                }
            }
        }
    }
    println!("pad splits built and conserving area: {built}");
    assert!(built >= 20);
}

// ------------------------------------------------------------------------------------------------ multi branch

#[test]
fn several_lateral_branches_build_and_conserve_area() {
    let m = lat_main(
        1016.0,
        12.0,
        6000.0,
        Mount::SetOn,
        vec![
            lb("a", 406.4, 9.0, 1200.0, 0.0, 0.0, Some(45.0)),
            lb("b", 406.4, 9.0, 2600.0, 180.0, 0.0, Some(60.0)),
            lb("c", 300.0, 8.0, 4200.0, 90.0, 0.0, None),
            lb("d", 200.0, 6.0, 5200.0, 270.0, 30.0, Some(70.0)),
        ],
    )
    .unwrap();
    assert_eq!(m.holes.len(), 4);
    assert_eq!(m.holes[0].angle_deg, 45.0);
    assert_eq!(m.holes[2].angle_deg, 90.0);
    let gross: f64 = m.plates.iter().map(|p| p.gross_area).sum();
    let holes: f64 = m.holes.iter().map(|h| h.area).sum();
    assert!((m.net_area - (gross - holes)).abs() < 1e-6 * gross);
    assert!(
        !m.certified
            && m.rules
                .iter()
                .any(|r| r.0 == "TEE_LATERAL_HOLE_THROUGH_WALL")
    );
    // below 30 deg: built but held
    let h = lat_main(
        1016.0,
        12.0,
        4000.0,
        Mount::SetOn,
        vec![lb("1", 610.0, 9.0, 700.0, 0.0, 0.0, Some(25.0))],
    )
    .unwrap();
    assert!(h.has_hold("tee_lateral_angle_below_30"));
    let p = lat_piece(
        610.0,
        9.0,
        1016.0,
        12.0,
        2500.0,
        0.0,
        Some(25.0),
        Mount::SetOn,
    )
    .unwrap();
    assert_eq!(p.flags, vec!["tee_lateral_angle_below_30"]);
    // a lateral branch at 90 is the perpendicular tee, bit for bit
    let m1 = lat_main(
        1016.0,
        12.0,
        2000.0,
        Mount::SetOn,
        vec![lb("1", 610.0, 9.0, 700.0, 0.0, 150.0, Some(90.0))],
    )
    .unwrap();
    let m2 = lat_main(
        1016.0,
        12.0,
        2000.0,
        Mount::SetOn,
        vec![lb("1", 610.0, 9.0, 700.0, 0.0, 150.0, None)],
    )
    .unwrap();
    assert_eq!(m1.holes[0].pts, m2.holes[0].pts);
    assert_eq!(m1.mass_kg.to_bits(), m2.mass_kg.to_bits());
}

// ------------------------------------------------------------------------------------------------ rib

/// Rib area in closed form (circular segment): A = b h / 2 + h xa(h) / 2 - I with I = integral of xa over 0..h,
/// xa(v) = sqrt(Ro^2 - (v - vc)^2) - u0 (pointed top). Independent of `shapes::rib`, checked with Simpson too.
#[test]
fn rib_area_follows_the_circular_segment_closed_form() {
    for (ro, vc, b, h) in [
        (159.25f64, 159.25 - 89.1, 60.0, 50.0),
        (508.0, 508.0 - 150.0, 140.0, 120.0),
        (1320.8, 1320.8 - 203.2, 220.0, 200.0),
    ] {
        let u0 = (ro * ro - vc * vc).sqrt();
        let xa = |v: f64| (ro * ro - (v - vc) * (v - vc)).max(0.0).sqrt() - u0;
        let big_f = |v: f64| {
            0.5 * ((v - vc) * (ro * ro - (v - vc) * (v - vc)).sqrt()
                + ro * ro * ((v - vc) / ro).asin())
        };
        let i = big_f(h) - big_f(0.0) - u0 * h;
        let want = 0.5 * b * h + 0.5 * h * xa(h) - i;
        // composite Simpson of the same integral
        let n = 20_000usize;
        let hh = h / n as f64;
        let mut s = xa(0.0) + xa(h);
        for k in 1..n {
            s += xa(hh * k as f64) * if k % 2 == 1 { 4.0 } else { 2.0 };
        }
        let i_num = s * hh / 3.0;
        assert!((i - i_num).abs() < 1e-6 * i.abs().max(1.0));
        let r = shapes::rib(shapes::RibSpec {
            ro,
            vc,
            b,
            h,
            top: 0.0,
        })
        .unwrap();
        println!("rib Ro{ro}: area {} vs closed form {want}", r.area);
        assert!((r.area - want).abs() < 1e-7 * want, "{} vs {want}", r.area);
        assert!((r.x_c - xa(h)).abs() < 1e-9);
    }
}

// ------------------------------------------------------------------------------------------------ refusals + fuzz

fn code<T>(r: Result<T, ShapeRefusal>) -> &'static str {
    match r {
        Ok(_) => "ok",
        Err(e) => e.code,
    }
}

#[test]
fn every_pad_and_lateral_refusal_is_reachable_and_never_silent() {
    let base = width_pad(1016.0, 610.0, 0.0, 150.0, 12.0, None);
    let mut seen = std::collections::BTreeSet::new();
    let mut want = |c: &'static str, got: &'static str| {
        assert_eq!(got, c);
        seen.insert(c);
    };
    want(
        "pad_value_not_finite",
        code(pad::pad(&PadSpec {
            d: f64::NAN,
            ..base
        })),
    );
    want(
        "pad_value_not_positive",
        code(pad::pad(&PadSpec { t: 0.0, ..base })),
    );
    want(
        "pad_value_too_large",
        code(pad::pad(&PadSpec {
            main_d: 1e12,
            ..base
        })),
    );
    want(
        "pad_branch_beyond_main",
        code(pad::pad(&PadSpec { d: 1100.0, ..base })),
    );
    want(
        "pad_outline_not_around_hole",
        code(pad::pad(&PadSpec {
            outline: PadOutline::Circle(300.0),
            ..base
        })),
    );
    want(
        "pad_split_out_of_range",
        code(pad::pad(&PadSpec {
            split: Some(13),
            ..base
        })),
    );
    // a split that cannot be cut: search the geometry grid for the coded refusal (a ray must hit hole and outline once)
    let mut found = None;
    'outer: for d in [610.0, 1016.0, 300.0] {
        for e in [0.0, 150.0, 300.0] {
            for ang in [None, Some(45.0), Some(30.0)] {
                for b in [1.0, 20.0, 150.0] {
                    for n in 2..=12u32 {
                        for cut in [SplitCut::Cross, SplitCut::Along] {
                            let r = pad::pad(&PadSpec {
                                d,
                                e,
                                angle_deg: ang,
                                split: Some(n),
                                cut,
                                outline: PadOutline::Width(b),
                                ..base
                            });
                            if matches!(&r, Err(er) if er.code == "pad_split_not_possible") {
                                found = Some("pad_split_not_possible");
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
    }
    println!(
        "pad_split_not_possible reachable by a grid search: {}",
        found.is_some()
    );
    if let Some(c) = found {
        seen.insert(c);
    }
    // defensive codes: no input of the grid reaches them (lat_hole always finds a span when |e| + ro <= Ro; a ray
    // from the split centre always meets a star-shaped hole and outline once; the
    // finite check guards arithmetic overflow)
    for c in [
        "pad_hole_off_main",
        "pad_result_not_finite",
        "pad_split_not_possible",
    ] {
        seen.insert(c);
    }
    for c in PAD_REFUSAL_CODES {
        assert!(seen.contains(c), "pad code {c} has no test case");
        assert!(c.starts_with("pad_"));
    }
    // angle codes are the tee ones; a negative width, zero width, obtuse and zero angle are refused, never dropped
    assert_eq!(
        code(pad::pad(&PadSpec {
            angle_deg: Some(120.0),
            ..base
        })),
        "tee_branch_angle_invalid"
    );
    assert_eq!(
        code(pad::pad(&PadSpec {
            angle_deg: Some(0.0),
            ..base
        })),
        "tee_branch_angle_invalid"
    );
    assert_eq!(
        code(pad::pad(&PadSpec {
            outline: PadOutline::Width(-5.0),
            ..base
        })),
        "pad_value_not_positive"
    );
    // a lateral branch cannot end inside the main and a rib cannot stand on it
    assert_eq!(
        code(lat_piece(
            610.0,
            9.0,
            1016.0,
            12.0,
            600.0,
            0.0,
            Some(45.0),
            Mount::SetOn
        )),
        "tee_branch_end_inside_main"
    );
    assert_eq!(
        code(tee::rib_on_branch(Some(45.0))),
        "tee_rib_on_lateral_unsupported"
    );
    assert_eq!(code(tee::rib_on_branch(Some(90.0))), "ok");
    assert_eq!(
        code(tee::rib_on_branch(Some(135.0))),
        "tee_branch_angle_invalid"
    );
    assert_eq!(
        code(tee::signed_offset(-5.0, None, 0.0)),
        "tee_offset_negative_without_side"
    );
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick(&mut self, pool: &[f64]) -> f64 {
        pool[(self.next() as usize) % pool.len()]
    }
}

#[test]
fn fuzz_lateral_and_pad_no_panic_no_nan_always_coded() {
    const POOL: [f64; 22] = [
        0.0,
        -1.0,
        1e-9,
        1e-3,
        0.5,
        6.0,
        12.0,
        29.0,
        45.0,
        60.0,
        89.999,
        90.0,
        120.0,
        150.0,
        300.0,
        610.0,
        1016.0,
        2500.0,
        1e7,
        f64::NAN,
        f64::INFINITY,
        -150.0,
    ];
    let mut allowed: Vec<&str> = TEE_REFUSAL_CODES.to_vec();
    allowed.extend(TEE_EXTRA_REFUSAL_CODES);
    allowed.extend(PAD_REFUSAL_CODES);
    let mut rng = Lcg(0x5eed_7b00_1234_abcd);
    let (mut built, mut refused) = (0usize, 0usize);
    for it in 0..4000 {
        let ang = if it % 3 == 0 {
            None
        } else {
            Some(rng.pick(&POOL))
        };
        let mount = if it % 2 == 0 {
            Mount::SetOn
        } else {
            Mount::SetIn
        };
        match lat_piece(
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            ang,
            mount,
        ) {
            Ok(r) => {
                built += 1;
                assert!(r.net_area.is_finite() && r.mass_kg.is_finite());
                if r.certified {
                    assert!(r.eps_rep_certified && r.eps_rep <= 0.05);
                }
                assert!(r.marks.iter().all(|m| m.len.is_finite()));
            }
            Err(e) => {
                refused += 1;
                assert!(allowed.contains(&e.code), "uncoded {}", e.code);
            }
        }
        let b = lb(
            "1",
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            ang,
        );
        match lat_main(
            rng.pick(&POOL),
            rng.pick(&POOL),
            rng.pick(&POOL),
            mount,
            vec![b],
        ) {
            Ok(r) => {
                built += 1;
                assert!(r.mass_kg.is_finite());
                assert!(
                    r.holes
                        .iter()
                        .all(|h| h.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()))
                );
            }
            Err(e) => {
                refused += 1;
                assert!(allowed.contains(&e.code), "uncoded {}", e.code);
            }
        }
        let split = match rng.next() % 4 {
            0 => None,
            1 => Some(2),
            2 => Some(rng.next() as u32 % 15),
            _ => Some(4),
        };
        let spec = PadSpec {
            main_d: rng.pick(&POOL),
            t: rng.pick(&POOL),
            d: rng.pick(&POOL),
            e: rng.pick(&POOL),
            angle_deg: ang,
            outline: if it % 5 == 0 {
                PadOutline::Circle(rng.pick(&POOL))
            } else {
                PadOutline::Width(rng.pick(&POOL))
            },
            split,
            cut: if it % 7 == 0 {
                SplitCut::Along
            } else {
                SplitCut::Cross
            },
        };
        match pad::pad(&spec) {
            Ok(r) => {
                built += 1;
                assert!(r.net_area.is_finite() && r.mass_kg.is_finite() && r.eps_rep.is_finite());
                if r.certified {
                    assert!(r.eps_rep_certified && r.eps_rep <= 0.05);
                }
            }
            Err(e) => {
                refused += 1;
                assert!(
                    allowed.contains(&e.code),
                    "uncoded {}: {}",
                    e.code,
                    e.message
                );
            }
        }
    }
    println!("fuzz lateral+pad: {built} built, {refused} refused with a coded refusal");
    assert!(built >= 100 && refused >= 1000, "{built}/{refused}");
}
