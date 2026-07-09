# Desktop scope — persistent store for the standalone `full` build

Status: scope (the ask). Promotes to `public/desktop/desktop.md` once shipped.

The standalone `full` desktop binary boots a **100% in-memory node per launch** (`Node::boot` →
`open_store()` uses `Store::memory()` because `LB_STORE_PATH` is unset). So **every restart loses all
user work** — channels, dashboards, panels, registered datasources, agent runs, everything. A shipped
desktop app must not do this. This scope makes the `full` desktop default to a **durable on-disk
store under the user's data directory**, so state survives restart — closing the standalone scope's
recorded persistence non-goal (its open question "should full default to a durable store under the
user's data dir? Leaning: yes, but as a separate [scope]").

## Goals

- The **windowed** `full` desktop binary (the shipped app) opens a **persistent SurrealKV store** by
  default, at an OS-standard per-user data directory (`dirs::data_dir()/lazybones/store`). Write
  something, quit, relaunch — it is still there.
- The default is **config, not a code branch** (§3.1): the desktop binary resolves and sets
  `LB_STORE_PATH` before booting the node, exactly the "thin boot-wiring layer that reads config"
  the store selector already documents. `Node::boot`/`open_store` are **unchanged** — they already do
  the right thing when `LB_STORE_PATH` is set (`Store::open`, durable). No core change.
- **Overridable**: `LB_STORE_PATH` set in the environment wins (a portable/USB layout, a custom
  location, a test). Unset → the per-user default. Empty → in-memory (the escape hatch stays).
- **Seeders stay idempotent and run every boot** (the chosen posture): the built-in role/skill/agent
  seeders are LWW-upserts, so an app update refreshes built-ins without clobbering user data; the
  federation install + demo-source pre-registration are already idempotent. A returning user keeps
  their channels/dashboards; their built-ins stay current.
- **Tests stay in-memory.** The default-path resolution lives at the **windowed binary boundary**
  (`desktop.rs::run`), NOT in `NodeHandle::boot` / `Node::boot` — the headless command tests and the
  `full_loopback`/`full_federation` integration tests call those directly and MUST stay ephemeral +
  isolated (a shared on-disk path would cross-contaminate concurrent tests). The persistence
  regression test sets an explicit temp `LB_STORE_PATH` itself.

## Non-goals

- **Persisted signing key.** Still out of scope (the standalone scope's other half): a restart mints
  a fresh gateway key, so a browser token from before the restart 401s and the user re-logs in. The
  *data* persists; the *session* does not. Persisting the key is a sibling follow-up (it wants the
  same data-dir + a `signing_key::resolve` that reads/writes a key file). Called out so "my data is
  back but I'm logged out" is expected, not a new bug.
- **Store migrations / schema versioning.** SurrealDB is schemaless here and the records are
  append/LWW; a persistent store across app versions is assumed compatible. A real migration story
  (if a record shape ever changes incompatibly) is its own scope.
- **Multi-instance locking.** SurrealKV is a single-process embedded engine; two desktop instances
  pointed at one store dir is undefined. The app is single-instance in practice; a hard lock/guard is
  out of scope (note it, don't build it).
- **Backup / export / reset UI.** No "clear my data" or "export" button here — a user can delete the
  data dir. A settings affordance is a UI follow-up.
- **The thin shell.** Unchanged — it has no in-process node store to persist.

## Intent / approach

The persistence engine already exists and is proven — `Store::open(path)` opens SurrealKV, durable
across restart, used by `make cloud`/`edge` via `LB_STORE_PATH` today. The selector
(`boot.rs::open_store`) already branches on `LB_STORE_PATH` (set → `open`, unset → `memory`). **The
slice is: at the windowed desktop boot, if `LB_STORE_PATH` is unset, resolve the per-user default and
set it — before `NodeHandle::boot`.** One new resolver + one line in `run()`.

1. **Resolve the default path (`store.rs`).** A new `#[cfg(feature = "full")]` helper
   `resolve_store_path()` → `dirs::data_dir().join("lazybones").join("store")` (create the parent
   dir), returning the path string. `dirs` is already a workspace dep (used in
   `host_tools/fs/home.rs`); add it to the shell crate. Windows → `%APPDATA%\lazybones\store`,
   Linux → `~/.local/share/lazybones/store`, macOS → `~/Library/Application Support/lazybones/store`.
2. **Set it at the binary boundary (`desktop.rs::run`).** Before `NodeHandle::boot("acme")`: if
   `LB_STORE_PATH` is unset/empty, `std::env::set_var("LB_STORE_PATH", resolve_store_path())` and log
   the resolved path. `Node::boot` then opens the durable store with zero further change. Env is the
   seam the store selector already reads — the desktop just fills it in.
3. **Nothing else changes.** The seeders, the federation mount, the gateway, the reactors all run as
   they do — they're idempotent, and now they idempotently refresh a *persistent* store instead of
   re-creating an ephemeral one.

**Why the binary boundary, not `NodeHandle::boot`?** `NodeHandle::boot` and `Node::boot` are called
directly by the command tests and the integration tests, which need isolated in-memory stores. Baking
a default path there would make every test share one on-disk store (cross-contamination, flaky
concurrent runs). The windowed `run()` is the one path only the shipped app takes — the correct place
for a shipped-app default. Tests that WANT persistence set `LB_STORE_PATH` to their own temp dir.

## How it fits the core

- **Placement / symmetric nodes (§3.1):** no core change, no `if cloud`, no `if desktop` in a core
  crate. The store engine is selected by `LB_STORE_PATH` exactly as it is for `make cloud`; the
  desktop binary is a thin config layer that fills the env when the shipped app leaves it unset. A
  cloud node sets `LB_STORE_PATH` via its runner; the desktop sets it via `dirs` — same seam, different
  config source.
- **Tenancy / isolation:** unchanged and re-verified. All workspaces live in one on-disk store scoped
  by `use_ns` (the namespace-per-workspace wall, identical to the in-memory engine per
  `store/open.rs`). A persistent store does not widen any workspace boundary.
- **Capabilities / secrets:** unchanged. The mediated DSN in `lb-secrets` (the demo source) now
  persists across restart under the stable `ext:federation` owner — the pre-registration stays
  idempotent (LWW upsert + single-owner secret write), so a re-boot over the existing record is a
  clean no-op, not a collision.
- **Data (SurrealDB):** same tables, same shapes — now durable. No new records.
- **MCP surface:** none. No new verbs, routes, or automatable tasks. Skill doc: **N/A**.
- **Sync / authority:** the desktop node stays solo + authoritative for its own store; persistence
  changes nothing about sync (there is none here).

## Example flow

1. User launches `lazybones-shell` (full). `LB_STORE_PATH` unset → `run()` resolves
   `~/.local/share/lazybones/store`, creates the parent, sets the env, logs
   `full: persistent store at <path>`.
2. `NodeHandle::boot("acme")` → `Node::boot` → `open_store()` sees `LB_STORE_PATH` → `Store::open` →
   durable SurrealKV. Seeders + federation mount run (idempotent).
3. User creates a channel, posts messages, registers a datasource, builds a dashboard.
4. User quits. Relaunches. Same store dir opens — **the channel, messages, datasource, and dashboard
   are all still there.** (They re-log in — the signing key is fresh; that's the recorded non-goal.)
5. A power user runs with `LB_STORE_PATH=/mnt/usb/lb-store ./lazybones-shell` → portable store on the
   stick. Or `LB_STORE_PATH= ./lazybones-shell` (empty) → back to ephemeral for a throwaway session.

## Testing plan

Per `scope/testing/testing-scope.md`, real infra, no mocks (§0) — a real on-disk SurrealKV store,
real gateway, real records:

- **The headline persistence proof (the reported bug):** boot `full` with an explicit temp
  `LB_STORE_PATH`, write a record over the real gateway (e.g. post to a channel / register a
  datasource), drop the node + gateway, **re-boot at the same path**, and assert the record is still
  there. This is the regression that proves "restart loses my work" is gone. Before the persistence:
  a re-boot on `memory()` would find nothing.
- **Isolation still holds on disk (mandatory workspace-isolation):** two workspaces written to the
  same persistent store remain namespace-isolated across a re-open (a `ws-B` read cannot see a `ws-A`
  record) — the on-disk engine keeps the same wall the in-memory one does.
- **Capability-deny (mandatory):** unchanged by persistence, but re-assert over the persistent store
  that an unauthorized caller is still refused (the store engine is not a cap bypass).
- **Idempotent re-seed:** a second boot over an existing store re-runs the seeders + federation
  install cleanly (no error, no duplicate), and pre-existing user data is untouched.
- **In-memory default preserved for tests:** the existing `full_loopback_test` / `full_federation_test`
  / `commands_test` continue to boot in-memory (they don't set `LB_STORE_PATH`) — proving the default
  resolution is at the binary boundary, not in `Node::boot`.
- **Packaging smoke:** extend `smoke-full` (or a new `smoke-full-persist`) to boot the packaged binary
  twice against a fixed store dir and assert a datasource registered in run 1 is listed in run 2.

## Risks & hard problems

- **Wrong seam = flaky tests.** Putting the default in `Node::boot` would make every test share an
  on-disk store. The default MUST be at the windowed `run()` boundary. This is the load-bearing design
  choice; get it wrong and CI goes flaky.
- **Data-dir resolution failure.** `dirs::data_dir()` can return `None` (a headless/unusual
  environment). Fall back to a sensible path (`./store` beside the exe, or the temp dir) and log —
  never panic, never silently drop to in-memory (that would reproduce the data-loss bug quietly).
- **Store dir permissions.** The per-user data dir is writable by definition; but log the resolved
  path loudly so a permission failure (`Store::open` errors) is diagnosable, not a mystery.
- **Signing-key mismatch UX.** The data persists but the key is fresh → the user is logged out on
  restart. Expected (recorded non-goal), but must be communicated so it doesn't read as "persistence
  is broken." The sibling key-persistence scope removes it.
- **Store growth (demo db).** Unrelated to this scope but worth noting: the seeded 956k-reading
  `demo-buildings.db` is a separate sqlite file, not in this store; the SurrealKV store holds only
  platform records and stays small.

## Open questions

- **Persisted signing key** — the obvious next scope (data survives but the session doesn't). Same
  data-dir; a `signing_key::resolve` that reads/writes a key file, injected before `Gateway::new_live`.
  Lean: do it next, it's the other half of "restart just works."
- **Per-workspace vs one store.** One on-disk store holds all workspaces (namespace-scoped), matching
  cloud. Confirmed correct — no per-ws store files. Noting it so it isn't re-litigated.
- **A "reset / clear data" affordance.** Deleting the data dir works today; a settings button is a UI
  follow-up, not this scope.

## Related

- **Extends / reverses:** [`desktop-standalone-backend-scope.md`](desktop-standalone-backend-scope.md)
  — its "Persistent store / persisted signing key" non-goal is the line this scope reverses for the
  data half (the key half stays deferred).
- **The store engine:** `rust/crates/store/src/open.rs` (`Store::open` = SurrealKV, `Store::memory` =
  ephemeral), `rust/crates/host/src/boot.rs::open_store` (the `LB_STORE_PATH` selector — unchanged).
- **The federation bundle:** [`desktop-federation-bundle-scope.md`](desktop-federation-bundle-scope.md)
  — the demo datasource pre-registration whose idempotency this scope re-verifies against a persistent
  store.
- **README:** `§3.1` (symmetric nodes — store by config), `§2` (one datastore, SurrealDB), `§7`
  (workspace isolation, identical on disk).
- **Skill doc:** **N/A** — no new agent-/API-drivable surface (no verb, route, or task added).
