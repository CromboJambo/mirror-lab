# Contributing to ***

Thanks for your interest in contributing! This project aims to ***

---

## Quick Links

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Architecture Overview](#architecture-overview)
- [Testing Guidelines](#testing-guidelines)
- [Style Guide](#style-guide)
- [Agent-Friendly Prompts](#agent-friendly-prompts)

---

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

**TL;DR**: Be respectful, constructive, and collaborative.

---

## Getting Started

### Prerequisites

- Rust 1.70+ (edition 2021)
- Familiarity with async Rust (tokio)
- ...

### Clone and Build
````bash
git clone https://github.com/***/***.git
cd ***
cargo build
cargo test
````

### Run Examples
````bash
cargo run --example optimistic_ui
cargo run --example async_race
cargo run --example simple_check
````

---

## Development Setup

### Recommended Tools
````bash
# Format checker
cargo install rustfmt

# Linter
cargo install clippy

# Documentation generator
cargo doc --open

# Mutation testing (optional)
cargo install cargo-mutants
````

### IDE Setup

**Zed** (recommended):
- Zed is built in rust so 'rust-analyzer' is already included

- **VS Code** :
- Install `rust-analyzer` extension
- Enable format-on-save
- Configure clippy lints

**Other IDEs**: Any editor with LSP support works great.

---

## How to Contribute

### Types of Contributions We Need

#### 🐛 Bug Reports
Found a causality violation we're missing? Or worse, a false positive?

**Template**:
````markdown
**Describe the bug**
*** reports [Honest/Leaky/Deceptive] but should report [X]

**Code to reproduce**
```rust
// Minimal example here
```s

**Expected behavior**
Should detect/not detect: ...

**Environment**
- Rust version: 
- *** version:
- Async runtime: tokio/async-std/other
````

#### ✨ Feature Requests

**Good feature requests**:
- Solve a real problem you've encountered
- Include example use case
- Consider implementation complexity

**Template**:
````markdown
**Problem**
I need to detect [specific pattern] but currently can't because...

**Proposed solution**
Add a method/feature that...

**Example usage**
```rust
// How you'd use it
```

**Alternatives considered**
- Could use X but it doesn't work because...
````

#### 📚 Documentation
- Fix typos
- Clarify confusing sections
- Add more examples
- Improve API docs

#### 🧪 Tests
- Add edge cases
- Improve coverage
- Add integration tests
- Benchmark performance

#### 🚀 New Features

See [Priority Areas](#priority-areas) below.

---

## Architecture Overview

### Core Components
````
***/
├── src/
│   ├── lib.rs           # Core types: Event, EventKind, ***
│   ├── tracing.rs       # Tracing integration (WIP)
│   ├── macros.rs        # Convenience macros (planned)
│   └── bin/
│       └── ***.rs  # CLI tool
├── examples/            # Real-world usage examples
└── tests/               # Integration tests
````

### Key Types
````rust
// Event in the system

### Running Tests
````bash
# All tests
cargo test

# Specific test
cargo test test_***

# With output
cargo test -- --nocapture

# Integration tests only
cargo test --test integration_test
````

---

## Style Guide

### Code Style

**Follow Rust conventions**:
- Use `rustfmt` (enforced in CI)
- Follow `clippy` lints
- Prefer explicit over implicit
- Document public APIs

**Naming**:
````rust
// Good
pub fn detect_violations(&self) -> Vec<Violation>
pub struct AsyncIntegrity

// Avoid
pub fn check(&self) -> Vec<V>  // Too vague
pub struct AI  // Ambiguous acronym
````

**Documentation**:
````rust
````

### Commit Messages

**Format**: `type(scope): description`

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `test`: Adding tests
- `refactor`: Code restructuring
- `perf`: Performance improvement
- `chore`: Maintenance

**Examples**:
````
feat(core): add support for compensation tracking
fix(cli): handle empty event streams correctly
docs(readme): clarify causality concept
test(integration): add async race condition tests
````

### PR Guidelines

**Good PR**:
- Single logical change
- Tests included
- Documentation updated
- Passes CI
- Clear description

**Template**:
````markdown
## Description
What does this PR do?

## Motivation
Why is this change needed?

## Testing
How was this tested?

## Checklist
- [ ] Tests pass locally
- [ ] Added tests for new code
- [ ] Updated documentation
- [ ] Ran `cargo fmt`
- [ ] Ran `cargo clippy`
````

---

## Priority Areas

### High Priority

#### 1. 

**Skills needed**: Proc macros, tracing ecosystem

---

#### 2. **Visualization**

**Goal**: 

**Skills needed**: 

#### 3. **Production Monitoring**
Emit violations as metrics/logs for production use.

**Goal**: Integration with ***, ***, etc.
````rust
````

**Skills needed**: Observability tools, metrics

---

#### 4. **Compensation Tracking**
Track rollback/compensation operations.

**Goal**: Detect when reversible operations aren't actually reversed.
````rust
Event::compensation("Rolled back UI update")
    .for_effect(ui_effect_id)
````

**Skills needed**: System design, state machines

---

### Medium Priority

#### 5. **Macro-based Instrumentation**
Ergonomic macros for common patterns.
````rust
track_async! {
    checker,
    "Process payment",
    {
        validate().await?;
        charge_card().await?;
    }
}
````

---

#### 6. **JSON Export**
Export events and violations for analysis.

**Goal**: Integration with Polars, Jupyter, etc.
````bash
*** run -- ./app > violations.json
polars analyze violations.json
````

---

#### 7. **More Examples**
Real-world patterns:
- 
- 
- 
- 
- 

---

### Lower Priority (but welcome!)

- WASM support
- No-std support
- Async-std integration
- Performance benchmarks
- Fuzzing integration

---

## Agent-Friendly Prompts

> **Note**: This section is specifically for AI coding assistants working with this codebase.

### Adding a New Event Type

**Prompt**:
````
Add a new event type called [EventName] to ***.

Requirements:
1. Add variant to EventKind enum in src/lib.rs
2. Add convenience constructor to Event impl
3. Update detect_violations() if needed
4. Add tests in mod tests
5. Update documentation
6. Add example in examples/

Event semantics: [describe what this event means]
````

### Implementing a New Detector

**Prompt**:
````
Implement detection for [pattern name] in ***.

Pattern: [describe the anti-pattern]
Example: [code showing the bug]

Requirements:
1. Add method to AsyncHonestyChecker
2. Define new Violation type if needed
3. Add comprehensive tests
4. Document with examples
5. Update README.md

Return type should be Vec<Violation> or similar.
````

### Adding Integration

**Prompt**:
````
Add integration with [tool name] to ***.

Goal: [what should the integration do]

Requirements:
1. Create new module: src/integrations/[tool].rs
2. Add feature flag in Cargo.toml
3. Document usage in README
4. Add example: examples/[tool]_integration.rs
5. Add integration test

Dependencies: [list required crates]
````

### Improving Documentation

**Prompt**:
````
Improve documentation for [component] in ***.

Current issues: [what's unclear/missing]

Requirements:
1. Add/improve doc comments
2. Add code examples
3. Cross-reference related functions
4. Update README if public API
5. Add doctests

Target audience: [beginners/advanced users/both]
````

### Adding Tests

**Prompt**:
````
Add tests for [feature/edge case] in async-honesty.

Scenario: [describe what should be tested]

Requirements:
1. Add unit tests in src/lib.rs::tests
2. Add integration test if needed
3. Test both success and failure cases
4. Test edge cases: [list specific edges]
5. Ensure tests are deterministic

Use std::thread::sleep for timing when needed.
````

### Performance Optimization

**Prompt**:
````
Optimize [component] in async-honesty for performance.

Current bottleneck: [describe issue]
Benchmark showing problem: [data]

Requirements:
1. Profile with cargo flamegraph
2. Implement optimization
3. Add benchmark in benches/
4. Ensure correctness (all tests pass)
5. Document performance impact

Constraints: [any correctness requirements]
````

---

## Review Process

### For Maintainers

**Review checklist**:
- [ ] Code follows style guide
- [ ] Tests cover new functionality
- [ ] Documentation is clear
- [ ] No breaking changes (or justified)
- [ ] CI passes
- [ ] Commit messages are clear

**Typical timeline**:
- Small PRs (< 100 lines): 1-2 days
- Medium PRs: 3-5 days
- Large PRs: 1 week+

---

## Questions?

**Best ways to get help**:

1. **GitHub Discussions**: For design questions, feature ideas
2. **GitHub Issues**: For bug reports, concrete proposals

**Response time**: Expect response within 2-3 days.

---

## Recognition

Contributors are recognized in:
- CHANGELOG.md for each release
- README.md contributors section
- Git history (your commits!)

Significant contributions may warrant co-authorship on any future papers/talks about the project.

---

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (AGPL-3.0 or MIT or MPL-2.0 or Apache-2.0).

---

**Thank you for helping! 🦀⚡**

---

## Appendix: Example Contribution Workflow

### Adding a Feature
````bash
# 1. Fork and clone
git clone https://github.com/yourusername/***.git
cd ***
# 2. Create branch
git checkout -b feat/my-feature

# 3. Make changes
# ... edit code ...

# 4. Test
cargo test
cargo clippy
cargo fmt

# 5. Commit
git add .
git commit -m "feat(core): add my feature"

# 6. Push and PR
git push origin feat/my-feature
# Open PR on GitHub
````

### Fixing a Bug
````bash
# 1. Create branch
git checkout -b fix/issue-123

# 2. Write failing test
# Add test that reproduces the bug

# 3. Fix bug
# Implement fix

# 4. Verify
cargo test  # Test should now pass

# 5. Commit and PR
git commit -m "fix(core): resolve issue #123"
git push origin fix/issue-123
````

---

**Made with 🦀 by developers who believe in honest software.**
