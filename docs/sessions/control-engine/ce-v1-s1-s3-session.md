# Session — control-engine v1: slices S1 + S3 (built in parallel)

Status: **done, green.** Both slices implemented, tested, and committed on their own
branches (no push). S1 lives upstream in `NubeIO/ce-wiresheet`; S3 lives in this repo.

Parent scope: [`control-engine-scope.md`](../../../rust/extensions/control-engine/docs/control-engine-scope.md).
Slice docs (acceptance criteria):
[S1](../../../rust/extensions/control-engine/docs/slice-1-wiresheet-transport-seam.md) ·
[S3](../../../rust/extensions/control-engine/docs/slice-3-sidecar-local-mode.md).
Prior (docs-only) scoping session: [`control-engine-slices-session.md`](control-engine-slices-session.md).

The two slices are independent (S3 depends on nothing in S1/S2) and were built in
parallel: S1 as a TypeScript refactor in the wiresheet repo, S3 as a fresh Rust crate here.

---

## S1 — the `EngineTransport` seam (`ce-wiresheet`, branch `lb-transport`)

**Repo:** `~/code/ce/ce-wiresheet` — `NubeIO/ce-wiresheet` (our org). Branch `lb-transport`
cut from `origin/main @ 818f0f8` (which includes merged PR #2, the editor decomposition).
**Not pushed** — left local for review; written to be mergeable to `main` (pure refactor +
one optional prop; standalone behavior byte-identical).

### What shipped

- **`lib/transport.ts`** — the `EngineTransport` / `EngineStream` interface at the
  **protocol** altitude (typed request + decoded stream), plus `EngineRequest`,
  `StreamHandlers`, `EngineRequestError`. This is the seam a bridge transport (the S7
  LB MCP/Zenoh bridge) implements *outside* the package.
- **`lib/transport-direct.ts`** (a rename of the old `lib/ws.ts`, + the extracted REST
  half) — `DirectTransport`, today's direct-to-CE behavior **verbatim**: the `fetch`
  `{data}|{error}` unwrap (with `X-CE-Session`/`X-Actor-Id`/`X-Gesture-Id` now carried as
  `request` fields, not module state), the binary WS with session resume, cross-tab
  BroadcastChannel dup-tab guard, reconnect backoff, and the `wire.ts` binary decode
  call-site. The only `EngineTransport` implementation in the package; the default.
- **`lib/rest.ts`** — every typed wrapper now funnels through the single private
  `http<T>()`, which is the ONE place calling `transport.request()`. `getSchema()`
  (`GET /schema`, the palette catalogue) was a raw `fetch`; it now rides the seam too, so
  a bridge transport needs no second raw-fetch path.
- **`CeEditor.tsx`** — gained the optional `transport?: EngineTransport` prop (default
  `new DirectTransport()`); a module-level `streamRef` singleton (replaces the old
  `wsClient` `CeRestWs`, which was deleted with `ws.ts`); the stream is driven through
  `StreamHandlers`. Presence stays **direct-mode-only** in v1, wired as an optional second
  arg to `openStream` that a non-direct transport ignores (duck-typed, not `instanceof`).
- **`index.ts`** — exports the seam types + `DirectTransport` / `setRestTransport` /
  `wsUrlFromBase` / `RestError`.
- **`lib/transport.test.tsx`** — the conformance / exit-gate test.

### Blocker fixed (left by the prior in-flight session)

`streamRef` was referenced throughout `CeEditor` but **never declared** — the branch did
not compile. Declared it as the module-level singleton (the natural replacement for the
old `wsClient`). Also removed the now-dead `CeRestWs`/`ws.ts` (all its machinery had been
copied verbatim into `transport-direct.ts`).

### Incidental: base branch was red under `tsc`

`pnpm typecheck` was **already failing on 818f0f8** (the merged PR #2) on three unrelated
dead-code declarations (`rowYCenter` in `FunctionBlock.tsx`, `PROPS_PER` in `perf.bench.ts`,
an unused `inferDataType` import in `CollectionWidget.tsx`). Since the S1 exit gate requires
a green typecheck, these three trivial dead-code removals were made too (zero behavior
change). Noted here because they fall outside the transport seam.

### Green output (S1)

```
$ pnpm typecheck
> tsc --noEmit
(no errors)

$ pnpm test
 ✓ src/lib/transport.test.tsx  (1 test) 80ms
 … (20 other suites)
 Test Files  21 passed (21)
      Tests  145 passed (145)

$ pnpm build            # app (vite.config.ts) + lib (vite.lib.config.ts)
✓ built in 6.22s        # app
✓ built in 8.96s        # lib  (declaration files + cjs/esm bundles)
```

### Exit gate — MET

> "a test renders the editor against MockTransport with zero `fetch`/`WebSocket`
> globals touched."

`lib/transport.test.tsx` renders `CeEditor` against a `MockTransport`, proves it renders a
seeded tree (via `transport.request("GET /nodes")`) and applies a decoded value frame (via
`stream.onFrame`), and asserts `fetch` **and** `WebSocket` were never called (both stubbed
to throw on any call). Green.

### Open questions — RESOLVED (written into the slice doc)

- **WS `schema` message vs `GET /schema`** — two distinct things; the seam covers both. The
  WS message is a slim value-plane decode table (rides the stream); `GET /schema` is the
  add-node palette catalogue (a typed REST wrapper, confirmed by grep to be what the palette
  reads). `GET /schema` now rides `transport.request()` — zero raw `fetch(` remains in
  `CeEditor.tsx`.
- **Full `http()` call-site coverage** — every `rest.ts` wrapper funnels through the single
  `http<T>()` → `transport.request()`, so the seam covers all of them (reads, writes,
  overrides, actions, edges, facets, bulk, copy/restore, undo/redo/changelog) by construction.

### Commit

`d0bf28b feat(transport): carve the EngineTransport seam (S1)` on `lb-transport`.
9 files, +589/−153; git tracks `ws.ts → transport-direct.ts` as a rename.

---

## S3 — the sidecar crate + local read verbs (this repo, branch `ce-v1`)

The native Tier-2 CE bridge, end to end for the **local** case: the sidecar binary, the
manifest, the pinned `rubix-ce` dep, and the two read verbs `control-engine.tree` +
`control-engine.schema`, with the mandatory deny/happy/hot-restart tests from day one.

### What shipped (`rust/extensions/control-engine/`)

Mirrors the `federation` sidecar shape; every file within FILE-LAYOUT limits:

| File | Lines | Responsibility |
|---|---|---|
| `src/main.rs` | 202 | stdio control loop (init/health/shutdown/call, `lb_supervisor` wire) + dispatch; `#[cfg(test)]` dispatch/counter unit tests |
| `src/engine.rs` | 64 | `Arc<dyn ControlEngine>` per appliance, lazily built from `base` via `CeRestClient`, cached by id |
| `src/args.rs` | 144 | the `{appliance, …}` arg envelope + `NodeRef`/base parsing (canonical `7979`) |
| `src/tools/{mod,tree,schema}.rs` | 37/35/16 | one verb per file: parse args → trait call → **verbatim** serde JSON |
| `src/ce_fake.rs` | 165 | the ONE sanctioned CE stub (in-memory `ControlEngine`) + an `AtomicUsize` call counter |
| `extension.toml` | 46 | tier=native, placement=either, `net:tcp:127.0.0.1:7979:connect`, `[[tools]]` tree+schema |
| `Cargo.toml` | 52 | pinned `rubix-ce` git dep (+ commented path override), `ce-fake` feature |
| `build.sh` | 10 | federation precedent |

- **`rubix-ce` pinned git dep** — `rev = 51ab97edf32d622f94d00401aee3ae2daf8859c8`, features
  `["rest","ws"]` (the default `CeRestClient<WsStreamTransport>` needs both). The git form
  **fetched and resolved cleanly** (in `Cargo.lock`); the commented-out `path` override for
  `~/code/ce/ce-client-rust` sits right below it for side-by-side local dev.
- Added `extensions/control-engine` to `rust/Cargo.toml` `[workspace] members`.

### The ONE fake, wired to prove the real path

`ce_fake` is compiled into the binary under the `ce-fake` cargo feature and armed **per-run**
by `LB_CE_FAKE=1` (OFF in a shipped binary — the fake never leaks into the real path, CLAUDE
§9). This lets the host integration test drive the **real** supervisor + **real** caps gate +
**real** stdio ABI against a CE we cannot build in Rust CI (the C++20 engine), with only the
external CE stubbed behind `rubix-ce`'s `ControlEngine` trait. The fake bumps an `AtomicUsize`
on every trait method so the dispatch-layer unit test can prove **0 trait calls before a
denied/unknown call**.

### Capability / deny path (mandatory)

The tool **NAME** is the gate: `authorize_tool` maps `control-engine.tree` →
`mcp:control-engine.tree:call` with **no CE knowledge**, workspace-first then capability. The
gate runs **before** `call_sidecar`, so a denied caller never reaches the sidecar or any CE
trait call. Two places assert this:
- host test: a caller without the cap → opaque `lb_mcp::ToolError::Denied` at the gate;
- crate unit test: the dispatch fn makes **0** counter increments for an unknown/denied tool,
  exactly **1** per allowed call (the "0 trait calls before deny" proof at the dispatch seam).

### Green output (S3)

```
$ cargo test -p control-engine --features ce-fake
running 6 tests
test args::tests::base_defaults_to_canonical_local ... ok
test args::tests::base_parses_host_port_bare_port_and_scheme ... ok
test args::tests::noderef_root_and_keyed ... ok
test dispatch_tests::unknown_tool_errors_without_a_trait_call ... ok
test dispatch_tests::tree_returns_seeded_graph_verbatim_and_counts_one_call ... ok
test dispatch_tests::schema_returns_manifest_list_verbatim ... ok
test result: ok. 6 passed; 0 failed

$ cargo test -p lb-host --test control_engine_test
running 2 tests
test control_engine_against_real_ce_studio ... ignored, needs a real ce-studio engine …
test control_engine_local_read_verbs_and_supervision ... ok      # deny + happy + hot-restart
test result: ok. 1 passed; 0 failed; 1 ignored     (finished in 10.25s)

$ cargo build --workspace
Finished `dev` profile     # green (one pre-existing lb-external-agent warning, unrelated)
```

### The real-engine tier — ran GREEN against live ce-studio

A real CE was running on `:7979` this session, so the opt-in `#[ignore]`d tier ran for real
(not just a documented skip): the sidecar drove both verbs through the **real** `rubix-ce`
REST/WS client to the actual engine.

```
$ curl -s -o /dev/null -w 'HTTP %{http_code}\n' http://127.0.0.1:7979/api/v0/schema
HTTP 200
$ CE_ENGINE_URL=127.0.0.1:7979 cargo test -p lb-host --test control_engine_test \
    -- --ignored control_engine_against_real_ce_studio
test control_engine_against_real_ce_studio ... ok
test result: ok. 1 passed; 0 failed     (finished in 10.26s)
```

To reproduce: `cd ~/code/ce/ce-studio && ./run.sh --engine-only` (ce-rest on `:7979`), then
the command above.

### No CE leak into core (sanity grep — clean)

`grep -rnE "control-engine|control_engine|rubix-ce\b|ControlEngine|ce-rest|ce_fake"` over
`crates/host/src crates/mcp/src crates/caps/src role/gateway/src` returns **nothing**. (The
only core reference is the host *test* file, which is allowed to name the extension. `rubix-cube`
matches in the rules/flows comments are a different, unrelated project.)

### Exit gate — MET

> `cargo test --workspace` green including the new extension; one documented real-engine
> run of `control-engine.tree` against ce-studio's engine.

Both met — the workspace builds/tests green with the new crate, and the real-engine run above
is genuine, not documented-as-skipped.

### Open questions — RESOLVED (written into the slice doc)

- **DTO shape → VERBATIM.** `tree`/`schema` serialize `rubix-ce`'s own serde form straight
  through. Confirmed clean (round-trips through JSON; uids ride the keyed `NodeKey`/`Uid`
  form, not a bigint-hostile bare u64) by the crate unit tests + the real-engine run.
- **Canonical port → `7979`** everywhere (manifest, arg default, test tier). The parent
  scope's stray `7878` example is superseded.

---

## Cross-slice note — the host-callback dependency (S5+)

A parallel, independent effort on the `ui-ext` branch of the *main* `lb` worktree shipped the
**native-sidecar → host MCP callback transport** (`lb-sidecar-client`, node-signed
`LB_EXT_TOKEN`, `LB_GATEWAY_URL` injection). S3 as scoped does **not** need it — the two read
verbs only reach *outward* to the CE over `net:tcp`. But S3's later slices **will**: the CE
**write** verbs (S5) enqueue setpoints via `outbox.enqueue`, which is exactly a host-callback.
When S5 lands, `control-engine` should adopt `lb-sidecar-client` rather than re-invent it — the
same mechanism the ROS driver already uses.

## Deferred / next

- **Workspace isolation** for the verbs is meaningfully testable only with the appliance
  registry → **S4** (noted in the test, NOT faked here).
- **S4**: the appliance registry + the registry-routed `call_tool` hop (so the wiresheet/agent
  reach the verbs uniformly). **S5**: write verbs (via the host-callback above). Then S2 (vendor
  the byte-identical wiresheet snapshot), S6 (`ce.watch` COV), S7 (the `BridgeTransport`
  implementing S1's interface + the page), S8 (harden/ship).
