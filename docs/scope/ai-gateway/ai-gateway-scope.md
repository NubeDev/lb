# AI gateway scope

Status: draft.

Define the shared AI gateway: the controlled service layer that routes AI requests from
users, agents, jobs, and extensions to approved model providers.

## Intent

The AI gateway centralizes model/provider access so extensions do not each own API keys,
routing logic, streaming, quota checks, retention policy, or audit logs. It is the single
chokepoint where "a request wants a model" meets "the workspace's policy, budget,
capabilities, and providers."

The gateway is **not an agent** and it does not run the agent tool-call loop. That loop —
model proposes a tool, the result is fed back, repeat — belongs to the *caller* (the central
agent §6.15, or a workflow job §6.9). The gateway is a stateless model-access service: a
request goes in, a (possibly streaming) completion or embedding comes back. Tool-call
routing and the per-call capability checks live with the agent/job, over the normal MCP
namespace (§6.5). Keeping the loop out of the gateway is what lets the gateway be swapped.

### A swappable microservice behind a stable contract

This is the load-bearing design decision. The gateway is a **swappable microservice** — a
Tier-2 native sidecar (§6.3) the node talks to, *not* a core crate. The core depends only on
the **contract**, never on the implementation. The implementation could be anything: a
custom Rust service, SpiceAI's LLM gateway, a LiteLLM-style proxy, or a thin local-model
wrapper. Swap it without touching the core or any caller.

Placement and behavior are chosen by config, never by an `if cloud {…}` branch (core
principle 1):

- **Hub placement (default):** shared provider keys, pooled quotas, heavy/hosted models,
  long retention. Edge users and local extensions route here when online.
- **Edge placement:** serves **local-only** and **offline** execution by routing to a local
  model provider (`llama.cpp`/`ollama`/`candle`). Same contract, resolving to a local
  provider and **refusing** remote routing.

Consequence: "local-only execution" is not a special path. It is the gateway honoring a
capability flag that forbids remote providers, on a node that has a local provider
configured. Sensitive workflows set the flag; whichever gateway implementation is deployed
enforces it the same way everywhere.

## Position in the stack (reconcile, don't duplicate)

- The gateway is the **policy + routing + model-access** layer. It is *not* a model, and
  *not* an agent.
- **SpiceAI (§6.14)** is one possible *implementation/provider* behind the contract — its LLM
  gateway can be the hosted-model adapter, but it sits **behind** the contract, not beside it.
  There is exactly one gateway contract; SpiceAI, OpenAI-compatible HTTP, and local sidecars
  are all implementations of it. Two gateways is the failure mode to avoid.
- Callers (central agent §6.15, workflow jobs §6.9, edge UI, extensions) reach the gateway
  as **MCP tools** (§6.5) — the same routed namespace as everything else. The gateway does
  not invent a side-channel.

## The stable internal contract (a forever commitment)

Like the SDK/WIT boundary, the gateway's internal request/response shape is versioned and
deliberate, because every caller and every provider adapter depends on it.

- `AiRequest` — model class (or pinned model id), messages/prompt, tool schema, retention
  mode, budget ceiling, local-only flag, idempotency key, workspace/actor scope.
- `AiResponse` — content, tool calls, token/cost usage, model id actually used, finish reason.
- `ToolCall` / `ToolResult` — the model's proposed tool calls are returned to the caller and
  prior results are passed back in; the gateway carries these but does **not** execute the
  loop (that is the caller's job).
- Provider adapters translate this to/from provider wire formats. Provider churn stays
  behind the contract and never leaks to callers.

## Responsibilities

- **Provider routing & failover** for hosted, local, enterprise, or OpenAI-compatible
  models: ordered fallback chains, circuit-breaking on a dead provider, degraded/offline mode.
- **Provider secrets** via the secrets crate (§6.7) — workspace-owned or hub-owned keys,
  envelope-encrypted, never handed to the calling extension.
- **Streaming with state/motion split (principle 3):** stream tokens as ephemeral Zenoh
  messages (motion) to edge UIs, channels, jobs, and inbox items; persist the durable
  transcript (prompt, retrieved context refs, output, tool calls, usage) to SurrealDB as
  *state*, with delivery guaranteed via outbox (§6.10). Partial streams are never the record.
- **Embeddings, not just completions** — serve the embedding models that feed HNSW vector
  search (§6.1). **Pin the embedding model version per index**; changing it silently
  corrupts retrieval, so re-embedding is an explicit, tracked migration.
- **Model policy** — allowed models, retention mode, max cost, local-only requirement,
  expressed as **projections of the unified scope model (§6.6)**, not a separate ACL. Model
  access is a capability/MCP grant like every other capability.
- **Quotas & budgets** by workspace, team, user, extension, and agent — with **mid-stream
  enforcement**: pre-flight estimate, post-flight metering, hard vs soft caps, and defined
  behavior when budget is exhausted while a response is streaming (truncate-and-mark, not
  silent drop).
- **Replay-safe calls for durable jobs** — every `AiRequest` carries an idempotency key; the
  gateway caches the response keyed by it, so a resumed job (§6.9) does not re-spend budget
  or diverge. Non-determinism is pinned to the first execution.
- **Audit** — an append-only, tamper-evident record for every model call and every tool call
  inside the loop (actor, workspace, model, tokens, cost, capabilities exercised, retention
  mode, idempotency key). Audit is mandatory and capability-independent.
- **Capability-filtered context retrieval** over docs, skills, messages, files, and workflow
  session state — every retrieval passes the same workspace-first capability check, so the
  model only ever sees granted context.

## Example flow

1. A coding workflow job asks the central AI agent to triage a GitHub issue (over MCP).
2. The agent calls the gateway with an `AiRequest` (model class, tool schema, budget,
   idempotency key, retention mode).
3. The gateway checks workspace policy, quotas, provider credentials, and context
   permissions; resolves a provider (honoring any local-only flag).
4. The model returns content and/or proposed tool calls. The **agent (the caller)** runs the
   loop — capability-checking and routing each tool call over MCP, then sending results back
   to the gateway for the next turn — until completion or its own budget/iteration limit.
5. The gateway streams output (motion) to the job and channel, and persists the transcript
   (state) via outbox; it writes the audit record.
6. The job saves generated docs, creates approval inbox items, and sends durable external
   actions through outbox. A later resume reuses the cached response by idempotency key.

## What shipped in S5 (the swappable model-access sidecar, mock provider)

The `lb-role-ai-gateway` crate behind the stable contract: `complete(AiRequest) -> AiResponse`,
**model access only — no loop** (the agent owns the loop, agent scope). S5 builds the contract and
a **mock provider** wired at the test boundary (testing §3 — deterministic, no network): a provider
that, given a prompt, returns scripted content and/or proposed `ToolCall`s, so the agent's loop is
exercised end-to-end without a real model. The provider is a trait (`Provider::complete`) so the
real OpenAI-compatible / SpiceAI / local adapters slot in behind the same contract later (one
gateway contract, many implementations — never two gateways).

**Replay-safe by idempotency key (S5):** every `AiRequest` carries an `idempotency_key`; the
gateway caches the `AiResponse` keyed by it, so a **resumed** agent job (agent scope, offline/sync)
does not re-spend budget or diverge — non-determinism is pinned to the first execution. This is the
gateway half of the resume-idempotency guarantee; the agent half is the append-addressed transcript.

**Deferred past S5:** real provider adapters, streaming token motion, embeddings, mid-stream budget
enforcement, the audit hash-chain, and secrets-backed provider keys — the contract is shaped for
them (the fields exist) but S5 ships completions + the idempotency cache only, against the mock.

## Open questions

- Provider keys: per-workspace, hub-managed, or both — and the precedence when both exist?
- Routing policy language: declarative fallback chains + model-class aliases, or richer rules?
- Mandatory audit fields, and the tamper-evidence mechanism (hash chain? signed batches?).
- Retention: how long are prompts, retrieved context, and outputs kept, per retention mode,
  and how does that interact with workspace data-residency requirements?
- Local-only enforcement: is it a capability flag, a workspace policy, or both — and how does
  a caller *prove* a request ran locally (attestation in the audit record)?
- Embedding-model migration: how is a re-embedding triggered and tracked when a pinned model
  changes?
- Deployment shape: how does the node discover and supervise the gateway sidecar, and what is
  the transport (local socket vs Zenoh) for streaming?
- Relationship to SpiceAI: confirm it is strictly one possible implementation behind the
  contract, never a parallel gateway.

## Related

- Core scope `../../../README.md` — §6.5 MCP, §6.6 capabilities, §6.7 secrets, §6.9 jobs,
  §6.10 inbox/outbox, §6.14 SpiceAI, §6.15 shared AI agents.
- `../../vision/0002-coding-agent-workplace.md` — the worked example that drives these
  requirements.
