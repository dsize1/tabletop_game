"""最小 stdio MCP server（离线测试替身）：只实现 initialize / tools/list / tools/call。

协议：stdio 传输、**按行分隔的 JSON-RPC 2.0**。工具仅一个 `echo`，用于验证
`dev_executor.harness.load_mcp_tools` 的工具发现与调用链路，不依赖 npx / 网络。
"""

from __future__ import annotations

import json
import sys

TOOLS = [
    {
        "name": "echo",
        "description": "回显输入文本（测试替身）",
        "inputSchema": {
            "type": "object",
            "properties": {"text": {"type": "string", "description": "要回显的文本"}},
            "required": ["text"],
        },
    }
]


def _write(payload: dict) -> None:
    out = sys.stdout.buffer
    out.write((json.dumps(payload, ensure_ascii=False) + "\n").encode("utf-8"))
    out.flush()


def _error(mid: object, code: int, message: str) -> None:
    _write({"jsonrpc": "2.0", "id": mid, "error": {"code": code, "message": message}})


def _handle(msg: dict) -> None:
    mid = msg.get("id")
    method = msg.get("method")
    if mid is None:  # notification（如 notifications/initialized）：不回应
        return
    params = msg.get("params") or {}
    if method == "initialize":
        _write(
            {
                "jsonrpc": "2.0",
                "id": mid,
                "result": {
                    "protocolVersion": params.get("protocolVersion") or "2025-06-18",
                    "capabilities": {"tools": {"listChanged": False}},
                    "serverInfo": {"name": "fake-mcp", "version": "0.1.0"},
                },
            }
        )
    elif method == "tools/list":
        _write({"jsonrpc": "2.0", "id": mid, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments") or {}
        if name != "echo":
            _error(mid, -32602, f"未知工具: {name}")
            return
        _write(
            {
                "jsonrpc": "2.0",
                "id": mid,
                "result": {
                    "content": [{"type": "text", "text": f"echo:{args.get('text', '')}"}],
                    "isError": False,
                },
            }
        )
    elif method == "ping":
        _write({"jsonrpc": "2.0", "id": mid, "result": {}})
    else:
        _error(mid, -32601, f"未实现: {method}")


def main() -> None:
    for line in sys.stdin.buffer:
        raw = line.strip()
        if not raw:
            continue
        try:
            msg = json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError:
            continue
        _handle(msg)


if __name__ == "__main__":
    main()
