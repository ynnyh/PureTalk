pub mod assets;
pub mod clipboard;
pub mod recording;
pub mod stt_cloud;
pub mod stt_local;

use crate::config;
use crate::indicator;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static VOICE_BUSY: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static REGISTERED_HOTKEY: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTranscribed {
    pub text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceError {
    pub message: String,
}

pub fn global_shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let busy = { *VOICE_BUSY.lock().unwrap() };
                if busy {
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        stop_and_transcribe(h).await;
                    });
                } else {
                    let h = app.clone();
                    tauri::async_runtime::spawn(async move {
                        start_recording(h).await;
                    });
                }
            }
        })
        .build()
}

pub fn sync_hotkey(app: &AppHandle) -> Result<(), String> {
    let gs = app.global_shortcut();
    let cfg = config::load_config();

    let mut registered = REGISTERED_HOTKEY
        .lock()
        .map_err(|_| "热键注册状态锁中毒".to_string())?;

    // Unregister previous hotkey
    if let Some(prev_accel) = registered.take() {
        if let Ok(prev) = prev_accel.parse::<Shortcut>() {
            if gs.is_registered(prev) {
                let _ = gs.unregister(prev);
            }
        }
    }

    // Register new hotkey
    let accel = &cfg.voice_hotkey;
    let shortcut: Shortcut = accel
        .parse()
        .map_err(|e| format!("无效的快捷键 '{}': {}", accel, e))?;

    if gs.is_registered(shortcut) {
        let _ = gs.unregister(shortcut);
    }

    gs.register(shortcut)
        .map_err(|e| format!("注册快捷键 '{}' 失败: {}", accel, e))?;

    *registered = Some(accel.clone());
    Ok(())
}

async fn start_recording(app: AppHandle) {
    {
        let mut busy = VOICE_BUSY.lock().unwrap();
        if *busy {
            return;
        }
        *busy = true;
    }

    indicator::show_indicator(&app, "recording");

    if let Err(e) = recording::start() {
        let mut busy = VOICE_BUSY.lock().unwrap();
        *busy = false;
        indicator::hide_indicator(&app);
        let _ = app.emit("voice:error", VoiceError { message: e });
    }
}

async fn stop_and_transcribe(app: AppHandle) {
    indicator::show_indicator(&app, "transcribing");

    let samples = match recording::stop() {
        Ok(s) => s,
        Err(e) => {
            let mut busy = VOICE_BUSY.lock().unwrap();
            *busy = false;
            indicator::hide_indicator(&app);
            let _ = app.emit("voice:error", VoiceError { message: e });
            return;
        }
    };

    if samples.is_empty() {
        let mut busy = VOICE_BUSY.lock().unwrap();
        *busy = false;
        indicator::hide_indicator(&app);
        let _ = app.emit("voice:error", VoiceError {
            message: "未检测到音频数据".to_string(),
        });
        return;
    }

    let cfg = config::load_config();
    let result = if cfg.voice_engine == "cloud" {
        stt_cloud::transcribe(&samples, &cfg).await
    } else {
        stt_local::transcribe(&samples, &cfg)
    };

    match result {
        Ok(text) if !text.trim().is_empty() => {
            if let Err(e) = clipboard::inject_text(&text) {
                let _ = app.emit("voice:error", VoiceError { message: e });
            }
            indicator::show_done_then_hide(&app);
            let _ = app.emit("voice:transcribed", VoiceTranscribed { text });
        }
        Ok(_) => {
            indicator::hide_indicator(&app);
            let _ = app.emit("voice:error", VoiceError {
                message: "未识别到有效文本".to_string(),
            });
        }
        Err(e) => {
            indicator::hide_indicator(&app);
            let _ = app.emit("voice:error", VoiceError { message: e });
        }
    }

    let mut busy = VOICE_BUSY.lock().unwrap();
    *busy = false;
}

// ---- Tauri Commands ----

#[tauri::command]
pub async fn voice_start(app: AppHandle) -> Result<(), String> {
    start_recording(app).await;
    Ok(())
}

#[tauri::command]
pub async fn voice_stop_and_transcribe(app: AppHandle) -> Result<(), String> {
    stop_and_transcribe(app).await;
    Ok(())
}

#[tauri::command]
pub fn voice_cancel(app: AppHandle) -> Result<(), String> {
    recording::stop().ok();
    let mut busy = VOICE_BUSY.lock().unwrap();
    *busy = false;
    indicator::hide_indicator(&app);
    Ok(())
}

#[tauri::command]
pub fn voice_assets_status() -> serde_json::Value {
    let ready = assets::assets_ready();
    serde_json::json!({ "ready": ready })
}

#[tauri::command]
pub async fn voice_download_assets(app: AppHandle, proxy: Option<String>) -> Result<(), String> {
    let proxy = proxy.unwrap_or_default();
    if proxy.is_empty() {
        let cfg = config::load_config();
        assets::download_assets(&app, &cfg.download_proxy).await
    } else {
        assets::download_assets(&app, &proxy).await
    }
}

#[tauri::command]
pub fn voice_open_dir() -> Result<(), String> {
    let dir = assets::voice_dir();
    open::that(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn voice_hotkey_sync(app: AppHandle) -> Result<(), String> {
    sync_hotkey(&app)
}

#[tauri::command]
pub fn voice_cloud_status() -> serde_json::Value {
    let cfg = config::load_config();
    serde_json::json!({
        "hasCredentials": !cfg.voice_cloud.volc_app_id.is_empty() && !cfg.voice_cloud.volc_access_token.is_empty()
    })
}
