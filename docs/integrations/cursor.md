# Cursor Integration

Quarry supports Cursor through project-local MCP plus a manual prompt workflow.

## Install

From repository root:

```bash
python3 scripts/install_integrations.py --cursor
```

This generates `.cursor/mcp.json` in the project with a `quarry` server entry.

## Verify MCP visibility

1. Reopen the workspace in Cursor.
2. Open MCP settings and ensure `quarry` appears.
3. Run a simple validate request through the chat agent.

## Manual prompt workflow

Use this template in Cursor chat:

```text
Use Quarry MCP tools only.
1) quarry_validate with model_path=models/example/model.yml.
2) quarry_query with:
   - model_path=models/example/model.yml
   - catalog=local
   - tenant_id=tenant_123
   - local_data_dir=models/example/data
   - query_json with revenue metric grouped by orders.region and status=completed.
3) Return a concise summary and include the raw JSON envelope.
```

## Available MCP tools

- `quarry_validate`
- `quarry_query`
- `quarry_explain`
- `quarry_collection_create`
- `quarry_collection_list`
- `quarry_sync`
- `quarry_search`

## Optional context retrieval prompt

```text
Use Quarry MCP tools only.
1) quarry_collection_create tenant_id=tenant_123 name=sales_docs.
2) quarry_sync tenant_id=tenant_123 collection=sales_docs connector=filesystem with
   config_json={"paths":["models/example/context"],"recursive":true,"extensions":["txt","md"]}.
3) quarry_search tenant_id=tenant_123 collection=sales_docs query="revenue playbook" top_k=5.
4) Return top hits with source_uri and a short summary.
```
