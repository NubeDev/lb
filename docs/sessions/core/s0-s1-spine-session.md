# Core — S0 skeleton + S1 spine (session)

- Date: 2026-06-26
- Scope: ../../scope/core/core-scope.md (+ auth-caps, mcp, crate-layout, extensions, jobs, bus)
- Stage: S0 (decisions + skeleton) → S1 (the spine), per STAGES.md
- Status: done

## Goal

Two parts in one session: (1) resolve the S0 *forever* decisions and stand up the Cargo
workspace so `cargo build` is green + CI runs; (2) build the thinnest S1 vertical slice —
host + embedded SurrealDB + embedded Zenoh + auth + caps + mcp + one **real WASM** extension
exposed as an MCP tool — and pass the mandatory capability-deny and workspace-isolation tests.

**Exit gates targeted.** S0: `cargo build` green on the workspace; CI runs the FILE-LAYOUT
size check + tests; the manifest + capability grammar written as scope docs. S1: a tool call
routed through MCP succeeds *with* the grant and is refused *without* it; a second workspace
cannot see the first's data.

## What changed

### S0 — forever decisions (written into scope docs)

- **SDK/WIT boundary** (`scope/crate-layout`): the stable plugin ABI is the WASI 0.2
  Component-Model world in `rust/sdk/wit/world.wit` (`lazybones:ext/extension@0.1.0`),
  semver-versioned; loader refuses a mismatched world major. One WIT, host + guest generated
  from it.
- **Capability grammar + token shape** (`scope/auth-caps`): cap = `<surface>:<resource>:
  <action>` with `*`/`**` wildcards; token = Ed25519 JWT with a single `ws` claim; the check
  is **two gates, workspace-first then capability**. Segment delimiter clarified as `/` *or*
  `.` (mcp names `<ext>.<tool>`).
- **Job queue** (`scope/jobs`): build the thin **native SurrealDB queue**, not an
  apalis-surrealdb backend (rationale recorded). Implementation deferred to S5.
- **Extension manifest** (`scope/extensions`): **TOML** `extension.toml` declaring id,
  version, tier, world, placement, *requested* caps, declared tools, visibility. The host
  grants `requested ∩ admin_approved` — "public" never means "privileged".

### S0 — workspace

- `rust/Cargo.toml` virtual workspace with the §9 crate split: `crates/{host,store,bus,
  runtime,mcp,auth,caps,tags,inbox,jobs,secrets,sync,ext-loader}`, `sdk/`, `node/`,
  `extensions/hello/` (wasm guest, excluded from the workspace), and `role/{gateway,
  ai-gateway,registry-host,bootstrap-ui}` placeholders (the role boundary made physical).
- CI: `.github/workflows/ci.yml` (FILE-LAYOUT check + build wasm guest + build/test + fmt) and
  `rust/scripts/check-file-size.sh`.

### S1 — the spine (real implementations)

- **auth**: Ed25519 JWT mint/verify with an injected clock; `Principal` (no public raw ctor).
- **caps**: the grammar matcher (`grammar.rs`) + the two-gate `check` (`check.rs`) — the one
  authorization chokepoint.
- **store**: embedded SurrealDB (`mem://`), workspace = namespace; `read`/`write` over
  `serde_json::Value` (wrapped under a `data` field — see Debugging).
- **bus**: embedded Zenoh peer + `ws_key` workspace prefixing (pub/sub deferred to S2).
- **runtime**: wasmtime component host generated from the WIT (`bindgen!`); loads a component,
  calls its `tool.call` export.
- **ext-loader**: `Manifest::parse` (+ world-major check) and `grant` (requested ∩ approved).
- **mcp**: the `call → authorize → resolve → dispatch` pipeline. `authorize` runs the shared
  `caps::check`; it runs *before* resolve so a denial never leaks tool existence.
- **host**: `Node::boot` (store+bus+engine+registry) and `load_extension`.
- **extensions/hello**: a real `wasm32-wasip2` component (`hello.echo`), the spine probe.
- **node**: boots solo, loads hello, calls `hello.echo` once — runs live (output below).

## Decisions & alternatives

- **Real WASM component now, not a host stub** (user: "do whatever is best long term"). S1
  proves the capability model through the *actual* forever ABI — the whole point of the stage
  — rather than deferring the WIT wiring. Cost: a `wasm32-wasip2` build step in CI; accepted.
- **Ed25519 via `ed25519-dalek` directly, dropped `jsonwebtoken`** — see Debugging
  (ring↔dalek PKCS#8 mismatch). One crypto lib owns the token; no fragile seam.
- **`authorize` before `resolve`** in the MCP pipeline — guarantees a denied caller cannot
  distinguish "not allowed" from "no such tool" (tested).
- **`role/` as a sibling directory**, not inside `crates/` — makes "no core crate is
  role-aware" physically visible (no `if cloud`).
- **Store wraps host JSON under a `data` field** — see Debugging (SurrealDB `.content()`
  rejects raw `serde_json::Value`). Keeps the store API in `serde_json::Value`.

## Tests

Mandatory categories that apply at S1: **capability-deny** and **workspace-isolation** (both
present from day one). Offline/sync and hot-reload: **n/a at S1** (solo node, no live swap;
arrive S2/S3). Determinism held: auth uses an injected clock, no RNG in test logic.

- `caps`: `match_test` (7, the grammar table), `match_prop_test` (3, wildcard segment-boundary
  invariant), `deny_test` (4, **mandatory deny**), `isolation_test` (4, **mandatory
  isolation**, all surfaces).
- `auth`: `token_test` (4, mint→verify round-trip, expiry, wrong-key, tamper).
- `store`: `isolation_test` (2, **mandatory isolation** with real embedded SurrealDB).
- `ext-loader`: grant intersection incl. requested-but-unapproved-is-absent (2).
- `sdk`: world-major match (3). `bus`: `ws_key` (2).
- **`host/spine_test` (4) — the S1 EXIT GATE, end to end through the real wasm:**
  `echo_succeeds_with_the_grant`, `echo_is_refused_without_the_grant`,
  `denied_call_does_not_reveal_tool_existence`, `second_workspace_cannot_call_into_the_first`.

### Green output

```
$ cargo run -p node
loaded hello: tools=["echo"] granted_caps=[]
hello.echo -> {"echo":"hi"}

$ cargo test --workspace            # 35 tests, 0 failed
  token_test ......... 4 passed
  bus (unit) ......... 2 passed
  caps/deny_test ..... 4 passed     # MANDATORY capability-deny
  caps/isolation ..... 4 passed     # MANDATORY workspace-isolation (all surfaces)
  caps/match_prop .... 3 passed
  caps/match_test .... 7 passed
  ext_loader (unit) .. 2 passed
  host/spine_test .... 4 passed     # S1 EXIT GATE (real wasm: with/without grant; ws isolation)
  sdk (unit) ......... 3 passed
  store/isolation .... 2 passed     # MANDATORY workspace-isolation (real SurrealDB)
  TOTAL PASSED: 35

$ cargo fmt --all --check
FMT OK

$ FILE-LAYOUT size check: checked 55 .rs files; over-limit: 0   (largest 107 lines)
```

## Debugging

Three non-trivial breakages, each with a debug entry + regression test (cross-linked):

- [auth/valid-token-fails-verification](../../debugging/auth/valid-token-fails-verification.md)
  — jsonwebtoken/ring needed PKCS#8 v2; dalek emitted v1. Fixed by signing JWTs with dalek
  directly. Regression: `auth/tests/token_test.rs`.
- [bus/zenoh-needs-multi-thread-runtime](../../debugging/bus/zenoh-needs-multi-thread-runtime.md)
  — Zenoh panics under the current-thread Tokio runtime. Fixed by `#[tokio::test(flavor =
  "multi_thread", worker_threads = 1)]`. Recorded as a standing constraint in the bus scope.
- [store/content-rejects-serde-json-value](../../debugging/store/content-rejects-serde-json-value.md)
  — SurrealDB `.content()` rejects raw `serde_json::Value`. Fixed by binding `$data` and
  wrapping under a typed `data` field. Regression: `store/tests/isolation_test.rs`.

## Public / scope updates

- Promoted to `public/`: `core`, `auth-caps`, `mcp`, `crate-layout` (what shipped in S1) and
  `public/SCOPE.md` updated.
- Scope open questions refreshed in: core, auth-caps, mcp, crate-layout, extensions, jobs,
  bus (each now distinguishes resolved-in-S1 vs deferred-with-stage).

## Dead ends / surprises

- `serde_json::to_value(&surrealdb::Value)` yields the *internally tagged* form
  (`{"Strand":..}`); the public `into_json` is `pub(crate)`. The supportable bridge was a
  concrete `data`-field struct, not a Value↔Value conversion.
- wasmtime 39's `bindgen!` async is per-direction (`exports: { default: async }`), and host
  imports stay sync; the linker getter uses `HasSelf<_>` under the new `HasData` model.

## Follow-ups

- bus pub/sub + presence (S2 messaging); routing seam in `mcp/dispatch` becomes real at S3.
- jobs/secrets/sync/tags/inbox are stubs — fill at their stages.
- A `#[lb_test]` macro that bakes in the multi-thread flavor, if the Zenoh constraint bites.
- STATUS.md updated? **Yes** — S0 exit gate met; S1 slice marked `tested`/`shipped`.
