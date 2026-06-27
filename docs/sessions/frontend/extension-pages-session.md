# Session — extension UI pages in the sidebar (ui-federation, slice 1)

Status: in-progress → **slice shipped** (extension PAGES). Widgets-on-dashboard (slice 2) deferred to a
follow-up. Scope: `scope/extensions/ui-federation-scope.md` + `scope/frontend/dashboard-widgets-scope.md`
(the `[widget]` half of the contract). Stage: S9+ (extension UX).

## The ask

"I have 2 extensions loaded but see no UIs. Let an extension add a page/pages on the sidebar — and more
broadly, let an extension contribute **full pages** and/or **widgets** droppable into the core dashboard.
Use a proper extension (shadcn/tailwind)."

## Why nothing showed (the finding)

There was **no path** for an extension to contribute UI: the manifest had no `[ui]`/`[widget]` block, the
`Install` record didn't carry one, `ExtRow` (what `ext.list` returns) didn't surface one, and the shell's
`NavRail` was a hardcoded list. So "no UIs" was correct — the mechanism didn't exist. This slice builds it.

## What shipped (end to end)

**Backend (Rust):**

- **Manifest forever-contract** (`crates/ext-loader/src/manifest.rs`): frozen `[ui]` (page) + `[widget]`
  (tile) blocks — `entry`, `label`, `icon`, `scope` — both serde-defaulted (existing manifests still
  parse). An empty `entry` is treated as absent. 6 new parse tests.
- **Install carries the contribution** (`crates/assets/src/install/model.rs`): new serde `ExtUi` struct +
  `Install.ui` / `Install.widget` (serde-defaulted `Option`, like `tier`/`enabled` were added) + a
  `with_ui` builder. `install_extension` projects the manifest blocks onto the install **scope-narrowed
  to the grant** (`ui_from`) — a page can never claim a tool the admin didn't approve.
- **`ext.list` surfaces it** (`crates/host/src/ext/row.rs`): `ExtRow.ui` / `ExtRow.widget`.
- **The host-mediated bridge** (`crates/host/src/tool_call.rs` → `lb_host::call_tool`): `lb_mcp::call`
  with the node filled in — the generic MCP entry a page reaches through, authorize-first.
- **Gateway routes** (`role/gateway/src/routes/`): `GET /extensions/{ext}/ui/{*path}` serves the UI
  bundle (non-secret static code; path-traversal-guarded; CORS for the dev origin), and `POST /mcp/call`
  is the bridge endpoint (authenticate the session token the shell holds → `call_tool` → host re-checks
  cap + workspace). New `ext_ui_dir` on `Gateway` (`LB_EXT_UI_DIR`, `with_ext_ui_dir` for tests).

**Frontend (React/TS):**

- `lib/ext/ext.api.ts`: `ExtUi` type + `ExtRow.ui`/`.widget`. `lib/ipc/http.ts`: `mcp_call` → `POST
  /mcp/call`.
- `features/ext-host/`: `bridge.ts` (the `{call}` bridge — filters to the granted scope, forwards via
  IPC, **never holds the token**), `ExtHost.tsx` (dynamic-`import()` the bundle, call
  `mount(el, ctx, bridge)`, clean up on unmount, honest load/error state), `useExtensionPages.ts`
  (derive cap-gated page slots from `ext.list`).
- `features/shell/NavRail.tsx`: `Surface` is now `CoreSurface | ext:${string}`; the rail renders dynamic
  extension page slots. `App.tsx` mounts `<ExtHost>` when an `ext:<id>` surface is active. (Hook-order
  bug caught + fixed: `useExtensionPages` moved above the logged-out early return.)

**The reference extension (`ui/extensions/hello-ui/`):** a real Vite **library build** reusing the
shell's React/Vite (no second npm install) → a self-contained ESM bundle exposing `mount(el, ctx,
bridge)`. It renders a polished dark-control-surface page (matching the shell's aesthetic), shows the
workspace from `ctx`, and calls `series.find` **through the bridge** (degrading honestly to "no data" if
unavailable — no mock value). Output lands at `ui/public/extensions/hello-ui/ui/entry.mjs` so the **Vite
dev server** serves it at the same path the **gateway** would — visible in both the no-backend dev build
and the real gateway path. A matching `rust/extensions/hello-ui/extension.toml` declares `[ui]` for the
real install path.

## Trust model (the load-bearing decisions)

- **Trusted/in-process this slice** (dynamic-import + `mount`), so the page shares the shell's React and
  looks native. The untrusted **iframe sandbox** tier is the immediate follow-up (same `mount` contract,
  postMessage transport) — keyed off the publisher allow-list, not the manifest.
- **A page never holds the session token and never touches the DB.** It reaches data only via the bridge
  → `POST /mcp/call` → host re-checks cap + workspace. The UI bundle is **non-secret static code**, so it
  is served like any web asset; the *data* is gated.
- **Scope narrowed to the grant** at install + re-checked at the host: a page's reachable tool set is
  `manifest.scope ∩ granted`, re-enforced server-side.

## Tests (green)

- **Rust** `crates/ext-loader` (11): the 6 new `[ui]`/`[widget]` parse cases + existing.
- **Rust** `crates/host/tests/ext_ui_test.rs` (3): install persists + `ext.list` surfaces ui+widget;
  **scope narrowed to the grant** (an unapproved verb is dropped); **bridge denies an ungranted tool**.
- **Rust** `role/gateway/tests/ext_ui_routes_test.rs` (3): serves a bundle (correct JS MIME);
  **rejects path traversal** + 404s a missing file; **`/mcp/call` denies** (401 no-token, 403 ungranted).
- **Vitest** `features/ext-host/ExtHost.test.tsx` (3): a `[ui]` extension shows a cap-gated nav slot; an
  extension with no page shows none; the bridge **forwards in-scope, rejects out-of-scope**.
- Full suites: **63 Vitest pass**, gateway + host suites pass, `cargo build --workspace` + `cargo fmt`
  green, all new files well under the FILE-LAYOUT 400-line limit.
- Pre-existing unrelated red: `github_bridge_normalize_test` needs the github-bridge wasm built (env
  gap, not this change); `ui/src/test/real-gateway.ts` has node-types tsc errors (the in-flight
  real-gateway harness, not this change).

## How to see it

- **No-backend dev (what was running):** `cd ui && npm run dev` → log in → a **Hello UI** slot appears in
  the sidebar; opening it mounts the extension's page (its `series.find` call shows "no series yet" since
  the dev fake has no data — honest, not faked).
- **Real gateway:** run the node with `LB_GATEWAY_ADDR` + point `LB_EXT_UI_DIR` at a dir holding
  `hello-ui/entry.mjs`; install `rust/extensions/hello-ui/extension.toml` (approving the series caps); set
  `VITE_GATEWAY_URL`. The page mounts from the gateway and `series.find` returns real series.

## Follow-ups (named, not silent)

- **Untrusted iframe-sandbox tier** (the security baseline for non-allow-listed publishers).
- **Externalize React via an importmap** so bundles aren't ~750 KB (share the shell's React).
- **Design-token exposure** so a trusted page styles with the shell tokens (drops the inline palette).
- **Widgets onto the dashboard** (slice 2): the `[widget]` contract is shipped end-to-end through
  `ext.list`; rendering it needs the dashboard core (`scope/frontend/dashboard-scope.md` Phase 1).
- **Tauri desktop path** for extension pages (this slice is the browser/gateway path).
- Retire the `ext.fake` dev seed once the real-gateway test harness lands (consistent with STATUS item 00).
