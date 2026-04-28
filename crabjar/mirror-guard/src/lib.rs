/// mirror-guard: Security layer for the CrabJar orchestrator.
///
/// Provides three complementary checks that act as a "lock with an alarm":
///
/// 1. **Gitignore pattern matcher** — files already marked as `.gitignore`
///    are considered "safe" because the human has explicitly excluded them.
/// 2. **Shell history whitelist** — commands the user has run in their
///    shell history are whitelisted, trusting human intent.
/// 3. **Command permission gating** — high-risk commands are denied or
///    flagged for confirmation before execution.
///
/// The checks are applied in order: whitelist → gitignore → risk assessment.
/// A command passes if it is whitelisted, or if gitignore matches the
/// target path, or if the risk level is low.
use ignore::gitignore::Gitignore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Risk level for a command. Higher risk means more scrutiny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandRisk {
    /// Safe to execute without further checks.
    Low,
    /// Should be flagged for confirmation but not auto-denied.
    Medium,
    /// Auto-denied unless explicitly whitelisted.
    High,
    /// Action taken without gate enforcement — detection ≠ authorization.
    Unauthorized,
}

/// Result of a security check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum SecurityCheckResult {
    /// Command is approved for execution.
    Approved {
        /// Reason the command was approved.
        reason: String,
    },
    /// Command should be flagged for user confirmation.
    Flagged {
        /// Warning message for the user.
        warning: String,
        /// Risk level assessed.
        risk: CommandRisk,
    },
    /// Command is denied.
    Denied {
        /// Reason the command was denied.
        reason: String,
        /// Risk level assessed.
        risk: CommandRisk,
    },
}

/// Error types for mirror-guard operations.
#[derive(Debug, Error)]
pub enum GuardError {
    #[error("failed to load gitignore patterns from {path}: {source}")]
    GitignoreLoad {
        path: PathBuf,
        source: ignore::Error,
    },

    #[error("failed to read shell history at {path}: {source}")]
    HistoryRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("security check failed: {0}")]
    CheckFailed(String),
}

// ---------------------------------------------------------------------------
// Gitignore pattern matcher
// ---------------------------------------------------------------------------

/// Loads `.gitignore` patterns from a directory and its ancestors.
pub struct GitignoreMatcher {
    /// The loaded gitignore matcher.
    gitignore: Gitignore,
    /// The root directory from which patterns were loaded.
    root: PathBuf,
}

impl GitignoreMatcher {
    /// Creates a new matcher by loading gitignore patterns from `root`
    /// and walking up to the filesystem root.
    ///
    /// The `root` directory is treated as the base for pattern matching.
    /// We load `.gitignore` from the root directory.
    pub fn new(root: PathBuf) -> Result<Self, GuardError> {
        let gitignore_path = root.join(".gitignore");

        let (gitignore, err_opt) = Gitignore::new(&gitignore_path);

        // Report any I/O errors but still return a valid matcher.
        if let Some(err) = err_opt {
            return Err(GuardError::GitignoreLoad {
                path: gitignore_path,
                source: err,
            });
        }

        Ok(Self { gitignore, root })
    }

    /// Checks whether `path` is ignored by the loaded gitignore patterns.
    /// Returns `true` if the path matches any ignore rule.
    pub fn is_ignored(&self, path: &Path) -> bool {
        let match_result = self.gitignore.matched(path, path.is_dir());

        matches!(match_result, ignore::Match::Ignore(_))
    }

    /// Returns the root directory from which patterns were loaded.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

// ---------------------------------------------------------------------------
// Shell history whitelist
// ---------------------------------------------------------------------------

/// Parses shell history files to extract whitelisted commands.
pub struct ShellHistoryWhitelist {
    /// Set of whitelisted command names (basename of the executable).
    whitelisted_commands: HashSet<String>,
    /// Set of whitelisted full command strings (for exact matches).
    whitelisted_full_commands: HashSet<String>,
}

impl ShellHistoryWhitelist {
    /// Creates a new whitelist by reading common shell history files.
    ///
    /// Checks these paths in order:
    /// - `$HISTFILE` (if set)
    /// - `~/.zsh_history`
    /// - `~/.bash_history`
    /// - `~/.fish_history`
    pub fn new() -> Result<Self, GuardError> {
        let mut whitelisted_commands = HashSet::new();
        let mut whitelisted_full_commands = HashSet::new();

        // Collect history files to read.
        let history_files = Self::collect_history_files();

        for history_file in history_files {
            if !history_file.exists() {
                continue;
            }

            debug!("Reading shell history from {}", history_file.display());

            match Self::parse_history_file(&history_file) {
                Ok(commands) => {
                    for cmd in commands {
                        // Extract the basename (command name).
                        if let Some(basename) = cmd.split_whitespace().next() {
                            whitelisted_commands.insert(basename.to_string());
                        }
                        // Store the full command for exact matching.
                        whitelisted_full_commands.insert(cmd);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to parse history file {}: {}",
                        history_file.display(),
                        e
                    );
                }
            }
        }

        debug!(
            "Loaded {} whitelisted commands, {} full command patterns",
            whitelisted_commands.len(),
            whitelisted_full_commands.len()
        );

        Ok(Self {
            whitelisted_commands,
            whitelisted_full_commands,
        })
    }

    /// Collects available shell history file paths.
    fn collect_history_files() -> Vec<PathBuf> {
        let mut files = Vec::new();

        // $HISTFILE if set.
        if let Ok(histfile) = std::env::var("HISTFILE") {
            files.push(PathBuf::from(histfile));
        }

        // Common shell history paths.
        if let Ok(home) = std::env::var("HOME") {
            files.push(PathBuf::from(&home).join(".zsh_history"));
            files.push(PathBuf::from(&home).join(".bash_history"));
            files.push(PathBuf::from(&home).join(".fish_history"));
        }

        files
    }

    /// Parses a shell history file and extracts commands.
    ///
    /// Different shells use different formats:
    /// - zsh: `: <timestamp>:<duration>;command`
    /// - bash: plain text, one command per line
    /// - fish: XML-like format
    fn parse_history_file(path: &Path) -> Result<Vec<String>, GuardError> {
        let content = std::fs::read_to_string(path).map_err(|source| GuardError::HistoryRead {
            path: path.to_path_buf(),
            source,
        })?;

        let mut commands = Vec::new();

        if path.ends_with(".zsh_history") {
            // zsh format: `: 1710000000:0;ls -la`
            for line in content.lines() {
                if let Some(cmd) = line.strip_prefix(": ")
                    && let Some(semi_pos) = cmd.find(';')
                {
                    let full_command = cmd[semi_pos + 1..].trim();
                    if !full_command.is_empty() {
                        commands.push(full_command.to_string());
                    }
                }
            }
        } else if path.ends_with(".fish_history") {
            // fish format: `<historyItem>command</historyItem>`
            for line in content.lines() {
                if let Some(cmd) = line.strip_prefix("<historyItem>")
                    && let Some(cmd) = cmd.strip_suffix("</historyItem>")
                    && !cmd.is_empty()
                {
                    commands.push(cmd.to_string());
                }
            }
        } else {
            // bash and others: plain text.
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    commands.push(trimmed.to_string());
                }
            }
        }

        Ok(commands)
    }

    /// Checks whether `command` (the executable name) is whitelisted.
    pub fn is_command_whitelisted(&self, command: &str) -> bool {
        self.whitelisted_commands.contains(command)
    }

    /// Checks whether the full command string is whitelisted.
    pub fn is_full_command_whitelisted(&self, full_command: &str) -> bool {
        self.whitelisted_full_commands.contains(full_command)
    }

    /// Returns the number of whitelisted commands.
    pub fn count(&self) -> usize {
        self.whitelisted_commands.len()
    }
}

// ---------------------------------------------------------------------------
// Command risk assessment
// ---------------------------------------------------------------------------

/// High-risk command patterns that should be flagged or denied.
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
    "firewall",
    "kill",
    "killall",
    "shutdown",
    "reboot",
    "halt",
    "format",
    "curl",
    "wget",
    "fetch",
    "fetchmail",
    "nc",
    "netcat",
    "socat",
    "dd",
    "cp",
    "copy",
    "mv",
    "move",
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
    "make",
    "cmake",
    "cargo build",
    "cargo test",
    "cargo run",
];

/// Medium-risk command patterns that should be flagged.
const MEDIUM_RISK_COMMANDS: &[&str] = &[
    "git",
    "clone",
    "checkout",
    "branch",
    "docker",
    "podman",
    "container",
    "ssh",
    "scp",
    "rsync",
    "vim",
    "vi",
    "nano",
    "emacs",
    "cargo",
    "rustc",
    "python",
    "pip",
    "node",
    "npm",
    "npx",
    "cargo",
    "cargo",
];

/// Assesses whether a detected event has authorization to trigger an action.
/// Detection ≠ authorization — knowing what happened does not grant the right to change what happens.
pub fn assess_detection_authorization(ctx: CheckContext<'_>) -> CommandRisk {
    if !ctx.has_raw_data {
        return CommandRisk::Unauthorized;
    }
    if !ctx.has_uncertainty {
        return CommandRisk::Unauthorized;
    }
    if !ctx.can_interrupt {
        return CommandRisk::Unauthorized;
    }
    CommandRisk::Low
}

/// Context for detection authorization gate.
#[derive(Debug, Clone)]
pub struct CheckContext<'a> {
    pub event_kind: Option<&'a str>,
    pub has_raw_data: bool,
    pub has_uncertainty: bool,
    pub can_interrupt: bool,
}

/// Assesses the risk level of a command based on its name and arguments.
pub fn assess_command_risk(command: &str, args: &[&str]) -> CommandRisk {
    let command_basename = command.split('/').next_back().unwrap_or(command);

    // Check high-risk commands first.
    for risk_cmd in HIGH_RISK_COMMANDS {
        if command_basename.eq_ignore_ascii_case(risk_cmd) {
            return CommandRisk::High;
        }
        // Check full command patterns (e.g., "git push").
        let full_cmd = format!("{} {}", command_basename, args.join(" "));
        let full_cmd_str = full_cmd.to_string();
        if full_cmd_str.eq_ignore_ascii_case(risk_cmd) {
            return CommandRisk::High;
        }
    }

    // Check medium-risk commands.
    for risk_cmd in MEDIUM_RISK_COMMANDS {
        if command_basename.eq_ignore_ascii_case(risk_cmd) {
            return CommandRisk::Medium;
        }
    }

    // Default to low risk.
    CommandRisk::Low
}

// ---------------------------------------------------------------------------
// SecurityGuard: the "lock with an alarm"
// ---------------------------------------------------------------------------

/// The main security guard that combines gitignore matching, shell history
/// whitelisting, command risk assessment, and detection authorization gate.
pub struct SecurityGuard {
    /// Gitignore pattern matcher.
    gitignore: Option<GitignoreMatcher>,
    /// Shell history whitelist.
    history: ShellHistoryWhitelist,
}

impl SecurityGuard {
    /// Creates a new security guard.
    ///
    /// `root` is the directory from which gitignore patterns are loaded.
    /// If gitignore loading fails, the guard still works with just the
    /// shell history whitelist and risk assessment.
    pub fn new(root: PathBuf) -> Self {
        let gitignore = GitignoreMatcher::new(root).ok();
        let history = ShellHistoryWhitelist::new().unwrap_or_else(|e| {
            warn!("Failed to load shell history whitelist: {}", e);
            // Return an empty whitelist as a fallback.
            Self::empty_whitelist()
        });

        Self { gitignore, history }
    }

    /// Creates a security guard with an empty whitelist (for testing).
    pub(crate) fn empty_whitelist() -> ShellHistoryWhitelist {
        ShellHistoryWhitelist {
            whitelisted_commands: HashSet::new(),
            whitelisted_full_commands: HashSet::new(),
        }
    }

    /// Performs a full security check on a command.
    ///
    /// The check proceeds in four stages:
    /// 1. **Whitelist** — if the command or full command string is whitelisted, approve.
    /// 2. **Gitignore** — if the target path is gitignored, approve (human has excluded it).
    /// 3. **Detection authorization** — if action is triggered by detected event, gate must enforce raw data reference + uncertainty exposure + interruptibility.
    /// 4. **Risk assessment** — deny high-risk commands, flag medium-risk ones.
    pub fn check(
        &self,
        command: &str,
        args: &[&str],
        target_path: Option<&Path>,
        ctx: CheckContext,
    ) -> SecurityCheckResult {
        // Stage 1: Whitelist check.
        let command_basename = command.split('/').next_back().unwrap_or(command);
        let full_command = format!("{} {}", command_basename, args.join(" "));
        let full_command_str = full_command.to_string();

        if self.history.is_command_whitelisted(command_basename) {
            debug!(
                "Command '{}' is whitelisted from shell history",
                command_basename
            );
            return SecurityCheckResult::Approved {
                reason: format!(
                    "Command '{}' is whitelisted from shell history",
                    command_basename
                ),
            };
        }

        if self.history.is_full_command_whitelisted(&full_command_str) {
            debug!(
                "Full command '{}' is whitelisted from shell history",
                full_command
            );
            return SecurityCheckResult::Approved {
                reason: format!(
                    "Full command '{}' is whitelisted from shell history",
                    full_command
                ),
            };
        }

        // Stage 2: Gitignore check.
        if let Some(ref gitignore) = self.gitignore
            && let Some(target) = target_path
            && gitignore.is_ignored(target)
        {
            debug!(
                "Path '{}' is matched by gitignore patterns",
                target.display()
            );
            return SecurityCheckResult::Approved {
                reason: format!(
                    "Target path '{}' is matched by gitignore patterns",
                    target.display()
                ),
            };
        }

        // Stage 3: Detection authorization gate.
        if let Some(event_kind) = ctx.event_kind {
            let auth_risk = assess_detection_authorization(CheckContext {
                event_kind: Some(event_kind),
                has_raw_data: ctx.has_raw_data,
                has_uncertainty: ctx.has_uncertainty,
                can_interrupt: ctx.can_interrupt,
            });

            if auth_risk == CommandRisk::Unauthorized {
                warn!(
                    "Action triggered by detected event '{}' without gate enforcement; denied",
                    event_kind
                );
                return SecurityCheckResult::Denied {
                    reason: format!(
                        "Action triggered by detected event '{}' — detection ≠ authorization; gate not enforced",
                        event_kind
                    ),
                    risk: CommandRisk::Unauthorized,
                };
            }
        }

        // Stage 4: Risk assessment.
        let risk = assess_command_risk(command, args);

        match risk {
            CommandRisk::High => {
                warn!(
                    "High-risk command detected: {} with args {:?}",
                    command, args
                );
                SecurityCheckResult::Denied {
                    reason: format!(
                        "High-risk command '{}' detected; denied unless whitelisted",
                        command
                    ),
                    risk: CommandRisk::High,
                }
            }
            CommandRisk::Medium => {
                warn!(
                    "Medium-risk command detected: {} with args {:?}",
                    command, args
                );
                SecurityCheckResult::Flagged {
                    warning: format!(
                        "Medium-risk command '{}' — please confirm execution",
                        command
                    ),
                    risk: CommandRisk::Medium,
                }
            }
            CommandRisk::Low => {
                debug!("Low-risk command: {} with args {:?}", command, args);
                SecurityCheckResult::Approved {
                    reason: format!("Low-risk command '{}'", command),
                }
            }
            CommandRisk::Unauthorized => {
                warn!(
                    "Unauthorized action detected: {} with args {:?}",
                    command, args
                );
                SecurityCheckResult::Denied {
                    reason: format!(
                        "Action '{}' — detection ≠ authorization; gate not enforced",
                        command
                    ),
                    risk: CommandRisk::Unauthorized,
                }
            }
        }
    }

    /// Returns the number of whitelisted commands.
    pub fn whitelisted_count(&self) -> usize {
        self.history.count()
    }

    /// Returns whether gitignore patterns are loaded.
    pub fn has_gitignore(&self) -> bool {
        self.gitignore.is_some()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_gitignore_matcher_new() {
        let dir = tempdir().unwrap();
        let matcher = GitignoreMatcher::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(matcher.root(), dir.path());
    }

    #[test]
    fn test_gitignore_matcher_is_ignored() {
        let dir = tempdir().unwrap();
        let gitignore_path = dir.path().join(".gitignore");
        std::fs::write(&gitignore_path, "*.tmp\nlogs/\n").unwrap();

        let matcher = GitignoreMatcher::new(dir.path().to_path_buf()).unwrap();

        // Create a .tmp file.
        let tmp_file = dir.path().join("test.tmp");
        std::fs::write(&tmp_file, "data").unwrap();
        assert!(matcher.is_ignored(&tmp_file));

        // Create a logs directory.
        let logs_dir = dir.path().join("logs");
        std::fs::create_dir(&logs_dir).unwrap();
        assert!(matcher.is_ignored(&logs_dir));

        // A non-matching file.
        let other_file = dir.path().join("other.rs");
        std::fs::write(&other_file, "code").unwrap();
        assert!(!matcher.is_ignored(&other_file));
    }

    #[test]
    fn test_shell_history_whitelist_zsh_format() {
        let dir = tempdir().unwrap();
        let history_path = dir.path().join(".zsh_history");
        std::fs::write(
            &history_path,
            ": 1710000000:0;ls -la\n: 1710000001:0;cargo build\n: 1710000002:0;echo hello\n",
        )
        .unwrap();

        // Temporarily set HOME to the temp dir for parsing.
        let old_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", dir.path().to_str().unwrap());
        }

        let whitelist = ShellHistoryWhitelist::new().unwrap();
        assert!(whitelist.is_command_whitelisted("ls"));
        assert!(whitelist.is_command_whitelisted("cargo"));
        assert!(whitelist.is_command_whitelisted("echo"));

        // Restore HOME.
        if let Some(home) = old_home {
            unsafe {
                std::env::set_var("HOME", home);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn test_shell_history_whitelist_bash_format() {
        let dir = tempdir().unwrap();
        let history_path = dir.path().join(".bash_history");
        std::fs::write(
            &history_path,
            "ls -la\ncargo build\necho hello\n# commented line\n",
        )
        .unwrap();

        let old_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", dir.path().to_str().unwrap());
        }

        let whitelist = ShellHistoryWhitelist::new().unwrap();
        assert!(whitelist.is_command_whitelisted("ls"));
        assert!(whitelist.is_command_whitelisted("cargo"));
        assert!(whitelist.is_command_whitelisted("echo"));

        if let Some(home) = old_home {
            unsafe {
                std::env::set_var("HOME", home);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn test_command_risk_assessment() {
        // High-risk commands.
        assert_eq!(
            assess_command_risk("rm", &["-rf", "/tmp/test"]),
            CommandRisk::High
        );
        assert_eq!(assess_command_risk("sudo", &["reboot"]), CommandRisk::High);
        assert_eq!(
            assess_command_risk("git", &["push", "--force"]),
            CommandRisk::Medium
        );

        // Medium-risk commands.
        assert_eq!(
            assess_command_risk("git", &["clone", "https://example.com/repo"]),
            CommandRisk::Medium
        );
        assert_eq!(
            assess_command_risk("docker", &["run", "nginx"]),
            CommandRisk::Medium
        );

        // Low-risk commands.
        assert_eq!(assess_command_risk("ls", &["-la"]), CommandRisk::Low);
        assert_eq!(assess_command_risk("echo", &["hello"]), CommandRisk::Low);
        // cargo is medium risk (it can build/install anything).
        assert_eq!(
            assess_command_risk("cargo", &["check"]),
            CommandRisk::Medium
        );
    }

    #[test]
    fn test_security_guard_whitelist_approval() {
        let _dir = tempdir().unwrap();

        // Use an empty whitelist and manually add the command to ensure no pollution from system environment.
        let mut guard = SecurityGuard {
            gitignore: None,
            history: SecurityGuard::empty_whitelist(),
        };
        guard.history.whitelisted_commands.insert("ls".to_string());

        // Whitelisted command should be approved.
        let result = guard.check(
            "ls",
            &["-la"],
            None,
            CheckContext {
                event_kind: None,
                has_raw_data: false,
                has_uncertainty: false,
                can_interrupt: false,
            },
        );
        match result {
            SecurityCheckResult::Approved { ref reason } => {
                assert!(reason.contains("whitelisted"));
            }
            _ => panic!("Expected Approved, got {:?}", result),
        }
    }

    #[test]
    fn test_security_guard_gitignore_approval() {
        let dir = tempdir().unwrap();
        let gitignore_path = dir.path().join(".gitignore");
        std::fs::write(&gitignore_path, "*.tmp\n").unwrap();

        let tmp_file = dir.path().join("test.tmp");
        std::fs::write(&tmp_file, "data").unwrap();

        let guard = SecurityGuard::new(dir.path().to_path_buf());

        // Gitignored path should be approved even for risky commands.
        let result = guard.check(
            "rm",
            &["-f"],
            Some(&tmp_file),
            CheckContext {
                event_kind: None,
                has_raw_data: false,
                has_uncertainty: false,
                can_interrupt: false,
            },
        );
        match result {
            SecurityCheckResult::Approved { ref reason } => {
                assert!(reason.contains("gitignore"));
            }
            _ => panic!("Expected Approved, got {:?}", result),
        }
    }

    #[test]
    fn test_security_guard_high_risk_denial() {
        let guard = SecurityGuard {
            gitignore: None,
            history: SecurityGuard::empty_whitelist(),
        };

        // High-risk command without whitelist or gitignore should be denied.
        let result = guard.check(
            "rm",
            &["-rf", "/tmp/test"],
            None,
            CheckContext {
                event_kind: None,
                has_raw_data: false,
                has_uncertainty: false,
                can_interrupt: false,
            },
        );
        match result {
            SecurityCheckResult::Denied { ref reason, risk } => {
                assert!(reason.contains("High-risk"));
                assert_eq!(risk, CommandRisk::High);
            }
            _ => panic!("Expected Denied, got {:?}", result),
        }
    }

    #[test]
    fn test_security_guard_medium_risk_flagging() {
        let guard = SecurityGuard {
            gitignore: None,
            history: SecurityGuard::empty_whitelist(),
        };

        // Medium-risk command should be flagged.
        let result = guard.check(
            "git",
            &["push"],
            None,
            CheckContext {
                event_kind: None,
                has_raw_data: false,
                has_uncertainty: false,
                can_interrupt: false,
            },
        );
        match result {
            SecurityCheckResult::Flagged { ref warning, risk } => {
                assert!(warning.contains("Medium-risk"));
                assert_eq!(risk, CommandRisk::Medium);
            }
            _ => panic!("Expected Flagged, got {:?}", result),
        }
    }

    #[test]
    fn test_security_guard_low_risk_approval() {
        let guard = SecurityGuard {
            gitignore: None,
            history: SecurityGuard::empty_whitelist(),
        };

        // Low-risk command should be approved.
        let result = guard.check(
            "ls",
            &["-la"],
            None,
            CheckContext {
                event_kind: None,
                has_raw_data: false,
                has_uncertainty: false,
                can_interrupt: false,
            },
        );
        match result {
            SecurityCheckResult::Approved { ref reason } => {
                assert!(reason.contains("Low-risk"));
            }
            _ => panic!("Expected Approved, got {:?}", result),
        }
    }
}
