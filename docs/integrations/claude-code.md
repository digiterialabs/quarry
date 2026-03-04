# Claude Code Revenue Quickstart

This guide gives you a copy-paste flow that proves Claude Code can use Quarry MCP tools to
produce tenant-isolated analytics.

## What this example proves

Claude Code can call Quarry MCP tools and return revenue by region for `tenant_123` from the
bundled local fixture data.

## Prerequisites

- `python3` and `cargo` installed
- Quarry repository checked out
- Commands run from repo root

## Install Claude Code MCP integration

```bash
python3 scripts/install_integrations.py --claude
```

This writes a project-local `.mcp.json` with a `quarry` server entry.

Then reopen the project in Claude Code so MCP servers are reloaded.

## Quick environment check (before Claude)

```bash
cargo run -q -p quarry-cli -- validate --model models/example/model.yml
```

Expected result includes:

- `"schema_version": "v1"`
- `"status": "ok"`

## Canonical query payload

Quarry query JSON for this quickstart:

```json
{
  "metrics": ["revenue"],
  "dimensions": [{ "name": "orders.region" }],
  "filters": [{ "field": "orders.status", "op": "eq", "value": "completed" }],
  "order_by": [{ "field": "revenue", "direction": "desc" }],
  "limit": 1000
}
```

Canonical file version: `models/example/query_by_region.json`.

## Copy-paste prompt sequence for Claude Code

### Prompt 1: validate model

```text
Use the `quarry_validate` MCP tool with:
{
  "model_path": "models/example/model.yml"
}
Return whether status is ok and any validation issues.
```

### Prompt 2: run revenue-by-region query

```text
Use the `quarry_query` MCP tool with:
{
  "model_path": "models/example/model.yml",
  "catalog": "local",
  "tenant_id": "tenant_123",
  "local_data_dir": "models/example/data",
  "query_json": {
    "metrics": ["revenue"],
    "dimensions": [{ "name": "orders.region" }],
    "filters": [{ "field": "orders.status", "op": "eq", "value": "completed" }],
    "order_by": [{ "field": "revenue", "direction": "desc" }],
    "limit": 1000
  }
}
Return the raw JSON response.
```

### Prompt 3: summarize and verify

```text
From the previous Quarry JSON result:
1) Output a table with columns `region` and `revenue`
2) Add a total revenue line
3) Verify expected values EU=250.0 and NA=100.0 (total 350.0)
4) If any value differs, report a mismatch
```

### Prompt 4 (optional): explain plan for debugging

```text
Use the `quarry_explain` MCP tool with the same model_path, catalog, tenant_id, local_data_dir,
and query_json. Summarize the tenant filter and grouping in one short paragraph.
```

## Strict expected output

For `tenant_123`, this quickstart should return exactly:

- `EU: 250.0`
- `NA: 100.0`
- total revenue: `350.0`

Expected response metadata includes:

- `tenant_id = "tenant_123"`
- `catalog = "local"`

## Pass/fail checklist

- Claude Code can see Quarry MCP tools:
  - `quarry_validate`, `quarry_query`, `quarry_explain`
  - `quarry_collection_create`, `quarry_collection_list`, `quarry_sync`, `quarry_search`
- `quarry_validate` returns `status: ok`
- `quarry_query` returns `status: ok`
- Query rows match expected values exactly
- Meta shows `tenant_id: tenant_123` and `catalog: local`

## Optional context retrieval flow

If you also want retrieval-style context for the same tenant, run:

```text
Use these Quarry MCP tools in order:
1) quarry_collection_create { "tenant_id": "tenant_123", "name": "sales_docs" }
2) quarry_sync {
  "tenant_id": "tenant_123",
  "collection": "sales_docs",
  "connector": "filesystem",
  "config_json": { "paths": ["models/example/context"], "recursive": true, "extensions": ["txt","md"] }
}
3) quarry_search {
  "tenant_id": "tenant_123",
  "collection": "sales_docs",
  "query": "revenue playbook",
  "top_k": 5
}
Return raw JSON plus a concise summary.
```

## Troubleshooting

- `.mcp.json` missing:
  - Re-run `python3 scripts/install_integrations.py --claude` from repo root.
- `.mcp.json` malformed:
  - Fix JSON syntax or delete and regenerate with installer.
- Claude does not show Quarry tools:
  - Reopen the project/workspace to reload MCP.
- Python not found:
  - Ensure `python3` is installed and available on `PATH`.
- First query is slow:
  - `cargo run` compiles on first use; reruns are faster.
- Query input error:
  - `quarry_query` requires exactly one of `query_json` or `query_file`.
- Wrong dimension:
  - Use `orders.region` for this model, not `customers.region`.
