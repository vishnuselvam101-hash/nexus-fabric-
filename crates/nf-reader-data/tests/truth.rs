use nf_reader_data::params::rerun;
use nf_reader_data::render::{read_gray, render_png, svg_text_only};
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
                assert_eq!(
                    Some(tol["plus"].as_f64().unwrap()),
                    linear_tolerance(cls, v)
                );
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

fn ink_stats(
    img: &[u8],
    w: usize,
    h: usize,
    win: [f64; 4],
    skip: &[[f64; 4]],
    thr: f64,
    denoise: bool,
) -> Option<[f64; 4]> {
    let (x0, y0) = (
        win[0].floor().max(0.0) as usize,
        win[1].floor().max(0.0) as usize,
    );
    let (x1, y1) = (
        (win[2].ceil() as usize).min(w - 1),
        (win[3].ceil() as usize).min(h - 1),
    );
    let mut r = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for y in y0..=y1 {
        for x in x0..=x1 {
            if (img[y * w + x] as f64) >= thr {
                continue;
            }
            // scan noise leaves isolated dark pixels: ink has at least 3 dark neighbours
            let mut nb = 0;
            for (dy, dx) in [
                (-1i64, -1i64),
                (-1, 0),
                (-1, 1),
                (0, -1),
                (0, 1),
                (1, -1),
                (1, 0),
                (1, 1),
            ] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx >= 0
                    && ny >= 0
                    && (nx as usize) < w
                    && (ny as usize) < h
                    && (img[ny as usize * w + nx as usize] as f64) < thr
                {
                    nb += 1;
                }
            }
            if denoise && nb < 3 {
                continue;
            }
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            if skip
                .iter()
                .any(|b| fx > b[0] && fx < b[2] && fy > b[1] && fy < b[3])
            {
                continue;
            }
            r[0] = r[0].min(x as f64);
            r[1] = r[1].min(y as f64);
            r[2] = r[2].max(x as f64 + 1.0);
            r[3] = r[3].max(y as f64 + 1.0);
        }
    }
    (r[0].is_finite()).then_some(r)
}

fn iou(a: [f64; 4], b: [f64; 4]) -> f64 {
    let iw = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let ih = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let i = iw * ih;
    let u = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - i;
    i / u
}

#[test]
fn text_only_render_ink_matches_truth_boxes() {
    let root = std::env::temp_dir().join("nf-reader-data-test-textonly");
    let _ = std::fs::create_dir_all(&root);
    let (mut n, mut worst, mut worst_t) = (0, 1.0f64, String::new());
    let (mut gsum, mut gn) = ([0.0f64; 2], [0usize; 2]);
    let mut sum = 0.0;
    // 40 cells: every family x language x style x clean/scan once
    for k in 0..40u64 {
        let (spec, req, out) = request_for(1, "test", k, &HashSet::new());
        let b = build(&spec, &req, out);
        let skew = b.fx.as_ref().map(|f| f.skew).unwrap_or(0.0);
        let paper = if spec.scan { "#f3f1ea" } else { "#ffffff" };
        let png = root.join(format!("t-{k}.png"));
        render_png(
            &svg_text_only(&b.page, skew, paper),
            b.w,
            b.h,
            &png,
            b.fx.as_ref(),
        )
        .expect("render");
        let img = read_gray(&png).unwrap();
        let (w, h) = (b.w as usize, b.h as usize);
        let (paper_v, ink_v) = (percentile(&img, 0.5), percentile(&img, 0.002));
        let thr = paper_v - (paper_v - ink_v) * 0.4;
        let boxes: Vec<[f64; 4]> = b.truth["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| boxf(&i["box"]))
            .collect();
        for (i, bx) in boxes.iter().enumerate() {
            let win = [bx[0] - 4.0, bx[1] - 4.0, bx[2] + 4.0, bx[3] + 4.0];
            let others: Vec<[f64; 4]> = boxes
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| *b)
                .collect();
            let ink = ink_stats(&img, w, h, win, &others, thr, spec.scan);
            let v = ink.map(|r| iou(r, *bx)).unwrap_or(0.0);
            let t = b.truth["items"][i]["text"].as_str().unwrap();
            if v < worst {
                worst = v;
                worst_t = format!("{t} page {k} scan {}", spec.scan);
            }
            sum += v;
            n += 1;
            // pixel quantisation limits very short or small strings (one glyph is ~8 px wide, small BOM text is ~12 px high), and scan noise next to a
            // box can add a stray cluster: those get a lower per-item floor, every group must still average >= 0.9
            let floor = if spec.scan {
                if t.chars().count() >= 3 {
                    0.7
                } else if t.chars().count() == 1 {
                    0.5
                } else {
                    0.6
                }
            } else if t.chars().count() >= 3 && bx[3] - bx[1] >= 18.0 {
                0.9
            } else if t.chars().count() >= 3 {
                0.85
            } else if t.chars().count() == 1 {
                0.6
            } else {
                0.7
            };
            assert!(
                v >= floor,
                "ink IoU {v:.3} < {floor} for {t:?} on page {k} (scan {}): box {bx:?} ink {ink:?}",
                spec.scan
            );
            let g = usize::from(spec.scan);
            gsum[g] += v;
            gn[g] += 1;
        }
    }
    for g in 0..2 {
        let m = gsum[g] / gn[g] as f64;
        println!(
            "text-only ink IoU {}: {} items, mean {m:.3}",
            ["clean", "scan"][g],
            gn[g]
        );
        assert!(m >= 0.9, "mean ink IoU {m:.3} for group {g}");
    }
    println!(
        "text-only ink IoU: {n} items, mean {:.3}, worst {worst:.3} ({worst_t})",
        sum / n as f64
    );
}

fn built(split: &str, k: u64) -> nf_reader_data::run::Built {
    let (spec, req, out) = request_for(1, split, k, &HashSet::new());
    build(&spec, &req, out)
}

#[test]
fn no_radius_label_on_a_full_diameter_span() {
    let (mut r_items, mut phi_items) = (0, 0);
    for k in 0..200u64 {
        let b = built("val", k);
        let u = b.page.u;
        for it in &b.page.items {
            let Some(Source::Request { factor, .. }) = &it.src else {
                continue;
            };
            if it.text.starts_with('R') {
                // radius: the value is the request radius itself, and it hangs on a leader (half-span), never a span line
                assert_eq!(*factor, 1.0, "R label with a diameter value: {}", it.text);
                let (lx, ly) = (it.b[0] - 4.0 * u, (it.b[1] + it.b[3]) / 2.0);
                assert!(
                    b.page
                        .segs
                        .iter()
                        .any(|s| (s[0] - lx).abs() < 0.5 && (s[1] - ly).abs() < 0.5),
                    "R label without a leader on page {k}: {}",
                    it.text
                );
                r_items += 1;
            }
            if it.text.starts_with('\u{03C6}') {
                assert_eq!(*factor, 2.0, "phi label with a radius value: {}", it.text);
                phi_items += 1;
            }
        }
    }
    println!("R items {r_items}, phi items {phi_items}");
    assert!(phi_items > 100);
}

#[test]
fn no_line_segment_crosses_a_text_box() {
    let mut segs = 0usize;
    for k in 0..160u64 {
        let b = built("test", k);
        for s in &b.page.segs {
            segs += 1;
            for it in &b.page.items {
                assert!(
                    !nf_reader_data::layout::seg_hits(*s, it.b, 0.0),
                    "page {k}: segment {s:?} crosses text {:?} {:?}; near: {:?}",
                    it.text,
                    it.b,
                    b.page
                        .segs
                        .iter()
                        .filter(|q| (q[0] - s[0]).abs() < 40.0 && (q[1] - s[1]).abs() < 40.0)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    println!("segments checked: {segs}");
}

#[test]
fn class_f_tolerance_stops_at_2000_mm() {
    assert_eq!(linear_tolerance("f", 2000.0), Some(0.5));
    assert_eq!(linear_tolerance("f", 2000.5), None);
    assert!(linear_tolerance("m", 2500.0).is_some());
    let mut tols = 0;
    for k in 0..400u64 {
        let t = truth_for("train", k);
        let cls = t["general_tolerance"].as_str().unwrap().to_string();
        for it in t["items"].as_array().unwrap() {
            if it["tolerance"].is_object() {
                tols += 1;
                assert!(
                    !(cls == "f" && it["value"].as_f64().unwrap() > 2000.0),
                    "class f tolerance on {}",
                    it["text"]
                );
            }
        }
    }
    assert!(tols > 100);
}

#[test]
fn val_and_test_parameter_sets_are_disjoint() {
    use nf_reader_data::run::{held_for, split_key};
    let counts = vec![
        ("train".to_string(), 0),
        ("val".to_string(), 120),
        ("test".to_string(), 120),
    ];
    let held = held_for(1, "test", &counts);
    let val = held_for(1, "train", &counts);
    for k in 0..120 {
        let (_, req, _) = request_for(1, "test", k, &held);
        assert!(!held.contains(&split_key(&req)));
        let (_, r, _) = request_for(1, "train", k, &val);
        assert!(!val.contains(&split_key(&r)), "train page {k} leaks");
    }
}
