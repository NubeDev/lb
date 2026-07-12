# Session — agent loop hardening (slices D, A, C, B, E)

Status: **done** (2026-07-12), branch `agent-loop-hardening` (5 commits, one per slice, not pushed).
Scope: [`../../scope/agent/agent-loop-hardening-scope.md`](../../scope/agent/agent-loop-hardening-scope.md).
Unsupervised session — every open question was decided here and recorded below.

## Summary — what shipped

All five slices, built in dependency order D → A → C → B → E, each its own commit:

| Slice | Commit | What |
|---|---|---|
| **D** error taxonomy | `ea7c4d6` | `ProviderFault` (status + `Retry-After` secs + overflow discriminant) replaces the stop-string flattening; `Provider::complete` / `ModelAccess::turn` → `Result`; `fault.lane()` classifies on **structured** evidence (21-row table test); gateway never caches a fault; `attempt_turn` bounded retry (3 attempts, `Retry-After` capped 30s) *below* step accounting; `fail_run` = job **Failed** + `RunFinish(Failed)` + attributed answer; `MockProvider::scripted()` failure arm. |
| **A** compaction | `6b2d0f6` | `agent/compact.rs`: chars/4 preflight estimate incl. tool schemas; drops oldest **whole turn groups** (system msgs / goal / latest-user-group / last group protected); one cumulative `[earlier steps compacted: N turns]` breadcrumb; provider overflow → compact to half the estimate + continue the **same** run (≤3 rounds, then honest failure). `agent.config.compact_budget` (additive; node default 48 000 est. tokens). |
| **C** dangling-call invariant | `ce8abbb` | All host `lb_jobs::append_event` call sites collapse into ONE chokepoint (`agent/transcript.rs::TranscriptWriter` — durable append + the same `project_one` motion + pending-call tracking). New additive `TranscriptEvent::ToolCancelled` / `RunEvent::ToolCancelled`; `writer.cancel_pending()` is the dead-turn protocol; load-time sanitizer (`lb_jobs::orphaned_calls` + `writer.heal_orphans`) heals pre-fix records by **appending at the cursor — never renumbering**; rehydrate folds a cancel as an error outcome the model sees. |
| **B** loop detector + ceiling exit | `71c565e` | `agent/loop_detector.rs`: window (default 20; `agent.config.loop_window`, `0` = off) over `(tool, fnv1a(args), fnv1a(result))`; exact-repeat 3 / ping-pong 4 cycles / interleaved no-progress 5; strike ladder **warn → block → break**, reset on a genuinely new result, blocked-only turns escalate (never reset on our own refusal). Ceiling exit: ONE tools-free summary completion, persisted as a normal assistant turn; fault/empty → the plain honest note. In-house runtime only. |
| **E** exfiltration taint | `fcab7c2` | `ToolDescriptor.emits_external` + manifest `Tool.emits_external` (additive, versioned by absence, opaque data — no tool-name list in core); `agent.config.exfiltration_guard`; a guarded run filters the advertised menu AND re-denies at dispatch (`agent/exfil.rs`, `run_calls` tainted check, `EXFIL_DENIED` error-as-observation). In-house coverage only. |

**Nothing was cut.** External-runtime coverage for B and E was **explicitly deferred by the brief**
(the wall-level detector/guard wait for `scope/external-agent/capability-wall`); it is a stated
non-goal here, not a silent gap.

## Unsupervised decisions (and why)

1. **Close-out slices A/B (`Turn.usage`, `max_steps`/`max_run_tokens`) have NOT shipped**, though
   the scope assumed they compose. Consequences handled explicitly:
   - The ceiling summary cannot be gated on `max_run_tokens` (no budget exists), so it is
     **unconditional but bounded to exactly one tools-free call**; the gate composes in
     `ceiling.rs` when close-out B lands (documented in the module header).
   - Retry usage accounting ("a failed step still records partial usage") has no `usage` to
     record; the retry sits below step accounting structurally, so it composes when close-out A
     lands. No fake counting was added.
2. **Compaction transforms the LIVE context only; the durable transcript keeps every event.** The
   scope's "breadcrumb into the transcript" is read as the conversation the model sees. Rationale:
   durable loss is strictly worse than none; resume re-folds the full record and re-compacts
   deterministically (same estimate, same budget), so live and resumed views agree. Rejected: a
   persisted `Compacted` transcript event (would entangle rehydrate with compaction and make the
   record lossy for zero benefit).
3. **Slice C's "strip the pending calls from the assistant message" translated to our shape** as:
   every persisted `ToolCallProposed` must gain a resolution (`ToolResult`, `ToolCancelled`, or a
   parking `SuspensionOpened`). Our transcript stores proposals as separate slots and rehydrate
   never replays them into messages, so the provider-validity risk lives in the *watcher view* and
   the *resume fold* — both now resolved by `ToolCancelled`. The heal **appends** (per the scope's
   hard rule: never renumber; resume idempotency is a step-index lookup).
4. **Sanitizer runs load-time (lazy), not as a boot heal** — the scope's stated lean, adopted. A
   suspension-parked call is NOT an orphan; an Allow-replay of a human-decided suspension is
   deliberately not re-vetoed by the exfiltration guard (the human decision is the stronger gate).
5. **Detector thresholds stay node constants; config gets the on/off + window only**
   (`loop_window`, `0` disables) — the scope's lean, adopted.
6. **Compact budget is a flat configured number** (`compact_budget`, default 48 000 estimated
   tokens ≈ 192 KB — inside every mainstream 128k model with completion headroom); per-model
   context metadata stays an agent-catalog follow-up — the scope's lean, adopted.
7. **`exfiltration_guard` lives on `agent.config`, not personas.** A persona narrows the *menu*
   only (`narrow_tools`) and can be switched per-invoke by the caller; the guard must also deny at
   dispatch and be an admin-held workspace posture — putting it on the admin-gated config record
   avoids a second narrowing seam AND keeps "the model can't opt out" true. (Resolves the scope's
   fourth open question.)
8. **Detector ladder vs. its own refusals:** a turn whose only outcomes are our `LOOP_BLOCKED`
   refusals escalates the ladder directly instead of entering the window — otherwise the blocked
   error's "novel" result hash reset the ladder and block→break never fired (caught by the unit
   test while building; design fix, not a shipped bug, so no debugging entry).
9. **A fatal turn now marks the job `Failed`** (was: a fault dressed as a normal `Done` answer).
   `run_session` still returns `Ok(answer)` so the channel worker has one message to post — the
   honesty moved into the job status + `RunFinish(Failed)` + the `[run failed: …]` note.
10. **`agent_answer_fallback_test`'s ceiling script now varies tool names** — its old
    identical-call-×16 script is *precisely* the spiral slice B ends at turn 5, which is the new
    intended behavior; the ceiling path is still covered with genuinely distinct work.
11. **Overflow discriminant** = `error.code == "context_length_exceeded"` (the OpenAI-compat
    machine field) or HTTP 413. A machine code is structured data — the "no error-string parsing"
    rule bans classifying on prose, not on codes. `Retry-After` is parsed in delta-seconds form
    only; the HTTP-date form is ignored (would need a wall clock; default backoff covers it).
12. **run.rs stayed under the 400-line cap** (it entered the session at 488, already over): the
    seeding phase (`seed_context.rs`), policy partition (`partition.rs`), activation interception
    (moved beside `activate.rs`), terminal exits (`attempt.rs::finish_run`, `ceiling.rs`,
    `step.rs::pause_exit`) each moved to one-responsibility files. Final: 395 lines.

## SDK follow-up (flagged, not implemented)

**`lb-ext-sdk`** (standalone repo): the manifest **authoring** type should gain the optional
`emits_external` field on `[[tools]]` so out-of-tree extensions can declare the taint. The in-tree
parse (`lb-ext-loader::manifest::Tool.emits_external`, serde-default) already accepts it —
versioned by absence, so SDK adoption is additive and unforced.

## Pre-existing failures encountered (verified on clean master, NOT chased)

- `agent_persona_catalog_test`: 6/8 fail (`PersonaSkill` — builtin personas pin `core.*` skills not
  seeded in the test ws). Reproduced identically on a clean master worktree.
- `agent_persona_coding_test`: 2/10 fail (extension-builder persona's `allowed` runtimes list
  drifted from the assertion). Reproduced identically on clean master.
- The known sets from earlier sessions (4 `panel_test`, `agent_routed_test`, broad
  `pnpm test:gateway`) were not re-litigated. No UI files were touched this session.

## Test evidence (green output)

New suites, all green:

```
role/ai-gateway  fault_class_test .............. 1 passed  (21-row status×headers×overflow table)
role/ai-gateway  openai_compat_test ............ 12 passed (incl. overflow body, plain-400 fatal,
                                                            429 Retry-After header, fault-not-cached)
role/ai-gateway  gateway_test / mock ........... green (Result-shape ports)
lb-jobs          sanitize_test ................. 3 passed
lb-host (lib)    loop_detector unit ............ 6 passed
lb-host          agent_hardening_error_test .... 3 passed (429-retry, retry-exhaustion→Failed,
                                                            401 fatal no-retry)
lb-host          agent_compact_test ............ 5 passed (group atomicity/protection/subsequence,
                                                            cumulative breadcrumb, nothing-droppable,
                                                            overflow→compact→continue, ws-walled budget)
lb-host          agent_dangling_test ........... 2 passed (kill→resume heal w/o renumbering +
                                                            ToolCancelled projection; parked-call safety)
lb-host          agent_loop_detector_test ...... 3 passed (warn→block→break on the real loop,
                                                            loop_window=0 opt-out ws-walled, ceiling
                                                            summary tools-free + persisted)
lb-host          agent_exfil_test .............. 3 passed (menu absence + dispatch deny, unguarded
                                                            unchanged, ws-walled guard)
```

Mandatory categories: capability-deny (exfil menu+dispatch, §2.1) ✓; workspace-isolation for every
new config axis (`compact_budget`, `loop_window`, `exfiltration_guard` each proven ws-walled, §2.2)
✓; offline/sync (kill-with-pending-calls → resume replays a valid transcript, §2.3) ✓; unit
properties (compaction invariants, detector windows + ladder reset, classification table, sanitizer
never renumbers) ✓; integration lanes via scripted provider faults ✓.

Regression: the agent/jobs/run-events suites listed above plus `agent_test`,
`agent_answer_fallback_test`, `agent_offline_test`, `agent_rehydrate_test`, `agent_decision_test`,
`run_control_test`, `agent_watch_test`, `agent_config_test`, `agent_runtimes_test`,
`agent_default_runtime_test`, `channel_agent_worker_test`, `rules_ai_wiring_test`,
`agent_active_model_test`, `agent_memory_test`, `core_skills_test`, `agent_page_context_test`,
`agent_skill_test`, `agent_persona_session_test`, `agent_def_test_test` — all green. Full
`cargo test --workspace` run recorded at session end (pre-existing failures only — see above).

## Cross-links

- Scope: `../../scope/agent/agent-loop-hardening-scope.md` (open questions resolved in place).
- Public: `../../../doc-site/content/public/agent/agent.md` — "Loop hardening" section promoted.
- Code map: `role/ai-gateway/src/fault.rs`, `crates/host/src/agent/{attempt,compact,transcript,
  loop_detector,ceiling,exfil,seed_context,partition}.rs`, `crates/jobs/src/sanitize.rs`,
  `crates/run-events/src/{event,project}.rs`.
