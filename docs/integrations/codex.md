# Codex Integration

Quarry supports Codex through MCP plus an optional Codex skill.

## Install

From repository root:

```bash
python3 scripts/install_integrations.py --codex
```

This installer will:

- patch `~/.codex/config.toml` with `mcp_servers.quarry`
- install/update `quarry-analytics` in `~/.codex/skills/quarry-analytics`
- create backups in `~/.quarry/backups`

## Manual setup (optional)

Add this to `~/.codex/config.toml`:

```toml
[mcp_servers.quarry]
command = "python3"
args = ["/absolute/path/to/quarry/tools/mcp/quarry_mcp_server.py"]
```

## Verify

1. Restart Codex (or reload MCP servers).
2. Ask: "Run Quarry query for tenant_123 and summarize revenue by region."
3. Confirm Codex calls `quarry_validate` then `quarry_query`.

## MCP tool inventory

Quarry MCP now exposes:

- `quarry_validate`
- `quarry_query`
- `quarry_explain`
- `quarry_collection_create`
- `quarry_collection_list`
- `quarry_sync`
- `quarry_search`

## Context retrieval flow (optional)

Use this Codex prompt to index local docs and search context:

```text
Use Quarry MCP tools only.
1) quarry_collection_create with tenant_id=tenant_123 name=sales_docs.
2) quarry_sync with tenant_id=tenant_123 collection=sales_docs connector=filesystem and config_json:
   {"paths":["models/example/context"],"recursive":true,"extensions":["txt","md"]}.
3) quarry_search with tenant_id=tenant_123 collection=sales_docs query="revenue playbook" top_k=5.
Return the raw JSON envelopes and a short summary.
```
