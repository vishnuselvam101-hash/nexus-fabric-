//! The bridge: one session against the local NEXUS FABRIC server. Every tool is one or more calls to the server's own
//! API; nothing is computed here except hashes and the candidate record. Every call is written to the steps file.

use crate::http::{Reply, Request, Transport};
use crate::schema::validate_value;
use crate::steps::{StepLog, redact};
use crate::tools::{self, FAMILIES};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The largest PDF the bridge will upload.
const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;
/// The one answer for every path that is not a PDF inside the documents directory. The same text for a missing file,
/// a file outside the directory and a link out of it, so the answer shows nothing about the rest of the disk.
const REFUSED_DOCUMENT: &str = "document refused: give a PDF file inside the documents directory";

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
    /// The only directory uploads may come from (canonical). None refuses every upload.
    docs_dir: Option<PathBuf>,
    // scratch state of the call in progress
    statuses: Vec<u16>,
    engine: Option<String>,
    binary: Option<Vec<u8>>,
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
            docs_dir: None,
            statuses: Vec::new(),
            engine: None,
            binary: None,
        }
    }

    /// The bytes of the last `order_page_image` call, once. They are not part of the answer or the steps file.
    pub fn take_binary(&mut self) -> Option<Vec<u8>> {
        self.binary.take()
    }

    /// Sets the documents directory. Uploads may name only files inside it, after symlinks are resolved.
    pub fn with_docs_dir(mut self, dir: &Path) -> Result<Self, String> {
        let real = std::fs::canonicalize(dir)
            .map_err(|e| format!("cannot use the documents directory: {e}"))?;
        if !real.is_dir() {
            return Err("the documents directory is not a directory".into());
        }
        self.docs_dir = Some(real);
        Ok(self)
    }

    /// Number of HTTP requests sent so far (login included).
    pub fn http_calls(&self) -> u64 {
        self.http_calls
    }

    /// Signs in with the dedicated account. The password is sent once and is never recorded or printed.
    /// An administrator account is refused: the bridge is for engineering accounts only.
    pub fn login(&mut self, username: &str, password: &str) -> Result<(), String> {
        let started = Instant::now();
        self.statuses.clear();
        let body = json!({ "username": username, "password": password }).to_string();
        let outcome = match self.raw("POST", "/api/login", json_headers(), body.into_bytes()) {
            Ok(r) if r.status == 200 && r.set_cookie.is_some() => {
                self.cookie = r.set_cookie;
                self.check_role()
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

    fn check_role(&mut self) -> Result<(), String> {
        let me = self.get_answer("/api/me")?;
        match me["role"].as_str() {
            Some("admin") => Err(
                "refused: the bridge does not run with an administrator account; sign in with an engineer account"
                    .into(),
            ),
            Some(_) => Ok(()),
            None => Err("cannot confirm the account role".into()),
        }
    }

    /// Runs one tool call. A name that is not an allowed tool is refused before anything is sent.
    /// `Err` means the step record could not be written: the caller must stop, and no answer is given.
    pub fn call(&mut self, tool: &str, args: &Value) -> Result<Answer, String> {
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
        let step = self
            .record(tool, args, status, result.as_ref(), error.as_ref(), started)
            .map_err(|e| format!("cannot write the steps file: {e}"))?;
        Ok(Answer {
            ok: result.is_some(),
            result,
            error,
            step,
        })
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
            "order_get" => {
                let o = int(a, "order")?;
                self.get_answer(&format!("/api/orders/{o}"))
            }
            "order_page_image" => {
                let (o, n) = (int(a, "order")?, int(a, "page")?);
                let r = self.get_raw(&format!("/api/orders/{o}/pages/{n}.png"))?;
                if r.status != 200 {
                    return answer(&r);
                }
                let v = json!({ "order": o, "page": n, "size": r.body.len(), "sha256": sha256_hex(&r.body) });
                self.binary = Some(r.body);
                Ok(v)
            }
            "order_put_parts" => {
                let o = int(a, "order")?;
                let body = json!({ "parts": a["parts"], "note": a["reason"] });
                self.send_json("PUT", &format!("/api/orders/{o}/parts"), body)
            }
            "order_put_values" => {
                let o = int(a, "order")?;
                let mut body = json!({ "values": a["values"] });
                if let Some(events) = a.get("events") {
                    body["events"] = (*events).clone();
                }
                self.send_json("PUT", &format!("/api/orders/{o}/values"), body)
            }
            "order_compute" => {
                let o = int(a, "order")?;
                self.send_json(
                    "POST",
                    &format!("/api/orders/{o}/compute"),
                    json!({ "note": a["reason"] }),
                )
            }
            "order_post_events" => {
                let o = int(a, "order")?;
                self.send_json(
                    "POST",
                    &format!("/api/orders/{o}/operator/events"),
                    json!({ "events": a["events"] }),
                )
            }
            "order_poll_events" => {
                let o = int(a, "order")?;
                let after = a["after"].as_i64().unwrap_or(0).max(0);
                self.get_answer(&format!("/api/orders/{o}/operator/events?after={after}"))
            }
            _ => Err("tool not available".into()),
        }
    }

    fn upload(&mut self, a: &Value) -> Result<Value, String> {
        let project = int(a, "project")?;
        let docs = self
            .docs_dir
            .clone()
            .ok_or("no documents directory is set: start the bridge with --docs-dir")?;
        let path = upload_path(&docs, a["path"].as_str().unwrap_or_default())?;
        let file = File::open(&path).map_err(|_| REFUSED_DOCUMENT.to_string())?;
        let too_big = || "the file is larger than 100 MB: nothing was sent".to_string();
        if file
            .metadata()
            .map_err(|_| REFUSED_DOCUMENT.to_string())?
            .len()
            > MAX_UPLOAD_BYTES
        {
            return Err(too_big());
        }
        let mut bytes = Vec::new();
        file.take(MAX_UPLOAD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("cannot read the file: {e}"))?;
        if bytes.len() as u64 > MAX_UPLOAD_BYTES {
            return Err(too_big());
        }
        if !bytes.starts_with(b"%PDF-") {
            return Err(
                "not a PDF: the content does not start with %PDF-; nothing was sent".into(),
            );
        }
        let name = path
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

    /// Downloads a part's DXF only to measure it. A hold (HTTP 409) is returned as the server's body, unchanged.
    /// Any other refusal is a rejection, also unchanged; 401/403 and server errors are errors.
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
        if r.status == 409 {
            return Ok(json!({
                "job": job, "part": part, "held": true,
                "http_status": r.status, "hold": parse_body(&r.body),
            }));
        }
        if !(200..=299).contains(&r.status) {
            return answer(&r);
        }
        Ok(json!({
            "job": job, "part": part, "held": false,
            "bytes": r.body.len(), "sha256": sha256_hex(&r.body),
        }))
    }

    fn note_engine(&mut self, v: &Value) {
        self.engine = v["engine"].as_str().map(str::to_string);
    }

    fn send_json(
        &mut self,
        method: &'static str,
        path: &str,
        body: Value,
    ) -> Result<Value, String> {
        let r = self.raw(method, path, json_headers(), body.to_string().into_bytes())?;
        answer(&r)
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

/// Resolves a requested upload path to a real file inside the documents directory. Relative paths are taken from
/// the directory. Every failure gives the same answer, so a missing file and a file outside look alike.
fn upload_path(docs: &Path, given: &str) -> Result<PathBuf, String> {
    let candidate = if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        docs.join(given)
    };
    let real = std::fs::canonicalize(&candidate).map_err(|_| REFUSED_DOCUMENT.to_string())?;
    if !real.starts_with(docs) || !real.is_file() {
        return Err(REFUSED_DOCUMENT.into());
    }
    Ok(real)
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
