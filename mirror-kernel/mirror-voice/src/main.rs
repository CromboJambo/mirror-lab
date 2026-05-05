use std::process::Command;
use std::sync::{Arc, Mutex};

use piper_tts::{AudioOutput, PiperVoice};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};

const VOICE_MODEL: &str = "./piper-tts/voices/en_US-lessac-medium.onnx";

fn main() -> anyhow::Result<()> {
    let voice = PiperVoice::load(VOICE_MODEL).map_err(|e| anyhow::anyhow!(e))?;
    let voice = Arc::new(Mutex::new(voice));

    let listen_item = MenuItem::new("Listen & Paste", true, None);
    let read_item = MenuItem::new("Read Selection", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append(&listen_item)?;
    menu.append(&read_item)?;
    menu.append(&quit_item)?;

    let listen_id = listen_item.id().clone();
    let read_id = read_item.id().clone();
    let quit_id = quit_item.id().clone();

    MenuEvent::set_event_handler(Some(Box::new(move |event: MenuEvent| {
        if event.id == listen_id {
            listen_and_paste();
        } else if event.id == read_id {
            let voice = Arc::clone(&voice);
            std::thread::spawn(move || {
                if let Err(e) = read_selection(&voice) {
                    eprintln!("Read selection failed: {}", e);
                }
            });
        } else if event.id == quit_id {
            std::process::exit(0);
        }
    })));

    let icon_data: Vec<u8> = vec![255, 255, 255, 255];
    let icon = Icon::from_rgba(icon_data, 1, 1)
        .map_err(|e| anyhow::anyhow!("failed to create icon: {}", e))?;

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to create tray icon: {}", e))?;

    loop {
        std::thread::park();
    }
}

fn listen_and_paste() {
    let _ = Command::new("pw-cat")
        .args(["--record", "/tmp/mirror.wav"])
        .status();

    let _ = Command::new("whisper.cpp")
        .args([
            "-m",
            "/home/you/models/base.en",
            "-f",
            "/tmp/mirror.wav",
            "-otxt",
            "-o",
            "/tmp",
        ])
        .status();

    let txt = std::fs::read_to_string("/tmp/mirror.txt").unwrap_or_default();

    let _ = wl_clipboard_rs::copy::copy(
        wl_clipboard_rs::copy::Options::new(),
        wl_clipboard_rs::copy::Source::Bytes(txt.as_bytes().to_vec().into_boxed_slice()),
        wl_clipboard_rs::copy::MimeType::Text,
    );

    let _ = Command::new("wtype")
        .args(["-M", "ctrl", "v", "-m", "ctrl"])
        .status();
}

fn read_selection(voice: &Arc<Mutex<PiperVoice>>) -> anyhow::Result<()> {
    let output = Command::new("wl-paste")
        .arg("--primary")
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run wl-paste: {}", e))?;

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if text.is_empty() {
        return Ok(());
    }

    let mut voice = voice
        .lock()
        .map_err(|e| anyhow::anyhow!("voice lock poisoned: {}", e))?;
    let samples = voice.synthesize(&text).map_err(|e| anyhow::anyhow!(e))?;

    let output = AudioOutput::new(voice.sample_rate());
    output.play(&samples).map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}
