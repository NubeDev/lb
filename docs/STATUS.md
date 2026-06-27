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
| self-contained extension over real Module Federation (`fleet-monitor`) | extensions | S10 | **shipped** | [ui-federation](scope/extensions/ui-federation-scope.md) + [dashboard-widgets](scope/frontend/dashboard-widgets-scope.md) | [fleet-monitor-federation](sessions/extensions/fleet-monitor-federation-session.md) | an extension is now **one folder = backend + frontend** (each optional). New `rust/extensions/fleet-monitor/`: a **native Tier-2 sidecar** (own PID, supervised over stdio, `fleet.summary` MCP tool) **+** its co-located `ui/` built as a **real Vite Module Federation remote** (`@originjs/vite-plugin-federation`, shared React singletons — not a hand-rolled `import()`), real **shadcn/ui + Tailwind**, mounting a cap-gated sidebar **page with 3 nested routes** + declaring **2 dashboard widgets**. Data only via `mount(el,ctx,bridge)` → `POST /mcp/call` (frozen `series.*` reads; never token/DB). Shell is the federation **host** (`ui/vite.config.ts` shares react/react-dom; `ext-host/federation.ts` loads remotes by gateway URL; `ExtHost` rewritten raw-import→federated). **Contract refactor:** `[widget]`→**`[[widget]]`** (`widgets: Vec` end to end). **Load-bearing fix:** the **native** install path now persists `[ui]`/`[[widget]]` (shared `host/src/ui_decl.rs`) — it silently didn't before. **Fake removed** from page discovery. `ui/extensions/hello-ui` + `rust/extensions/hello-ui` **hard-deleted**. **Pre-existing CI red fixed** (`cargo build --workspace`: `test_gateway_seed` stray-bin → `autobins=false`). Tests: 3 backend + 12 manifest + 2 ui_decl + 3 ext_ui + **2 native e2e** (real `OsLauncher` child + page/2-widgets in `ext.list`) + **6 ext-UI Vitest** + 20 shell + **50 real-gateway** (incl. `fleet-monitor` page slot + both widget tiles from a real `Install`); `cargo build/test --workspace` green (1 pre-existing sync flake, untouched). Widget *rendering* + iframe tier = follow-ups |
| extension UI pages (ui-federation slice 1) | frontend | S9+ | **shipped** | [ui-federation](scope/extensions/ui-federation-scope.md) + [dashboard-widgets](scope/frontend/dashboard-widgets-scope.md) | [extension-pages](sessions/frontend/extension-pages-session.md) | an extension now contributes a **full sidebar page** (and/or a dashboard **widget**) end to end. Frozen manifest `[ui]`/`[widget]` blocks → carried on `Install` (scope-narrowed to the grant) → surfaced via `ext.list` `ExtRow.ui`/`.widget` → shell `features/ext-host/` renders it (trusted **in-process dynamic-import `mount(el,ctx,bridge)`**; iframe tier = follow-up). Data via the **host-mediated bridge** (`POST /mcp/call` → `lb_host::call_tool`, cap+ws re-checked; page never holds the token/DB). Gateway serves bundles (`GET /extensions/{ext}/ui/{*path}`, traversal-guarded). Reference ext `hello-ui` (Vite React, served in both dev + gateway paths). Tests: 6 manifest + 3 host (persist·**scope-narrow**·**bridge-deny**) + 3 gateway (serve·traversal·deny) + 3 Vitest (slot·bridge-filter); 63 Vitest + workspace build + fmt green. **Widgets-on-dashboard (slice 2) deferred** (needs dashboard core) |
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
| authz grants/roles/teams | auth-caps | S9+ | **shipped** | [authz-grants](scope/auth-caps/authz-grants-scope.md) | [authz-grants](sessions/auth-caps/authz-grants-session.md) | **slice 1** of the admin-CRUD/lifecycle/console build. New **`lb-authz`** crate (raw, ws-namespaced, no auth — mirrors `lb-assets`): `grant(subject→cap)` store (`assign`/`revoke`/`list`, revoke = idempotent **tombstone-upsert** §6.8), `role(name→caps[])` bundles (role-assign = a grant of `role:<name>`, no nesting), first-class `team(team,name)` records (member edges stay `lb_assets`). `resolve_caps(ws,user)` = `union(direct, roles, team-inherited)` deduped/sorted (the **Gate-2 cached half** of the freshness asymmetry, documented). Two **seams** for slice 2: `resolve_caps` (login projection) + `revoke_subject` (revocation-on-delete, returns count). Host **`authz`** service = the cap chokepoint: `grants.*`/`roles.*`/`teams.*` gated (`mcp:grants.assign`/`grants.list`/`roles.define`/`roles.list`/`teams.manage`/`teams.list`) + `holds_cap` **no-widening** guard (assign/define only caps you hold) + `call_authz_tool` MCP bridge. +5 host (deny-per-verb · 2-ws isolation store+MCP · grant resolution · no-widening · idempotent+revoke-seam) +2 crate units; cargo build/fmt/file-size green; no SDK/WIT/cap-grammar change |
| admin-crud destructive + user lifecycle | auth-caps | S9+ | **shipped** | [admin-crud](scope/auth-caps/admin-crud-scope.md) | [admin-crud](sessions/auth-caps/admin-crud-session.md) | **slice 2** — the destructive half + a real dev-store **user CRUD**. New host **`users`** svc: `UserRecord{user,active,role,cred_ref}` per `(ws,user)` + credential-free `UserView` (`cred_ref` never serialized); `user.create`/`list`/`disable`/`enable`/`delete` (gated `mcp:user.manage`/`user.disable`); **`user_login_check`** = the un-gated pre-mint seam wired into `POST /login` so **disable bites minting** (absent record still auto-seeds). Workspace lifecycle: `rename`(+un-archive)/`delete`(soft archive, hidden from list)/**`purge`** (hard: distinct `mcp:workspace.purge` cap **AND** typed confirm token; directory tombstone, no resurrection). `teams.delete` (cascade: drop member edges + `revoke_subject` + tombstone, returns count) + `teams.rename`; `members.remove`. `user.delete`/`teams.delete` call slice-1's **`revoke_subject`** seam (one revoke path). Gateway: `/admin/*` routes + `DELETE /teams/{team}/members/{user}`, each **re-checks the cap server-side** (UI gate is convenience); dev claim set now admin. `http.ts` gains every verb (+ `delJson`) + `admin.fake.ts` 1:1. +7 host (deny-per-verb · 2-ws iso · soft-before-hard+confirm · disable-bites-login · delete-revokes-grants · teams-cascade · tombstone-not-resurrected) +3 gateway (**server-deny-on-forged-call** · admin round-trip · login-refuses-disabled) ; 40 Vitest + tsc green; cargo build/fmt/file-size green; no SDK/WIT change |
| admin console UI | frontend | S9+ | **shipped** | [admin-console](scope/frontend/admin-console-scope.md) | [admin-console](sessions/frontend/admin-console-session.md) | **slice 4 of 4** — the UI that drives slices 1–3's destructive/admin verbs. One shared **`ConfirmDestructive`** (props: consequence · reversible · escalation `none\|type-name\|second-gate`) every delete/disable/remove/uninstall routes through (blocks until confirmed; type-the-name for ws purge; second-gate for uninstall; cancel = no-op). Cap-gated **`features/admin/`** section — `WorkspacesAdmin` (archive/purge), `UsersAdmin` (create/disable/delete), `TeamsAdmin` (create/rename/delete w/ live member count), `MembersAdmin` (add/remove + freshness-asymmetry copy), `GrantsAdmin` (read + assign/revoke, **no role editor**) under a tabbed `AdminView` (per-control cap gate). Top-level **`features/extensions/`** console over `ext_*` (both tiers · live state · restart count · start/stop/uninstall) — **retires `RegistryView`/`NativeView`**, coverage ported. Caps surfaced to the UI: `LoginReply` gained `caps` + `Session.caps` + `lib/session/admin-caps.ts` (`isAdmin`/`hasCap`, mirrors `dev_claims`); fake returns admin caps. Nav cap-gated in `App.tsx`/`NavRail`. **The gateway is the only boundary** — UI gate is convenience, server deny on a forged call proven in Rust (`admin_routes_test`). **Plus (follow-on):** the dev claims gained the `mcp:ext.*` caps so the Extensions section actually shows (was hidden — cap was missing); a dev-only fake seed so the demo build isn't empty; and **signed-artifact upload shipped end to end** — UI `UploadArtifact` + `publishArtifact`/`http.ts` `ext_publish` over the existing gateway `POST /extensions` → `lb_host::ext_publish` (verify-before-store, **per-workspace install**, `mcp:ext.publish:call`). **59 Vitest passed** (was 40) — confirm flow · per-sub-view on the fake · cap-gated visibility · ext both-tier/lifecycle · **upload (verified/tampered/malformed)**; tsc clean; gateway build + `admin_routes_test` (4) green; no SDK/WIT change |
| extension lifecycle over the gateway | extensions | S9+ | **shipped** | [lifecycle-management](scope/extensions/lifecycle-management-scope.md) | [lifecycle-management](sessions/extensions/lifecycle-management-session.md) | **slice 3** — closes the lifecycle matrix + **exposes it over the gateway** (the biggest gap: host had the mechanisms but only Tauri reached them → browser `unknown command`). `lb-assets` **`Install`** gained `tier{wasm,native}` + durable **`enabled`** intent + `kind` (serde-defaulted); new `list_installs` (union both tiers) + `delete_install` (tombstone, `read_install` reads-absent). New host **`ext`** surface (dispatch by `Install.tier`, no `if tier`): `ext.list` (uniform `ExtRow{ext,version,tier,enabled,running,health,restart_count}`, joins native `SidecarMap`), `ext.enable`/`disable` (durable intent — **disable also stops the native child**, distinct from stop), `ext.uninstall` (stop+tombstone, idempotent, ws-first), + the boot **`reconcile`** verb returning a plan that **honors disable** (a disabled ext is NOT auto-started). Native refactor: idempotent `stop_sidecar_internal` for cascades, `stop_native` keeps `NotRunning` ([bug caught+fixed](sessions/extensions/lifecycle-management-session.md)). **Registry publish**: `ArtifactStore::publish` **verify_artifact-before-store** (tamper/unsigned/foreign rejected, nothing stored; idempotent) + `POST /artifacts`. Gateway `/extensions` routes (re-check caps); `http.ts` `ext_*` + `ext.fake.ts` 1:1. +4 host (deny · 2-ws iso · list-unions-tiers · **reconcile-honors-disable**) +4 registry-host (publish/tamper/foreign/unsigned) +1 gateway (ext reachable + non-admin deny); 40 Vitest + tsc green; native/assets suites green; no SDK/WIT change |
| gateway assets+workflow wiring | frontend | S9+ | **shipped** | [files](scope/files/files-scope.md) + [coding-workflow](scope/coding-workflow/coding-workflow-scope.md) | [gateway-assets-workflow](sessions/frontend/gateway-assets-workflow-session.md) | Next-up item 4 (partial): routed the host **`assets.*`** + **`workflow.*`** verbs over the SSE/HTTP gateway (were Tauri-only → `unknown command` in the browser). New `routes/assets.rs` (`GET\|POST /docs`, `GET /docs/{id}`, `/share`,`/link`; `POST /skills`, `GET /skills/{id}`, `/grant`) + `routes/workflow.rs` (`POST /approvals/{id}/request\|resolve\|start`; `start` reads the `PrSpec` back by approval id, the S6 gate surfaces as `started:false`); outbox view stays `GET /outbox`. Each re-checks the gate server-side, ws+principal from the **token not the body** (§7). `http.ts` + `workflow.api.ts` (added `requestApproval`, `PrSpec`) mirror 1:1. **`agent_invoke` deferred** — needs the real model provider (no mock). +7 Rust (deny-per-verb + ws-iso doc/skill/approval + approval-gate); ~221+26+2 Rust + 58 Vitest green (1 red is a separate in-flight roles refactor); no SDK/WIT/cap change |
| data console (DB browser + ingest) | frontend | S9+ | **shipped** | [data-console](scope/frontend/data-console-scope.md) | [data-console](sessions/ingest/data-console-session.md) | two non-SQL pages on the shipped S8 data plane. **Data** = admin, READ-ONLY raw-store lens: new generic `lb_store::{tables,scan,graph}` reads (id-cursor, hard-capped) + host **`dbview`** svc (`store.tables`/`scan`/`graph`, **admin-only** — relaxes gate 3, so granted to ws-admin not `member_caps`) + `/store/*` gateway routes; UI table-picker+counts / paged row-grid (expand→JSON) / **react-flow** graph (`@xyflow/react`, code-split). **Ingest** = the S8 `ingest.*`/`series.*` verbs over the gateway (new small `series.list(prefix)`; `POST /ingest` writes-then-drains so a manual sample is instantly visible) + UI series list/search / latest+recent / manual write. **Built the real-gateway Vitest harness** (`test_gateway` bin + `vitest.gateway.config.ts`, `pnpm test:gateway`) — first step of retiring the fakes (#00), NO new `*.fake.ts`. +7 Rust route (deny-per-verb · ws-iso, real node seeded via real write path) ; 60 Vitest (incl. NavGating member-hides-Data) + **7 real-gateway** (4 ingest · 3 data) + tsc + code-split build green; no SDK/WIT change |
| admin console redesign | frontend | S9+ | **shipped** | [admin-console](scope/frontend/admin-console-scope.md) | [admin-console-redesign](sessions/frontend/admin-console-redesign-session.md) | the AI-built admin UI "looked like a chat window" and hid relationships — rebuilt **relationship-first**. Four tabs (**People · Teams · Roles · Workspaces**); the old Users/Members/Grants tabs folded in (members live inline under a selected Team; grant/role assignment lives in each subject's detail). New master-detail: a selected user shows **the teams they belong to** (assembled from the real membership endpoints via `useDirectory`, never typed) + roles + advanced caps. **Real role editor** (the headline gap): added gateway **`POST /admin/roles`** (`define_role`→`roles_define`, no-widening server-side) — the UI builds a role by **checking caps from a list** (the admin's own session caps = the no-widening set), no `role:<name>` typing; `roles.list` now keeps each role's caps (was discarded). Chat-composer create replaced by a header action everywhere. Shared `AdminPanel`/`AccessEditor`/`useSubjectGrants`/`useRoles`. Deleted `UsersAdmin`/`MembersAdmin`/`GrantsAdmin` + hooks. +1 Rust gateway test (define/list/no-widening/deny → **5 admin-route tests**); UI **57 Vitest** + tsc + build green; no SDK/WIT/cap-grammar change |
| dashboard surface (grid + widgets) | frontend | S9+ | **scoped** | [dashboard](scope/frontend/dashboard-scope.md) + [dashboard-widgets](scope/frontend/dashboard-widgets-scope.md) | — | the `vision/0003` IoT dashboard made buildable. **Phase 1 (build-ready):** `seed_iot_demo` (real ingest path) + `dashboard.*` host verbs (`get`/`list`/`save`-UPSERT/`delete`/`share`, 5 caps) with the **full S4 asset-sharing authz** (private→team→workspace, three-gate non-member deny) + `routes/dashboard.rs` gateway mirror + a **series live SSE route** (`GET /series/{s}/stream`) + `react-grid-layout` `features/dashboard/` with built-in chart/stat/gauge widgets bound to real series (`{series}`\|`{find:{tags}}`), layout in a `dashboard:{id}` SurrealDB record (not localStorage). **Phase 2 (build-ready):** widgets as installed extensions — host-mediated **read-only bridge** (a widget never holds the token or touches the DB; ws+cap re-checked per call), trusted module-federation vs untrusted iframe, `[widget]` manifest, widget palette. **Both Phase-2 forever-contracts (`[widget]` manifest block + bridge protocol) are FROZEN** (versioned `v:1`) — no open question left. **Phase 3:** real edge fleet (node-connection/fleet-presence/grant-by-tag). Phases 1 & 2 build-ready; remaining opens are only named follow-ups |
| make collaboration real | frontend | S7 | **shipped** | [collaboration](scope/frontend/collaboration-scope.md) | [collaboration](sessions/frontend/collaboration-session.md) | the UI went from a 1-screen S2 demo on fakes to a **real collaboration app over a real session**. **Identity keystone:** demo principal DELETED — `POST /login` mints a signed `lb_auth` token (dev credential store); **every** gateway route `verify`s the bearer token → workspace+caps from the **token, not the request** (§7); SSE auth via `?token=` (EventSource can't set a header). New host services: `channel_registry` (`channel_create`/`channel_list` + create-on-post, reuses chan pub/sub gate), `members` (`list_members`/`add_team_member` over S4 edges, `mcp:members.*`), `inbox` (`list_inbox`/`resolve_inbox` over `lb_inbox`, `mcp:inbox.*`), `outbox` (read-only `outbox_status`, `mcp:outbox.status`), `workspaces` (`workspace_list`/`create` in reserved ns). New gateway routes mirror each 1:1. UI: `lib/session/`+`useSession`, workspace switcher, channel list, members view, **rendered presence** (`usePresence` idempotent roster), real inbox view (replaces the workflow fake on the real path — Approve/Reject = S6 gate), read-only outbox view; `App.tsx` hardcoded `WS`/`CHANNEL`/`AUTHOR` gone. **Two real sessions** make the ws-isolation test real (ws-B sees none of ws-A). +5 host collab (cap-deny + ws-iso each verb) +14 gateway (session: issue/verify/forged/expired/ws-from-token · deny · 2-session iso · registry · real inbox · outbox pending→delivered · live SSE) +6 Vitest views; cargo build/fmt/file-size + pnpm build/test green; no core/WIT change |

---

## Scopes authored (ready to build)

The `scope/<topic>/` docs exist for all areas (see `scope/README.md`). **Fully authored:** core,
auth-caps, mcp, crate-layout, extensions (+ **native-tier**, + **ui-federation** — mount an extension's
own pages in the shell, module-federation/iframe by trust, host-mediated MCP bridge; scoped 2026-06-27,
not yet built), jobs, bus, inbox-outbox
(+ outbox), tenancy, frontend, sync, testing, debugging, ai-gateway, agent, coding-workflow (+ the
**workflow-driver**, new this slice), registry,
**node-roles** + **platform-targets** (filled this slice — placement × role + the native target tag),
and the **S10 cross-cutting retrofit** trio — **observability**, **audit**, **undo** (scoped
2026-06-27; three projections of the host dispatch chokepoint, sharing the `write_tx` seam — not yet built).
**frontend/dashboard** (scoped 2026-06-27 — the grid-of-widgets dashboard over real series: Phase 1
first-party/seeded `dashboard.*` CRUD + series live SSE + chart/stat/gauge widgets, Phase 2
widgets-as-extensions via the federation bridge, Phase 3 the real edge fleet; `vision/0003` made
buildable — not yet built).
**Promoted to `public/`:** core, auth-caps, mcp, crate-layout, bus, inbox-outbox (+ outbox + the
resolution facet), tenancy, **store** (persistent backend + spike matrix, this slice), **ingest** (this
slice), **tags** (this slice), frontend, sync, files, skills, agent, coding-workflow, registry,
**extensions** (the runtime + two tiers), **frontend/data-console** (the DB-browser + ingest-explorer
pages, this slice) (+ `public/SCOPE.md`).

---

## Next up

00. **Retire the `*.fake.ts` mock backend — DONE (2026-06-27, CLAUDE §9, testing §0).** The UI's 14
    `lib/ipc/*.fake.ts` files + the `fake.ts` dispatcher (a hand-written parallel backend that let work
    *look* shipped on an unbuilt path) are **deleted**. `src/lib/ipc/` now holds only `invoke.ts` (the
    seam) + `http.ts` (the real transport); `invoke` **throws** if no real node is reachable (no fake
    fallback), and the browser defaults `gatewayUrl()` to the local dev node. **Every** UI test now runs
    against a **real spawned gateway node**: `role/gateway/src/bin/test_gateway.rs` (feature-gated
    `test-harness`) boots a real gateway-role node + the production router PLUS test-only `/_seed/*`
    routes (real `lb_inbox::record`/`lb_outbox::enqueue`/`lb_assets::record_install` writes — seeding,
    not faking, §3.1); `ui/vitest.gateway.config.ts` + `src/test/real-gateway.ts` spawn it and run all
    `*.gateway.test.ts[x]` against it, seeded through the real write path. **Vitest now: 6 default
    (pure component/hook/logic) + 18 real-gateway files / 20 + 50 = 70 tests green.** The migration
    surfaced **real gaps the fakes had hidden**: the dev-login claim set was missing the `store:doc/*`,
    `store:skill/*`, and `mcp:workflow.*` caps (so those routes 403'd over the real gateway — now fixed
    in `credentials.rs`), and `useWorkflow.start()` passed an empty channel (invalid bus key — fixed).
    The two production hooks that imported a fake demo-seed (`useExtensions`/`useExtensionPages`) no
    longer do. **Follow-up:** the `agent` surface is unit-tested (its data hook mocked), not
    real-gateway, because `agent_invoke` needs a real model provider the gateway deliberately doesn't
    mock (documented S5 deferral) — wire it when the real provider lands (#3).
0. **S10 — cross-cutting retrofit (scoped 2026-06-27, NOT built)**: three concerns missed since S1,
   each a projection of the host dispatch chokepoint (§6.5/§6.6) and reusing the `write_tx` seam.
   **(a) Observability** (`scope/observability/`) — `tracing` spans/logs/metrics on every node, a
   `trace_id` that propagates across the routed Zenoh hop + into jobs/outbox, secret-safe by
   construction, OTLP export (no in-core dashboard). **(b) Audit** (`scope/audit/`) — an immutable,
   hash-chained, workspace-walled ledger of every allow/deny, appended at the chokepoint (complete by
   construction) and same-`write_tx` durable; generalizes §6.14's model-call audit. **(c) Undo**
   (`scope/undo/`) — a before-image reversible-command journal; the hard line is *reverse state,
   compensate motion* (host derives irreversibility from reaching the outbox). Build order:
   observability → audit → undo. **Co-design note:** observability's `trace_id` propagation and the
   open **token-on-the-bus** item should share **one** routed-call attachment envelope, not two.
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
4. **Gateway/Tauri wiring for `agent_invoke` + `assets_*` + `workflow_*`** (S4/S5/S6 follow-up):
   ~~route the host verbs through the SSE/HTTP gateway to a real node~~ **`assets_*` + `workflow_*`
   SHIPPED over the gateway** (`routes/assets.rs` + `routes/workflow.rs` + `http.ts`/`workflow.api.ts`;
   gate re-checked server-side, ws+principal from the token — see [gateway-assets-workflow
   session](sessions/frontend/gateway-assets-workflow-session.md)). **Remaining: `agent_invoke`** —
   deferred because it needs the real model provider (#3 below; **no mock** in the gateway, by
   decision) — and the **Tauri desktop** command layer for all three (the browser/gateway path is the
   one shipped; the desktop shell still fixes its workspace).
5b. **Management CRUD — close the create-only gap** (scoped 2026-06-27; **slices 1–4 of 4 SHIPPED**). Built
   as four independently-shippable vertical slices: **(1) authz-grants model — SHIPPED** (`lb-authz` +
   host `authz` service: grant/role/team records, `resolve_caps` login projection, `revoke_subject`
   revoke seam, no-widening guard, `grants.*`/`roles.*`/`teams.*` over MCP). **(2) admin-crud backend —
   SHIPPED** (host `users` svc + workspace rename/archive/purge + teams delete/rename cascade +
   members.remove; the **login active-check** wired into `POST /login`; `/admin/*` gateway routes
   re-checking caps; `http.ts` + `admin.fake.ts`). **(3) extensions lifecycle — SHIPPED** (host `ext`
   surface: `ext.list`/`enable`/`disable`/`uninstall` dispatching by tier + boot `reconcile` honoring
   disable + `Install` gaining `tier`/`enabled`; registry **publish** verify-before-store + `POST
   /artifacts`; `/extensions` gateway routes + `http.ts`/`ext.fake.ts` — see [lifecycle-management
   session](sessions/extensions/lifecycle-management-session.md)). **(4) admin console UI — SHIPPED**
   (`features/admin` tabbed section + top-level `features/extensions` console + one shared
   `ConfirmDestructive` every destructive path routes through; `RegistryView`/`NativeView` retired into
   the console, coverage ported; caps surfaced via `LoginReply.caps` → cap-gated nav/tabs, the gateway
   the only boundary; 56 Vitest green — see [admin-console
   session](sessions/frontend/admin-console-session.md)). **All four slices done.** No SDK/WIT change.
   **Remaining follow-ups:** install-from-catalog/upload in the extensions console; a role editor;
   live multi-admin refresh; the Tauri desktop session.
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
