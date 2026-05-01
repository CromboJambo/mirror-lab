use piper_rs::Piper;
use rodio::{buffer::SamplesBuffer, Sink, OutputStream};
use std::path::Path;
use std::process::Command;
use tray_icon::{MenuBuilder, MenuItem, TrayIconBuilder};
use wl_clipboard_rs::copy::{ClipboardCopy, MimeType};

const VOICE_MODEL: &str = "./piper-tts/voices/en_US-lessac-medium.onnx";
const VOICE_CONFIG: &str = "./piper-tts/voices/en_US-lessac-medium.onnx.json";

fn main() -> anyhow::Result<()> {
    let menu = MenuBuilder::new()
        .item(MenuItem::new("Listen & Paste"))
        .item(MenuItem::new("Read Selection"))
        .item(MenuItem::new("Quit"))
        .build();

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_icon(tray_icon::icon::Icon::from_file("mic.png").unwrap())
        .build()?;

    tray_icon::TrayIconEvent::set_handler(move |event| {
        if let tray_icon::TrayIconEvent::MenuItemClick(id) = event {
            let title = menu.get_item(id).unwrap().title();

            match title.as_str() {
                "Listen & Paste" => {
                    listen_and_paste();
                }
                "Read Selection" => {
                    read_selection();
                }
                "Quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    });

    loop {
        std::thread::park();
    }
}

fn listen_and_paste() {
    let _ = Command::new("pw-cat")
        .args(&["--record", "/tmp/mirror.wav"])
        .status();

    let _ = Command::new("whisper.cpp")
        .args(&[
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

    let _ = ClipboardCopy::new(MimeType::Text, txt.as_bytes()).copy();

    let _ = Command::new("wtype")
        .args(&["-M", "ctrl", "v", "-m", "ctrl"])
        .status();
}

fn read_selection() {
    let output = Command::new("wl-paste")
        .arg("--primary")
        .output()
        .expect("failed to run wl-paste");

    let text = String::from_utf8_lossy(&output.stdout);

    if text.is_empty() {
        return;
    }

    match Piper::new(Path::new(VOICE_MODEL), Path::new(VOICE_CONFIG)) {
        Ok(piper) => {
            match piper.create(&text, false, None, None, None, None) {
                Ok((samples, sample_rate)) => {
                    let (_stream, handle) = OutputStream::try_default().unwrap();
                    let sink = Sink::try_new(&handle).unwrap();
                    sink.append(SamplesBuffer::new(1, sample_rate, samples));
                    sink.sleep_until_end();
                }
                Err(e) => eprintln!("Synthesis failed: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to load Piper voice: {}", e),
    }
}
