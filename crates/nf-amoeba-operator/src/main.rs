//! `nf-operator`: run the AMOEBA operator on one order. Environment: NF_AGENT_USER, NF_AGENT_PASS (an agent
//! account), NF_NUMREAD_MODELS (the reader's models).

use nf_amoeba_agent::agent::Agent;
use nf_amoeba_agent::http::Tcp;
use nf_amoeba_agent::steps::StepLog;
use nf_amoeba_operator::eyes::NumreadEyes;
use nf_amoeba_operator::operator::{Manifest, Registry, RulePlanner, run};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: nf-operator --port <port> --steps <file.jsonl> --manifest <file.json>";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (mut port, mut steps, mut manifest) = (None, None, None);
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        match (a.as_str(), it.next()) {
            ("--port", Some(v)) => port = v.parse::<u16>().ok(),
            ("--steps", Some(v)) => steps = Some(PathBuf::from(v)),
            ("--manifest", Some(v)) => manifest = Some(PathBuf::from(v)),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(port), Some(steps), Some(manifest)) = (port, steps, manifest) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let (Ok(user), Ok(pass)) = (std::env::var("NF_AGENT_USER"), std::env::var("NF_AGENT_PASS")) else {
        eprintln!("nf-operator: set NF_AGENT_USER and NF_AGENT_PASS");
        return ExitCode::from(2);
    };
    let result = (|| -> Result<String, String> {
        let m: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let m = Manifest::from_json(&m)?;
        let log = StepLog::open(&steps).map_err(|e| e.to_string())?;
        let mut agent = Agent::new(Tcp { host: "127.0.0.1".into(), port }, log);
        agent.login(&user, &pass)?;
        let mut eyes = NumreadEyes::from_dir(&nf_numread::default_model_dir())?;
        let mut brain = RulePlanner::new(&m);
        let rep = run(&mut agent, &mut eyes, &mut brain, &Registry::order_tools(), &m)?;
        Ok(format!("{:?}: {} actions, {} events, {} errors", rep.status, rep.actions, rep.events, rep.errors.len()))
    })();
    match result {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nf-operator: {e}");
            ExitCode::FAILURE
        }
    }
}
