use crate::executor::PipelineExecutor;
use crate::ledger::Ledger;
use crate::reflection::{InputFingerprint, ReflectionEnvelope};
use mirror_guard::{ActionStatus, ExecutionGate, GateConcierge, GateContext, GuardDb, TrustScore};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Event payload passed through the Message Bus from an ingestion source to the daemon core.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EventPayload {
    /// The name of the pipeline responsible for processing this event (e.g., "obs_recorder").
    pub pipeline: String,
    /// The raw data associated with the event (e.g., file path).
    pub payload: String,
    /// Bounded retry count for transient processing failures.
    pub attempts: u8,
    /// Raw event ID from the source — provenance of the triggering observation.
    pub source_event_id: Option<String>,
    /// Trust layer of the source knowledge node.
    pub trust_layer: u32,
    /// Confidence score of the source node.
    pub confidence: TrustScore,
    /// Whether the event references raw data (not interpreted summaries).
    pub has_raw_data: bool,
    /// Whether uncertainty is exposed/surfaced.
    pub has_uncertainty: bool,
    /// Whether the action can be interrupted.
    pub can_interrupt: bool,
}

// Re-export so the watcher module can import it from here.

/// The core daemon - manages event ingestion, processing, and sealing of reflections.
pub struct MirrorDaemon {
    /// The append-only ledger
    pub ledger: Ledger,

    /// Pipeline executor
    executor: PipelineExecutor,

    /// Canonical directory containing pipeline definitions
    canonical_pipelines_dir: PathBuf,

    /// Guard database for execution gating
    guard_db: GuardDb,

    /// Gate concierge for provenance boundary enforcement
    concierge: GateConcierge,
}

impl MirrorDaemon {
    /// Create a new daemon instance
    pub fn new(
        ledger_path: impl AsRef<Path>,
        pipelines_dir: impl AsRef<Path>,
    ) -> std::io::Result<Self> {
        let ledger = Ledger::new(ledger_path.as_ref())?;

        // Work directory for pipeline executions
        let work_dir = ledger_path.as_ref().join("work");
        let executor = PipelineExecutor::new(work_dir)?;

        let pipelines_dir = pipelines_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&pipelines_dir)?;
        let canonical_pipelines_dir =
            std::fs::canonicalize(&pipelines_dir).map_err(std::io::Error::other)?;

        // Guard database for execution gating
        let guard_db_path = ledger_path.as_ref().join("guard.db");
        let guard_db = GuardDb::open(guard_db_path).map_err(std::io::Error::other)?;

        Ok(Self {
            ledger,
            executor,
            canonical_pipelines_dir,
            guard_db,
            concierge: GateConcierge::default(),
        })
    }

    /// Processes a single event payload received from the message bus.
    pub async fn process_event(
        &mut self,
        pipeline_name: &str,
        event: EventPayload,
    ) -> std::io::Result<String> {
        let pipeline_path = self.canonical_pipelines_dir.join(pipeline_name);

        if !pipeline_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Pipeline not found: {}", pipeline_name),
            ));
        }

        let canonical_pipeline =
            std::fs::canonicalize(&pipeline_path).map_err(std::io::Error::other)?;

        if !canonical_pipeline.starts_with(&self.canonical_pipelines_dir) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "Pipeline path escapes confinement: {} not under {}",
                    canonical_pipeline.display(),
                    self.canonical_pipelines_dir.display()
                ),
            ));
        }

        let ctx = GateContext {
            action_type: &event.pipeline,
            command: "nu",
            args: vec![pipeline_name.to_string()],
            trust_layer: event.trust_layer,
            confidence: event.confidence,
            source_event_id: event.source_event_id.as_deref(),
            has_raw_data: event.has_raw_data,
            has_uncertainty: event.has_uncertainty,
            can_interrupt: event.can_interrupt,
        };

        let gate = ExecutionGate::new(&self.guard_db, false, &self.canonical_pipelines_dir);
        let gate_result = gate.check(ctx).map_err(std::io::Error::other)?;

        let (action_status, pending_entry, interrupted_entry) = self.concierge.enforce(
            gate_result,
            &event.pipeline,
            "nu",
            &[pipeline_name.to_string()],
            event.trust_layer,
            event.confidence.get(),
            event.source_event_id.clone(),
        );

        match action_status {
            ActionStatus::TrustApproved => {
                let event_clone = event.clone();
                // Execute the pipeline. The executor is not Send, so we capture the
                // result synchronously inside spawn_blocking by cloning the paths.
                let work_dir = self.executor.work_dir().to_path_buf();
                let mut envelope = tokio::task::spawn_blocking(move || {
                    let executor = PipelineExecutor::new(work_dir)?;
                    executor.execute(&pipeline_path, event_clone.payload)
                })
                .await??;

                let source_fp = event.source_event_id.map(|id| InputFingerprint {
                    source: id,
                    hash: String::new(),
                    captured_at: chrono::Utc::now(),
                    schema: None,
                });

                envelope.inputs = vec![source_fp.unwrap_or(InputFingerprint {
                    source: String::new(),
                    hash: String::new(),
                    captured_at: chrono::Utc::now(),
                    schema: None,
                })];

                let reflection_id = self.ledger.append(&mut envelope)?;

                Ok(reflection_id)
            }
            ActionStatus::Pending => {
                if let Some(entry) = pending_entry {
                    self.guard_db
                        .persist_pending_queue_entry(&entry)
                        .map_err(std::io::Error::other)?;
                }
                Err(std::io::Error::other("Action pending human review"))
            }
            ActionStatus::Denied => {
                if let Some(entry) = interrupted_entry {
                    self.guard_db
                        .persist_interrupted_log_entry(&entry)
                        .map_err(std::io::Error::other)?;
                }
                Err(std::io::Error::other("Action denied by gate"))
            }
            ActionStatus::Executed | ActionStatus::Interrupted => {
                Err(std::io::Error::other("Unexpected action status"))
            }
        }
    }

    /// Runs the main asynchronous daemon loop, consuming events from the channel.
    pub async fn run_async(
        &mut self,
        mut receiver: mpsc::Receiver<EventPayload>,
    ) -> anyhow::Result<()> {
        println!("Daemon started and listening for incoming events...");

        while let Some(event) = receiver.recv().await {
            match self.process_event(&event.pipeline, event.clone()).await {
                Ok(id) => println!(
                    "[✅] Successfully processed event for pipeline '{}'. Reflection ID: {}",
                    &event.pipeline, id
                ),
                Err(e) => eprintln!(
                    "[❌] Failed to process event for pipeline '{}': {}",
                    &event.pipeline, e
                ),
            }
        }
        println!("Daemon stopping gracefully due to channel closure.");
        Ok(())
    }

    /// List available pipelines
    pub fn list_pipelines(&self) -> std::io::Result<Vec<String>> {
        let mut pipelines = Vec::new();

        for entry in std::fs::read_dir(&self.canonical_pipelines_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && path.extension().is_some_and(|ext| ext == "nu")
                && let Some(name) = path.file_name()
            {
                pipelines.push(name.to_string_lossy().to_string());
            }
        }

        pipelines.sort();
        Ok(pipelines)
    }

    /// Get the ledger instance
    #[allow(dead_code)]
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// Get a reflection by ID
    #[allow(dead_code)]
    pub fn get_reflection(&self, id: &str) -> std::io::Result<ReflectionEnvelope> {
        self.ledger.get_reflection(id)
    }

    /// List recent reflections
    #[allow(dead_code)]
    pub fn list_recent(&self, limit: usize) -> std::io::Result<Vec<crate::ledger::LedgerEntry>> {
        self.ledger.list_recent(limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirror_guard::GateResult;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_daemon_creation() {
        let tmp = TempDir::new().unwrap();
        let ledger_dir = tmp.path().join("ledger");
        let pipelines_dir = tmp.path().join("pipelines");

        let mut daemon = MirrorDaemon::new(&ledger_dir, &pipelines_dir).unwrap();

        assert!(daemon.canonical_pipelines_dir.is_absolute());
        assert!(daemon.ledger.get_reflection("dummy").is_err());

        let (tx, rx) = mpsc::channel::<EventPayload>(1);
        drop(tx);
        daemon.run_async(rx).await.ok();
    }

    #[test]
    fn test_list_pipelines() {
        let tmp = TempDir::new().unwrap();
        let ledger_dir = tmp.path().join("ledger");
        let pipelines_dir = tmp.path().join("pipelines");

        fs::create_dir_all(&pipelines_dir).unwrap();
        fs::write(pipelines_dir.join("test.nu"), "echo 'hello'").unwrap();
        fs::write(pipelines_dir.join("another.nu"), "ls").unwrap();

        let daemon = MirrorDaemon::new(&ledger_dir, &pipelines_dir).unwrap();
        let pipelines = daemon.list_pipelines().unwrap();

        assert_eq!(pipelines.len(), 2);
        assert!(pipelines.contains(&"test.nu".to_string()));
        assert!(pipelines.contains(&"another.nu".to_string()));
    }

    #[test]
    fn test_gate_proceeds_for_trusted_action() {
        let tmp = TempDir::new().unwrap();
        let guard_db = GuardDb::open(tmp.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&guard_db, false, tmp.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "nu",
            args: vec!["test.nu".to_string()],
            trust_layer: 3,
            confidence: TrustScore::new(0.9),
            source_event_id: Some("evt-daemon-1"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Proceed);
    }

    #[test]
    fn test_gate_interrupted_without_raw_data() {
        let tmp = TempDir::new().unwrap();
        let guard_db = GuardDb::open(tmp.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&guard_db, false, tmp.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "nu",
            args: vec!["test.nu".to_string()],
            trust_layer: 2,
            confidence: TrustScore::new(0.65),
            source_event_id: Some("evt-daemon-2"),
            has_raw_data: false,
            has_uncertainty: true,
            can_interrupt: true,
        };

        let result = gate.check(ctx).unwrap();
        assert!(matches!(result, GateResult::Interrupted { .. }));
    }

    #[test]
    fn test_gate_pending_for_working_layer() {
        let tmp = TempDir::new().unwrap();
        let guard_db = GuardDb::open(tmp.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&guard_db, false, tmp.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "nu",
            args: vec!["test.nu".to_string()],
            trust_layer: 2,
            confidence: TrustScore::new(0.65),
            source_event_id: Some("evt-daemon-3"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Pending);
    }
}
