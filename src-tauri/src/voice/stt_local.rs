use crate::config::AppConfig;
use crate::voice::assets;

pub async fn transcribe(samples: &[f32], _cfg: &AppConfig) -> Result<String, String> {
    if !assets::assets_ready() {
        return Err("本地语音资产未下载,请先在设置中下载".to_string());
    }

    // Samples are already f32 normalized, 16kHz mono (from recording::stop)
    let sample_rate = 16000;

    super::ws_client::offline_transcribe(samples, sample_rate).await
}
