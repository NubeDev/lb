# Phase-3 viz.query swap: dashboard gateway panels rendered empty (`viz.query` denied for the dev session)

- **Area:** frontend (gateway) / gateway dev-session caps
- **Status:** **resolved**
- **First seen:** 2026-06-29, during the dashboard-viz Phase-3 `pnpm test:gateway` run
- **Symptom:** After swapping `usePanelData`'s body to the new `viz.query` verb, ~9 dashboard
  `*.gateway.test.tsx` cases failed with `TestingLibraryElementError: Unable to find a label
  "timeseries line" / "stat value"` — the panels rendered **empty**. A handful of seeding cases ALSO
  threw `seed series failed: 500 ingest_write: Denied` / `tag: Denied`.

## Root cause

Two distinct gaps, both from the swap moving the render path onto a new gated verb:

1. **`mcp:viz.query:call` was not in the dev session's cap set.** Every panel now reads its data
   through `viz.query` (the Phase-3 backend resolver), but `member_caps()`
   (`role/gateway/src/session/credentials.rs`) — the cap set `/login` mints for a dev session — had
   no `mcp:viz.query:call`. So `useVizQuery`'s bridge call was denied → honest empty rows → no chart
   to find. (This is the deny path working correctly; the dev session simply lacked the new cap.)
2. **A handful of gateway tests seeded without the seed-route caps.** The `/_seed/series` route writes
   through `ingest_write` → `drain` → `tags_add`; a test using `signInWithCaps([...])` that omitted
   `mcp:ingest.write:call` / `mcp:tags.add:call` got `500 ... Denied` from the route (surfaced clearly
   once the route's error mapping was made `{e:?}` instead of the opaque `e.to_string()=="denied"`).

The host write path itself was never at fault — a headless repro of `ingest_write → drain → tags_add`
under member caps succeeded. The failures were purely the dev-session cap set + a couple of
under-granted test sign-ins.

## Fix

- Added `"mcp:viz.query:call"` to `member_caps()` (the dashboard cap group) — `viz.query` is the
  member-level render path every panel uses; it still dispatches each target under `caller ∩ grant`
  (composing the target tool's own cap), so a token can't read a target it lacks.
- Granted the seed-route caps (`mcp:ingest.write:call`, `mcp:tags.add:call`) in the Phase-3 gateway
  test's seeding sign-in.
- Made `/_seed/series`'s `ingest_write`/`drain` error mapping verbose (`{e:?}`) so a future seed denial
  names the failing call instead of an opaque `denied`.

## Regression coverage

- Headless: `crates/host/tests/viz_query_test.rs` proves `viz.query`'s deny-without-cap + the
  target-deny-is-honest-empty path (the same caps story, asserted on the real verb).
- Gateway: `viz.phase3.gateway.test.tsx` now renders a seeded panel through `viz.query` identically to
  Phase 2 (green), and `viz.query`-deny stays a denied state.

## Note — the one remaining full-run failure is NOT this

`SystemView.gateway.test.tsx > opens the subsystem detail sheet` fails only in a FULL `test:gateway`
run and passes **9/9 isolated** — the pre-existing SystemView sheet flake the Phase-3 brief named, not
a dashboard/viz regression.
