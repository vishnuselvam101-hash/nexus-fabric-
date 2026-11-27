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
    if cmd != "generate" && cmd != "sample" {
        eprintln!(
            "usage: nf-reader-data generate --out DIR [--seed N] [--train N] [--val N] [--test N] [--threads N] [--batch N]\n       nf-reader-data sample --out DIR [--seed N]   (20 pages)"
        );
        std::process::exit(2);
    }
    let num = |k: &str, d: u64| arg(&a, k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let out = PathBuf::from(arg(&a, "--out").unwrap_or_else(|| "out".into()));
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
