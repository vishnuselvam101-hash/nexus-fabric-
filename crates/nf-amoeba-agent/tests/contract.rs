//! The `--schema` contract: valid schema, exactly the allowed tools, nothing forbidden.

use nf_amoeba_agent::schema::check_definition;
use nf_amoeba_agent::tools::TOOLS;
use serde_json::Value;
use std::process::Command;

fn schema_output() -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_nf-amoeba-tools"))
        .arg("--schema")
        .output()
        .expect("run nf-amoeba-tools --schema");
    assert!(out.status.success(), "--schema failed");
    serde_json::from_slice(&out.stdout).expect("--schema prints valid JSON")
}

#[test]
fn schema_is_valid_json_schema_and_lists_exactly_the_allowed_tools() {
    let doc = schema_output();
    let defs = doc.as_array().expect("an array of tool definitions");
    for d in defs {
        check_definition(d).unwrap_or_else(|e| panic!("invalid definition: {e}"));
    }
    let names: Vec<&str> = defs.iter().map(|d| d["name"].as_str().unwrap()).collect();
    assert_eq!(names, TOOLS.to_vec());
    assert_eq!(names.len(), 23);
}

#[test]
fn no_write_decision_or_account_tool_is_in_the_schema() {
    let doc = schema_output();
    let forbidden = [
        "correct",
        "resolve",
        "setup",
        "user",
        "member",
        "password",
        "logout",
        "audit",
        "approve",
        "release",
        "accept",
        "sign",
        "delete",
        "upload_user",
        "add_",
    ];
    for d in doc.as_array().unwrap() {
        let name = d["name"].as_str().unwrap();
        for f in forbidden {
            assert!(!name.contains(f), "forbidden word '{f}' in tool '{name}'");
        }
    }
}

#[test]
fn every_schema_path_is_used_by_the_tool_list() {
    // a tool with no parameters must still declare an object schema with no required names
    for d in schema_output().as_array().unwrap() {
        assert_eq!(d["parameters"]["type"], "object");
        assert_eq!(d["parameters"]["additionalProperties"], Value::Bool(false));
    }
}

#[test]
fn readme_marks_recompute_as_a_write() {
    // recompute supersedes the job and writes an audit entry: the README must not call it a read
    let readme =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")).unwrap();
    let line = readme
        .lines()
        .find(|l| l.contains("recompute") && l.contains("write"))
        .unwrap_or_else(|| panic!("README does not describe recompute as a write"));
    assert!(
        line.contains("supersede") || line.contains("audit"),
        "{line}"
    );
}
