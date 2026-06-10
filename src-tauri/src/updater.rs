use serde::Serialize;
use tauri::AppHandle;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub body: String,
}

/// Check for updates and return status
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateStatus, String> {
    let current_version = app.package_info().version.to_string();

    // Updater plugin not enabled yet — return friendly error
    Err(format!(
        "自动更新功能暂未启用，请前往 GitHub 查看最新版本 (当前: v{})",
        current_version
    ))
}

/// Download and install update
#[tauri::command]
pub async fn install_update(_app: AppHandle) -> Result<(), String> {
    Err("自动更新功能暂未启用，请前往 GitHub 手动下载最新版本".into())
}
