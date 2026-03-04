# Quarry

Quarry is a Rust-native local analytics engine that sits between AI agents and your lakehouse.

Agents query a semantic layer (metrics, dimensions, entities), not raw tables. Quarry resolves those requests into DataFusion plans, injects tenant isolation, and executes against local files or Iceberg table metadata (including `s3://...` metadata locations).

It is designed for ephemeral, per-query compute: no long-running cluster and no shared always-on control plane.

## Why Quarry

- Semantic contract for agents: JSON metrics/dimensions/filters, no SQL prompt fragility
- Safe-by-default tenant isolation injection in planning
- Local-first execution with portable binaries
- MCP tools for Codex, Claude Code, and Cursor
- Plan-level observability in every response envelope

## v0.1.0 Capabilities

- CLI commands:
  - `quarry validate --model <path>`
  - `quarry query --model <path> --catalog <local|glue> --tenant <id> [--local-data-dir <path>] --input <file> --format json`
  - `quarry explain --model <path> --catalog <local|glue> --tenant <id> [--local-data-dir <path>] --input <file>`
- Semantic model in YAML (`entities`, `dimensions`, `measures`, `metrics`)
- Semantic query JSON input contract
- DataFusion logical/optimized/physical planning
- Row-level tenant isolation
- Local adapter sources:
  - CSV/Parquet files
  - Iceberg static table metadata (via `physical.format: iceberg` + `metadata_path`)
- Glue adapter baseline with AWS config enforcement
- Versioned response envelopes (`schema_version: "v1"`)

## Installation

### Option 1: GitHub Releases binaries

Download for your platform from [GitHub Releases](https://github.com/digiterialabs/quarry/releases), then run:

```bash
./quarry --help
```

### Option 2: Cargo from git

```bash
cargo install --git https://github.com/digiterialabs/quarry.git --bin quarry --locked
```

## 5-Minute Local Quickstart

```bash
quarry validate --model models/example/model.yml

quarry query \
  --model models/example/model.yml \
  --catalog local \
  --tenant tenant_123 \
  --local-data-dir models/example/data \
  --input models/example/query_by_region.json
```

Expected aggregate for `tenant_123`:

- `EU`: `250.0`
- `NA`: `100.0`

## Iceberg on S3/MinIO (Static Metadata Path)

Define physical source on entities in model YAML:

```yaml
entities:
  - name: orders
    table: orders
    physical:
      format: iceberg
      metadata_path: s3://warehouse/orders/metadata/v2.metadata.json
      options:
        s3.endpoint: http://localhost:9000
        s3.path-style-access: "true"
```

Set storage credentials/env (example):

```bash
export AWS_REGION=us-east-1
export AWS_ACCESS_KEY_ID=minioadmin
export AWS_SECRET_ACCESS_KEY=minioadmin
export QUARRY_S3_ENDPOINT=http://localhost:9000
export QUARRY_S3_PATH_STYLE_ACCESS=true
```

Optional bulk IO props override:

```bash
export QUARRY_ICEBERG_IO_PROPS_JSON='{"s3.region":"us-east-1"}'
```

Then run `quarry query` normally.

## MCP Integrations (Codex + Claude Code + Cursor)

One installer for all three:

```bash
python3 scripts/install_integrations.py --codex --claude --cursor
```

Docs:

- [Codex](docs/integrations/codex.md)
- [Claude Code](docs/integrations/claude-code.md)
- [Cursor](docs/integrations/cursor.md)

## Observability in Query Meta

Each success envelope now includes:

- `planning_ms`, `optimization_ms`, `physical_planning_ms`, `execution_ms`
- `generated_sql` (logical plan rendering)
- `optimized_plan`
- `physical_plan`
- `logical_plan_hash`, `optimized_plan_hash`, `physical_plan_hash`
- `sandbox_id`, `execution_mode`
- `table_bindings` (entity/table/source mapping)

## Development

```bash
cargo fmt --check
cargo test -q
python3 tests/mcp_smoke.py
```

## Current Boundaries

- Query execution path is single-entity scoped today for dimensions/filters
- Glue adapter currently enforces AWS config and uses static-source registration boundary
- Path-level tenant isolation remains out of scope for v0.1.x

## License

MIT. See [LICENSE](LICENSE).
