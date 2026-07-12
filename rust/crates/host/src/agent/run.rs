//! The tool-call **loop** — the agent itself (agent scope: "the agent owns the loop; the gateway
//! does model access only"). This is where the slice's behavior lives, so it is the one file worth
//! reading top to bottom.
//!
//! The loop, bounded by [`MAX_STEPS`] (no runaway / budget burn — the ceiling is the agent's, not
//! the gateway's):
//!   1. ask the model for a turn (`ModelAccess::turn`) — replay-safe by a per-step idempotency key;
//!   2. for each proposed tool call, run it through `lb_mcp::call` under the **derived** principal
//!      (capability-checked, workspace-first, routed if remote). A denial is fed back as a tool
//!      error, NOT a crash — the model can react;
//!   3. append the turn's typed events to the durable transcript (idempotent, append-addressed) and
//!      advance the cursor;
//!   4. repeat until the model is `done`, the ceiling is hit, or the run is cancelled; then set the
//!      terminal status.
//!
//! **Resume is now faithful (agent-run scope Part 0).** On load the loop *rehydrates* its exact
//! working state — `messages`, the previous turn's `prior` outcomes, active skills — by folding the
//! durable transcript (`rehydrate.rs`), instead of re-deriving from the goal alone (the old
//! `run.rs:68` behavior, which re-asked the model from scratch). Events already persisted are NOT
//! re-emitted (their model turn is cached by the gateway's idempotency key anyway), so a session
//! that survived an edge disconnect continues without double-applying or re-spending.

use std::sync::Arc;

use lb_auth::Principal;
use lb_jobs::{cancel, complete, create, load, Job, JobStatus, TranscriptEvent};

use super::activate::intercept_activations;
use super::attempt::{attempt_turn, fail_run, CompactState};
use super::compact::{estimate_tool_tokens, DEFAULT_COMPACT_BUDGET_TOKENS};
use super::decision::{open_suspension, resume_suspensions};
use super::error::AgentError;
use super::model_access::{AllowedTool, ModelAccess};
use super::partition::partition_by_policy;
use super::policy::load_policy;
use super::rehydrate::{rehydrate, summarize};
use super::seed_context::inject_context;
use super::step::{count_turns, is_cancelled, is_paused, run_calls};
use super::transcript::TranscriptWriter;
use crate::boot::Node;
use crate::run_events::publish_run_event;
use lb_run_events::{RunEvent, RunOutcome};

/// The loop ceiling, counted in **model turns**. A fixed default at S5 (a per-workspace policy is a
/// scope follow-up). The transcript may hold more than `MAX_STEPS` events (several per turn).
/// 16, not 8: a builder persona's honest path (discover datasource → federation.schema → probe
/// queries → viz.query → dashboard.save) measured 10–14 turns live; at 8 every such run died at the
/// ceiling mid-work (`docs/debugging/agent/run-ceiling-too-low-for-builder-personas.md`).
pub const MAX_STEPS: u32 = 16;

/// The system prompt seeding every run's conversation. One place owns it so rehydration and the
/// live loop seed the identical first message (the fold must reproduce the live message list).
pub const SYSTEM_PROMPT: &str = "You are a workspace agent.";

/// The derived-actor sub prefix — audit shows `agent:{skill-or-goal}` acted on the caller's behalf.
const AGENT_SUB: &str = "agent:session";

/// Run (or resume) an agent session to completion. `agent_caps` are the agent's own capabilities;
/// the effective grant is `agent_caps ∩ caller.caps` via the derived principal (no widening). The
/// session is the durable job `job_id`; on resume it **rehydrates** from the persisted transcript
/// and continues the conversation (Part 0), rather than re-asking from the goal.
///
/// `tools` are the qualified MCP tool names the model may propose. The loop returns the final
/// model content (the session's answer). Errors only on a gate refusal at the surface or a store
/// failure — a tool denial *inside* the loop is fed to the model, not surfaced as an error.
#[allow(clippy::too_many_arguments)]
pub async fn run_session<M: ModelAccess>(
    node: &Arc<Node>,
    model: &M,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    job_id: &str,
    goal: &str,
    tools: &[AllowedTool],
    persona_catalog: Option<&[String]>,
    persona_preset: Option<&super::personas::PolicyPreset>,
    ts: u64,
) -> Result<String, AgentError> {
    // The derived (intersected) principal: the agent acts under `agent_caps ∩ caller.caps`, same ws,
    // under a distinct `agent:*` sub (audit shows the agent acted). It inherits exactly what BOTH
    // sides allow — never more (agent scope no-widening).
    let agent = caller.derive(AGENT_SUB, agent_caps.to_vec());

    // Create the session if new; resume the existing record otherwise (idempotent on job_id).
    let job = match load(&node.store, ws, job_id).await? {
        Some(existing) => existing,
        None => {
            let job = Job::new(job_id, "agent-session", goal, ts);
            create(&node.store, ws, &job).await?;
            job
        }
    };

    // A genuinely-terminal run is not re-entered (cancelled/done/failed) — return its answer so far.
    if !job.status.is_resumable() {
        let state = rehydrate(SYSTEM_PROMPT, goal, &job.events().collect::<Vec<_>>());
        return Ok(state.last_content);
    }

    // The durable cursor + turn counter, owned by the ONE transcript-write chokepoint (slice C):
    // every append below goes through `writer`, which also tracks proposed-but-unresolved calls.
    let events: Vec<&TranscriptEvent> = job.events().collect();
    let mut turn_no = count_turns(&events);
    let mut writer = TranscriptWriter::new(node, ws, job_id, job.cursor, turn_no);

    // LOAD-TIME HEAL (slice C's sanitizer): orphaned proposals from a segment that died mid-turn
    // resolve as `ToolCancelled`, appended at the cursor (never renumbered), then fold into the
    // rehydrated view below.
    let healed = writer.heal_orphans(&events).await?;

    // Rehydrate the EXACT working state from the durable transcript — the Part-0 fix. On a fresh run
    // the transcript is empty and this yields just the system + goal seed (the old starting point);
    // on resume it reconstructs `messages` + `prior` + active skills so the model continues, not
    // re-asks.
    let all_events: Vec<&TranscriptEvent> =
        events.iter().copied().chain(healed.iter()).collect();
    let mut state = rehydrate(SYSTEM_PROMPT, goal, &all_events);
    let events = all_events;

    // The workspace permission policy consulted in front of `caps::check` (Part 2). Loaded once per
    // run; default-allow when absent, so a workspace with no policy behaves exactly as before. The
    // active persona's `policy_preset` (persona-coding #4) is applied per-call below as a FLOOR clamp
    // over the evaluated effect (see `clamp_to_preset` — a clamp, not a merged rule, because an Ask
    // floor can't beat a blanket Allow under the evaluator's Deny>Allow>Ask precedence).
    let policy = load_policy(&node.store, ws).await?;

    // CONTEXT COMPACTION (slice A): the run's budget — the workspace's `agent.config.compact_budget`
    // when set, else the node default — plus the tool-schema cost (rides every request) and the
    // cumulative dropped-group counter the breadcrumb reports. Read once per run, like the policy.
    let mut compact_state = CompactState {
        budget_tokens: super::config::get_agent_config(&node.store, ws)
            .await
            .ok()
            .flatten()
            .and_then(|c| c.compact_budget)
            .unwrap_or(DEFAULT_COMPACT_BUDGET_TOKENS),
        tool_tokens: estimate_tool_tokens(tools),
        dropped: 0,
    };

    // Seed the live context (never persisted; re-injected per segment): the granted-skills catalog
    // (Part 5, persona-filtered), the memory index (on-behalf-of read), and — on resume — the
    // bodies of previously-activated skills. See `seed_context.rs` for the full rationale.
    inject_context(node, &agent, caller, agent_caps, ws, persona_catalog, &mut state).await?;

    // RESUME PAST A SETTLED SUSPENSION (Part 2). A run that suspended on an Ask re-enters here with
    // an open suspension in its transcript. Apply the settled decision (Deny → denied result;
    // Allow → replay the original call), feeding the result into the live state before the model is
    // asked again. If the decision is still pending, the run is not actually resumable — leave it
    // suspended (a re-scan / premature resume is a no-op).
    if events
        .iter()
        .any(|e| matches!(e, TranscriptEvent::SuspensionOpened { .. }))
    {
        let resumed = resume_suspensions(node, &agent, &events, &mut writer).await?;
        if resumed.still_pending {
            return Ok(state.last_content);
        }
        if !resumed.outcomes.is_empty() {
            // Mirror the resolved suspension's results into the live conversation, identically to a
            // rehydrated fold, so the next model turn sees them.
            state
                .messages
                .push(("tool".into(), summarize(&resumed.outcomes)));
            state.prior = resumed.outcomes;
        }
    }

    // Did the model finish on its own (`done` / no calls)? False after the loop means the run hit
    // the MAX_STEPS ceiling mid-work — the answer must say so honestly, not read as a normal finish.
    let mut model_finished = false;

    // ONE-SHOT nudge for a bare stop: some models (GLM after think-stripping) end a tool-heavy run
    // with a `done` turn whose content is EMPTY — the fallback then answers with the last non-empty
    // text, usually the turn-1 preamble ("I'll help you…"), so the user never gets the real answer
    // (docs/debugging/agent/run-finished-empty-after-tool-work-answers-with-preamble.md). Rather
    // than settle, ask once for the final answer; a second empty stop finishes via the fallback.
    let mut nudged = false;

    while turn_no < MAX_STEPS {
        // Cancellation is a durable stop (Part 0): re-read the status before each turn so a
        // `cancel` written by a UI stop button / ACP `session/cancel` between turns is honored.
        if is_cancelled(node, ws, job_id).await? {
            return Ok(state.last_content);
        }

        // PAUSE is a durable, RESTARTABLE stop (agent-dock run controls): a `pause_run` flips the job
        // to `Suspended` between turns. Honor it at the boundary — emit a terminal `RunFinish(Suspended)`
        // so a watcher's stream ends cleanly (it resumes via a fresh watch after `resume_run`), and
        // return. The transcript + cursor are intact, so `resume_run` re-drives from exactly here. NOT
        // an error and NOT terminal like cancel — the job stays `Suspended` (resumable). Checked after
        // cancel so a run cancelled AND paused resolves as cancelled (terminal wins).
        if is_paused(node, ws, job_id).await? {
            publish_run_event(
                &node.bus,
                ws,
                job_id,
                &RunEvent::RunFinish {
                    outcome: RunOutcome::Suspended,
                    answer: state.last_content.clone(),
                },
            )
            .await;
            return Ok(state.last_content);
        }

        // Replay-safe: the gateway caches by this key, so a resumed turn does not re-spend. Keyed by
        // the TURN number (not the event index) so the same turn maps to the same cached response.
        // Slice D: a transient fault retries below step accounting inside `attempt_turn` (one turn,
        // N attempts, same key — the gateway never caches a fault). Slice A: over-budget context is
        // compacted (whole turn groups, breadcrumbed) before the call, and a provider overflow is
        // recovered by compacting harder and continuing THIS run. An unrecoverable fault ends the
        // run honestly (`Failed` + `RunFinish(Failed)`), never a fault dressed as a completion.
        let key = format!("{ws}:{job_id}:{turn_no}");
        let turn = match attempt_turn(
            model,
            ws,
            &mut state.messages,
            tools,
            &state.prior,
            &key,
            &mut compact_state,
        )
        .await
        {
            Ok(turn) => turn,
            Err(e) => return fail_run(node, ws, job_id, &state.last_content, e.detail()).await,
        };
        // Keep the LAST NON-EMPTY content as the running answer. A tool-call turn (and some models'
        // final `done` turn — GLM after think-stripping) carries empty content; overwriting here
        // wiped a real earlier answer and the run settled with an EMPTY durable result (see
        // debugging/agent/run-answer-empty-last-turn-content-overwrites.md).
        if !turn.content.is_empty() {
            state.last_content = turn.content.clone();
        }

        // Record the assistant turn durably first (the transcript is the record), then mirror it
        // into the live message list — the writer publishes the live `RunEvent` (StepStart +
        // TextDelta) for any watcher (Part 3).
        writer.turn = turn_no;
        writer
            .append(TranscriptEvent::AssistantTurn {
                content: turn.content.clone(),
            })
            .await?;
        if !turn.content.is_empty() {
            state
                .messages
                .push(("assistant".into(), turn.content.clone()));
        }

        if turn.done || turn.calls.is_empty() {
            // A bare stop (empty content, no calls) after tool work is a swallowed answer, not a
            // finish — nudge exactly once. The nudge is a live-context message only (like the skill
            // catalog, never persisted): a resume replays the same turn key and re-nudges cleanly.
            if turn.content.is_empty() && !nudged && !state.prior.is_empty() {
                nudged = true;
                state.messages.push((
                    "user".into(),
                    "[you stopped without an answer — write your final answer to the \
                     original request now, using the tool results above]"
                        .into(),
                ));
                turn_no += 1;
                continue;
            }
            model_finished = true;
            break;
        }

        // Persist each proposed call WITH ITS ARGS (needed for an Allow→replay resume, Part 2),
        // then consult the permission policy (Part 2) BEFORE dispatch — the gate that sits in front
        // of `caps::check` (defense in depth). The policy partitions this turn's calls into:
        //   - Allow → dispatch now under the DERIVED principal (still capability-checked);
        //   - Deny  → fed a "denied by policy" tool result, never dispatched;
        //   - Ask   → suspend the run durably and end the turn (resumed when a human decides).
        for c in &turn.calls {
            writer
                .append(TranscriptEvent::ToolCallProposed {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    args: c.input.clone(),
                })
                .await?;
        }

        // MODEL-ACTIVATED SKILLS (Part 5): `skill.activate` is a LOOP-INTERNAL built-in — the
        // interception (grant-gated activate + durable SkillActivated/ToolResult + body injection)
        // lives in `activate.rs::intercept_activations`; the wall holds there.
        let (mut activated_outcomes, model_calls) = intercept_activations(
            &node.store,
            &agent,
            ws,
            &turn.calls,
            &mut writer,
            &mut state.messages,
        )
        .await?;

        // Partition the remaining calls by the ws policy + persona floor (see `partition.rs`).
        let parts = partition_by_policy(model_calls, &policy, persona_preset);
        let (to_run, mut denied, ask) = (parts.to_run, parts.denied, parts.ask);

        // ASK → suspend durably and return. We open a suspension for each Ask call (each gets its own
        // `agent_decision` record + transcript `SuspensionOpened`), then end the turn — the run is now
        // `Suspended` and resumes when the decision settles. We suspend BEFORE running the Allowed
        // calls of this turn: a turn that needs a human gate on *any* call pauses as a whole, so the
        // model sees a coherent next-turn state (no half-applied turn racing a human decision).
        if !ask.is_empty() {
            for c in &ask {
                // The writer's append of `SuspensionOpened` publishes the live `Suspended` delta —
                // the stream follows the record (Part 3), one projection for live and snapshot.
                open_suspension(&mut writer, &agent, &c.id, ts).await?;
            }
            // The run is now `Suspended` — emit a terminal RunFinish(Suspended) so a watcher's stream
            // ends cleanly for this turn (it resumes via a fresh watch after the decision settles).
            publish_run_event(
                &node.bus,
                ws,
                job_id,
                &RunEvent::RunFinish {
                    outcome: RunOutcome::Suspended,
                    answer: state.last_content.clone(),
                },
            )
            .await;
            return Ok(state.last_content);
        }

        // Run the Allowed calls; combine with the policy-denied results, preserving proposal order is
        // unnecessary (the model keys on the call id), so denied-then-run is fine.
        let mut outcomes = run_calls(node, &agent, ws, &to_run).await;
        outcomes.append(&mut denied);
        for o in &outcomes {
            writer
                .append(TranscriptEvent::ToolResult {
                    id: o.id.clone(),
                    ok: o.ok.clone(),
                    err: o.error.clone(),
                })
                .await?;
        }

        // Fold the activation results (already recorded above) into the live outcomes so the next
        // turn sees them — including a turn that ONLY activated a skill (no other calls).
        outcomes.append(&mut activated_outcomes);

        // Mirror the durable results into the live conversation, identically to a rehydrated fold.
        let summary = summarize(&outcomes);
        state.messages.push(("tool".into(), summary));
        state.prior = outcomes;
        turn_no += 1;
    }

    // CEILING EXIT: the model was still proposing calls when MAX_STEPS ran out. Say so — a silent
    // `Done` with whatever text a mid-work turn happened to carry (often nothing) reads as a broken
    // or empty answer in the dock. The note is part of the answer (both runtimes' one channel).
    if !model_finished {
        let note = format!(
            "[the run stopped at its {MAX_STEPS}-turn ceiling before the agent finished; \
             tool effects already applied are saved — ask again to continue the task]"
        );
        state.last_content = if state.last_content.is_empty() {
            note
        } else {
            format!("{}\n\n{note}", state.last_content)
        };
    }

    complete(&node.store, ws, job_id, JobStatus::Done).await?;
    // The terminal motion: a watcher's stream ends with the final answer (Part 3). Best-effort —
    // the durable `Done` status is the record; `project` derives the same RunFinish on reattach.
    publish_run_event(
        &node.bus,
        ws,
        job_id,
        &RunEvent::RunFinish {
            outcome: RunOutcome::Done,
            answer: state.last_content.clone(),
        },
    )
    .await;
    Ok(state.last_content)
}

/// Cancel a run — the durable stop hook (Part 0). A UI stop button and ACP `session/cancel` both
/// reach this; the loop notices on its next turn boundary and ends with a terminal, restorable
/// transcript. Authorization (the cancel cap / session principal) is the caller's job.
pub async fn cancel_run(node: &Node, ws: &str, job_id: &str) -> Result<(), AgentError> {
    cancel(&node.store, ws, job_id).await.map_err(Into::into)
}
