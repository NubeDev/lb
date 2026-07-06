# Run "finished" after 16 tool calls but the answer was only the turn-1 preamble

**Area:** agent (run loop)
**Date:** 2026-07-06
**Symptom:** In the dock (data-analyst persona, GLM runtime), "give me a list of 5 timeseries
queries…" ran a healthy 9-turn session — `datasource.list`, 8× `federation.schema`, 6×
`federation.query`, all ✓ — then settled `Done` with the answer
*"I'll help you create 5 timeseries queries with joins and aggregations. First, let me explore…"*.
The user never got the queries. Not a ceiling exit (turn 9 of 16); the job record
(`run-mr955s7u-9oh964jo`, 40 transcript events) shows the final assistant turn carried
**empty content and no tool calls**.

## Root cause

Cause #1 from [run-answer-empty-last-turn-content-overwrites.md](run-answer-empty-last-turn-content-overwrites.md)
in its remaining form: GLM ended the run with a bare stop (its whole utterance was a `<think>`
block, which `strip_think` correctly reduces to `""`). The earlier fix stopped the empty turn from
*wiping* the answer, but the loop still took the empty `done` at face value — so the "last
non-empty content" fallback returned the only text the model ever produced: the turn-1 preamble.
A preamble is indistinguishable from an answer by emptiness checks alone; the model has to be made
to speak.

## Fix

`run.rs`: a `done` turn with **empty content after tool work** (`state.prior` non-empty) no longer
finishes the run. The loop nudges exactly once — a live-context user message
`[you stopped without an answer — write your final answer …]` — and re-asks (one-shot `nudged`
flag; a second empty stop finishes via the existing fallback, and the ceiling still bounds the
whole run). The nudge is not persisted to the transcript (same contract as the skill-catalog
injection), so resume replays cleanly.

Rejected alternative: detecting "preamble-shaped" answers heuristically — unfixable in general;
the nudge asks the model itself, which is the only party that knows the answer.

## Regression tests

`rust/crates/host/tests/agent_answer_fallback_test.rs` —
`a_bare_stop_after_tool_work_is_nudged_for_the_real_answer` (nudge produces the real answer), and
`an_empty_final_turn_does_not_wipe_the_answer` now scripts two empty stops (nudge is one-shot; the
fallback still holds).

## Lesson

An empty `done` after a run full of tool work is a *symptom*, not a finish. Fallbacks that reuse
earlier text can only ever return what the model already said — when that is a preamble, the run
must go back to the model, not settle.
