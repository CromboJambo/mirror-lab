pub mod clipboard_watcher;
/// Ingress subsystem — watches a directory for new recordings, processes them
/// through auto-editor + ffmpeg, and persists chunks to SQLite.
///
/// Previously a standalone binary crate; integrated here so `mirror-daemon`
/// can own the full ingest → witness → ledger pipeline.
pub mod config;
pub mod db;
pub mod doctor;
pub mod processor;
pub mod sanitizer;
pub mod transcription;
pub mod watcher;
