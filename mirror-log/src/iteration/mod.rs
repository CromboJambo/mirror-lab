//! Iteration tracking module for mirror-log.
//!
//! This module provides basic iteration tracking functionality.
//! Note: This module is currently in development and may be incomplete.

pub mod queries;
pub mod types;

pub use types::*;

// Re-export query functions
pub use queries::{
    get_iteration_passes, get_iteration_status, insert_iteration_pass, update_iteration_status,
};
