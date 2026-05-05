use mirror_guard::{ActionStatus, GateResult};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Pending queue entry for actions requiring review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingQueueEntry {
    pub id: String,
    pub gate_result_id: String,
    pub action_type: String,
    pub command: String,
    pub args: Vec<String>,
    pub trust_layer: u32,
    pub confidence: f64,
    pub queued_at: i64,
    pub reason: String,
}

/// Interrupted log entry for actions blocked by the gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptedLogEntry {
    pub id: String,
    pub gate_result_id: String,
    pub action_type: String,
    pub command: String,
    pub args: Vec<String>,
    pub trust_layer: u32,
    pub reason: String,
    pub logged_at: i64,
}

/// Gate concierge that enforces provenance boundaries on gate results.
///
/// Pending → PendingQueue (queued, not executed).
/// Interrupted → InterruptedLog (logged, returned, not proceeded).
/// No tool call path bypasses the gate.
pub struct GateConcierge {
    pending_queue: Vec<PendingQueueEntry>,
    interrupted_log: Vec<InterruptedLogEntry>,
}

#[allow(dead_code)]
impl GateConcierge {
    pub fn new() -> Self {
        Self {
            pending_queue: Vec::new(),
            interrupted_log: Vec::new(),
        }
    }

    /// Enforce a gate result through provenance boundaries.
    /// Returns a boundary_enforced status and any queued/logged entries.
    pub fn enforce(
        &mut self,
        gate_result: GateResult,
        action_type: &str,
        command: &str,
        args: &[String],
        trust_layer: u32,
        confidence: f64,
    ) -> (
        ActionStatus,
        Option<PendingQueueEntry>,
        Option<InterruptedLogEntry>,
    ) {
        let gate_result_id = Uuid::new_v4().to_string();

        match gate_result {
            GateResult::Proceed => {
                info!(
                    gate_result_id = %gate_result_id,
                    action_type = %action_type,
                    "Gate concierge: Proceed — action authorized"
                );
                (ActionStatus::TrustApproved, None, None)
            }
            GateResult::Pending => {
                let reason =
                    "Action requires review: trust layer below auto-execute threshold".to_string();
                let entry = PendingQueueEntry {
                    id: Uuid::new_v4().to_string(),
                    gate_result_id: gate_result_id.clone(),
                    action_type: action_type.to_string(),
                    command: command.to_string(),
                    args: args.to_vec(),
                    trust_layer,
                    confidence,
                    queued_at: chrono::Utc::now().timestamp(),
                    reason,
                };
                self.pending_queue.push(entry.clone());
                warn!(
                    gate_result_id = %gate_result_id,
                    action_type = %action_type,
                    command = %command,
                    "Gate concierge: Pending → PendingQueue"
                );
                (ActionStatus::Pending, Some(entry), None)
            }
            GateResult::Interrupted { reason } => {
                let entry = InterruptedLogEntry {
                    id: Uuid::new_v4().to_string(),
                    gate_result_id: gate_result_id.clone(),
                    action_type: action_type.to_string(),
                    command: command.to_string(),
                    args: args.to_vec(),
                    trust_layer,
                    reason: reason.clone(),
                    logged_at: chrono::Utc::now().timestamp(),
                };
                self.interrupted_log.push(entry.clone());
                error!(
                    gate_result_id = %gate_result_id,
                    action_type = %action_type,
                    command = %command,
                    reason = %reason,
                    "Gate concierge: Interrupted → InterruptedLog"
                );
                (ActionStatus::Denied, None, Some(entry))
            }
            GateResult::DryRun => {
                info!(
                    gate_result_id = %gate_result_id,
                    action_type = %action_type,
                    "Gate concierge: DryRun — no execution"
                );
                (ActionStatus::Denied, None, None)
            }
        }
    }

    /// Return the pending queue entries.
    pub fn pending_queue(&self) -> &[PendingQueueEntry] {
        &self.pending_queue
    }

    /// Return the interrupted log entries.
    pub fn interrupted_log(&self) -> &[InterruptedLogEntry] {
        &self.interrupted_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concierge_proceed() {
        let mut concierge = GateConcierge::new();
        let (status, pending, interrupted) = concierge.enforce(
            GateResult::Proceed,
            "echo",
            "echo",
            &["hello".to_string()],
            3,
            0.9,
        );
        assert_eq!(status, ActionStatus::TrustApproved);
        assert!(pending.is_none());
        assert!(interrupted.is_none());
    }

    #[test]
    fn test_concierge_pending_to_queue() {
        let mut concierge = GateConcierge::new();
        let (status, pending, interrupted) = concierge.enforce(
            GateResult::Pending,
            "git_commit",
            "git",
            &["commit".to_string(), "-m".to_string(), "test".to_string()],
            2,
            0.5,
        );
        assert_eq!(status, ActionStatus::Pending);
        assert!(pending.is_some());
        assert!(interrupted.is_none());
        assert_eq!(concierge.pending_queue().len(), 1);
    }

    #[test]
    fn test_concierge_interrupted_to_log() {
        let mut concierge = GateConcierge::new();
        let (status, pending, interrupted) = concierge.enforce(
            GateResult::Interrupted {
                reason: "High-risk command detected".to_string(),
            },
            "delete",
            "rm",
            &["-rf".to_string(), "/tmp/test".to_string()],
            3,
            0.9,
        );
        assert_eq!(status, ActionStatus::Denied);
        assert!(pending.is_none());
        assert!(interrupted.is_some());
        assert_eq!(concierge.interrupted_log().len(), 1);
    }

    #[test]
    fn test_concierge_dry_run() {
        let mut concierge = GateConcierge::new();
        let (status, pending, interrupted) = concierge.enforce(
            GateResult::DryRun,
            "echo",
            "echo",
            &["hello".to_string()],
            0,
            0.0,
        );
        assert_eq!(status, ActionStatus::Denied);
        assert!(pending.is_none());
        assert!(interrupted.is_none());
    }

    #[test]
    fn test_concierge_no_bypass() {
        let mut concierge = GateConcierge::new();
        let (status, pending, interrupted) = concierge.enforce(
            GateResult::Pending,
            "run_command",
            "rm",
            &["-rf".to_string(), "/tmp/test".to_string()],
            2,
            0.5,
        );
        assert_eq!(status, ActionStatus::Pending);
        assert!(pending.is_some());
        assert!(interrupted.is_none());
        // Pending must not proceed — no bypass chain
        assert!(status != ActionStatus::TrustApproved);
    }
}
