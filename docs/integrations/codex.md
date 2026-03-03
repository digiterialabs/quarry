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
