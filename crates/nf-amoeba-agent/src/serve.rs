//! The request loop: one JSON request per line in, one JSON answer per line out. A step record that cannot be written
//! stops the loop before its answer is written, so no answer leaves the bridge without its step.

use crate::agent::Agent;
use crate::http::Transport;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub fn serve<T: Transport, R: BufRead, W: Write>(
    agent: &mut Agent<T>,
    input: R,
    mut out: W,
) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<Value>(&line) {
            Err(_) => json!({ "id": null, "ok": false, "error": "request is not JSON" }),
            Ok(req) => {
                let id = req.get("id").cloned().unwrap_or(Value::Null);
                let tool = req["tool"].as_str().unwrap_or_default();
                let args = req.get("args").cloned().unwrap_or_else(|| json!({}));
                let a = agent.call(tool, &args).map_err(io::Error::other)?;
                let mut resp = json!({ "id": id, "ok": a.ok, "step": a.step });
                if let Some(r) = a.result {
                    resp["result"] = r;
                }
                if let Some(e) = a.error {
                    resp["error"] = json!(e);
                }
                resp
            }
        };
        writeln!(out, "{resp}")?;
        out.flush()?;
    }
    Ok(())
}
