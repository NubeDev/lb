# Agent scope — catalog "test" button + DB-sealed per-workspace model key

Status: scope (the ask). Promotes to `public/agent/agent.md` once shipped. Builds on the shipped
`agent-catalog-scope.md`.

Two additions to the shipped Agent-catalog surface, driven by two user asks:

1. **A "Test" button on a catalog entry** — send a canned prompt ("who are you, and what are your
   skills?") to the selected/target definition and show the reply, so a workspace admin can confirm the
   agent **has its real Lazybones context**: that it knows it can reach **MCP tools**, the **ACP**
   external-agent bridge, and its **granted skills**. The test is not a bare model ping — it drives the
   *same* context assembly a real run does (system prompt + reachable-tool menu + granted-skill catalog)
   so the reply proves the context was assembled and delivered, not just that a model answered.
2. **Let the user set the model key** — move the endpoint's key from "a node-env var name only" to a
   **DB-sealed per-workspace secret** (`lb-secrets`), so an admin can set/rotate the actual key in the
   UI. The catalog/`agent.config` records stay **names-only** (they reference a secret *path*, never the
   value); the value is written through the shipped sealed `secret.set` path and resolved at model-call
   time from `lb-secrets` — with the node env as a fallback so nothing that works today breaks.

## Goals

- **Test a definition end to end.** A gated `agent.def.test {id?}` verb runs a **single-turn** invoke of
  the target definition's runtime + model with the run's **real context assembled** (the shipped system
  prompt, `reachable_tools` menu, and `render_catalog` granted-skill list), using a canned self-describe
  prompt, and returns `{ answer, runtime, model, context: {tools, skills}, ok }`. The UI adds a **Test**
  button per entry (and on the active pick) that shows the reply inline.
- **Prove context, not just connectivity.** The test must exercise the *context path*: the returned
  payload names how many MCP tools + which skills were in scope, so even against the deterministic
  mock provider the admin sees "the agent was told it has N tools and these skills" — and against a real
  provider, the model's answer reflects them. This is the "does it know it has MCP/ACP/skills" check.
- **Set/rotate the model key from the UI, sealed in the DB.** An admin can write the actual API key for
  a definition's endpoint; it is stored **only** through `lb-secrets` (sealed, owner-stamped,
  workspace-scoped, never logged), addressed by a **secret path** the endpoint references. The key value
  never lands in an `agent_definition` / `agent.config` record, never in the manifest, never in a log.
- **Resolve the key from the secret, env as fallback.** At model-call time the endpoint's key resolves
  **secret path → `lb-secrets` value → else the node env var** (the current behavior). A workspace that
  set a sealed key uses it; one that didn't keeps working off the node env. Symmetric, no `if cloud`.

## Non-goals

- **Not the multi-turn agent loop as "the test".** The test is one turn (context + canned prompt →
  answer). It is not a durable run, has no suspend/resume, and does not persist a session/transcript —
  it is a diagnostic, not an `agent.invoke`. (It *may* internally reuse the invoke path with a
  step-ceiling of 1; that is an implementation choice, stated in the plan, not a new engine.)
- **Not a real model guarantee.** Same limit the agent core + rules carry: **no real `Provider` adapter
  exists yet** (only the sanctioned `MockProvider`; real adapters are ai-gateway-scope-deferred). The
  test proves the *pipe + context assembly + key resolution* for real against `AiGateway<MockProvider>`;
  the model's *understanding* of the context is only truly demonstrated once a real adapter lands. The
  UI must say so (an honest "responding via the configured provider" note), never imply a real LLM
  answered when the mock did.
- **Not raw secrets in the catalog record.** The `agent_definition` / `agent.config` shapes stay
  names-only. This scope adds an optional **secret-path reference** (a name), never a value, to the
  endpoint — the value lives in `lb-secrets`. `builtin.*` definitions stay read-only; a built-in's key
  is still the node-env name it ships with (a built-in cannot own a workspace secret path — see Risks).
- **Not a new secrets engine.** Reuse the shipped `lb-secrets` + `secret.set/get/delete` verbs wholesale
  (workspace-scoped, sealed, owner-stamped, visibility-gated). This scope *consumes* them; it does not
  add a persistence layer (rule 2).
- **Not per-user.** The test and the sealed key are **workspace** settings (like `agent.config`), never
  per-member. The test's *reachable tools/skills* are resolved for the calling admin's grants (the same
  wall a real run uses).

## Intent / approach

**Two small, gated additions that reuse shipped seams; no new machinery.**

### 1. `agent.def.test` — the context-proving diagnostic

Add one verb, `agent.def.test {id?}` (member-or-admin, gated by `mcp:agent.def.test:call`). It:
1. Resolves the target definition (the given `id`, or the active `agent.config` pick if omitted) →
   its `(runtime, model_endpoint)`.
2. Assembles the **real run context** for the caller's workspace exactly as `run.rs` does at run start:
   the shipped **system prompt**, the **`reachable_tools`** menu (the MCP/tool surface the agent may
   call — this is the "it has MCP/ACP access" proof, since ACP external agents are reached as tools),
   and the **`render_catalog`** granted-skill list (the "it has skills" proof). All three are already
   grant- + workspace-gated — the test inherits the wall, it does not widen it.
3. Runs **one** model turn with a canned prompt (`"Who are you, and what tools and skills do you have
   access to?"`) over the resolved model (`ModelAccess`, today `AiGateway<MockProvider>`), with a
   **step ceiling of 1** (no tool execution, no loop — the model answers from the injected context).
4. Returns `{ answer, runtime, model, context: { tool_count, tools:[names…], skills:[names…] },
   provider_configured: bool, ok }`. The UI shows the answer + a compact "context: N tools, M skills"
   line, so the admin sees the agent *was given* its Lazybones context even if the mock's answer is
   canned.

**Why one turn with real context, not a bare ping (rejected alt):** a ping proves the endpoint resolves;
it does NOT prove the agent knows what it is. The user's ask is specifically "make sure it has context on
Lazybones / knows it has MCP/ACP/skills". Assembling the real context and returning what was assembled is
the only thing that answers that against the mock **and** the real provider. A bare reachability check is
kept as the cheap failure signal (if the endpoint won't resolve, the test fails fast before spending a
turn), not as the whole test.

### 2. DB-sealed per-workspace key — reference, don't inline

- The endpoint gains an optional **`api_key_secret`** field: a **secret path** (a name, e.g.
  `agent/zaicoding-key`) into `lb-secrets`, beside the existing `api_key_env` (env-var name). Both are
  names; neither is a value. Precedence at model-call time: **`api_key_secret` (via `lb_secrets::get`) →
  `api_key_env` (node env) → unset**.
- The UI's custom-definition editor gains a **"Model key"** field. Entering a value calls the shipped
  **`secret.set { path, value, visibility: Private }`** (sealed, owner-stamped, workspace-scoped) and
  stores only the resulting **path** on the definition. The value is never echoed back (write-only in the
  UI; a "key is set ✓ / rotate" affordance, never a readback) — the shipped `secret.get` is not called
  from the browser for display.
- **Resolution seam:** the one place that turns an endpoint into a model today (the external-agent
  driver env handoff + the in-house model build) resolves the key **secret → env**. This is the single
  change to the key path; everything upstream stays names-only.

**Rejected alternatives.**
- *(a) Store the key in the `agent_definition` record (a plain column).* Rejected hard — breaks
  names-only (rule 5 / §6.7): a secret in a normal record is logged, replicated, and readable by any
  record read. `lb-secrets` exists precisely to avoid this.
- *(b) Keep names-only + tell users to set node env vars.* Rejected per the user's explicit call
  ("store in db as a secret, better long term") — an operator-only env var is not a workspace-admin
  self-serve flow; the sealed per-workspace secret is.
- *(c) Make the test the full multi-turn agent run.* Rejected — a diagnostic must be bounded, cheap, and
  side-effect-free; a full run persists a session and can call tools. One turn with injected context is
  the right altitude.

## How it fits the core

- **Tenancy / isolation:** the sealed key is a **workspace-scoped** `lb-secrets` entry (the hard wall —
  `secret.set/get` are ws-scoped + owner-stamped); a ws-B admin can never read/rotate ws-A's key. The
  test assembles context for the **caller's** workspace + grants (`reachable_tools`/`render_catalog` are
  already ws- + grant-gated) — it cannot surface another tenant's tools, skills, or key. Both verbs
  authorize workspace-first via `authorize_tool`.
- **Capabilities (the deny path):** `agent.def.test` gates on `mcp:agent.def.test:call` (opaque deny);
  proposed default-grant to the same members who hold `agent.def.list` (reading + testing the catalog is
  a read-ish diagnostic — *open question* on whether test needs its own admin-tier gate because it
  **spends model budget**, proposal below). Setting the key uses the shipped `mcp:secret.set:call` +
  `secret:<path>:write` (gate 2) + owner-only overwrite (gate 3) — an admin-tier act, already gated by
  the secrets surface. Referencing a secret path on a definition write is still `mcp:agent.def.create/
  update:call` (custom-only; a `builtin.*` write stays `Reserved → BadInput`).
- **Placement:** `either`. Both run on every node (symmetric); which model answers + whether a sealed
  key exists is config (a node with no provider adapter runs the test against the mock; a workspace with
  no sealed key resolves the env). No `if cloud`.
- **MCP surface** (API shape §6.1):
  - **New verb:** `agent.def.test {id?}` → `{ answer, runtime, model, context, provider_configured, ok }`
    — a **single action**, not CRUD (one responsibility file `defs/test.rs`). Gateway route `POST
    /agent/defs/:id/test` (and `POST /agent/defs/test` for the active pick).
  - **Consumes** the shipped `secret.set/get/delete` (the key), `reachable_tools` + `render_catalog`
    (the context), and the shipped `agent.config` (the active-pick default when `id` is omitted).
  - **Live feed: N/A** — the test is one request/response with a bounded answer; there is no stream
    (a single turn, not a durable run — state-vs-motion §3 rule 3). **Batch: N/A** — one definition per
    test, always bounded (one turn, step-ceiling 1), so no job. **CRUD/get-list: unchanged** — the
    catalog's five verbs ship; this adds one *action* verb, not a resource.
- **Data (SurrealDB):** no new table. The key is an `lb-secrets` entry (its own sealed store surface, one
  datastore — SurrealDB-backed, not a new layer). The definition gains one **names-only** optional field
  (`api_key_secret`, a path) — SCHEMAFULL-compatible, LWW like the rest.
- **Bus (Zenoh): N/A** — request/response; no motion.
- **Sync / authority:** the sealed key is a node-local workspace secret (offline-safe, LWW via the
  secrets store); the test is a stateless diagnostic (nothing persisted). No new authority.
- **Secrets:** this is the crux. The value flows **only** through the shipped sealed `secret.set`
  (owner-stamped, `Private` by default, never logged); the record carries a **path**, never a value; the
  browser never reads the value back (write-only + "set ✓/rotate"); resolution reads it server-side at
  model-call time. Redaction/never-log guarantees are inherited from `lb-secrets`, not re-invented. The
  test's returned `answer` must be scanned to ensure a mis-behaving mock/model cannot echo the injected
  key (it isn't injected into the prompt — the key goes to the *provider transport*, not the context —
  so structurally the answer can't contain it; assert this in a test).
- **No fake backend (rule 9 / testing §0):** real store (`mem://`), real caps, real gateway, real
  `lb-secrets`, real `reachable_tools`/`render_catalog`. The **model is the one true external**, behind
  the sanctioned `Provider` trait via `AiGateway<MockProvider>` — the test drives the real invoke path,
  not a fake. No `*.fake.ts`.
- **State vs motion:** the sealed key is state (`lb-secrets`); the test is a bounded action, not motion.
- **One responsibility per file (FILE-LAYOUT):** `defs/test.rs` (the verb), the key-resolution helper in
  the one place endpoints become models (extended, not duplicated), and small UI files
  (`AgentTestButton.tsx` / a `useAgentTest.ts`, the editor's "Model key" field). No file gains a second
  job.
- **SDK/WIT impact:** none — host + role wiring + UI; no plugin-boundary change.

## Example flow

1. Ada (admin) opens Settings → Agent, picks **"In-house — Z.AI GLM-4.6"** (active).
2. She clicks **Test** on it. The UI calls `agent.def.test {}` (no id → the active pick). The host
   assembles her workspace's real context — the system prompt, the `reachable_tools` menu (e.g. 23 MCP
   tools incl. the ACP external-agent bridge), and her `render_catalog` granted skills (e.g. `lb-cli`,
   `query`, `agent`) — and runs one model turn with "Who are you, and what tools and skills do you have
   access to?".
3. The reply shows inline, plus **"context: 23 tools, 3 skills · responding via the configured
   provider"**. Against the mock the answer is deterministic canned text, but the context line proves the
   agent *was given* its Lazybones context; against a real provider the answer names the tools/skills.
4. Ada edits a custom definition and enters a **Model key** value. The UI calls `secret.set { path:
   "agent/zaicoding-key", value: "<key>", visibility: "Private" }` (sealed) and saves the definition with
   `api_key_secret: "agent/zaicoding-key"` (a path, no value). The field shows **"key set ✓ · rotate"**;
   the value is never read back.
5. Next test/run of that definition resolves the key **secret → env**: `lb_secrets::get(ws,
   "agent/zaicoding-key")` supplies it; the value never touches a record or a log.
6. A ws-B admin `agent.def.test`/`secret.get`s Ada's key → denied / not found: the sealed key + the
   test context are workspace-walled.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all rule-9 (real store `mem://` / caps /
gateway / `lb-secrets` / real context assembly; the model is the sanctioned external via
`AiGateway<MockProvider>` — no `*.fake.ts`):

- **Capability-deny (§2.1):** `agent.def.test` without `mcp:agent.def.test:call` → opaque `Denied`;
  setting a key without `mcp:secret.set:call` / `secret:<path>:write` → denied (the shipped secrets
  gate). A member without the test cap cannot spend model budget via the test.
- **Test proves context (the headline):** `agent.def.test` returns a non-empty `context.tools` +
  `context.skills` reflecting the caller's **real** grants — seed a workspace with N granted skills + a
  known tool surface and assert the test's `context` names them (proving the real assembly ran, not a
  stub). Against the mock the `answer` is the deterministic reply; assert `provider_configured` is honest.
- **Test inherits the wall (§2.2):** a caller with fewer grants sees fewer tools/skills in `context`;
  ws-B's test never lists ws-A's tools/skills. No context widening.
- **Sealed key, names-only invariant:** after `secret.set` + a definition write referencing the path,
  assert the `agent_definition` / `agent.config` records contain **no key value** (only the path), and
  the test's returned `answer` never contains the key (it goes to transport, not the prompt) — the
  names-only / no-secret-in-record proof, mirrored from `agent_config_test`.
- **Key resolution precedence:** with a sealed secret set, the model call resolves the **secret** value;
  with only a node env var, it resolves the **env**; with neither, the endpoint has no key (a clear
  unconfigured path, not a panic). Cover all three.
- **Workspace-isolation of the key (§2.2):** a ws-B admin cannot `secret.get`/rotate ws-A's
  `agent/zaicoding-key`; a ws-A rotation does not move ws-B.
- **Built-in stays read-only + node-env:** a `builtin.*` definition cannot be given a workspace secret
  path (its write is `Reserved → BadInput`); its key stays the node-env name it ships with — assert a
  test of a built-in resolves the env (or is honestly unconfigured), never a per-workspace secret it
  can't own.
- **Bounded diagnostic:** the test runs exactly one turn (step-ceiling 1), executes **no** tools, and
  persists **no** session/transcript — assert no durable run record is created.
- **UI (Vitest against a real seeded gateway, no `*.fake.ts`):** the Test button shows the reply + the
  context line; the editor's Model-key field writes a sealed secret and shows "set ✓" without reading it
  back; a built-in has no Model-key field. Drive the real verbs over the real gateway.

## Risks & hard problems

- **Names-only is the invariant most at risk.** The whole point of `lb-secrets` is that a key value never
  enters a normal record or a log. The build must route the value **only** through `secret.set`, store
  **only** the path, and never read it back to the browser. A regression that put the value on the
  definition record (or logged it) is a serious §6.7 finding — the test suite asserts the record + the
  test answer are value-free.
- **The provider adapter is still the gate.** The test proves context assembly + key resolution + the
  pipe against the mock; the model's *understanding* of "you have MCP/ACP/skills" is only truly shown
  with a real adapter (the same dependency `agent-catalog` / `default-agent-wiring` / `rules-ai-wiring`
  name). The UI copy must be honest ("responding via the configured provider"); the **context line** is
  what makes the test meaningful pre-adapter — lean on it, don't over-promise the answer.
- **A built-in can't own a workspace secret path.** Built-ins are node-level + read-only; a per-workspace
  sealed key belongs to a **custom** definition (or the active-pick's `agent.config`, which is
  workspace-scoped). Decide cleanly: a workspace that wants a sealed key for an in-house model **clones**
  the built-in into a custom definition (or sets the key on `agent.config`), rather than mutating the
  built-in. State this so the UI guides it (the "New definition" path, or a key field on the active pick).
- **Budget + abuse on Test.** `agent.def.test` spends a model turn. A member spamming Test is real spend.
  Gate it (open question below) and consider a light per-workspace rate limit (reuse the meter posture)
  — a diagnostic that costs money needs a cap.
- **Key echo.** Structurally the key goes to the provider transport, not the prompt/context, so the
  answer can't contain it — but a future change that put the endpoint into the prompt would leak it.
  Assert the answer is key-free and keep the key out of the context payload by construction.

## Open questions

- **Does `agent.def.test` need its own admin-tier cap (or a rate limit) because it spends budget?**
  **Proposal: its own cap `mcp:agent.def.test:call`, default-granted to admins** (not every member),
  plus reuse the AI-meter posture for a light per-workspace ceiling — testing is cheap but non-zero, and
  "who may spend model budget" is a distinct authority (mirrors the `rules.ai` sub-cap proposal).
- **Where does key resolution live — a shared `resolve_endpoint_key(ws, endpoint)` helper?** Both the
  in-house model build and the external-agent env handoff need the same **secret → env** precedence.
  **Proposal: one shared host helper** (single source of truth), consumed by both the test and real runs,
  so the test resolves the key exactly as a run does (no divergence between "test passes" and "run works").
- **Canned prompt: fixed or admin-editable?** **Proposal: a fixed, well-crafted self-describe prompt for
  v1** ("who are you, what tools/skills do you have"); an editable prompt is additive later. A fixed
  prompt keeps the test comparable across definitions.
- **Should the test return the raw context (tool/skill names) or just counts?** **Proposal: names +
  counts** — names make "it has MCP/ACP/skills" concrete for the admin (the user's actual ask), and they
  carry no secret (a tool/skill name is not sensitive within the workspace). Bound the list length.
- **Key on the active pick (`agent.config`) vs. only on custom definitions?** **Proposal: allow a sealed
  key on `agent.config`** (the workspace's active selection is workspace-scoped and can own a secret
  path), so an admin can key the in-house built-in they picked without cloning it — resolves the
  built-in-can't-own-a-secret tension cleanly.

## Skill doc

Both surfaces are agent-/API-drivable (`agent.def.test` over the gateway; the sealed-key set via
`secret.set` + a definition write), so the implementing session **extends the agent catalog's skill**
(the same `skills/agent/SKILL.md` the `agent-catalog-scope.md` names): a grounded run that tests a
definition (reads back the context line) and sets/rotates a sealed model key, then watches a run use it.
Not a new skill — the drivable surface is the same catalog, with one new action verb + a secret handoff.
Grounded in a live run per `ABOUT-DOCS.md`.

## Related

- `agent/agent-catalog-scope.md` (the shipped catalog this extends — the definitions, `agent.config`
  pick, `agent.def.*` verbs, and the names-only endpoint this adds `api_key_secret` + a test to;
  `crates/host/src/agent/defs/*`).
- `agent/agent-scope.md` / `agent-run` (the run-start **context assembly** the test reuses:
  `crates/host/src/agent/run.rs` system prompt, `reachable_tools`, `render_catalog`), and the
  `ModelAccess` seam the single test turn calls.
- `agent/default-agent-wiring-scope.md` / `ai-gateway/…` (the `Provider` adapter dependency the honest
  test-copy waits on; `MockProvider` the test drives against today).
- `secrets/…` (the shipped `lb-secrets` + `secret.set/get/delete` verbs the sealed key reuses —
  `crates/secrets`, `crates/host/src/host_tools/secret/*`; owner-stamped, ws-scoped, visibility-gated).
- `rules/rules-ai-wiring-scope.md` (the sibling "wire `ai.*` to the real model" scope — shares the
  provider-adapter dependency and the "context/spend is a distinct authority" cap question).
- README `§6.16` (shared AI agents / model access), `§6.7` (secrets), `§6.14`/`§6.15` (gateway),
  `§7` (tenancy), `§3` (rules 1/2/5/6/9).
- Code the build will touch: `crates/host/src/agent/defs/test.rs` (new verb), `defs/model.rs`
  (`api_key_secret` names-only field), the shared `resolve_endpoint_key` helper (secret→env, consumed by
  the test + the in-house/external model build), `defs/mod.rs` + `agent/tool.rs` (dispatch),
  `role/gateway/src/routes/agent_defs.rs` (`POST /agent/defs[/:id]/test`),
  `ui/src/features/settings/agent/*` (Test button + Model-key field), `ui/src/lib/agent/agentDef.api.ts`
  (the test call + a sealed-key set via the secrets client).
