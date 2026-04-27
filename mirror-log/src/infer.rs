use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::path::Path;

use crate::stage::StagedEvent;

#[derive(Debug, Clone)]
pub struct Pattern {
    pub description: String,
    pub source_events: Vec<String>, // list of event IDs that triggered this pattern
}

pub fn detect_patterns(staging_dir: &Path) -> Result<Vec<Pattern>, Box<dyn std::error::Error>> {
    let mut patterns = Vec::new();

    let events = StagedEvent::load_all(staging_dir)?;

    if events.is_empty() {
        return Ok(patterns);
    }

    let one_week_ago = Utc::now() - Duration::weeks(1);

    // Pattern 1: Frequent shell commands (nushell-history)
    let mut shell_command_counts: HashMap<String, (i32, Vec<String>)> = HashMap::new();
    for event in &events {
        if event.source == "nushell-history" && event.timestamp_utc() > one_week_ago {
            let entry = shell_command_counts
                .entry(event.content.clone())
                .or_insert((0, Vec::new()));
            entry.0 += 1;
            entry.1.push(event.id.clone());
        }
    }

    for (command, (count, source_ids)) in &shell_command_counts {
        if *count >= 3 {
            patterns.push(Pattern {
                description: format!(
                    "* You ran `{}` {} times in the last week — this suggests you rely on it for routine tasks.",
                    command, count
                ),
                source_events: source_ids.clone(),
            });
        }
    }

    // Pattern 2: Repeated dotfile edits (e.g., .config, .bashrc, .rustfmt.toml)
    let mut dotfile_edits: HashMap<String, (i32, Vec<String>)> = HashMap::new();
    for event in &events {
        if event.source.starts_with("dotfile") && event.timestamp_utc() > one_week_ago {
            let entry = dotfile_edits
                .entry(event.content.clone())
                .or_insert((0, Vec::new()));
            entry.0 += 1;
            entry.1.push(event.id.clone());
        }
    }

    for (content, (count, source_ids)) in &dotfile_edits {
        if *count >= 2 {
            patterns.push(Pattern {
                description: format!(
                    "* You edited a configuration file with content like \"{}\" {} times — this suggests iterative refinement of your workflow.",
                    content, count
                ),
                source_events: source_ids.clone(),
            });
        }
    }

    // Pattern 3: Sensitive content (e.g., passwords, keys)
    for event in &events {
        if event.content.contains("password")
            || event.content.contains("secret")
            || event.content.contains("key=")
        {
            patterns.push(Pattern {
                description: format!(
                    "* You entered sensitive data: \"{}\" — consider using a password manager.",
                    event.content
                ),
                source_events: vec![event.id.clone()],
            });
        }
    }

    Ok(patterns)
}
