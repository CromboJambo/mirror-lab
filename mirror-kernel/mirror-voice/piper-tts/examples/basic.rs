use piper_tts::{AudioOutput, PiperVoice};

fn main() -> anyhow::Result<()> {
    let voice = PiperVoice::load("./voices/en_US-lessac-medium.onnx")?;
    let audio_data = voice.synthesize("overhead allocation needs review")?;

    let audio_output = AudioOutput::new(voice.sample_rate());
    audio_output.save("output.wav", &audio_data)?;
    audio_output.play(&audio_data)?;

    Ok(())
}
