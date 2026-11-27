//! `nf-amoeba-tools`: JSON lines on stdin and stdout, one request per line.
//! Request: {"id", "tool", "args"}. Response: {"id", "ok", "result" | "error", "step"}.

use nf_amoeba_agent::agent::Agent;
use nf_amoeba_agent::http::Tcp;
use nf_amoeba_agent::serve::serve;
use nf_amoeba_agent::steps::StepLog;
use nf_amoeba_agent::tools;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str =
    "usage: nf-amoeba-tools --base http://127.0.0.1:<port> --steps <file.jsonl> --docs-dir <dir>
       nf-amoeba-tools --schema
environment: NF_AGENT_USER and NF_AGENT_PASS (an engineer account; never printed)";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.len() == 1 && argv[0] == "--schema" {
        println!(
            "{}",
            serde_json::to_string_pretty(&tools::schema_document()).unwrap_or_default()
        );
        return ExitCode::SUCCESS;
    }
    if argv.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (mut base, mut steps, mut docs) = (None, None, None);
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        match (a.as_str(), it.next()) {
            ("--base", Some(v)) => base = Some(v.clone()),
            ("--steps", Some(v)) => steps = Some(PathBuf::from(v)),
            ("--docs-dir", Some(v)) => docs = Some(PathBuf::from(v)),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(base), Some(steps), Some(docs)) = (base, steps, docs) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let (host, port) = match parse_base(&base) {
        Ok(hp) => hp,
        Err(e) => {
            eprintln!("nf-amoeba-tools: {e}");
            return ExitCode::from(2);
        }
    };
    let (user, pass) = match (
        std::env::var("NF_AGENT_USER"),
        std::env::var("NF_AGENT_PASS"),
    ) {
        (Ok(u), Ok(p)) if !u.is_empty() && !p.is_empty() => (u, p),
        _ => {
            eprintln!("nf-amoeba-tools: set NF_AGENT_USER and NF_AGENT_PASS");
            return ExitCode::from(2);
        }
    };
    let log = match StepLog::open(&steps) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("nf-amoeba-tools: cannot open the steps file: {e}");
            return ExitCode::from(2);
        }
    };
    let mut agent = match Agent::new(Tcp { host, port }, log).with_docs_dir(&docs) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("nf-amoeba-tools: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = agent.login(&user, &pass) {
        eprintln!("nf-amoeba-tools: {e}");
        return ExitCode::FAILURE;
    }
    let stdout = io::stdout();
    match serve(&mut agent, io::stdin().lock(), stdout.lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nf-amoeba-tools: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Accepts only a loopback address: the bridge never talks to another machine.
fn parse_base(base: &str) -> Result<(String, u16), String> {
    let rest = base
        .strip_prefix("http://")
        .ok_or("--base must start with http://")?;
    let (host, port) = rest
        .rsplit_once(':')
        .ok_or("--base must be http://127.0.0.1:<port>")?;
    if host != "127.0.0.1" && host != "localhost" {
        return Err("--base must be a loopback address (127.0.0.1 or localhost)".into());
    }
    let port: u16 = port
        .parse()
        .map_err(|_| "--base: the port is not a number")?;
    Ok((host.to_string(), port))
}
