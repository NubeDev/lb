# extensions scope — the host-callback ABI: a guest can call host tools (inbox/outbox/db/MCP)

Status: scope (the ask). Promotes to `public/extensions/extensions.md` once shipped.

Today a Tier-1 WASM extension is a **one-way box**: the WIT world (`sdk/wit/world.wit`) lets the host
*call into* a guest (`tool.call(name, input) -> json`) and lets the guest only `host.log(msg)`. A guest
**cannot read or write the platform** — no inbox/outbox, no store, no calling another MCP tool. So a
real extension backend (a producer that ingests samples, a reactor that resolves an inbox item, a tool
that reads a series and derives another) **cannot be written as a guest** — that logic has to live in a
host service instead, which defeats the point of shipping a backend in the extension. This scope adds
the **one** missing primitive that makes a WASM extension a first-class platform citizen: a
**host-mediated call-back** so a guest can invoke the *same* MCP tool surface the UI bridge already
reaches (`POST /mcp/call` → `lb_host::call_tool`), under the guest's **delegated, intersected
authority**, capability- and workspace-checked on every call. This is the §11.2 "forever" ABI change —
done once, deliberately, behind the existing `call_tool` chokepoint, so it adds **zero** new trust
surface beyond what the browser bridge already has.

## Goals

- A guest can call **any host-native or extension MCP tool it is granted** — `series.*`, `ingest.write`,
  `outbox.status`, `inbox.list`/`resolve`, and other extensions' `<ext>.<tool>` — from inside
  `tool.call`, getting JSON back, exactly as the page's `bridge.call(tool, args)` does.
- The call is **authorized at the host**: workspace-first, then `mcp:<tool>:call`, against the
  **guest's effective principal** = `caller ∩ extension-grant` (delegation, never widening). A guest can
  never reach a tool its install grant didn't include, even if its caller could.
- **Identity reaches the instance.** The runtime carries the caller's principal + workspace into
  `HostState` for the duration of a `call_tool`, so the host-callback knows *who* and *which workspace*
  to authorize — the gap that blocks this today (`HostState` is identity-less).
- **One ABI addition, versioned forever.** `world extension` gains exactly one import:
  `host.call-tool(name, input-json) -> result<string, tool-error>`. WIT minor bump `@0.2.0`; the loader
  already checks world major, so `0.1.0` guests keep loading.
- The reference extension (`proof-panel`) gets a **real backend tool that uses it** — e.g.
  `proof.derive` reads a series via the callback and writes a derived sample — so the demo proves a
  guest doing real platform work, not just echoing input.

## Non-goals

- **No new tool surface.** The callback dispatches the *existing* verbs through the *existing*
  `call_tool`. It does not add CRUD/list/watch verbs — those are each their own scope.
- **No raw store/bus handle in the guest.** A guest never gets a `Store` or Zenoh handle (rule 5:
  everything host-mediated; rule 4: stateless). DB access is *only* via host MCP verbs (`series.*` etc.)
  through the callback — never a direct query.
- **No streaming/`watch` from a guest** this slice (request/response only). A guest subscribing to bus
  motion is a separate scope (it needs a poll/stream shape — see Open questions).
- **No host→guest async events** (the host already drives the guest via `tool.call`; a guest reactor
  ticked by the host is the workflow-driver's job, not the ABI's).
- **No change to the page bridge.** The frontend already has its callback (`POST /mcp/call`); this is
  the symmetric backend half.

## Intent / approach

**One chokepoint, two front doors.** The host already funnels every bridged tool call through
`lb_host::call_tool(node, principal, ws, tool, input)` — the page reaches it via `POST /mcp/call`. The
guest gets the *same* function as a WIT import. The browser and the wasm guest become two transports for
the one MCP contract (rule 7), each denied identically.

Three pieces:

1. **Identity into `HostState`.** When the host invokes a guest (`Instance::call_tool`), it first sets
   the instance's `HostState` to carry `{ principal, ws, node }` (an `Arc<Node>` + the effective
   principal). The host import `host.call-tool` reads them — so the callback authorizes against real
   identity, not nothing. Cleared after the call returns (no ambient identity leaks across calls; the
   instance stays stateless between calls, §3.4).

2. **The effective principal = `caller ∩ extension-grant`.** Reuse the S5 delegation primitive
   (`Principal::derive`, caps gate 2b — the same one the agent loop uses for `agent ∩ caller`). Before
   running the guest, the host derives a principal whose caps are the **intersection** of the caller's
   caps and the extension's install grant (`admin_approved`). The guest's callback can therefore reach
   *at most* what both the caller and the install allow — no widening, ever. This is the load-bearing
   security property and it already exists; we apply it here.

3. **The import dispatches through `call_tool`.** `host.call-tool(name, input)` →
   `lb_host::call_tool(node, &effective_principal, ws, name, input)` → the existing authorize-then-
   dispatch (host-native via `call_ingest_tool`/the workflow dispatcher, or `<ext>.<tool>` via the
   registry). A re-entrant guest→host→guest call is bounded by a depth guard (Open questions).

**Why this and not alternatives.** (a) *Give the guest a `Store`/bus handle* — rejected: breaks rules
4/5, creates a second un-gated data path, and an AI-written guest could bypass the wall. (b) *Add bespoke
host imports per need (`host.ingest`, `host.read_series`, …)* — rejected: every new need is a forever-ABI
change; the whole point of MCP-as-contract is one generic call. (c) *Keep guests one-way and force
backend logic into host services* — the status quo; rejected because it makes "ship a backend in your
extension" a lie (github-bridge had to stay a host service for exactly this reason). The generic
`call-tool` import is the minimal, principled, once-and-done answer.

## How it fits the core

- **Tenancy / isolation:** the callback's `ws` is the one the host set into `HostState` from the caller's
  token — never guest-supplied. A guest in a ws-A call can only ever reach ws-A tools/data. Two-workspace
  isolation is tested through the real callback (a guest invoked in ws-B sees none of ws-A).
- **Capabilities:** every callback runs the full `authorize_tool` gate against `caller ∩ grant`. Deny is
  opaque. Mandatory deny-tests: (i) a guest calling a tool **its install grant omits** → denied even
  when the caller holds it (delegation narrowing); (ii) a guest calling a tool **the caller lacks** →
  denied even when the install requested it (intersection both ways).
- **Placement:** either (symmetric). The callback runs wherever the guest runs; a `<ext>.<tool>` target
  on another node routes through the existing registry/queryable path unchanged. No `if cloud`.
- **MCP surface:** **consumes** the existing tool surface; **exposes** exactly one new ABI import,
  `host.call-tool` (not an MCP tool — it's the transport). API-shape (§6.1): request/response only this
  slice; no new CRUD/list (consumed verbs already have theirs); **no `watch`** from a guest yet (deferred);
  batch is the caller's concern (a guest looping is fine if bounded — a long job is still a host job).
- **Data (SurrealDB):** none added. The guest touches the store *only* through host verbs (`series.*`,
  `ingest.write`) via the callback — the one datastore, one mediated path.
- **Bus (Zenoh):** none added. (Guest-initiated motion/`watch` is the deferred follow-up.)
- **Sync / authority:** unchanged; a routed `<ext>.<tool>` callback uses the existing cross-node MCP route.
- **Secrets:** none. The guest never receives the token (it gets a derived principal *inside the host*,
  never serialized to the guest); the callback carries only `{name, input-json}`, like the page bridge.
- **SDK/WIT impact:** **YES — the forever boundary changes.** `world extension` gains one import; bump the
  WIT package to `@0.2.0`. This is a §11.2 commitment — flagged loudly. The host `bindgen!` and the guest
  `generate!` read the one WIT, so they can't drift. Loader: world **major** unchanged (still `0`), so
  existing `0.1.0` guests keep loading; a guest that *uses* the new import declares `@0.2.0`.

## Example flow

A `proof.derive` tool on the proof-panel guest: "read the latest `proof.demo`, write `proof.derived` =
value × 2".

1. The page (or any caller) calls `proof-panel.proof.derive {}` over its bridge → host `call_tool`.
2. Host authorizes `mcp:proof-panel.proof.derive:call` against the caller, derives the **effective
   principal** = caller ∩ proof-panel's grant (which includes `series.latest` + `ingest.write`), sets
   `{effective_principal, ws, node}` into the instance's `HostState`, and invokes the guest.
3. Inside the guest's `tool.call("proof.derive", …)`: it calls
   `host.call-tool("series.latest", "{\"series\":\"proof.demo\"}")` → host authorizes
   `mcp:series.latest:call` against the effective principal in `ws` → returns `{"sample":{"payload":21}}`.
4. The guest computes `42`, calls `host.call-tool("ingest.write", "{…payload:42…}")` → host authorizes
   `mcp:ingest.write:call`, stages + drains → `{"accepted":1}`.
5. The guest returns `{"derived":42}`. The host clears `HostState` identity, surfaces the result.
6. **Deny path:** install proof-panel with a grant that omits `ingest.write`; step 4's callback is
   **denied at the host** even though the caller holds `ingest.write` — the intersection narrowed it.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`, all through the **real** `lb-runtime`
component + real store + real caps (no mocks, CLAUDE §9):

- **Capability deny — per direction (both mandatory):**
  - guest calls a verb its **install grant omits** → `Denied` (delegation narrowing), even though caller holds it;
  - guest calls a verb the **caller lacks** → `Denied`, even though the install requested it.
- **Workspace isolation:** a guest invoked in ws-B, calling `series.find`/`inbox.list` via the callback,
  sees **none** of ws-A's data (the `ws` is host-set, un-spoofable).
- **Happy round-trip:** `proof.derive` reads a real seeded series and writes a derived one, end to end
  through the callback (assert the derived row committed via a separate `series.latest`).
- **Re-entrancy / depth:** a guest that calls `proof-panel.proof.derive` recursively is bounded by the
  depth guard (returns an error past the limit, never a stack blow-up or hang).
- **ABI compat:** an existing `@0.1.0` guest (e.g. `hello`) still loads and answers (world major
  unchanged) — the loader-accepts-old-guest regression.
- **Hot-reload:** identity is per-call (set→clear), so a swap mid-life loses nothing; a guest stays
  stateless between calls (assert no identity leaks from call A into call B).
- **Frontend:** extend `ProofPanel.gateway.test.tsx` — a "Run derive" button calls
  `proof-panel.proof.derive` over the page bridge; assert the derived series shows the computed value
  (the page → guest → host → store → page full loop, live). Playwright e2e: click it, see the value.

## Risks & hard problems

- **It is a forever-ABI change.** Get the import signature right the first time. Keep it to the single
  generic `call-tool` — resist per-need imports. The signature mirrors `tool.call` exactly, so it is the
  obvious dual.
- **Re-entrancy & deadlock.** guest→host→guest (or →another ext) can recurse or, worse, re-enter the
  *same* instance while it's borrowed. Mitigation: a per-call **depth limit** and a rule that the
  callback dispatches through `call_tool` (which resolves a *fresh* instance/route), never re-borrows the
  in-flight `&mut Instance`. Spell out the borrow discipline in the session doc.
- **Identity lifetime in `HostState`.** Must be set before the guest runs and cleared after, with no
  bleed across calls (the node-global-instance finding,
  `debugging/extensions/loaded-extension-instance-is-node-global.md`, makes this sharp: one instance
  serves many workspaces, so identity MUST be per-call, never instance-sticky).
- **Effective-principal derivation cost.** `caller ∩ grant` per call — keep it cheap (set-intersect the
  cap lists once per guest invocation, not per callback).
- **AI-written guests.** The whole security story is "the guest can do nothing the gate doesn't allow."
  The intersection + per-call ws is what makes an untrusted/AI-generated guest safe; the tests must prove
  the deny path is real, not displayed.

## Open questions

1. **Re-entrancy depth limit** — fixed constant (e.g. 8) or configurable? Lean: a small fixed constant,
   surfaced as a `tool-error::failed("call depth exceeded")`.
2. **Effective principal = `caller ∩ grant`, or `grant` alone?** Lean: the intersection (a guest acts on
   behalf of its caller AND within its install — the strictest, matching the agent's `agent ∩ caller`).
   Confirm against the agent precedent.
3. **`watch`/motion from a guest** — out of scope here; does a guest ever need to *subscribe*? Likely a
   later scope (a guest reactor would more naturally be host-ticked). Record the decision.
4. **Does `host.log` stay separate** or fold into the callback? Keep separate — `log` is fire-and-forget
   audit, not an authorized tool call.
5. **Which `node` handle does `HostState` hold** — `Arc<Node>` directly, or a narrow callback trait to
   keep `lb-runtime` from depending on `lb-host` (layering)? Lean: a trait object (`HostBridge`) the host
   supplies, so `runtime` stays below `host` in the dep graph (crate-layout). Resolve at implementation.

## Related

- README `§11.2` (SDK/WIT forever commitment), `§6.5` (MCP as the contract), `§3` rules 4/5/7.
- `scope/crate-layout/crate-layout-scope.md` — the SDK/WIT boundary decision (this is its first minor bump).
- `scope/mcp/mcp-scope.md` — the authorize-then-dispatch gate the callback reuses.
- `scope/agent/agent-scope.md` — the `Principal::derive` / `agent ∩ caller` delegation precedent.
- `scope/extensions/proof-panel-scope.md` — the reference extension that will gain `proof.derive`.
- `scope/extensions/ui-federation-scope.md` — the **page** bridge (`POST /mcp/call`); this is its dual.
- `debugging/extensions/loaded-extension-instance-is-node-global.md` — why identity must be per-call.
- Current ABI: `sdk/wit/world.wit`; host side `crates/runtime/src/bindings.rs`; chokepoint
  `crates/host/src/tool_call.rs`.
