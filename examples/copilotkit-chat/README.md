# Quarry CopilotKit UI Demo

This example uses [CopilotKit](https://github.com/CopilotKit/CopilotKit) as the chat UI and
runtime, with a local HTTP MCP server that exposes Quarry tools.

## What this demo includes

- CopilotKit Next.js chat UI (`apps/web`)
- Local HTTP MCP server (`apps/threejs-server`) adapted to call:
  - `quarry_validate`
  - `quarry_query`
  - `quarry_explain`
  - `quarry_collection_create`
  - `quarry_sync`
  - `quarry_search`

## Prerequisites

- Node.js 20+
- `npm` (or `pnpm`)
- [Ollama](https://ollama.com/) running locally

## Setup

From repository root:

```bash
cd examples/copilotkit-chat
npm install
```

Start Ollama and pull a tool-capable local model:

```bash
ollama serve
ollama pull llama3.1:8b
```

Create `.env` in `examples/copilotkit-chat` (optional overrides):

```bash
cat > .env <<'ENV'
OLLAMA_BASE_URL=http://127.0.0.1:11434/v1
OLLAMA_MODEL=llama3.1:8b
ENV
```

## Run

Terminal 1 (Quarry MCP HTTP server on `:3108`):

```bash
cd examples/copilotkit-chat/apps/threejs-server
npm run dev
```

Terminal 2 (CopilotKit web UI on `:3000` by default):

```bash
cd examples/copilotkit-chat/apps/web
npm run dev
```

Open:

```text
http://127.0.0.1:3000
```

If chat sends but does not answer, check:

- Ollama is running (`curl http://127.0.0.1:11434/api/tags`)
- model exists (`ollama list` includes `llama3.1:8b`)
- `apps/web` logs for connection errors to Ollama

## Suggested prompts

- `revenue by region for tenant_123`
- `explain the revenue query for tenant_123`
- `search context for revenue playbook for tenant_123`
- `validate the semantic model`

## Environment overrides (optional)

For the MCP server (`apps/threejs-server`):

- `QUARRY_BIN` to use a prebuilt quarry binary
- `QUARRY_REPO_ROOT` to point to a non-default repo path
- `QUARRY_MODEL_PATH`
- `QUARRY_LOCAL_DATA_DIR`
- `QUARRY_CONTEXT_DIR`
- `QUARRY_QUERY_FILE`

For the web runtime (`apps/web`):

- `QUARRY_MCP_SERVER_URL` (defaults to `http://localhost:3108/mcp`)
- `OLLAMA_BASE_URL` (defaults to `http://127.0.0.1:11434/v1`)
- `OLLAMA_MODEL` (defaults to `llama3.1:8b`)
- `OLLAMA_API_KEY` (optional; defaults to `ollama`)
