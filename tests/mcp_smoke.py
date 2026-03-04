#!/usr/bin/env python3
"""Smoke test Quarry MCP server.

Checks:
- initialize handshake
- tools/list contains expected tools
- tools/call quarry_validate succeeds on example model
- tools/call quarry_query succeeds on example query fixture
- tools/call quarry_collection_create/list/sync/search succeeds on local fixture docs
"""

from __future__ import annotations

import json
import tempfile
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SERVER = REPO_ROOT / "tools" / "mcp" / "quarry_mcp_server.py"


def send(proc: subprocess.Popen[bytes], payload: dict) -> None:
    body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    header = f"Content-Length: {len(body)}\r\n\r\n".encode("ascii")
    assert proc.stdin is not None
    proc.stdin.write(header)
    proc.stdin.write(body)
    proc.stdin.flush()


def recv(proc: subprocess.Popen[bytes]) -> dict:
    assert proc.stdout is not None
    headers = {}
    while True:
        line = proc.stdout.readline()
        if not line:
            raise RuntimeError("MCP server closed stream unexpectedly")
        if line in (b"\r\n", b"\n"):
            break
        text = line.decode("ascii", errors="ignore").strip()
        if ":" in text:
            k, v = text.split(":", 1)
            headers[k.strip().lower()] = v.strip()

    content_length = int(headers.get("content-length", "0"))
    raw = proc.stdout.read(content_length)
    return json.loads(raw.decode("utf-8"))


def expect(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    cmd = ["python3", str(SERVER)]
    proc = subprocess.Popen(
        cmd,
        cwd=str(REPO_ROOT),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    try:
        send(
            proc,
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "mcp-smoke", "version": "0.1.0"},
                },
            },
        )
        init = recv(proc)
        expect("result" in init, "initialize did not return result")

        send(proc, {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
        tools = recv(proc)
        names = {t["name"] for t in tools["result"]["tools"]}
        expected = {
            "quarry_validate",
            "quarry_query",
            "quarry_explain",
            "quarry_collection_create",
            "quarry_collection_list",
            "quarry_sync",
            "quarry_search",
        }
        expect(names == expected, f"tools/list mismatch: {names}")

        send(
            proc,
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "quarry_validate",
                    "arguments": {"model_path": "models/example/model.yml"},
                },
            },
        )
        validate = recv(proc)
        expect("result" in validate, "tools/call did not return result")
        result = validate["result"]
        expect(not result.get("isError", False), "quarry_validate returned error")
        text = result["content"][0]["text"]
        payload = json.loads(text)
        expect(payload.get("status") == "ok", "quarry_validate status was not ok")

        send(
            proc,
            {
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "quarry_query",
                    "arguments": {
                        "model_path": "models/example/model.yml",
                        "catalog": "local",
                        "tenant_id": "tenant_123",
                        "local_data_dir": "models/example/data",
                        "query_file": "models/example/query_by_region.json",
                        "format": "json",
                    },
                },
            },
        )
        query = recv(proc)
        expect("result" in query, "quarry_query did not return result")
        query_result = query["result"]
        expect(not query_result.get("isError", False), "quarry_query returned error")
        query_payload = json.loads(query_result["content"][0]["text"])
        expect(query_payload.get("status") == "ok", "quarry_query status was not ok")

        rows = query_payload.get("data", {}).get("rows", [])
        expect(len(rows) == 2, f"expected 2 rows, got {len(rows)}")
        by_region = {row.get("orders.region"): row.get("revenue") for row in rows}
        expect(by_region.get("EU") == 250.0, f"EU revenue mismatch: {by_region.get('EU')}")
        expect(by_region.get("NA") == 100.0, f"NA revenue mismatch: {by_region.get('NA')}")
        expect(
            round(sum(by_region.values()), 4) == 350.0,
            f"total revenue mismatch: {sum(by_region.values())}",
        )

        meta = query_payload.get("meta", {})
        expect(meta.get("tenant_id") == "tenant_123", "tenant_id meta mismatch")
        expect(meta.get("catalog") == "local", "catalog meta mismatch")

        with tempfile.TemporaryDirectory(prefix="quarry-mcp-context-") as tmpdir:
            context_dir = Path(tmpdir) / "context"
            docs_dir = Path(tmpdir) / "docs"
            docs_dir.mkdir(parents=True, exist_ok=True)
            (docs_dir / "sales.txt").write_text(
                "Revenue playbook for EMEA and NA enterprise growth.",
                encoding="utf-8",
            )
            sync_config_path = Path(tmpdir) / "sync_config.json"
            sync_config_path.write_text(
                json.dumps({"paths": [str(docs_dir)], "recursive": True, "extensions": ["txt"]}),
                encoding="utf-8",
            )

            send(
                proc,
                {
                    "jsonrpc": "2.0",
                    "id": 5,
                    "method": "tools/call",
                    "params": {
                        "name": "quarry_collection_create",
                        "arguments": {
                            "tenant_id": "tenant_123",
                            "name": "sales_docs",
                            "context_dir": str(context_dir),
                        },
                    },
                },
            )
            create = recv(proc)
            expect("result" in create, "collection create missing result")
            expect(not create["result"].get("isError", False), "collection create errored")

            send(
                proc,
                {
                    "jsonrpc": "2.0",
                    "id": 6,
                    "method": "tools/call",
                    "params": {
                        "name": "quarry_collection_list",
                        "arguments": {
                            "tenant_id": "tenant_123",
                            "context_dir": str(context_dir),
                        },
                    },
                },
            )
            listed = recv(proc)
            expect("result" in listed, "collection list missing result")
            expect(not listed["result"].get("isError", False), "collection list errored")

            send(
                proc,
                {
                    "jsonrpc": "2.0",
                    "id": 7,
                    "method": "tools/call",
                    "params": {
                        "name": "quarry_sync",
                        "arguments": {
                            "tenant_id": "tenant_123",
                            "collection": "sales_docs",
                            "connector": "filesystem",
                            "config_file": str(sync_config_path),
                            "context_dir": str(context_dir),
                        },
                    },
                },
            )
            synced = recv(proc)
            expect("result" in synced, "sync missing result")
            expect(not synced["result"].get("isError", False), "sync errored")

            send(
                proc,
                {
                    "jsonrpc": "2.0",
                    "id": 8,
                    "method": "tools/call",
                    "params": {
                        "name": "quarry_search",
                        "arguments": {
                            "tenant_id": "tenant_123",
                            "collection": "sales_docs",
                            "query": "revenue",
                            "top_k": 3,
                            "context_dir": str(context_dir),
                        },
                    },
                },
            )
            searched = recv(proc)
            expect("result" in searched, "search missing result")
            expect(not searched["result"].get("isError", False), "search errored")

        print("MCP smoke test passed")
        return 0
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as err:
        print(f"MCP smoke test failed: {err}", file=sys.stderr)
        raise SystemExit(1)
