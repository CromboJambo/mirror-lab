use ort::session::Session;
use ort::{inputs, value::TensorRef};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::error::PiperTtsError;

/// Voice configuration file structure (.onnx.json)
#[derive(Debug, Deserialize)]
struct VoiceConfig {
    audio: AudioConfig,
    espeak: EspeakConfig,
    inference: InferenceConfig,
    phoneme_id_map: HashMap<String, Vec<i64>>,
    #[allow(dead_code)]
    num_symbols: u32,
}

#[derive(Debug, Deserialize)]
struct AudioConfig {
    sample_rate: u32,
}

#[derive(Debug, Deserialize)]
struct EspeakConfig {
    voice: String,
}

#[derive(Debug, Deserialize)]
struct InferenceConfig {
    noise_scale: f32,
    length_scale: f32,
    noise_w: f32,
}

/// PiperVoice represents a loaded neural TTS voice
pub struct PiperVoice {
    session: Session,
    sample_rate: u32,
    phoneme_id_map: HashMap<String, i64>,
    espeak_voice: String,
    noise_scale: f32,
    length_scale: f32,
    noise_w: f32,
}

impl PiperVoice {
    /// Load a Piper voice from ONNX model file
    pub fn load<P: AsRef<Path>>(voice_path: P) -> Result<Self, PiperTtsError> {
        let voice_path = voice_path.as_ref();

        if !voice_path.exists() {
            return Err(PiperTtsError::InvalidVoicePath(
                voice_path.display().to_string(),
            ));
        }

        // Load voice configuration
        let config_path = voice_path.with_extension("onnx.json");
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| PiperTtsError::MissingVoiceConfig(e.to_string()))?;

        let config: VoiceConfig =
            serde_json::from_str(&config_content).map_err(PiperTtsError::SerdeError)?;

        // Flatten phoneme_id_map from HashMap<String, Vec<i64>> to HashMap<String, i64>
        let mut phoneme_id_map: HashMap<String, i64> = HashMap::new();
        for (phoneme, ids) in config.phoneme_id_map {
            if let Some(&id) = ids.first() {
                phoneme_id_map.insert(phoneme, id);
            }
        }

        // Load ONNX model
        let model_path = voice_path;
        let session = Session::builder()?.commit_from_file(model_path)?;

        Ok(PiperVoice {
            session,
            sample_rate: config.audio.sample_rate,
            phoneme_id_map,
            espeak_voice: config.espeak.voice,
            noise_scale: config.inference.noise_scale,
            length_scale: config.inference.length_scale,
            noise_w: config.inference.noise_w,
        })
    }

    /// Convert text to phoneme ID sequence
    fn text_to_phoneme_ids(&self, text: &str) -> Result<Vec<i64>, PiperTtsError> {
        // Step 1: Text → phonemes via espeak
        let phonemes = espeak_rs::text_to_phonemes(text, &self.espeak_voice, None, true, false)
            .map_err(|e| PiperTtsError::PhonemizationError(e.to_string()))?;

        let phoneme_str = phonemes.join("");

        // Step 2: Phoneme string → phoneme IDs
        let mut ids: Vec<i64> = Vec::new();

        // BOS token (^, id=1)
        if let Some(&bos_id) = self.phoneme_id_map.get("^") {
            ids.push(bos_id);
        }

        for ch in phoneme_str.chars() {
            if let Some(&id) = self.phoneme_id_map.get(&ch.to_string()) {
                ids.push(id);
            }
        }

        // EOS token ($, id=2)
        if let Some(&eos_id) = self.phoneme_id_map.get("$") {
            ids.push(eos_id);
        }

        Ok(ids)
    }

    /// Synthesize text to audio
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<f32>, PiperTtsError> {
        if text.is_empty() {
            return Err(PiperTtsError::NoAudioData);
        }

        // Convert text to phoneme IDs
        let phoneme_ids = self.text_to_phoneme_ids(text)?;

        let seq_len = phoneme_ids.len();

        // Build input tensors
        // input: int64 [batch_size=1, phonemes]
        let input_tensor = TensorRef::from_array_view(([1usize, seq_len], &phoneme_ids[..]))
            .map_err(|e| PiperTtsError::InferenceError(e.to_string()))?;

        // input_lengths: int64 [batch_size=1]
        let lengths: Vec<i64> = vec![seq_len as i64];
        let lengths_tensor = TensorRef::from_array_view(([1usize], &lengths[..]))
            .map_err(|e| PiperTtsError::InferenceError(e.to_string()))?;

        // scales: float [3] — noise_scale, length_scale, noise_w
        let scales: [f32; 3] = [self.noise_scale, self.length_scale, self.noise_w];
        let scales_tensor = TensorRef::from_array_view(([3usize], &scales[..]))
            .map_err(|e| PiperTtsError::InferenceError(e.to_string()))?;

        // Run inference
        let outputs = self
            .session
            .run(inputs![
                "input" => input_tensor,
                "input_lengths" => lengths_tensor,
                "scales" => scales_tensor,
            ])
            .map_err(|e| PiperTtsError::InferenceError(e.to_string()))?;

        // Extract output tensor: float [1, 1, 1, audio_samples]
        let (_, audio_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| PiperTtsError::InferenceError(e.to_string()))?;

        Ok(audio_data.to_vec())
    }

    /// Get the sample rate of this voice
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_voice_load_missing_model_returns_invalid_path() {
        let tmp = TempDir::new().unwrap();
        let result = PiperVoice::load(tmp.path().join("nonexistent.onnx"));
        assert!(matches!(result, Err(PiperTtsError::InvalidVoicePath(_))));
    }

    #[test]
    fn test_voice_load_missing_config_returns_missing_config() {
        let tmp = TempDir::new().unwrap();
        let model_path = tmp.path().join("model.onnx");
        std::fs::write(&model_path, "dummy").unwrap();
        let result = PiperVoice::load(&model_path);
        assert!(matches!(result, Err(PiperTtsError::MissingVoiceConfig(_))));
    }

    #[test]
    fn test_voice_config_parse_valid_json() {
        let config_json = serde_json::json!({
            "audio": { "sample_rate": 16000 },
            "espeak": { "voice": "en-us" },
            "inference": { "noise_scale": 0.667, "length_scale": 1.0, "noise_w": 0.0 },
            "phoneme_id_map": { "^": [1], "$": [2], "a": [10] },
            "num_symbols": 100
        });

        let config: VoiceConfig = serde_json::from_str(&config_json.to_string()).unwrap();
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.espeak.voice, "en-us");
        assert_eq!(config.inference.noise_scale, 0.667);
        assert_eq!(config.phoneme_id_map.get("^"), Some(&vec![1]));
    }

    #[test]
    fn test_voice_config_parse_invalid_json() {
        let invalid_json = "not valid json";
        let result: Result<VoiceConfig, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_phoneme_map_flatten_bos_eos() {
        let raw_map: HashMap<String, Vec<i64>> = HashMap::from([
            ("^".to_string(), vec![1]),
            ("$".to_string(), vec![2]),
            ("a".to_string(), vec![10]),
        ]);

        let mut flattened: HashMap<String, i64> = HashMap::new();
        for (phoneme, ids) in raw_map {
            if let Some(&id) = ids.first() {
                flattened.insert(phoneme, id);
            }
        }

        assert_eq!(flattened.get("^"), Some(&1));
        assert_eq!(flattened.get("$"), Some(&2));
        assert_eq!(flattened.get("a"), Some(&10));
    }

    #[test]
    fn test_phoneme_map_flatten_missing_first() {
        let raw_map: HashMap<String, Vec<i64>> =
            HashMap::from([("^".to_string(), vec![]), ("$".to_string(), vec![2])]);

        let mut flattened: HashMap<String, i64> = HashMap::new();
        for (phoneme, ids) in raw_map {
            if let Some(&id) = ids.first() {
                flattened.insert(phoneme, id);
            }
        }

        assert!(flattened.get("^").is_none());
        assert_eq!(flattened.get("$"), Some(&2));
    }
}
