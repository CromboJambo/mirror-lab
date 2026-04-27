use std::io::{self, BufRead};
use std::sync::Arc;

use rusqlite::{Connection, Result};

use crate::{chunk, embedding::EmbeddingService, log};

pub const AUTO_CHUNK_THRESHOLD: usize = 2000;
pub const DEFAULT_CHUNK_SIZE: usize = 1500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Capture,
    Persist,
    Structure,
    Enrich,
}

impl PipelineStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineStage::Capture => "capture",
            PipelineStage::Persist => "persist",
            PipelineStage::Structure => "structure",
            PipelineStage::Enrich => "enrich",
        }
    }
}

pub const CANONICAL_PIPELINE: [PipelineStage; 4] = [
    PipelineStage::Capture,
    PipelineStage::Persist,
    PipelineStage::Structure,
    PipelineStage::Enrich,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceLayer {
    Law,
    Principle,
    Right,
    Rule,
    Guideline,
}

impl GovernanceLayer {
    pub fn as_str(&self) -> &'static str {
        match self {
            GovernanceLayer::Law => "law",
            GovernanceLayer::Principle => "principle",
            GovernanceLayer::Right => "right",
            GovernanceLayer::Rule => "rule",
            GovernanceLayer::Guideline => "guideline",
        }
    }
}

pub const GOVERNANCE_ORDER: [GovernanceLayer; 5] = [
    GovernanceLayer::Law,
    GovernanceLayer::Principle,
    GovernanceLayer::Right,
    GovernanceLayer::Rule,
    GovernanceLayer::Guideline,
];

pub struct IngestRequest<'a> {
    pub source: &'a str,
    pub content: &'a str,
    pub meta: Option<&'a str>,
    pub chunk_threshold: usize,
    pub chunk_size: usize,
}

impl<'a> IngestRequest<'a> {
    pub fn new(source: &'a str, content: &'a str, meta: Option<&'a str>) -> Self {
        Self {
            source,
            content,
            meta,
            chunk_threshold: AUTO_CHUNK_THRESHOLD,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    pub fn with_chunking(mut self, threshold: usize, chunk_size: usize) -> Self {
        self.chunk_threshold = threshold;
        self.chunk_size = chunk_size;
        self
    }

    fn should_chunk(&self) -> bool {
        self.chunk_size > 0 && self.content.len() > self.chunk_threshold
    }
}

#[derive(Debug, Clone)]
pub struct IngestResult {
    pub event_id: String,
    pub timestamp: i64,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct IngestBatchResult {
    pub event_ids: Vec<String>,
    pub total_chunks: usize,
    pub chunked_events: usize,
}

/// Ingests a single event and performs the pipeline stages (Capture -> Persist -> Structure -> Enrich).
/// If an `EmbeddingService` is provided, it attempts the enrichment stage.
#[allow(unused_variables)]
pub fn ingest_single(
    conn: &Connection,
    service: Option<Arc<EmbeddingService>>,
    request: IngestRequest<'_>,
) -> Result<IngestResult> {
    let tx = conn.unchecked_transaction()?;
    let receipt = log::append_batch_with_receipts_in_tx(
        &tx,
        request.source,
        &[request.content],
        request.meta,
    )?
    .remove(0);

    let chunk_count = if request.should_chunk() {
        chunk::create_chunks(
            &tx,
            &receipt.id,
            request.content,
            receipt.timestamp,
            request.chunk_size,
        )?
    } else {
        0
    };

    // Enrichment: If service is enough and feature enabled, generate embeddings.
    #[cfg(feature = "embedding")]
    if let Some(svc) = service {
        let _ = svc.generate_embedding(request.content);
    }

    tx.commit()?;

    Ok(IngestResult {
        event_id: receipt.id,
        timestamp: receipt.timestamp,
        chunk_count,
    })
}

pub fn ingest_stdin(
    conn: &Connection,
    source: &str,
    meta: Option<&str>,
    batch_size: usize,
) -> io::Result<IngestBatchResult> {
    ingest_stdin_with_policy(
        conn,
        source,
        meta,
        batch_size,
        AUTO_CHUNK_THRESHOLD,
        DEFAULT_CHUNK_SIZE,
    )
}

pub fn ingest_stdin_with_policy(
    conn: &Connection,
    source: &str,
    meta: Option<&str>,
    batch_size: usize,
    chunk_threshold: usize,
    chunk_size: usize,
) -> io::Result<IngestBatchResult> {
    let stdin = io::stdin();
    let reader = stdin.lock();
    ingest_reader(
        conn,
        source,
        meta,
        reader,
        batch_size,
        chunk_threshold,
        chunk_size,
    )
}

pub fn ingest_reader<R: BufRead>(
    conn: &Connection,
    source: &str,
    meta: Option<&str>,
    reader: R,
    batch_size: usize,
    chunk_threshold: usize,
    chunk_size: usize,
) -> io::Result<IngestBatchResult> {
    let effective_batch_size = batch_size.max(1);
    let mut result = IngestBatchResult::default();
    let mut batch: Vec<String> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        batch.push(trimmed.to_string());

        if batch.len() >= effective_batch_size {
            flush_batch(
                conn,
                source,
                meta,
                &mut batch,
                chunk_threshold,
                chunk_size,
                &mut result,
            )?;
        }
    }

    if !batch.is_empty() {
        flush_batch(
            conn,
            source,
            meta,
            &mut batch,
            chunk_threshold,
            chunk_size,
            &mut result,
        )?;
    }

    Ok(result)
}

fn flush_batch(
    conn: &Connection,
    source: &str,
    meta: Option<&str>,
    batch: &mut Vec<String>,
    chunk_threshold: usize,
    chunk_size: usize,
    result: &mut IngestBatchResult,
) -> io::Result<()> {
    let tx = conn.unchecked_transaction().map_err(io::Error::other)?;
    let content_refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
    let receipts = log::append_batch_with_receipts_in_tx(&tx, source, &content_refs, meta)
        .map_err(io::Error::other)?;

    for (content, receipt) in batch.iter().zip(receipts.into_iter()) {
        let event_id = receipt.id;
        result.event_ids.push(event_id.clone());

        if chunk_size > 0 && content.len() > chunk_threshold {
            let chunk_count =
                chunk::create_chunks(&tx, &event_id, content, receipt.timestamp, chunk_size)
                    .map_err(io::Error::other)?;
            if chunk_count > 0 {
                result.chunked_events += 1;
                result.total_chunks += chunk_count;
            }
        }
    }

    tx.commit().map_err(io::Error::other)?;
    batch.clear();
    Ok(())
}
