//! Mirror-Storage: Abstract storage trait for database-agnostic operations.

pub mod adapters;

use std::fmt::Debug;
use thiserror::Error;

/// Core storage errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Conversion error: {0}")]
    Conversion(String),
}

// Implement From for rusqlite errors
impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        StorageError::Query(err.to_string())
    }
}

// Implement From for Mutex lock errors  
use std::sync::{Mutex, PoisonError};

impl From<PoisonError<MutexGuard<'_, Connection>>> for StorageError {
    fn from(_err: PoisonError<MutexGuard<'_, Connection>>) -> Self {
        StorageError::Connection("Lock poisoned".to_string())
    }
}

/// Core trait representing a database-agnostic storage layer.
pub trait Storage: Send + Sync {
    fn query(
        &self,
        sql: &str,
        params: impl IntoParams,
    ) -> Box<dyn Iterator<Item = Result<Vec<Value>, StorageError>> + '_>;

    fn execute(
        &self,
        sql: &str,
        params: impl IntoParams,
    ) -> Result<i64, StorageError>;

    fn begin_transaction(&self) -> Result<Box<dyn Transaction>, StorageError>;

    fn backend_type(&self) -> &'static str;

    fn table_exists(&self, name: &str) -> Result<bool, StorageError> {
        let result = self.execute(
            &format!("SELECT 1 FROM {} LIMIT 1", name),
            [],
        );
        Ok(result.is_ok())
    }

    fn table_schema(&self, name: &str) -> Result<TableSchema, StorageError>;
}

/// Trait marker for parameterized queries
pub trait IntoParams {
    fn as_slice(&self) -> &[Value];
}

impl<const N: usize> IntoParams for [Value; N] {
    fn as_slice(&self) -> &[Value] {
        self
    }
}

/// A single row value in a query result
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Text(String),
    Integer(i64),
    Float(f64),
    Blob(Vec<u8>),
    Boolean(bool),
}

impl Value {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Integer(v) = self { *v } else { None }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    pub fn integer(n: i64) -> Self {
        Self::Integer(n)
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Null
    }
}

/// Schema information for a table
#[derive(Debug)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    pub indexes: Vec<String>,
}

/// Column definition within a table
#[derive(Debug)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
}

/// Transaction handle for atomic operations
pub trait Transaction: Send + Sync {
    fn execute(&self, sql: &str) -> Result<i64, StorageError>;
    fn query(
        &self,
        sql: &str,
        params: &[Value],
    ) -> Result<Vec<Vec<Value>>, StorageError>;
    fn commit(&self) -> Result<(), StorageError>;
    fn rollback(&self) -> Result<(), StorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_conversions() {
        let text = Value::text("hello");
        assert_eq!(text.as_text(), Some("hello"));

        let int = Value::integer(42);
        assert_eq!(int.as_i64(), Some(42));

        let null = Value::Null;
        assert!(null.is_null());
    }

    #[test]
    fn test_into_params_array() {
        let params: [Value; 1] = [Value::text("test".to_string())];
        assert_eq!(params.as_slice().len(), 1);
    }

    #[test]
    fn test_value_default() {
        let default = Value::default();
        assert!(matches!(default, Value::Null));
    }
}
