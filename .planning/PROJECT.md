# Quarry

## What This Is

A Rust-native local analytics engine that sits between AI agents and lakehouse data. Agents query a YAML-defined semantic layer (metrics, dimensions, entities, joins) instead of raw tables. Quarry resolves those into physical SQL, injects tenant isolation, executes ephemerally via DataFusion against Iceberg tables on S3, and returns rich JSON results. No LLM inside — pure compute.

## Core Value

AI agents can query structured analytics through a semantic layer without knowing the physical schema — one query in, rich data out, tenant-safe, ephemeral.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

(None yet — ship to validate)

### Active

<!-- Current scope. Building toward these. -->

- [ ] YAML-defined semantic layer (metrics, dimensions, entities, joins)
- [ ] Auto-discovery of Iceberg table schemas to bootstrap semantic model
- [ ] Resolution of semantic queries into physical SQL
- [ ] Row-level tenant isolation (WHERE clause injection)
- [ ] Path-level tenant isolation (per-tenant Iceberg paths)
- [ ] Ephemeral per-query execution (spin up DataFusion, query, return, die)
- [ ] Rich JSON output (data + query metadata, no LLM summarization)
- [ ] S3 access via standard AWS credential chain
- [ ] CLI interface for single-query execution
- [ ] Support for small through large data volumes

### Out of Scope

- LLM/AI summarization of results — Quarry is pure compute, consumers handle interpretation
- Long-running server/cluster mode — ephemeral per-query is the design
- Web UI or dashboard — CLI-first, API later
- Multi-query sessions or stateful connections — each invocation is independent
- Write-back to Iceberg tables — read-only analytics
- Real-time streaming — batch/analytical queries only
- User authentication — relies on caller providing tenant context

## Context

Quarry fills the gap between AI agents that need structured data and lakehouse architectures (Iceberg on S3) that require SQL knowledge to query. Today, agents either need hand-crafted SQL or go through heavyweight BI tools. Quarry provides a lightweight, embeddable alternative: define your semantic model in YAML, point it at your Iceberg catalog, and let agents query by concept rather than by table.

The architecture is inspired by TurboPuffer's approach — S3-native, bring compute to data, no shared infrastructure. DataFusion provides the in-process SQL engine. Iceberg provides the table format with schema evolution and partition pruning. The semantic layer is the glue that lets agents think in business terms while Quarry handles the physical mapping.

V1 is a single-query CLI tool. Future versions may expose an HTTP API, support caching, or add query planning optimizations — but the core loop (semantic query in, SQL resolution, DataFusion execution, JSON out) stays the same.

## Constraints

- **Language**: Rust — performance-critical, memory-safe, native binary distribution
- **Execution Model**: Ephemeral per-query — no daemon, no connection pooling, no warm state
- **Storage**: S3-compatible object storage only — no local database, no HDFS
- **Table Format**: Apache Iceberg — no Delta Lake, no Hudi
- **SQL Engine**: DataFusion — in-process, no external query engine dependency
- **Config Format**: YAML — human-readable semantic model definitions
- **Output Format**: JSON — structured for programmatic consumption by AI agents
- **Auth**: Standard AWS credential chain — no custom auth system

## Key Decisions

<!-- Decisions that constrain future work. Add throughout project lifecycle. -->

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| V1 is CLI-only | Simplest interface to validate core loop; HTTP API comes later | — Pending |
| YAML semantic layer + auto-discovery | YAML for human-authored models, auto-discovery from Iceberg schemas to bootstrap | — Pending |
| Both row-level and path-level tenant isolation | Different tenancy models for different use cases; support both from the start | — Pending |
| Ephemeral per-query execution | No state management complexity; aligns with serverless/agent usage patterns | — Pending |
| Rich JSON output (data + metadata) | Agents need both results and context (row count, query time, schema info) | — Pending |
| DataFusion as SQL engine | In-process Rust-native, no external dependencies, good Iceberg support | — Pending |
| No LLM inside Quarry | Keep it pure compute; AI interpretation is the caller's responsibility | — Pending |

---
*Last updated: 2026-02-24 after initialization*
