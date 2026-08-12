//! What both client-facing documents say about money and about the client,
//! decided once.
//!
//! The page and the PDF are rendered by two different renderers from the same
//! row, and the only way they can be trusted to agree is for the decision — the
//! figures, and which of them appear at all — to live above both of them.

use crate::invoicing::invoices::{is_settled, CENT_SLACK};
use crate::models::Invoice;

/// How many lines of a client's billing address a document will draw.
///
/// The PDF draws one row per line at a fixed offset with no page-break logic,
/// so an address pasted out of a spreadsheet would run off the bottom margin.
/// Six is a generous postal address anywhere; past that the block is telling
/// the reader something other than where to send a cheque.
pub const MAX_ADDRESS_LINES: usize = 6;

/// What stands in for the lines that did not fit.
///
/// Three ASCII dots rather than an ellipsis character: the PDF draws with the
/// built-in Helvetica, whose WinAnsi encoding does not carry `U+2026` through
/// to a reader intact, and the whole point of clamping in one place is that
/// both documents show the same thing.
pub const ADDRESS_TRUNCATED: &str = "...";

/// One line of the money block, in the order both documents print them.
pub struct MoneyLine {
    pub label: &'static str,
    pub amount: f64,
    /// The line a reader's eye should land on: the total, the balance, and the
    /// credit when there is one.
    pub emphasis: bool,
    /// A row the payment block introduced.
    ///
    /// These are new to *both* documents, so both render them the same way —
    /// `USD 60.00` — rather than one of them inheriting the PDF's older
    /// `$`-prefixed style, which cannot say which currency it means. The
    /// pre-existing Subtotal/Tax/Total rows keep each document's own
    /// convention; reconciling those is TASK-87's.
    pub payment_row: bool,
}

/// The figures both documents draw, and the rules about which of them appear.
pub struct MoneySummary {
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
    pub paid: f64,
    /// What is still owed. Never negative: an overpayment is a `credit`.
    pub balance: f64,
    /// What was paid beyond the total, when anything was. Zero otherwise.
    pub credit: f64,
}

impl MoneySummary {
    pub fn of(invoice: &Invoice, paid: f64) -> Self {
        // The same question `refresh_status` asks, through the same function:
        // a document that disagreed would print a balance under a status that
        // says `paid`.
        let settled = is_settled(invoice.total, paid);
        let over = paid - invoice.total;
        Self {
            subtotal: invoice.subtotal,
            tax: invoice.tax,
            total: invoice.total,
            paid,
            balance: if settled { 0.0 } else { invoice.total - paid },
            credit: if over > CENT_SLACK { over } else { 0.0 },
        }
    }

    /// A line appears when it has something to say.
    ///
    /// The subtotal/tax rule is the one the PDF has always applied: a one-line
    /// invoice with no tax prints one figure. Paid and Balance appear together,
    /// because a Paid row with no balance beside it leaves the reader to do the
    /// subtraction. Credit appears only when someone has paid too much, which
    /// is a fact about money owed the other way and never a negative balance.
    pub fn lines(&self) -> Vec<MoneyLine> {
        let mut lines = Vec::with_capacity(6);
        let mut push = |label, amount, emphasis, payment_row| {
            lines.push(MoneyLine {
                label,
                amount,
                emphasis,
                payment_row,
            })
        };
        if self.tax != 0.0 {
            push("Subtotal", self.subtotal, false, false);
            push("Tax", self.tax, false, false);
        }
        push("Total", self.total, true, false);
        if self.paid > 0.0 {
            push("Paid", self.paid, false, true);
            push("Balance due", self.balance, true, true);
            if self.credit > 0.0 {
                push("Credit", self.credit, true, true);
            }
        }
        lines
    }
}

/// A billing address as the lines it was typed as, blank ones dropped and the
/// block clamped to what a document can draw.
///
/// Both documents draw one line per row, so an address entered over two lines
/// stays two lines and an address entered with a trailing newline grows no
/// empty row. Beyond `MAX_ADDRESS_LINES` the block is cut and the cut is shown:
/// the PDF has no page-break logic under this loop, and silently dropping the
/// rest would leave the two documents saying different things.
pub fn address_lines(address: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = address
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(MAX_ADDRESS_LINES + 1)
        .collect();
    if lines.len() > MAX_ADDRESS_LINES {
        lines.truncate(MAX_ADDRESS_LINES - 1);
        lines.push(ADDRESS_TRUNCATED);
    }
    lines
}

/// The client's email as a document should print it, or nothing.
///
/// Blank-is-absent is a rule about the document, not about either renderer, so
/// it lives here beside `address_lines` — otherwise a client whose email is a
/// single space is a stray `<br>` on one document and a drawn empty row on the
/// other.
pub fn email_line(email: Option<&str>) -> Option<&str> {
    email.map(str::trim).filter(|e| !e.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Invoice;

    fn invoice(total: f64, tax: f64) -> Invoice {
        Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: None,
            status: "sent".into(),
            currency: "USD".into(),
            subtotal: total - tax,
            tax,
            total,
            notes: None,
            terms: None,
            token: "t".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: None,
            voided_at: None,
        }
    }

    fn labels(lines: &[MoneyLine]) -> Vec<&'static str> {
        lines.iter().map(|l| l.label).collect()
    }

    #[test]
    fn an_untaxed_unpaid_invoice_prints_one_line() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 0.0).lines();
        assert_eq!(labels(&lines), vec!["Total"]);
        assert!(lines[0].emphasis);
    }

    #[test]
    fn tax_brings_the_subtotal_with_it() {
        let lines = MoneySummary::of(&invoice(108.25, 8.25), 0.0).lines();
        assert_eq!(labels(&lines), vec!["Subtotal", "Tax", "Total"]);
        assert_eq!(lines[0].amount, 100.0);
        assert_eq!(lines[1].amount, 8.25);
        assert!(!lines[0].emphasis && !lines[1].emphasis);
    }

    #[test]
    fn a_payment_brings_paid_and_balance() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 40.0).lines();
        assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
        assert_eq!(lines[2].amount, 60.0);
        assert!(lines[2].emphasis, "the balance is what a client looks for");
    }

    #[test]
    fn a_settled_invoice_shows_a_zero_balance_rather_than_hiding_it() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 100.0).lines();
        assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
        assert_eq!(lines[2].amount, 0.0);
    }

    /// `f64` subtraction leaves a sliver behind; a document must never print
    /// `-0.00` next to "Balance due".
    #[test]
    fn a_balance_within_half_a_cent_of_zero_is_zero() {
        let summary = MoneySummary::of(&invoice(100.0, 0.0), 100.001);
        assert_eq!(summary.balance, 0.0);
        assert!(!summary.balance.is_sign_negative());
    }

    /// An overpaid invoice owes nothing; the excess is money going the other
    /// way, and a negative "Balance due" is not what that is.
    #[test]
    fn an_overpayment_reads_as_a_credit_rather_than_a_negative_balance() {
        let summary = MoneySummary::of(&invoice(100.0, 0.0), 130.0);
        let lines = summary.lines();

        assert_eq!(
            labels(&lines),
            vec!["Total", "Paid", "Balance due", "Credit"]
        );
        let due = lines.iter().find(|l| l.label == "Balance due").unwrap();
        assert_eq!(due.amount, 0.0);
        assert!(!due.amount.is_sign_negative(), "never a negative due");
        let credit = lines.iter().find(|l| l.label == "Credit").unwrap();
        assert_eq!(credit.amount, 30.0);
        assert_eq!(summary.credit, 30.0);
    }

    #[test]
    fn an_invoice_paid_to_the_penny_has_no_credit_line() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 100.0).lines();
        assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
    }

    /// The document and `refresh_status` must answer "is this settled?" the same
    /// way, or a page whose status says `paid` prints a balance under it.
    #[test]
    fn the_settled_test_is_exactly_the_one_refresh_status_uses() {
        // Paid to within exactly half a cent: `is_settled` is inclusive at the
        // edge, so this invoice is `paid` and its document must agree.
        let total = 100.0;
        let paid = total - CENT_SLACK;
        assert!(
            crate::invoicing::invoices::is_settled(total, paid),
            "the fixture has to sit on the boundary"
        );

        let summary = MoneySummary::of(&invoice(total, 0.0), paid);
        assert_eq!(summary.balance, 0.0);
        let lines = summary.lines();
        let due = lines.iter().find(|l| l.label == "Balance due").unwrap();
        assert_eq!(
            format!("{:.2}", due.amount),
            "0.00",
            "a settled invoice may not print a balance"
        );
    }

    /// The rows the payment block introduced are new to both documents, so both
    /// render them the same way rather than one inheriting an older convention.
    #[test]
    fn the_payment_rows_are_flagged_for_identical_rendering() {
        let lines = MoneySummary::of(&invoice(108.25, 8.25), 130.0).lines();
        for line in &lines {
            let expected = matches!(line.label, "Paid" | "Balance due" | "Credit");
            assert_eq!(
                line.payment_row, expected,
                "{} is flagged wrong",
                line.label
            );
        }
    }

    #[test]
    fn an_address_is_split_into_the_lines_it_was_typed_as() {
        assert_eq!(
            address_lines("123 Main St\nSpringfield, IL"),
            vec!["123 Main St", "Springfield, IL"]
        );
        assert!(
            address_lines("  \n \n").is_empty(),
            "blank lines say nothing"
        );
        assert!(address_lines("").is_empty());
        assert_eq!(
            address_lines("  123 Main St  \r\n\n  Springfield  "),
            vec!["123 Main St", "Springfield"],
            "a blank line in the middle is not a line"
        );
    }

    #[test]
    fn an_email_prints_only_when_there_is_one() {
        assert_eq!(email_line(Some("  ap@acme.test  ")), Some("ap@acme.test"));
        assert_eq!(email_line(Some("   ")), None);
        assert_eq!(email_line(Some("")), None);
        assert_eq!(email_line(None), None);
    }

    /// A client block that runs off the bottom of the page is never wanted on
    /// an invoice, and the two documents have to clamp identically or the
    /// parity this module exists for is gone.
    #[test]
    fn a_long_address_is_clamped_with_something_that_says_so() {
        let typed = (1..=12)
            .map(|n| format!("Line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = address_lines(&typed);

        assert_eq!(lines.len(), MAX_ADDRESS_LINES);
        assert_eq!(lines[0], "Line 1");
        assert_eq!(
            lines[MAX_ADDRESS_LINES - 1],
            ADDRESS_TRUNCATED,
            "the reader is told the block was cut, not left to wonder"
        );
        // An address that fits is untouched, indicator and all.
        let short = address_lines("A\nB");
        assert_eq!(short, vec!["A", "B"]);
        assert!(!short.contains(&ADDRESS_TRUNCATED));
    }

    /// Exactly at the limit is not truncation.
    #[test]
    fn an_address_that_just_fits_keeps_every_line() {
        let typed = (1..=MAX_ADDRESS_LINES)
            .map(|n| format!("Line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = address_lines(&typed);
        assert_eq!(lines.len(), MAX_ADDRESS_LINES);
        assert_eq!(
            lines[MAX_ADDRESS_LINES - 1],
            format!("Line {MAX_ADDRESS_LINES}")
        );
    }
}
