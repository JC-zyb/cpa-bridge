use std::fmt::{Display, Formatter};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::domain::sub2api::{Sub2ApiAccount, Sub2ApiExport};
use crate::push::account_list_query::{
    build_account_list_query, should_continue_account_list_paging,
};
use crate::push::account_keys::{
    ensure_remote_key_extraction, is_seen, remember, summarize_body_snippet, trimmed_value,
    ExistingAccountKeys, RemoteAccountFetchDiagnostics,
};

const ACCOUNT_LIST_ENDPOINT: &str = "/api/v1/admin/accounts";
const IMPORT_ENDPOINT: &str = "/api/v1/admin/accounts/data";

pub struct FilteredExport { pub export: Sub2ApiExport, pub skipped: usize }

#[derive(Debug, Clone, Deserialize)]
pub struct PushOptions { pub base_url: String, pub email: String, pub password: String }

#[derive(Debug, Clone, Deserialize)]
pub struct ImportDataResult {
    #[serde(default)] pub account_created: usize,
    #[serde(default)] pub account_failed: usize,
    #[serde(default)] pub errors: Vec<ImportDataError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportDataError {
    pub kind: String,
    pub name: Option<String>,
    pub proxy_key: Option<String>,
    pub message: String,
}

#[derive(Debug)]
pub enum PushClientError {
    LoginFailed { status: u16, body: String },
    Unauthorized { status: u16, body: String },
    RequestFailed { status: u16, body: String },
    Transport(String),
    InvalidResponse(String),
}

#[derive(Debug, Deserialize)]
struct DataEnvelope<T> { data: T }
#[derive(Debug, Deserialize)]
struct LoginData { access_token: String }
#[derive(Debug, Deserialize)]
struct AccountListData {
    #[serde(default)] items: Vec<RemoteAccount>,
    #[serde(default)] total: usize,
}
#[derive(Debug, Deserialize)]
struct RemoteAccount { #[serde(default)] credentials: RemoteAccountCredentials }
#[derive(Debug, Default, Deserialize)]
struct RemoteAccountCredentials {
    #[serde(default)] chatgpt_account_id: String,
    #[serde(default)] chatgpt_user_id: String,
}
#[derive(Debug, Serialize)]
struct ImportDataRequest<'a> { data: &'a Sub2ApiExport, skip_default_group_bind: bool }

impl Display for PushClientError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoginFailed { status, body } => write!(formatter, "Sub2Api 登录失败 ({status}): {body}"),
            Self::Unauthorized { status, body } => write!(formatter, "Sub2Api 导入失败 ({status}): {body}"),
            Self::RequestFailed { status, body } => write!(formatter, "Sub2Api 导入失败 ({status}): {body}"),
            Self::Transport(message) | Self::InvalidResponse(message) => formatter.write_str(message),
        }
    }
}
impl std::error::Error for PushClientError {}

// 1) 先登录拿 Bearer Token，后续列表查询和导入都复用它。
pub async fn login(client: &Client, options: &PushOptions) -> Result<String, PushClientError> {
    let response = client
        .post(build_api_url(options, "/api/v1/auth/login"))
        .json(&serde_json::json!({"email": options.email, "password": options.password}))
        .send()
        .await
        .map_err(|error| PushClientError::Transport(format!("Sub2Api 登录请求失败: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(PushClientError::LoginFailed {
            status: status.as_u16(),
            body: response.text().await.unwrap_or_default(),
        });
    }
    let body: DataEnvelope<LoginData> = response.json().await.map_err(|error| {
        PushClientError::InvalidResponse(format!("Sub2Api 登录返回解析失败: {error}"))
    })?;
    Ok(body.data.access_token)
}

// 2) 分页抓取远端账号键，用于上传前判重。
pub async fn fetch_existing_account_keys(
    client: &Client,
    options: &PushOptions,
    bearer_token: &str,
) -> Result<ExistingAccountKeys, PushClientError> {
    let mut diagnostics = RemoteAccountFetchDiagnostics::default();
    let mut existing_keys = ExistingAccountKeys::default();
    let mut page = 1;
    loop {
        let account_page = fetch_account_page(client, options, bearer_token, page).await?;
        let item_count = account_page.items.len();
        diagnostics.fetched_accounts += item_count;
        if page == 1 {
            diagnostics.first_page_item_count = item_count;
            diagnostics.first_page_total = account_page.total;
            diagnostics.first_page_body_snippet = account_page.body_snippet;
        }
        merge_remote_account_keys(&mut existing_keys, account_page.items);
        if !should_continue_account_list_paging(item_count) {
            return ensure_remote_key_extraction(existing_keys, &diagnostics)
                .map_err(PushClientError::InvalidResponse);
        }
        page += 1;
    }
}

pub async fn import_accounts_data(
    client: &Client,
    options: &PushOptions,
    bearer_token: &str,
    export: &Sub2ApiExport,
) -> Result<ImportDataResult, PushClientError> {
    let response = client
        .post(build_api_url(options, IMPORT_ENDPOINT))
        .bearer_auth(bearer_token)
        .json(&build_import_request(export))
        .send()
        .await
        .map_err(|error| PushClientError::Transport(format!("Sub2Api 导入请求失败: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        if status.as_u16() == 401 {
            return Err(PushClientError::Unauthorized { status: status.as_u16(), body });
        }
        return Err(PushClientError::RequestFailed { status: status.as_u16(), body });
    }
    let body: DataEnvelope<ImportDataResult> = response.json().await.map_err(|error| {
        PushClientError::InvalidResponse(format!("Sub2Api 导入返回解析失败: {error}"))
    })?;
    Ok(body.data)
}

// 3) 本地 export 先按 account_id / user_id 过滤，避免重复推送。
pub fn filter_export_accounts(export: &Sub2ApiExport, existing_keys: &ExistingAccountKeys) -> FilteredExport {
    let mut seen_account_ids = existing_keys.account_ids.clone();
    let mut seen_user_ids = existing_keys.user_ids.clone();
    let mut accounts = Vec::with_capacity(export.accounts.len());
    let mut skipped = 0;
    for account in &export.accounts {
        let account_id = normalized_account_id(account);
        let user_id = normalized_user_id(account);
        if is_seen(&account_id, &seen_account_ids) || is_seen(&user_id, &seen_user_ids) {
            skipped += 1;
            continue;
        }
        remember(account_id, &mut seen_account_ids);
        remember(user_id, &mut seen_user_ids);
        accounts.push(account.clone());
    }
    FilteredExport {
        export: Sub2ApiExport {
            exported_at: export.exported_at.clone(),
            proxies: export.proxies.clone(),
            accounts,
        },
        skipped,
    }
}

async fn fetch_account_page(
    client: &Client,
    options: &PushOptions,
    bearer_token: &str,
    page: usize,
) -> Result<FetchedAccountPage, PushClientError> {
    let response = client
        .get(build_api_url(options, ACCOUNT_LIST_ENDPOINT))
        .bearer_auth(bearer_token)
        .query(&build_account_list_query(page))
        .send()
        .await
        .map_err(|error| PushClientError::Transport(format!("Sub2Api 账号列表请求失败: {error}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        if status.as_u16() == 401 {
            return Err(PushClientError::Unauthorized { status: status.as_u16(), body });
        }
        return Err(PushClientError::RequestFailed { status: status.as_u16(), body });
    }
    let body_snippet = summarize_body_snippet(&body);
    let body: DataEnvelope<AccountListData> = serde_json::from_str(&body).map_err(|error| {
        PushClientError::InvalidResponse(format!("Sub2Api 账号列表返回解析失败: {error}"))
    })?;
    Ok(FetchedAccountPage {
        items: body.data.items,
        total: body.data.total,
        body_snippet,
    })
}

fn merge_remote_account_keys(existing_keys: &mut ExistingAccountKeys, accounts: Vec<RemoteAccount>) {
    for account in accounts {
        remember(trimmed_value(&account.credentials.chatgpt_account_id), &mut existing_keys.account_ids);
        remember(trimmed_value(&account.credentials.chatgpt_user_id), &mut existing_keys.user_ids);
    }
}
fn normalized_account_id(account: &Sub2ApiAccount) -> Option<String> { trimmed_value(&account.credentials.chatgpt_account_id) }
fn normalized_user_id(account: &Sub2ApiAccount) -> Option<String> { trimmed_value(&account.credentials.chatgpt_user_id) }
fn build_import_request(export: &Sub2ApiExport) -> ImportDataRequest<'_> {
    ImportDataRequest { data: export, skip_default_group_bind: true }
}

fn build_api_url(options: &PushOptions, endpoint: &str) -> String {
    format!("{}{}", options.base_url.trim_end_matches('/'), endpoint)
}

struct FetchedAccountPage {
    items: Vec<RemoteAccount>,
    total: usize,
    body_snippet: String,
}

#[cfg(test)]
mod tests {
    use super::{build_import_request, filter_export_accounts, ExistingAccountKeys};
    use crate::domain::sub2api::{Sub2ApiAccount, Sub2ApiCredentials, Sub2ApiExport, Sub2ApiExtra};
    use std::collections::HashSet;

    #[test]
    fn build_import_request_skips_default_group_bind() {
        let export = Sub2ApiExport { exported_at: "2026-04-08T08:31:43Z".to_string(), proxies: Vec::new(), accounts: Vec::new() };
        let request = build_import_request(&export);
        assert!(request.skip_default_group_bind);
        assert_eq!(request.data.exported_at, export.exported_at);
    }

    #[test]
    fn filter_export_accounts_skips_remote_existing_and_local_duplicates() {
        let export = sample_export(vec![
            sample_account("远端已有", "acc-1", "user-1"),
            sample_account("本次保留", "acc-2", "user-2"),
            sample_account("本次重复", "acc-2", "user-3"),
        ]);
        let filtered = filter_export_accounts(&export, &ExistingAccountKeys {
            account_ids: HashSet::from([String::from("acc-1")]),
            ..Default::default()
        });
        assert_eq!(filtered.export.accounts.len(), 1);
        assert_eq!(filtered.export.accounts[0].credentials.chatgpt_account_id, "acc-2");
        assert_eq!(filtered.skipped, 2);
    }

    #[test]
    fn filter_export_accounts_skips_local_duplicates_by_chatgpt_user_id() {
        let export = sample_export(vec![
            sample_account("首条", "acc-1", "user-1"),
            sample_account("重复用户", "acc-2", "user-1"),
        ]);
        let filtered = filter_export_accounts(&export, &ExistingAccountKeys::default());
        assert_eq!(filtered.export.accounts.len(), 1);
        assert_eq!(filtered.export.accounts[0].credentials.chatgpt_account_id, "acc-1");
        assert_eq!(filtered.skipped, 1);
    }

    #[test]
    fn filter_export_accounts_skips_remote_existing_by_chatgpt_user_id() {
        let export = sample_export(vec![sample_account("命中远端用户", "acc-2", "user-1")]);
        let filtered = filter_export_accounts(&export, &ExistingAccountKeys {
            user_ids: HashSet::from([String::from("user-1")]),
            ..Default::default()
        });
        assert!(filtered.export.accounts.is_empty());
        assert_eq!(filtered.skipped, 1);
    }

    fn sample_export(accounts: Vec<Sub2ApiAccount>) -> Sub2ApiExport {
        Sub2ApiExport { exported_at: "2026-04-08T08:31:43Z".to_string(), proxies: Vec::new(), accounts }
    }

    fn sample_account(name: &str, account_id: &str, user_id: &str) -> Sub2ApiAccount {
        Sub2ApiAccount {
            name: name.to_string(),
            platform: "openai".to_string(),
            account_type: "oauth".to_string(),
            credentials: Sub2ApiCredentials {
                access_token: "access-token".to_string(),
                chatgpt_account_id: account_id.to_string(),
                chatgpt_user_id: user_id.to_string(),
                expires_at: 0,
                expires_in: 864000,
                organization_id: "org-1".to_string(),
                refresh_token: "refresh-token".to_string(),
            },
            extra: Sub2ApiExtra { email: "demo@example.com".to_string() },
            concurrency: 10,
            priority: 1,
            rate_multiplier: 1,
            auto_pause_on_expired: true,
        }
    }
}











