# Project Research Summary

**Project:** Quarry
**Domain:** Rust-native local analytics engine — semantic layer over Apache Iceberg on S3
**Researched:** 2026-02-24
**Confidence:** MEDIUM-HIGH

## Executive Summary

Quarry is a Rust-native, ephemeral semantic layer that translates structured metric queries into DataFusion SQL executed against Apache Iceberg tables on S3. The product occupies a genuine gap: existing semantic layers (Cube, dbt/MetricFlow, Looker) are long-running server processes that require infrastructure, whereas Quarry executes as a single binary invocation — no daemon, no connection pool, no warm state. This architecture is uniquely suited to AI-agent tool use where analytics calls happen via MCP or CLI on demand, and where deploying a persistent analytics service is not viable. The research confirms this approach is technically sound and has a direct precedent in Wren Engine, which built a production semantic layer on DataFusion using the same LogicalPlan-as-IR pattern Quarry should adopt.

The recommended implementation builds Quarry in three library crates (quarry-core for semantic logic, quarry-exec for DataFusion/Iceberg execution, quarry-cli as the thin binary) with a clear dependency ordering: semantic model and query resolution first (no I/O), Iceberg integration second, CLI wiring last. The core technology decisions are locked in by a critical version constraint: iceberg-datafusion 0.8.0 requires datafusion 51.0.0 (an upgrade PR is open but unmerged as of today), which means the stack is pinned to DataFusion 51 + Arrow 57 + iceberg 0.8. This is a one-minor-version lag, not a stale dependency. YAML parsing must use serde_yaml_ng 0.10 — both the original serde_yaml (deprecated March 2024) and serde_yml (RUSTSEC-2025-0068 unsoundness) are disqualified.

The two highest-risk areas are semantic layer correctness and tenant isolation safety. Join fan-out — where aggregate metrics silently inflate due to one-to-many join cardinality — is documented in academic research as the most common correctness failure in semantic layers, and it produces wrong answers without errors. Tenant isolation implemented via SQL string manipulation is known to be bypassable by CTEs, subqueries, and UNION constructs; it must instead be implemented as a DataFusion AnalyzerRule operating on the LogicalPlan tree. Both risks are architectural — they must be addressed in Phase 1 design, not added later.

## Key Findings

### Recommended Stack

The stack is tightly constrained by the DataFusion/iceberg-rust version matrix. Use datafusion 51.0.0, iceberg 0.8.0, iceberg-datafusion 0.8.0, and arrow/parquet 57.0 as a pinned unit. All four must be aligned or Arrow type mismatches cause cryptic runtime errors. The tokio async runtime (1.47+) is required by all three major dependencies. For YAML parsing, serde_yaml_ng 0.10 is the only safe choice. For CLI argument parsing, clap 4.5 with derive macros. For error handling, thiserror for typed domain errors in the library crates and anyhow at the CLI binary level. For structured logging, tracing 0.1 (not the log crate — DataFusion and tokio both use tracing internally).

**Core technologies:**
- `datafusion 51.0.0`: In-process SQL query engine — the only serious Rust-native embeddable SQL engine; pinned for iceberg-datafusion compatibility
- `iceberg 0.8.0` + `iceberg-datafusion 0.8.0`: Apache Iceberg table format + DataFusion bridge — official Apache implementation with active governance
- `arrow 57.0` / `parquet 57.0`: Columnar memory format and file format — shared dependency; versions must be identical across datafusion and iceberg
- `serde_yaml_ng 0.10`: YAML parsing — the only maintained, safe fork; serde_yaml is deprecated, serde_yml has a RUSTSEC advisory
- `clap 4.5` + `tokio 1.47+` + `thiserror 2.0` + `anyhow 1.0` + `tracing 0.1`: CLI, async runtime, and error/observability stack

**Critical constraint:** Do not use datafusion 52.x. The iceberg-datafusion 0.8.0 dependency is `^51.0`. Upgrade only when iceberg-rust 0.9.0 ships with DataFusion 52 support (PR apache/iceberg-rust#1997, expected Q1-Q2 2026).

See `.planning/research/STACK.md` for full Cargo.toml skeleton, alternative analysis, and upgrade path.

### Expected Features

Quarry's feature set follows the standard semantic layer taxonomy (YAML model definition, metric types, join resolution, SQL compilation) but is tuned for the AI-agent use case: ephemeral execution, MCP server mode, machine-parseable errors, and schema auto-discovery from Iceberg metadata. The competitor reference point is dbt/MetricFlow for model conventions and Cube for multi-tenancy patterns.

**Must have (table stakes — v1):**
- YAML semantic model definition (entities, dimensions, measures, metrics) — follow dbt/MetricFlow conventions for YAML schema
- Metric definitions: simple, derived, cumulative, and ratio metric types
- Join resolution — automatic join path discovery from entity relationships; adopt MetricFlow's 3-table limit for safety
- Semantic-to-SQL compilation — translate metric queries to DataFusion LogicalPlan (NOT SQL strings)
- Dimension slicing and filtering — WHERE and GROUP BY generation with time grain support (day/week/month/quarter/year)
- Row-level tenant isolation — WHERE clause injection via DataFusion AnalyzerRule
- Query validation — validate metric/dimension/filter combinations against semantic model before execution
- Rich JSON output — data rows + metadata (row count, timing, generated SQL, schema, tenant context)
- CLI interface — `quarry query` returns JSON, `quarry validate` checks YAML model correctness
- Iceberg table integration — read metadata, resolve current snapshot, partition pruning via iceberg-datafusion

**Should have (AI-agent differentiators — v1.x):**
- Schema auto-discovery — `quarry discover` generates draft YAML from Iceberg table metadata
- MCP server mode — expose Quarry as a Model Context Protocol tool server for any MCP-compatible agent
- Semantic model introspection — `quarry describe` returns available metrics/dimensions as structured JSON for agent self-service
- Path-level tenant isolation — per-tenant Iceberg S3 prefix scoping for physical data separation
- Cumulative and ratio metrics — time-window aggregations and cross-metric calculations
- Query plan explanation — dry-run mode showing resolved SQL and join plan
- Structured error responses — machine-parseable errors with error codes and valid alternatives for agents

**Defer (v2+):**
- HTTP API — stateless REST wrapper for non-CLI consumers
- OSI/SML compatibility — import/export in Open Semantic Interchange format
- Iceberg metadata caching — local cache to reduce cold-start latency
- Multiple catalog support — REST, Glue, Hive metastore all behind single interface

**Confirmed anti-features (do not build):** Web UI, pre-aggregation/caching layer, LLM/natural language query interface, long-running daemon mode, write-back/data mutation, stateful multi-query sessions, real-time/streaming data support.

See `.planning/research/FEATURES.md` for full feature dependency graph, competitor comparison, and prioritization matrix.

### Architecture Approach

Quarry uses a linear pipeline architecture: CLI parse -> Config load -> Semantic model registry -> Query resolution to LogicalPlan -> Execution context setup (with tenant isolation rules) -> DataFusion execution -> JSON serialization. The pivot point of the entire architecture is the DataFusion LogicalPlan — semantic concepts resolve to it (never to SQL strings), and DataFusion executes it (with all optimizer rules including filter pushdown into Iceberg partition pruning). Tenant isolation is implemented as a DataFusion AnalyzerRule that walks every LogicalPlan and injects tenant WHERE predicates on every TableScan node. The crate structure enforces clean architecture: quarry-core has no I/O dependencies (fast to compile, testable with unit tests), quarry-exec owns all DataFusion/Iceberg/S3 integration, quarry-cli is a thin orchestrator.

**Major components:**
1. `quarry-core/model + config`: Semantic model types (metric, dimension, entity, join) + YAML loading/validation — no DataFusion dependency, pure Rust
2. `quarry-core/resolve`: Semantic query to DataFusion LogicalPlan — metric lookup, dimension lookup, join path resolution, plan construction
3. `quarry-core/tenant`: TenantIsolationRule (AnalyzerRule) for row-level isolation; path-scope for S3 prefix scoping
4. `quarry-exec/engine + catalog + table`: DataFusion SessionContext setup, Iceberg catalog connection, IcebergStaticTableProvider registration (lazy — only for tables referenced in the plan)
5. `quarry-exec/result`: Arrow RecordBatch stream to JSON with metadata envelope
6. `quarry-cli/main`: Argument parsing (clap) and pipeline orchestration; no business logic

**Key pattern decisions:**
- LogicalPlan as IR: build DataFusion plan trees directly — avoids SQL injection, dialect issues, and string manipulation bugs; automatic access to 30+ DataFusion optimizer rules
- Lazy table registration: analyze the LogicalPlan for referenced tables, then register only those Iceberg providers — avoids fetching metadata for unused tables
- AnalyzerRule for tenant isolation (not OptimizerRule — OptimizerRules must produce semantically equivalent plans; adding WHERE changes semantics)
- Ephemeral SessionContext per query — no warm state, correct by construction, V2+ can add warm-start without architectural change

See `.planning/research/ARCHITECTURE.md` for full component diagram, data flow walkthrough, build ordering, and anti-patterns.

### Critical Pitfalls

1. **Join fan-out producing silently wrong metric results** — Documented in academic research as the most common semantic layer correctness failure. When a fact table joins a dimension with one-to-many cardinality, aggregates (SUM, COUNT) silently multiply. Prevent by: adding explicit cardinality annotations to all entity relationships in the YAML schema, implementing join-type selection logic that rejects fan-out joins, and adding a consistency test that `SUM(metric)` is identical with and without dimension joins. Must be designed into Phase 1 — retrofitting requires a schema rewrite.

2. **Tenant isolation bypassed by complex query patterns** — Row-level isolation via SQL string manipulation is bypassable by CTEs, subqueries, UNION, and window functions. Prevent by: implementing tenant isolation as a DataFusion AnalyzerRule that walks the LogicalPlan tree (not SQL text), and building an adversarial test suite of 10+ bypass patterns. Design this as an architectural constraint in Phase 1 — it cannot be bolted on.

3. **DataFusion/Arrow/iceberg-rust version lock-in** — DataFusion releases breaking API changes every 6-8 weeks. Arrow types must be re-exported from datafusion (`use datafusion::arrow`) not imported separately. Any misaligned arrow version causes cryptic type mismatch errors at runtime. Prevent by: pinning the version matrix in Cargo.toml with explicit comments, never depending on `arrow` directly alongside `datafusion`, and tracking iceberg-rust#1997 for the next upgrade window.

4. **S3 metadata fetch latency destroying ephemeral performance** — Each Iceberg query requires serial S3 GETs (metadata.json -> manifest list -> manifests -> data files), adding 500ms–2s of cold start overhead with no warm cache. Prevent by: parallelizing manifest fetches once the manifest list is loaded, using manifest-level partition statistics for aggressive pruning, and tracking per-query S3 request count as a first-class metric from day one.

5. **iceberg-rust feature gaps calcifying into permanent workarounds** — iceberg-rust trails the Java reference implementation. Teams build workarounds for missing features and they become entangled. Prevent by: wrapping all Iceberg access behind a trait boundary (`IcebergReader` trait), documenting every workaround with a `// TODO(iceberg-upstream)` comment linking to the tracking issue, and never implementing Iceberg spec logic (manifest parsing, partition pruning) in your own code.

See `.planning/research/PITFALLS.md` for the full pitfall registry including technical debt patterns, integration gotchas, performance traps, and a "looks done but isn't" checklist.

## Implications for Roadmap

The architecture research provides an explicit build order based on dependency relationships. The semantic layer has no I/O dependencies and should be built and verified first. DataFusion/Iceberg integration is the highest integration risk and should come after the semantic layer is solid. CLI wiring is last and trivial. AI-agent differentiators (MCP, introspection, auto-discovery) depend on the core query loop and belong in a subsequent phase.

### Phase 1: Project Foundation and Semantic Model

**Rationale:** The architecture research identifies Phases 1-3 of the build order (semantic model, query resolution, tenant isolation rules) as having no I/O dependencies — they compile fast and can be fully tested with unit tests. This is where all the critical intellectual complexity lives and where the most dangerous pitfalls (join fan-out, tenant bypass) must be addressed by design. Starting here lets the team establish the correct foundations before any integration complexity enters.

**Delivers:** Cargo workspace setup with all pinned dependencies, quarry-core crate with semantic model types and YAML loading, query resolver producing DataFusion LogicalPlans, tenant isolation AnalyzerRule, and full unit test coverage of semantic resolution correctness.

**Addresses (from FEATURES.md):** YAML semantic model definition, metric definitions (simple + derived), entity and dimension types, query validation, semantic model validation (quarry validate)

**Avoids (from PITFALLS.md):**
- Join fan-out: cardinality annotations in YAML schema, join validation logic
- Tenant isolation bypass: AnalyzerRule designed from day one, not SQL string injection
- DataFusion version lock-in: Cargo.toml version matrix set, arrow re-exported from datafusion, trait boundaries established

### Phase 2: DataFusion and Iceberg Execution

**Rationale:** Once the semantic layer is tested in isolation, integrate the I/O layer. The architecture research flags this as the highest integration risk phase: iceberg-rust + DataFusion interop is the most uncertain area, and S3 latency patterns must be understood from the beginning. Building this after Phase 1 means integration debugging is isolated to execution concerns, not entangled with semantic logic.

**Delivers:** quarry-exec crate with DataFusion SessionContext setup, lazy Iceberg table provider registration, end-to-end query execution against real Iceberg tables on S3 (or local filesystem catalog for testing), Arrow RecordBatch to JSON serialization with metadata envelope.

**Uses (from STACK.md):** iceberg-datafusion 0.8.0 IcebergStaticTableProvider, iceberg-catalog-sql for local testing, DataFusion's filter pushdown optimizer, arrow_json for result serialization

**Implements (from ARCHITECTURE.md):** quarry-exec/engine, quarry-exec/catalog, quarry-exec/table (tenant-scoped provider wrapper), quarry-exec/result

**Avoids (from PITFALLS.md):**
- S3 metadata latency: parallel manifest fetching, S3 request count tracking from the start
- iceberg-rust feature gaps: IcebergReader trait boundary, workaround documentation pattern
- Memory exhaustion: DataFusion MemoryPool configured and tested
- Parquet type mapping: integration tests covering decimal, timestamp-with-tz, nested structs

### Phase 3: CLI Integration and End-to-End Validation

**Rationale:** With semantic resolution (Phase 1) and execution (Phase 2) independently tested, the CLI wiring is thin orchestration. This phase wires the pipeline together, adds the binary entry point, and validates the full system against real Iceberg data. It also addresses tenant isolation completeness with an adversarial test suite.

**Delivers:** quarry-cli binary (clap arg parsing, pipeline orchestration, exit codes), end-to-end integration tests against real Iceberg tables, adversarial tenant isolation test suite (CTEs, subqueries, UNIONs), error message wrapping for all DataFusion/Arrow errors, basic CLI UX (progress indication on metadata fetch, --format table option).

**Avoids (from PITFALLS.md):**
- Cryptic error messages: all DataFusion/Arrow errors wrapped with domain context before reaching user
- Tenant isolation bypass: adversarial test suite with 10+ bypass patterns

### Phase 4: AI-Agent Differentiators (v1.x)

**Rationale:** Once the core query loop is working and validated, add the features that make Quarry uniquely valuable for AI agents. MCP server mode depends on semantic model introspection, which in turn requires the full query validation logic from Phase 1. Schema auto-discovery requires Iceberg integration from Phase 2. This phase is logically blocked on Phases 1-3.

**Delivers:** MCP server mode (list_metrics, list_dimensions, query, describe_metric, validate_query tools), semantic model introspection API (quarry describe), schema auto-discovery (quarry discover generates draft YAML from Iceberg metadata), path-level tenant isolation (per-tenant S3 prefix scoping), structured error responses for agents, cumulative and ratio metric types.

**Addresses (from FEATURES.md):** All P2 features — MCP server mode, introspection, auto-discovery, path-level tenancy, structured errors, query plan explanation

### Phase 5: Performance, Hardening, and Distribution (v2)

**Rationale:** Once product-market fit is established and usage patterns are known, optimize the critical path and prepare for broader distribution. Iceberg metadata caching is deferred here because V1 cold-start latency may be acceptable for the agent use case; only add caching if benchmarks reveal it as a blocker.

**Delivers:** Optional local Iceberg metadata caching (SQLite-backed, per-metadata-file TTL), HTTP API mode (stateless REST wrapper for non-CLI consumers), partition-aware query optimization, OSI/SML model compatibility, binary distribution via GitHub releases + Homebrew + cargo install, cargo-deny integration for vulnerability scanning.

**Addresses (from FEATURES.md):** All P3 features deferred from launch

### Phase Ordering Rationale

- **Semantic logic before I/O:** Phases 1-3 of the architecture build order have no I/O dependencies. Unit testing is fast and complete. Integration complexity in Phase 2 does not entangle with semantic correctness.
- **Critical pitfalls addressed in Phase 1:** The two most dangerous pitfalls (join fan-out, tenant isolation bypass) must be designed into the data model and architecture — they cannot be bolted on. The schema must include cardinality annotations from the start.
- **MCP deferred to Phase 4:** MCP requires introspection and validation as foundations. Building it in Phase 4 means these foundations are already solid and tested.
- **Caching and distribution deferred to Phase 5:** V1 cold-start latency may be acceptable. Add caching only after measuring real usage patterns.

### Research Flags

Phases likely needing deeper research during planning:

- **Phase 1:** Join path resolution algorithm — the graph-based join path finder (given entity relationships, find a valid join path for a set of dimensions) requires a concrete algorithm decision (BFS vs. DFS, cycle detection, multi-hop limit). MetricFlow limits to 3 hops; Quarry should adopt the same limit but the implementation needs design work.
- **Phase 2:** iceberg-datafusion IcebergStaticTableProvider vs. IcebergCatalogTableProvider — the difference in behavior for snapshot resolution and partition pruning is documented but requires hands-on validation. The lazy registration pattern (register only tables in the LogicalPlan's TableScan nodes) needs implementation research.
- **Phase 4:** MCP server protocol implementation — MCP is specified at modelcontextprotocol.io but Rust SDK maturity is unclear. May require implementing directly against the JSON-RPC spec if no mature Rust MCP library exists.

Phases with standard patterns (skip research-phase):

- **Phase 1 (YAML parsing):** serde + serde_yaml_ng is a well-documented, standard pattern. No research needed.
- **Phase 1 (DataFusion AnalyzerRule):** Documented with examples in DataFusion's official library user guide. Standard pattern.
- **Phase 3 (CLI wiring):** clap with derive macros is a completely standard pattern. No research needed.
- **Phase 3 (JSON serialization):** arrow_json::ArrayWriter is official and well-documented. No research needed.
- **Phase 5 (binary distribution):** cargo release + GitHub Actions for cross-compilation is a fully established pattern.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core crates verified on docs.rs and crates.io. Version matrix confirmed from iceberg-rust workspace Cargo.toml. RUSTSEC advisory and deprecation notices verified from official sources. The only uncertainty is the DataFusion 52 upgrade timeline (PR open, not merged). |
| Features | MEDIUM-HIGH | Feature taxonomy from well-established semantic layer vendors (Cube, dbt/MetricFlow, AtScale). MCP specification is published and stable. AI-agent-specific patterns are newer but converging on clear conventions. |
| Architecture | HIGH (DataFusion) / MEDIUM (Iceberg integration) | DataFusion extension points (AnalyzerRule, TableProvider, LogicalPlan builder) are officially documented and stable. Iceberg integration via iceberg-datafusion has fewer examples in the wild. The Wren Engine case study provides strong directional evidence for the LogicalPlan-as-IR pattern. |
| Pitfalls | MEDIUM-HIGH | Core pitfalls verified across official docs, production post-mortems (Greptime, Quesma), GitHub issues, and academic research (join fan-out paper). Tenant isolation bypass patterns based on general SQL complexity reasoning, not Quarry-specific exploitation. |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address

- **Join path resolution algorithm:** The concrete algorithm for finding valid join paths given a set of requested dimensions and entity relationships needs design during Phase 1 planning. Research confirmed this is the hardest table-stakes feature but did not produce a reference implementation to copy directly.
- **iceberg-datafusion lazy registration:** The pattern of registering only the tables referenced in a resolved LogicalPlan is described as correct but no reference implementation exists in the research. Phase 2 planning should include a spike on `TableScan` node extraction from a LogicalPlan.
- **MCP Rust SDK availability:** No Rust-native MCP SDK was evaluated in the research. Phase 4 planning must assess whether to use an existing SDK or implement the MCP JSON-RPC protocol directly. This could affect Phase 4 complexity significantly.
- **Iceberg metadata cold-start benchmark:** The 500ms–2s cold-start estimate is from general Iceberg documentation and S3 latency knowledge. Actual benchmark against a real S3-backed Iceberg table should be done early in Phase 2 to determine whether metadata caching needs to move from Phase 5 to Phase 3.
- **StaticTable vs. catalog flow:** For V1, the research recommends `iceberg::table::StaticTable::from_metadata_file()` for catalog-free operation (simplest path). The tradeoff vs. using a full catalog (REST, SQLite) for V1 should be an explicit decision at Phase 2 kickoff.

## Sources

### Primary (HIGH confidence)
- [DataFusion 51.0.0 on docs.rs](https://docs.rs/crate/datafusion/51.0.0) — target version, published 2025-11-19
- [iceberg 0.8.0 on docs.rs](https://docs.rs/crate/iceberg/latest) — published 2026-01-19
- [iceberg-datafusion 0.8.0 on docs.rs](https://docs.rs/crate/iceberg-datafusion/latest) — datafusion ^51.0 constraint confirmed
- [Apache Iceberg Rust 0.8.0 Release Blog](https://iceberg.apache.org/blog/apache-iceberg-rust-0.8.0-release/) — 144 PRs, 37 contributors
- [iceberg-rust GitHub workspace Cargo.toml](https://github.com/apache/iceberg-rust) — edition 2024, rust-version 1.88, opendal 0.55
- [RUSTSEC-2025-0068](https://rustsec.org/advisories/RUSTSEC-2025-0068.html) — serde_yml unsoundness advisory
- [DataFusion TableProvider API](https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html) — extension point documentation
- [DataFusion Query Optimizer docs](https://datafusion.apache.org/library-user-guide/query-optimizer.html) — AnalyzerRule vs OptimizerRule distinction
- [DataFusion API Health Policy](https://datafusion.apache.org/contributor-guide/api-health.html) — breaking change cadence (6-8 weeks)
- [Aggregation Consistency Errors in Semantic Layers (arxiv)](https://arxiv.org/html/2307.00417) — join fan-out academic research
- [arrow_json writer docs](https://arrow.apache.org/rust/arrow_json/writer/index.html) — RecordBatch to JSON serialization
- [Practical Performance Lessons from Apache DataFusion (Greptime)](https://www.greptime.com/blogs/2025-11-25-datafusion) — production experience
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25) — Model Context Protocol standard

### Secondary (MEDIUM confidence)
- [Wren AI: Semantic SQL with DataFusion](https://www.getwren.ai/post/powering-semantic-sql-for-ai-agents-with-apache-datafusion) — LogicalPlan-as-IR pattern, production case study
- [Wren Engine GitHub](https://github.com/Canner/wren-engine) — open-source reference implementation
- [dbt Docs - Semantic Models](https://docs.getdbt.com/docs/build/semantic-models) — YAML schema conventions, entity/dimension/measure taxonomy
- [Cube Docs - Multitenancy](https://cube.dev/docs/product/configuration/multitenancy) — row-level WHERE injection patterns
- [Apache Iceberg Practical Limitations 2025 (Quesma)](https://quesma.com/blog/apache-iceberg-practical-limitations-2025/) — S3 metadata latency, iceberg-rust feature gaps
- [OpenDAL and Iceberg architecture](https://www.hackintoshrao.com/one-interface-many-backends-the-design-of-iceberg-rusts-universal-storage-layer-with-opendal/) — storage layer design
- [StaticTable docs](https://rust.iceberg.apache.org/api/iceberg/table/struct.StaticTable.html) — catalog-free table reading

### Tertiary (LOW confidence)
- [Multi-tenant data isolation architecture (Medium)](https://medium.com/@justhamade/architecting-secure-multi-tenant-data-isolation-d8f36cb0d25e) — general isolation patterns; needs validation against Quarry's specific DataFusion AnalyzerRule approach

---
*Research completed: 2026-02-24*
*Ready for roadmap: yes*
