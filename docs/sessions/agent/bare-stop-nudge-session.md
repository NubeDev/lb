# Session — bare-stop nudge (run answered with only the preamble)

**Date:** 2026-07-06
**Ask:** "see the last question i never got a response back" — a live dock ask (data-analyst
persona, "give me a list of 5 timeseries queries…") ran 16 successful tool calls and then answered
with only "I'll help you create 5 timeseries queries… let me explore".

## Diagnosis

- Node binary + process both fresh (not the stale-node trap).
- Read the durable job `run-mr955s7u-9oh964jo` via `POST /store/query`: 40 transcript events,
  9 model turns (ceiling is 16), every tool result ✓, final assistant turn = **empty content, no
  calls**, status `Done`.
- Cause #1 of `docs/debugging/agent/run-answer-empty-last-turn-content-overwrites.md` in its
  remaining form: GLM's last utterance was all `<think>`; strip → `""`; the loop accepted the bare
  `done` and the last-non-empty fallback returned the turn-1 preamble.

## Change

- `rust/crates/host/src/agent/run.rs`: one-shot **nudge** — a `done` turn with empty content after
  tool work (`state.prior` non-empty) pushes a live-context user message asking for the final
  answer and re-asks the model instead of finishing. Second empty stop → existing fallback.
  Nudge is not persisted (same contract as catalog injection); ceiling still bounds the run.

## Tests (green)

- `cargo test -p lb-host --test agent_answer_fallback_test` — 3 passed, including new
  `a_bare_stop_after_tool_work_is_nudged_for_the_real_answer`; the existing empty-final-turn test
  now scripts two empty stops (pins nudge-once + fallback).
- Agent loop suites re-run green: agent, skill, decision, rehydrate, isolation, watch, persona,
  persona-coding, persona-session, memory, page-context, channel-agent-worker.

## Docs

- New debugging entry: `docs/debugging/agent/run-finished-empty-after-tool-work-answers-with-preamble.md`
  (+ README history row).

## Note

The running dev node predates this fix — `make kill && make dev` to pick it up before retrying the
ask in the dock.
