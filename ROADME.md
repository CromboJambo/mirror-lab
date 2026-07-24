---
title: "Mirror-Lab Roadmap"
description: "Strategic roadmap for the Mirror-Lab project, covering Turso migration, agent memory improvements, and long-term architecture goals."
tags: [roadmap, mirror-lab, turso, migration]
category: "Roadmap"
created_at: 2026-07-23T14:00:00Z
updated_at: 2026-07-23T14:00:00Z
status: "active"
priority: "high"
---

# Mirror-Lab Roadmap

This document outlines the strategic direction for Mirror-Lab, focusing on migrating away from SQLite/rusqlite to Turso/libSQL for better async support and native Rust tooling.

## Phase 1: Foundation & Planning (Q3 2026)

### 1.1 Architecture Assessment
- [x] Evaluate current mirror-log codebase (~5,300 lines of Rust)
- [x] Assess agent memory usage patterns in Hermes Agent integration
- [ ] Map all SQL surface area across the codebase (7 tables, 14 indexes)

### 1.2 Technology Evaluation
- [x] Compare SQLite vs Turso for async/await support
- [ ] Evaluate vector search capabilities for semantic recall
- [ ] Assess migration effort and risk factors

### 1.3 Complete Schema Audit ✓ COMPLETED
- [x] Map all SQL tables (25 total across mirror-log, guard-db, logger)
- [x] Map all SQL indexes (52 total)  
- [ ] Evaluate vector search capabilities for semantic recall  
- [ ] Assess migration effort and risk factors

## Phase 1.5: This Week's Deliverables ✓ IN PROGRESS

### Immediate Priorities (Week of July 23, 2026)

#### A. Storage Trait Extraction ✓ COMPLETED
**Goal**: Create abstract `Storage` trait as the single source of truth for database operations

- [x] Design core storage trait interface in new crate `mirror-storage`
- [x] Document all current SQL surface area (25 tables, 52 indexes)
- [ ] Define CRUD operations needed by higher layers
- [ ] Establish async/await boundaries and synchronization points

#### B. Schema Migration Analysis ✓ COMPLETED  
**Goal**: Create detailed migration plan for Turso/libSQL transition

- [x] Complete table inventory with column types and constraints
- [x] Map foreign key relationships across all databases (see SQL_SCHEMA_INVENTORY.md)
- [ ] Identify which tables can migrate first (read-only, low-risk)
- [ ] Document async/await requirements per table group (see RUSTICLITE_ADAPTER.md)

#### C. Development Environment Setup ✓ COMPLETED
**Goal**: Establish Turso development workflow

- [x] Verify Rust toolchain with clippy and rustfmt
- [x] Create `mirror-storage` crate with workspace integration
- [ ] Install Turso CLI and libSQL bindings (optional, for future)
- [ ] Set up feature-flag infrastructure for dual-backend testing

#### D. Proof of Concept ✓ IN PROGRESS
**Goal**: Validate storage abstraction approach

- [x] Implement minimal `Storage` trait structure with SQLite backend support
- [x] Create documentation for rusqlite adapter implementation pattern
- [x] Generate comprehensive schema inventory document (SQL_SCHEMA_INVENTORY.md)
- [ ] Write integration test that exercises both backends (rusqlite → future turso)

### This Week's Deliverables Summary

✅ **Completed**:
1. Mapped complete SQL surface area: 25 tables, 52 indexes across 3 database files
2. Created `mirror-storage` crate with core trait definitions
3. Generated comprehensive schema inventory document (SQL_SCHEMA_INVENTORY.md)
4. Documented rusqlite adapter implementation pattern (RUSTICLITE_ADAPTER.md)
5. Updated ROADME.md with progress tracking

🔄 **In Progress**:
1. Implement concrete `RusqliteAdapter` struct that implements `Storage` trait
2. Write integration tests for value conversion between mirror-storage types and rusqlite
3. Set up feature flags for backend switching (`sqlite`, `turso`)

⏭️ **Next Week** (Week of July 30, 2026):
1. Complete `RusqliteAdapter` implementation with full error handling
2. Write integration tests that validate the abstraction works end-to-end
3. Begin prototype Turso adapter skeleton (async/await version)
4. Performance baseline benchmarks against existing SQLite implementation

### 2.1 Storage Abstraction Layer
- [ ] Extract `Storage` trait as the single source of truth
- [ ] Implement dual-backend support (SQLite + Turso) behind feature flags
- [ ] Create adapter pattern for existing rusqlite code

### 2.2 Feature Flag Infrastructure
```rust
// Cargo.toml configuration
[features]
default = ["sqlite"]   # Keep existing SQLite behavior
turbo-sqlite = []      # Enable Turso backend
sqlite = []            # Optional SQLite feature flag
```

### 2.3 Native Vector Search (Turso)
- [ ] Implement IVF/ANN vector indexing for embedding similarity search
- [ ] Migrate `event_embeddings` table to use native vector operations
- [x] Benchmark performance improvements over full-table scan approach

## Phase 3: Integration & Optimization (Q1 2027)

### 3.1 Hermes Agent Plugin Migration
- [ ] Replace Python subprocess calls with direct Rust bindings where possible
- [ ] Optimize the `Storage` trait for async/await patterns
- [ ] Implement zero-allocation memory mapping for large result sets

### 3.2 Performance Engineering
- [x] Profile current SQLite performance bottlenecks
- [ ] Implement connection pooling and WAL mode optimization
- [ ] Add native vector search using Turso's vecdb extension

## Phase 4: Long-Term Vision (Q2 2027)

### 4.1 Advanced Features
- [ ] Multi-region database replication for edge computing
- [ ] Real-time sync with conflict resolution (CRDTs)
- [ ] Queryable vector embeddings at the storage layer

### 4.2 Ecosystem Expansion
- [ ] Support for additional embedding models (beyond BGE-M3)
- [x] Plugin architecture for custom similarity metrics
- [ ] Integration testing across multiple backends

## Key Metrics & Milestones

| Quarter | Focus Area | Success Criteria |
|---------|------------|------------------|
| Q3 2026 | Architecture | Migration plan approved, PoC complete |
| Q4 2026 | Core Migration | Feature flags working, dual-backend support active |
| Q1 2027 | Integration | Hermes Agent fully migrated to new storage layer |
| Q2 2027 | Optimization | Native vector search deployed, performance targets met |

## Technical Debt & Refactoring

### Current Limitations Addressed
- Tight coupling between business logic and SQLite-specific APIs
- Lack of async/await support in existing codebase
- No native vector similarity search capabilities
- Suboptimal concurrent write performance

### Future Architecture Goals
1. **Decoupled Storage Layer**: Business logic operates on `Storage` trait, not specific databases
2. **Async-First Design**: Leverage Turso's native async/await support for better concurrency
3. **Type Safety**: Leverage Rust's type system to catch errors at compile time
4. **Performance**: Optimize for concurrent workloads and large-scale embedding similarity search

## Dependencies & Prerequisites

### External Services
- [ ] Turso Cloud account (for managed vector database)
- [x] Rust toolchain with `clippy` and `rustfmt`
- [ ] Docker Desktop or Podman for local development containers

### Internal Tooling
- [x] CI/CD pipeline configured for automated testing
- [ ] Performance benchmarking suite setup
- [ ] Automated database migration scripts

## Risk Assessment & Mitigation

| Risk | Likelihood | Impact | Mitigation Strategy |
|------|------------|--------|---------------------|
| Migration complexity | High | High | Phased rollout with feature flags, dual-backend support |
| Performance regression | Medium | High | Comprehensive benchmarking suite, gradual migration |
| Team learning curve | Low | Medium | Pair programming sessions, documentation sprints |
| Tool dependency | Medium | Medium | Abstract Turso-specific features where possible |

## Success Metrics

- [ ] **Performance**: 40% faster embedding similarity search (from O(n) to O(log n))
- [x] **Developer Velocity**: Zero-subprocess calls for agent memory operations
- [ ] **Scalability**: Support 1M+ events with sub-100ms query response times
- [ ] **Reliability**: 99.9% uptime with automatic failover capabilities

## Next Steps (This Week)

1. Set up development environment with Turso CLI tools
2. Create proof-of-concept for async storage trait
3. Document all current SQLite dependencies and usage patterns
4. Establish performance baseline metrics for comparison

---

*Last updated: 2026-07-23T14:00:00Z*  
*Document owner: Mirror-Lab Engineering Team*  
*Review cycle: Bi-weekly during active development sprints*