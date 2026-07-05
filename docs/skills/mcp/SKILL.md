---
name: mcp
description: >-
  Call any Lazybones platform verb through the ONE MCP contract — the universal seam every capability
  is reached through (rule 7). Use when a task says "call an MCP tool", "invoke a host verb / an
  extension tool over `/mcp/call`", "why is my call denied?", "discover which tools I can run", "read
  the tool catalog", "what capability gates `<ext>.<tool>`", or "route a `<ext>.<tool>` call". Covers
  the `call(principal, "<ext>.<tool>", json) -> result<json, ToolError>` shape, the two front doors
  (`POST /mcp/call {tool, args}` and `lb call <tool> '{json}'`) onto the SAME dispatch, the
  `mcp:<ext>.<tool>:call` cap grammar (workspace-first, re-checked every call), the four `ToolError`
  variants, the **honest deny** (a denial leaks no tool existence — never retry it), and `tools.catalog`
  discovery. This is the contract itself, so a grounded caller knows how to drive platform verbs
  without reading the codebase. Siblings: `lb-cli` (the terminal front door), `extensions` (authoring
  the tools this calls).
---

# The MCP contract — one way to call everything

In Lazybones **every capability is an MCP tool**, named `<ext>.<tool>`. The UI, AI agents, other
extensions, the operator CLI — all reach a platform verb the **same way**, through one call shape
(README §3.7, §6.5). There is no "internal" un-gated path; there is no per-caller API. Learn this one
contract and you can drive the whole platform.

The contract, verbatim from the code (`rust/crates/mcp/src/call/`):

```
call(principal, "<ext>.<tool>", json_input) -> Result<json_output, ToolError>
```

`principal` carries the workspace + the caps (from the caller's token — never the request body).
`"<ext>.<tool>"` is the dotted tool name. `json_input` is the args. You get JSON back, or a
`ToolError`. Four phases run in order, one file each: **resolve** the name → **authorize** (the deny
gate) → **dispatch** to the hosting extension → shape the result/error.

## 1. The two front doors, one dispatch

You never call `lb_mcp::call` directly — you reach it through one of two doors, and **both funnel into
the exact same `call_tool` chokepoint** (`rust/crates/host/src/tool_call.rs`). Same authorize gate,
same dispatch, same denials.

**Browser / gateway / extension UI — `POST /mcp/call {tool, args}`:**

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)

curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"series.list","args":{}}'
# → { "series": [] }
```

**Terminal / scripts / offline — `lb call <tool> '{json}'`** (the CLI is a pure client of the same
`POST /mcp/call`; see the `lb-cli` skill):

```bash
lb call series.list '{}' -o json          # → { "series": [] }
lb call inbox.list '{"channel":"ops"}'    # args default to {} if omitted
```

The `tool` field and the `lb call <tool>` argument are the **same dotted name**. `args` (`{json}`) is
the tool's input. The workspace is **never** in the body — the server reads it from your token (the
hard wall, §6/§7). That is the whole surface: one `{tool, args}` envelope reaches every verb, with
zero per-verb wiring.

## 2. The capability grammar — `mcp:<ext>.<tool>:call`

Every call is gated **before dispatch** by exactly one capability string, formed from the tool name:

```
mcp:<ext>.<tool>:call
```

Real examples: calling `series.latest` needs `mcp:series.latest:call`; calling a `hello` extension's
`echo` tool needs `mcp:hello.echo:call`. A wildcard grant `mcp:hello.*:call` matches every `hello`
tool. The resource in the cap is the qualified tool name **verbatim** — from `authorize.rs`:

```rust
// rust/crates/mcp/src/call/authorize.rs
let req = Request::new(ws, Surface::Mcp, qualified_tool, Action::Call);
match check(principal, &req) {
    Decision::Allowed => Ok(()),
    Decision::Denied(_) => Err(ToolError::Denied),   // both gates collapse to one opaque Denied
}
```

Two things this encodes, load-bearing for a caller:

- **Workspace-first, then capability.** `caps::check` runs workspace isolation as gate 1, the
  missing-grant check as gate 2 — the **same** chokepoint the store, bus, and secrets use. MCP is not
  a special case; there is no un-checked back path (mcp scope, "Capability-first").
- **The wall is re-checked on EVERY call.** There is no session-level "unlock", no widening. A grant
  you had for one call does not carry; each call re-authorizes from the principal's caps. A
  "built-in" extension takes the exact same gate as a third-party one — `granted = requested ∩
  admin_approved`, nothing pre-approved (rule 10).

## 3. `tools.catalog` — discover what you may run

You don't have to guess a tool's existence. `tools.catalog` returns, for the calling principal in
this workspace, **only the tools whose call would itself be allowed** — each with its descriptor
(`{ name, title, group, input_schema }`):

```bash
lb call tools.catalog '{}' -o json
# → { "ws":"acme", "tools":[ { "name":"series.list", "title":"…", "input_schema":{…} }, … ] }
```

Gated by `mcp:tools.catalog:call` (workspace-first). The cardinal rule (`tools/catalog.rs`): the
catalog advertises a tool **only if the call itself would allow it** — it runs the **same**
`authorize_tool` gate `call_tool` runs, once per candidate, and keeps only the passing subset:

```rust
// rust/crates/host/src/tools/catalog.rs — one gate, two callers
if authorize_tool(principal, ws, &qualified).is_ok() { tools.push(d); }
```

So **a denied tool is simply absent** — never greyed, never listed-but-forbidden. The menu *is* the
permission model rendered. It can never offer a tool that then denies, nor hide one that would pass
(proven by `host/tests/tools_catalog_test.rs`:
`catalog_omits_a_tool_the_principal_cannot_call_no_existence_leak`).

## 4. Error shapes — and the honest deny

A call returns one of four `ToolError` variants (`rust/crates/mcp/src/call/error.rs`):

| Variant | Means | What a caller does |
|---|---|---|
| `Denied` | authorization failed — workspace isolation **or** missing capability | **Stop.** Not transient. See below. |
| `NotFound` | the tool name is malformed or not hosted here | Fix the name — but only an **already-authorized** caller ever sees this |
| `Extension(msg)` | the extension ran but errored / trapped | A real failure in the tool; read `msg` |
| `BadInput(msg)` | the args were not valid for the tool | Fix the `{json}` args and retry |

Over `POST /mcp/call` these surface as HTTP: `Denied` → **403**, `BadInput` → 4xx, `Extension` → 5xx.
Over `lb`, a deny prints `DENIED  mcp:<tool>:call` and exits **3**; bad input exits **2**.

### The one load-bearing behavior: the honest deny

**A `Denied` reveals nothing about whether the tool exists.** From `error.rs`, `Denied` deliberately
carries no detail — not which gate failed, not whether the tool is real. `authorize` runs *before*
`resolve`, so an unauthorized caller can never reach the `NotFound` that would confirm a tool's
existence. A missing capability, a foreign workspace, and a tool that isn't there all read
**identically** as forbidden/absent.

For a caller — especially an agent — this is a rule, not a nuance:

- **Never retry a `Denied` as if it were transient.** It is not a rate-limit or a blip; it is the
  server refusing. Retrying wastes turns and never succeeds. (`lb-cli` gotcha: "A DENY is not a bug.")
- **Don't infer existence from a deny.** You cannot tell "not allowed" from "no such tool". If a call
  "vanishes", check your **caps** and your **workspace** — not the tool's spelling.
- **Use `tools.catalog` to know what's reachable** (§3), rather than probing names and reading the
  denials — probing tells you nothing the catalog doesn't tell you honestly.

## 5. How `<ext>.<tool>` resolves — all opaque, one path

The name splits on the **first** `.`: everything before it is the extension/host id, everything after
is the tool (`resolve.rs`: `qualified_tool.split_once('.')`). Two dispatch families share the one
contract, and a caller treats them **identically** — same envelope, same gate, same errors:

- **Extension verbs** (`<id>.<tool>`) — a published extension's tools (e.g. `hello.echo`,
  `weather-panel.ping`). Resolved in the runtime `Registry` to the hosting instance (local or, across
  nodes, routed over the bus) and run via `lb_mcp::call`. The id is **opaque data** — the core never
  branches on a specific extension id (rule 10); swapping one extension for an equivalent forces no
  core change.
- **Host-native verbs** — the platform's own services over the embedded store/bus, dispatched by
  prefix in `call_tool` (`is_host_native`): `series.*`, `ingest.*`, `inbox.*` / `outbox.*`,
  `agent.*`, `dashboard.*`, `nav.*`, `flows.*`, `prefs.*`, `secret.*`, `channel.*`, `reminder.*`,
  `tools.*`, `store.query|write|…`, `undo`/`redo`, and more. These are host verbs, not registry
  components, so they need no extension.

Both go through the **same** authorize gate first — a host-native `series.latest` and an extension's
`hello.echo` are called the same way and denied the same way. That symmetry is the point of rule 7:
a caller learns one contract, not two.

## 6. Common mistakes — "why is my call denied?"

Because a deny is opaque (§4), most "it doesn't work" cases are one of these — check them in order:

1. **Missing the capability.** You need `mcp:<ext>.<tool>:call` for the *exact* tool. A dev-login
   member does **not** hold admin verbs (e.g. `prefs.set_default`, `nav.set_default`) — those deny
   for a member exactly like a non-existent tool. Check your token's caps; `tools.catalog` lists only
   what you can run.
2. **Wrong workspace.** The workspace comes from your **token**, never the body. A ws-A token calling
   a tool that acts on ws-B data is denied at gate 1 (isolation), and it reads identically to a
   missing cap. If you meant another workspace, log in to it (`lb login -w <ws>`), don't add it to the
   args.
3. **Retrying a deny.** A `Denied` is final (§4) — don't loop on it. Fix the cap or the workspace.
4. **Malformed args are `BadInput`, not `Denied`.** A bad `{json}` string, or args that fail the
   tool's declared `input_schema`, is a clean `BadInput` (exit 2 / 4xx) — fix the args, don't touch
   caps. A tool that declares no schema skips that check (additive).
5. **Sending `workspace` in the body.** It is ignored — the server always uses the token's workspace.
   `-w` on the CLI *selects a stored credential*, it never overrides the wall.
6. **Assuming a "built-in" is pre-approved.** It isn't. Every verb, built-in or third-party, goes
   through `granted = requested ∩ admin_approved`. If it denies, it denies.

## Non-negotiable rules this encodes

- **MCP is the universal contract** (rule 7). One `call(principal, name, input)` shape serves every
  caller; the UI, agents, extensions, and the CLI all use it. No caller gets a private path.
- **Capability-first** (rule 5). `authorize.rs` is the *only* gate to dispatch — remove it and there
  is no other way in. Workspace isolation is gate 1, the capability is gate 2.
- **Workspace is the hard wall** (rule 6). The workspace is the principal's, from the token, checked
  first, never body-supplied.
- **Core knows no extension** (rule 10). The `<ext>` id is opaque data through resolve → registry;
  the core never special-cases one. A built-in and a third-party tool take the identical path.

## Test it (the proofs, no mocks)

Rule 9: the contract is exercised for real (in-process node, real SurrealDB `mem://`, real caps):

- **Honest deny + no existence leak** — `rust/crates/host/tests/tools_catalog_test.rs`
  (`catalog_omits_a_tool_the_principal_cannot_call_no_existence_leak`): a principal lacking a tool's
  cap gets it **absent** from the catalog; lacking `mcp:tools.catalog:call` gets an opaque
  `ToolError::Denied`.
- **Bridge dispatch of a host-native verb** — `rust/crates/host/tests/catalog_mcp_test.rs` drives
  `tools.catalog` over the real `/mcp/call` path.
- **Deny + workspace-isolation on the routed call path** — the mcp scope's exit gate
  (`echo_without_grant` → `Denied` with no existence signal; cross-node `host/cross_node_routing_test`
  proves deny + ws-isolation survive routing).

## Related

- Scope (the contract's source of truth): `docs/scope/mcp/mcp-scope.md` (the four phases, the
  honest-deny rule, the exit-gate tests).
- The terminal front door: `docs/skills/lb-cli/SKILL.md` (`lb call`, exit codes, `-w` as a selector).
- Authoring the tools this calls: `docs/skills/extensions/SKILL.md` (manifest `request` caps, the
  `<ext>.<tool>` a published extension exposes, the two mandatory deny + isolation tests).
- Implementation: `rust/crates/mcp/src/call/` (resolve / authorize / dispatch / error),
  `rust/crates/host/src/tool_call.rs` (the `call_tool` chokepoint + `is_host_native` families),
  `rust/crates/host/src/tools/catalog.rs` (`tools.catalog`),
  `rust/role/gateway/src/routes/mcp.rs` (`POST /mcp/call`).
- Capability grammar: `docs/scope/auth-caps/` (the `mcp:<ext>.<tool>:call` resource form).
