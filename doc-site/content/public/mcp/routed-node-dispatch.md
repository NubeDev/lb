# Routed dispatch — calling a tool on a *named* node

An MCP tool call normally addresses an **extension**: `modbus.device.add` runs wherever `modbus`
lives. That is enough while an extension lives on exactly one node. It stops being enough the
moment a **fleet** runs the same extension — ten gateways each running `modbus` — because then
"where does this call run?" has ten answers.

Routed dispatch lets a caller say **which node**, and makes the ambiguous case an error instead
of a guess.

## The problem it removes

Before this, every node hosting an extension answered on the same bus key, and the caller kept
whichever reply arrived **first**. With two gateways, a call meant for gateway A could run on
gateway B, return success, and leave you with a binding pointing at the wrong physical box.
Nothing detected it.

Measured on two real nodes: 40 identical calls, **25 landed on one node and 15 on the other**.
No error either way.

Two interchangeable replicas would make that harmless. Two gateways are not replicas — they are
two distinct physical things, wired to different equipment. So picking one for you is never
correct, and the fix is to refuse.

## Calling a specific node

```rust
use lb_bus::NodeId;

let node = NodeId::new("node:gw-01")?;
let out = lb_mcp::call_on_node(
    &registry, &bus, &principal, ws,
    "modbus.device.add", input_json,
    &node,
).await?;
```

The call runs on `gw-01` or fails. It never falls back to another node hosting `modbus` —
a fallback would reintroduce the exact bug above.

Untargeted calls are unchanged:

```rust
lb_mcp::call(&registry, &bus, &principal, ws, "hello.echo", input_json).await?
```

If one node hosts `hello`, this resolves and runs exactly as it always did — same path, no extra
bus hop, no added lookup. If *several* do, you get `Ambiguous` (below).

## The three routing errors

| Error | Means | HTTP |
|---|---|---|
| `Ambiguous { ext, candidates }` | You didn't name a node and several host this extension | **409** |
| `NodeUnreachable { node }` | The node you named isn't reachable in this workspace | **503** |
| `NodeTooOld { node }` | The node is up but predates routed dispatch | **502** |

`Ambiguous` carries the candidate node ids as **data**, not prose, so a caller can react:

```rust
match err {
    ToolError::Ambiguous { candidates, .. } => {
        // ask the operator which gateway, or pick by your own policy
    }
    ToolError::NodeUnreachable { node } => { /* render offline; write nothing */ }
    _ => {}
}
```

`NodeUnreachable` is a **refusal, not a queue**. A provisioning call must not be silently
deferred, or you record work that has not happened. If you want deferral, use the outbox
explicitly.

## Naming a node is not permission to use it

A targeted call authorizes exactly like an untargeted one: `mcp:<ext>.<tool>:call`. There is no
per-node grant, and no way to reach further by naming a node — **addressing is not
authorization**. Encoding the node into the tool name (`modbus@gw-01.device.add`) was rejected
for exactly this reason: grants would multiply by fleet size, and a new gateway would need new
grants to do what it is already allowed to do.

Authorization also runs **before** the node is looked at. A caller without the capability gets
`Denied` — identical whether or not the node it named exists — so the error cannot be used to
enumerate your fleet.

Once past that gate, `Ambiguous` *does* list candidate nodes to an authorized caller. That is a
deliberate trade: such a caller could discover them by targeting anyway, and an actionable error
is the point.

## Node ids

A `NodeId` is validated when constructed and cannot contain `/`, `*`, `$`, `?`, or `#`:

```rust
NodeId::new("node:gw-01")?;   // fine — `:` is not structural in a bus key
NodeId::new("gw-*")           // rejected
```

This is not cosmetic. Node ids become a **segment of a bus key**, so `gw-*` would read as one box
while addressing every node that matched it. Validating at construction means the id can be used
raw, with no encoding layer that two call sites could implement differently.

Ids must also be **stable across restarts** — that comes from your deployment config. A node
boots with a random id, which is fine for tests and solo nodes but would make a restart look like
a brand-new node to a fleet roster.

## Workspace isolation

The node-qualified key is declared **per workspace a node serves**. So a call from workspace B
does not merely get refused by a node serving only workspace A — it has nowhere to land, because
that node declared no matching key. The wall is the key space itself.

## What is not here yet

- **`NodeTooOld` is defined but not yet returned.** It needs a flag in the node presence payload
  saying the node speaks targeted dispatch. Until fleet presence ships that, an old node reads as
  `NodeUnreachable` — reachable but silent looks the same as absent.
- **Discovery.** Which nodes host which extension is currently supplied by explicit wiring, not
  learned from live presence. Targeting works; automatic fleet discovery is fleet presence's job.

## See also

- Capabilities and the `mcp:<ext>.<tool>:call` grammar — unchanged by this feature.
- Fleet presence — node identity, the roster, and the announcements that will feed discovery.
