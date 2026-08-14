use std::fmt;

use thiserror::Error;

/// Why a delete was refused. The variants are the vocabulary the API publishes
/// as `details.reason`, so the client can render its own wording instead of
/// parsing ours.
///
/// A reason that counts something carries the count, because the two are only
/// ever correct together: `NotDeletable` is about the row's own state and has
/// nothing to count, and a shared `count: 0` beside it would put a figure
/// nobody chose on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    HasTransactions(i64),
    HasActiveRules(i64),
    HasInvoices(i64),
    /// The row is not the freshly-entered draft this delete is for: it has been
    /// published, paid or voided.
    NotDeletable,
}

/// A refused delete: what was being deleted and why.
///
/// `Display` is the message the CLI and the TUI have always printed; the parts
/// stay separately readable so the API can answer with a code and a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteBlock {
    /// The noun in the message: "account", "category", "client" or "invoice".
    pub subject: &'static str,
    pub reason: BlockReason,
}

impl DeleteBlock {
    pub fn transactions(subject: &'static str, count: i64) -> Self {
        Self {
            subject,
            reason: BlockReason::HasTransactions(count),
        }
    }

    pub fn active_rules(subject: &'static str, count: i64) -> Self {
        Self {
            subject,
            reason: BlockReason::HasActiveRules(count),
        }
    }

    pub fn invoices(subject: &'static str, count: i64) -> Self {
        Self {
            subject,
            reason: BlockReason::HasInvoices(count),
        }
    }

    pub fn not_deletable(subject: &'static str) -> Self {
        Self {
            subject,
            reason: BlockReason::NotDeletable,
        }
    }

    pub fn reason_code(&self) -> &'static str {
        match self.reason {
            BlockReason::HasTransactions(_) => "has_transactions",
            BlockReason::HasActiveRules(_) => "has_active_rules",
            BlockReason::HasInvoices(_) => "has_invoices",
            BlockReason::NotDeletable => "not_deletable",
        }
    }

    /// How many blocking rows there are, for the reasons that count something.
    pub fn count(&self) -> Option<i64> {
        match self.reason {
            BlockReason::HasTransactions(n)
            | BlockReason::HasActiveRules(n)
            | BlockReason::HasInvoices(n) => Some(n),
            BlockReason::NotDeletable => None,
        }
    }
}

impl fmt::Display for DeleteBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let subject = self.subject;
        let plural = |count: i64| if count == 1 { "" } else { "s" };
        match self.reason {
            BlockReason::HasTransactions(count) => {
                write!(
                    f,
                    "Cannot delete: {subject} has {count} transaction{}",
                    plural(count)
                )
            }
            BlockReason::HasActiveRules(count) => {
                write!(
                    f,
                    "Cannot delete: {subject} has {count} active rule{}",
                    plural(count)
                )
            }
            BlockReason::HasInvoices(count) => {
                write!(
                    f,
                    "Cannot delete: {subject} has {count} invoice{}",
                    plural(count)
                )
            }
            BlockReason::NotDeletable => {
                write!(
                    f,
                    "Cannot delete: {subject} has been sent, paid or voided — only an unsent draft with no payments can be deleted"
                )
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum NigelError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Not initialized. Run `nigel init` first to set up your data directory.")]
    NotInitialized,

    #[error("Account '{0}' not found. Run `nigel accounts list` to see available accounts, or `nigel accounts add` to create one.")]
    UnknownAccount(String),

    #[error("Unknown format: '{0}'. Run `nigel import --help` for supported formats.")]
    UnknownFormat(String),

    #[error("Couldn't detect the format of this file for account type '{0}'. Use `--format <key>` to specify. Run `nigel import --help` for supported formats.")]
    NoImporter(String),

    #[error("No transactions found for {account} in {month}.")]
    NoTransactions { account: String, month: String },

    #[error("Unknown category: {0}")]
    UnknownCategory(String),

    #[error("Settings error: {0}")]
    Settings(String),

    /// A record that was addressed by id or name is not there.
    #[error("{0}")]
    NotFound(String),

    /// The caller's input is wrong: an empty name, an unknown type, a pattern
    /// that will not compile.
    #[error("{0}")]
    Invalid(String),

    /// A name that has to be unique is taken. `kind` is capitalized because it
    /// opens the sentence.
    #[error("{kind} name already exists: {name}")]
    DuplicateName { kind: &'static str, name: String },

    /// A delete refused by a guardrail.
    #[error("{0}")]
    Blocked(DeleteBlock),

    /// Any other state conflict, carrying the machine-readable reason the API
    /// publishes alongside the message.
    #[error("{message}")]
    Conflict { code: &'static str, message: String },

    #[cfg(feature = "pdf")]
    #[error("PDF error: {0}")]
    Pdf(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NigelError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// These strings are printed by the CLI and the TUI. They are asserted here
    /// so a future edit to the structured form cannot quietly reword them.
    #[test]
    fn delete_blocks_read_exactly_as_they_always_have() {
        let cases = [
            (
                DeleteBlock::transactions("account", 1),
                "Cannot delete: account has 1 transaction",
            ),
            (
                DeleteBlock::transactions("account", 12),
                "Cannot delete: account has 12 transactions",
            ),
            (
                DeleteBlock::transactions("category", 1),
                "Cannot delete: category has 1 transaction",
            ),
            (
                DeleteBlock::active_rules("category", 1),
                "Cannot delete: category has 1 active rule",
            ),
            (
                DeleteBlock::active_rules("category", 3),
                "Cannot delete: category has 3 active rules",
            ),
            (
                DeleteBlock::invoices("client", 1),
                "Cannot delete: client has 1 invoice",
            ),
            (
                DeleteBlock::invoices("client", 3),
                "Cannot delete: client has 3 invoices",
            ),
            (
                DeleteBlock::not_deletable("invoice"),
                "Cannot delete: invoice has been sent, paid or voided — only an unsent draft with no payments can be deleted",
            ),
        ];
        for (block, expected) in cases {
            assert_eq!(block.to_string(), expected);
            assert_eq!(NigelError::Blocked(block).to_string(), expected);
        }
    }

    #[test]
    fn block_reasons_have_stable_wire_codes() {
        assert_eq!(
            DeleteBlock::transactions("account", 1).reason_code(),
            "has_transactions"
        );
        assert_eq!(
            DeleteBlock::active_rules("category", 1).reason_code(),
            "has_active_rules"
        );
        assert_eq!(
            DeleteBlock::invoices("client", 1).reason_code(),
            "has_invoices"
        );
        assert_eq!(
            DeleteBlock::not_deletable("invoice").reason_code(),
            "not_deletable"
        );
    }

    /// A refusal about the row's own state counts nothing, and must not put a
    /// zero on the wire beside a reason that never had a figure.
    #[test]
    fn only_the_counting_reasons_carry_a_count() {
        assert_eq!(DeleteBlock::transactions("account", 12).count(), Some(12));
        assert_eq!(DeleteBlock::active_rules("category", 3).count(), Some(3));
        assert_eq!(DeleteBlock::invoices("client", 1).count(), Some(1));
        assert_eq!(DeleteBlock::not_deletable("invoice").count(), None);
    }

    #[test]
    fn duplicate_name_opens_with_the_kind() {
        let err = NigelError::DuplicateName {
            kind: "Account",
            name: "BofA Checking".into(),
        };
        assert_eq!(
            err.to_string(),
            "Account name already exists: BofA Checking"
        );
    }
}
