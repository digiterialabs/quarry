# Claude Code Integration

Quarry supports Claude Code through project-local MCP config.

## Install

From repository root:

```bash
python3 scripts/install_integrations.py --claude
```

This generates `.mcp.json` in the project root with a `quarry` server entry.

## Generated config

```json
{
  "mcpServers": {
    "quarry": {
      "command": "python3",
      "args": ["/absolute/path/to/quarry/tools/mcp/quarry_mcp_server.py"]
    }
  }
}
```

## Verify

1. Reopen the project in Claude Code.
2. Ask: "Use Quarry to validate models/example/model.yml."
3. Ask: "Run Quarry query for tenant_123 and summarize revenue by region."

## Prompt template

```text
Use the quarry_query MCP tool with:
- model_path: models/example/model.yml
- catalog: local
- tenant_id: tenant_123
- local_data_dir: models/example/data
- query_json metrics=["revenue"], dimensions=[{"name":"customers.region"}], filter orders.status=completed.
Then summarize totals by region.
```
