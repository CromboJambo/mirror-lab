# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `mirror-log-mcp` crate: JSON-RPC 2.0 over stdio MCP server exposing observation operations
- Hybrid attention layer in crabjar-conductor (ADR-009)
- Comprehensive CI pipeline with reproducible build verification

### Changed
- Updated README with comprehensive documentation of architecture and components
- Added LICENSE file (MIT)
- Standardized project structure for public presentation

## [0.1.0] - 2026-10-18

### Added
- Initial release of mirror-lab observation system
- `mirror-log` core library with attention-based observation scoring
- `labctl` CLI tool for declarative infrastructure management
- SQLite-based persistence layer with WAL mode support
