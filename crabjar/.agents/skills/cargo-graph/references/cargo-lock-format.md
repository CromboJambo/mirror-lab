# Cargo.lock Format Reference (v3)

> Reference for manual dependency graph analysis when `cargo-declared` is not available. Read this when walking the lockfile by hand to derive the compiled set or trace transitive paths.

## File structure

`Cargo.lock` is a TOML file. The top-level structure is:

```toml
version = 3

[[package]]
name = "my-crate"
version = "0.1.0"
dependencies = [
  "dep-a",
  "dep-b 2.0.0",
]

[[package]]
name = "dep-a"
version = "1.5.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123..."
dependencies = [
  "dep-c 0.9.1 (registry+https://github.com/rust-lang/crates.io-index)",
]
```

---

## `[[package]]` fields

| Field | Required | Notes |
|---|---|---|
| `name` | Yes | Crate name as published |
| `version` | Yes | Resolved semver version |
| `source` | No | Absent for path deps and workspace members. `"registry+..."` for crates.io deps. `"git+..."` for git deps. |
| `checksum` | No | SHA-256 of the `.crate` tarball. Only present for registry deps. |
| `dependencies` | No | List of direct dependencies of this package in the resolved graph. |

---

## Dependency reference format in `dependencies` array

Each entry is a string. Three formats:

1. **Name only** — `"dep-a"` — used when the name is unambiguous in the lockfile (only one version present)
2. **Name + version** — `"dep-b 2.0.0"` — used when multiple versions of the same crate are present
3. **Name + version + source** — `"dep-c 0.9.1 (registry+https://github.com/rust-lang/crates.io-index)"` — used when source disambiguation is needed

When walking the graph manually, resolve each dependency string to its `[[package]]` entry by matching name, then version if present, then source if present.

---

## Deriving the compiled set

1. Find the root package — the `[[package]]` entry whose `name` matches the `[package] name` in `Cargo.toml`.
2. Every other `[[package]]` entry is a compiled dependency. The root package itself is excluded.
3. The `source` field distinguishes registry deps (have source) from workspace members and path deps (no source).

## Tracing transitive paths (BFS)

To find what pulled in crate X:

1. Build an adjacency map: for each `[[package]]`, map its name+version → its `dependencies` list.
2. Start a BFS from the root package.
3. For each package visited, record its predecessor (the package that first reached it).
4. When X is reached, the predecessor chain back to root is the `via` path.
5. The first declared dependency in that chain (i.e., the one that appears in `Cargo.toml`) is the `via` attribution.

This is the BFS shortest-predecessor algorithm used in `cargo-declared`'s `delta.rs`.

---

## Workspace lockfiles

In a workspace, `Cargo.lock` lives at the workspace root and covers all members. Each workspace member appears as a `[[package]]` without a `source` field. The workspace root itself is also a `[[package]]` entry.

When analyzing a workspace member, filter the compiled set to packages reachable from that member's `[[package]]` entry, not from the workspace root.

---

## Version 3 vs version 2

Cargo.lock v3 (Cargo ≥ 1.78) uses a more compact dependency reference format. The parsing logic above applies to both v2 and v3. The `version = 3` field at the top distinguishes them. If absent, assume v2 — the format is compatible for graph walking purposes.

---

## What Cargo.lock cannot tell you

- Which dependencies are optional (that requires `Cargo.toml`)
- Which features are enabled (features affect which optional deps appear; `Cargo.lock` shows the resolved result after feature selection)
- Build-time vs runtime distinction for transitive deps (the `[[package]]` entries do not carry `kind`; kind propagation requires walking from root with kind tracking, as `cargo-declared`'s `metadata.rs` does)
