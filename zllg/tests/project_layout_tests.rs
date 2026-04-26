use zllg::project_layout;

#[test]
fn test_write_default_project_layouts() {
    let written = project_layout::write_default_project_layouts().unwrap();
    assert!(written.exists());
    let raw = std::fs::read_to_string(&written).unwrap();
    assert!(raw.contains("rust"));
}

#[test]
fn test_load_project_layouts_returns_defaults_when_absent() {
    let cfg = project_layout::load_project_layouts().unwrap();
    assert_eq!(cfg.layouts.len(), 5);
}

#[test]
fn test_find_project_layout_rust() {
    let cfg = project_layout::load_project_layouts().unwrap();
    let found = project_layout::find_project_layout(&cfg.layouts, "rust");
    assert!(found.is_some());
}

#[test]
fn test_find_project_layout_missing() {
    let cfg = project_layout::load_project_layouts().unwrap();
    let found = project_layout::find_project_layout(&cfg.layouts, "go");
    assert!(found.is_none());
}
