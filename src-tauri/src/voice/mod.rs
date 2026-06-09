pub mod assets;
pub mod clipboard;
pub mod recording;
pub mod server;
pub mod stt_cloud;
pub mod stt_local;
pub mod ws_client;

use crate::config;
use crate::indicator;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::mpsc;

static VOICE_BUSY: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static REGISTERED_HOTKEY: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
static STREAMING_TX: Lazy<Mutex<Option<mpsc::Sender<Vec<f32>>>>> = Lazy::new(|| Mutex::new(None));

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
        return;
    }

    // Start streaming task if streaming model is ready
    if assets::streaming_assets_ready() {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = start_streaming_task(app_clone).await {
                eprintln!("[streaming] 流式任务失败: {}", e);
            }
        });
    }
}

async fn start_streaming_task(app: AppHandle) -> Result<(), String> {
    // Connect to online server
    let (audio_tx, mut text_rx) = ws_client::online_stream_connect().await?;

    // Create channel for recording -> streaming pipeline
    let (rec_tx, mut rec_rx) = mpsc::channel::<Vec<f32>>(8);

    // Register tx in recording module
    recording::set_stream_tx(Some(rec_tx));

    // Store WS tx for cleanup
    {
        let mut tx_guard = STREAMING_TX.lock().unwrap();
        *tx_guard = Some(audio_tx.clone());
    }

    // Spawn pipeline: rec_rx -> resample -> audio_tx (online WS)
    let audio_tx_clone = audio_tx.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(chunk) = rec_rx.recv().await {
            // chunk is at device sample rate, need to resample to 16kHz for online server
            // For simplicity, assume device is already 16kHz or close (TODO: proper resampling)
            // Send mono chunk (recording already converts to mono in stop())
            if audio_tx_clone.send(chunk).await.is_err() {
                break;
            }
        }
    });

    // Spawn task to listen for streaming text and emit to frontend
    tauri::async_runtime::spawn(async move {
        while let Some(text) = text_rx.recv().await {
            let _ = app.emit("voice:streaming-text", VoiceTranscribed { text });
        }
    });

    Ok(())
}

async fn stop_and_transcribe(app: AppHandle) {
    // Stop streaming if active
    recording::set_stream_tx(None); // Clear recording callback tx
    {
        let mut tx_guard = STREAMING_TX.lock().unwrap();
        *tx_guard = None; // Drop WS sender to signal end
    }

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

    // Trim silence using energy threshold
    let trimmed = recording::trim_silence_energy(&samples);

    if trimmed.is_empty() {
        let mut busy = VOICE_BUSY.lock().unwrap();
        *busy = false;
        indicator::hide_indicator(&app);
        let _ = app.emit("voice:error", VoiceError {
            message: "音频过短或全为静音".to_string(),
        });
        return;
    }

    // 2-pass: always use local SenseVoice for final transcription (high quality + punctuation)
    let cfg = config::load_config();
    let result = if cfg.voice_engine == "cloud" {
        stt_cloud::transcribe(&trimmed, &cfg).await
    } else {
        stt_local::transcribe(&trimmed, &cfg).await
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

#[tauri::command]
pub fn voice_streaming_status() -> serde_json::Value {
    serde_json::json!({
        "ready": assets::streaming_assets_ready()
    })
}

#[tauri::command]
pub async fn voice_download_streaming(app: tauri::AppHandle, proxy: Option<String>) -> Result<(), String> {
    let proxy = proxy.unwrap_or_default();
    let cfg = config::load_config();
    let proxy_to_use = if proxy.is_empty() { &cfg.download_proxy } else { &proxy };
    assets::download_streaming_model(&app, proxy_to_use).await
}
