# Technology Stack

**Analysis Date:** 2026-02-24

## Languages

**Primary:**
- Rust - Core analytics engine implementation, DataFusion integration, S3/Iceberg support

**Secondary:**
- JavaScript/Node.js (CommonJS) - Build tools and automation (GSD framework hooks)

## Runtime

**Environment:**
- Rust runtime with no external runtime dependency (native binary)
- Node.js - For development tooling only

**Package Manager:**
- Cargo - Rust dependency management
- npm - JavaScript/Node.js dependencies for development tools

## Frameworks

**Core:**
- DataFusion - Distributed SQL execution engine (in-process, ephemeral compute)
- Apache Iceberg - Table format for S3-based data storage

**Development/Tooling:**
- GSD (Get Shit Done) framework - Project management and workflow automation
- Node.js CommonJS - Development scripts and hooks

## Key Dependencies

**Critical:**
- DataFusion - Vectorized SQL execution engine for structured analytics
- Iceberg - S3-native table format with ACID transactions and schema evolution
- S3 SDK - For reading/writing data to S3 (AWS or compatible)

**Infrastructure:**
- Apache Arrow - In-memory columnar format (DataFusion dependency)

## Configuration

**Environment:**
- No .env file detected - configuration to be determined in development
- Development setup uses GSD configuration system (`.claude/settings.json`)

**Build:**
- Cargo.toml - Standard Rust project manifest (not yet created)
- Cargo.lock - Dependency lock file (to be generated)

## Platform Requirements

**Development:**
- Rust toolchain (latest stable or nightly)
- Cargo package manager
- Node.js for GSD automation scripts

**Production:**
- Linux or macOS (native binary)
- S3-compatible object storage access
- Network connectivity to data lakes

**Architecture:**
- Ephemeral compute model - no long-running cluster
- Sandboxed execution per query
- Local in-memory compute bringing DataFusion to S3-native data

---

*Stack analysis: 2026-02-24*
