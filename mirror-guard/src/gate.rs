use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::guard_db::GuardDb;
use crate::guard_db::GuardDbError;
use crate::trust::TrustManager;
use crate::types::TrustScore;

/// Result of a gate check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResult {
    Proceed,
    Interrupted {
        reason: String,
    },
    Pending,
    DryRun,
    /// Process was revoked — guide out gracefully, don't block hard.
    Revoked {
        reason: String,
    },
}

/// Execution gate that combines trust-layer gating with command security checks.
///
/// This is the single point where the system transitions from detection to action.
/// Detection != authorization: knowing what happened does not grant the right to change what happens.
pub struct ExecutionGate<'a> {
    trust: TrustManager<'a>,
    dry_run: bool,
    risk_config: RiskConfig,
}

impl<'a> ExecutionGate<'a> {
    pub fn new(db: &'a GuardDb, dry_run: bool, _root: impl Into<PathBuf>) -> Self {
        let _ = _root;
        Self {
            trust: TrustManager::new(db),
            dry_run,
            risk_config: RiskConfig::default(),
        }
    }

    pub fn with_risk_config(mut self, risk_config: RiskConfig) -> Self {
        self.risk_config = risk_config;
        self
    }

    /// Run the full gate check before executing an action.
    ///
    /// The gate enforces:
    /// 1. Raw data reference: the event must reference raw data, not interpreted summaries
    /// 2. Uncertainty exposure: if confidence is below threshold, surface it before executing
    /// 3. Interruptibility: allow the gate to return Interrupted instead of executing
    /// 4. Trust layer check: auto-execute only for trusted layers
    pub fn check(&self, ctx: GateContext<'_>) -> Result<GateResult, GuardDbError> {
        // 1. Dry-run check
        if self.dry_run {
            info!(
                action = %ctx.action_type,
                trust_layer = ctx.trust_layer,
                "Dry-run: would execute action"
            );
            return Ok(GateResult::DryRun);
        }

        // 2. Raw data reference check
        if !ctx.has_raw_data {
            let reason = "Action triggered without raw data reference; detection != authorization"
                .to_string();
            warn!(action = %ctx.action_type, %reason, "Gate interrupted");
            return Ok(GateResult::Interrupted { reason });
        }

        // 3. Uncertainty exposure check
        if !ctx.has_uncertainty {
            let reason =
                "Action triggered without uncertainty exposure; gate not enforced".to_string();
            warn!(action = %ctx.action_type, %reason, "Gate interrupted");
            return Ok(GateResult::Interrupted { reason });
        }

        // 4. Provenance verification: source_event_id must exist in action_requests
        let provenance_exists = if let Some(id) = ctx.source_event_id {
            self.verify_provenance(id)?
        } else {
            false
        };

        if !provenance_exists {
            let reason = "No provenance found in GuardDb; detection != authorization".to_string();
            warn!(action = %ctx.action_type, %reason, "Gate interrupted");
            return Ok(GateResult::Interrupted { reason });
        }

        // 5. Confidence threshold
        if ctx.confidence.get() < self.risk_config.confidence_floor {
            let reason = format!(
                "Confidence {:.3} below floor {:.3}; must surface before execution",
                ctx.confidence.get(),
                self.risk_config.confidence_floor
            );
            warn!(
                action = %ctx.action_type,
                confidence = ctx.confidence.get(),
                %reason,
                "Gate interrupted"
            );
            return Ok(GateResult::Interrupted { reason });
        }

        // 6. Interruptibility check
        if !ctx.can_interrupt {
            let reason =
                "Action cannot be interrupted; gate safety requirement not met".to_string();
            warn!(action = %ctx.action_type, %reason, "Gate interrupted");
            return Ok(GateResult::Interrupted { reason });
        }

        // 7. Trust layer check
        let can_auto = self.trust.can_auto_execute(ctx.trust_layer)?;
        let needs_review = self.trust.requires_review(ctx.trust_layer)?;

        if needs_review {
            debug!(
                action = %ctx.action_type,
                trust_layer = ctx.trust_layer,
                "Action requires human review"
            );
            return Ok(GateResult::Pending);
        }

        if !can_auto {
            let reason = format!(
                "Trust layer {} does not allow auto-execute",
                ctx.trust_layer
            );
            return Ok(GateResult::Interrupted { reason });
        }

        // 8. PID trust check
        if let Some(pid) = ctx.pid
            && let Some(gate_result) = self.check_pid_trust(pid, ctx.command)?
        {
            return Ok(gate_result);
        }

        // 9. Command risk assessment
        let risk = self.assess_command_risk(ctx.command, &ctx.args);
        match risk {
            CommandRisk::High => {
                return Ok(GateResult::Interrupted {
                    reason: format!("High-risk command '{}' detected", ctx.command),
                });
            }
            CommandRisk::Medium => {
                debug!(
                    action = %ctx.action_type,
                    command = %ctx.command,
                    "Medium-risk command requires review"
                );
                return Ok(GateResult::Pending);
            }
            CommandRisk::Low => {
                debug!(action = %ctx.action_type, "Low-risk command approved");
            }
            CommandRisk::Unauthorized => {
                return Ok(GateResult::Interrupted {
                    reason: "Unauthorized action: detection != authorization".to_string(),
                });
            }
        }

        Ok(GateResult::Proceed)
    }

    /// Gate knowledge write based on source.
    ///
    /// External-sourced writes always land in quarantine (pending) regardless
    /// of confidence. User/Agent writes follow normal trust layer gating.
    pub fn check_knowledge_write(&self, source: &str) -> Result<GateResult, GuardDbError> {
        match source {
            "external" => {
                debug!(source = source, "Knowledge write from external source → quarantine");
                Ok(GateResult::Pending)
            }
            "agent" | "system" => {
                debug!(source = source, "Knowledge write from trusted source");
                Ok(GateResult::Proceed)
            }
            "user" => {
                Ok(GateResult::Proceed)
            }
            _ => {
                warn!(source = source, "Unknown knowledge write source → quarantine");
                Ok(GateResult::Pending)
            }
        }
    }

    /// Verify provenance: source_event_id exists in GuardDb action_requests.
    fn verify_provenance(&self, id: &str) -> Result<bool, GuardDbError> {
        let conn = self.trust.conn();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM action_requests WHERE source_event_id = ?1)",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .map_err(|_| GuardDbError::SchemaError("Provenance check failed".into()))?;
        Ok(exists)
    }

    /// Check PID trust layer.
    /// Returns Revoked if trust has decayed below the action's trust layer.
    fn check_pid_trust(&self, pid: i32, command: &str) -> Result<Option<GateResult>, GuardDbError> {
        let conn = self.trust.conn();

        let row = conn.query_row(
            "SELECT trust_layer, use_count, last_use, auto_grant, decay_interval, decay_rate
             FROM pid_trust WHERE pid = ?1",
            rusqlite::params![pid],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, f64>(5)?,
                ))
            },
        );

        let (current_layer, _use_count, last_use, auto_grant, decay_interval, decay_rate) =
            match row {
                Ok(r) => r,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Ok(Some(GateResult::Pending));
                }
                Err(e) => return Err(GuardDbError::Sqlite(e)),
            };

        // Compute decayed trust layer
        let elapsed = chrono::Utc::now().timestamp() - last_use;
        let effective_layer = if elapsed > decay_interval {
            let decayed = (decay_rate * elapsed as f64).min(1.0);
            let new_confidence = (current_layer as f64 / 3.0) * (1.0 - decayed);
            (new_confidence * 3.0) as u32
        } else {
            current_layer
        };

        // If trust has dropped below the action's layer, revoke
        if effective_layer < (self.risk_config.confidence_floor * 3.0) as u32 {
            let reason = format!(
                "PID {} trust decayed from layer {} to {} (auto_grant={})",
                pid, current_layer, effective_layer, auto_grant
            );
            warn!(pid, command, %reason, "PID trust revoked");
            return Ok(Some(GateResult::Revoked { reason }));
        }

        Ok(None)
    }

    /// Assess command risk based on name and arguments.
    fn assess_command_risk(&self, command: &str, args: &[String]) -> CommandRisk {
        let basename = command.split('/').next_back().unwrap_or(command);

        for risk_cmd in &self.risk_config.high_risk {
            if basename.eq_ignore_ascii_case(risk_cmd) {
                return CommandRisk::High;
            }
            let full_cmd = format!("{} {}", basename, args.join(" "));
            if full_cmd.eq_ignore_ascii_case(risk_cmd) {
                return CommandRisk::High;
            }
        }

        for risk_cmd in &self.risk_config.medium_risk {
            if basename.eq_ignore_ascii_case(risk_cmd) {
                return CommandRisk::Medium;
            }
        }

        CommandRisk::Low
    }
}

/// Context for gate checks
pub struct GateContext<'a> {
    pub action_type: &'a str,
    pub command: &'a str,
    pub args: Vec<String>,
    pub trust_layer: u32,
    pub confidence: TrustScore,
    pub source_event_id: Option<&'a str>,
    pub has_raw_data: bool,
    pub has_uncertainty: bool,
    pub can_interrupt: bool,
    /// PID of the calling process (for pid_trust lookup)
    pub pid: Option<i32>,
}

/// Risk level for a command. Higher risk means more scrutiny.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    Low,
    Medium,
    High,
    Unauthorized,
}

#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub high_risk: Vec<String>,
    pub medium_risk: Vec<String>,
    pub confidence_floor: f64,
    pub set_at: i64,
    pub reason: String,
    pub source: String,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            high_risk: HIGH_RISK_COMMANDS.iter().map(|s| s.to_string()).collect(),
            medium_risk: MEDIUM_RISK_COMMANDS.iter().map(|s| s.to_string()).collect(),
            confidence_floor: 0.6,
            set_at: chrono::Utc::now().timestamp(),
            reason: "default risk thresholds".to_string(),
            source: "mirror-guard".to_string(),
        }
    }
}

impl RiskConfig {
    pub fn with_high_risk(mut self, commands: Vec<String>) -> Self {
        self.high_risk = commands;
        self.set_at = chrono::Utc::now().timestamp();
        self
    }

    pub fn with_medium_risk(mut self, commands: Vec<String>) -> Self {
        self.medium_risk = commands;
        self.set_at = chrono::Utc::now().timestamp();
        self
    }

    pub fn with_confidence_floor(mut self, floor: f64) -> Self {
        self.confidence_floor = floor.clamp(0.0, 1.0);
        self.set_at = chrono::Utc::now().timestamp();
        self
    }
}

const HIGH_RISK_COMMANDS: &[&str] = &[
    "rm",
    "remove",
    "del",
    "delete",
    "unlink",
    "sudo",
    "su",
    "chmod",
    "chown",
    "mkfs",
    "fdisk",
    "dd",
    "iptables",
    "kill",
    "killall",
    "shutdown",
    "reboot",
    "halt",
    "format",
    "curl",
    "wget",
    "nc",
    "netcat",
    "socat",
    "cp",
    "mv",
    "tar",
    "zip",
    "unzip",
    "pip install",
    "npm install",
    "cargo install",
    "apt",
    "apt-get",
    "yum",
    "dnf",
    "pacman",
];

const MEDIUM_RISK_COMMANDS: &[&str] = &[
    "git", "clone", "checkout", "branch", "docker", "podman", "ssh", "scp", "rsync", "vim", "vi",
    "nano", "emacs", "cargo", "rustc", "python", "pip", "node", "npm", "npx",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annealing::AnnealingPipeline;
    use crate::memory::MemoryGraph;
    use crate::types::NodeKind;
    use crate::types::TrustScore;
    use tempfile::tempdir;

    #[test]
    fn test_gate_proceeds_for_trusted_action() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let conn = db.conn();
        conn.execute(
            "INSERT INTO action_requests (id, source_event_id, action_type, payload, trust_layer, confidence, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "test-action-1",
                "evt-1",
                "echo",
                "hello",
                4,
                0.95,
                "trust-approved",
            ],
        )
        .unwrap();
        drop(conn);

        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 4,
            confidence: TrustScore::new(0.95),
            source_event_id: Some("evt-1"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Proceed);
    }

    #[test]
    fn test_gate_pending_for_working_layer() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let conn = db.conn();
        conn.execute(
            "INSERT INTO action_requests (id, source_event_id, action_type, payload, trust_layer, confidence, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "test-action-2",
                "evt-2",
                "echo",
                "hello",
                2,
                0.65,
                "trust-approved",
            ],
        )
        .unwrap();
        drop(conn);

        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 2,
            confidence: TrustScore::new(0.65),
            source_event_id: Some("evt-2"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Pending);
    }

    #[test]
    fn test_gate_pending_for_low_trust() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let conn = db.conn();
        conn.execute(
            "INSERT INTO action_requests (id, source_event_id, action_type, payload, trust_layer, confidence, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "test-action-4",
                "evt-4",
                "echo",
                "hello",
                0,
                0.65,
                "trust-approved",
            ],
        )
        .unwrap();
        drop(conn);

        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 0,
            confidence: TrustScore::new(0.65),
            source_event_id: Some("evt-4"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Pending);
    }

    #[test]
    fn test_gate_dry_run() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&db, true, dir.path());

        let ctx = GateContext {
            action_type: "rm",
            command: "rm",
            args: vec!["-rf".to_string(), "/".to_string()],
            trust_layer: 0,
            confidence: TrustScore::new(0.65),
            source_event_id: Some("evt-dry-run"),
            has_raw_data: false,
            has_uncertainty: false,
            can_interrupt: false,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::DryRun);
    }

    #[test]
    fn test_high_risk_command_blocked() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let conn = db.conn();
        conn.execute(
            "INSERT INTO action_requests (id, source_event_id, action_type, payload, trust_layer, confidence, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "test-action-5",
                "evt-5",
                "rm",
                "-rf /tmp/test",
                3,
                0.9,
                "trust-approved",
            ],
        )
        .unwrap();
        drop(conn);

        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "delete",
            command: "rm",
            args: vec!["-rf".to_string(), "/tmp/test".to_string()],
            trust_layer: 4,
            confidence: TrustScore::new(0.95),
            source_event_id: Some("evt-5"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert!(matches!(result, GateResult::Interrupted { .. }));
    }

    #[test]
    fn test_medium_risk_command_pending() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let conn = db.conn();
        conn.execute(
            "INSERT INTO action_requests (id, source_event_id, action_type, payload, trust_layer, confidence, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "test-action-6",
                "evt-6",
                "git",
                "commit -m test",
                3,
                0.9,
                "trust-approved",
            ],
        )
        .unwrap();
        drop(conn);

        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "git_commit",
            command: "git",
            args: vec!["commit".to_string(), "-m".to_string(), "test".to_string()],
            trust_layer: 3,
            confidence: TrustScore::new(0.9),
            source_event_id: Some("evt-6"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Pending);
    }

    #[test]
    fn test_gate_interrupts_below_confidence_floor() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 3,
            confidence: TrustScore::new(0.1),
            source_event_id: Some("evt-7"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert!(matches!(result, GateResult::Interrupted { .. }));
    }

    #[test]
    fn test_gate_denies_missing_provenance() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 3,
            confidence: TrustScore::new(0.9),
            source_event_id: Some("nonexistent-provenance"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert!(matches!(result, GateResult::Interrupted { .. }));
    }

    #[test]
    fn test_gate_proceeds_with_valid_provenance() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();

        let conn = db.conn();
        conn.execute(
            "INSERT INTO action_requests (id, source_event_id, action_type, payload, trust_layer, confidence, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "test-action-1",
                "evt-1",
                "echo",
                "hello",
                4,
                0.95,
                "trust-approved",
            ],
        )
        .unwrap();
        drop(conn);

        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 4,
            confidence: TrustScore::new(0.95),
            source_event_id: Some("evt-1"),
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Proceed);
    }

    #[test]
    fn test_source_event_node_distinct() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let mg = MemoryGraph::new(&db);
        let pipeline = AnnealingPipeline::new(&db);

        let node_id = mg
            .add_node(NodeKind::Fact, "distinct ids", TrustScore::new(0.5))
            .unwrap();

        let pipeline = pipeline.unwrap();
        let request = pipeline
            .request_action(
                Some("raw-event-123".to_string()),
                Some(node_id.clone()),
                "test_action",
                "payload",
                3,
                TrustScore::new(0.9),
            )
            .unwrap();

        assert_eq!(request.source_event_id, Some("raw-event-123".to_string()));
        assert_eq!(request.source_node_id, Some(node_id));
        assert_ne!(request.source_event_id, request.source_node_id);
    }

    #[test]
    fn test_gate_interrupts_without_raw_data() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 2,
            confidence: TrustScore::new(0.65),
            source_event_id: Some("evt-3"),
            has_raw_data: false,
            has_uncertainty: true,
            can_interrupt: true,
            pid: None,
        };

        let result = gate.check(ctx).unwrap();
        assert!(matches!(result, GateResult::Interrupted { .. }));
    }
}
