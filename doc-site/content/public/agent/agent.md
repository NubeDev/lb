# Agent

TODO — filled as the `docs/scope/agent/` slices ship. Several shipped scope docs already
reference this page (`default-agent-wiring`, `agent-catalog`, `active-agent-wiring`,
`agent-context-basket`); their shipped truth should be promoted here, followed by
`agent-close-out` when it lands.

## Loop hardening (shipped 2026-07-12)

The in-house tool-call loop detects itself getting stuck, recovers from a full context instead of
erroring, never poisons its own transcript, classifies every provider failure honestly, and can
declare exfiltration-capable tools off-limits per workspace. All of it lives inside the existing
loop seams — zero new verbs, zero new tables. Scope + decisions:
`docs/scope/agent/agent-loop-hardening-scope.md` →
`docs/sessions/agent/agent-loop-hardening-session.md`.

### Provider faults are typed, not flattened

A failed model call is a `ProviderFault` carrying **structured** evidence — HTTP status,
`Retry-After` delta-seconds, and a context-overflow discriminant (`error.code ==
"context_length_exceeded"` or a 413) — never a completion-shaped error string. Classification
(`fault.lane()`, pinned by a table test) routes each fault:

- **transient** (network, timeout, 408/429/5xx, malformed body) → the same turn retries, bounded
  (3 attempts, `Retry-After` honored up to 30s), *below* step accounting — one turn, N attempts,
  same idempotency key; the gateway never caches a fault, so a retry really re-calls the provider;
- **overflow** → recovered by compaction (below), never retried verbatim;
- **fatal** (auth, malformed request — anything a verbatim retry cannot fix) → the run ends
  honestly: job **Failed**, `RunFinish(Failed)`, an attributed `[run failed: …]` answer. Never a
  fault dressed as a normal completion.

The fourth lane of the taxonomy — *model-recoverable* (denied tool, unknown tool, bad args) — was
already uniform: the error text becomes the tool result and the loop continues.

### Context compaction

Before every turn the loop estimates the conversation + advertised tool schemas (chars/4 — a
threshold heuristic, never billing). Over budget (`agent.config.compact_budget`, additive; node
default 48 000 estimated tokens), the oldest **whole turn groups** are dropped — an assistant
message and its tool summary are one atomic unit, never split; system messages, the goal, and the
latest user group always survive — and one cumulative breadcrumb (`[earlier steps compacted: N
turns]`) marks the loss visibly. A provider overflow mid-run compacts harder (half the current
estimate, up to 3 rounds) and **continues the same run**. Compaction transforms only the *live*
context: the durable transcript keeps every event, so a resume re-folds and re-compacts
deterministically.

### The dangling-tool-call invariant

Every durable transcript append in the agent service goes through **one chokepoint**
(`TranscriptWriter`), which also publishes the identical run-event projection a snapshot uses and
tracks proposed-but-unresolved calls. A turn that dies after proposing calls resolves each as
`ToolCancelled` (transcript event + run event) — a watcher's "tool running…" spinner always
resolves, and a resumed model sees "cancelled", not a silent gap. Pre-fix records heal lazily on
first resume: orphans (no result, no cancel, no parking suspension) gain `ToolCancelled` events
**appended at the cursor — step indices are never renumbered** (resume idempotency is a step-index
lookup). A suspension-parked call awaiting a human decision is not an orphan.

### The loop detector and the graceful ceiling exit

A sliding window (default 20; `agent.config.loop_window`, `0` disables) over
`(tool, hash(args), hash(result))` triples catches: **exact repeat** (3 consecutive identical
calls), **ping-pong** (A→B alternation, 4 cycles), and **no-progress** (same tool, identical
result, different args, 5 hits anywhere in the window — interleaving can't evade it). The strike
ladder: **warn** (a corrective message the model sees — deliberate polling can say so and proceed)
→ **block** (matching calls refused at dispatch, error-as-observation) → **break** (an honest
`Failed` terminal). A genuinely new result resets the ladder; a turn of nothing but blocked calls
escalates it. Thresholds are node constants; the config owns only on/off + window.

When a run hits the step ceiling mid-work, the loop now makes **one final tools-free completion**
asking the model to summarize where it got to — persisted as a normal assistant turn — then appends
the honest ceiling note. A fault or empty summary degrades to the plain note.

### The exfiltration guard

A tool that can transmit data off the node (send a message, fetch a URL, call a webhook) declares
`emits_external: true` on its **descriptor** — self-declared data like every other descriptor
field, so no tool-name list exists in core; extension manifests carry the same optional field
(versioned by absence). A workspace with `agent.config.exfiltration_guard: true` runs its in-house
agents with every tainted tool **excluded from the advertised menu AND denied at dispatch** (the
model can hallucinate a tool it was never shown — both gates fire). This is the standard
prompt-injection→exfiltration mitigation: a steered model has nothing to exfiltrate *with*.
Honest caveat: the taint is only as good as the declaration — a tool that lies is not caught; the
guard is defense-in-depth over the capability wall, never a replacement.

### Not yet

- **Wall-level coverage for external runtimes** (the detector and the guard scoring/starving a
  foreign ACP loop at the MCP chokepoint) — waits for
  `docs/scope/external-agent/capability-wall-scope.md`; the in-house loop is covered today.
- **Budget-gating the ceiling summary and real retry usage accounting** — compose with
  `agent-close-out` slices A/B (`Turn.usage`, `max_run_tokens`) when they ship; the seams are in
  place.
- **`lb-ext-sdk`**: the out-of-tree manifest authoring type gains the optional `emits_external`
  field (the in-tree parse already accepts it).
