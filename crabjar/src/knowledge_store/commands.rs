use crate::knowledge_store::{KnowledgeBridge, knowledge_response};
use agent_context::{KnowledgeEntry, KnowledgeKind, Source};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::json;

/// CLI commands for knowledge store integration
#[derive(Debug, Subcommand, Clone)]
pub enum KnowledgeCommand {
    /// Insert a new knowledge entry
    Insert(KnowledgeInsertArgs),

    /// Query knowledge entries by tags
    Query(KnowledgeQueryArgs),

    /// Sync a state-doc's annotations to the knowledge store
    Sync(KnowledgeSyncArgs),

    /// Promote a raw event from mirror-log to a knowledge entry
    Promote(KnowledgePromoteArgs),

    /// List all synced state-docs
    #[command(name = "list-synced")]
    ListSyncedDocs,

    /// Get knowledge entries for a specific state-doc
    #[command(name = "get-state-doc")]
    GetStateDocKnowledge(KnowledgeStateDocArgs),

    /// Verify knowledge store integrity
    Verify,

    /// Get recent events
    Events(KnowledgeEventsArgs),

    /// Deactivate a knowledge entry
    Deactivate(KnowledgeDeactivateArgs),

    /// Resolve an annotation and deactivate its derived knowledge
    #[command(name = "resolve-annotation")]
    ResolveAnnotation(KnowledgeResolveAnnotationArgs),
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgeInsertArgs {
    #[arg(long)]
    content: String,

    #[arg(long, default_value = "instruction")]
    kind: String,

    #[arg(long, value_delimiter = ',', default_value = "")]
    tags: Vec<String>,

    #[arg(long)]
    meta: Option<String>,

    #[arg(long)]
    weight: Option<f64>,
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgeQueryArgs {
    #[arg(long, value_delimiter = ',', default_value = "")]
    tags: Vec<String>,

    #[arg(long, default_value_t = 10)]
    limit: usize,
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgeSyncArgs {
    doc_name: String,
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgePromoteArgs {
    /// The ID of the event in mirror-log to promote
    event_id: i64,
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgeStateDocArgs {
    doc_name: String,
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgeEventsArgs {
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgeDeactivateArgs {
    id: i64,

    #[arg(long)]
    reason: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct KnowledgeResolveAnnotationArgs {
    doc_name: String,

    #[arg(long)]
    annotation_id: String,

    #[arg(long)]
    reason: Option<String>,
}

impl KnowledgeCommand {
    /// Execute the command
    pub async fn execute(
        &self,
        bridge: &KnowledgeBridge<'_>,
    ) -> Result<serde_json::Value, agent_context::Error> {
        match self {
            Self::Insert(args) => {
                let kind = match args.kind.as_str() {
                    "context" => agent_context::KnowledgeKind::Context,
                    "pattern" => agent_context::KnowledgeKind::Pattern,
                    "example" => agent_context::KnowledgeKind::Example,
                    _ => agent_context::KnowledgeKind::Instruction,
                };
                let mut entry = KnowledgeEntry::new(&args.content, kind);
                entry.tags = args
                    .tags
                    .iter()
                    .filter(|tag| !tag.is_empty())
                    .cloned()
                    .collect();
                entry.source = Source::User;

                if let Some(weight) = args.weight {
                    entry = entry.weight(weight);
                }

                if let Some(meta) = &args.meta {
                    let meta_value: serde_json::Value =
                        serde_json::from_str(meta).map_err(agent_context::Error::Json)?;
                    entry = entry.meta("cli-meta", meta_value);
                }

                let id = bridge.knowledge_store.insert(entry)?;
                Ok(knowledge_response(
                    format!("inserted id={id}"),
                    json!({ "id": id }),
                ))
            }

            Self::Query(args) => {
                let tags: Vec<&str> = args
                    .tags
                    .iter()
                    .filter(|tag| !tag.is_empty())
                    .map(String::as_str)
                    .collect();
                let rows = bridge.query_state_docs(tags.as_slice(), args.limit)?;
                Ok(knowledge_response(
                    if rows.is_empty() {
                        "no results".to_string()
                    } else {
                        format!("returned {} knowledge rows", rows.len())
                    },
                    json!({ "rows": rows }),
                ))
            }

            Self::Sync(args) => {
                let ids = bridge.sync_state_doc_annotations(&args.doc_name)?;
                Ok(knowledge_response(
                    format!("synced {} annotations for {}", ids.len(), args.doc_name),
                    json!({
                        "doc": args.doc_name,
                        "ids": ids,
                    }),
                ))
            }

            Self::Promote(args) => {
                let new_id = bridge.promote_event(args.event_id)?;
                Ok(knowledge_response(
                    format!(
                        "promoted event id={} to knowledge entry id={}",
                        args.event_id, new_id
                    ),
                    json!({ "event_id": args.event_id, "new_id": new_id }),
                ))
            }

            Self::ListSyncedDocs => {
                let docs = bridge.list_synced_state_docs()?;
                Ok(knowledge_response(
                    if docs.is_empty() {
                        "no synced state-docs".to_string()
                    } else {
                        format!("listed {} synced state-docs", docs.len())
                    },
                    json!({ "docs": docs }),
                ))
            }

            Self::GetStateDocKnowledge(args) => {
                let rows = bridge.get_state_doc_knowledge(&args.doc_name)?;
                Ok(knowledge_response(
                    if rows.is_empty() {
                        format!("no knowledge entries for {}", args.doc_name)
                    } else {
                        format!(
                            "returned {} knowledge entries for {}",
                            rows.len(),
                            args.doc_name
                        )
                    },
                    json!({
                        "doc": args.doc_name,
                        "rows": rows,
                    }),
                ))
            }

            Self::Verify => {
                let bad = bridge.knowledge_store.verify()?;
                if bad.is_empty() {
                    return Ok(knowledge_response(
                        "all checksums ok",
                        json!({ "bad_ids": [] }),
                    ));
                }
                Err(agent_context::Error::ChecksumMismatch {
                    id: bad[0],
                    stored: "error".to_string(),
                    computed: "error".to_string(),
                })
            }

            Self::Events(args) => {
                let events = bridge.get_events(args.limit)?;
                Ok(knowledge_response(
                    format!("returned {} events", events.len()),
                    json!({ "events": events }),
                ))
            }

            Self::Deactivate(args) => {
                bridge
                    .knowledge_store
                    .deactivate(args.id, Source::User, args.reason.as_deref())?;
                Ok(knowledge_response(
                    format!("deactivated id={}", args.id),
                    json!({
                        "id": args.id,
                        "reason": args.reason,
                    }),
                ))
            }

            Self::ResolveAnnotation(args) => {
                let resolved = bridge
                    .state_docs
                    .resolve_annotation(&args.doc_name, &args.annotation_id)?;

                let deactivated = if let Some(entry) = &resolved {
                    bridge
                        .deactivate_resolved_annotation_knowledge(&entry, args.reason.as_deref())?
                } else {
                    0
                };

                Ok(knowledge_response(
                    format!(
                        "resolved annotation {} for {} (deactivated {} derived entries)",
                        args.annotation_id,
                        args.doc_name,
                        deactivated
                    ),
                    json!({
                        "doc": args.doc_name,
                        "annotation_id": args.annotation_id,
                        "resolved": resolved,
                        "deactivated": deactivated,
                        "reason": args.reason,
                    }),
                ))
            }
        }
    }
}
