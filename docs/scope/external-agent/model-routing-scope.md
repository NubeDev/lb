# External-agent scope — routing the agent's models through our gateway

Status: scope (the ask). Sub-scope #4 of `external-agent-scope.md`. Promotes to `public/external-agent/`.

Make the external agent reach a model **only** through Lazybones' AI-gateway, presented as an
**OpenAI-compatible HTTP endpoint**, authenticated by a **short-lived, workspace-scoped token** — so
provider keys, model policy, budget, retention, and audit stay in the gateway **regardless of which
agent is plugged in**. The agent never holds a provider key and (by #3's sandbox) cannot reach a
provider directly. This sub-scope also carries the **ai-gateway dependency**: the gateway must grow an
OpenAI-compatible face.

## Goals

- **Gateway-as-OpenAI-endpoint:** every ACP agent we'd plug in already accepts a custom OpenAI-compatible
  `base_url` + key (verified for VT Code and dirge). Point the agent's `model_endpoint_ref` (from the
  `AgentProfile`, #2) at the gateway's OpenAI-compatible endpoint.
- **Scoped token, not a provider key:** the agent receives a **short-lived token bound to one
  workspace** (minted like the trusted ACP session token, `role/acp`), used as the `base_url`'s bearer.
  It authorizes model access for **this run, this workspace** — not provider credentials.
- **Audit attribution:** model calls the agent makes are recorded by the gateway as **agent-originated**
  (agent + caller + ws + run/job id), folding into the gateway's existing audit (ai-gateway scope) — so
  an external agent's spend and prompts are as accountable as the in-house loop's.
- **No-key enforcement, two ways:** the agent is never *given* a provider key (config), and #3's egress
  sandbox makes a direct provider call *impossible* (kernel). Policy/budget/local-only flags apply in the
  gateway exactly as for the in-house loop.

## Non-goals

- **Designing the OpenAI-compatible shim itself.** That is an **ai-gateway deliverable** (named here as
  the hard dependency); this sub-scope specifies what the external-agent side needs from it and how the
  token + attribution work, not the shim's internals.
- The provider adapters / rig integration behind the gateway — ai-gateway scope.
- The sandbox/egress mechanics (#3) — this sub-scope defines *what* the single allowed egress target is
  (the gateway socket); #3 enforces it.
- Streaming token motion to the UI — that rides the `RunEvent`/SSE path (#5), not this sub-scope.

## Intent / approach

**One model door for every runtime.** The in-house loop already calls the gateway via `ModelAccess`.
External agents can't call `ModelAccess` (they're foreign processes) — but they *all* speak
OpenAI-compatible HTTP. So expose the **same gateway** through an OpenAI-compatible face and point the
agent at it. The result: swapping the agent (or adding a tenth one) changes **nothing** about keys,
policy, budget, or audit — they're all behind the one door. This is the model-side twin of "tools = our
MCP server": **models = our gateway**.

**A token, never a key.** Handing an external process a provider key would scatter secrets and defeat the
gateway's whole reason to exist (§6.7). Instead mint a short-lived, ws-scoped bearer — the same trusted-
session pattern `role/acp` uses for the editor — that the gateway recognizes and attributes. The agent
authenticates *to us*; we authenticate to providers. Rejected: per-agent provider keys (scatters
secrets, no central budget/audit, breaks local-only enforcement).

**Belt and braces on no-direct-egress.** Config (no key) + kernel (#3 sandbox: only the gateway socket
reachable) means even a misconfigured or hostile agent cannot bypass the gateway to a provider. Either
alone is weaker; together a direct provider call is impossible.

## How it fits the core

- **Tenancy / isolation:** the scoped token binds the agent's model access to **one workspace**; the
  gateway enforces ws-first policy/budget on every call. A run in ws-B can't spend ws-A budget or use
  ws-A provider config.
- **Capabilities:** model access is a capability/policy projection in the gateway (ai-gateway scope) —
  the external agent inherits the **derived principal's** model policy; no new caller cap here.
- **Placement:** `either`. Hub → shared provider keys/pooled quota. Edge → the gateway resolves to a
  **local** provider and can enforce local-only; the agent's `base_url` points at the local gateway. Same
  contract, no branch.
- **MCP surface:** N/A — model access is HTTP to the gateway, not an MCP verb. (The agent's *tools* are
  MCP, #2/#3; its *model* is the gateway endpoint.)
- **Data (SurrealDB):** the gateway's audit/usage records gain agent-origin fields; no new external-agent
  table. State.
- **Bus:** N/A here (token streaming is #5).
- **Secrets (the core of this sub-scope):** provider keys stay envelope-encrypted in the gateway via the
  secrets crate (§6.7), never handed out. The minted run token is short-lived and ws-scoped; revocation =
  expiry + the gateway refusing it.
- **No fake backend (rule 9):** the gateway endpoint is **real**; the **provider HTTP behind it** is the
  one permitted fake (the existing `MockProvider`). So tests point the real agent at the real gateway
  endpoint, and the model responses are scripted — exercising the real auth + attribution path with no
  network.

## Example flow

1. The node mints a short-lived ws-scoped token for the run and sets the agent's `base_url` =
   gateway OpenAI-compatible endpoint, key = that token (profile `model_endpoint_ref`, #2).
2. The agent calls `POST {base_url}/chat/completions` (OpenAI-shaped) with the token.
3. The gateway verifies the token (ws + run), applies the derived principal's model policy + budget,
   resolves a provider (mock at the test boundary), and returns an OpenAI-shaped response.
4. The gateway writes an audit/usage record attributing the call to **agent + caller + ws + run id**.
5. The agent never saw a provider key; #3's sandbox guaranteed it could reach nothing but this endpoint.

## Testing plan

- **No-key + no-direct-egress:** assert the agent is launched with **no** provider key in its env/config,
  and (with #3) a scripted direct-provider call is blocked by the sandbox.
- **Token scope + attribution:** the run token authorizes model calls for its ws only; a call presenting
  it is audited as agent-originated with caller + run id; an expired/foreign token is refused by the
  gateway.
- **Workspace-isolation (§2.2):** a ws-B run's token cannot spend ws-B... err, ws-A budget or read ws-A
  provider config (ws-first gateway policy).
- **Budget/local-only honored:** the gateway applies the same budget ceiling + local-only flag to
  agent-originated calls as to the in-house loop (reuse gateway tests; assert parity).
- **Real-endpoint integration (rule 9):** real agent → real gateway endpoint → scripted provider; the
  full auth + attribution path with no network.

## Cross-scope prerequisite (BLOCKING — needs an owner before #4 starts)

**The gateway must *serve* an OpenAI-compatible endpoint (agent → gateway). This direction is not yet
scoped anywhere and has no owner — elevate it from "open question" to a tracked ai-gateway deliverable
now.** Critical nuance the review surfaced: `ai-gateway-scope.md` defines OpenAI-compatibility only as a
**consumed** provider contract (gateway → upstream providers), i.e. the gateway *calling* an
OpenAI-style API. #4 needs the **opposite** direction — the gateway *listening* as an OpenAI-style
server that the external agent's HTTP client posts `/chat/completions` (etc.) to. That is **new gateway
surface** (a served route, request/response translation into `ModelAccess`, the scoped-token auth on
that route), not a reuse of the existing provider-adapter code. Until an ai-gateway slice owns the
**served** face, #4 cannot integrate. Confirm endpoint coverage against what the chosen agents actually
call (`/chat/completions`, possibly `/models`, streaming).

## Risks & hard problems

- **Token lifetime vs long runs.** A run can outlive a short token; need refresh (or a run-length token
  the gateway can revoke) without widening scope. Get expiry/refresh wrong → either runs break mid-stream
  or tokens live too long.
- **OpenAI-dialect quirks per agent.** Agents differ in how they send tools/streaming over the
  OpenAI-compatible API; the shim must tolerate the dialect the default agent uses (test against the real
  agent, not a spec assumption).
- **Attribution gaps.** If the shim can't tie a call back to the run/agent, audit loses the external
  agent's accountability — make run id part of the token claims.

## Open questions

- **Does the gateway already expose an OpenAI-compatible endpoint, or must it be built first?** (The
  blocking dependency — resolve in ai-gateway scope.)
- **Token model:** short token + refresh, or a run-scoped token revocable by the gateway? Proposal:
  run-scoped, ws-bound, gateway-revocable, with run id in the claims.
- **Streaming:** do we need the shim's streaming (SSE) for the agents we ship, or is non-streaming
  acceptable for v1 (progress still flows via `RunEvent`s, #5)?
- **Local-only proof:** how does an edge run *prove* it stayed local (attestation in the audit record)?
  (Shared with ai-gateway open questions.)

## Related

- `external-agent-scope.md` (umbrella), `acp-driver-scope.md` (#2, the `model_endpoint_ref`),
  `capability-wall-scope.md` (#3, the single allowed egress).
- `scope/ai-gateway/ai-gateway-scope.md` — the model-access contract + the OpenAI-compatible shim this
  depends on. `scope/secrets/` — provider keys + the minted token pattern (cf. `role/acp` trusted token).
  README `§6.14`/`§6.15` (gateway), `§6.7` (secrets), `§7` (tenancy).
