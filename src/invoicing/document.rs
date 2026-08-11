//! What both client-facing documents say about money and about the client,
//! decided once.
//!
//! The page and the PDF are rendered by two different renderers from the same
//! row, and the only way they can be trusted to agree is for the decision — the
//! figures, and which of them appear at all — to live above both of them.

use crate::models::Invoice;

/// Within half a cent is settled, the slack `refresh_status` already uses when
/// it decides an invoice is paid in full.
const CENT_SLACK: f64 = 0.005;

/// One line of the money block, in the order both documents print them.
pub struct MoneyLine {
    pub label: &'static str,
    pub amount: f64,
    /// The line a reader's eye should land on: the total, and the balance when
    /// there is one.
    pub emphasis: bool,
}

/// The figures both documents draw, and the rules about which of them appear.
pub struct MoneySummary {
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
    pub paid: f64,
    pub balance: f64,
}

impl MoneySummary {
    pub fn of(invoice: &Invoice, paid: f64) -> Self {
        let balance = invoice.total - paid;
        Self {
            subtotal: invoice.subtotal,
            tax: invoice.tax,
            total: invoice.total,
            paid,
            // Nothing branches on this, but a document that prints `-0.00`
            // beside "Balance due" is telling a client something untrue about
            // an invoice they have settled.
            balance: if balance.abs() < CENT_SLACK {
                0.0
            } else {
                balance
            },
        }
    }

    /// A line appears when it has something to say.
    ///
    /// The subtotal/tax rule is the one the PDF has always applied: a one-line
    /// invoice with no tax prints one figure. Paid and Balance appear together,
    /// because a Paid row with no balance beside it leaves the reader to do the
    /// subtraction.
    pub fn lines(&self) -> Vec<MoneyLine> {
        let mut lines = Vec::with_capacity(5);
        if self.tax != 0.0 {
            lines.push(MoneyLine {
                label: "Subtotal",
                amount: self.subtotal,
                emphasis: false,
            });
            lines.push(MoneyLine {
                label: "Tax",
                amount: self.tax,
                emphasis: false,
            });
        }
        lines.push(MoneyLine {
            label: "Total",
            amount: self.total,
            emphasis: true,
        });
        if self.paid > 0.0 {
            lines.push(MoneyLine {
                label: "Paid",
                amount: self.paid,
                emphasis: false,
            });
            lines.push(MoneyLine {
                label: "Balance due",
                amount: self.balance,
                emphasis: true,
            });
        }
        lines
    }
}

/// A billing address as the lines it was typed as, blank ones dropped. Both
/// documents draw one line per row, so an address entered over two lines stays
/// two lines and an address entered with a trailing newline grows no empty row.
pub fn address_lines(address: &str) -> Vec<&str> {
    address
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
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
}
