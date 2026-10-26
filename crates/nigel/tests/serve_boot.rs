//! `nigel serve` startup, through the real binary: unlocking from the
//! environment, the mode it chooses, and what it says before and once it is
//! listening.

#![cfg(feature = "serve")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use predicates::prelude::*;

mod common;
use common::{TestEnv, TEST_TIMEOUT};

const PASSWORD: &str = "correct horse battery staple";
/// A wrong password distinctive enough that finding it in output is a leak.
const WRONG: &str = "wrong-pw-q8z3";

/// `nigel init`, then the update check turned off so a server stays off the
/// network.
fn init(env: &TestEnv) {
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

/// A running `nigel serve`, killed by its PID when dropped.
struct Server {
    child: Child,
    /// Every stdout line up to and including "Press Ctrl-C to stop."
    banner: Vec<String>,
    port: u16,
}

impl Server {
    fn start(env: &TestEnv, vars: &[(&str, &str)]) -> Self {
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

    /// Kill the server and return everything it wrote to stderr.
    fn stop(&mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut stderr = String::new();
        if let Some(mut pipe) = self.child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        stderr
    }

    /// One HTTP/1.1 GET to the server, with `host` as the Host header and
    /// `extra` as further header lines. Returns the raw response.
    fn get(&self, path: &str, host: &str, extra: &str) -> String {
        let mut stream =
            TcpStream::connect(("127.0.0.1", self.port)).expect("connect to nigel serve");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("read timeout");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {host}\r\n{extra}Connection: close\r\n\r\n"
        )
        .expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        response
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

fn add_admin(env: &TestEnv, vars: &[(&str, &str)]) {
    env.cmd()
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
        .assert()
        .success();
}

#[test]
fn a_wrong_password_variable_is_fatal_and_named_without_the_password() {
    let env = TestEnv::new();
    init(&env);
    env.encrypt(PASSWORD);

    env.cmd()
        .args(["serve", "--no-open", "--port", "0"])
        .env("NIGEL_DB_PASSWORD", WRONG)
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("NIGEL_DB_PASSWORD did not unlock")
                .and(predicate::str::contains(WRONG).not()),
        )
        .stdout(predicate::str::contains("Nigel is serving").not());

    let file = env.home.path().join("db-password");
    std::fs::write(&file, format!("{WRONG}\n")).expect("write password file");
    env.cmd()
        .args(["serve", "--no-open", "--port", "0"])
        .env("NIGEL_DB_PASSWORD_FILE", &file)
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("NIGEL_DB_PASSWORD_FILE did not unlock")
                .and(predicate::str::contains(WRONG).not()),
        )
        .stdout(predicate::str::contains("Nigel is serving").not());
}

#[test]
fn an_invalid_public_url_is_refused_at_startup_by_value() {
    let env = TestEnv::new();
    init(&env);

    env.cmd()
        .args(["serve", "--no-open", "--port", "0"])
        .env("NIGEL_SERVER_PUBLIC_URL", "http://books.example.com")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("http://books.example.com"))
        .stdout(predicate::str::contains("Nigel is serving").not());
}

#[test]
fn books_unlocked_from_the_environment_without_users_are_served_in_local_mode() {
    let env = TestEnv::new();
    init(&env);
    env.encrypt(PASSWORD);

    let mut server = Server::start(&env, &[("NIGEL_DB_PASSWORD", PASSWORD)]);
    assert_eq!(
        server.banner[0],
        format!("Nigel is serving on http://127.0.0.1:{}", server.port)
    );
    let token = server
        .banner
        .iter()
        .find_map(|l| {
            l.split_once("/auth?token=")
                .map(|(_, t)| t.trim().to_string())
        })
        .unwrap_or_else(|| panic!("no token in {:?}", server.banner));

    let status = server.get(
        "/api/status",
        &format!("127.0.0.1:{}", server.port),
        &format!("Cookie: nigel_session={token}\r\n"),
    );
    assert!(status.starts_with("HTTP/1.1 200"), "{status}");
    assert!(status.contains(r#""locked":false"#), "{status}");
    assert!(status.contains(r#""mode":"local""#), "{status}");

    let stderr = server.stop();
    assert!(!stderr.contains(PASSWORD), "{stderr}");
    assert!(!stderr.contains("not encrypted"), "{stderr}");
}

#[test]
fn encrypted_books_with_users_unlocked_from_the_environment_are_shared() {
    let env = TestEnv::new();
    init(&env);
    env.encrypt(PASSWORD);
    add_admin(&env, &[("NIGEL_DB_PASSWORD", PASSWORD)]);

    let mut server = Server::start(
        &env,
        &[
            ("NIGEL_DB_PASSWORD", PASSWORD),
            ("NIGEL_SERVER_PUBLIC_URL", "https://books.example.com"),
            ("NIGEL_CLIENT_IP_HEADER", "CF-Connecting-IP"),
        ],
    );
    assert_eq!(
        server.banner[0],
        format!(
            "Nigel is serving these books to signed-in users at https://books.example.com (and http://127.0.0.1:{}).",
            server.port
        )
    );
    let stderr = server.stop();
    assert!(stderr.trim().is_empty(), "no notices expected: {stderr}");
}

#[test]
fn plain_shared_books_behind_a_public_url_are_warned_about_and_reachable_by_name() {
    let env = TestEnv::new();
    init(&env);
    add_admin(&env, &[]);

    let mut server = Server::start(
        &env,
        &[("NIGEL_SERVER_PUBLIC_URL", "https://books.example.com")],
    );
    assert_eq!(
        server.banner[0],
        format!(
            "Nigel is serving these books to signed-in users at https://books.example.com (and http://127.0.0.1:{}).",
            server.port
        )
    );
    assert!(
        !server.banner.iter().any(|l| l.contains("token")),
        "{:?}",
        server.banner
    );

    let me = server.get("/api/auth/me", "books.example.com", "");
    assert!(me.starts_with("HTTP/1.1 401"), "{me}");
    assert!(
        me.to_ascii_lowercase()
            .contains("strict-transport-security: max-age=31536000"),
        "{me}"
    );

    let stderr = server.stop();
    assert!(
        stderr.contains("These books are not encrypted."),
        "{stderr}"
    );
    assert!(
        stderr.contains("client_ip_header is not") && stderr.contains("one sign-in rate limit"),
        "{stderr}"
    );
}
