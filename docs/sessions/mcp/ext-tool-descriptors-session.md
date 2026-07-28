# Session — extension tool descriptors over the `init` handshake

Scope: [`docs/scope/mcp/ext-tool-descriptors-scope.md`](../../scope/mcp/ext-tool-descriptors-scope.md)
Date: 2026-07-28
Repos touched this session: **lb-ext-sdk** (the wire), **lb** (this one), and downstream
**rubix-ai-extensions** (modbus, the first declarer) + **rubix-ai** (the consumer UX) — each with its
own session doc.

Status: **built** in lb + lb-ext-sdk. Not released — no tag was cut and no pin was bumped (the user
owns all git in this program). See "Pending release" below.

---

## What the scope asked vs what the code actually was

Four of the scope's premises did not survive contact with the tree. Recording them here because the
scope doc reads as if they were true, and the next reader will hit the same wall.

**1. `install.rs` never saw `InitReply` at all.** The scope says "consume `InitReply.descriptors` in
`native/install.rs`". But `install.rs:77` took tools from the **manifest** (`spec.rs:96`), and
`supervisor/src/sidecar.rs` `handshake()` read frames until `id == 0`, checked `reply.error`, and
**discarded `reply.result` entirely**. There was no `InitReply` type anywhere in `rust/`. So the work
was not "read a new field" but "make the handshake return its payload at all":

- `crates/supervisor/src/init.rs` (new) — `InitReply` + `ToolDescriptor`, the wire body.
- `handshake()` now returns `(Conn, Option<InitReply>)`; `Sidecar` holds it and exposes `declared()`.
- `replace_generation` re-reads it, so a restart onto an upgraded binary replaces the declaration
  rather than leaving a stale one behind a live process.

**2. lb does not depend on the SDK, and must not start.** `rust/Cargo.toml` pulls only
`lb-sidecar-client` (tag `sdk-v0.3.0`); `lb-supervisor` has no SDK dep, and `echo-sidecar` consumes
`lb-supervisor` directly. **Decision: mirror the types host-side in `lb-supervisor`**, exactly as
`CallParams`/`Caller` are already mirrored *into* the SDK. Rejected: adding `lb-ext-native` as a git
dep — it would have made this slice un-buildable until a tag existed, inverting the release order
(SDK tag first, then node) that the native-caller-identity slice established. The wire is the
contract; each side writes its own half.

**3. `forms.*` rich descriptors had already shipped.** `host/src/tools/descriptor.rs:80-85` has
returned `forms::{save,get,list,delete}_descriptor()` since commit `e94d7ed2` (2026-07-25) — after
the ci-red-baseline session the scope cites. The actual gap was the **name-only `HOST_TOOLS` table**
in `system/catalog.rs`, where `forms.` had zero rows, leaving
`host_catalog_covers_dispatch_prefixes()` **red on master**. Fixed as part of the split below.

**4. Routed registration had no message to widen.** `host/src/remote.rs`
`register_remote_extension(&[String])` is a plain function with no production caller (the
fleet-presence announce that would populate it is unimplemented). "Carry descriptors through routed
registration" therefore meant adding a descriptor-taking sibling, not changing a wire shape.

---

## What was built

### lb-ext-sdk (`lb-ext-native`) — the wire

- `src/descriptor.rs` (new) — `ToolDescriptor` with `name_only` + consuming builders. Serialize **and**
  Deserialize (both ends read it); `PartialEq` but not `Eq` (schemas are `serde_json::Value`).
- `src/schema.rs` (new, feature `schemars`) — `schema_for::<T>()` generating the input schema from the
  same serde struct the tool parses.
- `handshake.rs` — additive `InitReply.descriptors` (`default` + `skip_serializing_if = Vec::is_empty`)
  and `with_descriptors`. `PROTOCOL_MAJOR` **not** bumped (it is `0`, not `1` as the scope's example
  JSON shows — the snippet is illustrative).
- `serve.rs` — `Tools::descriptors()` default method returning one `name_only` per `tools()` entry.
- `Cargo.toml` — the repo's first `[features]` table: `schemars = ["dep:schemars"]`, off by default.

**Two decisions worth the ink:**

*draft-07, and `$schema` stripped.* The consumers are `ajv` instances in a browser. A default `ajv`
speaks draft-07 and **refuses to compile** a document declaring `$schema: ".../2020-12/schema"`
rather than degrading. So `schema_for` emits draft-07 and removes the dialect key, letting each
consumer's default apply. The alternative — emit 2020-12 and make every downstream UI instantiate a
matching validator — pushes coordination onto every consumer for no gain at the shapes tool args take.

*A non-declaring child emits the byte-identical old frame.* The default `descriptors()` body returns
name-only entries, which carry nothing `tools` doesn't. Sending them would fatten every frame and make
"descriptors absent" stop meaning "nothing declared". So `serve`'s `init` arm checks
`descriptors.iter().all(ToolDescriptor::is_name_only)` and omits the field entirely in that case.
`tests/descriptors_wire_test.rs` asserts the exact bytes, so the compat claim is a property, not a hope.

### lb — the host fold-in

- `crates/supervisor/src/init.rs` (new, 154) — the wire body. **Parsing is fail-open by design**:
  `InitReply::parse` returns `None` on anything unreadable and every field is `#[serde(default)]`.
  This is load-bearing, not defensive habit — lb's own child serve loop
  (`supervisor/src/serve.rs:119`) answers `{"ready":true,"ext":"…"}`, and the existing
  `native_deny_test`/`native_isolation_test` fake launchers answer `Reply::ok(id, "ready")`. A strict
  parse would have failed every one of those spawns. A child cannot break its own boot by
  mis-declaring; the worst case is the behaviour that already shipped.
- `crates/host/src/native/descriptors.rs` (new, 213) — `join_descriptors(ext_id, tools, declared)`.
  **The manifest stays the allowlist**: it says *which* tools exist (it is the capability source, so a
  child cannot widen its own surface at runtime), the declaration says what each one *looks like*.
  Descriptors naming a tool the manifest does not list are dropped with a `tracing::warn!`. Handles
  qualified-vs-bare names on either side.
- `crates/host/src/native/install.rs` — reads `sidecar.declared()` off the live handle before it moves
  into the runtime map, and calls the join.
- `crates/host/src/remote.rs` — `register_remote_descriptors`, the descriptor-carrying sibling.
- `crates/mcp/src/registry.rs` — `ToolDescriptor` gains `PartialEq` so the fallback can be *proven*
  bit-identical rather than asserted field by field.
- `extensions/echo-sidecar` — now declares a **partial** contract: full for `echo`, nothing for
  `whoami`. Deliberate: one real child then exercises both the enriched path and the `name_only`
  fallback in a single spawn. A child that declared everything would leave the fallback — the path
  every already-published extension takes — proven only against a fake.
- `crates/host/src/system/catalog.rs` → **`system/catalog/` (33 files, largest 175)**. The split the
  scope asked for, which is what let the `forms.*` rows land after the previous attempt was reverted
  over the file-size ratchet.

**Catalog audit bonus.** While splitting, a sweep of every `"<prefix>.<verb>" =>` match arm under
`crates/host/src` against the catalog found **38 further dispatched-but-uncataloged verbs** beyond the
4 `forms.*` ones — most notably the entire `agent.*` config/persona/memory surface (19 verbs), which
was invisible to `tools.catalog`, i.e. to the agent's own menu. All 42 rows added; 247 → 289 rows,
every retained row byte-identical.

`bus.watch` was **deliberately left uncataloged**: it is dispatched but unconditionally returns
`BadInput("bus.watch is a stream — use GET /bus/{subject}/stream")`. Cataloging it would advertise a
verb no MCP call can succeed at.

---

## Tests

`crates/host/tests/native_descriptors_test.rs` (new) — real `echo-sidecar` OS child, real embedded
SurrealDB, real install path, real catalog verb. No mocks.

```
running 5 tests
test a_remote_registration_carries_its_descriptors ... ok
test a_declared_tool_does_not_reach_another_workspaces_child ... ok
test the_catalog_hides_a_declared_row_from_an_uncapped_subject ... ok
test an_undeclared_tool_falls_back_to_name_only ... ok
test a_declared_schema_reaches_the_catalog_from_a_real_child ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.25s
```

Plus `supervisor/src/init.rs` unit tests (5), `native/descriptors.rs` unit tests (6), and the
regression cover in the SDK (`descriptors_wire_test.rs`, 4 + handshake unit tests).

No regressions: `native_test` 5/5, `native_deny_test` 3/3, `native_isolation_test` 1/1,
`native_concurrent_call_test` 3/3, `native_install_grant_durability_test` 3/3,
`routed_host_entry_test` 5/5, `cross_node_routing_test` 3/3, `system_map_test` 13/13,
`lb-host --lib` 356/356 (incl. `host_catalog_covers_dispatch_prefixes`, previously red).

### Two tests I wrote wrong first, and what they taught

**Workspace isolation.** I first asserted that a neighbour workspace cannot *see* the catalog row. It
failed — and correctly. The MCP registry is deliberately **node-global** (one `SidecarDispatch` entry
per ext id serves every workspace's child); the wall is structural, in the `SidecarMap` keyed by
`(ws, ext_id)`. So the test now pins the wall where it actually is: ws-b's identical call is refused
and never reaches ws-a's process. Asserting invisibility would have pinned a property the design never
promised — and would have passed for the wrong reason before schemas existed, since a name-only row
leaks exactly the same fact a schema'd one does. **Worth noting as a real (pre-existing, out-of-scope)
observation: `tools.catalog` does disclose the *existence* of an extension installed in another
workspace on the same node, to a caller holding the matching cap.**

**The name-only fallback's group/title.** I asserted `group.is_empty()`. It failed: `tools/catalog.rs`
:98-103 has long defaulted an empty group to the ext id and an empty title to the qualified name. The
fallback must *reproduce* that, not bypass it — so the test now asserts the defaults, which is the
actual "bit-identical" property.

---

## Pending release (no git performed — the user owns tags and pins)

1. Cut the next **`sdk-v*`** tag on lb-ext-sdk (additive; no `PROTOCOL_MAJOR` bump).
2. Cut the next **`node-v*`** tag on lb.
3. Bump pins: rubix-ai (node tag), rubix-ai-extensions/modbus (sdk tag) — and **delete the local
   `[patch]`** in `extensions/modbus/.cargo/config.toml` that currently points at this checkout.
4. `rust/scripts/file-size-baseline.txt` needs exactly one edit: **delete line 36's stale
   `rust/crates/host/src/system/catalog.rs 1366`** entry (the file no longer exists). I did not run
   `--update`, which would have rewritten baseline lines owned by other in-flight sessions.

## Known-red, not mine

`check-file-size.sh` still reports 8 findings, all pre-existing from concurrent sessions
(`builtin_roles.rs`, `lib.rs`, `tool_call.rs`, `federation_sqlite_test.rs`,
`series_retention_patch_test.rs`, `series_align_grid_test.rs`, `packs/manifest.rs`,
`packs/validate.rs`). `system/catalog.rs` — the one this slice owned — is gone from the list. My net
contribution to `host/src/lib.rs` is **zero lines**: the routed export was named
`register_remote_descriptors` (matching the registry method) specifically so it fits on the existing
single export line rather than growing a file already past its baseline.

`cargo clippy -p lb-host` cannot go green in this workspace and does not on master either — it dies
in dependency crates (`frame/src/group.rs`, `store/src/{compact,open}.rs`, `flows/src/*`). Linting
`lb-host` with those allowed: zero diagnostics under `system/catalog/` or `native/descriptors.rs`.

## Related

- Scope: `docs/scope/mcp/ext-tool-descriptors-scope.md`
- Downstream: rubix-ai-extensions `extensions/modbus/docs/sessions/2026-07-28-tool-schemas-session.md`
  (the first real declarer), rubix-ai `docs/sessions/frontend/dashboard/write-action-builder-session.md`
  (the consumer UX).
