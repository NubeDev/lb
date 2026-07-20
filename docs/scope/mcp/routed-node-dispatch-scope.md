# MCP scope — routed dispatch to a NAMED node

Status: **BUILT 2026-07-20** (all eight open questions resolved below; the `NodeId` prerequisite was
minted in this build, owned by fleet-presence). Session:
[`../../sessions/mcp/routed-node-dispatch-session.md`](../../sessions/mcp/routed-node-dispatch-session.md).
Public docs: [`doc-site/content/public/mcp/routed-node-dispatch.md`](../../../doc-site/content/public/mcp/routed-node-dispatch.md).

**Not yet load-bearing in production:** the guard's candidate set is still populated only by
explicit wiring — the ext-hosting announce that would feed it from live liveliness is
fleet-presence's, as is the `targeted_dispatch` flag that `NodeTooOld` needs. Both are named in
Findings A/B below. The guard is *armed before the hazard is reachable*, which was the point.
Closes the open question already standing in [`mcp-scope.md`](mcp-scope.md) §"What shipped in S3"
(line 96: *"Routing tie-breaks when two nodes host the same extension — still open"*) and unblocks
[lb#81](https://github.com/NubeDev/lb/issues/81).

MCP routing today addresses an **extension**, never a **node**: `call_key(ext)` is
`mcp/{ext}/call`, and the registry maps `ext_id → exactly one Target`. That was correct while an
extension lived on exactly one node. It stops being correct the moment a **fleet** runs the same
extension — ten CM4 gateways each running `modbus` — because there is then no way to say *which*
box a call is for. This scope adds the target node as ordinary call data: **"call tool T on node N,
subject to the caller's caps."** It is the seam a supervisor needs to provision an edge appliance,
and the last platform blocker under ems's gateways scope slice 2.

## The failure is silent, not loud — this is the real severity

The issue title says a fleet is *unaddressable*. The code is worse than that: it is **silently
misaddressable**. `lb_bus::query` (`rust/crates/bus/src/query.rs:111`) takes the first reply and
says so in a comment that encodes the very assumption this scope breaks:

```rust
// Take the first successful reply; a routed tool call has exactly one responder.
```

Every node hosting `{ext}` declares a queryable on the same workspace-relative key
(`rust/crates/host/src/serve.rs:37`). So with two gateways, both answer, and the caller keeps
**whichever replied first** — no error, no ambiguity signal, nondeterministic per call. A
supervisor calling `modbus.device.add` for gateway A can provision gateway B, get a success reply,
and write a binding that points at the wrong physical box. Nothing in the current code can detect
that this happened.

That reframes the work: this is not only a missing feature, it is a **correctness hazard that arms
itself the moment a second node hosts an existing extension**. The fan-in guard (below) is
therefore not a nice-to-have alongside addressing — it is the part that must land even if
addressing slipped.

> **Corrected 2026-07-20 (implementing session, entry-gate audit).** The hazard is **latent, not
> active**: neither `serve_ext` nor `register_remote_extension` has any production caller, so no
> production path can reach the fan-in key today (see Open questions → Finding A). The urgency is
> therefore *ordering*, not firefighting — the guard must land **before** fleet-presence's
> discovery wires a second host and loads the gun. The nondeterminism is real and demonstrated
> against real nodes on the real bus, but in the library seam; it is not evidence of a live
> production misprovisioning, and must not be cited as one.

## Goals

- **Address a node as call data.** A caller may name a target node on an MCP call; the target is a
  value on the call, never an `if node == …` branch and never a new per-node tool name (rule 10).
- **Make the ambiguous case an ERROR, not a coin flip.** When a call could be served by more than
  one node and the caller did not disambiguate, refuse with a structured, machine-readable error
  naming the candidates. Never silently pick one.
- **Unaddressed calls keep working, unchanged.** The overwhelming majority of calls target an
  extension hosted once. Those must not gain a node field, a lookup, or a latency cost — this is
  additive, and the existing S3 routing path stays the fast path.
- **One authorization story.** A routed call carries the *same* capability grammar
  (`mcp:<ext>.<tool>:call`); naming a node adds no new grant surface and no way to widen reach.
- **Honest failure when the node is gone.** Targeting a node that is not connected returns a
  distinct, structured refusal — never a hang, never a half-write, never a fallback to "some other
  node that also hosts this ext" (the fallback is exactly the silent-misprovision bug).

## Non-goals

- **No node registry, presence roster, or liveness plane.** That is
  [`node-roles/fleet-presence-scope.md`](../node-roles/fleet-presence-scope.md), and it is a
  **prerequisite, not part of this scope** (see Risks — `NodeId` does not exist in code yet).
- **No remote control of nodes** (restart/drain/evict/OTA). fleet-presence explicitly defers it;
  we defer with it. This scope routes *tool calls*, nothing else.
- **No serve-side re-authorization / token-on-the-bus.** Today the calling node authorizes and the
  workspace-prefixed bus key is the wall (`mcp/src/serve.rs` deliberately does not re-check). That
  is a real open gap, already named in [`../sync/sync-scope.md`](../sync/sync-scope.md) §Risks —
  it is **not widened here**, and it is flagged loudly below rather than quietly inherited.
- **No cross-workspace routing.** A node is addressable only within the workspace whose key space
  it answers on. The wall is unchanged and untouchable.
- **No node-targeting for host-native verbs.** `rules.*`, `series.*`, `grants.*` etc. dispatch by
  name prefix in `host/src/tool_call.rs` and are always local; giving them a node target is a
  separate ask with a different shape. State it and move on.
- **No load balancing / failover / replica sets.** "Two nodes host this ext" means *two distinct
  physical things*, not two interchangeable replicas. Picking one for the caller is precisely the
  behaviour being removed.

## Intent / approach

**The target node is a field on the call, resolved at one chokepoint.** Three places hardcode
ext-only addressing, and all three are narrow:

| File | Today | Change |
|---|---|---|
| `mcp/src/route.rs:16` | `call_key(ext) -> "mcp/{ext}/call"` | add a node-qualified key alongside it |
| `mcp/src/registry.rs:123` | `reachable: HashMap<String, Target>` — `ext_id → ONE Target` | one ext id must be able to resolve to N node-bound targets |
| `mcp/src/call/resolve.rs:12` | splits `<ext>.<tool>`, yields an ext | resolve `(ext, tool, node?)` → an unambiguous target |

The key idea: **`Target::Remote` learns a node identity.** Today it is
`Remote { tools: Vec<ToolDescriptor> }` — it means only "not here, go on the bus," which is
exactly why a second host is not even representable (`register_remote_descriptors` overwrites the
prior entry for the same ext id). Carrying the node makes the multi-host case expressible, and
once it is expressible it can be *checked*.

Addressing rides the bus key, not the payload: a node-targeted call goes to
`mcp/{ext}/{node}/call`, which only that node declares. This keeps the "exactly one responder"
property that `lb_bus::query` already assumes **true by construction** for targeted calls, rather
than restating the assumption in a comment and hoping. Every node hosting an ext declares *both*
its node-qualified key and (as today) the shared key.

**Prefer the node key for ALL remote dispatch, not just targeted calls.** Once `Target::Remote`
carries a node, resolve *always* knows the node — even for an untargeted call to a singly-hosted
ext. So dispatch can always use the node-qualified key, and the shared key stops carrying calls
entirely: it survives only as a transitional path for mixed-version fleets (an old caller that
predates this change). This closes the residual hazard below (a caller with a partial registry
resolving "unambiguously" and still coin-flipping on the shared key). Belt-and-braces on top,
cheap and worth doing: `lb_bus::query` keeps listening briefly after the first reply and returns
an error on a **second** responder — turning the "exactly one responder" comment into a runtime
check at the call site. (This is call-site detection, not the rejected serve-side guard — no
cross-node coordination is involved.)

**Rejected: a node field inside `CallRequest`.** It leaves every host answering the shared key,
so all N nodes still receive, deserialize, and must each decide whether the call is theirs — the
fan-in stays real, the wasted work scales with fleet size, and correctness depends on every node
implementing the discard identically. Routing in the key makes the network do the addressing.

**Rejected: encoding the node in the tool name** (`modbus@gw-01.device.add`). It breaks the
capability grammar — `mcp:<ext>.<tool>:call` would become per-node, so grants would multiply by
fleet size and a new gateway would need new grants to do what it already may do. Addressing is not
authorization; conflating them is the leak.

**The ambiguity guard is the load-bearing half.** With the registry able to hold N targets for an
ext, `resolve` gains exactly one new outcome: *ambiguous*. An untargeted call to a singly-hosted
ext resolves as it does today (unchanged fast path); an untargeted call to a multiply-hosted ext
returns a structured `Ambiguous { ext, candidates: [node…] }` — a **new `ToolError` variant**, not
a generic string, so a caller (ems's `Provisioner`) can react rather than parse prose. This
converts today's nondeterministic silent wrong answer into a loud, actionable one, and it is the
part worth shipping first.

## How it fits the core

- **Tenancy / isolation:** load-bearing, and **the doc's first draft overstated it — corrected
  here (peer review 2026-07-20).** The serving queryable today is **workspace-wildcarded**:
  `serve_ext` (`rust/crates/host/src/serve.rs:37`) declares `ws/*/mcp/{ext}/call` so one
  queryable serves every workspace, and the answer loop recovers the concrete ws from the key it
  arrived on. So the key space is **not** the wall on the serving side — a ws-B call *does* reach
  a ws-A node's queryable, and the wall is that the answer loop resolves only the arriving
  workspace's instance. The build must **decide the wall's shape for node-qualified keys**: (a)
  declare the node key **per workspace served** (a real key-space wall, at the cost of N tokens
  for a hub serving N workspaces — mirrors fleet-presence's per-ws announce), or (b) keep the
  wildcard and make the wall "the serving node refuses a ws it does not serve" — a weaker,
  different claim that the isolation test must then match. Do not write the test to assert
  "ws-A's node never observes the call" unless (a) is chosen; under (b) that assertion is false
  by design. Node ids are *not* a new reach dimension either way — naming a node you cannot
  reach fails at the same wall as today.
- **Capabilities:** **no new grammar.** A targeted call authorizes as `mcp:<ext>.<tool>:call`, on
  the calling node, before resolve — the existing order in `call/mod.rs:79-88` (authorize →
  resolve → dispatch) is preserved exactly, so a `NotFound`/`Ambiguous` is still never observable
  by an unauthorized caller. **Deny path:** capless caller → `Denied` (before any node lookup, so
  the fleet's shape leaks nothing); unknown/disconnected node → `NodeUnreachable`; untargeted call
  to a multiply-hosted ext → `Ambiguous`. **Stated trade-off:** `Ambiguous` lists candidate node
  ids to any caller holding a cap on *one tool* of that ext — a deliberate, accepted leak of fleet
  shape to *authorized* callers (they could probe by targeting anyway, and the actionable error is
  the point). The line held is against **unauthorized** enumeration, hence authorize-before-resolve.
- **Placement:** either. This is peer-to-peer over the bus; a supervisor may be cloud or a
  head-end appliance, and the addressed node may be either. No `if cloud` — role stays config
  (`Node::boot_as`), per symmetric nodes.
- **MCP surface (§6.1):** **no new tools.** This is a change to how *existing* tools are
  addressed, so CRUD / get-list / live-feed / batch are all N/A as new verbs — deliberately. The
  surface change is an optional `node` on the call path plus two new `ToolError` variants.
  *Get/list of what is addressable* is fleet-presence's `nodes.list`, not ours. Batch is N/A: one
  call, one node. If a future caller needs "this call against all N gateways", that is a fan-out
  built **on** this seam (and per §6.1 a long one must be a job) — not part of it.
- **Data (SurrealDB):** **none.** Addressing is motion, not state. Which node hosts what is
  derived from live queryables/presence at call time; persisting a node↔ext table would go stale
  exactly when it matters (a gateway swapped at a switchboard) and would recreate the
  durable-registry design fleet-presence already rejected.
- **Bus (Zenoh):** new key `mcp/{ext}/{node}/call`, message class **must-deliver, request/reply**
  (a queryable, like today's). The existing `mcp/{ext}/call` stays for untargeted calls.
- **Sync / authority:** unchanged. This routes a call; it does not move authority. An edge node
  remains authoritative for its own data per [`../sync/sync-scope.md`](../sync/sync-scope.md).
  Offline: an addressed node that is not connected is a **refusal, not a queue** — a provisioning
  call must not be silently deferred, or the caller writes a binding for work that has not
  happened. (If a caller wants deferral, that is the outbox, explicitly.)
- **Secrets:** none introduced. Node identity is an id, not a credential; the node's own token/cert
  live on the node per edge-trust.
- **Stateless extensions / hot-reload:** unaffected — an ext still holds no durable state, and a
  reload re-declares its queryables (both keys) as it does today.
- **SDK/WIT impact:** **none expected, and this must be verified in the build.** The WIT call-tool
  boundary is per-extension and node-agnostic — the addressed node dispatches locally through the
  unchanged path. If threading a node target turns out to touch the guest ABI, **stop and flag
  it**: that is a stable-boundary change and a different, larger conversation.
- **Skill doc:** **N/A as a new skill.** No new drivable verb ships. `skills/core.mcp` (if/when it
  covers routed calls) gains a section on targeting and on the two new errors; the implementing
  session owns that edit rather than a new `skills/<name>/SKILL.md`.

## Example flow

1. A workspace has one cloud supervisor and two gateways, `node:gw-01` and `node:gw-02`, **both
   running the `modbus` extension**. Each declares `mcp/modbus/call` (shared) and
   `mcp/modbus/{its own node id}/call` (targeted).
2. The supervisor's ems extension stamps a meter onto a network under `gw-01`. Its `Provisioner`
   issues `modbus.device.add` **targeted at `node:gw-01`**.
3. `authorize` checks `mcp:modbus.device.add:call` for the caller, on the supervisor, exactly as
   today — before any node is looked at.
4. `resolve` sees an explicit target, finds `gw-01` among `modbus`'s reachable targets, and returns
   it unambiguously.
5. `dispatch` queries `ws/acme/mcp/modbus/node:gw-01/call`. Only `gw-01` declares that key, so
   there is exactly one responder — by construction, not by assumption. The device is created on
   the correct physical box.
6. A second call is made **without** a target. Because `modbus` is hosted by two nodes, resolve
   returns `Ambiguous { ext: "modbus", candidates: ["node:gw-01", "node:gw-02"] }`. The caller gets
   a loud, structured error instead of today's coin flip.
7. `gw-02`'s WAN drops. A call targeted at it returns `NodeUnreachable { node: "node:gw-02" }` —
   the supervisor renders the gateway as offline and writes nothing. It does **not** fall back to
   `gw-01`.
8. Meanwhile a single-node install calls `hello.echo` with no target. One host, one target, the
   shared key, the same code path as before this scope existed.

## Testing plan

Real store, real bus, real nodes via `Node::boot_as(role)` — no mocks, no fake transport
(`testing-scope.md` §0). The two-node substrate this needs already exists (two in-process Zenoh
peers auto-discover, per `sync-scope.md`), so the fleet case is testable **today**, before any
hardware.

Mandatory categories that apply:

- **Capability deny-tests (mandatory):** a capless caller targeting a node → `Denied`, and — the
  sharp one — **the deny must be indistinguishable whether or not the named node exists**, so the
  error cannot be used to enumerate a fleet.
- **Workspace-isolation (mandatory):** a caller in ws-B naming a node that serves only ws-A →
  refused, and nothing is executed or written on the ws-A node. **Match the assertion to the
  wall shape chosen above:** if the node key is declared per-workspace, additionally assert the
  ws-A node never observes the call (key space is the wall); if the queryable stays
  ws-wildcarded, the honest assertion is refusal-without-effect, not non-observation — the
  current `serve_ext` wildcard means the call *is* observed and must be refused downstream.
- **Hot-reload (mandatory):** reload an ext on a targeted node; assert both its keys are
  re-declared and in-flight targeting recovers.
- **Offline/sync:** a targeted node disconnected → `NodeUnreachable`, promptly (a bounded wait, not
  the query's default timeout), and **nothing is written** on either side.

Key cases beyond the mandatory set:

- **The regression that motivates this, pinned:** two nodes hosting the same ext, an untargeted
  call → `Ambiguous` listing both. Assert it **fails before this change** (the current code
  answers nondeterministically) — a test that cannot fail on the old code is not a regression test.
- **Nondeterminism, proven not argued:** with two hosts and the old shared-key path, N sequential
  untargeted calls do not all land on the same node. This is the evidence that "first reply wins"
  is a real hazard; run it enough times to be convincing, and keep it as the documented rationale.
- **The unchanged fast path:** single-host untargeted call resolves `Local`/`Remote` exactly as
  today, with no new bus hop and no registry lookup added. Guard against a regression in the
  common case.
- **Ordering:** authorize strictly precedes resolve — a capless caller on an ambiguous ext gets
  `Denied`, never `Ambiguous` (otherwise resolve leaks fleet shape to unauthorized callers).
- **No routing loops:** a targeted call arriving at the wrong node is refused, never re-routed
  (`serve.rs` already refuses a misroute — extend the invariant to node targets).
- **Self-targeting:** a node addressing itself resolves `Local` with no bus hop.

## Risks & hard problems

- **`NodeId` does not exist yet — this scope has a hard prerequisite.** A grep across `rust/` for
  `declare_node_presence` / `nodes.list` / `nodes.watch` / a `NodeId` type returns **zero hits**;
  what exists is *channel-member* presence (`bus/src/presence.rs`, key `ws/{id}/presence/{member}`)
  and `Role {Edge,Hub,Solo}`. So fleet-presence is not merely a sibling — **you cannot address a
  node by an identity the platform does not mint.** Either fleet-presence's identity half lands
  first, or this scope's build begins by minting the durable `NodeId` *in coordination with it*.
  Do not let two scopes each invent a node identity; that fork would be very expensive to unpick.
  **This is the first thing to settle before estimating.**
- **Serve-side authorization is inherited, not solved.** A routed call is authorized on the
  *calling* node; the workspace key is the only wall on the serving side. That is defensible
  within one workspace and is the status quo — but this scope makes routed calls
  **much more common** (every fleet provisioning call), which raises the stakes on an
  already-open gap (`sync-scope.md` §Risks: token-on-the-bus). Naming it here so it is a decision,
  not a drift. If token-on-the-bus lands, targeted calls should be its first consumer. (For the
  Niagara-minded: Fox authenticates at the *receiving* station; lb today authorizes only on the
  calling node. This is the one dimension where the design is **not** yet Fox-equivalent — and
  fleet provisioning is exactly the traffic that most wants serve-side auth.)
- **The shared key cannot simply be deleted — but it can stop carrying calls.** Keeping both keys
  means a multiply-hosted ext still has a fan-in key that any old caller can hit. Mitigate by
  dispatching **all** remote calls on the node-qualified key once resolve knows the node (see
  Intent), demoting the shared key to a mixed-version transitional path. The `Ambiguous` guard
  itself must live at **resolve on the calling side** (where the candidate set is known) — a
  serving-side guard cannot work, since each node only knows about itself and would have to
  coordinate to detect the duplicate. Getting this backwards produces a guard that silently
  never fires.
- **The ambiguity guard has NO data source today — this is a third hard dependency, not a
  freshness footnote.** Resolve can only refuse what it can see, and today the calling node's
  registry learns of remote hosts by **manual wiring only**: `register_remote_extension`
  (`rust/crates/host/src/remote.rs`) says outright "passed in by the wiring layer; a
  discovery/registry flow lands at S4/S7", and nothing in the codebase populates a second host.
  So a guard built on the registry as-is will compile, pass tests that hand-wire both hosts, and
  then **silently never fire in production** — exactly the failure mode this scope condemns in
  the serve-side alternative. The build needs an **ext-hosting announcement** the calling side
  can derive candidates from — plausibly a liveliness token at `ws/{id}/nodes/{node}/ext/{ext}`,
  the keyspace fleet-presence's open questions already gesture at but **neither scope currently
  owns**. Settle ownership explicitly (recommendation: fleet-presence mints the keyspace since it
  owns node liveliness; this scope consumes it), and derive candidates from live liveliness
  rather than a cached map, for the same stale-online reasons fleet-presence rejected a durable
  registry.
- **Mixed-version fleets get a misleading refusal.** An old-version gateway that is online but
  predates this change never declares its node-qualified key, so a new supervisor targeting it
  gets `NodeUnreachable` — "unreachable" when the truth is "reachable but does not speak
  targeting." For a scope whose pitch is honest failure, that is a dishonest error during every
  rolling upgrade. See open question 7.
- **`NodeUnreachable` detection mechanics need a decision, not just a bound.** A Zenoh `get`
  against a key with zero matching queryables completes quickly (routing knows nothing matches);
  a declared-but-hung node runs to the query timeout. "A bounded wait" (testing plan) does not by
  itself distinguish offline from slow — the likely answer is consulting the fleet-presence
  liveliness roster, which introduces a check-then-dispatch race (node drops between the roster
  read and the query) that still ends in a timeout and must be handled, not assumed away. See
  open question 8.
- **ems is waiting on exactly this, and will be tempted to route around it.** ems's gateways
  scope slice 2 is blocked on this seam and only on this seam. The pressure to "just have ems dial
  the box's IP meanwhile" is the rule-10 violation both scopes exist to refuse — the fix belongs
  here, generically, once.
- **Series naming across nodes** (flagged by ems, verify before a two-gateway install):
  `modbus.<net>.<dev>.<point>` must stay unique when several gateways sync into one hub store.
  Likely already fine, but a collision here corrupts dashboards silently — the same failure class
  as the routing bug above, one layer up.

## Open questions — all RESOLVED 2026-07-20 (implementing session)

> Resolved during the build's Phase 0, after an entry-gate audit of every routed-path caller.
> Two findings reframed the answers and are recorded first, because several questions below
> depend on them. Each answer records **why**, not just what — the alternative rejected and the
> reason, per the repo's doc conventions.

### Finding A (Phase 0) — the routed path has ZERO production wiring

The scope's discovery risk says remote registration is hand-wired. Verified: it is worse, and
symmetrically so. **Neither half** of the routed path has a non-test caller.

| Seam | Production callers | Test callers |
|---|---|---|
| `register_remote_extension` (calling side, `host/src/remote.rs:13`) | **0** | 3 |
| `serve_ext` (serving side, `host/src/serve.rs:30`) | **0** | ~12 |

(`grep serve_ext` outside tests hits only `serve_ext_ui`, an unrelated static-file route.) So
`Target::Remote` is never constructed in production, and no production caller can reach the
fan-in key. **Consequence for severity:** the "supervisor provisions the wrong physical box"
scenario is **latent, not active** — it cannot happen today because nothing wires a second host.
It arms itself the instant fleet-presence's discovery populates a second `Target::Remote`.

This does not weaken the case for the guard; it **sharpens the ordering**. The guard must land
*before* discovery makes the hazard reachable, not after. The severity section above is corrected
accordingly — the argument is "land the guard before the gun is loaded," not "stop an ongoing
production bug." Claiming the latter would misstate what the evidence shows.

**It also bounds what the nondeterminism test can prove.** Two responders require two `serve_ext`
calls, and only a test can make those. The test therefore demonstrates the defect **in the
library seam** — real nodes, real bus, real coin flip — but it is not evidence of a live
production misprovisioning, and this session does not present it as such (rule 9: work must not
*look* done past what was actually shown).

### Finding B (Phase 0) — `NodeId` does not exist, and nothing can stand in for it

Confirmed zero hits for `NodeId`, `declare_node_presence`, `nodes.list`, `nodes.watch`. Every
candidate for an existing stable per-node string fails:

- `Node.key` — `SigningKey::generate()` per boot (`boot.rs:126,149,171`): not stable across a
  restart, and a **secret** that must never appear in a key expression.
- `gateway_url` — `None` on any headless node.
- Zenoh peer ZID — unreachable; `cross_node_routing_test.rs:73-77` records that
  `Session::info().locators()` sits behind zenoh's `unstable` feature the workspace declined.

**Decision: this build mints `NodeId`, placed as fleet-presence's primitive, not mcp's.** The
scope permits either ("or this scope's build begins by minting the durable `NodeId` *in
coordination with it*"). Deferring entirely was the cleaner-looking option and was **rejected**:
it blocks #81 behind a slice nobody has started, and ems's gateways slice 2 behind that. The fork
both docs warn about is avoided structurally — the type lives in the crate fleet-presence will
own and is designed to its stated constraints (key-safe charset, stable across restart), so
fleet-presence **consumes what is already there** rather than minting a second one. The mcp crate
gets no private node identity (explicitly forbidden by the brief, and the right call).

### The eight

1. **Where does the node target ride on the caller's API?** **An explicit optional parameter**,
   not a `CallCtx` field — as recommended. A target is *call data*, not ambient context; putting
   it in ctx would make targeting invisible at the call site and let it be inherited accidentally
   by a nested call that meant to stay local. Visible-at-the-call-site wins.
2. **Node id in the key: raw or encoded?** **Raw, with the charset constrained at the mint.** No
   encoding layer — an encode/decode pair is a drift hazard between the caller's key and the
   serving node's declaration (the exact drift `route.rs` exists to prevent). Instead `NodeId`
   *cannot be constructed* with a key-unsafe character, so raw interpolation is always safe. Note
   the scope's own example (`node:7f3a…`) contains `:` — that is safe in a Zenoh key expression
   (only `/ * $ ? #` are structural), so `:` stays and ids remain readable.
3. **Does `Target::Remote` carry one node or many?** **One — `Remote { node, tools }`**, registry
   holding N targets per ext. As recommended, and for the stated reason: a plural `Target` is a
   `Target` that can still be ambiguous *after* resolve, which defeats the guard by construction.
   Ambiguity must be resolved at resolve or not at all.
4. **Must descriptors match across hosts?** **No — per-node fact.** A fleet mid-rolling-upgrade is
   normal operation; refusing a descriptor mismatch would make rolling upgrades impossible, which
   is a worse failure than the inconsistency it prevents. Resolve consults the *targeted* node's
   descriptors.
5. **May an ambiguous call auto-pick?** **Never.** That is the bug this scope exists to remove.
   The entry gate confirms this breaks nothing: **no test anywhere registers the same ext id on
   two nodes**, and no production caller can (Finding A). The behaviour change has an empty blast
   radius today — which is precisely why now is the cheapest moment to make it.
6. **What is the workspace wall's shape for the node-qualified key?** **Per-workspace
   declaration** (option (a)) — a real key-space wall. Rejected the `ws/*` wildcard because it
   makes the isolation guarantee a *downstream refusal* rather than a structural one, and this
   scope's whole thesis is preferring true-by-construction over true-by-assumption; keeping the
   wildcard here would contradict the same argument used to reject the payload-field design. The
   token cost the scope asked to measure first is N tokens for a hub serving N workspaces —
   accepted, and it mirrors what fleet-presence already announces per workspace, so the cost is
   shared rather than additive. **The isolation test therefore asserts the strong form** (the
   ws-A node never observes the call). Note the pre-existing shared key stays ws-wildcarded;
   this decision governs the *node-qualified* key only.
7. **How does a mixed-version fleet fail honestly?** **A `targeted_dispatch: bool` in the presence
   payload**, yielding a distinct `NodeTooOld` refusal instead of a lying `NodeUnreachable`. As
   recommended: cheap now, impossible to retrofit honestly later (an old node cannot be taught to
   announce a flag retroactively, so the ambiguity would be permanent).
8. **How is "unreachable" actually detected?** **Zero-matching-queryable fast-fail as the primary
   signal, with a bounded wait as the backstop** — and the roster consulted only to *classify*
   the refusal (offline vs. too-old), never as the gate. Rejected roster-check-before-dispatch as
   the primary mechanism: it introduces the check-then-dispatch race the scope names and *still*
   ends in a timeout, so it adds a race without removing the case it was meant to remove. Dispatch
   first, classify the failure second — the race then has no window, because the dispatch attempt
   is itself the liveness test.

### Consequent scope corrections

- The severity framing is corrected per Finding A (latent, not active) — see that finding.
- Fleet-presence is amended in the same session to record that `NodeId` is minted here to its
  constraints, so the two docs cannot fork.

## Open questions (original, for the record)

Specific and answerable during the build:

1. **Where does the node target ride on the caller's API?** An optional argument on
   `lb_mcp::call*`, or a `CallCtx` field? The ctx already threads through `call_with_ctx`, which
   argues for ctx — but a target is call data, not ambient context, which argues against.
   Recommendation: an explicit optional parameter, so targeting is visible at the call site.
2. **Node id in the key: raw or encoded?** `node:7f3a…` contains a `:` — confirm it is safe and
   readable in a Zenoh key expression, or define an encoding. Decide before the key shape is
   frozen; changing it later churns every declared queryable. **The charset constraint really
   belongs to fleet-presence** (it mints the id): a `NodeId` must be key-expression-safe — no
   `/`, no `*` / `$` / `?` / `#` — and that requirement is now recorded there too, so an id shape
   is never frozen that cannot ride a key.
3. **Does `Target::Remote` carry one node or many?** Either `Remote { node, tools }` with the
   registry holding a `Vec<Target>` per ext, or `Remote { hosts: Vec<(node, tools)> }`. The first
   keeps a target singular (better for dispatch); the second keeps the map one-entry-per-ext
   (smaller diff). Recommendation: the first — a `Target` that is plural is a `Target` that can be
   ambiguous downstream of resolve, which defeats the guard.
4. **Do descriptors have to match across hosts?** If `gw-01` runs modbus v2 and `gw-02` v1 and
   their tool lists differ, is that an error at registration or a per-node fact resolve consults?
   Recommendation: per-node fact — a fleet mid-upgrade is normal, and refusing it would make
   rolling upgrades impossible.
5. **Should the untargeted-but-ambiguous case ever be allowed to auto-pick?** Recommendation:
   **never** — that is the bug. But confirm no existing internal caller relies on the current
   behaviour before making it an error (a grep for routed callers is part of the build's entry
   gate).
6. **What is the workspace wall's shape for the node-qualified key?** Per-workspace declaration
   (real key-space wall, N liveliness-era tokens for a hub serving N workspaces) vs the current
   `ws/*` wildcard plus downstream refusal (weaker claim, smaller diff). See the corrected
   Tenancy bullet — the isolation test's assertion depends on this choice. Recommendation:
   per-workspace, matching how fleet-presence announces; but measure the token cost first.
7. **How does a mixed-version fleet fail honestly?** An old node that is online but does not
   declare its node key would read as `NodeUnreachable`. Should the fleet-presence token payload
   carry a "supports targeted dispatch" flag (or a min version) so resolve can return a distinct
   `NodeTooOld`-style refusal instead? Recommendation: yes, one boolean in the presence payload —
   cheap now, impossible to retrofit honestly later.
8. **How is "unreachable" actually detected?** Zero-matching-queryable `get` (fast completion) vs
   declared-but-hung node (query timeout) vs consulting the liveliness roster before dispatch
   (introduces a check-then-dispatch race that still ends in a timeout). Decide the mechanism and
   its bound explicitly; "a bounded wait" alone does not distinguish offline from slow.

## Related

- [`mcp-scope.md`](mcp-scope.md) — the tool layer this extends; §"What shipped in S3" line 96
  already names this exact gap as open. The pipeline files
  (`mcp/src/call/{resolve,dispatch}.rs`) are where the change lands.
- [`../node-roles/fleet-presence-scope.md`](../node-roles/fleet-presence-scope.md) — **the
  prerequisite, twice over**: mints the `NodeId` this scope addresses by (key-expression-safe,
  see open question 2), the roster that answers "which nodes could I target?", and — per the
  discovery risk above — the natural owner of the **ext-hosting announce**
  (`ws/{id}/nodes/{node}/ext/{ext}`) that gives the ambiguity guard its candidate set. Read
  together; do not fork node identity.
- [`../node-roles/node-connection-scope.md`](../node-roles/node-connection-scope.md) — the
  appliance↔hub connection a targeted call rides.
- [`../sync/sync-scope.md`](../sync/sync-scope.md) — the routed-call substrate, and the open
  token-on-the-bus question this scope inherits rather than solves.
- [`ems-provisioning-verb-shapes-scope.md`](ems-provisioning-verb-shapes-scope.md) — the confirmed
  wire shapes an out-of-tree native ext already calls; those are host-native and always local,
  which is why node targeting does not apply to them.
- [`../auth-caps/`](../auth-caps/) — the unchanged `mcp:<ext>.<tool>:call` grammar.
- **Downstream consumer:** ems `docs/scope/gateways/gateways-scope.md` slice 2 (remote gateways) —
  blocked on this and only on this; its `Provisioner` trait already isolates every routed call in
  one place, so adoption is a narrow change once this ships.
- Issue: [lb#81](https://github.com/NubeDev/lb/issues/81).
