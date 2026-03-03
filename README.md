# Quarry

Quarry is a CLI-first Rust analytics engine that accepts semantic JSON queries, resolves them into DataFusion `LogicalPlan`s, enforces tenant isolation, and returns versioned JSON envelopes.

## v0.1.0 Includes

- `quarry` CLI: `validate`, `query`, `explain`
- semantic model validation from YAML
- semantic query execution from JSON (no SQL input)
- row-level tenant isolation
- local + glue catalog adapters
- MCP server for Codex, Claude Code, and Cursor

## Installation

### Option 1: GitHub Release binaries

Download the matching archive from [GitHub Releases](https://github.com/digiterialabs/quarry/releases), then unpack and run:

```bash
./quarry --help
```

Release artifacts are published for:

- Linux x86_64
- macOS arm64
- macOS x86_64
- Windows x86_64

### Option 2: Cargo from git

```bash
cargo install --git https://github.com/digiterialabs/quarry.git --bin quarry --locked
```

## 5-Minute Quickstart

From repo root:

```bash
cargo run -p quarry-cli -- validate --model models/example/model.yml

cargo run -p quarry-cli -- query \
  --model models/example/model.yml \
  --catalog local \
  --tenant tenant_123 \
  --local-data-dir models/example/data \
  --input models/example/query.json

cargo run -p quarry-cli -- explain \
  --model models/example/model.yml \
  --catalog local \
  --tenant tenant_123 \
  --local-data-dir models/example/data \
  --input models/example/query.json
```

Example multi-tenant check:

```bash
cargo run -p quarry-cli -- query \
  --model models/example/model.yml \
  --catalog local \
  --tenant tenant_999 \
  --local-data-dir models/example/data \
  --input models/example/query.json
```

Revenue by region for `tenant_123`:

```bash
cargo run -p quarry-cli -- query \
  --model models/example/model.yml \
  --catalog local \
  --tenant tenant_123 \
  --local-data-dir models/example/data \
  --input models/example/query_by_region.json
```

## AI Tool Integrations (Codex + Claude Code + Cursor)

Use the single installer:

```bash
python3 scripts/install_integrations.py --codex --claude --cursor
```

This installer safely patches/writes config with backups and is idempotent.

Integration guides:

- [Codex](docs/integrations/codex.md)
- [Claude Code](docs/integrations/claude-code.md)
- [Cursor](docs/integrations/cursor.md)

## MCP Server

Quarry MCP wrapper path:

`tools/mcp/quarry_mcp_server.py`

Exposed tools:

- `quarry_validate`
- `quarry_query`
- `quarry_explain`

Environment overrides:

- `QUARRY_BIN`: path to compiled `quarry` binary (fast path)
- `QUARRY_REPO_ROOT`: explicit repository root for command execution

## CLI Surface

- `quarry validate --model <path>`
- `quarry query --model <path> --catalog <local|glue> --tenant <id> [--local-data-dir <path>] --input <file> --format json`
- `quarry explain --model <path> --catalog <local|glue> --tenant <id> [--local-data-dir <path>] --input <file>`

## Development

```bash
cargo fmt --check
cargo test -q
python3 tests/mcp_smoke.py
```

## Troubleshooting

- MCP server does not appear:
  - rerun installer with target flag(s)
  - restart/reopen the IDE after config changes
- Codex config parse error:
  - fix `~/.codex/config.toml` and rerun installer
- Python missing:
  - install Python 3.11+ and rerun installer
- Glue catalog fails:
  - ensure `AWS_REGION` and credentials are configured

## License

MIT. See [LICENSE](LICENSE).
