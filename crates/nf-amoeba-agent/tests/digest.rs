//! Only a hold (HTTP 409) is reported as `held: true`. Any other server refusal of a DXF request is a rejection,
//! and the server's body is passed on unchanged. Uses a canned transport: no server is started.

use nf_amoeba_agent::agent::Agent;
use nf_amoeba_agent::http::{Reply, Request, Transport};
use nf_amoeba_agent::steps::StepLog;
use serde_json::{Value, json};
use std::path::PathBuf;

struct Canned {
    status: u16,
    body: &'static str,
}

impl Transport for Canned {
    fn send(&mut self, _req: &Request) -> Result<Reply, String> {
        Ok(Reply {
            status: self.status,
            set_cookie: None,
            body: self.body.as_bytes().to_vec(),
        })
    }
}

fn digest_of(status: u16, body: &'static str, tag: &str) -> Value {
    let path: PathBuf = std::env::temp_dir().join(format!(
        "nf-amb5t-digest-{tag}-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let log = StepLog::open(&path).expect("steps file");
    let mut agent = Agent::new(Canned { status, body }, log);
    let answer = agent
        .call("get_dxf_digest", &json!({ "job": 1, "part": "reducer" }))
        .expect("the step is written");
    let _ = std::fs::remove_file(&path);
    assert!(answer.ok, "{:?}", answer.error);
    answer.result.expect("a result")
}

#[test]
fn a_hold_is_reported_as_held_with_the_server_body() {
    let r = digest_of(409, r#"{"error":"held","code":"G1","checks":[]}"#, "hold");
    assert_eq!(r["held"], true, "{r}");
    assert_eq!(r["hold"]["code"], "G1");
}

#[test]
fn other_refusals_are_rejections_not_holds() {
    let r = digest_of(404, r#"{"error":"no DXF for this part"}"#, "refused");
    assert!(r.get("held").is_none(), "a 404 is not a hold: {r}");
    assert_eq!(r["http_status"], 404);
    assert_eq!(r["rejected"]["error"], "no DXF for this part");
}
