mod config;
mod indicator;
mod tray;
mod voice;

pub fn run() {
    tauri::Builder::default()
        .plugin(voice::global_shortcut_plugin())
        .setup(|app| {
            tray::setup_tray(app)?;

            if let Err(e) = voice::sync_hotkey(app.handle()) {
                eprintln!("[voice] 启动注册语音热键失败: {}", e);
            }

            // 首次运行：打开设置窗口
            let cfg = config::load_config();
            if cfg.first_run {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // 等待托盘图标就绪
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    tray::open_settings(&handle);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            voice::voice_start,
            voice::voice_stop_and_transcribe,
            voice::voice_cancel,
            voice::voice_assets_status,
            voice::voice_download_assets,
            voice::voice_open_dir,
            voice::voice_hotkey_sync,
            voice::voice_cloud_status,
            config::config_load,
            config::config_save,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build tauri app")
        .run(|_app_handle, _event| {});
}
