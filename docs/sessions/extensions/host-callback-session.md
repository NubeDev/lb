# Session — the host-callback ABI: a wasm guest calls host MCP tools

- **Scope:** [`scope/extensions/host-callback-scope.md`](../../scope/extensions/host-callback-scope.md)
- **Public:** [`public/extensions/extensions.md`](../../public/extensions/extensions.md)
- **Debug:** [`debugging/extensions/wit-minor-bump-breaks-0_1-guest-linking.md`](../../debugging/extensions/wit-minor-bump-breaks-0_1-guest-linking.md)
- **Stage:** S10 (extensions / ABI slice). S8 shipped; this is the §11.2 **forever-ABI** addition.
- **Status:** shipped. Backend + frontend green. (E2E nav-slot step blocked by an unrelated concurrent
  shell rework — see "E2E" below; the live path is proven by curl + the real-gateway Vitest instead.)
- **Date:** 2026-06-27.

## The ask, restated

A Tier-1 WASM extension was a **one-way box**: the host could call *into* a guest (`tool.call`), but a
guest could only `host.log`. So an extension backend that reads/writes the platform (a producer, a
reactor, a "read a series → derive another" tool) couldn't be written as a guest — it had to live in a
host service. This slice adds the **one** missing primitive: a host-mediated `host.call-tool` so a guest
reaches the SAME MCP tool surface the page bridge reaches (`POST /mcp/call` → `lb_host::call_tool`),
under its **delegated, intersected** authority (`caller ∩ install-grant`), capability- and
workspace-checked on every call. It is the symmetric backend dual of the page bridge, behind the
existing `call_tool` chokepoint — **zero new trust surface**.

## What shipped (end to end)

**1. WIT — one import, minor bump `@0.2.0`** (`sdk/wit/world.wit`).
`world extension`'s `host` interface gained `call-tool(name, input-json) -> result<string, tool-error>`
(reusing `tool`'s error shape via `use tool.{tool-error}`). World MAJOR stays `0`, so the loader's
major-check still accepts `@0.1.0` guests. `lb_sdk::WORLD` bumped to `@0.2.0`.

**2. Identity into the instance** (`crates/runtime/`). `HostState` gained a per-call `call_ctx:
Option<CallContext>`, set BEFORE the guest runs and CLEARED after (`Instance::call_tool_with`) — never
instance-sticky, because the loaded instance is node-global (one instance serves many workspaces, so a
sticky identity would leak across the wall — see the node-global finding). The `host.call-tool` import
is generated **async** and dispatches through the context's bridge.

**3. The narrow seam, not `Arc<Node>`** (open question 5 → resolved). `lb-runtime` defines a
`HostBridge` trait + `CallContext` (`crates/runtime/src/bridge.rs`); `lb-host` implements it
(`crates/host/src/callback.rs`) over `lb_host::call_tool`. So `runtime` stays BELOW `host` in the dep
graph — no layering inversion, no cycle.

**4. Effective principal = `caller ∩ install-grant`** (open question 2 → the intersection).
`build_call_context` (`crates/host/src/tool_call.rs`) reads the ext's `Install.granted` for the
workspace and derives `caller.derive("ext:<id>", granted)` — the S5 delegation primitive, the same
`agent ∩ caller` the agent loop uses. The callback can reach AT MOST what both the caller and the
install allow. (Found + fixed a latent cross-hop widening in `Principal::derive`: a nested derive now
preserves the ORIGINAL caller's constraint, so a re-entrant chain never widens at depth ≥2.)

**5. Dispatch through the chokepoint + depth guard** (open question 1 → fixed constant `MAX_CALL_DEPTH
= 8`). `host.call-tool` → `Bridge::call_tool` → `call_tool_at_depth(...)` → the existing
authorize-then-dispatch. A re-entrant call carries `depth+1`; past the limit it returns
`tool-error::failed("call depth exceeded")`. **Borrow discipline:** a re-entrant call `try_lock`s the
target instance (it would deadlock on its OWN in-flight instance otherwise) and fails fast as "extension
busy" instead of hanging; a top-level call blocks normally. The dispatch resolves a FRESH instance/route
— it never re-borrows the in-flight `&mut Instance`.

**6. ABI back-compat** (the load-bearing promise). A WIT minor bump turned out to break `@0.1.0` guests
at *instantiation* (wasmtime treats a `0.x` minor as semver-incompatible at link time — see the debug
entry). Fixed by linking BOTH `host` versions and falling back to the frozen 0.1.0 export bindings
(`sdk/wit-compat-0_1/`, `crates/runtime/src/compat_v0_1.rs`, `Instance::Bindings{V2|V1}`). `hello` /
`github-bridge` (`@0.1.0`) load and answer on the `@0.2.0` host.

**7. The reference guest uses it** (`extensions/proof-panel/`). New tool `proof.derive`: via
`host.call-tool` it reads the latest `proof.demo` (`series.latest`) and writes `proof.derived = value*2`
(`ingest.write`), returning `{derived, source_seq}` — a guest doing real platform work. Added to the
manifest `[[tools]]` + `[ui] scope`, world `@0.2.0`. Plus `proof.recurse` (self-recursive) for the
depth-guard regression.

**8. UI** (`extensions/proof-panel/ui/`). One hook per verb (`useDerive`), one section
(`DeriveSection`), thin `Panel.tsx` — FILE-LAYOUT respected, frozen contract untouched. "Run derive" →
`proof-panel.proof.derive` → shows the derived value and reads `proof.derived` back over the bridge.

## Open questions — resolved

1. **Re-entrancy depth limit** → a small fixed constant `MAX_CALL_DEPTH = 8`, surfaced as
   `tool-error::failed("call depth exceeded")`. Plus a `try_lock` borrow-discipline so a *self*-re-entry
   fails fast ("extension busy") rather than deadlocking — the depth guard bounds cross-instance chains.
2. **Effective principal = `caller ∩ grant`, or grant alone?** → the **intersection** (both ways),
   matching the agent's `agent ∩ caller`. Proven by the deny-per-direction tests. Also hardened
   `Principal::derive` so nested delegation never widens across hops.
3. **`watch`/motion from a guest** → out of scope here (request/response only); a guest reactor is more
   naturally host-ticked. Recorded as a later scope.
4. **Does `host.log` stay separate?** → yes, kept separate — `log` is fire-and-forget audit, not an
   authorized tool call.
5. **Which handle does `HostState` hold — `Arc<Node>` or a trait?** → a narrow `HostBridge` trait object
   the host supplies, so `lb-runtime` stays below `lb-host` (no dep inversion). Resolved as the lean.

## Tests (real infra, seeded via the real write path — no mocks, CLAUDE §9)

### Backend — `cargo test -p lb-host --test proof_panel_test` (REAL wasm + store + caps)

```
running 16 tests
test ingest_write_then_latest_round_trips_through_the_bridge ... ok
test outbox_status_reads_real_effects_and_denies_without_the_grant ... ok
test ingest_write_is_denied_without_the_grant ... ok
test workflow_surface_is_workspace_isolated ... ok
test inbox_list_then_resolve_round_trips_and_denies_per_verb ... ok
test proof_derive_reads_and_writes_through_the_host_callback ... ok
test callback_denied_when_install_grant_omits_the_verb ... ok
test grant_intersection_denies_the_unapproved_verb_at_the_bridge ... ok
test proof_ping_is_denied_without_the_grant ... ok
test callback_denied_when_caller_lacks_the_verb ... ok
test proof_ping_is_callable_after_publish ... ok
test re_entrancy_is_bounded_never_hangs ... ok
test hello_v0_1_guest_still_loads_alongside_a_v0_2_callback_guest ... ok
test identity_does_not_leak_between_calls_on_the_node_global_instance ... ok
test callback_is_workspace_isolated ... ok
test workspace_isolation_series_and_ping ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.04s
```

The host-callback additions, mapped to the mandatory categories:
- **capability deny — per direction (both):** `callback_denied_when_install_grant_omits_the_verb` (grant
  omits `ingest.write`, caller HOLDS it → denied; nothing written) +
  `callback_denied_when_caller_lacks_the_verb` (install requested it, caller LACKS it → denied).
- **workspace isolation:** `callback_is_workspace_isolated` (ws-B's guest, fully granted in ws-B, sees
  NONE of ws-A's `proof.demo` — the host-set ws walls the callback; ws-A derives fine).
- **happy round-trip:** `proof_derive_reads_and_writes_through_the_host_callback` (reads seeded
  `proof.demo`=21, writes `proof.derived`=42; asserted via a SEPARATE `series.latest`).
- **re-entrancy / depth:** `re_entrancy_is_bounded_never_hangs` (self-recursive `proof.recurse` returns
  promptly — "extension busy" / "call depth exceeded" — never a hang/overflow).
- **ABI compat:** `hello_v0_1_guest_still_loads_alongside_a_v0_2_callback_guest` (a `@0.1.0` guest +
  a `@0.2.0` callback guest coexist on one node).
- **no identity leak / hot-reload:** `identity_does_not_leak_between_calls_on_the_node_global_instance`
  (call A in ws-A, call B in ws-B on the SAME instance — B never inherits A's ws/data).

**Whole workspace:** `cargo test --workspace` → **387 passed, 0 failed**; `cargo fmt --check` clean.

### Frontend — proof-panel unit (`vitest run`, the bridge-interface double, testing §0)

```
 Test Files  2 passed (2)
      Tests  10 passed (10)
```
(+2 new: derive happy round-trip; derive denied → honest error.)

### Frontend — real spawned gateway (`vitest.gateway.config.ts`, REAL node)

```
 ✓ src/features/ext-host/ProofPanel.gateway.test.tsx (11 tests)
   ✓ host-callback: proof.derive reads proof.demo and writes proof.derived = value*2, live
   ✓ host-callback: proof.derive is denied for an out-of-scope page (deny per verb)
 Test Files  1 passed (1)
      Tests  11 passed (11)
```
A new `/_seed/proof_panel` route installs+LOADS the real wasm so its tool is callable over the live
bridge. The page → guest → host → store → page loop runs over a real socket.

### Live node (the e2e-equivalent proof)

In-memory node on :8080, `make publish-ext EXT=proof-panel`, then over the real `POST /mcp/call`:

```
$ curl .../ingest  → proof.demo = 21          ingest=200
$ curl .../mcp/call {tool: proof-panel.proof.derive}
  {"derived":42.0,"source_seq":1}             http=200
```

The wasm guest read `proof.demo` and wrote `proof.derived` via the host callback, live.

### E2E (Playwright) — extended, but BLOCKED by an unrelated concurrent change

`ui/e2e/proof-panel.spec.ts` gained a "Run derive" step (click → `derive-result` shows `Derived N` →
`derived-latest` shows the committed value → no console/hook errors → screenshot). It currently fails at
an EARLIER step (3): the built shell renders no per-extension "Proof Panel" **nav slot**. This is NOT a
host-callback regression — a **concurrent AI session** is mid-refactor of the shell's ext-page mounting
(`ui/src/features/ext-host/{index.ts,useExtensionPages.ts}`, `App.tsx`, `dashboard/*`, all modified
outside this slice). Reverting those (temporarily) restores the slot; the screenshot showed the shell
logs in and renders the core nav but no extension page slot. Per the ground rules I left those files
untouched and noted it. The derive UI itself is exercised live by the real-gateway Vitest above (which
drives the exact `mount(el, ctx, bridge)` seam) + the unit tests + the curl proof — so the behavior is
proven; only the e2e's nav-slot precondition is externally broken. Re-run the e2e once the shell rework
lands.

## Files

- WIT/SDK: `sdk/wit/world.wit` (`@0.2.0` + `host.call-tool`), `sdk/wit-compat-0_1/world.wit` (frozen
  0.1.0), `sdk/src/lib.rs` (`WORLD` bump + test).
- Runtime: `crates/runtime/src/{bridge.rs (new), compat_v0_1.rs (new), bindings.rs, instance.rs,
  engine.rs, lib.rs}`; `Cargo.toml` +`async-trait`.
- Host: `crates/host/src/{callback.rs (new), tool_call.rs, lib.rs}`; `Cargo.toml` +`async-trait`.
- MCP: `crates/mcp/src/call/{mod.rs, dispatch.rs}` (`call_with_ctx` + reentrant lock discipline).
- Auth: `crates/auth/src/principal.rs` (`derive` cross-hop constraint hardening).
- Ext: `extensions/proof-panel/{src/lib.rs, extension.toml}` (`proof.derive` + `proof.recurse`),
  `ui/src/data/useDerive.ts` (new), `ui/src/pages/DeriveSection.tsx` (new), `ui/src/pages/Panel.tsx`.
- Gateway harness: `role/gateway/src/bin/test_gateway_seed.rs` (`/_seed/proof_panel`),
  `role/gateway/src/session/credentials.rs` (+`mcp:proof-panel.proof.derive:call`), `Cargo.toml`
  (+`lb-ext-loader` test-harness dep).
- Tests: `crates/host/tests/proof_panel_test.rs` (+7), `ui/src/features/ext-host/ProofPanel.gateway.test.tsx`
  (+2), `extensions/proof-panel/ui/src/pages/Panel.test.tsx` (+2), `ui/src/test/gateway-session.ts`
  (`seedProofPanel`), `ui/e2e/proof-panel.spec.ts` (+derive step).
