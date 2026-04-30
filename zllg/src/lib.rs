pub mod cli;
pub mod config;
pub mod dashboard;
pub mod dashboard_tui;
pub mod detect;
pub mod error;
pub mod gate;
pub mod keybind;
pub mod layout;
pub mod logger;
pub mod pane;
pub mod project_layout;
pub mod project_pane;
pub mod tools;
pub mod workspace;

pub use error::{Result, ZllgError};
