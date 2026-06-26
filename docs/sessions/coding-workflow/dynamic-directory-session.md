# Coding workflow — the dynamic workspace directory (session)

- Date: 2026-06-27
- Scope: ../../scope/coding-workflow/workflow-driver-scope.md (the "dynamic workspace set" open question)
- Stage: S7 — platform maturity (STAGES.md). The workflow-driver follow-up: onboard/retire a workspace
  **without restarting the node**. The S7 exit gate was already MET.
- Status: done

## Goal

The driver shipped with a binding list fixed at boot — adding a workspace meant a restart. Make the set
of serviced workspaces a **durable, runtime-mutable directory** the driver re-reads each tick, so
`register_workspace` onboards and `deregister_workspace` retires, both taking effect on the next tick.

## What changed

**Host: the directory (`crates/host/src/workflow/directory.rs`, new):**

- `WorkspaceEntry { ws, channel, status, ts }` + `EntryStatus { Enabled, Disabled }`. The status is a
  **kebab-case string** discriminant (not a bool) so the generic store equality filter — which binds
  the compared value as a string — can select on it (the same reason the outbox stores `status` as a
  string; a bool field would never match a `"true"` string param). The entry is **secret-free** — the
  service principal is minted from caps by the binary at tick time, so no credential is persisted.
- `register(store, ws, channel, ts)` — upsert an enabled entry (idempotent; re-register re-enables +
  updates the channel). `deregister(store, ws, ts)` — soft-disable (the row stays for audit/re-enable).
  `enabled_workspaces(store)` — the enabled-only scan, oldest→newest, the driver's per-tick read.
- Lives in a **reserved namespace** `DIRECTORY_NS = "_lb_workflow_directory"`, NOT inside a tenant
  workspace: it is node-level operator config (which workspaces this node drives), and a per-workspace
  directory is a chicken-and-egg. This is the one deliberate exception to "every key is
  workspace-scoped" — node-infrastructure state, like the relay loop's existence — and the *entries*
  still name real workspaces, into which every reactor/relay call re-enters with its caps gate.
- Raw verbs, **no MCP surface** (infrastructure, like the relay — not reachable through a workspace
  token). Exported from `lb-host` as `register_workspace` / `deregister_workspace` (prefixed at the
  crate boundary so a bare `register` isn't ambiguous next to `register_remote_extension`) +
  `enabled_workspaces`, `WorkspaceEntry`, `EntryStatus`, `DIRECTORY_NS`.

**Driver: directory-backed loop (`role/github-workflow/src/directory_drive.rs`, new):**

- `drive_directory_once(node, target, now, principal_for, on_error)` — read `enabled_workspaces`,
  build a `WorkflowBinding` per entry (minting its principal via the injected `principal_for(ws)`
  closure — the crate has no caps knowledge), then `drive_once` them all. A directory-read error is
  reported and the tick skipped (the next re-reads — never wedged).
- `run_directory_loop(...)` — the forever loop, re-reading the directory each tick (so runtime
  register/deregister take effect next tick), with the same injected clock + per-ws error isolation.

**Node wiring (`node/src/github.rs`):** `mount` is now async; it **seeds** the directory with the
`LB_WORKFLOW_WS` workspace at boot, then runs `run_directory_loop` (instead of a fixed binding). So the
env supplies the initial workspace and an operator can `register_workspace` more at runtime.

## How it fits the core (the platform checklist)

- **Stateless service, durable truth.** The directory is a record, re-read each tick — the driver
  holds no in-memory workspace set. Restart the node and the directory is intact (a `register` survives
  because it is a row). Same discipline as the relay/reactor, lifted to the *set* of workspaces.
- **Workspace is the hard wall.** Each entry names a real workspace; every reactor/relay call the
  driver then makes re-enters that namespace and its caps gate. The reserved directory namespace is
  structurally separate from any tenant's (the isolation test proves a directory entry is not visible
  as data inside the named workspace). The multi-workspace tick test proves no job/effect crosses.
- **No `if cloud`.** The directory + the directory loop are the same on every node; which workspaces a
  node drives is its directory content (config), not a code branch.
- **No SDK/WIT/cap-grammar change.** A durable table + three verbs + a loop variant + binary wiring.

## Tests (all green — pasted below)

Host (`crates/host/tests/workflow_directory_test.rs`, 5): register→list; deregister soft-disables +
drops from the scan; register is idempotent + re-enables; durability (independent reads see prior
writes); the directory namespace is separate from a tenant workspace.

Driver (`role/github-workflow/tests/directory_driver_test.rs`, 3): **a workspace registered at runtime
is picked up the next tick** (the headline — no restart); a deregistered workspace is dropped the next
tick; the directory driver isolates two workspaces (mandatory isolation — each job in its own ws).

```
$ cargo test -p lb-host --test workflow_directory_test
running 5 tests ... test result: ok. 5 passed; 0 failed

$ cargo test -p lb-role-github-workflow
   tests/directory_driver_test.rs ... ok. 3 passed   (NEW)
   tests/driver_test.rs ............. ok. 4 passed    (regression)

$ cargo build --workspace      # green (incl. the node binary)
$ cargo fmt --all --check       # clean
$ cargo clippy -p lb-host -p lb-role-github-workflow --tests   # no new warnings in the touched crates
$ bash scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (337 checked)
```

Net: **~222 Rust + 26 Vitest + 2 shell** tests green (+8 Rust this slice).

## Decisions & alternatives rejected

- **A reserved namespace, not a per-workspace table.** The directory is node-level (which workspaces
  this node drives). Putting it in a workspace would be a chicken-and-egg and would scope node config
  to one tenant. *Rejected:* a config file — it would not survive a runtime change without a reload and
  re-introduces the restart this slice removes.
- **`status` string, not a bool.** The store equality filter binds the compared value as a string, so a
  bool `enabled` would never match — the same constraint that makes the outbox store its status as a
  string. Modeled it as an enum discriminant from the start.
- **Soft-disable, not delete.** Deregister keeps the row (audit + cheap re-enable). A hard delete is a
  later GC concern, not the runtime path.
- **Injected `principal_for`.** The driver crate has no caps/identity knowledge; the binary mints the
  service principal. Keeps the crate a pure orchestration loop (no `lb-auth` policy inside it).

## Open questions still open

- **Dynamic webhook tenant directory + `lb-secrets`.** The webhook `TenantRegistry` is still built at
  boot; making it directory-backed needs per-tenant **secrets** done right (behind `lb-secrets`), so
  the two are paired — the next slice. (The workflow directory is deliberately secret-free for this
  reason.)
- **An MCP/admin surface to register a workspace.** The verbs are host-internal today (an operator
  drives them); exposing a capability-gated admin tool is a follow-up.
- **GC of disabled rows.** A retention/compaction policy for long-disabled entries (deferred, like the
  outbox's delivered-row compaction).

## Cross-links

- Scope: ../../scope/coding-workflow/workflow-driver-scope.md ("dynamic workspace set" resolved;
  webhook tenant directory + `lb-secrets`, admin surface, GC still open).
- Public: ../../public/coding-workflow/coding-workflow.md (the driver is now dynamic).
- Builds on: ./workflow-driver-session.md (the driver this makes dynamic).
- No debugging entry — nothing broke (a stale `enabled=false` doc comment was fixed in review).
