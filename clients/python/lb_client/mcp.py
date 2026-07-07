"""``POST /mcp/call {tool, args}`` — the universal host-mediated bridge. Every
platform verb that isn't wrapped by name in this library is reachable from here
without a library update (see ``docs/skills/ingest-series/SKILL.md`` for the
verb table). Re-checks the workspace + ``mcp:<tool>:call`` capability."""

from __future__ import annotations

from typing import Any

from lb_client.client import Client


def call_mcp(client: Client, tool: str, args: Any = None) -> Any:
    """Call ``tool`` with ``args`` over the bridge. ``args`` may be any
    JSON-serializable value (pass ``None`` for a no-arg tool). Returns the
    tool's raw JSON output."""
    return client._request_json(
        "POST", "/mcp/call", body={"tool": tool, "args": args if args is not None else {}},
    )
