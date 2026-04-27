//! Bridge module connecting mirror-kernel decision layer to mirror-log iteration tracking.
//!
//! This module provides the `LogIterationTracker` which implements the
//! `IterationTracker` trait from mirror-kernel, allowing kernel reflections
//! to automatically trigger iteration updates in the mirror-log database.

#[cfg(feature = "iteration")]
use crate::iteration::{PassType, insert_iteration_pass, update_iteration_status};
#[cfg(feature = "iteration")]
use mirror_kernel::{IterationTracker, Reflection};
#[cfg(feature = "iteration")]
use rusqlite::Connection;
#[cfg(feature = "iteration")]
use std::sync::Arc;

/// Tracks iteration updates produced by mirror-kernel reflections.
///
/// When a kernel produces a reflection, this tracker automatically:
/// 1. Records a "Re-Encoding" iteration pass for the event
/// 2. Updates the iteration status to reflect the new state
#[cfg(feature = "iteration")]
pub struct LogIterationTracker;

#[cfg(feature = "iteration")]
impl LogIterationTracker {
    /// Records a Re-Encoding pass for the event when a reflection is produced.
    fn record_re_encoding(conn: &Connection, event_id: &str) -> Result<(), rusqlite::Error> {
        // Get current iteration number from status
        let current_iteration: i32 = conn
            .query_row(
                "SELECT current_iteration FROM iteration_status WHERE event_id = ?1",
                [event_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Increment iteration number
        let next_iteration = current_iteration + 1;

        // Insert the Re-Encoding pass
        insert_iteration_pass(
            conn,
            event_id,
            next_iteration,
            &PassType::ReEncoding.display_name(),
        )?;

        // Update status to reflect the new state
        update_iteration_status(
            conn,
            event_id,
            next_iteration,
            Some(PassType::ReEncoding.display_name()),
            false, // Not complete yet
        )?;

        Ok(())
    }
}

#[cfg(feature = "iteration")]
impl IterationTracker for LogIterationTracker {
    fn on_reflection(&self, conn: &Connection, event_id: &str, _reflection: &Reflection) {
        // Record the Re-Encoding pass triggered by the kernel reflection
        if let Err(e) = Self::record_re_encoding(conn, event_id) {
            tracing::error!(
                event_id = event_id,
                error = %e,
                "Failed to record Re-Encoding pass for kernel reflection"
            );
        }
    }
}
