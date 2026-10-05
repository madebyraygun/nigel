//! The one copy of which action is allowed from which status.
use rusqlite::Connection;

use crate::documents::model::DocumentStatus;
use crate::error::{NigelError, Result};
use crate::invoicing::clients::ensure_client_active_for;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Edit,
    Send,
    Revise,
    Accept,
    RequestChanges,
    Decline,
    Countersign,
    Withdraw,
}

impl Action {
    pub const ALL: [Action; 8] = [
        Action::Edit,
        Action::Send,
        Action::Revise,
        Action::Accept,
        Action::RequestChanges,
        Action::Decline,
        Action::Countersign,
        Action::Withdraw,
    ];

    pub fn allowed_from(self) -> &'static [DocumentStatus] {
        use DocumentStatus::*;
        match self {
            Action::Edit | Action::Send => &[Draft],
            Action::Revise | Action::Accept | Action::Decline => &[Sent, ChangesRequested],
            Action::RequestChanges => &[Sent],
            Action::Countersign => &[Accepted],
            Action::Withdraw => &[Draft, Sent, ChangesRequested],
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Action::Edit => "edited",
            Action::Send => "sent",
            Action::Revise => "revised",
            Action::Accept => "accepted",
            Action::RequestChanges => "sent back for changes",
            Action::Decline => "declined",
            Action::Countersign => "countersigned",
            Action::Withdraw => "withdrawn",
        }
    }
}

pub fn can(status: DocumentStatus, action: Action) -> bool {
    action.allowed_from().contains(&status)
}

pub fn ensure_allowed(id: i64, status: DocumentStatus, action: Action) -> Result<()> {
    if can(status, action) {
        return Ok(());
    }
    let state = status.as_str();
    let (code, message) = match status {
        DocumentStatus::Withdrawn | DocumentStatus::Executed | DocumentStatus::Declined => (
            "document_terminal",
            format!("Document #{id} is {state} — nothing more can be done to it."),
        ),
        DocumentStatus::Accepted => (
            "document_accepted",
            format!("Document #{id} is {state} — the only step left is to countersign it."),
        ),
        _ => {
            let needs = action
                .allowed_from()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" or ");
            (
                "document_wrong_state",
                format!(
                    "Document #{id} is {state} and cannot be {}; that needs it to be {needs}.",
                    action.describe()
                ),
            )
        }
    };
    Err(NigelError::Conflict { code, message })
}

pub fn ensure_client_active_for_documents(conn: &Connection, client_id: i64) -> Result<()> {
    ensure_client_active_for(conn, client_id, "filing or sending documents")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guard_table_matches_the_spec() {
        use Action::*;
        use DocumentStatus::*;
        let expected: [(Action, &[DocumentStatus]); 8] = [
            (Edit, &[Draft]),
            (Send, &[Draft]),
            (Revise, &[Sent, ChangesRequested]),
            (Accept, &[Sent, ChangesRequested]),
            (RequestChanges, &[Sent]),
            (Decline, &[Sent, ChangesRequested]),
            (Countersign, &[Accepted]),
            (Withdraw, &[Draft, Sent, ChangesRequested]),
        ];
        for (action, allowed) in expected {
            for status in DocumentStatus::ALL {
                assert_eq!(
                    can(status, action),
                    allowed.contains(&status),
                    "{action:?} from {status:?}"
                );
            }
        }
    }

    #[test]
    fn terminal_states_refuse_everything_with_one_code() {
        for status in [
            DocumentStatus::Withdrawn,
            DocumentStatus::Executed,
            DocumentStatus::Declined,
        ] {
            for action in Action::ALL {
                let err = ensure_allowed(3, status, action).unwrap_err();
                assert!(
                    matches!(
                        err,
                        NigelError::Conflict {
                            code: "document_terminal",
                            ..
                        }
                    ),
                    "{status:?} {action:?}"
                );
            }
        }
    }

    #[test]
    fn accepted_admits_countersign_and_nothing_else() {
        for action in Action::ALL {
            let result = ensure_allowed(3, DocumentStatus::Accepted, action);
            match action {
                Action::Countersign => assert!(result.is_ok()),
                _ => assert!(matches!(
                    result,
                    Err(NigelError::Conflict {
                        code: "document_accepted",
                        ..
                    })
                )),
            }
        }
    }

    #[test]
    fn only_a_draft_is_editable() {
        let err = ensure_allowed(3, DocumentStatus::Sent, Action::Edit).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Document #3 is sent and cannot be edited; that needs it to be draft."
        );
    }

    #[test]
    fn terminal_and_accepted_sentences_read_as_specified() {
        let err = ensure_allowed(3, DocumentStatus::Executed, Action::Edit).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Document #3 is executed — nothing more can be done to it."
        );
        let err = ensure_allowed(3, DocumentStatus::Accepted, Action::Edit).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Document #3 is accepted — the only step left is to countersign it."
        );
    }

    #[test]
    fn an_archived_client_refuses_documents_with_its_own_sentence() {
        let (_d, conn) = crate::documents::testing::test_conn();
        let id = crate::invoicing::clients::add_client(
            &conn,
            "Cedar Systems",
            Some("ap@cedar.test"),
            None,
            None,
        )
        .unwrap();
        crate::invoicing::clients::archive_client(&conn, id, "2026-10-01").unwrap();
        let err = ensure_client_active_for_documents(&conn, id).unwrap_err();
        assert!(matches!(
            err,
            NigelError::Conflict {
                code: "client_archived",
                ..
            }
        ));
        assert_eq!(
            err.to_string(),
            "client 'Cedar Systems' is archived — unarchive it before filing or sending documents"
        );
    }
}
