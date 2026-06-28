# Agent-run Part 2 — per-tool Allow/Deny/Ask + durable first-settle decision

Built on Parts 0/1 (typed transcript, rehydrate, suspend/cancel, `lb_store::create`).

## What shipped

- **Policy** (`crates/host/src/agent/policy/`): one ws record `agent_policy:{ws}` (id = ws),
  a rule list (`*`-glob on tool name + optional shallow top-level arg equality) → `Allow|Deny|Ask`.
  Pure evaluator, precedence **Deny > Allow > Ask** (order-insensitive), **default-allow** when no
  rule matches. Tiny in-house `*` matcher (no glob crate for one wildcard).
- **`agent.policy.set`** (admin cap `mcp:agent.policy.set:call`) + **`agent.decide`**
  (`mcp:agent.decide:call`) wired via a single `agent.` branch in `tool_call.rs` →
  `call_agent_tool` in `crates/host/src/agent/tool.rs` (leaves an explicit `NotFound` arm + comment
  for Part 3's `agent.watch`).
- **First-settle decision** (`crates/host/src/agent/decision/`): `agent_decision:{job}:{tool_call}`.
  `open` = `lb_store::create` pending (first-write reservation) + `needs:approval` inbox item +
  transcript `SuspensionOpened` (before suspend) + `lb_jobs::suspend`. `settle` = conditional
  `Pending→Settled` flip + `unsuspend`; second decide → `AlreadySettled` no-op; decide for unopened
  call → store `Conflict`.
- **Loop gate** in `run.rs`: policy consulted per call before dispatch; Ask → `open_suspension` +
  return (Suspended); Deny → denied-by-policy ToolResult; Allow → dispatch. On resume,
  `resume_suspensions` applies the settled decision (Deny → denied result; Allow → **replay** from
  persisted `ToolCallProposed.args`), appends `SuspensionSettled` + `ToolResult`.

## First-settle design choice

Open `create`s the pending record (reserves the key); settle does a **guarded update** (only-if-
pending), not a second `create`. Rationale: open must `create` to reserve, settle must mutate the
*same* row — a guarded flip is the clean conditional. `SettleOutcome::{Bound, AlreadySettled}` makes
the no-op observable. (Read-then-conditional-write is correct for the single-node single-settler path
today; noted as the one place to harden to `UPDATE … WHERE state='pending'` if a multi-writer settle
path ever appears.)

## run.rs edits (for integrator reconciling with Part 3/5)

- Added imports: `decision::{open_suspension, resume_suspensions, DENIED_BY_POLICY}`,
  `policy::{evaluate, load_policy, Effect}`.
- `run_calls` made `pub(crate)` (called by `decision/resume.rs`).
- After rehydrate: `load_policy` once; a resume block that calls `resume_suspensions` when the
  transcript has an open `SuspensionOpened`.
- Inside the loop: after appending `ToolCallProposed`, partition calls by policy effect (Ask → open +
  return; Deny → synth error; Allow → run). File is 296 lines.

## Tests (all green, real store/bus/wasm; only MockProvider stubbed)

- `crates/host/tests/agent_decision_test.rs`: 7 passed — first-settle (the one that fails vs LWW
  Resolution), caps-deny (decide + policy.set), ws-isolation (decide + policy), offline/sync
  (suspend→reload→decide resumes exactly once), Ask→Deny→resume, Ask→Allow→replay.
- Policy unit tests (in-crate): 10 passed — Deny>Allow>Ask, glob, shallow arg equality, default-allow.
- Pre-existing `agent_test` (4) and `agent_offline_test` (2) still green.

## Caps added (dev_claims member_caps)

`mcp:agent.decide:call` + `mcp:agent.policy.set:call`.

## Follow-ups / notes

- Reactor wake: `settle` leaves the job Running (unsuspended) + resumable; a direct `resume()` after
  settle is the wired path (proven by tests). A standalone `react_to_decisions` scan (mirroring
  `react_to_approvals`) was NOT built this session — it is a thin add later (scan suspended jobs whose
  decision is settled, call resume). Noted as a follow-up.
- `UseDecisionAsResult` resume mode deferred (enum field exists; only Deny/Allow built).
- Per-run policy overrides deferred (ws-scoped only), per scope.
- Pre-existing build error in `role/gateway/src/routes/ext.rs` + `crates/devkit` (Part 3 territory,
  `lb_devkit::sign_artifact` arity) is unrelated to Part 2; lb-host builds clean.
