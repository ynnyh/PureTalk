mod config;
mod indicator;
mod tray;
pub mod updater;
mod voice;

pub fn run() {
    use tauri::Emitter;

    tauri::Builder::default()
        // TODO: 配置签名密钥后启用 updater 插件
        // .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(voice::global_shortcut_plugin())
        .setup(|app| {
            tray::setup_tray(app)?;

            if let Err(e) = voice::sync_hotkey(app.handle()) {
                eprintln!("[voice] 启动注册语音热键失败: {}", e);
            }

            // 启动 offline server(常驻,首次冷启动会慢,后续转写快)
            tauri::async_runtime::spawn(async move {
                if let Err(e) = voice::server::start_offline_server() {
                    eprintln!("[server] 启动 offline server 失败: {}", e);
                }
            });

            // 启动 online server(流式,如果模型已下载)
            tauri::async_runtime::spawn(async move {
                if voice::assets::streaming_assets_ready() {
                    if let Err(e) = voice::server::start_online_server() {
                        eprintln!("[server] 启动 online server 失败: {}", e);
                    }
                } else {
                    eprintln!("[server] 流式模型未下载,跳过 online server 启动");
                }
            });

            // 首次运行：打开设置窗口引导用户
            let cfg = config::load_config();
            if cfg.first_run {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
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
            voice::voice_streaming_status,
            voice::voice_download_streaming,
            config::config_load,
            config::config_save,
            config::autostart_enable,
            config::autostart_disable,
            config::autostart_is_enabled,
            updater::check_update,
            updater::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build tauri app")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                voice::server::stop_offline_server();
                voice::server::stop_online_server();
            }
        });
}
