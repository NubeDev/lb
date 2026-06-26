# Project scope (as built)

The trimmed source of truth for what exists now. The full architecture spec is the root
`README.md`; the staged plan is `../STAGES.md`; live status is `../STATUS.md`.

## Shipped (S7 — platform maturity: the signed extension registry)

The first S7 vertical slice: a node installs an extension from a **signed registry**, runs it
**offline once cached**, and **rolls back** to a prior version — built by composing the S4 install
flow with a new artifact-verification crate, not by re-cutting either.

- **`lb-registry`** — artifact identity + the one new crypto surface. A `digest(manifest, wasm)`
  binds the manifest AND the bytes (SHA-256, length-prefixed framing); `verify_artifact` recomputes
  the digest and Ed25519-verifies the signature against an allow-listed publisher key, returning a
  **`VerifiedArtifact` newtype it alone can mint**. Reuses the `lb_auth` `ed25519-dalek` idiom — no
  second crypto stack. See `registry/registry.md`.
- **The host `registry` service** (beside `agent`/`channel`/`assets`/`workflow`) — `pull` (fetch ·
  **verify** · cache, serving the cache **without a source call** when present — the offline path),
  `install_from_registry` (pull THEN the **existing** S4 `install_extension`; **rollback = the same
  verb with a prior version**), `list`/`resolve` over MCP. The fetch is behind a host-owned `Source`
  trait (the outbox `Target` / agent `ModelAccess` analogue); `cache_artifact` accepts only a
  `VerifiedArtifact`, so **verify-before-cache is a compile-time guarantee**. Cache + catalog are
  SurrealDB records in the workspace namespace (no second datastore; isolation is structural). See
  `registry/registry.md`.
- **Two independent gates** — the **capability** gate (`mcp:registry.<verb>:call`, workspace-first)
  and the **signature** gate (`verify_artifact`). Granted ≠ trusted: a fully-granted caller is still
  refused a tampered/unsigned/untrusted artifact, and a trusted artifact still needs the grant.
- **UI** — a `RegistryView` + `registry.api` client mirroring the verbs, with a faithful in-memory
  fake exercising the capability gate, the signature gate, and rollback. See `frontend/frontend.md`.

**Exit gate — first half MET.** An extension installs from the signed registry; runs offline once
cached (zero source calls on the cached path); rolls back to a prior version with **no durable
workspace state lost** (a channel message + job step survive an N→N−1 install through the real wasm);
a tampered/unsigned/foreign-key artifact is rejected before caching, even with the grant. **145 Rust +
22 Vitest + 2 shell tests** pass — incl. capability-deny (each registry verb), workspace-isolation
across **store + MCP** (a ws-B caller sees no ws-A cache/catalog and cannot ride its cache offline),
offline (install succeeds with the source unreachable once cached), rollback/hot-reload (durable state
preserved), and signing/verification (the new crypto surface). The native Tier-2 sidecar — the exit
gate's second half — remains.

## Shipped (S6 — coding workflow)

A vertical slice composing the S5 agent + jobs with a new must-deliver outbox — the worked example
end to end (vision `0002`), built **entirely from core primitives** (the core never learns "coding
agent").

- **The transactional must-deliver outbox** — a new `lb-outbox` crate: an `Effect` record + raw
  verbs. The pattern: `enqueue` (over the new `lb_store::write_tx`) writes the **domain change AND
  the effect in one transaction** — both commit or neither. `relay_outbox` delivers `pending`
  at-least-once through a host-owned `Target` trait, retrying `failed` rows; the receiver dedups on
  `idempotency_key` (never lost, never double-sent). See `inbox-outbox/inbox-outbox.md`.
- **The inbox resolution facet** — `lb_inbox::Resolution` (approve/reject/defer + actor + ts), a
  sibling record keyed by the item id (the `Item` shape stayed stable). The subject of the approval
  gate. See `inbox-outbox/inbox-outbox.md`.
- **The host `workflow` service** (beside `agent`/`channel`/`assets`) — the orchestrator, holding no
  durable state: `ingest_issue` → `triage` (drives the S5 agent over MCP to draft + `share_doc` a
  scope doc) → `request_approval` → `resolve_approval` → `start_coding_job` (**THE GATE**: starts the
  durable job only on `Approved`, creating nothing otherwise) → `emit_effect` (job step + effect in
  one transaction) → `relay_outbox`. `workflow.*` is reached over the one MCP contract. See
  `coding-workflow/coding-workflow.md`.
- **UI** — a `WorkflowView` + `workflow.api` client mirroring the verbs, with a faithful in-memory
  fake exercising the capability + approval gates and the outbox. See `frontend/frontend.md`.

**Exit gate met.** The full flow runs; the approval **genuinely gates** the job (no job record
before approval; refused with `AwaitingApproval`; a rejected approval starts nothing); every external
effect goes through the outbox with retry. **124 Rust + 18 Vitest + 2 shell tests** pass — incl.
capability-deny (each workflow verb), workspace-isolation across **store + MCP** (a ws-B caller/relay
sees no ws-A state), and offline/sync (an effect survives an outage and is delivered at-least-once,
idempotently — never lost, never double-sent).

## Shipped (S5 — AI core)

A vertical slice through every layer: a central agent, the swappable gateway, and durable jobs.

- **Central AI agent** — a host `agent` service (beside `channel`/`assets`): a workspace-scoped
  actor that **owns the tool-call loop** (ask the gateway for a turn → run each proposed tool call,
  capability-checked → feed results back → repeat, bounded by `MAX_STEPS`). Reached over MCP
  (`mcp:agent.invoke:call`) and over the **routed namespace** — an edge invokes the hub agent via a
  Zenoh queryable (`ws/*/agent/invoke`, the S3 routing seam), `caps::check` on the calling node.
  See `agent/agent.md`.
- **Grant delegation (the intersection)** — the agent acts under `agent ∩ caller`:
  `Principal::derive` mints a strictly **narrower** actor, and `caps::check` gained **gate 2b** (a
  delegated request must match the caller's caps too). An agent can never widen its own access.
  Substrate reads (granted skill + shared doc) run **on the caller's behalf** (caller's identity for
  the S4 membership/grant gate, intersected caps for the capability gate). See `auth-caps/auth-caps.md`.
- **Swappable AI-gateway sidecar** — `lb-role-ai-gateway` behind a stable contract
  (`AiRequest`/`AiResponse`/`ToolCall`): **model access only, no loop** (that's the agent's). A
  `Provider` trait + a deterministic mock (the only external stubbed) + a **replay-safe idempotency
  cache** (same key → cached response, never re-spent). One contract, many implementations.
- **Durable resumable jobs** — a new `lb-jobs` crate: the session is a `job:{id}` record with an
  **append-addressed transcript** + a cursor (no separate datastore, §3.2). Resume continues from
  the cursor; re-applying a persisted step is a no-op. The atomic-claim multi-worker queue is
  deferred (jobs scope). See `../scope/jobs/jobs-scope.md`.
- **UI** — an `AgentView` + `agent.api` client mirroring the verb, with a faithful in-memory fake
  exercising the invoke + grant gates. See `frontend/frontend.md`.

**Exit gate met.** An edge user invokes the central agent; the agent calls the gateway for a model
and a granted MCP tool; a workflow job survives the edge disconnecting and resumes idempotently.
**105 Rust + 14 Vitest + 2 shell tests** pass — incl. capability-deny (invoke gate + the in-loop
intersection), workspace-isolation across **store + MCP**, and offline/sync (interrupted session
resumes from its cursor; a duplicated invocation does not double-apply or re-spend).

## Shipped (S4 — shared workspace assets)

A vertical slice through every layer, building the asset substrate the AI workflows (S5–S6) stand on:

- **Docs as workspace assets** — a new `lb-assets` crate (the store side: `Doc`/`Skill`/relation/
  install models + raw verbs) + a host `assets` service (the auth side). A doc read passes **three
  gates in order**: workspace (namespace) → capability (`store:doc/*:read`, no grammar change) →
  **membership** (owner / shared-team-member / linked-channel-`sub`-grantee — the layer tenancy
  deferred). Sharing is a live graph relation, not a content copy; revoke = delete one edge. See
  `files/files.md`.
- **Content as a record, not a bucket** — `DEFINE BUCKET` isn't in our embedded `kv-mem` build, so
  asset content is stored as a record value behind a bucket-compatible verb (S7 swaps the backend
  by config). See `../debugging/store/define-bucket-unavailable-in-kv-mem-build.md`.
- **Skills as versioned, grant-gated assets** — `skill:{id}@{version}` immutable per version
  (rollback = a prior version); `load_skill` returns the body **only when the workspace granted the
  skill** (`grant:skill/{id}` relation) — the §6.12 "load only when granted" rule. See `skills/skills.md`.
- **Extension install records** — `install_extension` persists `granted = requested ∩ admin_approved`
  as an `install:{ext_id}` record (closing the S1 deferral); `installed` reads it back,
  workspace-isolated. See `extensions/extensions.md`.
- **`assets.*` over MCP** — the verbs are reachable through the one MCP contract via a host-native
  bridge (`call_asset_tool`): the MCP gate (`mcp:assets.*:call`, workspace-first) then the verb's
  own store + membership gate. See `mcp/mcp.md`.
- **UI** — a `DocView` + `assets.api` client mirroring the verbs, with a faithful in-memory fake
  exercising the allow/deny paths. See `frontend/frontend.md`.

**Exit gate met.** A doc private to a user can be shared to a team and linked into a channel; a
non-member is denied; a skill loads only when granted. **83 Rust + 11 Vitest + 2 shell tests** pass
— incl. capability-deny (non-member / no-grant) and workspace-isolation across **store + MCP**.

## Shipped (S3 — multi-node / sync / SSE)

A vertical slice through every layer, standing up a second node and the browser path:

- **Node roles as config** — `lb_host::Role` (`Edge | Hub | Solo`) + `Node::boot_as(role)`. Same
  binary, same crates; the only role-derived policy is data authority (§6.8). A second node is just
  a second `boot_as` (two in-process Zenoh peers auto-discover). No `if cloud` anywhere. See
  `sync/sync.md`.
- **Cross-node MCP routing** — the dispatch seam is real: a tool call on the edge routes over a
  Zenoh **queryable** (`mcp/{ext}/call`) to the hosting hub, callers + `authorize` unchanged,
  `caps::check` on the calling node workspace-first. See `mcp/mcp.md`. New bus primitive:
  `declare_queryable`/`query` (`bus/bus.md`).
- **Channel sync (edge↔hub, §6.8 append-style subset)** — `sync_channel` idempotently applies bus
  items into the local store; `replay_history` catches a reconnecting node up. Offline write →
  reconnect → idempotent merge, conflict-free (inbox upserts on `(channel,id)`). See `sync/sync.md`.
- **SSE/HTTP gateway** — `lb-role-gateway` (axum): POST/GET `/channels/{cid}/messages` + SSE
  `/channels/{cid}/stream` (live `message` + `presence`). Every route forwards to a
  capability-checked `lb_host` verb. The browser reaches a real node; only `ui/lib/ipc/invoke.ts`
  swapped transport. See `frontend/frontend.md`.

**Exit gate met.** A second node joins; a cross-node tool call routes and is capability-checked;
channel data syncs edge↔hub with idempotent offline apply; the browser reaches a node over SSE/HTTP
(replacing the S2 in-memory fake) and sees live messages appear. **61 Rust + 8 Vitest + 2 shell
tests** pass — incl. capability-deny, workspace-isolation, and the first offline/sync categories,
all now **across two nodes** and the gateway.

## Shipped (S2 — first app: messaging + UI + hot-reload)

A vertical slice through every layer, on top of the S1 spine:

- **Bus** — Zenoh pub/sub (`publish`/`subscribe`) + **presence** via liveliness tokens, all
  workspace-scoped (`ws/{id}/chan/{cid}/**`). See `bus/bus.md`.
- **Inbox** — a normalized `Item` model persisted to SurrealDB; idempotent on `(channel,id)`,
  ordered by `ts`. See `inbox-outbox/inbox-outbox.md`.
- **Channels (state vs motion)** — the host `channel` service: post → `caps::check` → **persist
  (store) then publish (bus)**; history reads the durable record. The capability chokepoint for
  messaging (`bus:chan/{cid}:{pub|sub}`).
- **Store** — added a workspace-scoped `list` filter verb. See `store/store.md`.
- **UI** — a React + Tailwind **channel view** in a Tauri v2 shell, talking to the in-process
  node over IPC; the api client mirrors the Rust verbs. See `frontend/frontend.md`.
- **Hot-reload** — `reload_extension` swaps a live component (hello v1→v2) with **no durable
  state lost** (the stateless-extension guarantee).

**Exit gate met.** Post a message in the UI and it appears (Vitest); history survives independent
of the bus / a restart (the store keeps it); an extension version swaps live with state intact.
54 Rust tests + 6 Vitest + 2 shell tests pass — incl. the mandatory capability-deny,
workspace-isolation (bus+store+inbox), and hot-reload categories.

## Shipped (S0 + S1 — the spine)

A single Rust binary (`node`) built from a Cargo workspace, running a **solo node** that
proves the capability model end to end:

- **Workspace** (`rust/`) — the §9 crate split: real `host store bus runtime mcp auth caps
  ext-loader`, plus stubs for `tags inbox jobs secrets sync` and placeholder `role/*` crates.
  See `crate-layout/crate-layout.md`.
- **Stable extension ABI** — a WASI 0.2 Component-Model world in `sdk/wit/world.wit`
  (`lazybones:ext/extension@0.1.0`), versioned; host + guest generated from the one file.
- **Capabilities** — `<surface>:<resource>:<action>` grammar; an Ed25519 JWT with a single
  `ws` claim; a two-gate check (workspace-first, then capability) as the single chokepoint.
  See `auth-caps/auth-caps.md`.
- **MCP** — `call → authorize → resolve → dispatch`; authorize before resolve so denials don't
  leak tool existence. See `mcp/mcp.md`.
- **Store** — embedded SurrealDB, workspace = namespace (isolation is structural).
- **Bus** — embedded Zenoh peer + workspace key prefixing (pub/sub at S2).
- **Runtime** — wasmtime component host; loads `extensions/hello` and answers `hello.echo`.
- **CI** — FILE-LAYOUT size check + build (incl. the wasm guest) + test + fmt.

**Exit gates met.** S0: `cargo build` green; CI runs; manifest + capability grammar written as
scope docs. S1: tool call refused without the grant / allowed with it; cross-workspace data is
invisible. 35 tests pass (mandatory capability-deny + workspace-isolation included).

## The four forever decisions (resolved in S0)

| Decision | Choice | Where |
|---|---|---|
| SDK/WIT boundary | WASI 0.2 Component-Model world, semver-versioned | `crate-layout/` |
| Capability grammar + token | `surface:resource:action` + Ed25519 JWT, two-gate check | `auth-caps/` |
| Job queue | thin **native** SurrealDB queue (not apalis); resumable-session subset built at S5 | `../scope/jobs/` |
| Extension manifest | **TOML** `extension.toml`; host grants `requested ∩ approved` | `../scope/extensions/` |

## Not yet built

The **native Tier-2 sidecar** tier (S7 exit gate's second half) and **packaging the S6
workflow/github-bridge as installed wasm artifacts** (now that the registry exists to install them
through). For the registry itself: a **real HTTP `Source`/`registry-host` server** (the in-memory test
source is the only stub), a **durable publisher-key allow-list** + the admin trust-management flow,
**key rotation/revocation** (needs the hub identity directory), **cache eviction/GC**, the **public
catalog read-only union** (S7 ships per-workspace catalog entries), and **`registry.update`** semantics.
A **real model provider** behind the gateway contract (the S5 mock is the only stub) and **streaming**
agent progress as Zenoh motion remain. The outbox's **real `Target`
adapters** (GitHub HTTP, email, sync), **backoff + dead-letter**, the **multi-relay atomic claim**,
FIFO-per-target ordering, and the LIVE-query relay reactor are deferred (S6 shipped the transactional
enqueue + at-least-once retry + receiver dedup with an in-test target). **Token-on-the-bus** for
routed agent invocations (S5 is in-process co-trust), bus message classification, serve-side
authorization for hub-authoritative routed calls, packaging the workflow/bridge as installed wasm
artifacts, and explicit edge→hub router endpoints (S7) remain. Tracked in `../STATUS.md` and
`../STAGES.md`.
