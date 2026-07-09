# Desktop scope — federation in the standalone `full` build (bundled sidecar)

Status: **SHIPPED** (sqlite-only; postgres deferred — see open questions). Session:
[`../../sessions/desktop/desktop-federation-bundle-session.md`](../../sessions/desktop/desktop-federation-bundle-session.md).
Public: `public/desktop/desktop.md`.

The standalone `full` desktop binary (`desktop-standalone-backend-scope.md`) boots the node +
the in-process gateway, so login/MCP/SSE/agents/flows/rules/insights all work with no external
node. **Datasources are the one hole.** A user can *register* a source over the loopback gateway
(`datasource.add` → 200, the record persists), but the moment they **test** or **query** it the
call is refused — `datasource.test` / `federation.query` route to the **federation native
sidecar**, which the `full` boot deliberately does not ship, and the pre-connect `net:*` check
(`federation/src/net.rs::enforce_endpoint`) has no federation install-grant to pass against, so it
denies opaquely (a 403 the UI shows as "denied"). This scope closes that hole for the packaged
app: **bundle the federation sidecar into the `full` build and auto-install it at boot** with a
`net:*` grant for local/loopback sources — so a double-clicked `.exe` can register **and** query a
sqlite datasource (the shipped `demo-buildings.db`) end to end, no `make dev`.

## Goals

- The `full` desktop binary (`make windows-full` / `make linux-full`) ships the **federation
  sidecar binary** alongside the shell, and **auto-installs + supervises** it at boot in the seeded
  workspace (`acme`), mirroring `node/src/federation.rs::mount` — but driven by the desktop boot,
  not `LB_FEDERATION_ENDPOINTS`.
- Out of the box, `datasource.test` and `federation.query` against a **local sqlite** source
  succeed: the federation install-grant carries the `net:*` the local-loopback endpoint convention
  (`127.0.0.1:0`, sqlite has no network endpoint) needs, plus `secret:federation/*:get`.
- The shipped **`demo-buildings.db`** is **pre-registered** as a working datasource on first boot
  (the desktop analogue of the `LB_FEDERATION_SEED_*` demo seed), so the Datasources page shows a
  green, queryable entry with zero setup — the "seeded data" the packaging already copies becomes
  *reachable*, not just present.
- The **thin** shell and the browser/`make dev` paths are unchanged. This is additive: a new
  `#[cfg(feature = "full")]` boot step + a packaging step that copies the sidecar binary.
- Same shape on both OSes (§3.1): Linux and Windows `full` both bundle the sidecar; the only delta
  is the sidecar's own build target (a second cross-compile for `x86_64-pc-windows-msvc`).

## Non-goals

- **`control-engine`** (the other native sidecar). Same bundling shape, but out of scope here —
  datasources are the reported gap. Recorded as a sibling follow-up.
- **postgres / timescale sources in the desktop default.** The bundled grant approves only the
  **local sqlite** convention out of the box (no arbitrary outbound network from a shipped desktop
  app without an explicit admin approval). Registering a postgres source still works, but its
  endpoint must be approved — a **runtime admin action** (widen the federation install grant), not a
  baked default. The v1 desktop default is sqlite-only; postgres-in-desktop is an open question.
- **Persistent store / persisted federation install.** `full` still boots an in-memory node per
  launch (`desktop-standalone-backend-scope.md` non-goal); the install + seed are idempotent and
  re-run every boot. Persistence is the same orthogonal follow-up already recorded.
- **A federation UI beyond what exists.** The Datasources page, Add form, and test/query flows are
  already built; this scope makes them *work* in `full`, it does not change them.
- **Shipping the sidecar for the `hello`/wasm demo or any other extension.** Only the federation
  native sidecar is bundled; the generic "auto-install every published extension from the durable
  cache" path stays a no-op on a fresh store.

## Intent / approach

The install machinery already exists and is proven — `node/src/federation.rs` installs +
supervises the sidecar for `make dev` today via `lb_host::install_native`. The `full` desktop boot
already mirrors `node/main.rs`'s seeders in `ui/src-tauri/src/full.rs::boot_full`. **The slice is
one more boot step in `boot_full` plus one packaging step** — not new core surface.

1. **Bundle the binary (packaging).** `desktop/docker/build.sh` + `build-windows.sh` build the
   `federation` sidecar (`cargo build -p federation --release [--target …]` — **sqlite is the
   default feature set**, `rusqlite` is bundled so there is no system-sqlite C dep; there is no
   `--features sqlite` flag, and `--features postgres` is deliberately *omitted* for the desktop
   default per the postgres non-goal) and
   the Makefile copies it next to the shell into `build/<os>-full/` (e.g. `federation` /
   `federation.exe`). The shell resolves it at boot relative to its own exe dir (a new
   `LB_FEDERATION_DIR` default = `dirname(current_exe)`), overridable by env.
2. **Auto-install at boot (`full.rs`).** A new `#[cfg(feature = "full")]` `mount_federation(node,
   ws)` step in `boot_full`, called after the gateway installs the signing key (the same ordering
   `node/main.rs` uses so the sidecar's minted `LB_EXT_TOKEN` verifies). It reuses
   `node/src/federation.rs`'s logic — the admin bootstrap principal, the compiled-in manifest
   (`include_str!` the federation `extension.toml`), `install_native` with the approved grant — but
   the approved endpoints are the **desktop default** (`net:tls:127.0.0.1:0:connect` for the sqlite
   convention + `secret:federation/*:get`), not read from `LB_FEDERATION_ENDPOINTS`. Best-effort:
   an install failure prints and continues (the app still opens; datasources just stay red), exactly
   like the other `full` seeders.
3. **Pre-register the demo source.** After install, `datasource_add(node, admin, ws, "demo-buildings",
   "sqlite", "127.0.0.1:0", dsn = <path to the bundled demo-buildings.db>, …)`. The DSN is the
   node-local file path resolved relative to the exe dir (the packaging copies `demo-buildings.db`
   beside the binary already). Idempotent (LWW upsert), best-effort.

**Reuse, don't fork.** `node/src/federation.rs` and the desktop `mount_federation` share the same
install logic. Lift the reusable core (`admin_principal`, the manifest, the `install_native` call,
the seed-source add) into a place both binaries call — candidate: a small `lb_host` helper
`install_federation(node, ws, approved, seed)` so neither binary re-implements the install, and the
CLAUDE §10 rule holds (the *binary* decides to install federation; the *core* stays
extension-agnostic — it installs an opaque manifest+grant, never "the federation extension by id").
Decide the exact seam in the build (open question). The alternative — copy-paste the mount into
`full.rs` — was rejected: two copies of a security-sensitive install (grant computation, token
mint) drift, and the drift is exactly the class of bug that produced this scope (`full.rs` had
already dropped `ensure_builtin_authz_roles` from its `node/main.rs` twin).

## How it fits the core

- **Tenancy / isolation:** the sidecar is installed in the seeded workspace (`acme`) only; the
  install record, the `net:*` grant, and the datasource record are all workspace-scoped keys. A
  second workspace on the same desktop node gets no federation until it too installs. Isolation is
  the existing `install_native` / datasource behavior — unchanged, re-tested here for the desktop
  boot.
- **Capabilities:** the boot admin principal holds exactly `mcp:native.install:call`,
  `mcp:datasource.add:call`, `secret:federation/*:write` (the `node/main.rs` set). The **granted**
  sidecar caps are `requested ∩ admin_approved` computed in `install_native` — the desktop approves
  `net:tls:127.0.0.1:0:connect` + `secret:federation/*:get`, nothing wider. The **deny path** is the
  headline: a source whose endpoint is *not* in the approved grant is refused by
  `enforce_endpoint` — opaque, even with the sidecar present. That is the exact wall this scope
  makes *pass* for the approved local endpoint while keeping it *closed* for unapproved ones.
- **Placement:** local-only concern (the desktop `full` binary). No cloud branch — a cloud node
  installs federation via `node/src/federation.rs` + env as it does today. Symmetric: both binaries
  call the same install; the *config* (approved endpoints, seed source) differs, never the code.
- **MCP surface:** **no new MCP tools.** The datasource verbs (`datasource.add`/`list`/`remove`/
  `test`, `federation.query`) already exist; this scope makes the sidecar they depend on present.
  API-shape (§6.1): N/A — no new verbs. CRUD/get/list/watch/batch are all unchanged.
- **Data (SurrealDB):** the `Install` record (native sidecar), the `datasource:{ws}:{name}` record,
  and the mediated DSN in `lb-secrets` (ref only in the record) — all existing shapes, written by
  existing host code. No new tables.
- **Bus (Zenoh):** N/A directly. The sidecar's `native.call` transport rides the existing
  cross-node routed hop; no new subjects.
- **Sync / authority:** node-local, non-authoritative (in-memory `full` node). Re-installed each
  boot; no offline/sync semantics beyond the existing best-effort seeders.
- **Secrets:** the demo source's DSN (the sqlite **file path**) is mediated into `lb-secrets` under
  the stable `ext:federation` owner via the existing `store_dsn` path (`datasource.add`). It never
  lands in the datasource record, a log, or a response (§6.7). A sqlite path is not sensitive, but
  it flows the same mediated path as a postgres password — no special-case.

## Example flow

1. User double-clicks `lazybones-shell.exe` (the `full` Windows build). Packaging placed
   `federation.exe` and `demo-buildings.db` beside it.
2. `boot_full` runs: seeds identity + roles (`ada` → workspace-admin), mounts the loopback gateway,
   installs the signing key, then **`mount_federation(node, "acme")`** fires.
3. `mount_federation` resolves `federation.exe` from the exe dir, `install_native`s it in `acme`
   with the approved grant `[net:tls:127.0.0.1:0:connect, secret:federation/*:get]`, and supervises
   the child. Console: `federation: installed sidecar in 'acme' (tools=[…], approved=[127.0.0.1:0])`.
4. It then pre-registers `demo-buildings` (sqlite, dsn = the bundled db path) via `datasource_add`.
5. The user logs in (`user:ada` / `acme`), opens **Datasources**: `demo-buildings` is listed. They
   click **Test** → `POST /datasources/demo-buildings/test` → host `datasource.test` →
   `enforce_endpoint` passes (`127.0.0.1:0` is approved) → the sidecar opens the sqlite file →
   **green**.
6. They build a panel/query over it → `federation.query` → the same sidecar returns rows from
   `demo-buildings.db`. The seeded 956k readings are now *queryable*, not just present on disk.
7. They try to register a **postgres** source at `db.example:5432` → `enforce_endpoint` refuses
   (not in the approved grant) → an honest red, until an admin widens the federation install grant
   (the open question / documented runtime step).

## Testing plan

Per `scope/testing/testing-scope.md`, real infra, no mocks (§0) — the sidecar is a **real** spawned
child, the store/gateway are real, seeded with a real sqlite file:

- **Capability-deny (mandatory):** a source whose endpoint is NOT in the desktop-approved grant is
  refused by `enforce_endpoint` — opaque, sidecar present. Assert the approved sqlite endpoint
  passes and an unapproved `host:port` denies (the headline wall, both directions).
- **Workspace-isolation (mandatory):** the federation install + demo source live in `acme` only; a
  second workspace on the same node sees no federation install and cannot query `demo-buildings`.
- **The end-to-end that was broken:** boot the `full` binary (the existing `full_loopback_test`
  harness pattern), install federation, register the bundled sqlite db, and drive `datasource.test`
  → green **and** `federation.query` → rows. This is the regression that proves the "denied" is
  gone. Gate behind the sidecar binary being built (like `federation_test.rs`), not skipped silently.
- **Hot-reload / supervision:** N/A new — the existing `install_native` + supervision tests cover
  restart; add only that the desktop boot's install is idempotent (a second boot re-installs cleanly
  over the in-memory store).
- **Packaging smoke:** `make windows-full` / `linux-full` emit `federation(.exe)` beside the shell;
  the Linux `smoke-full` target is extended to assert the sidecar installs (grep the boot log for
  `federation: installed`) and a `datasource.test` over the loopback gateway returns green.

## Risks & hard problems

- **Second cross-compile.** The Windows `full` build must now cross-compile the `federation` sidecar
  to `x86_64-pc-windows-msvc` too. Sqlite-only (the desktop default) links **`rusqlite` bundled** —
  it compiles its own sqlite3 from source, no system C dep and no TLS/OpenSSL (that only comes in
  with `--features postgres`, which the desktop default omits). The root Makefile already
  cross-compiles this exact sidecar (`make docker-build TARGET=windows-x86_64 PKG=federation`), so
  the toolchain is proven; the desktop packaging reuses it rather than routing the sidecar through
  cargo-xwin.
- **Binary size.** Bundling a second binary grows the package (the shell is already ~145 MB). The
  federation sidecar is small by comparison; note it, don't optimize prematurely.
- **The reuse seam vs CLAUDE §10.** Lifting the install into a shared helper must keep the core
  extension-agnostic: the helper installs an **opaque** manifest + grant, and the *binary* is what
  names "federation" (via `include_str!` of its manifest + the approved endpoints). If the shared
  helper ends up with `if ext == "federation"`, that is the leak — the helper must take the manifest
  and grant as data. Get this boundary right or the copy-paste alternative is honestly better.
- **Path resolution.** `dirname(current_exe)` for the sidecar + the demo db must be robust on both
  OSes (Windows path separators, a `.exe` suffix, a moved app dir). The existing `LB_FEDERATION_DIR`
  override is the escape hatch; the desktop default is the new code.
- **Best-effort vs silent failure.** An install/seed failure prints and continues (the app opens).
  That is the right posture, but it must be *loud in the console* — a silent skip would reproduce
  the current "why is my datasource denied" confusion. The boot log lines are load-bearing.

## Open questions

- **Where does the shared install helper live?** ✅ **RESOLVED** — `lb_host::install_federation`
  ([`federation/install.rs`](../../../rust/crates/host/src/federation/install.rs)), taking
  `manifest_toml` + `approved` grant + optional `SeedSource` as **opaque data** (keeps §10); both
  `node/src/federation.rs` and the desktop `ui/src-tauri/src/federation.rs` call it, each supplying
  the federation-specific values. No `if ext == "federation"` in the helper.
- **postgres-in-desktop.** Do we ship a way for the desktop user to approve a postgres endpoint at
  runtime (widen the federation install grant via an admin action in the UI), or is desktop
  sqlite-only for v1? Lean: sqlite-only v1; the runtime-widen is its own scope.
- **Demo source DSN = absolute path.** ✅ **RESOLVED** — the boot resolves the demo db beside the
  exe and hands the sidecar the absolute file path; `SqliteSource::connect` strips an optional
  `file:` prefix and opens the path directly (no `sqlite://` scheme). The e2e test proves the
  separate sidecar process opens the same absolute path and returns rows.
- **Should the demo source be opt-out?** An env (`LB_DESKTOP_NO_DEMO_SOURCE=1`) to skip the
  pre-registration for a user who wants a clean workspace. Lean: yes, cheap, mirrors the seed
  toggles.

## Related

- **Extends:** [`desktop-standalone-backend-scope.md`](desktop-standalone-backend-scope.md) (the
  `full` mode this fills the datasource hole in) — its "Native sidecars" non-goal is the line this
  scope reverses for federation.
- **The install machinery:** [`../extensions/native-tier-scope.md`](../extensions/native-tier-scope.md),
  [`../extensions/lifecycle-management-scope.md`](../extensions/lifecycle-management-scope.md) —
  `install_native`, supervision, the `LB_EXT_TOKEN` mint.
- **The federation feature itself:** [`../datasources/datasources-scope.md`](../datasources/datasources-scope.md),
  [`../datasources/sqlite-datasource-demo-scope.md`](../datasources/sqlite-datasource-demo-scope.md)
  (the sqlite kind + the demo dataset), and the `net:*` pre-connect wall.
- **Packaging:** [`desktop-build-container-scope.md`](desktop-build-container-scope.md) (the build
  container the second cross-compile lands in), [`desktop-packaging-scope.md`](desktop-packaging-scope.md).
- **The bug that motivated it:** `debugging/desktop/full-seed-user-missing-admin-caps.md` (the
  admin-caps half) and the datasource `dsn`-required-422 fix shipped alongside — this scope is the
  structural follow-up to those point fixes.
- **README:** `§6.7` (datasource DSN redaction), `§6` (the native tier), `§3.1` (symmetric nodes).
- **Skill doc:** **N/A** — this scope adds no new agent-/API-drivable surface. The datasource MCP
  verbs (`datasource.*`, `federation.query`) already exist and are covered by the datasources
  scope's skill/docs; making the sidecar present in one build mode changes no verb, route, or
  automatable task. (If a "set up a desktop datasource" runbook is later wanted, it belongs to the
  datasources topic, not here.)
