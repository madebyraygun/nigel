//! The checks `docs/runbooks/verify-invoice-recurrence.md` asks an operator to
//! make by hand, made by the suite instead.
//!
//! Everything here drives the real binary, because what the runbook tells the
//! operator to trust is the *printed* line — the number, the money, the period
//! and the word `draft` — not the row behind it. The core already proves the
//! arithmetic; these prove the operator sees it.
//!
//! **On dates.** `nigel` reads the clock once, at dispatch (`cli::today()`),
//! and there is no override. So no test here may hardcode a period: every
//! schedule starts at a date derived from today, and the expectations are
//! derived from the same arithmetic. A fixed `--start` would quietly bill one
//! more period every month, which is the bug that rotted the runbook's own
//! step 4.

use chrono::{Datelike, Duration, Local, NaiveDate};
use predicates::prelude::*;

mod common;
use common::{TestEnv, TEST_TIMEOUT};

/// Today, as the binary will read it moments from now.
fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The first of the month `months` before this one.
///
/// Day 1 is the only anchor that exists in every month, so a schedule started
/// here bills exactly one period per month with no clamping — which is what
/// makes the generated count a fixed number rather than a function of today.
fn first_of_month_back(months: u32) -> NaiveDate {
    let mut year = today().year();
    let mut month = today().month() as i32 - months as i32;
    while month < 1 {
        month += 12;
        year -= 1;
    }
    NaiveDate::from_ymd_opt(year, month as u32, 1).expect("the first of a month always exists")
}

/// The last day of February in `year` — 29 in a leap year, 28 otherwise.
///
/// Calendar arithmetic, deliberately not a call into the production clamp: a
/// test that asked `clamp_day` what it expected would assert nothing.
fn end_of_february(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 2, 29)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 2, 28).expect("February has a 28th"))
}

fn iso(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Runbook step 0: a client and one invoice, #1248, in a lab with no
/// credentials anywhere in reach.
fn lab(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args([
            "client",
            "add",
            "Cedar Systems",
            "--email",
            "cedar@example.test",
        ])
        .assert()
        .success();
    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-01-15",
            "--due",
            "2026-02-14",
            "--item",
            "Retainer:1:2400",
            "--item",
            "Hosting:2:45",
            "--notes",
            "Thanks",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created draft invoice #1248"));
}

/// Step 1. The runbook quotes this line in full, so the amount and currency are
/// part of what it promises — not just the two numbers.
#[test]
fn duplicating_names_both_numbers_and_the_amount_the_copy_carries() {
    let env = TestEnv::new();
    lab(&env);

    env.cmd()
        .args(["invoice", "duplicate", "1248", "--issue", "2026-03-01"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Duplicated invoice #1248 as draft #1249 for 2490.00 USD",
        ));
}

/// Step 1's sharp edge: the copy inherits the *term*, not the date. A copy that
/// took the literal due date would arrive already overdue, which is the whole
/// reason the behaviour exists. The core pins the offset; this pins what the
/// operator reads back off `invoice show`.
#[test]
fn a_duplicate_carries_the_issue_to_due_term_rather_than_the_literal_due_date() {
    let env = TestEnv::new();
    lab(&env);

    env.cmd()
        .args(["invoice", "duplicate", "1248", "--issue", "2026-03-01"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    // 2026-01-15 to 2026-02-14 is thirty days, so 2026-03-01 comes due 03-31.
    env.cmd()
        .args(["invoice", "show", "1249"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Issued:   2026-03-01"))
        .stdout(predicate::str::contains("Due:      2026-03-31"));
}

/// Step 2 says `--item` and `--from` are mutually exclusive. Clap enforces it;
/// nothing proved the refusal reaches the operator.
#[test]
fn a_schedule_refuses_both_an_invoice_to_copy_and_items_to_type() {
    let env = TestEnv::new();
    lab(&env);

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            "2026-06-15",
            "--from",
            "1248",
            "--item",
            "Retainer:1:2400",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure();
}

/// Step 2 claims, flatly, that leaving `--net-days` off means generated
/// invoices carry no due date. That is true when the items were typed, and
/// **false when the schedule was seeded with `--from`**: the source invoice's
/// own issue-to-due term comes across with the rest of its shape.
///
/// This test pins the behaviour, which is the useful one — a schedule seeded
/// from a Net-30 invoice should keep billing Net-30. The runbook's sentence is
/// what needs correcting.
#[test]
fn seeding_from_an_invoice_carries_its_term_even_with_no_net_days_given() {
    let env = TestEnv::new();
    lab(&env);

    let start = first_of_month_back(1);
    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(start),
            "--from",
            "1248",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    // #1248 ran 2026-01-15 to 2026-02-14: thirty days, carried onto the period.
    let due = iso(start + Duration::days(30));
    env.cmd()
        .args(["invoice", "show", "1249"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Due:      {due}")));
}

/// Step 3, the one the runbook says surprises people: a schedule does not start
/// from today. Started three months back on the first, it bills four periods at
/// once — this month's included — each invoice dated its own period rather than
/// the day the run happened.
#[test]
fn a_run_bills_every_missed_period_dated_by_its_own_period() {
    let env = TestEnv::new();
    lab(&env);

    let start = first_of_month_back(3);
    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(start),
            "--net-days",
            "30",
            "--item",
            "Retainer:1:2400",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let expected: Vec<String> = (0..4).map(|n| iso(first_of_month_back(3 - n))).collect();

    let run = env
        .cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated 4 invoice(s)."));

    let printed = String::from_utf8(run.get_output().stdout.clone()).expect("stdout is utf-8");
    for (offset, period) in expected.iter().enumerate() {
        let number = 1249 + offset;
        assert!(
            printed.contains(&format!(
                "#{number}  Cedar Systems  $2,400.00  {period}  draft"
            )),
            "the run should have printed #{number} for {period}, got:\n{printed}"
        );
    }

    // Nothing is billed twice, which is what makes the command safe under cron.
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated 0 invoice(s)."));
}

/// Step 4. The billing day is remembered, not recomputed from the last invoice,
/// so February clamps and **March returns to the 31st**. A schedule that walked
/// forward from what it last generated would stick on the 28th for the rest of
/// the year, quietly moving a client's billing date.
///
/// Only the first four periods are asserted: the schedule starts two years back
/// so the run also bills everything since, and that tail grows every month.
#[test]
fn a_month_end_anchor_clamps_february_and_then_returns_to_the_month_end() {
    let env = TestEnv::new();
    lab(&env);

    let year = today().year() - 2;
    let january = NaiveDate::from_ymd_opt(year, 1, 31).expect("January has a 31st");
    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(january),
            "--net-days",
            "15",
            "--item",
            "Support:1:500",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let run = env
        .cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
    let printed = String::from_utf8(run.get_output().stdout.clone()).expect("stdout is utf-8");

    let opening = [
        january,
        end_of_february(year),
        NaiveDate::from_ymd_opt(year, 3, 31).expect("March has a 31st"),
        NaiveDate::from_ymd_opt(year, 4, 30).expect("April has a 30th"),
    ];
    for (offset, period) in opening.iter().enumerate() {
        let number = 1249 + offset;
        assert!(
            printed.contains(&format!(
                "#{number}  Cedar Systems  $500.00  {}  draft",
                iso(*period)
            )),
            "period {} should have billed as #{number}, got:\n{printed}",
            iso(*period)
        );
    }
}

/// Step 4's closing sentence — `--anchor-day` sets a billing day different from
/// the start date's — is true of every period **but the first**.
///
/// `--start` is the first period's issue date and is billed verbatim; the
/// anchor is what the cycle advances on from there. So a schedule started on
/// the 1st with `--anchor-day 15` bills the 1st once and the 15th ever after.
/// Worth pinning because it reads like a contradiction until you know that
/// `--start` names a period rather than a bound.
#[test]
fn an_anchor_day_governs_every_period_after_the_first() {
    let env = TestEnv::new();
    lab(&env);

    let start = first_of_month_back(2);
    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(start),
            "--anchor-day",
            "15",
            "--item",
            "Support:1:500",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let run = env
        .cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
    let printed = String::from_utf8(run.get_output().stdout.clone()).expect("stdout is utf-8");

    assert!(
        printed.contains(&iso(start)),
        "the first period is the start date itself, got:\n{printed}"
    );

    let second = first_of_month_back(1)
        .with_day(15)
        .expect("every month has a 15th");
    assert!(
        printed.contains(&iso(second)),
        "every period after the first should ride the anchor day, got:\n{printed}"
    );
}

/// Step 5. A schedule keeps its generated invoices as the record of what each
/// period billed, so delete is refused — in the exact words the runbook quotes,
/// with the pointer at `void` and a non-zero exit.
#[test]
fn a_schedule_generated_invoice_refuses_deletion_and_points_at_void() {
    let env = TestEnv::new();
    lab(&env);

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(first_of_month_back(1)),
            "--item",
            "Retainer:1:2400",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "delete", "1249", "--yes"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot delete: invoice was generated by schedule 1, which keeps it as the record of what that period billed — void it instead",
        ))
        .stderr(predicate::str::contains(
            "Run `nigel invoice void 1249` to cancel it instead.",
        ));

    // The contrast the runbook draws: a hand-made draft still deletes.
    env.cmd()
        .args(["invoice", "delete", "1248", "--yes"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
}

/// Step 6. `list` is the working view and hides what is not running; `--all` is
/// the archive. The core proves the scope filter — this proves the table the
/// runbook tells the operator to read.
#[test]
fn the_default_schedule_list_hides_a_paused_schedule_and_all_shows_it() {
    let env = TestEnv::new();
    lab(&env);

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(first_of_month_back(1)),
            "--item",
            "Retainer:1:2400",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "schedule", "pause", "1"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "schedule", "list"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("paused").not());

    env.cmd()
        .args(["invoice", "schedule", "list", "--all"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("paused"));
}

/// Step 7. A rate change must not rewrite bills a client has already seen: the
/// invoices already generated keep the old figure, and only the next period
/// takes the new one.
#[test]
fn editing_a_schedule_leaves_billed_invoices_alone_and_changes_only_the_next() {
    let env = TestEnv::new();
    lab(&env);

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(first_of_month_back(1)),
            "--item",
            "Retainer:1:2400",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
    env.cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "edit",
            "1",
            "--item",
            "Retainer:1:2600",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Updated schedule 1. Future invoices use the new figures.",
        ));

    env.cmd()
        .args(["invoice", "show", "1249"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("$2,400.00"))
        .stdout(predicate::str::contains("$2,600.00").not());

    env.cmd()
        .args(["invoice", "schedule", "list"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("$2,600.00"));
}

/// Step 8, and the two facts the runbook says matter for automation: an
/// autosend schedule that cannot send still **writes the draft**, and the run
/// **exits non-zero** so cron reports it rather than failing silently.
#[test]
fn an_autosend_run_with_no_credentials_still_drafts_and_exits_non_zero() {
    let env = TestEnv::new();
    lab(&env);

    env.cmd()
        .args([
            "invoice",
            "schedule",
            "add",
            "--client",
            "1",
            "--cadence",
            "monthly",
            "--start",
            &iso(first_of_month_back(1)),
            "--item",
            "Audit:1:1500",
            "--autosend",
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "schedule", "run"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "draft — not sent: sending is not configured on this installation",
        ))
        .stderr(predicate::str::contains(
            "Some invoices were not sent. See the lines above.",
        ));

    // The draft is there regardless: nothing was thrown away because the mail
    // could not go out.
    let drafted: i64 = env
        .db()
        .query_row(
            "SELECT COUNT(*) FROM invoices WHERE status = 'draft' AND number >= 1249",
            [],
            |row| row.get(0),
        )
        .expect("count the generated drafts");
    assert!(drafted > 0, "an unsendable autosend run must still draft");
}
