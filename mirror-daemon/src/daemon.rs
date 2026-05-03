use crate::executor::PipelineExecutor;
use crate::ledger::Ledger;
use crate::reflection::ReflectionEnvelope;
use mirror_guard::{ExecutionGate, GateContext, GateResult, GuardDb, TrustScore};
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

    /// Directory containing pipeline definitions
    pipelines_dir: PathBuf,

    /// Guard database for execution gating
    guard_db: GuardDb,
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

        // Guard database for execution gating
        let guard_db_path = ledger_path.as_ref().join("guard.db");
        let guard_db = GuardDb::open(guard_db_path)
            .map_err(std::io::Error::other)?;

        Ok(Self {
            ledger,
            executor,
            pipelines_dir,
            guard_db,
        })
    }

    /// Processes a single event payload received from the message bus.
    pub async fn process_event(
        &self,
        pipeline_name: &str,
        event: EventPayload,
    ) -> std::io::Result<String> {
        let pipeline_path = self.pipelines_dir.join(pipeline_name);

        if !pipeline_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Pipeline not found: {}", pipeline_name),
            ));
        }

        let ctx = GateContext {
            action_type: &event.pipeline,
            command: "nu",
            args: vec![pipeline_name.to_string()],
            trust_layer: event.trust_layer,
            has_raw_data: event.has_raw_data,
            has_uncertainty: event.has_uncertainty,
            can_interrupt: event.can_interrupt,
        };

        let gate = ExecutionGate::new(&self.guard_db, false, &self.pipelines_dir);
        let gate_result = gate
            .check(ctx)
            .map_err(std::io::Error::other)?;

        match gate_result {
            GateResult::Interrupted { reason } => Err(std::io::Error::other(reason)),
            GateResult::Pending => Err(std::io::Error::other("Action pending human review")),
            GateResult::DryRun | GateResult::Proceed => {
                let event_clone = event.clone();
                // Execute the pipeline. The executor is not Send, so we capture the
                // result synchronously inside spawn_blocking by cloning the paths.
                let work_dir = self.executor.work_dir().to_path_buf();
                let mut envelope = tokio::task::spawn_blocking(move || {
                    let executor = PipelineExecutor::new(work_dir)?;
                    executor.execute(&pipeline_path, event_clone.payload)
                })
                .await??;

                // Append to ledger
                let reflection_id = self.ledger.append(&mut envelope)?;

                Ok(reflection_id)
            }
        }
    }

    /// Runs the main asynchronous daemon loop, consuming events from the channel.
    pub async fn run_async(
        &self,
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

        for entry in std::fs::read_dir(&self.pipelines_dir)? {
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
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_daemon_creation() {
        let tmp = TempDir::new().unwrap();
        let ledger_dir = tmp.path().join("ledger");
        let pipelines_dir = tmp.path().join("pipelines");

        let daemon = MirrorDaemon::new(&ledger_dir, &pipelines_dir).unwrap();

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
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Pending);
    }
}
