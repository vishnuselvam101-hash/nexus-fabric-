//! PZ-3: the read-only what-if preview. A preview is two pure computes; applying the same edit through the values or
//! rules API must persist exactly the `after` result the preview showed. The golden set below is fixed so that a
//! regression in one edit kind cannot hide behind a different case.

use std::time::Instant;

use nf_core::order_compute::{self as oc, OrderRules, PartIn};
use nf_core::preview::{Edit, PreviewError, apply_edit, preview, rule_impact};
use nf_core::tee::Mount;
use serde_json::{Value, json};

fn parts() -> Vec<PartIn> {
    vec![
        PartIn::new("C1", "cylinder")
            .v("D", 1016.0)
            .v("t", 12.0)
            .v("L", 2000.0)
            .qty(2),
        PartIn::new("P1", "plate")
            .v("t", 16.0)
            .v("plate_W", 400.0)
            .v("plate_H", 300.0),
        PartIn::new("R1", "cone")
            .v("D1", 1016.0)
            .v("D2", 762.0)
            .v("L", 600.0)
            .v("t", 12.0),
        PartIn::new("B1", "tee_branch")
            .v("D", 508.0)
            .v("t", 9.0)
            .v("H", 600.0)
            .attached("C1"),
    ]
}

fn rules() -> Value {
    json!({"allow": 4.0, "seam_from": "bottom", "mount": "set_on"})
}

fn compute(parts: &[PartIn], rules: &Value) -> oc::OrderResult {
    oc::compute_order(parts, &OrderRules::from_json(rules).unwrap())
}

/// Fifty fixed edits: values, splits and the three order-wide rule changes, cycling so that each kind is exercised
/// ten times against the same starting order.
fn golden(i: usize) -> Edit {
    let length = 900.0 + i as f64 * 23.0;
    match i % 5 {
        0 => Edit::Value {
            part: "C1".into(),
            key: "L".into(),
            value: Some(length),
        },
        1 => Edit::Value {
            part: "P1".into(),
            key: "plate_H".into(),
            value: Some(250.0 + i as f64 * 3.0),
        },
        2 => Edit::Split {
            part: "C1".into(),
            k: 2 + (i % 5) as u32,
        },
        3 => Edit::SeamSide {
            from_bottom: i.is_multiple_of(2),
        },
        _ => Edit::MountNote {
            mount: if i.is_multiple_of(2) {
                Mount::SetIn
            } else {
                Mount::SetOn
            },
        },
    }
}

fn result_of(edit: &Edit) -> (Vec<PartIn>, Value) {
    apply_edit(&parts(), &rules(), edit).unwrap()
}

fn largest(pattern: &[oc::PatternPiece]) -> Option<[f64; 2]> {
    pattern
        .iter()
        .filter_map(|p| Some([p.w?, p.h?]))
        .max_by(|a, b| (a[0] * a[1]).total_cmp(&(b[0] * b[1])))
}

#[test]
fn fifty_golden_edits_preview_exactly_what_the_same_edit_persists() {
    let ps = parts();
    let rs = rules();
    let before = compute(&ps, &rs);
    for i in 0..50 {
        let edit = golden(i);
        let p = preview(&ps, &rs, &edit, true)
            .unwrap_or_else(|e| panic!("golden edit {i} ({:?}) refused: {e:?}", edit.to_json()));
        let (applied_parts, applied_rules) = result_of(&edit);
        let applied = compute(&applied_parts, &applied_rules);

        assert_eq!(
            p.before_fingerprint,
            oc::result_fingerprint(&before),
            "golden edit {i}: the before result moved"
        );
        assert_eq!(
            p.after_fingerprint,
            oc::result_fingerprint(&applied),
            "golden edit {i}: the preview is not the result the values/rules API would persist"
        );
        assert_eq!(
            serde_json::to_value(&p.after).unwrap(),
            serde_json::to_value(&applied).unwrap(),
            "golden edit {i}: the after result differs, not just its fingerprint"
        );
        assert_eq!(p.edit, edit.to_json());
        assert_eq!(p.scope, edit.scope());

        if let Some(no) = edit.part() {
            let d = p.parts.iter().find(|d| d.no == no).unwrap();
            let b = before.parts.iter().find(|b| b.no == no).unwrap();
            let a = applied.parts.iter().find(|a| a.no == no).unwrap();
            assert!(d.edited);
            assert_eq!(
                serde_json::to_value(&d.outline_before).unwrap(),
                serde_json::to_value(&b.pattern).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&d.outline_after).unwrap(),
                serde_json::to_value(&a.pattern).unwrap()
            );
            assert_eq!(d.size_before, largest(&b.pattern));
            assert_eq!(d.size_after, largest(&a.pattern));
            assert_eq!(d.unit_mass_before, b.unit_mass_kg);
            assert_eq!(d.unit_mass_after, a.unit_mass_kg);
        } else {
            assert_eq!(p.scope, "order");
            assert!(p.summary.text_en.starts_with("Whole order:"));
            assert!(p.summary.text_ja.starts_with("工事の全部材："));
        }
    }
}

fn changed_from_computes(before: &oc::OrderResult, after: &oc::OrderResult) -> Vec<String> {
    before
        .parts
        .iter()
        .zip(&after.parts)
        .filter(|(b, a)| {
            serde_json::to_value(&b.pattern).unwrap() != serde_json::to_value(&a.pattern).unwrap()
                || serde_json::to_value((b.unit_mass_kg, b.mass_kg, b.gross_mass_kg)).unwrap()
                    != serde_json::to_value((a.unit_mass_kg, a.mass_kg, a.gross_mass_kg)).unwrap()
        })
        .map(|(b, _)| b.no.clone())
        .collect()
}

#[test]
fn rule_impact_lists_exactly_the_parts_two_full_computes_change() {
    let ps = parts();
    // The development basis is a real rule effect: a cone fan is cut on a different surface.
    let a = json!({"basis": "mid", "seam_from": "bottom", "mount": "set_on"});
    let b = json!({"basis": "outer", "seam_from": "top", "mount": "set_in"});
    let impact = rule_impact(&ps, &a, &b, false).unwrap();
    let before = compute(&ps, &a);
    let after = compute(&ps, &b);
    let expected = changed_from_computes(&before, &after);
    let listed: Vec<String> = impact
        .parts
        .iter()
        .filter(|d| d.pattern_changed || d.mass_changed)
        .map(|d| d.no.clone())
        .collect();

    assert!(
        !expected.is_empty(),
        "this rule change must change a pattern"
    );
    assert_eq!(listed, expected);
    assert_eq!(impact.summary.changed, expected);
    assert_eq!(impact.scope, "order");
    assert_eq!(impact.before_fingerprint, oc::result_fingerprint(&before));
    assert_eq!(impact.after_fingerprint, oc::result_fingerprint(&after));
    assert_eq!(impact.rules_json_after, b);
    let names: Vec<&str> = impact
        .rules_changed
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(names.contains(&"order.basis"));
    assert!(names.contains(&"order.seam"));
    assert!(names.contains(&"order.mount"));
    for d in &impact.parts {
        let b = before.parts.iter().find(|x| x.no == d.no).unwrap();
        let a = after.parts.iter().find(|x| x.no == d.no).unwrap();
        let codes = |hs: &[oc::HoldOut]| -> Vec<(String, String)> {
            hs.iter()
                .map(|h| (h.level.to_string(), h.code.clone()))
                .collect()
        };
        assert_eq!(
            d.holds_added
                .iter()
                .map(|h| (h.level.to_string(), h.code.clone()))
                .collect::<Vec<_>>(),
            codes(&a.holds)
                .into_iter()
                .filter(|x| !codes(&b.holds).contains(x))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            d.holds_removed
                .iter()
                .map(|h| (h.level.to_string(), h.code.clone()))
                .collect::<Vec<_>>(),
            codes(&b.holds)
                .into_iter()
                .filter(|x| !codes(&a.holds).contains(x))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn rules_out_names_every_source_and_marks_every_rule_unverified() {
    let explicit = OrderRules::from_json(&json!({"allow": 4.0})).unwrap();
    let out = explicit.rules_out();
    assert_eq!(out.len(), 11);
    let allow = out.iter().find(|r| r["name"] == "order.allow_mm").unwrap();
    let mount = out.iter().find(|r| r["name"] == "order.mount").unwrap();
    assert_eq!(allow["status"], "UNVERIFIED");
    assert_eq!(mount["status"], "UNVERIFIED");
    assert!(
        allow["source"]
            .as_str()
            .unwrap()
            .contains("this order's rules")
    );
    assert!(
        allow["source_ja"]
            .as_str()
            .unwrap()
            .contains("この工事の決まり")
    );
    assert!(
        mount["source"]
            .as_str()
            .unwrap()
            .contains("provisional default")
    );
    assert!(!mount["source_ja"].as_str().unwrap().is_empty());
    // Spelling out the defaults does not change the effective rules, so it does not stale a decision.
    assert_eq!(explicit.fingerprint(), OrderRules::default().fingerprint());
}

#[test]
fn preview_refuses_unknown_parts_and_invalid_rules_without_computing() {
    let ps = parts();
    let rs = rules();
    let e: PreviewError = preview(
        &ps,
        &rs,
        &Edit::Value {
            part: "nope".into(),
            key: "L".into(),
            value: Some(1.0),
        },
        false,
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "edit_part_unknown");
    assert_eq!(e.fields, vec!["part"]);
    let e = preview(
        &ps,
        &json!({"unknown_rule": 1}),
        &Edit::Split {
            part: "C1".into(),
            k: 2,
        },
        false,
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "rules_invalid");
    let e = preview(
        &ps,
        &rs,
        &Edit::Rules {
            set: json!({"allow": "x"}).as_object().unwrap().clone(),
        },
        false,
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "edit_rules");
}

#[test]
fn preview_100_parts_release_timing() {
    let ps: Vec<PartIn> = (0..100)
        .map(|i| {
            PartIn::new(&format!("C{i:03}"), "cylinder")
                .v("D", 1016.0)
                .v("t", 12.0)
                .v("L", 2000.0)
        })
        .collect();
    let edit = Edit::Value {
        part: "C099".into(),
        key: "L".into(),
        value: Some(2100.0),
    };
    let t = Instant::now();
    let p = preview(&ps, &json!({}), &edit, true).unwrap();
    let elapsed = t.elapsed();
    println!(
        "PZ-3 100-part preview (two computes with outlines): {elapsed:?}; parts={}, changed={}, edited_pieces={}",
        p.summary.parts,
        p.summary.changed.len(),
        p.parts[0].outline_after.len()
    );
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "100-part preview took {elapsed:?}"
    );
    assert_eq!(p.summary.parts, 100);
    assert_eq!(p.summary.changed, vec!["C099".to_string()]);
    assert!(!p.parts[0].outline_after.is_empty());
}
