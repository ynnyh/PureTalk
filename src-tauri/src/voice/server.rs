// sherpa-onnx 常驻服务进程管理(offline / online)
use once_cell::sync::Lazy;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

static OFFLINE_SERVER: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));
static ONLINE_SERVER: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));

const OFFLINE_PORT: u16 = 6021;
const ONLINE_PORT: u16 = 6020;

/// 启动 offline server(SenseVoice),阻塞直到就绪或超时
pub fn start_offline_server() -> Result<(), String> {
    let mut guard = OFFLINE_SERVER.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return Ok(()); // already running
    }

    let voice_dir = crate::voice::assets::voice_dir();
    let bin = voice_dir
        .join("sherpa-onnx-v1.13.2-win-x64-shared-MT-Release")
        .join("bin")
        .join("sherpa-onnx-offline-websocket-server.exe");

    if !bin.exists() {
        return Err("sherpa-onnx offline server 未找到,请先下载语音资产".into());
    }

    let model_dir = voice_dir;
    let model = model_dir.join("model.int8.onnx");
    let tokens = model_dir.join("tokens.txt");

    if !model.exists() || !tokens.exists() {
        return Err("SenseVoice 模型文件未找到,请先下载语音资产".into());
    }

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new(&bin);
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        c.creation_flags(CREATE_NO_WINDOW);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new(&bin);

    cmd.arg(format!("--port={}", OFFLINE_PORT))
        .arg(format!("--sense-voice-model={}", model.display()))
        .arg(format!("--tokens={}", tokens.display()))
        .arg("--sense-voice-use-itn=1")
        .arg("--num-threads=2");

    let child = cmd.spawn().map_err(|e| format!("启动 offline server 失败: {}", e))?;
    *guard = Some(child);
    drop(guard);

    // Wait for server to be ready (retry connecting)
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(200));
        if probe_offline_server().is_ok() {
            return Ok(());
        }
    }
    Err("offline server 启动超时,6秒内未就绪".into())
}

/// 探测 offline server 是否就绪(尝试 TCP 连接)
fn probe_offline_server() -> Result<(), String> {
    std::net::TcpStream::connect(("127.0.0.1", OFFLINE_PORT))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 停止 offline server
pub fn stop_offline_server() {
    let mut guard = OFFLINE_SERVER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn offline_server_port() -> u16 {
    OFFLINE_PORT
}

/// 启动 online server(streaming zipformer),阻塞直到就绪或超时
pub fn start_online_server() -> Result<(), String> {
    let mut guard = ONLINE_SERVER.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return Ok(()); // already running
    }

    if !crate::voice::assets::streaming_assets_ready() {
        return Err("流式模型未下载,请先在设置中下载".into());
    }

    let voice_dir = crate::voice::assets::voice_dir();
    let bin = voice_dir
        .join("sherpa-onnx-v1.13.2-win-x64-shared-MT-Release")
        .join("bin")
        .join("sherpa-onnx-online-websocket-server.exe");

    if !bin.exists() {
        return Err("sherpa-onnx online server 未找到,请先下载语音资产".into());
    }

    let encoder = crate::voice::assets::streaming_encoder_path();
    let decoder = crate::voice::assets::streaming_decoder_path();
    let joiner = crate::voice::assets::streaming_joiner_path();
    let bpe_model = crate::voice::assets::streaming_tokens_path(); // actually bpe.model

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new(&bin);
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        c.creation_flags(CREATE_NO_WINDOW);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new(&bin);

    cmd.arg(format!("--port={}", ONLINE_PORT))
        .arg(format!("--encoder={}", encoder))
        .arg(format!("--decoder={}", decoder))
        .arg(format!("--joiner={}", joiner))
        .arg(format!("--tokens={}", bpe_model))
        .arg("--enable-endpoint=1")
        .arg("--num-threads=2");

    let child = cmd.spawn().map_err(|e| format!("启动 online server 失败: {}", e))?;
    *guard = Some(child);
    drop(guard);

    // Wait for server to be ready
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(200));
        if probe_online_server().is_ok() {
            return Ok(());
        }
    }
    Err("online server 启动超时,6秒内未就绪".into())
}

fn probe_online_server() -> Result<(), String> {
    std::net::TcpStream::connect(("127.0.0.1", ONLINE_PORT))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn stop_online_server() {
    let mut guard = ONLINE_SERVER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn online_server_port() -> u16 {
    ONLINE_PORT
}
