//! Concrete storage adapters implementing the `Storage` trait.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, params, Row};

use crate::{Storage, Transaction as StorageTransaction, IntoParams, Value, StorageError, TableSchema, ColumnSchema};

/// Rusqlite-based implementation of the `Storage` trait.
/// 
/// This adapter wraps a rusqlite `Connection` and exposes it through the abstracted
/// `Storage` interface defined in `mirror-storage`. It serves as the reference
/// implementation before adding Turso/libSQL support.
pub struct RusqliteAdapter {
    conn: Mutex<Connection>,
}

impl RusqliteAdapter {
    /// Create a new adapter from an existing rusqlite Connection.
    pub fn from_connection(conn: Connection) -> Self {
        Self { 
            conn: Mutex::new(conn),
        }
    }

    /// Open or create a database at the given path.
    /// Open or create a database at the given path.
    /// Applies performance optimizations (WAL mode, etc.).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(&path)
            .map_err(|e| StorageError::Connection(e.to_string()))?;

        // Apply performance optimizations
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;\n             PRAGMA synchronous = NORMAL;\n             PRAGMA temp_store = MEMORY"
        )
        .map_err(|e| StorageError::Connection(e.to_string()))?;

        Ok(Self { 
            conn: Mutex::new(conn),
        })
    }

    /// Derive guard DB path from mirror DB path.
    pub fn derive_guard_path(mirror_path: &Path) -> PathBuf {
        let mut guard_path = mirror_path.parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        guard_path.push("guard.db");
        guard_path
    }

    /// Open guard DB co-located with mirror DB.
    pub fn open_guard_from_mirror(mirror_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let guard_path = Self::derive_guard_path(mirror_path.as_ref());
        Self::open(&guard_path)
    }
}

impl Storage for RusqliteAdapter {
    fn query<'a>(
        &'a self,
        sql: &str,
        params: impl IntoParams + 'a,
    ) -> Box<dyn Iterator<Item = Result<Vec<Value>, StorageError>> + Send> 
    where
        Self: 'a,
    {
        let conn = self.conn.lock().unwrap();
        
        // Convert parameters to rusqlite format
        let param_slice = params.as_slice();
        let mut stmt = conn.prepare(sql)
            .map_err(|e| StorageError::Query(e.to_string()))?;

        Box::new(
            stmt.query_map(param_slice, move |row| {
                row_to_values(row).map_err(|e| StorageError::Query(e.to_string()))
            })
            .map_err(|e| StorageError::Query(e.to_string()))
            .unwrap_or_else(|| vec![].into_iter())
            .filter_map(Result::ok), // Filter out any errors during mapping
        )
    }

    fn execute(&self, sql: &str, params: impl IntoParams) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        
        let param_slice = params.as_slice();
        let rows_affected = conn.execute(sql, param_slice)
            .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(rows_affected as i64)
    }

    fn begin_transaction(&self) -> Result<Box<dyn StorageTransaction>, StorageError> {
        let conn = self.conn.lock().unwrap();
        
        // Execute BEGIN TRANSACTION
        conn.execute("BEGIN IMMEDIATE", [])
            .map_err(|e| StorageError::Transaction(e.to_string()))?;

        Ok(Box::new(RusqliteTransaction { 
            conn: std::sync::Arc::new(Mutex::new(conn)),
            committed: false,
        }))
    }

    fn backend_type(&self) -> &'static str {
        "rusqlite"
    }

    fn table_schema(&self, name: &str) -> Result<TableSchema, StorageError> {
        use std::collections::HashMap;
        
        let conn = self.conn.lock().unwrap();
        
        // Get column info using PRAGMA table_info
        let mut columns = Vec::new();
        for row in conn.pragma_query_table(name, |row| {
            Ok((
                row.get::<_, String>(1)?,   // name
                row.get::<_, String>(2)?,   // type
                row.get::<_, i32>(4)? == 0, // notnull
                row.get::<_, i32>(5)? != 0, // pk
            ))
        })? {
            let (name, data_type, nullable, is_primary) = row?;
            
            columns.push(ColumnSchema {
                name,
                data_type,
                nullable,
                is_primary_key: is_primary,
                default_value: None, // Would need PRAGMA index_list for defaults
            });
        }

        let schema = TableSchema {
            name: name.to_string(),
            columns,
            indexes: Vec::new(), // TODO: PRAGMA index_list(name)
        };
        
        Ok(schema)
    }
}

/// Transaction wrapper for rusqlite connection.
pub struct RusqliteTransaction {
    conn: std::sync::Arc<Mutex<Connection>>,
    committed: bool,
}

impl StorageTransaction for RusqliteTransaction {
    fn execute(&self, sql: &str) -> Result<i64, StorageError> {
        let mut conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(sql, [])?;
        Ok(rows_affected as i64)
    }

    fn query(
        &self,
        sql: &str,
        _params: &[Value],
    ) -> Result<Vec<Vec<Value>>, StorageError> {
        let conn = self.conn.lock().unwrap();
        
        // Note: params not yet implemented for transactions
        // TODO: Add parameter binding support
        
        let mut stmt = conn.prepare(sql)
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            row_to_values(row).map_err(|e| e.to_string().into())
        })?;

        Ok(rows.filter_map(Result::ok).collect())
    }

    fn commit(&self) -> Result<(), StorageError> {
        let mut conn = self.conn.lock().unwrap();
        conn.execute("COMMIT", [])?;
        self.committed = true;
        Ok(())
    }

    fn rollback(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("ROLLBACK", [])?;
        Ok(())
    }
}

/// Convert a rusqlite Row to Vec<Value>.
fn row_to_values(row: &Row<'_>) -> Result<Vec<Value>, String> {
    let count = row.as_ref().len() as u8;
    
    (0..count)
        .map(|i| {
            match row.get_ref(i).unwrap_or(rusqlite::types::ValueRef::Null) {
                rusqlite::types::ValueRef::Null => Ok(Value::Null),
                rusqlite::types::ValueRef::Integer(n) => Ok(Value::integer(*n)),
                rusqlite::types::ValueRef::Real(f) => Ok(Value::Float(*f)),
                rusqlite::types::ValueRef::Blob(b) => {
                    let b = b.to_vec();
                    Ok(Value::Blob(b))
                }
                rusqlite::types::ValueRef::Text(t) => {
                    let s = String::from_utf8_lossy(t).to_string();
                    Ok(Value::text(s))
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_and_query() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let adapter = RusqliteAdapter::open(&db_path).unwrap();
        
        // Create table
        adapter.execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
            [],
        ).unwrap();

        // Insert data
        adapter.execute(
            "INSERT INTO users (name) VALUES (?)",
            [Value::text("Alice")],
        ).unwrap();

        // Query data
        let results: Vec<_> = adapter.query("SELECT * FROM users WHERE name = ?", [])
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0][1].as_text().unwrap(), "Alice");
    }

    #[test]
    fn test_transaction_commit() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let adapter = RusqliteAdapter::open(&db_path).unwrap();
        
        // Create table
        adapter.execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
            [],
        ).unwrap();

        {
            let tx = adapter.begin_transaction().unwrap();
            
            // Insert within transaction
            tx.execute("INSERT INTO users (name) VALUES ('Bob')").unwrap();
            
            tx.commit().unwrap();
        }

        // Verify commit worked
        let results: Vec<_> = adapter.query("SELECT COUNT(*) FROM users", [])
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0].as_i64().unwrap(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let adapter = RusqliteAdapter::open(&db_path).unwrap();
        
        // Create table
        adapter.execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
            [],
        ).unwrap();

        {
            let tx = adapter.begin_transaction().unwrap();
            
            // Insert within transaction
            tx.execute("INSERT INTO users (name) VALUES ('Bob')").unwrap();
            
            tx.rollback().unwrap();
        }

        // Verify rollback worked
        let results: Vec<_> = adapter.query("SELECT COUNT(*) FROM users", [])
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0].as_i64().unwrap(), 0); // Should be empty
    }

    #[test]
    fn test_table_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let adapter = RusqliteAdapter::open(&db_path).unwrap();
        
        // Create table with various column types
        adapter.execute(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY, 
                name TEXT NOT NULL, 
                age INTEGER DEFAULT 0,
                active INTEGER DEFAULT 1
            )",
            [],
        ).unwrap();

        let schema = adapter.table_schema("users").unwrap();
        
        assert_eq!(schema.name, "users");
        assert_eq!(schema.columns.len(), 4);
        
        // Check first column (id) is primary key
        assert!(schema.columns[0].is_primary_key);
    }

    #[test]
    fn test_derive_guard_path() {
        let mirror_path = PathBuf::from("/data/mirror.db");
        let guard_path = RusqliteAdapter::derive_guard_path(&mirror_path);
        
        assert_eq!(guard_path, PathBuf::from("/data/guard.db"));
    }

    #[test]
    fn test_backend_type() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let adapter = RusqliteAdapter::open(&db_path).unwrap();
        assert_eq!(adapter.backend_type(), "rusqlite");
    }
}
