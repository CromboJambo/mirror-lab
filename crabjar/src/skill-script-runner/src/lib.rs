pub mod discovery;
pub mod execution;

use anyhow::Result;

/// Discover skill directories in project-level and user-level scan paths.
pub fn discover_skills(
    project_root: &std::path::Path,
    home_dir: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>> {
    let mut found = Vec::new();

    // Project-level paths
    for ancestor in project_root.ancestors() {
        let candidate = ancestor.join(".agents/skills");
        if candidate.is_dir() {
            for entry in std::fs::read_dir(&candidate)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() && path.join("SKILL.md").exists() {
                    found.push(path);
                }
            }
        }
    }

    // User-level paths
    for scope in [".corust-agent/skills", ".agents/skills"] {
        let candidate = home_dir.join(scope);
        if candidate.is_dir() {
            for entry in std::fs::read_dir(&candidate)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() && path.join("SKILL.md").exists() {
                    found.push(path);
                }
            }
        }
    }

    Ok(found)
}

/// Find bundled scripts in a skill directory.
pub fn find_scripts(skill_dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut scripts = Vec::new();
    for entry in std::fs::read_dir(&scripts_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            scripts.push(path);
        }
    }

    Ok(scripts)
}

/// Execute a skill script with configurable environment.
pub async fn execute_script(
    script_path: &std::path::Path,
    args: &[String],
    env: std::collections::HashMap<String, String>,
) -> Result<serde_json::Value> {
    let output = tokio::process::Command::new(script_path)
        .args(args)
        .envs(&env)
        .output()
        .await?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        tracing::error!("script execution failed: {}", error);
        anyhow::bail!("script execution failed: {}", error);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)?;

    Ok(json)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    #[test]
    fn discover_skills_finds_valid_skill_dirs() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join(".agents/skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        let skill_dir = skills_dir.join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: |\n  test\n---\n",
        )
        .unwrap();

        let found = discover_skills(dir.path(), dir.path()).unwrap();
        assert!(found.contains(&skill_dir));
    }

    #[test]
    fn find_scripts_returns_empty_when_no_scripts_dir() {
        let dir = tempdir().unwrap();
        let scripts = find_scripts(dir.path()).unwrap();
        assert!(scripts.is_empty());
    }

    #[test]
    fn find_scripts_returns_scripts_when_dir_exists() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join("scripts");
        std::fs::create_dir_all(&scripts_dir).unwrap();
        std::fs::write(
            scripts_dir.join("test.sh"),
            "#!/usr/bin/env bash\necho test\n",
        )
        .unwrap();

        let scripts = find_scripts(dir.path()).unwrap();
        assert_eq!(scripts.len(), 1);
    }
}
