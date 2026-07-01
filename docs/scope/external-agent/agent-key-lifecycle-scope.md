# External-agent scope — the scoped-token lifecycle across a long resumable run

Status: scope (the ask). Sub-scope of `model-routing-scope.md` (#4) × `run-lifecycle-scope.md` (#5).
Promotes to `public/external-agent/` once shipped.

The external agent reaches a model **only** through our gateway, authenticated by a **short-lived,
workspace-scoped token** — never a provider key (#4). But a run can outlive a short token: an
external-agent run is a durable job that survives an edge disconnect and **resumes** (#5), possibly
hours later, on a possibly-different node. Both #4 and #5 flag the same open question and never resolve
it: *what is the token's lifetime, how is it refreshed across resume, and how is it revoked?* Get it
wrong and either long runs break mid-stream when the token expires, or tokens live too long and a
revoked grant keeps spending. This scope owns exactly that seam — the **mint → refresh-on-resume →
revoke** lifecycle of the model token — as the boundary between #4 (what the token authorizes) and #5
(the durable run that consumes it).

## Goals

- **Mint per dispatch, never persist the secret.** The job record stores the run's **principal**
  (`caller ∩ agent`, ws, run id), **not** a token. The token is minted fresh at each dispatch/resume
  from that principal and handed to the subprocess in-memory only — so a leaked job record leaks no
  credential.
- **Refresh on resume, re-deriving authority.** When a run resumes (#5), the token is re-minted from the
  stored principal **after re-checking the grant** — so authorization is re-evaluated at every resume,
  not frozen at run start.
- **Short-lived + renewable within a live run.** During a single continuous run, a short token is
  transparently renewed before expiry (the run isn't interrupted); across a resume it is re-minted. One
  policy, two triggers (expiry-during-run, resume-after-suspend).
- **Revocable, and revocation actually bites.** Revoking the run's grant (or killing the run) makes the
  **next** mint/refresh fail — a long run cannot outlive its authorization. Expiry + gateway refusal is
  the revocation mechanism (#4); this scope makes it bite at every refresh point.
- **Run id in the claims, always.** Every minted token carries `{ws, run_id, principal}` so the gateway
  attributes spend to the run (#4's audit requirement) and can refuse a token whose run is
  cancelled/dead.

## Non-goals

- **The served OpenAI-compatible endpoint** — that is #4's blocking ai-gateway dependency; this scope
  assumes it exists and specifies only the token's lifecycle on it.
- **The gateway's token *verification* internals** — the gateway verifies + attributes (ai-gateway +
  #4); this scope owns the *node-side* mint/refresh/revoke and the run-side triggers.
- **Provider key management** — provider keys stay in the gateway secret store (§6.7), never here.
- **Foreign-loop resume mechanics** (ACP `session/load` vs restart-from-goal) — that's #5; this scope
  only ensures a valid token exists at each resume, whichever resume strategy #5 picks.
- **Subprocess supervision / kill** — #5; this scope reacts to a kill (revoke the token) but doesn't own
  the killing.

## Intent / approach

**The principal is durable; the token is ephemeral and derived.** This is the same split the platform
already makes everywhere: authority is a **grant** (durable, in the store, revocable), and a token is a
**short-lived projection** of it. The in-house loop never persists a token — it calls `ModelAccess`
which mints/uses credentials per call behind the gateway. An external subprocess can't call
`ModelAccess` (it's foreign), so it needs a bearer — but we keep the same discipline: **mint the bearer
from the durable principal at the moment of use, never store the bearer.**

So the run job stores `{ goal, profile_id, caller, ws, run_id }` (as #5 already specifies) and — the
one addition — nothing else credential-like. At three trigger points a token is minted:

1. **Run start** — mint from the principal, inject into the subprocess env/config (#4's `base_url` +
   bearer), launch.
2. **Mid-run renewal** — a short token nears expiry during a *continuous* run; a node-side refresher
   re-mints (same principal, no grant re-check needed mid-continuous-run — the grant hasn't been
   re-evaluated because nothing suspended) and rotates the subprocess's bearer before the old one
   expires, so the run never sees an auth failure.
3. **Resume** — a suspended run resumes (#5): **re-check the grant** (the principal may have been
   revoked while suspended), and only if it still holds, mint a fresh token and relaunch/continue. A
   revoked grant → the resume fails closed, the run ends `failed`/`cancelled`, no token minted.

**Why re-check at resume but not mid-continuous-run?** Mid-run, the grant was checked at start and the
run has been continuously authorized; re-minting is a mechanical rotation. At resume, arbitrary time has
passed and the world may have changed — resume is exactly the boundary where re-authorization is cheap
and necessary. This mirrors the reminder scope's principle: **check caps at fire time, not create time**
— here, check at *resume* time, not *start* time.

**Revocation = expiry + refusal, made to bite.** #4 defines revocation as "expiry + the gateway refusing
it." This scope makes that concrete: because the token is short-lived and every renewal/resume re-mints
(and resume re-checks the grant), a revoked grant stops the **next** mint. Worst case a run keeps
spending until the *current* short token expires — so the token TTL is the revocation-latency ceiling.
Pick it deliberately (see open questions). A hard cancel (#5's kill) *also* immediately marks the run
dead so the gateway refuses even an unexpired token whose `run_id` is cancelled — belt (TTL) and braces
(run-status check in the gateway).

**Rejected:** a single run-length token minted once at start (what a naive reading of #4's "run-scoped
token" suggests). Rejected because a run's length is unbounded and unknowable, a long-lived bearer in a
subprocess is a fat credential to leak, and it freezes authorization at start — a grant revoked
mid-run couldn't bite until the run ended. Short-token-plus-refresh keeps the credential small and
re-authorizes at every resume. **Also rejected:** persisting the token in the job record for resume —
it would put a (possibly still-valid) credential in durable storage that syncs across nodes; re-minting
from the principal is strictly safer and costs one gateway call.

## How it fits the core

- **Tenancy / isolation:** every minted token is bound to the run's **one workspace**; a ws-B resume
  cannot mint a token that spends ws-A budget or reads ws-A provider config (#4's ws-first policy). The
  `run_id` + `ws` in the claims are the wall. **Mandatory isolation test.**
- **Capabilities:** no new *caller* cap — the run already required `mcp:agent.invoke:call` (#5). The
  token projects the **derived principal's model policy** (#4). The lifecycle addition is: **resume
  re-runs the grant check** that start ran, so a revoked grant fails the resume. Deny path: revoked
  grant → resume mints nothing → run ends failed, logged, no spend.
- **Placement:** either. A run may start on the hub and resume on an edge (or vice versa); because the
  token is re-minted from the durable principal at resume, **it doesn't matter which node resumes** —
  there's no node-pinned credential to carry. The edge mints against its local gateway (#4's local
  resolution). No `if cloud`.
- **MCP surface:** N/A — token mint/refresh is a node-internal operation against the gateway, not an MCP
  verb (like #4, model access is HTTP, not MCP). The run itself is observed via `agent.watch`/`job.watch`
  (#5, `jobs/job-control-scope.md`).
- **Data (SurrealDB):** **no token persisted.** The job record keeps `{goal, profile_id, caller, ws,
  run_id}` (#5, unchanged) — this scope's whole point is that nothing credential-like is added to it. The
  gateway's audit/usage records carry the `run_id` attribution (#4). State: the durable principal; the
  token is never state.
- **Bus (Zenoh):** N/A — the token rides the subprocess's HTTP-to-gateway pipe (local motion), never the
  bus (#4/#5). A run-cancelled signal reaches the gateway via the run-status record it checks, not a bus
  message.
- **Sync / authority:** the principal is workspace-authoritative and syncs normally; the token is
  node-local ephemeral and **never syncs**. A run suspended on one node and resumed on another re-mints
  locally from the synced principal — the offline/resume story #5 tests, with the credential re-derived
  not carried.
- **Secrets (the core):** provider keys stay envelope-encrypted in the gateway (§6.7), never handed to
  the agent (#4). The minted bearer is short-lived, ws+run-scoped, in-memory only, and rotated before
  expiry. Revocation is expiry + gateway refusal + run-status refusal. This is the `role/acp`
  trusted-session-token pattern (#4) with an explicit refresh/resume lifecycle.
- **No fake backend (rule 9):** the gateway endpoint + token verification are **real**; the one permitted
  fake is the provider HTTP behind the gateway (#4's `MockProvider`). Tests mint a **real** token, present
  it to the **real** gateway endpoint, and script only the provider response — exercising mint → refresh →
  revoke against the real auth path with no network.
- **Stateless:** the node-side refresher holds only the ephemeral current token in memory; the authority
  is the durable principal. A node restart re-mints on resume from the principal — no durable per-instance
  token state (hot-reload / restart safe).
- **One responsibility per file (FILE-LAYOUT):** `role/acp` (or the runtime crate) gets `token/mint.rs`
  (principal → token), `token/refresh.rs` (the mid-run rotator), `token/resume.rs` (re-check grant +
  re-mint). No `token.rs` grab-bag. The run-status "is this run still live" check the gateway consults is
  one field on the job record (#5).
- **SDK/WIT impact:** none on the guest ABI — this is host/runtime + gateway, not the WASM boundary.

## Example flow

A run starts on the hub, the edge disconnects, and it resumes after the grant was tightened:

1. **Start (hub):** the node mints a short token from the run's principal `{ws, run_id, caller∩agent}`,
   sets the subprocess `base_url` = gateway endpoint + that token, launches. The job record stores the
   principal — **no token**.
2. **Mid-run renewal:** 4 minutes into a 20-minute run the token nears its 5-minute TTL; the refresher
   re-mints (same principal) and rotates the subprocess's bearer transparently. The agent never sees an
   auth error; each model call is audited with `run_id`.
3. **Suspend:** the edge hosting the run disconnects; the run suspends (#5). No token persists anywhere —
   the last one simply expires unused.
4. **Grant tightened:** meanwhile an admin narrows the agent's model policy (still granted, lower budget).
5. **Resume (edge, on reconnect):** #5 resumes the run; this scope **re-checks the grant** → still held →
   mints a fresh token against the edge's **local** gateway with the tightened policy now in force. The
   run continues under the *new* budget — authorization was re-evaluated at resume, not frozen at start.
6. **Revoke variant:** had the admin *revoked* the agent grant during the suspend, step 5's re-check
   fails → no token minted → the run ends `failed` (logged, opaque), zero further spend. A hard
   `agent.cancel` mid-run would additionally mark `run_id` dead so the gateway refuses even the unexpired
   current token immediately.

## Testing plan

Per `scope/testing/testing-scope.md`; real gateway + real token verification, scripted provider only
(rule 9, §0).

- **Capability-deny (mandatory):** a run whose grant is **revoked while suspended** fails to resume — the
  re-check denies, no token is minted, the run ends failed with no further model call. Assert zero spend
  after revocation.
- **Workspace-isolation (mandatory):** a ws-B resume mints a ws-B-scoped token only; it cannot spend ws-A
  budget or read ws-A provider config; a token with ws-A `run_id` presented in ws-B is refused by the
  gateway.
- **No persisted credential:** assert the job record **never** contains a token/bearer at any lifecycle
  point (start, mid-run, suspended, resumed) — only the principal. Inspect the stored record directly.
- **Mid-run renewal is seamless:** a run longer than the token TTL completes with **no** auth failure
  visible to the agent (the rotator refreshed before expiry). Assert continuity across ≥2 renewals.
- **Resume re-mints + re-authorizes:** a run suspended then resumed mints a fresh token reflecting a
  **changed policy** (tightened budget) applied on resume — proving authorization is re-evaluated, not
  frozen at start.
- **Cross-node resume:** a run started on node A and resumed on node B mints locally on B from the synced
  principal (no node-pinned credential carried). Reuse #5's offline/resume harness.
- **Revocation latency bound:** after a grant revoke with no hard cancel, spend stops within the token
  TTL (the current short token expires and is not renewed). Assert the TTL is the ceiling.
- **Hard-cancel immediacy:** `agent.cancel` marks `run_id` dead; the gateway refuses even the unexpired
  current token immediately (run-status check), not only at TTL. Assert refusal before expiry.
- **Attribution (with #4):** every model call across start/renewal/resume is audited with the run id +
  agent + caller + ws.

## Risks & hard problems

- **Renewal race at expiry.** If the rotator re-mints too late, an in-flight model call fails auth mid-run.
  **Mitigation:** refresh at a safe fraction of TTL (e.g. 60%), and have the runtime **retry once** on a
  401 by forcing an immediate re-mint before surfacing an error — so a narrow race self-heals.
- **Revocation latency vs run continuity.** Short TTL → fast revocation but more re-mint churn; long TTL →
  smoother runs but a revoked grant keeps spending longer. This is a real trade the TTL sets. **Mitigation:**
  pick a short default (minutes), back it with the immediate run-status refusal on hard cancel so hard-stop
  is instant and only *soft* revoke waits for TTL.
- **Cross-node clock skew on TTL.** A token minted on the hub and validated on an edge with a skewed clock
  could be seen expired/valid wrongly. **Mitigation:** the gateway validates on **its** clock at the point
  of use (the token is validated where it's spent), and the injected-logical-clock discipline (testing §3)
  keeps tests deterministic; allow a small leeway window.
- **Re-mint storm on a flapping edge.** An edge that repeatedly disconnects/reconnects could re-mint on
  every resume. **Mitigation:** resume is already rate-limited by #5's supervision/backoff; a re-mint is
  cheap (one gateway call) and gated behind the grant re-check, so a flap can't escalate privilege — only
  churn, bounded by backoff.
- **Forgetting the run-status refusal.** If the gateway only checks TTL and not run-status, a hard-cancelled
  run could still spend on its unexpired token. **Mitigation:** the gateway's token check MUST consult the
  run's `status` (the `run_id` claim → job record) — this is the braces to the TTL belt; tested explicitly.

## Open questions

- **Token TTL default:** what short lifetime balances revocation latency against renewal churn — 5
  minutes? Tie it to the prompt-cache/run cadence? (Recommend ~5 min default, configurable per profile;
  it's the soft-revocation ceiling.)
- **Refresh fraction:** re-mint at 60% of TTL, or on a fixed lead time before expiry? (Recommend a
  fraction so it scales with the chosen TTL.)
- **Does the gateway *need* the run-status check, or is short TTL enough for v1?** (Recommend include the
  run-status refusal — it's what makes hard-cancel instant; TTL-only leaves a spend window after cancel.)
- **Where the mint lives:** in `role/acp` (reusing its trusted-session-token minter) or in the runtime
  crate that owns the subprocess? (Recommend reuse `role/acp`'s minter — same pattern, one place —
  invoked by the runtime.)
- **Streaming + renewal:** if #4 ships streaming, does a mid-stream renewal require re-establishing the
  stream, or can the bearer rotate on the next request only? (Depends on #4's streaming decision; flag as
  joint.)

## Related

- `scope/external-agent/model-routing-scope.md` (#4) — what the token authorizes, the gateway
  OpenAI-compatible face, the audit attribution; **this scope resolves #4's "token lifetime vs long runs"
  open question.**
- `scope/external-agent/run-lifecycle-scope.md` (#5) — the durable run + resume + supervision this hooks
  its mint/refresh/revoke triggers into; **resolves #5's "model token lifetime across a long run" non-goal
  hand-off.**
- `scope/external-agent/capability-wall-scope.md` (#3) — the sandbox that makes the gateway socket the
  *only* reachable egress, so a leaked-in-memory token still can't reach a provider directly.
- `scope/external-agent/external-agent-scope.md` — the umbrella + `AgentProfile` (`model_endpoint_ref`).
- `scope/jobs/job-control-scope.md` — `job.cancel`/`agent.cancel` is the hard-stop that marks `run_id`
  dead for the gateway's run-status refusal.
- `scope/ai-gateway/ai-gateway-scope.md` — the served endpoint + token verification this mints against.
- `scope/secrets/` — the `role/acp` trusted-token minter pattern reused; provider keys stay here, §6.7.
- `README.md` §6.14/§6.15 (gateway), §6.7 (secrets), §6.9 (jobs), §7 (tenancy).
- `public/external-agent/external-agent.md` — promotion target on ship.
</content>
