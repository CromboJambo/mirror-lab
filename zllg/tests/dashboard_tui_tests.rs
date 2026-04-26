use zllg::dashboard_tui;

#[test]
fn test_dashboard_tui_module_exists() {
    // The TUI is interactive and can't be tested in CI.
    // This test just verifies the module is compiled.
    let _ = dashboard_tui::run_dashboard_tui;
}
