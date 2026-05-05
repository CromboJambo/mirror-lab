use anyhow::Result;

use crate::discover_skills;
use crate::find_scripts;

/// Discover all skill directories and their bundled scripts.
pub fn discover_all(
    project_root: &std::path::Path,
    home_dir: &std::path::Path,
) -> Result<Vec<(std::path::PathBuf, Vec<std::path::PathBuf>)>> {
    let skill_dirs = discover_skills(project_root, home_dir)?;

    let mut results = Vec::new();
    for skill_dir in skill_dirs {
        let scripts = find_scripts(&skill_dir)?;
        results.push((skill_dir, scripts));
    }

    Ok(results)
}

/// Filter scripts by skill name.
pub fn filter_by_skill(
    discoveries: &[(std::path::PathBuf, Vec<std::path::PathBuf>)],
    skill_name: &str,
) -> Result<Vec<std::path::PathBuf>> {
    let mut found_scripts = Vec::new();
    for (skill_dir, scripts) in discoveries {
        if skill_dir.file_name().map(|n| n.to_string_lossy()) == Some(std::borrow::Cow::Borrowed(skill_name)) {
            found_scripts.extend(scripts.clone());
        }
    }

    Ok(found_scripts)
}
