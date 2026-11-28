//! PZ-1: review catalogue, ZK listed-kind parity, boundaries and the never-wave list.

use nf_core::order_compute::{self, OrderRules, PartIn};
use nf_core::review::{self, NEVER_WAVE, ReviewContext};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn has_ja(s: &str) -> bool {
    s.chars()
        .any(|c| ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c))
}

#[test]
fn every_hold_code_has_catalogue_row() {
    let cat: BTreeSet<String> = review::codes().into_iter().collect();
    for code in order_compute::hold_codes() {
        assert!(cat.contains(code), "hold code {code} has no catalogue row");
        let r = review::row(code).unwrap_or_else(|| panic!("no catalogue row for {code}"));
        assert!(!r.title_en.is_empty(), "{code}: English title");
        assert!(!r.title_ja.is_empty(), "{code}: Japanese title");
    }
    for code in [
        "bend_joints_not_on_drawing",
        "tee_x_not_on_drawing",
        "tee_dir_not_on_drawing",
        "mass_total",
        "row_unmatched",
        "page_empty",
        "shape_unsupported",
        "seam_from_unknown",
        "tee_angle_obtuse",
        "tee_flange_face",
        "tee_mount_note",
        "pad_qty",
        "tee_qty",
        "split_cone",
        "split_ring",
        "pad_split",
        "size_vs_table",
        "cyl_len",
        "bend_len",
        "mass",
        "stock_rule",
        "unreadable",
    ] {
        assert!(
            cat.contains(code),
            "review code {code} has no catalogue row"
        );
    }
    let mut seen = BTreeSet::new();
    for code in review::codes() {
        assert!(seen.insert(code.clone()), "duplicate catalogue code {code}");
    }
    println!(
        "review catalogue: {} rows, {} hold codes, {} never-wave codes",
        review::codes().len(),
        order_compute::hold_codes().len(),
        NEVER_WAVE.len()
    );
}

#[test]
fn catalogue_has_english_and_japanese_answers() {
    for r in review::catalogue() {
        assert!(!r.title_en.trim().is_empty(), "{}: English title", r.code);
        assert!(
            !r.answers_en.trim().is_empty(),
            "{}: English answer",
            r.code
        );
        assert!(has_ja(&r.title_ja), "{}: Japanese title {r:?}", r.code);
        assert!(has_ja(&r.answers_ja), "{}: Japanese answer {r:?}", r.code);
    }
    let v = review::catalogue_json();
    let rows = v.as_array().expect("catalogue array");
    assert_eq!(rows.len(), review::codes().len());
    for row in rows {
        for key in ["code", "title_en", "title_ja", "answers_en", "answers_ja"] {
            assert!(row[key].as_str().is_some_and(|s| !s.is_empty()), "{key}");
        }
    }
}

#[test]
fn never_wave_list_covers_flat_stop_and_the_boundary_kinds() {
    let never: BTreeSet<&str> = NEVER_WAVE.iter().copied().collect();
    for code in [
        "FLAT_STOP",
        "split_ring",
        "seam_from",
        "missing",
        "unsupported",
        "empty",
        "split_cone",
        "tee_bad",
        "tee_branches",
        "plate_bad",
        "plate_off",
        "plate_C_big",
        "rib_count",
        "rib_n1",
        "bend_bad",
        "wye_bad",
    ] {
        assert!(never.contains(code), "{code} may be waved through");
    }
    assert!(
        !never.contains("mass"),
        "an information review is not a never-wave kind"
    );
    for code in NEVER_WAVE {
        assert!(
            review::row(code).is_some(),
            "never-wave code {code} is not catalogued"
        );
    }
}

fn ctx_with_mass(table_mass: f64, unit: f64) -> ReviewContext {
    ReviewContext {
        pages: 1,
        unit_mass_kg: Some(unit),
        material_rows: vec![review::MaterialRow {
            no: "1".into(),
            mass: Some(table_mass),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn codes(res: &order_compute::OrderResult) -> Vec<&str> {
    res.review_issues.iter().map(|i| i.code.as_str()).collect()
}

#[test]
fn mass_total_boundary_is_one_kg_or_point_two_percent() {
    let p = [PartIn::new("1", "plate")];
    let r = OrderRules::default();
    let level = |table: f64, unit: f64| {
        let c = ctx_with_mass(table, unit);
        let got = review::compute_order_with_context(&p, &r, &c);
        got.review_issues
            .iter()
            .find(|i| i.code == "mass_total")
            .expect("mass total review")
            .level
    };
    assert_eq!(level(101.0, 100.0), "info");
    assert_eq!(level(101.01, 100.0), "need");
    // 0.2 % of a large total is larger than 1 kg.
    assert_eq!(level(1002.0, 1000.0), "info");
    assert_eq!(level(1002.01, 1000.0), "need");
}

#[test]
fn one_millimetre_boundaries_do_not_raise_a_length_hold() {
    let r = OrderRules::default();
    for (w, want_len) in [(300.0, false), (301.0, false), (301.01, true)] {
        let p = [PartIn::new("1", "plate")
            .v("t", 9.0)
            .v("plate_W", w)
            .v("plate_H", 200.0)];
        let mut c = ReviewContext {
            pages: 1,
            ..Default::default()
        };
        c.material_rows.push(review::MaterialRow {
            no: "1".into(),
            size: "PL-9×300×200".into(),
            mass: Some(4.24),
            part: Some("1".into()),
            ..Default::default()
        });
        let got = review::compute_order_with_context(&p, &r, &c);
        assert!(codes(&got).contains(&"size_vs_table"));
        let issue = got
            .review_issues
            .iter()
            .find(|i| i.code == "size_vs_table")
            .expect("size review");
        assert_eq!(issue.level == "need", want_len, "plate width {w}");
        if want_len {
            assert!(got.part("1").is_some_and(|x| x.has_hold("size_vs_table")));
        }
    }
    for (len, want) in [(2000.0, false), (2001.0, false), (2001.01, true)] {
        let p = [PartIn::new("1", "cylinder")
            .v("D", 1000.0)
            .v("t", 9.0)
            .v("split", 2.0)
            .v("L", len)];
        let mut c = ReviewContext {
            pages: 1,
            ..Default::default()
        };
        c.material_rows.push(review::MaterialRow {
            no: "1".into(),
            length: "2000".into(),
            part: Some("1".into()),
            ..Default::default()
        });
        let got = review::compute_order_with_context(&p, &r, &c);
        let issue = got
            .review_issues
            .iter()
            .find(|i| i.code == "cyl_len")
            .expect("cylinder length review");
        assert_eq!(issue.level == "need", want, "cylinder length {len}");
        if want {
            assert!(got.part("1").is_some_and(|x| x.has_hold("cyl_len")));
        }
    }
}

#[test]
fn mass_tolerance_is_half_a_percent() {
    let r = OrderRules::default();
    let rows: &[(f64, bool)] = &[(100.0, false), (100.5, false), (100.51, true)];
    for &(calc, want) in rows {
        let p = [PartIn::new("1", "purchased")];
        let mut c = ReviewContext {
            pages: 1,
            ..Default::default()
        };
        c.material_rows.push(review::MaterialRow {
            no: "1".into(),
            mass: Some(100.0),
            part: Some("1".into()),
            ..Default::default()
        });
        let mut got = review::compute_order_with_context(&p, &r, &c);
        // Purchased parts have no computed mass. The table row is still reviewed; a computed value, when present,
        // is the one compared. The explicit tolerance table is checked here so a change cannot pass silently.
        if let Some(m) = got.part_mut("1") {
            m.mass_kg = Some(calc);
        }
        assert!(
            (calc - 100.0).abs() / 100.0 <= nf_core::order::MASS_TABLE_REL_TOL + f64::EPSILON
                || want,
            "tolerance table"
        );
        assert_eq!(
            want,
            (calc - 100.0).abs() / 100.0 > nf_core::order::MASS_TABLE_REL_TOL
        );
        assert!(
            codes(&got).contains(&"mass"),
            "a table mass row raises the mass review"
        );
    }
}

fn golden() -> PathBuf {
    root().join("tests").join("golden").join("zumen")
}

fn got_codes(res: &order_compute::OrderResult) -> HashSet<(String, Option<String>)> {
    let mut out: HashSet<(String, Option<String>)> = res
        .review_issues
        .iter()
        .map(|i| (i.code.clone(), i.part.clone()))
        .collect();
    for p in &res.parts {
        let codes = p
            .holds
            .iter()
            .map(|h| h.code.as_str())
            .chain(p.questions.iter().map(|q| q.code));
        for code in codes {
            if review::LISTED_CODES.contains(&code) {
                out.insert((code.to_string(), Some(p.no.clone())));
            }
        }
    }
    out
}

#[test]
fn each_reading_raises_the_zk_listed_review_kinds_on_the_same_part() {
    let expected: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(golden().join("review_kinds.json")).unwrap())
            .unwrap();
    let dir = golden().join("readings");
    let mut readings: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    readings.sort();
    assert_eq!(readings.len(), 23, "every golden reading is replayed");
    let mut checked = 0usize;
    for name in &readings {
        let reading: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap();
        let (parts, ctx) = review::parts_and_context_from_reading(&reading);
        let res = review::compute_order_with_context(&parts, &OrderRules::default(), &ctx);
        let got = got_codes(&res);
        let want: Vec<&Value> = expected
            .iter()
            .filter(|e| e["file"].as_str() == Some(name.as_str()))
            .collect();
        for e in &want {
            let code = e["code"].as_str().unwrap().to_string();
            let part = e["part"].as_str().map(str::to_string);
            let found = got.iter().any(|(c, p)| {
                c == &code
                    && match (&part, p) {
                        (Some(a), Some(b)) => a == b,
                        (None, _) => true,
                        _ => false,
                    }
            });
            assert!(
                found,
                "{name}: ZK {} on {:?} (Rust code {code}); got {:?}",
                e["kind"], part, got
            );
            checked += 1;
        }
        // No listed kind on a reading where ZK did not raise it.
        let want_codes: BTreeSet<&str> = want.iter().filter_map(|e| e["code"].as_str()).collect();
        for (code, part) in &got {
            assert!(
                want_codes.contains(code.as_str()),
                "{name}: Rust raised {code} on {part:?} but ZK did not list it"
            );
        }
    }
    assert_eq!(checked, expected.len(), "every expected ZK row was checked");
    println!(
        "review golden: {} readings, {} listed ZK rows, {} matched",
        readings.len(),
        expected.len(),
        checked
    );
}
