# Extensions — `fleet-monitor`: a self-contained backend+frontend extension over real Module Federation (session)

- Date: 2026-06-27
- Scope: ../../scope/extensions/ui-federation-scope.md + ../../scope/frontend/dashboard-widgets-scope.md
- Stage: S10 (extension UX) — the UI-federation slice
- Status: done (native sidecar + federated shadcn page (3 nested routes) + 2 widgets + contract refactor + tests, all green)

## Goal

Make an extension **one thing**: backend + frontend in **one folder**, each part optional. Replace the
wrong split (`ui/extensions/hello-ui` — a frontend living apart from any backend) with a single
self-contained extension `rust/extensions/fleet-monitor/` that ships BOTH:

- a **native Tier-2 backend** (its own OS process / PID, supervised over stdio) exposing an MCP tool;
- its **own frontend co-located** under `rust/extensions/fleet-monitor/ui/`, built as a **real Vite
  Module Federation remote** (shares the shell's React singletons — not a hand-rolled `import()` of a
  self-bundled ESM), with real **shadcn/ui + Tailwind**, mounting a **cap-gated sidebar page with 3
  nested routes** and declaring **2 dashboard widgets**.

Data reaches the UI **only** through the host-mediated bridge (`mount(el, ctx, bridge)` → `POST
/mcp/call`); never the token or DB.

## What changed

### The deletion
`ui/extensions/hello-ui` and `rust/extensions/hello-ui` are **hard-deleted**. The premise was wrong: a
frontend is not a standalone thing under `ui/` — it is a *part of* an extension, beside its backend.

### Contract refactor — `[widget]` → `[[widget]]` (singular → plural)
The model was singular `widget: Option<Widget>` end to end. The ask is **2 widgets**, and a palette is
plural by nature, so I changed the durable contract to **`widgets: Vec<Widget>`** (TOML array-of-tables
`[[widget]]`), serde-defaulted to empty, across the whole chain:
- `lb-ext-loader::Manifest.widgets: Vec<Widget>` (parser + tests, incl. a new multi-widget test).
- `lb-assets::Install.widgets: Vec<ExtUi>` + `with_ui(ui, widgets)`.
- `lb-host::ExtRow.widgets: Vec<ExtUi>`.
- `ui/.../ext.api.ts ExtRow.widgets?: ExtUi[]` (mirrors the Rust row).
This is strictly more general and additive (empty default = old behavior). Recorded in the
dashboard-widgets scope as a resolved decision (the frozen `[widget]` block is now `[[widget]]`, fields
unchanged).

### Load-bearing fix — the **native** install path did not persist `[ui]`/`[widget]`
`install_native` built the `Install` with `Install::new(...)` and **never** called `.with_ui`, so a
**native** extension's page/widgets silently never surfaced in `ext.list`. Since `fleet-monitor` is
native **and** has a UI, this was the difference between "looks done" and "works". Fixed by extracting
the manifest→`ExtUi` projection into a shared `crates/host/src/ui_decl.rs` (`project(manifest,
granted) -> (Option<ExtUi>, Vec<ExtUi>)`, scope-narrowed to the grant) and calling it from **both** the
wasm (`install.rs`) and native (`native/install.rs`) install paths. Regression test:
`fleet_monitor_test.rs::native_install_spawns_child_and_surfaces_page_plus_two_widgets`.

### The backend (`rust/extensions/fleet-monitor/`)
A native Tier-2 sidecar modeled on `echo-sidecar`: `[runtime] tier="native"`, `[native] exec=…`,
supervised over `Content-Length`-framed stdio using the shared `lb-supervisor` wire types (no ABI
drift). FILE-LAYOUT split: `main.rs` (the stdio loop) + `call.rs` (tool dispatch). One tool,
`fleet.summary`, stateless, tagged with the injected `LB_EXT_WS` (proves the scoped identity reached
the child — its own PID). The UI does **not** bind to `fleet.*` — it binds to the **frozen series read
verbs** (`series.find`/`series.latest`/`series.read`), keeping the bridge contract un-widened; the
native tool exists to prove a real backend ships alongside the frontend in one folder.

### The frontend (`rust/extensions/fleet-monitor/ui/`) — real Module Federation
- `vite.config.ts` uses `@originjs/vite-plugin-federation` as a **remote**: `exposes: { "./mount":
  "./src/mount.tsx" }`, `shared: ["react","react-dom"]`, `filename: "remoteEntry.js"`, target esnext.
  Output `dist/assets/remoteEntry.js` — exactly the manifest's `[ui] entry`. React is **shared** (the
  shared chunks are 62-byte re-export stubs — no second React copy).
- `mount(el, ctx, bridge)` → `createRoot(el)` → returns `() => root.unmount()`. Byte-for-byte the
  shell's `RemoteMount` contract.
- **3 pages, real nested routing** (`MemoryRouter`, so it never touches the shell URL): parent
  `Overview` (header + sub-nav + `<Outlet/>`) with `Nodes` (index, `series.find`) and `Alerts` (nested,
  `series.latest`) rendering in the Outlet. Honest loading/empty/error states; no fabricated data.
- **2 widgets** (`FleetStatusWidget`, `FleetSparklineWidget`) — real shadcn Cards, honest
  "placeholder" notes (non-functional this slice, per the ask), declared in the manifest's two
  `[[widget]]` tiles.
- Real shadcn/ui primitives (`Card`/`Button` (cva)/`TabBar`) + Tailwind, styled with the **shell's
  token names** (`--bg/--panel/--border/--fg/--muted/--accent`) so the page looks native.

### The shell (host) side
- `ui/vite.config.ts` (and `vitest.gateway.config.ts`) now run the federation plugin as the **HOST**
  (`name: "shell"`, `shared: ["react","react-dom"]`), so the shell exposes its React singletons to
  remotes. Build target esnext.
- New `ui/src/features/ext-host/federation.ts` — the runtime loader: dynamically registers an
  extension's remote by gateway URL (`__federation_method_setRemote`/`getRemote`) and returns its
  `./mount`. Idempotent per extension id (React stays one instance).
- `ExtHost.tsx` rewritten from raw `import(url)` → `loadRemoteMount(ext, remoteEntryUrl)` (federated,
  shared React). Bridge/`/mcp/call`/`ext.list` wiring unchanged.
- **Fake removed from page discovery**: `useExtensionPages.ts` no longer calls `seedDevExtensions()` —
  it reads only the real `ext.list`. The dead `seedDevExtensions` (which referenced the deleted
  `hello-ui`) is gone. The shared in-memory IPC harness (`fake.ts`) used by many unrelated suites is
  left intact (ripping it out is a separate, large migration — the project already has a parallel
  real-gateway suite, which is where this slice's ext-host test now lives).
- `build.sh` builds both halves and **stages** `ui/dist/*` → `{LB_EXT_UI_DIR}/fleet-monitor/`, the
  exact path the gateway serves (`GET /extensions/fleet-monitor/ui/assets/remoteEntry.js`).

### Pre-existing CI breakage fixed (in-bounds, I was editing the file)
`cargo build --workspace` (a CI exit-gate command) was **already red** at HEAD: `test_gateway_seed.rs`
is a `#[path]` module of the feature-gated `test_gateway` bin, but living under `src/bin/` cargo
auto-discovered it as a second, un-gated bin and failed on the `test-harness`-only `lb-outbox` import.
Fixed with `autobins = false` + the one explicit `[[bin]]` already declared. `cargo build --workspace`
is green again.

## Decisions (and rejected alternatives)

- **Real Vite Module Federation, not dynamic `import()`.** The frozen scope leaned "dynamic `import()`
  of an ESM remote"; the explicit ask (and the long-term-correct choice) is **true MF with shared
  singletons** so the remote uses the shell's React and feels native. Rejected the raw-`import()` path
  (it bundles a second React → hook-dispatcher mismatch, not native). Recorded as an upgrade of the
  scope's federation-tooling open question.
- **UI binds to the frozen `series.*` read verbs, not new `fleet.*` UI tools.** Keeps the bridge/widget
  trust surface frozen (no stop-and-confirm re-open). The native `fleet.summary` exists for the backend
  proof, not as a UI data source. Rejected adding `fleet.list`/`fleet.summary` to the bridge (widens a
  frozen trust boundary for cosmetics).
- **`[[widget]]` plural.** Rejected a second singular field or a hacky `widget2`. A palette is plural;
  the contract should be `Vec`.
- **Left the shared `fake.ts` harness intact.** Rejected a full rip-out (dozens of unrelated suites
  depend on it; the project already migrates incrementally to the real-gateway suite). Removed only the
  fake **dependency in page discovery**, per the ask.

## Tests (real infra, seeded via the real write path — no mock node, no fake backend)

### Rust (`cd rust`)
- `cargo test -p fleet-monitor` — backend tool dispatch (3): `fleet.summary` tagged with injected ws,
  unknown-tool is an explicit error, bad-params is an error not a panic.
- `cargo test -p lb-ext-loader` — manifest (12) incl. `parses_multiple_widget_blocks`,
  `ui_and_widget_together`.
- `cargo test -p lb-host --lib ui_decl` — projection (2): projects page + every widget; narrows scope
  to grant.
- `cargo test -p lb-host --test ext_ui_test` — wasm UI persist + bridge deny (3); both `[[widget]]`
  tiles surface, scope-narrowed.
- `cargo test -p lb-host --test fleet_monitor_test` — **native** e2e (2): real `OsLauncher` child
  spawns (own PID) + `fleet.summary` answers tagged with ws; `ext.list` surfaces the page + **both**
  widget tiles for a **native** extension; native UI scope narrowed to the grant (the regression test
  for the native-persist fix).
- `cargo build --workspace` — green (CI gate restored).

Green output:
```
running 3 tests  (fleet-monitor)            ... ok   (3 passed)
running 12 tests (lb-ext-loader manifest)   ... ok  (12 passed)
running 2 tests  (lb-host ui_decl)          ... ok   (2 passed)
running 3 tests  (lb-host ext_ui_test)      ... ok   (3 passed)
running 2 tests  (lb-host fleet_monitor_test) .. ok  (2 passed)
   Finished cargo build --workspace
```
Mandatory categories: **capability deny** — `ext_ui_test::bridge_denies_an_ungranted_tool` +
scope-narrowing tests (page/widget can't claim an ungranted tool); **workspace isolation** — the native
e2e asserts `fleet.summary` is tagged with the install's own ws (the injected `LB_EXT_WS`), and the
bridge derives ws from the session token, never the page.

### Frontend
- Extension UI (`cd rust/extensions/fleet-monitor/ui && pnpm test`) — **6 passed**: `mount()` renders
  + returns unmount; nested routing renders Overview → Nodes/Alerts in the Outlet; `Nodes` calls
  `series.find` and renders the list; a rejected bridge call renders an honest error. `pnpm build`
  produces `dist/assets/remoteEntry.js` (shared-React stubs confirm no bundled copy).
- Shell default suite (`cd ui && pnpm test`) — **20 passed** (App nav cap-gating intact after the
  federation + `ext.api` changes).
- Shell real-gateway suite (`pnpm test:gateway`) — **50 passed**, incl. `ExtHost.gateway.test.tsx` (4):
  `fleet-monitor`'s `[ui]` page slot shows from the **real** `ext.list`, and **both** `[[widget]]`
  tiles round-trip through a **real** seeded `Install` — the no-fake path end to end.

## Debugging

One pre-existing **flaky** test surfaced under full-workspace parallel load:
`lb-host offline_sync_test::offline_edge_writes_apply_idempotently_on_reconnect` (a sync/timing test,
untouched by this slice; passes 3/3 in isolation). Not this slice's regression — noted, not chased. No
new debug entry (nothing this slice broke).

## Open questions → resolved/refreshed

- ui-federation scope: federation tooling — **resolved to real Vite Module Federation** (shared
  singletons), upgrading the earlier "lean: dynamic import()". Manifest `[ui]` multi-page — this slice
  ships a single `[ui]` entry that itself does **client-side nested routing** (3 routes), which covers
  the multi-page need without a multi-entry manifest.
- dashboard-widgets scope: the frozen `[widget]` block is now **`[[widget]]`** (plural tiles); fields
  unchanged. Widget *mounting* (a separate federated expose) remains a follow-up; the tiles are
  declared + surfaced in `ext.list`/palette this slice (non-functional placeholders, as scoped).

## Follow-ups (explicit, not silent gaps)

- Widget **rendering** in a grid cell (a `./widget` federated expose + the `WidgetHost` renderer) — the
  tiles are declared and surfaced now; mounting them is the next slice.
- The **untrusted iframe** tier (this slice ships the trusted in-process MF tier only).
- Fully retiring `fake.ts` across all shell suites (incremental, tracked by the real-gateway migration).
