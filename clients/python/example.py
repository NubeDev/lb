"""roundtrip.py — login → write a Sample → read it back, against a real
`make cloud` node.

Run with:
    make cloud                            # terminal 1: boot 127.0.0.1:8080
    cd clients/python
    LB_URL=http://127.0.0.1:8080 LB_USER=ada LB_WORKSPACE=acme python example.py
    # or with an API key:
    LB_KEY=lbk_acme.k7f3a.ABCDEF23 python example.py
"""

from __future__ import annotations

import os
import time

from lb_client import (
    Client,
    call_mcp,
    latest_sample,
    write_samples,
)


def main() -> None:
    url = os.environ.get("LB_URL", "http://127.0.0.1:8080")
    key = os.environ.get("LB_KEY")
    user = os.environ.get("LB_USER", "ada")
    ws = os.environ.get("LB_WORKSPACE", "acme")

    client = Client(url, "placeholder")
    if key:
        client = client.with_bearer(key)
    else:
        client, reply = client.login(user, ws)
        print(f"logged in as {reply['principal']} in {reply['workspace']}")

    # 1. Push one Sample. `producer` is host-forced to the principal, so omit it.
    written = write_samples(client, [
        {
            "series": "demo.cpu_temp",
            "ts": int(time.time() * 1000),
            "seq": 1,
            "payload": 61.4,
            "labels": {"host": "pi-7"},
        },
    ])
    print(f"accepted={written['accepted']} committed={written['committed']}")

    # 2. Read the newest value back — the round-trip.
    latest = latest_sample(client, "demo.cpu_temp")
    print("latest sample:", latest)

    # 3. The universal MCP bridge: every other verb is one call away.
    series_list = call_mcp(client, "series.list", {})
    print("series in workspace:", series_list)


if __name__ == "__main__":
    main()
