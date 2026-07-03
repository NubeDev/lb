# Rules scope — wire the `ai.*` verbs to the real agent model

Status: **shipped** (2026-07-03) — session `../../sessions/rules/rules-ai-wiring-session.md`. The
`DisabledModel` hardcode is retired from the configured `rules.run` path; a rule's `ai.*` reaches the
node's real `ModelAccess`, resolved per-workspace from `agent.config`. Proven green against
`AiGateway<MockProvider>` (`crates/host/tests/rules_ai_wiring_test.rs`, 8/8). Promotes to
`public/rules/rules.md` on the next public-doc revision. (Original ask preserved below.)

The rules engine already ships the `ai.*` verbs (`ai.ask` / `ai.complete` / `ai.classify` / `ai.embed`)
with their budget meter and nsql fence intact, and the data half (`query(...)` / `source(...)`) is
genuinely wired to `store.query` / `series.*` / `federation.query`. But the **AI half is not hooked up**:
the production dispatch path (`call_rules_tool` → `rules.run`) hardcodes `Arc::new(DisabledModel)`, whose
`complete` / `propose_sql` both return `Err("AI not configured for rules")`. So today a rule that calls
`ai.ask("which coolers ran hot today?")` fails with that error over the gateway/palette — only the
`rules_test.rs` path, which injects a scripted model, exercises the real seam. This scope **binds the
rule engine's model seam to the real agent** — a `RuleModel` adapter over the host's `ModelAccess`
(the AI-gateway), resolving the workspace's **selected** model from the shipped `agent.config`
(the definition the agent-catalog picker writes) — so `ai.*` works for real in a rule.

## Goals

- **`ai.*` in a rule reaches the real model.** Replace `DisabledModel` on the `rules.run` bridge with a
  `RuleModel` implemented over `lb_host::ModelAccess` (the AI-gateway surface the agent core already
  uses). `ai.complete` → one model turn; `ai.ask`'s `propose_sql` → one model turn with the nsql prompt.
  The budget meter and the nsql fence (proposed SQL re-validated through `DataSeam::collect`) are
  **unchanged** — this scope only fills the model, it does not touch the fence or the cage.
- **Per-workspace model selection, honestly.** The rule's model routes to the **workspace's selected
  agent definition** — the `agent.config` record the agent-catalog picker writes (`default_runtime` +
  `model_endpoint`). This makes the catalog's pick *do something* for rules: the same endpoint a
  workspace chose for its agent is the one its rules' `ai.*` use.
- **A clean "AI not configured" path stays.** A workspace with no model configured (no provider adapter,
  or no selection) still runs data-only rules; a rule that calls `ai.*` gets the clear, honest error —
  never a silent wrong answer, never a panic. The disabled path becomes a *resolved* state, not a
  hardcoded default.
- **No new machinery.** Reuse the shipped `ModelAccess` seam, the shipped `agent.config` resolution, and
  the existing `RuleModel` trait (which `rules_run` already takes as a parameter — the seam is already
  open). One adapter file + one wiring change on the bridge.

## Non-goals

- **Not the multi-turn agent loop / tool use.** A rule's `ai.*` is **single-turn** completion +
  nsql proposal — it does not need `agent.invoke`, the tool-calling loop, suspend/resume, or the durable
  session. The `RuleModel` adapter calls `ModelAccess::turn` with **no tools** and reads the content;
  giving a rule the full agent loop is explicitly out (a rule is a bounded, metered, sandboxed script,
  not an agent run). If a rule genuinely needs the agent, it emits an `agent.invoke` — a separate seam.
- **Not a new provider adapter.** This inherits the agent core's exact limit: **no real `Provider`
  adapter exists yet** (only the sanctioned `MockProvider`; real adapters are ai-gateway-scope-deferred,
  the same dependency `default-agent-wiring` and `agent-catalog` name). This scope wires the rule engine
  to `ModelAccess` so the day a real adapter lands, rules' `ai.*` answer with a real LLM with **no
  further rule change**. Until then it is proven for real against `AiGateway<MockProvider>`.
- **Not per-rule model override.** The model is the **workspace** selection (`agent.config`), like the
  agent. A per-rule "use this definition" arg is a named follow-up, not v1.
- **Not touching the fence, meter, cage, or the data seam.** Those ship and are tested; this scope does
  not modify them. The nsql fence in particular is load-bearing security — it stays exactly as-is.
- **No new MCP verb.** `rules.run` already carries `ai.*`; this changes what the model *is*, not the API
  surface. No `rules.*` verb is added, changed, or removed.

## Intent / approach

**The seam is already open — fill it, don't rebuild it.** `rules_run(node, principal, ws, …, model:
Arc<dyn RuleModel>, now)` takes the model as a parameter; only `call_rules_tool` passes the wrong thing
(`DisabledModel`). The fix is one adapter + one resolution:

1. **`RuleModel` over `ModelAccess`** (`crates/host/src/rules/model.rs`, new). An `AgentRuleModel` holds
   the resolved model handle (the same `ModelAccess` the agent core builds) and the workspace id.
   - `complete(prompt)` → `model.turn(ws, [("user", prompt)], &[], &[], key).await` on the seam's
     `block_on` handle → return `(turn.content, tokens)`. No tools passed (a rule's `ai.complete` is a
     pure completion; the rule's data/emit power comes from the *rule verbs*, gated by the cage + caps —
     the model must not get an independent tool channel).
   - `propose_sql(question, schema_hint)` → one `turn` with the shipped nsql prompt (the schema hint +
     "propose read-only SQL") → return the SQL string. The **fence is unchanged**: the returned SQL
     still flows back through `DataSeam::collect`'s validator + `caps::check` before it can run.
2. **Resolve the workspace's model on the bridge.** In `call_rules_tool`'s `rules.run` arm, resolve the
   model the same way the agent does — from `agent.config` (the catalog pick) → the node's configured
   `ModelAccess`. If a model resolves, pass `Arc::new(AgentRuleModel::new(model, ws))`; if none is
   configured, pass a `DisabledModel` (the honest error stays, now as a *resolved* outcome). This makes
   the agent-catalog selection the single source of truth for "which model do my rules use".

**Rejected alternatives.**
- *(a) Give rules the full `agent.invoke` loop.* Rejected: a rule is a sandboxed, metered, single-shot
  script; the agent loop brings tool use, suspend/resume, and a durable session a rule neither wants nor
  should have (it would blow the cage's determinism + budget guarantees). Single-turn `ModelAccess` is
  the right altitude — the same call the agent's own loop makes per turn, minus the loop.
- *(b) A second, rules-only model config* (its own endpoint record). Rejected: it duplicates
  `agent.config` and splits "which model does this workspace use" into two places. The catalog pick
  already *is* the workspace's model choice; rules should honor it, not fork it.
- *(c) Keep `DisabledModel` and tell users to call `agent.invoke` from a rule.* Rejected: the `ai.*`
  verbs are shipped, documented, and fenced — leaving them dead is a worse lie than wiring them. The
  meter + fence exist precisely so a rule *can* safely call a model.

## How it fits the core

- **Tenancy / isolation:** the model is resolved **per workspace** from that workspace's own
  `agent.config` (the hard wall — `agent_config_get` is workspace-scoped). `ModelAccess::turn` is called
  with the run's `ws`, and the idempotency key is workspace-derived. A rule in ws-A can never route to
  ws-B's model or read ws-B's schema (the nsql prompt is built from `DataSeam::schemas`, already
  workspace-pinned). No cross-tenant path is introduced.
- **Capabilities (the deny path):** unchanged at the rule surface — `rules.run` is already gated by
  `mcp:rules.run:call` (workspace-first, opaque deny), and every `ai.*` call charges the budget meter
  before it runs. The **model resolution reads `agent.config`** (member-level `agent.config.get`
  semantics, but here it is a host-internal read on the already-authorized `rules.run` path, not a new
  caller-facing verb). No new capability is minted; a rule that lacks `rules.run` never reaches the
  model. *Open question:* whether an additional `mcp:rules.ai:call`-style sub-gate is warranted so an
  admin can grant "run rules" without "let rules spend model budget" — proposal below.
- **Placement:** `either`. The rule engine + the `ModelAccess` seam run on every node (symmetric); which
  model is *available* is config (a node with no provider adapter resolves to `DisabledModel`) — exactly
  the agent core's posture. No `if cloud`.
- **MCP surface** (API shape §6.1): **no change.** `rules.run` already exists and already carries the
  `ai.*` result in its `{output, findings, log, ms, ai}` return. This scope changes the *model behind*
  the verb, not the verb. **CRUD / get-list / live-feed / batch: all N/A** — `rules.*` CRUD ships; a
  rule run is a bounded synchronous call (the cage + meter bound it), not a long job; there is no motion
  to stream (a rule run returns its result). Said explicitly per §6.1.
- **Data (SurrealDB):** none new. The adapter reads the existing `agent.config` record to resolve the
  model; it writes nothing. The rule's data reads still go through the shipped `DataSeam`.
- **Bus (Zenoh): N/A** — a rule run is request/response; no subject, no motion. (A rule's *emitted*
  effects still go through the shipped emit verbs / outbox — unchanged.)
- **Sync / authority:** node-local — the model handle is the node's configured `ModelAccess`; the
  selection is the workspace's `agent.config` (LWW record, offline-safe, already shipped). No new
  authority.
- **Secrets:** none handled here. The model endpoint is names-only (`api_key_env`), mediated exactly as
  the agent core mediates it — the rule adapter passes the resolved `ModelAccess`, never a key. The nsql
  prompt carries only the workspace's own schema names, never a secret.
- **No fake backend (rule 9 / testing §0):** the **model is the one true external**, already behind the
  sanctioned `Provider` trait with the deterministic `MockProvider` (the only mock the platform allows,
  testing §3). Tests wire the rule engine to `AiGateway<MockProvider>` (real gateway, real meter, real
  fence, real store) and drive `ai.*` through the real bridge — no `*.fake.ts`, no hand-rolled model.
  The existing `rules_test.rs` `ScriptedModel` is the same sanctioned pattern; this scope makes the
  **production bridge** use the real seam, and tests prove it against the real gateway path.
- **State vs motion:** the model call is a bounded synchronous turn inside the cage (motion is N/A).
- **One responsibility per file (FILE-LAYOUT):** the adapter is one new file
  (`crates/host/src/rules/model.rs` — `AgentRuleModel` + its `RuleModel` impl); the bridge change is a
  few lines in `rules/mod.rs` (resolve the model, pass it). No file grows a second responsibility.
- **SDK/WIT impact:** none — host-internal wiring; no plugin-boundary change, no new verb.

## Example flow

1. Ada (admin) has picked **"In-house — Z.AI GLM-4.6"** in Settings → Agent (the agent-catalog picker
   wrote `agent.config = { default_runtime: "default", model_endpoint: {provider:"zaicoding",
   model:"glm-4.6", api_key_env:"ZAI_API_KEY", …} }`).
2. Ada writes a rule:
   ```
   let hot = query("timescale", "SELECT point, value FROM readings WHERE value > 30 ORDER BY ts DESC LIMIT 100");
   let summary = ai.ask("which coolers ran hot today?");
   emit(summary);
   ```
3. She runs it (`rules.run`, gated by `mcp:rules.run:call`). The bridge resolves the workspace model from
   `agent.config` → the node's `ModelAccess` (today `AiGateway<MockProvider>`; a real adapter later).
   It builds `AgentRuleModel` and passes it into `rules_run` (no more `DisabledModel`).
4. `query("timescale", …)` collects through the shipped `federation.query` seam under caller ∩ grant
   (unchanged).
5. `ai.ask(...)` charges the budget meter, asks the model to `propose_sql` against the workspace's own
   schemas, and the proposed SQL is **re-validated through `DataSeam::collect`** (the fence) before it
   runs — a proposed cross-source or non-granted query is rejected at collect. The result is a Grid.
6. A workspace with **no model configured** runs the same rule: `query(...)` works; `ai.ask(...)` returns
   the honest `"AI not configured for rules"` error (now a *resolved* state, not a hardcoded default).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all rule-9 (real store `mem://` / caps /
gateway / meter / fence; the model is the sanctioned external behind `Provider`, driven via
`AiGateway<MockProvider>` — no `*.fake.ts`):

- **AI wired (the headline):** a rule calling `ai.complete` / `ai.ask` over the **real bridge**
  (`call_rules_tool` → `rules.run`) with a workspace whose `agent.config` resolves a model returns the
  model's real (mock-deterministic) output — proving `DisabledModel` is gone from the configured path.
- **Fence still holds (regression):** `ai.ask`'s proposed SQL is re-validated through `DataSeam::collect`
  — a proposed query against a non-granted / cross-source table is denied at collect, exactly as today.
  This must not regress; assert a model-proposed bad query cannot execute.
- **Budget meter still charges:** each `ai.*` call charges before running; a rule that exceeds its call
  or token budget is stopped (the meter is unchanged — assert it still bites with a real model behind).
- **Capability-deny (§2.1):** `rules.run` without `mcp:rules.run:call` is denied opaquely (unchanged);
  and — per the open question — if a `rules.ai` sub-gate is added, a rule calling `ai.*` without it is
  denied while a data-only rule still runs.
- **Workspace-isolation (§2.2):** ws-A and ws-B select **different** model endpoints in `agent.config`;
  a rule in ws-B resolves ws-B's model and NEVER ws-A's; the nsql schema prompt lists only ws-B's own
  granted sources.
- **AI-not-configured path:** a workspace with no configured model runs a data-only rule fine, and a
  rule calling `ai.*` gets the clear `"AI not configured for rules"` error (no panic, no wrong answer).
- **Selection round-trip (composition):** picking a definition in the agent-catalog changes which model a
  subsequent rule run uses (compose with the shipped `agent.config` write + this resolution) — proving
  the catalog pick drives rules' `ai.*`.
- **Unit:** the `AgentRuleModel` adapter maps `complete`/`propose_sql` onto `ModelAccess::turn` with no
  tools and reads content/tokens correctly (a small direct test over a scripted `ModelAccess`).

## Risks & hard problems

- **The provider adapter is still the gate.** Like the agent core, rules' `ai.*` truly answers only when
  a real `impl Provider` exists; until then it is proven against `AiGateway<MockProvider>`. The risk is
  **over-promising** in the UI/docs: the rules workbench must not imply a rule's `ai.ask` hits a real LLM
  before an adapter is configured. Copy this honestly (mirror `default-agent-wiring` / `agent-catalog`).
- **Fence regression is the scariest failure mode.** The nsql fence is the security boundary between a
  model-proposed query and execution. This scope must not touch it, and the test suite must assert it
  still rejects a bad proposed query with a real model behind. A change here that quietly weakened the
  fence would be a serious finding.
- **Budget under a real model.** The meter is charged on the rule side, but a real provider's token
  accounting must feed `charge_tokens` accurately (the mock returns a fixed count). When a real adapter
  lands, verify the reported token count is the provider's, not a placeholder — else the budget is a lie.
- **Single-turn vs. the agent loop drift.** Calling `ModelAccess::turn` with no tools must genuinely
  return content on the first turn (a model that *only* emits tool calls would loop forever in the agent,
  but here there is no loop — a `turn` that returns `calls` with empty `content` must be handled: treat
  it as "no completion", surface a clear error, never hang). Name this in the adapter.
- **"Configured" resolution latency.** Resolving `agent.config` on every `rules.run` is one small record
  read; fine for v1. If rule runs get hot, cache the resolved model per (ws, config-rev) — a follow-up,
  not v1.

## Open questions

- **A `rules.ai` sub-capability?** Should calling `ai.*` inside a rule need a distinct grant
  (`mcp:rules.ai:call`) beside `mcp:rules.run:call`, so an admin can allow "run rules" without "spend
  model budget from rules"? **Proposal: yes, add it** — model spend is a distinct authority worth
  gating, it mirrors how `agent.invoke` is its own cap, and it is cheap (one check at the first `ai.*`
  call). Default-grant it to the same role that gets `rules.run` so nothing breaks; an admin can revoke
  it to make a workspace's rules data-only.
- **Where does the model resolve — reuse the agent's resolver or a rules-local one?** Reuse the agent's
  `agent.config` → `ModelAccess` resolution verbatim (a shared helper), or a small rules-local resolve?
  **Proposal: share the agent's resolver** (single source of truth for "the workspace's model"); if the
  agent's resolver isn't cleanly callable, extract the minimal shared helper rather than forking it.
- **Per-run vs. per-turn idempotency key.** `ModelAccess::turn` wants an idempotency key (replay-safe).
  A rule run has no durable job today; derive the key from `(ws, rule_id|body-hash, run-ts)`?
  **Proposal: yes** — deterministic per run so a re-run replays cleanly through the gateway cache;
  document that a rule run is not itself durable (no resume), only the model call is replay-cached.
- **What does `ai.embed` do?** The `AiSeam::embed` default errors "not supported". Does `ModelAccess`
  expose embeddings? **Proposal: leave `ai.embed` erroring for v1** (the agent core doesn't need
  embeddings yet either); wire it when an embedding provider surface exists — named follow-up.
- **Honest UI copy in the workbench.** The rules workbench (`rules-workbench-scope.md`) should show, when
  a workspace has no configured model, that `ai.*` will error — mirroring the Agent tab's honesty note.
  **Proposal: a small "AI: not configured / configured (model)" indicator** derived from `agent.config`,
  so a rule author knows before running.

## Related

- `rules/rules-engine-scope.md` (the shipped engine + the `AiSeam` / `RuleModel` seam this fills;
  `crates/rules/src/verbs/ai.rs`, `crates/host/src/rules/seam.rs`, `crates/host/src/rules/mod.rs`
  where `DisabledModel` is hardcoded today), `rules/rule-chains-scope.md`.
- `agent/agent-catalog-scope.md` (the catalog + `agent.config` selection this resolution reads — the
  pick that decides which model a workspace's rules use), `agent/default-agent-wiring-scope.md`
  (the in-house model + boot wiring, and the ai-gateway-provider-adapter dependency this shares),
  `agent/agent-scope.md` (the `ModelAccess` seam + the agent loop this deliberately does NOT reuse for a
  rule).
- `ai-gateway/…` (the `Provider` seam + `AiGateway` + the deterministic `MockProvider` tests drive;
  `role/ai-gateway/src/bridge.rs` adapts `AiGateway` to `lb_host::ModelAccess`).
- `rules/rules-workbench-scope.md` (the workbench that should carry the honest "AI configured?" copy).
- README `§6.16` (shared AI agents / model access), `§6.14`/`§6.15` (gateway), `§6.7` (secrets),
  `§7` (tenancy), `§3` (rules 1/5/6/9).
- Code the build will touch: `crates/host/src/rules/model.rs` (new — `AgentRuleModel`),
  `crates/host/src/rules/mod.rs` (resolve the model on the `rules.run` bridge; retire the hardcoded
  `DisabledModel` default), `crates/host/src/rules/seam.rs` (`RuleModel` trait — unchanged, consumed),
  and the agent-side `agent.config` → `ModelAccess` resolver (shared).

## Skill doc

The `ai.*`-in-rules surface is agent-/API-drivable (a rule calling `ai.ask` over `rules.run`), so the
implementing session **extends the existing rules skill** (`skills/<rules-skill>/SKILL.md`) with a
grounded run that calls `ai.ask` / `ai.complete` in a rule against a configured model — not a new skill
(the drivable verb `rules.run` is unchanged; only its model behavior is new). If no rules skill exists
yet, the session creates `skills/rules/SKILL.md` covering write → run (with `ai.*`) → save. Grounded in
a live run per `ABOUT-DOCS.md`.
