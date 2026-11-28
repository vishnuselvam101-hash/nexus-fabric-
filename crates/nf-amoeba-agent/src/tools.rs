//! The tool contract: the allowed names and their function-calling schema (`--schema`).
//! Anything not listed here is refused by name, with no HTTP call.

use serde_json::{Value, json};

/// Allowed tools, in schema order. Nothing that writes a decision, a correction, an account, or the audit log is here.
pub const TOOLS: [&str; 23] = [
    "list_projects",
    "get_project",
    "upload_document",
    "pdf_to_dxf",
    "get_conversion",
    "calculate",
    "get_job",
    "get_amoeba",
    "get_dependencies",
    "replay",
    "recompute",
    "list_obligations",
    "list_lessons",
    "amoeba_summary",
    "get_dxf_digest",
    "propose_read",
    // AIOP-1: the order API and the operator's event stream. Writes are reasoned and made under the agent account;
    // none of them confirms a person's check, approves or releases.
    "order_get",
    "order_page_image",
    "order_put_parts",
    "order_put_values",
    "order_compute",
    "order_post_events",
    "order_poll_events",
];

/// The five calculation families the server accepts (`family` of a calculation request).
pub const FAMILIES: [&str; 5] = [
    "cylinder_shell",
    "cone_frustum",
    "eccentric_reducer",
    "branch_stub",
    "header_with_branch",
];

pub fn is_allowed(name: &str) -> bool {
    TOOLS.contains(&name)
}

/// The definition of one allowed tool.
pub fn definition(name: &str) -> Option<Value> {
    definitions().into_iter().find(|d| d["name"] == name)
}

fn id(desc: &str) -> Value {
    json!({ "type": "integer", "minimum": 1, "description": desc })
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

/// The complete tool list with function-calling schemas (English descriptions).
pub fn definitions() -> Vec<Value> {
    vec![
        def(
            "list_projects",
            "List the projects you can see.",
            obj(json!({}), &[]),
        ),
        def(
            "get_project",
            "Get one project: documents, PDF conversions, calculations, members, snapshot.",
            obj(json!({ "project": id("Project id.") }), &["project"]),
        ),
        def(
            "upload_document",
            "Upload a PDF file from the documents directory into a project. Only files inside that directory whose content starts with %PDF- and that are at most 100 MB are sent. Returns the document id.",
            obj(
                json!({ "project": id("Project id."), "path": { "type": "string", "minLength": 1, "description": "Local path of the PDF file." } }),
                &["project", "path"],
            ),
        ),
        def(
            "pdf_to_dxf",
            "Convert a PDF document to DXF. The scale is required: a drawing scale 1:N (a PDF has no real-world scale). Returns the conversion record and report.",
            obj(
                json!({
                    "document": id("Document id of a PDF."),
                    "scale_n": { "type": "number", "exclusiveMinimum": 0, "description": "N of the drawing scale 1:N." },
                    "options": obj(json!({
                        "page": { "type": "integer", "minimum": 1, "description": "Page number, default 1." },
                        "tolerance_mm": { "type": "number", "exclusiveMinimum": 0, "description": "Tolerance in millimetres." },
                        "include_fill": { "type": "boolean", "description": "Include filled areas, default true." },
                        "include_text": { "type": "boolean", "description": "Include text, default true." }
                    }), &[]),
                }),
                &["document", "scale_n"],
            ),
        ),
        def(
            "get_conversion",
            "Get one PDF to DXF conversion record and its report.",
            obj(
                json!({ "conversion": id("Conversion id.") }),
                &["conversion"],
            ),
        ),
        def(
            "calculate",
            "Run one calculation job of a family in a project. Every parameter must be given explicitly; nothing is filled in. Returns the job id and state; the state is never an approval.",
            obj(
                json!({
                    "project": id("Project id."),
                    "family": { "type": "string", "enum": FAMILIES, "description": "Calculation family." },
                    "params": { "type": "object", "description": "The family's parameters, as numbers or text. Unknown names are refused by the server.", "additionalProperties": { "type": ["number", "string", "boolean"] } }
                }),
                &["project", "family", "params"],
            ),
        ),
        def(
            "get_job",
            "Get one calculation job: state, checks, gates, parts.",
            obj(json!({ "job": id("Calculation job id.") }), &["job"]),
        ),
        def(
            "get_amoeba",
            "Get the AMOEBA digest state of a job: digest records per part, gate, stages and hold reasons.",
            obj(json!({ "job": id("Calculation job id.") }), &["job"]),
        ),
        def(
            "get_dependencies",
            "Get the dependency edges of a job (assembly parts that depend on each other).",
            obj(json!({ "job": id("Calculation job id.") }), &["job"]),
        ),
        def(
            "replay",
            "Replay a job: re-run its stored request and compare the result with the stored one.",
            obj(json!({ "job": id("Calculation job id.") }), &["job"]),
        ),
        def(
            "recompute",
            "Recompute a job with the current engine version. This is a write: it supersedes the job and records an audit entry.",
            obj(json!({ "job": id("Calculation job id.") }), &["job"]),
        ),
        def(
            "list_obligations",
            "List the open and resolved obligations of all projects you can see.",
            obj(json!({}), &[]),
        ),
        def(
            "list_lessons",
            "List the lessons recorded from corrections and engine failures.",
            obj(json!({}), &[]),
        ),
        def(
            "amoeba_summary",
            "Summary of the AMOEBA activity records.",
            obj(json!({}), &[]),
        ),
        def(
            "get_dxf_digest",
            "Download one part's DXF only to measure it: returns its size and sha256. When the server withholds the file, returns the server's hold reasons unchanged and no bytes. A hold is never bypassed.",
            obj(
                json!({
                    "job": id("Calculation job id."),
                    "part": { "type": "string", "minLength": 1, "maxLength": 32, "description": "Part name, lower case letters and underscores, e.g. reducer or header." }
                }),
                &["job", "part"],
            ),
        ),
        def(
            "propose_read",
            "Record a CANDIDATE reading of a value from a drawing (document, page, box, value, unit). It is only written to the steps file and is never used as a parameter of calculate.",
            obj(
                json!({
                    "document": id("Document id."),
                    "page": { "type": "integer", "minimum": 1, "description": "Page number." },
                    "box": { "type": "array", "minItems": 4, "maxItems": 4, "items": { "type": "number" }, "description": "x0, y0, x1, y1 on the page, in PDF points." },
                    "value": { "type": "number", "description": "The value read." },
                    "unit": { "type": "string", "maxLength": 16, "description": "Unit of the value, e.g. mm." },
                    "kind": { "type": "string", "maxLength": 32, "description": "What the value is, e.g. header_radius." }
                }),
                &["document", "page", "box", "value", "unit", "kind"],
            ),
        ),
        def(
            "order_get",
            "Get one order: files, pages, parts, values with their states, decisions.",
            obj(json!({ "order": id("Order id.") }), &["order"]),
        ),
        def(
            "order_page_image",
            "Fetch the picture of one page of an order. Returns its size and sha256; the bytes stay with the caller that runs the reader.",
            obj(
                json!({ "order": id("Order id."), "page": id("Page number in the order.") }),
                &["order", "page"],
            ),
        ),
        def(
            "order_put_parts",
            "Create or update the parts list of an order. A reason is required.",
            obj(
                json!({
                    "order": id("Order id."),
                    "parts": { "type": "array", "items": { "type": "object", "additionalProperties": true }, "description": "Parts: no, name, shape, qty, page, attach_part, group_key." },
                    "reason": { "type": "string", "minLength": 1, "maxLength": 500, "description": "Why the operator makes this change." }
                }),
                &["order", "parts", "reason"],
            ),
        ),
        def(
            "order_put_values",
            "Set values of parts. Each value has part, key, value, text, bbox, source and a note (the reason). A value that carries a reader flag must not be sent with confirm true.",
            obj(
                json!({ "order": id("Order id."), "values": { "type": "array", "items": { "type": "object", "additionalProperties": true }, "description": "Values: part, key, value, text, bbox, source, confirm, note." } }),
                &["order", "values"],
            ),
        ),
        def(
            "order_compute",
            "Compute the order with the current engine. The result never approves or releases anything; held parts get no DXF.",
            obj(
                json!({ "order": id("Order id."), "reason": { "type": "string", "minLength": 1, "maxLength": 500, "description": "Why the operator computes now." } }),
                &["order", "reason"],
            ),
        ),
        def(
            "order_post_events",
            "Send operator events (what the cursor shows) to the server, which stores and audits them.",
            obj(
                json!({ "order": id("Order id."), "events": { "type": "array", "items": { "type": "object", "additionalProperties": true }, "description": "Operator events: step, action, target, value, reason, confidence, flags, tool, write_ref." } }),
                &["order", "events"],
            ),
        ),
        def(
            "order_poll_events",
            "Read the operator events after a sequence number, and whether a person paused the operator.",
            obj(
                json!({ "order": id("Order id."), "after": { "type": "integer", "minimum": 0, "description": "Last seq already seen." } }),
                &["order"],
            ),
        ),
    ]
}

fn def(name: &str, description: &str, parameters: Value) -> Value {
    json!({ "name": name, "description": description, "parameters": parameters })
}

/// The `--schema` document: an array of tool definitions.
pub fn schema_document() -> Value {
    Value::Array(definitions())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_match_the_allowed_list_exactly() {
        let names: Vec<String> = definitions()
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            TOOLS.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
    }
}
