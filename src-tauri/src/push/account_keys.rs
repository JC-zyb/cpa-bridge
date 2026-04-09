use std::collections::HashSet;

#[derive(Debug, Default, Clone)]
pub struct ExistingAccountKeys {
    pub account_ids: HashSet<String>,
    pub user_ids: HashSet<String>,
}

#[derive(Debug, Default, Clone)]
pub struct RemoteAccountFetchDiagnostics {
    pub fetched_accounts: usize,
    pub first_page_item_count: usize,
    pub first_page_total: usize,
    pub first_page_body_snippet: String,
}

pub fn ensure_remote_key_extraction(
    existing_keys: ExistingAccountKeys,
    diagnostics: &RemoteAccountFetchDiagnostics,
) -> Result<ExistingAccountKeys, String> {
    let extracted_account_ids = existing_keys.account_ids.len();
    let extracted_user_ids = existing_keys.user_ids.len();
    if diagnostics.fetched_accounts == 0 {
        if diagnostics.first_page_total == 0 {
            return Ok(existing_keys);
        }
        return Err(format!(
            "Sub2Api 账号列表抓取结果为 0，但接口 total={}。第一页 items={}，响应片段: {}",
            diagnostics.first_page_total, diagnostics.first_page_item_count, diagnostics.first_page_body_snippet,
        ));
    }
    if extracted_account_ids == 0 && extracted_user_ids == 0 {
        return Err(format!(
            "Sub2Api 账号列表返回了 {} 条账号，但未提取到任何 chatgpt_account_id 或 chatgpt_user_id。第一页 total={}，items={}，响应片段: {}",
            diagnostics.fetched_accounts, diagnostics.first_page_total, diagnostics.first_page_item_count, diagnostics.first_page_body_snippet,
        ));
    }
    Ok(existing_keys)
}

pub fn summarize_body_snippet(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() { return String::from("<empty>"); }
    let mut snippet: String = compact.chars().take(200).collect();
    if compact.chars().count() > 200 { snippet.push_str("..."); }
    snippet
}

pub fn trimmed_value(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() { return None; }
    Some(normalized.to_string())
}

pub fn is_seen(value: &Option<String>, seen: &HashSet<String>) -> bool {
    value.as_ref().is_some_and(|item| seen.contains(item))
}

pub fn remember(value: Option<String>, seen: &mut HashSet<String>) {
    if let Some(item) = value { seen.insert(item); }
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_remote_key_extraction, ExistingAccountKeys, RemoteAccountFetchDiagnostics,
        summarize_body_snippet,
    };

    #[test]
    fn ensure_remote_key_extraction_rejects_non_empty_remote_items_without_keys() {
        let diagnostics = RemoteAccountFetchDiagnostics {
            fetched_accounts: 3,
            first_page_item_count: 3,
            first_page_total: 3,
            first_page_body_snippet: String::from("{\"data\":{\"items\":[{},{}]}}"),
        };
        let error = ensure_remote_key_extraction(ExistingAccountKeys::default(), &diagnostics).unwrap_err();
        assert!(error.contains("未提取到任何 chatgpt_account_id 或 chatgpt_user_id"));
    }

    #[test]
    fn ensure_remote_key_extraction_allows_empty_remote_accounts() {
        let diagnostics = RemoteAccountFetchDiagnostics {
            fetched_accounts: 0,
            first_page_item_count: 0,
            first_page_total: 0,
            first_page_body_snippet: String::from("{\"data\":{\"items\":[]}}"),
        };
        let result = ensure_remote_key_extraction(ExistingAccountKeys::default(), &diagnostics)
            .expect("空库应允许继续上传");
        assert!(result.account_ids.is_empty());
        assert!(result.user_ids.is_empty());
    }

    #[test]
    fn ensure_remote_key_extraction_rejects_zero_fetched_accounts_with_non_zero_total() {
        let diagnostics = RemoteAccountFetchDiagnostics {
            fetched_accounts: 0,
            first_page_item_count: 0,
            first_page_total: 5,
            first_page_body_snippet: String::from("{\"data\":{\"items\":[],\"total\":5}}"),
        };
        let error = ensure_remote_key_extraction(ExistingAccountKeys::default(), &diagnostics).unwrap_err();
        assert!(error.contains("抓取结果为 0"));
        assert!(error.contains("total=5"));
    }

    #[test]
    fn summarize_body_snippet_compacts_whitespace() {
        let snippet = summarize_body_snippet("{\n  \"data\": { \"items\": [] }\n}");
        assert_eq!(snippet, String::from("{ \"data\": { \"items\": [] } }"));
    }
}
