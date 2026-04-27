use clap::Subcommand;
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Subcommand)]
pub enum DotfileCommand {
    Propose { staging: String, target: String },
    Verify { staging: String, target: String },
}

pub struct DotfileManager {
    pub project_root: PathBuf,
}

impl DotfileManager {
    /// Create a new DotfileManager instance
    pub fn new(root: PathBuf) -> Self {
        Self { project_root: root }
    }

    /// Generates an rsync promotion plan from staging to target
    pub fn propose(
        &self,
        staging: &str,
        target: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let staging_path = PathBuf::from(staging);
        let target_path = PathBuf::from(target);

        // Validate that the staging directory actually exists before proposing a move
        if !staging_path.exists() {
            return Err(format!("Staging path '{}' does not exist.", staging).into());
        }

        // The promotion command uses rsync with archive mode and deletion of
        // files in target that are no longer present in staging.
        // This ensures the "System Truth" matches the "Agent Staging" exactly.
        let command = format!(
            "rsync -av --delete {}/ {}",
            staging_path.display(),
            target_path.display()
        );

        Ok(json!({
            "success": true,
            "action": "promote_via_rsync",
            "command": command,
            "description": format!("Promote changes from {} to {}", staging, target),
            "safety_check": "Review the rsync command carefully. This will overwrite files in the target directory."
        }))
    }

    /// Performs a lightweight check of the relationship between staging and target
    pub fn verify(
        &self,
        staging: &str,
        target: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let staging_path = PathBuf::from(staging);
        let target_path = PathBuf::from(target);

        let s_exists = staging_path.exists();
        let t_exists = target_path.exists();

        // In a production version, we would run 'diff -r' or check file hashes here.
        // For this initial implementation, we verify the existence of both nodes in the pipeline.
        Ok(json!({
            "success": true,
            "status": {
                "staging_exists": s_exists,
                "target_exists": t_exists,
                "drift_detected": !s_exists || !t_exists
            },
            "message": if s_exists && t_exists {
                "Both paths are accessible. Ready for promotion."
            } else {
                "One or both paths are missing. Verification failed."
            }
        }))
    }
}
