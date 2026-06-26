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

**In S7 — platform maturity** (see `STAGES.md`); **both** S7 exit-gate slices have shipped — the
**signed extension registry** and the **native Tier-2 supervisor** — so the **S7 exit gate is fully
MET**. The Rust workspace + a React/Tauri UI exist and build; messaging, a second node + sync,
cross-node routed tool calls, the browser SSE/HTTP path, shared workspace assets, the **AI core**
(central agent + AI-gateway sidecar + durable jobs), the **coding workflow** (issue → triage →
approval-gated job → progress → transactional outbox), the **signed registry** (pull · verify · cache ·
install · offline · rollback), and now the **native Tier-2 supervisor** (a supervised OS-process
sidecar that restarts cleanly with no durable state lost) are all proven end to end. Packaging the S6
workflow as installed artifacts remains (S7 follow-up). No doc-site build and no native desktop window
(webkit toolchain) yet.

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

---

## Scopes authored (ready to build)

The `scope/<topic>/` docs exist for all areas (see `scope/README.md`). **Fully authored:** core,
auth-caps, mcp, crate-layout, extensions (+ **native-tier**, new this slice), jobs, bus, inbox-outbox
(+ outbox), tenancy, frontend, sync, testing, debugging, ai-gateway, agent, coding-workflow, registry,
**node-roles** + **platform-targets** (filled this slice — placement × role + the native target tag).
**Promoted to `public/`:** core, auth-caps, mcp, crate-layout, bus, inbox-outbox (+ outbox + the
resolution facet), tenancy, store, frontend, sync, files, skills, agent, coding-workflow, registry,
**extensions** (the runtime + two tiers, filled this slice) (+ `public/SCOPE.md`).

---

## Next up

1. **S7 platform maturity** (`STAGES.md`): the **extension registry** AND the **native Tier-2
   supervisor** are **shipped** — the **S7 exit gate is fully MET** (~~install from a signed registry,
   run offline once cached, roll back~~; ~~a native sidecar is supervised and restarts cleanly~~).
   Remaining S7 work: **packaging the S6 workflow/github-bridge as installed wasm artifacts** (they are
   host services today; the registry now exists to install them through — author `scope/extensions/`
   for the packaging first). **Native-tier follow-ups:** a boot reconciler (re-spawn `lifecycle=started`
   from records), OS-level hardening (cgroups/seccomp/userns), a background health-poll reactor (the
   slice restarts on-demand at the call boundary), the child→host MCP callback transport, and native
   platform-target enforcement.
1b. **Registry follow-ups** (deferred from this slice, all in `scope/registry/`): a real HTTP
   `Source`/`registry-host` server (the in-memory source is the only stub); a durable publisher-key
   allow-list + admin trust-management flow; key rotation/revocation (needs the hub identity directory);
   cache eviction/GC; the public catalog read-only union (per-workspace entries ship now);
   `registry.update` semantics; gateway/Tauri wiring for `registry_*`.
2. **Real outbox `Target` adapters + relay hardening** — GitHub HTTP / email / the sync publish
   behind the `Target` trait (in-test target is the only stub); **backoff + dead-letter**, the
   **multi-relay atomic claim**, FIFO-per-target ordering, and the **LIVE-query relay reactor** (S6
   uses durable scans + an explicit `start_job`); plus a **resolution reactor** that auto-starts the
   job on approval.
3. **Real model provider + streaming** behind the S5 gateway contract — the mock is the only stub;
   add an OpenAI-compatible / local adapter and stream tokens as Zenoh motion. Agent/job progress can
   now also ride the durable outbox for the must-deliver transcript.
4. **Gateway/Tauri wiring for `agent_invoke` + `assets_*` + `workflow_*`** (S4/S5/S6 follow-up): the
   host verbs + MCP bridges + UI fakes exist; route them through the SSE/HTTP gateway + Tauri shell to
   a real node (mirrors the S3 channel transport swap).
5. **Fit-and-finish carryover:** render presence in the UI; a real login→token→principal session
   (replacing the gateway's demo principal); **token-on-the-bus** so the hub can verify a routed
   caller's grant (S5/S6 are in-process co-trust); sync the asset/job/outbox tables (all `(table,id)`
   upserts the channel sync path already covers); explicit edge→hub router endpoints (S7).

---

## How to keep this current

Every session that changes state updates the relevant cell here as its **last step**
(`HOW-TO-CODE.md` §3 step 9). Keep it to one screen — if a section grows past a few rows,
the detail belongs in the per-feature docs, not here.
