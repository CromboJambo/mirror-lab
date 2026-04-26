use std::path::Path;
use tempfile::TempDir;

use zllg::detect::{ProjectType, detect_project_type};

fn make_marker(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), "").unwrap();
}

#[test]
fn test_detect_rust_from_cargo_toml() {
    let tmp = TempDir::new().unwrap();
    make_marker(tmp.path(), "Cargo.toml");
    assert_eq!(detect_project_type(tmp.path()), ProjectType::Rust);
}

#[test]
fn test_detect_node_from_package_json() {
    let tmp = TempDir::new().unwrap();
    make_marker(tmp.path(), "package.json");
    assert_eq!(detect_project_type(tmp.path()), ProjectType::Node);
}

#[test]
fn test_detect_python_from_pyproject() {
    let tmp = TempDir::new().unwrap();
    make_marker(tmp.path(), "pyproject.toml");
    assert_eq!(detect_project_type(tmp.path()), ProjectType::Python);
}

#[test]
fn test_detect_python_from_setup_py() {
    let tmp = TempDir::new().unwrap();
    make_marker(tmp.path(), "setup.py");
    assert_eq!(detect_project_type(tmp.path()), ProjectType::Python);
}

#[test]
fn test_detect_nix_from_flake() {
    let tmp = TempDir::new().unwrap();
    make_marker(tmp.path(), "flake.nix");
    assert_eq!(detect_project_type(tmp.path()), ProjectType::Nix);
}

#[test]
fn test_detect_default_when_no_markers() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(detect_project_type(tmp.path()), ProjectType::Default);
}

#[test]
fn test_rust_takes_priority_when_multiple_markers_present() {
    // Cargo.toml is checked before package.json in the marker list.
    let tmp = TempDir::new().unwrap();
    make_marker(tmp.path(), "Cargo.toml");
    make_marker(tmp.path(), "package.json");
    assert_eq!(detect_project_type(tmp.path()), ProjectType::Rust);
}

#[test]
fn test_layout_name_roundtrip() {
    assert_eq!(ProjectType::Rust.layout_name(), "rust");
    assert_eq!(ProjectType::Node.layout_name(), "node");
    assert_eq!(ProjectType::Python.layout_name(), "python");
    assert_eq!(ProjectType::Nix.layout_name(), "nix");
    assert_eq!(ProjectType::Default.layout_name(), "default");
}
