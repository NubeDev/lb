# Client libraries — external callers of the gateway

Status: **TODO stub** — fills in when `scope/clients/client-libraries-scope.md`
ships. The five-method thin client per language (TypeScript/Node, Python, Go,
Rust) lives under `clients/<lang>/` in the repo root.

> Scope: [`../../scope/clients/client-libraries-scope.md`](../../scope/clients/client-libraries-scope.md).
> Session: [`../../sessions/clients/client-libraries-session.md`](../../sessions/clients/client-libraries-session.md).
> Canonical wire reference: [`../skills/ingest-series/SKILL.md`](../skills/ingest-series/SKILL.md)
> (the routes these clients call).

## What ships here (when populated)

One folder per language under `clients/`:

- `clients/node-ts/` — TypeScript / Node.js (`tsc`, no transpile pipeline).
- `clients/python/` — Python 3 (stdlib `urllib` + `hmac`; no SDK dep).
- `clients/go/` — Go module (`net/http` + `crypto/hmac`).
- `clients/rust/` — Rust crate (`reqwest` + `hmac`).

Each exposes the same five-method shape: `Client` (base URL + bearer),
`login()`, `writeSamples()`, `latestSample()`, `callMcp()`, plus a
`signWebhook()` / `postWebhook()` helper for the third-party caller path.

TODO on ship: per-language install snippets, the round-trip example, and the
`make cloud` test recipe.
