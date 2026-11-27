//! LAYER, LTYPE, STYLE and BLOCK_RECORD tables, and the units rule, from hand-written fixtures.

mod common;

use nf_cadio::{Document, Reason, Units};

fn tables_section() -> String {
    common::section(
        "TABLES",
        "0\nTABLE\n2\nLAYER\n70\n3\n\
         0\nLAYER\n2\nCUT\n70\n0\n62\n1\n6\nCONTINUOUS\n\
         0\nLAYER\n2\nNOTE\n70\n1\n62\n-3\n6\nDASHED\n\
         0\nENDTAB\n\
         0\nTABLE\n2\nLTYPE\n70\n2\n\
         0\nLTYPE\n2\nDASHED\n3\nDash - - -\n73\n2\n40\n0.75\n49\n0.5\n49\n-0.25\n\
         0\nENDTAB\n\
         0\nTABLE\n2\nSTYLE\n70\n1\n\
         0\nSTYLE\n2\nSTANDARD\n3\ntxt\n40\n2.5\n41\n0.8\n70\n0\n\
         0\nENDTAB\n\
         0\nTABLE\n2\nBLOCK_RECORD\n70\n2\n\
         0\nBLOCK_RECORD\n2\n*Model_Space\n\
         0\nBLOCK_RECORD\n2\n*Paper_Space\n\
         0\nENDTAB\n",
    )
}

fn drawing_of(text: &str) -> Result<nf_cadio::Drawing, nf_cadio::CadError> {
    Document::parse(text.as_bytes())?.drawing()
}

#[test]
fn layers_linetypes_styles_and_blocks_are_read() {
    let text = common::dxf(&common::header("AC1009", Some(4)), &tables_section());
    let drawing = drawing_of(&text).expect("valid tables");
    assert_eq!(drawing.version.as_deref(), Some("AC1009"));
    assert_eq!(drawing.units, Units::Millimeter);

    assert_eq!(drawing.layers.len(), 2);
    let cut = &drawing.layers[0];
    assert_eq!(cut.name, "CUT");
    assert_eq!(cut.color, 1);
    assert_eq!(cut.line_type.as_deref(), Some("CONTINUOUS"));
    assert!(cut.on && !cut.frozen);
    let note = &drawing.layers[1];
    assert_eq!(note.name, "NOTE");
    assert_eq!(
        note.color, 3,
        "the colour is the absolute value of group 62"
    );
    assert!(!note.on, "a negative group 62 means the layer is off");
    assert!(note.frozen, "bit 1 of group 70 means frozen");

    assert_eq!(drawing.line_types.len(), 1);
    assert_eq!(drawing.line_types[0].name, "DASHED");
    assert_eq!(drawing.line_types[0].description, "Dash - - -");
    assert_eq!(drawing.line_types[0].elements, vec![0.5, -0.25]);

    assert_eq!(drawing.text_styles.len(), 1);
    let style = &drawing.text_styles[0];
    assert_eq!(style.name, "STANDARD");
    assert_eq!(style.font, "txt");
    assert_eq!(style.height, 2.5);
    assert_eq!(style.width_factor, 0.8);

    assert_eq!(drawing.blocks, ["*Model_Space", "*Paper_Space"]);
    assert!(drawing.entities.is_empty());
}

#[test]
fn linetype_element_count_must_match_group_73() {
    let tables = common::section(
        "TABLES",
        "0\nTABLE\n2\nLTYPE\n70\n1\n\
         0\nLTYPE\n2\nDASHED\n73\n3\n49\n0.5\n49\n-0.25\n\
         0\nENDTAB\n",
    );
    let err = drawing_of(&common::dxf(&common::header("AC1009", Some(4)), &tables))
        .expect_err("three declared, two given");
    assert!(matches!(err.reason, Reason::InvalidValue { code: 73, .. }));
}

#[test]
fn missing_or_zero_insunits_is_unknown_and_never_defaulted() {
    for insunits in [None, Some(0), Some(999)] {
        let text = common::dxf(&common::header("AC1009", insunits), &tables_section());
        let drawing = drawing_of(&text).expect("valid file");
        assert_eq!(
            drawing.units,
            Units::Unknown,
            "$INSUNITS {insunits:?} must give unknown units"
        );
        assert_eq!(drawing.units.as_str(), "unknown");
    }
}

#[test]
fn insunits_four_is_millimetres() {
    let text = common::dxf(&common::header("AC1009", Some(4)), &tables_section());
    assert_eq!(drawing_of(&text).unwrap().units, Units::Millimeter);
}

#[test]
fn a_layer_without_a_name_is_refused() {
    let tables = common::section(
        "TABLES",
        "0\nTABLE\n2\nLAYER\n70\n1\n0\nLAYER\n62\n7\n0\nENDTAB\n",
    );
    let err =
        drawing_of(&common::dxf(&common::header("AC1009", Some(4)), &tables)).expect_err("no name");
    assert!(matches!(err.reason, Reason::MissingField { code: 2, .. }));
}
