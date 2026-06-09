// sherpa-onnx WebSocket 客户端(offline / online)
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// 通过 offline server 转写音频(单次请求,完整音频)
pub async fn offline_transcribe(samples: &[f32], sample_rate: u32) -> Result<String, String> {
    let port = super::server::offline_server_port();
    let url = format!("ws://127.0.0.1:{}", port);

    let (ws_stream, _) = connect_async(&url)
        .await
        .map_err(|e| format!("连接 offline server 失败: {}", e))?;

    let (mut write, mut read) = ws_stream.split();

    // Protocol: send sample_rate (i32 LE) + num_bytes (i32 LE) + float32 audio bytes
    let sample_rate_bytes = (sample_rate as i32).to_le_bytes();
    let num_bytes = (samples.len() * 4) as i32;
    let num_bytes_bytes = num_bytes.to_le_bytes();

    let mut header = Vec::with_capacity(8);
    header.extend_from_slice(&sample_rate_bytes);
    header.extend_from_slice(&num_bytes_bytes);

    write
        .send(Message::Binary(header))
        .await
        .map_err(|e| format!("发送 header 失败: {}", e))?;

    // Send audio in chunks (≤10240 bytes per message to avoid huge single message)
    let audio_bytes: Vec<u8> = samples
        .iter()
        .flat_map(|&s| s.to_le_bytes())
        .collect();

    const CHUNK_SIZE: usize = 10240;
    for chunk in audio_bytes.chunks(CHUNK_SIZE) {
        write
            .send(Message::Binary(chunk.to_vec()))
            .await
            .map_err(|e| format!("发送音频失败: {}", e))?;
    }

    // Receive result
    let result_msg = read
        .next()
        .await
        .ok_or_else(|| "未收到 offline server 响应".to_string())?
        .map_err(|e| format!("接收结果失败: {}", e))?;

    let result_text = match result_msg {
        Message::Text(t) => t,
        _ => return Err("offline server 返回了非文本消息".into()),
    };

    let json: serde_json::Value = serde_json::from_str(&result_text)
        .map_err(|e| format!("解析 offline server JSON 失败: {}", e))?;

    json.get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "offline server 响应中未找到 text 字段".to_string())
}

/// 流式转写:连接 online server,返回 (audio_tx, text_rx)
/// audio_tx: 发送 f32 音频块(每块 ~2400 samples = 150ms @ 16kHz)
/// text_rx: 接收增量文本(每次收到更新的完整识别结果)
pub async fn online_stream_connect() -> Result<(mpsc::Sender<Vec<f32>>, mpsc::Receiver<String>), String> {
    let port = super::server::online_server_port();
    let url = format!("ws://127.0.0.1:{}", port);

    let (ws_stream, _) = connect_async(&url)
        .await
        .map_err(|e| format!("连接 online server 失败: {}", e))?;

    let (mut write, mut read) = ws_stream.split();

    let (audio_tx, mut audio_rx) = mpsc::channel::<Vec<f32>>(8);
    let (text_tx, text_rx) = mpsc::channel::<String>(8);

    // Spawn send task: audio_rx → WS
    tokio::spawn(async move {
        while let Some(samples) = audio_rx.recv().await {
            let bytes: Vec<u8> = samples.iter().flat_map(|&s| s.to_le_bytes()).collect();
            if write.send(Message::Binary(bytes)).await.is_err() {
                break;
            }
        }
        // Send "Done" to signal end
        let _ = write.send(Message::Text("Done".into())).await;
    });

    // Spawn receive task: WS → text_tx
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            match msg {
                Message::Text(t) => {
                    if t == "Done!" {
                        break;
                    }
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&t) {
                        if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                            let _ = text_tx.send(text.to_string()).await;
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    Ok((audio_tx, text_rx))
}
