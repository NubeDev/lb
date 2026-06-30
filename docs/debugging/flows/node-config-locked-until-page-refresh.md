# Editing a flow node was locked read-only until you refreshed the page

- **Date:** 2026-07-01
- **Area:** flows (canvas UI — the executed-node editor lock)
- **Status:** resolved

## Symptom

User on the live canvas: "when the flow is running the user needs to stop the flow to edit a node
but it's buggy — you have to refresh the page to be able to edit the node, so it's really annoying."

After hitting **Stop**, the selected node's config panel stayed read-only ("This node has executed in
the active run — it is read-only…"), with every field disabled. Only a full page reload let you edit.
Same dead state appeared on an armed cron flow *between* firings — the canvas showed the last finite
run's nodes as locked even though nothing was running.

## Root cause

The editor lock derived from the snapshot's **presence**, not from whether a run was actually in
flight. In `FlowCanvas.tsx`:

```ts
const locked = snapshot ? executedNodeIds(snapshot) : new Set();   // bug: locks on ANY snapshot
```

Two paths left a **terminal** snapshot sitting in state, so `locked` stayed populated:

1. **Stop/cancel** (`handleLifecycle("cancel")`) called `cancelFlow(runId)` on the host but never
   updated the local snapshot. It relied entirely on the SSE `run-finished` frame to fold `status:
   "cancelled"` in — but if the stream had already closed (the run was terminal, or reattach latched
   onto a finished run), that frame never arrived. The snapshot kept its executed steps → nodes stayed
   locked until a reload re-fetched state.
2. **Armed cron flows.** `reattach` deliberately watches the latest run (`all[0]`) to paint its values,
   even when that run is *finished* (finite firings, persistent-runtime-view design). So a terminal
   snapshot is the normal steady state — and it was locking every executed node between firings.

The lock message and the `canSave = !locked` gate both keyed off this, so Save node / Save flow were
disabled too. A page refresh "fixed" it only because the fresh load had no live run to lock against.

## Fix (UI only)

The lock must reflect a genuinely **in-flight** run, and Stop must release it immediately.

- New pure `lockedNodeIds(snap)` in `flowGraph.ts`: returns `executedNodeIds` **only** when the
  snapshot is non-terminal; a terminal (or null) snapshot locks nothing. The canvas now derives
  `locked` from it. A finished manual run and an armed cron flow's latest firing both correctly leave
  the graph editable.
- New `isTerminalStatus(status)` in `flowGraph.ts` — the single source of "settled" truth, now shared
  by the canvas (`runActive`), the hook's poll-stop, and the lock (deduped two copies).
- `useFlowRun` exposes `markTerminal(status)`. The canvas's cancel path calls `markTerminal("cancelled")`
  right after `cancelFlow` succeeds, so the lock releases instantly without waiting on an SSE frame
  that may never come. The host is re-read as the source of truth on the next open/refresh.

## Regression test (unit — pure derivation)

`ui/src/features/flows/flowGraph.test.ts` (`lockedNodeIds` suite):
- locks executed nodes while `status:"running"`;
- locks **nothing** for each of `success`/`partialFailure`/`failed`/`cancelled` (the Stop / between-
  firings case — fails before the fix, which returned `{a}`);
- locks nothing with a null snapshot;
- `isTerminalStatus` classifies the four settled statuses, not `running`.

All 9 `flowGraph` tests + the 33 flows unit tests pass; my files typecheck clean (two pre-existing
`FlowsCanvas.gateway.test.ts` errors are unrelated and untouched).

> Note: UI-only change. The running dev node needs no rebuild; the Vite dev server hot-reloads it.
