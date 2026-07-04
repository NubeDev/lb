# Flows scope — node / flow / global context (the Node-RED context store)

Status: scope (the ask). Promotes to `public/flows/` once shipped.

We want the Node-RED **context** model in flows: a small key-value surface a node can read
and write at three visibilities — **node** (private to one node instance: its own counters and
scratch state), **flow** (shared by every node in the same flow: computed thresholds, last
alert time), and **global** (shared by every flow in the workspace: shared settings, lookup
tables) — usable first from the **`rhai` node** (`context.get/set`, `flow.get/set`,
`global.get/set`), readable from **`with` bindings** so declarative nodes (`change`, `switch`,
`template`) consume it too, and inspectable/settable over **`flows.context.*`** MCP verbs so
the canvas gets a context sidebar and an operator can seed a value. This is not a new store:
it generalises the shipped `flow_node_memory` durable-counter record (the `counter` node's
atomic memory) from "one i64 per node" to "JSON values at three scopes", on SurrealDB, behind
the existing rules-cage seam pattern.

## Goals

- Three context scopes with Node-RED semantics: **node** (this node instance only), **flow**
  (all nodes in this flow), **global** (all flows in this workspace).
- **Rhai first-class:** `context` / `flow` / `global` handles pushed into the cage exactly like
  the shipped `ai`/`inbox`/`outbox`/`channel` handles — `get`, `get_or`, `set`, `del`, `keys`,
  and an **atomic `incr`** (the `lb_store::increment` primitive the counter node already uses).
- **Durable by default.** Context survives runs, restarts, and flow re-deploys (Node-RED's
  file-store behaviour; its lose-on-restart memory store is the thing people trip on).
- **Bindings read it:** the `with` binding grammar gains `${context.node.<key>}`,
  `${context.flow.<key>}`, `${context.global.<key>}` so `change`/`switch`/`template` compare
  against a stored threshold without a `rhai` cage.
- **Inspectable:** `flows.context.get/list/set/delete` MCP verbs (per-verb caps) → a canvas
  context panel and an ops/debug surface.
- **Guarded growth:** per-value size and per-scope key-count governors, and teardown/orphan
  GC so deleted flows and removed nodes don't leak keys.

## Non-goals

- **A secret store.** API keys and credentials do NOT go in global context — secrets stay in
  the host's secret mediation. The docs and the context panel say so explicitly.
- **A cross-workspace global.** "Global" means *workspace-global*. Workspace is the hard wall
  (rule 6); a value shared across tenants does not exist in this system.
- **A non-durable in-memory tier** (Node-RED's `memory` store). One datastore (rule 2);
  SurrealDB embedded is on every node and `mem://` in tests. If a hot path ever needs a cache,
  that is a perf follow-up, not a second store class.
- **Per-key TTL / expiry.** Defer until a real caller needs it; `del` + GC covers v1.
- **A CAS/transaction API in rhai.** `set` is last-write-wins; `incr` is the one atomic verb.
  Concurrent-branch counters use `incr`; anything fancier is a smell (put it in one node).

## Intent / approach

**One new table, three id shapes, one seam trait.** A `flow_context` record holds one key:

```
flow_context:{ws}:global:{key}                 // global — every flow in the workspace
flow_context:{ws}:flow:{flow}:{key}            // flow   — every node in this flow
flow_context:{ws}:node:{flow}:{node}:{key}     // node   — this node instance only
```

Value is any JSON (size-capped). Node and flow scope key by **flow id, not version** — context
deliberately survives re-deploys (Decision 1 pins the *graph* per run; context is the state
that persists *across* runs, the whole point of the feature).

**Rhai wiring is the shipped handle pattern.** `lb-rules` gains a `ContextSeam` trait (the
`DataSeam`/`AiSeam` shape: sync methods, host bridges to the async store with `block_on`),
and `verbs::register` pushes three handles closing over the run's pinned `{ws, flow, node}`
identity. The host injects that identity at the **flows → rules seam** (where
`execute_node::rhai` calls `rules.run`) — it is not a public field on the `rules.run` request,
so a direct `rules.run` caller gets the `global` handle only (`ws` is its whole identity);
`context`/`flow` calls outside a flow raise a clear rhai error. Within-workspace scope
separation is a *correctness* convenience, not a security boundary — the workspace is the wall.

```rhai
// the temperature-alarm debounce, the canonical use
let last = context.get_or("last_alert_ts", 0);
if msg.payload > flow.get_or("threshold", 80.0) && ts - last > 300 {
    context.set("last_alert_ts", ts);
    inbox.raise("temp high: " + msg.payload);
}
let hits = context.incr("alarm_count", 1);   // atomic across concurrent branch jobs
```

**Alternative rejected:** widening `flow_node_state` / `flow_input` to carry arbitrary keys.
Those records have settled single meanings (Decision 5's last-output; Decision 9's retained
inject value) and the dashboard reads them on hot paths; overloading them muddies both. A
sibling table with explicit scope prefixes is one mechanism, three visibilities, zero changes
to shipped records. Also rejected: exposing context as `store.*` verbs inside the cage — the
cage stays I/O-free except for named seams (rule 5 posture), and a raw store verb is exactly
the escape hatch the sandbox exists to not have.

## How it fits the core

- **Tenancy / isolation:** every id starts `flow_context:{ws}:…`; the seam is closed over the
  run's pinned ws; the MCP verbs resolve ws from the caller like every `flows.*` verb. A rhai
  script physically cannot name another workspace. Isolation tests mandatory.
- **Capabilities (Decision 2):** reads are member-level — `mcp:flows.context.get:call`,
  `mcp:flows.context.list:call`. Writes split by scope, mirroring `prefs` verbatim: `flow`/`node`
  writes are member-level (`mcp:flows.context.set:call`, `mcp:flows.context.delete:call`);
  **`global`** writes are admin-gated through **distinct verbs** (`flows.context.set_global` /
  `flows.context.delete_global`, gated by `mcp:flows.context.set_global:call` etc.), the
  `prefs.set` vs `prefs.set_default` shape (`rust/crates/host/src/prefs/authorize.rs`) — the
  authority boundary lives in the cap grammar, not a branch inside the handler. Rhai access needs
  **no new cap**: executing the flow already passed `flows.run` ∩ every node's own gates, and the
  seam confines reach to the run's own ws/flow/node identity. The deny tests are on the MCP verbs
  (member-verb-denied and global-verb-denied-to-non-admin).
- **Placement:** either — context is a store record; whichever node owns the flow (Decision 10)
  reads/writes it locally, and store sync carries it like any ws record. No `if cloud`.
- **MCP surface (§6.1):** `get` (one key), `list` (a scope's keys + values inline, Decision 3),
  `set` / `delete` (member — `flow`/`node` scope), `set_global` / `delete_global` (admin —
  `global` scope, Decision 2). No batch — a scope is bounded by the key-count governor, so bulk
  import doesn't exist as a use case; say so rather than ship an unused job. No live feed in v1 —
  the canvas context panel refreshes on `list`; if watching a context key becomes real, it rides
  the existing `flows.watch` SSE, not a new stream.
- **Data (SurrealDB):** one new `flow_context` table above, and **one deleted** —
  `flow_node_memory` folds into node-scope context under the reserved `__count` key
  (Decision 1), net zero table growth. State, not motion: a context write is not an event and
  publishes nothing on the bus. (A node that wants downstream reaction emits on its wire —
  context is exactly the place data goes when it should *not* flow.) Governors: value ≤ 64 KiB,
  ≤ 256 keys per scope (config knobs, `RuleLimits` style); breach → a clear rhai error / MCP
  error, never silent truncation. Reserved keys (`__count`, and any future `__`-prefixed key)
  are host-owned: `flows.context.set` rejects a `__`-prefixed key from a caller, so the fold
  can't be corrupted by a user write.
- **Bus (Zenoh):** N/A (see above — deliberately none).
- **Sync / authority:** the flow's owner node is the natural writer; cross-node concurrent
  writes to the *same* key are last-write-wins like any synced record. `incr` is atomic
  per-store (the counter-node guarantee, unchanged), not cross-node-transactional — document
  that; a cross-node shared counter was already out of scope for `counter`.
- **Secrets:** none stored, by decree (Non-goals). The context panel shows a "not for
  secrets" note.
- **Lifecycle / GC:** `flows.delete` teardown (Decision 13) extends one step: purge
  `flow_context:{ws}:flow:{flow}:*` and `…:node:{flow}:*` (this reaps the folded `__count` key
  with its node, replacing the old `flow_node_memory` reap in `retention_sweep`). The shipped
  `orphan_sweep` also reaps node-scope keys whose node id no longer exists in the flow's latest
  version. Global keys live until explicit `flows.context.delete_global`. The context panel and
  `list` hide `__`-reserved keys from the user (the counter total shows in the node's own UI,
  not as a raw context row).
- **SDK/WIT:** untouched. Extension nodes do not see context in v1 (they are stateless by
  rule 4 and get inputs on the wire); if an extension node ever needs it, that is a
  host-callback addition to gate deliberately — flagged, not smuggled.

## Gaps flagged while scoping (the "what else is missing" pass)

Reviewed against Node-RED's runtime feature set; disposition for each:

1. **Context (this doc)** — the biggest gap; a stateful loop today needs the `counter` node
   or abusing retained inputs. **Build.**
2. **Context in bindings** — Node-RED's `change` node reads flow/global context; without it,
   every threshold comparison needs rhai. **Build here** (the `${context.…}` grammar row).
3. **Context visibility** — Node-RED's context sidebar. **Build here** (`flows.context.list`
   + a canvas panel; small, and debugging stateful flows without it is misery).
4. **`catch` / `status` / `complete` / `link` nodes** — already flagged as a separate
   observability-node scope in `data-nodes-scope.md` (its explicit defer-list). Recommend it
   as the *next* flows scope after this one; error-handling-as-a-wire is the other big
   Node-RED parity gap. **Not this doc.**
5. **Subflow instance env/config** (Node-RED subflow env vars) — Decision 4's
   `${params.<name>}` mapping already covers the mechanism; what's missing is only canvas UX.
   **Defer to `flows-canvas` follow-up.**
6. **Fold `counter`'s `flow_node_memory` into node context** — same primitive, two tables.
   **Resolved: fold and drop the table** (Decision 1), in the same session.

## Example flow

The Node-RED debounced-alarm classic, impossible cleanly today:

1. An operator sets the shared threshold once: dashboard slider → `flows.context.set
   {scope: "flow", flow, key: "threshold", value: 85}` (or a setup rhai node writes it).
2. `mqtt.in` (source) fires a run per reading; the `rhai` node reads
   `flow.get_or("threshold", 80.0)` and its own `context.get_or("last_alert_ts", 0)`.
3. Over threshold and outside the 5-min window → `context.set("last_alert_ts", ts)`,
   `context.incr("alarm_count", 1)`, emit; a `switch` downstream routes on
   `${context.flow.maintenance_mode}` to suppress paging during maintenance.
4. The node restarts mid-week; `last_alert_ts` and `alarm_count` are records, not RAM —
   the debounce window holds. A second flow's identical node ids see **their own** node
   context (different flow id in the key); a second workspace sees nothing.
5. The user opens the canvas context panel → `flows.context.list` shows all three scopes
   for the selected node, with edit/delete.

## Testing plan

Against the real store (`mem://`) / gateway per `scope/testing/testing-scope.md` — the
counter-node tests are the template:

- **Capability-deny (mandatory):** member `flows.context.set`/`delete` without the cap →
  opaque deny; **admin-deny (Decision 2):** a non-admin member calling `set_global` /
  `delete_global` → opaque deny even though it holds the member `set` cap; both denies audited.
- **Workspace-isolation (mandatory):** same flow/node/key ids in two workspaces; reads and
  `list` never cross; a rhai `global.get` in ws A never sees ws B.
- **Scope walls:** node A's `context` invisible to node B's rhai in the same flow; flow A's
  `flow` scope invisible to flow B; `global` visible to both.
- **Atomicity:** N concurrent branch jobs `incr` the same key → exact total (the shipped
  `increment` guarantee, re-proven through the seam).
- **Counter fold (Decision 1):** the existing `counter`-node tests (`flows_multi_trigger_test`,
  `increment_test`) pass **unchanged in behaviour** after re-pointing to `flow_context` /
  `__count` — same tick/throughput/reset totals, same atomicity; `flow_node_memory` is gone
  (no reference survives). This is the regression gate on the fold.
- **Reserved-key guard:** `flows.context.set` with a `__`-prefixed key → rejected; a user
  cannot clobber `__count`.
- **Durability:** set → complete run → new run reads it; set → node restart (real store,
  not `mem://` for this one) → survives; re-deploy a new flow version → survives.
- **Governors:** oversize value and key-count breach → clear errors, state unchanged.
- **GC:** `flows.delete` purges flow+node scope (incl. `__count`), leaves global;
  `orphan_sweep` reaps a removed node's keys.
- **Bindings (Decision 4):** a `switch` routing on `${context.flow.x}` **and** a `template`
  rendering `{{context.flow.x}}`, each with the key set/unset (unset → the binding's normal
  missing-value behaviour, not a crash) — proving the one resolver reaches both node types.
- **Not-in-a-flow:** direct `rules.run` script calling `flow.get` → clear error; `global`
  works.

## Risks & hard problems

- **Context as a hidden wire.** The real failure mode: flows that "work" via invisible
  global state instead of edges, undebuggable by graph inspection. Mitigations: the context
  panel (visibility), governors (can't become a database), and doc voice ("if downstream
  should react, emit on the wire").
- **Read-modify-write races.** `get`+`set` across concurrent branches is inherently racy;
  we ship `incr` for the counter case and *document* LWW for the rest rather than pretend
  a transaction API into the cage.
- **Blocking-bridge cost.** Each seam call is a `block_on` store round-trip inside the rhai
  thread; a script hammering `get` in a loop burns its time budget. Acceptable (the budget
  is the governor), but the seam should read whole-key, not offer sub-path reads that invite
  chatty loops.
- **GC correctness over versions.** "Node removed in v5 but a v3-pinned run still executes
  it" — the orphan sweep must key off *latest version + no active pinned run*, mirroring the
  existing sweep's rule for node state.

## Decisions (resolved — no open questions)

These were the scoping-time open questions; each is now settled for the long term, with the
rejected alternative named. The implementing session executes these, it does not re-decide.

1. **Fold `counter`'s `flow_node_memory` into node-scope context — drop the table.** The
   `counter` node's atomic total *is* a node-scope context value; keeping two tables for one
   primitive is exactly the duplication rule 8 warns against. The counter node's `increment`
   call re-points from `flow_node_memory` / `node_scoped_id(flow, node)` to `flow_context` /
   the node-scope id with a reserved key (`__count`), and `FLOW_NODE_MEMORY` is deleted. This
   is a 4-file change (`execute_node/core.rs`, `record.rs`, `flows/src/lib.rs`, plus the
   `retention_sweep`/`orphan_sweep` reap), pre-1.0, no migration owed (dev store). The atomic
   `incr` verb the fold needs is the same verb the feature ships anyway — the fold is nearly
   free once context exists. **Sequencing:** build context first, migrate `counter` onto it in
   the *same* session (so `increment`'s durable-total guarantee is proven once, on the new
   table). *Rejected:* leaving `flow_node_memory` shipped — two tables, two GC paths, two
   places to reason about node-durable state forever, to save a one-session edit now.

2. **Write-cap tier splits by scope, mirroring the `prefs` precedent verbatim.** `flow` and
   `node` scope writes are **member-level** (`mcp:flows.context.set:call`) — a dashboard
   slider writing a per-flow `threshold` is the canonical member action, exactly like
   `prefs.set`. **`global` scope** writes are **admin-gated** (`mcp:flows.context.set_global:call`,
   a *distinct* verb+cap, not a scope argument on the member verb) — a workspace-wide value is
   the `prefs.set_default` shape (`rust/crates/host/src/prefs/authorize.rs`), granted only to
   admins. Two verbs, not one verb branching on scope, so the cap grammar gates it without the
   handler re-checking scope (the deny is at the seam, per §6.1). Reads (`get`/`list`) are
   member-level for all three scopes. *Rejected:* one `set` verb taking a `scope` field and
   branching to an admin check inside the handler (hides an authority boundary inside a handler
   — the anti-pattern rule 5 exists to prevent); and making `global` member-writable (any
   member could rewrite a shared lookup table every flow depends on).

3. **`list` returns values inline, with a per-value `truncated` flag.** Values are ≤ 64 KiB by
   governor, a scope is ≤ 256 keys, so a whole-scope `list` is bounded and panel-friendly in
   one round-trip — the context sidebar needs the values, not a fan-out of `get` calls. Any
   value over a `preview_bytes` cap (default 4 KiB) comes back truncated with
   `{ truncated: true }` set and the full value fetched by `get`. *Rejected:* keys-then-`get`
   (N+1 round-trips to paint a panel that is already size-bounded).

4. **`${context.<scope>.<key>}` resolves everywhere a binding does — including `template`'s
   mustache scope — through one resolver.** One grammar, one resolver, no per-node surprise:
   if `change`/`switch` can read `${context.flow.x}`, so can `template`. The resolver is added
   once at the binding-resolution seam and every node that resolves `with` bindings inherits
   it; `template` additionally exposes the same values in its mustache namespace so
   `{{context.flow.threshold}}` works. *Rejected:* the `with`-grammar-only scope (a user who
   learns `${context.…}` on a `switch` and finds it dead in a `template` has hit an arbitrary
   wall).

## Related

- `flows-scope.md` — Decisions 1 (versioning), 5 (`flow_node_state` last-value), 9 (retained
  inputs — the sibling record this must *not* be confused with), 10 (owner node), 13 (teardown).
- `data-nodes-scope.md` — the `change`/`switch`/`template` nodes that gain context bindings;
  its defer-list owns the `catch`/`status` gap (item 4 above).
- `../rules/rules-engine-scope.md` — the cage, `RuleLimits`, the seam pattern
  (`rust/crates/rules/src/seam.rs`, `verbs/mod.rs` handles).
- `flow-message-envelope-scope.md` — the wire the context deliberately is not.
- README §3 rules 2/3/5/6; `scope/testing/testing-scope.md`.
- Skill doc (on ship): `skills/flow-context/SKILL.md` — the `flows.context.*` verbs are an
  agent-drivable surface; the implementing session writes it from a live run.
