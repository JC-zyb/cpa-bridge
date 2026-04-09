use std::sync::atomic::{AtomicBool, Ordering};
use reqwest::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use crate::commands::conversion::{preview_cpa_source, SourceKind};
use crate::push::client::{
    fetch_existing_account_keys, filter_export_accounts, import_accounts_data, login,
    ImportDataError, PushClientError, PushOptions,
};

const PUSH_PROGRESS_EVENT: &str = "sub2api://push-progress";
const PUSH_ALREADY_RUNNING_MESSAGE: &str = "已有推送任务正在进行，请勿重复启动";

// 控制同一时刻只允许一个推送任务运行，同时支持用户主动取消。
#[derive(Default)]
pub struct PushCancellation { flag: AtomicBool, running: AtomicBool }

impl PushCancellation {
    fn begin_push(&self) -> Result<PushExecutionGuard<'_>, String> {
        if self.running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return Err(PUSH_ALREADY_RUNNING_MESSAGE.to_string());
        }
        self.reset();
        Ok(PushExecutionGuard { cancellation: self })
    }
    fn cancel(&self) { self.flag.store(true, Ordering::SeqCst); }
    fn is_cancelled(&self) -> bool { self.flag.load(Ordering::SeqCst) }
    fn reset(&self) { self.flag.store(false, Ordering::SeqCst); }
    fn finish(&self) {
        self.reset();
        self.running.store(false, Ordering::SeqCst);
    }
}

struct PushExecutionGuard<'a> { cancellation: &'a PushCancellation }

impl Drop for PushExecutionGuard<'_> {
    fn drop(&mut self) { self.cancellation.finish(); }
}

#[derive(Debug, Serialize, Clone)]
pub struct PushFailureDetail {
    pub account_name: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct PushSummary {
    pub total: usize,
    pub success: usize,
    pub failure: usize,
    pub skipped: usize,
    pub canceled: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum PushProgressStage { Started, Succeeded, Failed }

#[derive(Clone, Debug, Serialize)]
struct PushProgressEvent {
    index: usize,
    total: usize,
    account_name: String,
    success: usize,
    failure: usize,
    stage: PushProgressStage,
    reason: Option<String>,
}

#[tauri::command]
pub async fn check_sub2api_connection(options: PushOptions) -> Result<(), String> {
    let client = Client::new();
    login(&client, &options)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cancel_cpa_push(cancellation: State<'_, PushCancellation>) { cancellation.cancel(); }

// 推送主流程：本地预览 -> 远端判重 -> 导入 -> 回传进度汇总。
#[tauri::command]
pub async fn push_cpa_source_to_sub2api(
    app_handle: AppHandle,
    cancellation: State<'_, PushCancellation>,
    source_path: String,
    source_kind: SourceKind,
    type_filter: Option<String>,
    options: PushOptions,
) -> Result<PushSummary, String> {
    let _guard = cancellation.begin_push()?;
    let preview = preview_cpa_source(source_path, source_kind, type_filter)?;
    let total = preview.export.accounts.len();
    if cancellation.is_cancelled() {
        return Ok(build_push_summary(total, 0, 0, 0, true));
    }

    // 登录只做一次，若列表查询或导入遇到 401 再局部重登。
    let client = Client::new();
    let mut bearer_token = login(&client, &options).await.map_err(|error| error.to_string())?;
    let filtered_export = filter_export_with_retry(&client, &options, &mut bearer_token, &preview.export)
        .await
        .map_err(|error| error.to_string())?;
    if cancellation.is_cancelled() {
        return Ok(build_push_summary(total, 0, 0, filtered_export.skipped, true));
    }

    let filtered_total = filtered_export.export.accounts.len();
    if filtered_total == 0 {
        return Ok(build_push_summary(total, 0, 0, filtered_export.skipped, false));
    }

    emit_collection_started(&app_handle, filtered_total)?;
    let import_result = import_with_retry(&client, &options, &mut bearer_token, &filtered_export.export)
        .await
        .map_err(|error| error.to_string())?;
    let failures = collect_account_failures(import_result.account_failed, import_result.errors);
    emit_import_progress(&app_handle, filtered_total, import_result.account_created, &failures)?;
    Ok(build_push_summary(
        total,
        import_result.account_created,
        failures.len(),
        filtered_export.skipped,
        false,
    ))
}

fn build_push_summary(
    total: usize,
    success: usize,
    failure: usize,
    skipped: usize,
    canceled: bool,
) -> PushSummary {
    PushSummary { total, success, failure, skipped, canceled }
}

fn build_progress_event(
    index: usize,
    total: usize,
    account_name: &str,
    success: usize,
    failure: usize,
    stage: PushProgressStage,
    reason: Option<String>,
) -> PushProgressEvent {
    PushProgressEvent {
        index,
        total,
        account_name: normalize_account_name(account_name),
        success,
        failure,
        stage,
        reason,
    }
}

fn emit_collection_started(app_handle: &AppHandle, total: usize) -> Result<(), String> {
    emit_push_progress(
        app_handle,
        build_progress_event(1, total, &collection_name(total), 0, 0, PushProgressStage::Started, None),
    )
}

fn emit_import_progress(
    app_handle: &AppHandle,
    total: usize,
    success: usize,
    failures: &[PushFailureDetail],
) -> Result<(), String> {
    if failures.is_empty() {
        return emit_push_progress(
            app_handle,
            build_progress_event(total, total, &collection_name(total), success, 0, PushProgressStage::Succeeded, None),
        );
    }

    for (offset, failure) in failures.iter().enumerate() {
        emit_push_progress(
            app_handle,
            build_progress_event(
                success + offset + 1,
                total,
                &failure.account_name,
                success,
                offset + 1,
                PushProgressStage::Failed,
                Some(failure.reason.clone()),
            ),
        )?;
    }
    Ok(())
}

fn emit_push_progress(app_handle: &AppHandle, event: PushProgressEvent) -> Result<(), String> {
    app_handle.emit(PUSH_PROGRESS_EVENT, event).map_err(|error| error.to_string())
}

fn collection_name(total: usize) -> String { format!("账号集合（{}条）", total) }

fn normalize_account_name(account_name: &str) -> String {
    let normalized = account_name.trim();
    if normalized.is_empty() { return "未命名账号".to_string(); }
    normalized.to_string()
}

fn collect_account_failures(expected_failures: usize, errors: Vec<ImportDataError>) -> Vec<PushFailureDetail> {
    let mut failures: Vec<_> = errors
        .into_iter()
        .filter(|error| error.kind == "account")
        .map(map_import_error)
        .collect();
    // 某些失败只有数量没有明细，这里补齐占位信息，避免前端统计失真。
    while failures.len() < expected_failures {
        failures.push(PushFailureDetail {
            account_name: "未命名账号".to_string(),
            reason: "Sub2Api 返回失败但未提供明细".to_string(),
        });
    }
    failures
}

fn map_import_error(error: ImportDataError) -> PushFailureDetail {
    PushFailureDetail {
        account_name: normalize_account_name(
            error.name.as_deref().or(error.proxy_key.as_deref()).unwrap_or("未命名账号"),
        ),
        reason: error.message,
    }
}

async fn filter_export_with_retry(
    client: &Client,
    options: &PushOptions,
    bearer_token: &mut String,
    export: &crate::domain::sub2api::Sub2ApiExport,
) -> Result<crate::push::client::FilteredExport, PushClientError> {
    // 先查远端已有账号；若 token 过期，只对这一段重登一次。
    match fetch_existing_account_keys(client, options, bearer_token).await {
        Ok(existing_keys) => Ok(filter_export_accounts(export, &existing_keys)),
        Err(PushClientError::Unauthorized { .. }) => {
            *bearer_token = login(client, options).await?;
            let existing_keys = fetch_existing_account_keys(client, options, bearer_token).await?;
            Ok(filter_export_accounts(export, &existing_keys))
        }
        Err(error) => Err(error),
    }
}

async fn import_with_retry(
    client: &Client,
    options: &PushOptions,
    bearer_token: &mut String,
    export: &crate::domain::sub2api::Sub2ApiExport,
) -> Result<crate::push::client::ImportDataResult, PushClientError> {
    // 导入同样只允许一次 401 自动重试，避免无穷循环。
    match import_accounts_data(client, options, bearer_token, export).await {
        Ok(result) => Ok(result),
        Err(PushClientError::Unauthorized { .. }) => {
            *bearer_token = login(client, options).await?;
            import_accounts_data(client, options, bearer_token, export).await
        }
        Err(error) => Err(error),
    }
}


#[cfg(test)]
mod tests {
    use super::PushCancellation;

    #[test]
    fn begin_push_blocks_second_start_until_guard_drops() {
        let state = PushCancellation::default();
        let guard = state.begin_push().expect("第一次启动应成功");
        assert!(state.begin_push().is_err(), "第二次并发启动应被拒绝");
        drop(guard);
        assert!(state.begin_push().is_ok(), "前一个任务结束后应允许再次启动");
    }
}

