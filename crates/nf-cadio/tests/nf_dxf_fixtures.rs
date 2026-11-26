//! R12 files written by nf-dxf (test-only dependency) must read back through the general reader.

use nf_cadio::{Document, HeaderValue, Units};
use nf_dxf::Entity;

fn families() -> Vec<(&'static str, Vec<Entity>)> {
    vec![
        (
            "line",
            vec![Entity::Line {
                layer: "CUT".into(),
                a: (0.0, 0.0),
                b: (120.5, 40.25),
            }],
        ),
        (
            "arc",
            vec![Entity::Arc {
                layer: "CUT".into(),
                c: (10.0, 10.0),
                r: 5.0,
                start_deg: 0.0,
                end_deg: 270.0,
            }],
        ),
        (
            "polyline_open",
            vec![Entity::Polyline {
                layer: "BEND".into(),
                pts: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
                closed: false,
            }],
        ),
        (
            "polyline_closed",
            vec![Entity::Polyline {
                layer: "CUT".into(),
                pts: vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
                closed: true,
            }],
        ),
        (
            "text",
            vec![Entity::Text {
                layer: "NOTE".into(),
                at: (1.0, 2.0),
                height: 3.5,
                text: "PART 1".into(),
            }],
        ),
        (
            "mixed",
            vec![
                Entity::Line {
                    layer: "CUT".into(),
                    a: (0.0, 0.0),
                    b: (1.0, 1.0),
                },
                Entity::Arc {
                    layer: "CUT".into(),
                    c: (0.0, 0.0),
                    r: 2.0,
                    start_deg: 90.0,
                    end_deg: 180.0,
                },
            ],
        ),
    ]
}

fn entity_types(doc: &Document) -> Vec<String> {
    let entities = doc.section("ENTITIES").expect("ENTITIES section");
    entities
        .pairs
        .iter()
        .filter(|p| p.code == 0)
        .map(|p| p.value.clone())
        .collect()
}

#[test]
fn every_family_written_by_nf_dxf_reads_back() {
    for (family, entities) in families() {
        for text in [
            nf_dxf::write(&entities),
            nf_dxf::write_annotated(
                &entities,
                &["NOTE ONE".to_string(), "fine print".to_string()],
            ),
        ] {
            let doc = Document::parse(text.as_bytes()).unwrap_or_else(|e| panic!("{family}: {e}"));
            assert_eq!(
                doc.variable("$ACADVER"),
                Some(&HeaderValue::Text("AC1009".to_string())),
                "{family}"
            );
            let types = entity_types(&doc);
            let expected = match family {
                "line" => "LINE",
                "arc" => "ARC",
                "polyline_open" | "polyline_closed" => "LWPOLYLINE",
                "text" => "TEXT",
                _ => "LINE",
            };
            assert!(
                types.iter().any(|t| t == expected) || types.iter().any(|t| t == "POLYLINE"),
                "{family}: entity types {types:?}"
            );
            assert!(doc.sections.iter().any(|s| s.name == "TABLES"), "{family}");
            assert!(doc.sections.iter().any(|s| s.name == "HEADER"), "{family}");
        }
    }
}

#[test]
fn nf_dxf_files_report_units_unknown_because_they_declare_none() {
    // nf-dxf states its units as a 999 comment and writes no $INSUNITS. The reader must not guess them.
    let text = nf_dxf::write(&families()[0].1);
    let doc = Document::parse(text.as_bytes()).unwrap();
    assert!(doc.variable("$INSUNITS").is_none());
    assert_eq!(doc.units(), Units::Unknown);
    assert!(
        doc.comments.iter().any(|c| c.ends_with(": mm")),
        "{:?}",
        doc.comments
    );
}

#[test]
fn nf_dxf_header_extents_are_typed_points() {
    let text = nf_dxf::write(&families()[0].1);
    let doc = Document::parse(text.as_bytes()).unwrap();
    match doc.variable("$EXTMIN") {
        Some(HeaderValue::Point(p)) => assert_eq!(p.len(), 2),
        other => panic!("$EXTMIN is {other:?}"),
    }
}

#[test]
fn nf_dxf_file_truncated_anywhere_before_eof_is_refused() {
    let text = nf_dxf::write(&families()[5].1);
    let eof_at = text.rfind("  0\n").and_then(|_| text.rfind("EOF")).unwrap();
    for cut in (0..eof_at).step_by(7) {
        assert!(
            Document::parse(&text.as_bytes()[..cut]).is_err(),
            "prefix {cut} accepted"
        );
    }
}
