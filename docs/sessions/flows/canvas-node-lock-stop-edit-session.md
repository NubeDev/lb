# Session — canvas node lock: edit after Stop without a page refresh

- **Date:** 2026-07-01
- **Area:** flows (canvas UI)
- **Scope:** flow-runtime-control (the executed-node lock / `flows.patch_run` surface)

## The ask

"The UI is hard to use: when the flow is running the user has to stop the flow to edit a node, but it's
buggy — you have to refresh the page to be able to edit the node." Fix the UI.

## What I found

The editor lock (`NodeConfigPanel` `locked`/`canSave`) was derived in `FlowCanvas.tsx` from the
*presence* of a run snapshot, not from whether a run was actually in flight:

```ts
const locked = snapshot ? executedNodeIds(snapshot) : ∅;
```

Two normal situations leave a **terminal** snapshot in component state, so the lock never lifted:

1. **Stop.** `handleLifecycle("cancel")` called `cancelFlow(runId)` but never touched the local
   snapshot — it relied on the SSE `run-finished` frame to fold in `cancelled`. Once the stream has
   closed (terminal run, or `reattach` latched onto a finished run), that frame never arrives.
2. **Armed cron flows.** `reattach` intentionally watches the latest *finished* run to paint its values
   (persistent-runtime-view design), so a terminal snapshot is the steady state — and it was locking
   every executed node between firings.

A page reload "fixed" it because a fresh load has no live run to lock against.

## What I changed (UI only)

- `flowGraph.ts`: new pure `lockedNodeIds(snap)` — locks executed nodes **only** on a non-terminal
  snapshot; terminal/null locks nothing. New `isTerminalStatus(status)` as the single "settled" source,
  shared by the canvas, the hook, and the lock (deduped two inline copies).
- `useFlowRun.ts`: new `markTerminal(status)` to optimistically settle the watched snapshot.
- `FlowCanvas.tsx`: `locked = lockedNodeIds(snapshot)`; `runActive = !isTerminalStatus(snapshot.status)`;
  the cancel path calls `markTerminal("cancelled")` after `cancelFlow` so the lock releases instantly.

## Tests

`flowGraph.test.ts` — new `lockedNodeIds` suite (locks while running; locks nothing for each terminal
status — the Stop / between-firings regression, fails before the fix; locks nothing with no snapshot;
`isTerminalStatus` classification). `npx vitest run src/features/flows/` → **33 passed**. My files
typecheck clean (two pre-existing `FlowsCanvas.gateway.test.ts` errors are unrelated/untouched).

## Debug history

[flows/node-config-locked-until-page-refresh.md](../../debugging/flows/node-config-locked-until-page-refresh.md);
row added to `docs/debugging/README.md`.

## Follow-up (noted, not done)

The **Stop** button shown for an *armed cron* flow currently cancels the latched run; the durable
headless lifecycle switch is `flows.enable` (Deploy/Stop toggle, see
[armed-banner-reads-dormant-cron-no-stop-for-headless.md](../../debugging/flows/armed-banner-reads-dormant-cron-no-stop-for-headless.md)).
Worth auditing that the canvas surfaces the *enable* toggle, not run-cancel, for headless flows.
