use nf_reader_data::derive::derive;
use nf_reader_data::source::{Source, round_to};
use nf_reader_data::v2::{Mix, generate_smoke, generate_smoke_mix, resolve_v2};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nf-reader-data-v2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn smoke_is_deterministic_by_seed() {
    let (a, b) = (tmp("a"), tmp("b"));
    assert_eq!(generate_smoke(&a, 16, 7, 3).unwrap(), 16);
    assert_eq!(generate_smoke(&b, 16, 7, 2).unwrap(), 16);
    let ta = std::fs::read_to_string(a.join("smoke/truth.jsonl")).unwrap();
    let tb = std::fs::read_to_string(b.join("smoke/truth.jsonl")).unwrap();
    assert_eq!(ta, tb);
    let _ = std::fs::remove_dir_all(a);
    let _ = std::fs::remove_dir_all(b);
}

#[test]
fn every_spec_and_derived_value_re_resolves_to_its_printed_value() {
    let d = tmp("resolve");
    assert_eq!(generate_smoke(&d, 40, 3, 4).unwrap(), 40);
    let text = std::fs::read_to_string(d.join("smoke/truth.jsonl")).unwrap();
    let (mut spec, mut derived) = (0, 0);
    for line in text.lines() {
        let t: serde_json::Value = serde_json::from_str(line).unwrap();
        let dec = t["decimals"].as_u64().unwrap() as usize;
        for it in t["items"].as_array().unwrap() {
            let Some(src) = it["source"].as_object() else {
                continue;
            };
            let kind = src.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            if kind != "spec" && kind != "derived" {
                continue;
            }
            let s: Source = serde_json::from_value(it["source"].clone()).unwrap();
            let v = resolve_v2(&t["spec"]["params"], &s).expect("resolvable");
            let printed = it["value"].as_f64().unwrap();
            assert_eq!(round_to(v, dec), printed, "{} {}", t["page_id"], it["text"]);
            if kind == "spec" {
                spec += 1
            } else {
                derived += 1
            }
        }
    }
    assert!(spec > 0 && derived > 0, "spec {spec} derived {derived}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn derived_formula_matches_truth_inputs() {
    let inputs = [("bcd", 100.0), ("n", 4.0)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let v = derive("bolt_pitch", &inputs).unwrap();
    assert_eq!(round_to(v, 1), 70.7);
}

fn overlaps(a: &[serde_json::Value], b: &[serde_json::Value]) -> bool {
    let f =
        |v: &[serde_json::Value]| -> Vec<f64> { v.iter().map(|x| x.as_f64().unwrap()).collect() };
    let (a, b) = (f(a), f(b));
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// One key policy for every page and mix: title/notes text is clutter, never an item; compare by id and box.
fn check_policy(mix: Mix, n: u64, seed: u64) -> (usize, usize, usize) {
    let d = tmp(&format!("policy-{}", mix.name()));
    assert_eq!(generate_smoke_mix(&d, n, seed, 4, mix).unwrap(), n as usize);
    let text = std::fs::read_to_string(d.join("smoke/truth.jsonl")).unwrap();
    let (mut clutter, mut items, mut max_bom) = (0, 0, 0);
    for line in text.lines() {
        let t: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(t["mix"], mix.name());
        let cl = t["clutter"].as_array().unwrap();
        let its = t["items"].as_array().unwrap();
        assert!(mix == Mix::Default || !cl.is_empty(), "{}", t["page_id"]);
        max_bom = max_bom.max(its.iter().filter(|i| i["type"] == "bom").count());
        for it in its {
            assert!(it["type"] != "title", "{}", it["text"]);
            if it["type"] == "title_field" {
                assert!(
                    [
                        "material",
                        "thickness",
                        "quantity",
                        "drawing_no",
                        "scale",
                        "rev"
                    ]
                    .contains(&it["field"].as_str().unwrap()),
                    "{}",
                    it["text"]
                );
            }
            if it["type"] == "note" {
                assert!(it["number"].is_array());
            }
            for c in cl {
                assert_ne!(it["id"], c["id"]);
                assert!(
                    !overlaps(
                        it["box_page"].as_array().unwrap(),
                        c["box_page"].as_array().unwrap()
                    ),
                    "{} item {} overlaps clutter {}",
                    t["page_id"],
                    it["text"],
                    c["text"]
                );
            }
        }
        clutter += cl.len();
        items += its.len();
    }
    let _ = std::fs::remove_dir_all(d);
    (items, clutter, max_bom)
}

#[test]
fn clutter_is_printed_but_never_in_the_answer_key_in_every_mix() {
    let (wi, wc, wb) = check_policy(Mix::Weak1, 48, 5);
    assert!(wc > wi / 4, "clutter {wc} items {wi}");
    assert!(wb > 8, "long BOM tables: {wb}");
    let (di, dc, _) = check_policy(Mix::Default, 48, 5);
    assert!(dc > 0 && di > 0);
}

#[test]
fn title_facts_and_fabrication_notes_are_keyed_with_types() {
    let d = tmp("typed");
    generate_smoke_mix(&d, 32, 11, 4, Mix::Default).unwrap();
    let text = std::fs::read_to_string(d.join("smoke/truth.jsonl")).unwrap();
    for line in text.lines() {
        let t: serde_json::Value = serde_json::from_str(line).unwrap();
        let its = t["items"].as_array().unwrap();
        for f in [
            "material",
            "thickness",
            "quantity",
            "drawing_no",
            "scale",
            "rev",
        ] {
            assert!(
                its.iter()
                    .any(|i| i["type"] == "title_field" && i["field"] == f),
                "{} lacks {f}",
                t["page_id"]
            );
        }
        assert!(its.iter().any(|i| i["type"] == "note"), "{}", t["page_id"]);
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn default_mix_is_unchanged_by_the_mix_argument() {
    let (a, b) = (tmp("m1"), tmp("m2"));
    generate_smoke(&a, 24, 9, 3).unwrap();
    generate_smoke_mix(&b, 24, 9, 2, Mix::Default).unwrap();
    let ta = std::fs::read_to_string(a.join("smoke/truth.jsonl")).unwrap();
    assert_eq!(
        ta,
        std::fs::read_to_string(b.join("smoke/truth.jsonl")).unwrap()
    );
    let _ = std::fs::remove_dir_all(a);
    let _ = std::fs::remove_dir_all(b);
}
