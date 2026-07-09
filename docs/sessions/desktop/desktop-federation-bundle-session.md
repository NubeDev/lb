# Session — bundle the federation sidecar into the `full` desktop build

Scope: [`docs/scope/desktop/desktop-federation-bundle-scope.md`](../../scope/desktop/desktop-federation-bundle-scope.md).

## The ask

The standalone `full` desktop binary boots node + in-process gateway, so everything works
standalone **except datasources**: a user could `datasource.add` over the loopback gateway (200,
record persists), but `datasource.test` / `federation.query` returned an opaque "denied". Cause: the
federation native sidecar (which serves those verbs) was deliberately not shipped in `full`, so
there was no federation `Install` record → `enforce_endpoint` had no grant to pass and refused. This
session bundles the sidecar into `full` and auto-installs it at boot with a sqlite-loopback grant,
plus pre-registers the shipped `demo-buildings.db`, so a double-clicked binary registers **and**
queries a sqlite source end to end.

## Decision: sqlite-only for the desktop default (postgres deferred)

We considered bundling postgres too. Left it out for v1 (confirmed with the user):

- Sqlite is the **default** feature set of the `federation` crate — `rusqlite` is `bundled` (it
  compiles its own sqlite3, no system C dep, **no TLS/OpenSSL**). Postgres is `--features postgres`
  and pulls vendored OpenSSL + a second TLS toolchain. Sqlite-only keeps the desktop cross-compile
  simple and the binary smaller.
- The reported gap was the sqlite `demo-buildings.db`; sqlite-only closes exactly that.
- Nothing structural blocks postgres later — the install grant takes endpoints as opaque data, and
  the root Makefile already cross-compiles `federation --features postgres` for Windows
  (`make docker-build TARGET=windows-x86_64 PKG=federation FEATURES=postgres`). Adding it is a
  build-flag + wider-grant change, recorded as the scope's open question.

**Scope-doc correction:** the scope said `cargo build -p federation --features sqlite` — there is no
`sqlite` feature (sqlite is default-on). Fixed the scope's build step + the cross-compile risk note.

## What shipped

**Shared install helper (CLAUDE §10-safe reuse, not copy-paste).**
[`rust/crates/host/src/federation/install.rs`](../../../rust/crates/host/src/federation/install.rs)
— `install_federation(node, launcher, ws, manifest_toml, install_dir, approved, seed, ts)`. It owns
the security-sensitive path once: the admin bootstrap principal, the `requested ∩ approved` grant via
`install_native`, and the optional seed-source `datasource_add`. It names **no** extension — the
manifest, grant, and seed are opaque data the *binary* supplies (§3.1 permits the role-aware binary;
the core stays extension-agnostic). Re-exported as `lb_host::install_federation` (+ `SeedSource`,
`FederationInstalled`).

Rejected alternative: copy-paste the mount into `full.rs`. Two copies of a grant/token path drift —
exactly the bug class that produced this scope (`full.rs` had already dropped
`ensure_builtin_authz_roles` from its `node/main.rs` twin, see the debugging entry).

**`node/src/federation.rs` refactored onto the helper** — dropped its duplicated `admin_principal` +
inline `install_native`/`datasource_add`; keeps env parsing, endpoint→grant mapping, and logging.
Behavior unchanged (host `federation_sqlite_test` still green).

**Desktop boot step.**
[`ui/src-tauri/src/federation.rs`](../../../ui/src-tauri/src/federation.rs) — `mount_federation`,
`full`-only. Resolves the sidecar (`federation`/`federation.exe`) + `demo-buildings.db` beside the
exe (`LB_FEDERATION_DIR` override), approves **only** `net:tls:127.0.0.1:0:connect` +
`secret:federation/*:get`, pre-registers the demo db (skippable via `LB_DESKTOP_NO_DEMO_SOURCE=1`).
Best-effort + LOUD (a failure prints and the app still opens — so it can't silently reproduce the
"why denied" confusion). Called from `boot_full` **after** `Gateway::new_live` installs the signing
key (so the child token verifies), mirroring `node/main.rs` ordering.

**Packaging.** `desktop/docker/build.sh` + `build-windows.sh` build the sqlite-only sidecar in `full`
mode (`cargo build -p federation --release` / `cargo xwin build … --target x86_64-pc-windows-msvc`);
the desktop `Makefile` copies `federation(.exe)` beside the shell in `linux-full` / `windows-full`.
`smoke-full` extended: mounts the whole full dir and asserts a token'd `datasource.test` on
`demo-buildings` returns `"ok":true` (not just login).

## Tests (real infra, no mocks — rule 9)

[`ui/src-tauri/tests/full_federation_test.rs`](../../../ui/src-tauri/tests/full_federation_test.rs)
— builds the REAL sidecar, stages a bundle dir (binary + a real seeded `demo-buildings.db`), boots
`full`, and over the loopback gateway proves:

- **The regression:** `datasource.test` on the pre-registered demo → `ok:true` (was the opaque
  "denied"); `federation.query` returns the seeded rows. Queryable, not just present.
- **DSN redaction (§6.7):** `datasource.list` never contains the db path (only the secret ref).
- **Capability-deny (mandatory):** a `postgres` source at `db.example:5432` registers, but its
  `datasource.test` is refused pre-connect — the desktop grant approves only `127.0.0.1:0`. The deny
  wall holds *with the sidecar present*.
- **Workspace-isolation (mandatory):** a source registered in `acme` only (`acme-only`) is not
  resolvable from a second node booted for ws `other`. (Note: each boot independently seeds its own
  `demo-buildings`, so isolation is proven on a NON-seeded source — an early version of the test got
  a false green by querying `demo-buildings` from `other`, which `other` had seeded itself.)

Green:
```
cargo test --features full --test full_federation_test   # 1 passed
cargo test --features full --test full_loopback_test     # 3 passed (mount skips cleanly, no sidecar dir)
cargo test -p lb-host --test federation_sqlite_test      # 1 passed (helper refactor no regression)
```

## Docs

- Public: [`doc-site/content/public/desktop/desktop.md`](../../../doc-site/content/public/desktop/desktop.md)
  already carried the accurate "federation sidecar bundled + auto-installed" paragraph (created with
  the scope) — no change needed.
- No new debugging entry: nothing broke that wasn't caught by a failing-then-passing test. The
  motivating bug (`full-seed-user-missing-admin-caps.md`) already has its entry; this session is its
  structural follow-up. The one in-session false-green (isolation test) is recorded above, not as a
  debug entry (it was a test-premise error, not a product defect).

## Open (from the scope, unchanged)

- **postgres-in-desktop** — sqlite-only for v1; a runtime admin action to widen the grant is its own
  scope.
- **Persistence** — `full` still boots in-memory per launch; the install + seed are idempotent and
  re-run every boot (orthogonal follow-up).
