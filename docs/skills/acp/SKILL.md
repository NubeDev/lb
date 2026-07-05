---
name: acp
description: >-
  Explain and operate the two ACP (Agent Client Protocol) surfaces a Lazybones node has. Use when a
  task says "connect Zed/Cursor to the node", "drive us as an ACP agent", "which ACP methods do we
  implement", "run the lb-acp adapter", "editor-drives-us vs us-drives-an-external-agent", "how does an
  external agent reach our tools", "run lifecycle over ACP (watch/permission/control)", or "does the
  ACP server support session/load / mcpServers". Grounds the WE-as-ACP-server role (`role/acp`, the
  `lb-acp` stdio binary an editor launches) against the SEPARATE runtime seam where WE drive an external
  ACP agent (`agent.runtimes` / `agent.invoke { runtime }`). Both sides are workspace-walled and
  caps-checked — being on either ACP surface grants NO extra authority.
---

# The two ACP surfaces of a node

A Lazybones node touches the **Agent Client Protocol (ACP)** on two *opposite* sides, and confusing them
is the #1 source of "which ACP thing do I want?":

1. **We ARE an ACP server** — an external editor (Zed / Cursor) connects to us and drives *our* agent.
   This is `role/acp` (the `lb-acp` stdio binary). The editor is the ACP **client**; we are the agent.
2. **We DRIVE an external ACP agent** — our run spawns Open Interpreter / VT Code / Codex and drives
   *it*. This is the host **runtime seam** (`agent.runtimes` + `agent.invoke { runtime }`), documented
   fully in the sibling **`external-agent`** skill. Here we are the ACP client; the subprocess is the agent.

Both are workspace-walled and caps-checked. Neither ACP surface is a new grant — see §2, §5.

---

## 1. We AS an ACP server — `role/acp` (`lb-acp`)

An editor launches the `lb-acp` binary and speaks **JSON-RPC 2.0 over stdio** (`role/acp/src/stdio.rs` +
`rpc.rs`). The adapter is a **thin encoder**: it maps the ACP turn lifecycle onto the host's run
primitives and streams the run's `RunEvent`s back as `session/update` notifications. It owns no kernel
logic — the stable contract is the `RunEvent` vocabulary + the durable job transcript.

### Methods actually implemented (`role/acp/src/session.rs::handle`)

This is **partial ACP v1 — only the methods the turn lifecycle needs**, honestly:

| ACP method | What it does here | Maps to |
|---|---|---|
| `initialize` | Advertise `protocolVersion: 1` + our capabilities (see below) | capability handshake |
| `session/new` | Start a durable run; the `sessionId` **IS** the durable job id | `invoke` job id |
| `session/prompt` | Drive ONE turn; stream `RunEvent`s as `session/update`s; end with a `stopReason` | `watch_run` + `invoke` |
| `session/cancel` | Idempotent durable cancel; leaves a restorable transcript | `cancel_run` |
| `session/load` / `session/resume` | Rehydrate from the transcript, replay updates, continue if resumable | `watch_run` + `resume` |

Anything else → `METHOD_NOT_FOUND` (`-32601`). Note what is **NOT** a dispatched method:
`session/request_permission` and `session/update` are things the adapter *emits*, not handles.
A suspension (an Ask policy hit mid-prompt, a `RunEvent::Suspended`) does **not** stream as an update —
the turn ends with the `"refusal"` stop reason and the decision settles **out-of-band** (via
`session/resume` once resolved). The disconnect-mid-permission contract holds because the run is durably
suspended regardless of the socket (`session.rs` `session_prompt` doc).

### What `initialize` advertises (and declines)

```json
{ "protocolVersion": 1,
  "agentCapabilities": {
    "loadSession": true,
    "promptCapabilities": { "image": false, "audio": false },
    "mcpCapabilities": { "http": false, "sse": false } } }
```

We back **only** our already-known internal MCP tools. **Client-provided `mcpServers` / `cwd` on
`session/new` are rejected cleanly** — not silently dropped — with app error code `-32010`
(`UNSUPPORTED_CLIENT_SERVERS`), because bridging client-side tools would need a `net:*`-style grant
(a future scope). Declining less at the handshake is how the wall starts at the protocol edge.

### Authentication is the trusted-session path

`AcpSession::authenticate` verifies a real `lb_auth` session token with the node key and **binds the
session to exactly the workspace in the token** (§7 the wall). A forged/expired token → opaque
`UNAUTHENTICATED` (`-32001`); a cap denial inside `watch_run`/`cancel_run` → opaque `DENIED` (`-32002`).
Config is **environment, never the wire** (the editor cannot self-elevate): `LB_ACP_WS` (required
workspace), `LB_ACP_USER`, `LB_ACP_TOOLS` (the qualified MCP tools the model may propose). The binary
mints the token's caps to include `mcp:agent.invoke:call` + `mcp:agent.watch:call` + the proposable
tools' `mcp:<tool>:call` (`role/acp/src/main.rs`).

### Run it

```bash
cd rust
LB_ACP_WS=default LB_ACP_TOOLS=store.read,data.query \
  cargo run -p lb-role-acp --bin lb-acp
# then point Zed/Cursor's ACP agent config at this command; it speaks JSON-RPC over stdio.
```

The lifecycle a driver sees (from `acp_stdio_test.rs`, a **real** spawned binary over a **real** pipe):

```
→ {"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{…}}}
→ {"jsonrpc":"2.0","id":2,"method":"session/new","params":{"sessionId":"run1"}}
← {"jsonrpc":"2.0","id":2,"result":{"sessionId":"run1"}}
→ {"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"run1","prompt":"hi"}}
← {"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"run1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"…"}}}}
← {"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}
```

`RunEvent → session/update` mapping (`role/acp/src/encode.rs`): text→`agent_message_chunk`,
reasoning→`agent_thought_chunk`, tool start→`tool_call`, args/result→`tool_call_update`,
skill→`plan`. Stop reasons: `Done`→`end_turn`, `Cancelled`→`cancelled`, `Failed`/`Suspended`→`refusal`.

---

## 2. We DRIVING an external ACP agent — the runtime seam

The *other* direction: a run spawns a third-party agent and drives it. This is the **host-owned
`AgentRuntime` seam** (`crates/host/src/agent/runtime.rs`), and it is a **registry, not a `match`** —
`Box<dyn AgentRuntime>` selected by an id. Full operating manual: the **`external-agent`** skill. In brief:

```bash
# What this node offers (member cap mcp:agent.runtimes:call). Feature OFF → just ["default"].
lb call agent.runtimes '{}' -o json
# { "default":"default", "runtimes":["codex-default","default","open-interpreter-default","vtcode-default"],
#   "workspace_default": null }

# Persist the workspace default (admin cap mcp:agent.config.set:call):
lb call agent.config.set '{"patch":{"default_runtime":"open-interpreter-default"}}' -o json

# Run — the runtime is an ARGUMENT, same gate for every engine:
lb call agent.invoke '{"runtime":"open-interpreter-default","goal":"…"}' -o json
```

The external runtimes are compiled in **only** with `cargo build -p node --features external-agent`
(OFF by default — a plain build registers only the in-house `"default"`). The shipped transport for
these is `exec --json` over NDJSON stdout (`role/external-agent`); the full ACP-SDK client wire
(`spawn.rs`/`bridge_mcp.rs`/`encode.rs`) is the next additive slice behind the same feature — see
`docs/scope/external-agent/acp-driver-scope.md`, which is honest that the on-disk transport is not yet
ACP. Either way the seam is transport-agnostic and a watcher observes an external run identically to an
in-house one (the runtime forwards projected `RunEvent`s onto the run's bus subject).

**The runtime seam is the boundary.** Selecting a runtime is an **argument** (the `runtime` field, or
the workspace default), **never a new grant** — the invoke gate `mcp:agent.invoke:call` is identical
for `default`, `open-interpreter-default`, `vtcode-default`, `codex-default`. A **persona** is
orthogonal: the persona picks *focus* (its pinned skills narrow the advertised catalog,
`RunContext::persona_catalog`), the runtime picks the *engine*. Persona verbs (`agent.persona.*`) are the
**`agent`** skill's surface.

---

## 3. Run lifecycle over ACP (watch / permission / control)

The run a session drives exposes three verbs, each its **own** cap (grounded in
`gateway/src/session/credentials.rs`) — one authority never implies another:

- **Watch** — `mcp:agent.watch:call` (member). The live `RunEvent` feed:
  `GET /runs/{job}/stream?token=<jwt>` (`gateway/src/routes/run_stream.rs`), snapshot-then-live, a `403`
  before any body on deny. On the ACP server side this is what `session/prompt`/`session/load` subscribe
  to and re-encode as `session/update`s.
- **Permission (Ask)** — `mcp:agent.decide:call` (member). A per-tool-call human gate: `agent.decide`
  first-settles a `RunEvent::Suspended`. Over ACP the suspension is **not** streamed; the turn ends
  `"refusal"` and `agent.decide` (or the surfaced inbox item) settles it, after which `session/resume`
  picks the run back up. `agent.policy.set` (**admin**) edits the workspace Allow/Deny/Ask policy.
- **Control** — `mcp:agent.control:call` (member). STOP / PAUSE / RESUME on a run
  (`POST /runs/{job}/{cancel|pause|resume}`), opaque `403` on deny. **Distinct from `agent.watch`** —
  watching a run never implies authority to control it. `session/cancel` is the ACP door to STOP.

The workspace wall isolates every one of these: a ws-B caller can never reach a ws-A run, regardless of
which ACP surface they came in on.

---

## 4. How the external agent reaches tools

The external agent's **only** tools are **our** caps-checked MCP surface — it holds no extra authority.
Its MCP bridge advertises exactly the run's **allowed tools** (the granted, narrowed set — the derived
`caller ∩ agent` principal, scoped to `ws`), so it literally cannot enumerate another workspace's tools.
Every tool it proposes is dispatched through `lb_mcp` → `caps::check` under that derived principal;
ungranted → an ACP **tool error** returned to the agent (it reacts; not a crash). It pulls skill bodies
on demand via the grant-gated `load_skill` (the `grant:skill/{id}` gate), the same model-activated-skills
path the in-house loop ships. The persona skill is loaded the same grant-gated way — "this agent is a
data analyst" requires the workspace to have granted that skill.

The MCP **bridge is feature-gated** (`role/external-agent`, `--features external-agent`): a feature-off
node never compiles it and offers only `["default"]`, so there is nothing to bridge.

---

## 5. Which one do I want?

- **Editor-drives-us** (an editor like Zed/Cursor should use *our* agent + tools) → **§1**, run the
  `lb-acp` binary; the editor is the ACP client, we are the agent. Partial ACP v1, trusted-session auth,
  no client `mcpServers`.
- **Us-drives-external** (our run should be executed by Open Interpreter / VT Code / Codex instead of the
  in-house loop) → **§2** + the **`external-agent`** skill; pick a `runtime` (arg or workspace default),
  build `--features external-agent`.
- **In-house default agent** (no external engine, just configure/drive the built-in loop) → the
  **`agent`** skill (`core.agent`); the `"default"` runtime.

**Never**: treat picking a runtime as a new grant (it's an argument under the one `mcp:agent.invoke:call`
gate); expect an external runtime without `--features external-agent`; let an ACP editor pass its own
`mcpServers`/`cwd` (rejected `-32010`); assume `role/acp` implements full ACP (it implements the five
lifecycle methods in §1, and emits — not handles — `session/update` / `request_permission`).

## Related

- Skill: [`../external-agent/SKILL.md`](../external-agent/SKILL.md) (us-driving-an-external-agent — the
  runtime seam operating manual), [`../agent/SKILL.md`](../agent/SKILL.md) (`core.agent`, the in-house
  default agent + `agent.persona.*`), [`../lb-cli/SKILL.md`](../lb-cli/SKILL.md) (the `lb call` transport).
- Scope: [`../../scope/agent-run/agent-run-scope.md`](../../scope/agent-run/agent-run-scope.md) (Part 4 =
  the ACP session driver; the `RunEvent` vocabulary),
  [`../../scope/external-agent/acp-driver-scope.md`](../../scope/external-agent/acp-driver-scope.md)
  (the ACP-client `AcpRuntime`, honest about the shipped `exec --json` transport vs the ACP target),
  [`../../scope/external-agent/run-lifecycle-scope.md`](../../scope/external-agent/run-lifecycle-scope.md).
- Source: `rust/role/acp/src/` (`session.rs`, `encode.rs`, `rpc.rs`, `stdio.rs`, `main.rs`),
  `rust/crates/host/src/agent/runtime.rs` + `runtimes.rs`.
