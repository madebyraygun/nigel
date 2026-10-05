use serde::{Deserialize, Serialize};

use crate::documents::model::RecipientRole;
use crate::invoicing::r2::{object_key, KeyPrefix};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestState {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRecipient {
    pub token: String,
    pub role: RecipientRole,
    pub name: String,
}

/// What the Worker reads to decide whether a recipient may still respond.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: i64,
    pub checksum: String,
    pub state: ManifestState,
    pub recipients: Vec<ManifestRecipient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseAction {
    Accept,
    RequestChanges,
}

/// One recipient's answer, as the Worker writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResponse {
    pub action: ResponseAction,
    pub version: i64,
    pub checksum: String,
    pub recipient_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<bool>,
    #[serde(default)]
    pub note: Option<String>,
    pub received_at: String,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

pub fn manifest_key(token: &str) -> String {
    object_key(KeyPrefix::Documents, token, "manifest.json")
}

pub fn response_key(token: &str, version: i64, recipient_token: &str) -> String {
    object_key(
        KeyPrefix::Documents,
        token,
        &format!("v{version}/{recipient_token}.json"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_matches_the_spec_shape() {
        let m = Manifest {
            version: 2,
            checksum: "sha256:ab".into(),
            state: ManifestState::Open,
            recipients: vec![ManifestRecipient {
                token: "t1".into(),
                role: RecipientRole::Signer,
                name: "Pat Example".into(),
            }],
        };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({
                "version": 2, "checksum": "sha256:ab", "state": "open",
                "recipients": [{ "token": "t1", "role": "signer", "name": "Pat Example" }]
            })
        );
    }

    #[test]
    fn a_worker_response_parses_both_actions() {
        let accept: DocumentResponse = serde_json::from_value(serde_json::json!({
            "action": "accept", "version": 2, "checksum": "sha256:ab", "recipientToken": "t1",
            "typedName": "Pat Example", "consent": true, "note": null,
            "receivedAt": "2026-10-05T17:04:11Z", "ip": "203.0.113.7", "userAgent": "UA"
        }))
        .unwrap();
        assert_eq!(accept.action, ResponseAction::Accept);
        let changes: DocumentResponse = serde_json::from_value(serde_json::json!({
            "action": "request_changes", "version": 2, "checksum": "sha256:ab", "recipientToken": "t2",
            "note": "Fix the dates", "receivedAt": "2026-10-05T17:04:11Z"
        }))
        .unwrap();
        assert_eq!((changes.typed_name, changes.consent), (None, None));
    }

    #[test]
    fn private_keys_sit_under_the_document_token() {
        assert_eq!(manifest_key("abc"), "d/abc/manifest.json");
        assert_eq!(response_key("abc", 2, "r1"), "d/abc/v2/r1.json");
    }
}
