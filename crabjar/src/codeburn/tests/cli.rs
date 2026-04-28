use std::process::Command;

#[test]
fn codeburn_help_returns_json() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "-p",
            "codeburn",
            "--",
            "help",
        ])
        .output()
        .expect("failed to run codeburn");

    let stdout = std::str::from_utf8(&output.stdout).expect("invalid utf8");
    assert!(stdout.contains("\"success\": true"));
    assert!(stdout.contains("\"usage"));
}

#[test]
fn codeburn_missing_command_exits_nonzero() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "-p",
            "codeburn",
        ])
        .output()
        .expect("failed to run codeburn");

    assert!(output.status.code().expect("no exit code") != 0);
}

#[test]
fn codeburn_config_soft_failure() {
    let _temp_dir = tempfile::tempdir().expect("failed to create temp dir");

    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "-p",
            "codeburn",
            "--",
            "status",
        ])
        .output()
        .expect("failed to run codeburn");

    let stdout = std::str::from_utf8(&output.stdout).expect("invalid utf8");
    assert!(stdout.contains("\"success\": true"));
}
