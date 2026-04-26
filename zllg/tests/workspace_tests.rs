use zllg::workspace;

#[test]
fn test_write_default_workspaces() {
    let written = workspace::write_default_workspaces().unwrap();
    assert!(written.exists());
    let raw = std::fs::read_to_string(&written).unwrap();
    assert!(raw.contains("monitor-1"));
}

#[test]
fn test_load_workspaces_returns_defaults_when_absent() {
    let cfg = workspace::load_workspaces().unwrap();
    assert_eq!(cfg.workspaces.len(), 3);
}

#[test]
fn test_find_workspace() {
    let cfg = workspace::load_workspaces().unwrap();
    let found = workspace::find_workspace(&cfg.workspaces, "monitor-1");
    assert!(found.is_some());
    assert_eq!(found.unwrap().label, "Main");
}

#[test]
fn test_find_workspace_missing() {
    let cfg = workspace::load_workspaces().unwrap();
    let found = workspace::find_workspace(&cfg.workspaces, "nonexistent");
    assert!(found.is_none());
}
