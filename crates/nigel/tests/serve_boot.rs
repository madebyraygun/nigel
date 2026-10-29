//! `nigel serve` startup, through the real binary: unlocking from the
//! environment, the mode it chooses, and what it says before and once it is
//! listening.

#![cfg(feature = "serve")]

use predicates::prelude::*;

mod common;
use common::serve::{add_admin, init, Server};
use common::{TestEnv, TEST_TIMEOUT};

const PASSWORD: &str = "correct horse battery staple";
/// A wrong password distinctive enough that finding it in output is a leak.
const WRONG: &str = "wrong-pw-q8z3";

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
        .token()
        .unwrap_or_else(|| panic!("no token in {:?}", server.banner));

    let status = server.get(
        "/api/status",
        &server.loopback(),
        &format!("Cookie: nigel_session={token}\r\n"),
    );
    assert_eq!(status.status, 200, "{status:?}");
    assert!(status.body.contains(r#""locked":false"#), "{status:?}");
    assert!(status.body.contains(r#""mode":"local""#), "{status:?}");

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
    assert_eq!(me.status, 401, "{me:?}");
    assert!(
        me.head
            .contains("strict-transport-security: max-age=31536000"),
        "{me:?}"
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
