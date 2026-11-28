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

#[test]
fn weak1_clutter_is_printed_but_never_in_the_answer_key() {
    let d = tmp("weak1");
    assert_eq!(generate_smoke_mix(&d, 48, 5, 4, Mix::Weak1).unwrap(), 48);
    let text = std::fs::read_to_string(d.join("smoke/truth.jsonl")).unwrap();
    let (mut clutter, mut items, mut max_bom) = (0, 0, 0);
    for line in text.lines() {
        let t: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(t["mix"], "weak1");
        let cl: Vec<&str> = t["clutter"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        assert!(!cl.is_empty(), "{}", t["page_id"]);
        let its = t["items"].as_array().unwrap();
        max_bom = max_bom.max(its.iter().filter(|i| i["type"] == "bom").count());
        for it in its {
            let txt = it["text"].as_str().unwrap();
            // the title block, notes, grid references, revision table, part and drawing numbers are not labelled
            // (single grid letters and digits can coincide with a BOM number by text; compare longer strings)
            assert!(
                !cl.iter().any(|c| c.chars().count() > 2 && *c == txt),
                "{} {txt}",
                t["page_id"]
            );
            assert!(
                !txt.starts_with("DWG NO") && !txt.starts_with("P/N"),
                "{txt}"
            );
        }
        clutter += cl.len();
        items += its.len();
    }
    assert!(clutter > items / 4, "clutter {clutter} items {items}");
    assert!(max_bom > 8, "long BOM tables: {max_bom}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn default_mix_has_no_clutter_and_is_unchanged_by_the_mix_argument() {
    let (a, b) = (tmp("m1"), tmp("m2"));
    generate_smoke(&a, 24, 9, 3).unwrap();
    generate_smoke_mix(&b, 24, 9, 2, Mix::Default).unwrap();
    let ta = std::fs::read_to_string(a.join("smoke/truth.jsonl")).unwrap();
    assert_eq!(
        ta,
        std::fs::read_to_string(b.join("smoke/truth.jsonl")).unwrap()
    );
    assert!(!ta.contains("\"clutter\""));
    let _ = std::fs::remove_dir_all(a);
    let _ = std::fs::remove_dir_all(b);
}
