use crate::executor::PipelineExecutor;
use crate::ledger::Ledger;
use crate::reflection::ReflectionEnvelope;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Result of the execution gate check.
#[derive(Debug, Clone, PartialEq)]
pub enum GateResult {
    /// Raw data validated, confidence sufficient, pipeline may execute.
    Validated {
        /// Confidence level (0.0 to 1.0).
        confidence: f64,
    },
    /// Confidence below threshold; execution deferred.
    Uncertain {
        /// Confidence level (0.0 to 1.0).
        confidence: f64,
    },
    /// Raw data reference invalid or gate interrupt triggered.
    Interrupted,
}

/// The execution gate — validates raw input before pipeline execution.
pub struct ExecutionGate {
    /// Minimum confidence threshold for automatic execution.
    threshold: f64,
}

impl ExecutionGate {
    /// Create a new gate with the default threshold.
    pub fn new() -> Self {
        Self { threshold: 0.3 }
    }

    /// Evaluate the event payload against the gate criteria.
    pub fn evaluate(&self, event_payload: &str) -> GateResult {
        if event_payload.is_empty() {
            return GateResult::Interrupted;
        }

        let confidence = self.compute_confidence(event_payload);
        if confidence >= self.threshold {
            GateResult::Validated { confidence }
        } else {
            GateResult::Uncertain { confidence }
        }
    }

    /// Compute confidence from event payload characteristics.
    fn compute_confidence(&self, event_payload: &str) -> f64 {
        let length_factor = (event_payload.len().min(1000) as f64) / 1000.0;
        let content_factor = if event_payload.contains(' ') {
            0.5
        } else {
            0.3
        };
        (length_factor + content_factor) / 2.0
    }
}

/// Event payload passed through the Message Bus from an ingestion source to the daemon core.
#[derive(Debug, Clone)]
pub struct EventPayload {
    /// The name of the pipeline responsible for processing this event (e.g., "obs_recorder").
    pub pipeline: String,
    /// The raw data associated with the event (e.g., file path).
    pub payload: String,
    /// Bounded retry count for transient processing failures.
    pub attempts: u8,
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

        Ok(Self {
            ledger,
            executor,
            pipelines_dir,
        })
    }

    /// Processes a single event payload received from the message bus.
    pub async fn process_event(
        &self,
        pipeline_name: &str,
        event_payload: String,
    ) -> std::io::Result<String> {
        let pipeline_path = self.pipelines_dir.join(pipeline_name);

        if !pipeline_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Pipeline not found: {}", pipeline_name),
            ));
        }

        let gate = ExecutionGate::new();
        let gate_result = gate.evaluate(&event_payload);

        match gate_result {
            GateResult::Interrupted => Err(std::io::Error::other("Execution gate interrupted")),
            GateResult::Uncertain { confidence } => Err(std::io::Error::other(format!(
                "Execution gate uncertain: confidence {}",
                confidence
            ))),
            GateResult::Validated { confidence: _ } => {
                // Execute the pipeline. The executor is not Send, so we capture the
                // result synchronously inside spawn_blocking by cloning the paths.
                let work_dir = self.executor.work_dir().to_path_buf();
                let mut envelope = tokio::task::spawn_blocking(move || {
                    let executor = PipelineExecutor::new(work_dir)?;
                    executor.execute(&pipeline_path, event_payload)
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
            match self.process_event(&event.pipeline, event.payload).await {
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
    fn test_gate_validated_on_nonempty_payload() {
        let gate = ExecutionGate::new();
        let result = gate.evaluate("ls -la --color=auto --human-readable --recursive --verbose --sort=name --group-directories-first --time-style=full-iso --quote-name --indicator-style=classify --file-type");
        assert!(matches!(result, GateResult::Validated { confidence: _ }));
    }

    #[test]
    fn test_gate_interrupted_on_empty_payload() {
        let gate = ExecutionGate::new();
        let result = gate.evaluate("");
        assert_eq!(result, GateResult::Interrupted);
    }

    #[test]
    fn test_gate_uncertain_on_short_payload() {
        let gate = ExecutionGate::new();
        let result = gate.evaluate("x");
        assert_eq!(result, GateResult::Uncertain { confidence: 0.1505 });
    }
}
