use crate::knowledge_store::{KnowledgeBridge, knowledge_response};
use serde_json::json;

use crabjar_lib::KnowledgeCommand;

pub trait KnowledgeCommandExt {
    async fn execute(
        &self,
        bridge: &KnowledgeBridge<'_>,
    ) -> Result<serde_json::Value, agent_context::Error>;
}

impl KnowledgeCommandExt for KnowledgeCommand {
    async fn execute(
        &self,
        bridge: &KnowledgeBridge<'_>,
    ) -> Result<serde_json::Value, agent_context::Error> {
        match self {
            Self::Index { doc } => {
                let ids = bridge.sync_state_doc_annotations(doc)?;
                Ok(knowledge_response(
                    format!("synced annotations for {}", doc),
                    json!({ "doc": doc, "ids": ids }),
                ))
            }
        }
    }
}
