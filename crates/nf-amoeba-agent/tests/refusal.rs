//! Forbidden tool names are refused by the binary itself, with no HTTP call. A counting listener proves it:
//! the only connection is the sign-in made at start-up.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const FORBIDDEN: [&str; 14] = [
    "corrections",
    "correct",
    "resolve_obligation",
    "setup",
    "users_active",
    "users/active",
    "members",
    "password",
    "logout",
    "audit",
    "approve",
    "release",
    "delete_project",
    "add_user",
];

fn serve_counting(listener: TcpListener, count: Arc<AtomicUsize>) {
    for stream in listener.incoming() {
        let Ok(mut s) = stream else { continue };
        count.fetch_add(1, Ordering::SeqCst);
        // read the request head (the body is not needed for the answer)
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            match s.read(&mut byte) {
                Ok(1) => head.push(byte[0]),
                _ => break,
            }
        }
        let text = String::from_utf8_lossy(&head).to_ascii_lowercase();
        let length: usize = text
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let mut body = vec![0u8; length];
        let _ = s.read_exact(&mut body);
        // sign-in gets a session cookie; the role probe gets an engineer role; nothing else is ever answered
        let body = if text.starts_with("get /api/me") {
            r#"{"id":2,"username":"agent","role":"engineer"}"#
        } else {
            "{}"
        };
        let cookie = if text.starts_with("post /api/login") {
            "Set-Cookie: nf_session=fake; Path=/\r\n"
        } else {
            ""
        };
        let reply = format!(
            "HTTP/1.1 200 OK\r\n{cookie}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = s.write_all(reply.as_bytes());
    }
}

#[test]
fn forbidden_tools_are_refused_without_any_http_call() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let c2 = count.clone();
    std::thread::spawn(move || serve_counting(listener, c2));

    let dir = std::env::temp_dir().join(format!("nf-refusal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let steps = dir.join("steps.jsonl");
    let _ = std::fs::remove_file(&steps);

    let mut child = Command::new(env!("CARGO_BIN_EXE_nf-amoeba-tools"))
        .args(["--base", &format!("http://127.0.0.1:{port}"), "--steps"])
        .arg(&steps)
        .env("NF_AGENT_USER", "agent")
        .env("NF_AGENT_PASS", "not-a-real-password")
        .arg("--docs-dir")
        .arg(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    let mut answers = Vec::new();
    for (i, tool) in FORBIDDEN.iter().enumerate() {
        let req =
            json!({ "id": i, "tool": tool, "args": { "project": 1, "job": 1, "reason": "x" } });
        writeln!(stdin, "{req}").unwrap();
        stdin.flush().unwrap();
        line.clear();
        let n = out.read_line(&mut line).unwrap();
        let mut err = String::new();
        if n == 0 {
            let _ = stderr.read_to_string(&mut err);
            panic!("bridge closed its output: {err}");
        }
        answers.push(serde_json::from_str::<Value>(&line).unwrap());
    }
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "binary exited with {status}");

    for (tool, a) in FORBIDDEN.iter().zip(&answers) {
        assert_eq!(a["ok"], false, "{tool} must not succeed");
        assert_eq!(a["error"], "tool not available", "{tool}");
    }
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "only the start-up sign-in and its role check may reach the server"
    );

    let text = std::fs::read_to_string(&steps).unwrap();
    let refused = text
        .lines()
        .filter(|l| l.contains("\"status\":\"refused\""))
        .count();
    assert_eq!(refused, FORBIDDEN.len(), "every refused call is recorded");
    assert!(
        !text.contains("not-a-real-password"),
        "the password never reaches the steps file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
