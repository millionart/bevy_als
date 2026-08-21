#!/usr/bin/env python
"""Minimal Streamable-HTTP MCP client probe for the UE editor MCP server (M0.4).

Usage:
  python tools/mcp_probe.py list_toolsets
  python tools/mcp_probe.py call <toolset_name|-> <tool_name> [json_arguments]
"""
import json
import sys
import urllib.request
import urllib.error

URL = "http://127.0.0.1:8000/mcp"
SID_FILE = ".mcp-session-id"


def post(payload: dict, session_id: str | None) -> tuple[dict | None, str | None]:
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
    }
    if session_id:
        headers["Mcp-Session-Id"] = session_id
    req = urllib.request.Request(
        URL, data=json.dumps(payload).encode("utf-8"), headers=headers, method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            sid = resp.headers.get("Mcp-Session-Id")
            body = resp.read().decode("utf-8", errors="replace")
            return (json.loads(body) if body.strip() else {}), sid
    except urllib.error.HTTPError as e:
        return None, f"HTTP {e.code}: {e.read().decode('utf-8', errors='replace')[:500]}"


def load_sid() -> str | None:
    try:
        with open(SID_FILE, encoding="utf-8") as f:
            sid = f.read().strip()
            return sid or None
    except FileNotFoundError:
        return None


def save_sid(sid: str) -> None:
    with open(SID_FILE, "w", encoding="utf-8") as f:
        f.write(sid)


def ensure_session() -> str:
    sid = load_sid()
    if sid:
        # verify still valid with a cheap call
        body, _ = post({"jsonrpc": "2.0", "id": 0, "method": "tools/list", "params": {}}, sid)
        if body is not None and "error" not in body:
            return sid
        sid = None
    body, sid = post(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "bevy_als-probe", "version": "0.1"},
            },
        },
        None,
    )
    if body is None or "result" not in body or not sid:
        print(f"initialize failed: {err_or_body(body, sid)}", file=sys.stderr)
        sys.exit(2)
    save_sid(sid)
    # notification: empty body expected
    post({"jsonrpc": "2.0", "method": "notifications/initialized"}, sid)
    return sid


def err_or_body(body: dict | None, sid: str | None) -> str:
    if body is None:
        return "no response"
    if "error" in body:
        return json.dumps(body["error"])[:500]
    if not sid:
        return "no Mcp-Session-Id header returned"
    return json.dumps(body)[:500]


def extract_text(body: dict) -> str:
    result = body.get("result")
    if result is None:
        return json.dumps(body.get("error", body), indent=1)[:2000]
    content = result.get("content") or []
    parts = []
    for item in content:
        if item.get("type") == "text":
            parts.append(item.get("text", ""))
        else:
            parts.append(json.dumps(item)[:500])
    return "\n".join(parts)


def main() -> None:
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        sys.exit(1)
    sid = ensure_session()
    if args[0] == "list_toolsets":
        body, _ = post({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                        "params": {"name": "list_toolsets", "arguments": {}}}, sid)
        print(extract_text(body))
    elif args[0] == "call":
        toolset = None if len(args) < 2 or args[1] == "-" else args[1]
        tool = args[2] if len(args) > 2 else ""
        arguments = json.loads(args[3]) if len(args) > 3 else {}
        params = {"name": tool, "arguments": arguments}
        if toolset:
            params["toolset_name"] = toolset
        body, _ = post({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
                        "params": params}, sid)
        print(extract_text(body))
    elif args[0] == "raw_tools_list":
        body, _ = post({"jsonrpc": "2.0", "id": 5, "method": "tools/list", "params": {}}, sid)
        print(extract_text(body))
    else:
        print(f"unknown subcommand: {args[0]}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
