use std::path::PathBuf;

use rusqlite::Connection;

use crate::error::{Result, ZllgError};

pub struct ZllgLogger {
    conn: Option<Connection>,
}

impl ZllgLogger {
    /// Open a connection to the mirror-log database.
    /// If the DB cannot be opened, the logger operates in no-op mode.
    pub fn new() -> Self {
        let db_path = data_dir()
            .join("zllg")
            .join("mirror.log.sqlite");

        let conn = mirror_log::db::init_db(&db_path).ok();

        Self { conn }
    }

    /// Log an event to mirror-log. Silently drops if DB is unavailable.
    pub fn log(&self, content: &str, meta: Option<&str>) -> Result<()> {
        if let Some(ref conn) = self.conn {
            mirror_log::append(conn, "zllg", content, meta)
                .map(|_| ())
                .map_err(|e| {
                ZllgError::logging(format!("mirror-log append failed: {e}"))
            })
        } else {
            Ok(())
        }
    }

    /// Returns true if the DB connection is live.
    pub fn is_active(&self) -> bool {
        self.conn.is_some()
    }
}

/// Resolve XDG data directory (~/.local/share).
fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
}
