# Node roles scope

Status: scope (the ask). The roles a single node binary takes on — **config and role, never a code
branch** (README §3.1, the symmetric-nodes non-negotiable). This doc records the role axis as it
actually exists in the code (`lb_host::Role`, `Node::boot_as`) and the one thing the **native tier**
(S7) adds to it: how placement interacts with role for an OS-process extension.

> Read with: `README.md` §3.1 (symmetric nodes), §6.8 (authority partition), `STAGES.md`
> (S3 made roles real; S7 native tier touches placement), `extensions/native-tier-scope.md`
> (the native placement question this answers), `extensions/extensions-scope.md` (`placement`).

## Goals

- Name the roles and state that they are **config**, selected at boot, never an `if cloud {…}` in
  core crates.
- Record where role *is* allowed to matter (the two thin wiring layers) and where it must not (every
  core crate — store, bus, caps, mcp, runtime, the host services).
- Answer the native-tier question: how a `placement` (`local-only`/`cloud-only`/`either`) extension
  is scheduled against a node's role **without** a code branch.

## Non-goals

- Role-specific *config schemas* (ports, peer lists, sync intervals) — operational detail, not the
  architecture axis.
- New roles beyond what ships (`Solo`/`Edge`/`Hub`); coin one only for genuinely new surface.

## The roles (as they exist)

`lb_host::Role` is the config the wiring layers read; `Node::boot_as(role)` opens the **same** store
+ Zenoh peer + runtime engine for every role (`boot.rs`). The differences live only in what the
`node` binary *mounts*:

- **`Solo`** — N=1, its own authority, fully offline-capable. The S1→S2 posture. No sync relay, no
  gateway required.
- **`Edge`** — a peer that syncs to a hub and may run offline, reconnecting with idempotent apply
  (§6.8). Mounts the sync client.
- **`Hub`** — the cloud authority: mounts the sync relay + the SSE/HTTP gateway + (S7) the
  registry-host origin. Still the same binary; "cloud" is this role, not a second codebase.

A role is **the data-authority axis** (§6.8: who is authoritative for a record) plus **which wiring
layers mount**. It is read by the `node` binary and the gateway/sync layers — **never** by a core
crate. If a core crate ever needs to branch on role, the design is wrong (the S1 non-negotiable).

## How it fits the core

- **Symmetric nodes:** the whole point. `boot_as` proves it — one constructor, role only selects
  mounts. Tested since S3 (`Node::boot_as(role)`, the second node is just another `boot_as`).
- **Capabilities / tenancy:** role does not change the workspace wall or the capability gate — those
  run identically on every role. A hub is not "more privileged"; it is authoritative for different
  records.
- **Placement (the native-tier addition):** an extension manifest's `placement` is matched against
  the node's role **as data, by the loader/scheduler — not a code branch**:
  - `either` → schedulable on any role (the default; a portable wasm extension).
  - `cloud-only` → scheduled only where `role` is a hub-class role; on an edge it is simply *not
    scheduled* (the install record persists; no instance starts) — a data check, not `if cloud`.
  - `local-only` → scheduled only on the node it was installed to (e.g. a native sidecar bound to
    local hardware).
  This is exactly the symmetric-nodes rule applied to scheduling: placement is metadata the loader
  reads, and "not scheduled here" is a `match` on config, never a behavioral fork in a core path.
  A **native** extension additionally carries a platform target (platform-targets scope) — a
  native binary is not portable the way a `.wasm` is, so placement *and* target both gate where it
  can run.

## Testing plan

- Covered transitively by the existing S3 multi-node tests (`boot_as(role)`, cross-node routing) —
  role is config there already.
- Native-tier slice: a placement-vs-role scheduling check belongs with the native scope's tests if
  scheduling lands; this slice spawns on the installing node (`local`-style), so the deeper
  placement matrix is exercised when cloud-only native scheduling is built (follow-up).

## Open questions

- A `role`-aware scheduler that *places* an `either` extension on the best-fit node (vs. installing
  where asked) — deferred; today install targets the node it is called on.
- Hub-class sub-roles (a dedicated registry-host vs. a full hub) — the `registry-host` role crate
  exists as a placeholder; whether it is a distinct `Role` variant or a mount flag is open.

## Related

- `README.md` §3.1, §6.8. `crates/host/src/role.rs`, `crates/host/src/boot.rs` (`boot_as`).
- `extensions/native-tier-scope.md` (placement × role for a process), `extensions/extensions-scope.md`
  (`placement` field), `platform-targets/platform-targets-scope.md` (the target axis a native
  binary adds on top of placement), `sync/sync-scope.md` (the authority axis a role selects).
