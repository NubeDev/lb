# AI core slice (session)

- Date: 2026-06-26
- Scope: ../../scope/agent/agent-scope.md (NEW — authored this session) +
  ../../scope/ai-gateway/ai-gateway-scope.md + ../../scope/jobs/jobs-scope.md +
  ../../scope/auth-caps/auth-caps-scope.md (the delegation open question) + mcp, skills, files
- Stage: S5 — AI core (STAGES.md)
- Status: shipped

## Goal

Build S5 as a **vertical slice** through every layer (store → caps → bus → MCP → UI): a central,
workspace-scoped AI agent hosted on the hub, callable by edge users over the routed MCP namespace;
the swappable AI-gateway sidecar (model access only); and durable, resumable remote workflow jobs.

**Exit gate (S5), restated as the acceptance criterion:** an edge user invokes the central agent;
the agent calls the gateway for a model and a granted MCP tool; a workflow job survives the edge
disconnecting and resumes.

## What changed

### Scope authored first (the §6.16 agent had NO scope doc)

Per HOW-TO-CODE §1 + SCOPE-WRITTING, the missing **agent scope** was written before any code — the
central agent (README §6.16) had no `scope/agent/` doc, only the gateway (§6.14) and jobs (§6.9)
did. The load-bearing contracts come from there: the agent owns the loop (the gateway does model
access only); effective caps are the **intersection** `agent ∩ caller` (never a widening); the
session is a durable job; substrate (skills/docs) is read through the S4 gates. The jobs +
ai-gateway scopes' S5 open questions were finalized in-scope (record shape, idempotency cache,
deferred queue).

### caps + auth — grant **delegation** (the auth-caps S5 open question)

The security-critical primitive: `Principal::derive(sub, agent_caps)` mints a **narrower** actor —
same ws (delegation can't cross the wall), a distinct `agent:*` sub, the agent's caps, and the
caller's caps as a `constraint`. `caps::check` gained **gate 2b**: a delegated principal's request
must match its caps **AND** the constraint — exact set intersection with no pattern algebra, so an
agent can never do something *either* side forbids. `Principal::routed` reconstructs a caller from a
routed request (the S5 co-trust path; token-on-the-bus is the mcp-scope open question).

### store — the new `lb-jobs` crate (durable resumable session, no auth)

Mirrors `lb-inbox`/`lb-assets`: the `Job` record + raw `lb_store` verbs, workspace-namespaced, **no
authorization** (the agent service is the chokepoint). The transcript is **append-addressed**
(`steps[i]`), which is what makes resume idempotent. One verb per file: `create` / `load` /
`append_step` (upsert slot + advance cursor) / `complete`. The atomic-claim multi-worker queue is
deferred past S5 (jobs scope) — the single hub-hosted session has no contention.

### role — the `lb-role-ai-gateway` sidecar (swappable model access, mock provider)

Behind the stable contract: `AiRequest`/`AiResponse`/`ToolCall`/`ToolResult` + a `Provider` trait +
a deterministic `MockProvider` (the ONLY external stubbed — testing §3) + `AiGateway` with the
**replay-safe idempotency cache** (same key → cached response, never re-spent). **No loop, no tool
execution** — that is the agent's. Native async-fn-in-trait (no `async_trait` dep); generic over
`P: Provider` so the contract stays dependency-free.

### host — the `agent` service (the tool-call loop + the gates), beside `channel/` + `assets/`

The agent is a **host service** (not a wasm ext) because the loop must call `caps::check` per
dispatch, read S4 assets through the host verbs, and drive a job. One responsibility per file:

- **`model_access`** — the host-owned `ModelAccess` seam, so the host does NOT build-depend on the
  gateway *role* crate (roles depend on host — the role supplies a blanket `impl ModelAccess for
  AiGateway`). Model access only.
- **`authorize`** — the `mcp:agent.invoke:call` gate (gate 1, on the calling node).
- **`substrate`** — load the granted skill + read the shared doc **on the caller's behalf**: the
  caller's identity resolves the S4 membership/grant gate 3, the intersected caps bound gate 2. The
  agent reads what the caller may read — no privileged back door, no stranger to the caller's docs.
- **`run`** — the bounded loop (`MAX_STEPS`): ask the model → run each proposed tool call under the
  DERIVED principal (capability-checked, routed if remote; a denial is fed back, not a crash) →
  persist the step (idempotent) + advance the cursor → repeat → `complete`.
- **`invoke` / `resume`** — gate → substrate → loop; resume continues from the persisted cursor.
- **`serve` / `route` / `invoke_remote`** — the routed-MCP wiring: the hub declares a queryable on
  `ws/*/agent/invoke` (reusing the S3 seam), the edge authorizes locally then `query`s. `caps::check`
  on the CALLING node, workspace-first.

### MCP — the agent over the one contract

`agent.invoke` is reached through `lb_mcp::authorize_tool` (the same host-native bridge gate as
`assets.*`), workspace-first, then the loop. Two independent surfaces (MCP grant + each in-loop tool
grant), both enforced — invoking the agent never implies the tools it then calls.

### UI — minimal agent view + api client (mirrors the verb)

`lib/agent/{agent.types,agent.api}.ts` (one call per export) → `lib/ipc/agent.fake.ts` (a faithful
in-memory node: the invoke-cap gate + the substrate grant gate, so the UI's allow/deny paths are
exercised) → `features/agent/` (`useAgent` hook + `AgentView` + barrel). Wired into the `fake.ts`
dispatcher (agent commands first, then assets, then channel). No change to existing surfaces.

## Decisions & alternatives

- **The agent owns the loop; the gateway is a function** — the load-bearing split (ai-gateway
  scope). `AiGateway::complete` is one stateless turn; the host loop runs the proposed calls. This
  is what lets the gateway be a swappable sidecar. **Rejected:** a gateway that runs the loop — it
  would have to call `caps::check` and read the store, making it un-swappable core.
- **Delegation = exact intersection via a `constraint`, not pattern algebra** — a delegated
  principal carries its caps + the caller's as a second set; `check` requires both. Sound by
  construction (no cap-pattern intersection to get wrong), and it reuses the one `caps::check`
  chokepoint — there is no second authorization path. **Rejected:** computing a single merged cap
  list (pattern intersection of wildcards is subtle and a bug there is a privilege escalation).
- **Substrate reads on the caller's behalf (caller's sub), tool calls as `agent:session`** — gate 3
  for docs/skills is membership/ownership/grant, so substrate must resolve as the caller (the agent
  isn't the doc owner). Tool calls are ws+capability gated only (no membership), so a distinct
  `agent:*` sub is correct there (audit shows the agent acted). Capabilities still bound BOTH to
  `agent ∩ caller`. **Rejected:** the agent reading docs as `agent:session` — it owns nothing, so it
  could read nothing (the happy path failed exactly this way first; the fix is principled, not a
  workaround).
- **Resume idempotency = append-addressed transcript + gateway idempotency cache** — the agent half
  (re-applying `steps[i]` is a no-op; the cursor only moves past landed steps) and the gateway half
  (same idempotency key → cached response, no re-spend) together make a resumed session safe. The
  contended question "did this step complete?" becomes a `steps[i]` lookup. **Rejected:** a
  re-execute-from-scratch resume — it would re-spend budget and could diverge.
- **`ModelAccess` trait host-owned; the gateway role provides the impl** — keeps the symmetric
  layering rule (host never build-depends on a role). The host tests use the gateway as a DEV-dep
  (host(dev) → role → host(lib) is a legal dev cycle). **Rejected:** putting the contract types in a
  shared crate host depends on — unnecessary; the trait seam is smaller and clearer.
- **Routed agent carries the caller's caps on the wire (S5 co-trust)** — unlike a routed *tool*
  call (loop on the serving node), the agent loop runs on the HUB and needs the caller's grant. The
  request carries `caller_sub/ws/caps`; the workspace-scoped key still enforces isolation. Signing
  it (token-on-the-bus) is the mcp-scope "serve-side authorization" open question — recorded, not
  built (the in-process edge+hub are co-trusted). Named `Principal::routed` loudly so the trust
  assumption can't be used by accident.

## Tests

Mandatory categories (testing §2) — the S5 gate, not extras. Determinism held: all `ts`/ids
injected; a unique workspace id per test; multi-thread flavor on every Node-booting test (Zenoh
peer); the model provider is the **only** mock (real embedded SurrealDB + in-proc Zenoh + real wasm
everywhere else).

New this slice:

- **caps `delegation_test` (4)** — the intersection in both directions: a delegated actor can do
  what BOTH grant; cannot use a cap the caller lacks even if the agent holds it (no widening); nor
  one the agent lacks even if the caller holds it; delegation can't cross the workspace wall.
- **jobs `resume_test` (4)** — a session persists + resumes from its cursor; re-applying a persisted
  step is a no-op (idempotent resume); `complete` sets the terminal status; **store-layer ws
  isolation** (a ws-B load can't read a ws-A job).
- **gateway `gateway_test` (3)** — returns the provider's turn; a repeated idempotency key is served
  from cache, **not re-spent**; distinct keys advance the provider.
- **host `agent_test` (4)** — THE EXIT GATE (local): invoke → gateway turn → **granted tool call** →
  answer, over a granted skill + shared doc substrate; **invoke deny** (no `mcp:agent.invoke:call`);
  **in-loop tool deny via the intersection** (agent holds echo, caller doesn't → denied, fed back,
  loop still completes); **ungranted skill denied** to the agent.
- **host `agent_isolation_test` (2)** — **MANDATORY ws-isolation** (store + MCP): a ws-B agent can't
  read a ws-A substrate doc; an agent's job is invisible across the wall.
- **host `agent_offline_test` (2)** — **MANDATORY offline/sync**: a session interrupted mid-loop
  **resumes from its cursor** (step 0 survived, not re-run; step 1 appended; done); a duplicated
  invocation **does not double-apply or re-spend** (same answer, same provider-call count, one step).
- **host `agent_routed_test` (3)** — THE EXIT GATE (routing): an edge **invokes the hub agent over
  the routed namespace** (the hub runs the loop + a hub-hosted granted tool, replies); routed
  **deny** on the edge (never leaves it); routed **ws-isolation** (ws-B can't route into ws-A).
- **ui `AgentView.test.tsx` (3, Vitest)** — a user with the invoke grant gets the answer; **without
  it is denied** (the gate surfaced to the user); a granted skill succeeds, an ungranted one denied
  — driving the real `agent.api` → `invoke` → fake path.

### Green output

Run per-binary / bounded parallelism — node-booting tests make a single `cargo test --workspace`
OOM (debugging/bus/cargo-test-workspace-ooms-with-many-peers.md).

```
# Rust — light crates (real embedded SurrealDB)
$ cargo test -p lb-jobs                 → 4 passed   # NEW: resume + idempotency + ws isolation
$ cargo test -p lb-role-ai-gateway      → 3 passed   # NEW: gateway turn + replay-safe cache
$ cargo test -p lb-caps                 → 22 passed  # +4 delegation (was 18)
  auth 4   inbox 4   bus 2   ext-loader 2   store 5   assets 8   → 25 (S1–S4, unchanged)
  light total: 54 passed   (was 43 at S4; +4 caps +4 jobs +3 gateway)

# Rust — host integration (real wasm + real SurrealDB + Zenoh)
$ cargo test -p lb-host --test spine_test               → 4 passed
$ cargo test -p lb-host --test messaging_test           → 3 passed
$ cargo test -p lb-host --test messaging_deny_test      → 3 passed
$ cargo test -p lb-host --test messaging_isolation_test → 2 passed
$ cargo test -p lb-host --test presence_test            → 2 passed
$ cargo test -p lb-host --test hot_reload_test          → 2 passed
$ cargo test -p lb-host --test cross_node_routing_test  → 3 passed
$ cargo test -p lb-host --test offline_sync_test        → 3 passed
$ cargo test -p lb-host --test assets_doc_test          → 6 passed
$ cargo test -p lb-host --test assets_skill_test        → 3 passed
$ cargo test -p lb-host --test assets_isolation_test    → 3 passed
$ cargo test -p lb-host --test assets_mcp_test          → 4 passed
$ cargo test -p lb-host --test install_record_test      → 2 passed
$ cargo test -p lb-host --test agent_test               → 4 passed   # NEW: EXIT GATE (local) + deny
$ cargo test -p lb-host --test agent_isolation_test     → 2 passed   # NEW: MANDATORY ws-isolation
$ cargo test -p lb-host --test agent_offline_test       → 2 passed   # NEW: MANDATORY offline/resume
$ cargo test -p lb-host --test agent_routed_test        → 3 passed   # NEW: EXIT GATE (routed edge→hub)
   host total: 51 passed   (was 40 at S4; +11 agent)

   RUST TOTAL: 105 passed, 0 failed   (was 83 at S4; +22)

# Tauri shell command layer (headless) — unchanged, still green
$ cd ui/src-tauri && cargo test          → 2 passed

# UI (Vitest) + type-check + bundle
$ cd ui && pnpm test                     → 14 passed (5 files)   # +3: AgentView invoke gate
  ChannelView 3   channel.api 3   useChannel 2   DocView 3   AgentView 3
$ pnpm build                             → tsc --noEmit clean; vite build ✓

# Formatting + file size
$ cargo fmt --all --check                → FMT OK
$ bash rust/scripts/check-file-size.sh   → all source files within 400 lines (175 checked)
```

## Debugging

One non-trivial discovery, with an entry:

- [agent/agent-reads-doc-it-doesnt-own-is-denied](../../debugging/agent/agent-reads-doc-it-doesnt-own-is-denied.md)
  — the happy path failed `Denied`: the agent's derived principal had sub `agent:session`, so the S4
  doc membership gate (owner = `user:ada`) refused the substrate read. Root cause: substrate reads
  are membership-gated and must resolve as the **caller**, not a distinct agent actor. Fixed by
  reading substrate on the caller's behalf (caller's sub + intersected caps); tool calls keep the
  `agent:*` sub (they're ws+cap-gated only). Regression: `agent_test::an_edge_user_invokes…` (the
  doc-substrate happy path) + `agent_isolation_test` (the caller's-behalf read still can't cross ws).

## Public / scope updates

- Promoted to `public/`: `agent` (new — the central agent, the loop, the intersection, the durable
  session). Refreshed `public/SCOPE.md` with the S5 row.
- Resolved/refreshed open questions: `auth-caps` (grant **delegation** landed — `derive` +
  gate 2b, the intersection); `jobs` (record shape finalized; queue deferred; "what shipped in S5"
  added); `ai-gateway` ("what shipped in S5" — contract + mock + idempotency cache); `agent` (the
  derived-sub, the loop ceiling, and outbox integration recorded as follow-ups).

## Follow-ups

- **Real provider adapter** behind the gateway contract (OpenAI-compatible / SpiceAI / local) — the
  mock is the only stub; the trait seam is ready.
- **Streaming** progress as Zenoh motion to a channel + the durable transcript via outbox (§6.10) —
  S5 persists the job and returns the final answer; live token streaming + the cursor-driven outbox
  relay is the next slice (and the S6 coding-workflow driver).
- **Token-on-the-bus** for routed agent invocations (sign the carried caller grant) — the mcp-scope
  "serve-side authorization" open question; S5 is in-process co-trust.
- **Gateway/Tauri wiring for `agent_invoke`** — the UI reaches the fake in tests; route it through
  the SSE/HTTP gateway + Tauri shell to a real node (mirrors the S3 channel transport swap, the same
  follow-up as `assets_*`).
- **Per-workspace loop policy** (ceiling, model class, local-only flag) as a §6.6 scope projection.
- STATUS.md updated? **Yes** — AI core slice marked `shipped`; S5 exit gate met.
