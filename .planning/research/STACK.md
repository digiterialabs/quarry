# Stack Research

**Domain:** Rust-native local analytics engine (semantic layer over Iceberg on S3)
**Researched:** 2026-02-24
**Confidence:** MEDIUM -- core crates are verified and current, but the DataFusion/Iceberg version pinning creates a constraint that requires careful navigation

## Critical Version Constraint

**The most important finding in this research:** `iceberg-datafusion 0.8.0` depends on `datafusion ^51.0` (meaning `>=51.0.0, <52.0.0`). DataFusion is currently at `52.1.0`. The upgrade PR (apache/iceberg-rust#1997) is **open but not merged** as of 2026-02-24.

**Recommendation:** Pin to `datafusion 51.0.0` and `iceberg-datafusion 0.8.0` for now. This is a one-minor-version lag (Nov 2025 release), not a stale dependency. When iceberg-rust 0.9.0 ships with DataFusion 52 support, upgrade. Do not fight this constraint -- the version gap is small and 51.0 is fully capable.

## Recommended Stack

### Core Engine

| Technology | Version | Purpose | Why Recommended | Maturity | Confidence |
|------------|---------|---------|-----------------|----------|------------|
| datafusion | 51.0.0 | In-process SQL query engine | The only serious Rust-native embeddable SQL engine. Arrow-native, supports custom TableProviders, AnalyzerRules, and OptimizerRules. Used by InfluxDB, Wren Engine, and dozens of production systems. Pinned to 51.0 for iceberg-datafusion compatibility. | Stable | HIGH |
| iceberg | 0.8.0 | Apache Iceberg table format | Official Apache implementation. Supports V3 metadata, partitioned reads, schema evolution. 144 PRs from 37 contributors in this release alone. | Beta (rapidly maturing) | HIGH |
| iceberg-datafusion | 0.8.0 | DataFusion <-> Iceberg bridge | Provides `IcebergTableProvider` and `IcebergStaticTableProvider`. Registers Iceberg tables as DataFusion table sources. The glue between our SQL engine and table format. | Beta | HIGH |
| arrow | 57.0 | In-memory columnar format | Required by both DataFusion 51.0 and Iceberg 0.8.0. Shared dependency -- version must match exactly. | Stable | HIGH |
| parquet | 57.0 | Columnar file format | Iceberg stores data as Parquet files. Version locked to arrow 57.0. | Stable | HIGH |

### Storage & IO

| Technology | Version | Purpose | Why Recommended | Maturity | Confidence |
|------------|---------|---------|-----------------|----------|------------|
| opendal | 0.55.0 | Storage abstraction (S3, local, GCS, etc.) | Used internally by iceberg 0.8.0 for all file IO. You do NOT add this directly -- iceberg brings it. Supports S3, S3-compatible (MinIO), local filesystem, GCS, Azure. | Stable | HIGH |
| iceberg-catalog-rest | 0.8.0 | REST catalog client | For production deployments with a catalog server (Nessie, Polaris, Tabular). Vendor-neutral, recommended for new deployments by Iceberg community. | Beta | HIGH |
| iceberg-catalog-glue | 0.8.0 | AWS Glue catalog | For AWS-native deployments using Glue Data Catalog. Improved concurrency error handling in 0.8.0. | Beta | MEDIUM |
| iceberg-catalog-sql | 0.8.0 | SQL-backed catalog (SQLite/Postgres) | For lightweight self-managed catalog. SQLite for dev/testing, Postgres for production. Uses sqlx internally. | Beta | MEDIUM |

**Note on S3 access:** iceberg-rust uses OpenDAL (not `object_store` or `aws-sdk-s3`) for S3 access. OpenDAL is configured via `FileIOBuilder::new("s3")` with properties for region, credentials, endpoint. You do NOT need `aws-sdk-s3` or `object_store` crates directly. AWS credential chain is supported through OpenDAL's S3 backend.

**Note on StaticTable:** For the simplest case (reading Iceberg tables directly from S3 metadata.json without a catalog server), use `iceberg::table::StaticTable::from_metadata_file()`. This is read-only and requires knowing the metadata file path, but eliminates the need for any catalog infrastructure. Perfect for the ephemeral CLI use case.

### Semantic Layer & Configuration

| Technology | Version | Purpose | Why Recommended | Maturity | Confidence |
|------------|---------|---------|-----------------|----------|------------|
| serde | 1.0.219 | Serialization framework | Universal Rust serialization. Required by everything. | Stable | HIGH |
| serde_yaml_ng | 0.10.0 | YAML parsing | The maintained fork of serde_yaml. The original `serde_yaml` is deprecated (Mar 2024). `serde_yml` (the other fork) has a RUSTSEC advisory for unsoundness and is archived. `serde_yaml_ng` is the correct choice. | Stable | HIGH |
| serde_json | 1.0.149 | JSON output | For rich JSON query results. Rock solid, maintained by dtolnay. | Stable | HIGH |

### CLI & Runtime

| Technology | Version | Purpose | Why Recommended | Maturity | Confidence |
|------------|---------|---------|-----------------|----------|------------|
| clap | 4.5.60 | CLI argument parsing | Industry standard. Derive macros for declarative CLI definition. 680M+ downloads. | Stable | HIGH |
| tokio | 1.47+ | Async runtime | Required by DataFusion, iceberg, and OpenDAL. All three depend on tokio. Use `features = ["full"]` for the CLI binary, or `["rt-multi-thread", "macros"]` for minimal. Pin to `^1.47` to match iceberg 0.8.0's requirement. | Stable | HIGH |

### Error Handling & Observability

| Technology | Version | Purpose | Why Recommended | Maturity | Confidence |
|------------|---------|---------|-----------------|----------|------------|
| thiserror | 2.0.18 | Typed error definitions | For library/domain error types (SemanticLayerError, CatalogError, etc.). thiserror 2.0 supports `#[error(transparent)]` and better ergonomics. | Stable | HIGH |
| anyhow | 1.0.102 | Application error handling | For the CLI binary layer where you just need to propagate and report errors. Use anyhow at the top level, thiserror for domain types. | Stable | HIGH |
| tracing | 0.1.41 | Structured logging | Async-aware, span-based. Integrates with tokio. Use `tracing-subscriber` for output formatting. Much better than `log` crate for async code with DataFusion. | Stable | HIGH |
| tracing-subscriber | 0.3.x | Log output formatting | Provides `fmt::Subscriber` for console output. Use `EnvFilter` for RUST_LOG control. | Stable | HIGH |

### Development & Testing

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo-nextest | Test runner | Faster parallel test execution than `cargo test`. |
| cargo-deny | Dependency auditing | License checking, vulnerability scanning, duplicate detection. |
| cargo-clippy | Linting | Use `#![deny(clippy::all, clippy::pedantic)]` for strict linting. |
| rustfmt | Formatting | Standard formatting. Use `edition = "2024"` in rustfmt.toml. |

## Rust Edition & MSRV

| Setting | Value | Rationale |
|---------|-------|-----------|
| Edition | 2024 | Matches iceberg-rust 0.8.0 workspace (edition = "2024"). Requires Rust 1.85+. Current stable is 1.93.1. |
| MSRV | 1.88.0 | Set by iceberg-rust 0.8.0 workspace (`rust-version = "1.88"`). This is the binding constraint. |

## Version Compatibility Matrix

This is the critical compatibility chain. All versions must align.

| Crate | Version | Arrow Version | DataFusion Version | Tokio Version |
|-------|---------|---------------|--------------------|---------------|
| datafusion | 51.0.0 | 57.0 | -- | ^1.48 |
| iceberg | 0.8.0 | 57.0 | -- | ^1.47 |
| iceberg-datafusion | 0.8.0 | -- | ^51.0 | ^1.47 |
| parquet | 57.0 | 57.0 | -- | -- |

**Why this matters:** Arrow 57.0 is shared across DataFusion 51 and Iceberg 0.8.0. If you accidentally pull in DataFusion 52 (which uses Arrow 57.1), Cargo may resolve conflicting Arrow versions and cause compilation errors or runtime type mismatches with Arrow RecordBatches.

## Cargo.toml Skeleton

```toml
[package]
name = "quarry"
version = "0.1.0"
edition = "2024"
rust-version = "1.88"

[dependencies]
# Core engine
datafusion = "51.0"
iceberg = "0.8"
iceberg-datafusion = "0.8"
arrow = { version = "57.0", features = ["prettyprint"] }
parquet = "57.0"

# Catalog support (pick what you need)
iceberg-catalog-rest = { version = "0.8", optional = true }
iceberg-catalog-glue = { version = "0.8", optional = true }
iceberg-catalog-sql = { version = "0.8", optional = true }

# Semantic layer
serde = { version = "1.0", features = ["derive"] }
serde_yaml_ng = "0.10"
serde_json = "1.0"

# CLI
clap = { version = "4.5", features = ["derive"] }

# Runtime
tokio = { version = "1.47", features = ["rt-multi-thread", "macros"] }

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utilities
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.18", features = ["v4"] }
futures = "0.3"
async-trait = "0.1"

[features]
default = ["catalog-rest"]
catalog-rest = ["dep:iceberg-catalog-rest"]
catalog-glue = ["dep:iceberg-catalog-glue"]
catalog-sql = ["dep:iceberg-catalog-sql"]

[dev-dependencies]
tempfile = "3.18"
assert_json_diff = "2.0"
tokio = { version = "1.47", features = ["full", "test-util"] }
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| SQL Engine | datafusion 51.0 | DuckDB (via duckdb-rs) | DuckDB is C++ with Rust bindings, not Rust-native. Adds C++ build dependency. Less extensible for custom semantic rewriting (no AnalyzerRule/OptimizerRule equivalent in bindings). Does have its own Iceberg support but through different path. |
| SQL Engine | datafusion 51.0 | polars | Polars is a DataFrame library, not a SQL engine. No equivalent of TableProvider, CatalogProvider, or SQL planning pipeline. Wrong tool for SQL-first semantic layer. |
| Iceberg | iceberg 0.8.0 (apache) | iceberg-rust (JanKaul) | JanKaul's is an unofficial fork. The official Apache implementation has more contributors (120+), active governance, and aligned release cadence with iceberg-datafusion. |
| Iceberg | iceberg 0.8.0 (apache) | delta-rs (Delta Lake) | Project constraint: Iceberg only. Delta Lake is out of scope per PROJECT.md. |
| YAML | serde_yaml_ng | serde_yml | RUSTSEC-2025-0068: unsound, archived. Do not use. |
| YAML | serde_yaml_ng | serde_yaml | Deprecated since Mar 2024. Do not use. |
| S3 Access | opendal (via iceberg) | aws-sdk-s3 | Not needed. OpenDAL handles S3 internally within iceberg. Adding aws-sdk-s3 would create a parallel S3 client with separate config. |
| S3 Access | opendal (via iceberg) | object_store | DataFusion uses object_store internally, but iceberg uses OpenDAL. They don't conflict at the API level, but you configure S3 through iceberg's FileIO, not through object_store directly. |
| Error Handling | thiserror + anyhow | eyre | eyre is excellent but less conventional. thiserror+anyhow is the idiomatic Rust pairing recommended by the Rust community. |
| Logging | tracing | log + env_logger | tracing is span-aware and async-native. log is synchronous and flat. DataFusion and tokio both use tracing internally. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| serde_yaml (dtolnay) | Deprecated since March 2024, unmaintained | serde_yaml_ng 0.10 |
| serde_yml | RUSTSEC-2025-0068 unsoundness advisory, archived | serde_yaml_ng 0.10 |
| datafusion 52.x | Incompatible with iceberg-datafusion 0.8.0 (`^51.0` pin) | datafusion 51.0 |
| aws-sdk-s3 directly | Iceberg uses OpenDAL internally; separate S3 client creates config split | Configure S3 through iceberg's FileIO |
| object_store directly | Not needed for Iceberg IO; may conflict with OpenDAL's S3 handling | Let iceberg manage storage via OpenDAL |
| log crate | Synchronous, flat, no spans. Poor fit for async DataFusion workloads | tracing 0.1 |
| iceberg-rust (JanKaul unofficial) | Unofficial fork, lower contributor count, unclear governance | iceberg 0.8.0 (apache official) |
| datafusion_iceberg (JanKaul) | Different project from apache/iceberg-datafusion. Uses DataFusion 50, older APIs | iceberg-datafusion 0.8.0 (apache official) |

## Stack Patterns by Variant

**If using a REST catalog (Nessie, Polaris, Tabular):**
- Add `iceberg-catalog-rest` feature
- Configure catalog URL + credentials at runtime
- Best for multi-tenant production deployments

**If using AWS Glue catalog:**
- Add `iceberg-catalog-glue` feature
- Uses AWS credential chain automatically
- Best for AWS-native data lake deployments

**If using no catalog (direct S3 metadata path):**
- Use `iceberg::table::StaticTable::from_metadata_file()`
- No catalog crate needed
- Pass S3 path to metadata.json directly
- Best for the V1 CLI ephemeral use case -- simplest path to working code
- Limitation: must know the metadata file path; no table listing/discovery

**If using SQLite catalog (dev/testing):**
- Add `iceberg-catalog-sql` feature
- `SqlCatalog::new("sqlite://path/to/catalog.db")`
- Good for local development and integration testing

## Semantic Layer Architecture Pattern (from Wren Engine)

Wren AI (open source, github.com/Canner/wren-engine) has built a production semantic layer on DataFusion with a pattern directly applicable to Quarry:

1. **Define semantic models as custom `UserDefinedLogicalNode`** -- Models become nodes in the logical plan
2. **Implement `AnalyzerRule`** to rewrite semantic references into physical SQL subqueries
3. **Use DataFusion's `Unparser`** to convert LogicalPlans back to SQL text if needed
4. **DataFusion validates the rewritten plan** -- catches invalid queries before execution

This is the recommended pattern for Quarry's semantic layer. It works WITH DataFusion's query planning instead of around it. The alternative (string-based SQL rewriting) is fragile and loses DataFusion's type checking and optimization.

**Confidence:** MEDIUM -- Wren Engine is a verified open-source project using this pattern, but Quarry's specific semantic model (YAML-defined metrics/dimensions) will need custom implementation. The DataFusion APIs (AnalyzerRule, UserDefinedLogicalNode) are stable and documented.

## Upgrade Path

| When | Action | Why |
|------|--------|-----|
| iceberg-rust 0.9.0 ships | Upgrade to datafusion 52.x + iceberg 0.9.0 + iceberg-datafusion 0.9.0 | PR #1997 is open; expected Q1-Q2 2026 |
| arrow 58.x releases | Wait for iceberg to adopt it | Arrow versions must stay aligned across iceberg + datafusion |
| opendal 0.56+ releases | No action needed | iceberg pins its own opendal version internally |

## Sources

- [DataFusion 52.1.0 on docs.rs](https://docs.rs/crate/datafusion/latest) -- latest version, published 2026-01-24 (HIGH confidence)
- [DataFusion 51.0.0 on docs.rs](https://docs.rs/crate/datafusion/51.0.0) -- our target version, published 2025-11-19 (HIGH confidence)
- [iceberg 0.8.0 on docs.rs](https://docs.rs/crate/iceberg/latest) -- published 2026-01-19 (HIGH confidence)
- [iceberg-datafusion 0.8.0 on docs.rs](https://docs.rs/crate/iceberg-datafusion/latest) -- depends on datafusion ^51.0 (HIGH confidence)
- [Apache Iceberg Rust 0.8.0 Release Blog](https://iceberg.apache.org/blog/apache-iceberg-rust-0.8.0-release/) -- 144 PRs, 37 contributors (HIGH confidence)
- [iceberg-rust GitHub](https://github.com/apache/iceberg-rust) -- workspace Cargo.toml confirms datafusion = "51.0", edition = "2024", rust-version = "1.88" (HIGH confidence)
- [DataFusion 52 upgrade PR #1997](https://github.com/apache/iceberg-rust/pull/1997) -- open as of 2026-02-24 (HIGH confidence)
- [iceberg-rust main Cargo.toml](https://github.com/apache/iceberg-rust/blob/main/Cargo.toml) -- opendal = 0.55.0 (HIGH confidence)
- [serde_yaml deprecation discussion](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868) -- serde_yaml_ng recommended (MEDIUM confidence)
- [RUSTSEC-2025-0068](https://rustsec.org/advisories/RUSTSEC-2025-0068.html) -- serde_yml unsoundness advisory (HIGH confidence)
- [Wren Engine on DataFusion](https://www.getwren.ai/post/powering-semantic-sql-for-ai-agents-with-apache-datafusion) -- semantic layer pattern reference (MEDIUM confidence)
- [Wren Engine GitHub](https://github.com/Canner/wren-engine) -- open source reference implementation (MEDIUM confidence)
- [StaticTable docs](https://rust.iceberg.apache.org/api/iceberg/table/struct.StaticTable.html) -- catalog-free table reading (HIGH confidence)
- [FileIO docs](https://rust.iceberg.apache.org/api/iceberg/io/struct.FileIO.html) -- S3 storage abstraction (HIGH confidence)
- [OpenDAL and Iceberg architecture](https://www.hackintoshrao.com/one-interface-many-backends-the-design-of-iceberg-rusts-universal-storage-layer-with-opendal/) -- storage layer design (MEDIUM confidence)
- [tokio releases](https://github.com/tokio-rs/tokio/releases) -- latest 1.49.0 (HIGH confidence)
- [clap on docs.rs](https://docs.rs/crate/clap/latest) -- 4.5.60 (HIGH confidence)
- [Rust 1.93.1 stable](https://blog.rust-lang.org/releases/latest/) -- current stable compiler (HIGH confidence)

---
*Stack research for: Quarry -- Rust-native local analytics engine*
*Researched: 2026-02-24*
