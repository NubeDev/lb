# Clients — starter client libraries for the gateway (session)

- Date: 2026-07-05
- Scope: ../../scope/clients/client-libraries-scope.md
- Stage: post-S8 (the data plane surface these clients target is shipped)
- Status: in-progress

## Goal

Land the four thin client libraries (TypeScript/Node, Python, Go, Rust) under
`clients/<lang>/` per the scope: authenticate → connect → round-trip a `Sample`,
plus the webhook third-party caller path and the universal `POST /mcp/call`
bridge. Each library stays small enough that a reader can expand it without first
untangling a framework.

## What changed

- `docs/scope/clients/client-libraries-scope.md` — the ask (this session's
  source of truth).
- `docs/public/clients/clients.md` — TODO stub (filled on ship).
- `clients/` — one folder per language, each self-contained (own manifest,
  README, example; deliberately outside the core workspaces):
  - `clients/node-ts/` — `src/{client,ingest,mcp,webhook,index}.ts`, `example.ts`,
    `package.json`, `tsconfig.json`, `README.md`.
  - `clients/python/` — `lb_client/{client,ingest,mcp,webhook,__init__}.py`,
    `example.py`, `pyproject.toml`, `README.md`.
  - `clients/go/` — `client.go`, `ingest.go`, `mcp.go`, `webhook.go`, `example.go`,
    `go.mod`, `README.md`.
  - `clients/rust/` — `src/{client,ingest,mcp,webhook,error}.rs`,
    `examples/roundtrip.rs`, `Cargo.toml`, `README.md`.
  - `clients/README.md` — the index that ties them together and points at the
    scope + skill.

## Decisions & alternatives

- **Same five-method shape across all four languages.** Consistency is the
  contract; idioms differ (Promise vs `async def` vs channels vs `Future`).
  *Rejected: per-language "native-feeling" renames* — a reader switching
  languages should recognize the surface instantly.
- **Bearer is opaque to the client.** The client never branches on "API key vs
  JWT"; the gateway already doesn't (`session/authenticate.rs` splits on the
  `lbk_` prefix in one place). *Rejected: a `Client.withApiKey()` constructor
  that validates the prefix* — duplicates the gateway's grammar and breaks the
  day we extend it.
- **`writeSamples` + `latestSample` only**, not the full read set. They prove
  the loop; `series.list`/`read`/`find` stay one `callMcp` away. *Rejected:
  wrapping every read verb* — coverage-for-its-own-sake locks the shape before a
  real caller tells us which verbs it actually reaches.
- **Webhook helper takes bytes, never a string.** The HMAC-over-re-serialized-
  body bug is the single most common webhook integration failure (pinned in
  `webhook_routes_test.rs::signature_mode_body_tamper_breaks_signature`). The
  type system is the cheapest correct guard.
- **`clients/` is outside both workspaces.** Not in `pnpm-workspace.yaml`, not
  in `rust/Cargo.toml`. *Rejected: adding `clients/node-ts` to the pnpm
  workspace* — would force a transitive install on every contributor for a
  library the core does not consume; the README says how to install.
- **`clients/rust` is the one language that pulls a core crate.** Its tests
  reuse the in-process `test_gateway` pattern for a real round-trip without
  needing `make cloud` running. The other three keep a static check + documented
  live-recipe (rule 9 still obeyed — their live tests live where the language's
  toolchain expects).

## Tests

- **Static check (all four):** `cargo build -p lb-client` / `tsc --noEmit` /
  `go build ./...` / `python -c "import lb_client"`. Output pasted below on
  completion.
- **Capability-deny (mandatory):** `clients/rust` includes a test that mints a
  key lacking `mcp:ingest.write:call` and asserts the client surfaces the `403`.
- **Workspace-isolation (mandatory):** `clients/rust` mints a ws-A key and
  asserts a cross-ws read is denied opaquely.
- **Webhook raw-body:** `clients/rust` signs the compact body and posts the
  pretty-printed body; it must NOT verify.

## Debugging

None opened this session (fill in if any break).

## Public / scope updates

- Promote `docs/public/clients/clients.md` from TODO to the real per-language
  install/usage snippets on ship.
- `docs/scope/README.md` adds the `clients/` topic (done).

## Skill docs

n/a for v1: the clients do not expose a *new* drivable surface — they are
callers of the already-skill-documented gateway routes (`skills/ingest-series/`
is the canonical wire reference the READMEs link to). When the libraries
publish and a published-package "how to" becomes valuable, a `skills/clients/`
SKILL.md lands then.

## Dead ends / surprises

(filled as the work surfaces any)

## Follow-ups

- Live round-trip tests for `clients/go/` and `clients/python/` (deferred from
  v1 per the scope's testing plan — both ship a runnable `example.<ext>` that
  hits `make cloud`).
- Registry publication decision (npm/PyPI/crates.io/pkg.go.dev) — open question
  in the scope.
- A `make client-roundtrip` target once all four round-trip tests exist.
- A streaming/SSE helper (`EventSource` over `/series/{s}/stream`) — open
  question in the scope.
- `STATUS.md` slice entry (TODO).
