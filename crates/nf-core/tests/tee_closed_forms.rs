//! NFJ-1 S1-L7a: closed-form and independent checks of `nf_core::tee` (tee main with branches, branch piece).
//! Every expected value below is derived in this file from the geometry (brute force over the wall, complete elliptic
//! integral by the AGM, 3D length of the intersection curve by composite Simpson, membership of every drawn vertex on
//! the bore cylinder) and never copied from the port under test or from ZK's `shared.js`. The golden table
//! (tests/golden_zumen.rs) is the second, separate source. Also here: the thin-wall identity (branch cut length equals
//! hole perimeter equals the 3D curve), the cross-check against the existing HeaderWithBranch family and its
//! branch_stub, the refusal table and a seeded fuzz.

use nf_core::job::{Request, run};
use nf_core::shapes::ShapeRefusal;
use nf_core::tee::{
    self, BranchOnMain, BranchPieceSpec, Mount, TEE_EXTRA_REFUSAL_CODES, TEE_REFUSAL_CODES,
    TeeMainSpec, TeeSeamFrom,
};
use std::f64::consts::PI;

fn br(name: &str, d: f64, t: f64, x: f64, dir: f64, e: f64) -> BranchOnMain {
    BranchOnMain::new(name, d, t, Some(x), dir, e)
}

fn piece(
    d: f64,
    t: f64,
    main_d: f64,
    main_t: f64,
    h: f64,
    e: f64,
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
        angle_deg: None,
    })
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

/// Complete elliptic integral of the second kind by the arithmetic-geometric mean.
fn ell_e(k: f64) -> f64 {
    let (mut a, mut b, c0) = (1.0f64, (1.0 - k * k).sqrt(), k);
    let mut sum = 0.5 * c0 * c0;
    let mut pw = 0.5;
    for _ in 0..40 {
        let (an, cn) = ((a + b) / 2.0, (a - b) / 2.0);
        b = (a * b).sqrt();
        a = an;
        pw *= 2.0;
        sum += pw * cn * cn;
    }
    PI / (2.0 * a) * (1.0 - sum)
}

/// 3D length of the curve where a cylinder of radius `r` (axis perpendicular to and `e` off the axis of the main
/// cylinder of radius `rr`) meets the main cylinder, by composite Simpson on the angle round the branch.
fn curve_len_3d(rr: f64, r: f64, e: f64) -> f64 {
    let n = 400_000usize;
    let f = |th: f64| {
        let y = e + r * th.sin();
        let z = (rr * rr - y * y).sqrt();
        r * (1.0 + th.cos().powi(2) * y * y / (z * z)).sqrt()
    };
    let h = 2.0 * PI / n as f64;
    let mut s = f(0.0) + f(2.0 * PI);
    for i in 1..n {
        s += f(h * i as f64) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    s * h / 3.0
}

// ------------------------------------------------------------------------------------------------ branch piece

/// Highest contact height by brute force: the branch wall point at radius rho in [ri, ro] and angle th stands at
/// lateral position y = e + rho sin(th); the main surface of radius rc is `sqrt(rc^2 - y^2)` high there.
fn tau_brute(rc: f64, ri: f64, ro: f64, e: f64, th: f64) -> f64 {
    let n = 20_000;
    (0..=n)
        .map(|i| {
            let rho = ri + (ro - ri) * i as f64 / n as f64;
            let y = e + rho * th.sin();
            (rc * rc - y * y).max(0.0).sqrt()
        })
        .fold(0.0, f64::max)
}

#[test]
fn contact_height_equals_brute_force_over_the_wall() {
    let (rc, ri, ro) = (508.0, 296.0, 305.0);
    for e in [0.0, 50.0, -50.0, 296.0, 300.0, 420.0, -420.0] {
        for k in 0..64 {
            let th = 2.0 * PI * k as f64 / 64.0 + 0.0123;
            let a = tee::tee_y(rc, ri, ro, e, th);
            let b = tau_brute(rc, ri, ro, e, th);
            assert!((a - b).abs() < 5e-4, "e {e} th {th}: {a} vs {b}");
            assert!(
                a >= b - 1e-9,
                "closed form must not be lower than the sampled maximum"
            );
        }
    }
}

#[test]
fn branch_marks_follow_the_closed_form_and_the_exact_extremes() {
    // set-on, e = 0: tau(th) = sqrt(Ro^2 - ri^2 sin^2 th); marks at th = 90 deg + 22.5 deg k
    let (d, t, md, mt, h) = (610.0, 9.0, 1016.0, 12.0, 900.0);
    let (ro, ri, ro_m) = (d / 2.0, d / 2.0 - t, md / 2.0);
    let r = piece(d, t, md, mt, h, 0.0, Mount::SetOn).unwrap();
    for m in &r.marks {
        let th = PI / 2.0 + 2.0 * PI * m.i as f64 / 16.0;
        let want = h - (ro_m * ro_m - ri * ri * th.sin().powi(2)).sqrt();
        assert!((m.len - want).abs() < 1e-9, "mark {}", m.i);
    }
    assert_eq!(r.marks.len(), 17);
    assert!(
        (r.marks[16].len - r.marks[0].len).abs() < 1e-9,
        "the cut closes"
    );
    assert!((r.circ - 2.0 * PI * (ro - t / 2.0)).abs() < 1e-9);
    assert!((r.shortest - (h - ro_m)).abs() < 1e-9);
    assert!((r.longest - (h - (ro_m * ro_m - ri * ri).sqrt())).abs() < 1e-9);
    // set-in uses the main inner surface
    let rin = piece(d, t, md, mt, h, 0.0, Mount::SetIn).unwrap();
    let rc = md / 2.0 - mt;
    for m in &rin.marks {
        let th = PI / 2.0 + 2.0 * PI * m.i as f64 / 16.0;
        assert!((m.len - (h - (rc * rc - ri * ri * th.sin().powi(2)).sqrt())).abs() < 1e-9);
    }
}

#[test]
fn branch_area_equals_the_complete_elliptic_integral() {
    for (d, t, md, mt, h, mount) in [
        (610.0, 9.0, 1016.0, 12.0, 900.0, Mount::SetOn),
        (1016.0, 12.0, 1016.0, 12.0, 1000.0, Mount::SetOn),
        (200.0, 10.0, 1000.0, 10.0, 700.0, Mount::SetOn),
        (610.0, 9.0, 1016.0, 12.0, 900.0, Mount::SetIn),
    ] {
        let r = piece(d, t, md, mt, h, 0.0, mount).unwrap();
        let (ro, ri) = (d / 2.0, d / 2.0 - t);
        let rm = ro - t / 2.0;
        let rc = if mount == Mount::SetIn {
            md / 2.0 - mt
        } else {
            md / 2.0
        };
        let want = rm * (2.0 * PI * h - 4.0 * rc * ell_e(ri / rc));
        let rel = (r.net_area - want).abs() / want;
        assert!(rel < 1e-9, "{d} {mount:?}: rel {rel:e}");
        // the stated estimate covers the true error
        assert!(
            (r.net_area - want).abs() <= r.area_err.max(1e-9 * want),
            "estimate must cover the error"
        );
        // the drawn polygon is within the polyline's own error of the exact area
        println!(
            "area_drawn rel err {:e} (d {d} {mount:?})",
            (r.area_drawn - want).abs() / want
        );
        assert!((r.area_drawn - want).abs() / want < 1e-4);
        assert_eq!(r.method, tee::TEE_METHOD);
        assert!(!r.rules.is_empty());
        // A branch wall tangent to the main surface has a square-root singularity in the cut curve;
        // L11 refuses to certify that interval. The other cases have a strict clearance and are certified.
        assert_eq!(r.certified, ro < rc, "d {d} {mount:?}: ro {ro}, rc {rc}");
    }
}

#[test]
fn short_offset_branch_is_built_and_its_shortest_length_is_exact() {
    // D200 on main D1000, e 200, H 495: highest contact at the bore edge, shortest = H - sqrt(Ro^2 - (e - ro)^2)
    let r = piece(200.0, 10.0, 1000.0, 10.0, 495.0, 200.0, Mount::SetOn).unwrap();
    let want = 495.0 - (500.0f64.powi(2) - 100.0f64.powi(2)).sqrt();
    assert!((r.shortest - want).abs() < 1e-6, "{} vs {want}", r.shortest);
    assert!(r.extent_refine_gain >= 0.0);
}

// ------------------------------------------------------------------------------------------------ hole and plate

fn main_one(dir: f64, e: f64) -> tee::TeeMainResult {
    tee::tee_main(&TeeMainSpec::new(
        1016.0,
        12.0,
        1500.0,
        vec![br("1", 610.0, 9.0, 750.0, dir, e)],
    ))
    .unwrap()
}

#[test]
fn plate_and_hole_follow_the_closed_form() {
    let (ro, t, rh) = (508.0, 12.0, 305.0 - 9.0);
    let (ri, rm) = (ro - t, ro - t / 2.0);
    let r = main_one(0.0, 0.0);
    assert!((r.circ - 2.0 * PI * rm).abs() < 1e-9 && (r.plates[0].w - 2.0 * PI * rm).abs() < 1e-9);
    let h = &r.holes[0];
    // half angle of the hole on the inner surface: sin a = rh / ri
    let a = (rh / ri).asin();
    assert!(
        (h.hole_w - 2.0 * rm * a).abs() < 0.05,
        "{} vs {}",
        h.hole_w,
        2.0 * rm * a
    );
    assert!((h.hole_h - 2.0 * rh).abs() < 1e-9);
    assert!((h.hole_x - 750.0).abs() < 1e-12);
    // not on the drawing: centred on the length, with a question
    let s = tee::tee_main(&TeeMainSpec::new(
        1016.0,
        12.0,
        1500.0,
        vec![BranchOnMain::new("1", 610.0, 9.0, None, 0.0, 0.0)],
    ))
    .unwrap();
    assert!((s.holes[0].hole_x - 750.0).abs() < 1e-9 && s.questions == [tee::TEE_X_QUESTION]);
    assert!(r.questions.is_empty());
}

#[test]
fn direction_rotates_the_hole_by_rm_times_the_angle_and_keeps_the_seam() {
    let rm = 508.0 - 6.0;
    let base = main_one(0.0, 0.0);
    for dir in [45.0, 90.0, 180.0, 270.0] {
        let r = main_one(dir, 0.0);
        let mut du = r.holes[0].hole_u - base.holes[0].hole_u;
        let w = base.circ;
        du = ((du % w) + w) % w;
        let want = ((rm * dir.to_radians()) % w + w) % w;
        assert!((du - want).abs() < 1e-9, "dir {dir}: {du} vs {want}");
        assert!(
            (r.holes[0].hole_w - base.holes[0].hole_w).abs() < 1e-9,
            "same shape"
        );
        assert_eq!(r.moved_deg, 0.0, "seam not rotated");
        assert!(!r.cross_seam);
    }
}

#[test]
fn every_drawn_hole_vertex_lies_on_the_bore_cylinder() {
    // non-thick: vertex = (azimuth on the main inner surface, x); it must stand at distance rh from the branch axis
    for (e, dir, mount) in [
        (0.0, 0.0, Mount::SetOn),
        (120.0, 90.0, Mount::SetOn),
        (-80.0, 200.0, Mount::SetIn),
    ] {
        let mut s = TeeMainSpec::new(
            1016.0,
            12.0,
            1500.0,
            vec![br("1", 610.0, 9.0, 750.0, dir, e)],
        );
        s.mount = mount;
        let r = tee::tee_main(&s).unwrap();
        let h = &r.holes[0];
        let (ri, rm) = (496.0, 502.0);
        let rh = if mount == Mount::SetIn { 305.0 } else { 296.0 };
        let centre_az = dir.to_radians() + (e / ri).asin();
        for p in &h.pts {
            let az = centre_az + (p[0] - h.hole_u) / rm;
            let psi = az - dir.to_radians();
            let (y, x) = (ri * psi.sin(), p[1] - 750.0);
            let dist = (x * x + (y - e) * (y - e)).sqrt();
            assert!((dist - rh).abs() < 1e-6, "e {e} dir {dir}: {dist} vs {rh}");
            assert!(psi.cos() > 0.0, "vertex on the branch side");
        }
        assert_eq!(h.pts.len(), 360);
        assert!(h.eps_rep_certified && h.eps_rep.is_finite() && h.eps_rep >= 0.0);
    }
}

#[test]
fn thick_hole_vertices_lie_on_the_bore_through_the_wall() {
    // drain branch on the invert: offset beyond the bore, hole cut through the whole wall
    let e3 = (609.6 - 216.3) / 2.0;
    let r = tee::tee_main(&TeeMainSpec::new(
        609.6,
        6.0,
        1500.0,
        vec![br("d", 216.3, 5.8, 750.0, 90.0, e3)],
    ))
    .unwrap();
    let h = &r.holes[0];
    assert!(h.thick && h.e > 216.3 / 2.0 - 5.8);
    let (ri, ro, rh, rm) = (
        609.6 / 2.0 - 6.0,
        609.6 / 2.0,
        216.3 / 2.0 - 5.8,
        609.6 / 2.0 - 3.0,
    );
    let az0 = 90f64.to_radians();
    // recover the azimuth from the hole centre line: hole_u is the plate position of the centre azimuth dir + asin(e/ri)
    let centre = az0 + (e3 / ri).asin();
    for p in &h.pts {
        let psi = centre + (p[0] - h.hole_u) / rm - az0;
        let w = ((rh * rh) - (p[1] - 750.0).powi(2)).max(0.0).sqrt();
        let s = psi.sin();
        if s.abs() < 1e-6 {
            continue;
        }
        let ok = [e3 + w, e3 - w].iter().any(|y| {
            let rr = y / s;
            rr >= ri - 1e-6 && rr <= ro + 1e-6
        });
        assert!(
            ok,
            "vertex ({}, {}) is not on the bore inside the wall",
            p[0], p[1]
        );
    }
    assert!(h.edge > 0.0, "the edge along the invert line is reported");
    assert!(h.eps_rep.is_finite());
}

#[test]
fn two_holes_top_and_bottom_closed_form_gap_and_seam_distance() {
    let (ri, rm, rh) = (496.0f64, 502.0f64, 296.0f64);
    let r = tee::tee_main(&TeeMainSpec::new(
        1016.0,
        12.0,
        1500.0,
        vec![
            br("t", 610.0, 9.0, 750.0, 0.0, 0.0),
            br("b", 610.0, 9.0, 750.0, 180.0, 0.0),
        ],
    ))
    .unwrap();
    let a = (rh / ri).asin();
    // nearest edges at the main axis position: angular distance pi - 2a at the mid surface
    let want = rm * (PI - 2.0 * a);
    assert!(
        (r.gaps[0].gap - want).abs() < 0.01,
        "{} vs {want}",
        r.gaps[0].gap
    );
    // outer weld lines: sin b = ro / Ro
    let (ro_b, ro_m) = (305.0f64, 508.0f64);
    let b = (ro_b / ro_m).asin();
    assert!((r.gaps[0].weld - 2.0 * PI * ro_m / (2.0 * PI) * (PI - 2.0 * b)).abs() < 0.01);
    // seam at 135 deg from the bottom side: azimuth -135 deg = 225 deg; the bottom hole spans 180 +- a
    let seam = 225f64.to_radians();
    let want_sd = rm * (seam - (PI + a));
    assert!(
        (r.holes[1].seam_dist - want_sd).abs() < 1e-6,
        "{} vs {want_sd}",
        r.holes[1].seam_dist
    );
    assert!(!r.cross_seam && r.holds.is_empty());
}

#[test]
fn seam_crossing_is_a_hold_not_a_silent_drop_and_the_mirror_moves_it() {
    let e4 = (1016.0 - 216.3) / 2.0;
    let mk = |mirror: bool| {
        let mut s = TeeMainSpec::new(
            1016.0,
            9.0,
            2500.0,
            vec![
                br("v", 609.6, 6.0, 1000.0, 0.0, 0.0),
                br("d", 216.3, 5.8, 2100.0, 270.0, -e4),
            ],
        );
        s.seam_mirror = mirror;
        tee::tee_main(&s).unwrap()
    };
    let (a, b) = (mk(false), mk(true));
    assert!(a.cross_seam && a.has_hold("tee_seam_crosses_hole") && a.holds.len() == 1);
    assert!(!b.cross_seam && b.holds.is_empty());
    // a single branch is rotated off the seam instead (moved), never silently kept on it
    let mut s = TeeMainSpec::new(
        1016.0,
        12.0,
        1500.0,
        vec![br("1", 610.0, 9.0, 750.0, 225.0, 0.0)],
    );
    s.seams = 1;
    let r = tee::tee_main(&s).unwrap();
    assert!(!r.cross_seam);
    assert!(r.moved_deg.abs() > 0.0, "seam rotated");
    // the rule seam with a top reference
    s.seam_from = TeeSeamFrom::Top;
    assert!(tee::tee_main(&s).is_ok());
}

#[test]
fn split_main_puts_each_hole_on_one_plate_and_conserves_area() {
    let e4 = (1016.0 - 216.3) / 2.0;
    let mut s = TeeMainSpec::new(
        1016.0,
        9.0,
        2500.0,
        vec![
            br("v", 609.6, 6.0, 1000.0, 0.0, 0.0),
            br("d", 216.3, 5.8, 2100.0, 90.0, e4),
        ],
    );
    s.seams = 2;
    let r = tee::tee_main(&s).unwrap();
    let rm = 508.0 - 4.5;
    assert_eq!(r.plates.len(), 2);
    assert!((r.plates[0].w * 2.0 - 2.0 * PI * rm).abs() < 1e-9);
    let gross: f64 = r.plates.iter().map(|p| p.gross_area).sum();
    assert!((gross - 2.0 * PI * rm * 2500.0).abs() < 1e-6);
    let holes: f64 = r.holes.iter().map(|h| h.area).sum();
    assert!((r.net_area - (gross - holes)).abs() < 1e-6);
    assert_eq!(r.plates.iter().map(|p| p.holes.len()).sum::<usize>(), 2);
    assert!((r.mass_kg - nf_core::shapes::mass_kg(r.net_area, 9.0)).abs() < 1e-12);
}

// ------------------------------------------------------------------------------------------------ t -> 0 identity

#[test]
fn thin_wall_cut_length_equals_hole_perimeter_equals_the_3d_curve() {
    // the branch cut and the main hole are the same curve when the wall vanishes, and its length is the 3D length
    for (rr, r, e, dir) in [
        (500.0, 300.0, 0.0, 0.0),
        (500.0, 300.0, 150.0, 90.0),
        (500.0, 100.0, 300.0, 90.0),
        (500.0, 300.0, -150.0, 200.0),
    ] {
        let t = 1e-3;
        let want = curve_len_3d(rr, r, e);
        let b = piece(2.0 * r, t, 2.0 * rr, t, 900.0, e, Mount::SetOn).unwrap();
        let m = tee::tee_main(&TeeMainSpec::new(
            2.0 * rr,
            t,
            3000.0,
            vec![br("1", 2.0 * r, t, 1500.0, dir, e)],
        ))
        .unwrap();
        let (cut, per) = (b.cut_length, hole_perimeter(&m.holes[0]));
        assert!(
            (cut - want).abs() < 0.1,
            "R{rr} r{r} e{e}: cut {cut} vs 3D {want}"
        );
        assert!(
            (per - want).abs() < 0.1,
            "R{rr} r{r} e{e}: hole {per} vs 3D {want}"
        );
        assert!((cut - per).abs() < 0.1, "cut {cut} vs hole {per}");
    }
}

// ------------------------------------------------------------------------------------------------ cross-check HeaderWithBranch

#[allow(clippy::too_many_arguments)]
fn header_with_branch_job(
    header_radius: f64,
    header_dev: f64,
    length: f64,
    station: f64,
    branch_radius: f64,
    branch_dev: f64,
    delta: f64,
    s_end: f64,
) -> nf_core::job::JobOutput {
    run(
        &Request::HeaderWithBranch {
            header_radius,
            header_dev_radius: header_dev,
            header_length: length,
            branch_station: station,
            branch_radius,
            branch_dev_radius: branch_dev,
            beta_deg: 90.0,
            delta,
            s_end,
            surface_reference: None,
            header_surface_reference: Some("outside".into()),
            branch_surface_reference: Some("inside".into()),
            joint: Some("set_on".into()),
        },
        "tee-cross-check",
    )
}

fn q(out: &nf_core::job::JobOutput, family: &str, key: &str) -> f64 {
    let p = out
        .patterns
        .iter()
        .find(|p| p.family == family)
        .unwrap_or_else(|| {
            panic!(
                "no pattern {family}: have {:?}, outcome {:?}",
                out.patterns
                    .iter()
                    .map(|p| (&p.part, &p.family))
                    .collect::<Vec<_>>(),
                out.outcome
            )
        });
    *p.quantities
        .get(key)
        .unwrap_or_else(|| panic!("no quantity {key} in {family}: {:?}", p.quantities.keys()))
}

/// The existing HeaderWithBranch family and `Tee` agree where both are defined: perpendicular set-on joint of one
/// branch. NF has ONE contact curve for both parts; Tee has two (hole on the main inner surface with the bore radius,
/// cut where the wall meets the main outer surface). They coincide as the wall vanishes, so the comparison is made
/// at t = 0.01 mm with a tolerance that scales with t and is stated.
#[test]
fn equals_header_with_branch_where_defined_thin_wall() {
    let t = 0.01;
    let (md, d, len, x, h) = (1016.0, 610.0, 1500.0, 750.0, 900.0);
    let mut checked = 0;
    for e in [0.0, 150.0, -200.0] {
        let ro_m = md / 2.0;
        let (ro_b, rh) = (d / 2.0, d / 2.0 - t); // NF set-on: header OUTSIDE radius, branch INSIDE radius
        let nf = header_with_branch_job(ro_m, ro_m - t / 2.0, len, x, rh, ro_b - t / 2.0, e, h);
        assert!(nf.refusal.is_none(), "NF refused: {:?}", nf.refusal);
        let m = tee::tee_main(&TeeMainSpec::new(
            md,
            t,
            len,
            vec![br("1", d, t, x, 0.0, e)],
        ))
        .unwrap();
        let b = piece(d, t, md, t, h, e, Mount::SetOn).unwrap();
        // observed (see the println below): branch lengths and width agree to 1e-13 (same contact curve in the exact
        // functions); the hole perimeter differs by at most 0.08 mm, the difference of two polylines (360 vs 720
        // points) of the same curve, so 0.2 mm
        let tol = 0.2;
        let tol_exact = 1e-6;
        let hp = hole_perimeter(&m.holes[0]);
        let nf_per = q(&nf, "header_shell_with_opening", "opening_perimeter");
        assert!(
            (hp - nf_per).abs() < tol,
            "e {e}: perimeter {hp} vs NF {nf_per}"
        );
        let nf_area = q(&nf, "header_shell_with_opening", "opening_area");
        assert!(
            (m.holes[0].area - nf_area).abs() / nf_area < 1e-4,
            "e {e}: hole area"
        );
        let (xmin, xmax) = (
            q(&nf, "header_shell_with_opening", "opening_x_min"),
            q(&nf, "header_shell_with_opening", "opening_x_max"),
        );
        assert!(
            (m.holes[0].xmin - xmin).abs() < tol && (m.holes[0].xmax - xmax).abs() < tol,
            "e {e}: x range"
        );
        // branch: NF s is the saddle height from the header axis; the piece length is H - s
        let (s_min, s_max) = (
            q(&nf, "branch_stub", "s_min"),
            q(&nf, "branch_stub", "s_max"),
        );
        assert!(
            (b.shortest - (h - s_max)).abs() < tol_exact,
            "e {e}: shortest {} vs {}",
            b.shortest,
            h - s_max
        );
        assert!(
            (b.longest - (h - s_min)).abs() < tol_exact,
            "e {e}: longest {} vs {}",
            b.longest,
            h - s_min
        );
        let nf_w = q(&nf, "branch_stub", "developed_width");
        assert!(
            (b.circ - nf_w).abs() < tol_exact,
            "e {e}: width {} vs {nf_w}",
            b.circ
        );
        println!(
            "thin e {e}: d_perimeter {:e} d_shortest {:e} d_longest {:e} d_width {:e}",
            (hp - nf_per).abs(),
            (b.shortest - (h - s_max)).abs(),
            (b.longest - (h - s_min)).abs(),
            (b.circ - nf_w).abs()
        );
        let nf_a = q(&nf, "branch_stub", "area");
        assert!(
            (b.net_area - nf_a).abs() / nf_a < 2e-4,
            "e {e}: branch area {} vs {nf_a}",
            b.net_area
        );
        checked += 1;
    }
    assert_eq!(checked, 3);
}

/// Exact (thick wall) agreement with the two NF pieces, called with the per-part contact radii: the main hole equals
/// `header_with_opening` (header radius = inner radius of the main, branch radius = bore radius) for every offset the
/// bore keeps on the inner face; the branch piece equals `branch_stub` (header radius = outer radius of the main,
/// branch radius = inner radius of the branch) when the offset is 0, where the highest contact point of the wall is
/// the inner surface.
#[test]
fn equals_branch_stub_and_header_opening_with_part_contact_radii() {
    use uem::develop::{branch_stub, header_with_opening};
    use uem::geometry::Branch;
    let (md, mt, d, bt, len, x, h) = (1016.0, 12.0, 610.0, 9.0, 1500.0, 750.0, 900.0);
    let (ro_m, ri_m, rm_m) = (md / 2.0, md / 2.0 - mt, md / 2.0 - mt / 2.0);
    let (ro_b, ri_b, rm_b) = (d / 2.0, d / 2.0 - bt, d / 2.0 - bt / 2.0);
    for e in [0.0, 100.0, -180.0] {
        // main hole
        let b = Branch {
            big_r: ri_m,
            r: ri_b,
            beta: PI / 2.0,
            delta: e,
        };
        let nf = header_with_opening(&b, rm_m, len, x).unwrap();
        let m = tee::tee_main(&TeeMainSpec::new(
            md,
            mt,
            len,
            vec![br("1", d, bt, x, 0.0, e)],
        ))
        .unwrap();
        let hp = hole_perimeter(&m.holes[0]);
        let nfp = nf.quantity("opening_perimeter").unwrap();
        assert!(
            (hp - nfp).abs() < 0.1,
            "e {e}: hole perimeter {hp} vs NF {nfp}"
        );
        let (a_t, a_nf) = (m.holes[0].area, nf.quantity("opening_area").unwrap());
        println!(
            "e {e}: hole area {a_t} vs NF {a_nf}, rel {:e}; perimeter {hp} vs {nfp}",
            (a_t - a_nf).abs() / a_nf
        );
        assert!((a_t - a_nf).abs() / a_nf < 2e-4);
        assert!((m.holes[0].xmin - nf.quantity("opening_x_min").unwrap()).abs() < 0.05);
        assert!((m.holes[0].xmax - nf.quantity("opening_x_max").unwrap()).abs() < 0.05);
    }
    // branch piece at e = 0
    let nf = branch_stub(ro_m, ri_b, rm_b, 90.0, 0.0, h).unwrap();
    let p = piece(d, bt, md, mt, h, 0.0, Mount::SetOn).unwrap();
    assert!((p.shortest - (h - nf.quantity("s_max").unwrap())).abs() < 1e-6);
    assert!((p.longest - (h - nf.quantity("s_min").unwrap())).abs() < 1e-6);
    assert!((p.circ - nf.quantity("developed_width").unwrap()).abs() < 1e-9);
    assert!((p.net_area - nf.quantity("area").unwrap()).abs() / p.net_area < 2e-4);
    let _ = (ro_b, ri_b);
}

// ------------------------------------------------------------------------------------------------ refusals

fn code_of<T: std::fmt::Debug>(r: Result<T, ShapeRefusal>) -> &'static str {
    match r {
        Err(e) => {
            assert!(!e.message.is_empty() && !e.fields.is_empty());
            e.code
        }
        Ok(v) => panic!("expected a refusal, got {v:?}"),
    }
}

#[test]
fn every_refusal_code_is_reachable_and_inputs_are_never_ignored() {
    let ok = || {
        TeeMainSpec::new(
            1016.0,
            12.0,
            1500.0,
            vec![br("1", 610.0, 9.0, 750.0, 0.0, 0.0)],
        )
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut want = |code: &'static str, got: &'static str| {
        assert_eq!(got, code);
        seen.insert(code);
    };
    let mut s = ok();
    s.d = f64::NAN;
    want("tee_value_not_finite", code_of(tee::tee_main(&s)));
    s = ok();
    s.t = -1.0;
    want("tee_value_not_positive", code_of(tee::tee_main(&s)));
    s = ok();
    s.l = 1e12;
    want("tee_value_too_large", code_of(tee::tee_main(&s)));
    s = ok();
    s.t = 600.0;
    want("tee_wall_too_thick", code_of(tee::tee_main(&s)));
    s = ok();
    s.branches.clear();
    want("tee_no_branch", code_of(tee::tee_main(&s)));
    s = ok();
    s.branches = (0..17)
        .map(|i| br(&i.to_string(), 100.0, 5.0, 750.0, 0.0, 0.0))
        .collect();
    want("tee_branches_above_limit", code_of(tee::tee_main(&s)));
    s = ok();
    s.seams = 0;
    want("tee_seams_out_of_range", code_of(tee::tee_main(&s)));
    s = ok();
    s.seam_deg = 200.0;
    want("tee_seam_angle_invalid", code_of(tee::tee_main(&s)));
    s = ok();
    s.branches[0].angle_deg = Some(95.0);
    want("tee_branch_angle_invalid", code_of(tee::tee_main(&s)));
    s = ok();
    s.branches[0].angle_deg = Some(60.0);
    assert!(tee::tee_main(&s).is_ok(), "60 deg is a lateral branch now");
    s = ok();
    s.mount = Mount::SetIn;
    s.branches[0] = br("1", 1016.0, 12.0, 750.0, 0.0, 0.0);
    want("tee_branch_too_large_to_insert", code_of(tee::tee_main(&s)));
    want(
        "tee_branch_bore_beyond_main",
        code_of(piece(900.0, 5.0, 1000.0, 10.0, 2000.0, 300.0, Mount::SetOn)),
    );
    want(
        "tee_branch_end_inside_main",
        code_of(piece(610.0, 9.0, 1016.0, 12.0, 400.0, 0.0, Mount::SetOn)),
    );
    s = ok();
    s.branches[0] = br("1", 610.0, 9.0, 750.0, 0.0, 300.0);
    want("tee_hole_off_pipe", code_of(tee::tee_main(&s)));
    s = ok();
    s.branches[0].x = Some(200.0);
    want("tee_hole_beyond_pipe_end", code_of(tee::tee_main(&s)));
    s = ok();
    s.branches.push(br("2", 610.0, 9.0, 750.0, 30.0, 0.0));
    want("tee_holes_overlap", code_of(tee::tee_main(&s)));
    want(
        "tee_offset_side_not_perpendicular",
        code_of(tee::signed_offset(100.0, Some(90.0), 90.0)),
    );
    want(
        "tee_offset_negative_without_side",
        code_of(tee::signed_offset(-100.0, None, 0.0)),
    );
    want(
        "tee_rib_on_lateral_unsupported",
        code_of(tee::rib_on_branch(Some(45.0))),
    );
    for c in TEE_REFUSAL_CODES {
        assert!(seen.contains(c), "code {c} has no test case");
        assert!(c.starts_with("tee_"));
    }
    assert_eq!(seen.len(), TEE_REFUSAL_CODES.len());
    // an obtuse angle given on the branch piece is refused (the sharp angle is asked for)
    let mut bp = BranchPieceSpec {
        d: 610.0,
        t: 9.0,
        main_d: 1016.0,
        main_t: 12.0,
        h: 900.0,
        e: 0.0,
        mount: Mount::SetOn,
        angle_deg: Some(135.0),
    };
    assert_eq!(code_of(tee::branch_piece(&bp)), "tee_branch_angle_invalid");
    bp.angle_deg = Some(90.0);
    assert!(
        tee::branch_piece(&bp).is_ok(),
        "90 deg is the perpendicular branch"
    );
    // signed offset: stated side decides the sign
    assert_eq!(tee::signed_offset(100.0, Some(180.0), 90.0).unwrap(), 100.0);
    assert_eq!(
        tee::signed_offset(100.0, Some(180.0), 270.0).unwrap(),
        -100.0
    );
    assert_eq!(
        tee::signed_offset(100.0, None, 0.0).unwrap(),
        100.0,
        "no side: a non-negative offset is taken as given"
    );
    assert_eq!(
        tee::signed_offset(-100.0, Some(180.0), 90.0).unwrap(),
        100.0,
        "with a side the side decides the sign"
    );
    assert_eq!(tee::signed_offset(0.0, Some(33.0), 0.0).unwrap(), 0.0);
    for c in TEE_EXTRA_REFUSAL_CODES {
        assert!(c.starts_with("tee_"));
    }
}

// ------------------------------------------------------------------------------------------------ fuzz

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

const POOL: [f64; 14] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    0.0,
    -1.0,
    1e-9,
    1e-6,
    1e9,
    1e12,
    -1e12,
    5.0,
    100.0,
    610.0,
    1016.0,
];

fn check_finite_main(r: &tee::TeeMainResult) {
    assert!(r.mass_kg.is_finite() && r.net_area.is_finite());
    if r.certified {
        assert!(
            r.holes
                .iter()
                .all(|h| h.eps_rep_certified && h.eps_rep <= 0.05)
        );
    }
    for h in &r.holes {
        assert!(
            h.hole_u.is_finite()
                && h.hole_w.is_finite()
                && h.seam_dist.is_finite()
                && h.eps_rep.is_finite()
        );
        assert!(h.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()));
    }
    for g in &r.gaps {
        assert!(g.gap.is_finite() && g.weld.is_finite());
    }
}

#[test]
fn fuzz_no_panic_no_nan_always_a_tee_code() {
    let mut rng = Lcg(0x5EED_0007);
    let allowed: Vec<&str> = TEE_REFUSAL_CODES
        .iter()
        .chain(TEE_EXTRA_REFUSAL_CODES.iter())
        .copied()
        .collect();
    let (mut ok_n, mut refused_n) = (0, 0);
    for it in 0..700 {
        // two thirds of the draws start from a valid spec and corrupt one field, the rest are fully random
        let mut s = TeeMainSpec::new(
            1016.0,
            12.0,
            1500.0,
            vec![
                br("1", 610.0, 9.0, 750.0, 0.0, 0.0),
                br("2", 200.0, 6.0, 1200.0, 180.0, 0.0),
            ],
        );
        if it % 3 == 0 {
            s.d = rng.pick(&POOL);
            s.t = rng.pick(&POOL);
            s.l = rng.pick(&POOL);
            s.branches[0].d = rng.pick(&POOL);
            s.branches[0].t = rng.pick(&POOL);
            s.branches[0].e = rng.pick(&POOL);
            s.branches[0].dir_deg = rng.pick(&POOL);
            s.branches[0].x = Some(rng.pick(&POOL));
            s.seam_deg = rng.pick(&POOL);
            s.seams = (rng.next() % 16) as u32;
            s.gap = Some(rng.pick(&POOL));
        } else {
            match rng.next() % 9 {
                0 => s.d = rng.pick(&POOL),
                1 => s.t = rng.pick(&POOL),
                2 => s.l = rng.pick(&POOL),
                3 => s.branches[1].d = rng.pick(&POOL),
                4 => s.branches[1].e = rng.pick(&POOL),
                5 => s.branches[0].dir_deg = rng.pick(&POOL),
                6 => s.branches[1].x = Some(rng.pick(&POOL)),
                7 => s.branches[0].angle_deg = Some(rng.pick(&POOL)),
                _ => s.seams = (rng.next() % 16) as u32,
            }
        }
        if it % 5 == 0 {
            s.mount = Mount::SetIn;
        }
        match tee::tee_main(&s) {
            Ok(r) => {
                ok_n += 1;
                check_finite_main(&r);
            }
            Err(e) => {
                refused_n += 1;
                assert!(
                    allowed.contains(&e.code),
                    "uncoded refusal {}: {}",
                    e.code,
                    e.message
                );
            }
        }
        let bp = BranchPieceSpec {
            d: rng.pick(&POOL),
            t: rng.pick(&POOL),
            main_d: rng.pick(&POOL),
            main_t: rng.pick(&POOL),
            h: rng.pick(&POOL),
            e: rng.pick(&POOL),
            mount: if it % 2 == 0 {
                Mount::SetOn
            } else {
                Mount::SetIn
            },
            angle_deg: None,
        };
        match tee::branch_piece(&bp) {
            Ok(r) => {
                assert!(r.net_area.is_finite() && r.mass_kg.is_finite() && r.shortest.is_finite());
                if r.certified {
                    assert!(r.eps_rep_certified && r.eps_rep <= 0.05);
                }
                assert!(r.marks.iter().all(|m| m.len.is_finite() && m.len > 0.0));
            }
            Err(e) => assert!(allowed.contains(&e.code), "{}", e.code),
        }
        let _ = tee::signed_offset(rng.pick(&POOL), Some(rng.pick(&POOL)), rng.pick(&POOL));
    }
    println!("fuzz tee: {ok_n} built, {refused_n} refused with a tee_* code");
    assert!(
        ok_n >= 50 && refused_n >= 200,
        "fuzz must exercise both outcomes ({ok_n}/{refused_n})"
    );
}

/// The worst accepted input stays fast (no hang): thin wall, huge branch ratio.
#[test]
fn extreme_but_valid_inputs_finish_quickly() {
    let t0 = std::time::Instant::now();
    let _ = piece(1.0, 0.4, 1e6, 10.0, 1e6, 0.0, Mount::SetOn);
    let _ = piece(999.999, 0.001, 1000.0, 0.001, 2000.0, 0.0, Mount::SetOn);
    let _ = tee::tee_main(&TeeMainSpec::new(
        1e6,
        1e-3,
        1e7,
        vec![
            br("1", 1e5, 1e-3, 5e6, 0.0, 0.0),
            br("2", 10.0, 1.0, 1e6, 170.0, 5.0),
        ],
    ));
    assert!(t0.elapsed().as_secs() < 20, "took {:?}", t0.elapsed());
}
