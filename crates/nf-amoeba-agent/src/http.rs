//! Minimal HTTP/1.1 client for the loopback NEXUS FABRIC server. Standard library only: no network crate is added.
//! One request per connection (`Connection: close`). The transport is a trait so tests can count requests.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// The largest answer the bridge accepts from the server (the DXF and PDF downloads are the big ones).
pub const MAX_ANSWER: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    /// `name=value` of the first Set-Cookie header, without its attributes.
    pub set_cookie: Option<String>,
    pub body: Vec<u8>,
}

pub trait Transport {
    fn send(&mut self, req: &Request) -> Result<Reply, String>;
}

/// A TCP connection to `host:port` (host must be a loopback name; the caller checks that).
pub struct Tcp {
    pub host: String,
    pub port: u16,
}

impl Transport for Tcp {
    fn send(&mut self, req: &Request) -> Result<Reply, String> {
        let addr = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|e| format!("cannot resolve {}: {e}", self.host))?
            .next()
            .ok_or_else(|| format!("no address for {}", self.host))?;
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
            .map_err(|e| format!("cannot connect to the server: {e}"))?;
        s.set_read_timeout(Some(Duration::from_secs(600))).ok();
        s.set_write_timeout(Some(Duration::from_secs(60))).ok();
        let mut head = format!(
            "{} {} HTTP/1.1\r\nHost: {}:{}\r\nX-NF-Request: 1\r\nConnection: close\r\nContent-Length: {}\r\n",
            req.method,
            req.path,
            self.host,
            self.port,
            req.body.len()
        );
        for (k, v) in &req.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str("\r\n");
        s.write_all(head.as_bytes())
            .and_then(|()| s.write_all(&req.body))
            .map_err(|e| format!("cannot send the request: {e}"))?;
        let raw = read_capped(&mut s, MAX_ANSWER)?;
        parse_reply(&raw)
    }
}

/// Reads a whole answer, refusing one larger than `cap` bytes instead of holding it in memory.
pub fn read_capped<R: Read>(r: R, cap: u64) -> Result<Vec<u8>, String> {
    let mut raw = Vec::new();
    r.take(cap.saturating_add(1))
        .read_to_end(&mut raw)
        .map_err(|e| format!("cannot read the answer: {e}"))?;
    if raw.len() as u64 > cap {
        return Err(format!(
            "the answer is larger than {} MiB: refused",
            cap / (1024 * 1024)
        ));
    }
    Ok(raw)
}

/// Splits a raw HTTP/1.1 answer into status, first Set-Cookie and the body (chunked or length-delimited).
pub fn parse_reply(raw: &[u8]) -> Result<Reply, String> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed answer: no end of headers")?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut body = raw[split + 4..].to_vec();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("malformed answer: no status line")?;
    let mut set_cookie = None;
    let mut chunked = false;
    let mut length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "set-cookie" if set_cookie.is_none() => {
                set_cookie = value.split(';').next().map(|c| c.trim().to_string());
            }
            "transfer-encoding" => chunked = value.to_ascii_lowercase().contains("chunked"),
            "content-length" => length = value.parse::<usize>().ok(),
            _ => {}
        }
    }
    if chunked {
        body = dechunk(&body)?;
    } else if let Some(n) = length {
        body.truncate(n);
    }
    Ok(Reply {
        status,
        set_cookie,
        body,
    })
}

fn dechunk(mut data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let eol = data
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("malformed chunked answer")?;
        let size_text = String::from_utf8_lossy(&data[..eol]).into_owned();
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "malformed chunk size")?;
        data = &data[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        let need = size.checked_add(2).ok_or("malformed chunk size")?;
        if data.len() < need {
            return Err("truncated chunked answer".into());
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_delimited_answer_with_cookie() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nSet-Cookie: nf_session=abc; HttpOnly; Path=/\r\n\r\n{}";
        let r = parse_reply(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.set_cookie.as_deref(), Some("nf_session=abc"));
        assert_eq!(r.body, b"{}");
    }

    #[test]
    fn chunked_answer_is_decoded() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5;x=1\r\npedia\r\n0\r\n\r\n";
        let r = parse_reply(raw).unwrap();
        assert_eq!(r.body, b"Wikipedia");
        assert!(r.set_cookie.is_none());
    }

    #[test]
    fn truncated_chunk_is_an_error() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n9\r\nabc";
        assert!(parse_reply(raw).is_err());
    }

    #[test]
    fn missing_headers_end_is_an_error() {
        assert!(parse_reply(b"HTTP/1.1 200 OK\r\n").is_err());
    }

    #[test]
    fn chunk_size_that_overflows_is_an_error() {
        // size + 2 would overflow usize: this must be an error, not a panic or a wrapped length
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\nabc";
        assert!(parse_reply(raw).is_err());
    }

    #[test]
    fn answer_larger_than_the_cap_is_refused() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            let (mut s, _) = listener.accept().unwrap();
            let mut head = BufReader::new(s.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if head.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            // no length header: the body runs to the close, and it is one byte over the cap
            s.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n")
                .unwrap();
            let chunk = vec![b'a'; 1 << 20];
            for _ in 0..(MAX_ANSWER >> 20) {
                if s.write_all(&chunk).is_err() {
                    return;
                }
            }
            let _ = s.write_all(b"a");
        });
        let mut t = Tcp {
            host: "127.0.0.1".into(),
            port,
        };
        let req = Request {
            method: "GET",
            path: "/big".into(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let err = t
            .send(&req)
            .expect_err("an answer over the cap is an error");
        assert!(err.contains("larger"), "{err}");
        server.join().unwrap();
    }
}
