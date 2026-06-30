# Armed banner read the dormant `flow.cron` → wrong "running?" after restart; no Stop for a headless flow

- **Date:** 2026-06-30
- **Area:** flows (UI / runtime control)
- **Status:** resolved

## Symptom

Three reports against the Node-RED-style canvas, all about *runtime visibility/control*:

1. **"Can't see the Stop button."** A cron/source ("headless") flow never shows a Stop button. The
   existing Stop (`Suspend`/`Resume`/`Cancel`) renders only while a **live run snapshot** is non-terminal
   (`runActive = !!snapshot && !isTerminalSnapshot(snapshot)`). A headless flow's runs are each *finite*
   (one firing → one terminal run), so between firings there is no live run to cancel — `runActive` is
   always `false` and no Stop ever appears. There was *no* control to stop the flow *itself*.
2. **"Show whether the flow is running after a server restart + page reload"** — not just right after the
   user clicks Run.
3. The armed banner could show a cron flow as **idle** after a restart.

## Root cause

The armed/running indicator (`armedState.ts → deriveArmedState`) derived its truth from the **persisted
flow record's** `flow.cron` / `flow.next_attempt_ts` fields. Those went **dormant** when triggers moved
to per-node cursors (`flow-multi-trigger-reactive-scope`): the reactor now scans **trigger nodes** and
keeps each schedule in `flow_trigger_state:{flow}:{node}`; nothing writes the flow-level `cron`/
`next_attempt_ts` anymore (the prior session left them in place with "no reader" to keep blast radius
down). So on reload `flow.cron` was empty → `isScheduled` was false → an armed cron flow rendered as
**idle**.

The authoritative, durable runtime view already existed — `flows.node_state` returns `enabled` plus the
soonest `cron`/`nextAttemptTs` computed from the live per-trigger cursors — but the UI banner wasn't
reading it (the canvas fetched `node_state` only to paint per-node values). And there was no UI binding to
`flows.enable`, the durable lifecycle flag that is the real "is this flow running headless" switch.

## Fix (UI only — the backend contract was already complete)

- **`armedState.ts`**: `deriveArmedState(flow, runs, nodeState?)` now takes the `flows.node_state`
  response as the authoritative source for `enabled` / `cron` / `nextAttemptTs` (falling back to the flow
  record only until it loads). `isScheduled` is derived from the **graph** (any `trigger` node whose
  `mode` ∈ {`cron`,`event`,`boot`}) so it stays correct even when the flow is disabled (a disabled cron
  flow is still "scheduled", just stopped). This makes the banner correct on reload with no run in flight.
- **`FlowArmedBanner.tsx`**: added a durable **Deploy/Stop** toggle (`onToggle`) — `Stop` (disarm) when
  armed, `Deploy` (re-arm) when disabled. This is the Stop the user couldn't find for a headless flow,
  and because it flips the **durable** `enabled` flag the stopped/running state survives a restart.
- **`FlowCanvas.tsx`**: passes `nodeState` into `deriveArmedState`; `handleToggleEnabled` calls
  `flows.enable {enabled: !current}` then re-reads `node_state` + runs so the banner flips immediately.

The existing per-run Suspend/Resume/Cancel is unchanged — it remains the control for an in-flight
**manual** run; the new toggle governs the **headless** lifecycle.

## Why this shape (rejected alternatives)

- *Make the backend re-populate `flow.cron`*: rejected — it re-introduces the single-flow-schedule notion
  the multi-trigger slice tore out. `node_state` is already the right per-trigger summary.
- *Show Stop based on the runs list*: a headless flow has no in-flight run between firings, so this still
  shows nothing. The flow's durable `enabled` flag is the correct "running?" truth.

## Regression tests

- `armedState.test.ts` — rewritten to the new model: scheduled is graph-derived (cron/event/boot) and
  holds when disabled; armed fields come from `node_state` (incl. the explicit "after restart, no live
  run, still ARMED" case); `enabled:false` → disabled; flow-record fallback before `node_state` loads.
- `FlowArmedBanner.test.tsx` (new) — armed → Stop fires `onToggle`; disabled → Deploy fires `onToggle`;
  idle renders no banner; omitting `onToggle` hides the control.

`pnpm test` green (203). Backend untouched (`flows.enable` / `flows.node_state` already shipped + tested
in `flows_runtime_control_test` / `flows_multi_trigger_test`).
