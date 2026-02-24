# Feature Research

**Domain:** Semantic layer / analytics engine for AI agents (Rust CLI, Iceberg/S3, DataFusion)
**Researched:** 2026-02-24
**Confidence:** MEDIUM-HIGH (well-established domain with clear patterns; AI-agent-specific patterns are newer but converging)

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any semantic layer must have. Without these, the product is not a semantic layer -- it is just a SQL wrapper.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **YAML semantic model definition** | Every major semantic layer (Cube, dbt/MetricFlow, AtScale SML, Looker LookML) uses declarative config. YAML is the standard format. Agents and humans both need a schema-as-code definition that can be version-controlled. | MEDIUM | Core schema: entities (join keys with types: primary, unique, foreign, natural), dimensions (categorical, temporal, boolean), measures (aggregations: sum, count, avg, min, max, count_distinct). Follow dbt/MetricFlow conventions -- they are becoming the de facto standard via OSI. |
| **Metric definitions** | Metrics (KPIs computed from measures) are the entire point of a semantic layer. Without them, users must write raw SQL aggregations. | MEDIUM | Support at minimum: simple metrics (single measure + optional filter), derived metrics (expressions over other metrics, e.g., revenue_per_user = revenue / user_count), cumulative metrics (running totals over time windows), ratio metrics (measure A / measure B). These four types cover the vast majority of business analytics. |
| **Join resolution** | Semantic models span multiple tables. The engine must automatically resolve joins from entity relationships so the user never writes JOIN clauses. | HIGH | This is the hardest table-stakes feature. Must handle: entity-based join path discovery, left joins for fact-to-dimension, multi-hop joins (at least 2 hops, matching MetricFlow), fan-out and chasm trap avoidance. MetricFlow limits to 3-table joins for safety -- a reasonable constraint to adopt. |
| **Semantic-to-SQL compilation** | The core value proposition: translate a semantic query (metric + dimensions + filters) into physical SQL that DataFusion can execute. | HIGH | Must generate correct, optimized SQL with: proper GROUP BY, WHERE clause injection, join ordering, predicate pushdown hints. The generated SQL should be inspectable (audit/debug requirement from every competitor). |
| **Dimension slicing and filtering** | Users must be able to request "revenue by region where country = US" -- slice by dimensions, filter by dimension values. | LOW | Standard WHERE and GROUP BY generation. Support both equality and range filters. Temporal dimensions need special handling (time grain selection). |
| **Time grain selection** | Temporal dimensions must support multiple granularities (day, week, month, quarter, year). Every analytics product supports this. | LOW | Map semantic time dimensions to appropriate DATE_TRUNC or equivalent. The time spine pattern from MetricFlow is worth adopting for cumulative metrics. |
| **Tenant isolation** | Multi-tenancy is a core requirement from PROJECT.md. Any analytics tool serving multiple customers must enforce data boundaries. | MEDIUM | Two modes per PROJECT.md: row-level (WHERE clause injection, like Cube's queryRewrite) and path-level (per-tenant Iceberg table paths). Row-level is table stakes; path-level is a differentiator. |
| **Iceberg table integration** | The engine reads Iceberg tables on S3. Must handle Iceberg metadata, partition pruning, schema evolution. | HIGH | Use iceberg-rust crate for catalog interaction. Must read table metadata, resolve current snapshot, handle partition specs for pruning. Schema evolution (column adds/renames) must not break semantic model mappings. |
| **Rich JSON output** | AI agents consume structured data, not tables. Output must include both result data and metadata (row count, column types, query timing, generated SQL). | LOW | This is straightforward serialization but critical for the AI-agent use case. Include: data rows, column metadata, row count, execution time, generated SQL (for debuggability), semantic model version/hash. |
| **CLI interface** | V1 is CLI-only per PROJECT.md. Must accept a semantic query and return JSON. | LOW | Accept query as JSON/YAML on stdin or as CLI arguments. Return structured JSON to stdout. Stderr for diagnostics. Exit codes for error types. |
| **Query validation** | Before executing, validate that requested metrics, dimensions, and filters are valid against the semantic model. Provide clear error messages. | MEDIUM | Validate: metric exists, dimensions are compatible with requested metrics, filter dimensions exist, time grains are valid for temporal dimensions. Errors must be structured (not just strings) so AI agents can programmatically handle them. |
| **S3 access via AWS credential chain** | Standard credential resolution (env vars, config files, instance profiles, SSO). Not custom auth. | LOW | Use standard Rust AWS SDK credential chain. This is commodity functionality but must work correctly with AssumeRole, SSO, and instance profiles. |

### Differentiators (Competitive Advantage)

Features that set Quarry apart from Cube, dbt, Looker. These exploit the unique positioning: Rust-native, ephemeral, local-first, AI-agent-facing.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Ephemeral per-query execution** | No daemon, no connection pool, no warm state. Spin up, query, return, exit. This is fundamentally different from Cube (long-running server) and dbt (warehouse-dependent). Perfect for serverless, Lambda, agent tool calls. | MEDIUM | The architecture itself is the differentiator. DataFusion spins up in-process, reads Iceberg metadata from S3, executes query, returns result. Cold start performance is critical -- must be sub-second for metadata resolution. Consider caching Iceberg metadata snapshots locally. |
| **Schema auto-discovery from Iceberg** | Bootstrap semantic models automatically by reading Iceberg table metadata. No other CLI tool does this well. Cube requires manual model writing; dbt requires dbt models first. | MEDIUM | Read Iceberg catalog, enumerate tables, extract column names/types/partitioning, generate draft YAML semantic model. This dramatically lowers the barrier to first query. Output should be a starting point that users refine, not a finished model. |
| **Model Context Protocol (MCP) server mode** | MCP is becoming the standard for AI agent tool integration (Anthropic-led, adopted by OpenAI, Google). Exposing Quarry as an MCP server means any MCP-compatible agent can discover and query analytics without custom integration. | MEDIUM | Expose tools: list_metrics, list_dimensions, query, describe_metric, validate_query. Expose resources: semantic model definition, available entities. This is the single most important differentiator for the AI-agent use case. MCP support turns Quarry from "a CLI tool" into "a universal agent analytics capability." |
| **Semantic model introspection API** | AI agents need to discover what they can query. Provide structured metadata about available metrics, dimensions, entities, valid combinations, and descriptions. This is the "meta API" that Cube identifies as essential for agentic analytics. | LOW | Return JSON describing: all metrics with descriptions and compatible dimensions, all dimensions with types and valid values (for categoricals), entity relationships, valid time grains. This is what lets an agent self-serve without a human configuring each query. |
| **Path-level tenant isolation** | Beyond standard row-level WHERE clause injection, support entirely separate Iceberg table paths per tenant. This provides physical data isolation -- stronger security guarantee than row-level filtering. | LOW | Resolve tenant context to different S3 prefixes / Iceberg catalog namespaces. Simple but powerful. Most competitors only offer row-level. Physical isolation is valuable for regulated industries and enterprise customers. |
| **Structured error responses for agents** | Errors as machine-parseable JSON with error codes, suggested fixes, and valid alternatives. When an agent requests a non-existent metric, return the list of valid metrics. When dimensions are incompatible, explain which dimensions work. | LOW | This is table stakes for human tools but a differentiator for agent-facing tools. Most analytics tools return human-readable error strings. Agents need: error_code, message, suggestions (valid metrics, valid dimensions), context (what was requested vs what exists). |
| **Zero-dependency binary distribution** | Single static binary. No JVM, no Python, no Node.js, no Docker. Download and run. This is a Rust advantage that no competitor in this space offers. | LOW | Compile with musl for fully static Linux binaries. Cross-compile for macOS (aarch64 + x86_64). Distribute via GitHub releases, Homebrew, cargo install. The deployment story is "copy binary + YAML files + AWS creds = working analytics." |
| **Query plan explanation** | Return the logical query plan showing how the semantic query was resolved: which tables, which joins, which filters, estimated row counts. | MEDIUM | Useful for debugging and for agents that need to understand query cost before executing. DataFusion has built-in EXPLAIN support. Surface this as part of the response metadata or as a separate dry-run mode. |
| **Semantic model validation CLI** | Validate YAML semantic models without executing queries. Check for: dangling entity references, circular joins, type mismatches, missing required fields. | LOW | Fast feedback loop for model authors. Run as `quarry validate` subcommand. Return structured validation results. This is cheap to build and high value for adoption. |

### Anti-Features (Deliberately NOT Building)

These are features that seem obvious for a semantic layer but are wrong for Quarry's positioning. Building them would dilute focus, increase complexity, and move Quarry toward being a bad version of Cube or Looker instead of being a great version of itself.

| Anti-Feature | Why Requested | Why Problematic | What to Do Instead |
|--------------|---------------|-----------------|-------------------|
| **Web UI / dashboard builder** | Every BI tool has one. Users expect visualization. | Quarry serves AI agents, not human eyeballs. A UI adds massive frontend complexity, ongoing maintenance burden, and moves Quarry into direct competition with Cube, Metabase, Looker -- tools with years of UI investment. The CLI + JSON output IS the interface. | Provide rich JSON that downstream tools (agents, notebooks, custom UIs) can render. Focus on being the best headless analytics engine. |
| **Pre-aggregation / caching layer** | Cube's pre-aggregation is its killer performance feature. Seems essential for fast analytics. | Pre-aggregation requires a persistent cache store, background refresh jobs, cache invalidation logic, and a long-running process to manage it all. This directly contradicts the ephemeral per-query architecture. It adds enormous complexity. | Rely on Iceberg's partition pruning and DataFusion's query optimization for performance. For repeated queries, let the caller (agent framework, application) implement caching at their layer. If performance becomes a bottleneck, consider optional local Parquet cache of Iceberg metadata (not data). |
| **LLM / natural language query interface** | "AI agents use natural language, so the semantic layer should accept natural language." | Quarry is the compute layer BELOW the LLM. The agent's LLM translates natural language to a structured semantic query; Quarry executes it. Putting an LLM inside Quarry creates circular dependency, adds latency, requires API keys/costs, and makes results non-deterministic. | Accept structured semantic queries (JSON/YAML). Let the agent's own LLM handle natural language translation using Quarry's introspection API to understand available metrics/dimensions. The MCP server mode enables this pattern naturally. |
| **Long-running server / daemon mode** | Enables connection pooling, warm caches, lower latency for repeated queries. | Contradicts the core ephemeral architecture. A daemon requires health checks, graceful shutdown, state management, port binding, process supervision. It changes the operational model completely. | Keep V1 strictly ephemeral. If latency becomes an issue, explore V2 HTTP API mode as a separate execution model -- but even then, prefer stateless request-response over persistent connections. |
| **Write-back / data mutation** | Users want to write computed results back to Iceberg tables. | Read-only analytics is a deliberate constraint. Write-back requires transaction management, conflict resolution, schema management, and dramatically increases the blast radius of bugs. | Quarry outputs JSON. If the caller needs to persist results, they write to their own storage. Keep Quarry's Iceberg access strictly read-only. |
| **Multi-query sessions / stateful connections** | Allow agents to build on previous query results, like a notebook session. | State management across queries adds enormous complexity: result caching, reference resolution, garbage collection, session timeouts. It breaks the ephemeral model. | Each query is self-contained. If an agent needs to combine results from multiple queries, it does that in its own memory. Quarry's structured JSON output makes this straightforward for agents. |
| **Real-time / streaming data** | Support for Kafka, real-time event streams alongside batch Iceberg data. | Streaming requires fundamentally different infrastructure (message consumers, windowing, watermarks). Iceberg is a batch table format. Mixing paradigms creates complexity without clear value for the target use case. | Focus on batch/analytical queries over Iceberg snapshots. Iceberg's snapshot isolation already provides "as-of" consistency. If users need fresher data, they should update their Iceberg tables more frequently. |
| **User authentication / authorization system** | Every enterprise tool needs auth. | Quarry runs locally or in the caller's infrastructure. The caller provides tenant context; Quarry enforces isolation based on that context. Building a full auth system (users, roles, tokens, RBAC) is massive scope for no value in the agent use case. | Accept tenant context as input (CLI flag, environment variable, query parameter). Trust the caller to authenticate users upstream. Quarry enforces data isolation, not user identity. |
| **SQL dialect compatibility layer** | Support MySQL, Postgres, Snowflake SQL dialects for the query interface. | Quarry's input is semantic queries, not SQL. The output is DataFusion SQL (Arrow-compatible). Adding dialect support means maintaining parsers and transpilers for SQL variants that Quarry does not need. | The semantic query interface abstracts away SQL entirely. DataFusion handles the execution SQL. If users need to write raw SQL, they should use DataFusion directly. |
| **Data quality / observability features** | Freshness checks, row count monitoring, schema drift detection. | These are data platform features, not semantic layer features. Building them creates scope creep toward a full data platform. | Expose metadata about the Iceberg snapshot being queried (snapshot ID, timestamp, row count) so callers can make their own freshness decisions. Let dedicated tools (Monte Carlo, Great Expectations, dbt tests) handle data quality. |

## Feature Dependencies

```
[YAML Semantic Model Definition]
    |
    +--requires--> [Iceberg Table Integration] (physical schema to map against)
    |
    +--enables--> [Metric Definitions] (metrics reference measures in semantic models)
    |                |
    |                +--enables--> [Time Grain Selection] (temporal metrics need grain support)
    |                |
    |                +--enables--> [Cumulative / Derived Metrics] (build on simple metrics)
    |
    +--enables--> [Join Resolution] (entities in semantic models define join paths)
    |
    +--enables--> [Query Validation] (validates against semantic model schema)
    |
    +--enables--> [Semantic Model Introspection] (exposes model metadata)
    |                |
    |                +--enables--> [MCP Server Mode] (introspection is core MCP capability)
    |
    +--enables--> [Semantic Model Validation CLI] (lint/check YAML correctness)

[Semantic-to-SQL Compilation]
    |
    +--requires--> [YAML Semantic Model Definition]
    +--requires--> [Join Resolution]
    +--requires--> [Dimension Slicing and Filtering]
    +--requires--> [Metric Definitions]
    |
    +--enables--> [Query Plan Explanation] (explain what SQL was generated)
    +--enables--> [Rich JSON Output] (wraps SQL results + metadata)

[Tenant Isolation]
    |
    +--row-level requires--> [Semantic-to-SQL Compilation] (WHERE injection)
    +--path-level requires--> [Iceberg Table Integration] (path resolution)

[Schema Auto-Discovery]
    +--requires--> [Iceberg Table Integration]
    +--generates--> [YAML Semantic Model Definition] (draft models)

[CLI Interface]
    +--requires--> [Semantic-to-SQL Compilation]
    +--requires--> [Rich JSON Output]
    +--requires--> [Query Validation]

[MCP Server Mode]
    +--requires--> [CLI Interface] (same core engine)
    +--requires--> [Semantic Model Introspection]
    +--requires--> [Query Validation]
    +--requires--> [Structured Error Responses]
```

### Dependency Notes

- **Join Resolution requires Semantic Model Definition:** Join paths are defined by entity relationships in the YAML schema. Without entity type annotations (primary, foreign, etc.), the engine cannot determine valid join paths.
- **Semantic-to-SQL Compilation is the critical path:** Nearly everything depends on this working correctly. It pulls together model definitions, joins, filters, and metrics into executable SQL.
- **MCP Server Mode requires Introspection + Validation:** An MCP server must expose what can be queried (introspection) and validate requests before execution. Building MCP without these foundations would produce a fragile integration.
- **Schema Auto-Discovery enhances but does not replace Model Definition:** Auto-discovery generates draft YAML; humans (or agents) refine it. The semantic model definition format must exist first.
- **Tenant Isolation has two independent paths:** Row-level and path-level isolation can be implemented independently. Row-level (WHERE injection) depends on SQL compilation; path-level depends on Iceberg integration.

## MVP Definition

### Launch With (v1)

Minimum viable product -- enough to validate that AI agents can query Iceberg data through a semantic layer without writing SQL.

- [ ] **YAML semantic model definition** -- entities, dimensions, measures, metrics (simple + derived at minimum)
- [ ] **Iceberg table integration** -- read table metadata, resolve snapshots, execute via DataFusion
- [ ] **Semantic-to-SQL compilation** -- translate semantic queries into DataFusion SQL
- [ ] **Join resolution** -- automatic join path discovery from entity relationships
- [ ] **Dimension slicing and filtering** -- WHERE and GROUP BY generation
- [ ] **Time grain selection** -- day/week/month/quarter/year for temporal dimensions
- [ ] **Row-level tenant isolation** -- WHERE clause injection based on tenant context
- [ ] **Query validation** -- validate requests against semantic model before execution
- [ ] **Rich JSON output** -- data + metadata (row count, timing, generated SQL)
- [ ] **CLI interface** -- `quarry query` accepts semantic query, returns JSON
- [ ] **Semantic model validation** -- `quarry validate` checks YAML correctness

### Add After Validation (v1.x)

Features to add once the core query loop works and early users provide feedback.

- [ ] **Schema auto-discovery** -- `quarry discover` generates draft YAML from Iceberg metadata
- [ ] **MCP server mode** -- expose Quarry as an MCP tool server for AI agents
- [ ] **Semantic model introspection** -- `quarry describe` returns available metrics/dimensions as JSON
- [ ] **Path-level tenant isolation** -- per-tenant Iceberg paths for physical data separation
- [ ] **Cumulative and ratio metrics** -- time-window aggregations and cross-metric calculations
- [ ] **Query plan explanation** -- dry-run mode showing resolved SQL and join plan
- [ ] **Structured error responses** -- machine-parseable errors with suggestions for agents

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **HTTP API** -- stateless REST endpoint wrapping the core engine, for non-CLI consumers
- [ ] **OSI / SML compatibility** -- import/export semantic models in Open Semantic Interchange format
- [ ] **Iceberg metadata caching** -- local cache of catalog metadata to reduce cold-start latency
- [ ] **Partition-aware query optimization** -- leverage Iceberg partition specs for predicate pushdown
- [ ] **Multiple Iceberg catalog support** -- REST catalog, AWS Glue, Hive metastore
- [ ] **Metric lineage** -- trace which tables/columns contribute to each metric

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| YAML semantic model definition | HIGH | MEDIUM | P1 |
| Iceberg table integration | HIGH | HIGH | P1 |
| Semantic-to-SQL compilation | HIGH | HIGH | P1 |
| Join resolution | HIGH | HIGH | P1 |
| Dimension slicing + filtering | HIGH | LOW | P1 |
| Time grain selection | HIGH | LOW | P1 |
| Row-level tenant isolation | HIGH | LOW | P1 |
| Query validation | HIGH | MEDIUM | P1 |
| Rich JSON output | HIGH | LOW | P1 |
| CLI interface | HIGH | LOW | P1 |
| Semantic model validation | MEDIUM | LOW | P1 |
| Schema auto-discovery | HIGH | MEDIUM | P2 |
| MCP server mode | HIGH | MEDIUM | P2 |
| Semantic model introspection | HIGH | LOW | P2 |
| Path-level tenant isolation | MEDIUM | LOW | P2 |
| Cumulative + ratio metrics | MEDIUM | MEDIUM | P2 |
| Query plan explanation | MEDIUM | LOW | P2 |
| Structured error responses | MEDIUM | LOW | P2 |
| HTTP API | MEDIUM | MEDIUM | P3 |
| OSI/SML compatibility | LOW | HIGH | P3 |
| Iceberg metadata caching | MEDIUM | MEDIUM | P3 |
| Partition-aware optimization | MEDIUM | HIGH | P3 |
| Multiple catalog support | LOW | MEDIUM | P3 |
| Metric lineage | LOW | MEDIUM | P3 |

**Priority key:**
- P1: Must have for launch -- the core query loop
- P2: Should have -- what makes Quarry valuable specifically for AI agents
- P3: Nice to have -- optimization and ecosystem features for later

## Competitor Feature Analysis

| Feature | Cube | dbt/MetricFlow | Looker | Quarry Approach |
|---------|------|----------------|--------|-----------------|
| **Model definition format** | JavaScript/YAML data models | YAML semantic models | LookML (custom DSL) | YAML -- follow dbt/MetricFlow conventions as they become OSI standard |
| **Metric types** | Measures with rollup types | Simple, derived, cumulative, ratio | Measures + table calculations | Simple, derived, cumulative, ratio (match MetricFlow taxonomy) |
| **Join handling** | Explicit join definitions in model | Entity-based automatic resolution | Explore-based joins | Entity-based automatic resolution (MetricFlow approach -- more agent-friendly) |
| **Query interface** | REST, GraphQL, SQL, AI API | JDBC, GraphQL, Python SDK | Explore UI, SQL Runner, API | CLI (v1), MCP server (v1.x), HTTP API (v2) |
| **Multi-tenancy** | Security context + query rewrite | Warehouse-level (not built-in) | User attributes + access filters | Row-level WHERE injection + path-level Iceberg isolation |
| **Caching** | Pre-aggregations in Cube Store | Relies on warehouse cache | PDTs (Persistent Derived Tables) | None (ephemeral) -- rely on Iceberg partition pruning + DataFusion optimization |
| **Execution engine** | Pushes SQL to source database | Pushes SQL to warehouse | Pushes SQL to database | In-process DataFusion -- brings compute to data on S3 |
| **Deployment** | Docker/cloud service | dbt Cloud or CLI | Google Cloud SaaS | Single binary -- zero infrastructure |
| **AI/Agent support** | AI API, MCP server, RAG | MetricFlow open source + MCP | Gemini integration | MCP server, structured JSON, introspection API -- agent-first design |
| **Schema discovery** | Manual model creation | From dbt models (requires dbt) | LookML generator from DB | Auto-discover from Iceberg metadata -- no dbt dependency |
| **Cost model** | Cloud pricing or self-hosted | dbt Cloud pricing | Google Cloud pricing | Free/open source -- single binary, no infrastructure cost |

## Sources

- [Cube.dev - Agentic Analytics Platform](https://cube.dev/) -- Cube's core features and AI capabilities
- [Cube Blog - Semantic Layer and AI](https://cube.dev/blog/semantic-layer-and-ai-the-future-of-data-querying-with-natural-language) -- AI API features, RAG integration
- [Cube Blog - Universal Semantic Layer Capabilities](https://cube.dev/blog/universal-semantic-layer-capabilities-integrations-and-enterprise-benefits) -- Feature enumeration
- [Cube Docs - Multitenancy](https://cube.dev/docs/product/configuration/multitenancy) -- Multi-tenant patterns
- [dbt Docs - About MetricFlow](https://docs.getdbt.com/docs/build/about-metricflow) -- MetricFlow architecture and features
- [dbt Docs - Semantic Models](https://docs.getdbt.com/docs/build/semantic-models) -- YAML schema, entities, dimensions, measures
- [dbt Labs - Open Source MetricFlow](https://www.getdbt.com/blog/open-source-metricflow-governed-metrics) -- OSI initiative, Apache 2.0 licensing
- [Typedef.ai - Semantic Layer Architectures Explained](https://www.typedef.ai/resources/semantic-layer-architectures-explained-warehouse-native-vs-dbt-vs-cube) -- Architecture comparison
- [Typedef.ai - MetricFlow vs Snowflake vs Databricks](https://www.typedef.ai/resources/semantic-layer-metricflow-vs-snowflake-vs-databricks) -- Feature comparison
- [AtScale - Introduction to SML](https://www.atscale.com/blog/introduction-to-sml-a-standard-semantic-modeling-language/) -- Open Semantic Interchange, SML specification
- [AtScale - Semantic Layer 2025 in Review](https://www.atscale.com/blog/semantic-layer-2025-in-review/) -- Industry trends
- [ThoughtSpot - Agentic Semantic Layer](https://www.thoughtspot.com/blog/introducing-the-agentic-semantic-layer) -- Agentic features, AI-specific capabilities
- [Salesforce - Open Semantic Layer](https://www.salesforce.com/blog/agentic-future-demands-open-semantic-layer/) -- OSI initiative
- [Definite - Semantic Layer AI Analytics 2026](https://www.definite.app/blog/semantic-layer-ai-analytics) -- 2026 trends
- [Davidsj Substack - Semantic Layers Buyer's Guide](https://davidsj.substack.com/p/semantic-layers-a-buyers-guide) -- Feature comparison across vendors
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25) -- Model Context Protocol standard
- [Cube Blog - MCP Integration](https://cube.dev/blog/unlocking-universal-data-access-for-ai-with-anthropics-model-context) -- MCP + semantic layer pattern
- [MotherDuck - Semantic Layer with DuckDB](https://motherduck.com/blog/semantic-layer-duckdb-tutorial/) -- Lightweight semantic layer patterns

---
*Feature research for: Semantic layer / analytics engine for AI agents*
*Researched: 2026-02-24*
