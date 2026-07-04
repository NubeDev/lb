# Frontend (as built)

One React + TypeScript codebase. A **channel view** runs in a Tauri v2 shell (in-process node over
IPC) AND in a plain browser against a real node over **SSE/HTTP** (S3). Promoted from
`scope/frontend/` after the messaging slice; the S3 transport swap is in
`../../sessions/sync/multi-node-sync-session.md`.

> **UI standard:** every surface is held to `scope/frontend/ui-standards-scope.md` — shadcn/ui
> primitives only (`components/ui/*`), the Members page + NavRail sidebar as the canonical look,
> and responsive/mobile auto-resize. `features/members/MembersView.tsx`,
> `features/extensions/ExtensionsView.tsx`, and `features/shell/NavRail.tsx` are migrated
> references; the rest are moving onto it incrementally.

## Layout (FILE-LAYOUT §4 — one component/hook per file)

```
ui/src/
  features/channel/
    ChannelView.tsx       ← composes the screen (layout + wiring only)
    MessageList.tsx       ← presentation only
    palette/CommandPalette.tsx ← the input as a command surface (/ menu, arg rail; supersedes the
                                 removed MessageComposer.tsx)
    useChannel.ts         ← data/state (history load, send → reconcile, postQuery/postAgent)
    index.ts              ← barrel (re-export only)
  lib/channel/
    channel.api.ts        ← one call per export: post(), history()
    channel.stream.ts     ← the SSE live feed (openChannelStream) — S3
    channel.types.ts      ← Item (mirrors lb_inbox::Item)
  lib/ipc/
    invoke.ts             ← the single transport seam (Tauri | HTTP | fake)
    http.ts               ← real HTTP transport to the gateway — S3
    fake.ts               ← in-memory node stand-in (tests)
ui/src-tauri/             ← the Tauri v2 desktop shell (the node runs in-process)
```

## Cross-stack symmetry

A verb has the **same name** in the host, the shell command, and the client:
`lb_host::post` ↔ Tauri `channel_post` ↔ `channel.api.ts` `post()`. Opening any one tells you
where to look for the others.

## The transport seam (one file, three transports)

`lib/ipc/invoke.ts` is the one place that knows how to reach the node. It picks by environment:

1. **Tauri shell** → the Rust command via `@tauri-apps/api` (the node runs in-process).
2. **Browser + gateway** → real **HTTP** (`http.ts`) to the node's SSE/HTTP gateway, when
   `VITE_GATEWAY_URL` is set (the browser build). This is the S3 swap that replaced the fake.
3. **Tests** → a faithful in-memory **fake** (`fake.ts`) with the same contract (ordered,
   idempotent on id, workspace-scoped).

Feature code never branches on the transport, so the same `ChannelView`/`channel.api` power all
three unchanged — the S3 change was literally this one file (plus the new `http.ts`/`channel.stream.ts`).

## Live updates over SSE (S3)

`channel.stream.ts` opens `GET /channels/{cid}/stream` and receives the gateway's `message` and
`presence` events. `useChannel` subscribes and folds OTHERS' live messages into its **existing
`setItems` sink** — an idempotent merge by id (the node's contract), so a live item that also
arrives via a later history refresh never duplicates. In the Tauri shell / tests there is no
gateway URL, so the stream is a no-op and the post→refresh round trip is the feed (as at S2).

## The Tauri shell

`ui/src-tauri/` is a Tauri v2 shell; **the node runs in-process** (the shell IS a node, §3.1). The
IPC commands `channel_post` / `channel_history` are thin glue over `lb_host::post`/`history` with
the session principal — the *same* capability check guards the desktop UI as every other caller.
Command logic is a library so it is unit-tested **headlessly** (no webkit toolchain); the window
wiring is behind a `desktop` feature, and the windowed `tauri build` is a packaging step for a
machine with the desktop toolchain.

## Visual direction

Quiet control-surface tokens (CSS variables, themed by a `.dark` class): near-black dark / warm
paper light, one warm amber accent, hairline borders, lucide icons. Tailwind utilities; shadcn-
style primitives to be pulled in as the component set grows.

## Theme preferences — the Customizer (in Settings → Theme)

The full theme customizer lives in **Settings → Theme** (`features/settings/ThemeSettingsTab.tsx`,
deep-linkable at `/t/<ws>/settings/theme`) — the old nav-footer `ThemeSwitcher`/`Customizer` sheet was
removed. Settings tabs are URL-routable (`/settings/<tab>` — preferences/theme/agent), so each is
shareable and the back button works; bare `/settings` redirects to the default tab. The member's
preference is `{ mode, preset, radius, layout, custom?, imported? }` (`ThemePreference`), and the Theme
tab has two sub-tabs:

- **Theme** — light/dark, a preset library (three built-in accents amber/teal/blue + a curated
  shadcn/tweakcn subset), a radius control, **paste-to-import** a tweakcn CSS block, and per-token
  **brand colors**.
- **Layout** — the sidebar **variant** (sidebar/floating/inset), **collapsible mode**
  (offcanvas/icon/none), and **position** (left/right), spread by `NavRail` onto the shipped shadcn
  `<Sidebar>`.

**The load-bearing choice: presets write the project's BASE tokens, not shadcn tokens.**
`styles/globals.css` DERIVES the shadcn tokens (`--primary`/`--background`/`--card`) FROM a small base
palette (`--bg`/`--panel`/`--fg`/`--muted`/`--muted-foreground`/`--accent`/`--border`), and every host
surface (charts via `features/charts/chartTheme.ts`, panels, nav) reads the BASE tokens.
So a preset is normalized **back onto base tokens** by the adapter (`lib/theme/preset-adapter.ts`:
`--primary`→`--accent`, `--background`→`--bg`, `--card`/`--popover`→`--panel`,
`--border`/`--input`/`--ring`→`--border`, …), written as inline HSL-triplet overrides on `<html>`, and
the CSS derivation re-themes **charts, panels, dashboards, nav rail, and editor chrome at once**. A
built-in accent instead uses `data-theme-accent` (values in `globals.css`); custom/imported/library
presets write inline base tokens and clear the attribute. Import/oklch/hex/hsl all normalize through
`lib/theme/color-to-hsl.ts`.

**Persistence rides the shipped `prefs` verbs** — a new nullable, opaque `ui_theme` axis on the
`lb_prefs::Prefs`/`ResolvedPrefs` record (NOT a generic key/value store — the prefs record is a closed
struct). The whole `ThemePreference` (incl. `layout`) is stored as one JSON blob and folds **whole**
through the existing resolve chain: **member → workspace-default → built-in**. So a member's theme
roams across browser/desktop, an admin can set a **workspace-default** theme via the admin-gated
`prefs.set_default`, and a member override wins where set. `localStorage` (`lb.theme`) is only the
first-paint cache; `prefs` is the authority, reconciled on mount. No new MCP verb, table, or
capability — persistence reuses `prefs.get`/`set`/`resolve`/`set_default` and their gates.

## Tested

Vitest `ChannelView.test.tsx` — **post a message, see it appear** (ordering, empty-message guard);
`useChannel.test.ts` (S3) — a message arriving over the (mocked) SSE stream is folded into items via
`setItems`, idempotently. `channel.api.test.ts` asserts the node contract over the fake. Rust
`commands_test` proves the IPC path reaches the real capability-checked node; the gateway's
`gateway_test` proves the HTTP/SSE path (incl. a live message pushed to the browser over a real
socket).

Customizer coverage (unit, `pnpm test`): `preset-adapter.test.ts` (the load-bearing shadcn→base
round-trip — the "existing UI re-themes" guard), `theme-import.test.ts` (tweakcn paste → base tokens,
fail-closed on malformed), `color-to-hsl.test.ts` (hex/oklch/hsl→triplet), `theme-dom.test.ts` (inline
base tokens vs. built-in accent path, light↔dark variant re-apply, radius), `theme-storage.test.ts`
(validation/fallback, no legacy compat), `ThemeProvider.test.tsx` (cache→apply→persist),
`LayoutTab.test.tsx` (sidebar variant/collapsible/side pickers), and `NavRail.test.tsx` (the themed
layout reaches the `<Sidebar>` as `data-variant`/`data-side`). Persistence over the REAL gateway
(`pnpm test:gateway` — `theme-prefs.gateway.test.ts`): member round-trip + roam, workspace-default
fold, **capability-deny** (member without `prefs.set`; non-admin without `prefs.set_default`), and
**workspace-isolation** (ws-A theme never resolves in ws-B). Rust `cargo test -p lb-prefs`
(`ui_theme_test`, `resolve_test`) proves the axis round-trip, whole-fold, and isolation on the real
store. Verified with `pnpm test` (472), the gateway suite, `cargo test -p lb-prefs -p lb-host` (green),
`cargo fmt`, `tsc`, and `eslint` (0 errors on new files).

## Make collaboration real (shipped)

The UI is no longer a single-screen demo on fakes. A **real login→token→principal session** (the
gateway mints + verifies a signed `lb_auth` token per request; the demo principal is gone), a
**workspace switcher**, a **channel registry** (list / create / create-on-post), **members/teams**,
**rendered presence**, the **real `lb_inbox` queue** (Approve/Reject = the S6 gate as a UI action),
and a **read-only outbox status** view. The workspace is the token's hard wall, so the two-session
isolation test is finally real. See `frontend/collaboration.md`.

## Not yet built

The full operational shell (dashboard / extensions / settings, the rest of the P0 plan in
`scope/frontend/`); token-on-the-bus for a routed cross-node caller; a real IdP behind the `verify`
seam (the credential check is a dev-login today); the Tauri **desktop** command layer's session (the
collaboration slice wired the browser/gateway path; `src-tauri/src/state.rs` still fixes its
workspace); the native window packaging build.
