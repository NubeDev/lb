# Session — `assets.*` + `workflow.*` over the gateway (Next-up item 4)

**Date:** 2026-06-27 · **Stage:** S9+ (gateway/transport wiring) · **State:** shipped

## The ask

STATUS "Next up" item 4: *"Gateway/Tauri wiring for `agent_invoke` + `assets_*` + `workflow_*`
(S4/S5/S6 follow-up): the host verbs + MCP bridges + UI fakes exist; route them through the SSE/HTTP
gateway to a real node (mirrors the S3 channel transport swap)."*

The gap was concrete: the host had the doc/skill verbs (`lb_host::put_doc`/`get_doc`/…), the
coding-workflow verbs (`request_approval`/`resolve_approval`/`start_coding_job`), and their `assets.*`
/ `workflow.*` MCP bridges — but **only the Tauri shell reached them**. In a plain browser the IPC
seam fell through to the in-memory fake; against a real node gateway it threw `unknown command`. This
slice adds the missing REST routes + the `http.ts` cases, exactly the one-transport change the S3
channel swap and the lifecycle-management `ext.*` slice established.

## Decision: no mock; agent deferred

The agent's `invoke` needs a `ModelAccess`, and the only impl that exists is the scripted
`MockProvider` (a real provider is its own Next-up item — #3). The user's call was **"no mock"** — so
a "real" agent route is not buildable in this slice without the real provider. We therefore **wired
`assets_*` + `workflow_*` now** (real host verbs, no model needed) and **left `agent_invoke` for the
real-provider slice**. When that lands, the agent route is a thin sibling of these (a `POST /agent/invoke`
over `lb_host::invoke`), reusing the same authenticate-then-forward pattern.

## What shipped

### Gateway (Rust, `role/gateway`)

- **`routes/assets.rs`** — the browser's `assets.*` surface, each route a thin call over the host verb
  (the established gateway pattern — call `lb_host::<verb>` directly, *not* the MCP bridge, so the gate
  is the `store:doc/*` / `store:skill/*` capability, consistent with `inbox`/`ext` routes):
  - `GET|POST /docs` → `list_docs` / `put_doc`
  - `GET /docs/{id}` → `get_doc`
  - `POST /docs/{id}/share` → `share_doc`, `POST /docs/{id}/link` → `link_doc`
  - `POST /skills` → `put_skill`, `GET /skills/{id}?version=` → `load_skill`,
    `POST /skills/{id}/grant` → `grant_skill`
  - Status map: gate refusal → `403` (opaque), `NotFound` (only after the gates pass) → `404`.
- **`routes/workflow.rs`** — the browser's `workflow.*` surface:
  - `POST /approvals/{id}/request` → `request_approval` (records the `PrSpec` keyed by approval id)
  - `POST /approvals/{id}/resolve` → `resolve_approval`
  - `POST /approvals/{id}/start` → `start_coding_job` — reads the `PrSpec` back by approval id
    (the same contract the MCP bridge's `start_job` uses; no PR args on the start wire). The S6
    **approval gate** surfaces as `{ started: false }` (NOT an error) when the approval isn't
    `Approved` — the genuine gate, shown to the user.
  - Reading the outbox (the "PR queued → delivered" view) is the **existing** `GET /outbox`; not
    re-exposed.
- Both modules wired into `server.rs`'s `router` + re-exported from `routes/mod.rs` (the workflow
  `resolve_approval` aliased to `resolve_workflow_approval` to avoid colliding with the inbox one).
- `lb-assets` added as a gateway dep (for the `Doc`/`Skill` return types — `lb_host` doesn't re-export
  them; same as `ext.rs` importing `lb_registry` types).
- **Every route derives the workspace + principal from the token, never the body** (the hard wall, §7)
  — the `ws`/`author` the UI fake passes are simply dropped.

### UI (`ui/src/lib/ipc`)

- **`http.ts`** — `assets_*` cases (`put_doc`/`get_doc`/`list_docs`/`share_doc`/`link_doc`/`put_skill`/
  `grant_skill`/`load_skill`) and `workflow_*` cases (`request_approval`/`resolve_approval`/`start_job`/
  `list_effects`). `workflow_list_effects` has no per-workflow node verb — it maps onto `GET /outbox`
  and flattens the lifecycle groups into the workflow `Effect[]` shape (`dead-lettered` → `failed`).
- **`workflow.api.ts`** — added `requestApproval` (the real-node producer the gateway path needs before
  a job can start; the fake doesn't model it). `startCodingJob` gained optional `scopeDoc`/`channel`/
  `prKey` (the fake ignores them — it only models the gate; the real route needs them). `PrSpec` type
  added to `workflow.types.ts`.

## Tests

- **Rust:** new `role/gateway/tests/assets_workflow_routes_test.rs` (7 tests, all green), driving the
  real routes with `oneshot` over real signed sessions:
  - assets: put→get→list round-trip · **put without the write cap → `403`** (server-side, the token's
    caps not the body) · **ws-B session cannot read ws-A's doc** (`404`, two sessions one node) ·
    grant→load skill round-trip + **ungranted load → `403`**.
  - workflow: **approval gate** (start before approval → `started:false`; after `approved` →
    `started:true`) · **verb without the cap → `403`** · **ws-B cannot resolve/start ws-A's approval**
    (ws-B has no PR spec for ws-A's id → `400`; ws-A's own start still works — its spec/approval
    intact).
- Full `cargo test -p lb-role-gateway` green (7 + 5 + 9). `cargo build --workspace`, `cargo fmt`,
  file-size all green.
- **Vitest:** 58/59 pass; `tsc` clean on every file this slice touched. The one red
  (`GrantsAdmin.test.tsx`, + a `listRoles` tsc error) is a **separate, concurrent in-flight
  admin/roles refactor** (untracked `AccessEditor.tsx`/`roles.api.ts`/`useRoles.ts`/…) — not this
  slice's files, not introduced here.

## Notes / follow-ups

- **`agent_invoke` still deferred** — needs the real model provider (Next-up #3). The route is a thin
  sibling of these once it lands.
- **Tauri command layer** for these verbs is the desktop counterpart (the collaboration slice wired the
  browser/gateway path; the desktop shell still fixes its workspace) — same as the standing carryover.
- The UI **feature code/views** for assets + workflow already exist (they ran on the fake); they now
  reach a real node when `VITE_GATEWAY_URL` is set — no view change needed, the seam is `http.ts`.
