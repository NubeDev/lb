# ce-v1 · S7 — `BridgeTransport` + the federated wiresheet page

Branch: `ce-v1` (worktree). Slice: [`slice-7-bridge-transport-ui.md`](../../../rust/extensions/control-engine/docs/slice-7-bridge-transport-ui.md).
Parent: [`control-engine-scope.md`](../../../rust/extensions/control-engine/docs/control-engine-scope.md).

## What shipped

The LB-authored UI half of the control engine: a federated remote under
`rust/extensions/control-engine/ui/` that mounts the vendored `@nube/ce-wiresheet` `CeEditor` wired to a
`BridgeTransport implementing EngineTransport`. The browser never touches CE — every canvas action is a
caps-gated `bridge.call('control-engine.*')`, and the live COV feed rides `bridge.watch('series.watch')`
over the shipped series SSE. The vendored package is **untouched** (S2 rule); the transport is injected
via `<CeEditor base transport={new BridgeTransport(bridge, appliance)} />`.

Files (one responsibility each):
- `ui/src/bridge-transport.ts` — the `BridgeTransport` (request map + stream half).
- `ui/src/frames.ts` — decode S6 `{kind:"cov"|"topology"}` JSON frames → `StreamHandlers` callbacks.
- `ui/src/Page.tsx` — appliance picker + empty-state add flow + editor mount.
- `ui/src/mount.tsx` + `ui/src/remoteEntry.ts` — the federation entry (proof-panel pattern).
- `ui/src/contract.ts` — `MountCtx` + `Bridge` (with the OPTIONAL `watch`).
- `ui/src/test/{bridge.stub.ts, ce-wiresheet.stub.tsx, setup.ts}` — test doubles of the bridge + the
  vendored editor SURFACE (not fakes of node behavior; see the test-harness note).
- configs: `package.json` (standalone lockfile, `--ignore-workspace`), `vite.config.ts`, `tsconfig.json`,
  `postcss.config.js`, `tailwind.config.ts`, `src/styles/tokens.css`, `src/vite-env.d.ts`, `.gitignore`.
- manifest `[ui]` block in `extension.toml`; `build.sh` extended to build the vendored dist + the remote.

## The request-map table

The wiresheet's `rest.ts` typed wrappers emit `/api/v0`-relative REST paths through the seam's
`request(EngineRequest{method,path,body})`. `BridgeTransport.map()` translates each to a
`control-engine.*` tool + arg shape (keyed node `{uid,kind:"component"}`; the selected `appliance` always
injected), and maps the tool result back to the `data` shape rest.ts expects:

| method | path | tool | body → args |
|---|---|---|---|
| GET | `/schema` | `control-engine.schema` | — → unwrap `.manifests` |
| GET | `/nodes` | `control-engine.tree` | `?depth` → `{depth}` |
| GET | `/nodes/uid/{uid}` | `control-engine.tree` | uid → `{node:{uid,kind}}` |
| POST | `/nodes` | `control-engine.add-node` | `{type,parentUid,name,initialValues}` → `{type,parent,name,initial_values}` |
| PATCH | `/nodes/uid/{uid}` | `control-engine.patch` | `{properties:{n:{value}}}` → `{node,values:{n:v}}`; unwrap `.component` |
| PATCH | `/overrides/nodes/uid/{uid}` | `control-engine.set-override` / `.clear-override` | first `setOverrides[]` → set `{node,property,value,ttl_secs}`; else first `clearOverrides[]` → clear `{node,property}` |
| POST | `/edge` | `control-engine.add-edge` | `{source*,target*Property}` → `{source,source_property,target,target_property}` |
| DELETE | `/nodes/uid/{uid}` | `control-engine.remove-node` | uid → `{node}` |
| POST | `/call/nodes/uid/{uid}` | `control-engine.call-action` | `{action,params}` → `{node,action,params}` |

**Every other path the wiresheet can emit throws a LOUD `UnmappedPathError` naming the path** (never a
silent 404) — these are the named follow-up verbs S7 does NOT back: `/undo` `/redo` `/changelog`
`/extensions` `/group` `/ungroup` `/facets/*` `/bulknodes` `/copy/nodes` `/restore` `/edges` (GET)
`/edge/uid/{uid}` (PATCH/DELETE). A canvas gesture hitting one fails visibly with the exact path, which is
the signal the extension owes that verb.

## The stream design

`openStream(handlers)`:
1. `bridge.call('control-engine.watch', {appliance})` → `{series, subject}`.
2. `bridge.watch('series.watch', {series}, onEvent)` — each SSE sample is `{payload, seq}`; the `payload`
   is one S6 frame. `frames.ts::dispatchFrame` routes `cov`→`onFrame`, `topology`→`onTopology`.
3. `onStatus("open")` when the watch arms; `"closed"` on unsubscribe / arm-failure.

`frames.ts` builds the editor's `DecodedFrame` from the cov frame: values → one non-STATUS section
(generic `TYPE_F64=0x22` tag; the editor routes any non-STATUS tag to its value store), nonzero status →
a `TYPE_STATUS=0x40` section. A `>2^53` integer that arrived as a JSON string (frame.rs's bigint-safety
rule) is coerced back to a `bigint`. Topology frames map to `topologyAdded/Removed/Changed` with the uid
lists (a resync signal — the editor refetches via `control-engine.tree`).

**Subscriptions are v1-simple:** `openStream` arms the WHOLE appliance's COV feed, so
`setSubscriptions`/`setPropSubscriptions` are no-ops (the editor filters to visible uids client-side).
Per-uid bus scoping is a measured follow-up. `setTickHz`/`getTickHz` are inert (the sidecar owns the CE
tick rate); `sessionId` is null. If the bridge has **no `watch`** (Tauri desktop, the vitest harness),
`openStream` reports `closed` and degrades to a static canvas — the request half still works, so structure
renders without live values (absent-not-broken, no throw).

## v1 gaps (absent-not-broken, per the slice)

- **Presence hidden** — presence is direct-mode-only in the seam; a bridge transport drops it. The
  PresenceBar simply shows nobody.
- **Per-actor undo is engine-shared** — no `actor` id is forwarded over the bridge, so undo/redo run on
  CE's shared stack (fine for v1). `originSessionId` on decoded topology frames is `null`.
- **Drag-position not persisted** — `PATCH /nodes/uid/{uid}` maps only property `values` to
  `control-engine.patch`; `name`/`position` are NOT carried (there is no `set-layout` verb yet). A drag or
  rename shows locally but snaps back on reload. Parked until the deferred `control-engine.set-layout`.
- **Override batch** — the wiresheet's `OverridesRequest` can carry several set/clear ops; the v1 map
  routes the FIRST set (else first clear). The canvas gesture is one op at a time; a true batch is a
  follow-up.

## `@nube/ce-wiresheet` resolution decision

The ext UI is a **standalone package** (its own `pnpm-lock.yaml`, installed `--ignore-workspace`), exactly
like `proof-panel/ui` — NOT a pnpm workspace member. `packages/ce-wiresheet` IS a root-workspace member and
produces a `dist/` via `pnpm build:lib`. So the ext UI resolves the vendored package by **vite alias to the
built dist** (`vite.config.ts` `resolve.alias`: `@nube/ce-wiresheet` → `../../../../packages/ce-wiresheet/
dist/ce-wiresheet.js`, `@nube/ce-wiresheet/style.css` → the built stylesheet). `react` is externalised in
BOTH this build and the ce-wiresheet lib build, so there is no second React. `build.sh` builds the vendored
dist FIRST, then the CE remote. No `workspace:*` / publish / link dance; no vendored edit.

The editor exports `CeEditor` as a NAMED export in the built lib (`export { default as CeEditor }` in
`index.ts` → `CeEditor` named in the dist), so the page imports `{ CeEditor }`. The compiled editor CSS is
injected `?raw` (verbatim bytes) rather than `?inline` — the ext UI runs a tailwind-v3 PostCSS pipeline and
must NOT re-process the already-compiled tailwind-v4 vendored stylesheet.

## Seam gap hit (resolved WITHOUT a vendored edit — flagged for upstream)

The vendored `index.ts` exports `StreamHandlers` (and the transport interfaces) but does **NOT** re-export
the `DecodedFrame` / `DecodedValue` / `TopologyMsg` / `SchemaMessage` TYPES nor the wire-tag CONSTANTS
(`TYPE_STATUS` / `MSG_UPDATE`) — those live only in `lib/wire.ts` + `lib/engine-types.ts`. `frames.ts`
needs them. Resolved on the LB side without touching the package: the TYPES are recovered from the exported
`StreamHandlers` signature via `Parameters<>`, and the two wire tags are declared as fixed S6-protocol
literals in `frames.ts`. **Recommended upstream follow-up (S1 branch, then re-vendor):** re-export
`DecodedFrame`/`DecodedValue`/`TopologyMsg`/`SchemaMessage` + the wire/msg tag constants from `index.ts`
so a transport author imports them by name. Not blocking — the derivation is exact.

## Tests (the gate) + the test-harness reality

Co-located vitest UNIT tests (no network) — **27 green**:
- `frames.test.ts` (7): cov values+status decode, clean-tick omits STATUS, the `>2^53`-as-string→bigint
  case, topology added/removed, dispatch routing, unknown-kind ignored.
- `bridge-transport.test.ts` (18): a fixture list asserts every REST path the wiresheet can emit maps to a
  tool with the appliance injected; unmapped paths throw the loud `UnmappedPathError` naming the path; arg
  translation (PATCH→patch, DELETE→remove-node, POST /nodes→add-node, overrides→set-override) against a
  recording stub bridge; `openStream` pipes a seeded cov frame into `onFrame` (watch bridge) and degrades
  to `onStatus("closed")` with no throw when the bridge has no `watch`.
- `mount.test.tsx` (2): the page + appliance picker renders with a stub bridge whose `appliance.list`
  returns one appliance (and the stubbed editor mounts); the empty-state add form renders when the list is
  empty.

Builds green: `pnpm test` (unit), `vite build` (the `dist/remoteEntry.js` lib build), the vendored
`packages/ce-wiresheet` `pnpm build:lib`, and `tsc --noEmit` clean.

**Why the live path is NOT proven in `pnpm test:gateway`:** the gateway vitest harness has no SSE/watch
transport and has never spawned a native sidecar (thecrew + proof-panel explicitly punt live SSE to
Playwright — see the NOTE in `ui/src/features/ext-host/TheCrew.gateway.test.tsx`). So per the slice's
harness constraint, S7's gate is the UNIT tests; the real-gateway proof (cloud UI editing a `ce-studio`
engine on the same box through the full bridge, incl. a live COV frame updating a rendered value) is done
MANUALLY by the integrator against a live engine, not in `test:gateway`. No native-spawn / SSE plumbing was
added to the harness (out of scope).

**Live-engine proof (added at integration — now an automated opt-in tier):** rather than leave the live
read path unproven, `src/bridge-transport.live.test.ts` spawns the REAL `control-engine` sidecar (real
`rubix-ce` client, no fake) and drives the REAL `BridgeTransport` over its stdio control line against a
live ce-studio. Env-gated (`CE_ENGINE_URL` + `CONTROL_ENGINE_BIN`), skipped by default so CI stays green.
**Verified GREEN against a live engine on `:7979`** (2026-07-02): `GET /nodes` → the engine's real
component tree, `GET /schema` → the live type catalogue — i.e. `BridgeTransport` (REST path → tool → args)
→ sidecar → live CE → verbatim DTO → result-unwrap, no stub anywhere in the chain. The live COV frame →
rendered value remains a Playwright/manual step (the SSE side still has no vitest transport).

The test doubles (`bridge.stub.ts`, `ce-wiresheet.stub.tsx`) are doubles of the bridge INTERFACE and the
vendored editor's SURFACE — allowed per testing-scope §0 (the real node isn't in the remote's process; the
vendored editor is a UI component, not node behavior). The `ce-wiresheet.stub.tsx` re-exports the GENUINE
vendored types from source so the transport type-checks against the real seam.
