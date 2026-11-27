//! The steps file: one JSON line per call, append-only evidence. Secrets are redacted before writing.

use serde_json::{Map, Value};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

pub struct StepLog {
    file: File,
    count: u64,
}

impl StepLog {
    /// Opens (creating it if needed) the steps file for appending. Existing lines are counted so step numbers continue.
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut existing = Vec::new();
        if path.exists() {
            File::open(path)?.read_to_end(&mut existing)?;
        }
        let count = existing.iter().filter(|b| **b == b'\n').count() as u64;
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file, count })
    }

    /// Appends one record and returns its step number.
    pub fn append(&mut self, mut record: Map<String, Value>) -> io::Result<u64> {
        let step = self.count + 1;
        record.insert("step".into(), Value::from(step));
        let mut line = serde_json::to_string(&Value::Object(record)).map_err(io::Error::other)?;
        line.push('\n');
        self.file.write_all(line.as_bytes())?;
        self.file.sync_data()?;
        self.count = step;
        Ok(step)
    }
}

/// Replaces the value of every key that may hold a secret, at any depth.
pub fn redact(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, val)| {
                    let l = k.to_ascii_lowercase();
                    let secret = [
                        "password",
                        "pass",
                        "secret",
                        "token",
                        "cookie",
                        "setup",
                        "authorization",
                    ]
                    .iter()
                    .any(|s| l.contains(s));
                    let out = if secret {
                        Value::from("[redacted]")
                    } else {
                        redact(val)
                    };
                    (k.clone(), out)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(redact).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secrets_are_redacted_at_any_depth() {
        let v = json!({"username":"agent","password":"p1","nested":{"setup_token":"t","list":[{"x_secret":"s"}]},"id":3});
        let r = redact(&v);
        assert_eq!(r["username"], "agent");
        assert_eq!(r["password"], "[redacted]");
        assert_eq!(r["nested"]["setup_token"], "[redacted]");
        assert_eq!(r["nested"]["list"][0]["x_secret"], "[redacted]");
        assert_eq!(r["id"], 3);
    }

    #[test]
    fn steps_continue_numbering_across_opens() {
        let dir = std::env::temp_dir().join(format!("nf-steps-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("steps.jsonl");
        let _ = std::fs::remove_file(&p);
        let mut a = StepLog::open(&p).unwrap();
        assert_eq!(a.append(Map::new()).unwrap(), 1);
        drop(a);
        let mut b = StepLog::open(&p).unwrap();
        assert_eq!(b.append(Map::new()).unwrap(), 2);
        let text = std::fs::read_to_string(&p).unwrap();
        assert_eq!(text.lines().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
