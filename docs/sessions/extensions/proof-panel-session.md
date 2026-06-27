# Session — `proof-panel`, the Tier-1 WASM reference extension

**Date:** 2026-06-27
**Area:** extensions / ui-federation
**Goal:** prove the basics end to end with a NEW self-contained extension on the **WASM (Tier-1)** tier —
a real MCP tool reachable through the capability gate AND a federated UI page reaching real platform data
through the host bridge, both in one folder. The counterpart to the existing native (Tier-2)
`fleet-monitor`, on the lighter tier that had no full-stack reference yet.

## Why (the gap)

The audit at the start of the session found the platform's basics (MCP, users/teams/members/roles,
inbox/outbox/ingest, the extension host) **already real and free of mock/fake data** — far past the
"empty scaffolding" CLAUDE.md still claimed (that claim is now corrected). The one honest gap on the
extension side: `fleet-monitor` proves the **native** path, and its two dashboard *widgets* are explicit
placeholders. There was **no full-stack Tier-1 (WASM) reference** that proves, in one self-contained
folder, that a WASM extension can ship a real backend tool *and* a federated frontend. `proof-panel` is
that reference — the honest "the basics work" proof, built on the real publish → install → load → call
path with no new platform code.

## What shipped

A new extension at `rust/extensions/proof-panel/` (excluded from the host workspace like the other wasm
guests; added to `rust/Cargo.toml` `workspace.exclude`):

- **Backend** — `src/lib.rs`: a `wasm32-wasip2` component serving ONE tool, `proof.status`, through the
  host's WASM component runtime (no sidecar/PID). Stateless: the reply is a pure function of the input
  plus a fixed `tier: "wasm"` tag, so a hot-reload swap loses nothing (§3.4). `extension.toml` declares
  `tier = "wasm"`, the one tool, the `[ui]` page, and requests read-only `series.find`/`series.latest`.
- **Frontend** — `ui/`: a module-federation remote exposing `./mount` (shared React singletons), built
  on the **frozen** `MountCtx`/`Bridge` contract (mirrored byte-for-byte from the shell). A single
  `Panel` page reads the workspace's series via `bridge.call("series.find")` with honest
  loading/empty/error states and a workspace badge proving the host `ctx` (tenant wall) reached the
  remote. `pnpm build` emits `dist/assets/remoteEntry.js` — the exact path the manifest `[ui] entry`
  and the shell loader expect.
- **`build.sh`** — the one-command path: builds the wasm component + the federated UI bundle.

No platform/host/runtime/WIT change — `proof-panel` rides the existing `ext_publish` → install → load →
`mcp::call` path exactly as `hello`/`hello-v2` do.

## How it was tested (all green, no fakes)

- **Backend (host integration test, real wasm + runtime, §9)** —
  `rust/crates/host/tests/proof_panel_test.rs`, 3 tests against a real `Node::boot()` loading the real
  `proof_panel_ext.wasm`:
  - `proof_panel_publishes_and_its_tool_is_callable` — a signed artifact publishes-installs-loads and
    `proof-panel.proof.status` is callable RIGHT NOW; the note round-trips and `tier == "wasm"` proves
    the Tier-1 component served the call.
  - `proof_status_is_denied_without_the_grant` — **mandatory capability-deny**: refused without
    `mcp:proof-panel.proof.status:call`.
  - `another_workspace_without_the_grant_is_denied` — **mandatory workspace-isolation**, stated honestly
    (see the finding below): a second workspace lacking the grant is denied.
- **Frontend (Vitest, bridge test-double = the allowed seam, testing-scope §0)** — `ui/` 4 tests:
  `mount.test.tsx` (the federation handshake renders into `el` with the host ctx and unmounts clean) and
  `pages/Panel.test.tsx` (renders the rows the bridge returns · honest empty state · a denied/out-of-
  scope call surfaces as an error, never a fabricated list).
- `cargo build --workspace`, `cargo fmt -p lb-host`, `pnpm build` (federation bundle) all green.

Run: `bash rust/extensions/proof-panel/build.sh` then
`cargo test -p lb-host --test proof_panel_test` and `cd rust/extensions/proof-panel/ui && pnpm test`.

## Finding worth keeping (a test that was wrong, then made honest)

The first cut of the isolation test asserted that a second workspace **holding the cap** could not reach
ws-A's installed extension. It **failed** — the call succeeded. That is correct behaviour, not a bug: a
stateless WASM tool is loaded into the node's **process-global** registry, and
`mcp::call::authorize` checks `workspace + capability` against the **caller's own token**, never that the
*extension* belongs to the caller's workspace. Workspace isolation for a data-less tool therefore bites
where the tool **touches data** (its workspace-scoped store/series reads), not at tool reachability. The
honest, meaningful per-workspace wall to assert for such a tool is the **capability gate**: a second
workspace **without** the grant is denied even though the component is loaded process-wide. The test was
rewritten to assert exactly that, and the rationale is captured in its doc comment so the next reader
doesn't re-introduce the wrong assumption. (This matches `ext_publish_test`, which likewise never asserts
cross-workspace tool denial.)

## Follow-ups (not done here)

- Wire `fleet-monitor`'s two placeholder widgets to real `series.latest`/`series.read` — the remaining
  honest gap on the native reference. Tracked as the smaller sibling of this work.
- The dashboard scope (`dashboard.*` 5 verbs + S4 three-gate sharing + grid UI) remains build-ready but
  unbuilt; `proof-panel`'s federated page is a clean precedent for a dashboard widget remote.
