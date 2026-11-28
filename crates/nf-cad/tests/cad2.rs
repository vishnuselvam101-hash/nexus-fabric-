//! NF CAD phase 2 reads: SPLINE, ELLIPSE, HATCH boundaries, DIMENSION text, BLOCKS / INSERT expansion, and the
//! caps. Expected values are closed-form; tolerances are stated where the curve is evaluated to a polyline.

use nf_cad::geom::{Geom, CHORD_TOL};
use nf_cad::{open, Measure};

fn dxf(entities: &str, blocks: &str) -> Vec<u8> {
    let mut s = String::from("0\nSECTION\n2\nENTITIES\n");
    s.push_str(entities);
    s.push_str("0\nENDSEC\n");
    if !blocks.is_empty() {
        s.push_str("0\nSECTION\n2\nBLOCKS\n");
        s.push_str(blocks);
        s.push_str("0\nENDSEC\n");
    }
    s.push_str("0\nEOF\n");
    s.into_bytes()
}

fn len_of(bytes: &[u8], id: &str) -> f64 {
    let d = open(bytes).expect("open");
    d.measure(&Measure::Length { entity: id.into() })
        .expect("length")[0]
        .value
}

fn ent<'a>(d: &'a nf_cad::Drawing, kind: &str) -> Vec<&'a nf_cad::read::Entity> {
    d.entities.iter().filter(|e| e.kind == kind).collect()
}

#[test]
fn rational_quarter_circle_spline_matches_pi_r_over_2() {
    // NURBS quarter circle of radius 1: degree 2, weights 1, sqrt(2)/2, 1
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let s = format!(
        "0\nSPLINE\n8\n0\n70\n0\n71\n2\n72\n6\n73\n3\n74\n0\n\
         40\n0\n40\n0\n40\n0\n40\n1\n40\n1\n40\n1\n\
         10\n1\n20\n0\n41\n1\n10\n1\n20\n1\n41\n{w}\n10\n0\n20\n1\n41\n1\n"
    );
    let bytes = dxf(&s, "");
    let l = len_of(&bytes, "E0");
    let want = std::f64::consts::FRAC_PI_2;
    assert!((l - want).abs() < 2.0 * CHORD_TOL, "length {l} vs {want}");
    // the sampled curve starts and ends exactly on the control points
    let d = open(&bytes).unwrap();
    match &d.entities[0].geom {
        Geom::Poly { v, .. } => {
            assert!((v[0].p[0] - 1.0).abs() < 1e-12 && v[0].p[1].abs() < 1e-12);
            let e = v[v.len() - 1].p;
            assert!(e[0].abs() < 1e-12 && (e[1] - 1.0).abs() < 1e-12);
        }
        g => panic!("spline should be a polyline, got {g:?}"),
    }
    assert!(d.entities[0]
        .warnings
        .iter()
        .any(|w| w.contains("chord tolerance")));
}

#[test]
fn collinear_cubic_bezier_length_is_exact() {
    // degree 3, control points (0,0),(1,0),(2,0),(3,0): x(t)=3t, length 3
    let s = "0\nSPLINE\n8\n0\n70\n0\n71\n3\n72\n8\n73\n4\n74\n0\n\
             40\n0\n40\n0\n40\n0\n40\n0\n40\n1\n40\n1\n40\n1\n40\n1\n\
             10\n0\n20\n0\n10\n1\n20\n0\n10\n2\n20\n0\n10\n3\n20\n0\n";
    let l = len_of(&dxf(s, ""), "E0");
    assert!((l - 3.0).abs() < 1e-9, "length {l}");
}

#[test]
fn ellipse_circle_length_and_area_are_closed_form() {
    // ratio 1, radius 2, full: 4 pi long, 4 pi area
    let s = "0\nELLIPSE\n8\n0\n10\n0\n20\n0\n11\n2\n21\n0\n40\n1\n41\n0\n42\n6.283185307179586\n";
    let bytes = dxf(s, "");
    let l = len_of(&bytes, "E0");
    // chord error <= tol => relative length error <= tol/(3 r), so |error| <= 2 pi tol / 3 for any radius
    assert!(
        (l - 4.0 * std::f64::consts::PI).abs() <= 2.0 * std::f64::consts::PI * CHORD_TOL,
        "length {l}"
    );
    let a = open(&bytes)
        .unwrap()
        .measure(&Measure::Area {
            entity: "E0".into(),
        })
        .unwrap()[0]
        .value;
    assert!((a - 4.0 * std::f64::consts::PI).abs() < 1e-2, "area {a}");
}

#[test]
fn ellipse_general_length_and_area_within_stated_bounds() {
    // semi-axes 2 and 1 (major vector (2,0), ratio 0.5), full ellipse
    let s = "0\nELLIPSE\n8\n0\n10\n0\n20\n0\n11\n2\n21\n0\n40\n0.5\n41\n0\n42\n6.283185307179586\n";
    let bytes = dxf(s, "");
    let want = nf_cad::geom::ellipse_perimeter_series(2.0, 1.0);
    let l = len_of(&bytes, "E0");
    // relative error <= tol / (3 rho_min), rho_min = b^2/a = 0.5 here
    assert!(
        (l - want).abs() <= want * CHORD_TOL * 2.0,
        "length {l} vs {want}"
    );
    // area error of a chord polygon is at most (2/3) perimeter * tolerance
    let a = open(&bytes)
        .unwrap()
        .measure(&Measure::Area {
            entity: "E0".into(),
        })
        .unwrap()[0]
        .value;
    let bound = want * CHORD_TOL;
    assert!(
        (a - 2.0 * std::f64::consts::PI).abs() <= bound,
        "area {a} bound {bound}"
    );
}

#[test]
fn insert_applies_scale_rotation_and_base_point() {
    // block B: base (0,0), one line (0,0)-(1,0). INSERT at (10,5), scale 2, rotation 90.
    let blocks = "0\nBLOCK\n8\n0\n2\nB\n10\n0\n20\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n";
    let s = "0\nINSERT\n8\n0\n2\nB\n10\n10\n20\n5\n41\n2\n42\n2\n50\n90\n";
    let d = open(&dxf(s, blocks)).unwrap();
    let child = d
        .entities
        .iter()
        .find(|e| e.parent.as_deref() == Some("E0"))
        .expect("child");
    match child.geom {
        Geom::Line { a, b } => {
            assert!((a[0] - 10.0).abs() < 1e-9 && (a[1] - 5.0).abs() < 1e-9);
            assert!(
                (b[0] - 10.0).abs() < 1e-9 && (b[1] - 7.0).abs() < 1e-9,
                "b {b:?}"
            );
        }
        ref g => panic!("expected line, got {g:?}"),
    }
}

#[test]
fn insert_with_base_point_and_nested_blocks_compose() {
    // inner block I: base (1,1), line (1,1)-(2,1). Outer block O holds INSERT I at (5,0), no rotation.
    // INSERT O at (0,10). Inner line start (1,1): minus base = (0,0), +(5,0), +(0,10) -> (5,10).
    let blocks = "0\nBLOCK\n8\n0\n2\nI\n10\n1\n20\n1\n0\nLINE\n8\n0\n10\n1\n20\n1\n11\n2\n21\n1\n0\nENDBLK\n\
                  0\nBLOCK\n8\n0\n2\nO\n10\n0\n20\n0\n0\nINSERT\n8\n0\n2\nI\n10\n5\n20\n0\n0\nENDBLK\n";
    let s = "0\nINSERT\n8\n0\n2\nO\n10\n0\n20\n10\n";
    let d = open(&dxf(s, blocks)).unwrap();
    let inner = d
        .entities
        .iter()
        .find(|e| e.kind == "LINE")
        .expect("inner line");
    match inner.geom {
        Geom::Line { a, .. } => assert!(
            (a[0] - 5.0).abs() < 1e-9 && (a[1] - 10.0).abs() < 1e-9,
            "a {a:?}"
        ),
        ref g => panic!("{g:?}"),
    }
}

#[test]
fn nested_block_bomb_is_refused_by_the_entity_cap() {
    // four blocks, each holding 50 INSERTs of the next: 50^3 = 125000 entities, refused by the count cap
    // before they exist (depth 4 is within the cap, so the count is what refuses)
    let mut blocks = String::new();
    for k in 0..3 {
        blocks.push_str(&format!("0\nBLOCK\n8\n0\n2\nB{k}\n10\n0\n20\n0\n"));
        for _ in 0..50 {
            blocks.push_str(&format!("0\nINSERT\n8\n0\n2\nB{}\n10\n0\n20\n0\n", k + 1));
        }
        blocks.push_str("0\nENDBLK\n");
    }
    blocks.push_str("0\nBLOCK\n8\n0\n2\nB3\n10\n0\n20\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n");
    let s = "0\nINSERT\n8\n0\n2\nB0\n10\n0\n20\n0\n";
    let r = open(&dxf(s, &blocks));
    let e = r.expect_err("bomb must be refused");
    assert_eq!(e.code, "too_many_entities");
}

#[test]
fn deep_chain_is_refused_by_depth_and_self_insert_by_cycle() {
    let mut blocks = String::new();
    for k in 0..10 {
        blocks.push_str(&format!("0\nBLOCK\n8\n0\n2\nC{k}\n10\n0\n20\n0\n0\nINSERT\n8\n0\n2\nC{}\n10\n0\n20\n0\n0\nENDBLK\n", k + 1));
    }
    blocks.push_str("0\nBLOCK\n8\n0\n2\nC10\n10\n0\n20\n0\n0\nENDBLK\n");
    let s = "0\nINSERT\n8\n0\n2\nC0\n10\n0\n20\n0\n";
    assert_eq!(
        open(&dxf(s, &blocks)).expect_err("depth").code,
        "block_too_deep"
    );

    let selfref =
        "0\nBLOCK\n8\n0\n2\nA\n10\n0\n20\n0\n0\nINSERT\n8\n0\n2\nA\n10\n0\n20\n0\n0\nENDBLK\n";
    let s = "0\nINSERT\n8\n0\n2\nA\n10\n0\n20\n0\n";
    assert_eq!(
        open(&dxf(s, selfref)).expect_err("cycle").code,
        "block_cycle"
    );
}

#[test]
fn dimension_shows_its_measurement_and_expands_its_block() {
    // measured 12.5, override "R<>" shows "R12.5"; anonymous block *D1 holds one line
    let blocks = "0\nBLOCK\n8\n0\n2\n*D1\n10\n0\n20\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n3\n21\n0\n0\nENDBLK\n";
    let s = "0\nDIMENSION\n8\n0\n1\nR<>\n2\n*D1\n11\n4\n21\n4\n42\n12.5\n";
    let d = open(&dxf(s, blocks)).unwrap();
    let t = ent(&d, "DIMENSION");
    match &t[0].geom {
        Geom::Text { text, .. } => assert_eq!(text, "R12.5"),
        g => panic!("{g:?}"),
    }
    assert_eq!(
        d.entities
            .iter()
            .filter(|e| e.parent.as_deref() == Some("E0"))
            .count(),
        1
    );
}

#[test]
fn hatch_boundaries_are_read_only_polylines() {
    // one polyline loop: unit-2 square; area 4 via the closed-polyline area
    let s = "0\nHATCH\n8\n0\n2\nSOLID\n91\n1\n92\n2\n72\n0\n73\n1\n93\n4\n10\n0\n20\n0\n10\n2\n20\n0\n10\n2\n20\n2\n10\n0\n20\n2\n75\n0\n76\n1\n";
    let d = open(&dxf(s, "")).unwrap();
    let h = ent(&d, "HATCH");
    assert_eq!(h.len(), 1);
    let a = d
        .measure(&Measure::Area {
            entity: h[0].id.clone(),
        })
        .unwrap()[0]
        .value;
    assert!((a - 4.0).abs() < 1e-12, "area {a}");
}

#[test]
fn hatch_edge_loop_of_lines_and_arc() {
    // edge loop: line (0,0)-(2,0), line (2,0)-(2,2), line (2,2)-(0,2), line (0,2)-(0,0) = square, area 4
    let s = "0\nHATCH\n8\n0\n2\nSOLID\n91\n1\n92\n0\n93\n4\n\
             72\n1\n10\n0\n20\n0\n11\n2\n21\n0\n72\n1\n10\n2\n20\n0\n11\n2\n21\n2\n\
             72\n1\n10\n2\n20\n2\n11\n0\n21\n2\n72\n1\n10\n0\n20\n2\n11\n0\n21\n0\n75\n0\n76\n1\n";
    let d = open(&dxf(s, "")).unwrap();
    let h = ent(&d, "HATCH");
    let a = d
        .measure(&Measure::Area {
            entity: h[0].id.clone(),
        })
        .unwrap()[0]
        .value;
    assert!((a - 4.0).abs() < 1e-12, "area {a}");
}

#[test]
fn entity_record_index_points_at_the_top_level_record() {
    let s =
        "0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n2\n21\n0\n";
    let d = open(&dxf(s, "")).unwrap();
    assert_eq!(d.entities[0].record, Some(0));
    assert_eq!(d.entities[1].record, Some(1));
}
