//! Work plan 3.3: every gate G00-G23, every check name, every refusal code and every record kind has a Japanese label
//! (or message) in the client's catalogue `web/i18n.js`, and every gate and refusal code has its text on the offline
//! help page (3.5). The lists come from the build itself: the refusal codes from `nf_core::refusal::ALL`, the gate ids
//! from the engine and server sources, the check names, record kinds, record families and relations from jobs the
//! engine computes for every family (the example values of the new-calculation form and the requests of
//! crates/nf-core/tests/refusal_codes.rs; no new value).
use nf_core::job::{self, Request};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn i18n() -> String {
    std::fs::read_to_string(root().join("web/i18n.js")).expect("web/i18n.js")
}

/// The text of the object literal that starts with `start` (up to its closing line `end`).
fn block<'a>(src: &'a str, start: &str, end: &str) -> &'a str {
    let s = src
        .find(start)
        .unwrap_or_else(|| panic!("{start:?} not found in web/i18n.js"));
    let e = src[s..]
        .find(end)
        .unwrap_or_else(|| panic!("end of {start:?} not found"));
    &src[s + start.len()..s + e]
}

/// The entries `key: [english, japanese ...]` of an identifier kind of the table ID: (key, English or None, Japanese).
fn id_entries(src: &str, kind: &str) -> Vec<(String, Option<String>, String)> {
    let ids = block(src, "\nconst ID = {", "\n};\n");
    let body = block(ids, &format!("\n  {kind}: {{"), "\n  },");
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find(": [") {
        // the key: a quoted string or a bare identifier, just before ": ["
        let before = rest[..i].trim_end();
        let key = if let Some(stripped) = before.strip_suffix('\'') {
            let q = stripped.rfind('\'').expect("opening quote");
            stripped[q + 1..].to_string()
        } else {
            let k = before
                .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .map_or(0, |p| p + 1);
            before[k..].to_string()
        };
        let after = &rest[i + 3..];
        let close = after.find(']').expect("closing bracket");
        let items: Vec<&str> = after[..close].split("', ").collect();
        let strip = |s: &str| {
            s.trim()
                .trim_start_matches('\'')
                .trim_end_matches('\'')
                .to_string()
        };
        let en = if items[0].trim() == "null" || items[0].trim().starts_with("null,") {
            None
        } else {
            Some(strip(items[0]))
        };
        let ja = if en.is_none() {
            strip(
                after[..close]
                    .trim()
                    .trim_start_matches("null,")
                    .split("', ")
                    .next()
                    .unwrap_or(""),
            )
        } else {
            strip(items.get(1).copied().unwrap_or(""))
        };
        out.push((key, en, ja));
        rest = &after[close..];
    }
    out
}

fn has_ja(text: &str) -> bool {
    text.chars()
        .any(|c| ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c))
}

#[test]
fn every_refusal_code_has_a_message_in_english_and_japanese() {
    let src = i18n();
    let en = block(&src, "const REFUSAL_EN = {", "\n};");
    let ja = block(&src, "const REFUSAL_JA = {", "\n};");
    for code in nf_core::refusal::ALL {
        let key = format!("'refusal.{code}': '");
        assert_eq!(en.matches(&key).count(), 1, "English message of {code}");
        let at = ja
            .find(&key)
            .unwrap_or_else(|| panic!("no Japanese message for {code}"));
        let line = &ja[at..ja[at..].find('\n').map_or(ja.len(), |e| at + e)];
        assert!(
            has_ja(line),
            "the Japanese message of {code} is not Japanese: {line}"
        );
    }
    // and no message for a code the build does not have
    for blk in [en, ja] {
        assert_eq!(
            blk.matches("'refusal.").count(),
            nf_core::refusal::ALL.len()
        );
    }
}

/// Gate ids written as string literals in the engine and server sources.
fn gates_in_sources() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for dir in [
        "crates/nf-core/src",
        "crates/nf-server/src",
        "crates/uem/src",
    ] {
        for entry in std::fs::read_dir(root().join(dir)).expect(dir) {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&path).expect("source");
                let b = text.as_bytes();
                for i in 0..b.len().saturating_sub(4) {
                    if b[i] == b'"'
                        && b[i + 1] == b'G'
                        && b[i + 2].is_ascii_digit()
                        && b[i + 3].is_ascii_digit()
                        && b[i + 4] == b'"'
                    {
                        out.insert(text[i + 1..i + 4].to_string());
                    }
                }
            }
        }
    }
    out
}

#[test]
fn every_gate_g00_to_g23_has_a_label_and_a_help_text_in_both_languages() {
    let src = i18n();
    let gates = id_entries(&src, "gate");
    let ids: Vec<String> = gates.iter().map(|g| g.0.clone()).collect();
    let all: Vec<String> = (0..=23).map(|i| format!("G{i:02}")).collect();
    assert_eq!(ids, all, "the gate table lists G00 to G23 in order");
    for (id, en, ja) in &gates {
        assert!(
            en.as_deref().is_some_and(|e| !e.is_empty()),
            "{id}: English name"
        );
        assert!(has_ja(ja), "{id}: Japanese label {ja:?}");
        assert_eq!(
            src.matches(&format!("'hp.gate.{id}': '")).count(),
            2,
            "{id}: help text in English and Japanese"
        );
    }
    // the gates the build evaluates are exactly those not listed as not evaluated
    let line = src
        .lines()
        .find(|l| l.starts_with("export const GATES_NOT_EVALUATED"))
        .expect("GATES_NOT_EVALUATED");
    let not_evaluated: BTreeSet<String> = line
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(String::from)
        .collect();
    let evaluated = gates_in_sources();
    assert!(
        evaluated.is_disjoint(&not_evaluated),
        "a gate listed as not evaluated is in the sources: {:?}",
        evaluated.intersection(&not_evaluated).collect::<Vec<_>>()
    );
    let union: BTreeSet<String> = evaluated.union(&not_evaluated).cloned().collect();
    assert_eq!(
        union,
        all.into_iter().collect::<BTreeSet<_>>(),
        "every gate is either evaluated by the build or listed as not evaluated"
    );
}

fn requests() -> Vec<Value> {
    // the example values of the new-calculation form (web/app.js, F) with each declared surface, and the accepted
    // set-on joint of crates/nf-core/tests/refusal_codes.rs
    let hb = json!({"family": "header_with_branch", "header_radius": 150.0, "header_dev_radius": 150.0, "header_length": 1200.0,
        "branch_station": 600.0, "branch_radius": 100.0, "branch_dev_radius": 100.0, "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0});
    let stub = json!({"family": "branch_stub", "header_radius": 150.0, "branch_radius": 100.0, "branch_dev_radius": 100.0,
        "beta_deg": 60.0, "delta": 0.0, "s_end": 400.0});
    let mut out = vec![
        json!({"family": "cylinder_shell", "r_dev": 500.0, "length": 1000.0}),
        json!({"family": "cone_frustum", "r1": 200.0, "r2": 100.0, "h": 300.0}),
        json!({"family": "eccentric_reducer", "r1": 200.0, "r2": 100.0, "h": 300.0, "e": 100.0}),
        json!({"family": "header_with_branch", "header_radius": 161.95, "header_dev_radius": 156.795, "header_length": 1200.0,
            "branch_station": 600.0, "branch_radius": 77.095, "branch_dev_radius": 80.6, "beta_deg": 90.0, "delta": 0.0, "s_end": 400.0,
            "header_surface_reference": "outside", "branch_surface_reference": "inside", "joint": "set_on"}),
    ];
    for sref in ["outside", "unknown"] {
        for base in [&hb, &stub] {
            let mut v = base.clone();
            v["surface_reference"] = json!(sref);
            out.push(v);
        }
        for fam in ["cylinder_shell", "cone_frustum", "eccentric_reducer"] {
            let mut v = out
                .iter()
                .find(|r| r["family"] == fam)
                .expect("family")
                .clone();
            v["surface_reference"] = json!(sref);
            out.push(v);
        }
    }
    out
}

#[test]
fn every_check_name_record_kind_family_and_relation_of_the_engine_has_a_japanese_label() {
    let src = i18n();
    let labelled = |kind: &str| -> BTreeSet<String> {
        id_entries(&src, kind)
            .into_iter()
            .filter(|(_, _, ja)| has_ja(ja))
            .map(|e| e.0)
            .collect()
    };
    let (checks, kinds, families, rels, gates) = (
        labelled("check"),
        labelled("rkind"),
        labelled("rfamily"),
        labelled("rel"),
        labelled("gate"),
    );
    let mut missing = BTreeSet::new();
    let mut seen = (0, 0);
    for v in requests() {
        let req = Request::parse_refusal(&v).unwrap_or_else(|r| panic!("{v}: {}", r.message));
        let out = job::run(&req, "labels-test");
        assert!(out.refusal.is_none(), "{v}: refused");
        let o = serde_json::to_value(&out).expect("job output");
        for c in o["checks"].as_array().expect("checks") {
            seen.0 += 1;
            let n = c["name"].as_str().expect("check name");
            if !checks.contains(n) {
                missing.insert(format!("check {n}"));
            }
        }
        for g in o["gates"].as_array().expect("gates") {
            let id = g["id"].as_str().expect("gate id");
            if !gates.contains(id) {
                missing.insert(format!("gate {id}"));
            }
        }
        for r in o["records"].as_array().expect("records") {
            seen.1 += 1;
            let k = r["kind"].as_str().expect("record kind");
            if !kinds.contains(k) {
                missing.insert(format!("record kind {k}"));
            }
            let f = r["family"].as_str().expect("record family");
            if !families.contains(f) {
                missing.insert(format!("record family {f}"));
            }
            for rel in r["relations"].as_array().into_iter().flatten() {
                let n = rel["rel"].as_str().expect("relation");
                if !rels.contains(n) {
                    missing.insert(format!("relation {n}"));
                }
            }
        }
    }
    assert!(
        seen.0 > 50 && seen.1 > 50,
        "the jobs produced checks and records: {seen:?}"
    );
    assert!(
        missing.is_empty(),
        "identifiers without a Japanese label in web/i18n.js: {missing:?}"
    );
}
