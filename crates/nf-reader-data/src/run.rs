//! Page plan (stratified, deterministic from a seed), page build (layout + truth), and the batched run.

use crate::compose::{Ids, MATERIALS, compose};
use crate::layout::{B, Lang, Page, Style};
use crate::params::{FAMILIES, sample};
use crate::render::{Fx, render_pdf, render_png, svg};
use crate::rng::{Rng, mix};
use nf_core::job::{JobOutput, Request};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "nf-reader-data/1";
pub const SPLITS: [&str; 3] = ["train", "val", "test"];

#[derive(Clone, Debug)]
pub struct Spec {
    pub split: String,
    pub k: u64,
    pub fam: &'static str,
    pub lang: Lang,
    pub style: Style,
    pub scan: bool,
    pub seed: u64,
}

pub fn split_id(s: &str) -> u64 {
    match s {
        "train" => 1,
        "val" => 2,
        "test" => 3,
        _ => 9,
    }
}

/// Stratified cell of page `k`: 5 families x 2 languages x 2 styles x clean/scan = 40 cells, cycled.
pub fn spec_for(base: u64, split: &str, k: u64, attempt: u64) -> Spec {
    let c = if split == "sample" {
        (2 * k) % 40
    } else {
        k % 40
    };
    Spec {
        split: split.to_string(),
        k,
        fam: FAMILIES[(c % 5) as usize],
        lang: if (c / 5) % 2 == 0 { Lang::En } else { Lang::Ja },
        style: if (c / 10) % 2 == 0 {
            Style::Iso
        } else {
            Style::Jis
        },
        scan: (c / 20) % 2 == 1,
        seed: mix(&[base, split_id(split), k, attempt]),
    }
}

pub fn page_id(s: &Spec) -> String {
    format!("{}-{:05}", s.split, s.k)
}

pub struct Built {
    pub page: Page,
    pub out: JobOutput,
    pub w: u32,
    pub h: u32,
    pub fx: Option<Fx>,
    pub gtol: char,
    pub truth: Value,
}

fn rot(b: B, th: f64, cx: f64, cy: f64) -> B {
    let (s, c) = th.to_radians().sin_cos();
    let mut r = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for (x, y) in [(b[0], b[1]), (b[2], b[1]), (b[2], b[3]), (b[0], b[3])] {
        let (dx, dy) = (x - cx, y - cy);
        let (nx, ny) = (cx + dx * c - dy * s, cy + dx * s + dy * c);
        r[0] = r[0].min(nx);
        r[1] = r[1].min(ny);
        r[2] = r[2].max(nx);
        r[3] = r[3].max(ny);
    }
    r
}
fn r1(b: B) -> Value {
    json!([
        (b[0] * 10.0).round() / 10.0,
        (b[1] * 10.0).round() / 10.0,
        (b[2] * 10.0).round() / 10.0,
        (b[3] * 10.0).round() / 10.0
    ])
}

fn fam_code(f: &str) -> &'static str {
    match f {
        "cylinder_shell" => "CS",
        "cone_frustum" => "CF",
        "eccentric_reducer" => "ER",
        "branch_stub" => "BS",
        _ => "HB",
    }
}

/// Request for a page; train pages avoid parameter sets used by val/test (`held`).
pub fn request_for(
    base: u64,
    split: &str,
    k: u64,
    held: &HashSet<String>,
) -> (Spec, Request, JobOutput) {
    for attempt in 0..1000 {
        let spec = spec_for(base, split, k, attempt);
        let mut rp = Rng::new(mix(&[spec.seed, 1]));
        let (req, out) = sample(spec.fam, &mut rp);
        let key = serde_json::to_string(&req).unwrap_or_default();
        if split == "train" && held.contains(&key) {
            continue;
        }
        return (spec, req, out);
    }
    panic!("no free parameter set");
}

pub fn build(spec: &Spec, req: &Request, out: JobOutput) -> Built {
    let _ = req;
    let mut rl = Rng::new(mix(&[spec.seed, 2]));
    let mut rfx = Rng::new(mix(&[spec.seed, 3]));
    let w = rl.int(1600, 2400);
    let h = (w as f64 / std::f64::consts::SQRT_2).round() as i64;
    let dec = if rl.chance(0.5) { 1 } else { 2 };
    let gtol = *rl.pick(&['f', 'm', 'm', 'c']);
    let mut page = Page::new(w as f64, h as f64, spec.style, spec.lang, dec, gtol);
    let ids = Ids {
        drawing_no: format!("NFR-{}-{:08X}", fam_code(spec.fam), spec.seed & 0xFFFF_FFFF),
        date: format!("2026-{:02}-{:02}", rl.int(1, 12), rl.int(1, 28)),
        material: MATERIALS[rl.int(0, 4) as usize],
    };
    compose(&mut page, &mut rl, &out, spec.fam, &ids);
    let fx = spec.scan.then(|| {
        let mut skew = rfx.range(0.3, 3.0);
        if rfx.chance(0.5) {
            skew = -skew;
        }
        let low = rfx.chance(0.4);
        Fx {
            skew,
            blur: if rfx.chance(0.6) {
                *rfx.pick(&[0.6, 1.0, 1.4])
            } else {
                0.0
            },
            noise: *rfx.pick(&[0.6, 1.0, 1.6]),
            lo: if low { rfx.int(15, 35) as u32 } else { 0 },
            hi: if low { rfx.int(65, 85) as u32 } else { 100 },
            jpeg: rfx.int(25, 70) as u32,
            seed: rfx.int(1, 1_000_000) as u64,
        }
    });
    let skew = fx.as_ref().map(|f| f.skew).unwrap_or(0.0);
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let pid = page_id(spec);
    let part_id = format!("{}-{}", fam_code(spec.fam), ids.drawing_no);
    let req_json = serde_json::to_value(&out.request).unwrap_or(Value::Null);
    let items: Vec<Value> = page
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            json!({
                "id": format!("{pid}-{i:03}"),
                "type": it.kind,
                "text": it.text,
                "value": it.value,
                "unit": it.unit,
                "tolerance": it.tol,
                "tolerance_source": it.tol_source,
                "source": it.src,
                "decimals": it.decimals,
                "box": r1(rot(it.b, skew, cx, cy)),
                "box_page": r1(it.b),
                "page": 0,
                "language": spec.lang.name(),
                "style": spec.style.name(),
                "family": spec.fam,
                "part": it.part,
                "part_id": part_id,
                "seed": spec.seed
            })
        })
        .collect();
    let truth = json!({
        "schema": SCHEMA,
        "page_id": pid,
        "split": spec.split,
        "family": spec.fam,
        "part_id": part_id,
        "seed": spec.seed,
        "language": spec.lang.name(),
        "style": spec.style.name(),
        "scan": spec.scan,
        "scan_effects": fx.as_ref().map(|f| json!({"skew_deg": (f.skew * 1000.0).round() / 1000.0, "blur_sigma": f.blur, "noise": f.noise,
            "level_lo_pct": f.lo, "level_hi_pct": f.hi, "jpeg_quality": f.jpeg, "noise_seed": f.seed})),
        "width": w, "height": h,
        "decimals": dec,
        "general_tolerance": gtol.to_string(),
        "png": format!("{}/png/{pid}.png", spec.split),
        "pdf": format!("{}/pdf/{pid}.pdf", spec.split),
        "engine": out.engine,
        "request": req_json,
        "items": items
    });
    Built {
        page,
        out,
        w: w as u32,
        h: h as u32,
        fx,
        gtol,
        truth,
    }
}

pub fn write_page(root: &Path, spec: &Spec, b: &Built) -> Result<(), String> {
    let pid = page_id(spec);
    let png = root
        .join(&spec.split)
        .join("png")
        .join(format!("{pid}.png"));
    let pdf = root
        .join(&spec.split)
        .join("pdf")
        .join(format!("{pid}.pdf"));
    let paper = if spec.scan { "#f3f1ea" } else { "#ffffff" };
    let skew = b.fx.as_ref().map(|f| f.skew).unwrap_or(0.0);
    render_png(&svg(&b.page, skew, paper), b.w, b.h, &png, b.fx.as_ref())?;
    render_pdf(&svg(&b.page, 0.0, "#ffffff"), &pdf)
}

pub struct Opts {
    pub out: PathBuf,
    pub seed: u64,
    pub counts: Vec<(String, u64)>,
    pub threads: usize,
    pub batch: usize,
}

pub fn held_set(base: u64) -> HashSet<String> {
    let mut s = HashSet::new();
    for (sp, n) in [("val", 1000u64), ("test", 1000u64)] {
        for k in 0..n {
            let spec = spec_for(base, sp, k, 0);
            let mut rp = Rng::new(mix(&[spec.seed, 1]));
            let (req, _) = sample(spec.fam, &mut rp);
            s.insert(serde_json::to_string(&req).unwrap_or_default());
        }
    }
    s
}

pub fn generate(o: &Opts) -> Result<BTreeMap<String, u64>, String> {
    let held = held_set(o.seed);
    let t0 = std::time::Instant::now();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for (split, n) in &o.counts {
        std::fs::create_dir_all(o.out.join(split).join("png")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(o.out.join(split).join("pdf")).map_err(|e| e.to_string())?;
        let mut truth = std::io::BufWriter::new(
            std::fs::File::create(o.out.join(split).join("truth.jsonl"))
                .map_err(|e| e.to_string())?,
        );
        let mut k0 = 0u64;
        while k0 < *n {
            let k1 = (k0 + o.batch as u64).min(*n);
            let ks: Vec<u64> = (k0..k1).collect();
            let mut lines: Vec<(u64, Result<String, String>)> = std::thread::scope(|sc| {
                let hs: Vec<_> = (0..o.threads)
                    .map(|t| {
                        let ks = &ks;
                        let held = &held;
                        sc.spawn(move || {
                            let mut v = vec![];
                            for &k in ks.iter().skip(t).step_by(o.threads) {
                                let (spec, req, out) = request_for(o.seed, split, k, held);
                                let b = build(&spec, &req, out);
                                let r = write_page(&o.out, &spec, &b).map(|_| b.truth.to_string());
                                v.push((k, r));
                            }
                            v
                        })
                    })
                    .collect();
                hs.into_iter()
                    .flat_map(|h| h.join().unwrap_or_default())
                    .collect()
            });
            lines.sort_by_key(|x| x.0);
            use std::io::Write;
            for (k, l) in lines {
                let l = l.map_err(|e| format!("{split} page {k}: {e}"))?;
                writeln!(truth, "{l}").map_err(|e| e.to_string())?;
                *counts.entry(split.clone()).or_default() += 1;
            }
            truth.flush().map_err(|e| e.to_string())?;
            k0 = k1;
            eprintln!("[{:>7.1}s] {split} {k0}/{n}", t0.elapsed().as_secs_f64());
        }
    }
    Ok(counts)
}
