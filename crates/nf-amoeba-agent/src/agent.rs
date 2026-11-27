//! The bridge: one session against the local NEXUS FABRIC server. Every tool is one or more calls to the server's own
//! API; nothing is computed here except hashes and the candidate record. Every call is written to the steps file.

use crate::http::{Reply, Request, Transport};
use crate::schema::validate_value;
use crate::steps::{StepLog, redact};
use crate::tools::{self, FAMILIES};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The answer to one tool call, as written to stdout.
pub struct Answer {
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub step: u64,
}

pub struct Agent<T: Transport> {
    transport: T,
    cookie: Option<String>,
    steps: StepLog,
    http_calls: u64,
    // scratch state of the call in progress
    statuses: Vec<u16>,
    engine: Option<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

impl<T: Transport> Agent<T> {
    pub fn new(transport: T, steps: StepLog) -> Self {
        Self {
            transport,
            cookie: None,
            steps,
            http_calls: 0,
            statuses: Vec::new(),
            engine: None,
        }
    }

    /// Number of HTTP requests sent so far (login included).
    pub fn http_calls(&self) -> u64 {
        self.http_calls
    }

    /// Signs in with the dedicated account. The password is sent once and is never recorded or printed.
    pub fn login(&mut self, username: &str, password: &str) -> Result<(), String> {
        let started = Instant::now();
        self.statuses.clear();
        let body = json!({ "username": username, "password": password }).to_string();
        let outcome = match self.raw("POST", "/api/login", json_headers(), body.into_bytes()) {
            Ok(r) if r.status == 200 && r.set_cookie.is_some() => {
                self.cookie = r.set_cookie;
                Ok(())
            }
            Ok(r) => Err(format!("login refused (HTTP {})", r.status)),
            Err(e) => Err(e),
        };
        let args = json!({ "username": username, "password": "[redacted]" });
        self.record(
            "login",
            &args,
            if outcome.is_ok() { "ok" } else { "error" },
            None,
            outcome.as_ref().err(),
            started,
        )
        .map_err(|e| format!("cannot write the steps file: {e}"))?;
        outcome
    }

    /// Runs one tool call. A name that is not an allowed tool is refused before anything is sent.
    pub fn call(&mut self, tool: &str, args: &Value) -> Answer {
        let started = Instant::now();
        self.statuses.clear();
        self.engine = None;
        let refused = !tools::is_allowed(tool);
        let outcome: Result<Value, String> = if refused {
            Err("tool not available".to_string())
        } else {
            self.dispatch(tool, args)
        };
        let status = match (&outcome, refused) {
            (_, true) => "refused",
            (Err(_), _) => "error",
            (Ok(_), _) if tool == "propose_read" => "CANDIDATE",
            (Ok(_), _) => "ok",
        };
        let error = outcome.as_ref().err().cloned();
        let result = outcome.ok();
        match self.record(tool, args, status, result.as_ref(), error.as_ref(), started) {
            Ok(step) => Answer {
                ok: result.is_some(),
                result,
                error,
                step,
            },
            Err(e) => Answer {
                ok: false,
                result: None,
                error: Some(format!("cannot write the steps file: {e}")),
                step: 0,
            },
        }
    }

    fn dispatch(&mut self, tool: &str, a: &Value) -> Result<Value, String> {
        let def = tools::definition(tool).ok_or("tool not available")?;
        validate_value(&def["parameters"], a, tool)?;
        match tool {
            "list_projects" => self.get_answer("/api/projects"),
            "get_project" => {
                let p = int(a, "project")?;
                self.get_answer(&format!("/api/projects/{p}"))
            }
            "upload_document" => self.upload(a),
            "pdf_to_dxf" => self.pdf_to_dxf(a),
            "get_conversion" => {
                let c = int(a, "conversion")?;
                self.get_answer(&format!("/api/pdf-conversions/{c}"))
            }
            "calculate" => self.calculate(a),
            "get_job" => {
                let j = int(a, "job")?;
                let v = self.get_answer(&format!("/api/calculations/{j}"))?;
                self.note_engine(&v);
                Ok(v)
            }
            "get_amoeba" => {
                let j = int(a, "job")?;
                let v = self.get_answer(&format!("/api/calculations/{j}/amoeba"))?;
                self.note_engine(&v);
                Ok(v)
            }
            "get_dependencies" => {
                let j = int(a, "job")?;
                self.get_answer(&format!("/api/calculations/{j}/dependencies"))
            }
            "replay" => {
                let j = int(a, "job")?;
                let v = self.post_answer(&format!("/api/calculations/{j}/replay"), None)?;
                self.note_engine(&v);
                Ok(v)
            }
            "recompute" => {
                let j = int(a, "job")?;
                let v = self.post_answer(&format!("/api/calculations/{j}/recompute"), None)?;
                self.note_engine(&v);
                Ok(v)
            }
            "list_obligations" => self.get_answer("/api/obligations"),
            "list_lessons" => self.get_answer("/api/lessons"),
            "amoeba_summary" => self.get_answer("/api/amoeba/summary"),
            "get_dxf_digest" => self.dxf_digest(a),
            "propose_read" => propose_read(a),
            _ => Err("tool not available".into()),
        }
    }

    fn upload(&mut self, a: &Value) -> Result<Value, String> {
        let project = int(a, "project")?;
        let path = a["path"].as_str().unwrap_or_default();
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read the file: {e}"))?;
        if !bytes.starts_with(b"%PDF-") {
            return Err(
                "not a PDF: the content does not start with %PDF-; nothing was sent".into(),
            );
        }
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| n.chars().all(|c| c.is_ascii_graphic() || c == ' '))
            .ok_or("the file name must be plain ASCII")?
            .to_string();
        let headers = vec![
            (
                "Content-Type".to_string(),
                "application/octet-stream".to_string(),
            ),
            ("X-Filename".to_string(), name.clone()),
        ];
        let r = self.raw(
            "POST",
            &format!("/api/projects/{project}/documents"),
            headers,
            bytes,
        )?;
        let mut v = answer(&r)?;
        let Some(sha) = v["sha256"].as_str().map(str::to_string) else {
            return Ok(v);
        };
        // the upload answer carries no id: find the document of this content in the project
        let p = self.get_answer(&format!("/api/projects/{project}"))?;
        let doc = p["documents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["sha256"].as_str() == Some(sha.as_str()))
            .filter_map(|d| d["id"].as_i64())
            .max();
        v["document"] = json!(doc);
        v["project"] = json!(project);
        Ok(v)
    }

    fn pdf_to_dxf(&mut self, a: &Value) -> Result<Value, String> {
        let doc = int(a, "document")?;
        let mut body = Map::new();
        body.insert("page".into(), json!(1));
        body.insert("scale_kind".into(), json!("ratio"));
        body.insert("scale_value".into(), a["scale_n"].clone());
        if let Some(opts) = a.get("options").and_then(Value::as_object) {
            for (k, v) in opts {
                body.insert(k.clone(), v.clone());
            }
        }
        let bytes = Value::Object(body).to_string().into_bytes();
        let r = self.raw(
            "POST",
            &format!("/api/documents/{doc}/pdf-to-dxf"),
            json_headers(),
            bytes,
        )?;
        answer(&r)
    }

    fn calculate(&mut self, a: &Value) -> Result<Value, String> {
        let project = int(a, "project")?;
        let family = a["family"].as_str().unwrap_or_default();
        if !FAMILIES.contains(&family) {
            return Err("unknown family".into());
        }
        let params = a["params"]
            .as_object()
            .ok_or("'params' must be an object")?;
        if params.contains_key("family") {
            return Err("'family' is given once, as the family argument".into());
        }
        // the parameters are exactly those the model gave; nothing is filled in here
        let mut body = Map::new();
        body.insert("family".into(), json!(family));
        for (k, v) in params {
            body.insert(k.clone(), v.clone());
        }
        let bytes = Value::Object(body).to_string().into_bytes();
        let r = self.raw(
            "POST",
            &format!("/api/projects/{project}/calculations"),
            json_headers(),
            bytes,
        )?;
        answer(&r)
    }

    /// Downloads a part's DXF only to measure it. A server refusal (a hold) is returned as the server's body, unchanged.
    fn dxf_digest(&mut self, a: &Value) -> Result<Value, String> {
        let job = int(a, "job")?;
        let part = a["part"].as_str().unwrap_or_default();
        if part.is_empty()
            || part.len() > 32
            || !part.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        {
            return Err(
                "'part' must be lower-case letters and underscores, 1 to 32 characters".into(),
            );
        }
        let r = self.get_raw(&format!("/api/calculations/{job}/dxf/{part}"))?;
        match r.status {
            200..=299 => Ok(json!({
                "job": job, "part": part, "held": false,
                "bytes": r.body.len(), "sha256": sha256_hex(&r.body),
            })),
            401 | 403 => Err(server_error(r.status, &r.body)),
            400..=499 => Ok(json!({
                "job": job, "part": part, "held": true,
                "http_status": r.status, "hold": parse_body(&r.body),
            })),
            s => Err(server_error(s, &r.body)),
        }
    }

    fn note_engine(&mut self, v: &Value) {
        self.engine = v["engine"].as_str().map(str::to_string);
    }

    fn get_answer(&mut self, path: &str) -> Result<Value, String> {
        let r = self.get_raw(path)?;
        answer(&r)
    }

    fn post_answer(&mut self, path: &str, body: Option<Value>) -> Result<Value, String> {
        let bytes = body
            .map(|b| b.to_string().into_bytes())
            .unwrap_or_else(|| b"{}".to_vec());
        let r = self.raw("POST", path, json_headers(), bytes)?;
        answer(&r)
    }

    fn get_raw(&mut self, path: &str) -> Result<Reply, String> {
        self.raw("GET", path, Vec::new(), Vec::new())
    }

    fn raw(
        &mut self,
        method: &'static str,
        path: &str,
        mut headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<Reply, String> {
        if let Some(c) = &self.cookie {
            headers.push(("Cookie".into(), c.clone()));
        }
        self.http_calls += 1;
        let reply = self.transport.send(&Request {
            method,
            path: path.to_string(),
            headers,
            body,
        })?;
        self.statuses.push(reply.status);
        Ok(reply)
    }

    fn record(
        &mut self,
        tool: &str,
        args: &Value,
        status: &str,
        result: Option<&Value>,
        error: Option<&String>,
        started: Instant,
    ) -> std::io::Result<u64> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut rec = Map::new();
        rec.insert("ts_ms".into(), json!(ts));
        rec.insert("tool".into(), json!(tool));
        rec.insert("args".into(), redact(args));
        rec.insert("status".into(), json!(status));
        rec.insert("http_status".into(), json!(self.statuses));
        rec.insert(
            "result_sha256".into(),
            match result {
                Some(v) => json!(sha256_hex(
                    &serde_json::to_vec(&without_clock(v)).unwrap_or_default()
                )),
                None => Value::Null,
            },
        );
        rec.insert(
            "duration_ms".into(),
            json!(started.elapsed().as_millis() as u64),
        );
        rec.insert("engine_id".into(), json!(self.engine));
        rec.insert("error".into(), json!(error));
        self.steps.append(rec)
    }
}

/// The result without wall-clock fields (`at`, `ts`, and any key ending in `_at`), so that the same job gives the same
/// hash on every run. The answer printed to the model keeps every field.
pub fn without_clock(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| !(k.as_str() == "at" || k.as_str() == "ts" || k.ends_with("_at")))
                .map(|(k, x)| (k.clone(), without_clock(x)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(without_clock).collect()),
        other => other.clone(),
    }
}

fn propose_read(a: &Value) -> Result<Value, String> {
    let box_ = a["box"].as_array().ok_or("'box' must be four numbers")?;
    if box_.iter().any(|n| !n.as_f64().is_some_and(f64::is_finite)) {
        return Err("'box' must be four finite numbers".into());
    }
    let mut out = a.as_object().cloned().unwrap_or_default();
    out.insert("status".into(), json!("CANDIDATE"));
    Ok(Value::Object(out))
}

fn int(a: &Value, key: &str) -> Result<i64, String> {
    a[key]
        .as_i64()
        .filter(|n| *n >= 1)
        .ok_or_else(|| format!("'{key}' must be a whole number from 1"))
}

fn json_headers() -> Vec<(String, String)> {
    vec![("Content-Type".into(), "application/json".into())]
}

fn parse_body(body: &[u8]) -> Value {
    serde_json::from_slice(body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(body).into_owned()))
}

fn server_error(status: u16, body: &[u8]) -> String {
    let msg = parse_body(body)["error"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "no message".into());
    format!("HTTP {status}: {msg}")
}

/// Maps a server answer: 2xx is the answer; other 4xx is a refusal kept as data (`rejected`); the rest is an error.
fn answer(r: &Reply) -> Result<Value, String> {
    match r.status {
        200..=299 => Ok(parse_body(&r.body)),
        401 | 403 => Err(server_error(r.status, &r.body)),
        400..=499 => Ok(json!({ "http_status": r.status, "rejected": parse_body(&r.body) })),
        s => Err(server_error(s, &r.body)),
    }
}
