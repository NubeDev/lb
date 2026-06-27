# Extensions — lifecycle management over the gateway (session)

- Date: 2026-06-27
- Scope: ../../scope/extensions/lifecycle-management-scope.md
- Stage: S9+ — slice 3 of 4 (admin-CRUD/lifecycle/console). Builds on slices 1–2.
- Status: done (host lifecycle surface + boot reconciler + registry publish + gateway routes + http.ts + fake)

## Goal
Close the extension-lifecycle matrix and **expose it over the gateway** — the biggest real gap: the
host had the Tier-1/Tier-2/registry mechanisms but only the Tauri shell reached them, so a browser
threw `unknown command`. Add a uniform `ext.*` surface (list · enable · disable · uninstall) that
dispatches by tier, a **boot reconciler** honoring `enabled ∧ started` (so a disabled extension does
not silently return after a restart), and the registry **publish** producer (verify-before-store).

## What changed
**`lb-assets` `Install` extended** — gained `tier: Tier{Wasm,Native}`, `enabled: bool` (durable
intent), and a `kind` discriminant (all `#[serde(default)]` so pre-existing records load). New raw
verbs: `list_installs` (union both tiers via the kind filter) + `delete_install` (tombstone-upsert;
`read_install` now reads a tombstone as absent). `Install::with_tier` builder.

**New host `ext` lifecycle surface** (`crates/host/src/ext/`) — one service, dispatch by `Install.tier`,
no `if tier` in callers:
- `row.rs` — `ExtRow{ext,version,tier,enabled,running,health,restart_count}`, the uniform list row.
- `list.rs` — `ext.list` (`mcp:ext.list:call`): unions installs, joins the native `SidecarMap`
  (running/restart_count) for native rows; wasm `running == enabled`.
- `enable.rs` — `ext.enable`/`ext.disable` (`mcp:ext.disable:call`): flip the durable `enabled` flag;
  **disable also stops a running native child** (the load-bearing distinct-from-stop intent).
- `uninstall.rs` — `ext.uninstall` (`mcp:ext.uninstall:call`): stop native child + tombstone the
  install, one op, idempotent, workspace-first.
- `reconcile.rs` — the **boot reconciler**: returns a `ReconcilePlan` (start enabled+not-running, skip
  disabled/already-running) the node acts on; testable headlessly, symmetric (the resolved open Q).
- `tool.rs` — `call_ext_tool` MCP bridge.

**Native refactor** — extracted `stop_sidecar_internal` (idempotent, no cap) for the ext cascades;
`stop_native` keeps its `NotRunning`-on-missing operator contract (preserved a native-isolation test).

**Registry publish** (`role/registry-host/`) — `ArtifactStore` gained the publisher **allow-list** +
`publish()` that **`verify_artifact`s BEFORE storing** (authenticity before authority): tampered /
unsigned / foreign-key uploads are rejected and nothing is stored; idempotent on `(ext,version)`. New
`POST /artifacts` route (`204` ok / `403` reject). `with_trusted` constructor; `get` made `pub`.

**Gateway** (`role/gateway/`) — `ext.rs` routes: `GET /extensions`, `POST /extensions/{ext}/enable`,
`.../disable`, `DELETE /extensions/{ext}` — each re-checks the cap server-side. Mounted in `server.rs`.

**UI transport** — `http.ts` gained `ext_list`/`ext_enable`/`ext_disable`/`ext_uninstall` (no
`unknown command`); `ext.fake.ts` mirrors the routes + the `ExtRow` shape 1:1 for Vitest.

## Decisions (open questions resolved)
- **`enabled` flag on the wasm `Install` + tier on every row**, unified by the lifecycle surface (one
  uniform `ext.list` row across tiers).
- **`uninstall` evicts** (tombstones the install; cache GC stays a registry follow-up).
- **Upload reuses the existing `Artifact`** (digest binds manifest+wasm) — verify unchanged; publish is
  just its producer transport, with verify-before-store.
- **Boot reconciler is a host `reconcile` verb the node calls on start** (returns a plan; the node owns
  the actual native respawn via the `Launcher` it holds) — testable, symmetric.
- **`ext.list` is one verb unioning both tiers** from `Install` records, `tier` on each row.

## Tests (green)
Host — `cargo test -p lb-host --test ext_lifecycle_test` (4):
```
denies_each_lifecycle_verb_without_its_cap ... ok            (capability DENY, per verb)
ws_b_cannot_list_or_uninstall_ws_a_extensions ... ok         (two-workspace ISOLATION, store+runtime)
list_unions_both_tiers_and_reflects_enable_disable_uninstall ... ok
boot_reconcile_honors_disable_intent ... ok                  (a DISABLED ext is NOT in the start plan)
test result: ok. 4 passed; 0 failed
```
Registry publish — `cargo test -p lb-role-registry-host --test publish_test` (4):
```
signed_artifact_publishes_then_serves_and_is_idempotent ... ok
tampered_artifact_is_rejected_before_storing ... ok          (verify BEFORE store)
foreign_key_artifact_is_rejected_before_storing ... ok
unsigned_artifact_is_rejected_before_storing ... ok
test result: ok. 4 passed; 0 failed
```
Gateway — `admin_routes_test::ext_routes_are_reachable_for_an_admin_and_deny_a_non_admin ... ok`
(ext_list reachable over the gateway; non-admin denied server-side on every ext route).

UI — `tsc` clean; `vitest` 40 passed (no regression). `cargo build --workspace` green; `cargo fmt`
clean; native + registry + assets suites green (regression caught + fixed: `stop_native`'s
`NotRunning` contract preserved while adding the idempotent internal — see below).

Pre-existing unrelated failures (NOT touched): `github_bridge_normalize_test` needs a prebuilt wasm
guest absent in this checkout.

## Bug found + fixed (this session)
Refactoring `stop_native` to delegate to the new idempotent `stop_sidecar_internal` silently changed
its contract (missing sidecar: `NotRunning` → no-op success), breaking `native_isolation_test`. Fixed
by keeping the `NotRunning` guard in the operator `stop_native` and using the idempotent variant only
for the `ext.disable`/`uninstall` cascades. (Caught by running the full native suite.)

## Follow-ups
- Slice 4 (UI): `features/extensions` console consuming `ext_*` + the registry catalog; retire
  `RegistryView`/`NativeView`; one `ConfirmDestructive` for uninstall.
- A host `registry.publish` verb riding the outbox `Target` (the durable producer side) — the
  registry-host `POST /artifacts` + verify-before-store ships now; the outbox-backed host verb is the
  remaining durability wrapper.
- The node wires `reconcile`'s plan to real native respawn on boot (the host returns the plan today).

## Related
- scope: ../../scope/extensions/lifecycle-management-scope.md
- builds on: ../auth-caps/authz-grants-session.md, ../auth-caps/admin-crud-session.md
- consumed by: ../../scope/frontend/admin-console-scope.md (slice 4)
