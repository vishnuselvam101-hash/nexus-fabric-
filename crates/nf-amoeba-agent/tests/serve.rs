//! A steps-file write that fails stops the serve loop: no answer leaves the bridge without its step record.

use nf_amoeba_agent::agent::Agent;
use nf_amoeba_agent::http::{Reply, Request, Transport};
use nf_amoeba_agent::serve::serve;
use nf_amoeba_agent::steps::{Sink, StepLog};
use std::io::{self, Write};

/// Answers sign-in, the role probe and every other call with 200.
struct Fake;

impl Transport for Fake {
    fn send(&mut self, req: &Request) -> Result<Reply, String> {
        let (set_cookie, body) = match req.path.as_str() {
            "/api/login" => (Some("nf_session=fake".to_string()), "{}"),
            "/api/me" => (None, r#"{"id":2,"username":"agent","role":"engineer"}"#),
            _ => (None, "[]"),
        };
        Ok(Reply {
            status: 200,
            set_cookie,
            body: body.as_bytes().to_vec(),
        })
    }
}

/// Accepts a fixed number of writes, then fails every write like a full disk.
struct Budget {
    left: usize,
}

impl Write for Budget {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Err(io::Error::other("no space left on device"));
        }
        self.left -= 1;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Sink for Budget {
    fn persist(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_failed_steps_write_stops_the_serve_loop_and_answers_nothing() {
    // the sign-in record is the only write that succeeds
    let log = StepLog::with_sink(Box::new(Budget { left: 1 }), 0);
    let mut agent = Agent::new(Fake, log);
    agent.login("agent", "not-printed").expect("sign-in");
    let input = b"{\"id\":1,\"tool\":\"list_projects\",\"args\":{}}\n";
    let mut out = Vec::new();
    let result = serve(&mut agent, &input[..], &mut out);
    assert!(result.is_err(), "serve must stop when the steps file fails");
    assert!(
        out.is_empty(),
        "no answer may be written without its step: {}",
        String::from_utf8_lossy(&out)
    );
}
