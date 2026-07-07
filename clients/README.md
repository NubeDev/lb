# `clients/` — starter client libraries for the gateway

One small client library per language — **TypeScript/Node, Python, Go, Rust** —
that authenticates to a Lazybones gateway node and proves the
**authenticate → connect → round-trip a `Sample`** path in the language's idiom.
Deliberately incomplete: the shape to extend, not an SDK. Everything else the
platform does is one `callMcp` (or `call_mcp` / `CallMCP` / `call_mcp`) away.

> **Scope:** [`../docs/scope/clients/client-libraries-scope.md`](../docs/scope/clients/client-libraries-scope.md)
> — the design, the goals, the non-goals (no full SDK, no streaming, no
> codegen), the testing plan, and the rejected alternatives.
> **Wire reference:** [`../docs/skills/ingest-series/SKILL.md`](../docs/skills/ingest-series/SKILL.md)
> — the canonical routes / payloads / gotchas these clients call.
> **Shipped status:** [`../docs/public/clients/clients.md`](../docs/public/clients/clients.md).

## The shared five-method shape

Every client exposes exactly these (idiomatic syntax per language):

| Method | Why |
|---|---|
| `Client(baseURL, bearer)` | Configure. Bearer is an API key **or** a JWT — the gateway splits on the `lbk_` prefix. |
| `login(user, workspace)` | Dev-login → 12h session token. For scripts/admin. |
| `writeSamples(samples)` | `POST /ingest` — the durable write. |
| `latestSample(series)` | `GET /series/{series}/latest` — the read-back. |
| `callMcp(tool, args)` | `POST /mcp/call` — the universal bridge to **every** other verb. |
| `signWebhook(secret, body)` + `postWebhook(ws, id, headers, body)` | The third-party caller path (HMAC-SHA256 over raw bytes). |

## The folders

| Folder | Language | Build / verify | Install |
|---|---|---|---|
| [`node-ts/`](node-ts/README.md) | TypeScript / Node.js 18+ | `pnpm install && pnpm run typecheck` | `pnpm install` (vendored; not a workspace member) |
| [`python/`](python/README.md) | Python 3.9+ (stdlib only) | `python -m py_compile lb_client/*.py` | `pip install --user -e .` |
| [`go/`](go/README.md) | Go 1.22+ (stdlib only) | `go build ./...` | `go get github.com/lazybones/lb/clients/go` |
| [`rust/`](rust/README.md) | Rust (reqwest + hmac + sha2) | `cargo build` | path / git dep |

Each folder is **self-contained** — own manifest, own README, own example. None
of them is a member of the core `rust/Cargo.toml` workspace or the root
`pnpm-workspace.yaml`, so a change here cannot break the core build.

## The 60-second round-trip (any language)

```bash
make cloud                            # terminal 1: boot 127.0.0.1:8080
```

Then in another terminal, from any of the four folders (see its README for the
exact command):

1. **Get a bearer.** Mint an API key via the admin console (`/admin/apikeys`
   with a session token) for a long-lived producer, OR call `login("ada", "acme")`
   for a dev script.
2. `writeSamples([{series, ts, seq, payload}])` → `accepted=N committed=N`.
3. `latestSample(series)` → the value you just wrote.

## What's NOT here (intentional)

- **No typed wrappers for every verb.** A wrapper per verb per language × four
  languages = a maintenance wall. The MCP bridge (`callMcp`) is the universal
  escape hatch — wrap the verbs *you* actually call, in *your* code.
- **No streaming/SSE helper.** `EventSource` over `/series/{s}/stream` is a
  v2 once a real consumer needs live motion instead of poll-then-read.
- **No retry/backoff, no pagination iterator.** Each is a design decision the
  library refuses to lock in prematurely.
- **No registry publication (npm / PyPI / pkg.go.dev / crates.io).** The README
  in each folder says how to vendor / path-depend. Publish the day the v1 shape
  is signed off and a real external consumer exists.

## Rule 9 (no mocks) compliance

Each library is a thin HTTP/MCP caller — there is nothing in-process to mock.
The live round-trip recipes (in each `README.md`) hit a **real** `make cloud`
node, seeded with real records via the real write paths, exactly as the testing
policy requires. A future `clients/rust` integration test will boot the
in-process `test_gateway` (the same pattern `rust/role/gateway` uses) for a
no-cloud-required round-trip.

## Related

- Auth: [`../docs/scope/auth-caps/api-keys-scope.md`](../docs/scope/auth-caps/api-keys-scope.md),
  [`../docs/public/auth-caps/auth-caps.md`](../docs/public/auth-caps/auth-caps.md).
- Ingest: [`../docs/scope/ingest/ingest-scope.md`](../docs/scope/ingest/ingest-scope.md),
  [`../docs/public/ingest/ingest.md`](../docs/public/ingest/ingest.md).
- Webhooks: [`../docs/scope/ingest/webhooks-scope.md`](../docs/scope/ingest/webhooks-scope.md),
  [`../docs/public/ingest/webhooks.md`](../docs/public/ingest/webhooks.md).
- MCP bridge: [`../docs/scope/mcp/`](../docs/scope/mcp/),
  route [`../rust/role/gateway/src/routes/mcp.rs`](../rust/role/gateway/src/routes/mcp.rs).
- The in-repo CLI (the fourth client of the gateway, same posture):
  [`../docs/scope/cli/operator-cli-scope.md`](../docs/scope/cli/operator-cli-scope.md).
