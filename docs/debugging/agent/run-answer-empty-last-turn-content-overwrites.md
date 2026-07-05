# Agent answered `_(empty)_` — the loop's answer is the LAST turn's content, even when empty

**Area:** agent (run loop) + agent-dock UI
**Date:** 2026-07-05
**Symptom:** In the dock (widget-builder persona), "can you add a widget for me…" produced an
`agent_result` whose answer was the empty string — the dock rendered `_(empty)_`. The same session's
previous ask had answered richly. A second, related complaint: the user could not see WHICH tools the
run called — the dock showed only a transient "calling X…" line and dropped even that once the
durable answer landed.

## Root cause

`run.rs` set `state.last_content = turn.content` **unconditionally every turn**, and returned it as
the run's answer. Two common shapes wipe it to `""`:

1. a final `done` turn with empty content (models often return a bare stop after tool work; GLM
   after `<think>`-stripping especially), overwriting real text from an earlier turn;
2. a **MAX_STEPS (8-turn) ceiling exit** mid-work — a multi-step task (explore schema → query →
   save panel…) burns all 8 turns on tool calls whose turns carry no text, the loop falls out of
   `while`, marks the job `Done`, and answers with whatever the last mid-work turn carried — usually
   nothing. Silent, and indistinguishable from a broken run in the UI.

## Fix

- `run.rs`: `last_content` only updates on **non-empty** content (the answer is the last thing the
  model actually said), and a ceiling exit now appends an honest note to the answer —
  `[the run stopped at its 8-turn ceiling before the agent finished; tool effects already applied
  are saved — ask again to continue the task]` — instead of settling silently.
- Dock UI (`DockRunStatus.tsx` + `AgentDock.tsx`): the run's tool calls now render as a live list
  (✓ done / ✗ failed+reason / spinner running) and the list **stays visible after the run settles**
  (the durable channel item never carries tool calls; the live-captured feed was being discarded).
- `exportTranscript.ts`: the "Copy for AI" markdown now appends the latest run's live-captured tool
  calls with statuses (older runs' calls stay in the run-job record — noted in the header).

## Regression tests

- `rust/crates/host/tests/agent_answer_fallback_test.rs` —
  `an_empty_final_turn_does_not_wipe_the_answer`, `a_ceiling_exit_answers_with_the_honest_note`.
- `ui/src/features/agent-dock/DockRunStatus.test.tsx` — tool rows while working, list retained on
  done, nothing rendered on done with no tools.
- `ui/src/features/agent-dock/exportTranscript.test.ts` — tool section with honest statuses / omitted
  when none captured.

## Lesson

A loop's "answer = last turn's content" conflates *most recent* with *most meaningful* — keep the
last non-empty utterance. And any resource-ceiling exit must be visible in the output itself; a
silent `Done` at the ceiling reads as data loss. Follow-up worth considering: `MAX_STEPS = 8` is
tight for multi-step builder personas (schema → query → panel → pin is easily 5+ turns of tool
work); a per-workspace/persona ceiling is already noted in the scope as a follow-up.
