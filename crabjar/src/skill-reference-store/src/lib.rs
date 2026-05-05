pub mod index;
pub mod staleness;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReferenceStoreError {
    #[error("file not found: {path}")]
    FileNotFound { path: std::path::PathBuf },
    #[error("index corrupted: {reason}")]
    IndexCorrupted { reason: String },
    #[error("staleness threshold exceeded: {path} stale after {days} days")]
    Stale { path: std::path::PathBuf, days: u64 },
}

impl From<std::io::Error> for ReferenceStoreError {
    fn from(e: std::io::Error) -> Self {
        ReferenceStoreError::IndexCorrupted {
            reason: e.to_string(),
        }
    }
}

/// Index a skill reference file into the store.
pub fn index_reference(
    path: &std::path::Path,
    skill_name: &str,
) -> Result<serde_json::Value, ReferenceStoreError> {
    if !path.exists() {
        return Err(ReferenceStoreError::FileNotFound {
            path: path.to_path_buf(),
        });
    }

    let metadata = std::fs::metadata(path)?;
    let _modified = metadata.modified()?;
    let line_count = count_lines(path)?;

    Ok(serde_json::json! {
        {
            "path": path.to_string_lossy(),
            "skill_name": skill_name,
            "modified_at": chrono::Utc::now().to_rfc3339(),
            "line_count": line_count,
            "staleness_threshold": 7,
            "stale_after_days": 7
        }
    })
}

/// Retrieve a reference file, checking staleness first.
pub fn retrieve_reference(
    path: &std::path::Path,
    staleness_days: u64,
) -> Result<String, ReferenceStoreError> {
    if !path.exists() {
        return Err(ReferenceStoreError::FileNotFound {
            path: path.to_path_buf(),
        });
    }

    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    let age = chrono::Utc::now() - chrono::DateTime::<chrono::Utc>::from(modified);

    if age.num_days() > staleness_days as i64 {
        return Err(ReferenceStoreError::Stale {
            path: path.to_path_buf(),
            days: staleness_days,
        });
    }

    std::fs::read_to_string(path).map_err(|e| ReferenceStoreError::IndexCorrupted {
        reason: e.to_string(),
    })
}

/// Check staleness of a reference file.
pub fn check_staleness(
    path: &std::path::Path,
    threshold: u64,
) -> Result<bool, ReferenceStoreError> {
    if !path.exists() {
        return Err(ReferenceStoreError::FileNotFound {
            path: path.to_path_buf(),
        });
    }

    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    let age = chrono::Utc::now() - chrono::DateTime::<chrono::Utc>::from(modified);

    Ok(age.num_days() > threshold as i64)
}

fn count_lines(path: &std::path::Path) -> Result<u64, ReferenceStoreError> {
    let content = std::fs::read_to_string(path)?;
    Ok(content.lines().count() as u64)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    #[test]
    fn index_reference_works_for_existing_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.md");
        std::fs::write(&file, "test content\n").unwrap();

        let result = crate::index_reference(&file, "test-skill").unwrap();
        assert_eq!(result["line_count"], 1);
    }

    #[test]
    fn index_reference_errors_for_missing_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("missing.md");

        let result = crate::index_reference(&file, "test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn retrieve_reference_works_for_non_stale_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.md");
        std::fs::write(&file, "test content\n").unwrap();

        let result = crate::retrieve_reference(&file, 7).unwrap();
        assert_eq!(result, "test content\n");
    }

    #[test]
    fn check_staleness_returns_false_for_new_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.md");
        std::fs::write(&file, "test content\n").unwrap();

        let result = crate::check_staleness(&file, 7).unwrap();
        assert!(!result);
    }
}
