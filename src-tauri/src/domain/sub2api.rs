use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::domain::cpa::CpaTokenRecord;
use crate::domain::jwt::decode_payload;

const DEFAULT_EXPIRES_IN: i64 = 864000;
const DEFAULT_CONCURRENCY: u32 = 10;
const DEFAULT_PRIORITY: i32 = 1;
const DEFAULT_RATE_MULTIPLIER: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct Sub2ApiCredentials {
    pub access_token: String,
    pub chatgpt_account_id: String,
    pub chatgpt_user_id: String,
    pub expires_at: i64,
    pub expires_in: i64,
    pub organization_id: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sub2ApiAccount {
    pub name: String,
    pub platform: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub credentials: Sub2ApiCredentials,
    pub extra: Sub2ApiExtra,
    pub concurrency: u32,
    pub priority: i32,
    pub rate_multiplier: u32,
    pub auto_pause_on_expired: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sub2ApiExtra {
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sub2ApiExport {
    pub exported_at: String,
    pub proxies: Vec<String>,
    pub accounts: Vec<Sub2ApiAccount>,
}

pub fn convert_cpa_record(record: &CpaTokenRecord, index: usize) -> Sub2ApiAccount {
    let access_payload = decode_payload(&record.access_token);
    let id_payload = decode_payload(&record.id_token);

    let expires_at = parse_expired_at(&record.expired_at)
        .or_else(|| extract_i64(access_payload.as_ref(), &["exp"]))
        .unwrap_or(0);

    Sub2ApiAccount {
        name: record.build_name(index),
        platform: "openai".to_string(),
        account_type: "oauth".to_string(),
        credentials: Sub2ApiCredentials {
            access_token: record.access_token.clone(),
            chatgpt_account_id: record.account_id.clone(),
            chatgpt_user_id: extract_string(
                access_payload.as_ref(),
                &["https://api.openai.com/auth", "chatgpt_user_id"],
            ),
            expires_at,
            expires_in: DEFAULT_EXPIRES_IN,
            organization_id: extract_first_organization_id(id_payload.as_ref()),
            refresh_token: record.refresh_token.clone(),
        },
        extra: Sub2ApiExtra {
            email: record.email.clone(),
        },
        concurrency: DEFAULT_CONCURRENCY,
        priority: DEFAULT_PRIORITY,
        rate_multiplier: DEFAULT_RATE_MULTIPLIER,
        auto_pause_on_expired: true,
    }
}

fn parse_expired_at(expired_at: &str) -> Option<i64> {
    if expired_at.trim().is_empty() {
        return None;
    }

    let parsed = OffsetDateTime::parse(expired_at.trim(), &Rfc3339).ok()?;
    Some(parsed.unix_timestamp())
}

fn extract_first_organization_id(payload: Option<&serde_json::Value>) -> String {
    payload
        .and_then(|value| value.get("https://api.openai.com/auth"))
        .and_then(|value| value.get("organizations"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn extract_i64(payload: Option<&serde_json::Value>, path: &[&str]) -> Option<i64> {
    extract_value(payload, path)?.as_i64()
}

fn extract_string(payload: Option<&serde_json::Value>, path: &[&str]) -> String {
    extract_value(payload, path)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn extract_value<'a>(
    payload: Option<&'a serde_json::Value>,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = payload?;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::convert_cpa_record;
    use crate::domain::cpa::CpaTokenRecord;

    #[test]
    fn convert_cpa_record_maps_fields() {
        let record = CpaTokenRecord {
            account_id: "acc-1".to_string(),
            access_token: "eyJhbGciOiJub25lIn0.eyJleHAiOjEyMzQ1NiwiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS9hdXRoIjp7ImNoYXRncHRfdXNlcl9pZCI6InVzZXItMSJ9fQ.".to_string(),
            refresh_token: "refresh-1".to_string(),
            id_token: "eyJhbGciOiJub25lIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsib3JnYW5pemF0aW9ucyI6W3siaWQiOiJvcmctMSJ9XX19.".to_string(),
            email: "demo@example.com".to_string(),
            expired_at: String::new(),
            account_type: "codex".to_string(),
        };

        let account = convert_cpa_record(&record, 1);
        assert_eq!(account.name, "demo@example.com");
        assert_eq!(account.credentials.chatgpt_user_id, "user-1");
        assert_eq!(account.credentials.organization_id, "org-1");
        assert_eq!(account.credentials.expires_at, 123456);
    }
}
