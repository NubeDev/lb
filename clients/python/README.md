# `lb-client` (Python)

A thin external client for a Lazybones gateway node — **authenticate → connect
→ round-trip a `Sample`**, plus the webhook third-party caller path and the
universal `POST /mcp/call` bridge. Deliberately small: the shape to extend,
not an SDK. **Zero runtime dependencies** — stdlib only (`urllib`, `hmac`,
`hashlib`, `json`).

> Scope: [`docs/scope/clients/client-libraries-scope.md`](../../docs/scope/clients/client-libraries-scope.md).
> Wire reference: [`docs/skills/ingest-series/SKILL.md`](../../docs/skills/ingest-series/SKILL.md).

## Install

The package is unpublished (vendored in-repo). Install editable from a checkout:

```bash
cd clients/python
pip install --user -e .                # registers `lb_client` on your sys.path
```

Or vendor it: copy `lb_client/` into your project — there are no transitive
deps. Requires Python 3.9+.

## Authenticate

The bearer is **either** an API key (`lbk_{ws}.{id}.{secret}`) **or** a JWT from
`/login`. The library doesn't care which — the gateway splits on the `lbk_`
prefix in one place. **Read the key from an env var; never hard-code it.**

```python
import os
from lb_client import Client

# long-lived producer (recommended): an API key minted via the admin console
client = Client("http://127.0.0.1:8080", os.environ["LB_KEY"])

# or dev/admin script: log in to get a 12h session token
client = Client("http://127.0.0.1:8080", "placeholder")
client, reply = client.login("ada", "acme")
```

## The round-trip

```python
from lb_client import write_samples, latest_sample

written = write_samples(client, [
    {
        "series": "node.cpu_temp",
        "ts": 1719800000000,
        "seq": 1,
        "payload": 61.4,
        "labels": {"host": "pi-7"},
        # producer is host-forced to the authenticated principal; omit it
    },
])
# {"accepted": 1, "committed": 1}   (the gateway drains staging on the same call)

latest = latest_sample(client, "node.cpu_temp")
# latest["sample"]["payload"] == 61.4
```

## The universal MCP bridge

Every other platform verb — `series.list`, `series.find`, `inbox.read`,
`channel.post`, … — is one `call_mcp` away without a library update:

```python
from lb_client import call_mcp

series = call_mcp(client, "series.list", {"prefix": "node."})
```

## Webhook (the third-party caller path)

A service the admin has shared a webhook secret with signs the raw body and
POSTs to `/hooks/{ws}/{id}`. **Sign the exact bytes you POST.**

```python
from lb_client import sign_webhook, post_webhook

body = b'{"event":"furnace-on"}'
sig = sign_webhook(shared_secret.encode(), body)
accepted = post_webhook(
    client, "acme", "wh_x",
    {"X-Signature": sig},
    body,
)
```

## Errors

`ApiError` carries the gateway's status + body verbatim. `is_denied()` covers
the opaque `401|403|404` statuses the gateway returns for missing-cap /
cross-workspace / unknown-record (the contract never distinguishes them):

```python
from lb_client import ApiError, write_samples

try:
    write_samples(client, samples)
except ApiError as e:
    if e.is_denied():
        ...  # opaque deny — missing cap, cross-workspace, or unknown record
    else:
        raise
```

## Run the example

```bash
make cloud                              # terminal 1: boot 127.0.0.1:8080
cd clients/python && pip install --user -e .
LB_URL=http://127.0.0.1:8080 LB_USER=ada LB_WORKSPACE=acme python example.py
# or with an API key:
LB_KEY=lbk_acme.k7f3a.ABCDEF23 python example.py
```

## Lay of the land

One verb per file (≤150 lines), per the project's FILE-LAYOUT rule:

```
lb_client/
  __init__.py    — barrel re-export
  client.py      — Client + login() + the shared urllib plumbing + ApiError
  ingest.py      — write_samples() + latest_sample() + the Sample TypedDict
  mcp.py         — call_mcp() (universal bridge)
  webhook.py     — sign_webhook() + post_webhook()
example.py       — login → write → read demo
```
