use codeburn_provider::SessionData;
use serde::Serialize;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("classification failed: {0}")]
    ClassificationFailed(String),
}

#[derive(Debug, Clone, Serialize)]
pub enum TaskCategory {
    Edit,
    Test,
    Fix,
    Refactor,
    Design,
    Research,
    Documentation,
    Debugging,
    Architecture,
    Integration,
    Deployment,
    Review,
    Other,
}

impl fmt::Display for TaskCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskCategory::Edit => write!(f, "edit"),
            TaskCategory::Test => write!(f, "test"),
            TaskCategory::Fix => write!(f, "fix"),
            TaskCategory::Refactor => write!(f, "refactor"),
            TaskCategory::Design => write!(f, "design"),
            TaskCategory::Research => write!(f, "research"),
            TaskCategory::Documentation => write!(f, "documentation"),
            TaskCategory::Debugging => write!(f, "debugging"),
            TaskCategory::Architecture => write!(f, "architecture"),
            TaskCategory::Integration => write!(f, "integration"),
            TaskCategory::Deployment => write!(f, "deployment"),
            TaskCategory::Review => write!(f, "review"),
            TaskCategory::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug)]
pub struct TaskClassifier {
    rules: Vec<ClassificationRule>,
}

#[derive(Debug, Clone)]
pub struct ClassificationRule {
    pub pattern: String,
    pub category: TaskCategory,
}

impl Default for TaskClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskClassifier {
    pub fn new() -> Self {
        Self {
            rules: vec![
                ClassificationRule {
                    pattern: "test".to_string(),
                    category: TaskCategory::Test,
                },
                ClassificationRule {
                    pattern: "fix".to_string(),
                    category: TaskCategory::Fix,
                },
                ClassificationRule {
                    pattern: "refactor".to_string(),
                    category: TaskCategory::Refactor,
                },
                ClassificationRule {
                    pattern: "design".to_string(),
                    category: TaskCategory::Design,
                },
                ClassificationRule {
                    pattern: "docs".to_string(),
                    category: TaskCategory::Documentation,
                },
                ClassificationRule {
                    pattern: "debug".to_string(),
                    category: TaskCategory::Debugging,
                },
                ClassificationRule {
                    pattern: "arch".to_string(),
                    category: TaskCategory::Architecture,
                },
                ClassificationRule {
                    pattern: "deploy".to_string(),
                    category: TaskCategory::Deployment,
                },
                ClassificationRule {
                    pattern: "review".to_string(),
                    category: TaskCategory::Review,
                },
                ClassificationRule {
                    pattern: "edit".to_string(),
                    category: TaskCategory::Edit,
                },
                ClassificationRule {
                    pattern: "research".to_string(),
                    category: TaskCategory::Research,
                },
                ClassificationRule {
                    pattern: "integrate".to_string(),
                    category: TaskCategory::Integration,
                },
            ],
        }
    }

    pub fn classify(
        &self,
        sessions: &[codeburn_provider::SessionData],
    ) -> Result<Vec<SessionData>, Error> {
        let mut classified = Vec::new();

        for session in sessions {
            let category = self.find_category(session)?;
            classified.push(SessionData {
                provider: session.provider.clone(),
                date: session.date,
                input_tokens: session.input_tokens,
                output_tokens: session.output_tokens,
                model: session.model.clone(),
                task_category: category.to_string(),
                project: session.project.clone(),
                message_id: session.message_id.clone(),
            });
        }

        Ok(classified)
    }

    fn find_category(
        &self,
        session: &codeburn_provider::SessionData,
    ) -> Result<TaskCategory, Error> {
        for rule in &self.rules {
            if session.model.contains(&rule.pattern)
                || session
                    .project
                    .as_ref()
                    .map(|p| p.contains(&rule.pattern))
                    .unwrap_or(false)
            {
                return Ok(rule.category.clone());
            }
        }

        Ok(TaskCategory::Other)
    }
}
