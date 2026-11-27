//! nf-numread page.png > numbers.json   (use "-" to read the PNG from standard input)

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 || args[0] == "--help" || args[0] == "-h" {
        eprintln!("usage: nf-numread page.png > numbers.json\n       (reads the PNG from stdin when the argument is -)");
        return ExitCode::from(2);
    }
    let bytes = if args[0] == "-" {
        let mut b = Vec::new();
        if let Err(e) = std::io::stdin().take(512 * 1024 * 1024).read_to_end(&mut b) {
            eprintln!("nf-numread: cannot read standard input: {e}");
            return ExitCode::from(1);
        }
        b
    } else {
        match std::fs::read(&args[0]) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("nf-numread: cannot read {}: {e}", args[0]);
                return ExitCode::from(1);
            }
        }
    };
    let run = || -> Result<String, nf_numread::Error> {
        let page = nf_numread::image_in::decode_png(&bytes)?;
        let reader = nf_numread::Reader::new()?;
        let items = reader.read_gray(&page)?;
        Ok(nf_numread::to_json(&items, page.w, page.h))
    };
    match run() {
        Ok(json) => {
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{json}").is_err() {
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nf-numread: {e}");
            ExitCode::from(1)
        }
    }
}
