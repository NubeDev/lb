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

**S4 complete → entering S5 — AI core** (see `STAGES.md`). The Rust workspace + a React/Tauri UI
exist and build; messaging, a second node + sync, cross-node routed tool calls, the browser
SSE/HTTP path, and now **shared workspace assets** (docs/skills/install records behind
capability- and membership-checked reads) are all proven end to end. No doc-site build and no
native desktop window (webkit toolchain) yet.

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

**Exit gate (S5):** an edge user invokes the central agent; the agent calls the gateway for a model
and a granted MCP tool; a workflow job survives the edge disconnecting and resumes.

---

## Slices in flight

One row per vertical slice being built. State: `scoped` → `building` → `tested` → `shipped`.

| Slice | Topic | Stage | State | Scope | Session | Notes |
|---|---|---|---|---|---|---|
| Spine | core | S1 | **shipped** | [core](scope/core/core-scope.md) | [s0-s1-spine](sessions/core/s0-s1-spine-session.md) | host+store+bus+caps+mcp+runtime+1 WASM ext |
| Messaging | bus | S2 | **shipped** | [bus](scope/bus/bus-scope.md) | [messaging](sessions/bus/messaging-session.md) | pub/sub + presence + inbox + channel svc + React/Tauri UI + hot-reload |
| Sync / SSE | sync | S3 | **shipped** | [sync](scope/sync/sync-scope.md) | [multi-node-sync](sessions/sync/multi-node-sync-session.md) | 2nd node (role=config) + queryable routed MCP + edge↔hub sync + axum SSE/HTTP gateway + UI transport swap; 61+8+2 green |
| Shared assets | files | S4 | **shipped** | [files](scope/files/files-scope.md) + [skills](scope/skills/skills-scope.md) | [shared-assets](sessions/files/shared-assets-session.md) | `lb-assets` crate + host 3-gate (ws→cap→membership) doc/skill svc + grant-gated skills + persisted install records + `assets.*` MCP bridge + UI DocView; 83+11+2 green |

---

## Scopes authored (ready to build)

The `scope/<topic>/` docs exist for all areas (see `scope/README.md`). **Fully authored:** core,
auth-caps, mcp, crate-layout, extensions, jobs, bus, inbox-outbox, tenancy, frontend, sync, testing,
debugging. **Promoted to `public/`:** core, auth-caps, mcp, crate-layout, bus, inbox-outbox,
tenancy, store, frontend, sync, **files, skills** (+ `public/SCOPE.md`). The remaining `public/` and
`sessions/` files are still stubs until their slice ships.

---

## Next up

1. **S5 AI core:** a central workspace-scoped AI agent on the hub, callable by edge users over the
   routed MCP namespace; remote workflow jobs (durable, resumable); the AI-gateway sidecar. The
   agent loads granted **skills** (S4) and reads shared **docs** (S4) as its substrate.
2. **Gateway/Tauri wiring for `assets_*`** (S4 follow-up, not blocking S5): the host verbs + MCP
   bridge + UI fake exist; route `assets_*` through the SSE/HTTP gateway + Tauri shell to a real
   node (mirrors the S3 channel transport swap).
3. **Transactional must-deliver outbox** (§6.10) — S3 shipped the append-style idempotent-apply
   sync subset; the durable outbox with a delivery cursor + change-feed-driven relay is next
   (bus + inbox-outbox + sync open questions).
4. **Fit-and-finish carryover:** render presence in the UI; a real login→token→principal session
   (replacing the gateway's demo principal — the S4 asset verbs derive the owner from it); sync the
   asset tables (doc/skill/install are `(table,id)` upserts the channel sync path already covers);
   serve-side auth for hub-authoritative routed calls; explicit edge→hub router endpoints (S7).

---

## How to keep this current

Every session that changes state updates the relevant cell here as its **last step**
(`HOW-TO-CODE.md` §3 step 9). Keep it to one screen — if a section grows past a few rows,
the detail belongs in the per-feature docs, not here.
