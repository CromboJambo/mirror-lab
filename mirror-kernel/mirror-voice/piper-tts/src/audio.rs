use anyhow::Result;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::error::PiperTtsError;

/// AudioOutput handles playback and saving of synthesized audio
pub struct AudioOutput {
    sample_rate: u32,
}

impl AudioOutput {
    /// Create a new AudioOutput with the given sample rate
    pub fn new(sample_rate: u32) -> Self {
        AudioOutput { sample_rate }
    }

    /// Play audio data (PCM samples)
    pub fn play(&self, audio_data: &[f32]) -> Result<(), PiperTtsError> {
        if audio_data.is_empty() {
            return Err(PiperTtsError::NoAudioData);
        }

        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;

        // Convert PCM samples to audio format
        let source = rodio::buffer::SamplesBuffer::new(1, self.sample_rate, audio_data.to_vec());

        sink.append(source);
        sink.sleep_until_end();

        Ok(())
    }

    /// Save audio data to a WAV file
    pub fn save<P: AsRef<Path>>(&self, path: P, audio_data: &[f32]) -> Result<(), PiperTtsError> {
        let path = path.as_ref();

        if audio_data.is_empty() {
            return Err(PiperTtsError::NoAudioData);
        }

        // Use hound for WAV encoding
        use hound::{WavSpec, WavWriter};

        let spec = WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let file =
            File::create(path).map_err(|e| PiperTtsError::AudioOutputError(e.to_string()))?;

        let mut writer = WavWriter::new(file, spec)?;

        for &sample in audio_data {
            writer
                .write_sample(sample)
                .map_err(|e| PiperTtsError::AudioOutputError(e.to_string()))?;
        }

        writer
            .finalize()
            .map_err(|e| PiperTtsError::AudioOutputError(e.to_string()))?;

        Ok(())
    }

    /// Play from a WAV file
    pub fn play_file<P: AsRef<Path>>(&self, path: P) -> Result<(), PiperTtsError> {
        let path = path.as_ref();

        let file = File::open(path).map_err(|e| PiperTtsError::AudioOutputError(e.to_string()))?;
        let source = Decoder::new(BufReader::new(file))?;

        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;

        sink.append(source);
        sink.sleep_until_end();

        Ok(())
    }
}
