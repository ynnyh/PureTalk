use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

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

    let updater = app.updater().map_err(|e| format!("初始化更新器失败: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            Ok(UpdateStatus {
                available: true,
                current_version: current_version.clone(),
                latest_version: update.version.clone(),
                body: update.body.unwrap_or_default(),
            })
        }
        Ok(None) => {
            Ok(UpdateStatus {
                available: false,
                current_version: current_version.clone(),
                latest_version: current_version,
                body: String::new(),
            })
        }
        Err(e) => Err(format!("检查更新失败: {}", e)),
    }
}

/// Download and install update
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| format!("初始化更新器失败: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            update
                .download_and_install(
                    |chunk_length, content_length| {
                        if let Some(total) = content_length {
                            let _ = app.emit(
                                "update:progress",
                                serde_json::json!({
                                    "downloaded": chunk_length,
                                    "total": total,
                                }),
                            );
                        }
                    },
                    || {
                        std::process::exit(0);
                    },
                )
                .await
                .map_err(|e| format!("下载更新失败: {}", e))?;

            Ok(())
        }
        Ok(None) => Err("没有可用的更新".to_string()),
        Err(e) => Err(format!("检查更新失败: {}", e)),
    }
}
