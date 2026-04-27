# Kickstart: `piper-tts` — native Rust TTS crate wrapping Piper

## Role

You are a senior Rust systems engineer. Prioritize legibility over cleverness.
Explicit data flow over magic. Minimal abstractions — only introduce a layer when
it earns its place by removing real complexity. Think in:
**partition → transform → reduce → reindex → repeat.**

`.unwrap()` and `.clone()` are intentional prototyping style, not debt. Do not
flag them unless asked. We tighten later once the shape is right.

---

## What we are building

A Rust crate — working name `piper-tts` — that wraps the Piper neural TTS engine
and exposes a clean, ergonomic API. The goal is a crate that does not exist yet:
`cargo add piper-tts` and you have a good local voice in your Rust program.

**The gap:** `whisper-rs` solved the STT (speech-to-text) side cleanly with
well-maintained bindings to whisper.cpp. The TTS side has no equivalent. The `tts`
crate wraps system backends (Speech Dispatcher on Linux) which sound robotic.
`msedge-tts` phones home to Microsoft. Nothing gives you a quality local neural
voice via a clean Rust API. This crate is that thing.

**Prior art to model:** `whisper-rs` — study its API surface, how it wraps a C++
inference engine, how it handles model loading, and how it exposes feature flags
for GPU backends. We want the same feel on the TTS side.

---

## Technical context

- **Target platform:** Arch Linux, Wayland, Nushell shell
- **GPU:** NVIDIA RTX 4070 Ti Super (16GB VRAM) — CUDA available and preferred for
  inference. Do not pretend GPU acceleration is not available.
- **Piper:** the underlying TTS engine. It runs ONNX models. Voices are `.onnx` +
  `.onnx.json` pairs. The developer has used Piper directly before and knows how
  it feels from the user side.
- **Audio playback:** use `rodio` for playback. It is the established Rust audio
  crate and the ecosystem knows it.
- **ONNX runtime:** `ort` crate (ONNX Runtime bindings for Rust) is the likely
  path for running Piper models natively. Evaluate whether to shell out to the
  Piper binary vs. running inference directly via `ort`. Direct inference preferred
  if the complexity is manageable in a prototype.

---

## API shape to aim for

The ideal ergonomic target — something like:

```rust
use piper_tts::{PiperVoice, AudioOutput};

fn main() -> anyhow::Result<()> {
    let voice = PiperVoice::load("./voices/en_US-lessac-medium.onnx")?;
    let audio = voice.synthesize("overhead allocation needs review")?;
    audio.play()?;
    Ok(())
}
```

Optionally write to file:

```rust
audio.save("output.wav")?;
```

That is the target feel. Work toward it. Do not over-engineer the path to get
there — a prototype that makes that API work is the first milestone.

---

## Prototype phase rules

- Prefer `anyhow` for error handling in the prototype. Typed errors come later.
- `unwrap()` and `clone()` are fine. We are finding the shape, not shipping to prod.
- Do not introduce traits or generics until there is a concrete second implementor
  that justifies them.
- Start with synchronous API. Async is a later concern.
- One Cargo feature flag early: `cuda` — mirrors the whisper-rs pattern and sets
  up the GPU path cleanly without making it mandatory.
- Model loading should be explicit, not magic. No auto-discovery of voice files.
  The caller passes a path. Simple.

---

## Crate structure to start with

```
piper-tts/
  Cargo.toml
  build.rs          # if needed for native lib linking
  src/
    lib.rs          # public API surface
    voice.rs        # PiperVoice — model loading + synthesis
    audio.rs        # AudioOutput — playback + file write via rodio
    error.rs        # Error type (anyhow in prototype, typed later)
  voices/           # gitignored, local model files go here
  examples/
    basic.rs        # the 6-line example from the API shape above
```

---

## First task

Scaffold the crate. Get `PiperVoice::load()` to successfully load an ONNX model
file using `ort`. Get `voice.synthesize("hello world")` to return raw PCM samples.
Get `audio.play()` to produce audible output via `rodio`. That is the first
working prototype. Nothing else matters until that loop closes.

If shelling out to the Piper binary is faster to close the loop than `ort`
directly, do that first and note where the native inference swap point would be.
Pragmatism first.

---

## What success looks like

A developer on Arch Linux with a Piper voice file downloaded can:

```toml
[dependencies]
piper-tts = "0.1"
```

Write 6 lines of Rust, `cargo run`, and hear their text spoken locally with a
quality neural voice. No Python. No external service. No robot voice.

That is the crate that does not exist yet. Build it.
