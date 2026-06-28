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
    MessageComposer.tsx   ← input + send
    useChannel.ts         ← data/state (history load, send → reconcile)
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

## Theme preferences

The shell now has a local theme preference layer (`ui/src/lib/theme/`) and a shadcn-style
`ThemeSwitcher` in the sidebar footer. Users can choose explicit **dark** or **light** mode and one of
three accent palettes: **amber** (default), **teal**, or **blue**. The preference is browser/webview
local under `lb.theme`, validated before use, and applied to `<html>` as `.dark` plus
`data-theme-accent="<accent>"`.

All palettes flow through the existing CSS-variable contract (`bg`, `panel`, `fg`, `muted`,
`accent`, `border`, plus the shadcn aliases such as `primary`, `card`, and `ring`) so first-party
screens, shadcn primitives, graphs, and extension UI inherit the selected palette without per-screen
branches. A small guarded script in `index.html` applies the saved preference before React mounts to
avoid a first-paint flash; `ThemeProvider` then owns the live React state.

Contrast was checked for accent text against the base background: light amber 4.66:1, light teal
4.80:1, light blue 6.38:1, dark amber 8.98:1, dark teal 10.13:1, and dark blue 6.46:1.

## Tested

Vitest `ChannelView.test.tsx` — **post a message, see it appear** (ordering, empty-message guard);
`useChannel.test.ts` (S3) — a message arriving over the (mocked) SSE stream is folded into items via
`setItems`, idempotently. `channel.api.test.ts` asserts the node contract over the fake. Rust
`commands_test` proves the IPC path reaches the real capability-checked node; the gateway's
`gateway_test` proves the HTTP/SSE path (incl. a live message pushed to the browser over a real
socket).

Theme preference coverage: `theme-storage.test.ts`, `theme-dom.test.ts`, `ThemeProvider.test.tsx`,
and `ThemeSwitcher.test.tsx` cover validation/fallback, DOM application, persistence, and accessible
switcher interaction. The shipped slice was verified with `pnpm test` (56 tests), `pnpm test:gateway`
(110 real-gateway tests), `pnpm build`, and `pnpm lint` (0 errors; legacy allowlist warnings remain).

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
