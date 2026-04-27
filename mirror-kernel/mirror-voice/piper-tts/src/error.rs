use thiserror::Error;

#[derive(Error, Debug)]
pub enum PiperTtsError {
    #[error("Failed to load ONNX model: {0}")]
    ModelLoadError(String),

    #[error("Failed to run inference: {0}")]
    InferenceError(String),

    #[error("No audio data available")]
    NoAudioData,

    #[error("Audio output failed: {0}")]
    AudioOutputError(String),

    #[error("Invalid voice file path: {0}")]
    InvalidVoicePath(String),

    #[error("Failed to find voice config file: {0}")]
    MissingVoiceConfig(String),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

impl From<ort::Error> for PiperTtsError {
    fn from(err: ort::Error) -> Self {
        PiperTtsError::ModelLoadError(err.to_string())
    }
}

impl From<rodio::StreamError> for PiperTtsError {
    fn from(err: rodio::StreamError) -> Self {
        PiperTtsError::AudioOutputError(err.to_string())
    }
}

impl From<rodio::decoder::DecoderError> for PiperTtsError {
    fn from(err: rodio::decoder::DecoderError) -> Self {
        PiperTtsError::AudioOutputError(err.to_string())
    }
}

impl From<rodio::PlayError> for PiperTtsError {
    fn from(err: rodio::PlayError) -> Self {
        PiperTtsError::AudioOutputError(err.to_string())
    }
}

impl From<hound::Error> for PiperTtsError {
    fn from(err: hound::Error) -> Self {
        PiperTtsError::AudioOutputError(err.to_string())
    }
}
