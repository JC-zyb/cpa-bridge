use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::domain::cpa::CpaTokenRecord;
use crate::domain::sub2api::{convert_cpa_record, Sub2ApiExport};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Directory,
    File,
}

#[derive(Debug, Serialize)]
pub struct ConversionPreview {
    pub scanned_files: usize,
    pub converted_files: usize,
    pub skipped_files: usize,
    pub export: Sub2ApiExport,
}

#[derive(Debug, Serialize)]
pub struct ExportAccountsResult {
    pub exported_files: usize,
    pub file_path: String,
}

// 预览阶段只做本地扫描和格式转换，不触发任何远端请求。
#[tauri::command]
pub fn preview_cpa_source(
    source_path: String,
    source_kind: SourceKind,
    type_filter: Option<String>,
) -> Result<ConversionPreview, String> {
    let mut paths = collect_source_paths(&source_path, &source_kind)?;
    paths.sort();

    let scanned_files = paths.len();
    let records = load_records(&paths, type_filter.as_deref())?;
    if records.is_empty() {
        return Err(empty_result_message(&source_kind, &source_path));
    }

    let accounts = records
        .iter()
        .enumerate()
        .map(|(index, record)| convert_cpa_record(record, index + 1))
        .collect();

    Ok(ConversionPreview {
        scanned_files,
        converted_files: records.len(),
        skipped_files: scanned_files.saturating_sub(records.len()),
        export: Sub2ApiExport {
            exported_at: format_now_utc(),
            proxies: Vec::new(),
            accounts,
        },
    })
}

#[tauri::command]
pub fn export_cpa_preview_accounts(
    export_data: Value,
    target_file: String,
) -> Result<ExportAccountsResult, String> {
    let exported_files = count_exported_accounts(&export_data)?;
    let file_path = resolve_target_file_path(&target_file)?;
    write_export_file(&file_path, &export_data)?;
    Ok(ExportAccountsResult {
        exported_files,
        file_path: file_path.to_string_lossy().to_string(),
    })
}

fn resolve_target_file_path(target_file: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(target_file);
    if path.as_os_str().is_empty() {
        return Err("导出文件路径不能为空".to_string());
    }
    if path.is_dir() {
        return Err(format!("请选择导出文件而不是目录: {}", path.display()));
    }
    let parent = path.parent().filter(|value| !value.as_os_str().is_empty());
    if let Some(parent_dir) = parent {
        if !parent_dir.is_dir() {
            return Err(format!("导出目录不存在: {}", parent_dir.display()));
        }
    }
    Ok(append_json_extension(path))
}

fn append_json_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    if path.extension().and_then(|value| value.to_str()) == Some("json") {
        return path;
    }
    path.with_extension("json")
}

fn count_exported_accounts(export_data: &Value) -> Result<usize, String> {
    let object = export_data
        .as_object()
        .ok_or_else(|| "导出数据格式无效：必须是 JSON 对象".to_string())?;
    let accounts = object
        .get("accounts")
        .and_then(Value::as_array)
        .ok_or_else(|| "导出数据格式无效：缺少 accounts 数组".to_string())?;
    Ok(accounts.len())
}

fn write_export_file(path: &Path, export: &Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(export).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| format!("写入失败 {}: {}", path.display(), error))
}

fn collect_source_paths(
    source_path: &str,
    source_kind: &SourceKind,
) -> Result<Vec<String>, String> {
    match source_kind {
        SourceKind::Directory => collect_directory_json_files(Path::new(source_path)),
        SourceKind::File => collect_single_json_file(Path::new(source_path)),
    }
}

fn collect_directory_json_files(root: &Path) -> Result<Vec<String>, String> {
    if !root.is_dir() {
        return Err(format!("目录不存在: {}", root.display()));
    }

    let mut result = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|item| item.to_str()) != Some("json") {
            continue;
        }
        result.push(path.to_string_lossy().to_string());
    }
    Ok(result)
}

fn collect_single_json_file(path: &Path) -> Result<Vec<String>, String> {
    if !path.is_file() {
        return Err(format!("文件不存在: {}", path.display()));
    }
    if path.extension().and_then(|item| item.to_str()) != Some("json") {
        return Err(format!("请选择 JSON 文件: {}", path.display()));
    }
    Ok(vec![path.to_string_lossy().to_string()])
}

fn load_records(
    paths: &[String],
    type_filter: Option<&str>,
) -> Result<Vec<CpaTokenRecord>, String> {
    // 逐个读取并过滤无效 CPA 文件，保证后续预览结果可直接导出/上传。
    let mut records = Vec::new();
    for path in paths {
        let Some(record) = load_record(path, type_filter)? else {
            continue;
        };
        records.push(record);
    }
    Ok(records)
}

fn load_record(path: &str, type_filter: Option<&str>) -> Result<Option<CpaTokenRecord>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("读取失败 {path}: {error}"))?;
    let record: CpaTokenRecord = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    if !record.has_required_fields() {
        return Ok(None);
    }
    if let Some(expected_type) = type_filter {
        if record.account_type.trim() != expected_type.trim() {
            return Ok(None);
        }
    }
    Ok(Some(record))
}

fn empty_result_message(source_kind: &SourceKind, source_path: &str) -> String {
    match source_kind {
        SourceKind::Directory => format!("目录中没有符合条件的 CPA JSON: {source_path}"),
        SourceKind::File => format!("文件不符合 CPA JSON 条件: {source_path}"),
    }
}

fn format_now_utc() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::new())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{export_cpa_preview_accounts, preview_cpa_source, SourceKind};
    use serde_json::json;

    #[test]
    fn preview_cpa_source_supports_single_file() {
        let temp_dir = create_temp_dir("single-file");
        let file_path = temp_dir.join("account.json");
        fs::write(&file_path, sample_cpa_json("demo@example.com")).expect("write sample json");

        let result = preview_cpa_source(
            file_path.to_string_lossy().to_string(),
            SourceKind::File,
            None,
        )
        .expect("preview should succeed");

        assert_eq!(result.scanned_files, 1);
        assert_eq!(result.converted_files, 1);
        assert_eq!(result.export.accounts.len(), 1);
    }

    #[test]
    fn export_cpa_preview_accounts_writes_current_preview_data() {
        let export_dir = create_temp_dir("export-target");
        let export_file = export_dir.join("sub2api-account-20260408163143.json");
        let export_data = json!({
            "exported_at": "2026-04-08T08:31:43Z",
            "proxies": [],
            "accounts": [{ "name": "demo@example.com" }]
        });

        let result = export_cpa_preview_accounts(
            export_data,
            export_file.to_string_lossy().to_string(),
        )
        .expect("export should succeed");

        assert_eq!(result.exported_files, 1);
        assert_eq!(result.file_path, export_file.to_string_lossy().to_string());
        let content = fs::read_to_string(export_file).expect("read exported file");
        assert!(content.contains("\"accounts\""));
        assert!(content.contains("demo@example.com"));
    }

    fn create_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cpa-bridge-{label}-{unique}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn sample_cpa_json(email: &str) -> String {
        format!(
            "{{\"account_id\":\"acc-1\",\"access_token\":\"eyJhbGciOiJub25lIn0.eyJleHAiOjEyMzQ1NiwiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS9hdXRoIjp7ImNoYXRncHRfdXNlcl9pZCI6InVzZXItMSJ9fQ.\",\"refresh_token\":\"refresh-1\",\"id_token\":\"eyJhbGciOiJub25lIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsib3JnYW5pemF0aW9ucyI6W3siaWQiOiJvcmctMSJ9XX19.\",\"email\":\"{email}\",\"type\":\"codex\"}}"
        )
    }
}
