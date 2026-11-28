use nf_reader_data::run::{Opts, SPLITS, generate};
use std::path::PathBuf;

fn arg(a: &[String], k: &str) -> Option<String> {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .cloned()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let cmd = a.first().map(String::as_str).unwrap_or("");
    if cmd != "generate" && cmd != "sample" && cmd != "smoke" && cmd != "dataset" {
        eprintln!(
            "usage: nf-reader-data generate --out DIR [--seed N] [--train N] [--val N] [--test N] [--threads N] [--batch N]\n       nf-reader-data sample --out DIR [--seed N]   (20 pages)\n       nf-reader-data smoke --out DIR [--n N] [--seed N] [--threads N] [--mix default|weak1]   (v2 families, split smoke)"
        );
        std::process::exit(2);
    }
    let num = |k: &str, d: u64| arg(&a, k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let out = PathBuf::from(arg(&a, "--out").unwrap_or_else(|| "out".into()));
    if cmd == "smoke" {
        let mix = match arg(&a, "--mix") {
            None => nf_reader_data::v2::Mix::Default,
            Some(n) => nf_reader_data::v2::Mix::from_name(&n).unwrap_or_else(|| {
                eprintln!("unknown --mix {n} (default | weak1)");
                std::process::exit(2);
            }),
        };
        match nf_reader_data::v2::generate_smoke_mix(
            &out,
            num("--n", 200),
            num("--seed", 1),
            num("--threads", 4) as usize,
            mix,
        ) {
            Ok(n) => println!("smoke pages {n}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if cmd == "dataset" {
        // round-2 dataset: train 50/50 default+weak1, val default, test default and test weak1, parameter-disjoint
        let th = num("--threads", 3) as usize;
        let base = num("--seed", 20261128);
        let mut held = std::collections::HashSet::new();
        for (split, n, seed_off, weak) in [
            ("train", num("--train", 4000), 1u64, 2u64),
            ("val", num("--val", 300), 2, 0),
            ("test", num("--test", 300), 3, 0),
            ("test_weak1", num("--test", 300), 4, 1),
        ] {
            match nf_reader_data::v2::generate_split(
                &out,
                split,
                n,
                base + seed_off,
                th,
                weak,
                &held,
            ) {
                Ok(keys) => {
                    println!("{split}: {} pages", n);
                    held.extend(keys);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        return;
    }
    let counts: Vec<(String, u64)> = if cmd == "sample" {
        vec![("sample".into(), 20)]
    } else {
        SPLITS
            .iter()
            .map(|s| {
                (
                    s.to_string(),
                    num(&format!("--{s}"), if *s == "train" { 8000 } else { 1000 }),
                )
            })
            .collect()
    };
    let o = Opts {
        out,
        seed: num("--seed", 1),
        counts,
        threads: num("--threads", 3) as usize,
        batch: num("--batch", 240) as usize,
    };
    match generate(&o) {
        Ok(c) => println!("{c:?}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
