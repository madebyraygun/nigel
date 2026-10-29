//! A real `nigel serve` on `--port 0`, spoken to in raw HTTP/1.1 so every
//! header — `Host` above all — is exactly what the test wrote.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use super::{TestEnv, TEST_TIMEOUT};

/// `nigel init`, then the update check turned off so a server stays off the
/// network.
pub fn init(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
    let path = env.home.path().join(".config/nigel/settings.json");
    let text = std::fs::read_to_string(&path).expect("read settings.json");
    let mut settings: serde_json::Value = serde_json::from_str(&text).expect("parse settings");
    settings["update_check"] = serde_json::Value::Bool(false);
    std::fs::write(&path, settings.to_string()).expect("write settings.json");
}

/// `nigel user add pat@example.com --admin`; returns the temporary password.
pub fn add_admin(env: &TestEnv, vars: &[(&str, &str)]) -> String {
    let output = env
        .cmd()
        .args([
            "user",
            "add",
            "pat@example.com",
            "--name",
            "Pat Example",
            "--admin",
        ])
        .envs(vars.iter().copied())
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .output()
        .expect("run nigel user add");
    assert!(output.status.success(), "nigel user add failed: {output:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8 stdout")
        .lines()
        .find_map(|line| line.trim().strip_prefix("Temporary password: "))
        .expect("a temporary password in the output")
        .trim()
        .to_string()
}

/// One HTTP response: the status, the head lower-cased, and the body.
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub head: String,
    pub body: String,
}

impl Reply {
    /// The `set-cookie:` line, if the response has one.
    pub fn cookie(&self) -> Option<&str> {
        self.head.lines().find(|l| l.starts_with("set-cookie:"))
    }
}

/// A running `nigel serve`, killed by its PID when dropped.
pub struct Server {
    child: Child,
    /// Every stdout line up to and including "Press Ctrl-C to stop."
    pub banner: Vec<String>,
    pub port: u16,
}

impl Server {
    /// Start `nigel serve --no-open --port 0` with `vars` set, and wait for
    /// the line that says it is listening.
    pub fn start(env: &TestEnv, vars: &[(&str, &str)]) -> Self {
        let mut cmd = env.std_cmd();
        cmd.args(["serve", "--no-open", "--port", "0"])
            .envs(vars.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().expect("spawn nigel serve");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut server = Server {
            child,
            banner: Vec::new(),
            port: 0,
        };
        loop {
            match rx.recv_timeout(TEST_TIMEOUT) {
                Ok(line) => {
                    let done = line == "Press Ctrl-C to stop.";
                    server.banner.push(line);
                    if done {
                        break;
                    }
                }
                Err(_) => {
                    let stderr = server.stop();
                    panic!(
                        "no banner from nigel serve; stdout so far: {:?}\nstderr: {stderr}",
                        server.banner
                    );
                }
            }
        }
        server.port = port_in(&server.banner[0]);
        server
    }

    /// `127.0.0.1:<port>`, the loopback `Host` a browser sends.
    pub fn loopback(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// The local-mode token from the banner's `/auth?token=` link.
    pub fn token(&self) -> Option<String> {
        self.banner.iter().find_map(|l| {
            l.split_once("/auth?token=")
                .map(|(_, t)| t.trim().to_string())
        })
    }

    /// Kill the server and return everything it wrote to stderr.
    pub fn stop(&mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut stderr = String::new();
        if let Some(mut pipe) = self.child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        stderr
    }

    /// Send `request` as written and read the response to the end. The
    /// request must say `Connection: close`.
    pub fn send(&self, request: &str) -> Reply {
        let mut stream =
            TcpStream::connect(("127.0.0.1", self.port)).expect("connect to nigel serve");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("read timeout");
        stream.write_all(request.as_bytes()).expect("send request");
        let mut raw = String::new();
        stream.read_to_string(&mut raw).expect("read response");
        let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("no status line in {raw:?}"));
        Reply {
            status,
            head: head.to_ascii_lowercase(),
            body: body.to_string(),
        }
    }

    /// A GET with `host` as the Host header and `extra` as further header
    /// lines, each ending `\r\n`.
    pub fn get(&self, path: &str, host: &str, extra: &str) -> Reply {
        self.send(&format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\n{extra}Connection: close\r\n\r\n"
        ))
    }

    /// A browser-style `POST /api/auth/login`.
    pub fn login(&self, host: &str, origin: &str, email: &str, password: &str) -> Reply {
        let body = serde_json::json!({ "email": email, "password": password }).to_string();
        self.send(&format!(
            "POST /api/auth/login HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        ))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The port in the banner's first line, which ends `http://127.0.0.1:<port>`
/// plus at most a closing parenthesis and full stop.
fn port_in(line: &str) -> u16 {
    let (_, rest) = line
        .rsplit_once("http://127.0.0.1:")
        .unwrap_or_else(|| panic!("no loopback URL in {line:?}"));
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("no port in {line:?}"))
}
