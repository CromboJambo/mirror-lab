// src/transcription.rs
//
// Transcription layer using whisper-rs bindings to whisper.cpp.

use anyhow::Result;
use std::path::Path;
#[cfg(feature = "transcription")]
use tracing::info;
use tracing::warn;

use crate::ingress::config::TranscriptionConfig;

#[cfg(feature = "transcription")]
pub fn transcribe_chunk(chunk_path: &Path, config: &TranscriptionConfig) -> Result<Option<String>> {
    use anyhow::Context;
    use hound::{SampleFormat, WavReader};
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let model_path = config
        .model_path
        .as_ref()
        .context("transcription.enabled=true but transcription.model_path is not set")?;

    info!("Transcribing chunk: {}", chunk_path.display());
    let wav_path = extract_audio_from_video(chunk_path)?;
    let transcript_result: Result<Option<String>> = (|| {
        let mut reader =
            WavReader::open(&wav_path).context("Failed to open extracted WAV for transcription")?;
        let spec = reader.spec();

        let audio: Vec<f32> = match (spec.sample_format, spec.bits_per_sample, spec.channels) {
            (SampleFormat::Int, 16, 1) => {
                let pcm: Vec<i16> = reader
                    .samples::<i16>()
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to read WAV samples")?;
                let mut audio = vec![0.0f32; pcm.len()];
                whisper_rs::convert_integer_to_float_audio(&pcm, &mut audio)
                    .context("Failed to convert integer WAV samples to f32")?;
                audio
            }
            (SampleFormat::Float, 32, 1) => reader
                .samples::<f32>()
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("Failed to read WAV samples")?,
            (SampleFormat::Int, 16, 2) => {
                let pcm_stereo: Vec<i16> = reader
                    .samples::<i16>()
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to read WAV samples")?;
                let mut stereo_f32 = vec![0.0f32; pcm_stereo.len()];
                whisper_rs::convert_integer_to_float_audio(&pcm_stereo, &mut stereo_f32)
                    .context("Failed to convert integer WAV samples to f32")?;
                whisper_rs::convert_stereo_to_mono_audio(&stereo_f32)
                    .context("Failed to convert stereo audio to mono")?
            }
            _ => {
                anyhow::bail!(
                    "Unsupported WAV format: {:?}, {}-bit, {} channels",
                    spec.sample_format,
                    spec.bits_per_sample,
                    spec.channels
                )
            }
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(false);
        if let Some(threads) = config.threads {
            params.set_n_threads(i32::from(threads));
        }
        if let Some(language) = config.language.as_deref() {
            params.set_language(Some(language));
        }

        let model_str = model_path
            .to_str()
            .context("transcription.model_path is not valid UTF-8")?;
        let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
            .context("Failed to load Whisper model")?;
        let mut state = ctx
            .create_state()
            .context("Failed to create Whisper state")?;
        state
            .full(params, &audio[..])
            .context("Whisper transcription failed")?;

        let mut lines = Vec::new();
        for segment in state.as_iter() {
            let line = segment.to_string().trim().to_string();
            if !line.is_empty() {
                lines.push(line);
            }
        }

        if lines.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lines.join(" ")))
        }
    })();

    // Best effort cleanup of temporary wav file
    if let Err(e) = std::fs::remove_file(&wav_path) {
        warn!(
            "Failed to remove temporary transcription audio {}: {}",
            wav_path.display(),
            e
        );
    }

    transcript_result
}

#[cfg(not(feature = "transcription"))]
pub fn transcribe_chunk(
    _chunk_path: &Path,
    _config: &TranscriptionConfig,
) -> Result<Option<String>> {
    warn!("Transcription feature not enabled - skipping");
    Ok(None)
}

#[cfg(feature = "transcription")]
pub fn extract_audio_from_video(video_path: &Path) -> Result<std::path::PathBuf> {
    use anyhow::Context;
    use chrono::Utc;
    use std::process::Command;

    let mut output = std::env::temp_dir();
    output.push(format!(
        "ingress_transcribe_{}_{}.wav",
        std::process::id(),
        Utc::now().timestamp_millis()
    ));

    info!("Extracting audio from video: {}", video_path.display());

    let status = Command::new("ffmpeg")
        .arg("-i")
        .arg(video_path)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", "-y"])
        .arg(&output)
        .status()
        .context("Failed to run ffmpeg for audio extraction")?;

    if !status.success() {
        anyhow::bail!("ffmpeg audio extraction failed with status {}", status);
    }

    Ok(output)
}
