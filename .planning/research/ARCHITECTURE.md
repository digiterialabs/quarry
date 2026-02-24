# Architecture Research

**Domain:** Rust-native local analytics engine (semantic layer + DataFusion + Iceberg)
**Researched:** 2026-02-24
**Confidence:** HIGH (DataFusion architecture), MEDIUM (iceberg-rust integration), MEDIUM (semantic layer design)

## System Overview

```
                          CLI Input
                     (metric query + tenant ctx)
                              |
                              v
┌─────────────────────────────────────────────────────────────────┐
│                      INPUT LAYER                                │
│  ┌──────────────┐  ┌──────────────────┐  ┌───────────────────┐  │
│  │ CLI Parser   │  │ Config Loader    │  │ Credential Chain  │  │
│  │ (clap)       │  │ (YAML semantic   │  │ (AWS SDK default  │  │
│  │              │  │  model files)    │  │  credential chain)│  │
│  └──────┬───────┘  └────────┬─────────┘  └─────────┬─────────┘  │
│         │                   │                      │            │
├─────────┴───────────────────┴──────────────────────┴────────────┤
│                   SEMANTIC RESOLUTION LAYER                      │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  Semantic Model Registry                  │   │
│  │  (metrics, dimensions, entities, joins, relationships)   │   │
│  └──────────────────────────┬───────────────────────────────┘   │
│                             │                                   │
│  ┌──────────────────────────┴───────────────────────────────┐   │
│  │                  Query Resolver                           │   │
│  │  semantic query --> LogicalPlan (metrics->measures->SQL)  │   │
│  └──────────────────────────┬───────────────────────────────┘   │
│                             │                                   │
├─────────────────────────────┴───────────────────────────────────┤
│                   TENANT ISOLATION LAYER                         │
│  ┌─────────────────────┐  ┌────────────────────────────────┐   │
│  │ Row-Level Injection │  │ Path-Level Iceberg Filtering   │   │
│  │ (AnalyzerRule:      │  │ (TableProvider wrapper:         │   │
│  │  add WHERE clause)  │  │  scope S3 prefix per tenant)   │   │
│  └─────────┬───────────┘  └──────────────┬─────────────────┘   │
│            │                             │                      │
├────────────┴─────────────────────────────┴──────────────────────┤
│                   EXECUTION LAYER                                │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              DataFusion SessionContext                    │   │
│  │  ┌────────────────┐  ┌──────────────┐  ┌──────────────┐ │   │
│  │  │ Analyzer       │  │ Optimizer    │  │ Physical     │ │   │
│  │  │ (type coercion │  │ (filter push │  │ Planner      │ │   │
│  │  │  + tenant      │  │  down, prune │  │              │ │   │
│  │  │  injection)    │  │  columns)    │  │              │ │   │
│  │  └───────┬────────┘  └──────┬───────┘  └──────┬───────┘ │   │
│  │          v                  v                  v         │   │
│  │  ┌──────────────────────────────────────────────────┐   │   │
│  │  │           IcebergTableProvider                    │   │   │
│  │  │  (iceberg-datafusion crate: partition pruning,   │   │   │
│  │  │   predicate pushdown into Parquet, S3 FileIO)    │   │   │
│  │  └──────────────────────┬───────────────────────────┘   │   │
│  └─────────────────────────┼───────────────────────────────┘   │
│                            │                                    │
├────────────────────────────┴────────────────────────────────────┤
│                   OUTPUT LAYER                                   │
│  ┌────────────────────┐  ┌─────────────────────────────────┐   │
│  │ RecordBatch Stream │→ │ JSON Serializer                 │   │
│  │ (Arrow columnar)   │  │ (arrow_json + metadata envelope)│   │
│  └────────────────────┘  └──────────────┬──────────────────┘   │
│                                         │                      │
└─────────────────────────────────────────┴──────────────────────┘
                              |
                              v
                      JSON to stdout
              (data rows + query metadata)
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **CLI Parser** | Parse query arguments (metric name, dimensions, filters, tenant ID, config path) | `clap` crate with derive macros |
| **Config Loader** | Read and validate YAML semantic model definitions; build in-memory model registry | `serde` + `serde-saphyr` (pure Rust YAML, no unsafe libyaml) |
| **Credential Chain** | Resolve AWS credentials for S3 access | AWS SDK default credential chain via `object_store` crate's S3 support |
| **Semantic Model Registry** | Hold parsed semantic model in memory: metrics, dimensions, entities, join paths, measures | Custom structs with validation; loaded once per invocation |
| **Query Resolver** | Transform a semantic query (metric + dimensions + filters) into a DataFusion `LogicalPlan` | Custom code: metric -> measures -> aggregation expressions -> join paths -> LogicalPlan |
| **Tenant Isolation (Row-Level)** | Inject `WHERE tenant_id = ?` predicates into every `LogicalPlan` before execution | Custom `AnalyzerRule` registered with `SessionState` |
| **Tenant Isolation (Path-Level)** | Scope Iceberg table access to tenant-specific S3 prefixes | Wrapper around `IcebergTableProvider` that constrains table location |
| **DataFusion Engine** | Full query execution: analysis, optimization, physical planning, execution | `SessionContext` with custom analyzer rules + registered Iceberg table providers |
| **Iceberg Integration** | Read Iceberg table metadata, apply partition pruning, read Parquet from S3 | `iceberg-datafusion` crate: `IcebergStaticTableProvider` for read-only snapshots |
| **JSON Serializer** | Convert Arrow `RecordBatch` stream to JSON with metadata envelope | `arrow_json::ArrayWriter` + custom metadata struct serialized via `serde_json` |

## Recommended Project Structure

```
quarry/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── quarry-cli/             # Binary crate: CLI entry point
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs         # Arg parsing, orchestration, exit codes
│   │
│   ├── quarry-core/            # Library crate: semantic layer + query resolution
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model/          # Semantic model types
│   │       │   ├── mod.rs
│   │       │   ├── metric.rs   # Metric definitions (sum, avg, count, derived)
│   │       │   ├── dimension.rs # Dimension definitions (categorical, temporal)
│   │       │   ├── entity.rs   # Entity definitions (join keys, relationships)
│   │       │   ├── join.rs     # Join path resolution
│   │       │   └── validate.rs # Model validation rules
│   │       ├── config/         # YAML loading + validation
│   │       │   ├── mod.rs
│   │       │   ├── loader.rs   # File reading, multi-file merging
│   │       │   └── schema.rs   # serde deserialization types
│   │       ├── resolve/        # Semantic query -> LogicalPlan
│   │       │   ├── mod.rs
│   │       │   ├── query.rs    # Input query representation
│   │       │   ├── planner.rs  # Metric resolution -> LogicalPlan
│   │       │   └── join_graph.rs # Join path finder between entities
│   │       └── tenant/         # Tenant isolation logic
│   │           ├── mod.rs
│   │           ├── context.rs  # TenantContext (ID, isolation mode)
│   │           ├── row_filter.rs    # AnalyzerRule for WHERE injection
│   │           └── path_scope.rs    # S3 path scoping for Iceberg
│   │
│   ├── quarry-exec/            # Library crate: DataFusion + Iceberg execution
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs       # SessionContext setup, rule registration
│   │       ├── catalog.rs      # Iceberg catalog integration
│   │       ├── table.rs        # TableProvider wrappers (tenant-scoped)
│   │       └── result.rs       # RecordBatch -> JSON + metadata
│   │
│   └── quarry-test/            # Test utilities + integration test fixtures
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs          # Shared test helpers, fixture builders
│
├── models/                     # Example YAML semantic models
│   └── example/
│       ├── metrics.yml
│       ├── dimensions.yml
│       └── entities.yml
│
└── tests/                      # Integration tests
    ├── resolve_test.rs
    ├── tenant_test.rs
    └── e2e_test.rs
```

### Structure Rationale

- **quarry-cli/:** Thin binary. Only argument parsing and orchestration. No business logic. This keeps the library crates testable without spawning a CLI process. Future HTTP API entry point would be a sibling crate (quarry-server), not a modification of quarry-cli.
- **quarry-core/:** The heart of Quarry. Pure logic with no I/O dependencies on DataFusion or Iceberg. Semantic model types, YAML parsing, query resolution, and tenant isolation rules live here. Depends on `datafusion-expr` for LogicalPlan construction but NOT on the full `datafusion` crate -- this keeps it fast to compile and testable with mock plans.
- **quarry-exec/:** I/O boundary. Owns the DataFusion `SessionContext`, Iceberg catalog connections, S3 access, and result serialization. This is where trait objects meet real implementations. Depends on `datafusion`, `iceberg-datafusion`, `arrow-json`.
- **quarry-test/:** Shared test utilities. Prevents test helper duplication across crates. Provides fixture builders for semantic models, mock catalogs, and in-memory table providers.
- **Three-crate core (cli/core/exec):** Follows the clean architecture principle where core has no infrastructure dependencies. This means quarry-core can be compiled and tested in isolation (fast iteration), and quarry-exec can be swapped or extended without touching semantic logic.

## Architectural Patterns

### Pattern 1: Pipeline Architecture (Query Lifecycle)

**What:** The entire query lifecycle is a linear pipeline of transformations, each with a well-defined input and output type. No component reaches back to a prior stage.

**When to use:** Always -- this is the core architectural pattern for Quarry.

**Trade-offs:** Simple to reason about and test in isolation. Slightly less flexible than a graph-based architecture, but Quarry's ephemeral nature (one query, one invocation) means there is no need for shared mutable state or feedback loops.

**Pipeline stages:**

```rust
// Conceptual pipeline -- each stage is a function with typed input/output
fn execute_query(args: CliArgs) -> Result<JsonOutput> {
    // Stage 1: Load config
    let model = config::load_semantic_model(&args.config_path)?;

    // Stage 2: Parse query
    let query = resolve::parse_query(&args.query, &model)?;

    // Stage 3: Resolve to LogicalPlan
    let plan = resolve::to_logical_plan(&query, &model)?;

    // Stage 4: Build execution context with tenant isolation
    let ctx = exec::build_context(
        &model,
        &args.tenant_context,
        &args.catalog_config,
    ).await?;

    // Stage 5: Execute
    let batches = exec::execute(ctx, plan).await?;

    // Stage 6: Serialize
    let output = result::to_json(batches, &query)?;

    Ok(output)
}
```

### Pattern 2: Semantic Resolution via LogicalPlan IR (Inspired by Wren Engine)

**What:** Rather than generating SQL strings from semantic definitions, build DataFusion `LogicalPlan` trees directly. This avoids SQL injection risks, dialect issues, and string manipulation bugs. The LogicalPlan serves as the intermediate representation (IR) between semantic concepts and physical execution.

**When to use:** For all metric-to-SQL resolution. This is the core translation mechanism.

**Trade-offs:** Requires understanding DataFusion's `LogicalPlan` API (steeper learning curve than string concatenation), but provides type safety, automatic optimization (DataFusion's built-in optimizer works on the plan), and correctness guarantees.

**How it works:**

```
Semantic Query: "revenue by region, filtered to Q4 2025"
    |
    v
Metric Lookup: revenue = SUM(orders.amount) WHERE orders.status = 'completed'
    |
    v
Dimension Lookup: region = customers.region (requires join: orders -> customers)
    |
    v
Join Path Resolution: orders JOIN customers ON orders.customer_id = customers.id
    |
    v
LogicalPlan:
  Projection [SUM(orders.amount) AS revenue, customers.region]
    Aggregate [customers.region] [SUM(orders.amount)]
      Filter [orders.status = 'completed' AND orders.order_date >= '2025-10-01']
        Join [orders.customer_id = customers.id]
          TableScan [orders]
          TableScan [customers]
```

**Why not SQL string generation:** Wren AI moved from Java-based SQL string manipulation to Rust LogicalPlan construction specifically because the IR approach eliminated entire classes of bugs around SQL dialect handling, escaping, and optimization. Quarry benefits from the same insight -- since DataFusion is both our planner and executor, generating LogicalPlan directly means zero SQL parsing overhead and automatic access to DataFusion's 30+ optimization rules.

### Pattern 3: Tenant Isolation via AnalyzerRule

**What:** Implement row-level tenant isolation as a DataFusion `AnalyzerRule` that walks every `LogicalPlan` and injects `WHERE tenant_id = ?` predicates on every `TableScan` node. This runs in the analysis phase, before optimization, so DataFusion's filter pushdown automatically pushes these predicates into the Iceberg table scan (partition pruning + Parquet predicate pushdown).

**When to use:** For row-level tenant isolation where all tenants share the same physical tables.

**Trade-offs:** Elegant and automatic -- every query gets isolation regardless of how the plan was constructed. Requires care to avoid double-injection if a plan is re-analyzed. Must ensure the tenant_id column exists on all relevant tables.

**How it works:**

```rust
// Conceptual AnalyzerRule for tenant isolation
struct TenantIsolationRule {
    tenant_id: String,
    tenant_column: String, // e.g., "tenant_id"
}

// The rule walks the LogicalPlan tree and wraps every TableScan
// with a Filter node: Filter(tenant_column = tenant_id, TableScan)
// DataFusion's optimizer then pushes this filter down automatically.
```

**Path-level isolation is separate:** For tenants with dedicated S3 paths (e.g., `s3://lakehouse/tenant_a/`), the isolation happens at the `TableProvider` level. A wrapping provider overrides the Iceberg table location to point at the tenant-specific prefix. This is configured during `SessionContext` setup, not via plan rewriting.

### Pattern 4: Ephemeral Context (No Warm State)

**What:** Every query invocation creates a fresh `SessionContext`, loads the semantic model, registers table providers, executes, serializes, and exits. No caching, no connection pooling, no state carried between invocations.

**When to use:** Always in V1. This is a design constraint, not a pattern to be debated.

**Trade-offs:** Simplicity and correctness (no stale state, no cache invalidation). Cold start cost for every query: YAML parsing (~microseconds), Iceberg metadata fetch (~100ms-1s depending on catalog), S3 connection setup (~10-50ms). For the AI agent use case where queries are seconds apart, this overhead is acceptable. Future optimization (V2+) could add warm-start modes without changing the architecture -- just cache the `SessionContext` across invocations.

## Data Flow

### Complete Query Lifecycle

```
[1] CLI invocation
    quarry query --metric revenue --dimensions region,quarter \
                 --filter "year=2025" --tenant acme \
                 --config ./models/ --catalog rest://catalog:8181
    |
    v
[2] Argument parsing (clap)
    CliArgs { metric: "revenue", dimensions: ["region", "quarter"],
              filters: ["year=2025"], tenant: "acme", ... }
    |
    v
[3] Config loading (serde + serde-saphyr)
    Read YAML files from --config path
    Parse into SemanticModel { metrics, dimensions, entities, joins }
    Validate: all referenced columns exist, join paths are valid,
              metric expressions parse correctly
    |
    v
[4] Query resolution (quarry-core/resolve)
    Input:  SemanticQuery { metric: "revenue", dims: [...], filters: [...] }
    Lookup: Metric "revenue" = SUM(orders.amount) WHERE status='completed'
    Lookup: Dimension "region" = customers.region (entity: customers)
    Lookup: Dimension "quarter" = QUARTER(orders.order_date) (entity: orders)
    Resolve join path: orders <-> customers via orders.customer_id
    Output: LogicalPlan tree (Projection -> Aggregate -> Filter -> Join -> TableScans)
    |
    v
[5] Execution context setup (quarry-exec/engine)
    Create SessionContext
    Configure S3 ObjectStore (AWS credentials from environment)
    Connect to Iceberg catalog (REST, Glue, or filesystem)
    Register IcebergStaticTableProvider for each referenced table
    (If path-level tenancy: scope table locations to tenant prefix)
    Register TenantIsolationRule as AnalyzerRule
    (If row-level tenancy: rule will inject WHERE tenant_id='acme')
    |
    v
[6] Plan execution (DataFusion pipeline)
    LogicalPlan
      -> Analyzer (type coercion + tenant WHERE injection)
      -> Optimizer (filter pushdown into Iceberg scans, projection pruning)
      -> Physical Planner (choose hash join vs merge join, etc.)
      -> Physical Optimizer (repartition, pipeline breaking)
      -> Execution (stream of Arrow RecordBatches from Iceberg/Parquet/S3)
    |
    v
[7] Result serialization (quarry-exec/result)
    Input:  Vec<RecordBatch> + query metadata
    Convert RecordBatches to JSON rows via arrow_json
    Wrap in metadata envelope:
    {
      "data": [ { "region": "US", "quarter": "Q4", "revenue": 1234567 }, ... ],
      "metadata": {
        "metric": "revenue",
        "dimensions": ["region", "quarter"],
        "row_count": 42,
        "query_time_ms": 340,
        "tenant": "acme",
        "schema": { "revenue": "Float64", "region": "Utf8", "quarter": "Utf8" }
      }
    }
    |
    v
[8] Output to stdout
    JSON blob written to stdout; process exits with code 0
```

### Key Data Flow Decisions

1. **LogicalPlan is the pivot point.** Everything before stage 4 produces a LogicalPlan. Everything after stage 5 consumes it. This is the contract boundary between semantic resolution and physical execution.

2. **Tenant isolation happens at two levels.** Row-level (AnalyzerRule on the LogicalPlan) and path-level (TableProvider configuration during context setup). Both are applied before DataFusion's optimizer runs, so the optimizer can push tenant filters down into Iceberg partition pruning and Parquet predicate evaluation.

3. **No SQL string exists in the pipeline.** The semantic layer builds a LogicalPlan directly. DataFusion never parses SQL text. This eliminates SQL injection as a concern and avoids the complexity of SQL dialect handling.

## DataFusion Integration Points

These are the specific DataFusion extension points Quarry uses and why.

| Extension Point | Quarry Usage | Why This One |
|----------------|--------------|--------------|
| `SessionContext` | Created fresh per query; registers all providers and rules | Entry point for all DataFusion operations |
| `AnalyzerRule` | `TenantIsolationRule` -- injects tenant WHERE predicates | Runs before optimization, so filters get pushed down automatically |
| `TableProvider` (via `IcebergStaticTableProvider`) | Read-only Iceberg table access with partition pruning | Read-only snapshot access is exactly what ephemeral queries need |
| `TableProvider` (custom wrapper) | Tenant-scoped Iceberg table provider for path-level isolation | Wraps IcebergStaticTableProvider to override table location |
| `LogicalPlan` builder API | Semantic resolution builds plans programmatically | Avoids SQL parsing; type-safe plan construction |
| Arrow `RecordBatch` | Query results in columnar format | Native DataFusion output; efficient JSON serialization via arrow_json |
| `RuntimeEnv` | Configure S3 ObjectStore for Iceberg data access | Required to register S3 credentials and endpoint configuration |

### Extension Points We Explicitly Do NOT Use

| Extension Point | Why Not |
|----------------|---------|
| `OptimizerRule` | Tenant isolation is semantic (changes query meaning), not optimization. Use `AnalyzerRule` instead. |
| `UserDefinedLogicalNode` | Our semantic constructs resolve to standard LogicalPlan nodes (Aggregate, Filter, Join). No need for custom plan nodes. |
| `ExprPlanner` / `RelationPlanner` | We do not parse SQL text, so SQL extension points are irrelevant. |
| `PhysicalOptimizerRule` | No custom physical optimization needed; DataFusion's defaults (filter pushdown into Parquet, repartition) are sufficient. |
| SQL Parser extensions | We bypass SQL entirely by building LogicalPlan directly. |

## Build Order (Dependency Graph)

This ordering is critical for phasing. Each layer depends only on layers above it.

```
Phase 1: Semantic Model Foundation
         quarry-core/model + quarry-core/config
         (No DataFusion dependency. Pure Rust types + YAML parsing.)
         CAN TEST: Load YAML, validate models, inspect structures.

Phase 2: Query Resolution
         quarry-core/resolve
         (Depends on Phase 1. Adds datafusion-expr for LogicalPlan.)
         CAN TEST: Semantic query -> LogicalPlan, verify plan structure.

Phase 3: Tenant Isolation Rules
         quarry-core/tenant
         (Depends on Phase 2. Implements AnalyzerRule trait.)
         CAN TEST: Plan in -> plan with tenant filters out.

Phase 4: DataFusion + Iceberg Execution
         quarry-exec/
         (Depends on Phases 1-3. Full DataFusion + iceberg-datafusion.)
         CAN TEST: Execute LogicalPlan against in-memory tables first,
                   then Iceberg tables on local/mock S3.

Phase 5: Result Serialization
         quarry-exec/result
         (Depends on Phase 4. Arrow RecordBatch -> JSON.)
         CAN TEST: Feed RecordBatches, verify JSON output format.

Phase 6: CLI Integration
         quarry-cli/
         (Depends on all above. Thin orchestration.)
         CAN TEST: End-to-end with real or mocked Iceberg catalog.
```

### Why This Order

- **Phases 1-3 have no I/O dependencies.** They compile fast and can be developed with unit tests only. This is where the core intellectual complexity lives (semantic resolution, join path finding, tenant isolation correctness).
- **Phase 4 is the integration risk.** Iceberg-rust + DataFusion interop is the most uncertain area. By the time we reach it, the semantic layer is solid and tested, so integration debugging is isolated to execution concerns.
- **Phase 5 is trivial** once Phase 4 works -- it is a thin serialization layer.
- **Phase 6 is glue** -- it calls the library crates in sequence. Minimal logic, minimal risk.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Small data (MBs, <1M rows) | Default configuration works. Ephemeral context overhead is negligible relative to query time. Single-threaded DataFusion execution may suffice. |
| Medium data (GBs, 1M-100M rows) | Iceberg partition pruning becomes critical. Ensure semantic model dimensions align with Iceberg partition columns. DataFusion's parallel execution (multiple cores) helps. |
| Large data (10s of GBs, 100M+ rows) | S3 request latency dominates. Consider: (1) Iceberg manifest caching across invocations, (2) predicate pushdown to minimize Parquet row groups read, (3) projection pushdown to minimize columns decoded. May need to relax ephemeral constraint for warm-start mode. |
| Very large data (TBs) | Beyond ephemeral single-process scope. Would need distributed execution (not in V1 scope). DataFusion's `SessionContext` is single-process by design. |

### Scaling Priorities

1. **First bottleneck: Iceberg metadata fetch.** Loading table metadata from the catalog on every query invocation is the most likely cold-start cost. Mitigation: Iceberg metadata is small (KB-MB); REST catalog responses cache well. V2 could add metadata caching.
2. **Second bottleneck: S3 read latency for Parquet files.** Partition pruning is the primary mitigation. Quarry must ensure tenant isolation filters are pushed down to partition evaluation so irrelevant files are never opened.
3. **Third bottleneck: Memory for large result sets.** DataFusion streams RecordBatches, so memory is proportional to batch size, not total result size. JSON serialization should also stream (write batches as they arrive, not collect all in memory first).

## Anti-Patterns

### Anti-Pattern 1: SQL String Assembly

**What people do:** Build SQL queries as format strings from semantic model definitions.
**Why it is wrong:** SQL injection risk, dialect-specific quoting bugs, no automatic optimization, impossible to reliably inject tenant isolation. Every new metric type requires new string templates.
**Do this instead:** Build `LogicalPlan` trees directly using DataFusion's builder API. This is type-safe, automatically optimized, and tenant filters compose naturally as plan tree transformations.

### Anti-Pattern 2: Eager Full Table Registration

**What people do:** Register every table in the Iceberg catalog as a DataFusion table provider at startup, even if the query only touches 2 of 50 tables.
**Why it is wrong:** Each table registration fetches Iceberg metadata from the catalog (network I/O). In an ephemeral context, this adds seconds of unnecessary latency.
**Do this instead:** Analyze the resolved `LogicalPlan` to determine which tables are actually referenced, then register only those tables. Lazy registration based on the plan's `TableScan` nodes.

### Anti-Pattern 3: Monolithic Crate

**What people do:** Put everything in one crate for simplicity.
**Why it is wrong:** Compile times explode because DataFusion and iceberg-rust are large dependency trees. Every change to YAML parsing recompiles the Iceberg integration. Testing requires standing up real infrastructure.
**Do this instead:** Split into core (pure logic, fast compilation) and exec (I/O, DataFusion, Iceberg). The core crate should be testable in <5 seconds of compile time.

### Anti-Pattern 4: Custom Query Language

**What people do:** Invent a custom DSL for metric queries that then gets parsed and translated.
**Why it is wrong:** Parser maintenance burden, poor error messages, limited expressiveness. Users already know metric/dimension vocabulary from dbt/Looker.
**Do this instead:** Use structured input (CLI flags or JSON/YAML query format) that maps directly to semantic concepts. No parsing beyond argument extraction. The "language" is the semantic model vocabulary, not a syntax.

### Anti-Pattern 5: Optimizer Rule for Tenant Isolation

**What people do:** Implement tenant isolation as an `OptimizerRule`.
**Why it is wrong:** OptimizerRules must produce semantically equivalent plans. Adding a WHERE clause changes semantics. DataFusion may skip optimizer rules or reorder them, potentially dropping tenant filters.
**Do this instead:** Use `AnalyzerRule`, which runs before optimization and is explicitly designed for semantic rewrites (type coercion, validation, and -- exactly our use case -- injecting required predicates).

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| S3-compatible storage | `object_store` crate with S3 backend, configured via `RuntimeEnv` | Uses standard AWS credential chain. Must register ObjectStore with DataFusion's RuntimeEnv before creating table providers. |
| Iceberg REST Catalog | `iceberg-catalog-rest` crate: `RestCatalog` via `CatalogBuilder` | Most common catalog for production. Configure with URI + warehouse path. |
| AWS Glue Catalog | `iceberg-catalog-glue` crate: `GlueCatalog` via `CatalogBuilder` | For AWS-native environments. Configure with region + warehouse path. |
| Filesystem Catalog | `iceberg` crate built-in: for local development/testing | Point at a local directory with Iceberg metadata. No network dependency. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| quarry-cli -> quarry-core | Direct function calls: `load_model()`, `resolve_query()` | CLI passes parsed args; core returns `Result<LogicalPlan>` |
| quarry-core -> quarry-exec | `LogicalPlan` is the contract type | Core produces plans; exec consumes them. Core never touches DataFusion's SessionContext. |
| quarry-exec -> DataFusion | `SessionContext::execute_logical_plan()` | Exec owns the SessionContext lifecycle. Registers providers, rules, then executes. |
| quarry-exec -> Iceberg | `IcebergStaticTableProvider` registered with SessionContext | One provider per referenced table. Exec resolves catalog + table name to provider. |
| quarry-exec -> S3 | Via DataFusion's ObjectStore registry (transparent to Quarry code) | Iceberg's FileIO reads Parquet files from S3. Quarry registers the ObjectStore; reads happen automatically during execution. |

## Sources

- [DataFusion Introduction](https://datafusion.apache.org/user-guide/introduction.html) -- HIGH confidence: official docs
- [DataFusion TableProvider trait](https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html) -- HIGH confidence: API docs
- [DataFusion Query Optimizer](https://datafusion.apache.org/library-user-guide/query-optimizer.html) -- HIGH confidence: official docs
- [DataFusion Extending SQL](https://datafusion.apache.org/library-user-guide/extending-sql.html) -- HIGH confidence: official docs
- [DataFusion Architecture (DeepWiki)](https://deepwiki.com/apache/datafusion) -- MEDIUM confidence: third-party synthesis of official sources
- [DataFusion Optimization Blog Part 1](https://datafusion.apache.org/blog/2025/06/15/optimizing-sql-dataframes-part-one/) -- HIGH confidence: official blog
- [Wren AI: Semantic SQL with DataFusion](https://www.getwren.ai/post/powering-semantic-sql-for-ai-agents-with-apache-datafusion) -- MEDIUM confidence: production case study directly relevant to Quarry's architecture
- [iceberg-datafusion table module](https://rust.iceberg.apache.org/api/iceberg_datafusion/table/index.html) -- HIGH confidence: official API docs
- [iceberg-rust FileIO](https://rust.iceberg.apache.org/api/iceberg/io/struct.FileIO.html) -- HIGH confidence: official API docs
- [iceberg-rust GlueCatalog](https://rust.iceberg.apache.org/api/iceberg_catalog_glue/) -- HIGH confidence: official API docs
- [dbt MetricFlow Architecture](https://www.getdbt.com/blog/how-the-dbt-semantic-layer-works) -- MEDIUM confidence: vendor blog but authoritative for semantic layer patterns
- [arrow_json writer](https://arrow.apache.org/rust/arrow_json/writer/index.html) -- HIGH confidence: official Arrow docs
- [serde_yaml deprecation discussion](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868) -- MEDIUM confidence: community discussion
- [RUSTSEC-2025-0068: serde_yml unsound](https://rustsec.org/advisories/RUSTSEC-2025-0068.html) -- HIGH confidence: official security advisory

---
*Architecture research for: Quarry -- Rust-native local analytics engine*
*Researched: 2026-02-24*
