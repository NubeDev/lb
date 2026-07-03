# Agent scope — the active agent is the one implicit agent everywhere

Status: scope (the ask). Promotes to `public/agent/agent.md` once shipped.

A workspace picks ONE agent in the catalog ("Use") — and from that moment no surface should ever
ask the user to pick again. Today that promise is broken three ways: the **channel** composer
auto-sends an explicit `runtime:"default"` that outranks the pick; **rules** `ai.complete` only
rides the in-house model (always `UnconfiguredModel` — no real provider adapter exists), so an
active *external* agent is invisible to rules and every call returns "AI not configured for
rules"; and the **dashboard AI widget** calls an `agent_invoke` command that was never wired on
any transport ("unknown command: agent_invoke"). This scope makes the active pick the single
implicit agent for every consumer — channels, rules, the AI widget, and `agent.invoke` — and
lands the one missing primitive underneath them all: a real (OpenAI-compatible) provider adapter
so the active definition's `model_endpoint` is actually consumed per workspace.

## Goals

- **One resolution, zero selection.** Every agent consumer that doesn't receive an *explicit
  user override* resolves through the shipped seam (`resolve_effective_runtime`: explicit →
  `agent.config.default_runtime` → registry default) and therefore runs the workspace's active
  pick. No surface pre-fills or silently sends an explicit runtime.
- **A real provider adapter (the unblocking primitive).** One `Provider` implementation for the
  OpenAI-compatible chat-completions shape (covers Z.AI `zaicoding` and any `base_url` that
  speaks it), plugged into `build_in_house_model`'s match (today every provider falls to
  `UnconfiguredModel` — `rust/node/src/agent.rs:84-97`). This makes the in-house `default`
  runtime honest instead of a de-facto mock, and unlocks the named catalog follow-up:
  **per-workspace endpoint consumption** (the active definition's `model_endpoint` → a live
  `ModelAccess`), which rules and the in-house loop then ride.
- **Rules ride the active agent's model.** `resolve_rule_model` resolves the *workspace's*
  model — the active definition's `model_endpoint` through the adapter — instead of only the
  node-level in-house model. A workspace whose active agent is external (e.g. Open Interpreter
  over GLM-4.6) gets working `ai.complete` from the same endpoint; a workspace with no active
  pick keeps the honest `DisabledModel` answer.
- **Channels default to the active pick.** The `/agent` composer stops auto-selecting the
  registry default; the runtime dropdown's default entry reads "Active — <definition label>"
  and sends **no** `runtime` field, so the shipped backend fallback does its job. Picking a
  different entry remains an explicit per-message override (kept, but no longer required).
- **The AI widget works.** `agent_invoke` is wired end to end: a gateway `POST /agent/invoke`
  route over the existing `lb_host::invoke`, the matching `agent_invoke` case in the browser
  IPC seam, and the Tauri shell command — so the genui author flow (already built to spec)
  runs, resolving the active agent because it passes no runtime.
- **"Active" is first-class, not derived.** `agent.config` gains an optional
  `active_definition` id written by the pick (alongside the copied fields, back-compat), and
  the definition-resolution helper inlined in `agent.def.test` (`defs/test.rs::resolve_target`)
  is promoted to one shared `agent/resolve_definition.rs` — the single answer to "which
  definition is active" for the UI badge, rules, and the test button.

## Non-goals

- **Not a second resolution seam.** `resolve_effective_runtime` stays the only runtime ladder;
  this scope removes the callers that defeat it, it does not add precedence rules.
- **Not streaming/budget/token-motion work in ai-gateway.** The adapter is the minimal
  request/response `Provider` (turn in, text + usage out); mid-stream token events, provider
  fallback chains, and the served OpenAI face for external agents (`model-routing-scope.md` #4)
  stay deferred.
- **Not per-rule / per-widget / per-message agent *configuration*.** Consumers get the active
  agent implicitly; the only explicit override that exists is the one that already ships
  (`runtime` on a channel post / `agent.invoke` arg). No new per-surface selection UI —
  removing selection is the point.
- **Not a rules-drives-the-full-agent-loop change.** `ai.complete` stays a single model turn
  (no tools, no durable run) — rules need a completion, not an agent. Driving a full external
  ACP subprocess per `ai.complete` call was considered and rejected (latency, budget, and a
  rule's blocking thread are wrong for a subprocess loop); the external *agent* stays the
  channel/invoke surface, while rules share only its **model endpoint**.
- **Not new caps or tables.** `POST /agent/invoke` rides the existing `mcp:agent.invoke:call`
  gate; `active_definition` is one additive field on the existing `workspace_agent_config`
  record.

## Intent / approach

**Everything already funnels through one seam — finish the last mile at each consumer, and give
the seam a real model to hand back.** The investigation (2026-07-03) found the backend ladder
correct and the failures peripheral:

1. **The adapter (the root unblock).** `role/ai-gateway` gets `providers/openai_compat.rs`: one
   `Provider` impl speaking the OpenAI chat-completions shape against a configurable `base_url`,
   key from the sealed-secret→env resolution that already ships (`agent/resolve_key.rs`).
   `build_in_house_model` matches `zaicoding` (and a generic `openai-compat`) to it. Rejected:
   waiting for a full multi-provider ai-gateway build — the user-visible system is dead until
   *some* adapter exists, and one OpenAI-compatible impl covers every endpoint currently in the
   catalog manifest.
2. **Per-workspace model resolution.** A small `resolve_workspace_model(node, ws)` beside
   `resolve_effective_runtime`: active definition (via the promoted `resolve_definition`) →
   its `model_endpoint` → `AiGateway<OpenAiCompat>` (memoized per ws+endpoint), falling back to
   the node-level in-house model, else `UnconfiguredModel`. Rules consume it; the in-house
   loop's per-run model override is the same call. This is exactly the "reference, not copy"
   follow-up the catalog scope named — landed here because three consumers now depend on
   "active" being crisp.
3. **Rules.** `resolve_rule_model` swaps its two-gate check for `resolve_workspace_model`. The
   `DisabledModel` honest-error path stays for the truly unconfigured workspace. No change to
   the rhai surface, fence, or meter.
4. **Channels (UI-only fix).** `RuntimeArg` stops auto-preselecting the registry default
   (`RuntimeArg.tsx:26-28`); the default option is "Active — <label>" (label from
   `agent.def.list` + `active_definition`, or the resolved runtime id) and maps to *omitting*
   `runtime`. `agent.runtimes` gains one additive `workspace_default` field so the dropdown can
   label without a second fetch. The stale "sent verbatim or omitted — identical" comment goes.
5. **The widget transport.** `routes/agent_invoke.rs` (`POST /agent/invoke` →
   `lb_host::invoke`, which already self-gates), registered in `server.rs`; an `agent_invoke`
   case in `ui/src/lib/ipc/http.ts`; the command registered in `ui/src-tauri/src/desktop.rs`.
   The genui flow above it is already built to its scope and needs no change.

## How it fits the core

- **Tenancy / isolation:** `active_definition` + the resolved model live on the
  workspace-scoped `workspace_agent_config` record (the hard wall holds). `POST /agent/invoke`
  authorizes workspace-first through the shipped `agent/authorize.rs` path. A ws-B rule/widget
  can never resolve ws-A's endpoint or key.
- **Capabilities:** no new caps. The invoke route gates on the existing `mcp:agent.invoke:call`
  (deny = opaque 403, same as `/mcp/call`); rules stay under `rules.run`'s gate; the pick stays
  admin-gated `mcp:agent.config.set:call`. Deny path tested per consumer.
- **Placement:** either. The adapter is config (`base_url` + key name) — a hub uses a pooled
  provider, an edge a local OpenAI-compatible server (ollama/llama.cpp) or stays unconfigured.
  No `if cloud`.
- **MCP surface:** **no new verbs.** One new gateway route (`POST /agent/invoke`) exposing the
  existing routed verb to browsers (mirrors `/mcp/call`'s pattern); one additive read field on
  `agent.runtimes` (`workspace_default`); one additive field on `agent.config`
  (`active_definition`). CRUD/live-feed/batch: N/A — no new resource; the run's feed stays the
  shipped `agent.watch` SSE (the genui flow already streams over it).
- **Data (SurrealDB):** one additive optional field on `workspace_agent_config`. State only.
- **Bus (Zenoh):** unchanged — the route calls the same `lb_host::invoke` the routed
  `agent/invoke` queryable serves.
- **Sync / authority:** unchanged; `agent.config` UPSERT/LWW semantics carry the new field.
- **Secrets:** the adapter resolves the key through the shipped sealed-secret → env-NAME
  precedence (`resolve_endpoint_key_host`); never a value in a record or log.
- **No fake backend (rule 9):** store/bus/caps/gateway/loop/rules all real. The ONE sanctioned
  fake is the provider HTTP: adapter tests run `AiGateway<OpenAiCompat>` against a local
  scripted HTTP server serving canned chat-completions responses (a true external, behind the
  `Provider` trait, in one named test file); everything above the adapter keeps using the real
  `AiGateway` (over `MockProvider` where a live endpoint isn't wanted).
- **State vs motion / stateless extensions / symmetric nodes:** untouched; N/A beyond the above.
- **One responsibility per file:** `providers/openai_compat.rs`, `agent/resolve_definition.rs`,
  `agent/resolve_model.rs`, `routes/agent_invoke.rs` — one verb/seam each.
- **SDK/WIT impact:** none.
- **Skill doc:** yes — extend `docs/skills/agent/SKILL.md`: pick a definition, then drive it
  implicitly from a channel (`/agent` with no dropdown touch), a rule (`ai.complete`), and the
  dashboard AI widget, grounded in a live run.

## Example flow

1. Ada opens Settings → Agent and clicks **Use** on "Open Interpreter — Z.AI GLM-4.6". The pick
   writes `agent.config { default_runtime, model_endpoint, active_definition }`.
2. In a channel she types `/agent summarize today's boiler alerts` and hits send without
   touching the dropdown (it reads "Active — Open Interpreter — Z.AI GLM-4.6"). The post
   carries **no** `runtime`; the worker resolves the active pick and the run streams back.
3. A rule fires: `ai.complete("summarize", grid)`. `resolve_rule_model` resolves the active
   definition's endpoint → the OpenAI-compat adapter → GLM-4.6 answers; the meter charges real
   usage. No selection anywhere in the rule.
4. On a dashboard she adds a `genui` cell and prompts "a stat tile of open alerts".
   `agent_invoke` → `POST /agent/invoke` → `lb_host::invoke` under her principal → the active
   agent authors the widget; the preview streams over `agent.watch`.
5. Bob's workspace has no active pick and no node-level model: the channel run and the widget
   return the honest "unconfigured" answer; `ai.complete` returns "AI not configured for
   rules". Nothing pretends.
6. A rule in ws-B fires while Ada's ws-A pick exists: ws-B resolves **its own** config —
   `DisabledModel` — never ws-A's endpoint or key. The wall held.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), rule 9 throughout:

- **Capability-deny:** `POST /agent/invoke` without `mcp:agent.invoke:call` → opaque deny
  (route-level, mirroring the `/mcp/call` 403 tests); rules and channel paths re-run their
  shipped deny tests unchanged (regression).
- **Workspace-isolation:** ws-B's `resolve_workspace_model` never yields ws-A's endpoint; a
  ws-B `POST /agent/invoke` cannot start a run in ws-A (routed key `ws/{caller.ws}/…`
  regression).
- **The headline (per consumer):**
  - channel: a posted `kind:"agent"` item with `runtime` **omitted** runs the definition the
    workspace picked (extend `agent_default_runtime_test`); a UI gateway test drives `/agent`
    through the palette untouched and asserts the payload carries no `runtime` field.
  - rules: with an active definition over `AiGateway<MockProvider>` installed per-workspace,
    `ai.complete("summarize", grid)` returns the model answer (today: "AI not configured for
    rules"); with no pick, the honest `DisabledModel` error.
  - widget: a gateway test posts `POST /agent/invoke` and receives `{job}`; a UI gateway test
    drives the genui author flow to a streamed preview (the flow's existing choreography,
    finally reachable).
- **Adapter:** `AiGateway<OpenAiCompat>` against a local scripted chat-completions HTTP server
  (happy turn, API error, key-missing → honest unconfigured); the sealed-secret→env key
  precedence regression.
- **Unconfigured→configured swap:** per-workspace — same invoke answers "unconfigured" then
  runs after the pick (the boot-registry swap test, lifted to workspace altitude).
- **Offline/sync:** `agent.config` double-delivery UPSERT keeps `active_definition` idempotent
  (LWW regression).
- **Boot smoke (opt-in, `#[ignore]`):** a node with `ZAI_API_KEY` set, pick a GLM built-in,
  drive channel + rule + widget live — the grounding for the SKILL.md update.

## Risks & hard problems

- **The adapter is now load-bearing for three surfaces.** A provider outage/misconfig surfaces
  in channels, rules, and widgets at once. Keep every failure honest and attributed ("model
  call failed: <provider>/<model>: …"), never a silent empty answer; rules must keep charging
  the meter only for real usage.
- **Rules latency/budget against a real model.** `ai.complete` blocks a rule thread on a
  network call. The shipped budget meter bounds spend; confirm a timeout bounds wall-clock
  (a hung provider must not wedge the rules executor).
- **Per-workspace model memoization.** Building `AiGateway` per call is wasteful; caching per
  (ws, endpoint) must invalidate on `agent.config.set` — get this wrong and a rotated key or
  changed pick keeps answering with the old model.
- **The channel dropdown regression class.** The bug was a UI default silently *widening* into
  an explicit arg. The gateway test that asserts "untouched palette ⇒ no `runtime` on the
  wire" is the guard; keep it registry-driven so a future arg widget can't reintroduce it.
- **Copy/reference drift.** `active_definition` referencing a definition whose fields were
  since edited (or deleted) must resolve sanely: re-resolve from the definition when present,
  fall back to the copied fields, never a panic — and the UI badge must reflect which one won.

## Open questions

- **Adapter home:** `role/ai-gateway/src/providers/openai_compat.rs` behind the existing
  `Provider` trait (proposal: yes — it's the gateway's job; the host only sees `ModelAccess`).
- **Memoization shape:** a `DashMap<(ws, endpoint-hash), Arc<dyn ModelAccess>>` on the node vs
  rebuild-per-run? Proposal: memoize with invalidation on `agent.config.set` (rules may call
  per row-batch; rebuild-per-call is measurable waste).
- **`workspace_default` on `agent.runtimes` vs the UI composing `agent.config.get` +
  `agent.def.list` it already fetches?** Proposal: the additive field — the dropdown is
  rendered in the channel composer where the settings queries aren't otherwise loaded.
- **Does the in-house loop consume the per-workspace endpoint in this scope,** or only rules
  (the loop keeps the node-level `LB_AGENT_MODEL_*` model)? Proposal: yes, same
  `resolve_workspace_model` call at run start — it's the catalog's named follow-up and the
  seam is being built anyway; keep node-level env as the fallback tier.

## Related

- The investigation this scope encodes (2026-07-03): channel `RuntimeArg` auto-preselect
  (`ui/src/lib/widgets/inputs/RuntimeArg.tsx`, `CommandPalette.tsx`), rules gates
  (`crates/host/src/rules/mod.rs::resolve_rule_model`, `node/src/agent.rs::build_in_house_model`),
  the unwired command (`ui/src/lib/ipc/http.ts` default arm, no gateway route, no Tauri cmd),
  the resolution seam (`crates/host/src/agent/resolve_default.rs`,
  `defs/test.rs::resolve_target`).
- `agent-catalog-scope.md` (the pick this makes first-class; names the copy→reference and
  per-workspace-endpoint follow-ups landed here), `default-agent-wiring-scope.md` (the
  in-house wiring + the adapter dependency it deferred),
  `agent-catalog-test-and-secrets-scope.md` (the sealed key the adapter resolves).
- `../rules/rules-ai-wiring-scope.md` (shipped seam this feeds a real model),
  `../genui/genui-scope.md` (the widget flow this makes reachable),
  `../channels/channels-agent-scope.md` (the worker whose fallback was right all along),
  `../external-agent/model-routing-scope.md` (#4 — still deferred),
  `../ai-gateway/ai-gateway-scope.md` (owns the `Provider` trait the adapter implements).
- Skills: `../../skills/agent/SKILL.md` (extend). README §6.16, §6.14/§6.15, §6.7, §7,
  §3 (rules 1/5/6/9).
- Code the build touches: `role/ai-gateway/src/providers/openai_compat.rs` (new),
  `node/src/agent.rs`, `crates/host/src/agent/{resolve_definition.rs (new),
  resolve_model.rs (new), resolve_default.rs, runtimes.rs, config/*}`,
  `crates/host/src/rules/mod.rs`, `role/gateway/src/routes/agent_invoke.rs` (new) +
  `server.rs`, `ui/src/lib/ipc/http.ts`, `ui/src-tauri/src/desktop.rs`,
  `ui/src/lib/widgets/inputs/RuntimeArg.tsx`, `ui/src/features/channel/palette/*`,
  `ui/src/features/settings/agent/useAgentCatalog.ts`.
