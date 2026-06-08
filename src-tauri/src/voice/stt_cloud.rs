use crate::config::AppConfig;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::io::{Read, Write};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

const PROTOCOL_VERSION: u8 = 0b0001;
const HEADER_SIZE: u8 = 0b0001;
const MSG_TYPE_FULL_CLIENT_REQUEST: u8 = 0b0001;
const MSG_TYPE_AUDIO_ONLY: u8 = 0b0010;
const MSG_TYPE_SERVER_ACK: u8 = 0b1001;
const MSG_TYPE_SERVER_RESPONSE: u8 = 0b1011;
const MSG_TYPE_ERROR: u8 = 0b1111;
const SERIAL_JSON: u8 = 0b0001;
const COMPRESS_GZIP: u8 = 0b0001;
const COMPRESS_NONE: u8 = 0b0000;

pub async fn transcribe(samples: &[f32], cfg: &AppConfig) -> Result<String, String> {
    let app_id = &cfg.voice_cloud.volc_app_id;
    let access_token = &cfg.voice_cloud.volc_access_token;

    if app_id.is_empty() || access_token.is_empty() {
        return Err("云端引擎未配置 App ID 或 Access Token".to_string());
    }

    // Convert f32 to 16-bit PCM bytes
    let pcm_bytes: Vec<u8> = samples
        .iter()
        .flat_map(|&s| {
            let scaled = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            scaled.to_le_bytes().to_vec()
        })
        .collect();

    let url = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel";
    let mut request = url.into_client_request().map_err(|e| e.to_string())?;
    let headers = request.headers_mut();
    headers.insert("X-Api-App-Key", app_id.parse().unwrap());
    headers.insert("X-Api-Access-Key", access_token.parse().unwrap());
    headers.insert("X-Api-Resource-Id", "volc.bigasr.sauc.duration".parse().unwrap());
    headers.insert("X-Api-Connect-Id", uuid().parse().unwrap());

    let (mut ws, _) = connect_async(request)
        .await
        .map_err(|e| format!("连接火山引擎失败: {}", e))?;

    // Send init request
    let init_json = serde_json::json!({
        "app": { "appid": app_id, "token": "access_token", "cluster": "volc.bigasr.sauc.duration" },
        "user": { "uid": "purevoice" },
        "audio": { "format": "pcm", "rate": 16000, "bits": 16, "channel": 1, "language": "zh-CN" },
        "request": { "model_name": "bigmodel", "enable_itn": true, "enable_punc": true, "result_type": "full" }
    });

    let init_frame = marshal(MSG_TYPE_FULL_CLIENT_REQUEST, COMPRESS_GZIP, &init_json.to_string())?;
    ws.send(Message::Binary(init_frame.into()))
        .await
        .map_err(|e| format!("发送初始化帧失败: {}", e))?;

    // Wait for init ack
    wait_for_ack(&mut ws).await?;

    // Send audio in chunks (6400 bytes = ~200ms at 16kHz 16-bit mono)
    let chunk_size = 6400;
    for chunk in pcm_bytes.chunks(chunk_size) {
        let frame = marshal(MSG_TYPE_AUDIO_ONLY, COMPRESS_NONE, "")?;
        let mut payload = frame;
        payload.extend_from_slice(chunk);
        ws.send(Message::Binary(payload.into()))
            .await
            .map_err(|e| format!("发送音频数据失败: {}", e))?;
    }

    // Send final empty audio
    let final_frame = marshal(MSG_TYPE_AUDIO_ONLY, COMPRESS_GZIP, "")?;
    ws.send(Message::Binary(final_frame.into()))
        .await
        .map_err(|e| format!("发送结束帧失败: {}", e))?;

    // Collect results
    let mut final_text = String::new();
    while let Some(msg) = ws.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                let bytes: Vec<u8> = data.into();
                if bytes.len() < 4 {
                    continue;
                }
                let msg_type = (bytes[1] >> 4) & 0x0F;
                let compression = bytes[1] & 0x0F;

                match msg_type {
                    MSG_TYPE_SERVER_ACK => continue,
                    MSG_TYPE_SERVER_RESPONSE => {
                        let payload = &bytes[4..];
                        let text = if compression == COMPRESS_GZIP {
                            decompress_gzip(payload)?
                        } else {
                            String::from_utf8_lossy(payload).to_string()
                        };
                        if let Ok(json) = serde_json::from_str::<Value>(&text) {
                            if let Some(result) = json.get("result") {
                                if let Some(t) = result.get("text").and_then(|v| v.as_str()) {
                                    final_text = t.to_string();
                                }
                                if result.get("is_final").and_then(|v| v.as_bool()).unwrap_or(false) {
                                    break;
                                }
                            }
                        }
                    }
                    MSG_TYPE_ERROR => {
                        let payload = &bytes[4..];
                        let text = if compression == COMPRESS_GZIP {
                            decompress_gzip(payload).unwrap_or_default()
                        } else {
                            String::from_utf8_lossy(payload).to_string()
                        };
                        return Err(format!("火山引擎错误: {}", text));
                    }
                    _ => continue,
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => return Err(format!("WebSocket 错误: {}", e)),
            _ => {}
        }
    }

    let _ = ws.close(None).await;
    Ok(final_text)
}

async fn wait_for_ack(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<(), String> {
    if let Some(msg) = ws.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                let bytes: Vec<u8> = data.into();
                if bytes.len() >= 2 {
                    let msg_type = (bytes[1] >> 4) & 0x0F;
                    if msg_type == MSG_TYPE_SERVER_ACK {
                        return Ok(());
                    }
                    if msg_type == MSG_TYPE_ERROR {
                        return Err("初始化失败".to_string());
                    }
                }
                Ok(())
            }
            Err(e) => Err(format!("等待确认失败: {}", e)),
            _ => Ok(()),
        }
    } else {
        Err("连接已关闭".to_string())
    }
}

fn marshal(msg_type: u8, compression: u8, payload: &str) -> Result<Vec<u8>, String> {
    let mut header = vec![
        (PROTOCOL_VERSION << 4) | HEADER_SIZE,
        (msg_type << 4) | compression,
        0, // reserved
        SERIAL_JSON,
    ];

    if payload.is_empty() {
        // Audio-only: 4 byte header + 4 byte size (0)
        header.extend_from_slice(&0u32.to_be_bytes());
        return Ok(header);
    }

    let compressed = compress_gzip(payload.as_bytes())?;
    let size = compressed.len() as u32;
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&compressed);
    Ok(header)
}

fn compress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

fn decompress_gzip(data: &[u8]) -> Result<String, String> {
    let mut decoder = GzDecoder::new(data);
    let mut output = String::new();
    decoder.read_to_string(&mut output).map_err(|e| e.to_string())?;
    Ok(output)
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", t)
}
