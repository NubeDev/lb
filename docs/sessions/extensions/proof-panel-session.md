# Session — `proof-panel`, the Tier-1 WASM reference extension (both halves, no placeholders)

**Date:** 2026-06-27
**Area:** extensions / ui-federation
**Status:** shipped — code + real tests + docs; all green.
**Scope:** [../../scope/extensions/proof-panel-scope.md](../../scope/extensions/proof-panel-scope.md)
**Public:** [../../public/extensions/extensions.md](../../public/extensions/extensions.md) (`proof-panel` subsection)
**Debugging:** [../../debugging/extensions/bridge-cannot-dispatch-host-native-series.md](../../debugging/extensions/bridge-cannot-dispatch-host-native-series.md),
[../../debugging/extensions/series-find-needs-tag-edges-not-labels.md](../../debugging/extensions/series-find-needs-tag-edges-not-labels.md)

## Goal

Ship the proof-panel scope to fully built: a NEW self-contained **Tier-1 WASM** extension at
`rust/extensions/proof-panel/` mirroring `fleet-monitor`'s one-folder shape but on the in-process wasm
tier, with **no placeholders** — both halves real:
- a wasm guest serving ONE MCP tool `proof.ping` (stateless; `{"ok":true,"ws":…,"node":"proof-panel","tier":"wasm"}`);
- a co-located federated `ui/` page that lists series via `series.find` and shows a selected series' latest
  value via `series.latest`, reaching data ONLY through the host-mediated bridge.

The exit gate: `cargo test` (wasm tool + host install/grant-intersection/deny/isolation) and
`pnpm test:gateway` (the page's data path over a real gateway) green, the extension's own `build.sh`
emitting both the wasm component and the federated `remoteEntry.js`.

## What shipped

**Backend** — `rust/extensions/proof-panel/src/lib.rs`: a `wasm32-wasip2` component (modelled on
`hello`) serving `proof.ping` through the existing `tool.call` WIT world. Stateless — the reply is a
pure function of the input + a fixed `tier:"wasm"` tag. `extension.toml`: `tier="wasm"`, the same WIT
world major as hello/fleet-monitor, `[[tools]] proof.ping`, `[capabilities] request =
[series.find, series.latest, series.read]`, and a `[ui]` block (entry `assets/remoteEntry.js`, label
"Proof Panel", icon `shield-check`, scope `[series.find, series.latest]`). NO `[[widget]]` (deferred).
The crate is **excluded** from the cargo workspace like every other wasm guest (`hello`, `hello-v2`,
`github-bridge`) — see the assumption below. `build.sh` builds the wasm component + the federated bundle.

**Frontend** — `rust/extensions/proof-panel/ui/`: a module-federation remote (vite config, build.sh,
`mount(el, ctx, bridge)`, frozen `Bridge`/`MountCtx` contract, design-token CSS all mirrored from
`fleet-monitor`). One page (`pages/Panel.tsx`): a tag-facet search box → lists the workspace's series
via `series.find` → select one → shows its latest value via `series.latest`. Honest idle / loading /
empty / error states throughout; the workspace badge proves the host `ctx` reached the remote. Data
hooks `useSeriesFind`/`useSeriesLatest` send the REAL host arg shape (`{facets}` / `{series}`) and
unwrap the REAL result shape (`{series}` / `{sample}`).

**The load-bearing host fix** — `rust/crates/host/src/tool_call.rs`: `call_tool` (the host's bridge
entry, the SAME function `POST /mcp/call` forwards through) now dispatches host-native `series.*` /
`ingest.*` verbs (authorize with the same MCP gate, then delegate to `call_ingest_tool`) instead of
only resolving the runtime registry. Extension `<ext>.<tool>` calls route through the registry
unchanged. **No new verb, no WIT change** — see the findings below.

**Test infra** — `rust/role/gateway/src/bin/test_gateway_seed.rs` gains a `/_seed/series` route (real
`ingest_write`+`drain_workspace`+`tags_add`); `mcp:tags.add:call` added to the dev member claims (a
member may tag their own series). `ui/src/test/gateway-session.ts` gains `seedSeries`.

## How it was tested (all green, no fakes)

**1. Wasm unit tests** — `rust/extensions/proof-panel/src/lib.rs` (4): ok / empty-input-defaults /
unknown-tool-is-error / bad-params-is-error. The pure dispatch the WIT export drives.

```
running 4 tests
test tests::bad_params_is_an_error_not_a_panic ... ok
test tests::ping_returns_a_workspace_tagged_wasm_snapshot ... ok
test tests::unknown_tool_is_an_explicit_error ... ok
test tests::ping_with_empty_input_defaults_the_workspace ... ok
test result: ok. 4 passed; 0 failed
```

**2. Host integration tests** — `rust/crates/host/tests/proof_panel_test.rs` (4), real `Node::boot()` +
real `proof_panel_ext.wasm` + real store, all routed through `call_tool` (the bridge entry):

```
running 4 tests
test proof_ping_is_denied_without_the_grant ... ok
test proof_ping_is_callable_after_publish ... ok
test grant_intersection_denies_the_unapproved_verb_at_the_bridge ... ok
test workspace_isolation_series_and_ping ... ok
test result: ok. 4 passed; 0 failed
```

- `proof_ping_is_callable_after_publish` — signed artifact publishes→installs→loads; `proof.ping` is
  callable now, `tier=="wasm"`, ws round-trips (proves the Tier-1 component served it).
- `proof_ping_is_denied_without_the_grant` — **mandatory cap-deny**: opaque `Denied` without the grant.
- `grant_intersection_denies_the_unapproved_verb_at_the_bridge` — install approving only `series.find`;
  the persisted page scope drops `series.latest` AND a bridge `series.latest` call by the granted
  principal is denied at CALL time (403), while `series.find` lists the seeded series. The narrowing is
  **enforced, not displayed**.
- `workspace_isolation_series_and_ping` — **mandatory isolation**: ws-B's `series.find` (granted) sees
  NONE of ws-A's seeded series; ws-B's `proof.ping` without the grant is denied.

**3. Real-gateway UI test** — `ui/src/features/ext-host/ProofPanel.gateway.test.tsx` (4), against the
real spawned `test_gateway`, driving the real `makeBridge(scope)` seam (the exact bridge the shell hands
`mount`): empty-state → seed a real series via `/_seed/series` → `series.find` lists it → `series.latest`
shows its value (61.4) → workspace-isolation → an ungranted verb denied. Part of `pnpm test:gateway`:

```
✓ src/features/ext-host/ProofPanel.gateway.test.tsx (4 tests)
Test Files  20 passed (20)
     Tests  57 passed (57)   ← the whole gateway suite, unbroken by the dev-claims + tool_call change
```

**4. In-memory page tests** — `rust/extensions/proof-panel/ui/src/pages/Panel.test.tsx` (5) +
`mount.test.tsx` (1): idle→search→list, select→latest, empty state, denied-find error, denied-latest
(grant-intersection) error — against the bridge test-double (the allowed seam, testing §0).

```
✓ src/mount.test.tsx (1 test)
✓ src/pages/Panel.test.tsx (5 tests)
Test Files  2 passed (2)   Tests  6 passed (6)
```

**5. Whole-workspace + fmt + build.sh.** `cargo build --workspace` ok; `cargo test --workspace` →
**359 passed, 0 failed**; `cargo fmt --check` clean; `bash rust/extensions/proof-panel/build.sh` emits
`target/wasm32-wasip2/release/proof_panel_ext.wasm` + `ui/dist/assets/remoteEntry.js` (exit 0).

## Findings & decisions (recorded, not re-asked)

- **The bridge could not dispatch host-native `series.*` at all (fixed).** `POST /mcp/call` →
  `call_tool` → `lb_mcp::call` resolved only the runtime registry; `series.*` are host verbs, not
  registry entries, so they `NotFound`-ed. A federated page reading series through the bridge was
  therefore impossible — yet the bridge contract is *defined* in terms of `series.find`/`series.latest`.
  Fixed in `tool_call.rs` (no new verb / no WIT change — exactly the scope's premise that the basics are
  already sufficient). Full write-up + why no prior test caught it:
  `debugging/extensions/bridge-cannot-dispatch-host-native-series.md`.
- **`series.find` discovery needs tag edges the ingest path doesn't create from `labels`.** `Sample.labels`
  is documented as "converted to tag edges at commit" but no code does that conversion, so a series
  seeded purely via `ingest_write` is not discoverable by `series.find`. The tests seed the discovery
  edge explicitly through the real tag path (`lb_tags::add` / `lb_host::tags_add`) — the edge a producer's
  labels *should* eventually produce. The page lists by a `key:value` search box (mirroring IngestView's
  faceted search); an unconstrained `series.find` returns nothing by design, so the page shows an honest
  "search to list" prompt rather than a misleading empty list. Root fix (implement label→tag at commit)
  is a tracked follow-up: `debugging/extensions/series-find-needs-tag-edges-not-labels.md`.
- **`proof.ping` cap lives on the CALLER (hello convention).** Verified against `ext_publish_test.rs`:
  the caller is granted `mcp:<ext>.<tool>:call` before calling `hello.echo`; the manifest requests no
  host-side cap for its own tool. `proof-panel` follows this — `extension.toml [capabilities] request`
  lists only the series read verbs the page needs; `mcp:proof-panel.proof.ping:call` is granted on the
  caller's token, asserted by the deny test. No bespoke host-side cap added.
- **Workspace `ws` field on `proof.ping` is echoed from input, not ambient.** The WIT `call(name,
  input-json)` ABI gives a wasm guest no injected identity (unlike a native sidecar's `LB_EXT_WS` env),
  so the caller supplies `ws` in the input and the guest echoes it. The real per-workspace wall is the
  host's capability gate (re-checked against the caller's token), not the echoed field — documented in
  `lib.rs`. This is the honest Tier-1 analogue of `fleet.summary`.
- **proof-panel stays excluded from the cargo workspace.** The scope prompt said "add as a real
  workspace member crate", but a `wasm32-wasip2` crate in the workspace forces the whole workspace onto
  the wasm target (the root `Cargo.toml` comment + the exclusion of `hello`/`hello-v2`/`github-bridge`
  say exactly this). fleet-monitor is a member only because it is **native** (host target). The
  fleet-consistent choice for a **wasm** extension is the wasm-sibling convention: excluded, built by its
  own `build.sh`, loaded by the host as real bytes in the host tests. Assumption stated per the prompt.
- **Open questions resolved:** live feed → request/response only this slice (no `series.watch`);
  `proof.ping` host-side cap → none (caller convention); keep `hello`/`hello-v2` as-is; no widget tiles.

## Load-and-test pass (2026-06-27, follow-up)

Re-ran the loadable-artifact path to confirm the extension still builds and loads:

- `build.sh` → emits a valid `proof_panel_ext.wasm` component (WebAssembly binary module
  `0x1000d`) + `ui/dist/assets/remoteEntry.js` (exit 0). The wasm target needs no `cc`.
- Proof-panel **UI in-memory tests green again** — `mount.test.tsx` (1) + `pages/Panel.test.tsx`
  (5) = **6 passed**.
  - **Fix:** `ui/package.json` declared `@testing-library/user-event` but it was missing from
    `node_modules` (an earlier `pnpm install || true` had swallowed the failure), so
    `Panel.test.tsx` failed to resolve the import and collected 0 tests. Ran `pnpm install` to
    sync; the dep installs and the lockfile now carries it. No source change needed.

**Sandbox limitation (not a code defect):** the native-target tests — `cargo test -p lb-host`
(incl. `proof_panel_test`), `cargo test --workspace`, and `pnpm test:gateway` (spawns the native
`test_gateway` bin) — cannot be re-run in this environment: no system C compiler (`cc`) and no
sudo to install one, so any test/binary *link* step fails (library builds succeed; only the final
link is blocked). These were run green in the build session above and are unchanged here.

## Replaces the earlier draft

An earlier session draft described a `proof.status` tool with no grant-intersection or real-gateway
test. This session supersedes it: the tool is `proof.ping` with the scope-mandated snapshot shape, and
the grant-intersection + real-gateway page-data tests are now real and green.

## Follow-ups (not done here)

- Implement label→tag conversion at commit (the root of the discovery finding) and revisit the
  IngestView faceted-search assertion.
- A `series.watch` (bus-backed SSE through the bridge) upgrade once that verb exists.
- Wire `fleet-monitor`'s two placeholder widgets to real series verbs (the remaining native-side gap).

---

# Session 2 — the "whole platform, one page" all-features demo (2026-06-27)

**Status:** shipped — code + real tests (backend + frontend + live e2e) + docs; all green.
**Debugging (new):** [../../debugging/store/surrealkv-invalid-revision-on-drain-reread.md](../../debugging/store/surrealkv-invalid-revision-on-drain-reread.md)

## Goal

The page proved only the READ half (`series.find`/`series.latest`). This slice makes it prove the
**full round-trip** from inside the one cap-gated federated page, through the host-mediated bridge:
1. **Ingest → read round-trip (headline):** a "Write sample" button → `ingest.write { samples }` →
   `series.latest` reads it back live (write → stage → drain → read, in the browser). The page CREATES
   the data it shows.
2. **Outbox status:** a card of `outbox.status {}` → `{pending,delivered,dead_lettered}` + Refresh.
3. **Inbox triage:** `inbox.list { channel }` items with Approve/Reject → `inbox.resolve { item_id,
   decision }` — the page's first WRITE that mutates durable workflow state.

## What shipped

**Backend (the load-bearing bit).** The bridge entry `lb_host::call_tool` (the gateway's `POST
/mcp/call`) could dispatch only `series.*`/`ingest.*`. Extended `is_host_native` + added a
`call_workflow_tool` dispatcher so `outbox.status` / `inbox.list` / `inbox.resolve` reach their host
verbs (`outbox_status`/`list_inbox`/`resolve_inbox`) — same MCP gate first (opaque `Denied`), workspace
from the token. `inbox.resolve` forces the actor to the principal's `sub` (un-spoofable). **No new core
verb, no WIT change.**

**Write-then-read visibility.** There is no background drain worker (the gateway's own `POST /ingest`
route drains synchronously for exactly this reason). So the bridge's `ingest.write` now **drains
staging → `series` after staging** (`call_ingest_tool`'s `ingest.write` arm), so a write is visible to
the very next `series.latest` over the same bridge — the round-trip the page proves. Drain is
exactly-once, so write-then-read never double-commits. Updated `ingest_test`'s round-trip to assert the
second explicit drain is now a no-op.

**Manifest.** `extension.toml` `[capabilities] request` AND `[ui] scope` gained the four verbs
(`ingest.write`, `outbox.status`, `inbox.list`, `inbox.resolve`). Verified the persisted page scope
carries all six over `GET /extensions` after publish.

**Frontend (FILE-LAYOUT: one hook per verb, thin Panel).**
- Hooks: `data/useIngestWrite.ts`, `useOutboxStatus.ts`, `useInboxList.ts`, `useInboxResolve.ts` +
  `data/workflow.types.ts` (the view types).
- Sections: `pages/IngestSection.tsx` (headline write→read), `OutboxSection.tsx`, `InboxSection.tsx`,
  and the READ half extracted to `pages/SeriesSection.tsx`. `Panel.tsx` is now a thin composition
  (header + four sections). Frozen `app/contract.ts` untouched. Dev bridge (`dev.tsx`) extended with
  honest empty defaults for the new verbs.

## Tests — all real infra, seeded via the real write path (CLAUDE §9), green

**Rust host — `crates/host/tests/proof_panel_test.rs` (9 passed, 5 new):**
```
running 9 tests
test ingest_write_then_latest_round_trips_through_the_bridge ... ok
test ingest_write_is_denied_without_the_grant ... ok
test outbox_status_reads_real_effects_and_denies_without_the_grant ... ok
test inbox_list_then_resolve_round_trips_and_denies_per_verb ... ok
test workflow_surface_is_workspace_isolated ... ok
test grant_intersection_denies_the_unapproved_verb_at_the_bridge ... ok
test proof_ping_is_callable_after_publish ... ok
test proof_ping_is_denied_without_the_grant ... ok
test workspace_isolation_series_and_ping ... ok
test result: ok. 9 passed; 0 failed
```
`cargo test -p lb-host` → all suites green (incl. the updated `ingest_test`). `cargo test --workspace`
green (144 test-result lines, 0 failures). `cargo fmt` clean.

**Proof-panel UI unit — `vitest run` (8 passed):** the demo composition — header/ws-badge,
ingest→read round-trip (asserts the real `Sample` shape + the read-back value), write-denied honest
error, outbox counts + Refresh re-read, inbox list + Approve→`inbox.resolve`, inbox honest empty,
series browse. Remote `vite build` → `dist/remoteEntry.js` (92.76 kB) green.

**Live real-gateway — `ProofPanel.gateway.test.tsx` (9 passed, 5 new)** against a REAL spawned
gateway node, seeded via the real write path:
```
✓ ingest.write → series.latest round-trips live (the page creates its own data)
✓ ingest.write is denied for an out-of-scope page (local filter) — deny per verb
✓ outbox.status reads real effects live, and denies an out-of-scope page
✓ inbox.list → inbox.resolve round-trips live, and denies an out-of-scope page
✓ the workflow surface is workspace-isolated — ws-B sees none of ws-A's items/effects
(+ the 4 pre-existing read-half / grant-intersection cases)
```
Full shell `test:gateway` suite: **65 passed (21 files)**, no regressions.

**Live Playwright e2e — `e2e/proof-panel.spec.ts` (1 passed):** built shell on :4173 → real node on
:8080 → login `user:ada`/`acme` → open Proof Panel → **click Write sample → `demo-latest` renders the
committed value** → **Refresh outbox → counts render** → NO "Invalid hook call" / two-React / console
errors. Fresh screenshot at `ui/e2e/__screenshots__/proof-panel-mounted.png` (all four sections live).

## Finding — SurrealKV "Invalid revision" on the persistent store (engine bug, not ours)

The live demo's write path failed on the **persistent SurrealKV** store with `Invalid revision N for
type Value` on the SECOND ingest write to a workspace (the drain that re-reads staging over an
already-committed `series` table). Proven **pre-existing and independent of this slice** by reproducing
it on the untouched `POST /ingest` route. The in-memory engine (every automated test) is unaffected.
**Worked around:** ran the live-demo node on the in-memory engine (unset `LB_STORE_PATH`) — still a
real node (caps/bus/ingest/federation), just ephemeral. Logged in
[../../debugging/store/surrealkv-invalid-revision-on-drain-reread.md](../../debugging/store/surrealkv-invalid-revision-on-drain-reread.md);
durable on-disk ingest is a store-owner follow-up. (The user reset the corrupt dev-store this session.)

## Decisions (open questions resolved)

- **Build order (Q3):** ingest + outbox first (guaranteed-green live round-trip), then inbox — followed.
- **`seq` source (Q2):** auto from `series.latest`'s last seq + 1, fall back to 1 — one-click demo.
- **Inbox producer (Q1):** option (a) — honest empty state when the node emits no items. No fabricated
  workflow state; Approve/Reject is exercised by seeding a real item in the host + gateway tests.
- **Drain on `ingest.write`:** the bridge verb drains synchronously (mirrors `POST /ingest`); rejected
  the alternative of a background worker (none exists; the demo needs immediate read-back).

## Follow-ups (this slice)

- The SurrealKV persistent-store revision bug (above) — store owner.
- `fleet-monitor`'s matching rework (its widgets/page to the same surface) — the deliberately-deferred
  separate slice; untouched here per the scope.
