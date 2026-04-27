use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const STATE_DOCS_DIR: &str = "state-docs";
const OVERLAY_DIR: &str = "overlay";
static ANNOTATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A trait for emitting events whenever an annotation is created or resolved.
/// This allows crabjar to bridge its local state changes into the mirror-log event stream.
pub trait AnnotationEventEmitter {
    fn emit_annotation_created(&self, entry: &AnnotationEntry);
    fn emit_annotation_resolved(&self, entry: &AnnotationEntry);
}

pub struct StateDocsManager<'a> {
    root: PathBuf,
    emitter: Option<&'a dyn AnnotationEventEmitter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    Note,
    Question,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationStatus {
    Open,
    Resolved,
}

impl std::fmt::Display for AnnotationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnnotationStatus::Open => write!(f, "open"),
            AnnotationStatus::Resolved => write!(f, "resolved"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnnotationEntry {
    pub id: String,
    pub kind: AnnotationKind,
    pub message: String,
    pub author: String,
    pub doc: String,
    pub line: Option<usize>,
    pub status: AnnotationStatus,
    pub created_at_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AnnotationOverlay {
    pub entries: Vec<AnnotationEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StateDocSummary {
    pub doc: String,
    pub path: String,
    pub open_notes: usize,
    pub open_questions: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StateDocView {
    pub doc: String,
    pub path: String,
    pub content: String,
    pub overlay: AnnotationOverlay,
}

impl<'a> Clone for StateDocsManager<'a> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            emitter: self.emitter,
        }
    }
}

impl<'a> StateDocsManager<'a> {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            root: project_root.into(),
            emitter: None,
        }
    }

    pub fn with_emitter(mut self, emitter: &'a dyn AnnotationEventEmitter) -> Self {
        self.emitter = Some(emitter);
        self
    }

    pub fn project_root(&self) -> &Path {
        &self.root
    }

    pub fn list_docs(&self) -> io::Result<Vec<StateDocSummary>> {
        let mut docs = Vec::new();
        let docs_dir = self.docs_dir();

        if !docs_dir.exists() {
            return Ok(docs);
        }

        for entry in fs::read_dir(&docs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if file_name == "README.md" {
                continue;
            }

            let overlay = self.load_overlay_for_path(&path)?;
            let open_notes = overlay
                .entries
                .iter()
                .filter(|entry| {
                    entry.status == AnnotationStatus::Open && entry.kind == AnnotationKind::Note
                })
                .count();
            let open_questions = overlay
                .entries
                .iter()
                .filter(|entry| {
                    entry.status == AnnotationStatus::Open && entry.kind == AnnotationKind::Question
                })
                .count();

            docs.push(StateDocSummary {
                doc: file_name.to_string(),
                path: relative_display_path(&self.root, &path),
                open_notes,
                open_questions,
            });
        }

        docs.sort_by(|left, right| left.doc.cmp(&right.doc));
        Ok(docs)
    }

    pub fn show_doc(&self, doc: &str) -> io::Result<StateDocView> {
        let path = self.resolve_doc_path(doc)?;
        let content = fs::read_to_string(&path)?;
        let overlay = self.load_overlay_for_path(&path)?;

        Ok(StateDocView {
            doc: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(doc)
                .to_string(),
            path: relative_display_path(&self.root, &path),
            content,
            overlay,
        })
    }

    pub fn add_annotation(
        &self,
        doc: &str,
        kind: AnnotationKind,
        message: &str,
        author: &str,
        line: Option<usize>,
    ) -> io::Result<AnnotationEntry> {
        let path = self.resolve_doc_path(doc)?;
        let mut overlay = self.load_overlay_for_path(&path)?;
        let doc_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(doc)
            .to_string();

        let message = message.trim();
        if message.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "annotation message cannot be empty",
            ));
        }

        let author = author.trim();
        if author.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "annotation author cannot be empty",
            ));
        }

        if matches!(line, Some(0)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "annotation line must be greater than zero",
            ));
        }

        let created_at_unix_ms = now_unix_ms();
        let entry = AnnotationEntry {
            id: next_annotation_id(&doc_name, created_at_unix_ms),
            kind,
            message: message.to_string(),
            author: author.to_string(),
            doc: doc_name.clone(),
            line,
            status: AnnotationStatus::Open,
            created_at_unix_ms,
        };

        overlay.entries.push(entry.clone());
        self.write_overlay(&doc_name, &overlay)?;

        if let Some(emitter) = self.emitter {
            emitter.emit_annotation_created(&entry);
        }

        Ok(entry)
    }

    pub fn resolve_annotation(&self, doc: &str, id: &str) -> io::Result<Option<AnnotationEntry>> {
        let path = self.resolve_doc_path(doc)?;
        let doc_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(doc)
            .to_string();
        let mut overlay = self.load_overlay_for_path(&path)?;

        for entry in &mut overlay.entries {
            if entry.id == id {
                entry.status = AnnotationStatus::Resolved;
                let resolved = entry.clone();
                self.write_overlay(&doc_name, &overlay)?;

                if let Some(emitter) = self.emitter {
                    emitter.emit_annotation_resolved(&resolved);
                }

                return Ok(Some(resolved));
            }
        }

        Ok(None)
    }

    fn docs_dir(&self) -> PathBuf {
        self.root.join(STATE_DOCS_DIR)
    }

    fn overlay_dir(&self) -> PathBuf {
        self.docs_dir().join(OVERLAY_DIR)
    }

    pub(crate) fn resolve_doc_path(&self, doc: &str) -> io::Result<PathBuf> {
        let requested = doc.trim();
        if requested.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "state doc name cannot be empty",
            ));
        }

        let docs = self.list_docs()?;
        for summary in docs {
            if summary.doc == requested || summary.doc.trim_end_matches(".md") == requested {
                return Ok(self.docs_dir().join(summary.doc));
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("state doc not found: {requested}"),
        ))
    }

    pub(crate) fn load_overlay_for_path(&self, doc_path: &Path) -> io::Result<AnnotationOverlay> {
        let doc_name = doc_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let overlay_path = self.overlay_path(doc_name);

        if !overlay_path.exists() {
            return Ok(AnnotationOverlay::default());
        }

        let content = fs::read_to_string(&overlay_path)?;
        serde_json::from_str(&content).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse overlay {}: {err}", overlay_path.display()),
            )
        })
    }

    fn write_overlay(&self, doc_name: &str, overlay: &AnnotationOverlay) -> io::Result<()> {
        let overlay_dir = self.overlay_dir();
        fs::create_dir_all(&overlay_dir)?;
        let overlay_path = self.overlay_path(doc_name);
        let json = serde_json::to_string_pretty(overlay).map_err(io::Error::other)?;
        fs::write(overlay_path, json)?;
        Ok(())
    }

    fn overlay_path(&self, doc_name: &str) -> PathBuf {
        let stem = doc_name.trim_end_matches(".md");
        self.overlay_dir().join(format!("{stem}.overlay.json"))
    }
}

fn relative_display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn next_annotation_id(doc_name: &str, created_at_unix_ms: u128) -> String {
    let sequence = ANNOTATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", slugify(doc_name), created_at_unix_ms, sequence)
}

fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }

    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn empty_doc_name_returns_error() {
        let dir = tempdir().unwrap();
        let manager = StateDocsManager::new(dir.path());
        let err = manager.show_doc("").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn show_nonexistent_doc_returns_not_found() {
        let dir = tempdir().unwrap();
        let docs_dir = dir.path().join(STATE_DOCS_DIR);
        fs::create_dir_all(&docs_dir).unwrap();

        let manager = StateDocsManager::new(dir.path());
        let err = manager.show_doc("ghost.md").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn resolve_nonexistent_annotation_returns_none() {
        let dir = tempdir().unwrap();
        let docs_dir = dir.path().join(STATE_DOCS_DIR);
        fs::create_dir_all(&docs_dir).unwrap();
        fs::write(docs_dir.join("alpha.md"), "# Alpha\n").unwrap();

        let manager = StateDocsManager::new(dir.path());
        let resolved = manager
            .resolve_annotation("alpha.md", "missing-annotation")
            .unwrap();

        assert_eq!(resolved, None);
    }
}
