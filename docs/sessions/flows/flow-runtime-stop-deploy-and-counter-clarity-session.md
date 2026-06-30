# Session — headless flow Stop/Deploy, armed-after-restart, and Count/Counter clarity

- **Date:** 2026-06-30
- **Area:** flows (UI runtime control + builtin clarity)
- **Scope refs:** flow-multi-trigger-reactive-scope, flow-persistent-runtime-scope, triggers-lifecycle

## The ask (three reports against the Node-RED-style canvas)

1. "Can't see the Stop button."
2. "Show whether the flow is running after a server restart + page reload" (not just right after Run).
3. "The count still isn't going up." (with a screenshot of a cron flow whose `count` nodes sat at 4/1/0.)

## What was actually wrong, and what I changed

### 1 + 2 — Stop button + running state after restart (real UI bug, fixed)

The armed/running banner derived from the **dormant** `flow.cron`/`flow.next_attempt_ts` fields (no
writer since triggers moved to per-node cursors), so an armed cron flow showed as **idle** after reload;
and the only Stop (per-run Suspend/Resume/Cancel) renders only for a non-terminal **live run**, which a
finite-firing headless flow never has between firings — so a cron/source flow had **no Stop at all**.

Fix (UI only — the backend `flows.enable` / `flows.node_state` contract was already complete):

- `armedState.ts` — `deriveArmedState(flow, runs, nodeState?)` now reads the AUTHORITATIVE durable
  `flows.node_state` (`enabled` + soonest cron/nextAttemptTs from the per-trigger cursors); `isScheduled`
  is graph-derived (any `trigger` with mode ∈ {cron,event,boot}) so it's right even when disabled.
- `FlowArmedBanner.tsx` — added a durable **Deploy/Stop** toggle bound to `flows.enable` (Stop=disarm,
  Deploy=re-arm). Because `enabled` is durable, the stopped/running state is correct after a restart.
- `FlowCanvas.tsx` — passes `nodeState` to `deriveArmedState`; `handleToggleEnabled` flips `enabled` then
  re-reads node_state + runs so the banner flips immediately.

Verified live: the screenshot now shows "Armed — running headless · next fire in 40s · 78 runs · Stop".

### 3 — "Count not going up" (NOT a bug — wrong node type + a naming trap)

E2E against the running gateway (`127.0.0.1:8080`, ws `acme`, `chain4`) showed `chain4` used **`count`**
nodes. `count` is a *pure transform of this firing's input* (array len / object keys / scalar→1) — it
does not accumulate, by design. The accumulator is **`counter`** (durable `flow_node_memory` + atomic
`lb_store::increment`).

Proof the engine is correct (real store, no mocks):

- A throwaway `trigger(manual) → counter(step:1)` flow ticked **1 → 2 → 3** across runs (rev 1→2→3),
  then was deleted.
- Converted `chain4`'s `count` nodes to `counter`. `count-5` (empty `with`) ticks **+1 per firing**
  (step mode); `a` (`with.items:[1,2,3,4]`) ticks **+4** and `b` (`with.items:${steps.a.output}`, a
  2-key object) ticks **+2** — the documented "throughput mode = increment by the wired input's size".
  So `step` vs throughput is selected by whether `items` is bound — both reachable, working as specced.
- **Headless proof:** with no manual run and no page interaction, at the 19:40 minute boundary the cron
  reactor fired `trigger-5` on its own and `a` climbed **20 → 24**.

Root trap: the palette showed two near-identical titles, **"Count"** and **"Counter"** (the descriptor
has no description field the palette renders). Fixed by making the titles self-describing:
`count` → **"Count (input size)"**, `counter` → **"Counter (running total)"** (`flows/src/builtins.rs`).
Type ids are unchanged, so existing flows still load. *Requires a node rebuild + restart to surface in
the live palette (`make kill && make dev`).*

## Tests

- `armedState.test.ts` rewritten to the node_state model (incl. "after restart, no live run, still
  ARMED"); `FlowArmedBanner.test.tsx` (new) covers Stop/Deploy firing `onToggle` + idle renders nothing.
  `pnpm test` green (203).
- Backend untouched except the two title strings; `cargo build -p lb-flows` clean. `counter` increment is
  already covered by `increment_test` + `flows_multi_trigger_test`.

## Debugging entry

`docs/debugging/flows/armed-banner-reads-dormant-cron-no-stop-for-headless.md` (+ README row).

## Follow-ups (not done this slice)

- Surface a node `description` in the palette (descriptor field + Palette tooltip) — the deeper fix for
  the Count/Counter confusion; titles are the stopgap.
- A per-trigger "armed" chip on each trigger node (data already in `node_state.nodes[].armed`).
