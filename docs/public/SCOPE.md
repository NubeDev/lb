# Project scope (as built)

The trimmed source of truth for what exists now. The full architecture spec is the root
`README.md`; the staged plan is `../STAGES.md`; live status is `../STATUS.md`.

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
| Job queue | thin **native** SurrealDB queue (not apalis), built at S5 | `../scope/jobs/` |
| Extension manifest | **TOML** `extension.toml`; host grants `requested ∩ approved` | `../scope/extensions/` |

## Not yet built

AI core (S5); coding workflow (S6); registry + native tier (S7). The
transactional must-deliver **outbox** (S3 shipped the append-style idempotent-apply sync subset;
the durable outbox with a delivery cursor + change-feed-driven relay is next), bus message
classification, serve-side authorization for hub-authoritative routed calls, and explicit
edge→hub router endpoints (S7) remain. Tracked in `../STATUS.md` and `../STAGES.md`.
