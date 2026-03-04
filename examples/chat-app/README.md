# Quarry Chat Demo App

This is a local chat-style demo that routes user intents to Quarry commands so you can see:

- semantic analytics (`validate`, `query`, `explain`)
- context retrieval (`collection create`, `sync`, `search`)

## Run

From repo root:

```bash
python3 examples/chat-app/app.py --port 8090
```

Open:

```text
http://127.0.0.1:8090
```

## Faster startup (optional)

By default, the demo uses:

```text
cargo run -q -p quarry-cli --
```

For faster responses, pre-build once:

```bash
cargo build -p quarry-cli
```

Then run the app with:

```bash
QUARRY_BIN=target/debug/quarry python3 examples/chat-app/app.py --port 8090
```

## Suggested prompts

- `validate model`
- `revenue by region for tenant_123`
- `explain revenue query for tenant_123`
- `search context for revenue playbook for tenant_123`

## Notes

- Default tenant is `tenant_123`.
- Default context storage path is `.quarry-demo-context` at repo root.
- Search flow auto-creates/syncs collection `sales_docs` from `models/example/context`.
