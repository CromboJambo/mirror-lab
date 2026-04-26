use zllg::keybind;

#[test]
fn test_write_default_keybinds() {
    // We can't override dirs::config_dir(), so just test that the write succeeds.
    let written = keybind::write_default_keybinds().unwrap();
    assert!(written.exists());
    let raw = std::fs::read_to_string(&written).unwrap();
    assert!(raw.contains("alt"));
    assert!(raw.contains("f"));
}

#[test]
fn test_load_keybinds_returns_defaults_when_absent() {
    let cfg = keybind::load_keybinds().unwrap();
    assert_eq!(cfg.keybinds.len(), 6);
}

#[test]
fn test_render_keybind_kdl() {
    let kdl = keybind::render_keybind_kdl();
    assert!(kdl.contains("plugin location=\"zellij:plugin-kdl\""));
    assert!(kdl.contains("keybind modifier=\"alt\" key=\"f\""));
}

#[test]
fn test_render_keybind_file() {
    let kdl = keybind::render_keybind_file();
    assert!(kdl.contains("keybinds {"));
    assert!(kdl.contains("}"));
}
