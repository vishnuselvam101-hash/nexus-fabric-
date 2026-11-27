use nf_reader_data::params::rerun;
use nf_reader_data::render::read_gray;
use nf_reader_data::rng::Rng;
use nf_reader_data::run::{build, request_for, spec_for, write_page};
use nf_reader_data::source::{Source, linear_tolerance, resolve, round_to};
use serde_json::Value;
use std::collections::HashSet;

fn truth_for(split: &str, k: u64) -> Value {
    let (spec, req, out) = request_for(1, split, k, &HashSet::new());
    build(&spec, &req, out).truth
}

fn boxf(v: &Value) -> [f64; 4] {
    let a = v.as_array().unwrap();
    [
        a[0].as_f64().unwrap(),
        a[1].as_f64().unwrap(),
        a[2].as_f64().unwrap(),
        a[3].as_f64().unwrap(),
    ]
}

#[test]
fn same_seed_same_truth() {
    for k in 0..40 {
        assert_eq!(
            truth_for("test", k).to_string(),
            truth_for("test", k).to_string()
        );
    }
    assert_ne!(
        truth_for("test", 0).to_string(),
        truth_for("test", 1).to_string()
    );
}

#[test]
fn boxes_disjoint_and_inside_page() {
    for k in 0..80 {
        let t = truth_for("val", k);
        let (w, h) = (t["width"].as_f64().unwrap(), t["height"].as_f64().unwrap());
        let items = t["items"].as_array().unwrap();
        assert!(items.len() > 15, "page {k} has only {} items", items.len());
        for (i, a) in items.iter().enumerate() {
            let ba = boxf(&a["box_page"]);
            assert!(ba[0] >= 0.0 && ba[1] >= 0.0 && ba[2] <= w && ba[3] <= h);
            for b in &items[i + 1..] {
                let bb = boxf(&b["box_page"]);
                let overlap = ba[0] < bb[2] && bb[0] < ba[2] && ba[1] < bb[3] && bb[1] < ba[3];
                assert!(!overlap, "page {k}: {} overlaps {}", a["text"], b["text"]);
            }
        }
    }
}

#[test]
fn values_equal_engine_output() {
    let mut rng = Rng::new(99);
    let mut checked = 0;
    let mut k = 0;
    while checked < 500 {
        let t = truth_for("train", k * 7 + rng.int(0, 6) as u64);
        k += 1;
        let out = rerun(&t["request"]).expect("engine accepts stored request");
        for it in t["items"].as_array().unwrap() {
            let src: Option<Source> = serde_json::from_value(it["source"].clone()).unwrap();
            let Some(src) = src else { continue };
            let Some(v) = resolve(&out, &src) else {
                continue;
            };
            let dec = it["decimals"].as_u64().unwrap() as usize;
            assert_eq!(
                it["value"].as_f64().unwrap(),
                round_to(v, dec),
                "item {}",
                it["text"]
            );
            if let Some(tol) = it["tolerance"].as_object() {
                let cls = t["general_tolerance"].as_str().unwrap();
                assert_eq!(tol["plus"].as_f64().unwrap(), linear_tolerance(cls, v));
            }
            checked += 1;
        }
    }
    assert!(checked >= 500);
    println!("engine re-query: {checked} items equal");
}

fn percentile(img: &[u8], p: f64) -> f64 {
    let mut h = [0usize; 256];
    for &b in img {
        h[b as usize] += 1;
    }
    let target = (img.len() as f64 * p) as usize;
    let mut acc = 0;
    for (i, c) in h.iter().enumerate() {
        acc += c;
        if acc >= target {
            return i as f64;
        }
    }
    255.0
}

#[test]
fn rendered_boxes_hold_their_text() {
    let root = std::env::temp_dir().join("nf-reader-data-test");
    let _ = std::fs::create_dir_all(root.join("test/png"));
    let _ = std::fs::create_dir_all(root.join("test/pdf"));
    let mut rng = Rng::new(5);
    let mut checked = 0;
    let mut clean_checked = 0;
    // 16 pages across all 40 cells (clean and skewed scans), 13 items each = 208 items
    for n in 0..16u64 {
        let k = n * 5 + 2;
        let (spec, req, out) = request_for(1, "test", k, &HashSet::new());
        let b = build(&spec, &req, out);
        write_page(&root, &spec, &b).expect("render");
        let png = root.join(format!("test/png/test-{k:05}.png"));
        let img = read_gray(&png).unwrap();
        let (w, h) = (b.w as usize, b.h as usize);
        assert_eq!(img.len(), w * h);
        let paper = percentile(&img, 0.5);
        let ink = percentile(&img, 0.01);
        let thr = paper - (paper - ink) * 0.4;
        let items = b.truth["items"].as_array().unwrap().clone();
        for _ in 0..13 {
            let it = &items[rng.int(0, items.len() as i64 - 1) as usize];
            let bx = boxf(&it["box"]);
            let (x0, y0, x1, y1) = (
                bx[0].floor().max(0.0) as usize,
                bx[1].floor().max(0.0) as usize,
                (bx[2].ceil() as usize).min(w - 1),
                (bx[3].ceil() as usize).min(h - 1),
            );
            let mut dark = 0;
            let (mut ix0, mut ix1) = (usize::MAX, 0);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if (img[y * w + x] as f64) < thr {
                        dark += 1;
                        ix0 = ix0.min(x);
                        ix1 = ix1.max(x);
                    }
                }
            }
            assert!(dark > 10, "blank box for {} on page {k}", it["text"]);
            checked += 1;
            if !spec.scan && it["text"].as_str().unwrap().chars().count() >= 5 {
                let (bw, iw) = (bx[2] - bx[0], (ix1 - ix0) as f64);
                assert!(
                    iw >= 0.75 * bw,
                    "ink {iw} much narrower than box {bw} for {} on page {k}",
                    it["text"]
                );
                clean_checked += 1;
            }
        }
    }
    println!(
        "rendered box check: {checked} items non-blank, {clean_checked} width-fit on clean pages"
    );
    assert!(checked >= 200);
    let _ = spec_for(1, "test", 0, 0);
}

#[test]
fn schema_file_is_json() {
    let s = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema.json")).unwrap();
    let v: Value = serde_json::from_str(&s).unwrap();
    assert!(v["$schema"].as_str().unwrap().contains("json-schema"));
    let t = truth_for("test", 3);
    for key in v["required"].as_array().unwrap() {
        assert!(t.get(key.as_str().unwrap()).is_some(), "missing {key}");
    }
    let item_req = v["properties"]["items"]["items"]["required"]
        .as_array()
        .unwrap();
    for it in t["items"].as_array().unwrap() {
        for key in item_req {
            assert!(
                it.get(key.as_str().unwrap()).is_some(),
                "item missing {key}"
            );
        }
    }
}
