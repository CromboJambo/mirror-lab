use piper_tts::{AudioOutput, PiperVoice};
use tray_icon::{MenuBuilder, MenuItem, TrayIconBuilder};
use wl_clipboard_rs::copy::ClipboardCopy;
use wl_clipboard_rs::copy::MimeType;

// MAIN
fn main() -> anyhow::Result<()> {
    let menu = MenuBuilder::new()
        .item(MenuItem::new("Listen & Paste"))
        .item(MenuItem::new("Read Selection"))
        .item(MenuItem::new("Quit"))
        .build();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_icon(tray_icon::icon::Icon::from_file("mic.png").unwrap())
        .build()?;

    // Event loop
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

    // Prevent app exit
    loop {
        std::thread::park();
    }
}

fn listen_and_paste() {
    // 1. Record mic audio
    let _ = Command::new("pw-cat")
        .args(&["--record", "/tmp/mirror.wav"])
        .status();

    // 2. Whisper STT
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

    // 3. Read result
    let txt = std::fs::read_to_string("/tmp/mirror.txt").unwrap_or_default();

    // 4. Put into wl-clipboard without touching Plasma history metadata
    let _ = ClipboardCopy::new(MimeType::Text, txt.as_bytes()).copy();

    // 5. Paste into active window
    let _ = Command::new("wtype")
        .args(&["-M", "ctrl", "v", "-m", "ctrl"])
        .status();
}

fn read_selection() {
    // Use wl-paste to fetch primary
    let output = Command::new("wl-paste")
        .arg("--primary")
        .output()
        .expect("failed to run wl-paste");

    let text = String::from_utf8_lossy(&output.stdout);

    if !text.is_empty() {
        // Load Piper voice and synthesize
        match PiperVoice::load(
            "/home/crombo/mirror-lab/mirror_voice/voices/en_US-lessac-medium.onnx",
        ) {
            Ok(voice) => {
                let audio = voice.synthesize(&text).unwrap();
                AudioOutput::new(voice.sample_rate()).play(&audio).unwrap();
            }
            Err(e) => eprintln!("Failed to load Piper voice: {}", e),
        }
    }
}
