# ce-v1 — Slice S5 (graph WRITE verbs) session

Branch: `ce-v1` (worktree). Depends on S3 (read verbs + sidecar) + S4 (resolve + routing).
Spec: `rust/extensions/control-engine/docs/slice-5-write-verbs.md`.

## What shipped

The seven v1 graph-mutation verbs, each a thin caps-gated map onto ONE `ControlEngine`
trait method, working over the local AND the routed path. One file per verb under
`src/tools/` (folder-of-verbs).

### Verb map (all shipped)

| MCP tool | Trait method | Returns (wire) |
|---|---|---|
| `control-engine.add-node` | `add_node(parent: NodeRef, NewNode) -> NodeKey` | `{ uid, kind: "component" }` |
| `control-engine.patch` | `patch(&NodeKey, Vec<PropPatch>) -> ComponentDto` | `{ component: <DTO verbatim> }` |
| `control-engine.set-override` | `set_override(&NodeKey, prop, FlexValue, Duration) -> ()` | `{ ok: true }` |
| `control-engine.clear-override` | `clear_override(&NodeKey, prop) -> ()` | `{ ok: true }` |
| `control-engine.add-edge` | `add_edge(EdgeSpec) -> NodeKey` | `{ uid, kind: "edge" }` |
| `control-engine.remove-node` | `remove_node(&NodeKey) -> DeletedItems` | `{ deleted: { component_uids, edge_uids } }` |
| `control-engine.call-action` | `call_action(&NodeKey, action, Params) -> ActionResult` | `{ returns: { <name>: <value> } }` |

- `set-override`: arg `ttl_secs: u64`, `0` = permanent → `Duration::from_secs(ttl_secs)`.
- `add-node`: `name?` passed through as `Option<String>` — CE (the client) supplies a
  sanitized default when absent. `initial_values` is a name→scalar object.
- `remove-node`: returns the soft-deleted UIDs (CE's 24h-undo handle) — S8's `restore`
  consumes them. `DeletedItems`/`ActionResult`/`NodeKey` are in-process types (no serde
  derive), so their UID lists / returns are projected to JSON in the verb (the wire form
  lives in the verb + `args.rs`, exactly as `NodeRefArg` owns the read wire form).

### Node identity on the wire (write vs read)

`args.rs` grew `NodeKeyArg` + `require_node_key(input, instance)`: a write MUST address a
concrete keyed node (`{ uid, kind?, path? }`) — an absent/malformed `uid` is a `bad node
arg` error, never a silent root fallback (unlike `NodeRefArg` for reads, which defaults to
Root). Also added `flex_value` (JSON scalar → `FlexValue`, untagged) and `value_pairs`
(name→scalar object → `Vec<(String, FlexValue)>`, shared by `PropPatch` batches + action
`Params`). Edge endpoints reuse `NodeKeyArg` and carry an optional `path` (the client's
bulk edge-create workaround addresses endpoints by component path).

### Caps + self-check (mirrors S4)

- Each verb has its OWN gate `mcp:control-engine.<verb>:call`, added to `extension.toml`
  `[[tools]]`. The seven caps were also added to the manifest `request = [...]` (so the
  sidecar's grant = requested ∩ admin-approved actually carries them).
- **No new store/net/secret cap.** The write verbs reach CE over the already-granted
  `net:tcp:127.0.0.1:7979:connect` socket. The manifest's earlier "`secret:*` lands in S5"
  note was corrected — S5 needs no secret cap.
- **Self-check first (defense-in-depth).** The inbound `native.call` carries no caller
  identity, so each write verb calls `host.require("control-engine.<verb>")?` FIRST (opaque
  `Denied`), before resolve/parse/trait-call — the finer gate the host's coarse
  `mcp:native.call:call` cannot express. Deny is ALSO enforced host-side by `authorize_tool`
  on the tool name at the routed/native.call boundary; both hold.
- `serve.rs::dispatch` gained a minimal, localized write arm: `is_write_verb(tool)` →
  `HostCtx::grant_only_from_env()` → `tools::dispatch_write(host, engine, instance, …)`.
  Reads stay ungated here (S3's concern). The change is small so S6's parallel serve.rs edit
  doesn't conflict.

### Open question resolved: `{ session?, actor? }` envelope

**Deferred, per spec.** The slice doc's optional `{ session?, actor? }` attribution maps to
CE's `X-CE-Session`/`X-Actor-Id` headers — but the **pinned `ce-client-rust` rev
`51ab97e` exposes NO per-call header/session/actor hook** (checked `src/`: only WS
`sessionId` in the COV configure handshake; the REST `ControlEngine` trait methods take no
headers). We did NOT invent a client API. The "LB principal → CE actor" mapping stays a
later follow-up (CE actors are a per-editor-tab concept; LB does not map its identity onto
them in v1). The verbs accept and ignore the optional envelope args for now.

### `grant_only_from_env` (a new `HostCtx` constructor)

The registry verbs build a full `HostCtx::from_env()` (needs `LB_GATEWAY_URL` for the
`store.*` callback). The write verbs never use a callback — they reach CE directly — so a
full `HostCtx` is more than they need, and the real-engine / routing dev tiers run the
sidecar with NO gateway. `HostCtx::grant_only_from_env()` parses only the `LB_EXT_TOKEN`
grant for the self-check (best-effort client, never called on the write path). Without this,
a routed write panicked with "no callback address: LB_GATEWAY_URL is not set".

## The ce_fake

Kept the inert write-method stubs that `bump()` the counter (the ONE sanctioned fake). Crate
unit tests assert counter 0→1 and arg parsing. No other fake added.

## Tests (all green, real infra)

1. **Crate unit** (`src/tools/mod.rs` `dispatch_tests`): `each_write_verb_self_checks_then_
   calls_the_trait_once` (per-verb: no cap → `Denied` + counter stays 0; with cap → counter
   0→1) + `write_verb_rejects_missing_node_before_trait_call` (arg validation before any
   trait call). **8 passed.**
2. **Host integration deny + happy** (`crates/host/tests/control_engine_test.rs`
   `control_engine_write_verbs_happy_and_deny_matrix`): per-verb happy write against the
   fake-backed sidecar + per-verb deny (caller lacking the verb cap → opaque `Denied`). Added
   the write caps to `admin()` AND the `install()` approved list (the sidecar's grant must
   carry them for the self-check). **2 passed** (+ the S3 read/supervision test).
3. **Routed write** (`control_engine_appliance_routing_test.rs`
   `appliance_record_routes_ce_patch_write_and_offline_fails_loud`): two in-process nodes over
   loopback TCP, node B runs the sidecar (`--features ce-fake`, `LB_CE_FAKE=1`), appliance
   record on A → B; `control-engine.patch` crosses the hop with the ws claim re-check.
4. **Fail-loud on unreachable write**: same test drops node B; the routed patch errors promptly
   (`ToolError::Extension`) and the workspace outbox stays empty (no queued retry). **2 passed.**
5. **Real-engine opt-in tier** (`control_engine_real_write_flow`, `#[ignore]`d, gated on
   `CE_ENGINE_URL`): scripted add two `NubeIO-math::add` nodes → tree reflects both (by uid) →
   add-edge → patch → call-action → remove-node → tree no longer has the removed node.

## Real-engine run notes (tried here, per spec)

Ran against live ce-studio (`~/code/ce/ce-studio/run.sh --engine-only`, ce-rest on `:7979`).
**Proven end-to-end against the REAL engine:** `add-node` (created nodes at real uids
100014/100015), `tree` (reflected both adds), `patch` (returned the DTO), `remove-node`. Two
steps hit **documented `ce-client-rust` ↔ live-engine decode quirks, NOT faults in the S5
verb mapping**, so the real-flow test treats them as best-effort (accepts a clean CE error):
  - `add-edge`: the client's bulk edge-create workaround returned "no edge UID" on this rev
    (`CE_REST_API_NOTES.md` flags `POST /edge` as broken; the bulk fallback decode is brittle).
  - `call-action`: `NubeIO-math::add` defines no action, so CE returns a clean 400 — the verb
    still round-trips.
A `tree withEdges=true` decode also failed against a persisted edge (`missing field
source_uid`) — an `EdgeDto` shape mismatch in the pinned client, again outside S5's scope.
The removal of the persisted `data.db` to get a clean run left the local engine in a stuck
boot state; restored from backup. The **fake-backed CI gate is fully green** and is the
authority; the real-engine tier is opt-in and confirms the write verbs mutate a live engine.

## Commands

- `cargo build --workspace` — green.
- `cargo test -p control-engine` — 8 + 6 + units green.
- `cargo test -p lb-host --test control_engine_test --test control_engine_appliance_routing_test`
  — 2 + 2 (+ 2 ignored real-engine) green.
- `cargo fmt` — applied.
- Sanity-grep (`control-engine|control_engine|rubix-ce|ControlEngine|ce-rest|ce_fake` over
  `crates/{host,mcp,caps,runtime,bus}/src` + `role/gateway/src`) — **empty (clean)**.
- Pre-existing, unrelated: `lb-cli`/`lb-role-gateway` test binaries fail to COMPILE because a
  prebuilt `extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm` is absent
  (no `wasm32-wasip2` target installed in this env). Reproduces on the base commit without
  any S5 change; not caused by this slice.

## Files

- New verbs: `src/tools/{add_node,patch,set_override,clear_override,add_edge,remove_node,
  call_action}.rs`.
- `src/tools/mod.rs`: `is_write_verb`, `dispatch_write`, module decls, write dispatch tests.
- `src/args.rs`: `NodeKeyArg`, `require_node_key`, `flex_value`, `value_pairs`.
- `src/host.rs`: `grant_only_from_env`, `require`/`require_caps` refactor.
- `src/serve.rs`: minimal write-dispatch arm.
- `extension.toml`: 7 `[[tools]]` + 7 `request` caps.
- Tests: `crates/host/tests/control_engine_test.rs`,
  `crates/host/tests/control_engine_appliance_routing_test.rs`.
