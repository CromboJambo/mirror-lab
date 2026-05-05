use anyhow::Result;
use serde_json::Value;

use crate::execute_script;

/// Execute a skill script with default environment.
pub fn execute_default(
    script_path: &std::path::Path,
    args: &[String],
    work_dir: &std::path::Path,
) -> Result<Value> {
    let env = std::collections::HashMap::from_iter([
        ("HOME".to_string(), std::env::var("HOME").unwrap_or_default()),
        ("PWD".to_string(), std::env::var("PWD").unwrap_or_default()),
    ]);

    let allowlist = std::collections::HashSet::from_iter([script_path.to_owned()]);

    let timeout = std::time::Duration::from_secs(30);

    tokio::runtime::Runtime::new()?.block_on(execute_script(
        script_path,
        args,
        env,
        work_dir,
        timeout,
        &allowlist,
    ))
}

/// Execute multiple scripts in parallel.
pub async fn execute_parallel(scripts: &[(std::path::PathBuf, Vec<String>)]) -> Result<Vec<Value>> {
    let mut handles = Vec::new();

    #[allow(clippy::unnecessary_to_owned)]
    for (path, args) in scripts.iter().cloned() {
        let env = std::collections::HashMap::from_iter([
            (
                "HOME".to_string(),
                std::env::var("HOME").unwrap_or_default(),
            ),
            ("PWD".to_string(), std::env::var("PWD").unwrap_or_default()),
        ]);
        handles.push(tokio::spawn(async move {
            execute_script(&path, &args, env).await
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await?;
        results.push(result?);
    }

    Ok(results)
}
