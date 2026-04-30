use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::guard_db::{GuardDb, GuardDbError};
use crate::trust::TrustManager;
use crate::types::*;

/// Result of a gate check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResult {
    Proceed,
    Interrupted { reason: String },
    Pending,
    DryRun,
}

/// Execution gate that combines trust-layer gating with command security checks.
///
/// This is the single point where the system transitions from detection to action.
/// Detection != authorization: knowing what happened does not grant the right to change what happens.
pub struct ExecutionGate<'a> {
    db: &'a GuardDb,
    trust: TrustManager<'a>,
    dry_run: bool,
    root: PathBuf,
}

impl<'a> ExecutionGate<'a> {
    pub fn new(db: &'a GuardDb, dry_run: bool, root: impl Into<PathBuf>) -> Self {
        Self {
            db,
            trust: TrustManager::new(db),
            dry_run,
            root: root.into(),
        }
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
            let reason = "Action triggered without raw data reference; detection != authorization".to_string();
            warn!(action = %ctx.action_type, %reason, "Gate interrupted");
            return Ok(GateResult::Interrupted { reason });
        }

        // 3. Uncertainty exposure
        if !ctx.has_uncertainty {
            let reason = "Action triggered without uncertainty exposure; gate not enforced".to_string();
            warn!(action = %ctx.action_type, %reason, "Gate interrupted");
            return Ok(GateResult::Interrupted { reason });
        }

        // 4. Interruptibility check
        if !ctx.can_interrupt {
            let reason = "Action cannot be interrupted; gate safety requirement not met".to_string();
            warn!(action = %ctx.action_type, %reason, "Gate interrupted");
            return Ok(GateResult::Interrupted { reason });
        }

        // 5. Trust layer check
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

        // 6. Command risk assessment (from existing guard logic)
        let risk = self.assess_command_risk(&ctx.command, &ctx.args);
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
                    "Medium-risk command flagged"
                );
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

    /// Assess command risk based on name and arguments.
    fn assess_command_risk(&self, command: &str, args: &[String]) -> CommandRisk {
        let basename = command.split('/').next_back().unwrap_or(command);
        let args_strs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        for risk_cmd in HIGH_RISK_COMMANDS {
            if basename.eq_ignore_ascii_case(risk_cmd) {
                return CommandRisk::High;
            }
            let full_cmd = format!("{} {}", basename, args.join(" "));
            if full_cmd.eq_ignore_ascii_case(*risk_cmd) {
                return CommandRisk::High;
            }
        }

        for risk_cmd in MEDIUM_RISK_COMMANDS {
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
    pub has_raw_data: bool,
    pub has_uncertainty: bool,
    pub can_interrupt: bool,
}

/// Risk level for a command. Higher risk means more scrutiny.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    Low,
    Medium,
    High,
    Unauthorized,
}

const HIGH_RISK_COMMANDS: &[&str] = &[
    "rm", "remove", "del", "delete", "unlink", "sudo", "su", "chmod", "chown",
    "mkfs", "fdisk", "dd", "iptables", "kill", "killall", "shutdown", "reboot",
    "halt", "format", "curl", "wget", "nc", "netcat", "socat", "cp", "mv",
    "tar", "zip", "unzip", "pip install", "npm install", "cargo install",
    "apt", "apt-get", "yum", "dnf", "pacman",
];

const MEDIUM_RISK_COMMANDS: &[&str] = &[
    "git", "clone", "checkout", "branch", "docker", "podman", "ssh", "scp",
    "rsync", "vim", "vi", "nano", "emacs", "cargo", "rustc", "python",
    "pip", "node", "npm", "npx",
];

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_gate_proceeds_for_trusted_action() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 2,
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::Proceed);
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
            has_raw_data: false,
            has_uncertainty: true,
            can_interrupt: true,
        };

        let result = gate.check(ctx).unwrap();
        assert!(matches!(result, GateResult::Interrupted { .. }));
    }

    #[test]
    fn test_gate_pending_for_low_trust() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "echo",
            command: "echo",
            args: vec!["hello".to_string()],
            trust_layer: 0,
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
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
            has_raw_data: false,
            has_uncertainty: false,
            can_interrupt: false,
        };

        let result = gate.check(ctx).unwrap();
        assert_eq!(result, GateResult::DryRun);
    }

    #[test]
    fn test_high_risk_command_blocked() {
        let dir = tempdir().unwrap();
        let db = GuardDb::open(dir.path().join("guard.db")).unwrap();
        let gate = ExecutionGate::new(&db, false, dir.path());

        let ctx = GateContext {
            action_type: "delete",
            command: "rm",
            args: vec!["-rf".to_string(), "/tmp/test".to_string()],
            trust_layer: 3,
            has_raw_data: true,
            has_uncertainty: true,
            can_interrupt: true,
        };

        let result = gate.check(ctx).unwrap();
        assert!(matches!(result, GateResult::Interrupted { .. }));
    }
}
