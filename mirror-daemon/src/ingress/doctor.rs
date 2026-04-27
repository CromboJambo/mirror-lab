use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::ingress::config::Config;

pub fn run(config: &Config) -> Result<()> {
    let mut failures = Vec::new();

    check_command("ffmpeg", &["-version"], &mut failures);
    check_command("ffprobe", &["-version"], &mut failures);
    check_command("auto-editor", &["--version"], &mut failures);

    check_path_exists(
        "capture.watch_dir",
        &config.capture.watch_dir,
        &mut failures,
    );
    check_parent_writable("storage.db_path", &config.storage.db_path, &mut failures);
    check_path_exists(
        "storage.chunks_dir",
        &config.storage.chunks_dir,
        &mut failures,
    );
    check_path_exists(
        "processing.staging_dir",
        &config.processing.staging_dir,
        &mut failures,
    );

    if config.transcription.enabled {
        #[cfg(not(feature = "transcription"))]
        failures.push(
            "transcription.enabled=true but binary was built without `--features transcription`"
                .to_string(),
        );

        if let Some(model_path) = config.transcription.model_path.as_ref() {
            check_path_exists("transcription.model_path", model_path, &mut failures);
        } else {
            failures.push(
                "transcription.enabled=true but transcription.model_path is missing".to_string(),
            );
        }
    }

    if failures.is_empty() {
        println!("doctor: OK");
        return Ok(());
    }

    println!("doctor: FAILED");
    for f in failures {
        println!(" - {}", f);
    }
    bail!("one or more doctor checks failed")
}

fn check_command(cmd: &str, args: &[&str], failures: &mut Vec<String>) {
    match Command::new(cmd).args(args).output() {
        Ok(output) if output.status.success() => {}
        Ok(output) => failures.push(format!(
            "{cmd} present but returned non-zero exit status: {}",
            output.status
        )),
        Err(_) => failures.push(format!("{cmd} not found in PATH")),
    }
}

fn check_path_exists(label: &str, path: &Path, failures: &mut Vec<String>) {
    if !path.exists() {
        failures.push(format!("{label} does not exist: {}", path.display()));
    }
}

fn check_parent_writable(label: &str, path: &Path, failures: &mut Vec<String>) {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        failures.push(format!(
            "{label} parent does not exist: {}",
            parent.display()
        ));
        return;
    }

    let test_file = parent.join(format!(".ingress_doctor_write_test_{}", std::process::id()));
    match std::fs::File::create(&test_file).context("write test") {
        Ok(_) => {
            let _ = std::fs::remove_file(test_file);
        }
        Err(_) => {
            failures.push(format!(
                "{label} parent is not writable: {}",
                parent.display()
            ));
        }
    }
}
