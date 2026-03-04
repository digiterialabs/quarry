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
   - query_json with revenue metric grouped by customers.region and status=completed.
3) Return a concise summary and include the raw JSON envelope.
```
