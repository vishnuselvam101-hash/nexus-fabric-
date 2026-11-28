//! Operator events: what the cursor shows. The server stores them, assigns `seq` and audits them.

use serde_json::{Value, json};

/// Where an event happens. Exactly one kind per event.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A box on a page picture, in page pixels (without a box: the page as a whole).
    Page { page: i64, bbox: Option<[f64; 4]> },
    /// An element of the interface, by id.
    Ui(String),
    /// A point of a CAD canvas and/or an entity id (for the CAD tools that come later).
    Cad {
        canvas: Option<[f64; 2]>,
        entity: Option<String>,
    },
}

impl Target {
    pub fn page_box(page: i64, b: [f32; 4]) -> Target {
        Target::Page {
            page,
            bbox: Some(b.map(f64::from)),
        }
    }
    pub fn to_json(&self) -> Value {
        match self {
            Target::Page {
                page,
                bbox: Some(b),
            } => json!({ "page": page, "bbox": b }),
            Target::Page { page, bbox: None } => json!({ "page": page }),
            Target::Ui(id) => json!({ "ui": id }),
            Target::Cad { canvas, entity } => {
                json!({ "cad": { "canvas": canvas, "entity": entity } })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub step: String,
    /// look, point, click, type, compute or ask today; any lower-case word later
    pub action: String,
    pub target: Target,
    pub value: Value,
    pub reason: String,
    /// Translation key and arguments for the cursor; the English `reason` stays for the audit.
    pub reason_key: Option<String>,
    pub reason_args: Value,
    pub confidence: Option<f64>,
    pub flags: Vec<String>,
    /// registry name of the tool that made the call
    pub tool: String,
    /// the value this event wrote (part no, key): what an undo puts back
    pub write_ref: Option<(String, String)>,
}

impl Event {
    pub fn new(step: &str, action: &str, target: Target, reason: &str) -> Event {
        Event {
            step: step.into(),
            action: action.into(),
            target,
            value: Value::Null,
            reason: reason.into(),
            reason_key: None,
            reason_args: json!({}),
            confidence: None,
            flags: vec![],
            tool: String::new(),
            write_ref: None,
        }
    }
    pub fn value(mut self, v: Value) -> Event {
        self.value = v;
        self
    }
    pub fn reason_key(mut self, key: &str, args: Value) -> Event {
        self.reason_key = Some(key.into());
        self.reason_args = args;
        self
    }
    pub fn confidence(mut self, c: f32) -> Event {
        self.confidence = Some(f64::from(c).clamp(0.0, 1.0));
        self
    }
    pub fn flags(mut self, f: &[String]) -> Event {
        self.flags = f.to_vec();
        self
    }
    pub fn written(mut self, part: &str, key: &str) -> Event {
        self.write_ref = Some((part.into(), key.into()));
        self
    }
    pub fn to_json(&self) -> Value {
        json!({
            "step": self.step, "action": self.action, "target": self.target.to_json(),
            "value": self.value, "reason": self.reason,
            "reason_key": self.reason_key, "reason_args": self.reason_args,
            "confidence": self.confidence, "flags": self.flags,
            "tool": if self.tool.is_empty() { Value::Null } else { json!(self.tool) },
            "write_ref": self.write_ref.as_ref().map(|(p, k)| json!({ "part": p, "key": k })),
        })
    }
}
