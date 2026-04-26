use zllg::project_pane::{ProjectPaneConfig, apply_overrides};

#[test]
fn test_apply_overrides_watch() {
    let cfg = ProjectPaneConfig::default();
    let (cmd, args) = apply_overrides("nu", &vec![], &cfg);
    assert_eq!(cmd, "cargo watch");
    assert_eq!(args, vec!["-x", "check --message-format short"]);
}

#[test]
fn test_apply_overrides_finds_watch_first() {
    let cfg = ProjectPaneConfig::default();
    let (cmd, args) = apply_overrides("zsh", &vec![], &cfg);
    // watch is checked first in the default config.
    assert_eq!(cmd, "cargo watch");
    assert_eq!(args, vec!["-x", "check --message-format short"]);
}

#[test]
fn test_apply_overrides_no_match() {
    let cfg = ProjectPaneConfig {
        project_type: "python".into(),
        panes: vec![],
    };
    let (cmd, args) = apply_overrides("nu", &vec!["-c".into(), "echo hello".into()], &cfg);
    assert_eq!(cmd, "nu");
    assert_eq!(args, vec!["-c", "echo hello"]);
}
