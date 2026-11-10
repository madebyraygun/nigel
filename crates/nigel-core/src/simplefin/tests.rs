use base64::Engine;
use chrono::TimeZone;
use rusqlite::Connection;

use super::testing::StubBridge;
use super::*;
use crate::db::{get_connection, init_db};

fn test_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = get_connection(&dir.path().join("test.db")).unwrap();
    init_db(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts (name, account_type) VALUES ('Operating', 'checking')",
        [],
    )
    .unwrap();
    (dir, conn)
}

/// Local noon on `day`, so the posted time lands on that day in any zone.
fn noon(day: &str) -> i64 {
    let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap();
    chrono::Local
        .from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
        .unwrap()
        .timestamp()
}

fn txn(id: &str, day: &str, amount: &str, description: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "posted": noon(day), "amount": amount, "description": description
    })
}

fn account_json(
    id: &str,
    balance: &str,
    transactions: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "org": { "domain": "harborvale.example", "name": "Harbor & Vale Bank", "sfin-url": "https://bridge.example/simplefin" },
        "id": id,
        "name": format!("Checking {id}"),
        "currency": "USD",
        "balance": balance,
        "balance-date": noon("2025-03-10"),
        "transactions": transactions,
    })
}

fn set(accounts: Vec<serde_json::Value>) -> AccountSet {
    serde_json::from_value(serde_json::json!({ "errors": [], "accounts": accounts })).unwrap()
}

fn link(conn: &Connection, set: &AccountSet, id: &str) {
    remember_accounts(conn, set).unwrap();
    map_account(conn, &format!("harborvale.example/{id}"), "Operating", true).unwrap();
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn a_setup_token_is_claimed_with_a_post_and_yields_the_access_url() {
    let access = "http://demo:s3cret@127.0.0.1:9/simplefin";
    let bridge = StubBridge::start(vec![(200, access.to_string())]);
    let token = base64::engine::general_purpose::STANDARD.encode(format!(
        "http://127.0.0.1:{}/simplefin/claim/abc",
        bridge.port
    ));
    let claimed = claim(&token).unwrap();
    assert_eq!(claimed.expose(), access);
    let seen = bridge.seen();
    assert_eq!(seen[0].method, "POST");
    assert_eq!(seen[0].target, "/simplefin/claim/abc");
}

#[test]
fn a_claimed_token_is_refused_with_a_warning() {
    let bridge = StubBridge::start(vec![(403, String::new())]);
    let token = base64::engine::general_purpose::STANDARD.encode(format!(
        "http://127.0.0.1:{}/simplefin/claim/abc",
        bridge.port
    ));
    let err = claim(&token).unwrap_err().to_string();
    assert!(err.contains("already been claimed"), "{err}");
}

#[test]
fn tokens_and_access_urls_off_loopback_must_be_https() {
    let token = base64::engine::general_purpose::STANDARD.encode("http://bridge.example/claim/abc");
    assert!(claim(&token).unwrap_err().to_string().contains("HTTPS"));
    assert!(AccessUrl::parse("http://u:p@bridge.example/simplefin").is_err());
    assert!(AccessUrl::parse("https://bridge.example/simplefin").is_err());
    assert!(AccessUrl::parse("https://u:p@bridge.example/simplefin").is_ok());
    assert!(claim("not base64 at all!").is_err());
}

#[test]
fn the_access_url_never_shows_in_debug_or_errors() {
    let url = AccessUrl::parse("https://demo:s3cret@127.0.0.1:1/simplefin").unwrap();
    assert!(!format!("{url:?}").contains("s3cret"));
    let err = fetch_accounts(&url, &AccountsQuery::default())
        .unwrap_err()
        .to_string();
    assert!(!err.contains("s3cret") && !err.contains("demo"), "{err}");
}

#[test]
fn accounts_are_read_with_basic_auth_and_the_requested_window() {
    let body =
        serde_json::json!({ "errors": [], "accounts": [account_json("1", "100.00", vec![])] });
    let bridge = StubBridge::start(vec![(200, body.to_string())]);
    let query = AccountsQuery {
        start: Some(1_700_000_000),
        end: None,
        balances_only: true,
    };
    let fetched = fetch_accounts(&bridge.access_url(), &query).unwrap();
    assert_eq!(fetched.accounts.len(), 1);
    let seen = bridge.seen();
    assert_eq!(seen[0].method, "GET");
    assert_eq!(
        seen[0].target,
        "/simplefin/accounts?start-date=1700000000&balances-only=1"
    );
    // demo:s3cret
    assert_eq!(
        seen[0].authorization.as_deref(),
        Some("Basic ZGVtbzpzM2NyZXQ=")
    );
}

#[test]
fn revoked_and_unpaid_access_say_what_to_do() {
    let bridge = StubBridge::start(vec![(403, String::new()), (402, String::new())]);
    let url = bridge.access_url();
    let revoked = fetch_accounts(&url, &AccountsQuery::default())
        .unwrap_err()
        .to_string();
    assert!(revoked.contains("nigel sync setup"), "{revoked}");
    let unpaid = fetch_accounts(&url, &AccountsQuery::default())
        .unwrap_err()
        .to_string();
    assert!(unpaid.contains("payment"), "{unpaid}");
}

#[test]
fn unmapped_accounts_are_listed_and_skipped() {
    let (_dir, conn) = test_db();
    let feed = set(vec![
        account_json(
            "1",
            "100.00",
            vec![txn("a", "2025-03-01", "-5.00", "GLOBEX DELI")],
        ),
        account_json(
            "2",
            "50.00",
            vec![txn("b", "2025-03-01", "-9.00", "INITECH")],
        ),
    ]);
    link(&conn, &feed, "1");
    let report = apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    assert_eq!(report.accounts.len(), 1);
    assert_eq!(report.unmapped, vec!["Harbor & Vale Bank — Checking 2"]);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 1);
    let listed = list_accounts(&conn).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].account.as_deref(), Some("Operating"));
    assert_eq!(listed[1].account, None);
}

#[test]
fn a_sync_imports_once_by_id_even_when_the_description_changes() {
    let (_dir, conn) = test_db();
    let first = set(vec![account_json(
        "1",
        "100.00",
        vec![
            txn("a", "2025-03-01", "-40.00", "CEDAR SYS PENDING"),
            txn("b", "2025-03-02", "1200.00", "ACME CORP PAYMENT"),
            txn("p", "2025-03-03", "-3.00", "STILL PENDING"),
        ],
    )]);
    let mut first = first;
    first.accounts[0].transactions[2].pending = true;
    link(&conn, &first, "1");
    let report = apply(&conn, &first, None, "2025-03-05", false).unwrap();
    assert_eq!(
        (report.accounts[0].imported, report.accounts[0].skipped),
        (2, 0)
    );
    assert!(report.accounts[0].import_id.is_some());

    let second = set(vec![account_json(
        "1",
        "100.00",
        vec![
            txn("a", "2025-03-01", "-40.00", "CEDAR SYSTEMS INC"),
            txn("c", "2025-03-06", "-12.00", "JUNIPER LABS"),
        ],
    )]);
    let report = apply(&conn, &second, None, "2025-03-10", false).unwrap();
    assert_eq!(
        (report.accounts[0].imported, report.accounts[0].skipped),
        (1, 1)
    );
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 3);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM imports"), 2);
}

#[test]
fn a_dry_run_reports_the_sync_and_writes_nothing() {
    let (_dir, conn) = test_db();
    let feed = set(vec![account_json(
        "1",
        "100.00",
        vec![txn("a", "2025-03-01", "-40.00", "CEDAR SYSTEMS")],
    )]);
    link(&conn, &feed, "1");
    let preview = apply(&conn, &feed, None, "2025-03-10", true).unwrap();
    assert_eq!(preview.accounts[0].imported, 1);
    assert_eq!(preview.accounts[0].import_id, None);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM account_balances"), 0);
    let real = apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    assert_eq!(real.accounts[0].imported, preview.accounts[0].imported);
}

#[test]
fn nothing_new_writes_no_import_row() {
    let (_dir, conn) = test_db();
    let feed = set(vec![account_json(
        "1",
        "100.00",
        vec![txn("a", "2025-03-01", "-40.00", "CEDAR SYSTEMS")],
    )]);
    link(&conn, &feed, "1");
    apply(&conn, &feed, None, "2025-03-02", false).unwrap();
    let again = apply(&conn, &feed, None, "2025-03-03", false).unwrap();
    assert_eq!(
        (again.accounts[0].imported, again.accounts[0].skipped),
        (0, 1)
    );
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM imports"), 1);
    let empty = set(vec![account_json("1", "100.00", vec![])]);
    apply(&conn, &empty, None, "2025-03-04", false).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM imports"), 1);
}

#[test]
fn a_first_sync_starts_after_the_last_csv_import() {
    let (_dir, conn) = test_db();
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount) \
         VALUES (1, '2025-03-04', 'GLOBEX DELI', -14.50)",
        [],
    )
    .unwrap();
    let feed = set(vec![account_json(
        "1",
        "100.00",
        vec![
            txn("a", "2025-03-04", "-14.50", "GLOBEX DELI #2231"),
            txn("b", "2025-03-05", "-8.00", "JUNIPER LABS CAFE"),
        ],
    )]);
    link(&conn, &feed, "1");
    let query = query_for(&conn, None).unwrap().unwrap();
    assert_eq!(
        query.start,
        Some(local_midnight(parse_day("2025-03-05").unwrap()))
    );
    let report = apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    assert_eq!(report.accounts[0].start.as_deref(), Some("2025-03-05"));
    assert_eq!(report.accounts[0].imported, 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 2);

    // A repeat sync re-reads a week back from the last sync day, but never
    // into the days the CSV covered: its rows have no id to match against.
    let query = query_for(&conn, None).unwrap().unwrap();
    assert_eq!(
        query.start,
        Some(local_midnight(parse_day("2025-03-05").unwrap()))
    );
    let again = apply(&conn, &feed, None, "2025-03-11", false).unwrap();
    assert_eq!(
        (again.accounts[0].imported, again.accounts[0].skipped),
        (0, 1)
    );
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 2);

    // With only feed rows in the account, the overlap reaches back a week from
    // the day the bank last reported (the feed's balance date, 2025-03-10).
    conn.execute("DELETE FROM transactions WHERE external_id IS NULL", [])
        .unwrap();
    let query = query_for(&conn, None).unwrap().unwrap();
    assert_eq!(
        query.start,
        Some(local_midnight(parse_day("2025-03-03").unwrap()))
    );
    // --since overrides both.
    let since = parse_day("2025-01-01").unwrap();
    let query = query_for(&conn, Some(since)).unwrap().unwrap();
    assert_eq!(query.start, Some(local_midnight(since)));
}

/// The feed for account `id` as the bridge last refreshed it on `day`.
fn reported_on(id: &str, day: Option<&str>, transactions: Vec<serde_json::Value>) -> AccountSet {
    let mut account = account_json(id, "100.00", transactions);
    account["balance-date"] = serde_json::json!(day.map_or(0, noon));
    set(vec![account])
}

fn last_synced(conn: &Connection) -> Option<String> {
    list_accounts(conn).unwrap()[0].last_synced.clone()
}

#[test]
fn an_outage_does_not_move_the_sync_window_past_unfetched_days() {
    let (_dir, conn) = test_db();
    let fresh = reported_on(
        "1",
        Some("2025-03-10"),
        vec![txn("a", "2025-03-09", "-5.00", "CAFE")],
    );
    link(&conn, &fresh, "1");
    apply(&conn, &fresh, None, "2025-03-10", false).unwrap();
    assert_eq!(last_synced(&conn).as_deref(), Some("2025-03-10"));

    // The bank connection breaks: ten days of syncs come back stale, the
    // bridge's balance date stuck where the bank last answered.
    let mut stale = reported_on("1", Some("2025-03-10"), vec![]);
    stale
        .errors
        .push("Harbor & Vale Bank needs attention".to_string());
    for today in ["2025-03-15", "2025-03-20"] {
        apply(&conn, &stale, None, today, false).unwrap();
    }
    assert_eq!(last_synced(&conn).as_deref(), Some("2025-03-10"));

    // So the first sync after the repair still reaches back over the outage.
    let query = query_for(&conn, None).unwrap().unwrap();
    assert_eq!(
        query.start,
        Some(local_midnight(parse_day("2025-03-03").unwrap()))
    );
}

#[test]
fn the_sync_day_never_runs_ahead_of_today_or_backwards() {
    let (_dir, conn) = test_db();
    let ahead = reported_on("1", Some("2025-03-12"), vec![]);
    link(&conn, &ahead, "1");
    apply(&conn, &ahead, None, "2025-03-10", false).unwrap();
    assert_eq!(last_synced(&conn).as_deref(), Some("2025-03-10"));

    let older = reported_on("1", Some("2025-03-08"), vec![]);
    apply(&conn, &older, None, "2025-03-11", false).unwrap();
    assert_eq!(last_synced(&conn).as_deref(), Some("2025-03-10"));

    let undated = reported_on("1", None, vec![]);
    apply(&conn, &undated, None, "2025-03-12", false).unwrap();
    assert_eq!(last_synced(&conn).as_deref(), Some("2025-03-10"));
}

#[test]
fn a_first_sync_without_a_balance_date_records_no_sync_day() {
    let (_dir, conn) = test_db();
    let undated = reported_on("1", None, vec![txn("a", "2025-03-09", "-5.00", "CAFE")]);
    link(&conn, &undated, "1");
    apply(&conn, &undated, None, "2025-03-10", false).unwrap();
    assert_eq!(last_synced(&conn), None);
}

#[test]
fn nothing_mapped_means_nothing_to_ask_for() {
    let (_dir, conn) = test_db();
    assert!(query_for(&conn, None).unwrap().is_none());
}

#[test]
fn the_reported_balance_is_stored_and_shown_beside_the_computed_one() {
    let (_dir, conn) = test_db();
    let feed = set(vec![account_json(
        "1",
        "1234.56",
        vec![txn("a", "2025-03-01", "-40.00", "CEDAR SYSTEMS")],
    )]);
    link(&conn, &feed, "1");
    let report = apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    let expected = ReportedBalance {
        balance: 1234.56,
        as_of: "2025-03-10".into(),
    };
    assert_eq!(report.accounts[0].balance.as_ref(), Some(&expected));

    let balance = crate::reports::get_balance(&conn).unwrap();
    let operating = balance
        .accounts
        .iter()
        .find(|a| a.name == "Operating")
        .unwrap();
    assert_eq!(operating.balance, -40.0);
    assert_eq!(operating.reported.as_ref(), Some(&expected));
    let text = crate::reports::text::format_balance(&balance);
    assert!(text.contains("Bank reported"), "{text}");
    assert!(text.contains("as of 2025-03-10"), "{text}");
}

#[test]
fn bad_rows_are_rejects_and_bridge_errors_are_sanitized_warnings() {
    let (_dir, conn) = test_db();
    let mut feed = set(vec![account_json(
        "1",
        "100.00",
        vec![
            txn("a", "2025-03-01", "-40.00", "CEDAR SYSTEMS"),
            txn("b", "2025-03-02", "twelve", "INITECH"),
        ],
    )]);
    feed.errors = vec!["Connection to\u{7} Harbor & Vale needs attention".into()];
    link(&conn, &feed, "1");
    let report = apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    assert_eq!(report.accounts[0].malformed, 1);
    assert_eq!(
        report.warnings,
        vec!["Connection to Harbor & Vale needs attention"]
    );
    let import_id = report.accounts[0].import_id.unwrap();
    let reason: String = conn
        .query_row(
            "SELECT reason FROM import_rejects WHERE import_id = ?1",
            [import_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(reason.contains("twelve"), "{reason}");
}

#[test]
fn one_account_takes_several_feeds_but_one_balance_feed() {
    let (_dir, conn) = test_db();
    let feed = set(vec![
        account_json("1", "1.00", vec![]),
        account_json("2", "1.00", vec![]),
        account_json("3", "1.00", vec![]),
    ]);
    link(&conn, &feed, "1");

    let err = map_account(&conn, "harborvale.example/2", "Operating", true)
        .unwrap_err()
        .to_string();
    assert!(err.contains("harborvale.example/1"), "{err}");
    assert!(err.contains("--no-balance"), "{err}");

    map_account(&conn, "harborvale.example/2", "Operating", false).unwrap();
    map_account(&conn, "harborvale.example/3", "Operating", false).unwrap();
    let linked = list_accounts(&conn).unwrap();
    let flags: Vec<(Option<&str>, bool)> = linked
        .iter()
        .map(|a| (a.account.as_deref(), a.supplies_balance))
        .collect();
    assert_eq!(
        flags,
        vec![
            (Some("Operating"), true),
            (Some("Operating"), false),
            (Some("Operating"), false),
        ]
    );
    let json = serde_json::to_value(&linked[1]).unwrap();
    assert_eq!(json["suppliesBalance"], false);

    // Re-mapping the balance feed as transactions-only frees the balance for
    // another, and re-mapping a key to the same account keeps its sync day.
    conn.execute(
        "UPDATE simplefin_accounts SET last_synced = '2025-03-01'",
        [],
    )
    .unwrap();
    map_account(&conn, "harborvale.example/1", "Operating", false).unwrap();
    map_account(&conn, "harborvale.example/2", "Operating", true).unwrap();
    let linked = list_accounts(&conn).unwrap();
    assert!(!linked[0].supplies_balance && linked[1].supplies_balance);
    assert!(linked
        .iter()
        .all(|a| a.last_synced.as_deref() == Some("2025-03-01")));

    // Unmapping the balance feed leaves the others transactions-only, and an
    // unmapped feed supplies nothing.
    unmap_account(&conn, "harborvale.example/2").unwrap();
    let linked = list_accounts(&conn).unwrap();
    assert!(linked.iter().all(|a| !a.supplies_balance));
    map_account(&conn, "harborvale.example/3", "Operating", true).unwrap();

    assert!(map_account(&conn, "nope/9", "Operating", true).is_err());
    assert!(map_account(&conn, "harborvale.example/1", "Nope", true).is_err());
}

/// A card program: the parent feed carries payments and the card's total
/// balance, the cardholder feed carries that person's charges.
fn card_program(parent: Vec<serde_json::Value>, cardholder: Vec<serde_json::Value>) -> AccountSet {
    set(vec![
        account_json("parent", "-500.00", parent),
        account_json("holder", "-120.00", cardholder),
    ])
}

fn link_card_program(conn: &Connection, feed: &AccountSet) {
    remember_accounts(conn, feed).unwrap();
    map_account(conn, "harborvale.example/parent", "Operating", true).unwrap();
    map_account(conn, "harborvale.example/holder", "Operating", false).unwrap();
}

#[test]
fn two_feeds_sync_into_one_account_with_one_balance() {
    let (_dir, conn) = test_db();
    // The same id, date, amount and description on both feeds: the external
    // id carries the feed's key, so neither swallows the other's row.
    let feed = card_program(
        vec![
            txn("1", "2025-03-02", "300.00", "PAYMENT THANK YOU"),
            txn("2", "2025-03-03", "-9.00", "INITECH"),
        ],
        vec![
            txn("1", "2025-03-01", "-45.00", "CEDAR SYSTEMS"),
            txn("2", "2025-03-03", "-9.00", "INITECH"),
        ],
    );
    link_card_program(&conn, &feed);

    let report = apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    assert_eq!(report.accounts.len(), 2);
    assert!(report.accounts.iter().all(|a| a.account == "Operating"));
    assert_eq!(report.imported(), 4);
    let parent = &report.accounts[0];
    let holder = &report.accounts[1];
    assert_eq!(parent.balance.as_ref().map(|b| b.balance), Some(-500.0));
    assert_eq!(holder.balance, None);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 4);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM imports"), 2);

    let stored: Vec<(f64, String)> = conn
        .prepare("SELECT balance, source FROM account_balances")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    assert_eq!(stored, vec![(-500.0, BALANCE_SOURCE.to_string())]);
    let reported = latest_reported_balances(&conn).unwrap();
    assert_eq!(reported[&1].balance, -500.0);

    // Re-reading every row holds each by its own feed's id.
    let since = parse_day("2025-03-01").unwrap();
    let again = apply(&conn, &feed, Some(since), "2025-03-11", false).unwrap();
    assert_eq!(again.imported(), 0);
    assert_eq!(again.accounts.iter().map(|a| a.skipped).sum::<usize>(), 4);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 4);
}

#[test]
fn a_feed_added_beside_another_starts_after_its_own_rows_not_the_others() {
    let (_dir, conn) = test_db();
    // A statement import, then the parent feed syncing past it.
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount) \
         VALUES (1, '2025-02-20', 'GLOBEX DELI', -14.50)",
        [],
    )
    .unwrap();
    let parent_only = set(vec![account_json(
        "parent",
        "-500.00",
        vec![txn("p1", "2025-03-08", "300.00", "PAYMENT THANK YOU")],
    )]);
    remember_accounts(&conn, &parent_only).unwrap();
    map_account(&conn, "harborvale.example/parent", "Operating", true).unwrap();
    apply(&conn, &parent_only, None, "2025-03-10", false).unwrap();

    // A key holding `_` must not let LIKE treat the other feed's ids as its
    // own; the cardholder feed's key is a near miss of the parent's.
    let feed = set(vec![
        account_json(
            "parent",
            "-500.00",
            vec![txn("p1", "2025-03-08", "300.00", "PAYMENT THANK YOU")],
        ),
        account_json(
            "paren_",
            "-120.00",
            vec![
                txn("h0", "2025-02-20", "-3.00", "ACME"),
                txn("h1", "2025-02-25", "-45.00", "CEDAR SYSTEMS"),
                txn("h2", "2025-03-09", "-12.00", "JUNIPER LABS"),
            ],
        ),
    ]);
    remember_accounts(&conn, &feed).unwrap();
    map_account(&conn, "harborvale.example/paren_", "Operating", false).unwrap();

    // The cardholder's first sync starts after the CSV row, not after the
    // parent feed's newer payment, so its earlier charge is not skipped.
    let report = apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    let holder = report
        .accounts
        .iter()
        .find(|a| a.key == "harborvale.example/paren_")
        .unwrap();
    assert_eq!(holder.start.as_deref(), Some("2025-02-21"));
    assert_eq!(holder.imported, 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM transactions"), 4);
}

#[test]
fn remapping_starts_the_new_account_after_its_own_history() {
    let (_dir, conn) = test_db();
    conn.execute(
        "INSERT INTO accounts (name, account_type) VALUES ('Reserve', 'checking')",
        [],
    )
    .unwrap();
    let feed = set(vec![account_json(
        "1",
        "1.00",
        vec![txn("a", "2025-03-01", "-1.00", "GLOBEX DELI")],
    )]);
    link(&conn, &feed, "1");
    apply(&conn, &feed, None, "2025-03-10", false).unwrap();
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount) \
         VALUES (2, '2025-03-20', 'INITECH', -5.00)",
        [],
    )
    .unwrap();
    map_account(&conn, "harborvale.example/1", "Reserve", true).unwrap();
    assert_eq!(list_accounts(&conn).unwrap()[0].last_synced, None);
    let query = query_for(&conn, None).unwrap().unwrap();
    assert_eq!(
        query.start,
        Some(local_midnight(parse_day("2025-03-21").unwrap()))
    );

    unmap_account(&conn, "harborvale.example/1").unwrap();
    assert_eq!(list_accounts(&conn).unwrap()[0].last_synced, None);
}

#[test]
fn a_reject_with_nothing_new_writes_no_import() {
    let (_dir, conn) = test_db();
    let good = txn("a", "2025-03-01", "-40.00", "CEDAR SYSTEMS");
    let feed = set(vec![account_json("1", "1.00", vec![good.clone()])]);
    link(&conn, &feed, "1");
    apply(&conn, &feed, None, "2025-03-02", false).unwrap();
    let with_bad = set(vec![account_json(
        "1",
        "1.00",
        vec![good, txn("b", "2025-03-02", "twelve", "INITECH")],
    )]);
    let report = apply(&conn, &with_bad, None, "2025-03-03", false).unwrap();
    assert_eq!(report.accounts[0].malformed, 1);
    assert_eq!(report.accounts[0].import_id, None);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM imports"), 1);
}
