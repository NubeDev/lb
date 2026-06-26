# Agent (public)

The central, workspace-scoped AI agent — shipped at S5. The ask lives at
`../../scope/agent/agent-scope.md`; the build session at `../../sessions/agent/ai-core-session.md`.

## What it is

A **host service** (`lb_host::agent`, beside `channel`/`assets`) that hosts a workspace-scoped actor
which **owns the tool-call loop**. It is not a model and not a wasm extension: the loop must call
`caps::check` on each tool dispatch, read S4 assets through the host verbs, and drive a durable job —
all host-internal seams.

## The loop (the agent is the loop)

Bounded by `MAX_STEPS`:

1. ask the model for a turn via the **gateway** (`ModelAccess::turn`) — replay-safe by a per-step
   idempotency key;
2. for each proposed tool call, run it through `lb_mcp::call` under the **derived** principal
   (capability-checked, workspace-first, routed if the tool is on another node) — a denial is fed
   back to the model, not a crash;
3. persist the step to the job (idempotent, append-addressed) and advance the cursor;
4. repeat until the model is done or the ceiling is hit; then `complete` the job.

The gateway does **model access only** — keeping the loop out of it is what lets the gateway be a
swappable sidecar (`ai-gateway/ai-gateway.md` / the ai-gateway scope).

## The intersection (no widening)

The agent acts under `agent ∩ caller`:

- `Principal::derive(sub, agent_caps)` mints a strictly **narrower** actor — same workspace
  (delegation can't cross the wall), a distinct `agent:*` sub (audit shows the agent acted), the
  agent's caps, and the caller's caps as a `constraint`.
- `caps::check` runs **gate 2b**: a delegated request must match the caller's caps too. Exact set
  intersection, no pattern algebra — an agent can never do something *either* side forbids.
- **Substrate** (granted skill + shared doc) is read **on the caller's behalf**: the caller's
  identity resolves the S4 membership/grant gate 3, the intersected caps bound the capability gate.
  (See `../../debugging/agent/agent-reads-doc-it-doesnt-own-is-denied.md` for why.)

## Reachability — MCP + routed

`agent.invoke` is a host-native MCP tool (the same `lb_mcp::authorize_tool` bridge as `assets.*`):
the MCP gate (`mcp:agent.invoke:call`, workspace-first) then the loop. An **edge** invokes the
**hub** agent over the routed namespace — `invoke_remote` authorizes on the edge, then `query`s the
hub's queryable (`ws/*/agent/invoke`, the S3 routing seam). `caps::check` runs on the calling node;
the workspace-scoped key keeps isolation structural.

## The session is a durable job

The agent's session is an `lb-jobs` `job:{id}` record (workspace-scoped, no separate datastore): an
**append-addressed transcript** + a cursor. The edge that invokes does not hold the session — if it
disconnects mid-loop, the hub keeps running and each step persists. `resume` re-reads the record and
continues from the cursor; re-applying a persisted step is a no-op, and the gateway's idempotency
cache means the resumed turn is not re-spent — so resume is idempotent.

## Tested (the S5 mandatory categories)

- **Capability-deny:** the invoke gate (no `mcp:agent.invoke:call` → refused before the loop); the
  in-loop intersection (a tool the *caller* lacks is denied even if the agent holds it — no
  widening); an ungranted substrate skill is invisible.
- **Workspace-isolation:** an agent in workspace B can never read workspace A's docs/skills/jobs —
  across store + MCP, and across the routed edge→hub path.
- **Offline/sync:** a session interrupted mid-loop resumes from its cursor (no double-apply); a
  duplicated invocation does not re-spend the gateway or duplicate a step.

## Not yet (follow-ups)

A real model provider behind the gateway contract; streaming progress as Zenoh motion + the durable
transcript via the outbox; token-on-the-bus for routed invocations (S5 is in-process co-trust);
gateway/Tauri wiring for `agent_invoke` against a real node; per-workspace loop policy. The coding
workflow that composes the agent (issue → triage → approval → job → outbox) is S6.
