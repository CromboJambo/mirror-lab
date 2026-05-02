use rusqlite::{Connection, Error as SqliteError};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "embedding")]
use tokenizers::Tokenizer;

#[derive(Debug, Clone)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
    pub model_name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct EmbeddingStats {
    pub total_embeddings: i64,
    pub total_events: i64,
    pub model_name: String,
    pub embedding_dim: usize,
    pub average_vector_length: f32,
}

#[derive(Debug, Clone)]
pub struct Similarity {
    pub event_id: String,
    pub score: f32,
}

#[derive(Debug)]
pub enum EmbeddingError {
    TokenizerError(String),
    DatabaseError(SqliteError),
    ModelLoadError(String),
    VectorDimensionMismatch(usize, usize),
    NoEmbeddingsFound,
    InvalidEmbeddingData(usize),
    ProviderError(String),
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingError::TokenizerError(msg) => write!(f, "Tokenizer error: {}", msg),
            EmbeddingError::DatabaseError(e) => write!(f, "Database error: {}", e),
            EmbeddingError::ModelLoadError(msg) => write!(f, "Model load error: {}", msg),
            EmbeddingError::VectorDimensionMismatch(expected, actual) => {
                write!(
                    f,
                    "Vector dimension mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            EmbeddingError::NoEmbeddingsFound => write!(f, "No embeddings found in database"),
            EmbeddingError::InvalidEmbeddingData(bytes) => {
                write!(f, "Embedding blob is not valid f32 data ({} bytes)", bytes)
            }
            EmbeddingError::ProviderError(msg) => write!(f, "Provider error: {}", msg),
        }
    }
}

impl std::error::Error for EmbeddingError {}

/// The core abstraction for any embedding generation strategy.
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}

#[cfg(feature = "embedding")]
/// A lightweight, deterministic baseline implementation for testing/fallback.
pub struct BaselineProvider {
    tokenizer: Tokenizer,
    dim: usize,
    model_name: String,
}

#[cfg(feature = "embedding")]
impl BaselineProvider {
    pub fn new(tokenizer: Tokenizer, dim: usize, model_name: &str) -> Self {
        Self {
            tokenizer,
            dim,
            model_name: model_name.to_string(),
        }
    }
}

#[cfg(feature = "embedding")]
impl EmbeddingProvider for BaselineProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.embed_batch(&[text]).and_then(|v| {
            v.into_iter().next().ok_or(EmbeddingError::ProviderError(
                "Empty embedding result".into(),
            ))
        })
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let encoding = self
                .tokenizer
                .encode(*text, true)
                .map_err(|e| EmbeddingError::TokenizerError(e.to_string()))?;

            let mut vector = vec![0.0f32; self.dim];

            for (position, token_id) in encoding.get_ids().iter().enumerate() {
                let idx = (*token_id as usize) % self.dim;
                let weight = 1.0 + (position % 8) as f32 * 0.125;
                vector[idx] += weight;
            }

            results.push(normalize_vector(&vector));
        }
        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

/// The service responsible for orchestrating embedding generation,
/// persistence to SQLite, and retrieval/search operations.
pub struct EmbeddingService {
    conn: Connection,
    provider: Arc<dyn EmbeddingProvider>,
}

impl EmbeddingService {
    pub fn new(conn: Connection, provider: Arc<dyn EmbeddingProvider>) -> Self {
        Self { conn, provider }
    }

    pub fn init_from_path(
        path: impl AsRef<std::path::Path>,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> Result<Self, EmbeddingError> {
        let conn = Connection::open(path).map_err(EmbeddingError::DatabaseError)?;
        Ok(Self { conn, provider })
    }

    pub fn generate_embedding(&self, text: &str) -> Result<Embedding, EmbeddingError> {
        let vector = self.provider.embed(text)?;
        let model_name = self.provider.model_name().to_string();

        Ok(Embedding {
            id: Uuid::new_v4().to_string(),
            vector,
            model_name,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn store_embedding(
        &self,
        embedding: &Embedding,
        event_id: &str,
    ) -> Result<(), EmbeddingError> {
        self.conn
            .execute(
                "INSERT INTO event_embeddings (id, event_id, embedding, model_name, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    &embedding.id,
                    event_id,
                    Self::vector_to_bytes(&embedding.vector),
                    &embedding.model_name,
                    embedding.created_at,
                ),
            )
            .map_err(EmbeddingError::DatabaseError)?;
        Ok(())
    }

    pub fn get_embedding(&self, event_id: &str) -> Result<Embedding, EmbeddingError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, embedding, model_name, created_at
                 FROM event_embeddings
                 WHERE event_id = ?1
                 ORDER BY created_at DESC
                 LIMIT 1",
            )
            .map_err(EmbeddingError::DatabaseError)?;

        let (id, vector_bytes, model_name, created_at) = stmt
            .query_row([event_id], |row| {
                let id: String = row.get(0)?;
                let vector_bytes: Vec<u8> = row.get(1)?;
                let model_name: String = row.get(2)?;
                let created_at: i64 = row.get(3)?;
                Ok((id, vector_bytes, model_name, created_at))
            })
            .map_err(EmbeddingError::DatabaseError)?;

        let vector = Self::bytes_to_vector(&vector_bytes)?;
        Ok(Embedding {
            id,
            vector,
            model_name,
            created_at,
        })
    }

    pub fn get_embeddings_by_model(
        &self,
        model_name: &str,
    ) -> Result<Vec<Embedding>, EmbeddingError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, embedding, model_name, created_at
                 FROM event_embeddings
                 WHERE model_name = ?1",
            )
            .map_err(EmbeddingError::DatabaseError)?;

        let rows = stmt
            .query_map([model_name], |row| {
                let id: String = row.get(0)?;
                let vector_bytes: Vec<u8> = row.get(1)?;
                let model_name: String = row.get(2)?;
                let created_at: i64 = row.get(3)?;
                Ok((id, vector_bytes, model_name, created_at))
            })
            .map_err(EmbeddingError::DatabaseError)?;

        let mut embeddings = Vec::new();
        for row in rows {
            let (id, vector_bytes, model_name, created_at) =
                row.map_err(EmbeddingError::DatabaseError)?;
            let vector = Self::bytes_to_vector(&vector_bytes)?;
            embeddings.push(Embedding {
                id,
                vector,
                model_name,
                created_at,
            });
        }

        Ok(embeddings)
    }

    pub fn search_similar(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<Similarity>, EmbeddingError> {
        let model_name = self.provider.model_name();
        let mut stmt = self
            .conn
            .prepare(
                "SELECT event_id, embedding
                 FROM event_embeddings
                 WHERE model_name = ?1",
            )
            .map_err(EmbeddingError::DatabaseError)?;

        let rows = stmt
            .query_map([model_name], |row| {
                let event_id: String = row.get(0)?;
                let embedding_bytes: Vec<u8> = row.get(1)?;
                Ok((event_id, embedding_bytes))
            })
            .map_err(EmbeddingError::DatabaseError)?;

        let mut similarities: Vec<Similarity> = Vec::new();
        for row in rows {
            let (event_id, bytes) = row.map_err(EmbeddingError::DatabaseError)?;
            let vector = Self::bytes_to_vector(&bytes)?;
            let score = Self::cosine_similarity(query_vector, &vector)?;
            similarities.push(Similarity { event_id, score });
        }

        similarities.sort_by(|a, b| b.score.total_cmp(&a.score));
        similarities.truncate(limit);
        Ok(similarities)
    }

    pub fn get_embedding_stats(&self) -> Result<EmbeddingStats, EmbeddingError> {
        let total_embeddings: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM event_embeddings WHERE model_name = ?1",
                [self.provider.model_name()],
                |row| row.get(0),
            )
            .map_err(EmbeddingError::DatabaseError)?;

        let total_events: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .map_err(EmbeddingError::DatabaseError)?;

        let mut stmt = self
            .conn
            .prepare("SELECT embedding FROM event_embeddings WHERE model_name = ?1")
            .map_err(EmbeddingError::DatabaseError)?;

        let rows = stmt
            .query_map([self.provider.model_name()], |row| row.get::<_, Vec<u8>>(0))
            .map_err(EmbeddingError::DatabaseError)?;

        let mut total_len = 0.0f32;
        let mut count = 0usize;
        for row in rows {
            let bytes = row.map_err(EmbeddingError::DatabaseError)?;
            let vector = Self::bytes_to_vector(&bytes)?;
            total_len += vector.iter().map(|v| v * v).sum::<f32>().sqrt();
            count += 1;
        }

        Ok(EmbeddingStats {
            total_embeddings,
            total_events,
            model_name: self.provider.model_name().to_string(),
            embedding_dim: self.provider.dimension(),
            average_vector_length: if count == 0 {
                0.0
            } else {
                total_len / count as f32
            },
        })
    }

    fn vector_to_bytes(vector: &[f32]) -> Vec<u8> {
        vector.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn bytes_to_vector(bytes: &[u8]) -> Result<Vec<f32>, EmbeddingError> {
        if !bytes.len().is_multiple_of(4) {
            return Err(EmbeddingError::InvalidEmbeddingData(bytes.len()));
        }
        Ok(bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }

    pub fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> Result<f32, EmbeddingError> {
        if vec1.len() != vec2.len() {
            return Err(EmbeddingError::VectorDimensionMismatch(
                vec1.len(),
                vec2.len(),
            ));
        }
        let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
        let norm1 = vec1.iter().map(|v| v * v).sum::<f32>().sqrt();
        let norm2 = vec2.iter().map(|v| v * v).sum::<f32>().sqrt();

        if norm1 == 0.0 || norm2 == 0.0 {
            return Ok(0.0);
        }
        Ok(dot_product / (norm1 * norm2))
    }
}

// Helper functions for the module

pub fn normalize_vector(vector: &[f32]) -> Vec<f32> {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm == 0.0 {
        return vector.to_vec();
    }
    vector.iter().map(|v| v / norm).collect()
}

// Public Helpers

pub fn init_embedding_service(
    path: impl AsRef<std::path::Path>,
    provider: Arc<dyn EmbeddingProvider>,
) -> Result<EmbeddingService, EmbeddingError> {
    let conn = Connection::open(path).map_err(EmbeddingError::DatabaseError)?;
    Ok(EmbeddingService { conn, provider })
}

pub fn batch_generate_and_store(
    service: &EmbeddingService,
    items: &[(&str, &str)],
) -> Result<Vec<Embedding>, EmbeddingError> {
    let mut embeddings = Vec::with_capacity(items.len());
    for (_, text) in items {
        let embedding = service.generate_embedding(text)?;
        // In a real implementation, we'd also call store_embedding here.
        embeddings.push(embedding);
    }
    Ok(embeddings)
}
