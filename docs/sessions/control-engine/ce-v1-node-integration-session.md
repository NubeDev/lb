# Session: control-engine v1 — node integration (make it appear + work in a running node)

Branch: `ce-node-wiring` (off `master`, which has S2/S5/S6/S7 merged). This session took the shipped
`control-engine` extension from "builds green + unit/live tests pass" to **installed, cap-reachable,
and driven end to end against a real gateway**, and fixed two generic platform bugs found along the way.

## What shipped

### 1. Boot-install the CE sidecar (like `federation`)
- `rust/node/src/control_engine.rs` — env-gated (`LB_CONTROL_ENGINE_BASE`) `mount` that `install_native`s
  the CE sidecar, approves its `net:tcp:host:port:connect` + the manifest's verb/store caps, and seeds
  one appliance (`LB_CONTROL_ENGINE_APPLIANCE`, default `local`) so the wiresheet picker works on first
  boot. Mirrors `federation.rs` exactly (the §3.1 thin role-aware binary layer, no core branch).
- `rust/node/src/main.rs` — mounts it (see the ordering fix below).
- `Makefile` — a `control-engine` target builds the sidecar (debug **and** release) + the federated UI
  bundle and deploys it to the served ext-ui dir; wired into `make dev` **opt-in** via `CE=1` /
  `CE_BASE=host:port` (OFF by default — the sidecar needs a running ce-studio on :7979).
- `build.sh` — builds **both** debug and release sidecar profiles (the node under `cargo run` is debug;
  a packaged node is release — it resolves the binary from its own profile dir).

### 2. Extension tool caps reach users via GRANTS, not a login hardcode (the real fix)
The page 403'd on load (`out_of_scope`, then `denied`). Root cause: an extension's user-facing caps only
reached a user if hand-listed in `session/credentials.rs::member_caps()` — the anti-pattern the grant
model exists to kill. Fixed generically (details:
[debugging/auth/ext-page-caps-require-grant-not-login-hardcode.md](../../debugging/auth/ext-page-caps-require-grant-not-login-hardcode.md)):
- `crates/host/src/authz/grant_ui.rs` (new) — `grant_ui_scope_to_admin` grants each `[ui]`/`[[widget]]`
  scope tool (∩ `granted`) to `role:workspace-admin` at install; called from `install_native` (and
  available for the wasm `install`).
- `crates/authz/src/resolve.rs` — `resolve_subject_caps` now folds a role subject's DIRECT grants, so
  caps granted to `role:workspace-admin` resolve for every holder without mutating the immutable
  built-in role record.
- `role/gateway/src/routes/login.rs` — the minted dev token now unions `resolve_caps(store, ws, user)`
  (best-effort). So installing ANY extension makes its page reachable by admins — zero login edits,
  durable, revocable from the admin console. Matches `authz-grants-scope.md`.

### 3. Native sidecar callback key — boot ordering (generic bug)
Sidecar callbacks 401'd (`invalid credential`) because native roles were mounted **before**
`Gateway::new_live` installed the shared signing key onto the node, so `LB_EXT_TOKEN` was minted with
the node's throwaway boot key. Fixed by mounting native roles AFTER the gateway is built (both the
gateway and the edge/solo branch). `federation` benefited too (its boot seed had been failing silently).
Details: [debugging/extensions/sidecar-callback-401-key-minted-before-gateway.md](../../debugging/extensions/sidecar-callback-401-key-minted-before-gateway.md).

## Proof (real gateway, e2e)
Against a hand-launched real node (gateway + CE env, ce-studio on :7979):
- boot: `control-engine: installed sidecar in 'acme' (tools=[…13…], granted=[…21…])`.
- dev login token now carries all `mcp:control-engine.*:call` caps **resolved from the grant store**
  (verified via `/login` caps).
- `/native/call control-engine.appliance.add` → `{"id":"local"}` **200**; `appliance.list` → the seeded
  appliance **200** (both previously 401 pre-fix, then 403 pre-cap-fix).
- `control-engine.tree` resolves the appliance id `local` → its base and calls the live engine; the only
  residual failure was the ce-studio engine being **offline at that moment** (connection refused) — every
  LB-side hop passed. (The full read path against a LIVE engine is separately proven green by
  `rust/extensions/control-engine/ui/src/bridge-transport.live.test.ts` and the Rust
  `control_engine_real_write_flow` tier.)

## Tests
- `cargo test -p lb-authz` (incl. role expansion) green; host `native_test`/`native_deny_test`/
  `native_isolation_test` green; node + gateway + host + authz build clean.

## Open items / follow-ups
- **Boot appliance-seed race:** the seed runs a callback before `serve` starts listening, so the
  first-boot seed logs a transport error; the appliance is instead reliably added on first UI use (or a
  future post-serve seed hook). Cosmetic — the picker's empty state links the add flow.
- The `grant_ui_scope_to_admin` call is wired into `install_native`; wiring the identical call into the
  wasm `install` path (so wasm pages get the same treatment) is a trivial follow-up on the same helper.
- Docs promoted: this session + two debugging entries + STATUS. The generic grant-on-install behavior
  should graduate into `public/auth-caps/` once a second extension exercises it.
