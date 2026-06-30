# External-agent scope — overview & index

Status: scope (the ask, umbrella). Promotes to `public/external-agent/` once the slices ship.

Lazybones already owns a sound in-house agent loop (`scope/agent/`) and exposes it to editors as an
ACP **server** (`scope/agent-run/` Part 4 — Zed/Cursor drive *our* loop). This topic is the
**inverse and an add-on**: let a node *optionally* **drive a third-party agent** (VT Code, dirge,
Claude Code, Gemini CLI, Goose, Pi, … — any [Agent-Client-Protocol](https://agentclientprotocol.com)
agent) as a **subprocess**, while every tool that agent can touch is **our** capability-checked MCP
surface and every model token it spends goes through **our** gateway. It must be a **compile-time
feature** (build the node with or without it) and **swappable by config** (change which agent runs
without touching code).

This is a big task, so it is **broken into five sub-scopes** (below), each independently reviewable
and shippable, with one umbrella exit gate. This doc is the index, the thesis, and the cross-cutting
rules; the detail lives in the sub-scopes.

> The unit we adopt is **a protocol (ACP), not an agent.** We implement the ACP *client* once,
> against the [official Rust SDK](https://github.com/agentclientprotocol/rust-sdk), and the entire
> ACP agent registry becomes pluggable. VT Code is the *default*; dirge is the documented alternate.
> Neither is load-bearing — both are config. The default runtime stays the in-house loop.

---

## The thesis (read once, applies to every sub-scope)

**The seam is `AgentRuntime`; the wire is ACP; the wall is MCP.** Three already-proven ideas stacked:

1. **A host-owned trait, default + optional impls** — exactly the `ModelAccess`/`Provider` move that
   already lets the gateway be swapped. The in-house loop is the default impl; the external driver is
   an optional impl behind a cargo feature. → **runtime-seam**.
2. **Drive ACP via the official SDK, once** — `agent-client-protocol` (Client role),
   `-tokio` (spawn/stdio), `-rmcp` (bridge our MCP server to the agent). The whole ACP registry is
   reachable; the choice is an `AgentProfile`. → **acp-driver**.
3. **Tools = our MCP server, nothing else** — the agent's built-ins are disabled (fail-closed) and the
   only tools offered are the derived principal's granted MCP tools, behind an OS sandbox. → **capability-wall**.

Plus two cross-cutting concerns each big enough to scope alone: routing the agent's **models** through
our gateway (→ **model-routing**) and the durable **run** + resume + supervision (→ **run-lifecycle**).

**What a profile is — and why a coding agent becomes general-purpose.** An `AgentProfile` (acp-driver) is
`{ binary, granted_tools, persona_skill, model_endpoint, resume }`. Because the agent's built-ins are off
(#3), **what it can do = the granted MCP tools**, and **who it is = the granted persona skill** (loaded
via the *already-shipped* grant-gated `load_skill` — `public/skills/skills.md`). So a coding-branded agent
like VT Code, given `federation.query`/`data.query`/`series.*`/`viz.query` + a "data-analyst" skill, **is
a data agent** — same binary, same seam, different profile. "Coding agent vs data agent" is therefore a
**profile decision, not code and not a fork**: tools and persona are *granted data*. (Both are
**grant-gated** — a workspace can't get a data agent it didn't grant the data tools + skill for.)

**Rejected (applies topic-wide): forking or embedding one agent as the loop.** Embedding re-imports the
anti-pattern the gateway scope warns against — a second loop that runs tools *its* way (bypassing
`caps::check`), holds its own session state (breaking stateless-extension + durable resume), and (for
GPL agents) infects our license. Driving over ACP at a process boundary keeps the wall, the durability,
and the license clean — and makes the agent swappable, which forking never is.

## Architecture map

```
agent.invoke (existing MCP tool, caps-checked)               scope/agent/ , scope/agent-run/
        │  selects a runtime by profile
        ▼
  AgentRuntime  ──default──▶  in-house loop                  scope/agent/  (unchanged)
   (host trait) ──optional─▶  AcpRuntime ─────────────┐      [runtime-seam]
                                                       │
        spawn + stdio (official ACP SDK) ──────────────┤      [acp-driver]
                                                       │
   ┌───────────────────────────────────────────────────▼────────────────┐
   │  external agent subprocess (VT Code / dirge / Claude Code / …)       │
   │   • built-ins OFF (fail-closed) + OS sandbox (no egress/fs) ─────────┼─ [capability-wall]
   │   • tools = our MCP server only, via -rmcp bridge ──────────────────┼─ caps::check (derived principal)
   │   • model = our gateway, OpenAI-compatible, scoped token ───────────┼─ [model-routing]
   └───────────────────────────────────────────────────┬────────────────┘
        ACP SessionNotification ──encode──▶ RunEvent ───┘                   [run-lifecycle]
        run = durable job (resume, supervision, agent.watch)
```

## The five sub-scopes (build order)

| # | Sub-scope | Owns | Depends on |
|---|---|---|---|
| 1 | [runtime-seam](runtime-seam-scope.md) | The `AgentRuntime` host trait, the runtime registry + selection, and the **`external-agent` cargo feature** (compile with/without). The foundation everything else plugs into. | host, the `ModelAccess`/`Provider` pattern |
| 2 | [acp-driver](acp-driver-scope.md) | `AcpRuntime`: spawn the subprocess (`-tokio`), bridge our MCP server (`-rmcp`), encode ACP↔`RunEvent`, the `AgentProfile` schema, ACP version negotiation. | #1, official ACP SDK, `scope/agent-run/` events |
| 3 | [capability-wall](capability-wall-scope.md) | The security-critical sandbox: built-ins-off **fail-closed**, OS egress/fs sandbox, declining ACP fs/terminal capabilities, MCP-only tool exposure under the derived principal. **The topic's safety exit gate.** | #2, `scope/auth-caps/`, `scope/mcp/` |
| 4 | [model-routing](model-routing-scope.md) | Pointing the agent's model at the gateway's **OpenAI-compatible** endpoint, the short-lived scoped token, agent-originated audit attribution, no-key enforcement. Carries the **ai-gateway dependency**. | #2, `scope/ai-gateway/`, `scope/secrets/` |
| 5 | [run-lifecycle](run-lifecycle-scope.md) | The run as a durable **job**, resume strategy per profile (the hard problem), subprocess supervision (timeout/kill/restart/zombie), and the read surface (`agent.watch` reuse + `agent.runtimes`). | #2, `scope/jobs/`, `scope/agent-run/` |

Ship order: **#1 → #2 → then #3 / #4 / #5 in parallel.** #3 (the wall) is the gate that makes driving
an untrusted third-party loop *safe*; nothing that actually runs an external agent in anger ships
before #3 is green.

## Cross-cutting platform checklist (addressed topic-wide; sub-scopes carry the detail)

- **Workspace is the hard wall** — the subprocess is launched bound to one `ws`; the MCP endpoint it's
  handed is workspace-walled; the run job carries `ws`. Isolation is the MCP chokepoint, not the loop
  (proven in #3, #5).
- **Capability-first** — invoking reuses `mcp:agent.invoke:call` (no new caller cap); inside, every
  tool re-runs `caps::check` under the derived `caller ∩ agent` principal; deny is fed back as an ACP
  tool error, not a crash (#3).
- **Symmetric nodes** — the difference between a node that can drive an external agent and one that
  can't is the **cargo feature + config**, never an `if cloud {…}` branch (#1). Placement is `either`.
- **One datastore** — the run is a `job:{id}` record; no new tables, no new persistence (#5).
- **No mocks / no fake backend (rule 9)** — the external agent is a **real** subprocess over a **real**
  ACP pipe against a **real** in-proc MCP server; the **one** permitted fake stays the provider HTTP
  (the existing `MockProvider` behind the gateway). CI binary-availability is an open question (#2/#3).
- **State vs motion** — `RunEvent`s stream as motion; the job is the durable state; the ACP stdio pipe
  is local motion, never on the bus (#5).
- **Stateless** — the external agent's own session memory is **ephemeral subprocess scratch**, never
  authority; the job transcript is the source of truth (#5).
- **MCP is the contract** — the agent consumes our MCP tools; the runtime is selected behind the
  existing `agent.invoke`; the only new verb is the read-only `agent.runtimes` (#5).
- **SDK/WIT impact** — **none on the WASM guest ABI.** It adds one host-owned internal seam
  (`AgentRuntime`, alongside `ModelAccess`); flagged in #1.

## Umbrella exit gate

The topic is shippable when:

- #1 green: the node **builds + tests with the feature OFF and ON** (CI matrix; `cargo tree` asserts the
  ACP deps absent in the OFF build), and the default in-house runtime is unaffected either way.
- #3 green: the **wall test** passes against the **real** default agent binary — a tool the derived
  principal lacks is denied at `caps::check` and never executes; a profile that can't prove built-ins
  are off **aborts at launch**.
- The **swap test** passes: the same invoke path drives a second agent (dirge) via a second profile with
  **no code change**.
- #4 green: the agent reaches a model **only** through the gateway (no key handed out; direct egress
  impossible by sandbox); calls are audited as agent-originated.
- #5 green: a run survives edge disconnect and resumes without double-applying effects or re-spending
  budget; a hung subprocess is bounded and killed without pinning the job.

## Related

- README `§6.16` (shared AI agents), `§6.15`/`§6.14` (AI gateway), `§6.9` (jobs), `§6.5` (MCP),
  `§6.13` (gateway SSE), `§7` (tenancy).
- `scope/agent/agent-scope.md` (the default runtime), `scope/agent-run/agent-run-scope.md` (the ACP
  *server* mirror + `RunEvent`s), `scope/ai-gateway/ai-gateway-scope.md` (the model contract + shim),
  `scope/jobs/jobs-scope.md`, `scope/auth-caps/auth-caps-scope.md`, `scope/node-roles/node-roles-scope.md`.
- External: [Agent Client Protocol](https://agentclientprotocol.com) · [official Rust SDK](https://github.com/agentclientprotocol/rust-sdk)
  · [VT Code](https://github.com/vinhnx/vtcode) (MIT, default) · [dirge](https://github.com/dirge-code/dirge) (GPL-3.0, alternate).
