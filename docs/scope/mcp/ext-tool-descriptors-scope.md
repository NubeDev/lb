# MCP scope — extension tool descriptors over the init handshake

Status: **BUILT 2026-07-28, unreleased** (lb + lb-ext-sdk). Session:
[`sessions/mcp/ext-tool-descriptors-session.md`](../../sessions/mcp/ext-tool-descriptors-session.md).
No tag cut and no pin bumped — needs the next `sdk-v*` then `node-v*`. Four of this doc's premises
were wrong against the tree (the handshake discarded its payload; lb has no SDK dep; `forms.*` rich
descriptors had already shipped and the real gap was the name-only `HOST_TOOLS` table; routed
registration had no message to widen) — the session doc records each and what was built instead.
Promotes to `public/extensions/` + `public/mcp/` once released.
Owning repos: **lb-ext-sdk** (the wire + trait, released as the next `sdk-v*`) and **lb**
(host fold-in + catalog rows, released as the next `node-v*`). Downstream consumers then
bump pins: `rubix-ai` (node tag) and each extension repo (sdk tag).

The registry already models the full contract — `ToolDescriptor { name, title, group,
input_schema, emits_external, result }` (`rust/crates/mcp/src/registry.rs`), where
`input_schema = None` is valid and additive and the doc-comment explicitly anticipates
"widening the registry from bare names (`Vec<String>`) to descriptors" as the
SDK/manifest-adjacent change, versioned by absence. But the native-extension **init
handshake still carries bare names** (`InitReply { protocol_major, tools: Vec<String> }`
in `lb-ext-native`), so `host/src/native/install.rs` can only register
`ToolDescriptor::name_only(bare)` and `tools.catalog` serves every extension tool
schema-less. Every schema consumer — the channel command palette, the forms builder,
the dashboard write-action builder — degrades to a free-text arg for extension tools.
This scope closes that one gap: let an extension **self-declare** its descriptors in the
handshake, exactly as the registry was designed to receive them.

## Goals

- **SDK (`lb-ext-native`)**: add `Tools::descriptors() -> Vec<ToolDescriptor>` with a
  default body of `self.tools().into_iter().map(ToolDescriptor::name_only).collect()` —
  an existing extension recompiles unchanged. The wire's `InitReply` gains an additive
  `descriptors` field (serde `default` + `skip_serializing_if empty`); `PROTOCOL_MAJOR`
  does **not** bump (purely additive).
- **SDK ergonomics**: a feature-gated `schemars` integration so an extension derives
  `JsonSchema` on the same serde `Args` struct its tool already parses — one source of
  truth, schema cannot drift from the parser. Hand-authored `serde_json::json!` schemas
  remain equally valid; the helper is sugar, not a requirement.
- **lb host**: `native/install.rs` consumes `InitReply.descriptors` when non-empty and
  falls back to `name_only(tools)` when absent (old child). Names stay **bare** in the
  descriptor (the `<ext>.` prefix remains catalog-side, per the existing rule).
- **Routed dispatch**: the bus registration that feeds `Target::Remote { tools }` carries
  the full descriptors (schema fields included) so a remote node's `tools.catalog` is as
  rich as the hosting node's.
- **Catalog gap closure**: register descriptors for the dispatched-but-uncataloged
  host-native `forms.*` family (known gap from the ci-red-baseline session — the prior
  fix was reverted over the file-size ratchet; land it split per `FILE-LAYOUT.md`), and
  audit the other prefix-routed families for missing rows while there.

## Non-goals

- **No manifest `[[tools]].schema` field.** The manifest stays `name` + prose
  `description` (the caps source). A second schema source would drift from the serde
  struct; the running child is the single authority, reported at init.
- **No dispatch-time schema validation.** The host does not validate `input` against
  `input_schema` before dispatch — the extension's own parse remains the authority (a
  tool that lies about its schema only breaks its own form UX, same trust model as
  `emits_external`). Catalog schemas are a UI affordance, not a security boundary.
- **No `ext.caps` / extended `ext.list`** (widget-platform gap G5 / Slice E stays its own
  scope). `ext.list` keeps carrying zero tool information; `tools.catalog` is the one
  tool-discovery verb.
- **No wasm-tier change in this slice.** The wasm ABI's tool declaration is a separate
  seam; this scope is the native sidecar wire. (The registry fold-in is tier-agnostic,
  so wasm follows later with no host rework.)

## Intent / approach

Wire shape (all additive):

```jsonc
// child → host, reply to `init`
{
  "protocol_major": 1,
  "tools": ["point.read", "point.write", ...],        // unchanged, still authoritative for dispatch
  "descriptors": [                                      // NEW, optional
    { "name": "point.write",
      "title": "Write point",
      "group": "points",
      "input_schema": { "type": "object", "properties": { ... }, "required": [...] },
      "emits_external": true }
  ]
}
```

Back-compat matrix — every cell must keep working, tested:

| | old host | new host |
|---|---|---|
| **old ext** | today | `descriptors` absent → `name_only` fallback (current behavior, bit-identical) |
| **new ext** | unknown field ignored by serde | full descriptors registered |

When both `tools` and `descriptors` are present, `tools` stays the dispatch allowlist and
descriptors are joined onto it by name; a descriptor for an undeclared tool is dropped
with a warn (never a boot failure). The alternative rejected: replacing `tools` with
descriptors outright — it forces a `PROTOCOL_MAJOR` bump and a coordinated flag-day for
every published extension, for zero functional gain.

## How it fits

- **Rule 10**: nothing here names an extension. Schema is self-declared per-tool exactly
  like `emits_external`; the host folds whatever arrives through the one generic
  handshake. Swapping any extension changes no core line.
- **Capabilities & the deny path**: unchanged. Catalog row visibility remains gated by
  the tool's own `mcp:<ext>.<tool>:call` cap — a denied tool's schema is **absent, never
  an error** ("the menu is the permission model"). Dispatch authorization is untouched.
- **Symmetric nodes**: descriptors ride the same bus registration remote nodes already
  use; no role branch.
- **MCP surface**: no new verb. `tools.catalog` simply starts returning non-null
  `input_schema` for extension rows.
- **No mocks**: tests boot the real SDK serve loop over an in-memory duplex and the real
  host registry/install path.

## Example flow

1. A points extension derives `JsonSchema` on its `point.write` args struct
   `{ point: String, value: Value, read_back: Option<bool> }` and returns a descriptor
   for it from `Tools::descriptors()`, with an `x-lb` hint on `point`
   (`{ "widget": "select", "source": "<ext>.point.list" }`).
2. The host boots the sidecar; `init` returns names + descriptors; `install.rs` registers
   `Target::Local(Hosted { tools: descriptors, .. })`.
3. A UI calls `tools.catalog`; the viewer holds `mcp:<ext>.point.write:call`, so the row
   arrives with the schema; the forms/write-action builder renders a typed form — the
   `point` field is a dropdown populated by calling the (equally cap-gated) list verb.
4. A viewer **without** the cap calls `tools.catalog`: the row is absent. Nothing leaked.
5. Submit dispatches `<ext>.point.write` through `POST /mcp/call` — resolve → authorize →
   dispatch, audited at the chokepoint, classified irreversible for undo. All shipped
   behavior; this scope changed only step 1–3's schema availability.

## Testing plan

- **SDK unit**: wire roundtrip of `InitReply` with and without `descriptors`; the
  default-`descriptors()` trait body; old-frame (no field) deserialization.
- **Host integration (real child)**: spawn the SDK serve loop over duplex with a test
  `Tools` impl declaring one schema'd tool; assert the registry holds the schema and
  `tools.catalog` serves it. Second run with a name-only child; assert `name_only`
  fallback.
- **Capability-deny** (mandatory category): catalog hides the schema'd row for an
  uncapped subject; dispatch of the same tool still denies without leaking existence.
- **Workspace-isolation**: catalog under ws A never lists ws B's routed targets.
- **Routed**: register remote descriptors over the bus registration path; assert the
  calling node's catalog carries the schema.
- **forms.\* rows**: catalog test asserting `forms.get/list/save/delete` appear (cap-held)
  with schemas.

## Risks & hard problems

- **Descriptor bloat on init**: a large extension's schemas fatten the handshake frame.
  Accepted — frames are length-prefixed and read once at boot; no cap needed in v1.
- **The file-size ratchet** that reverted the previous `HOST_TOOLS` forms fix: land the
  catalog rows as per-family descriptor files (folder-of-verbs), not one growing table.
- **Schema/parse drift** for extensions that hand-author schemas instead of deriving:
  accepted trust model (self-declared, breaks only their own form UX).

## Open questions

None — the deliberate decisions this scope commits to: handshake-only (no manifest
schema), no dispatch-time validation, no `PROTOCOL_MAJOR` bump, `tools` remains the
dispatch allowlist, wasm tier deferred.

## Related

- `docs/scope/mcp/mcp-scope.md` — resolve → authorize → dispatch, the cap grammar.
- `docs/scope/widgets/widget-platform-scope.md` — `ToolDescriptor` carrying both halves
  of the contract; `tools.catalog` as the one discovery verb.
- `docs/scope/channels/channels-rich-responses-scope.md` — `x-lb` hints driving the
  request form.
- `docs/scope/mcp/routed-node-dispatch-scope.md` — `Target::Remote` descriptor carriage.
- Downstream: rubix-ai `docs/scope/frontend/dashboard/write-action-builder-scope.md`
  (the consumer UX), rubix-ai-extensions `docs/scope/modbus/modbus-tool-schemas-scope.md`
  (first real declarer).
