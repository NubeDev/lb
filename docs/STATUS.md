# STATUS — where the project is right now

The single **"where are we"** dashboard. One screen, always current. Read this first at the
start of any session; update it at the end of any session that changed state.

> This is a **living snapshot**, not a log. It is overwritten in place — it always describes
> *now*, never history. The history lives elsewhere, on purpose:
> - **per-feature narrative** → `sessions/<topic>/…` (the messy middle of each session)
> - **bug history** → `debugging/README.md` (append-only symptom → fix memory)
> - **what shipped** → `public/` (the trimmed source of truth)
>
> So there is **no `LOG.md`** — those three already are the log, each at the right altitude.
> STATUS.md just points at them and says "this is the front line."

---

## Current stage

**S8 — data plane (durable store + generic ingest + tagging): exit gate MET** (2026-06-27). All three
slices shipped on the pinned **SurrealKV** persistent engine — (0) `Store::open` + the capability-spike
matrix + crash-consistency set, (1) `lb-ingest` durable exactly-once buffer (proven across a
kill-mid-commit), (2) `lb-tags` typed graph + spike-gated full-text/vector/counts, with `series.find`
discovery wired on tags. Data survives a node restart; a fleet writes one series without collision;
isolation/deny/offline tests pass on disk. The S9 collaboration-UI work proceeds in parallel. (Earlier
context below.)

**In S7 — platform maturity** (see `STAGES.md`); **both** S7 exit-gate slices have shipped — the
**signed extension registry** and the **native Tier-2 supervisor** — so the **S7 exit gate is fully
MET**. The Rust workspace + a React/Tauri UI exist and build; messaging, a second node + sync,
cross-node routed tool calls, the browser SSE/HTTP path, shared workspace assets, the **AI core**
(central agent + AI-gateway sidecar + durable jobs), the **coding workflow** (issue → triage →
approval-gated job → progress → transactional outbox), the **signed registry** (pull · verify · cache ·
install · offline · rollback) — now over a **real HTTP transport** (`lb-role-registry-host` server +
`HttpSource` client, replacing the in-memory stub) — and the **native Tier-2 supervisor** (a supervised
OS-process sidecar that restarts cleanly with no durable state lost) are all proven end to end.
The S6 **github-bridge** is now packaged as an installed Tier-1 wasm artifact (the deferral resolved; the
orchestrator stays a host service by design), with a **live HTTP ingress** (**`lb-role-github-webhook`**,
HMAC-verify the `X-Hub-Signature-256` → `ingest_via_bridge`) and a **live HTTP egress** (the outbox's
**`lb-role-github-target`**, delivering `create_pr`/`comment` over GitHub REST) — the relay now hardened
with **backoff + dead-letter**. The ingress and egress now **connect end to end into a live PR**: the
producer emits the structured `{repo,head,base,title,body}` payload the GitHub target maps, and a
durable-scan **resolution reactor** (`react_to_approvals`) auto-starts the coding job the moment its
approval lands `Approved` — closing webhook → triage → approval → JOB → outbox → GitHub with no manual
step. The ingress is now **multi-tenant** (`tenant_router`: `POST /webhook/{tenant}` over a
`TenantRegistry`), one process fronting many workspaces, each authenticated by its own secret with the
workspace wall held at the front door. And the whole loop now **runs as a service**: `lb-role-github-workflow`
ticks the reactor + the outbox relay per workspace (`run_workflow_loop`), mounted into the `node` binary
by config (`node/src/github.rs`) alongside the webhook front door — so a real webhook delivery flows
issue → triage → approval → JOB → PR end to end in a running process — and the set of serviced
workspaces is a **durable directory** (`register_workspace`/`deregister_workspace`) the driver re-reads
each tick, so a workspace is onboarded/retired **without a restart**. No doc-site build and no native
desktop window (webkit toolchain) yet.

**S0 exit gate — MET.** `cargo build --workspace` green; CI runs (FILE-LAYOUT size check +
build wasm guest + test + fmt); the four forever decisions (SDK/WIT, capability grammar +
token, job-queue, extension manifest) are written as scope docs.

**S1 exit gate — MET.** A tool call routed through MCP succeeds *with* the grant and is
refused *without* it; a second workspace cannot see the first's data. Through the real WASM
component. See `sessions/core/s0-s1-spine-session.md`.

**S2 exit gate — MET.** Post a message in the UI and it appears (Vitest `ChannelView`); history
survives independent of the bus / a restart (the store keeps it); an extension version swaps live
(hello v1→v2) with state intact. Mandatory capability-deny, workspace-isolation (bus + store +
inbox), and hot-reload categories. See `sessions/bus/messaging-session.md`.

**S3 exit gate — MET.** A second node joins (config-only `Node::boot_as(role)`); a cross-node tool
call routes over a Zenoh queryable and is capability-checked on the calling node, workspace-first;
channel data syncs edge↔hub with **idempotent offline apply** (§6.8); the browser reaches a node
over **SSE/HTTP** (replacing the S2 in-memory fake) and sees live messages appear. **61 Rust + 8
Vitest + 2 shell tests** pass — incl. capability-deny, workspace-isolation, and the first
offline/sync categories, all now **across two nodes** and the gateway. See
`sessions/sync/multi-node-sync-session.md` and `public/SCOPE.md`.

**S4 exit gate — MET.** A doc private to a user is shared to a team and read by a member while a
**non-member is denied** (gate 3, the membership layer below the workspace wall); the doc linked
into a channel is read by a channel `sub`-grantee; a **skill loads only when the workspace granted
it**; extension install records persist `requested ∩ admin_approved` per workspace. Capability-deny
(non-member / no-grant) and workspace-isolation hold across **store + MCP**. New `lb-assets` crate +
host `assets` service + `assets.*` MCP bridge + UI `DocView`. **83 Rust + 11 Vitest + 2 shell
tests** pass. Content is stored as a record (not `DEFINE BUCKET` — unavailable in our `kv-mem`
build; an S7 config swap). See `sessions/files/shared-assets-session.md` and `public/SCOPE.md`.

**S5 exit gate — MET.** An edge user invokes the central agent over the routed MCP namespace; the
agent calls the gateway for a model turn and a **granted MCP tool** inside its loop (under
`agent ∩ caller` — no widening); a workflow **job survives the edge disconnecting and resumes
idempotently**. New: `lb-jobs` (durable resumable session), `lb-role-ai-gateway` (swappable model
access + replay-safe idempotency cache, mock provider), host `agent` service (the loop + the gates)
+ routed wiring, grant **delegation** (`Principal::derive` + caps gate 2b), `agent.*` MCP bridge, UI
`AgentView`. **105 Rust + 14 Vitest + 2 shell tests** pass — incl. capability-deny (invoke gate +
the in-loop intersection), workspace-isolation across **store + MCP**, and offline/sync (interrupted
session resumes; duplicate invocation does not re-spend). See `sessions/agent/ai-core-session.md`
and `public/SCOPE.md`.

**S6 exit gate — MET.** A GitHub issue → inbox `needs:triage` → the S5 agent triages + drafts a
**shared scope doc** → a `needs:approval` inbox item **genuinely gates** a durable coding job (no job
record before approval; refused with `AwaitingApproval`; a rejected approval starts nothing) →
progress streams to a channel (motion) → every external effect goes through the **transactional
outbox** with at-least-once retry + receiver dedup (never lost, never double-sent). New: `lb-outbox`
(the transactional `Effect` + `enqueue`/`pending`/`mark_*`/relay), `lb_store::write_tx` (the one-tx
seam), `lb_inbox::Resolution` (the approval facet), the host `workflow` service (the orchestrator +
the gate), `workflow.*` MCP bridge, UI `WorkflowView`. **124 Rust + 18 Vitest + 2 shell tests** pass
— incl. capability-deny (each workflow verb), workspace-isolation across **store + MCP**, and
offline/sync (the outbox delivers at-least-once, idempotently). See
`sessions/coding-workflow/coding-workflow-session.md` and `public/SCOPE.md`.

**S7 exit gate — FULLY MET.** *Registry half:* an extension installs from the **signed** registry →
pull · **verify** (Ed25519 over a digest binding manifest+wasm) · cache · install through the existing
S4 flow; runs **offline** once cached; **rolls back** with **no durable state lost**; a tampered/
unsigned/foreign-key artifact is **rejected before caching, even with the grant**. *Native half:* a
**native Tier-2 sidecar is supervised and restarts cleanly** — a killed child (a **real OS process**)
is respawned, resumes answering, and **no durable workspace state is lost** (a channel message posted
before the crash is intact after); install/lifecycle are capability-gated (no spawn without
`mcp:native.install:call`); ws-B can never see or control ws-A's sidecar (store + MCP + the runtime
map); a signed `tier="native"` artifact installs through the registry and a tampered one is rejected.
New (registry): `lb-registry` + the host `registry` service + `registry.*` MCP bridge + UI
`RegistryView`. New (native): `lb-supervisor` (spawn/frame/health/restart behind a `Launcher` seam) +
the `echo-sidecar` reference binary + the `[native]` manifest block + the host `native` service
(supervision stateless: live PID in a runtime `SidecarMap`, durable truth in `Install` + `native_status`
records) + `native.*` MCP bridge + UI `NativeView`. **~163 Rust + 26 Vitest + 2 shell tests** pass —
incl. capability-deny, workspace-isolation across **store + MCP**, offline, rollback/hot-reload,
signing/verification, and the **supervision/restart** category (real process, no durable state lost).
Posture: process-group isolation + scoped identity + bounded restart; OS hardening + a boot reconciler
are noted follow-ups. See `sessions/registry/registry-session.md`,
`sessions/extensions/native-tier-session.md`, and `public/SCOPE.md`.

---

## Slices in flight

One row per vertical slice being built. State: `scoped` → `building` → `tested` → `shipped`.

| Slice | Topic | Stage | State | Scope | Session | Notes |
|---|---|---|---|---|---|---|
| Spine | core | S1 | **shipped** | [core](scope/core/core-scope.md) | [s0-s1-spine](sessions/core/s0-s1-spine-session.md) | host+store+bus+caps+mcp+runtime+1 WASM ext |
| Messaging | bus | S2 | **shipped** | [bus](scope/bus/bus-scope.md) | [messaging](sessions/bus/messaging-session.md) | pub/sub + presence + inbox + channel svc + React/Tauri UI + hot-reload |
| Sync / SSE | sync | S3 | **shipped** | [sync](scope/sync/sync-scope.md) | [multi-node-sync](sessions/sync/multi-node-sync-session.md) | 2nd node (role=config) + queryable routed MCP + edge↔hub sync + axum SSE/HTTP gateway + UI transport swap; 61+8+2 green |
| Shared assets | files | S4 | **shipped** | [files](scope/files/files-scope.md) + [skills](scope/skills/skills-scope.md) | [shared-assets](sessions/files/shared-assets-session.md) | `lb-assets` crate + host 3-gate (ws→cap→membership) doc/skill svc + grant-gated skills + persisted install records + `assets.*` MCP bridge + UI DocView; 83+11+2 green |
| AI core | agent | S5 | **shipped** | [agent](scope/agent/agent-scope.md) + [ai-gateway](scope/ai-gateway/ai-gateway-scope.md) + [jobs](scope/jobs/jobs-scope.md) | [ai-core](sessions/agent/ai-core-session.md) | central agent (owns the loop) + routed edge→hub invoke + grant delegation (`agent ∩ caller`) + `lb-jobs` resumable session + `lb-role-ai-gateway` (mock + idempotency cache) + `agent.*` MCP bridge + UI AgentView; 105+14+2 green |
| Coding workflow | coding-workflow | S6 | **shipped** | [coding-workflow](scope/coding-workflow/coding-workflow-scope.md) + [outbox](scope/inbox-outbox/outbox-scope.md) | [coding-workflow](sessions/coding-workflow/coding-workflow-session.md) | `lb-outbox` (transactional `Effect` + at-least-once relay) + `lb_store::write_tx` + `lb_inbox::Resolution` + host `workflow` service (ingest→triage→approval-GATE→job→outbox) + `workflow.*` MCP bridge + UI WorkflowView; 124+18+2 green |
| Signed registry | registry | S7 | **shipped** | [registry](scope/registry/registry-scope.md) | [registry](sessions/registry/registry-session.md) | `lb-registry` (digest binds manifest+wasm + `verify_artifact` + `VerifiedArtifact` newtype → verify-before-cache) + host `registry` service (pull·verify·cache·catalog·install behind a `Source` seam; rollback = install prior ver) + `registry.*` MCP bridge + UI RegistryView; 145+22+2 green |
| Native Tier-2 | extensions | S7 | **shipped** | [native-tier](scope/extensions/native-tier-scope.md) | [native-tier](sessions/extensions/native-tier-session.md) | `lb-supervisor` (spawn·frame·health·shutdown·restart behind a `Launcher` seam) + `echo-sidecar` reference binary + `[native]` manifest block + host `native` service (stateless: runtime `SidecarMap` + durable `Install`/`native_status`; `mcp:native.<verb>:call` gate; crash-restart-on-fault) + `install_native_from_registry` + `native.*` MCP bridge + UI NativeView; ~163+26+2 green (real-process restart proof) |
| Registry HTTP transport | registry | S7 | **shipped** | [registry](scope/registry/registry-scope.md) | [http-source](sessions/registry/http-source-session.md) | `lb-role-registry-host` = real HTTP **server** (`router`/`serve`, dumb origin serving signed `Artifact`s at `GET /artifacts/{ext}/{ver}`) + **`HttpSource`** client filling the host `Source` seam's last mock; verify-before-cache holds over the wire (tamper-in-transit rejected); `reqwest`/`axum` in the role crate, never in core. +5 Rust (round-trip · offline-from-cache · tamper · isolation · deny over a real socket); ~168+26+2 green |
| github-bridge as wasm | extensions | S7 | **shipped** | [github-bridge](scope/extensions/github-bridge-scope.md) | [github-bridge](sessions/extensions/github-bridge-session.md) | the S6 deferral resolved — the workflow's inbound edge ships as an installed **Tier-1 wasm** artifact (2nd real ext after `hello`). **Pure-transform** guest (`github-bridge.normalize`: webhook → `{issue_id,payload,ts}`, no host callback — WIT unchanged) + host **`ingest_via_bridge`** composing normalize→`ingest_issue` (2 gates). Orchestrator stays a host service. +7 Rust (install-deny · isolation · offline · rollback · transform branches, all through real wasm); finding: node-global stateless instance, wall is caps+store ([debug](debugging/extensions/loaded-extension-instance-is-node-global.md)); ~175+26+2 green |
| github-webhook ingress | extensions | S7 | **shipped** | [github-webhook](scope/extensions/github-webhook-scope.md) | [github-webhook](sessions/extensions/github-webhook-session.md) | the github-bridge follow-up resolved — the **live HTTP ingress**. `lb-role-github-webhook` (beside `lb-role-registry-host`): `POST /webhook` → **constant-time HMAC-SHA256** verify of `X-Hub-Signature-256` over the **raw body** (mediated secret, never logged) → `ingest_via_bridge`. Two-layer boundary: authenticity (`401` forgery) *before* authority (`403` ungranted). +12 Rust (bad-sig · deny · isolation · idempotent re-delivery · happy/real-socket · malformed→`422` · HMAC units, through real wasm). `axum`/`hmac` in the role crate, no core/WIT/cap-grammar change; ~187+26+2 green |
| outbox egress + hardening | coding-workflow | S7 | **shipped** | [outbox](scope/inbox-outbox/outbox-scope.md) | [outbox-egress](sessions/coding-workflow/outbox-egress-session.md) | the outbox's **live HTTP egress** + relay hardening (2 scope follow-ups). `lb-role-github-target` delivers `create_pr`/`comment` over GitHub REST (`reqwest` in the role crate; `422 already-exists` = idempotent success, no double-PR; token mediated). **Backoff + dead-letter** in `lb-outbox`+relay: `Effect` gains `max_attempts`/`next_attempt_ts`, new `DeadLettered` status, relay scans `due` (backoff-gated) + tallies dead-letters. +11 Rust (2 outbox backoff/dead-letter + 9 github-target: mapping units + happy·422·dead-letter·transport over real socket); 8 workflow regression updated; no core/WIT/cap change; ~198+26+2 green |
| close the loop | coding-workflow | S7 | **shipped** | [outbox](scope/inbox-outbox/outbox-scope.md) + [coding-workflow](scope/coding-workflow/coding-workflow-scope.md) | [close-the-loop](sessions/coding-workflow/close-the-loop-session.md) | ingress+egress **connected end to end into a live PR** (2 scope follow-ups). **Producer enrichment**: `PrSpec{repo,head,base,title,body}` record keyed by approval → `start_coding_job` emits the structured `create_pr` payload github-target maps (was `{scope_doc}`; also fixes a `format!` escaping bug). **Resolution reactor**: `react_to_approvals` = durable scan over `lb_inbox::approved` → auto-`start_coding_job` on `Approved` (relay's altitude; LIVE-query version a follow-up); idempotent on a deterministic job id (re-resolve/re-scan → ONE job/PR). +8 Rust (5 reactor: deny·isolation·idempotency·skip + 1 **full-loop over a real socket** (reactor→real GithubTarget opens PR) + 2 pr_spec units); no core/WIT/cap change; ~206+26+2 green |
| webhook front door | extensions | S7 | **shipped** | [github-webhook](scope/extensions/github-webhook-scope.md) | [github-webhook-multitenant](sessions/extensions/github-webhook-multitenant-session.md) | the webhook ingress went **multi-tenant**: `tenant_router` (`POST /webhook/{tenant}`) over a `TenantRegistry` (opaque slug → `{ws, principal, secret}`) fronts many workspaces from one process, each with its own secret. Routing by URL slug (chosen BEFORE the HMAC check → authenticity-before-parse holds); the **workspace wall holds at the front door** — A's secret on B's slug → `401`, never crosses; unknown tenant = opaque `401` (no enumeration oracle). Single-tenant `/webhook` untouched. +4 Rust (per-tenant routing · cross-tenant-secret isolation · unknown-tenant 401 · capability-deny); no core/WIT/cap change; ~210+26+2 green |
| workflow driver + node wiring | coding-workflow | S7 | **shipped** | [workflow-driver](scope/coding-workflow/workflow-driver-scope.md) | [workflow-driver](sessions/coding-workflow/workflow-driver-session.md) | the loop now **runs in a process**, not just tests. New `lb-role-github-workflow`: `drive_once`/`run_workflow_loop` tick the **reactor then the relay** per workspace (reactor-first → same-tick PR), over a list of `WorkflowBinding`s (isolation structural), with an **injected clock** (no wall-clock in the crate). Env-gated **node wiring** (`node/src/github.rs`): mounts the webhook front door + the driver loop by config (`LB_WORKFLOW_WS`/`LB_WEBHOOK_*`/`LB_GITHUB_*`), no `if cloud`; `node` is now `Arc<Node>`. The host owns the verbs, the role owns the cadence; GitHub `Target` behind the trait, no net dep in core. +4 Rust (one-tick close · idempotent re-tick · per-ws isolation · injected-clock); no core/WIT/cap change; ~214+26+2 green |
| dynamic workspace directory | coding-workflow | S7 | **shipped** | [workflow-driver](scope/coding-workflow/workflow-driver-scope.md) | [dynamic-directory](sessions/coding-workflow/dynamic-directory-session.md) | onboard/retire a workspace **without a restart**. New host **directory** (`register_workspace`/`deregister_workspace`/`enabled_workspaces`, `WorkspaceEntry{ws,channel,status,ts}`) in a **reserved namespace** `_lb_workflow_directory` (node-level config, secret-free, no MCP surface). Driver `run_directory_loop`/`drive_directory_once` re-reads the directory **each tick** (binding via an injected `principal_for`), so a runtime `register` is picked up next tick; `node` seeds the directory from `LB_WORKFLOW_WS` then drives it. +8 Rust (5 host: register/deregister/idempotent/durable/ns-isolation + 3 driver: register-mid-loop · deregister-drops · multi-ws isolation); no core/WIT/cap change; ~222+26+2 green |
| persistent store + spike | store | S8 | **shipped** | [persistent-backend](scope/store/persistent-backend-scope.md) | [persistent-backend](sessions/store/persistent-backend-session.md) | slice 0 / the gate. `Store::open(path)` on the pinned **SurrealKV** engine (both engines compiled in; constructor by `LB_STORE_PATH`, no code-branch) + raw `query_ws` seam. Permanent hermetic **capability-spike matrix** (5 LOAD-BEARING ✓ → GO; DEGRADABLE: bucket ✗→record-as-content, SEARCH ✓, HNSW ✓, materialized-view defines-but-doesn't-populate, LIVE ✓). **Crash set** (subprocess SIGABRT): write→reopen, kill-mid-tx→rollback, flush-burst→last-commit-survives. Isolation/parity re-run on disk. 6 spike + 4 crash + 4 parity green |
| generic ingest | ingest | S8 | **shipped** | [ingest](scope/ingest/ingest-scope.md) | [ingest](sessions/ingest/ingest-session.md) | slice 1. New `lb-ingest`: `Sample{series,producer,ts,seq,payload,labels,qos}`; durable staging **append** (cheap path) → commit worker **one-tx-per-batch** UPSERT on `[series,producer,seq]` + delete-staged same-tx (atomic + exactly-once on re-drain); `series.read/latest`; overflow drop-oldest/dead-letter. Host `ingest` svc (MCP gate, producer=authenticated principal, drain worker = ingest role) + `ingest.write`/`series.read`/`series.latest`/`series.find` bridge. **Anti-IoT held** (no device/sensor/MQTT in core). Tests: deny · ws-iso (store+MCP) · **kill-mid-commit re-drain** · two-producer collision · overflow both QoS; 5 crate + 3 durable + 6 host green |
| typed tag graph | tags | S8 | **shipped** | [tags](scope/tags/tags-scope.md) | [tags](sessions/tags/tags-session.md) | slice 2. `lb-tags` built from stub: `tag:[key,value]` typed nodes + `(entity,tag,source)` provenance edges; `add`/`remove`/`of`/`find` (exact/key-only/faceted intersection) + required per-workspace **tag-node cap** (deny). Spike-gated add-ons: **BM25 full-text** ✓, **HNSW vector** ✓ (dimension pinned, mismatch rejected), **per-dimension counts** (per-query — view doesn't populate). Host `tags` svc + `tags.*` bridge (no event verb); `series.find` wired on top. Tests: deny-per-verb · **identical-tag two-ws isolation** · idempotent re-tag · index-correctness; 5+1+4 crate + 3+1 host green |
| make collaboration real | frontend | S7 | **shipped** | [collaboration](scope/frontend/collaboration-scope.md) | [collaboration](sessions/frontend/collaboration-session.md) | the UI went from a 1-screen S2 demo on fakes to a **real collaboration app over a real session**. **Identity keystone:** demo principal DELETED — `POST /login` mints a signed `lb_auth` token (dev credential store); **every** gateway route `verify`s the bearer token → workspace+caps from the **token, not the request** (§7); SSE auth via `?token=` (EventSource can't set a header). New host services: `channel_registry` (`channel_create`/`channel_list` + create-on-post, reuses chan pub/sub gate), `members` (`list_members`/`add_team_member` over S4 edges, `mcp:members.*`), `inbox` (`list_inbox`/`resolve_inbox` over `lb_inbox`, `mcp:inbox.*`), `outbox` (read-only `outbox_status`, `mcp:outbox.status`), `workspaces` (`workspace_list`/`create` in reserved ns). New gateway routes mirror each 1:1. UI: `lib/session/`+`useSession`, workspace switcher, channel list, members view, **rendered presence** (`usePresence` idempotent roster), real inbox view (replaces the workflow fake on the real path — Approve/Reject = S6 gate), read-only outbox view; `App.tsx` hardcoded `WS`/`CHANNEL`/`AUTHOR` gone. **Two real sessions** make the ws-isolation test real (ws-B sees none of ws-A). +5 host collab (cap-deny + ws-iso each verb) +14 gateway (session: issue/verify/forged/expired/ws-from-token · deny · 2-session iso · registry · real inbox · outbox pending→delivered · live SSE) +6 Vitest views; cargo build/fmt/file-size + pnpm build/test green; no core/WIT change |

---

## Scopes authored (ready to build)

The `scope/<topic>/` docs exist for all areas (see `scope/README.md`). **Fully authored:** core,
auth-caps, mcp, crate-layout, extensions (+ **native-tier**, new this slice), jobs, bus, inbox-outbox
(+ outbox), tenancy, frontend, sync, testing, debugging, ai-gateway, agent, coding-workflow (+ the
**workflow-driver**, new this slice), registry,
**node-roles** + **platform-targets** (filled this slice — placement × role + the native target tag).
**Promoted to `public/`:** core, auth-caps, mcp, crate-layout, bus, inbox-outbox (+ outbox + the
resolution facet), tenancy, **store** (persistent backend + spike matrix, this slice), **ingest** (this
slice), **tags** (this slice), frontend, sync, files, skills, agent, coding-workflow, registry,
**extensions** (the runtime + two tiers) (+ `public/SCOPE.md`).

---

## Next up

1. **S7 platform maturity** (`STAGES.md`): the **extension registry** AND the **native Tier-2
   supervisor** are **shipped** — the **S7 exit gate is fully MET** (~~install from a signed registry,
   run offline once cached, roll back~~; ~~a native sidecar is supervised and restarts cleanly~~).
   Remaining S7 work: ~~**packaging the S6 workflow/github-bridge as installed wasm artifacts**~~
   **(github-bridge SHIPPED** — a pure-transform Tier-1 wasm artifact; the orchestrator deliberately
   stays a host service since it drives host-internal seams a guest can't reach). ~~Open follow-up here: a
   **webhook-receiver role crate** that drives `ingest_via_bridge` on a real HTTP POST~~ **(SHIPPED —
   `lb-role-github-webhook`: HMAC-verify → `ingest_via_bridge`)**; remaining webhook opens — ~~a
   **multi-tenant front door**~~ **(SHIPPED — `tenant_router` + `TenantRegistry`, route-by-slug,
   per-tenant secret)**, a **dynamic** tenant directory (onboard without a restart) and an
   `lb-secrets`-backed secret (~~a **resolution reactor** that auto-starts the job on approval~~ **SHIPPED** — `react_to_approvals`).
   And the `host.call_tool`
   WIT question if a guest ever needs to call a host tool (a forever-ABI change, its own scope). **Native-tier follow-ups:** a boot reconciler (re-spawn `lifecycle=started`
   from records), OS-level hardening (cgroups/seccomp/userns), a background health-poll reactor (the
   slice restarts on-demand at the call boundary), the child→host MCP callback transport, and native
   platform-target enforcement.
1b. **Registry follow-ups** (in `scope/registry/`): ~~a real HTTP `Source`/`registry-host` server~~
   **(shipped — `lb-role-registry-host`)**; remaining — a **durable backing** for the registry-host
   catalog + a **publish** endpoint (an outbox `Target` write) + TLS/read-auth on the server; a durable
   publisher-key allow-list + admin trust-management flow; key rotation/revocation (needs the hub
   identity directory); cache eviction/GC; the public catalog read-only union (per-workspace entries
   ship now); `registry.update` semantics; gateway/Tauri wiring for `registry_*`.
2. **Outbox `Target` adapters + relay hardening** — ~~GitHub HTTP~~ **(SHIPPED — `lb-role-github-target`)**
   and ~~backoff + dead-letter~~ **(SHIPPED — `max_attempts`/`next_attempt_ts`/`DeadLettered` + `due`
   scan)**, the **producer payload enrichment** a live PR needs **(SHIPPED — `PrSpec` + structured
   `create_pr`)**, and a **resolution reactor** that auto-starts the job on approval **(SHIPPED —
   `react_to_approvals`, a durable scan)** are all done. Remaining: **email / sync-publish** adapters
   behind the `Target` trait + **search-before-create** dedup; the **multi-relay atomic claim**,
   FIFO-per-target ordering, and the **LIVE-query** driver (the **poll-tick driver SHIPPED** —
   `lb-role-github-workflow`'s `run_workflow_loop` ticks reactor+relay per ws, mounted in the `node`
   binary by config; the LIVE push is the latency optimization on top). The **dynamic workspace
   directory** (hot-add without a restart) **SHIPPED** (`register_workspace`/`deregister_workspace`,
   reserved-namespace record, re-read each tick); remaining: the **webhook tenant directory** (paired
   with `lb-secrets` for per-tenant secrets), an admin/MCP surface for the register verbs, and GC.
3. **Real model provider + streaming** behind the S5 gateway contract — the mock is the only stub;
   add an OpenAI-compatible / local adapter and stream tokens as Zenoh motion. Agent/job progress can
   now also ride the durable outbox for the must-deliver transcript.
4. **Gateway/Tauri wiring for `agent_invoke` + `assets_*` + `workflow_*`** (S4/S5/S6 follow-up): the
   host verbs + MCP bridges + UI fakes exist; route them through the SSE/HTTP gateway + Tauri shell to
   a real node (mirrors the S3 channel transport swap).
5. **Fit-and-finish carryover:** ~~render presence in the UI~~ **(SHIPPED — `usePresence` roster)**;
   ~~a real login→token→principal session (replacing the gateway's demo principal)~~ **(SHIPPED — the
   "make collaboration real" slice: `POST /login` mint + per-route `verify`, demo principal deleted)**;
   **token-on-the-bus** so the hub can verify a routed caller's grant (S5/S6 are in-process co-trust —
   still open); sync the asset/job/outbox tables (all `(table,id)` upserts the channel sync path
   already covers); explicit edge→hub router endpoints (S7); the **Tauri desktop** command layer's
   session (the collaboration slice wired the browser/gateway path; the desktop shell still fixes its
   workspace).

---

## How to keep this current

Every session that changes state updates the relevant cell here as its **last step**
(`HOW-TO-CODE.md` §3 step 9). Keep it to one screen — if a section grows past a few rows,
the detail belongs in the per-feature docs, not here.
