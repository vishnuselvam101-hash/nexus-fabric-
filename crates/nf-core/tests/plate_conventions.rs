//! Work plan task 1.3: every convention the plate of a part relies on is stated in the DXF file and in the job result,
//! read back by the quality gate, and cut geometry is unchanged except for the RB-008 closed polyline.
//!
//! Covered: a statement test per family and part (exact key set, values, annotation lines and the gate); tampered
//! files (altered, removed, duplicated, extra or contradicting statements) fail the gate; the RB-008 closed outer
//! profile has the same vertices as the curves it replaces; a cone (arcs) is byte-identical to engine 0.4.4.

use nf_core::job::{JobOutput, Request, run, to_entities, to_entities_per_curve};
use nf_dxf::quality::quality_gate_full;
use nf_dxf::{
    CONVENTION_KEYS, Conventions, Entity, JointStatement, MARK_LAYER, MAX_COMMENT_CHARS,
    statement_keys,
};
use serde_json::{Value, json};
use uem::verify::required_convention_keys;

fn hwb(surfaces: (&str, &str), joint: Option<&str>) -> Value {
    let mut v = json!({"family": "header_with_branch", "header_radius": 161.95, "header_dev_radius": 156.795,
        "header_length": 1200.0, "branch_station": 600.0, "branch_radius": 77.095,
        "branch_dev_radius": 80.6, "beta_deg": 90.0, "delta": 0.0, "s_end": 400.0,
        "header_surface_reference": surfaces.0, "branch_surface_reference": surfaces.1});
    if let Some(j) = joint {
        v["joint"] = json!(j);
    }
    v
}

/// One request per family, and the header-with-branch cases with an offset (delta and lean) so that the offset
/// statements are exercised too. (name, request, the joint the job declares for the header and branch parts)
fn cases() -> Vec<(&'static str, Value, Option<JointStatement>)> {
    let mut offset = hwb(("outside", "inside"), Some("set_on"));
    offset["delta"] = json!(12.5);
    offset["beta_deg"] = json!(70.0);
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
            "header with branch, set-on, no offset",
            hwb(("outside", "inside"), Some("set_on")),
            Some(JointStatement::new(true, "outside", "inside")),
        ),
        (
            "header with branch, set-on, offset and lean",
            offset,
            Some(JointStatement::new(true, "outside", "inside")),
        ),
        (
            "header with branch, joint not declared",
            hwb(("outside", "inside"), None),
            Some(JointStatement::new(false, "outside", "inside")),
        ),
    ]
}

/// The part's family as the engine names it (the header plate is `header_shell_with_opening`).
fn family_of(part: &str, out: &JobOutput) -> String {
    let rec = out
        .records
        .iter()
        .find(|r| r.kind == "pattern" && r.payload["part"] == part)
        .expect("pattern record");
    rec.payload["pattern"]["family"]
        .as_str()
        .expect("family")
        .to_string()
}

/// The conventions the job's DXF record states for a part: (key, value) in order.
fn stated(out: &JobOutput, part: &str) -> Conventions {
    let rec = out
        .records
        .iter()
        .find(|r| r.kind == "dxf_output" && r.payload["part"] == part)
        .expect("dxf_output record");
    let facts = rec.payload["statement"]["plate_conventions"]
        .as_array()
        .expect("plate_conventions")
        .iter()
        .map(|pair| {
            (
                pair[0].as_str().expect("key").to_string(),
                pair[1].as_str().expect("value").to_string(),
            )
        })
        .collect();
    Conventions { facts }
}

fn value<'a>(c: &'a Conventions, key: &str) -> Option<&'a str> {
    c.facts
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn run_case(body: &Value) -> JobOutput {
    let req = Request::parse_strict(body).expect("request");
    run(&req, "plate-conventions-test")
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

#[test]
fn every_family_and_part_states_exactly_the_conventions_its_family_requires() {
    for (name, body, joint) in cases() {
        let out = run_case(&body);
        assert!(out.refusal.is_none(), "{name}: refused: {:?}", out.outcome);
        assert!(!out.dxf.is_empty(), "{name}: no DXF was offered");
        for (part, text) in &out.dxf {
            let family = family_of(part, &out);
            let conv = stated(&out, part);
            // the statement is exactly the family's key set, each key once, in the canonical order
            let want_keys = required_convention_keys(&family);
            let got_keys: Vec<&str> = conv.facts.iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(
                {
                    let mut g = got_keys.clone();
                    g.sort();
                    g
                },
                {
                    let mut w = want_keys.clone();
                    w.sort();
                    w
                },
                "{name} {part}: the conventions of a {family} part"
            );
            let positions: Vec<usize> = got_keys
                .iter()
                .map(|k| {
                    CONVENTION_KEYS
                        .iter()
                        .position(|c| c == k)
                        .expect("known key")
                })
                .collect();
            assert!(
                positions.windows(2).all(|w| w[0] < w[1]),
                "{name} {part}: canonical order"
            );
            // the 999 block states the same pairs, in the same order, after the units and the joint
            let in_999: Vec<(String, String)> = statement_keys(text)
                .into_iter()
                .filter(|(k, _)| CONVENTION_KEYS.contains(&k.as_str()))
                .collect();
            assert_eq!(in_999, conv.facts, "{name} {part}: the 999 block");
            // the viewing side is the declared marking side, outside, for every part
            assert_eq!(
                value(&conv, "NF-VIEW-SIDE"),
                Some("outside"),
                "{name} {part}"
            );
            // every value fits one DXF comment line and states something
            for (k, v) in &conv.facts {
                assert!(
                    !v.is_empty() && v.chars().count() <= MAX_COMMENT_CHARS,
                    "{name} {part} {k}: {v}"
                );
                assert!(
                    !v.contains("NaN") && !v.contains("inf"),
                    "{name} {part} {k}: {v}"
                );
            }
            // the annotation layer carries each statement line, once
            let on_layer = texts_on_mark_layer(text);
            for t in conv.texts() {
                assert!(
                    on_layer.contains(&t),
                    "{name} {part}: annotation \"{t}\" is missing"
                );
            }
            // the gate reads the statement back and passes the file as written
            // the header and the branch of a header with branch both state the joint; no other part does
            let q = quality_gate_full(text, joint.as_ref(), Some(&conv));
            assert!(q.pass, "{name} {part}: {:?}", q.findings);
            assert!(
                record_pass(&out, "dxf_quality_report", part),
                "{name} {part}: the job's own gate"
            );
        }
    }
}

fn record_pass(out: &JobOutput, kind: &str, part: &str) -> bool {
    out.records
        .iter()
        .find(|r| r.kind == kind && r.payload["part"] == part)
        .map(|r| r.payload["pass"] == true)
        .unwrap_or(false)
}

#[test]
fn the_statements_say_what_each_family_means_by_its_offset_rolling_axis_seams_datum_origin_and_kerf()
 {
    let by_name = |name: &str| -> Vec<(String, Conventions, String)> {
        let (_, body, _) = cases()
            .into_iter()
            .find(|(n, _, _)| *n == name)
            .expect("case");
        let out = run_case(&body);
        out.dxf
            .iter()
            .map(|(part, text)| (part.clone(), stated(&out, part), text.clone()))
            .collect()
    };
    // cylinder shell: no offset; rolling about Y; seams at X = 0 and X = w; ends at Y = 0 and Y = L
    let shell = by_name("cylinder shell");
    let c = &shell[0].1;
    assert_eq!(value(c, "NF-OFFSET-DELTA"), None, "a shell has no offset");
    assert!(
        value(c, "NF-ROLLING-AXIS")
            .unwrap()
            .starts_with("parallel to Y")
    );
    assert!(value(c, "NF-SEAM-EDGES").unwrap().contains("X = 0"));
    assert!(value(c, "NF-END-EDGES").unwrap().contains("Y = 0"));
    assert!(
        value(c, "NF-KERF")
            .unwrap()
            .contains("kerf is NOT compensated")
    );
    // cone frustum: no offset, apex origin, rolling about the cone axis
    let cone = by_name("cone frustum");
    assert_eq!(value(&cone[0].1, "NF-OFFSET-DELTA"), None);
    assert!(value(&cone[0].1, "NF-ORIGIN").unwrap().contains("apex"));
    // eccentric reducer: the offset is e, positive toward the seam (the shortest generator)
    let ecc = by_name("eccentric reducer");
    assert!(
        value(&ecc[0].1, "NF-OFFSET-SIGN")
            .unwrap()
            .starts_with("POSITIVE = THE SMALL END IS OFFSET TOWARD THE SEAM")
    );
    assert_eq!(value(&ecc[0].1, "NF-BRANCH-ANGLE"), None);
    // branch stub: delta sign and lean, the datum phi on the positive-delta side
    let br = by_name("branch stub");
    assert!(
        value(&br[0].1, "NF-OFFSET-SIGN")
            .unwrap()
            .starts_with("POSITIVE = THE BRANCH AXIS IS DISPLACED TO THE LEFT")
    );
    assert!(
        value(&br[0].1, "NF-BRANCH-ANGLE")
            .unwrap()
            .starts_with("beta = 60 deg")
    );
    assert!(
        value(&br[0].1, "NF-ANGLE-DATUM")
            .unwrap()
            .contains("positive-delta side")
    );
    // header with branch: the header and the branch both state the delta and the lean that were requested
    let hb = by_name("header with branch, set-on, offset and lean");
    for (part, conv, _) in &hb {
        assert!(
            value(conv, "NF-OFFSET-DELTA")
                .unwrap()
                .starts_with("delta = 12.5 mm"),
            "{part}"
        );
        assert!(
            value(conv, "NF-BRANCH-ANGLE")
                .unwrap()
                .starts_with("beta = 70 deg"),
            "{part}"
        );
    }
    let header = hb
        .iter()
        .find(|(p, _, _)| p == "header")
        .expect("header part");
    assert!(
        value(&header.1, "NF-ROLLING-AXIS")
            .unwrap()
            .starts_with("parallel to X")
    );
    assert!(
        value(&header.1, "NF-PLATE-AXES")
            .unwrap()
            .starts_with("X = along the header axis")
    );
}

#[test]
fn tampered_statements_fail_the_gate_and_the_file_is_not_offered() {
    // one file of each family that has an offset and one that has none
    for (name, body) in [
        ("cylinder shell", cases()[0].1.clone()),
        ("branch stub", cases()[3].1.clone()),
        ("eccentric reducer", cases()[2].1.clone()),
    ] {
        let out = run_case(&body);
        let (part, text) = &out.dxf[0];
        let conv = stated(&out, part);
        assert!(
            quality_gate_full(text, None, Some(&conv)).pass,
            "{name}: as written passes"
        );
        let (k0, v0) = conv.facts[0].clone();
        let key_line = format!("999\n{k0}: {v0}\n");
        assert!(
            text.contains(&key_line),
            "{name}: the key line is in the file"
        );
        let origin_key = "NF-ORIGIN";
        let origin = conv
            .facts
            .iter()
            .find(|(k, _)| k == origin_key)
            .expect("origin")
            .1
            .clone();
        let origin_line = format!("999\nNF-ORIGIN: {origin}\n");
        let origin_text = conv
            .texts()
            .into_iter()
            .find(|t| t.starts_with("ORIGIN: "))
            .expect("origin text");
        let text_line = format!("  1\n{origin_text}\n");
        assert!(
            text.contains(&text_line),
            "{name}: the origin annotation is in the file"
        );
        let extra_key = format!(
            "999\n{}: {}\n",
            "NF-KERF", "theoretical line, kerf compensated"
        );
        let contradicting_text = "  0\nTEXT\n  8\nNF_NOT_RELEASED\n 10\n0\n 20\n0\n 40\n1\n  1\nKERF: CUT LINES INCLUDE THE KERF\n";
        // the entity section ends at the last ENDSEC of the file: the contradicting annotation goes before it
        let end = text.rfind("  0\nENDSEC\n").expect("end of entities");
        let with_contradiction = format!("{}{contradicting_text}{}", &text[..end], &text[end..]);
        let tampers: Vec<(&str, String)> = vec![
            (
                "key value altered",
                text.replacen(&key_line, &format!("999\n{k0}: altered\n"), 1),
            ),
            ("key removed", text.replacen(&key_line, "", 1)),
            (
                "key repeated",
                text.replacen(&key_line, &format!("{key_line}{key_line}"), 1),
            ),
            (
                "origin value altered",
                text.replacen(&origin_line, "999\nNF-ORIGIN: lower right corner\n", 1),
            ),
            (
                "origin annotation altered",
                text.replacen(&text_line, "  1\nORIGIN: UPPER RIGHT CORNER\n", 1),
            ),
            (
                "origin annotation removed",
                text.replacen(&text_line, "", 1),
            ),
            ("contradicting kerf statement added", with_contradiction),
            (
                "kerf key compensated, extra",
                text.replacen(
                    "999\nNF-UNITS: mm\n",
                    &format!("999\nNF-UNITS: mm\n{extra_key}"),
                    1,
                ),
            ),
            (
                "convention key repeated with another value",
                text.replacen(
                    "999\nNF-UNITS: mm\n",
                    "999\nNF-UNITS: mm\n999\nNF-ROLLING-AXIS: parallel to X\n",
                    1,
                ),
            ),
        ];
        for (what, bad) in tampers {
            assert_ne!(&bad, text, "{name} {what}: the alteration was applied");
            let q = quality_gate_full(&bad, None, Some(&conv));
            assert!(!q.pass, "{name} {what}: the gate passed the file");
            assert!(
                q.failed_rules().contains(&"convention_statement")
                    || q.failed_rules().contains(&"readable"),
                "{name} {what}: failed {:?}",
                q.failed_rules()
            );
        }
    }
}

#[test]
fn a_part_that_states_an_offset_it_does_not_have_fails_the_gate() {
    // the offset keys belong to a part with an offset only: a shell that states one is not the shell the job declares
    let out = run_case(&cases()[0].1);
    let (part, text) = &out.dxf[0];
    let conv = stated(&out, part);
    let with_offset = text.replacen(
        "999\nNF-UNITS: mm\n",
        "999\nNF-UNITS: mm\n999\nNF-OFFSET-DELTA: delta = 5 mm\n",
        1,
    );
    let q = quality_gate_full(&with_offset, None, Some(&conv));
    assert!(
        !q.pass && q.failed_rules().contains(&"convention_statement"),
        "{:?}",
        q.findings
    );
}

#[test]
fn rb008_the_outer_profile_is_one_closed_polyline_with_the_vertices_of_its_curves() {
    use uem::develop;
    let parts: Vec<(&str, uem::Pattern)> = vec![
        (
            "shell",
            develop::cylinder_shell(100.0, 250.0).expect("shell"),
        ),
        (
            "cone",
            develop::cone_frustum(200.0, 100.0, 300.0).expect("cone"),
        ),
        (
            "eccentric",
            develop::eccentric_reducer(200.0, 100.0, 300.0, 100.0).expect("eccentric"),
        ),
    ];
    for (name, p) in &parts {
        let new = to_entities(p);
        let old = to_entities_per_curve(p);
        let outer_new: Vec<&Entity> = new.iter().filter(|e| e.layer() == "CUT_OUTER").collect();
        let outer_old: Vec<&Entity> = old.iter().filter(|e| e.layer() == "CUT_OUTER").collect();
        let has_arc = p
            .outline
            .iter()
            .any(|c| matches!(c.geom, uem::Geom::Arc { .. }));
        if has_arc {
            // an arc needs a bulge encoding this build does not write: the profile stays one entity per curve, byte for byte
            assert_eq!(outer_new.len(), outer_old.len(), "{name}");
            assert_eq!(
                new, old,
                "{name}: an unchanged profile is the same entities as before"
            );
            assert!(
                nf_dxf::write_stated(&new, &[], &[], &[])
                    == nf_dxf::write_stated(&old, &[], &[], &[]),
                "{name}: an unchanged profile is written byte for byte as before"
            );
            continue;
        }
        // grouped: one closed polyline whose vertices are the vertices of the curves, in order, each once
        assert_eq!(outer_new.len(), 1, "{name}: one outer entity");
        let Entity::Polyline { pts, closed, .. } = outer_new[0] else {
            panic!("{name}: the outer profile is not a polyline");
        };
        assert!(*closed, "{name}: the polyline is flagged closed");
        let mut want: Vec<(f64, f64)> = Vec::new();
        for e in &outer_old {
            let seg: Vec<(f64, f64)> = match e {
                Entity::Line { a, b, .. } => vec![*a, *b],
                Entity::Polyline { pts, .. } => pts.clone(),
                Entity::Arc { .. } => panic!("{name}: arc in a grouped profile"),
                Entity::Text { .. } => unreachable!(),
            };
            let mut it = seg.into_iter();
            let first = it.next().expect("vertex");
            if want.last() != Some(&first) {
                want.push(first);
            }
            want.extend(it);
        }
        if want.first() == want.last() {
            want.pop();
        }
        assert_eq!(pts, &want, "{name}: the same vertices in the same order");
        // the cut length is the length of the curves it replaces
        let len_new = outer_new[0].length();
        let len_old: f64 = outer_old.iter().map(|e| e.length()).sum();
        assert!(
            (len_new - len_old).abs() <= 1e-9 * len_old.max(1.0),
            "{name}: {len_new} vs {len_old}"
        );
        // every other cut entity is unchanged
        let inner_new: Vec<&Entity> = new.iter().filter(|e| e.layer() != "CUT_OUTER").collect();
        let inner_old: Vec<&Entity> = old.iter().filter(|e| e.layer() != "CUT_OUTER").collect();
        assert_eq!(inner_new, inner_old, "{name}: the openings are unchanged");
    }
}

#[test]
fn rb008_written_file_reads_back_with_one_closed_outer_polyline_for_the_shell() {
    let out = run_case(&cases()[0].1);
    let (_, text) = &out.dxf[0];
    let (ents, unknown) = nf_dxf::read(text).expect("reads");
    assert!(unknown.is_empty());
    let outer: Vec<&Entity> = ents.iter().filter(|e| e.layer() == "CUT_OUTER").collect();
    assert_eq!(outer.len(), 1, "one entity on the outer layer");
    match outer[0] {
        Entity::Polyline { pts, closed, .. } => {
            assert!(closed);
            assert_eq!(
                pts,
                &vec![
                    (0.0, 0.0),
                    (628.3185307179587, 0.0),
                    (628.3185307179587, 250.0),
                    (0.0, 250.0)
                ]
            );
        }
        other => panic!("not a polyline: {other:?}"),
    }
}

#[test]
fn the_engine_is_0_4_8_and_its_statement_keys_are_the_verifiers_keys() {
    assert!(
        nf_core::ENGINE_VERSION.starts_with("nf-engine 0.4.9+src."),
        "{}",
        nf_core::ENGINE_VERSION
    );
    // every family's required key is a key this build writes
    for family in [
        "cylinder_shell",
        "cone_frustum",
        "eccentric_reducer",
        "branch_stub",
        "header_shell_with_opening",
    ] {
        for k in required_convention_keys(family) {
            assert!(CONVENTION_KEYS.contains(&k), "{family} {k}");
        }
    }
    assert!(required_convention_keys("unknown_family").is_empty());
}

#[test]
fn a_marking_side_text_that_contradicts_the_view_side_fails_the_gate() {
    // the declared side is outside (NF-VIEW-SIDE); the file's MARKING SIDE line says inside: it must fail
    let out = run_case(&cases()[0].1);
    let (part, text) = &out.dxf[0];
    let conv = stated(&out, part);
    assert!(
        quality_gate_full(text, None, Some(&conv)).pass,
        "as written passes"
    );
    let marking = "  1\nMARKING SIDE: PATTERN DRAWN AS SEEN FROM THE OUTSIDE OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED\n";
    assert!(
        text.contains(marking),
        "the marking side line is in the file"
    );
    let bad = text.replacen(
        marking,
        "  1\nMARKING SIDE: PATTERN DRAWN AS SEEN FROM THE INSIDE OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED\n",
        1,
    );
    let q = quality_gate_full(&bad, None, Some(&conv));
    assert!(
        !q.pass && q.failed_rules().contains(&"convention_statement"),
        "the gate must fail a marking side that contradicts NF-VIEW-SIDE: {:?}",
        q.findings
    );
    // a second marking side line that agrees is also a contradiction (the line must be stated once)
    // a second TEXT entity with the same marking line, before the end of the entities section
    let line = marking.trim_start_matches("  1\n");
    let end = text.rfind("  0\nENDSEC\n").expect("end of entities");
    let twice = format!(
        "{}  0\nTEXT\n  8\nNF_NOT_RELEASED\n 10\n0\n 20\n0\n 40\n1\n  1\n{line}  7\nSTANDARD\n{}",
        &text[..end],
        &text[end..]
    );
    let q = quality_gate_full(&twice, None, Some(&conv));
    assert!(
        !q.pass && q.failed_rules().contains(&"convention_statement"),
        "{:?}",
        q.findings
    );
}
