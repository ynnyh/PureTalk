use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

struct Recording {
    _stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

static STATE: Lazy<Mutex<Option<Recording>>> = Lazy::new(|| Mutex::new(None));

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

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    samples_clone.lock().unwrap().extend_from_slice(data);
                },
                |err| eprintln!("录音错误: {}", err),
                None,
            )
            .map_err(|e| format!("创建音频流失败: {}", e))?,
        cpal::SampleFormat::I16 => {
            let s = samples_clone.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mut buf = s.lock().unwrap();
                        for &sample in data {
                            buf.push(sample as f32 / i16::MAX as f32);
                        }
                    },
                    |err| eprintln!("录音错误: {}", err),
                    None,
                )
                .map_err(|e| format!("创建音频流失败: {}", e))?
        }
        cpal::SampleFormat::U16 => {
            let s = samples_clone.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let mut buf = s.lock().unwrap();
                        for &sample in data {
                            buf.push(sample as f32 / u16::MAX as f32 * 2.0 - 1.0);
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
