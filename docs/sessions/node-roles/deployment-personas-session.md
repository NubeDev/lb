# Session — deployment personas (hub / appliance / workstation / mobile)

**Topic:** naming the e2e deployment surfaces; docs-only.
**Date:** 2026-06-27.

## The ask

Set up Docker fixtures for full end-to-end testing with **two edge clients/users + a
hub**. The user clarified that "edge" means a *full vertical stack* (SurrealDB + UI +
single user), and asked for terminology: a Pi-class box vs a desktop user — and later
added a Flutter "app" surface.

## Constraints surfaced (by the user)

- **"node" is overloaded** — Node-RED uses it, and `README.md` §5 already uses "node"
  for *any* running binary (role-neutral). Can't mean "a hardware edge box."
- **"station" is out** — collides with Tridium Niagara (building-automation tooling).
- There are really **three edge surfaces**, matching `README.md` §5's "desktop, Pi,
  mobile": a headless Pi, a desktop user, and a Flutter app.

## Decision — personas, not new roles

A **role** is the architectural axis (`edge` / `hub` / `solo`, the `Role` enum in
`crates/host/src/role.rs`). A **persona** is a *named deployment* of a role, used only
for docs / compose / e2e fixtures. Personas add **no code branch** (§3, rule 1).

| Persona         | Role        | Stack                                              |
|-----------------|-------------|----------------------------------------------------|
| `hub`           | cloud-hub   | router · authority · AI gateway · registry · SSE   |
| `appliance` | **edge**    | SurrealDB + Zenoh peer · **no UI** · offline       |
| `workstation`  | **edge**    | same stack + **Tauri** UI (§6.12) · offline        |
| `mobile`        | *client*    | Flutter UI → hub gateway · **no local store**      |

`appliance` + `workstation` are the two full edge **nodes** (what was loosely
"edge") — same edge role, differing only by whether the UI mounts; `hub` is the server;
`mobile` is a UI **client** that rides the hub. Canonical fixture: `hub` + `appliance` +
`workstation`, `mobile` optional.

**Naming history:** the "app" surface was renamed `mobile` (clearer, matches §5's "mobile").
A brief intermediate pass used `edge-headless` / `edge-desktop` to make the shared role
explicit in the name; reverted to the standalone device names `appliance` / `workstation`
(the role is carried by the table's Role column, not the persona word — a reader cares what
the box *is*, not which role it maps to). "node" and "station" stay rejected (collisions:
Node-RED / the role-neutral "node"; Tridium Niagara "station").

**Rejected:** adding a new role/tier for the desktop persona (would imply role-specific
code, violating §3 rule 1); reusing "node" or "station" as persona names (collisions:
Node-RED / the role-neutral "node"; Tridium Niagara "station").

## Changes made (docs only)

- `README.md` §5 — added a **"Deployment personas (naming, not new roles)"** block
  defining the four personas and the canonical e2e fixture.
- `docker/README.md` — replaced the placeholder `cloud / edge-1 / edge-2` with the
  `hub / appliance / workstation / app` persona dirs.

## NOT done — and why (the blocker for actually running e2e)

The `node` binary **cannot yet run this topology**. Verified in `rust/node/src/main.rs`
+ `crates/bus/src/peer.rs`:

- `node` never reads the `Role` enum (no `LB_ROLE`); it boots a **solo** demo
  (hello.echo) + optional gateway/github roles only.
- `Bus::peer()` always opens a bare default Zenoh peer — **no router/connect config**
  (no `LB_ZENOH_MODE` / `LB_ZENOH_CONNECT`), so edges can't dial a hub.
- `Node::boot()` ignores `LB_STORE_PATH` (each container needs its own volume).

So writing live compose files now would only run the solo demo N times. The naming is
settled; the **binary config slice** (`LB_ROLE` + Zenoh router/connect + `LB_STORE_PATH`,
with isolation/deny/sync tests per `docs/scope/testing/testing-scope.md`) is the
prerequisite to real Docker e2e. That slice is the next step.

## Follow-on scope

The user then asked whether the backend/API lets an admin **see all connected clients**
(appliances/workstations/apps). Inventory: channel-member presence is shipped
(`crates/bus/src/presence.rs`), but **node-level** identity/roster is entirely absent (no
`NodeId`, `Role` is config-only, no `nodes.list`). Wrote
`scope/node-roles/fleet-presence-scope.md` — a node-presence roster reusing the liveliness
pattern, with the persona terms landing in code. Prerequisite is the same `LB_ROLE`/Zenoh
config slice noted above.

## Tests

Docs-only change; no code touched, so no test run. The follow-up binary slice carries the
mandatory isolation + capability-deny + sync tests.
