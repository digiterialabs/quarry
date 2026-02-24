# Requirements: Quarry

**Defined:** 2026-02-24
**Core Value:** AI agents can query structured analytics through a semantic layer without knowing the physical schema — one query in, rich data out, tenant-safe, ephemeral.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Semantic Model

- [ ] **SMDL-01**: Developer can define entities with typed join keys (primary, foreign) in YAML
- [ ] **SMDL-02**: Developer can define dimensions (categorical, temporal, boolean) on entities in YAML
- [ ] **SMDL-03**: Developer can define measures (sum, count, avg, min, max, count_distinct) on entities in YAML
- [ ] **SMDL-04**: Developer can define simple metrics (single measure + optional filter) in YAML
- [ ] **SMDL-05**: Developer can define derived metrics (expressions over other metrics) in YAML
- [ ] **SMDL-06**: Developer can define cumulative metrics (running totals over time windows) in YAML
- [ ] **SMDL-07**: Developer can define ratio metrics (measure A / measure B) in YAML
- [ ] **SMDL-08**: Developer can validate YAML semantic model correctness via `quarry validate` without executing queries
- [ ] **SMDL-09**: Developer can auto-discover Iceberg table schemas and generate draft YAML semantic models via `quarry discover`

### Query Resolution

- [ ] **QRES-01**: Agent can submit a semantic query specifying metrics, dimensions, and filters
- [ ] **QRES-02**: Engine resolves semantic queries into DataFusion LogicalPlans (not SQL strings)
- [ ] **QRES-03**: Engine automatically discovers and resolves join paths from entity relationships
- [ ] **QRES-04**: Engine generates correct GROUP BY clauses from requested dimensions
- [ ] **QRES-05**: Engine generates correct WHERE clauses from query filters
- [ ] **QRES-06**: Engine supports time grain selection (day, week, month, quarter, year) for temporal dimensions
- [ ] **QRES-07**: Engine validates metric/dimension/filter combinations against the semantic model before execution
- [ ] **QRES-08**: Engine returns structured validation errors with valid alternatives when a query is invalid
- [ ] **QRES-09**: Agent can request query plan explanation (dry-run mode) showing resolved SQL and join plan without executing

### Data Access

- [ ] **DATA-01**: Engine reads Iceberg table metadata and resolves current snapshots from S3
- [ ] **DATA-02**: Engine executes queries via DataFusion against Iceberg tables on S3
- [ ] **DATA-03**: Engine leverages Iceberg partition pruning for query performance
- [ ] **DATA-04**: Engine uses standard AWS credential chain for S3 access (env vars, config files, instance profiles, SSO)

### Tenant Isolation

- [ ] **TNNT-01**: Engine injects row-level tenant isolation as DataFusion AnalyzerRule on the LogicalPlan (not SQL string manipulation)
- [ ] **TNNT-02**: Tenant isolation applies to all query patterns including CTEs, subqueries, and window functions
- [ ] **TNNT-03**: Tenant context is provided as CLI flag or environment variable by the caller

### Output & Interface

- [ ] **OUTP-01**: Engine returns rich JSON output containing data rows, column metadata, row count, execution time, and generated SQL
- [ ] **OUTP-02**: CLI accepts semantic query via JSON on stdin or as CLI arguments and returns JSON to stdout
- [ ] **OUTP-03**: CLI uses exit codes to distinguish error types (invalid query, execution failure, config error)
- [ ] **OUTP-04**: Engine returns structured error responses with error codes, messages, and suggestions for agents

### AI Agent Integration

- [ ] **AGNT-01**: Engine exposes MCP server mode for AI agent tool integration (list_metrics, list_dimensions, query, describe_metric, validate_query)
- [ ] **AGNT-02**: Agent can introspect available metrics with descriptions and compatible dimensions via `quarry describe`
- [ ] **AGNT-03**: Agent can introspect available dimensions with types and valid time grains
- [ ] **AGNT-04**: Agent can discover valid metric/dimension combinations without trial and error

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Enhanced Tenancy

- **TNNT-04**: Engine supports path-level tenant isolation via per-tenant Iceberg S3 prefixes for physical data separation

### Performance

- **PERF-01**: Engine caches Iceberg metadata locally to reduce cold-start latency
- **PERF-02**: Engine supports partition-aware query optimization leveraging Iceberg partition specs

### Distribution

- **DIST-01**: Engine provides HTTP API mode (stateless REST wrapper) for non-CLI consumers
- **DIST-02**: Engine supports OSI/SML format for semantic model import/export
- **DIST-03**: Engine supports multiple Iceberg catalog types (REST, Glue, Hive metastore)

### Observability

- **OBSV-01**: Engine provides metric lineage tracing (which tables/columns contribute to each metric)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Web UI / dashboard builder | Quarry serves AI agents, not human eyeballs. Focus on headless JSON output. |
| Pre-aggregation / caching layer | Contradicts ephemeral per-query architecture. Relies on Iceberg partition pruning. |
| LLM / natural language query | Quarry is compute below the LLM. Agents translate NL to structured queries. |
| Long-running server / daemon | Core architecture is ephemeral. V2 HTTP API is stateless request-response. |
| Write-back / data mutation | Read-only analytics. Callers persist results in their own storage. |
| Multi-query sessions | Each query is self-contained. Agents combine results in their own memory. |
| Real-time / streaming data | Iceberg is batch. Focus on analytical queries over snapshots. |
| User authentication system | Caller provides tenant context. Quarry enforces isolation, not identity. |
| SQL dialect compatibility | Input is semantic queries, not SQL. DataFusion handles execution SQL. |
| Data quality / observability | Expose snapshot metadata; let dedicated tools handle data quality. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| _(populated by roadmapper)_ | | |

**Coverage:**
- v1 requirements: 27 total
- Mapped to phases: 0
- Unmapped: 27

---
*Requirements defined: 2026-02-24*
*Last updated: 2026-02-24 after initial definition*
