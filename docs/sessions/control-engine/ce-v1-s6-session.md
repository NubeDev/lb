# control-engine v1 — S6 session (`control-engine.watch` — live COV feed)

Branch: `ce-v1` (worktree). Depends on: S2 + S4 (routing/registry). Spec:
`rust/extensions/control-engine/docs/slice-6-ce-watch-cov.md`.

## What shipped

`control-engine.watch { appliance, scope? }` — a workspace-scoped **live change-of-value feed**. It
resolves the appliance (S4 `resolve.rs`, workspace-walled), derives a deterministic series name, arms a
CE COV subscription, and pumps each decoded `CovEvent` — re-encoded to a plumbing-agnostic JSON frame —
onto the series' **motion** subject via the host `ingest.write` callback. Returns `{ series, subject }`.

New/changed:
- `src/watch/frame.rs` — the frame contract re-encode + the >2^53 rule (unit-tested).
- `src/watch/series.rs` — series-name scheme + `scope` → `CovScope` parse (unit-tested).
- `src/watch/pump.rs` — the `subscribe_cov` → encode → `ingest.write` pump + bounded-backoff reconnect.
- `src/watch/mod.rs` — `WatchRegistry` (arm-on-first / disarm-on-last, per-series refcount, force-disarm)
  + lifecycle unit tests.
- `src/watch/verb.rs` — the gate → resolve → arm verb.
- `src/serve.rs` — self-contained `control-engine.watch` dispatch arm + a `WatchRegistry` at serve start;
  threaded into the appliance family for the remove hook. (Kept localized — S5 edits serve.rs in parallel.)
- `src/tools/appliance/remove.rs` — `appliance.remove` now `disarm_appliance(id)` before deleting.
- `src/ce_fake.rs` — wired `subscribe_cov` to emit a seeded frame + instrumentation (`active_cov`,
  `cov_subscribes`, `inject`, `drop_ws`).
- `extension.toml` — `control-engine.watch` `[[tools]]` + the ONE new cap `mcp:ingest.write:call`.
- `tests/watch_cov_test.rs` — the end-to-end exit gate + deny + isolation (feature-gated on `ce-fake`).
- **Core (CE-ignorant):** `crates/host/src/tool_call.rs` — the MCP `ingest.write` path now publishes
  motion after the durable write (see "Core fix" below).

## Decisions

### Frame contract (the real design work)
One JSON object per event as the ingest `Sample.payload`:
- `{ "kind":"cov", "ts":<ms>, "values":[{"uid","v"}...], "status":[{"uid","s"}...] }` from
  `CovEvent::Values`. `status` is **omitted** on a clean tick (only nonzero flags appear).
- `{ "kind":"topology", "ts":0, "msg":{ op, seq, componentUids, edgeUids? } }` from
  `CovEvent::Topology` (Added/Removed/Changed passthrough; the wiresheet resyncs via `control-engine.tree`).
- **>2^53 rule (decided once, in `frame.rs`):** an `i64` whose magnitude exceeds `2^53-1` serializes as a
  JSON **string** (the wiresheet's `DecodedValue` handles the bigint); everything in the safe range stays
  a JSON number. `FlexValue` is `#[serde(untagged)]`, so float/bool/str/null pass through naturally — only
  the `Int` arm needs the guard.
- **`schema` kind:** the pinned `rubix-ce` rev's `CovEvent` has only `Values` + `Topology` — there is no
  schema WS message surfaced. So we emit `cov` + `topology` only. **Named follow-up:** when the client
  surfaces a schema message, add a `schema` arm in `frame.rs` as a passthrough (`{kind:"schema", msg}`).

### Fallback plumbing vs the generic primitive
The generic extension-watch primitive is NOT in core (verified: no watch kind / subject-alloc in
`crates/{host,mcp,runtime}/src`), so per slice-6 §"Sequencing fallback" I shipped the **zero-core-change
bridge**: sidecar re-encodes each COV event and writes it via `ingest.write` onto a series; the shipped
`series` motion + gateway `GET /series/{series}/stream` SSE is the live read. Behind the same
`control-engine.watch` tool name + frame JSON so S7 is plumbing-agnostic.
- **NAMED MIGRATION FOLLOW-UP:** when the generic extension-watch primitive lands, swap the pump's
  `ingest.write` sink for the primitive's host-allocated subject. `control-engine.watch`'s tool name,
  args, `{series, subject}` return, and the frame contract stay identical — only `pump.rs`'s write seam
  changes. S7 must not need a change.

### Series-name scheme
`ce-cov:{appliance}:{fnv1a_hash(appliance, sorted-components, sorted-properties, tick_hz):016x}`.
- The `ce-cov:` prefix namespaces the feed; the appliance selector prevents cross-appliance collision;
  the hash of the canonical (sorted/deduped) scope coalesces callers with the SAME `(appliance, scope)`
  onto ONE series (the arm-on-first / disarm-on-last key). FNV-1a → deterministic across processes (no
  `RandomState`), so two nodes/callers derive the identical name.
- **Workspace-safe by construction:** the host walls every series under `ws/{id}/series/{name}`
  (`ingest/motion.rs::series_key`), so two workspaces watching the same appliance never share a subject.

### Lifecycle
`WatchRegistry` (in-memory pump-handle map — a connection pool, NOT durable state, §3.4). First subscriber
arms (opens the CE WS lazily, spawns the pump); further subscribers for the same series refcount up; last
release removes the entry, whose `Drop` aborts the pump → the `CovStream`/WS drops. `appliance.remove`
force-disarms every watch for that appliance. Reconnect on CE WS drop: `pump.rs` re-subscribes with
bounded backoff (200ms→5s) — the subscriber sees a gap, not a dead stream.

### What S7 consumes
Call `control-engine.watch { appliance, scope? }` → `{ series, subject }`. Open the gateway
`GET /series/{series}/stream?token=<jwt>` SSE for `series`; each `event: sample` is a `Sample` whose
`payload` is the frame JSON above. (`subject` = `ws/{ws}/series/{series}` is the bus subject the SSE
relays — for a caller that subscribes the bus directly.)

## Core fix (CE-ignorant, generic ingest)

The MCP `ingest.write` verb (`ingest/tool.rs::call_ingest_tool`) staged+drained to the durable `series`
table but did **NOT** publish live motion — only the gateway's `POST /ingest` HTTP route did. So a sample
written over the MCP callback (the sidecar's path) never surfaced on the `GET /series/{s}/stream` SSE.
Fixed in `crates/host/src/tool_call.rs`: after a successful `ingest.write` dispatch (where `node.bus` is
in scope), publish each sample's motion via `publish_sample`, mirroring `routes/ingest.rs` exactly
(producer stamped to the authenticated principal, best-effort). This is generic and domain-free — no CE
knowledge in core; the sanity-grep stays clean. Any MCP `ingest.write` caller (not just CE) now gets the
same write-then-motion semantics as the HTTP route, closing a latent inconsistency.

## Opt-in historian (scoped, NOT built — named follow-up)

A per-appliance `history: [prop-uid…]` on the `ce_appliance` record, mirrored to the series plane via
`ingest.write` — the DURABLE opt-in copy, distinct from the live (fire-and-forget) motion. It requests
only `mcp:ingest.write:call` (already present from the live path). Never all-COV by default. Build after
S7 if needed; kept here so it is not re-scoped.

## Testing (all real infra; green)

- **Unit** (`--features ce-fake`): `frame.rs` (5) — uid+value, clean-tick omits status, >2^53→string,
  null/string passthrough, topology; `series.rs` (4) — deterministic/order-invariant, distinct
  appliance/scope, empty scope, scope mapping; `watch/mod.rs` lifecycle (3) — arm-on-first/disarm-on-last
  (asserts `ce_fake.active_cov` returns to 0), appliance.remove force-disarm, CE-WS-drop-reconnect
  (asserts `cov_subscribes >= 2`).
- **Integration** `tests/watch_cov_test.rs` (real node + real axum gateway + real store + real bus, the ONE
  `ce_fake`): **exit gate** — arming publishes the seeded COV frame onto `ws/{ws}/series/{series}` (the
  motion the SSE relays), re-encoded to the frame contract; **deny** — no `mcp:control-engine.watch:call`
  → opaque `Denied` before any arm; **isolation** — a ws-B caller resolving a ws-A appliance → not-found.
- **SSE assertion choice:** asserted on the workspace **bus subject** `ws/{id}/series/{series}` (the
  motion the gateway `GET /series/{series}/stream` relays verbatim — `ingest/motion.rs`). Sanctioned by
  the task; proving the frame lands there proves S7's SSE receives it.
- **Routed two-node:** the S4 `control_engine_appliance_routing_test.rs` already proves the routed
  native-call hop (watch reuses the exact dispatch path); it stays green with the S6 manifest additions.
- **Real-engine tier (opt-in, NOT run here):** patch a prop via S5 `control-engine.patch` and observe the
  COV frame on the SSE. S5 is not in this worktree and `ce-studio` (`~/code/ce/ce-studio/run.sh
  --engine-only`, ce-rest on :7979) was not run in this environment — left as an opt-in manual step.

Commands: `cargo build --workspace` (green); `cargo test -p control-engine --features ce-fake --
--test-threads=4` → 18 lib + 6 appliance + 3 watch, all green; `cargo test -p lb-host --test
control_engine_appliance_routing_test` green; workspace suite green (two pre-existing failures are
missing WASM artifacts — `hello`/`hello-v2` `.wasm` not built in a fresh worktree — resolved by building
those crates for `wasm32-wasip2`; NOT S6 regressions). Sanity-grep over
`crates/{host,mcp,caps,runtime,bus}/src` + `role/gateway/src` is EMPTY.

## Nothing broke that needed a debug entry

No pre-existing behavior regressed. The core `ingest.write` motion addition is additive (motion is
best-effort; no existing test asserted its ABSENCE). No `docs/debugging` entry required.
