import argparse
import base64
import json
import tempfile
from pathlib import Path

from mcp_probe import ensure_session, post


TOOLSET = "SlateInspectorToolset.SlateInspectorToolset"
TOOL = "Screenshot"


def screenshot_payload(ref):
    session = ensure_session()
    body, error = post(
        {
            "jsonrpc": "2.0",
            "id": 301,
            "method": "tools/call",
            "params": {
                "name": "call_tool",
                "arguments": {
                    "toolset_name": TOOLSET,
                    "tool_name": TOOL,
                    "arguments": {"ref": ref},
                },
            },
        },
        session,
    )
    if body is None:
        raise RuntimeError(error or "MCP screenshot request failed")
    if "error" in body:
        raise RuntimeError(json.dumps(body["error"], ensure_ascii=False))
    content = body.get("result", {}).get("content", [])
    text = next(
        (
            item.get("text")
            for item in content
            if item.get("type") == "text"
        ),
        None,
    )
    if not isinstance(text, str):
        raise RuntimeError("MCP screenshot response has no text payload")
    payload = json.loads(text)
    value = payload.get("returnValue")
    if not isinstance(value, dict):
        raise RuntimeError("MCP screenshot return value is invalid")
    mime_type = value.get("mimeType")
    data = value.get("data")
    if not isinstance(mime_type, str) or not isinstance(data, str):
        raise RuntimeError("MCP screenshot image payload is invalid")
    return mime_type, base64.b64decode(data)


def run_self_test():
    value = b"\x89PNG\r\n\x1a\nfixture"
    encoded = base64.b64encode(value).decode("ascii")
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "image.png"
        path.write_bytes(base64.b64decode(encoded))
        assert path.read_bytes() == value
    return {"base64_png_round_trip": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    parser.add_argument("--ref", default="")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.output is None:
        raise ValueError("output is required")
    mime_type, data = screenshot_payload(args.ref)
    if mime_type != "image/png":
        raise RuntimeError(f"unsupported screenshot mime type: {mime_type}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(data)
    print(
        json.dumps(
            {
                "output": args.output.as_posix(),
                "mime_type": mime_type,
                "bytes": len(data),
            },
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    raise SystemExit(main())
