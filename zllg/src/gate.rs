use std::path::Path;

use crate::error::{Result, ZllgError};
use crate::layout;
use crate::logger::ZllgLogger;
use crate::tools;

pub enum GateResult {
    Proceed,
    Interrupted { reason: String },
    DryRun,
}

pub struct ExecutionGate<'a> {
    dry_run: bool,
    logger: Option<&'a ZllgLogger>,
}

impl<'a> ExecutionGate<'a> {
    pub fn new(dry_run: bool, logger: Option<&'a ZllgLogger>) -> Self {
        Self { dry_run, logger }
    }

    /// Run pre-flight checks before boot.
    pub fn check(&self, layout_name: &str, cwd: &Path) -> Result<GateResult> {
        // 1. Toolchain preflight
        let required_ok = Self::check_required_tools();
        if !required_ok {
            let reason = "missing required tool (zellij or wezterm)".to_string();
            if let Some(logger) = self.logger {
                let _ = logger.log(
                    "zllg boot interrupted by gate",
                    Some(&format!("{{\"gate_reason\":\"{reason}\"}}")),
                );
            }
            return Ok(GateResult::Interrupted { reason });
        }

        // 2. Layout exists
        let layout_path = layout::resolve_layout(layout_name)
            .map_err(|e| ZllgError::layout(format!("layout resolution failed: {e}")))?;

        // 3. Raw data capture in meta
        let meta = serde_json::json!({
            "project_type": layout_name,
            "layout": layout_path.display().to_string(),
            "dir": cwd.display().to_string(),
        })
        .to_string();

        // 4. Dry-run check
        if self.dry_run {
            if let Some(logger) = self.logger {
                let _ = logger.log("zllg boot dry-run", Some(&meta));
            }
            println!("dry-run: would boot with layout {}", layout_path.display());
            println!("  dir: {}", cwd.display());
            return Ok(GateResult::DryRun);
        }

        // 5. Boot start event
        if let Some(logger) = self.logger {
            let _ = logger.log("zllg boot initiated", Some(&meta));
        }

        Ok(GateResult::Proceed)
    }

    fn check_required_tools() -> bool {
        tools::is_available("zellij") && tools::is_available("wezterm")
    }
}
