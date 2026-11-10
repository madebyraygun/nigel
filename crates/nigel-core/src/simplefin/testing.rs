//! A loopback stand-in for a SimpleFIN bridge, shared by the module's tests
//! and the server's sync route tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use base64::Engine;

use super::AccessUrl;

/// The credentials every [`StubBridge::access_url`] carries.
pub(crate) const STUB_USER: &str = "demo";
pub(crate) const STUB_PASSWORD: &str = "s3cret";

/// What the stub saw of one request.
#[derive(Debug, Clone)]
pub(crate) struct Seen {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) authorization: Option<String>,
}

/// A one-thread HTTP/1.1 bridge on loopback that answers each request with
/// the next canned (status, body) and records what it was asked.
pub(crate) struct StubBridge {
    pub(crate) port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl StubBridge {
    pub(crate) fn start(responses: Vec<(u16, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut authorization = None;
                let mut length = 0usize;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let line = line.trim_end();
                    if line.is_empty() {
                        break;
                    }
                    let (name, value) = line.split_once(':').unwrap_or((line, ""));
                    match name.to_ascii_lowercase().as_str() {
                        "authorization" => authorization = Some(value.trim().to_string()),
                        "content-length" => length = value.trim().parse().unwrap_or(0),
                        _ => {}
                    }
                }
                let mut body_in = vec![0; length];
                reader.read_exact(&mut body_in).unwrap();
                let mut parts = request_line.split_whitespace();
                log.lock().unwrap().push(Seen {
                    method: parts.next().unwrap_or("").to_string(),
                    target: parts.next().unwrap_or("").to_string(),
                    authorization,
                });
                let mut stream = stream;
                write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        Self { port, seen }
    }

    pub(crate) fn access_url(&self) -> AccessUrl {
        AccessUrl::parse(&self.access_url_string()).unwrap()
    }

    pub(crate) fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
    /// The access URL as stored in settings.json.
    pub(crate) fn access_url_string(&self) -> String {
        format!(
            "http://{STUB_USER}:{STUB_PASSWORD}@127.0.0.1:{}/simplefin",
            self.port
        )
    }

    /// A setup token whose claim URL is this bridge.
    pub(crate) fn setup_token(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(format!(
            "http://127.0.0.1:{}/simplefin/claim/abc",
            self.port
        ))
    }
}
