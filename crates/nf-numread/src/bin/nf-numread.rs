//! nf-numread page.png > numbers.json   (use "-" to read the PNG from standard input)
//! nf-numread verify_box <page.png> --bbox x0,y0,x1,y1 --text TEXT --value N [--key D] [--source read]

use std::io::{Read, Write};
use std::process::ExitCode;

const USAGE: &str = "usage: nf-numread page.png > numbers.json\n       nf-numread verify_box <page.png> --bbox x0,y0,x1,y1 --text TEXT --value N [--key D] [--source read]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    if args.first().map(String::as_str) == Some("verify_box") {
        return verify_box(&args[1..]);
    }
    if args.len() != 1 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let bytes = match read_input(&args[0]) {
        Ok(b) => b,
        Err((code, msg)) => {
            eprintln!("nf-numread: {msg}");
            return ExitCode::from(code);
        }
    };
    let run = || -> Result<String, nf_numread::Error> {
        let page = nf_numread::image_in::decode_png(&bytes)?;
        let reader = nf_numread::Reader::new()?;
        let items = reader.read_gray(&page)?;
        Ok(nf_numread::to_json(&items, page.w, page.h))
    };
    match run() {
        Ok(json) => print_json(&json),
        Err(e) => {
            eprintln!("nf-numread: {e}");
            ExitCode::from(1)
        }
    }
}

fn read_input(path: &str) -> Result<Vec<u8>, (u8, String)> {
    if path == "-" {
        let mut b = Vec::new();
        std::io::stdin()
            .take(512 * 1024 * 1024)
            .read_to_end(&mut b)
            .map_err(|e| (1, format!("cannot read standard input: {e}")))?;
        Ok(b)
    } else {
        std::fs::read(path).map_err(|e| (1, format!("cannot read {path}: {e}")))
    }
}

fn print_json(json: &str) -> ExitCode {
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{json}").is_err() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Parse the flags of `verify_box`. Nothing here is a write path: the result is printed for a person.
fn verify_box(args: &[String]) -> ExitCode {
    let mut it = args.iter();
    let Some(path) = it.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let (mut bbox, mut text, mut value, mut key, mut source) =
        (None, None, None, "D".to_string(), "read".to_string());
    while let Some(flag) = it.next() {
        let mut next = || -> Option<String> { it.next().cloned() };
        match flag.as_str() {
            "--bbox" => {
                let Some(v) = next() else {
                    eprintln!("nf-numread: --bbox needs four numbers\n{USAGE}");
                    return ExitCode::from(2);
                };
                let p: Result<Vec<f32>, _> = v.split(',').map(|s| s.trim().parse()).collect();
                let a: [f32; 4] = match p
                    .ok()
                    .and_then(|p| p.try_into().ok())
                    .filter(|a: &[f32; 4]| a.iter().all(|n| n.is_finite()))
                {
                    Some(a) => a,
                    None => {
                        eprintln!("nf-numread: --bbox must be x0,y0,x1,y1 in page pixels");
                        return ExitCode::from(2);
                    }
                };
                bbox = Some(a);
            }
            "--text" => text = next(),
            "--value" => value = next().and_then(|s| s.parse().ok()),
            "--key" => key = next().unwrap_or(key),
            "--source" => source = next().unwrap_or(source),
            _ => {
                eprintln!("nf-numread: unknown argument {flag}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(bbox), Some(text), Some(value)) = (bbox, text, value) else {
        eprintln!("nf-numread: --bbox, --text and --value are required\n{USAGE}");
        return ExitCode::from(2);
    };
    let bytes = match read_input(path) {
        Ok(b) => b,
        Err((code, msg)) => {
            eprintln!("nf-numread: {msg}");
            return ExitCode::from(code);
        }
    };
    let run = || -> Result<String, nf_numread::Error> {
        let reader = nf_numread::Reader::new()?;
        let v = reader.verify_box(&bytes, bbox, &key, &text, value, &source)?;
        Ok(v.to_json())
    };
    match run() {
        Ok(json) => print_json(&json),
        Err(e) => {
            eprintln!("nf-numread: {e}");
            ExitCode::from(1)
        }
    }
}
