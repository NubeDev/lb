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

use super::activate::{activate_skill, SKILL_ACTIVATE};
use super::catalog::render_catalog_filtered;
use super::decision::{open_suspension, resume_suspensions, DENIED_BY_POLICY};
use super::error::AgentError;
use super::memory::memory_index_for_injection;
use super::model_access::{AllowedTool, CallOutcome, ModelAccess, ProposedCall};
use super::policy::{evaluate, load_policy, Effect};
use super::rehydrate::{rehydrate, summarize};
use super::step::{count_turns, emit, is_cancelled, is_paused, run_calls};
use crate::assets::load_skill;
use crate::boot::Node;
use crate::run_events::publish_run_event;
use lb_run_events::{RunEvent, RunOutcome};

/// The loop ceiling, counted in **model turns**. A fixed default at S5 (a per-workspace policy is a
/// scope follow-up). The transcript may hold more than `MAX_STEPS` events (several per turn).
pub const MAX_STEPS: u32 = 8;

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

    // Rehydrate the EXACT working state from the durable transcript — the Part-0 fix. On a fresh run
    // the transcript is empty and this yields just the system + goal seed (the old starting point);
    // on resume it reconstructs `messages` + `prior` + active skills so the model continues, not
    // re-asks.
    let events: Vec<&TranscriptEvent> = job.events().collect();
    let mut state = rehydrate(SYSTEM_PROMPT, goal, &events);

    // The next transcript slot to append at — also the count of recorded events (the durable
    // cursor). Turns are counted separately, from the assistant-turn events already recorded.
    let mut index = job.cursor;
    let mut turn_no = count_turns(&events);

    // The workspace permission policy consulted in front of `caps::check` (Part 2). Loaded once per
    // run; default-allow when absent, so a workspace with no policy behaves exactly as before. The
    // active persona's `policy_preset` (persona-coding #4) is applied per-call below as a FLOOR clamp
    // over the evaluated effect (see `clamp_to_preset` — a clamp, not a merged rule, because an Ask
    // floor can't beat a blanket Allow under the evaluator's Deny>Allow>Ask precedence).
    let policy = load_policy(&node.store, ws).await?;

    // MODEL-ACTIVATED SKILLS (Part 5): inject the granted-skills CATALOG (title+description only)
    // into context ONCE per run, so the model knows what it may `skill.activate`. Resolved decision:
    // render once + cache, re-inject only on change — here we render once at loop start and inject
    // once (it is context framing, never persisted to the transcript, so each run/resume re-injects
    // it cleanly without a rehydrate double-up). FOLLOW-UP: a re-inject-on-change hook (re-render
    // when the granted set changes mid-run, rare) is deferred — the once-per-run render is the
    // load-bearing half of the decision. The read is grant- + ws-gated (`render_catalog`); an
    // ungranted skill never reaches the rendered text.
    // Filter the catalog to the persona's pinned skills when a persona is active (agent-personas #1) —
    // the model's advertised skill set matches its focus. `None` → the full granted catalog (unchanged).
    // The persona's identity + pinned-skill BODIES are already in the goal (baked upstream in
    // dispatch.rs), so this is the catalog (name+description) half only. The grant is the wall either
    // way — filtering only removes already-granted entries.
    if let Some(catalog) = render_catalog_filtered(node, &agent, ws, persona_catalog).await? {
        state.messages.push(("system".into(), catalog));
    }

    // AGENT MEMORY (agent-memory scope): inject the derived memory index AFTER the persona + skill
    // catalog, framed as recalled background (not instructions). Read under an ON-BEHALF-OF principal
    // — the CALLER's sub (so the `member:{user}` scope resolves to the human behind the run, the same
    // identity-based contract as the skill/doc gate-3 in `substrate.rs`) with the AGENT's intersected
    // caps (so the read can never widen). Using the bare `agent:session` sub would resolve
    // `member:agent:session` and miss the caller's own memory. Best-effort (a deny/empty → no
    // injection, never a run failure); not persisted, so each run/resume re-injects cleanly.
    let on_behalf = caller.derive(caller.sub(), agent_caps.to_vec());
    if let Some(index) = memory_index_for_injection(&node.store, &on_behalf, ws).await {
        state.messages.push(("system".into(), index));
    }

    // RESUME: re-inject the bodies of any skills activated in a PRIOR run segment (Part 5 survives
    // resume — Part 0). `rehydrate` folds `SkillActivated` into `state.active_skills` (de-duped) but
    // not the body text (it has no store); here we reload each under the grant gate and re-inject, so
    // a resumed run continues with its activated skills in context. On a fresh run `active_skills` is
    // empty and this is a no-op. A skill revoked between segments simply drops (grant gate denies),
    // exactly as it would in the catalog.
    for id in state.active_skills.clone() {
        if let Ok(skill) = load_skill(&node.store, &agent, ws, &id, None).await {
            state
                .messages
                .push(("system".into(), format!("[skill {id}]\n{}", skill.body)));
        }
    }

    // RESUME PAST A SETTLED SUSPENSION (Part 2). A run that suspended on an Ask re-enters here with
    // an open suspension in its transcript. Apply the settled decision (Deny → denied result;
    // Allow → replay the original call), feeding the result into the live state before the model is
    // asked again. If the decision is still pending, the run is not actually resumable — leave it
    // suspended (a re-scan / premature resume is a no-op).
    if events
        .iter()
        .any(|e| matches!(e, TranscriptEvent::SuspensionOpened { .. }))
    {
        let resumed = resume_suspensions(node, &agent, ws, job_id, &events, index).await?;
        if resumed.still_pending {
            return Ok(state.last_content);
        }
        index = resumed.index;
        if !resumed.outcomes.is_empty() {
            // Mirror the resolved suspension's results into the live conversation, identically to a
            // rehydrated fold, so the next model turn sees them.
            state
                .messages
                .push(("tool".into(), summarize(&resumed.outcomes)));
            state.prior = resumed.outcomes;
        }
    }

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
        let key = format!("{ws}:{job_id}:{turn_no}");
        let turn = model
            .turn(ws, &state.messages, tools, &state.prior, &key)
            .await;
        state.last_content = turn.content.clone();

        // Record the assistant turn durably first (the transcript is the record), then mirror it
        // into the live message list — and emit the live `RunEvent` (StepStart + TextDelta) for any
        // watcher (Part 3).
        emit(
            node,
            ws,
            job_id,
            index,
            TranscriptEvent::AssistantTurn {
                content: turn.content.clone(),
            },
            turn_no,
        )
        .await?;
        index += 1;
        if !turn.content.is_empty() {
            state
                .messages
                .push(("assistant".into(), turn.content.clone()));
        }

        if turn.done || turn.calls.is_empty() {
            break;
        }

        // Persist each proposed call WITH ITS ARGS (needed for an Allow→replay resume, Part 2),
        // then consult the permission policy (Part 2) BEFORE dispatch — the gate that sits in front
        // of `caps::check` (defense in depth). The policy partitions this turn's calls into:
        //   - Allow → dispatch now under the DERIVED principal (still capability-checked);
        //   - Deny  → fed a "denied by policy" tool result, never dispatched;
        //   - Ask   → suspend the run durably and end the turn (resumed when a human decides).
        for c in &turn.calls {
            emit(
                node,
                ws,
                job_id,
                index,
                TranscriptEvent::ToolCallProposed {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    args: c.input.clone(),
                },
                turn_no,
            )
            .await?;
            index += 1;
        }

        // MODEL-ACTIVATED SKILLS (Part 5): `skill.activate` is a LOOP-INTERNAL built-in, intercepted
        // HERE rather than dispatched out through `lb_mcp::call` — the run-state mutation (append
        // `SkillActivated` + inject the body into context) lives only in the loop, where the
        // transcript/cursor/messages are. `activate_skill` enforces the S4 grant via `load_skill`
        // (ungranted → an error result the model sees, never an activation), so the wall holds. On
        // success we record `SkillActivated` (survives resume — Part 0; `rehydrate` de-dups) BEFORE
        // the `ToolResult`, then inject the body as a system message for the following turns.
        let mut activated_outcomes: Vec<CallOutcome> = Vec::new();
        let mut model_calls: Vec<&ProposedCall> = Vec::new();
        for c in &turn.calls {
            if c.name == SKILL_ACTIVATE {
                let act = activate_skill(&node.store, &agent, ws, &c.id, &c.input).await;
                if let Some((id, body)) = act.activated {
                    emit(
                        node,
                        ws,
                        job_id,
                        index,
                        TranscriptEvent::SkillActivated { id: id.clone() },
                        turn_no,
                    )
                    .await?;
                    index += 1;
                    // Inject the body once into context (idempotent at the conversation level — a
                    // re-activation re-appends, harmless; the rehydrated `active_skills` de-dups).
                    state
                        .messages
                        .push(("system".into(), format!("[skill {id}]\n{body}")));
                }
                activated_outcomes.push(act.outcome);
            } else {
                model_calls.push(c);
            }
        }

        // Record each activation's ToolResult durably NOW (before the possible Ask early-return), so
        // a turn that both activates a skill and suspends still has the activation in its transcript.
        for o in &activated_outcomes {
            emit(
                node,
                ws,
                job_id,
                index,
                TranscriptEvent::ToolResult {
                    id: o.id.clone(),
                    ok: o.ok.clone(),
                    err: o.error.clone(),
                },
                turn_no,
            )
            .await?;
            index += 1;
        }

        // Evaluate the policy per call (the args are parsed once for the shallow arg match).
        let mut to_run: Vec<ProposedCall> = Vec::new();
        let mut denied: Vec<CallOutcome> = Vec::new();
        let mut ask: Vec<&ProposedCall> = Vec::new();
        for c in model_calls {
            let args = serde_json::from_str(&c.input).unwrap_or(serde_json::Value::Null);
            // Evaluate the ws policy, then apply the persona's supervision FLOOR (persona-coding #4):
            // a preset Ask/Deny on a node-mutating tool clamps the evaluated effect UP unless the ws
            // policy explicitly ruled on that exact tool (the auditable loosen). `None` preset → no-op.
            let effect = super::personas::clamp_to_preset(
                evaluate(&policy, &c.name, &args),
                &c.name,
                &policy,
                persona_preset,
            );
            match effect {
                Effect::Allow => to_run.push(c.clone()),
                Effect::Deny => denied.push(CallOutcome {
                    id: c.id.clone(),
                    ok: None,
                    error: Some(DENIED_BY_POLICY.to_string()),
                }),
                Effect::Ask => ask.push(c),
            }
        }

        // ASK → suspend durably and return. We open a suspension for each Ask call (each gets its own
        // `agent_decision` record + transcript `SuspensionOpened`), then end the turn — the run is now
        // `Suspended` and resumes when the decision settles. We suspend BEFORE running the Allowed
        // calls of this turn: a turn that needs a human gate on *any* call pauses as a whole, so the
        // model sees a coherent next-turn state (no half-applied turn racing a human decision).
        if !ask.is_empty() {
            for c in &ask {
                index = open_suspension(node, &agent, ws, job_id, &c.id, index, ts).await?;
                // Emit the live `Suspended` delta so a watching lifecycle client (ACP
                // `session/request_permission`) sees the pause immediately — the durable
                // `SuspensionOpened` was just written by `open_suspension`, so the stream follows the
                // record (Part 3). The decision id is the convention `{job}:{tool_call}` (Part 2).
                publish_run_event(
                    &node.bus,
                    ws,
                    job_id,
                    &RunEvent::Suspended {
                        tool_call_id: c.id.clone(),
                        decision_id: format!("{job_id}:{}", c.id),
                    },
                )
                .await;
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
            emit(
                node,
                ws,
                job_id,
                index,
                TranscriptEvent::ToolResult {
                    id: o.id.clone(),
                    ok: o.ok.clone(),
                    err: o.error.clone(),
                },
                turn_no,
            )
            .await?;
            index += 1;
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
