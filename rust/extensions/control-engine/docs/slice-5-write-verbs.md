# Slice 5 — the write verbs (the v1 command surface)

Status: scope slice (S5). Depends on: S4 (resolution + routing — writes must work
local AND routed from day one). Parent: `control-engine-scope.md`.

Ship the seven v1 graph-mutation verbs, each a thin caps-gated map onto one
`ControlEngine` trait method, each with its own deny test, all working over both the
local and the routed path. Per HOW-TO-CODE §4a this is one contract — no shipping
`patch` and deferring the rest silently; the v1 cut below is the decided scope, and
anything outside it is an explicit deferred list.

## The verb map (v1, decided)

One file per verb under `src/tools/` (folder-of-verbs). Args always start with
`{ appliance }`; node identity is always the **keyed** form (`ce-client-rust`
`NodeKey` — uid + key), never a bare uid, so callers can't cross CE's UID pools.

| MCP tool | Trait method | CE endpoint (via ce-client-rust) | Notes / known CE quirks the client already absorbs |
|---|---|---|---|
| `ce.add-node` | `add_node(parent, NewNode)` | `POST /nodes` | CE 400s on absent name → client supplies sanitized default; we pass `name?` through |
| `ce.patch` | `patch(node, Vec<PropPatch>)` | `PATCH /nodes/uid/{uid}` | prop-name-keyed values |
| `ce.set-override` | `set_override(node, prop, value, ttl)` | `PATCH /overrides/nodes/uid/{uid}` | `ttl_secs: u64`, `0` = permanent |
| `ce.clear-override` | `clear_override(node, prop)` | same, `clearOverrides` | |
| `ce.add-edge` | `add_edge(EdgeSpec)` | `POST /bulknodes` | CE's `POST /edge` is broken — the client's workaround is invisible to us |
| `ce.remove-node` | `remove_node(node)` | `DELETE /nodes/uid/{uid}` | returns `DeletedItems` (component+edge uids) — **return them to the caller**; they are CE's 24h-undo handle and S8's `restore` follow-up consumes them |
| `ce.call-action` | `call_action(node, action, params)` | `POST /call/nodes/uid/{uid}` | returns `ActionResult.returns` |

**Deferred (explicit, additive on the same path):** `ce.remove-edge`, `ce.restore`,
`ce.copy`, `ce.bulk`, `ce.set-layout`, and graph-import-as-a-job. Deferring
`set-layout` means canvas drags don't persist positions through the bridge in v1 —
called out in S7; if that's unacceptable for the demo, `set-layout` is the first
follow-up (it's a 30-line verb on this pattern).

## Caps

- Every write verb has its own gate: `mcp:control-engine.<verb>:call` — read vs write
  is a *grant-bundle* concern (whoever grants can hand out read-only = the four read
  caps), not a code concern.
- Session/actor attribution: the wiresheet sends `X-CE-Session`/`X-Actor-Id` for echo
  suppression and per-user undo. Over MCP these become optional envelope args
  `{ session?, actor? }` forwarded by the sidecar into the client's headers. LB does
  NOT map its own identity onto CE actors in v1 (CE actors are a per-editor-tab
  concept); record as an open question for a later "LB principal → CE actor" mapping.

## Testing / exit gate

- Per-verb: happy path against `ce_fake` (assert the trait call + args), the deny
  test (no grant → denied before any trait call), and arg-validation failures
  (bad `NodeKey`, unknown appliance → not-found).
- One **routed write**: `ce.patch` via the two-node harness (S4) — proves writes
  cross the hop with the workspace claim re-check.
- Fail-loud on unreachable appliance for a write (`ce.patch` → error, nothing queued)
  — the parent scope's no-outbox decision, tested on the write path where it matters.
- Real-engine (opt-in tier): one scripted flow — add two `math::add` nodes, wire an
  edge, patch an input, `call-action`, `remove-node`, `ce.tree` reflects each step.
- **Exit gate:** all seven verbs green local + `ce.patch` green routed + deny matrix
  complete (`cargo test --workspace`).
