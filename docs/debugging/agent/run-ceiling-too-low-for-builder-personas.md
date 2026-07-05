# Builder-persona runs always died at the 8-turn ceiling — MAX_STEPS raised to 16

**Area:** agent (run loop)
**Date:** 2026-07-05
**Symptom:** After the answer-fallback fix made ceiling exits honest, *every* live widget-builder
run ("add a widget for avg meter usage per site", GLM-4.6) ended with
`[the run stopped at its 8-turn ceiling …]` — even a follow-up run pointed straight at the
datasource. The runs were healthy (correct discovery via `federation.schema`, no guessed tables);
they simply ran out of turns.

## Root cause

`MAX_STEPS = 8` was sized before builder personas existed. The honest path for a builder task —
orient (`dashboard.catalog`/`store.schema`) → find the datasource → `federation.schema` → probe
queries (a join usually needs a retry after a real column-name error) → `viz.query` →
`dashboard.save` — measured 10–14 model turns live. Eight turns is enough for a read-and-answer
persona, never for a build.

## Fix

`MAX_STEPS` raised 8 → 16 in `rust/crates/host/src/agent/run.rs` (comment records the live
measurement). The honest-ceiling note and answer fallback (see
[run-answer-empty-last-turn-content-overwrites.md](run-answer-empty-last-turn-content-overwrites.md))
stay as the backstop.

The real fix remains the recorded scope follow-up: a **per-workspace / per-persona ceiling**
(builder personas need more turns than Q&A personas; a fixed global constant will always be wrong
for someone).

## Regression tests

`agent_answer_fallback_test.rs` derives its script length from the `MAX_STEPS` constant, so the
ceiling-note behavior is pinned regardless of the value.
