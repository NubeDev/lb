# Frontend (as built)

One React + TypeScript codebase. At S2 it ships a **channel view** running in a Tauri v2 shell
that talks to the in-process node over IPC. Promoted from `scope/frontend/` after the messaging
slice.

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
    channel.types.ts      ← Item (mirrors lb_inbox::Item)
  lib/ipc/
    invoke.ts             ← the single IPC seam
    fake.ts               ← in-memory node stand-in (browser/tests)
ui/src-tauri/             ← the Tauri v2 desktop shell (the node runs in-process)
```

## Cross-stack symmetry

A verb has the **same name** in the host, the shell command, and the client:
`lb_host::post` ↔ Tauri `channel_post` ↔ `channel.api.ts` `post()`. Opening any one tells you
where to look for the others.

## The IPC seam

`lib/ipc/invoke.ts` is the one place that knows how to reach the node. In the Tauri shell it calls
the Rust command via `@tauri-apps/api`; outside Tauri (a plain browser at S2, or a test) it routes
to a **faithful in-memory fake** (`fake.ts`) with the same contract — ordered history, idempotent
on id, workspace-scoped. Feature code never branches on "am I in Tauri", so the same `ChannelView`
powers the desktop shell, the browser, and the tests unchanged. The fake is dropped at S3 when the
browser talks to a real node over SSE/HTTP.

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

## Tested

Vitest `ChannelView.test.tsx` — **post a message, see it appear** (the S2 exit gate in the UI),
ordering, empty-message guard — through the real hook + api client + IPC seam. `channel.api.test.ts`
asserts the node contract over the fake. Rust `commands_test` proves the IPC path reaches the real
capability-checked node.

## Not yet built

The operational shell (dashboard / extensions / workspaces / settings, the P0 plan in
`scope/frontend/`); the SSE/HTTP browser path (S3); live push of *others'* messages and presence in
the UI; the native window packaging build.
