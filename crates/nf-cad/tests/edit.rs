//! NF CAD phase 2 edits: each edit returns a new version (the input bytes are unchanged), carries the marker,
//! reads back, and refuses what it cannot regenerate.

use nf_cad::edit::{apply, is_edited, NewEntity, Op, MARK_TEXT};
use nf_cad::geom::Geom;
use nf_cad::open;

fn dxf(entities: &str) -> Vec<u8> {
    let s = format!(
        "0\nSECTION\n2\nHEADER\n9\n$HANDSEED\n5\nFF\n0\nENDSEC\n\
         0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n0\nLAYER\n2\n0\n62\n7\n70\n0\n0\nLAYER\n2\nCUT\n62\n1\n70\n0\n0\nENDTAB\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    );
    s.into_bytes()
}

const ARC: &str = "0\nARC\n8\n0\n5\nA1\n10\n1\n20\n1\n40\n1\n50\n0\n51\n90\n";
const LINE: &str = "0\nLINE\n8\n0\n5\nB1\n10\n0\n20\n0\n11\n3\n21\n0\n";
const POLY: &str = "0\nPOLYLINE\n8\n0\n5\nC1\n66\n1\n70\n0\n10\n0\n20\n0\n\
                    0\nVERTEX\n8\n0\n10\n0\n20\n0\n\
                    0\nVERTEX\n8\n0\n10\n2\n20\n0\n\
                    0\nVERTEX\n8\n0\n10\n2\n20\n2\n\
                    0\nSEQEND\n8\n0\n";

fn only(bytes: &[u8], kind: &str) -> nf_cad::read::Entity {
    let d = open(bytes).unwrap();
    d.entities.into_iter().find(|e| e.kind == kind).expect("entity")
}

#[test]
fn move_creates_a_new_marked_version_and_leaves_the_input_alone() {
    let parent = dxf(LINE);
    let child = apply(&parent, &[Op::Move { ids: vec!["E0".into()], dx: 5.0, dy: -1.0 }]).unwrap();
    assert!(is_edited(&child), "child carries the marker");
    assert!(!is_edited(&parent), "parent is untouched and unmarked");
    assert_eq!(open(&parent).unwrap().entities[0].geom, Geom::Line { a: [0.0, 0.0], b: [3.0, 0.0] });
    match open(&child).unwrap().entities[0].geom.clone() {
        Geom::Line { a, b } => {
            assert_eq!((a, b), ([5.0, -1.0], [8.0, -1.0]));
        }
        g => panic!("{g:?}"),
    }
    assert!(String::from_utf8_lossy(&child).contains(MARK_TEXT));
}

#[test]
fn rotate_and_mirror_of_an_arc_match_closed_forms() {
    // arc centre (1,1) r 1 from 0 to 90; rotate 90 about the origin -> centre (-1,1), angles 90..180
    let bytes = dxf(ARC);
    let r = apply(&bytes, &[Op::Rotate { ids: vec!["E0".into()], cx: 0.0, cy: 0.0, deg: 90.0 }]).unwrap();
    match only(&r, "ARC").geom {
        Geom::Arc { c, start, end, .. } => {
            assert!((c[0] + 1.0).abs() < 1e-9 && (c[1] - 1.0).abs() < 1e-9, "c {c:?}");
            assert!((start - 90.0).abs() < 1e-9 && (end - 180.0).abs() < 1e-9, "{start} {end}");
        }
        g => panic!("{g:?}"),
    }
    // mirror across the x axis: centre (1,-1), swept ccw from 270 to 0
    let m = apply(&bytes, &[Op::Mirror { ids: vec!["E0".into()], a: [0.0, 0.0], b: [1.0, 0.0] }]).unwrap();
    match only(&m, "ARC").geom {
        Geom::Arc { c, start, end, .. } => {
            assert!((c[0] - 1.0).abs() < 1e-9 && (c[1] + 1.0).abs() < 1e-9, "c {c:?}");
            assert!((start - 270.0).abs() < 1e-9 && end.abs() < 1e-9, "{start} {end}");
        }
        g => panic!("{g:?}"),
    }
}

#[test]
fn scale_and_delete_polyline_removes_its_whole_span() {
    let bytes = dxf(POLY);
    let s = apply(&bytes, &[Op::Scale { ids: vec!["E0".into()], cx: 0.0, cy: 0.0, factor: 2.0 }]).unwrap();
    match only(&s, "POLYLINE").geom {
        Geom::Poly { v, .. } => assert_eq!(v[2].p, [4.0, 4.0]),
        g => panic!("{g:?}"),
    }
    let d = apply(&bytes, &[Op::Delete { ids: vec!["E0".into()] }]).unwrap();
    let text = String::from_utf8_lossy(&d);
    assert!(!text.contains("VERTEX") && !text.contains("SEQEND"), "span removed");
    assert_eq!(open(&d).unwrap().entities.len(), 0);
}

#[test]
fn set_layer_and_add_use_the_table_and_a_fresh_handle() {
    let bytes = dxf(LINE);
    let l = apply(&bytes, &[Op::SetLayer { ids: vec!["E0".into()], layer: "CUT".into() }]).unwrap();
    assert_eq!(only(&l, "LINE").layer, "CUT");
    let bad = apply(&bytes, &[Op::SetLayer { ids: vec!["E0".into()], layer: "NOPE".into() }]);
    assert_eq!(bad.unwrap_err().code, "layer_missing");

    let added = apply(
        &bytes,
        &[Op::Add { entity: NewEntity::Circle { c: [5.0, 5.0], r: 2.0, layer: "CUT".into() } }],
    )
    .unwrap();
    let text = String::from_utf8_lossy(&added).to_string();
    assert_eq!(only(&added, "CIRCLE").handle.as_deref(), Some("100"), "max handle FF + 1");
    // $HANDSEED was FF; the new circle takes 100 (hex, after the largest handle in the file: B1 is below FF)
    let c = only(&added, "CIRCLE");
    assert_eq!(c.layer, "CUT");
    assert!(text.contains("$HANDSEED\n5\n101\n"), "handseed moved past the new handle");
    match c.geom {
        Geom::Circle { c, r } => assert_eq!((c, r), ([5.0, 5.0], 2.0)),
        g => panic!("{g:?}"),
    }
}

#[test]
fn refusals_are_explicit_and_leave_nothing_behind() {
    let bytes = dxf("0\nELLIPSE\n8\n0\n10\n0\n20\n0\n11\n2\n21\n0\n40\n0.5\n41\n0\n42\n6.28\n");
    let e = apply(&bytes, &[Op::Move { ids: vec!["E0".into()], dx: 1.0, dy: 0.0 }]).unwrap_err();
    assert_eq!(e.code, "unsupported_op", "ellipse is listed, not editable");

    let t = dxf("0\nTEXT\n8\n0\n10\n0\n20\n0\n40\n2\n1\nHI\n");
    let e = apply(&t, &[Op::Mirror { ids: vec!["E0".into()], a: [0.0, 0.0], b: [0.0, 1.0] }]).unwrap_err();
    assert_eq!(e.code, "unsupported_op");

    let e = apply(&bytes, &[Op::Delete { ids: vec!["E9".into()] }]).unwrap_err();
    assert_eq!(e.code, "unknown_entity");

    let e = apply(&bytes, &[Op::Scale { ids: vec![], cx: 0.0, cy: 0.0, factor: -1.0 }]).unwrap_err();
    assert_eq!(e.code, "bad_request");
}

#[test]
fn block_contents_are_edited_only_through_their_insert() {
    let blocks = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n8\n0\n2\nB\n10\n0\n20\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n0\nENDSEC\n";
    let base = format!("{}", String::from_utf8_lossy(&dxf("0\nINSERT\n8\n0\n2\nB\n10\n0\n20\n0\n")));
    let with_blocks = base.replace("0\nEOF\n", &format!("{blocks}0\nEOF\n"));
    let d = open(with_blocks.as_bytes()).unwrap();
    let child = d.entities.iter().find(|e| e.parent.is_some()).expect("child");
    let e = apply(with_blocks.as_bytes(), &[Op::Move { ids: vec![child.id.clone()], dx: 1.0, dy: 0.0 }]).unwrap_err();
    assert_eq!(e.code, "unknown_entity");
    // the INSERT itself can move
    let moved = apply(with_blocks.as_bytes(), &[Op::Move { ids: vec!["E0".into()], dx: 4.0, dy: 0.0 }]).unwrap();
    let c2 = open(&moved).unwrap();
    let ins = c2.entities.iter().find(|e| e.kind == "INSERT").unwrap();
    assert_eq!(ins.kind, "INSERT");
    let line = c2.entities.iter().find(|e| e.parent.is_some()).unwrap();
    match line.geom {
        Geom::Line { a, .. } => assert_eq!(a, [4.0, 0.0]),
        ref g => panic!("{g:?}"),
    }
}

#[test]
fn quality_gate_runs_on_an_edited_version_and_marks_it() {
    let child = apply(&dxf(LINE), &[Op::Move { ids: vec!["E0".into()], dx: 1.0, dy: 1.0 }]).unwrap();
    let checks = nf_cad::quality_checks(&child);
    assert_eq!(checks.len(), nf_dxf::quality::RULES.len(), "every rule reported");
    assert!(is_edited(&child));
}

#[test]
fn mirror_about_any_line_angle_through_a_point() {
    // line (0,0)-(3,0); mirror about the vertical line through the origin (90 degrees) -> (0,0)-(-3,0)
    let bytes = dxf(LINE);
    let m = apply(&bytes, &[Op::MirrorAngle { ids: vec!["E0".into()], c: [0.0, 0.0], deg: 90.0 }]).unwrap();
    match only(&m, "LINE").geom {
        Geom::Line { a, b } => {
            assert!(a[0].abs() < 1e-9 && a[1].abs() < 1e-9, "a {a:?}");
            assert!((b[0] + 3.0).abs() < 1e-9 && b[1].abs() < 1e-9, "b {b:?}");
        }
        g => panic!("{g:?}"),
    }
    // 0 degrees through the origin: the x axis, so (3,0) stays put
    let m = apply(&bytes, &[Op::MirrorAngle { ids: vec!["E0".into()], c: [0.0, 0.0], deg: 0.0 }]).unwrap();
    match only(&m, "LINE").geom {
        Geom::Line { b, .. } => assert!((b[0] - 3.0).abs() < 1e-9 && b[1].abs() < 1e-9, "b {b:?}"),
        g => panic!("{g:?}"),
    }
}
