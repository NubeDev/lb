# Flows scope — the data & JSON node pack (20 built-in nodes)

Status: scope (the ask). Promotes to `public/flows/flows.md` once shipped.

> Read the spine first: [`flows-scope.md`](./flows-scope.md) owns the canonical **Decisions
> (v1)** this doc references by number, and [`node-descriptor-scope.md`](./node-descriptor-scope.md)
> owns the **descriptor shape** every node here wears. This doc is the *content* ask, not a new
> mechanism: **twenty new built-in node descriptors** — data-processing and JSON-shaping nodes —
> added to the merged `flows.nodes` registry in the *exact* shape the existing eight built-ins
> (`trigger`/`tool`/`rhai`/`count`/`json`/`counter`/`subflow`/`sink`) already use.

A flow today can trigger, call a tool, run `rhai`, count, parse/stringify JSON, and sink. It
**cannot** reshape a payload, route on a condition, scale a sensor value, split an array into a
stream, join it back, or parse CSV/XML/YAML — without dropping into a hand-written `rhai` cage
for every one. Node-RED (which this engine explicitly mirrors — "node-red over the shipped
plane") ships these as first-class nodes; so should we. This scope adds the **twenty highest-value
data/JSON nodes** as host-resolved built-ins, in the `count`/`json` mold, speaking the
`{payload, topic}` envelope (Decision 6). The set and its semantics are drawn from Node-RED core
and the [edgelinkd](https://github.com/oldrev/edgelinkd) Rust port's node table (function /
sequence / parse categories).

## Goals

- **Twenty new built-in descriptors** in [builtins.rs](../../../rust/crates/flows/src/builtins.rs),
  each host-resolved (no MCP tool) and rendered through the same palette path as the existing
  built-ins — no new registry, no new manifest surface, no `is this native?` branch.
- **Declarative data work without `rhai`.** The common reshape/route/scale/parse operations become
  a configured node with a JSON-Schema form, not a hand-written script — `rhai` stays for the
  genuinely bespoke.
- **Every node speaks the envelope** (Decision 6): input port `payload` (+ `topic` carried
  through), output `payload`. Multi-output (`switch`) and sequence-emitting (`split`) nodes are
  the *only* two that deviate, and both are called out as engine-extending below.
- **No new external persistence and no second runtime** (CLAUDE rules 1–4). Stateful nodes reuse
  the existing `flow_node_state` last-value record (Decision 5) or a scoped accumulator; the one
  timer node (`delay`) reuses the durable suspend/resume the `subflow` park already uses
  (Decision 11).
- **Ship in risk order.** Pure stateless transforms first (drop-in), stateful next, the four
  engine-extending nodes last and behind their own tests.

## Non-goals

- **Network / IO nodes** (MQTT, HTTP, TCP, UDP, WebSocket, file, watch, exec). Those are
  side-effecting *external* seams and belong to **extensions**, not host built-ins —
  [`extension-nodes-scope.md`](./extension-nodes-scope.md) already owns the source/sink/transform
  extension shape and the `ingest.write`→series bridge (Decision 2). This pack is pure in-process
  data work only.
- **Observability nodes** (`debug`, `status`, `catch`, `complete`, `link`). Those are a separate
  runtime-observability concern — `flows.watch` / the per-node settle feed already covers the
  "see what a node emitted" need ([`flow-runtime-control-scope.md`](./flow-runtime-control-scope.md)).
- **A new descriptor field, port vocabulary, binding grammar, or schema dialect.** This pack is
  *content* for the frozen [`node-descriptor-scope.md`](./node-descriptor-scope.md) contract. If a
  node here needs a contract change, that's a finding to raise against the descriptor scope — not
  something this doc invents.
- **HTML scrape / XPath / JSONata.** `select` covers whole-value field-path projection; a full
  query language is deferred until a real caller needs it (resist the templating mini-language,
  per Decision 4's posture).

## Intent / approach

**These are descriptors, not a subsystem.** The keystone work is already done: the descriptor
shape, the merged registry, the JSON-Schema config gate, the envelope ports, and the
host-resolved dispatch arm all exist. Adding a node = one `NodeDescriptor` in `builtins.rs` + one
dispatch arm in [execute_node.rs](../../../rust/crates/host/src/flows/execute_node.rs) (its own
file where the logic is non-trivial, per FILE-LAYOUT) + a test. That is the whole surface for the
stateless majority.

*Rejected:* shipping these as an installable "std-lib extension" of nodes. It would fork the
value-add into the extension path (a WASM/native bundle, an install grant per node) for logic that
is pure, in-process, and has no capability of its own to gate — pure ceremony. Built-ins that ride
the host are the honest home for `map`/`sort`/`csv`. (Extensions remain the home for anything that
touches the outside world.)

The twenty split into **three tiers by execution risk** — the build order:

### Tier A — pure stateless transforms (drop-in, the `count`/`json` mold)

One input `payload` → one output `payload`, no durable state, no engine change. Thirteen nodes:

| `type` | category | what it does |
|---|---|---|
| `change` | Data | ordered ops on `payload`: `set path=value`, `move a→b`, `copy a→b`, `delete path`. The declarative reshape (no `rhai` for "rename a field"). |
| `select` | Data | project `payload` down to a chosen set of field paths → a new object (the "keep only these keys" node). |
| `merge` | Data | deep-merge multiple input payloads into one object (multi-input; last-writer-wins on scalar conflict). |
| `map` | Data | apply a per-element `change`-style op set over every element of an array `payload`. |
| `flatten` | Data | flatten a nested array (configurable depth) or dot-key a nested object → a flat map. |
| `sort` | Data | sort an array `payload` by field path + `asc`/`desc` (+ numeric vs. lexical). |
| `range` | Data | linearly scale a numeric `payload` from an input range to an output range, optional clamp (sensor→engineering-unit). |
| `aggregate` | Data | reduce an array `payload` to a scalar: `sum`/`min`/`max`/`mean`/`count`/`concat`. |
| `template` | Data | render a mustache-style text template from `payload` fields → a string (build a body / topic / small JSON doc). |
| `csv` | Parse | CSV text ↔ array-of-objects, both directions (like `json`'s parse/stringify; header row configurable). |
| `xml` | Parse | XML text ↔ structured value, both directions. |
| `yaml` | Parse | YAML text ↔ structured value, both directions. |
| `base64` | Parse | encode/decode `payload` ↔ base64 (text/bytes boundary). |

### Tier B — stateful, but no engine change (reuse the durable node record)

Reads its own durable last-value / accumulator across firings; survives restart. Reuses the
`flow_node_state` record (Decision 5) for last-value and a scoped accumulator table for the
buffering ones — **no new runtime**. Three nodes:

| `type` | category | state it holds |
|---|---|---|
| `filter` | Function | **report-by-exception (RBE).** Pass the message only if `payload` changed vs. the last one, or changed by more than a deadband. Needs only *last value* → fits Decision 5's record verbatim. |
| `unique` | Data | dedupe: for an array `payload`, drop duplicate elements (stateless); in `stream` mode, drop a `payload` already seen (a durable seen-set, capped-ring bounded). |
| `batch` | Sequence | group N incoming payloads (by count **or** by time window) into one array `payload`. Accumulates a buffer between firings. |

### Tier C — engine-extending (touches the run/frontier machinery — build last, own tests)

These are the honest hard ones: they add a genuinely new execution behaviour and must be scoped
against [`flow-run-scope.md`](./flow-run-scope.md), not bolted on. Four nodes:

| `type` | category | the new behaviour |
|---|---|---|
| `switch` | Function | **multi-output conditional routing.** Evaluate rules against `payload`/a field; fire only the *matched* output port(s). The frontier engine today activates *all* dependents of a settled node — conditional **edge gating** (a settled-but-not-fired outcome per port) is new. |
| `split` | Sequence | **sequence emit.** One array/object `payload` → a *sequence* of one-message-per-element, stamping sequence metadata (`parts`) on the envelope. The engine today is one-settle-one-value-per-port; per-message fan-out is new. |
| `join` | Sequence | **sequence collect.** Recombine a `split`/`batch` sequence back into an array/object, keyed by the `parts` metadata. Accumulates until the sequence completes (state + the sequence contract). |
| `delay` | Function | **durable delay + rate-limit.** Hold a message for a fixed delay, or rate-limit throughput. Reuses the `subflow` park (suspend/resume, Decision 11) for the timer; rate-limit needs a durable queue. |

## How it fits the core

- **Tenancy / isolation:** stateless nodes hold nothing. Stateful ones (`filter`/`unique`/`batch`/
  `join`) key their record by `{ws}:{flow}:{node}` exactly as `flow_node_state` and `flow_input`
  already do (Decision 5, spine "How it fits the core") — the workspace prefix is the hard wall, and
  the workspace-isolation test below proves node B in workspace 2 cannot read node A's accumulator.
- **Capabilities:** **no new MCP verb and no new capability.** These are descriptors returned by
  the existing `flows.nodes` registry and executed *inside* a `flows.run`, which is already gated by
  `mcp:flows.run:call`. A host-resolved node dispatches no external tool, so there is nothing new to
  grant — the deny path is the existing "no `flows.run` cap → the run never starts" (tested).
  (Contrast the generic `tool` node, which dispatches under the *caller's own* cap — none of these
  twenty do.)
- **Placement:** **either.** Pure functions of their input + (for Tier B/C) a ws-scoped record;
  nothing binds them to edge or cloud. They run wherever the run's owner node runs (Decision 10).
  No `if cloud {…}`.
- **MCP surface:** none added. The API shape for *this* feature is "extend the read-only
  `flows.nodes` registry payload" — the editor palette picks the new descriptors up for free. No
  CRUD, no live-feed, no batch verb is introduced (§6.1: all four are N/A — these are node
  *definitions*, consumed by the already-shipped run/watch verbs).
- **Data (SurrealDB):** Tier A touches **nothing**. Tier B/C reuse `flow_node_state` for last-value
  (`filter`) and need **one additive accumulator record** for the buffering nodes
  (`batch`/`join`/`unique`-stream) — a bounded per-node buffer (`flow_node_buffer:{ws}:{flow}:{node}`,
  capped-ring, the precedent already used for the plc-reliability ring). This is the one genuine
  storage addition and is flagged in Risks. State only — never motion.
- **Bus (Zenoh):** none directly. A node's settled `payload` rides the existing per-node settle
  subject the runtime-control feed already publishes ([`flow-runtime-control-scope.md`](./flow-runtime-control-scope.md));
  these nodes add no new subject.
- **Sync / authority:** node-local, run-owner-authoritative (Decision 10). Offline behaviour is the
  run engine's existing suspend/resume — `delay` in particular resumes its timer after a restart
  because it parks on the durable job, not an in-memory `sleep`.
- **Secrets:** none. No node here handles secret material.

## Example flow

A worked IoT path exercising one node from each tier — *MQTT temperature array → per-reading
scaling → route → dashboard*:

1. A `trigger` (mode=`event`) fires on a batch of raw sensor readings arriving as a JSON string
   payload `"[{\"id\":\"a\",\"raw\":512},{\"id\":\"b\",\"raw\":1023}]"`.
2. **`json`** (existing) parses the string → an array of two objects.
3. **`split`** (Tier C) fans the array into a sequence of two messages, one per reading, each
   stamped with `parts` metadata.
4. **`range`** (Tier A) scales each `raw` (0–1023) to `temp_c` (−40–125), clamped.
5. **`filter`** (Tier B, RBE) drops the message if that sensor's `temp_c` hasn't moved more than
   0.5° since last time — its last value read from `flow_node_state:{ws}:{flow}:filter`.
6. **`join`** (Tier C) recombines the surviving readings back into one array, keyed by `parts`.
7. A **`sink`** (existing, target `series`) writes the array to the dashboard's series.

Nothing in that path is a hand-written `rhai` cage; every step is a configured node.

## Testing plan

Per [`scope/testing/testing-scope.md`](../testing/testing-scope.md). Mandatory categories that
apply here:

- **Capability-deny (mandatory):** a `flows.run` attempted in a session without `mcp:flows.run:call`
  never executes any of these nodes — reuse the existing flows deny-test; assert no node state is
  written on deny. (No *new* cap exists to test — state that explicitly.)
- **Workspace-isolation (mandatory):** seed a `filter`/`batch`/`unique` node's accumulator in
  workspace 1; run the *same* flow id in workspace 2 and assert its node reads an **empty** state
  (the `{ws}:` prefix holds). Real store (`mem://`), real records — no fakes (CLAUDE §9).
- **Per-node unit tests (Tier A):** table-driven input→output per node, incl. the failure contract
  (e.g. `csv`/`xml`/`yaml` parse of malformed text **fails the node**, Node-RED parity with the
  existing `json` node — surfaces a bad body instead of flowing a wrong shape). Live in
  `crates/flows` next to the descriptors.
- **Stateful integration (Tier B):** run a flow twice against the **real** store; assert `filter`
  suppresses the unchanged second firing, `batch` releases at its count/window boundary, `unique`
  dedupes across firings — then assert the state survives a store round-trip (restart parity).
- **Engine integration (Tier C):** the hard ones, in `crates/host/tests/`:
  - `switch` — a payload matching output 2 fires *only* dependents wired to port 2; the port-1
    branch does **not** run (assert no `flow_step_output` row for it).
  - `split`→`join` — a 3-element array round-trips **under array-carry** (Decision 15): `split` stamps
    `parts` on one settle, `join` reassembles from the carried `parts`, order preserved. (Shipped as
    `split_join_round_trips_an_array` + `split_map_join_transforms_each_element` — a per-element `map`
    between them proves the `parts` carry-through. NOT "3 sequenced settles" — array-carry replaced
    per-message fan-out, resolving Q2.)
  - `delay` — a run parks on the timer and **resumes after a simulated node restart** (durable
    park, not in-memory sleep); rate-limit releases queued messages at the configured rate.
- **Regression:** any bug found gets a `docs/debugging/flows/<symptom>.md` entry + a regression
  test (`scope/debugging/debugging-scope.md`).

## Risks & hard problems (mirror README §11)

1. **Tier C is not "20 easy nodes."** `switch`/`split`/`join` change the *execution model*, not
   just the palette. Conditional edge-gating (a node settling a port that does **not** fire its
   dependents) and per-message sequence fan-out are the two things the frontier driver was built
   *not* to do (Decision 8: one job per node, in-degree-0 → enqueue). Scope these against
   `flow-run-scope.md` **before** writing them; if the driver can't express "settled-but-gated"
   cleanly, that's a driver change to land first. **Do not ship Tier A green and imply Tier C is
   done.**
2. **The sequence contract (`parts`).** `split`/`join`/`batch` need a shared, versioned envelope
   metadata for "this is message i of n in sequence s" (Node-RED's `msg.parts`). It touches the
   frozen envelope (Decision 6, `flow-message-envelope-scope.md`) — an additive field, but it must
   be designed once and reused by all three, not invented three times.
3. **The one storage addition.** The buffering nodes need a bounded per-node accumulator beyond
   Decision 5's last-value record. Keep it **capped** (the plc-reliability ring precedent) so a
   `batch` that never reaches its count can't grow unbounded; decide the overflow policy (drop-oldest
   vs. force-release) in the open questions.
4. **New crate dependencies.** `csv`, `xml`, `yaml`, `base64`, and a mustache/handlebars-lite each
   pull a crate into the pure `lb-flows` crate (today dependency-light: `serde_json`, `jsonschema`).
   Pick minimal, well-audited crates (e.g. `csv`, `quick-xml`, `serde_yaml`, `base64`) and add the
   rows to `key-stack.md`. A heavy templating engine is a smell — a mustache-lite is enough.
5. **`switch`/`change`/`select`/`filter` all express "a condition/path over a payload."** Design
   **one** small shared predicate + field-path helper (reusing the existing `${…}` binding
   grammar's path walker) and let all four consume it — do not write four bespoke matchers
   (FILE-LAYOUT: folder-of-verbs, no `utils.rs`).

## Open questions — ALL RESOLVED (shipped this session)

1. **Tier C driver seam — RESOLVED (new spine Decision 14).** `switch` is **edge-gating computed at
   release time**, not a new wire `Outcome` variant: it settles `Ok` (pass-through), and the executor
   reads its `config.rules` (each rule names its `to: [node_ids]` targets), releases only the matched
   dependents, and gates (skips the exclusive subtree of) the rest. A suppressing stateful node
   (`filter`/`batch`/`unique`) reuses the seam by settling `Skipped`. No new `Outcome` variant, no
   edge-model change, no sentinel payload. (`crates/host/src/flows/execute_node/switch.rs`;
   `run_store::{ready_one_dependent,skip_gated}`.)
2. **`split` fan-out unit — RESOLVED (new spine Decision 15): array-carry.** `split`/`join` do **not**
   fan out into N runs (Decision 9 / the fan-out-storm concern); `split` emits one settle carrying the
   array + a `parts` descriptor, and `join` recombines from the carried `parts`. They collapse into
   pure array transforms (`crates/flows/src/ops/sequence.rs`) — Tier C only in that they own the
   versioned `parts` contract (Risk 2). Per-element work is the array-native `map`/`sort`/`aggregate`.
3. **`batch`/`unique` overflow policy — RESOLVED: capped buffer, force-release.** The bounded
   accumulator (`flow_node_buffer`, `BATCH_MAX = 1000`) **force-releases** a `batch` at the cap (emit
   the partial group, never grow unbounded, never silently drop), and the `unique`-stream seen-set is
   a drop-oldest ring at the same bound. (`crates/host/src/flows/buffer.rs`.) *Time-window* batching
   is deferred to the cron/timer reactor (out of the one-shot-run scope) — **count** mode is the
   shipped `batch`; this is an explicit, named deferral, not a silent gap.
4. **`change`/`select` path grammar — RESOLVED: exactly the existing walker.** `change`/`select`/
   `switch`/`filter` all address values through one shared `ops::path` helper — dot-separated keys +
   numeric array indices, missing → `null` — the binding walker verbatim (`crates/flows/src/ops/path.rs`).
   No wildcards/superset until a caller forces it.
5. **Descriptor count vs. FILE-LAYOUT — RESOLVED: yes, split into `builtins/`.** `builtins.rs` became
   `builtins/{core,data,parse,sequence,function}.rs` (+ `mod.rs` concatenating), each well under 400
   lines. The dispatch arms likewise split into `execute_node/{core,sink,subflow,pure,stateful,switch,
   delay}.rs`, and the pure transform logic into `crates/flows/src/ops/`.

## Decisions recorded inline (this pack, not engine-contract-changing)

- **`merge` operates on an array payload** (deep-merge its object elements), not multiple input ports
  — the single-`payload` envelope model has no multi-port merge, and an array-of-objects is the
  natural Node-RED shape.
- **CSV cells stay strings** on parse (no number inference); the `range`/`predicate` layer coerces
  numeric strings later, so a `"512"` from CSV still compares/scales numerically.
- **XML convention** (`ops::parse::xml`): element→object, `@attr` attributes, `#text` text, repeated
  children→array, single top-level key = root. Round-trippable; not a full XML binding (namespaces
  stripped, CDATA/comments ignored) — enough for the flow boundary.
- **`template` is a hand-rolled mustache-lite** (`{{dot.path}}` holes, missing→empty) — no templating
  engine crate (Risk 4).
- **`filter` reuses the Decision-5 `flow_node_state` record** for its last value (no new storage);
  only the buffering nodes (`batch`/`unique`-stream) needed the one additive `flow_node_buffer` record.

## Related

- **Spine & contract:** [`flows-scope.md`](./flows-scope.md) (Decisions 5, 6, 8, 9, 10, 11),
  [`node-descriptor-scope.md`](./node-descriptor-scope.md) (the descriptor shape these wear).
- **Execution:** [`flow-run-scope.md`](./flow-run-scope.md) (the Tier C seam),
  [`flow-message-envelope-scope.md`](./flow-message-envelope-scope.md) (the envelope + the `parts`
  sequence metadata), [`flow-plc-reliability-scope.md`](./flow-plc-reliability-scope.md) (the
  capped-ring precedent + fan-out posture).
- **Boundaries:** [`extension-nodes-scope.md`](./extension-nodes-scope.md) (why network/IO nodes are
  extensions, not built-ins), [`flow-runtime-control-scope.md`](./flow-runtime-control-scope.md)
  (the settle feed these emit on).
- **External reference:** [edgelinkd](https://github.com/oldrev/edgelinkd) — a Rust Node-RED port;
  its node table (function/sequence/parse) is the source for this set. Node-RED core node docs for
  the per-node semantics.
- **Code:** [`rust/crates/flows/src/builtins.rs`](../../../rust/crates/flows/src/builtins.rs) (the
  descriptors), [`rust/crates/host/src/flows/execute_node.rs`](../../../rust/crates/host/src/flows/execute_node.rs)
  (the dispatch arms).
- **Platform:** README `§6.5` (flows/rules surface), `key-stack.md` (add the parse-crate rows).
</content>
</invoke>
