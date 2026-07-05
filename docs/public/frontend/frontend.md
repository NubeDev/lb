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

### Appearance — looks, fonts, surfaces, motion (theme-appearance scope)

The Theme tab was widened past colors into the whole **look and feel** — the preference is now
`{ mode, preset, radius, layout, look, fontSans?, fontMono?, surface?, motion?, custom?, imported? }`,
still one opaque `ui_theme` blob (zero backend change; a v1 seven-token custom palette migrates by
**deriving** the new tones, never a fail-closed drop).

- **Look packs** (`lib/theme/theme-looks.data.ts`) — six one-click looks as DATA, each reading as
  ITSELF: Operator Console (warm charcoal + amber, dark), Code Editor (cool slate-blue + cyan, dark,
  sharp), Professional (**light** paper + serif + indigo, elevated), Retro Terminal (phosphor green on
  near-black, mono, square), Modern Dashboard (**light** airy + large radius + cyan), Liquid Glass
  (violet + translucent blur, dark). Each is a bundle of per-axis defaults INCLUDING **`mode`** — a look
  stamps light/dark on pick, which is what makes Professional/Modern land as genuine *light* looks. The
  resolver (`look-resolve.ts`) folds per-axis: **pinned look axis → explicit member override → look
  default → built-in**. Picking a look resets the axes it defines (lands like its thumbnail); only
  `retro` *pins* its preset (data `pins:["preset"]`, no code branch — rule 10).
- **Fonts** — `--font-sans`/`--font-mono` tokens; a curated self-hosted list (Inter/Geist/IBM Plex
  Sans + Source Serif 4; JetBrains Mono/IBM Plex Mono). woff2 is **lazy-loaded on selection** via
  dynamic `import()` (`font-loader.ts`); the system stack is the zero-cost default and stays in the
  main bundle — a picked family is a separate chunk, never eager.
- **Surfaces** — a `data-surface` attribute (flat/elevated/glass) + tokens (`--surface-alpha`/`--blur`/
  `--shadow-1..3`/`--gradient-accent`) restyle every `[data-panel]` (card/sheet/dialog, **the nav rail,
  the shared `AppRail`, and dashboard grid cells**) by CASCADE. Glass degrades **glass→elevated→flat**
  via `@supports (backdrop-filter …)` — a runtime capability degrade, never an `if desktop` branch. Glass
  is an opt-in *look*, NOT the default: the default board is crisp/flat (the product register's
  anti-references reject decorative glass).
- **Motion** — a `data-motion` attribute (off/subtle/full) fences AND scales CSS transitions (e.g. the
  nav-rail collapse: off 0s / subtle 120ms / full 320ms spring-ease); a `useMotionPref` hook gates the
  springy `motion` (motion.dev) primitives — `Reveal` (page-body + settings-tab entrances), `Stagger`
  (look-card grid), `Collapse` (Brand-colors accordion). `motion` is imported in EXACTLY ONE seam
  (`lib/motion/motion.ts`) so the off switch is trustworthy; every primitive renders static when off.
  `prefers-reduced-motion` forces off unless the member explicitly chose full.
- **Wider tones, actually CONSUMED.** `--panel-2` (raised neutral layer — nav rail, page-header band,
  tab bars, `AppRail`; on dark it's nudged *cooler* per the product register), `--overlay` (a real modal
  scrim, dark in both modes), `--accent-2` (the active-nav pill + interaction accents), and semantic
  `--success`/`--warning` (telemetry badges) — so the shell reads **>2 tones**, not a two-step surface.
  The dark ramp is tuned for VISIBLE separation (bg 7% → panel 11% → panel-2 15% + a 22% hairline);
  elevation reads via crisp borders + a 1px inset top-highlight (the Linear/Stripe trick), not shadows.
- **The color picker** is a hand-authored in-DOM popover (H/S/L + hex, whole-row clickable) — no native
  `<input type="color">` (WebKitGTK ships none, so the old desktop click was a no-op).
- **Radius** now derives the FULL `rounded` scale from `--radius` in `@theme` + a cascade-last
  `:root:root` override (so a radius nudge visibly re-rounds every card/input/chip — the shipped bug).

**Extensions re-theme live (v4).** A single `lb:themechange` emitter (`theme-events.ts`) fires once per
application; `ExtWidget` resolves the widened `ctx.theme` (base + tones + radius + fonts + surface +
motion + the core chart ramp, from `getComputedStyle` so custom colors are honored) and pushes it
through the shipped `update(ctx)` path — a canvas widget (ECharts) recolors **in place, no re-mount**.
The widget contract bumped `WIDGET_CTX_V` 3→4 (additive, `ctx.theme`) in all three mirrors together
(host `federationWidget.ts`, devkit template, extension copies). DOM widgets re-theme for free via the
cascade. The core never names an extension — every widget gets the same signal (rule 10).

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
`cargo fmt`, `tsc`, and `eslint` (0 errors on new files). The theme-appearance widening added the look
resolver, tone-derivation/migration, DOM-axis, motion-gate, `ctx.theme` v4 fan-out, and AA-per-look
coverage — `pnpm test` now **532** green; the gateway theme suite **6/6** on a real node; `eslint` at the
pre-existing 8-error baseline (none in touched files).

## Make collaboration real (shipped)

The UI is no longer a single-screen demo on fakes. A **real login→token→principal session** (the
gateway mints + verifies a signed `lb_auth` token per request; the demo principal is gone), a
**workspace switcher**, a **channel registry** (list / create / create-on-post), **members/teams**,
**rendered presence**, the **real `lb_inbox` queue** (Approve/Reject = the S6 gate as a UI action),
and a **read-only outbox status** view. The workspace is the token's hard wall, so the two-session
isolation test is finally real. See `frontend/collaboration.md`.

## The agent dock (shipped)

A persistent, resizable, **non-modal** AI panel docked to the right of every authenticated page. It is
**shell-mounted** (`RoutedShell.tsx`, beside `<Outlet/>`), so it survives navigation — the page reflows
narrower, the run keeps streaming, the user keeps working. Open it from the **StatusBar launcher** (with
a run-in-progress pip) or the global **`mod+j`** (one shell keydown listener; `Escape` closes and returns
focus to the launcher; auto-closes below the mobile width floor). Built on `@nube/panel`'s **non-modal
primitives** (`useResizable` + `ResizeHandle`) — not its modal `Panel`/`Sheet`. Feature: `features/agent-dock/`.

It is a **thin client** over three shipped pieces — it adds **no** persistence, transport, or agent
plumbing:
- **Storage + history = channels.** Each dock session is an ordinary channel with a reserved id
  `dock-{user-slug}-{ulid}` (created on first post; the `-` separator keeps the id one capability segment
  so the member's `bus:chan/*:pub` grant covers it). History is `channel.history`; live items are the
  channel SSE. "New session" mints a fresh ulid (old sessions stay reopenable); the picker lists the
  user's own `dock-` sessions; the channels surface filters `dock-*` OUT.
- **The answer = the durable channel agent worker**, which resolves the workspace's **active** agent and
  posts `agent_result`/`agent_error` back. Switching the active agent in Settings changes the dock's brain
  with zero dock code.
- **Progress = the run-event SSE stream**, folded into **six honest states** — Sent → Working (live
  activity + elapsed timer) → Answering (text deltas) → Stalled (15 s no-delta hint, *not* an error) →
  Done (the durable `agent_result` is the message of record) → Error (with retry). Never a bare spinner.
  If the caller lacks `mcp:agent.watch:call` the dock **degrades honestly**: no live deltas, a notice, and
  the durable answer still renders.
- **Run controls = stop / pause / resume.** While a run is in flight the dock shows **Pause** + **Stop**;
  a paused run shows **Resume**. These ride ONE new cap `mcp:agent.control:call` (member-level, distinct
  from `agent.watch`) over `POST /runs/{job}/{op}` — a thin, authorized front door onto the shipped
  run-job lifecycle (`lb_jobs`), no new table: **stop** = `cancel` (terminal; the worker posts an honest
  `run stopped`); **pause** = `suspend` (the loop honors it at its next turn boundary via a new
  `is_paused` check, emits `RunFinish(Suspended)`, keeps the transcript/cursor; the worker posts
  nothing); **resume** = `unsuspend` + re-arm the channel enqueue job so the reactor re-drives from the
  cursor under the original asker's authority.

**Page context.** Each message captures where the user is — `{ surface, path, search }`, tenant-stripped,
derived from the router by a shell `PageContextProvider` (with an override `source` seam for later
features). The host fences it into the run's goal as **untrusted, client-reported context** (a labelled
block, **4 KB** cap that *rejects* oversize, absent ⇒ byte-identical), on the ONE seam both agent doors
reach (`invoke_via_runtime`) — so the channel `kind:"agent"` payload and `POST /agent/invoke` fence it
identically. This is the **only** host change: an additive optional `context` field on the agent item
payload + `InvokeRequest` — **no new verb, cap, or table**; the host never knows the `dock-` prefix (the
wall is caps, not the name). See `scope/frontend/agent-dock-scope.md`.

## Not yet built

The full operational shell (dashboard / extensions / settings, the rest of the P0 plan in
`scope/frontend/`); token-on-the-bus for a routed cross-node caller; a real IdP behind the `verify`
seam (the credential check is a dev-login today); the Tauri **desktop** command layer's session (the
collaboration slice wired the browser/gateway path; `src-tauri/src/state.rs` still fixes its
workspace); the native window packaging build.
