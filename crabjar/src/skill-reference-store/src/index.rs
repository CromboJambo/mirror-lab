use anyhow::Result;

use crate::index_reference;


/// Index a skill reference file into the store.
pub fn index_all(
    skill_dir: &std::path::Path,
) -> Result<Vec<serde_json::Value>> {
    let skill_name = skill_dir
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();

    let mut indexed = Vec::new();

    // Index references
    let refs_dir = skill_dir.join("references");
    if refs_dir.is_dir() {
        for entry in std::fs::read_dir(&refs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let index = index_reference(&path, &skill_name)?;
                indexed.push(index);
            }
        }
    }

    // Index scripts
    let scripts_dir = skill_dir.join("scripts");
    if scripts_dir.is_dir() {
        for entry in std::fs::read_dir(&scripts_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let line_count = std::fs::read_to_string(&path)?.lines().count() as u64;
                indexed.push(serde_json::json! {
                    {
                        "path": path.to_string_lossy(),
                        "skill_name": skill_name,
                        "type": "script",
                        "line_count": line_count
                    }
                });
            }
        }
    }

    Ok(indexed)
}
