# External agent (shipped truth)

TODO — filled as the external-agent slices ship. The ask is split across an umbrella + five sub-scopes
under `../../scope/external-agent/`; working logs will live under `../../sessions/external-agent/`.

The topic: optionally **drive** a third-party ACP coding/data agent (VT Code default, dirge alternate)
as a sandboxed subprocess behind a host-owned `AgentRuntime` seam — compile-time optional, swappable by
config, with the agent's only tools being our caps-checked MCP surface and its models routed through our
gateway. This page documents, once shipped, each slice:

- **runtime-seam** — the `AgentRuntime` trait + registry + the `external-agent` cargo feature (compile with/without).
- **acp-driver** — `AcpRuntime`: spawn + MCP bridge + ACP↔`RunEvent` encode + `AgentProfile` + version negotiation.
- **capability-wall** — built-ins-off fail-closed + OS sandbox + MCP-only tool exposure (the safety gate).
- **model-routing** — models via the gateway's OpenAI-compatible endpoint + scoped token + audit.
- **run-lifecycle** — the durable run job + resume-per-profile + subprocess supervision + `agent.watch`/`agent.runtimes`.

## Agent config — the per-workspace default runtime (shipped)

Scope: [`../../scope/external-agent/agent-config-scope.md`](../../scope/external-agent/agent-config-scope.md) ·
Session: [`agent-config-settings`](../../sessions/external-agent/agent-config-settings-session.md) ·
Skill (operating manual): [`../../skills/external-agent/SKILL.md`](../../skills/external-agent/SKILL.md)

A workspace can now **persist which agent runtime it uses by default** and the **model endpoint** it
routes through — beyond the node's compiled-in default. Mirrors the `prefs.set_default` pattern (an
admin-settable per-workspace default record):

- **Record:** `workspace_agent_config:[ws]` (SCHEMAFULL, composite id → idempotent offline replay,
  LWW). Holds a nullable `default_runtime` (validated against the node's `RuntimeRegistry` on write —
  an id the node can't run is a `BadInput`, not a silent accept) and a nullable `model_endpoint`
  (`provider`/`model`/`api_key_env`/`base_url` — **names only**, never a secret value).
- **Verbs:** `agent.config.get` — read (**member**, `mcp:agent.config.get:call`); `agent.config.set` —
  MERGE patch (**admin**, `mcp:agent.config.set:call`, beside `prefs.set_default`/`agent.policy.set`).
  Opaque deny. Gateway 1:1 mirror: `GET|PUT /agent/config`. No `delete` (unset an axis by patching it
  null — one record per ws), no live-feed, no batch (deliberate scope non-goals).
- **UI:** the **Agent** tab of the Settings surface (`ui/src/features/settings/AgentTab.tsx`) — a
  runtime dropdown backed by `agent.runtimes` (a workspace can never select a runtime the node can't
  run) + the names-only endpoint fields. Editable for an admin; **read-only** for a member without
  `agent.config.set`; a stored-but-now-unavailable runtime is flagged as registry drift rather than
  erroring.
### Honoring the stored default on a run (shipped)

Session: [`invoke-default-runtime`](../../sessions/external-agent/invoke-default-runtime-session.md)

A run that **omits `runtime`** now dispatches the workspace's stored default (not just the compiled-in
one). One resolution seam (`rust/crates/host/src/agent/resolve_default.rs`,
`resolve_effective_runtime`) with a single precedence — **explicit arg → workspace
`agent.config.default_runtime` → registry default** — wired into `invoke_via_runtime`, the one place
runtime selection happens, so BOTH entrypoints (`agent.invoke` via `serve`, the channel `/agent`
worker) resolve identically with no second copy.

- **Explicit wins:** a named `runtime` is used verbatim (an unknown named id still errors — no silent
  downgrade).
- **Registry drift is fail-open:** a stored default the node no longer offers (feature off / config
  changed) falls back to the registry default with a `warn!`, never erroring a run. A store read
  failure is likewise treated as "unset".
- **No widening:** resolution runs AFTER the `mcp:agent.invoke:call` gate and is pure selection — every
  tool the run calls is still re-checked under `agent ∩ caller`. Reading the config to resolve dispatch
  needs no `agent.config.get` grant (the host resolves its own dispatch).

- **Follow-up (named):** the per-workspace **`model_endpoint`** override at invoke time is deferred —
  runtimes are built at boot with a fixed endpoint, so honoring the stored endpoint means threading it
  through the stable `AgentRuntime::run`/`RunContext` seam + the external-agent wrapper (its own slice,
  not a one-liner). Also: wiring `serve_agent`/a callable `agent.invoke` from the node binary (the
  existing serve-wiring TODO) so the live channel run is drivable over the gateway; full `AgentProfile`
  authoring (`granted_tools`/`persona_skill`) when the feature ships in anger.
