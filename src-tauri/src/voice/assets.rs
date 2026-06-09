use futures_util::StreamExt;
use serde::Serialize;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::time::Instant;
use tauri::Emitter;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

const SHERPA_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.2/sherpa-onnx-v1.13.2-win-x64-shared-MT-Release.tar.bz2";
const SHERPA_URL_MIRROR: &str = "https://ghfast.top/https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.2/sherpa-onnx-v1.13.2-win-x64-shared-MT-Release.tar.bz2";
const MODEL_URL: &str = "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx";
const MODEL_URL_MIRROR: &str = "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx";
const TOKENS_URL: &str = "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt";
const TOKENS_URL_MIRROR: &str = "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt";

// 流式模型 (small bilingual zh-en zipformer)
const STREAMING_MODEL_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-small-bilingual-zh-en-2023-02-16.tar.bz2";
const STREAMING_MODEL_URL_MIRROR: &str = "https://ghfast.top/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-small-bilingual-zh-en-2023-02-16.tar.bz2";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub phase: String,
    pub phase_label: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64, // bytes/s
}

pub fn voice_dir() -> PathBuf {
    let home = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".puretalk").join("voice")
}

pub fn sherpa_offline_path() -> String {
    voice_dir()
        .join("sherpa-onnx-v1.13.2-win-x64-shared-MT-Release")
        .join("bin")
        .join("sherpa-onnx-offline.exe")
        .to_string_lossy()
        .to_string()
}

pub fn model_path() -> String {
    voice_dir().join("model.int8.onnx").to_string_lossy().to_string()
}

pub fn tokens_path() -> String {
    voice_dir().join("tokens.txt").to_string_lossy().to_string()
}

pub fn streaming_encoder_path() -> String {
    voice_dir().join("streaming").join("encoder-epoch-99-avg-1.int8.onnx").to_string_lossy().to_string()
}

pub fn streaming_decoder_path() -> String {
    voice_dir().join("streaming").join("decoder-epoch-99-avg-1.int8.onnx").to_string_lossy().to_string()
}

pub fn streaming_joiner_path() -> String {
    voice_dir().join("streaming").join("joiner-epoch-99-avg-1.int8.onnx").to_string_lossy().to_string()
}

pub fn streaming_tokens_path() -> String {
    voice_dir().join("streaming").join("bpe.model").to_string_lossy().to_string()
}

pub fn assets_ready() -> bool {
    #[cfg(windows)]
    {
        std::path::Path::new(&sherpa_offline_path()).exists()
            && std::path::Path::new(&model_path()).exists()
            && std::path::Path::new(&tokens_path()).exists()
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new(&model_path()).exists()
            && std::path::Path::new(&tokens_path()).exists()
    }
}

pub fn streaming_assets_ready() -> bool {
    let streaming_dir = voice_dir().join("streaming");
    // Only consider ready if tokens.txt exists (bpe.model not supported by online server)
    streaming_dir.join("encoder-epoch-99-avg-1.int8.onnx").exists()
        && streaming_dir.join("decoder-epoch-99-avg-1.int8.onnx").exists()
        && streaming_dir.join("joiner-epoch-99-avg-1.int8.onnx").exists()
        && streaming_dir.join("tokens.txt").exists()
}

pub async fn download_streaming_model(app: &tauri::AppHandle, proxy: &str) -> Result<(), String> {
    let dir = voice_dir();
    let streaming_dir = dir.join("streaming");
    std::fs::create_dir_all(&streaming_dir).map_err(|e| format!("创建 streaming 目录失败: {}", e))?;

    let client = build_client(proxy)?;

    // Download streaming model tarball
    let tarball_dest = dir.join("streaming-model.tar.bz2");
    download_with_progress(
        &client,
        STREAMING_MODEL_URL,
        STREAMING_MODEL_URL_MIRROR,
        &tarball_dest,
        app,
        "streaming",
        "流式模型",
    )
    .await?;

    // Emit extracting status
    emit_progress(app, "streaming", "解压中...", 0, 0, 0);

    // Extract tarball (contains a nested dir, need to flatten)
    let temp_extract = dir.join("streaming-temp");
    std::fs::create_dir_all(&temp_extract).map_err(|e| format!("创建临时目录失败: {}", e))?;
    extract_tarball(&tarball_dest, &temp_extract)?;

    // Move extracted files to streaming/ (use copy + remove to avoid cross-device link errors)
    let extracted_dir = temp_extract.join("sherpa-onnx-streaming-zipformer-small-bilingual-zh-en-2023-02-16");
    if extracted_dir.exists() {
        for entry in std::fs::read_dir(&extracted_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let src = entry.path();
            let dest = streaming_dir.join(entry.file_name());

            if src.is_file() {
                std::fs::copy(&src, &dest).map_err(|e| format!("复制文件失败: {}", e))?;
            } else if src.is_dir() {
                copy_dir_recursive(&src, &dest)?;
            }
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(&tarball_dest);
    let _ = std::fs::remove_dir_all(&temp_extract);

    // Emit done event
    let _ = app.emit("voice:download-done", ());

    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("创建目录失败: {}", e))?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_file() {
            std::fs::copy(&src_path, &dest_path).map_err(|e| format!("复制文件失败: {}", e))?;
        } else if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

pub async fn download_assets(app: &tauri::AppHandle, proxy: &str) -> Result<(), String> {
    let dir = voice_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let client = build_client(proxy)?;

    // 1. sherpa-onnx binary
    let sherpa_dest = dir.join("sherpa-onnx.tar.bz2");
    download_with_progress(&client, SHERPA_URL, SHERPA_URL_MIRROR, &sherpa_dest, app, "binary", "语音引擎").await?;
    emit_progress(app, "binary", "语音引擎", 0, 0, 0);
    extract_tarball(&sherpa_dest, &dir)?;
    let _ = std::fs::remove_file(&sherpa_dest);

    // 2. model
    let model_dest = dir.join("model.int8.onnx");
    download_with_progress(&client, MODEL_URL, MODEL_URL_MIRROR, &model_dest, app, "model", "语音模型").await?;

    // 3. tokens
    let tokens_dest = dir.join("tokens.txt");
    download_with_progress(&client, TOKENS_URL, TOKENS_URL_MIRROR, &tokens_dest, app, "tokens", "词表文件").await?;

    // Done signal
    let _ = app.emit("voice:download-done", serde_json::json!({ "ok": true }));

    Ok(())
}

fn build_client(proxy: &str) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600));
    if !proxy.is_empty() {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}

async fn download_with_progress(
    client: &reqwest::Client,
    url: &str,
    mirror: &str,
    dest: &std::path::Path,
    app: &tauri::AppHandle,
    phase: &str,
    label: &str,
) -> Result<(), String> {
    // Try primary, fall back to mirror
    let result = stream_download(client, url, dest, app, phase, label).await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(dest).await;
        let result2 = stream_download(client, mirror, dest, app, phase, label).await;
        if let Err(e2) = result2 {
            return Err(format!("主站和镜像均下载失败:\n1. {}\n2. {}\n\n请检查网络或设置代理", e, e2));
        }
    }
    Ok(())
}

async fn stream_download(
    client: &reqwest::Client,
    url: &str,
    dest: &std::path::Path,
    app: &tauri::AppHandle,
    phase: &str,
    label: &str,
) -> Result<(), String> {
    // Retry up to 3 times on transient errors
    let mut last_err = String::new();
    for attempt in 0..3 {
        match try_stream_download(client, url, dest, app, phase, label).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                // Don't retry on non-transient errors
                if !is_transient_error(&e) {
                    return Err(e);
                }
                last_err = e;
                if attempt < 2 {
                    let delay = std::time::Duration::from_secs((attempt as u64 + 1) * 3);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    Err(format!("重试 3 次后仍失败: {}", last_err))
}

fn is_transient_error(e: &str) -> bool {
    e.contains("502") || e.contains("503") || e.contains("504")
        || e.contains("timeout") || e.contains("Timeout")
        || e.contains("connection") || e.contains("Connection")
        || e.contains("reset") || e.contains("Reset")
}

async fn try_stream_download(
    client: &reqwest::Client,
    url: &str,
    dest: &std::path::Path,
    app: &tauri::AppHandle,
    phase: &str,
    label: &str,
) -> Result<(), String> {
    // Resume: check existing partial file
    let existing_size = if dest.exists() {
        tokio::fs::metadata(dest)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    let mut req = client.get(url);
    if existing_size > 0 {
        req = req.header("Range", format!("bytes={}-", existing_size));
    }

    let resp = req.send().await.map_err(|e| format!("请求失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() && status.as_u16() != 206 {
        return Err(format!("下载失败: HTTP {}", status));
    }

    // Determine total size
    let content_length = resp.content_length().unwrap_or(0);
    let total = if status.as_u16() == 206 {
        // Partial content: total = existing + new
        existing_size + content_length
    } else {
        // Full content: if we had a partial file, restart from 0
        if existing_size > 0 {
            // Server doesn't support resume, redownload
            content_length
        } else {
            content_length
        }
    };

    // Open file for writing (append if resuming with 206)
    let mut file = if status.as_u16() == 206 && existing_size > 0 {
        tokio::fs::OpenOptions::new()
            .write(true)
            .open(dest)
            .await
            .map_err(|e| format!("打开文件失败: {}", e))?
    } else {
        // Full download: create/truncate
        tokio::fs::File::create(dest)
            .await
            .map_err(|e| format!("创建文件失败: {}", e))?
    };

    if status.as_u16() == 206 {
        file.seek(SeekFrom::End(0))
            .await
            .map_err(|e| format!("seek 失败: {}", e))?;
    }

    // Stream download with progress
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = if status.as_u16() == 206 { existing_size } else { 0 };
    let mut last_report = Instant::now();
    let mut last_bytes = downloaded;
    let mut speed: u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("接收数据失败: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入文件失败: {}", e))?;
        downloaded += chunk.len() as u64;

        // Report progress every 200ms
        let elapsed = last_report.elapsed();
        if elapsed.as_millis() >= 200 {
            let bytes_delta = downloaded - last_bytes;
            speed = (bytes_delta as f64 / elapsed.as_secs_f64()) as u64;
            last_bytes = downloaded;
            last_report = Instant::now();

            emit_progress(app, phase, label, downloaded, total, speed);
        }
    }

    file.flush().await.map_err(|e| format!("刷新文件失败: {}", e))?;

    // Final progress
    emit_progress(app, phase, label, downloaded, total, 0);

    Ok(())
}

fn emit_progress(app: &tauri::AppHandle, phase: &str, label: &str, downloaded: u64, total: u64, speed: u64) {
    let _ = app.emit(
        "voice:download-progress",
        DownloadProgress {
            phase: phase.to_string(),
            phase_label: label.to_string(),
            downloaded,
            total,
            speed,
        },
    );
}

fn extract_tarball(archive_path: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| format!("打开压缩包失败: {}", e))?;
    let decoder = bzip2_rs::DecoderReader::new(std::io::BufReader::new(file));
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(dest)
        .map_err(|e| format!("解压失败: {}", e))?;
    Ok(())
}
