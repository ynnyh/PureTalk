use crate::config::AppConfig;
use crate::voice::assets;
use std::process::Command;

pub fn transcribe(samples: &[f32], _cfg: &AppConfig) -> Result<String, String> {
    if !assets::assets_ready() {
        return Err("本地语音资产未下载，请先在设置中下载".to_string());
    }

    let wav_path = write_temp_wav(samples)?;
    let text = run_sherpa(&wav_path)?;
    let _ = std::fs::remove_file(&wav_path);
    Ok(text)
}

fn write_temp_wav(samples: &[f32]) -> Result<String, String> {
    let path = std::env::temp_dir().join("purevoice_input.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(&path, spec).map_err(|e| format!("创建 WAV 失败: {}", e))?;
    for &s in samples {
        let scaled = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer
            .write_sample(scaled)
            .map_err(|e| format!("写入 WAV 失败: {}", e))?;
    }
    writer.finalize().map_err(|e| format!("完成 WAV 写入失败: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

fn run_sherpa(wav_path: &str) -> Result<String, String> {
    let bin = assets::sherpa_offline_path();
    let model = assets::model_path();
    let tokens = assets::tokens_path();

    let threads = num_cpus().min(4);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = Command::new(&bin)
            .args([
                &format!("--tokens={}", tokens),
                &format!("--sense-voice-model={}", model),
                &format!("--num-threads={}", threads),
                "--sense-voice-use-itn=1",
                "--debug=0",
                wav_path,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("执行 sherpa-onnx 失败: {}", e))?;

        parse_output(output)
    }

    #[cfg(not(windows))]
    {
        let output = Command::new(&bin)
            .args([
                &format!("--tokens={}", tokens),
                &format!("--sense-voice-model={}", model),
                &format!("--num-threads={}", threads),
                "--sense-voice-use-itn=1",
                "--debug=0",
                wav_path,
            ])
            .output()
            .map_err(|e| format!("执行 sherpa-onnx 失败: {}", e))?;

        parse_output(output)
    }
}

fn parse_output(output: std::process::Output) -> Result<String, String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("sherpa-onnx 执行失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("解析 sherpa-onnx 输出失败: {}", e))?;

    json.get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "sherpa-onnx 输出中未找到 text 字段".to_string())
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
}
