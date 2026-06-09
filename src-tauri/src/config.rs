use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCloudConfig {
    #[serde(default)]
    pub volc_app_id: String,
    #[serde(default)]
    pub volc_access_token: String,
}

impl Default for VoiceCloudConfig {
    fn default() -> Self {
        Self {
            volc_app_id: String::new(),
            volc_access_token: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub voice_engine: String,
    #[serde(default = "default_hotkey")]
    pub voice_hotkey: String,
    #[serde(default)]
    pub voice_cloud: VoiceCloudConfig,
    #[serde(default)]
    pub download_proxy: String,
    #[serde(default = "default_true")]
    pub first_run: bool,
    #[serde(default)]
    pub autostart: bool,
}

fn default_hotkey() -> String {
    "CommandOrControl+Shift+Space".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            voice_engine: "local".to_string(),
            voice_hotkey: default_hotkey(),
            voice_cloud: VoiceCloudConfig::default(),
            download_proxy: String::new(),
            first_run: true,
            autostart: false,
        }
    }
}

pub fn purevoice_dir() -> PathBuf {
    let home = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".puretalk")
}

pub fn config_path() -> PathBuf {
    purevoice_dir().join("config.json")
}

pub fn ensure_dir() {
    let dir = purevoice_dir();
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    ensure_dir();
    let path = config_path();
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn config_load() -> AppConfig {
    load_config()
}

#[tauri::command]
pub fn config_save(config: AppConfig) -> Result<(), String> {
    save_config(&config)
}

#[tauri::command]
pub async fn autostart_enable(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let autostart = app.autolaunch();
    autostart
        .enable()
        .map_err(|e| format!("启用开机自启失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn autostart_disable(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let autostart = app.autolaunch();
    autostart
        .disable()
        .map_err(|e| format!("禁用开机自启失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn autostart_is_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;

    let autostart = app.autolaunch();
    autostart
        .is_enabled()
        .map_err(|e| format!("检查自启状态失败: {}", e))
}
