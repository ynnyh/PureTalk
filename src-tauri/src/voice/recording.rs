use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

struct Recording {
    _stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

static STATE: Lazy<Mutex<Option<Recording>>> = Lazy::new(|| Mutex::new(None));
static STREAM_TX: Lazy<Mutex<Option<mpsc::Sender<Vec<f32>>>>> = Lazy::new(|| Mutex::new(None));

pub fn set_stream_tx(tx: Option<mpsc::Sender<Vec<f32>>>) {
    let mut guard = STREAM_TX.lock().unwrap();
    *guard = tx;
}

pub fn start() -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "未找到音频输入设备".to_string())?;

    let supported = device
        .default_input_config()
        .map_err(|e| format!("获取音频配置失败: {}", e))?;

    let sample_rate = supported.sample_rate();
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();

    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let samples_clone = samples.clone();

    // Streaming buffer: accumulate ~150ms chunks before sending
    let stream_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let chunk_size = (sample_rate as f32 * 0.15) as usize; // 150ms at input sample rate

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let sb = stream_buffer.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        samples_clone.lock().unwrap().extend_from_slice(data);

                        // Push to streaming buffer
                        let mut buf = sb.lock().unwrap();
                        buf.extend_from_slice(data);
                        if buf.len() >= chunk_size {
                            let chunk: Vec<f32> = buf.drain(..chunk_size).collect();
                            drop(buf);

                            // Try send to stream (non-blocking)
                            if let Some(tx) = STREAM_TX.lock().unwrap().clone() {
                                let _ = tx.try_send(chunk);
                            }
                        }
                    },
                    |err| eprintln!("录音错误: {}", err),
                    None,
                )
                .map_err(|e| format!("创建音频流失败: {}", e))?
        },
        cpal::SampleFormat::I16 => {
            let s = samples_clone.clone();
            let sb = stream_buffer.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mut buf = s.lock().unwrap();
                        let mut stream_buf = sb.lock().unwrap();
                        for &sample in data {
                            let f = sample as f32 / i16::MAX as f32;
                            buf.push(f);
                            stream_buf.push(f);
                        }
                        drop(buf);

                        if stream_buf.len() >= chunk_size {
                            let chunk: Vec<f32> = stream_buf.drain(..chunk_size).collect();
                            drop(stream_buf);
                            if let Some(tx) = STREAM_TX.lock().unwrap().clone() {
                                let _ = tx.try_send(chunk);
                            }
                        }
                    },
                    |err| eprintln!("录音错误: {}", err),
                    None,
                )
                .map_err(|e| format!("创建音频流失败: {}", e))?
        }
        cpal::SampleFormat::U16 => {
            let s = samples_clone.clone();
            let sb = stream_buffer.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let mut buf = s.lock().unwrap();
                        let mut stream_buf = sb.lock().unwrap();
                        for &sample in data {
                            let f = sample as f32 / u16::MAX as f32 * 2.0 - 1.0;
                            buf.push(f);
                            stream_buf.push(f);
                        }
                        drop(buf);

                        if stream_buf.len() >= chunk_size {
                            let chunk: Vec<f32> = stream_buf.drain(..chunk_size).collect();
                            drop(stream_buf);
                            if let Some(tx) = STREAM_TX.lock().unwrap().clone() {
                                let _ = tx.try_send(chunk);
                            }
                        }
                    },
                    |err| eprintln!("录音错误: {}", err),
                    None,
                )
                .map_err(|e| format!("创建音频流失败: {}", e))?
        }
        fmt => return Err(format!("不支持的音频格式: {:?}", fmt)),
    };

    stream.play().map_err(|e| format!("启动录音失败: {}", e))?;

    let mut state = STATE.lock().unwrap();
    *state = Some(Recording {
        _stream: stream,
        samples,
        sample_rate,
        channels,
    });

    Ok(())
}

pub fn stop() -> Result<Vec<f32>, String> {
    let mut state = STATE.lock().unwrap();
    let recording = state.take().ok_or_else(|| "未在录音中".to_string())?;

    let raw_samples = {
        let mut buf = recording.samples.lock().unwrap();
        std::mem::take(&mut *buf)
    };

    // Convert to mono
    let mono = to_mono(&raw_samples, recording.channels);

    // Resample to 16kHz
    let resampled = resample_to_16k(&mono, recording.sample_rate);

    Ok(resampled)
}

fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    samples
        .chunks(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn resample_to_16k(samples: &[f32], src_rate: u32) -> Vec<f32> {
    if src_rate == 16000 {
        return samples.to_vec();
    }
    // Linear interpolation (simple, fast, good enough for voice)
    // TODO: upgrade to rubato FFT/sinc for even better quality if needed
    let ratio = src_rate as f64 / 16000.0;
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = pos - idx as f64;
        let val = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac as f32) + samples[idx + 1] * frac as f32
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0.0
        };
        output.push(val);
    }
    output
}

/// 能量门限静音裁剪(trim head/tail silence)
pub fn trim_silence_energy(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    // Energy threshold (RMS-based)
    let frame_size = 400; // ~25ms at 16kHz
    let threshold = 0.02; // empirical, adjust as needed

    let frames_energy: Vec<f32> = samples
        .chunks(frame_size)
        .map(|chunk| {
            let sum: f32 = chunk.iter().map(|&s| s * s).sum();
            (sum / chunk.len() as f32).sqrt()
        })
        .collect();

    if frames_energy.is_empty() {
        return samples.to_vec();
    }

    // Find first non-silent frame
    let start_frame = frames_energy
        .iter()
        .position(|&e| e >= threshold)
        .unwrap_or(0);

    // Find last non-silent frame
    let end_frame = frames_energy
        .iter()
        .rposition(|&e| e >= threshold)
        .unwrap_or(frames_energy.len() - 1);

    if start_frame >= end_frame {
        return samples.to_vec();
    }

    let start_sample = start_frame * frame_size;
    let end_sample = ((end_frame + 1) * frame_size).min(samples.len());

    samples[start_sample..end_sample].to_vec()
}
