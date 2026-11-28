//! Replay: the state an event stream leads to. The values a stream typed (the last one per part and key,
//! not undone) must equal what the order holds; a stream that cannot reproduce the state is incomplete.

use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Default, PartialEq)]
pub struct Replayed {
    /// (part no, key) -> value typed last and not undone
    pub values: BTreeMap<(String, String), f64>,
    /// the questions asked, in order (step ids)
    pub asked: Vec<String>,
}

pub fn replay(events: &[Value]) -> Replayed {
    let mut out = Replayed::default();
    for e in events {
        if e["undone"].as_bool().unwrap_or(false) {
            continue;
        }
        if e["action"] == "ask" {
            out.asked
                .push(e["step"].as_str().unwrap_or_default().to_string());
        }
        if let (Some(p), Some(k), Some(v)) = (
            e["write_ref"]["part"].as_str(),
            e["write_ref"]["key"].as_str(),
            e["value"].as_f64(),
        ) {
            out.values.insert((p.to_string(), k.to_string()), v);
        }
    }
    out
}
