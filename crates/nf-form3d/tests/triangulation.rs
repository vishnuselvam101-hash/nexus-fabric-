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
//! `area` quantity. Chord acceptance (A-21 review, round 2): the cylinder shell, the branch stub and the header with
//! its opening are triangulated by `triangulate_formed` (slab decomposition) to a chord tolerance of 0.01 mm, and every
//! triangle has positive area in 3D with its centroid and edge midpoints within the tolerance of the surface (up to
//! 1e-9 times the largest dimension M).

use nf_form3d::triangulate::{
    Triangulation, net_area, pattern_polygons, signed_area, triangulate, triangulate_formed,
};
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

/// Chord bound of the seams (mm), and the allowance for rounding (1e-9 times the largest dimension M).
const CHORD: f64 = 0.01;

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mid(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ]
}

/// Checks a triangulation of a formed cylinder or header of the given radius to the chord bound: every triangle has
/// positive area in 3D (none collapses onto the seam), and the centroid and the edge midpoints of every triangle lie
/// within `chord` (mm) of the surface.
fn check_chord(form: &Form3d, radius: f64, chord: f64, t: &Triangulation) {
    let tol = chord + 1e-9 * form.largest_dimension();
    let off = |x: [f64; 3]| {
        let r = match form {
            Form3d::Header { .. } => x[1].hypot(x[2]),
            _ => x[0].hypot(x[1]),
        };
        (r - radius).abs()
    };
    for &[a, b, c] in &t.triangles {
        let (va, vb, vc) = (t.vertices[a], t.vertices[b], t.vertices[c]);
        let plate = signed_area(&[va, vb, vc]);
        assert!(plate > 0.0, "triangle {a} {b} {c} is not counter-clockwise");
        let (pa, pb, pc) = (form.map(va), form.map(vb), form.map(vc));
        let n = {
            let (e1, e2) = (sub(pb, pa), sub(pc, pa));
            [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ]
        };
        let area3 = 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!(
            area3 > 0.5 * plate,
            "triangle {a} {b} {c}: 3D area {area3} against plate area {plate}"
        );
        let centroid = [
            (pa[0] + pb[0] + pc[0]) / 3.0,
            (pa[1] + pb[1] + pc[1]) / 3.0,
            (pa[2] + pb[2] + pc[2]) / 3.0,
        ];
        let o = off(centroid);
        assert!(
            o <= tol,
            "triangle {a} {b} {c}: centroid {o:e} mm off the surface"
        );
        for (x, y) in [(pa, pb), (pb, pc), (pc, pa)] {
            let o = off(mid(x, y));
            assert!(
                o <= tol,
                "triangle {a} {b} {c}: edge midpoint {o:e} mm off the surface"
            );
        }
    }
}

/// The engine's own net area: `net_area` where the pattern reports it (the header with its opening), else `area`.
fn engine_area(pat: &Pattern) -> f64 {
    pat.quantity("net_area")
        .or_else(|| pat.quantity("area"))
        .expect("the pattern states its area")
}

/// The triangle area sum of a formed triangulation equals the net area of the plate, and the net area equals the
/// engine's own area of the pattern, both within 1e-9 relative.
fn check_formed_area(t: &Triangulation, outer: &[P2], holes: &[Vec<P2>], pat: &Pattern) {
    let sum = triangle_area_sum(&t.vertices, &t.triangles);
    let net = net_area(outer, holes);
    assert!(
        (sum - net).abs() <= 1e-9 * net,
        "triangles {sum} vs net {net}"
    );
    assert!(
        (net - engine_area(pat)).abs() <= 1e-9 * engine_area(pat),
        "net {net} vs engine area {}",
        engine_area(pat)
    );
}

#[test]
fn branch_stub_of_the_fixture_triangulates_within_the_chord_bound() {
    let pat = develop::branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap();
    let (outer, holes) = pattern_polygons(&pat).unwrap();
    assert!(holes.is_empty());
    let form = Form3d::from_pattern(&pat).unwrap();
    let t = triangulate_formed(&form, &outer, &holes, CHORD).unwrap();
    // the branch cylinder has radius 100 mm (its developed width is 2 pi r)
    check_chord(&form, 100.0, CHORD, &t);
    check_formed_area(&t, &outer, &holes, &pat);
}

#[test]
fn header_shell_with_opening_triangulates_within_the_chord_bound() {
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
    let form = Form3d::from_pattern(&pat).unwrap();
    let t = triangulate_formed(&form, &outer, &holes, CHORD).unwrap();
    // the header shell has radius 150 mm (its developed width is 2 pi R)
    check_chord(&form, 150.0, CHORD, &t);
    check_formed_area(&t, &outer, &holes, &pat);
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

#[test]
fn two_holes_with_repeated_bridge_vertices_are_triangulated() {
    // review A-21 finding 2: the second bridge must enter the copy of the repeated outline vertex whose wedge holds it
    let outer = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    let h1 = vec![
        p(6.673, 6.814),
        p(5.33, 7.82),
        p(3.958, 6.852),
        p(4.454, 5.249),
        p(6.132, 5.225),
    ];
    let h2 = vec![
        p(6.308, 8.764),
        p(5.823, 9.126),
        p(5.329, 8.778),
        p(5.508, 8.2),
        p(6.113, 8.192),
    ];
    // V = 4 + 5 + 5 vertices with two holes: V + 2h - 2 = 16 triangles
    assert_eq!(check_area(&outer, &[h1, h2]), 16);
}

#[test]
fn seeded_random_multi_hole_plates_are_triangulated() {
    // holes are star-shaped polygons in the cells of a 3 x 3 grid, so they are disjoint and inside the outline
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let outer = [p(0.0, 0.0), p(90.0, 0.0), p(90.0, 90.0), p(0.0, 90.0)];
    for case in 0..300 {
        let mut cells: Vec<usize> = (0..9).collect();
        let count = 2 + (next() * 4.0) as usize;
        let mut holes = Vec::new();
        for _ in 0..count {
            let pick = (next() * cells.len() as f64) as usize % cells.len();
            let cell = cells.swap_remove(pick);
            let (cx, cy) = (
                15.0 + 30.0 * (cell % 3) as f64 + 10.0 * (next() - 0.5),
                15.0 + 30.0 * (cell / 3) as f64 + 10.0 * (next() - 0.5),
            );
            let n = 3 + (next() * 6.0) as usize;
            let hole: Vec<P2> = (0..n)
                .map(|k| {
                    let step = std::f64::consts::TAU / n as f64;
                    let a = (k as f64 + 0.8 * next()) * step;
                    let r = 9.0 * (0.4 + 0.6 * next());
                    p(cx + r * a.cos(), cy + r * a.sin())
                })
                .collect();
            holes.push(hole);
        }
        let t = triangulate(&outer, &holes)
            .unwrap_or_else(|e| panic!("case {case}: {e} for {holes:?}"));
        let sum = triangle_area_sum(&t.vertices, &t.triangles);
        let net = net_area(&outer, &holes);
        assert!(
            (sum - net).abs() <= 1e-9 * net,
            "case {case}: triangle area sum {sum} differs from the net area {net}"
        );
    }
}

#[test]
fn zero_area_corner_that_carries_area_is_refused() {
    // review A-21 finding 3: the sharp corner at (0,0) has sin(angle) ~ 1e-13 and carries half the area
    let sliver = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(1.0, 1e-13)];
    assert!(matches!(
        triangulate(&sliver, &[]),
        Err(Form3dError::NotTriangulable(_))
    ));
}

#[test]
fn full_turn_seams_are_chord_bounded_and_refined_to_the_tolerance() {
    // a cylinder of radius 100 mm: the two long edges u = 0 and u = 2 pi R are the seam, and both map to the same
    // 3D points, so without subdivision every triangle along them would collapse or cut through the interior
    let cyl = Form3d::cylinder(100.0, 250.0).unwrap();
    let outer = [p(0.0, 0.0), p(628.0, 0.0), p(628.0, 250.0), p(0.0, 250.0)];
    let fine = triangulate_formed(&cyl, &outer, &[], CHORD).unwrap();
    assert!(
        (triangle_area_sum(&fine.vertices, &fine.triangles) - 157000.0).abs() <= 1e-9 * 157000.0
    );
    check_chord(&cyl, 100.0, CHORD, &fine);
    // a coarser tolerance subdivides less
    let coarse = triangulate_formed(&cyl, &outer, &[], 0.5).unwrap();
    assert!(fine.vertices.len() > coarse.vertices.len());
    check_chord(&cyl, 100.0, 0.5, &coarse);
}

#[test]
fn chord_bound_refuses_a_bad_tolerance_and_the_cone() {
    let outer = [p(0.0, 0.0), p(628.0, 0.0), p(628.0, 250.0), p(0.0, 250.0)];
    let cyl = Form3d::cylinder(100.0, 250.0).unwrap();
    assert!(matches!(
        triangulate_formed(&cyl, &outer, &[], 0.0),
        Err(Form3dError::NotPositive("chord_tol"))
    ));
    // at or above half the radius the chord bound does not hold for the whole triangle
    assert!(matches!(
        triangulate_formed(&cyl, &outer, &[], 50.0),
        Err(Form3dError::NotTriangulable(_))
    ));
    // the cone has no chord-bounded triangulation here
    let cone = Form3d::cone(200.0, 100.0, 300.0).unwrap();
    assert!(matches!(
        triangulate_formed(&cone, &outer, &[], CHORD),
        Err(Form3dError::NotTriangulable(_))
    ));
}
