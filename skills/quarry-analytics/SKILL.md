---
name: quarry-analytics
description: Route analytics intent to Quarry MCP tools for model validation, semantic query execution, and explain/debug flows.
---

# Quarry Analytics

Use this skill when the user asks for metrics/KPI queries, grouped aggregates, tenant analytics, or query-plan debugging against Quarry models.

## Tooling contract

1. Call `quarry_validate` before first query against a model, or after model edits.
2. Call `quarry_query` for analytics execution.
3. Call `quarry_explain` for dry-run/debug/plan inspection requests.

## Portable defaults

- Default `catalog`: `local`
- Default `model_path`: `models/example/model.yml` from the current repository root
- Default local data dir: `models/example/data` from the current repository root
- `tenant_id` is required for `query` and `explain`

## Query shape

Use Quarry semantic JSON (not SQL), for example:

```json
{
  "metrics": ["revenue"],
  "dimensions": [{ "name": "customers.region" }],
  "filters": [{ "field": "orders.status", "op": "eq", "value": "completed" }],
  "order_by": [{ "field": "revenue", "direction": "desc" }],
  "limit": 1000
}
```

## Response behavior

- Return a concise narrative summary.
- Include the raw JSON result envelope.
- If an error occurs, surface `error.code` and give the next concrete remediation step.
