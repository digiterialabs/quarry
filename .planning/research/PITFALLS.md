# Pitfalls Research

**Domain:** Rust-native local analytics engine (DataFusion + Iceberg + semantic layer)
**Researched:** 2026-02-24
**Confidence:** MEDIUM-HIGH (verified across official docs, GitHub issues, production post-mortems, and academic research)

---

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: iceberg-rust Feature Gaps Force Workarounds That Become Permanent

**What goes wrong:**
iceberg-rust is actively maturing but still trails the Java reference implementation significantly. Teams build workarounds for missing features (incomplete predicate pushdown types, missing Puffin file support, partial catalog implementations) and those workarounds calcify into the architecture. When the upstream crate adds proper support months later, the workaround is too entangled to remove.

**Why it happens:**
iceberg-rust ships roughly monthly with expanding capabilities (v0.8.0 landed Jan 2026 with V3 metadata format support, Boolean predicate pushdown, LIMIT pushdown). But critical features like full compaction, certain catalog operations, and some advanced scan predicates are still in-progress. Developers build custom code to fill gaps without designing for replaceability.

**How to avoid:**
- Wrap all Iceberg access behind a trait boundary (`IcebergReader` trait) so the implementation can evolve independently.
- Pin to a specific iceberg-rust version and evaluate upgrades deliberately rather than chasing `main`.
- For any workaround, add a `// TODO(iceberg-upstream): remove when iceberg-rust supports X` comment with a link to the tracking issue.
- Check the [iceberg-rust GitHub issues](https://github.com/apache/iceberg-rust/issues) for feature EPICs before implementing custom code.

**Warning signs:**
- You are implementing Iceberg spec logic (manifest parsing, partition pruning) in your own code rather than calling iceberg-rust APIs.
- You have forked or patched iceberg-rust locally.
- Your workaround code exceeds 200 lines for a single missing feature.

**Phase to address:**
Phase 1 (Core Iceberg Integration) -- design the abstraction layer from day one.

---

### Pitfall 2: S3 Metadata Fetch Latency Destroys Ephemeral Query Performance

**What goes wrong:**
Each Iceberg query requires fetching: table metadata JSON, manifest list, one or more manifest files, and then the actual Parquet data files. On S3, each of these is a separate HTTP GET with 50-200ms latency. For an ephemeral CLI tool with no warm cache, a simple query can spend 500ms-2s just reading metadata before touching any data. Users experience unacceptable latency for what should be sub-second queries on small tables.

**Why it happens:**
Iceberg was designed for long-running query engines (Spark, Trino) where metadata is cached across queries. The metadata tree structure (metadata.json -> manifest-list -> manifests -> data files) creates a serial dependency chain. S3's high per-request latency (vs. local disk) compounds at each level. There is no "single-file" metadata mode in Iceberg V1/V2 (discussed for V4 spec but purely conceptual).

**How to avoid:**
- Implement optional local metadata caching (e.g., SQLite or file-based cache of metadata/manifest files with TTL) that persists between CLI invocations.
- Parallelize manifest file reads once the manifest list is loaded -- these are independent and can be fetched concurrently.
- Use Iceberg's manifest-level partition statistics to aggressively prune manifests before fetching them.
- Consider a `--warm` flag or background metadata prefetch mode for latency-sensitive workflows.
- Profile S3 request count per query from the start; track it as a first-class metric.

**Warning signs:**
- Query latency is dominated by "planning" time rather than "execution" time.
- `EXPLAIN ANALYZE` or tracing shows >3 sequential S3 GETs before the first data file read.
- Small-table queries (< 1MB data) take >1 second.

**Phase to address:**
Phase 2 (Query Execution) for basic flow; Phase 3+ for caching optimization.

---

### Pitfall 3: Semantic Layer Join Fan-Out Produces Silently Wrong Results

**What goes wrong:**
The semantic layer generates SQL from metric/dimension definitions. When a query involves metrics from a fact table joined to a dimension table with a one-to-many relationship, aggregate results (SUM, COUNT) silently inflate due to row duplication. Users get plausible-looking but incorrect numbers. This is the single most dangerous class of bug in a semantic layer because it produces wrong answers without errors.

**Why it happens:**
Academic research (["Aggregation Consistency Errors in Semantic Layers"](https://arxiv.org/html/2307.00417)) documents this as a fundamental problem. When a fact table LEFT JOINs to a dimension table that has multiple rows per join key, every fact row duplicates. `SUM(revenue)` now counts each revenue value N times. The generated SQL is syntactically valid and executes without error -- only the results are wrong. Existing BI tools rely on undisclosed heuristics for deduplication, producing imprecise outcomes.

**How to avoid:**
- Model entity relationships with explicit cardinality annotations (one-to-one, one-to-many, many-to-many) in the YAML semantic model.
- Implement join-type selection logic like dbt MetricFlow: choose join types based on entity relationship types to avoid fan-out and chasm joins.
- For many-to-many relationships, either refuse the query with a clear error or require explicit pre-aggregation in the semantic model definition.
- Add a validation step that checks: if the query involves an aggregate + a join, verify the join cardinality cannot cause fan-out.
- Test metric consistency: `SELECT SUM(revenue) FROM orders` must equal `SELECT SUM(revenue) FROM orders JOIN dimensions ...` for any valid dimension join.

**Warning signs:**
- Metric values change when you add a dimension that shouldn't affect the aggregate.
- The same metric returns different values depending on which dimensions are requested.
- No cardinality metadata exists in your semantic model YAML.

**Phase to address:**
Phase 1 (Semantic Layer Design) -- the YAML model schema must include cardinality from the beginning. Retrofitting is a rewrite.

---

### Pitfall 4: DataFusion/Arrow/iceberg-rust Version Lock-In Creates Upgrade Hell

**What goes wrong:**
DataFusion releases a new major version roughly every 6-8 weeks with breaking API changes. iceberg-rust depends on specific DataFusion versions. The `arrow`, `parquet`, and `object_store` crates must all align. You end up pinned to a stale DataFusion version because iceberg-rust hasn't caught up, or you upgrade DataFusion and find iceberg-rust's `IcebergTableProvider` doesn't compile.

**Why it happens:**
DataFusion's API health policy explicitly allows breaking changes in major versions (deprecation period of 6 major versions or 6 months). Between DataFusion 46-52 (March 2025 to Jan 2026), scan operators were refactored, ParquetExec/CsvExec/AvroExec were removed, `DFSchema` methods changed from returning `&Field` to `&FieldRef`, and the SQL dialect parameter changed type. Each of these breaks downstream embedders. Arrow/Parquet/object_store are separate crates with their own release cadence. Using arrow 41 when DataFusion depends on arrow 40 causes cryptic type mismatch errors (`expected &Vec<datafusion::arrow::record_batch::RecordBatch> found &Vec<arrow::record_batch::RecordBatch>`).

**How to avoid:**
- Always re-export Arrow types from DataFusion (`use datafusion::arrow`) rather than depending on `arrow` directly.
- Pin all three together: DataFusion, iceberg-rust (iceberg-datafusion), and arrow/parquet/object_store to compatible versions. Document the version matrix in your Cargo.toml.
- Insulate your code from DataFusion internals behind a thin query-engine trait. Only interact with DataFusion through `SessionContext::sql()` and the `RecordBatch` output type where possible.
- Budget upgrade time: expect ~2-4 hours of migration work per DataFusion major version bump.
- Subscribe to [DataFusion release blog posts](https://datafusion.apache.org/blog/) for advance notice of breaking changes.

**Warning signs:**
- You depend on `arrow` AND `datafusion` as separate Cargo dependencies with different implicit Arrow versions.
- Your code uses internal DataFusion types (physical plan nodes, expression internals) rather than the SQL interface.
- You are >3 major versions behind current DataFusion.

**Phase to address:**
Phase 1 (Project Setup) -- set the dependency strategy and trait boundaries before writing application code.

---

### Pitfall 5: Tenant Isolation Bypassed Through Unprotected Query Paths

**What goes wrong:**
WHERE clause injection for row-level tenant isolation works for the happy path (simple SELECT queries). But edge cases leak data: subqueries, CTEs, UNION queries, window functions operating across partition boundaries, or queries against Iceberg metadata tables. Path-level isolation (per-tenant S3 prefixes) fails when catalog metadata references cross-tenant paths.

**Why it happens:**
SQL is complex. A semantic layer that generates SQL and then injects `WHERE tenant_id = ?` must handle every SQL construct that DataFusion supports. If tenant isolation is implemented as a string-level SQL transformation (regex/string replace), it will miss edge cases. If implemented at the logical plan level, it must handle every logical plan node type. Neither approach is trivial. Additionally, DataFusion's SQL dialect evolves -- new SQL features mean new bypass vectors.

**How to avoid:**
- Implement tenant isolation at the DataFusion logical plan level (as an `OptimizerRule` or `AnalyzerRule`), not as SQL string manipulation. This ensures every scan node gets the tenant filter regardless of query structure.
- For path-level isolation, configure the `IcebergTableProvider` per-tenant with scoped S3 prefixes so the catalog literally cannot see other tenants' data.
- Build an explicit test suite of bypass attempts: subqueries, CTEs, UNIONs, window functions, metadata queries, `EXPLAIN` output.
- Default to deny: if a query path hasn't been explicitly verified for tenant safety, reject it.

**Warning signs:**
- Tenant isolation is implemented with string manipulation on generated SQL.
- You test tenant isolation only with simple `SELECT ... FROM table` queries.
- No test exists that tries to read another tenant's data.

**Phase to address:**
Phase 2 (Tenant Isolation) -- must be designed as a core architectural constraint, not bolted on.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcoding S3 as the only storage backend | Simpler FileIO setup, skip abstraction | Cannot support local files for testing, Azure/GCS later | V1 only, but use `object_store` trait from start |
| Skipping DataFusion memory limits | Queries "just work" on small data | OOM kills on large tables with no useful error | Never -- set `MemoryPool` limits from day one |
| Parsing YAML semantic model on every query | No cache invalidation complexity | 5-50ms overhead per query adds up for ephemeral tool | V1 acceptable if model files are small (<100KB) |
| Single-threaded Tokio runtime for CLI | Simpler setup, less overhead | Cannot parallelize manifest reads or S3 fetches | Never -- use multi-threaded runtime; S3 parallelism is critical |
| Generating SQL as string concatenation | Fast to implement, easy to debug | SQL injection risks, dialect-specific escaping bugs, impossible to optimize | Never -- use DataFusion's logical plan builder or parameterized queries |
| Ignoring Iceberg partition pruning | Queries work, just slower | Full table scans on partitioned tables; 10-1000x slowdown on large tables | Never -- partition pruning is table stakes for Iceberg |

## Integration Gotchas

Common mistakes when connecting to external services and libraries.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| S3 via `object_store` | Hardcoding credentials instead of using AWS credential chain | Use `object_store`'s built-in AWS credential provider which supports env vars, instance profiles, SSO, etc. Only accept `AWS_*` uppercase env vars. |
| Iceberg REST Catalog | Assuming catalog operations are fast; blocking the runtime on catalog HTTP calls | All catalog operations are async and involve network I/O. Use async throughout. Handle catalog unavailability gracefully with timeouts. |
| Iceberg Glue Catalog | Not handling pagination for large databases with many tables | Glue `get_tables` returns paginated results. The catalog implementation must handle pagination or risk missing tables. |
| DataFusion `SessionContext` | Creating a new `RuntimeEnv` per query (the default) | Reuse `RuntimeEnv` across a session for memory limit enforcement. For ephemeral CLI, this matters less but still: configure memory limits on the RuntimeEnv. |
| Parquet on S3 | Reading many small column groups with individual range requests | Use `ParquetObjectReader` with coalesced reads. Configure `object_store` to batch range requests. Many small S3 GETs are far slower than fewer large reads. |
| Arrow type conversion | Assuming Iceberg schema types map 1:1 to Arrow types | Iceberg `timestamp` has timezone semantics (with/without tz) that must map correctly to Arrow `TimestampMicrosecond` variants. Decimal precision/scale must be preserved. |

## Performance Traps

Patterns that work at small scale but fail as data grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| No partition pruning in generated SQL | Queries return correct results but scan all data files | Ensure semantic layer generates predicates that align with Iceberg partition columns | >100 data files or >1GB total data |
| Collecting all RecordBatches into Vec before returning JSON | Memory spike proportional to result set size | Stream RecordBatches to JSON output incrementally | Result set >100MB or available memory <2GB |
| Single manifest file fetch (no parallelism) | Query planning takes N * S3-latency for N manifests | Parallel manifest fetches with `tokio::join!` or `FuturesUnordered` | >5 manifest files (common for tables with history) |
| Not setting DataFusion `target_partitions` | Defaults to number of CPU cores; spawns excessive tasks for small queries | Set `target_partitions` based on data size; 1-2 for small queries | Small queries on machines with many cores (wasted thread overhead) |
| Unbounded Parquet row group reads | All row groups loaded even when predicate eliminates most | Ensure predicate pushdown reaches Parquet row group metadata and page index filtering | Tables with >100 row groups |
| Loading full Iceberg snapshot history | Reads all snapshots to find current; slow for tables with deep history | Use latest-snapshot pointer directly; don't iterate snapshot list | Tables with >1000 snapshots (common with streaming writers) |

## Security Mistakes

Domain-specific security issues beyond general Rust safety.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Tenant ID accepted from untrusted input without validation | Cross-tenant data access; one agent reads another tenant's data | Validate tenant context at CLI entry point. Tenant ID must match a known value, not be arbitrary user input. |
| S3 credentials exposed in error messages or logs | Credential leakage in JSON output or stderr | Scrub AWS credentials from all error paths. Use `Display` impls that redact secrets. Never log full S3 request headers. |
| YAML semantic model allows arbitrary SQL expressions | SQL injection through metric definitions; a malicious YAML could define `metric: "1; DROP TABLE"` | Parse and validate YAML metric expressions against an allowlist of functions/operators. Never pass raw YAML values into SQL strings. |
| Generated SQL visible in output metadata | Query internals exposed to untrusted API consumers | Make generated SQL opt-in in output (e.g., `--show-sql` flag). Default output includes only data and safe metadata. |
| Path-level isolation relies on S3 prefix conventions only | Misconfigured prefix allows reading adjacent tenant paths | Validate that resolved S3 paths start with the expected tenant prefix before any read operation. Defense in depth. |

## CLI/UX Pitfalls

Common user experience mistakes for ephemeral CLI analytics tools.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| No progress indication during S3 metadata fetch | User thinks tool is hanging during 1-3 second metadata load | Show progress: "Loading table metadata..." on stderr. Use `indicatif` or simple status messages. |
| Cryptic DataFusion error messages passed through | User sees `ArrowError(ComputeError("..."))` with no context | Catch DataFusion/Arrow errors and wrap with domain context: "Failed to query metric 'revenue': column 'amount' not found in table 'orders'" |
| Binary output only (no human-readable mode) | JSON output is hard to scan for quick debugging | Support `--format table` for human-readable output alongside default JSON |
| No validation of semantic model before query | Errors surface deep in query execution with confusing stack traces | Add `quarry validate` command that checks YAML model for: valid table references, valid column names, valid join paths, metric expression syntax |
| Silently falling back to full table scan when predicates can't push down | User doesn't know query is scanning 10GB instead of 10MB | Warn when partition pruning didn't activate. Show scan statistics in output metadata (files scanned, bytes read). |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Iceberg table reads:** Works on simple tables but breaks on tables with partition evolution (multiple partition specs) -- test with evolved tables.
- [ ] **Tenant isolation:** Works for simple queries but bypassed by CTEs, subqueries, or UNION -- test with adversarial query patterns.
- [ ] **Semantic layer resolution:** Resolves single-metric queries but produces wrong results for multi-metric queries requiring different join paths -- test metric combinations.
- [ ] **S3 access:** Works with explicit credentials but fails with instance profiles, SSO, or assumed roles -- test all credential chain methods.
- [ ] **JSON output:** Includes data but missing metadata (query timing, rows scanned, files read, partition pruning stats) -- verify metadata completeness.
- [ ] **DataFusion memory limits:** Set but never tested with a query that would exceed them -- run an intentionally large aggregation and verify graceful failure.
- [ ] **YAML model loading:** Parses valid YAML but doesn't validate semantic correctness (orphaned joins, circular references, type mismatches) -- test with intentionally broken models.
- [ ] **Parquet type mapping:** Works for common types (int, string, timestamp) but fails on decimal, nested structs, or list types -- test with real-world Iceberg schemas.

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Fan-out producing wrong metrics | HIGH | Audit all join definitions in semantic model. Add cardinality annotations. Re-validate all existing queries against known-good values. Likely requires semantic model schema changes. |
| DataFusion version lock-in | MEDIUM | Create version compatibility matrix. Upgrade DataFusion first, then fix compilation errors using upgrade guide. Budget 2-4 hours per major version. |
| S3 metadata latency | LOW-MEDIUM | Add caching layer without changing query path. Profile to identify worst-case metadata chains. Can be done incrementally. |
| Tenant data leak | CRITICAL | Immediate incident response. Audit all query paths. Switch to logical-plan-level isolation if using string manipulation. Requires security review of all generated SQL. |
| iceberg-rust workaround calcified | MEDIUM-HIGH | Identify all workaround code. Check upstream status. Replace incrementally behind trait boundaries. Harder if workarounds are scattered through codebase. |
| Arrow type mismatch dependency hell | LOW | Delete Cargo.lock, align all arrow/datafusion/parquet/object_store versions, re-export arrow from datafusion. Usually fixable in <1 hour once understood. |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| iceberg-rust feature gaps | Phase 1: Core Setup | Trait boundary exists for all Iceberg access; no direct iceberg-rust calls in business logic |
| S3 metadata latency | Phase 2: Query Execution | Benchmark: simple query on 3-partition table completes in <2s including cold metadata load |
| Join fan-out wrong results | Phase 1: Semantic Model Design | Test: SUM(metric) is identical with and without dimension joins for all defined metrics |
| DataFusion version lock-in | Phase 1: Core Setup | Cargo.toml has version matrix comment; arrow types re-exported from datafusion; query engine behind trait |
| Tenant isolation bypass | Phase 2: Tenant Isolation | Test suite with 10+ adversarial query patterns (CTEs, subqueries, UNIONs) all blocked |
| SQL generation injection | Phase 1: Semantic Model Design | All metric expressions parsed, validated, and built via logical plan -- no string concatenation |
| Memory exhaustion on large queries | Phase 2: Query Execution | DataFusion MemoryPool configured; test with query exceeding limit returns clean error |
| Parquet type mapping failures | Phase 2: Query Execution | Integration test against Iceberg table with: decimal, timestamp with tz, nested struct, list columns |
| Cryptic error messages | Phase 3: CLI Polish | No raw DataFusion/Arrow errors in user-facing output; all wrapped with domain context |

## Sources

- [DataFusion cancellation and async design](https://datafusion.apache.org/blog/2025/06/30/cancellation/) -- HIGH confidence
- [Practical Performance Lessons from Apache DataFusion (Greptime)](https://www.greptime.com/blogs/2025-11-25-datafusion) -- HIGH confidence (production experience)
- [Apache Iceberg Practical Limitations 2025 (Quesma)](https://quesma.com/blog/apache-iceberg-practical-limitations-2025/) -- MEDIUM confidence
- [iceberg-rust 0.8.0 Release](https://iceberg.apache.org/blog/apache-iceberg-rust-0.8.0-release/) -- HIGH confidence (official)
- [iceberg-rust GitHub Issues](https://github.com/apache/iceberg-rust/issues) -- HIGH confidence
- [DataFusion Memory Management (Issue #3941)](https://github.com/apache/datafusion/issues/3941) -- HIGH confidence
- [DataFusion Batch Size Discussion (Issue #6916)](https://github.com/apache/datafusion/issues/6916) -- HIGH confidence
- [Aggregation Consistency Errors in Semantic Layers (arxiv)](https://arxiv.org/html/2307.00417) -- HIGH confidence (academic, peer-reviewed)
- [Hidden Pitfalls: Compaction and Partition Evolution in Iceberg](https://dev.to/alexmercedcoder/apache-iceberg-table-optimization-8-hidden-pitfalls-compaction-and-partition-evolution-in-13f1) -- MEDIUM confidence
- [DataFusion API Health Policy](https://datafusion.apache.org/contributor-guide/api-health.html) -- HIGH confidence (official)
- [DataFusion Arrow version mismatch (Issue #2090)](https://github.com/apache/datafusion/issues/2090) -- HIGH confidence
- [Wren AI: Powering Semantic SQL for AI Agents with DataFusion](https://www.getwren.ai/post/powering-semantic-sql-for-ai-agents-with-apache-datafusion) -- MEDIUM confidence
- [iceberg-datafusion Table Provider docs](https://docs.rs/iceberg-datafusion) -- HIGH confidence (official)
- [DataFusion Upgrade Guides](https://datafusion.apache.org/library-user-guide/upgrading.html) -- HIGH confidence (official)
- [Querying Parquet with Millisecond Latency (Arrow blog)](https://arrow.apache.org/blog/2022/12/26/querying-parquet-with-millisecond-latency/) -- HIGH confidence
- [Multi-tenant data isolation architecture (Medium)](https://medium.com/@justhamade/architecting-secure-multi-tenant-data-isolation-d8f36cb0d25e) -- LOW confidence (single source)
- [Apache Iceberg Performance Docs](https://iceberg.apache.org/docs/latest/performance/) -- HIGH confidence (official)
- [DataFusion SQL Dialect Extension Blog](https://datafusion.apache.org/blog/2026/01/12/extending-sql/) -- HIGH confidence (official)

---
*Pitfalls research for: Rust-native local analytics engine (DataFusion + Iceberg + semantic layer)*
*Researched: 2026-02-24*
