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

## Replaces the earlier draft

An earlier session draft described a `proof.status` tool with no grant-intersection or real-gateway
test. This session supersedes it: the tool is `proof.ping` with the scope-mandated snapshot shape, and
the grant-intersection + real-gateway page-data tests are now real and green.

## Follow-ups (not done here)

- Implement label→tag conversion at commit (the root of the discovery finding) and revisit the
  IngestView faceted-search assertion.
- A `series.watch` (bus-backed SSE through the bridge) upgrade once that verb exists.
- Wire `fleet-monitor`'s two placeholder widgets to real series verbs (the remaining native-side gap).
