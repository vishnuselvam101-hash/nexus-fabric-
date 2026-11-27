//! Entity set 1 (LINE, POINT, CIRCLE, ARC, LWPOLYLINE, POLYLINE, TEXT, MTEXT, SOLID) from hand-written
//! fixtures, and the refusals for malformed records.

mod common;

use nf_cadio::{CadError, Document, Drawing, Entity, Point3, Reason, Units, Vertex};

fn entities_of(body: &str) -> Result<Drawing, CadError> {
    let text = common::dxf(
        &common::header("AC1009", Some(4)),
        &common::section("ENTITIES", body),
    );
    Document::parse(text.as_bytes())?.drawing()
}

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3 { x, y, z }
}

#[test]
fn line_point_circle_and_arc_fields_are_read() {
    let drawing = entities_of(
        "0\nLINE\n8\nCUT\n10\n1.5\n20\n2\n11\n10\n21\n-4\n\
         0\nPOINT\n8\nMARK\n10\n3\n20\n4\n30\n5\n\
         0\nCIRCLE\n10\n1\n20\n2\n40\n6.5\n\
         0\nARC\n8\nCUT\n10\n0\n20\n0\n40\n10\n50\n30\n51\n120\n",
    )
    .expect("valid entities");
    assert_eq!(drawing.entities.len(), 4);
    assert_eq!(drawing.units, Units::Millimeter);
    assert_eq!(
        drawing.entities[0],
        Entity::Line {
            layer: "CUT".into(),
            start: p(1.5, 2.0, 0.0),
            end: p(10.0, -4.0, 0.0),
            non_z_extrusion: false,
        }
    );
    assert_eq!(
        drawing.entities[1],
        Entity::Point {
            layer: "MARK".into(),
            at: p(3.0, 4.0, 5.0),
            non_z_extrusion: false,
        }
    );
    assert_eq!(
        drawing.entities[2],
        Entity::Circle {
            layer: "0".into(),
            center: p(1.0, 2.0, 0.0),
            radius: 6.5,
            non_z_extrusion: false,
        }
    );
    assert_eq!(
        drawing.entities[3],
        Entity::Arc {
            layer: "CUT".into(),
            center: p(0.0, 0.0, 0.0),
            radius: 10.0,
            start_angle: 30.0,
            end_angle: 120.0,
            non_z_extrusion: false,
        }
    );
}

#[test]
fn lwpolyline_bulge_keeps_its_sign() {
    // Vertex 1 has bulge +1 (counter-clockwise), vertex 2 has -0.5 (clockwise), vertex 3 none.
    let drawing = entities_of(
        "0\nLWPOLYLINE\n8\nCUT\n90\n3\n70\n1\n\
         10\n0\n20\n0\n42\n1\n\
         10\n2\n20\n0\n42\n-0.5\n\
         10\n2\n20\n2\n\
         0\nLWPOLYLINE\n70\n0\n38\n7\n10\n0\n20\n0\n10\n1\n20\n1\n",
    )
    .expect("valid polylines");
    match &drawing.entities[0] {
        Entity::LwPolyline {
            layer,
            closed,
            vertices,
            ..
        } => {
            assert_eq!(layer, "CUT");
            assert!(*closed, "bit 1 of group 70 closes the polyline");
            assert_eq!(
                vertices,
                &vec![
                    Vertex {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        bulge: 1.0
                    },
                    Vertex {
                        x: 2.0,
                        y: 0.0,
                        z: 0.0,
                        bulge: -0.5
                    },
                    Vertex {
                        x: 2.0,
                        y: 2.0,
                        z: 0.0,
                        bulge: 0.0
                    },
                ]
            );
        }
        other => panic!("expected LWPOLYLINE, got {other:?}"),
    }
    match &drawing.entities[1] {
        Entity::LwPolyline {
            closed, vertices, ..
        } => {
            assert!(!*closed);
            assert!(
                vertices.iter().all(|v| v.z == 7.0),
                "group 38 is the elevation"
            );
        }
        other => panic!("expected LWPOLYLINE, got {other:?}"),
    }
}

#[test]
fn polyline_2d_and_3d_with_vertices() {
    let drawing = entities_of(
        "0\nPOLYLINE\n8\nCUT\n66\n1\n70\n1\n\
         0\nVERTEX\n10\n0\n20\n0\n42\n0.25\n\
         0\nVERTEX\n10\n5\n20\n0\n\
         0\nSEQEND\n\
         0\nPOLYLINE\n70\n8\n\
         0\nVERTEX\n10\n1\n20\n2\n30\n3\n\
         0\nVERTEX\n10\n4\n20\n5\n30\n6\n\
         0\nSEQEND\n",
    )
    .expect("valid polylines");
    assert_eq!(drawing.entities.len(), 2);
    assert_eq!(
        drawing.entities[0],
        Entity::Polyline {
            layer: "CUT".into(),
            closed: true,
            is_3d: false,
            vertices: vec![
                Vertex {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    bulge: 0.25
                },
                Vertex {
                    x: 5.0,
                    y: 0.0,
                    z: 0.0,
                    bulge: 0.0
                },
            ],
            non_z_extrusion: false,
        }
    );
    assert_eq!(
        drawing.entities[1],
        Entity::Polyline {
            layer: "0".into(),
            closed: false,
            is_3d: true,
            vertices: vec![
                Vertex {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    bulge: 0.0
                },
                Vertex {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                    bulge: 0.0
                },
            ],
            non_z_extrusion: false,
        }
    );
}

#[test]
fn text_and_mtext_keep_raw_and_plain_strings() {
    let drawing = entities_of(
        "0\nTEXT\n8\nNOTE\n10\n1\n20\n2\n40\n3.5\n50\n90\n1\nPART 1\\U+0041\n\
         0\nMTEXT\n8\nNOTE\n10\n0\n20\n0\n40\n2\n3\n{\\H2.5;\\fArial|b0;SS400\n3\n\\P\n1\nNOTE\\~A\\\\\n\
         0\nSOLID\n8\nCUT\n10\n0\n20\n0\n11\n10\n21\n0\n12\n0\n22\n10\n13\n10\n23\n10\n",
    )
    .expect("valid text");
    assert_eq!(drawing.entities.len(), 3);
    assert_eq!(
        drawing.entities[0],
        Entity::Text {
            layer: "NOTE".into(),
            at: p(1.0, 2.0, 0.0),
            height: 3.5,
            rotation: 90.0,
            raw: "PART 1\\U+0041".into(),
            plain: "PART 1A".into(),
            non_z_extrusion: false,
        }
    );
    assert_eq!(
        drawing.entities[1],
        Entity::MText {
            layer: "NOTE".into(),
            at: p(0.0, 0.0, 0.0),
            height: 2.0,
            raw: "{\\H2.5;\\fArial|b0;SS400\\PNOTE\\~A\\\\".into(),
            plain: "SS400\nNOTE A\\".into(),
            non_z_extrusion: false,
        }
    );
    match &drawing.entities[2] {
        Entity::Solid { layer, corners, .. } => {
            assert_eq!(layer, "CUT");
            assert_eq!(corners[0], p(0.0, 0.0, 0.0));
            assert_eq!(corners[1], p(10.0, 0.0, 0.0));
            assert_eq!(corners[2], p(0.0, 10.0, 0.0));
            assert_eq!(corners[3], p(10.0, 10.0, 0.0));
        }
        other => panic!("expected SOLID, got {other:?}"),
    }
}

#[test]
fn mtext_parts_before_group_one_are_joined() {
    let drawing =
        entities_of("0\nMTEXT\n10\n0\n20\n0\n40\n1\n3\nAB\n3\nCD\n1\nEF\n").expect("valid mtext");
    match &drawing.entities[0] {
        Entity::MText { raw, plain, .. } => {
            assert_eq!(raw, "ABCDEF");
            assert_eq!(plain, "ABCDEF");
        }
        other => panic!("expected MTEXT, got {other:?}"),
    }
}

#[test]
fn other_entity_kinds_are_counted_not_modelled() {
    let drawing = entities_of("0\nHATCH\n8\nCUT\n0\nHATCH\n8\nCUT\n0\nELLIPSE\n8\nCUT\n").unwrap();
    assert!(drawing.entities.is_empty());
    assert_eq!(drawing.skipped.get("HATCH"), Some(&2));
    assert_eq!(drawing.skipped.get("ELLIPSE"), Some(&1));
}

#[test]
fn missing_required_groups_are_refused_with_the_entity_and_group() {
    let cases: &[(&str, &str, u16)] = &[
        ("0\nLINE\n10\n0\n20\n0\n", "LINE", 11),
        ("0\nLINE\n10\n0\n11\n1\n21\n1\n", "LINE", 20),
        ("0\nCIRCLE\n10\n0\n20\n0\n", "CIRCLE", 40),
        ("0\nARC\n10\n0\n20\n0\n40\n1\n50\n0\n", "ARC", 51),
        ("0\nTEXT\n10\n0\n20\n0\n1\nX\n", "TEXT", 40),
    ];
    for (body, entity, code) in cases {
        let err = entities_of(body).expect_err(body);
        match err.reason {
            Reason::MissingField {
                entity: got,
                code: c,
            } => {
                assert_eq!(&got, entity, "{body}");
                assert_eq!(c, *code, "{body}");
            }
            other => panic!("{body}: expected MissingField, got {other:?}"),
        }
    }
}

#[test]
fn negative_radius_and_bad_numbers_are_refused() {
    let err = entities_of("0\nCIRCLE\n10\n0\n20\n0\n40\n-1\n").expect_err("negative radius");
    assert!(matches!(err.reason, Reason::InvalidValue { code: 40, .. }));
    let err = entities_of("0\nPOINT\n10\nabc\n20\n0\n").expect_err("not a number");
    assert!(matches!(err.reason, Reason::InvalidValue { code: 10, .. }));
}

#[test]
fn vertex_without_polyline_and_unterminated_polyline_are_refused() {
    let err = entities_of("0\nVERTEX\n10\n0\n20\n0\n").expect_err("orphan vertex");
    assert_eq!(err.reason, Reason::OrphanVertex);
    let err = entities_of("0\nPOLYLINE\n70\n0\n0\nVERTEX\n10\n0\n20\n0\n0\nLINE\n")
        .expect_err("no seqend");
    assert_eq!(err.reason, Reason::UnterminatedPolyline);
}

#[test]
fn lwpolyline_y_without_x_is_refused() {
    let err = entities_of("0\nLWPOLYLINE\n20\n1\n").expect_err("y before x");
    assert!(matches!(err.reason, Reason::MissingField { code: 10, .. }));
}

#[test]
fn truncated_drawings_never_panic() {
    let full = common::dxf(
        &common::header("AC1009", Some(4)),
        &format!(
            "{}{}",
            common::section(
                "TABLES",
                "0\nTABLE\n2\nLAYER\n70\n1\n0\nLAYER\n2\nCUT\n62\n1\n0\nENDTAB\n",
            ),
            common::section(
                "ENTITIES",
                "0\nLWPOLYLINE\n10\n0\n20\n0\n42\n1\n10\n1\n20\n1\n0\nPOLYLINE\n0\nVERTEX\n10\n2\n20\n2\n0\nSEQEND\n0\nTEXT\n10\n0\n20\n0\n40\n1\n1\n\\U+0041\n",
            )
        ),
    );
    let bytes = full.as_bytes();
    for cut in 0..bytes.len() {
        if let Ok(doc) = Document::parse(&bytes[..cut]) {
            let _ = doc.drawing();
        }
    }
    assert!(entities_of("0\nARC\n").is_err());
}

#[test]
fn extrusion_not_plus_z_sets_the_flag_and_warns() {
    // First LINE: extrusion (0, 0, -1), so the flag is set and the coordinates are kept as written.
    // Second LINE: no extrusion group. Third LINE: extrusion (0, 0, 1). Neither sets the flag.
    let drawing = entities_of(
        "0\nLINE\n8\nCUT\n10\n1\n20\n2\n11\n3\n21\n4\n230\n-1\n\
         0\nLINE\n8\nCUT\n10\n1\n20\n2\n11\n3\n21\n4\n\
         0\nLINE\n8\nCUT\n10\n1\n20\n2\n11\n3\n21\n4\n210\n0\n220\n0\n230\n1\n",
    )
    .expect("extrusion is read, not refused");
    let flags: Vec<bool> = drawing
        .entities
        .iter()
        .map(Entity::non_z_extrusion)
        .collect();
    assert_eq!(flags, [true, false, false]);
    assert_eq!(
        drawing.entities[0],
        Entity::Line {
            layer: "CUT".into(),
            start: p(1.0, 2.0, 0.0),
            end: p(3.0, 4.0, 0.0),
            non_z_extrusion: true,
        }
    );
    assert_eq!(drawing.warnings.len(), 1, "{:?}", drawing.warnings);
    assert!(
        drawing.warnings[0].contains("extrusion"),
        "{:?}",
        drawing.warnings
    );
}
