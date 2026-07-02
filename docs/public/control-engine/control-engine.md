# Control Engine

The `control-engine` native (Tier-2) extension bridges Control Engine (CE) instances into a workspace as
a caps-gated MCP surface (`control-engine.*`): a local CE over `localhost` REST/WS, and remote CEs on
**appliance** LB nodes reached by routed MCP over Zenoh. The visual editor is the vendored
`@nube/ce-wiresheet` package, mounted as the extension's federated page and driven through the MCP bridge.
It is **100% an extension** — no CE concept leaks into core crates. Scope (co-located with the extension):
`rust/extensions/control-engine/docs/control-engine-scope.md`. The generic live-feed primitive S6 builds
on: `docs/scope/extensions/extension-watch-scope.md`.

## Shipped (v1: S1, S3, S4)

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

## Platform primitives S4 added (generic, CE-ignorant — usable by any extension)

- **Native sidecars are first-class in the MCP routing registry.** A `LocalDispatch` trait
  (`lb_runtime`) abstracts the local-call target; both a wasm instance and a native sidecar (via a host
  `SidecarDispatch` adapter registered at `install_native`) implement it, so `resolve`/`dispatch`/
  `serve_call`/the catalog treat every tier uniformly — a native ext is reachable locally AND over the
  cross-node routed hop, with no per-tier branch.
- **`store.write` / `store.delete`** — generic host-native MCP verbs, gated per-table by the
  `store:<table>:<action>` capability grammar (the write half of the direct-store contract; `store.query`
  /`store.schema` are the read half). An extension gets a caps-scoped write path to its OWN table with no
  host code knowing that table exists.

## Deferred (additive, same path)

- **S5** write verbs (`patch`/`set-override`/`call-action`/`add-node`/`add-edge`/`remove-node`) +
  `secret_ref` mediation on the appliance record.
- **S6** `control-engine.watch` change-of-value (COV) live feed; `appliance.remove` disarming a live watch.
- **S7** the vendored `@nube/ce-wiresheet` federated page over a `BridgeTransport` (the S1 seam).
- A discovery layer that reads `ce_appliance` records to populate the remote-routing entry (S4 stands it
  in with `register_remote_extension` in tests).
