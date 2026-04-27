pub mod attention;
pub mod chunk;
pub mod db;
pub mod decay;
pub mod embedding;
pub mod export;
pub mod infer;
#[cfg(feature = "inference")]
pub mod inference;
#[cfg(feature = "iteration")]
pub mod iteration;
pub mod log;
pub mod pipeline;
pub mod sources;
pub mod stage;
pub mod view;

// Re-export commonly used types and functions
pub use log::{
    AppendReceipt, IntegrityReport, append, append_batch, append_batch_with_receipts,
    append_batch_with_receipts_in_tx, is_duplicate, verify_integrity,
};
pub use view::{Event, by_ingestion_time, dedup_stats, find_duplicates};

// Re-export attention types and functions
pub use attention::{
    AttentionItem, AttentionLayer, AttentionStats, init_tables, init_with_defaults,
};

// Re-export staging types
pub use stage::StagedEvent;

// Re-export inference/pattern types
pub use infer::{Pattern, detect_patterns};

// Re-export iteration types (only the core types)
#[cfg(feature = "iteration")]
pub use iteration::types::*;

// Re-export embedding types and functions
#[cfg(feature = "embedding")]
pub use embedding::{
    Embedding, EmbeddingError, EmbeddingStats, Similarity, batch_generate_and_store,
    init_embedding_service, normalize_vector,
};

// Re-export iteration types and functions
#[cfg(feature = "iteration")]
pub use iteration::{
    CompletionReason, FeedbackQuality, IterationError, IterationFeedback, IterationInsight,
    IterationPass, IterationStats, IterationStatus, IterationThreshold, PassType,
    get_iteration_passes, get_iteration_status, insert_iteration_pass, update_iteration_status,
};

pub mod catalogue;
pub mod orchestrator;

// Re-export pipeline constants and functions
pub use pipeline::{AUTO_CHUNK_THRESHOLD, DEFAULT_CHUNK_SIZE, ingest_stdin_with_policy};

// Re-export decay types and functions
pub use decay::{
    DecayStats, ShadowEvent, get_decay_score, get_decay_stats, get_flagged_events,
    get_shadow_events, init_decay_tables, is_flagged, move_to_shadow, pin_event,
    restore_from_shadow, track_access, unpin_event,
};
pub use orchestrator::PromotionOrchestrator;

// Re-export bridge types for kernel integration
#[cfg(feature = "iteration")]
pub mod bridge;
#[cfg(feature = "iteration")]
pub use bridge::LogIterationTracker;

// Re-export inference types and functions
#[cfg(feature = "inference")]
pub use inference::{
    Event as InferenceEvent, HttpBackend, HttpConfig, InferenceBackend, InferenceConfig,
    InferenceError,
};
