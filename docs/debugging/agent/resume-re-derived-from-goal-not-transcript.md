# Resume re-derived the conversation from the goal, not the transcript

- Area: agent
- Date: 2026-06-28
- Status: resolved
- Session: ../../sessions/agent-run/agent-run-session.md
- Scope: ../../scope/agent-run/agent-run-scope.md (Part 0)

## Symptom
`run_session` resume rebuilt its message list from **only the goal** (`run.rs:68`) and started
`prior` empty, and a job step was an opaque `String` (`jobs/src/model.rs:33`). So "resume at the
cursor" re-asked the model from scratch: fine for a one-shot stateless answer, **silently wrong** the
moment a run can pause mid-conversation for a human decision (the whole point of agent-run Parts 2/4).
A resumed run would lose every prior assistant turn, tool result, and activated skill.

## Root cause
The durable record was not rich enough to *replay* the loop. The opaque-`String` step could not carry
the assistant turns, the proposed calls **with their args**, the tool results, or the active skills —
the exact state the next model turn needs. Resume therefore had nothing to rehydrate from and fell
back to the goal.

## Fix
Make the transcript **typed and replayable** (Part 0):
- Replace `Step.result: String` with `Step.event: TranscriptEvent` (a `#[non_exhaustive]`,
  `#[serde(tag="kind")]` enum), and add a `schema_version` on the job (versioned from day one).
- Add `rehydrate()` (`crates/host/src/agent/rehydrate.rs`): fold the durable transcript back into the
  exact `messages` + `prior` + `active_skills` the live loop held. `run_session` now calls it on every
  load — a fresh run yields just the system+goal seed (old behavior); a resume reconstructs the full
  conversation and **continues** it.
- The same fold is the event-sourced basis for the `RunEvent` projection (Part 1), so live and replay
  can never diverge.

## Regression test
`crates/host/tests/agent_rehydrate_test.rs`:
- `rehydrate_reconstructs_messages_prior_and_active_skills` (unit — the fold).
- `a_reloaded_run_continues_the_conversation_instead_of_re_asking` (integration — a seeded one-turn
  transcript, reloaded, runs a SECOND turn that sees the prior result; the pre-disconnect turn is NOT
  re-run). This fails against the old goal-only resume and passes against the rehydrating one.

## Prevention
The transcript is the single durable record; `rehydrate` and `lb_run_events::project` are both pure
folds of it. Any new loop state that must survive resume is added as a `TranscriptEvent` variant
(additive, `#[non_exhaustive]`) and folded in both places — never reconstructed from the goal.
