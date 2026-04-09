use std::sync::Mutex;

use reqwest::Url;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const UPDATER_ENDPOINT_ENV: &str = "TAURI_UPDATER_ENDPOINT";
const UPDATER_PUBKEY_ENV: &str = "TAURI_UPDATER_PUBKEY";

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Debug, Serialize)]
pub struct AppUpdateStatus {
    pub configured: bool,
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub message: String,
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    pending_update: State<'_, PendingUpdate>,
) -> Result<AppUpdateStatus, String> {
    let current_version = app.package_info().version.to_string();
    let Some(updater_config) = read_updater_config()? else {
        clear_pending_update(&pending_update)?;
        return Ok(AppUpdateStatus {
            configured: false,
            available: false,
            current_version,
            latest_version: None,
            notes: None,
            pub_date: None,
            message: format!(
                "未配置自动更新。请先在构建环境设置 {} 和 {}。",
                UPDATER_ENDPOINT_ENV, UPDATER_PUBKEY_ENV
            ),
        });
    };

    let updater = app
        .updater_builder()
        .pubkey(updater_config.pubkey)
        .endpoints(vec![updater_config.endpoint])
        .map_err(|error| format!("配置更新地址失败: {error}"))?
        .build()
        .map_err(|error| format!("创建更新器失败: {error}"))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|error| format!("检查更新失败: {error}"))?
    else {
        clear_pending_update(&pending_update)?;
        return Ok(AppUpdateStatus {
            configured: true,
            available: false,
            current_version,
            latest_version: None,
            notes: None,
            pub_date: None,
            message: "当前已是最新版本".to_string(),
        });
    };

    let status = AppUpdateStatus {
        configured: true,
        available: true,
        current_version,
        latest_version: Some(update.version.clone()),
        notes: update.body.clone(),
        pub_date: update.date.as_ref().map(ToString::to_string),
        message: format!("发现新版本 v{}，可立即下载安装。", update.version),
    };
    replace_pending_update(&pending_update, update)?;
    Ok(status)
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    pending_update: State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = take_pending_update(&pending_update)?
        .ok_or_else(|| "当前没有待安装的更新，请先执行检查更新。".to_string())?;

    update
        .download_and_install(
            |_chunk_length, _content_length| {},
            || {},
        )
        .await
        .map_err(|error| format!("下载安装更新失败: {error}"))?;

    app.restart();
}

fn read_updater_config() -> Result<Option<UpdaterConfig>, String> {
    let endpoint = option_env!("TAURI_UPDATER_ENDPOINT").map(str::trim).unwrap_or_default();
    let pubkey = option_env!("TAURI_UPDATER_PUBKEY").map(str::trim).unwrap_or_default();
    if endpoint.is_empty() && pubkey.is_empty() {
        return Ok(None);
    }
    if endpoint.is_empty() || pubkey.is_empty() {
        return Err(format!(
            "自动更新配置不完整，请同时设置 {} 和 {}。",
            UPDATER_ENDPOINT_ENV, UPDATER_PUBKEY_ENV
        ));
    }
    Ok(Some(UpdaterConfig {
        endpoint: endpoint
            .parse()
            .map_err(|error| format!("更新地址格式无效: {error}"))?,
        pubkey: pubkey.to_string(),
    }))
}

fn replace_pending_update(
    pending_update: &State<'_, PendingUpdate>,
    update: Update,
) -> Result<(), String> {
    let mut guard = pending_update
        .0
        .lock()
        .map_err(|_| "更新状态锁已损坏".to_string())?;
    *guard = Some(update);
    Ok(())
}

fn take_pending_update(
    pending_update: &State<'_, PendingUpdate>,
) -> Result<Option<Update>, String> {
    let mut guard = pending_update
        .0
        .lock()
        .map_err(|_| "更新状态锁已损坏".to_string())?;
    Ok(guard.take())
}

fn clear_pending_update(pending_update: &State<'_, PendingUpdate>) -> Result<(), String> {
    let mut guard = pending_update
        .0
        .lock()
        .map_err(|_| "更新状态锁已损坏".to_string())?;
    *guard = None;
    Ok(())
}

struct UpdaterConfig {
    endpoint: Url,
    pubkey: String,
}
