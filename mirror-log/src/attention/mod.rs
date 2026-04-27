//! Attention layer for mirror-log.
//!
//! This module implements the attention layer concept from attention.md:
//! - A dynamic, living index of relevance that sits between cold storage and active thought
//! - Manages the boundary between thought and storage
//! - Implements decay, promotion, and reference-based retention
//! - Supports the three-reference-point principle and five-sense triangulation

use crate::decay::{get_decay_score, track_access};
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};

use std::time::UNIX_EPOCH;

/// Maximum number of active attention items
const MAX_ACTIVE_ITEMS: i32 = 15;

/// Default decay threshold in days
const DEFAULT_DECAY_THRESHOLD_DAYS: i64 = 30;

/// Minimum reference points required for long-term storage
const MIN_REFERENCE_POINTS: i32 = 3;

/// The attention layer - a dynamic view over events that are currently relevant
#[derive(Debug, Clone)]
pub struct AttentionLayer {
    /// Maximum number of items to display
    max_items: i32,
    /// Decay threshold for removing items
    decay_threshold: i64,
    /// Minimum reference points required
    #[allow(dead_code)]
    min_reference_points: i32,
}

impl Default for AttentionLayer {
    fn default() -> Self {
        Self {
            max_items: MAX_ACTIVE_ITEMS,
            decay_threshold: DEFAULT_DECAY_THRESHOLD_DAYS,
            min_reference_points: MIN_REFERENCE_POINTS,
        }
    }
}

impl AttentionLayer {
    /// Creates a new attention layer with custom configuration
    pub fn new(max_items: i32, decay_threshold: i64, min_reference_points: i32) -> Self {
        Self {
            max_items,
            decay_threshold,
            min_reference_points,
        }
    }

    /// Gets the active attention items
    pub fn get_active_items(&self, conn: &Connection) -> Result<Vec<AttentionItem>> {
        let mut items = Vec::new();

        // Query events with recent access or high decay score
        let query = r#"
            SELECT
                e.id,
                e.timestamp,
                e.source,
                e.content,
                e.meta,
                d.access_count,
                d.last_accessed,
                d.pinned,
                COUNT(DISTINCT et.tag) as tag_count,
                COUNT(DISTINCT el.to_event_id) as link_count
            FROM events e
            LEFT JOIN decay d ON e.id = d.event_id
            LEFT JOIN shadow_state s ON e.id = s.event_id
            LEFT JOIN event_tags et ON e.id = et.event_id
            LEFT JOIN event_links el ON e.id = el.from_event_id
            WHERE
                s.event_id IS NULL
                AND
                (d.pinned = 1 OR d.last_accessed > unixepoch() - ?1 * 86400)
                AND NOT EXISTS (
                    SELECT 1 FROM decay
                    WHERE event_id = e.id
                    AND access_count < ?2
                    AND (unixepoch() - last_accessed) > ?3 * 86400
                    AND pinned = 0
                )
            GROUP BY e.id, e.timestamp, e.source, e.content, e.meta, d.access_count, d.last_accessed, d.pinned
            ORDER BY d.last_accessed DESC, d.access_count DESC
            LIMIT ?
        "#;

        let mut stmt = conn.prepare(query)?;

        let rows = stmt.query_map(
            (
                self.decay_threshold,
                5,
                self.decay_threshold,
                self.max_items,
            ),
            |row| {
                let id: String = row.get(0)?;
                let timestamp: i64 = row.get(1)?;
                let source: String = row.get(2)?;
                let content: String = row.get(3)?;
                let meta: Option<String> = row.get(4)?;
                let access_count: i64 = row.get(5)?;
                let last_accessed: i64 = row.get(6)?;
                let pinned: bool = row.get(7)?;
                let tag_count: i64 = row.get(8)?;
                let link_count: i64 = row.get(9)?;

                Ok(AttentionItem {
                    id,
                    timestamp,
                    source,
                    content,
                    meta,
                    access_count,
                    last_accessed,
                    pinned,
                    tag_count,
                    link_count,
                })
            },
        )?;

        for item in rows {
            items.push(item?);
        }

        // Trim to max_items
        if items.len() > self.max_items as usize {
            items.truncate(self.max_items as usize);
        }

        Ok(items)
    }

    /// Adds an item to attention (promotes from storage)
    pub fn add_to_attention(&self, conn: &Connection, event_id: &str) -> Result<()> {
        // Mark as accessed and pinned
        track_access(conn, event_id)?;

        // Update last_accessed timestamp
        conn.execute(
            "UPDATE decay SET last_accessed = unixepoch(), pinned = 1 WHERE event_id = ?1",
            [event_id],
        )?;

        Ok(())
    }

    /// Removes an item from attention (demotes to storage)
    pub fn remove_from_attention(&self, conn: &Connection, event_id: &str) -> Result<()> {
        conn.execute(
            "UPDATE decay SET pinned = 0 WHERE event_id = ?1",
            [event_id],
        )?;

        Ok(())
    }

    /// Updates an attention item (revisits it)
    pub fn update_item(&self, conn: &Connection, event_id: &str) -> Result<()> {
        track_access(conn, event_id)?;

        // Optionally increase priority based on how often it's updated
        conn.execute(
            "UPDATE decay SET access_count = access_count + 1 WHERE event_id = ?1",
            [event_id],
        )?;

        Ok(())
    }

    /// Gets items that need attention (flagged for decay)
    pub fn get_flagged_items(&self, conn: &Connection) -> Result<Vec<AttentionItem>> {
        let mut items = Vec::new();

        let query = r#"
            SELECT
                e.id,
                e.timestamp,
                e.source,
                e.content,
                e.meta,
                d.access_count,
                d.last_accessed,
                d.pinned,
                COUNT(DISTINCT et.tag) as tag_count,
                COUNT(DISTINCT el.to_event_id) as link_count
            FROM events e
            LEFT JOIN decay d ON e.id = d.event_id
            LEFT JOIN shadow_state s ON e.id = s.event_id
            LEFT JOIN event_tags et ON e.id = et.event_id
            LEFT JOIN event_links el ON e.id = el.from_event_id
            WHERE
                s.event_id IS NULL
                AND
                EXISTS (
                    SELECT 1 FROM decay
                    WHERE event_id = e.id
                    AND access_count < ?1
                    AND (unixepoch() - last_accessed) > ?2 * 86400
                    AND pinned = 0
                )
            GROUP BY e.id, e.timestamp, e.source, e.content, e.meta, d.access_count, d.last_accessed, d.pinned
            ORDER BY d.last_accessed ASC, access_count ASC
        "#;

        let mut stmt = conn.prepare(query)?;

        let rows = stmt.query_map((5, self.decay_threshold), |row| {
            let id: String = row.get(0)?;
            let timestamp: i64 = row.get(1)?;
            let source: String = row.get(2)?;
            let content: String = row.get(3)?;
            let meta: Option<String> = row.get(4)?;
            let access_count: i64 = row.get(5)?;
            let last_accessed: i64 = row.get(6)?;
            let pinned: bool = row.get(7)?;
            let tag_count: i64 = row.get(8)?;
            let link_count: i64 = row.get(9)?;

            Ok(AttentionItem {
                id,
                timestamp,
                source,
                content,
                meta,
                access_count,
                last_accessed,
                pinned,
                tag_count,
                link_count,
            })
        })?;

        for item in rows {
            items.push(item?);
        }

        Ok(items)
    }

    /// Calculates attention score for an event
    pub fn calculate_attention_score(&self, conn: &Connection, event_id: &str) -> Result<f64> {
        let decay_score = get_decay_score(conn, event_id)?;

        // Higher decay score = more recently accessed
        // Lower decay score = more recently accessed
        // Normalize to 0-100 scale
        let score = (1.0 / (decay_score + 1.0)) * 100.0;

        Ok(score)
    }

    /// Gets attention statistics
    pub fn get_stats(&self, conn: &Connection) -> Result<AttentionStats> {
        let total_events: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM events e
             WHERE NOT EXISTS (
                 SELECT 1 FROM shadow_state s WHERE s.event_id = e.id
             )",
            [],
            |row| row.get(0),
        )?;

        let active_events: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM decay d
             WHERE d.last_accessed > unixepoch() - ?1 * 86400
             AND NOT EXISTS (
                 SELECT 1 FROM shadow_state s WHERE s.event_id = d.event_id
             )",
            (self.decay_threshold,),
            |row| row.get(0),
        )?;

        let pinned_events: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM decay d
             WHERE d.pinned = 1
             AND NOT EXISTS (
                 SELECT 1 FROM shadow_state s WHERE s.event_id = d.event_id
             )",
            [],
            |row| row.get(0),
        )?;

        let flagged_events: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM decay d
             WHERE d.access_count < ?1
             AND (unixepoch() - d.last_accessed) > ?2 * 86400
             AND d.pinned = 0
             AND NOT EXISTS (
                 SELECT 1 FROM shadow_state s WHERE s.event_id = d.event_id
             )",
            (5, self.decay_threshold),
            |row| row.get(0),
        )?;

        Ok(AttentionStats {
            total_events,
            active_events,
            pinned_events,
            flagged_events,
            decay_threshold: self.decay_threshold,
        })
    }
}

/// An item in the attention layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionItem {
    /// Event ID
    pub id: String,
    /// Timestamp of the event
    pub timestamp: i64,
    /// Source of the event
    pub source: String,
    /// Content of the event
    pub content: String,
    /// Optional metadata
    pub meta: Option<String>,
    /// Number of times accessed
    pub access_count: i64,
    /// Last access timestamp
    pub last_accessed: i64,
    /// Whether the item is pinned (not subject to decay)
    pub pinned: bool,
    /// Number of tags
    pub tag_count: i64,
    /// Number of links to/from this event
    pub link_count: i64,
}

impl AttentionItem {
    /// Gets the last access time as a human-readable string
    pub fn last_accessed_str(&self) -> String {
        if self.last_accessed == 0 {
            "Never".to_string()
        } else {
            let last_accessed = UNIX_EPOCH.elapsed().unwrap_or_default()
                - std::time::Duration::from_secs(self.last_accessed as u64);
            format_duration(last_accessed)
        }
    }

    /// Gets the timestamp as a human-readable string
    pub fn timestamp_str(&self) -> String {
        if self.timestamp == 0 {
            "Unknown".to_string()
        } else {
            let timestamp = UNIX_EPOCH.elapsed().unwrap_or_default()
                - std::time::Duration::from_secs(self.timestamp as u64);
            format_duration(timestamp)
        }
    }
}

/// Statistics for the attention layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionStats {
    /// Total number of events
    pub total_events: i64,
    /// Number of active events
    pub active_events: i64,
    /// Number of pinned events
    pub pinned_events: i64,
    /// Number of flagged events (due for decay)
    pub flagged_events: i64,
    /// Decay threshold in days
    pub decay_threshold: i64,
}

impl AttentionStats {
    /// Gets the percentage of active events
    pub fn active_percentage(&self) -> f64 {
        if self.total_events == 0 {
            0.0
        } else {
            (self.active_events as f64 / self.total_events as f64) * 100.0
        }
    }
}

/// Formats a duration as a human-readable string
fn format_duration(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();

    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        format!("{}m", minutes)
    } else if seconds < 86400 {
        let hours = seconds / 3600;
        format!("{}h", hours)
    } else {
        let days = seconds / 86400;
        format!("{}d", days)
    }
}

/// Initializes the attention layer tables
pub fn init_tables(conn: &Connection) -> Result<()> {
    // Create attention_items table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS attention_items (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 0,
            last_accessed INTEGER NOT NULL,
            reference_count INTEGER NOT NULL DEFAULT 0,
            tags TEXT,
            links TEXT,
            meta TEXT,
            FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create reference_points table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS reference_points (
            id TEXT PRIMARY KEY,
            attention_item_id TEXT NOT NULL,
            reference_type TEXT NOT NULL,
            reference_value TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            FOREIGN KEY (attention_item_id) REFERENCES attention_items(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create attention_decay table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS attention_decay (
            id TEXT PRIMARY KEY,
            attention_item_id TEXT NOT NULL,
            decay_score REAL NOT NULL,
            last_decay_check INTEGER NOT NULL DEFAULT (unixepoch()),
            FOREIGN KEY (attention_item_id) REFERENCES attention_items(id) ON DELETE CASCADE
        )",
        [],
    )?;

    Ok(())
}

/// Initializes the attention layer with defaults
pub fn init_with_defaults(conn: &Connection) -> Result<()> {
    init_tables(conn)?;
    Ok(())
}
