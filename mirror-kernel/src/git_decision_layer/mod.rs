use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{MirrorTag, Reflection};

/// Git-based decision layer that encodes decisions as commits.
pub struct GitDecisionLayer {
    repo_path: PathBuf,
    decision_dir: PathBuf,
}

impl GitDecisionLayer {
    /// Create a new Git decision layer at the specified repository path.
    pub fn new(repo_path: &Path) -> Result<Self, GitError> {
        let decision_dir = repo_path.join("mirror_decisions");

        if !repo_path.exists() {
            fs::create_dir_all(repo_path)
                .map_err(|e| GitError::FileError(repo_path.into(), e.to_string()))?;
            run_git(repo_path, ["init", "-q"])?;
        }

        if !decision_dir.exists() {
            fs::create_dir_all(&decision_dir)
                .map_err(|e| GitError::FileError(decision_dir.clone(), e.to_string()))?;
        }

        Ok(Self {
            repo_path: repo_path.to_path_buf(),
            decision_dir,
        })
    }

    pub fn commit_decision(
        &self,
        selected_reflection: &Reflection,
        context_tags: &[MirrorTag],
        kernel_name: &str,
        reason: &str,
        event_ids: &[String],
    ) -> Result<String, GitError> {
        let sanitized_name = Self::sanitize_filename(kernel_name);
        let filename = format!(
            "decision_{}_{}.json",
            Utc::now().timestamp_millis(),
            sanitized_name
        );
        let relative_path = Path::new("mirror_decisions").join(&filename);
        let file_path = self.decision_dir.join(&filename);

        let decision_blob = DecisionBlob {
            selected_reflection: selected_reflection.clone(),
            context_tags: context_tags.to_vec(),
            kernel_name: kernel_name.to_string(),
            reason: reason.to_string(),
            event_ids: event_ids.to_vec(),
            timestamp: Utc::now(),
            commit_hash: String::new(),
        };

        self.write_decision_blob(&file_path, &decision_blob)?;
        run_git(&self.repo_path, ["add", &relative_path.to_string_lossy()])?;

        let commit_message = self.format_commit_message(&decision_blob);
        run_git(
            &self.repo_path,
            ["commit", "-m", &commit_message, "--allow-empty"],
        )?;

        let commit_hash = self.get_latest_commit_hash()?;
        let mut updated_blob = decision_blob;
        updated_blob.commit_hash = commit_hash.clone();
        self.write_decision_blob(&file_path, &updated_blob)?;
        run_git(&self.repo_path, ["add", &relative_path.to_string_lossy()])?;
        run_git(&self.repo_path, ["commit", "--amend", "--no-edit"])?;

        Ok(commit_hash)
    }

    pub fn get_all_decisions(&self) -> Result<Vec<DecisionBlob>, GitError> {
        let mut decisions = Vec::new();

        if !self.decision_dir.exists() {
            return Ok(decisions);
        }

        for entry in fs::read_dir(&self.decision_dir)
            .map_err(|e| GitError::FileError(self.decision_dir.clone(), e.to_string()))?
        {
            let path = entry
                .map_err(|e| GitError::FileError(self.decision_dir.clone(), e.to_string()))?
                .path();

            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let mut content = String::new();
            File::open(&path)
                .and_then(|mut file| file.read_to_string(&mut content))
                .map_err(|e| GitError::FileError(path.clone(), e.to_string()))?;

            let blob = serde_json::from_str::<DecisionBlob>(&content)
                .map_err(|e| GitError::SerializationError(e.to_string()))?;
            decisions.push(blob);
        }

        Ok(decisions)
    }

    pub fn get_decisions_by_kernel(
        &self,
        kernel_name: &str,
    ) -> Result<Vec<DecisionBlob>, GitError> {
        Ok(self
            .get_all_decisions()?
            .into_iter()
            .filter(|decision| decision.kernel_name == kernel_name)
            .collect())
    }

    pub fn get_decisions_by_tag(&self, tag: &MirrorTag) -> Result<Vec<DecisionBlob>, GitError> {
        Ok(self
            .get_all_decisions()?
            .into_iter()
            .filter(|decision| decision.context_tags.contains(tag))
            .collect())
    }

    pub fn get_decision_history(&self) -> Result<Vec<DecisionBlob>, GitError> {
        let mut decisions = self.get_all_decisions()?;
        decisions.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
        Ok(decisions)
    }

    pub fn get_decision_tree(&self) -> Result<DecisionTree, GitError> {
        let mut tree = DecisionTree::new();
        for decision in self.get_all_decisions()? {
            tree.add_decision(decision);
        }
        Ok(tree)
    }

    pub fn create_branch(&self, branch_name: &str) -> Result<(), GitError> {
        run_git(&self.repo_path, ["branch", branch_name]).map(|_| ())
    }

    pub fn merge_branch(&self, branch_name: &str) -> Result<(), GitError> {
        run_git(&self.repo_path, ["merge", branch_name, "--no-edit"]).map(|_| ())
    }

    pub fn rebase_branch(&self, branch_name: &str) -> Result<(), GitError> {
        run_git(&self.repo_path, ["rebase", branch_name]).map(|_| ())
    }

    /// Sanitize a string for safe use as a filename component.
    /// Strips path separators, null bytes, and control characters.
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .filter(|c| {
                !c.is_control()
                    && *c != '/'
                    && *c != '\\'
                    && *c != ':'
                    && *c != '*'
                    && *c != '?'
                    && *c != '"'
                    && *c != '<'
                    && *c != '>'
                    && *c != '|'
            })
            .collect()
    }

    fn format_commit_message(&self, blob: &DecisionBlob) -> String {
        format!(
            "Mirror Decision: Select {} reflection ({})",
            blob.kernel_name,
            blob.selected_reflection.new_content.len()
        )
    }

    fn get_latest_commit_hash(&self) -> Result<String, GitError> {
        let output = run_git(&self.repo_path, ["rev-parse", "HEAD"])?;
        Ok(output.trim().to_string())
    }

    fn write_decision_blob(&self, file_path: &Path, blob: &DecisionBlob) -> Result<(), GitError> {
        let json = serde_json::to_string_pretty(blob)
            .map_err(|e| GitError::SerializationError(e.to_string()))?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)
            .map_err(|e| GitError::FileError(file_path.to_path_buf(), e.to_string()))?;

        file.write_all(json.as_bytes())
            .map_err(|e| GitError::FileError(file_path.to_path_buf(), e.to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionBlob {
    pub selected_reflection: Reflection,
    pub context_tags: Vec<MirrorTag>,
    pub kernel_name: String,
    pub reason: String,
    pub event_ids: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub commit_hash: String,
}

#[derive(Debug)]
pub struct DecisionTree {
    decisions: Vec<DecisionBlob>,
    by_commit: HashMap<String, DecisionBlob>,
    by_kernel: HashMap<String, Vec<DecisionBlob>>,
}

impl DecisionTree {
    pub fn new() -> Self {
        Self {
            decisions: Vec::new(),
            by_commit: HashMap::new(),
            by_kernel: HashMap::new(),
        }
    }
}

impl Default for DecisionTree {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionTree {
    pub fn add_decision(&mut self, decision: DecisionBlob) {
        self.decisions.push(decision.clone());
        self.by_commit
            .insert(decision.commit_hash.clone(), decision.clone());
        self.by_kernel
            .entry(decision.kernel_name.clone())
            .or_default()
            .push(decision);
    }

    pub fn get_decision_by_commit(&self, commit_hash: &str) -> Option<&DecisionBlob> {
        self.by_commit.get(commit_hash)
    }

    pub fn get_decisions_by_kernel(&self, kernel_name: &str) -> Vec<&DecisionBlob> {
        self.by_kernel
            .get(kernel_name)
            .map(|decisions| decisions.iter().collect())
            .unwrap_or_default()
    }

    pub fn get_all_decisions(&self) -> &[DecisionBlob] {
        &self.decisions
    }

    pub fn size(&self) -> usize {
        self.decisions.len()
    }
}

#[derive(Debug)]
pub enum GitError {
    FileError(PathBuf, String),
    SerializationError(String),
    GitCommandFailed(String, String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::FileError(path, msg) => {
                write!(f, "File error at {}: {}", path.display(), msg)
            }
            GitError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            GitError::GitCommandFailed(cmd, output) => {
                write!(f, "Git command '{}' failed: {}", cmd, output)
            }
        }
    }
}

impl std::error::Error for GitError {}

fn run_git<const N: usize>(repo_path: &Path, args: [&str; N]) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|e| GitError::GitCommandFailed(args[0].to_string(), e.to_string()))?;

    if !output.status.success() {
        return Err(GitError::GitCommandFailed(
            args[0].to_string(),
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
