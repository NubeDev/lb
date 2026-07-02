# Slice 3 — the sidecar crate + local mode (read verbs)

Status: scope slice (S3). Depends on: nothing in S1/S2 (pure Rust; parallelizable with
them). Parent: `control-engine-scope.md`.

Stand up the native Tier-2 extension end to end for the **local** case: the sidecar
binary, the manifest, the pinned `ce-client-rust` dependency, and the two read verbs
`ce.tree` + `ce.schema` served over the frozen `tool.call` world against a `localhost`
CE — with the mandatory deny tests from day one. This is the "hello, CE" vertical slice;
every later slice adds verbs or routing to this skeleton.

## Deliverables

- `rust/extensions/control-engine/` crate, mirroring the `federation` sidecar shape
  (`main.rs` + one file per responsibility, FILE-LAYOUT limits):
  - `src/main.rs` — sidecar entry: stdio/tool-loop wiring, supervision handshake
    (copy the `federation`/`echo-sidecar` pattern exactly).
  - `src/engine.rs` — holds the `Arc<dyn ControlEngine>` per bound CE: construct
    `ce-client-rust`'s REST client from an appliance record's `base` (+ optional
    token, S5), lazily, cached by appliance id.
  - `src/tools/mod.rs` + `src/tools/tree.rs`, `src/tools/schema.rs` — one verb per
    file (folder-of-verbs): parse args → trait call → JSON result.
  - `src/args.rs` — the shared arg envelope: `{ appliance: string, ...verb args }`,
    plus `NodeRef`/`NodeKey` (de)serialization (uid-keyed, per `ce-client-rust`
    `identity.rs` — never bare integers across the wire without the key form).
- `extension.toml` — the manifest (mqtt precedent): `tier = "native"`,
  `placement = "either"`, `[[tools]]` for `tree` + `schema` (more land per-slice),
  capability requests `net:tcp:127.0.0.1:<port>` only in this slice.
- **`ce-client-rust` pinned as a git dependency** (`rubix-ce` crate,
  `github.com/NubeIO/ce-client-rust`, pinned `rev`), with a commented-out `[patch]`
  path-dep line for side-by-side local dev (`~/code/ce/ce-client-rust`) — the parent
  scope's decision, executed here.
- `build.sh` per the extension convention (federation precedent).

## The CE test backend (decide + build here, reuse everywhere)

The parent scope sanctions exactly one external fake. Two tiers, both built in this
slice because every later slice needs them:

1. **`ce_fake.rs` (CI default)** — one file, implements `ControlEngine` in-memory
   (a `HashMap` graph honoring add/patch/tree semantics). Lives in the extension's
   test tree, behind the trait, named exactly `ce_fake.rs`.
2. **Real-engine tier (opt-in, preferred where available)** — `ce-studio` ships the
   engine prebuilt (`engine.tar.gz`, `run.sh`, ce-rest on `:7979` by default). An
   env-gated test harness (`CE_ENGINE_BUNDLE=… cargo test -- --ignored` or a
   `#[ignore]`d integration test) runs the same suite against the **real** engine +
   the **real** `ce-client-rust` REST/WS transport. This upgrades the parent scope's
   "stub OR tiny real server" choice: we don't need to hand-write a fake HTTP server —
   the actual engine is runnable on a dev box. CI keeps `ce_fake.rs`; the session doc
   shows one green real-engine run.

## Capabilities / deny path (mandatory, this slice)

- `ce.tree` / `ce.schema` gated by `mcp:control-engine.tree:call` /
  `mcp:control-engine.schema:call` — the manifest tool NAME is the gate (house rule).
- Deny test: caller without the grant → `DENIED mcp:control-engine.tree:call`,
  asserted **before** any `ControlEngine` trait call (instrument `ce_fake` with a
  call counter; assert 0).
- The sidecar requests nothing beyond `net:tcp` yet — the manifest grows per-slice,
  never speculatively.

## Testing / exit gate

- In-process `Node` + real registry install of the extension (the `mqtt`/
  `fleet-monitor` test pattern): `ce.tree` returns the fake's seeded graph;
  `ce.schema` returns its manifest list.
- Deny tests as above. Workspace isolation is meaningfully testable only with the
  appliance registry → S4 (note it, don't fake it here).
- Hot-restart: kill the sidecar, supervisor respawns, `ce.tree` works again with no
  state loss (it holds none yet — this pins the stateless guarantee early).
- **Exit gate:** `cargo test --workspace` green including the new extension; one
  documented real-engine run of `ce.tree` against ce-studio's engine.

## Open questions (resolve in-slice)

- `Tree`/`ComponentDto` JSON shape over MCP: pass `ce-client-rust`'s serde form
  through verbatim, or re-shape? **Default: verbatim** (the wiresheet already speaks
  engine DTOs; re-shaping buys nothing and adds drift) — confirm the DTOs serialize
  cleanly (no `bigint`-hostile u64s for the JS side).
- Which CE port is canonical in examples — ce-studio's `7979`; the parent scope's
  example says `7878`. Align the docs on `7979`.
