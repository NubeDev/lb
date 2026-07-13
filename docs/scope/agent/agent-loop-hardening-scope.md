# Agent scope — loop hardening (adopted ideas from zeroclaw, carapace, hermes-rs)

Status: **shipped (in-house slices, 2026-07-12)** — branch `agent-loop-hardening`, one commit per
slice (D `ea7c4d6`, A `6b2d0f6`, C `ce8abbb`, B `71c565e`, E `fcab7c2`); session log + decisions in
`../../sessions/agent/agent-loop-hardening-session.md`; promoted to
`doc-site/content/public/agent/agent.md` ("Loop hardening"). The **wall-level external-runtime
coverage of slices B and E** is explicitly deferred to `../external-agent/capability-wall-scope.md`
(the external runtime does not cross the wall yet) — a stated non-goal of the shipped cut, not a gap.

We surveyed three open-source Rust agent runtimes — **zeroclaw** (self-hosted agent runtime,
microkernel loop), **carapace** (security-hardened personal assistant), **hermes-rs** (ReAct
loop + TUI + autonomous mode) — for ideas worth stealing. This scope adopts the ones that fit
our architecture: they harden the in-house loop's *robustness* (a stuck agent detects itself,
a full context recovers instead of erroring, a cancelled turn never poisons the transcript)
and its *honesty* (errors are classified, loss is visible, exfiltration-capable tools are a
declared class). It deliberately overlaps nothing with `agent-close-out-scope.md` (accounting,
per-workspace policy, bus motion, routed tokens) — the two compose; ideas that belong to other
topics are routed there in Non-goals.

**Two runtimes, one wall.** The node also drives **external agents** (`scope/external-agent/` —
a foreign ACP loop we spawn but do not own). That splits this scope cleanly in two: slices that
live *inside the in-house loop* (A transcript compaction, C the transcript invariant, D's
retry lane) apply to the in-house runtime only — we don't checkpoint or repair a foreign
loop's context (run-lifecycle already settled that: our transcript is authority, resume safety
lives in effect idempotency). Slices enforced *at the MCP tool wall* (B's detector, E's
exfiltration guard) cover **both** runtimes for free, because the external agent's every tool
call already crosses the same `caps::check` chokepoint. That placement is deliberate: the
wall-level detector is also the first real answer to run-lifecycle's open question that an
external subprocess "is bounded only by wall-time today" — it gains a *semantic* stuck-ness
bound, not just a 15-minute clock.

## Goals

Five slices, each independently shippable, ordered by value:

- **A. Transcript-structural context management.** The loop today sends the whole transcript
  every step and dies on a provider context-overflow. Add: (1) a preflight token estimate
  (chars/4 is fine for a *threshold*, never for billing — close-out slice A owns real usage)
  including serialized tool schemas; (2) when over budget, compact by dropping the oldest
  **whole turn groups** — an assistant message with tool calls and its tool results are one
  atomic unit, never split (splitting produces API-invalid transcripts, the bug class every
  hand-rolled compactor hits); always keep the system message and the latest user group;
  (3) every drop injects a visible one-line breadcrumb into the transcript ("[earlier steps
  compacted: N turns]") — loss is never silent; (4) a provider context-overflow error *inside*
  the loop is recovered by compacting and continuing the same run, not failing it.
  (zeroclaw `history_trim`/`context_recovery`; hermes `compact_request_messages`.)
- **B. Loop detector + graceful ceiling exit.** A sliding window (default 20) over
  `(tool, hash(args), hash(result))` triples detects: exact repeat (3+ consecutive identical
  calls), ping-pong (A→B→A→B, 4+ cycles), and no-progress (same tool, different args,
  identical result hash 5+ times *across the window*, so interleaving other calls can't evade
  it). Escalation ladder: **warn** (inject a corrective message — "you appear to be
  repeating…") → **block** (refuse that specific call, error-as-observation) → **break** (end
  the run with an honest terminal event). And when the step ceiling (close-out slice B's
  `max_steps`) is hit, make **one final tools-free completion** asking the model to summarize
  where it got to — the user gets a coherent wrap-up instead of a bare "max steps exceeded".
  (zeroclaw `loop_detector` + `max_iter`.)
  The detector's *observation point* is the tool wall, so an **external** runtime's calls are
  scored by the same window; only the warn rung differs — we can't inject a message into a
  foreign loop, so warn/block are both delivered as the tool-error result the agent already
  understands (error-as-observation over ACP), and break rides the existing supervision kill
  path (run-lifecycle). The tools-free summary call is in-house-only.
- **C. The dangling-tool-call invariant.** A turn that dies after the model proposed tool
  calls but before their results landed (cancel, stall, provider error, budget/ceiling) must
  (1) emit one `tool_cancelled` run event per pending call — so a watcher (the dock) never
  hangs in "tool running…"; and (2) persist the assistant message **with the pending tool
  calls stripped** — the durable transcript never contains a proposed call without its
  result, which would poison the next provider request and every future resume. Enforce it
  where the transcript is written (one seam), plus a load-time sanitizer that drops orphaned
  entries from pre-fix records. (carapace `cancel_pending_tool_calls`; zeroclaw
  `history_pruner`.)
- **D. Error taxonomy at the loop seam.** Classify every failure into three lanes instead of
  the current ad-hoc handling: **transient** (network, 5xx, 429 with `Retry-After`, timeout →
  bounded mechanical retry of the same step, honoring `Retry-After`); **model-recoverable**
  (capability denied, unknown tool, bad args, tool runtime error → error text becomes the
  tool result, loop continues — we already do this for denials; make it the uniform rule);
  **fatal** (auth failure, budget exhausted, loop-detector break → honest terminal event).
  Context-overflow is its own lane: recover via slice A, never retry verbatim. Classification
  lives in the provider adapter on **structured** status/headers — do not parse error strings
  (zeroclaw's stringly-typed `parse_retry_after_ms` is the anti-pattern; carry
  status + `Retry-After` through `openai_compat`'s error type). A failed step that consumed
  provider tokens still records its partial usage (composes with close-out slice A).
- **E. Exfiltration taint as a declared tool property.** A tool that can transmit data off
  the node (send a message, fetch a URL, call a webhook) declares `emits_external: true` on
  its **descriptor** — self-declared data, exactly like every other descriptor field, so rule
  10 holds (no hardcoded tool-name list in core; carapace's static set naming
  `telegram_send_photo` in core is the anti-pattern). A run flagged `exfiltration_guard`
  (per-invoke or `agent.config`) excludes tainted tools from the advertised menu **and**
  denies them at dispatch — gate at both definition time and call time, because the model can
  hallucinate a tool it was never shown. This is the standard prompt-injection→exfiltration
  mitigation: untrusted content may steer the model, but the steered model has nothing to
  exfiltrate *with*. Because both the advertised menu and the dispatch check live at the wall,
  a guarded **external** run is covered identically — the foreign agent is only ever handed
  the filtered MCP surface (capability-wall scope), and a hallucinated tainted call dies at
  the same chokepoint.

## Non-goals

Ideas from the survey that are good but belong elsewhere — routed, not dropped:

- **Memory lifecycle** (zeroclaw's importance/decay/dedup/budget-eviction; hermes' async
  post-run distillation with a strict JSON-array contract) → `agent-memory` topic. The
  distill-on-run-end trigger and importance-threshold + token-budget injection are the two
  ideas to lift when that scope gets its next slice.
- **Job lifecycle upgrades** (carapace's `blocked` state with a categorized reason + operator
  patch-and-resume) → `jobs` topic. Today a stalled agent job is just `failed`; "blocked on a
  missing credential, patch it, resume" is the mature shape.
- **State-fingerprint failure pause** (hermes' autonomous mode: after N failures on the same
  hash of inputs, pause until the fingerprint changes) → the reactors that retry
  (flows triggers, workflow/coding loop). Keying retry suppression on a state hash instead of
  a counter is the fix for burning budget re-running an unchanged broken state.
- **Provider reliability decorators** (zeroclaw's `Reliable(Router(Concrete))` fallback
  chains) → `ai-gateway` topic, already on its deferred list. Slice D's taxonomy is the
  prerequisite half we take now.
- **Deferred tool loading + `tool_search`** (zeroclaw): stub-only tool menus with on-demand
  schema materialization. Deferred until our advertised catalog measurably hurts context; the
  personas roster already narrows the menu. If adopted later, the load-bearing rule is that
  **the capability check fires at discovery** — a denied tool is never surfaced even as a
  stub.
- **HMAC tool receipts** (zeroclaw): per-session key signing `tool|args|result` so a claimed
  tool run is verifiable. Clever, small, but our durable transcript already *is* the record
  of what ran, written by the host, not the model. Revisit only if agent-claimed effects ever
  flow somewhere the transcript doesn't.
- **Regex prompt-injection scanning** of inbound content (all three have a variant): rejected
  as a security boundary — blocklist regexes are theater against a determined attacker; the
  capability wall stays the control. Our existing posture (fence untrusted content
  structurally, refs-not-bodies — the context-basket seam) is the stronger version of
  carapace's `ContentSource` labeling and we already have it.
- **Sub-agent delegation**: all three converge on the same answer we already scoped —
  delegation is a tool, the child's grants are a strict subset of the parent's (zeroclaw's
  `ensure_no_escalation_beyond` is our `agent ∩ caller` intersection). Nothing new to adopt;
  noted as independent confirmation of the design.

## Intent / approach

**Everything lands inside the existing loop seams; zero new verbs, zero new tables.** The
in-house loop (`crates/host/src/agent/run.rs`) already owns step iteration, the one tool wall,
and transcript persistence — each slice is a small pure module the loop calls, one
responsibility per file (`agent/compact.rs`, `agent/loop_detector.rs`, `agent/error_class.rs`,
a transcript-write chokepoint for slice C). That is also the strongest lesson from the survey
*negatively*: zeroclaw's loop grew to a 15,000-line file and needed a 2,000-line "safety net"
test suite to decompose; carapace's executor is 4,200 lines with 15 `&mut` params. Our ≤400
rule is not cosmetics — it is what makes these features addable.

Config is additive: `agent.config` gains optional `loop_window`, `compact_budget`,
`exfiltration_guard` alongside close-out's `max_steps`/`max_run_tokens`, admin-written via the
shipped `agent.config.set`, node-clamped the same way. No behavior key ships before its
behavior (zeroclaw's dead `rerank_enabled` config key is the anti-pattern).

Rejected alternative: an LLM-based "is the agent stuck / is this injection" classifier on the
hot path (carapace runs one per inbound message). Cost and latency per message for a
heuristic signal; our detectors (slice B) are hash-based and free, and injection defense
stays structural.

## How it fits the core

- **Tenancy / isolation:** unchanged — every slice operates inside a run whose job record,
  tool wall, and events are already workspace-scoped. No new keys.
- **Capabilities:** slice E narrows, never widens: guarded runs advertise ∩ dispatch a subset
  of the derived principal's tools; the deny is the same chokepoint refusal the model already
  sees for a missing cap. Slices A–D touch no authorization path.
- **Placement:** either — pure loop logic, symmetric on every node.
- **MCP surface:** no new tools. `agent.config.set/get` carry the additive fields. Descriptor
  gains the optional `emits_external` flag (opaque, self-declared; consumed generically). N/A:
  CRUD/list/feed/batch — nothing new to expose.
- **Data (SurrealDB):** the transcript (job record) is the only state touched: compaction
  breadcrumbs and stripped-pending-call writes go through the one persistence seam. No new
  tables.
- **Bus (Zenoh):** `tool_cancelled` and the loop-detector warn/break become `RunEvent`
  variants on the existing run-event stream (fire-and-forget motion; close-out slice C moves
  that stream onto the bus — this scope just adds variants).
- **Sync / authority:** slice C is what makes resume trustworthy — a resumed job can rely on
  the invariant that every persisted tool call has its result.
- **Secrets:** N/A.
- **No mocks:** `MockProvider` grows deterministic failure scripting (a 429 with
  `Retry-After`, a truncated stream, a context-overflow error) so every lane in slice D and
  the recovery in slice A are exercised on the real loop against the real store.

## Example flow

1. A dock user asks the agent to reconcile a large dataset. By step 9 the transcript +
   tool schemas exceed the compact budget: the two oldest turn groups are dropped whole, a
   breadcrumb lands in the transcript, the step proceeds (slice A).
2. The model gets stuck calling `query.run` with slightly different SQL, identical empty
   result, five times. The detector's no-progress rule fires: a warning message is injected;
   the model tries once more identically; the call is blocked and the refusal fed back; the
   model changes approach (slice B).
3. Mid-step the provider returns 429 with `Retry-After: 2` — classified transient, the step
   retries once after 2s and succeeds (slice D). The retried call's partial usage was still
   recorded.
4. The user cancels the run while two tool calls are pending: two `tool_cancelled` events
   reach the dock (spinners resolve), and the persisted assistant entry carries no dangling
   calls — a later `resume` on the job replays a valid transcript (slice C).
5. An admin flips `exfiltration_guard` on for the workspace: the next run's advertised menu
   contains no `emits_external` tools, and a hallucinated `channel.post` proposal is denied
   at the wall like any missing cap (slice E).

## Testing plan

Per `scope/testing/testing-scope.md` — real store (`mem://`), real loop, `MockProvider` with
deterministic scripts (rule 9; the provider HTTP is the one sanctioned fake):

- **Capability-deny (§2.1):** a guarded run's tainted tool is absent from the advertised set
  AND denied at dispatch when proposed anyway; an unguarded run with a missing cap behaves
  exactly as today (slice E adds a filter dimension, it does not alter the wall's logic).
- **External runtime (feature-on builds only, real subprocess per rule 9):** the wall-level
  slices hold for an external run — a guarded run's tainted tool never reaches the foreign
  agent's menu and is denied if called anyway; a scripted repeat-calling agent trips the
  detector (block delivered as an ACP tool error; break reaps via the run-lifecycle
  supervision path). Loop-local slices are asserted *absent* here: no compaction/summary-call
  behavior is triggered for a foreign loop.
- **Workspace-isolation (§2.2):** covered by the existing run/`agent.config` isolation tests;
  the new config fields ride `agent.config` (assert a ws-B read never sees ws-A's guard flag).
- **Offline/sync (§2.3):** kill a run with pending tool calls; resume the job; the replayed
  transcript is provider-valid (no orphaned calls) and the resumed run completes.
- **Unit:** compaction never splits a turn group / always keeps system + latest user group /
  always injects a breadcrumb (property-style over generated transcripts); loop-detector
  windows (exact / ping-pong / interleaved no-progress, and that the escalation ladder resets
  on a genuinely new result hash); error classification table (status × headers → lane,
  ~20 rows, one table-driven test — carapace's finish-reason test is the model);
  transcript-write chokepoint strips pending calls; load-time sanitizer drops orphans.
- **Integration:** scripted MockProvider runs for each lane in slice D (429-retry,
  overflow→compact→continue, mid-stream death→cancel protocol) through the full
  invoke → loop → job-persist path; `tool_cancelled` observed on the run-event stream.

## Risks & hard problems

- **Compaction correctness is transcript-validity-critical.** A group-splitting bug turns
  into provider 400s on *resume*, far from the cause. Hence the property tests and one write
  seam — the same reasoning as the caps intersection in `agent-scope.md`.
- **Detector false positives.** Legitimate polling (a tool that checks job status) looks like
  no-progress. The window keys on the *result hash* — a genuinely unchanged result five times
  is worth a warning even when intentional; the warn rung exists precisely so the model can
  say "I am intentionally polling" and proceed. Block/break thresholds stay conservative.
- **Retry × budget interaction.** A transient retry must re-check the run budget and must not
  double-record usage. Retry sits *below* the step accounting (one step, N attempts, summed
  usage) — decide and test this ordering explicitly.
- **Taint coverage is only as good as declarations.** A tool that lies (`emits_external`
  absent) is not caught — same trust model as every descriptor field. The guard is
  defense-in-depth over the capability wall, not a replacement; say so in the public doc.

## Open questions — all resolved (2026-07-12, session doc "Unsupervised decisions")

- **Detector thresholds:** RESOLVED as the lean — node constants (window 20 / repeat 3 /
  ping-pong 4 / no-progress 5); `agent.config.loop_window` configures the on/off + window only
  (`0` disables). Revisit the numbers with real transcripts.
- **Compact budget source:** RESOLVED as the lean — a flat configured number
  (`agent.config.compact_budget`, node default 48 000 estimated tokens); per-model context
  metadata stays the `agent-catalog` follow-up.
- **Where slice C's sanitizer runs:** RESOLVED as the lean — load-time, lazily, on first resume:
  orphans are healed as `ToolCancelled` events **appended at the cursor** (existing step indices
  never renumbered). No boot heal; nothing measured in the wild demanded one.
- **Does `exfiltration_guard` belong on personas instead?** RESOLVED — `agent.config`. A persona
  narrows only the *advertised menu* and is caller-switchable per invoke; the guard must also deny
  at dispatch and be an admin-held workspace posture, so it rides the admin-gated config record
  (no second narrowing seam; the model/caller cannot opt out).
- **New (discovered):** the summary-call token gate and retry usage accounting await close-out
  slices A/B (`Turn.usage`, `max_run_tokens`) — the seams are in place (`ceiling.rs` header,
  `attempt_turn`), currently ungated because no budget exists to gate on.

## Related

- Surveyed repos (cloned as siblings for reference): `~/code/rust/zeroclaw`
  ([zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) — loop detector,
  history trim/prune, deferred tools, receipts, memory lifecycle),
  `~/code/rust/carapace` ([puremachinery/carapace](https://github.com/puremachinery/carapace)
  — cancellation protocol, two-point tool gating, error precedence tests, task lifecycle),
  `~/code/rust/hermes-rs` ([eikarna/hermes-rs](https://github.com/eikarna/hermes-rs) —
  group-preserving compaction, distillation, state-fingerprint pause, error taxonomy).
- `../external-agent/external-agent-scope.md` (+ `capability-wall`, `run-lifecycle`) — the
  second runtime: wall-level slices (B, E) must hold for it; loop-local slices (A, C, D-retry)
  are explicitly in-house-only, per the "two runtimes, one wall" split above.
- `agent-scope.md` — the loop this hardens; `agent-close-out-scope.md` — the sibling finish
  line this composes with (usage, policy, motion, routed tokens); `default-agent-wiring-scope.md`
  — the tool wall slice E gates at.
- `../agent-personas/` — the menu-narrowing seam slice E must not duplicate.
- `../agent-memory/agent-memory-scope.md`, `../jobs/jobs-scope.md`,
  `../ai-gateway/ai-gateway-scope.md` — the routed non-goals.
- Skill doc: **N/A** — no new drivable surface (no new verbs/routes); behavior changes ride
  existing `agent.invoke`/`agent.config`, whose skills the implementing session updates if
  their contracts gain fields.
