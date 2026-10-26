//! `nigel serve` is not reachable through a public hostname until the books
//! have a user: the real binary, raw HTTP over TCP.

#![cfg(feature = "serve")]

mod common;
use common::serve::{add_admin, init, Server};
use common::TestEnv;

const PUBLIC_URL: &str = "https://books.example.com";
const PUBLIC_HOST: &str = "books.example.com";
const BOOKS_PASSWORD: &str = "correct horse battery staple";
const LOCAL_MODE_NOTICE: &str = "server_public_url is set, but these books have no users";

#[test]
fn a_public_host_is_refused_while_the_books_have_no_users() {
    let env = TestEnv::new();
    init(&env);
    let mut server = Server::start(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
    let token = server.token().expect("local mode prints the token link");
    let loopback = server.loopback();

    let ping = server.get("/api/ping", PUBLIC_HOST, "");
    assert_eq!(ping.status, 403, "{ping:?}");
    assert!(ping.body.contains("forbidden"), "{ping:?}");

    let link = server.get(&format!("/auth?token={token}"), PUBLIC_HOST, "");
    assert_eq!(link.status, 403, "{link:?}");
    assert!(link.cookie().is_none(), "{link:?}");

    let forwarded = server.get(
        "/api/ping",
        PUBLIC_HOST,
        &format!("X-Forwarded-Host: {loopback}\r\n"),
    );
    assert_eq!(forwarded.status, 403, "X-Forwarded-Host is not the Host");

    let ping = server.get("/api/ping", &loopback, "");
    assert_eq!(ping.status, 401, "{ping:?}");
    assert!(!ping.body.contains(r#""mode":"shared""#), "{ping:?}");

    let link = server.get(&format!("/auth?token={token}"), &loopback, "");
    assert!(
        link.cookie().is_some(),
        "loopback still takes the token: {link:?}"
    );

    let stderr = server.stop();
    assert!(stderr.contains(LOCAL_MODE_NOTICE), "{stderr}");
}

#[test]
fn the_first_user_turns_on_shared_mode_and_the_public_host() {
    let env = TestEnv::new();
    init(&env);
    let old_token = Server::start(&env, &[]).token().expect("a local token");
    let temporary = add_admin(&env, &[]);

    let mut server = Server::start(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
    assert!(server.token().is_none(), "shared mode prints no token");
    let loopback = server.loopback();

    let ping = server.get("/api/ping", PUBLIC_HOST, "");
    assert_eq!(ping.status, 401, "{ping:?}");
    assert!(ping.body.contains(r#""mode":"shared""#), "{ping:?}");

    let other = server.get("/api/ping", "other.example.com", "");
    assert_eq!(other.status, 403, "{other:?}");

    let link = server.get(&format!("/auth?token={old_token}"), &loopback, "");
    assert_eq!(link.status, 401, "{link:?}");
    assert!(link.cookie().is_none(), "{link:?}");

    let public = server.login(PUBLIC_HOST, PUBLIC_URL, "pat@example.com", &temporary);
    assert_eq!(public.status, 200, "{public:?}");
    let cookie = public.cookie().expect("a session cookie");
    for part in ["nigel_session=", "httponly", "samesite=strict", "secure"] {
        assert!(cookie.contains(part), "{part}: {cookie}");
    }
    assert!(
        public
            .head
            .contains("strict-transport-security: max-age=31536000"),
        "{public:?}"
    );
    assert!(
        public.body.contains(r#""mustChangePassword":true"#),
        "{public:?}"
    );

    let local = server.login(
        &loopback,
        &format!("http://{loopback}"),
        "pat@example.com",
        &temporary,
    );
    assert_eq!(local.status, 200, "{local:?}");
    let cookie = local.cookie().expect("a session cookie");
    assert!(
        !cookie.contains("secure"),
        "loopback cookies are plain http: {cookie}"
    );
    assert!(
        !local.head.contains("strict-transport-security"),
        "{local:?}"
    );

    let stderr = server.stop();
    assert!(!stderr.contains(LOCAL_MODE_NOTICE), "{stderr}");
    assert!(!stderr.contains(&temporary), "{stderr}");
}

#[test]
fn x_forwarded_host_never_stands_in_for_the_host() {
    let env = TestEnv::new();
    init(&env);
    let temporary = add_admin(&env, &[]);
    let server = Server::start(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
    let loopback = server.loopback();
    let forwarded = format!("X-Forwarded-Host: {PUBLIC_HOST}\r\n");

    let other = server.get("/api/ping", "other.example.com", &forwarded);
    assert_eq!(other.status, 403, "{other:?}");

    let ping = server.get("/api/ping", &loopback, &forwarded);
    assert_eq!(ping.status, 401, "{ping:?}");
    assert!(!ping.head.contains("strict-transport-security"), "{ping:?}");

    let body = serde_json::json!({ "email": "pat@example.com", "password": temporary }).to_string();
    let login = server.send(&format!(
        "POST /api/auth/login HTTP/1.1\r\nHost: {loopback}\r\n{forwarded}\
         Origin: http://{loopback}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ));
    assert_eq!(login.status, 200, "{login:?}");
    let cookie = login.cookie().expect("a session cookie");
    assert!(!cookie.contains("secure"), "answered as loopback: {cookie}");
    assert!(
        !login.head.contains("strict-transport-security"),
        "{login:?}"
    );
}

#[test]
fn two_host_headers_are_refused() {
    let env = TestEnv::new();
    init(&env);
    let local = Server::start(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
    let loopback = local.loopback();
    for (first, second) in [
        (PUBLIC_HOST, loopback.as_str()),
        (loopback.as_str(), PUBLIC_HOST),
    ] {
        let reply = local.get("/api/ping", first, &format!("Host: {second}\r\n"));
        assert_eq!(
            reply.status, 403,
            "local, Host {first} then {second}: {reply:?}"
        );
    }
    drop(local);

    add_admin(&env, &[]);
    let shared = Server::start(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
    for (first, second) in [
        (PUBLIC_HOST, "other.example.com"),
        ("other.example.com", PUBLIC_HOST),
    ] {
        let reply = shared.get("/api/ping", first, &format!("Host: {second}\r\n"));
        assert_eq!(
            reply.status, 403,
            "shared, Host {first} then {second}: {reply:?}"
        );
    }
}

#[test]
fn locked_books_are_local_until_the_server_unlocks_them_at_boot() {
    let env = TestEnv::new();
    init(&env);
    add_admin(&env, &[]);
    env.encrypt(BOOKS_PASSWORD);

    {
        let mut server = Server::start(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
        assert!(server.token().is_some(), "locked books are local");
        let ping = server.get("/api/ping", PUBLIC_HOST, "");
        assert_eq!(ping.status, 403, "{ping:?}");
        let stderr = server.stop();
        assert!(stderr.contains(LOCAL_MODE_NOTICE), "{stderr}");
    }

    let password_file = env.home.path().join("books-password");
    std::fs::write(&password_file, format!("{BOOKS_PASSWORD}\n")).expect("write password file");
    let server = Server::start(
        &env,
        &[
            ("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL),
            (
                "NIGEL_DB_PASSWORD_FILE",
                password_file.to_str().expect("utf-8 path"),
            ),
        ],
    );
    assert!(server.token().is_none(), "unlocked at boot: shared");
    let ping = server.get("/api/ping", PUBLIC_HOST, "");
    assert_eq!(ping.status, 401, "{ping:?}");
    assert!(ping.body.contains(r#""mode":"shared""#), "{ping:?}");
}
