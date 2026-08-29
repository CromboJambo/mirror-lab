# plan.md — clippy remediation handoff

> **STATUS: COMPLETE (2026-08-29).** All items done and committed:
> - §1 mirror-log → `cd7438c6`
> - §2 mirror-daemon → `1baa84db`
> - §3 mirror-storage (Option A) → `d80bad1e`
> - Bonus: mirror-guard `pending_queue` confidence REAL/f64 mismatch → `1616249a`;
>   gate.rs fmt → `f1d0acb1`.
> - Gate: `cargo fmt --all -- --check` ✅, `cargo clippy --workspace -- -D warnings` ✅,
>   `cargo test --workspace --exclude mirror-voice` ✅ (mirror-voice needs system libxdo,
>   not installed here — pre-existing env gap, not a code issue).
>
> Created: 2026-08-29
> Context: after wiring the statefulness layer into `mirror-log` (commit `78fb810a`) and
> fixing the `mirror-kernel` clippy blocker (commit `4fa5aa5c`), the workspace clippy gate
> (`cargo clippy --workspace -- -D warnings`) still fails. This is because fixing the kernel
> unblocked compilation of the dependent crates, which surfaced **pre-existing** clippy errors
> in files that were never touched. This doc lists those so a fresh session can pick them up.
>
> Verified state at time of writing:
> - `mirror-kernel` — clippy clean, tests pass. ✅
> - `mirror-log` — state layer wired, 93 tests pass, `state.rs`/`cli.rs`/`bin`/`lib.rs`/`view` clippy-clean.
>   Two **pre-existing** errors remain (below).
> - `mirror-daemon` — two **pre-existing** errors (below).
> - `mirror-storage` — orphan crate, ~25 errors, does not compile (below).

---

## 1. mirror-log — 2 quick clippy fixes

Both are trivial, mechanical, and isolated. No behavior change.

### 1a. `mirror-log/src/log.rs:103` — variable used as a loop counter

Current (`append_batch_with_receipts_internal`, lines 100–110):

```rust
let mut receipts = Vec::with_capacity(contents.len());
let mut timestamp = next_timestamp(conn)?;

for content in contents {
    receipts.push(append_single_event_internal(
        conn, source, content, meta, timestamp,
    )?);
    timestamp += 1;
}
```

The manual `timestamp += 1` counter trips the loop-counter lint. Fix with `enumerate()` —
the offset is `i` past the base timestamp:

```rust
let mut receipts = Vec::with_capacity(contents.len());
let base_timestamp = next_timestamp(conn)?;

for (i, content) in contents.iter().enumerate() {
    let ts = base_timestamp + i as i64;
    receipts.push(append_single_event_internal(conn, source, content, meta, ts)?);
}
```

Note: `contents: &[&str]`, so `contents.iter().enumerate()` yields `(&&str)` — pass `content`
through as-is (it auto-derefs to `&str` at the call site) or use `contents.iter().copied()`.
Confirm the parameter type of `append_single_event_internal` before finalizing.

### 1b. `mirror-log/src/pipeline.rs:254` — explicit `.into_iter()`

Current (in `flush_batch`, line 254):

```rust
for (content, receipt) in batch.iter().zip(receipts.into_iter()) {
```

`receipts` is a `Vec`; the explicit `.into_iter()` is redundant. It's only read here, so
borrow it:

```rust
for (content, receipt) in batch.iter().zip(receipts.iter()) {
```

`receipt.id` is then behind a `&AppendReceipt` — `receipt.id.clone()` already appears on the
next line, so no further change needed.

### Verify (mirror-log)
```
cargo clippy -p mirror-log -- -D warnings   # expect: clean
cargo test -p mirror-log                    # expect: 93 pass
```

---

## 2. mirror-daemon — 2 quick clippy fixes

Both in `mirror-daemon/src/daemon.rs`, `run_async()`, lines 198–205. Redundant `&` in a
`println!`/`eprintln!` argument (the format string already takes a reference / the value is
already a `String`).

### 2a. `mirror-daemon/src/daemon.rs:200`
```rust
// current
"[✅] Successfully processed event for pipeline '{}'. Reflection ID: {}",
&event.pipeline, id
```
Remove the `&` → `event.pipeline`.

### 2b. `mirror-daemon/src/daemon.rs:204`
```rust
// current
"[❌] Failed to process event for pipeline '{}': {}",
&event.pipeline, e
```
Remove the `&` → `event.pipeline`.

### Verify (mirror-daemon)
```
cargo clippy -p mirror-daemon -- -D warnings   # expect: clean
cargo test -p mirror-daemon                    # expect: pass
```

---

## 3. mirror-storage — detailed handoff (does not compile)

**Status:** orphan crate. It is a workspace member (`Cargo.toml:13`) but **nothing depends on
it** (grep for `mirror-storage` across all `Cargo.toml` finds only the workspace member list).
`mirror-log` has its own direct `rusqlite` layer and does not use this abstraction. ~25 errors,
549 LOC across two files.

**Files:**
- `mirror-storage/src/lib.rs` (184 lines) — `Storage` trait, `Value` enum, `IntoParams`,
  `StorageError`, `TableSchema`/`ColumnSchema`, `Transaction` trait, 3 unit tests.
- `mirror-storage/src/adapters.rs` (365 lines) — `RusqliteAdapter` + `RusqliteTransaction`,
  `row_to_values`, 6 integration-style tests (tempfile-based).

**Dependency:** `rusqlite 0.32` (bundled), `thiserror`, dev: `tokio`, `tempfile 3.14`.

### 3.1 The root problem (architectural, not cosmetic)

The `Storage` trait is designed so `query()` returns a **lazy iterator that borrows the
connection**:

```rust
// lib.rs:46-51
pub trait Storage: Send + Sync {
    fn query(
        &self,
        sql: &str,
        params: impl IntoParams,
    ) -> Box<dyn Iterator<Item = Result<Vec<Value>, StorageError>> + '_>;
```

But `RusqliteAdapter` holds its connection behind `Mutex<Connection>` (`adapters.rs:18`). The
impl (`adapters.rs:63-89`) locks the mutex, prepares a statement, and tries to return
`Box::new(rows.into_iter().flatten())` where `rows: MappedRows`. This cannot work:

1. **Lifetime/guard conflict** — the returned iterator would have to outlive the `MutexGuard`
   that is dropped at end of `query()`. The guard can't be held across a returned value.
2. **`MappedRows` is not `Send`** — it carries `*mut sqlite3_stmt` plus `RefCell<InnerConnection>`,
   `RefCell<BTreeMap<...>>`, `RefCell<LruCache<...>>`. The trait requires `+ Send`. Errors at
   `adapters.rs:88` (the `Box::new(rows.into_iter().flatten())` line) are all this.

**Decision point (ask before starting):** this is a half-finished abstraction with zero
callers. Three viable paths:

- **Option A — fix it properly.** Change `query()` to return an **owned** `Vec<Vec<Value>>`
  (or `Result<Vec<Vec<Value>>, StorageError>`) instead of a lazy iterator. This kills the
  lifetime/Send problem entirely (materialize rows inside the locked section, drop the guard,
  return owned data). Re-derive the trait signature and update all 6 tests. This is the real
  work — estimate a focused session.
- **Option B — archive it.** Per the project roadmap Phase 3, move `mirror-storage/` to
  `archive/` and remove it from `Cargo.toml` members. Cheapest path to a green workspace gate;
  appropriate if the abstraction isn't actually planned.
- **Option C — remove from members** (temporarily) to unblock the gate while the design is
  reconsidered.

**Recommendation:** confirm intent first. If the abstraction is wanted (e.g. a future
Turso/libSQL backend per the doc comment at `adapters.rs:14-16`), go Option A. If not, Option B.

### 3.2 If Option A — the concrete fix list

Work top-down; the trait signature change cascades.

1. **`lib.rs:46-51`** — change `query()` return to owned:
   ```rust
   fn query(&self, sql: &str, params: impl IntoParams) -> Result<Vec<Vec<Value>>, StorageError>;
   ```
   (Drop the `'_` lifetime and the `Box<dyn Iterator>`.) Update the `Transaction::query`
   signature (`lib.rs:148-152`) to match — it already returns `Result<Vec<Vec<Value>>, _>`, so
   they converge.

2. **`lib.rs` duplicate imports** — line 8 and line 37 both import `Mutex`/`PoisonError`
   (E0252). Delete line 37's `use std::sync::{Mutex, PoisonError};` and keep line 8. The
   `From<PoisonError<...>>` impl (lines 39-43) then uses the single import.

3. **`lib.rs:75-83` `IntoParams`** — `as_slice() -> &[Value]` is fine as the *internal*
   conversion, but the adapter must convert this crate's `Value` enum into rusqlite params
   (see #4). Keep the trait; the conversion lives in the adapter.

4. **`adapters.rs` param conversion** — rusqlite's `query_map`/`execute` need `impl Params`;
   `&[crate::Value]` is **not** a `Params` (errors at `adapters.rs:83,100`). Add a helper:
   ```rust
   fn to_rusqlite_params(values: &[Value]) -> Vec<rusqlite::Value> { /* map each arm */ }
   ```
   then pass `rusqlite::params_from_iter(vec)` (or `&vec`). Map `Value::Null→Value::Null`,
   `Text→to_string`, `Integer→as i64`, `Float→as f64`, `Blob→as Vec<u8>`, `Boolean→as i64`.

5. **`adapters.rs:63-89` `query`** — rewrite to materialize:
   ```rust
   fn query(&self, sql: &str, params: impl IntoParams) -> Result<Vec<Vec<Value>>, StorageError> {
       let conn = self.conn.lock().unwrap();
       let rp = to_rusqlite_params(params.as_slice());
       let mut stmt = conn.prepare(sql).map_err(|e| StorageError::Query(e.to_string()))?;
       let rows = stmt.query_map(rusqlite::params_from_iter(rp), |row| row_to_values(row))?;
       rows.collect::<Result<Vec<_>, _>>().map_err(|e| StorageError::Query(e))
   }
   ```
   (Now returns owned data; guard drops at end of fn; no `Send`/lifetime issue.)

6. **`adapters.rs:91-104` `execute`** — same param conversion; `conn.execute(sql, params_from_iter(rp))`.

7. **`adapters.rs:106-117` `begin_transaction`** — type mismatch: `conn` here is a
   `MutexGuard<Connection>`, but it's wrapped as `Arc::new(Mutex::new(conn))` expecting a
   `Connection` (error at `:114`). Also manual `BEGIN IMMEDIATE` via SQL is fragile. Options:
   - Use rusqlite's `Transaction` API: `let tx = conn.transaction()?;` and have
     `RusqliteTransaction` wrap the `Transaction`. But `Transaction` is not `Send`, which
     conflicts with `trait Transaction: Send + Sync` (`lib.rs:146`). Either relax that bound
     (remove `Send + Sync`) or keep the manual SQL approach but fix the type: store a **cloned
     connection** or restructure so the transaction holds its own `Mutex<Connection>`.
   - Simplest correct path: drop `Send + Sync` from the `Transaction` trait, wrap the guard's
     connection properly, keep manual `BEGIN`/`COMMIT`/`ROLLBACK`.

8. **`adapters.rs:123-154` `table_schema`** — `pragma_query_table` closure returns
   `rusqlite::Result` but the tuple's `nullable`/`pk` logic is inverted-ish and
   `default_value` is unimplemented (noted in comment). Compiles once the import issues are
   gone, but verify the column-index mapping against `PRAGMA table_info` (0=cid,1=name,
   2=type,3=notnull,4=dflt_value,5=pk). Currently reads idx 4 as notnull and 5 as pk — **idx 4
   is `dflt_value`, not notnull**. Fix: notnull = idx 3, pk = idx 5, default = idx 4 (this also
   fills the `default_value` field).

9. **`adapters.rs:207-225` `row_to_values`** —
   - `:208` `row.as_ref().len()` — no such method. Use `row.column_count()`.
   - `:213` `ValueRef::Integer(n) => *n as i64` — `n` is already `i64` (not a ref); `*n` is E0614.
     Use `n`.
   - `:214` `ValueRef::Real(f) => *f` — same; `f` is `f64`, use `f`.
   - `:211` `row.get_ref(i)` — fine, but note `get_ref` returns `Result`; the
     `unwrap_or(ValueRef::Null)` swallows real errors. Consider propagating.

10. **`lib.rs:63-69` `table_exists`** — `SELECT 1 FROM {name} LIMIT 1` is a SQL-injection
    vector and has off-by-one semantics (empty table → no rows but `execute` succeeds →
    `true`; missing table → error → `false`, which happens to work). Replace with a
    `sqlite_master` lookup:
    ```rust
    let n: i64 = self.execute(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
        [Value::text(name)],
    )?;
    Ok(n > 0)
    ```

11. **`adapters.rs:6` unused `params` import** — remove `params` from
    `use rusqlite::{params, Connection, Row};` (replaced by `params_from_iter` in #4/#5).

12. **Update the 6 tests** (`adapters.rs:227-365`) to the new owned `query()` signature:
    `adapter.query(...)` now returns `Result<Vec<Vec<Value>>, _>`, so drop the
    `.map(|r| r.unwrap()).collect()` iterator chaining (lines 252-254, 283-285, 314-316).

### Verify (mirror-storage, Option A)
```
cargo clippy -p mirror-storage -- -D warnings   # expect: clean
cargo test -p mirror-storage                    # expect: 9 pass (3 lib + 6 adapter)
```

---

## 4. Final gate

After 1, 2, and 3 are done:
```
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```
All three must pass for the workspace gate to be green.

## 5. Suggested commit split (one logical change each, per AGENTS.md)
1. `Fix mirror-log clippy: enumerate batch timestamps, drop redundant into_iter`
2. `Fix mirror-daemon clippy: remove redundant references in println`
3. `mirror-storage: <fix per Option A>` **or** `Archive mirror-storage crate` (Option B)
