#!/usr/bin/env python3
"""Install Quarry integrations for Codex, Claude Code, and Cursor.

Usage:
  python3 scripts/install_integrations.py --codex --claude --cursor
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import time
from pathlib import Path

try:  # Python 3.11+
    import tomllib as _toml_parser
except ModuleNotFoundError:  # Python 3.10 and lower
    try:
        import tomli as _toml_parser  # type: ignore[no-redef]
    except ModuleNotFoundError:
        _toml_parser = None

MARKER_START = "# >>> quarry-mcp >>>"
MARKER_END = "# <<< quarry-mcp <<<"


def fail(message: str, *, hint: str | None = None) -> int:
    print(f"error: {message}", file=sys.stderr)
    if hint:
        print(f"hint: {hint}", file=sys.stderr)
    return 1


def ensure_parent(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def backup_file(path: Path, backup_dir: Path) -> None:
    if not path.exists():
        return
    backup_dir.mkdir(parents=True, exist_ok=True)
    ts = time.strftime("%Y%m%d-%H%M%S")
    backup_name = f"{path.name}.{ts}.bak"
    shutil.copy2(path, backup_dir / backup_name)


def backup_dir_tree(path: Path, backup_dir: Path) -> None:
    if not path.exists():
        return
    backup_dir.mkdir(parents=True, exist_ok=True)
    ts = time.strftime("%Y%m%d-%H%M%S")
    dst = backup_dir / f"{path.name}.{ts}.bak"
    shutil.copytree(path, dst)


def validate_toml_or_fail(path: Path, content: str) -> None:
    if not content.strip():
        return

    # Full TOML validation when parser is available.
    if _toml_parser is not None:
        try:
            _toml_parser.loads(content)
            return
        except Exception as err:
            raise ValueError(
                f"Malformed TOML in {path}: {err}. "
                "Fix the file first, or restore from backup and retry."
            ) from err

    # Fallback safety checks for environments without tomllib/tomli.
    try:
        if "\x00" in content:
            raise ValueError("NUL byte found")
        if content.count(MARKER_START) != content.count(MARKER_END):
            raise ValueError("Unbalanced Quarry MCP markers")
    except ValueError as err:
        raise ValueError(
            f"Malformed TOML in {path}: {err}. "
            "Fix the file first, or restore from backup and retry."
        ) from err


def build_codex_block(server_script: Path) -> str:
    script = str(server_script)
    lines = [
        MARKER_START,
        "[mcp_servers.quarry]",
        'command = "python3"',
        f'args = ["{script}"]',
        MARKER_END,
        "",
    ]
    return "\n".join(lines)


def patch_codex_config(config_path: Path, server_script: Path, backup_dir: Path) -> str:
    existing = ""
    if config_path.exists():
        existing = config_path.read_text(encoding="utf-8")

    validate_toml_or_fail(config_path, existing)

    block = build_codex_block(server_script)
    if MARKER_START in existing and MARKER_END in existing:
        before, rest = existing.split(MARKER_START, 1)
        _middle, after = rest.split(MARKER_END, 1)
        patched = before.rstrip() + "\n\n" + block + after.lstrip("\n")
    else:
        suffix = "\n" if existing and not existing.endswith("\n") else ""
        patched = existing + suffix
        if patched.strip():
            patched += "\n"
        patched += block

    validate_toml_or_fail(config_path, patched)
    backup_file(config_path, backup_dir)
    ensure_parent(config_path)
    config_path.write_text(patched, encoding="utf-8")
    return str(config_path)


def load_json_file(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise ValueError(
            f"Malformed JSON in {path}: {err}. "
            "Fix the file first, or restore from backup and retry."
        ) from err


def write_mcp_json(path: Path, server_script: Path, backup_dir: Path) -> str:
    payload = load_json_file(path)
    servers = payload.get("mcpServers")
    if servers is None:
        payload["mcpServers"] = {}
        servers = payload["mcpServers"]
    if not isinstance(servers, dict):
        raise ValueError(
            f"Expected object at {path}:mcpServers, found {type(servers).__name__}."
        )

    servers["quarry"] = {
        "command": "python3",
        "args": [str(server_script)],
    }

    backup_file(path, backup_dir)
    ensure_parent(path)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return str(path)


def install_skill(repo_root: Path, codex_home: Path, backup_dir: Path) -> str:
    source = repo_root / "skills" / "quarry-analytics"
    if not source.exists():
        raise ValueError(
            f"Missing skill directory: {source}. Expected skills/quarry-analytics in repo."
        )

    target = codex_home / "skills" / "quarry-analytics"
    if target.exists():
        backup_dir_tree(target, backup_dir)
        shutil.rmtree(target)
    ensure_parent(target)
    shutil.copytree(source, target)
    return str(target)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Install Quarry integrations for Codex, Claude Code, and Cursor"
    )
    parser.add_argument("--codex", action="store_true", help="Install Codex MCP + skill")
    parser.add_argument("--claude", action="store_true", help="Write project .mcp.json")
    parser.add_argument("--cursor", action="store_true", help="Write project .cursor/mcp.json")
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root (default: inferred from script path)",
    )
    parser.add_argument(
        "--codex-home",
        type=Path,
        default=Path.home() / ".codex",
        help="Codex home directory (default: ~/.codex)",
    )
    parser.add_argument(
        "--backup-dir",
        type=Path,
        default=Path.home() / ".quarry" / "backups",
        help="Backup directory for patched files",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not (args.codex or args.claude or args.cursor):
        return fail(
            "No targets selected.",
            hint="Use one or more flags: --codex --claude --cursor",
        )

    repo_root = args.repo_root.resolve()
    server_script = repo_root / "tools" / "mcp" / "quarry_mcp_server.py"
    if not server_script.exists():
        return fail(
            f"Missing MCP server script: {server_script}",
            hint="Run this installer from the Quarry repository root or pass --repo-root.",
        )

    backup_dir = args.backup_dir.resolve()
    results: list[str] = []

    try:
        if args.codex:
            codex_home = args.codex_home.resolve()
            config_path = codex_home / "config.toml"
            patched = patch_codex_config(config_path, server_script, backup_dir)
            skill_path = install_skill(repo_root, codex_home, backup_dir)
            results.append(f"Codex config patched: {patched}")
            results.append(f"Codex skill installed: {skill_path}")

        if args.claude:
            claude_path = repo_root / ".mcp.json"
            written = write_mcp_json(claude_path, server_script, backup_dir)
            results.append(f"Claude project MCP config written: {written}")

        if args.cursor:
            cursor_path = repo_root / ".cursor" / "mcp.json"
            written = write_mcp_json(cursor_path, server_script, backup_dir)
            results.append(f"Cursor project MCP config written: {written}")

    except ValueError as err:
        return fail(str(err), hint=f"Backups (if created): {backup_dir}")
    except OSError as err:
        return fail(str(err), hint=f"Backups (if created): {backup_dir}")

    for line in results:
        print(f"ok: {line}")

    print("\nNext steps:")
    if args.codex:
        print("- Codex: restart or reload MCP servers to pick up config changes.")
    if args.claude:
        print("- Claude Code: reopen this project so .mcp.json is loaded.")
    if args.cursor:
        print("- Cursor: reopen workspace, then enable MCP servers in settings if prompted.")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
