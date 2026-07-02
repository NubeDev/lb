# Control Engine

The `control-engine` native (Tier-2) extension bridges Control Engine (CE) instances into a workspace as
a caps-gated MCP surface (`control-engine.*`): a local CE over `localhost` REST/WS, and remote CEs on
**appliance** LB nodes reached by routed MCP over Zenoh. The visual editor is the vendored
`@nube/ce-wiresheet` package, mounted as the extension's federated page and driven through the MCP bridge.
It is **100% an extension** — no CE concept leaks into core crates. Scope (co-located with the extension):
`rust/extensions/control-engine/docs/control-engine-scope.md`. The generic live-feed primitive S6 builds
on: `docs/scope/extensions/extension-watch-scope.md`.

## Shipped (v1: S1, S2, S3, S4, S5, S6)

- **Graph reads (S3):** `control-engine.tree {appliance, node?, depth?}` → `{nodes, edges}` and
  `control-engine.schema {appliance}` → `{manifests}`, serving the engine's DTOs **verbatim** behind the
  caps gate (the tool NAME is the gate). A supervised native sidecar holds the long-lived `rubix-ce`
  REST/WS client; the ONE sanctioned fake (`ce_fake`, behind `--features ce-fake`) drives the real
  supervisor + gate + stdio ABI in CI, with an opt-in real-engine tier against a live ce-studio.

- **Appliance registry (S4):** `ce_appliance:{ws}:{id}` = `{id, name, mode:"local"|"appliance", node,
  base, secret_ref?, ts}` — the workspace-scoped map from an appliance id to the CE it names. Written
  through the **generic** `store:ce_appliance:*` verbs (no host table code). Verbs:
  `control-engine.appliance.add | list | remove` (each with its own cap; registry writes are distinct
  from graph writes).

- **Resolution + routing (S4):** a graph verb's `appliance` selector resolves (workspace-walled) to a CE
  base — empty → the canonical local CE, a known record → its `base`, an unknown/other-workspace id →
  **not-found** (the isolation wall, no existence leak). **Local vs remote is the host's job, not the
  sidecar's** (symmetric nodes): a call for an appliance owned by another node rides the existing
  cross-node routed-MCP hop to that node, whose identical sidecar serves it locally. Interactive graph
  commands are online request/response — an offline owner **fails loud** (no queue/retry).

- **Graph writes (S5):** the seven v1 command verbs, each a thin caps-gated map onto one `ControlEngine`
  trait method, working local AND routed:
  `control-engine.add-node {appliance, type, parent?, name?, initial_values?}` → `{uid, kind}`;
  `control-engine.patch {appliance, node, values}` → `{component}` (the DTO verbatim);
  `control-engine.set-override {appliance, node, property, value, ttl_secs}` (`0` = permanent) → `{ok}`;
  `control-engine.clear-override {appliance, node, property}` → `{ok}`;
  `control-engine.add-edge {appliance, source, source_property, target, target_property}` → `{uid, kind}`;
  `control-engine.remove-node {appliance, node}` → `{deleted:{component_uids, edge_uids}}` (the 24h-undo
  handle S8's `restore` consumes);
  `control-engine.call-action {appliance, node, action, params?}` → `{returns}`.
  Node identity on the wire is always the **keyed** form (`{uid, kind?, path?}`) — a write MUST address a
  concrete node (no root fallback). Each verb has its own gate `mcp:control-engine.<verb>:call` and
  **self-checks it FIRST** in the sidecar (the inbound `native.call` carries no caller identity;
  defense-in-depth alongside the host's `authorize_tool` at the routed boundary). No new store/net/secret
  cap — the writes reach CE over the already-granted `net:tcp` socket. The optional `{session?, actor?}`
  attribution is **deferred**: the pinned `ce-client-rust` exposes no per-call header hook, so the
  "LB principal → CE actor" mapping is a later follow-up.

- **Live COV feed (S6):** `control-engine.watch {appliance, scope?}` → `{series, subject}`. Arms a CE
  change-of-value subscription for the appliance's scope and pumps each decoded event — re-encoded to a
  plumbing-agnostic JSON **frame** — onto a workspace-scoped series; the shipped `series` motion + gateway
  `GET /series/{series}/stream` SSE is the live read (S7 opens it). Frames:
  `{kind:"cov", ts, values:[{uid,v}], status?:[{uid,s}]}` and `{kind:"topology", ts, msg}` — the sidecar
  re-encodes `rubix-ce`'s already-decoded event, never the binary wire. Integers past `2^53` serialize as
  strings (JS bigint safety). **Lifecycle:** arm-on-first / disarm-on-last per `(appliance, scope)` series
  (an in-memory pump refcount, not durable state); `control-engine.appliance.remove` force-disarms a live
  watch; the pump reconnects on a CE WS drop with bounded backoff (a gap, not a dead stream).
  **Plumbing:** the zero-core-change fallback (`ingest.write` onto a series) behind the same tool name +
  frame contract as the future generic extension-watch primitive — swappable without touching S7.

## Platform primitives S4 added (generic, CE-ignorant — usable by any extension)

- **Native sidecars are first-class in the MCP routing registry.** A `LocalDispatch` trait
  (`lb_runtime`) abstracts the local-call target; both a wasm instance and a native sidecar (via a host
  `SidecarDispatch` adapter registered at `install_native`) implement it, so `resolve`/`dispatch`/
  `serve_call`/the catalog treat every tier uniformly — a native ext is reachable locally AND over the
  cross-node routed hop, with no per-tier branch.
- **`ingest.write` over the MCP bridge now publishes live motion (S6).** Previously only the gateway's
  `POST /ingest` HTTP route published a sample onto its `ws/{id}/series/{series}` motion subject; the MCP
  `ingest.write` verb was durable-only, so a sidecar-written sample never surfaced on the
  `GET /series/{s}/stream` SSE. The MCP path now publishes motion after the durable write (best-effort,
  producer stamped to the caller), matching the HTTP route — any MCP `ingest.write` caller benefits, not
  just CE. Generic and domain-free; core stays CE-ignorant.
- **`store.write` / `store.delete`** — generic host-native MCP verbs, gated per-table by the
  `store:<table>:<action>` capability grammar (the write half of the direct-store contract; `store.query`
  /`store.schema` are the read half). An extension gets a caps-scoped write path to its OWN table with no
  host code knowing that table exists.

## Deferred (additive, same path)

- **S5 + S6 (shipped, above).** Still deferred, additive on the same path: `remove-edge`, `restore`,
  `copy`, `bulk`, `set-layout`, graph-import-as-a-job, `secret_ref` mediation on the appliance record; the
  COV `schema` frame kind (the pinned client's `CovEvent` surfaces only values + topology today); the
  opt-in per-appliance historian (`history:[prop-uid]` mirrored to a durable series); swapping the live-COV
  fallback plumbing to the generic extension-watch primitive when it lands; and **S8** batching/buffering
  measured under load (v1 coalesces to CE's server-side tick only).
- **S7** the vendored `@nube/ce-wiresheet` federated page over a `BridgeTransport` (the S1 seam).
- A discovery layer that reads `ce_appliance` records to populate the remote-routing entry (S4 stands it
  in with `register_remote_extension` in tests).
