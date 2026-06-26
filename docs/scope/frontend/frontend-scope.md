# Frontend scope

Status: scope. A first **channel view** shipped at S2 (the messaging slice); the operational
shell (dashboard/extensions/workspaces/settings) below is still the P0 plan.

## What shipped in S2 (the channel view)

- `ui/` — Vite + React + TS + Tailwind (quiet control-surface tokens) + lucide, one
  component/hook per file (FILE-LAYOUT §4): `features/channel/{ChannelView,MessageList,
  MessageComposer}.tsx` + `useChannel.ts`; `lib/channel/{channel.api,channel.types}.ts`.
- The **api client mirrors the Rust verbs** `post`/`history` and the Tauri command names
  one-to-one (`channel_post`/`channel_history`) — a verb has the same name in the host, the shell
  command, and the client.
- **One IPC seam** (`lib/ipc/invoke.ts`): Tauri `invoke` in the desktop shell; an in-memory
  *faithful* node fake (`lib/ipc/fake.ts`) in the browser/tests until SSE lands (S3). Feature code
  never branches on "am I in Tauri".
- `ui/src-tauri/` — a Tauri v2 shell; the node runs **in-process** (the shell IS a node, §3.1).
  Command logic is a library (headlessly unit-tested — no webkit toolchain needed); the window is
  behind a `desktop` feature.
- Tests: Vitest `ChannelView.test.tsx` (post a message, see it appear — the S2 exit gate in the
  UI) + `channel.api.test.ts`; Rust `commands_test` (the IPC path through the real node).

## Reference (P0 shell — not yet built)

Status: draft scope.

Build a simple first UI for the Lazybones platform using the existing
`/home/user/code/rust/lazybones/ui` app as the visual reference. Keep the first
pass small: an operational shell, a dashboard, and a few static/detail surfaces
that prove the design system before the backend is fully wired.

## Reference

- Source look: `/home/user/code/rust/lazybones/ui`
- Start with:
  - `src/styles/globals.css`
  - `src/components/layout/sidebar.tsx`
  - `src/components/layout/topbar.tsx`
  - `src/components/ui/`
  - `src/features/dashboard/`
- If a clean copy is useful, copy or clone the reference into `/tmp` first, then
  port only the needed tokens, layout ideas, and component patterns into this
  repo. Do not edit the reference app while building this UI.

## P0 screens

- **Shell:** compact left rail, top bar, dark/light theme support, workspace
  switcher placeholder, connection status, and settings entry.
- **Dashboard:** node role, store, bus, extension runtime, MCP, sync, jobs, and
  capability health at a glance.
- **Extensions:** simple installed/available list with capability badges and
  placement labels.
- **Workspaces:** list of workspaces plus member/team/channel counts.
- **Settings:** API endpoint, node role, profile, and local/cloud connection
  placeholders.

## Visual direction

Use the same quiet control-surface feel as the reference app:

- near-black dark mode and warm paper light mode
- one warm amber accent
- hairline borders, low-contrast panels, and compact spacing
- lucide icons in icon buttons and navigation
- shadcn-style primitives over Tailwind tokens
- dense dashboard cards, not a marketing landing page

Avoid broad restyling, decorative gradients, oversized hero sections, and new
palette experiments. The first UI should feel like the current Lazybones app has
been adapted to the reusable core-stack product.

## Implementation notes

- Prefer React, TypeScript, Tailwind, and the same component shape used by the
  reference app unless the local repo already establishes something else.
- Copy component ideas selectively instead of importing the whole UI.
- Keep mock data local and obvious until real endpoints exist.
- Keep the navigation small; add deeper pages only when the backend surface
  exists.
- Extension UI slots can be placeholders in P0, but the shell should leave room
  for them.

## What shipped in S3 (the real browser transport)

- **IPC vs SSE — resolved, the one-file-change promise held.** `lib/ipc/invoke.ts` now picks Tauri
  IPC (shell) → real HTTP (`http.ts`, when `VITE_GATEWAY_URL` is set) → the fake (tests). The fake
  is kept ONLY for tests; the browser hits a real node. `ChannelView`/`channel.api`/verb names
  unchanged. The new files are `lib/ipc/http.ts` and `lib/channel/channel.stream.ts`.
- **Live updates — shipped.** `channel.stream.ts` opens the gateway's SSE stream; `useChannel`
  folds OTHERS' `message` events into its existing `setItems` sink (idempotent merge by id). The
  event shape is the gateway's: `event: message` (an `Item`) and `event: presence`
  (`{member, present}`).

## Open questions

- **Presence in the UI:** the SSE `presence` event is delivered but not yet rendered — wire it to a
  presence indicator (a small `usePresence` hook beside `useChannel`).
- **Per-workspace gateway routing:** the gateway fixes the workspace to its session principal, so
  the UI's `ws` arg is plumbed but unused on the stream/HTTP path. A real login→token→principal
  session (replacing the demo principal) makes the workspace dynamic.
- **The native window:** building it needs the webkit2gtk toolchain (absent in CI). The windowed
  `tauri build` is a packaging step; the command layer is tested headlessly. Decide where the
  desktop packaging build runs.
- shadcn/ui primitives: S2 used hand-rolled Tailwind controls; pull in the shadcn generator when
  the component set grows past a couple of inputs/buttons.
- **Localization (client half).** `scope/prefs/user-prefs-scope.md` owns the preference/format contract;
  the **client responsibilities are the frontend's to build**: bootstrap-locale resolution (pre-auth),
  catalog bundling + workspace-override fetch/merge, the prefs settings surface (IANA tz picker + the
  closed unit-override enum, shared with the server), re-render on the "prefs changed" hint, and **RTL
  layout** (`dir=rtl`) when a non-LTR language is added. See that scope's "How the UI handles this".
