"""MCP client helper (Windows-safe).

Cel biznesowy:
- To jest mały, produkcyjny skrypt narzędziowy do testów E2E MCP na Windows.
- Unikamy problemów z kodowaniem (cp1252) oraz "findstr line too long".

Jak działa:
- Uruchamia `target/release/live-function-tree-mcp.exe` jako subprocess.
- Wysyła pojedynczą wiadomość JSON-RPC na stdin i czyta stdout.
- Parsuje odpowiedź JSON i wypisuje:
  - cały JSON (pretty)
  - opcjonalnie: listę nazw tooli lub uproszczony wynik.

Użycie:
- tools/list:
  python scripts\\mcp_call.py tools/list

- tools/call (przykład):
  python scripts\\mcp_call.py tools/call lft_clear_db '{"dry_run": true}'

Uwaga:
- Ten skrypt celowo nie używa emoji w output, żeby działał wszędzie.
"""

from __future__ import annotations

import json
import subprocess
import sys
from typing import Any, Dict, Optional

import ast
import pathlib


def _force_utf8_stdio() -> None:
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass


def _server_exe_path() -> str:
    return r"c:\taxi\TaxiTech\superagent\live-function-tree-mcp-rust\target\release\live-function-tree-mcp.exe"


def call_once(msg: Dict[str, Any], timeout_s: int = 30) -> Dict[str, Any]:
    process = subprocess.Popen(
        [_server_exe_path()],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )

    input_data = json.dumps(msg, ensure_ascii=False) + "\n"
    stdout, stderr = process.communicate(input=input_data, timeout=timeout_s)

    if stderr.strip():
        # stderr is informative logs; print but don't fail.
        print("STDERR:")
        print(stderr.strip())

    stdout_str = stdout.strip()
    if not stdout_str:
        raise RuntimeError("Empty stdout from MCP server")

    try:
        return json.loads(stdout_str)
    except json.JSONDecodeError as e:
        raise RuntimeError(f"Invalid JSON from server: {e}\nRAW: {stdout_str[:2000]}")


def main(argv: list[str]) -> int:
    _force_utf8_stdio()

    if len(argv) >= 2 and argv[1] in {"-h", "--help", "help"}:
        print("Usage:")
        print("  python scripts\\mcp_call.py tools/list")
        print("  python scripts\\mcp_call.py tools/call <tool_name> <json_args>")
        print("  python scripts\\mcp_call.py tools/call <tool_name> --args-file <path>")
        print("Optional:")
        print("  --timeout <seconds>")
        print("Notes:")
        print("  - <json_args> should be a JSON object with tool arguments")
        print("  - --args-file can contain either that JSON object, OR a full JSON-RPC request")
        return 0

    if len(argv) < 2:
        print("Usage:")
        print("  python scripts\\mcp_call.py tools/list")
        print("  python scripts\\mcp_call.py tools/call <tool_name> <json_args>")
        return 2

    method = argv[1]

    if method == "tools/list":
        msg = {"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}
        resp = call_once(msg)
        print(json.dumps(resp, indent=2, ensure_ascii=False))
        tools = resp.get("result", {}).get("tools", [])
        names = [t.get("name") for t in tools if isinstance(t, dict) and t.get("name")]
        if names:
            print("\nTool names:")
            for n in names:
                print(n)
        return 0

    if method == "tools/call":
        if len(argv) < 4:
            print("Usage: python scripts\\mcp_call.py tools/call <tool_name> <json_args>")
            print("  or:  python scripts\\mcp_call.py tools/call <tool_name> --args-file <path>")
            print("Optional:")
            print("  --timeout <seconds>")
            return 2

        tool_name = argv[2]

        # Optional timeout override
        timeout_s = 300
        if "--timeout" in argv:
            try:
                idx = argv.index("--timeout")
                timeout_s = int(argv[idx + 1])
            except Exception:
                print("--timeout requires an integer number of seconds")
                return 2

        # PowerShell quoting can be painful. We support:
        # - strict JSON (recommended)
        # - Python literal dict (single quotes) as fallback
        # - --args-file <path> containing JSON
        if argv[3] == "--args-file":
            if len(argv) < 5:
                print("--args-file requires a path")
                return 2
            path = pathlib.Path(argv[4])
            raw = path.read_text(encoding="utf-8", errors="replace")
        else:
            raw = argv[3]

        def _parse_args_payload(payload: str) -> Dict[str, Any]:
            payload = payload.strip()
            if not payload:
                return {}

            try:
                parsed_any = json.loads(payload)
            except json.JSONDecodeError:
                try:
                    parsed_any = ast.literal_eval(payload)
                except Exception as e:
                    raise ValueError(f"json_args must be valid JSON or a Python dict literal: {e}")

            if not isinstance(parsed_any, dict):
                raise ValueError("args must be an object/dict")

            # If someone passes a full JSON-RPC request (common in our repo fixtures),
            # extract the inner tool 'arguments'.
            if (
                parsed_any.get("jsonrpc")
                and parsed_any.get("method") in {"tools/call", "tools/list"}
                and isinstance(parsed_any.get("params"), dict)
            ):
                params = parsed_any["params"]
                if parsed_any["method"] == "tools/call":
                    inner_args = params.get("arguments")
                    if inner_args is None:
                        return {}
                    if not isinstance(inner_args, dict):
                        raise ValueError("params.arguments must be an object/dict")
                    return inner_args
                # For tools/list: nothing to extract, treat as empty args.
                return {}

            return parsed_any

        try:
            args = _parse_args_payload(raw)
        except ValueError as e:
            print(str(e))
            return 2

        msg = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": tool_name, "arguments": args},
        }
        resp = call_once(msg, timeout_s=timeout_s)
        print(json.dumps(resp, indent=2, ensure_ascii=False))
        return 0

    print(f"Unknown method: {method}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
