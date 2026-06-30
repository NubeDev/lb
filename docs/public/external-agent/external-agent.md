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
