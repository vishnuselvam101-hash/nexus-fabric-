//! BH-004 and BH-005: every DXF states its units; a header-with-branch part states its joint exactly as the job
//! declares it ("set-on" or "not declared"), in the 999 comment block and as TEXT annotations. The independent reader
//! reads the statement back, and a file whose statement is missing, altered or contradictory fails the quality gate.

use nf_core::job::{Request, run};
use nf_dxf::quality::quality_gate_stated;
use nf_dxf::{
    Entity, JOINT_NOT_DECLARED, JOINT_SET_ON, JointStatement, KEY_BRANCH_SURFACE,
    KEY_HEADER_SURFACE, KEY_JOINT, KEY_UNITS, MARK_LAYER,
};
use serde_json::{Value, json};

/// The request, and the joint statement the job declares for each of its header-with-branch parts (None: no joint).
fn cases() -> Vec<(&'static str, Value, Option<JointStatement>)> {
    let hwb = |surfaces: (&str, &str), joint: Option<&str>, extra_branch_dev: f64| {
        let mut v = json!({"family": "header_with_branch", "header_radius": 161.95, "header_dev_radius": 156.795,
            "header_length": 1200.0, "branch_station": 600.0, "branch_radius": 77.095,
            "branch_dev_radius": extra_branch_dev, "beta_deg": 90.0, "delta": 0.0, "s_end": 400.0,
            "header_surface_reference": surfaces.0, "branch_surface_reference": surfaces.1});
        if let Some(j) = joint {
            v["joint"] = json!(j);
        }
        v
    };
    vec![
        (
            "cylinder shell",
            json!({"family": "cylinder_shell", "r_dev": 100.0, "length": 250.0}),
            None,
        ),
        (
            "cone frustum",
            json!({"family": "cone_frustum", "r1": 200.0, "r2": 100.0, "h": 300.0}),
            None,
        ),
        (
            "eccentric reducer",
            json!({"family": "eccentric_reducer", "r1": 200.0, "r2": 100.0, "h": 300.0, "e": 100.0}),
            None,
        ),
        (
            "branch stub",
            json!({"family": "branch_stub", "header_radius": 150.0, "branch_radius": 100.0, "branch_dev_radius": 100.0,
                   "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0, "surface_reference": "outside"}),
            None,
        ),
        (
            "header set-on, the set-on pairing",
            hwb(("outside", "inside"), Some("set_on"), 80.6),
            Some(JointStatement::new(true, "outside", "inside")),
        ),
        (
            "header joint not declared, the same surfaces",
            hwb(("outside", "inside"), None, 80.6),
            Some(JointStatement::new(false, "outside", "inside")),
        ),
        (
            "header joint not declared, unknown surfaces",
            hwb(("unknown", "unknown"), None, 80.6),
            Some(JointStatement::new(false, "unknown", "unknown")),
        ),
    ]
}

/// The 999 keys of the units and the joint. The plate-convention keys (task 1.3) are checked by
/// tests/plate_conventions.rs, so they are left out here.
fn joint_and_units_keys(text: &str) -> Vec<(String, String)> {
    nf_dxf::statement_keys(text)
        .into_iter()
        .filter(|(k, _)| !nf_dxf::CONVENTION_KEYS.contains(&k.as_str()))
        .collect()
}

fn record<'a>(out: &'a nf_core::job::JobOutput, kind: &str, part: &str) -> Option<&'a Value> {
    out.records
        .iter()
        .find(|r| r.kind == kind && (part.is_empty() || r.payload["part"] == part))
        .map(|r| &r.payload)
}

fn texts_on_mark_layer(text: &str) -> Vec<String> {
    nf_dxf::read(text)
        .expect("the file reads")
        .0
        .into_iter()
        .filter_map(|e| match e {
            Entity::Text { layer, text, .. } if layer == MARK_LAYER => Some(text),
            _ => None,
        })
        .collect()
}

/// The cut geometry of a DXF: the ENTITIES section up to the first annotation.
fn cut_geometry(text: &str) -> &str {
    const START: &str = "  2\nENTITIES\n";
    let start = text.find(START).expect("ENTITIES section") + START.len();
    let end = match text[start..].find("  0\nTEXT\n") {
        Some(i) => start + i,
        None => text.rfind("  0\nENDSEC\n").expect("end of ENTITIES"),
    };
    &text[start..end]
}

#[test]
fn every_family_and_part_states_its_units_and_its_joint_exactly_as_declared() {
    for (name, body, joint) in cases() {
        let req = Request::parse_strict(&body).unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = run(&req, "statement-test");
        assert!(out.refusal.is_none(), "{name}: refused: {:?}", out.outcome);
        assert!(!out.dxf.is_empty(), "{name}: no DXF was offered");
        let interface = record(&out, "interface", "");
        match &joint {
            Some(js) => assert_eq!(
                interface.and_then(|p| p["declared"].as_str()),
                Some(js.joint),
                "{name}: the job's own interface record states the declared joint"
            ),
            None => assert!(
                interface.is_none(),
                "{name}: an interface record for a part without a joint"
            ),
        }
        for (part, text) in &out.dxf {
            // the independent reader reads back the geometry the job holds, and the statement (each check passed)
            let rr = record(&out, "reimport_report", part).expect("reimport report");
            assert_eq!(rr["pass"], true, "{name} {part}: re-import");
            assert_eq!(
                rr["joint_statement_matches"], true,
                "{name} {part}: joint statement"
            );
            assert_eq!(
                rr["entities_written"], rr["entities_read"],
                "{name} {part}: entity count"
            );
            assert_eq!(
                record(&out, "dxf_quality_report", part).expect("quality report")["pass"],
                true,
                "{name} {part}: gate"
            );
            // the units: the key line and the human line are both there, and nothing else is claimed
            assert!(
                nf_dxf::units_problems(text).is_empty(),
                "{name} {part}: {:?}",
                nf_dxf::units_problems(text)
            );
            assert!(
                text.contains("999\nNF-UNITS: mm\n"),
                "{name} {part}: the units key line"
            );
            assert!(
                !text.contains("$INSUNITS"),
                "{name} {part}: R12 has no $INSUNITS"
            );
            match &joint {
                Some(js) => {
                    assert_eq!(
                        joint_and_units_keys(text),
                        vec![
                            (KEY_UNITS.to_string(), "mm".to_string()),
                            (KEY_JOINT.to_string(), js.joint.to_string()),
                            (KEY_HEADER_SURFACE.to_string(), js.header_surface.clone()),
                            (KEY_BRANCH_SURFACE.to_string(), js.branch_surface.clone()),
                        ],
                        "{name} {part}: the 999 statement"
                    );
                    let on_layer = texts_on_mark_layer(text);
                    for want in js.texts() {
                        assert_eq!(
                            on_layer.iter().filter(|t| **t == want).count(),
                            1,
                            "{name} {part}: \"{want}\""
                        );
                    }
                    assert!(matches!(js.joint, JOINT_SET_ON | JOINT_NOT_DECLARED));
                }
                None => {
                    assert_eq!(
                        joint_and_units_keys(text),
                        vec![(KEY_UNITS.to_string(), "mm".to_string())],
                        "{name} {part}: a part without a joint states no joint"
                    );
                    assert!(
                        texts_on_mark_layer(text)
                            .iter()
                            .all(|t| !t.starts_with("JOINT:") && !t.starts_with("HEADER SURFACE:")),
                        "{name} {part}: no joint text"
                    );
                }
            }
        }
    }
}

#[test]
fn the_same_geometry_with_a_declared_and_an_undeclared_joint_differs_only_in_the_statement() {
    let run_case = |i: usize| {
        let (_, body, _) = &cases()[i];
        run(
            &Request::parse_strict(body).expect("request"),
            "statement-test",
        )
    };
    let set_on = run_case(4);
    let undeclared = run_case(5);
    let a = &set_on.dxf[0].1;
    let b = &undeclared.dxf[0].1;
    assert_ne!(a, b, "the statements differ");
    assert_eq!(
        cut_geometry(a),
        cut_geometry(b),
        "the cut geometry is the same bytes for both joint statements"
    );
    assert!(a.contains("NF-JOINT: set-on\n") && b.contains("NF-JOINT: not declared\n"));
    assert!(
        !b.contains("set-on") && !a.contains("not declared"),
        "neither file names the other joint"
    );
}

#[test]
fn an_altered_statement_fails_the_gate_and_the_reader() {
    let (_, body, _) = &cases()[4];
    let out = run(
        &Request::parse_strict(body).expect("request"),
        "statement-test",
    );
    let (_, text) = &out.dxf[0];
    let js = JointStatement::new(true, "outside", "inside");
    assert!(
        quality_gate_stated(text, Some(&js)).pass,
        "the file as written passes"
    );
    let geometry = nf_dxf::read(text).expect("reads").0;
    let wrong_joint = JointStatement::new(false, "outside", "inside").texts();
    let altered: Vec<(&str, String, &str)> = vec![
        (
            "joint key altered",
            text.replace("NF-JOINT: set-on", "NF-JOINT: not declared"),
            "joint_statement",
        ),
        (
            "joint key missing",
            text.replace("999\nNF-JOINT: set-on\n", ""),
            "joint_statement",
        ),
        (
            "joint key repeated",
            text.replace(
                "999\nNF-JOINT: set-on\n",
                "999\nNF-JOINT: set-on\n999\nNF-JOINT: set-on\n",
            ),
            "joint_statement",
        ),
        (
            "branch surface altered",
            text.replace("NF-BRANCH-SURFACE: inside", "NF-BRANCH-SURFACE: outside"),
            "joint_statement",
        ),
        (
            "header surface missing",
            text.replace("999\nNF-HEADER-SURFACE: outside\n", ""),
            "joint_statement",
        ),
        (
            "joint text contradicts the job",
            text.replace(&js.texts()[0], &wrong_joint[0]),
            "joint_statement",
        ),
        (
            "joint text missing",
            text.replace(&js.texts()[0], ""),
            "joint_statement",
        ),
        (
            "surface text repeated",
            text.replace(
                &js.texts()[1],
                &format!(
                    "{0}\n  0\nTEXT\n  8\n{MARK_LAYER}\n 10\n0\n 20\n0\n 40\n1\n  1\n{0}",
                    js.texts()[1]
                ),
            ),
            "joint_statement",
        ),
        (
            "units key altered",
            text.replace("NF-UNITS: mm", "NF-UNITS: in"),
            "units_declared",
        ),
        (
            "unknown application key",
            text.replace(
                "999\nNF-UNITS: mm\n",
                "999\nNF-UNITS: mm\n999\nNF-THICKNESS: 12\n",
            ),
            "readable",
        ),
    ];
    for (what, bad, rule) in altered {
        assert_ne!(bad, *text, "{what}: the alteration was applied");
        let q = quality_gate_stated(&bad, Some(&js));
        assert!(!q.pass, "{what}: the gate passed the file");
        assert!(
            q.failed_rules().contains(&rule),
            "{what}: failed {:?}, expected {rule}",
            q.failed_rules()
        );
        let rr = nf_dxf::reimport_check_stated(&geometry, &bad, 1e-8, &[], Some(&js));
        assert!(!rr.pass, "{what}: the re-import passed the file");
    }
    // a part of a family without joints must not state one
    let (_, cyl_body, _) = &cases()[0];
    let cyl = run(
        &Request::parse_strict(cyl_body).expect("request"),
        "statement-test",
    );
    let cut = &cyl.dxf[0].1;
    let with_joint = cut.replace(
        "999\nNF-UNITS: mm\n",
        &format!("999\nNF-UNITS: mm\n999\nNF-JOINT: {JOINT_SET_ON}\n"),
    );
    let q = quality_gate_stated(&with_joint, None);
    assert!(
        !q.pass && q.failed_rules().contains(&"joint_statement"),
        "a cylinder that states a joint"
    );
    assert!(
        quality_gate_stated(cut, None).pass,
        "the cylinder as written passes"
    );
}
