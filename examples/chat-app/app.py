#!/usr/bin/env python3
"""Local demo chat app to showcase Quarry analytics + context retrieval.

Run:
  python3 examples/chat-app/app.py --port 8090

Optional:
  QUARRY_BIN=/absolute/path/to/quarry python3 examples/chat-app/app.py
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict, List


REPO_ROOT = Path(__file__).resolve().parents[2]
MODEL_PATH = REPO_ROOT / "models" / "example" / "model.yml"
LOCAL_DATA_DIR = REPO_ROOT / "models" / "example" / "data"
QUERY_BY_REGION = REPO_ROOT / "models" / "example" / "query_by_region.json"
CONTEXT_SEED_DIR = REPO_ROOT / "models" / "example" / "context"
INDEX_HTML = Path(__file__).resolve().parent / "index.html"
DEFAULT_CONTEXT_DIR = REPO_ROOT / ".quarry-demo-context"


def quarry_base_cmd() -> List[str]:
    quarry_bin = os.environ.get("QUARRY_BIN", "").strip()
    if quarry_bin:
        return [quarry_bin]
    return ["cargo", "run", "-q", "-p", "quarry-cli", "--"]


def run_quarry(args: List[str]) -> Dict[str, Any]:
    cmd = quarry_base_cmd() + args
    started = time.time()
    proc = subprocess.run(
        cmd,
        cwd=str(REPO_ROOT),
        text=True,
        capture_output=True,
        check=False,
    )
    elapsed_ms = int((time.time() - started) * 1000)
    stdout = proc.stdout.strip()
    stderr = proc.stderr.strip()

    payload: Any = None
    raw = stdout if proc.returncode == 0 else (stderr or stdout)
    if raw:
        try:
            payload = json.loads(raw)
        except json.JSONDecodeError:
            payload = {"raw": raw}

    return {
        "ok": proc.returncode == 0,
        "cmd": cmd,
        "args": args,
        "elapsed_ms": elapsed_ms,
        "returncode": proc.returncode,
        "payload": payload,
        "stdout": stdout,
        "stderr": stderr,
    }


def resolve_tenant(message: str, fallback: str) -> str:
    match = re.search(r"\btenant_[a-zA-Z0-9_-]+\b", message)
    if match:
        return match.group(0)
    return fallback


def ensure_collection_synced(tenant_id: str, context_dir: Path) -> List[Dict[str, Any]]:
    sync_config = {
        "paths": [str(CONTEXT_SEED_DIR)],
        "recursive": True,
        "extensions": ["txt", "md"],
    }
    config_path = context_dir / "sync_config.json"
    context_dir.mkdir(parents=True, exist_ok=True)
    config_path.write_text(json.dumps(sync_config, indent=2), encoding="utf-8")

    steps = []
    create_result = run_quarry(
        [
            "collection",
            "create",
            "--tenant",
            tenant_id,
            "--name",
            "sales_docs",
            "--description",
            "Demo docs for chat app",
            "--context-dir",
            str(context_dir),
        ]
    )
    steps.append(create_result)

    # Duplicate collection is expected after the first run. Continue.
    if not create_result["ok"]:
        err_text = json.dumps(create_result.get("payload", {}))
        if "already exists" not in err_text:
            return steps

    sync_result = run_quarry(
        [
            "sync",
            "--tenant",
            tenant_id,
            "--collection",
            "sales_docs",
            "--connector",
            "filesystem",
            "--config",
            str(config_path),
            "--context-dir",
            str(context_dir),
        ]
    )
    steps.append(sync_result)
    return steps


def summarize_query(payload: Dict[str, Any], tenant_id: str) -> str:
    if payload.get("status") != "ok":
        return "Quarry query failed."
    rows = payload.get("data", {}).get("rows", [])
    if not rows:
        return f"No rows returned for {tenant_id}."

    parts = []
    total = 0.0
    for row in rows:
        region = row.get("orders.region") or row.get("region") or "unknown"
        revenue = float(row.get("revenue", 0.0))
        total += revenue
        parts.append(f"{region}: {revenue:.1f}")
    return (
        f"Revenue by region for {tenant_id}: "
        + ", ".join(parts)
        + f". Total: {total:.1f}."
    )


def summarize_search(payload: Dict[str, Any], tenant_id: str) -> str:
    if payload.get("status") != "ok":
        return "Quarry search failed."
    hits = payload.get("data", {}).get("hits", [])
    if not hits:
        return f"No context hits found for {tenant_id}."

    top = hits[0]
    title = top.get("title", "untitled")
    snippet = top.get("snippet", "").strip()
    if len(snippet) > 180:
        snippet = snippet[:177] + "..."
    return f"Top context hit for {tenant_id}: {title}. {snippet}"


def summarize_explain(payload: Dict[str, Any], tenant_id: str) -> str:
    if payload.get("status") != "ok":
        return "Quarry explain failed."
    plan = payload.get("data", {}).get("plan", "")
    tenant_filter = tenant_id in plan
    grouped = "Aggregate" in plan or "groupBy" in plan
    return (
        f"Explain plan ready. Tenant filter present: {tenant_filter}. "
        f"Aggregation present: {grouped}."
    )


def summarize_validate(payload: Dict[str, Any]) -> str:
    if payload.get("status") == "ok":
        return "Model validation passed."
    return "Model validation failed."


def run_demo_intent(message: str, tenant_id: str, context_dir: Path) -> Dict[str, Any]:
    text = message.lower()
    commands: List[Dict[str, Any]] = []

    if "help" in text:
        return {
            "intent": "help",
            "reply": (
                "Try: 'validate model', 'revenue by region', 'explain revenue query', "
                "or 'search context for playbook'."
            ),
            "commands": [],
            "quarry_payload": None,
        }

    if "validate" in text:
        result = run_quarry(["validate", "--model", str(MODEL_PATH)])
        commands.append(result)
        payload = result.get("payload") or {}
        return {
            "intent": "validate",
            "reply": summarize_validate(payload),
            "commands": commands,
            "quarry_payload": payload,
        }

    if "explain" in text:
        result = run_quarry(
            [
                "explain",
                "--model",
                str(MODEL_PATH),
                "--catalog",
                "local",
                "--tenant",
                tenant_id,
                "--local-data-dir",
                str(LOCAL_DATA_DIR),
                "--input",
                str(QUERY_BY_REGION),
            ]
        )
        commands.append(result)
        payload = result.get("payload") or {}
        return {
            "intent": "explain",
            "reply": summarize_explain(payload, tenant_id),
            "commands": commands,
            "quarry_payload": payload,
        }

    if "context" in text or "playbook" in text or "search" in text:
        commands.extend(ensure_collection_synced(tenant_id, context_dir))
        if commands and not commands[-1]["ok"]:
            payload = commands[-1].get("payload") or {}
            return {
                "intent": "search",
                "reply": "Failed while preparing collection sync.",
                "commands": commands,
                "quarry_payload": payload,
            }

        search_result = run_quarry(
            [
                "search",
                "--tenant",
                tenant_id,
                "--collection",
                "sales_docs",
                "--query",
                message,
                "--top-k",
                "5",
                "--context-dir",
                str(context_dir),
            ]
        )
        commands.append(search_result)
        payload = search_result.get("payload") or {}
        return {
            "intent": "search",
            "reply": summarize_search(payload, tenant_id),
            "commands": commands,
            "quarry_payload": payload,
        }

    query_result = run_quarry(
        [
            "query",
            "--model",
            str(MODEL_PATH),
            "--catalog",
            "local",
            "--tenant",
            tenant_id,
            "--local-data-dir",
            str(LOCAL_DATA_DIR),
            "--input",
            str(QUERY_BY_REGION),
            "--format",
            "json",
        ]
    )
    commands.append(query_result)
    payload = query_result.get("payload") or {}
    return {
        "intent": "query",
        "reply": summarize_query(payload, tenant_id),
        "commands": commands,
        "quarry_payload": payload,
    }


class QuarryChatHandler(BaseHTTPRequestHandler):
    context_dir = DEFAULT_CONTEXT_DIR

    def _send_json(self, payload: Dict[str, Any], status: int = 200) -> None:
        raw = json.dumps(payload, indent=2).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self) -> None:  # noqa: N802 (BaseHTTPRequestHandler API)
        if self.path in ("/", "/index.html"):
            content = INDEX_HTML.read_bytes()
            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)
            return

        if self.path == "/api/health":
            self._send_json({"ok": True, "service": "quarry-chat-demo"})
            return

        self._send_json({"ok": False, "error": "not found"}, status=404)

    def do_POST(self) -> None:  # noqa: N802 (BaseHTTPRequestHandler API)
        if self.path != "/api/chat":
            self._send_json({"ok": False, "error": "not found"}, status=404)
            return

        content_length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(content_length) if content_length > 0 else b"{}"
        try:
            payload = json.loads(body.decode("utf-8"))
        except json.JSONDecodeError:
            self._send_json({"ok": False, "error": "invalid JSON body"}, status=400)
            return

        message = str(payload.get("message", "")).strip()
        tenant_fallback = str(payload.get("tenant_id", "tenant_123")).strip() or "tenant_123"
        if not message:
            self._send_json({"ok": False, "error": "message is required"}, status=400)
            return

        tenant_id = resolve_tenant(message, tenant_fallback)
        result = run_demo_intent(message, tenant_id, self.context_dir)
        self._send_json(
            {
                "ok": True,
                "tenant_id": tenant_id,
                "message": message,
                "intent": result["intent"],
                "reply": result["reply"],
                "quarry_payload": result["quarry_payload"],
                "commands": [
                    {
                        "ok": entry["ok"],
                        "elapsed_ms": entry["elapsed_ms"],
                        "args": entry["args"],
                        "returncode": entry["returncode"],
                    }
                    for entry in result["commands"]
                ],
            }
        )

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A003
        # Keep terminal output concise.
        return


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run Quarry demo chat app")
    parser.add_argument("--host", default="127.0.0.1", help="HTTP host (default: 127.0.0.1)")
    parser.add_argument("--port", default=8090, type=int, help="HTTP port (default: 8090)")
    parser.add_argument(
        "--context-dir",
        default=str(DEFAULT_CONTEXT_DIR),
        help="Context storage directory for collection/sync/search demo",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    context_dir = Path(args.context_dir).resolve()
    QuarryChatHandler.context_dir = context_dir

    server = ThreadingHTTPServer((args.host, args.port), QuarryChatHandler)
    print(f"Quarry chat demo listening on http://{args.host}:{args.port}")
    print(f"Context dir: {context_dir}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
