use anyhow::Result;

use crate::ReferenceStoreError;
use crate::check_staleness;
use crate::retrieve_reference;

/// Retrieve all references, checking staleness for each.
pub fn retrieve_all(
    indexed: &[serde_json::Value],
    staleness_days: u64,
) -> Result<Vec<(std::path::PathBuf, String)>> {
    let mut retrieved = Vec::new();

    for entry in indexed {
        let path = entry["path"].as_str().unwrap_or_default();
        let path_buf = std::path::PathBuf::from(path);

        match retrieve_reference(&path_buf, staleness_days) {
            Ok(content) => retrieved.push((path_buf, content)),
            Err(ReferenceStoreError::Stale { .. }) => {
                // Skip stale references, flag for update
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(retrieved)
}

/// Flag stale references for update.
pub fn flag_stale(
    indexed: &[serde_json::Value],
    staleness_days: u64,
) -> Result<Vec<serde_json::Value>> {
    let mut stale = Vec::new();

    for entry in indexed {
        let path = entry["path"].as_str().unwrap_or_default();
        let path_buf = std::path::PathBuf::from(path);

        #[allow(clippy::collapsible_if)]
        if let Ok(is_stale) = check_staleness(&path_buf, staleness_days) {
            if is_stale {
                stale.push(serde_json::json! {
                    {
                        "path": path,
                        "skill_name": entry["skill_name"],
                        "type": entry["type"],
                        "action": "update"
                    }
                });
            }
        }
    }

    Ok(stale)
}
