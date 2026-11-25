//! Acceptance tests of A-21 (nf-form3d): ear-clipping with one hole (bridge method), the branch stub and the header
//! shell with its opening, and refusal of the eccentric reducer and of arc edges.
//!
//! Engine patterns (uem::develop) with the parameters of stored compatibility fixtures in
//! crates/nf-core/tests/fixtures/compat_0.4.0.json: entry i = 7 (branch_stub: header 150, branch 100, beta 60 deg,
//! delta 0, s_end 400) and entry i = 13 (header_with_branch: header_dev_radius 150, header_length 1200,
//! branch_station 600, the same branch), whose header shell is the pattern used here.
//!
//! Area acceptance: the triangle area sum equals the net area of the plate within 1e-9 relative. The net area is the
//! area of the drawn outline minus the holes, and for the engine patterns it is also compared with the pattern's own
//! `area` quantity. Surface acceptance: every triangle vertex lies on the declared surface within 1e-9 times the
//! largest dimension M of the formed part.

use nf_form3d::triangulate::{net_area, pattern_polygons, signed_area, triangulate};
use nf_form3d::{Form3d, Form3dError};
use uem::geometry::Branch;
use uem::{P2, Pattern, develop};

fn p(x: f64, y: f64) -> P2 {
    P2::new(x, y)
}

/// Sum of the triangle areas, each taken as a positive number (every triangle must be counter-clockwise).
fn triangle_area_sum(verts: &[P2], tris: &[[usize; 3]]) -> f64 {
    tris.iter()
        .map(|&[a, b, c]| {
            let area = signed_area(&[verts[a], verts[b], verts[c]]);
            assert!(
                area > 0.0,
                "triangle {a} {b} {c} is not counter-clockwise: {area}"
            );
            area
        })
        .sum()
}

fn check_area(outer: &[P2], holes: &[Vec<P2>]) -> usize {
    let t = triangulate(outer, holes).unwrap();
    let net = net_area(outer, holes);
    let sum = triangle_area_sum(&t.vertices, &t.triangles);
    assert!(
        (sum - net).abs() <= 1e-9 * net.abs(),
        "triangle area sum {sum} differs from the net area {net}"
    );
    t.triangles.len()
}

#[test]
fn square_with_a_square_hole() {
    let outer = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    // the hole given clockwise, and counter-clockwise to show the normalisation
    let hole_cw = vec![p(3.0, 3.0), p(3.0, 7.0), p(7.0, 7.0), p(7.0, 3.0)];
    let hole_ccw = vec![p(3.0, 3.0), p(7.0, 3.0), p(7.0, 7.0), p(3.0, 7.0)];
    // V = 8 vertices with one hole: V + 2h - 2 = 8 triangles
    assert_eq!(check_area(&outer, &[hole_cw]), 8);
    assert_eq!(check_area(&outer, &[hole_ccw]), 8);
    assert_eq!(
        net_area(
            &outer,
            &[vec![p(3.0, 3.0), p(7.0, 3.0), p(7.0, 7.0), p(3.0, 7.0)]]
        ),
        84.0
    );
}

#[test]
fn non_convex_outline_with_a_hole_near_a_reflex_corner() {
    // an L-shaped plate: outline area 300, hole area 4
    let outer = [
        p(0.0, 0.0),
        p(20.0, 0.0),
        p(20.0, 10.0),
        p(10.0, 10.0),
        p(10.0, 20.0),
        p(0.0, 20.0),
    ];
    let hole = vec![p(7.0, 7.0), p(9.0, 7.0), p(9.0, 9.0), p(7.0, 9.0)];
    // V = 6 + 4 vertices with one hole: V + 2h - 2 = 10 triangles
    let n = check_area(&outer, std::slice::from_ref(&hole));
    assert_eq!(n, 10);
    assert!((net_area(&outer, &[hole]) - 296.0).abs() < 1e-12);
}

#[test]
fn outline_only_is_ear_clipped_to_n_minus_two_triangles() {
    let outer: Vec<P2> = (0..12)
        .map(|i| {
            let a = i as f64 * std::f64::consts::TAU / 12.0;
            // a star-shaped, non-convex polygon
            let r = if i % 2 == 0 { 10.0 } else { 6.0 };
            p(r * a.cos(), r * a.sin())
        })
        .collect();
    assert_eq!(check_area(&outer, &[]), 10);
}

/// Each engine vertex sits on its surface: the branch and the header are cylinders of the developed radius.
fn on_cylinder_surface(form: &Form3d, t: &nf_form3d::triangulate::Triangulation, radius: f64) {
    let tol = 1e-9 * form.largest_dimension();
    for &uv in &t.vertices {
        let x = form.map(uv);
        let off = match form {
            Form3d::Header { .. } => (x[1].hypot(x[2]) - radius).abs(),
            _ => (x[0].hypot(x[1]) - radius).abs(),
        };
        assert!(
            off <= tol,
            "{uv:?} -> {x:?} is {off:e} mm off the cylinder of radius {radius}"
        );
    }
}

/// The engine's own net area: `net_area` where the pattern reports it (the header with its opening), else `area`.
fn engine_area(pat: &Pattern) -> f64 {
    pat.quantity("net_area")
        .or_else(|| pat.quantity("area"))
        .expect("the pattern states its area")
}

#[test]
fn branch_stub_of_the_fixture_triangulates_to_its_area() {
    let pat = develop::branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap();
    let (outer, holes) = pattern_polygons(&pat).unwrap();
    assert!(holes.is_empty());
    let t = triangulate(&outer, &holes).unwrap();
    let sum = triangle_area_sum(&t.vertices, &t.triangles);
    let net = net_area(&outer, &holes);
    assert!(
        (sum - net).abs() <= 1e-9 * net,
        "triangles {sum} vs net {net}"
    );
    assert!(
        (net - engine_area(&pat)).abs() <= 1e-9 * engine_area(&pat),
        "net {net} vs engine area {}",
        engine_area(&pat)
    );
    let form = Form3d::from_pattern(&pat).unwrap();
    on_cylinder_surface(&form, &t, 100.0);
}

#[test]
fn header_shell_with_opening_triangulates_to_its_area() {
    let branch = Branch {
        big_r: 150.0,
        r: 100.0,
        beta: 60f64.to_radians(),
        delta: 0.0,
    };
    let pat = develop::header_with_opening(&branch, 150.0, 1200.0, 600.0).unwrap();
    assert_eq!(pat.holes.len(), 1, "the opening is the one hole");
    let (outer, holes) = pattern_polygons(&pat).unwrap();
    assert_eq!(holes.len(), 1);
    let t = triangulate(&outer, &holes).unwrap();
    let sum = triangle_area_sum(&t.vertices, &t.triangles);
    let net = net_area(&outer, &holes);
    assert!(
        (sum - net).abs() <= 1e-9 * net,
        "triangles {sum} vs net {net}"
    );
    assert!(
        (net - engine_area(&pat)).abs() <= 1e-9 * engine_area(&pat),
        "net {net} vs engine area {}",
        engine_area(&pat)
    );
    let form = Form3d::from_pattern(&pat).unwrap();
    on_cylinder_surface(&form, &t, 150.0);
}

#[test]
fn eccentric_reducer_and_arc_edges_are_refused() {
    let ecc = develop::eccentric_reducer(200.0, 100.0, 300.0, 20.0).unwrap();
    assert!(matches!(
        Form3d::from_pattern(&ecc),
        Err(Form3dError::NotFormable {
            family: "eccentric_reducer",
            ..
        })
    ));
    let cone = develop::cone_frustum(200.0, 100.0, 300.0).unwrap();
    assert!(matches!(
        pattern_polygons(&cone),
        Err(Form3dError::NotFormable {
            family: "cone_frustum",
            ..
        })
    ));
}

#[test]
fn bad_input_is_refused_not_approximated() {
    let outer = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    // a hole outside the outline cannot be bridged
    let outside = vec![p(20.0, 20.0), p(22.0, 20.0), p(22.0, 22.0)];
    assert!(matches!(
        triangulate(&outer, &[outside]),
        Err(Form3dError::NotTriangulable(_))
    ));
    // a degenerate outline
    assert!(matches!(
        triangulate(&[p(0.0, 0.0), p(1.0, 1.0), p(2.0, 2.0)], &[]),
        Err(Form3dError::NotTriangulable(_))
    ));
}
