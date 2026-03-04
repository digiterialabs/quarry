#!/usr/bin/env python3
"""Minimal MCP stdio server for Quarry CLI tools.

Provides tools:
- quarry_validate
- quarry_query
- quarry_explain
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, Optional


SERVER_NAME = "quarry-mcp"
SERVER_VERSION = "0.1.0"
DEFAULT_PROTOCOL_VERSION = "2025-11-05"


SCRIPT_PATH = Path(__file__).resolve()
REPO_ROOT = Path(os.environ.get("QUARRY_REPO_ROOT", str(SCRIPT_PATH.parents[2]))).resolve()


def _send_message(payload: Dict[str, Any]) -> None:
    body = json.dumps(payload, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    header = f"Content-Length: {len(body)}\r\n\r\n".encode("ascii")
    sys.stdout.buffer.write(header)
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()


def _read_message() -> Optional[Dict[str, Any]]:
    headers: Dict[str, str] = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        text = line.decode("ascii", errors="ignore").strip()
        if ":" in text:
            key, value = text.split(":", 1)
            headers[key.strip().lower()] = value.strip()

    content_length = headers.get("content-length")
    if not content_length:
        return None

    try:
        n = int(content_length)
    except ValueError:
        return None

    raw = sys.stdin.buffer.read(n)
    if not raw:
        return None
    return json.loads(raw.decode("utf-8"))


def _tool_definitions() -> list[Dict[str, Any]]:
    return [
        {
            "name": "quarry_validate",
            "description": "Validate a Quarry semantic model YAML file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_path": {"type": "string"},
                },
                "required": ["model_path"],
                "additionalProperties": False,
            },
        },
        {
            "name": "quarry_query",
            "description": (
                "Execute a Quarry semantic query and return JSON results. "
                "Provide query_json or query_file."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_path": {"type": "string"},
                    "catalog": {"type": "string", "enum": ["local", "glue"]},
                    "tenant_id": {"type": "string"},
                    "query_json": {"type": "object"},
                    "query_file": {"type": "string"},
                    "local_data_dir": {"type": "string"},
                    "format": {"type": "string", "enum": ["json"]},
                },
                "required": ["model_path", "catalog", "tenant_id"],
                "additionalProperties": False,
            },
        },
        {
            "name": "quarry_explain",
            "description": (
                "Resolve a Quarry semantic query and return the logical plan "
                "without query execution."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_path": {"type": "string"},
                    "catalog": {"type": "string", "enum": ["local", "glue"]},
                    "tenant_id": {"type": "string"},
                    "query_json": {"type": "object"},
                    "query_file": {"type": "string"},
                    "local_data_dir": {"type": "string"},
                },
                "required": ["model_path", "catalog", "tenant_id"],
                "additionalProperties": False,
            },
        },
    ]


def _quarry_base_cmd() -> list[str]:
    # Optional fast path to compiled binary.
    quarry_bin = os.environ.get("QUARRY_BIN", "").strip()
    if quarry_bin:
        return [quarry_bin]

    return ["cargo", "run", "-q", "-p", "quarry-cli", "--"]


def _run_quarry(args: list[str]) -> subprocess.CompletedProcess[str]:
    cmd = _quarry_base_cmd() + args
    return subprocess.run(
        cmd,
        cwd=str(REPO_ROOT),
        text=True,
        capture_output=True,
        check=False,
    )


def _error_text(msg: str) -> Dict[str, Any]:
    return {"isError": True, "content": [{"type": "text", "text": msg}]}


def _success_text(text: str) -> Dict[str, Any]:
    return {"content": [{"type": "text", "text": text}]}


def _tool_error(code: str, message: str, details: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
    payload = {
        "schema_version": "v1",
        "status": "error",
        "error": {
            "code": code,
            "message": message,
            "details": details or {},
        },
    }
    return _error_text(json.dumps(payload))


def _query_input_file(arguments: Dict[str, Any]) -> tuple[Optional[str], Optional[str]]:
    query_file = arguments.get("query_file")
    query_json = arguments.get("query_json")

    if query_file and query_json:
        return None, "Provide only one of query_file or query_json"

    if query_file:
        return str(query_file), None

    if query_json is None:
        return None, "Missing query input: provide query_json or query_file"

    fd, tmp_path = tempfile.mkstemp(prefix="quarry-mcp-query-", suffix=".json")
    os.close(fd)
    with open(tmp_path, "w", encoding="utf-8") as f:
        json.dump(query_json, f)
    return tmp_path, None


def _handle_tool_call(name: str, arguments: Dict[str, Any]) -> Dict[str, Any]:
    if name == "quarry_validate":
        model_path = arguments.get("model_path")
        if not model_path:
            return _tool_error(
                "MISSING_ARGUMENT",
                "quarry_validate requires model_path",
                {"required": ["model_path"]},
            )

        proc = _run_quarry(["validate", "--model", str(model_path)])
        if proc.returncode != 0:
            return _error_text(proc.stderr.strip() or proc.stdout.strip())
        return _success_text(proc.stdout.strip())

    if name in {"quarry_query", "quarry_explain"}:
        model_path = arguments.get("model_path")
        catalog = arguments.get("catalog")
        tenant_id = arguments.get("tenant_id")
        local_data_dir = arguments.get("local_data_dir")
        if not model_path or not catalog or not tenant_id:
            return _tool_error(
                "MISSING_ARGUMENT",
                f"{name} requires model_path, catalog, and tenant_id",
                {"required": ["model_path", "catalog", "tenant_id"]},
            )

        input_path, input_err = _query_input_file(arguments)
        if input_err:
            return _tool_error(
                "INVALID_INPUT",
                f"{name}: {input_err}",
                {"accepted": ["query_json", "query_file"]},
            )

        subcmd = "query" if name == "quarry_query" else "explain"
        args = [
            subcmd,
            "--model",
            str(model_path),
            "--catalog",
            str(catalog),
            "--tenant",
            str(tenant_id),
            "--input",
            str(input_path),
        ]
        if local_data_dir:
            args.extend(["--local-data-dir", str(local_data_dir)])
        if name == "quarry_query":
            fmt = arguments.get("format", "json")
            args.extend(["--format", str(fmt)])

        try:
            proc = _run_quarry(args)
        finally:
            if arguments.get("query_json") is not None and input_path:
                try:
                    os.remove(input_path)
                except OSError:
                    pass

        if proc.returncode != 0:
            return _error_text(proc.stderr.strip() or proc.stdout.strip())
        return _success_text(proc.stdout.strip())

    return _error_text(f"Unknown tool: {name}")


def _handle_request(msg: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    method = msg.get("method")
    msg_id = msg.get("id")
    params = msg.get("params", {}) or {}

    # Notifications don't require responses.
    if msg_id is None:
        return None

    if method == "initialize":
        protocol_version = params.get("protocolVersion", DEFAULT_PROTOCOL_VERSION)
        return {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "protocolVersion": protocol_version,
                "capabilities": {"tools": {"listChanged": False}},
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
            },
        }

    if method == "tools/list":
        return {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {"tools": _tool_definitions()},
        }

    if method == "tools/call":
        name = params.get("name")
        arguments = params.get("arguments", {}) or {}
        if not name:
            return {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": _error_text("tools/call: missing tool name"),
            }
        return {
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": _handle_tool_call(str(name), dict(arguments)),
        }

    if method == "ping":
        return {"jsonrpc": "2.0", "id": msg_id, "result": {}}

    return {
        "jsonrpc": "2.0",
        "id": msg_id,
        "error": {"code": -32601, "message": f"Method not found: {method}"},
    }


def main() -> int:
    while True:
        msg = _read_message()
        if msg is None:
            return 0
        resp = _handle_request(msg)
        if resp is not None:
            _send_message(resp)


if __name__ == "__main__":
    raise SystemExit(main())
