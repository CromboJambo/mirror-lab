//! Concrete storage adapters implementing the `Storage` trait.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{Connection, Row, params_from_iter};

use crate::{
    ColumnSchema, IntoParams, Storage, StorageError, TableSchema,
    Transaction as StorageTransaction, Value,
};

/// Rusqlite-based implementation of the `Storage` trait.
///
/// This adapter wraps a rusqlite `Connection` and exposes it through the abstracted
/// `Storage` interface defined in `mirror-storage`. It serves as the reference
/// implementation before adding Turso/libSQL support.
pub struct RusqliteAdapter {
    conn: Arc<Mutex<Connection>>,
}

impl RusqliteAdapter {
    /// Create a new adapter from an existing rusqlite Connection.
    pub fn from_connection(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    /// Open or create a database at the given path.
    /// Applies performance optimizations (WAL mode, etc.).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(&path).map_err(|e| StorageError::Connection(e.to_string()))?;

        // Apply performance optimizations
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;\n             PRAGMA synchronous = NORMAL;\n             PRAGMA temp_store = MEMORY"
        )
        .map_err(|e| StorageError::Connection(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Derive guard DB path from mirror DB path.
    pub fn derive_guard_path(mirror_path: &Path) -> PathBuf {
        let mut guard_path = mirror_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        guard_path.push("guard.db");
        guard_path
    }

    /// Open guard DB co-located with mirror DB.
    pub fn open_guard_from_mirror(mirror_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let guard_path = Self::derive_guard_path(mirror_path.as_ref());
        Self::open(&guard_path)
    }

    /// Lock the connection, recovering a poisoned lock by reusing the guard.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        match self.conn.lock() {
            Ok(guard) => Ok(guard),
            Err(poisoned) => Ok(poisoned.into_inner()),
        }
    }
}

/// Convert this crate's `Value` enum into rusqlite parameter values.
fn to_rusqlite_params(values: &[Value]) -> Vec<rusqlite::types::Value> {
    values
        .iter()
        .map(|v| match v {
            Value::Null => rusqlite::types::Value::Null,
            Value::Text(s) => rusqlite::types::Value::Text(s.clone()),
            Value::Integer(n) => rusqlite::types::Value::Integer(*n),
            Value::Float(f) => rusqlite::types::Value::Real(*f),
            Value::Blob(b) => rusqlite::types::Value::Blob(b.clone()),
            Value::Boolean(b) => rusqlite::types::Value::Integer(i64::from(*b)),
        })
        .collect()
}

impl Storage for RusqliteAdapter {
    fn query(&self, sql: &str, params: impl IntoParams) -> Result<Vec<Vec<Value>>, StorageError> {
        let conn = self.lock()?;
        let rp = to_rusqlite_params(params.as_slice());
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| StorageError::Query(e.to_string()))?;
        let rows = stmt
            .query_map(params_from_iter(rp), |row| {
                row_to_values(row).map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .map_err(|e| StorageError::Query(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    fn execute(&self, sql: &str, params: impl IntoParams) -> Result<i64, StorageError> {
        let conn = self.lock()?;
        let rp = to_rusqlite_params(params.as_slice());
        let rows_affected = conn
            .execute(sql, params_from_iter(rp))
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(rows_affected as i64)
    }

    fn begin_transaction(&self) -> Result<Box<dyn StorageTransaction>, StorageError> {
        let conn = self.lock()?;
        conn.execute("BEGIN IMMEDIATE", [])
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        drop(conn);

        Ok(Box::new(RusqliteTransaction {
            conn: Arc::clone(&self.conn),
            committed: RefCell::new(false),
        }))
    }

    fn backend_type(&self) -> &'static str {
        "rusqlite"
    }

    fn table_schema(&self, name: &str) -> Result<TableSchema, StorageError> {
        let conn = self.lock()?;

        // PRAGMA table_info columns: 0=cid, 1=name, 2=type, 3=notnull,
        // 4=dflt_value, 5=pk
        let mut columns = Vec::new();
        let mut stmt = conn
            .prepare("SELECT * FROM pragma_table_info(?1)")
            .map_err(|e| StorageError::Schema(e.to_string()))?;
        let rows = stmt
            .query_map(
                params_from_iter(vec![rusqlite::types::Value::Text(name.to_string())]),
                |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i32>(3)? == 0,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, i32>(5)? != 0,
                    ))
                },
            )
            .map_err(|e| StorageError::Schema(e.to_string()))?;
        for row in rows {
            let (col_name, data_type, nullable, default_value, is_primary) =
                row.map_err(|e| StorageError::Schema(e.to_string()))?;

            columns.push(ColumnSchema {
                name: col_name,
                data_type,
                nullable,
                is_primary_key: is_primary,
                default_value,
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
    conn: Arc<Mutex<Connection>>,
    committed: RefCell<bool>,
}

impl StorageTransaction for RusqliteTransaction {
    fn execute(&self, sql: &str) -> Result<i64, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        let rows_affected = conn
            .execute(sql, [])
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        Ok(rows_affected as i64)
    }

    fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        let rp = to_rusqlite_params(params);
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| StorageError::Query(e.to_string()))?;
        let rows = stmt
            .query_map(params_from_iter(rp), |row| {
                row_to_values(row).map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .map_err(|e| StorageError::Query(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    fn commit(&self) -> Result<(), StorageError> {
        if *self.committed.borrow() {
            return Err(StorageError::Transaction(
                "Transaction already committed".to_string(),
            ));
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        conn.execute("COMMIT", [])
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        *self.committed.borrow_mut() = true;
        Ok(())
    }

    fn rollback(&self) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        conn.execute("ROLLBACK", [])
            .map_err(|e| StorageError::Transaction(e.to_string()))?;
        Ok(())
    }
}

/// Convert a rusqlite Row to Vec<Value>.
fn row_to_values(row: &Row<'_>) -> Result<Vec<Value>, String> {
    let count = row.as_ref().column_count();

    (0..count)
        .map(|i| match row.get_ref(i).map_err(|e| e.to_string())? {
            rusqlite::types::ValueRef::Null => Ok(Value::Null),
            rusqlite::types::ValueRef::Integer(n) => Ok(Value::integer(n)),
            rusqlite::types::ValueRef::Real(f) => Ok(Value::Float(f)),
            rusqlite::types::ValueRef::Blob(b) => Ok(Value::Blob(b.to_vec())),
            rusqlite::types::ValueRef::Text(t) => {
                let s = String::from_utf8_lossy(t).to_string();
                Ok(Value::text(s))
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
        adapter
            .execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)", [])
            .unwrap();

        // Insert data
        adapter
            .execute(
                "INSERT INTO users (name) VALUES (?)",
                [Value::text("Alice")],
            )
            .unwrap();

        // Query data
        let results = adapter
            .query("SELECT * FROM users WHERE name = ?", [Value::text("Alice")])
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0][1].as_text().unwrap(), "Alice");
    }

    #[test]
    fn test_transaction_commit() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let adapter = RusqliteAdapter::open(&db_path).unwrap();

        // Create table
        adapter
            .execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)", [])
            .unwrap();

        {
            let tx = adapter.begin_transaction().unwrap();

            // Insert within transaction
            tx.execute("INSERT INTO users (name) VALUES ('Bob')")
                .unwrap();

            tx.commit().unwrap();
        }

        // Verify commit worked
        let results = adapter.query("SELECT COUNT(*) FROM users", []).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0].as_i64().unwrap(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let adapter = RusqliteAdapter::open(&db_path).unwrap();

        // Create table
        adapter
            .execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)", [])
            .unwrap();

        {
            let tx = adapter.begin_transaction().unwrap();

            // Insert within transaction
            tx.execute("INSERT INTO users (name) VALUES ('Bob')")
                .unwrap();

            tx.rollback().unwrap();
        }

        // Verify rollback worked
        let results = adapter.query("SELECT COUNT(*) FROM users", []).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0][0].as_i64().unwrap(), 0); // Should be empty
    }

    #[test]
    fn test_table_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let adapter = RusqliteAdapter::open(&db_path).unwrap();

        // Create table with various column types
        adapter
            .execute(
                "CREATE TABLE users (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    age INTEGER DEFAULT 0,
                    active INTEGER DEFAULT 1
                )",
                [],
            )
            .unwrap();

        let schema = adapter.table_schema("users").unwrap();

        assert_eq!(schema.name, "users");
        assert_eq!(schema.columns.len(), 4);

        // Check first column (id) is primary key
        assert!(schema.columns[0].is_primary_key);
        // name is NOT NULL
        assert!(!schema.columns[1].nullable);
        // age has default 0
        assert_eq!(schema.columns[2].default_value.as_deref(), Some("0"));
        // active has default 1
        assert_eq!(schema.columns[3].default_value.as_deref(), Some("1"));
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
