use zllg::config::ZllgConfig;
use zllg::dashboard::{build_dashboard, render_dashboard};
use zllg::detect::ProjectType;

#[test]
fn test_build_dashboard_rust() {
    let cfg = ZllgConfig::default();
    let state = build_dashboard(&cfg, ProjectType::Rust);
    assert_eq!(state.project_type, "rust");
    assert_eq!(state.panes.len(), 4);
}

#[test]
fn test_build_dashboard_node() {
    let cfg = ZllgConfig::default();
    let state = build_dashboard(&cfg, ProjectType::Node);
    assert_eq!(state.project_type, "node");
}

#[test]
fn test_render_dashboard() {
    let cfg = ZllgConfig::default();
    let state = build_dashboard(&cfg, ProjectType::Rust);
    let rendered = render_dashboard(&state);
    assert!(rendered.contains("zllg IDE"));
    assert!(rendered.contains("rust"));
    assert!(rendered.contains("◉"));
}
