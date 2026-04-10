use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB},
};

const CONFIG_DIRECTORY: &str = "cpa-bridge";
const CONFIG_FILE_NAME: &str = "config.ini";
const SETTINGS_SECTION: &str = "sub2api";
const ENCRYPTED_PASSWORD_KEY: &str = "admin_password_encrypted";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LocalSettings {
    pub base_url: String,
    pub admin_email: String,
    pub admin_password: String,
}

#[tauri::command]
pub fn load_local_settings() -> Result<LocalSettings, String> {
    let path = settings_file_path()?;
    if !path.exists() {
        return Ok(LocalSettings::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("读取本地配置失败: {error}"))?;
    parse_settings(&content)
}

#[tauri::command]
pub fn save_local_settings(settings: LocalSettings) -> Result<(), String> {
    let path = settings_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建配置目录失败: {error}"))?;
    }
    let encrypted_password = encrypt_password(&settings.admin_password)?;
    let content = format!(
        "[{section}]\nbase_url={base_url}\nadmin_email={admin_email}\n{password_key}={admin_password}\n",
        section = SETTINGS_SECTION,
        base_url = settings.base_url,
        admin_email = settings.admin_email,
        password_key = ENCRYPTED_PASSWORD_KEY,
        admin_password = encrypted_password,
    );
    fs::write(&path, content).map_err(|error| format!("保存本地配置失败: {error}"))?;
    Ok(())
}

fn settings_file_path() -> Result<PathBuf, String> {
    let app_data = env::var_os("APPDATA")
        .ok_or_else(|| "未找到 APPDATA 环境变量".to_string())?;
    Ok(Path::new(&app_data)
        .join(CONFIG_DIRECTORY)
        .join(CONFIG_FILE_NAME))
}

fn parse_settings(content: &str) -> Result<LocalSettings, String> {
    let mut current_section = "";
    let mut settings = LocalSettings::default();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = &line[1..line.len() - 1];
            continue;
        }
        if current_section != SETTINGS_SECTION {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err("本地配置格式无效，请检查 config.ini".to_string());
        };
        apply_setting(&mut settings, key.trim(), value.trim())?;
    }

    Ok(settings)
}

fn apply_setting(settings: &mut LocalSettings, key: &str, value: &str) -> Result<(), String> {
    match key {
        "base_url" => settings.base_url = value.to_string(),
        "admin_email" => settings.admin_email = value.to_string(),
        ENCRYPTED_PASSWORD_KEY => settings.admin_password = decrypt_password(value)?,
        _ => {}
    }
    Ok(())
}

fn encrypt_password(password: &str) -> Result<String, String> {
    if password.is_empty() {
        return Ok(String::new());
    }
    let encrypted = protect_data(password.as_bytes())?;
    Ok(STANDARD.encode(encrypted))
}

fn decrypt_password(encoded: &str) -> Result<String, String> {
    if encoded.is_empty() {
        return Ok(String::new());
    }
    let encrypted = STANDARD
        .decode(encoded)
        .map_err(|error| format!("解码本地密码失败: {error}"))?;
    let decrypted = unprotect_data(&encrypted)?;
    String::from_utf8(decrypted).map_err(|error| format!("解析本地密码失败: {error}"))
}

fn protect_data(plain: &[u8]) -> Result<Vec<u8>, String> {
    let mut input = blob_from_slice(plain);
    let mut output = zero_blob();
    let status = unsafe {
        CryptProtectData(
            &mut input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if status == 0 {
        return Err("加密本地密码失败".to_string());
    }
    take_blob_bytes(output)
}

fn unprotect_data(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    let mut input = blob_from_slice(encrypted);
    let mut output = zero_blob();
    let status = unsafe {
        CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if status == 0 {
        return Err("解密本地密码失败".to_string());
    }
    take_blob_bytes(output)
}

fn blob_from_slice(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    }
}

fn zero_blob() -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    }
}

fn take_blob_bytes(blob: CRYPT_INTEGER_BLOB) -> Result<Vec<u8>, String> {
    if blob.pbData.is_null() || blob.cbData == 0 {
        return Ok(Vec::new());
    }
    let bytes = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec() };
    let _ = unsafe { LocalFree(blob.pbData.cast()) };
    Ok(bytes)
}
