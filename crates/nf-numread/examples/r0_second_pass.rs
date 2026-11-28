//! PZ-2: report what a second read adds on held-out R0 pages (no truth, no tuning).
//!
//! For each first-pass box we verify the printed value (a false flag would be a regression) and
//! a wrong value taken from another box on the same page (a flag is a gain). The report is a
//! measurement of agreement, not a claim about accuracy on a ground-truth set.

use nf_numread::{image_in, Reader};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: r0_second_pass <page.png> ...");
        std::process::exit(2);
    }
    let reader = match Reader::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("models: {e}");
            std::process::exit(1);
        }
    };
    let (mut right, mut right_ok, mut wrong, mut wrong_flagged) = (0u32, 0u32, 0u32, 0u32);
    for path in paths {
        let png = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{path}: {e}");
                continue;
            }
        };
        let page = match image_in::decode_png(&png) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{path}: {e}");
                continue;
            }
        };
        let items = match reader.read_gray(&page) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("{path}: {e}");
                continue;
            }
        };
        let sample: Vec<_> = items.iter().filter(|i| i.value.is_some()).take(5).collect();
        for (n, item) in sample.iter().enumerate() {
            let value = item.value.unwrap_or_default();
            let ok = reader
                .verify_box(&png, item.bbox, "D", &item.text, value, "read")
                .expect("verify");
            let ok_verdict = ok.check.verdict.code();
            right += 1;
            if ok_verdict == "ok" {
                right_ok += 1;
            }
            // A different number from the same page is an injected wrong value, not truth.
            let other = sample
                .get((n + 1) % sample.len())
                .and_then(|i| i.value)
                .unwrap_or(value + 1.0);
            let bad = reader
                .verify_box(&png, item.bbox, "D", &item.text, other, "read")
                .expect("verify");
            let bad_verdict = bad.check.verdict.code();
            wrong += 1;
            if bad_verdict != "ok" {
                wrong_flagged += 1;
            }
            println!(
                "{{\"page\": {:?}, \"text\": {:?}, \"bbox\": {:?}, \"right\": {:?}, \"injected\": {:?}}}",
                path, item.text, item.bbox, ok_verdict, bad_verdict
            );
        }
    }
    let rate = |n: u32, d: u32| -> f64 {
        if d == 0 {
            0.0
        } else {
            f64::from(n) * 100.0 / f64::from(d)
        }
    };
    println!(
        "RESULT PZ-2 R0 held-out second pass: right_verdict={:.3}% ({right_ok}/{right}), injected_wrong_flagged={:.3}% ({wrong_flagged}/{wrong}), net_gain={:.3} pp [EST, no truth labels; not tuned]",
        rate(right_ok, right),
        rate(wrong_flagged, wrong),
        rate(wrong_flagged, wrong) - (100.0 - rate(right_ok, right))
    );
}
