# Flows scope — the debug node + debug panel (Node-RED's debug sidebar, over the shipped plane)

Status: **shipped (2026-07-08)** — see [`sessions/flows/debug-node-session.md`](../../sessions/flows/debug-node-session.md)
+ the "debug node + panel" section of [`public/flows/flows.md`](../../public/flows/flows.md). This is
the ask kept as the contract. The **observability** sub-doc of the `scope/flows/` set — read the spine
[`flows-scope.md`](flows-scope.md) first; it owns the canonical **Decisions (v1)** this doc references
by number. [`node-descriptor-scope.md`](node-descriptor-scope.md) owns the **descriptor shape** the
node wears, and [`flow-runtime-control-scope.md`](flow-runtime-control-scope.md) owns the **`flows.watch`
SSE pattern** the live stream is a near-verbatim copy of. This doc ships `debug` (the Node-RED debug
node + sidebar); `catch`/`status`/`complete`/`link` stay sibling scopes (Non-goals).

A flow today can trigger, transform, route, and sink — but an author **cannot see what is flowing
down a wire** without either opening a run snapshot (`flows.runs.get`, a frozen post-hoc view) or
polling `flow_node_state` (the last value only). Node-RED's headline debugging posture — **drop a
`debug` node on a wire, open the sidebar, watch the messages stream past live** — has no equivalent.
This scope adds exactly that: a host-resolved **`debug` node** (a `sink` that publishes the message
envelope onto a workspace-walled bus subject) and a **debug panel** in the canvas that subscribes to
that subject over a gateway **SSE** route and renders each message **type-aware** — JSON as a
collapsible tree, text as monospace, markdown rendered, and **long content auto-collapsed** with a
"show more" disclosure. v1 is **motion-only** (fire-and-forget on the bus, no SurrealDB record); a
later `persist` mode stores debug messages to disc as a named follow-up.

## Goals

- **One new built-in node `debug`** in
  [`builtins.rs`](../../../rust/crates/flows/src/builtins.rs), `kind = "sink"`, one input port
  `payload`, no output port — a pure observer. Same descriptor shape, same palette path, same
  `{payload, topic}` envelope (Decision 6) as the data-node pack. **No new execution cap** (it runs
  inside a `flows.run`, already gated by `mcp:flows.run:call`).
- **Type-aware rendering in the panel:** the node declares the content type of its payload via
  `config.format` (`auto | json | text | markdown`, default `auto`); the panel renders JSON as a
  collapsible tree, text as `<pre>`, markdown via `react-markdown` + `remark-gfm` (already deps).
- **Auto-collapse long content:** any value over a `collapse_bytes` preview threshold (default
  2 KiB) renders collapsed with a "show more" disclosure — the full value is always on the wire,
  collapse is presentation only.
- **Live stream, browser-tail semantics.** A new `flows.debug.watch {flow_id}` MCP verb (member-level
  cap `mcp:flows.debug.watch:call`) + a gateway **SSE** route `GET /flows/{id}/debug/stream?token=`
  stream debug messages to the panel — a **per-flow** subject, deltas-only (attach = live tail; a late
  opener sees messages from attach onward, Node-RED sidebar parity). Reuses the `flows.watch` SSE
  machinery verbatim (the `run_events` trio + `routes/run_stream.rs`), re-seamed onto a `flow_debug`
  subject.
- **Multiple debug nodes per flow,** each carrying a `label` so the panel can attribute and filter
  messages by node.
- **Motion-only in v1** (rule 3): a debug message is the textbook example of motion — fire-and-forget
  on the bus, consumed by the SSE route, **no SurrealDB record**, no new table. Persistence-to-disc is
  a named follow-up (Open Q 2), not this scope.

## Non-goals

- **The rest of the Node-RED observability-node pack** — `catch` (error-as-a-wire), `status`
  (per-node status badge), `complete` (run-completion signal), `link` (cable-free wires). Those are
  **separate sibling scopes**, the explicit defer-list item in
  [`data-nodes-scope.md`](./data-nodes-scope.md) Non-goals and
  [`flow-context-scope.md`](./flow-context-scope.md) gap item 4. This doc ships `debug` only and
  recommends the pack as the follow-up.
- **Persistence / store-to-disc in v1.** A debug message is motion; v1 streams it to the browser and
  forgets it. A `persist` mode (per-node config that writes debug messages to a series
  `debug:{ws}:{flow}:{node}`, queryable/exportable, or a file sink) is Open Q 2 — designed here,
  built later. **No new SurrealDB table ships in this scope.**
- **A workspace-wide "debug console"** (all flows' debug in one view). v1 is **per-flow** (the panel
  subscribes to the open flow's stream). A workspace aggregate is Open Q 1.
- **Per-run debug replay.** Because v1 is motion-only with no snapshot, a late-attaching panel cannot
  replay past messages of a finished run — it tails from attach. Replay-on-open is folded into the
  persistence follow-up (it needs the durable record this scope deliberately defers).
- **A second SSE library, a second JSON-tree widget, or a syntax-highlight dep.** The SSE route copies
  the shipped `flows.watch` route; the JSON tree is a small shadcn-styled component (collapsible
  nodes, no dep); markdown rides the already-present `react-markdown`/`remark-gfm`. No `shiki`/`highlight.js`.
- **A new node-config verb, a new manifest field, or a new WIT world.** The `debug` node is a
  descriptor in the existing `flows.nodes` registry, configured through `flows.save` /
  `flows.node.update` (existing verbs). No SDK surface change.

## Intent / approach

**A sink that publishes, a panel that tails — both over shipped seams.** The `debug` node is a
host-resolved built-in dispatched exactly like the `count`/`json`/data-pack nodes: `execute_node`
reads `inputs["payload"]` (the wire envelope's primary slot, Decision 6), resolves the declared
`format`, and **publishes a debug message** onto a ws-walled Zenoh subject right after the node
settles — the same record-then-publish ordering `publish_run_event` enforces
([`flow-runtime-control-scope.md`](./flow-runtime-control-scope.md)). The message is the unit of
motion; it touches no store. The panel is a pure client of one new SSE route, itself a near-verbatim
copy of the shipped `GET /flows/runs/{run}/stream`.

```
  canvas: FlowCanvas + DebugPanel (right drawer, Node-RED style)
      │  open flow ──► subscribe GET /flows/{id}/debug/stream?token=  (SSE, deltas-only)
      │  each event ─► {node, run_id, ts, format, value, label} ─► render type-aware
      │                    json → collapsible tree · text → <pre> · markdown → react-markdown
      │                    value > collapse_bytes ─► collapsed + "show more"
      ▼
  gateway routes/flows_debug.rs  (re-check mcp:flows.debug.watch:call, ws from token)
      │  GET /flows/{id}/debug/stream  ── Zenoh sub on flow/{flow}/debug under ws/{id}/
      ▼
  host: execute_node/debug.rs  ── on debug-node settle, publish_run_debug()
      │  subject flow_debug:{ws}:{flow}  (fire-and-forget; no SurrealDB write)
      ▼  (motion only — rule 3)
```

**Per-flow, not per-run (Decision 3).** A debug message belongs to the *flow* (Node-RED's debug
sidebar is per-tab, not per-execution); the `run_id` is attribution carried *in* the message, not the
partition. This is the one real design call and it is settled below — it is what makes "open a flow,
drop a debug node, watch" honest, rather than "open a flow, start a run, subscribe to that run's
debug." The per-run settle feed (`flows.watch`) and the per-flow debug feed (`flows.debug.watch`) are
deliberately **two streams** — folding them muddies both (different partition, different event class,
different lifecycle).

*Rejected:* (a) reusing `flows.watch` and adding a `debug` event variant — it is per-`run_id` and
emits settle/run lifecycle; debug is per-flow and emits debug messages; a caller wanting one would have
to filter the other. (b) Making `flows.debug.watch` per-run — defeats the "watch the flow" posture and
forces a run to be in flight before the panel shows anything (a cron-triggered flow would never show
its debug output to an opener). (c) Persisting to `flow_node_state` — that record is Decision 5's
*last value* (state, the dashboard reads it on a hot path); a debug tail is *motion* by definition,
and overloading the state record is the exact mistake `flow-context-scope.md` Non-goals warns against.

## Decisions (resolved — no open questions on these)

These are settled for the long term at scope time; the implementing session executes them, it does not
re-decide. Each names the rejected alternative and why.

1. **The `debug` node is a host-resolved built-in `sink`; executing it needs no new cap.** `type =
   "debug"`, `kind = "sink"`, one input port `payload`, no output. It dispatches inside `flows.run`
   exactly like the `count`/`json`/data-pack nodes — no MCP tool, no `caller ∩ install-grant` callback
   (it does nothing external). The deny path is the existing "no `mcp:flows.run:call` → the run never
   starts." *Rejected:* shipping `debug` as an installable extension node (forks a pure in-process
   observer into the extension path with an install grant per node — pure ceremony, the identical
   posture `data-nodes-scope.md` took for its transforms). *Rejected:* a new `mcp:flows.debug.emit:call`
   cap on publish (the node is already inside a gated run; double-gating an observational sink adds a
   cap with no threat surface).
2. **Motion-only in v1 — no SurrealDB record, no new table.** A debug message is published onto the bus
   and consumed by the SSE route; nothing is persisted. Late-attaching browsers see messages from
   attach onward (deltas-only stream, no snapshot). This is rule 3 made literal: debug is *motion*, and
   making it state would be the violation. *Rejected:* a `flow_debug_log:{ws}:{flow}` ring record
   (becomes a second datastore concern, grows unbounded under a hot source, and the user explicitly
   deferred storage — "for now we just stream from backend and show in browser"). Persistence is Open
   Q 2, designed against the series substrate, not invented as a new table here.
3. **Per-flow stream, not per-run.** Subject `flow_debug:{ws}:{flow}` (relative `flow/{flow}/debug`
   under the `ws/{id}/` prefix). Each message carries `{node, run_id, ts, format, value, label}`. One
   subscription per open flow = one panel; `run_id` is attribution. *Rejected:* per-run (see Intent) —
   the Node-RED posture is per-tab, and a triggered/source flow has no long-lived run for a browser to
   subscribe to.
4. **`flows.debug.watch {flow_id}` is the one new MCP verb + cap.** A live-feed (SSE) verb,
   member-level, `mcp:flows.debug.watch:call`, ws-walled, **deltas-only** (no snapshot — motion-only
   v1). Gateway route `GET /flows/{id}/debug/stream?token=` mirrors `flows.watch`'s SSE shape verbatim
   (the `run_events` trio + `routes/run_stream.rs` re-seamed onto `flow_debug`). *Rejected:* (a) reusing
   `flows.watch` (per-run, wrong event class — see Intent); (b) making it a raw Zenoh sub exposed
   directly to the browser (the gateway is the workspace/cap wall — `flows.watch`/`agent.watch`/every
   other live feed goes through it; debug is not special).
5. **Content type is declared by the node config, not sniffed by the panel.** `config.format ∈
   {auto, json, text, markdown}` (default `auto`). `auto` sniffs **at publish time** in the host: a
   JSON object/array value → `json`; a string that parses as JSON object/array → `json`; a string with
   markdown markers (a leading `#`/`-`/`*`/`>` or a fenced ```` ``` ````) → `markdown`; else `text`.
   The resolved `format` rides on the message, so the panel renders deterministically — no per-message
   sniff in the browser, no client/agent disagreeing on what a value is. *Rejected:* always-`auto` in
   the panel (the author knows whether a string is markdown; an explicit `format` beats guessing, and
   putting the sniff host-side keeps the panel a pure renderer). *Rejected:* a `content_type` MIME
   string (over-engineered for four cases; the enum is the contract).
6. **Long-content collapse is a panel concern, not a wire concern.** The panel collapses any rendered
   value longer than `collapse_bytes` (default 2 KiB preview, full value on "show more" — a shadcn
   `Collapsible` disclosure). The **full value is always on the wire**, governed only by a per-message
   max-size cap (the publish governor, Risk 1). *Rejected:* truncating on the wire (loses the data the
   user dropped a debug node to see — the whole point); and a separate "fetch full value" round-trip
   (there is no durable record to fetch from in v1 — the value is on the bus once).

## How it fits the core

- **Tenancy / isolation (rule 6):** the subject is `flow_debug:{ws}:{flow}` and the verb resolves ws
  from the caller like every `flows.*` verb. A ws-B principal cannot subscribe to a ws-A flow's debug
  stream (read-first wall at the gateway). The debug message physically cannot leave its workspace —
  the `{ws}:` prefix is the partition. **Mandatory isolation test.**
- **Capabilities (rule 5):** **one new cap** — `mcp:flows.debug.watch:call` (member-level, read), gated
  at the gateway SSE route. **Executing the node needs no new cap** (Decision 1 — it runs inside
  `flows.run`). The deny path: no `mcp:flows.debug.watch:call` → the SSE handshake is refused before
  the first event; no `mcp:flows.run:call` → the run (hence the debug node) never executes. Tested per
  verb.
- **Placement (rule 1):** `either` — debug is motion; whichever node owns the flow (Decision 10)
  publishes locally and the SSE route is role-mounted by config. No `if cloud`.
- **MCP surface (§6.1):**
  - **Live feed (SSE / watch):** `flows.debug.watch {flow_id}` — the headline add, deltas-only
    (motion-only v1). This is the *only* API shape this scope adds.
  - **CRUD:** N/A — the `debug` node is a descriptor returned by the existing `flows.nodes` registry
    and configured via `flows.save` / `flows.node.update` (existing verbs). No new write verb.
  - **Get / list:** N/A — there is no persistent debug record to read back in v1 (motion-only). A late
    opener tails from attach; replay is the persistence follow-up.
  - **Batch:** N/A.
- **One datastore / state vs motion (rules 2, 3):** **no new table, no new record** in v1. A debug
  message is **motion** — published on Zenoh, consumed by SSE, never stored. This is the load-bearing
  principle: the moment debug becomes state it violates rule 3 (and the user explicitly deferred
  storage). The follow-up `persist` mode reuses the **series** substrate (`debug:{ws}:{flow}:{node}`,
  the `ingest.write`→series bridge) — not a new table.
- **Bus (Zenoh):** one new subject class `flow_debug:{ws}:{flow}` (relative `flow/{flow}/debug` under
  `ws/{id}/`), message class **fire-and-forget** — a dropped message is non-fatal (a browser that
  missed it simply doesn't show it; the flow is unaffected). Identical posture to `flows.watch`'s settle
  events. **Publish governor:** per-node rate cap (default N msgs/sec, configurable; breach → drop with
  a `dropped: k` sentinel, never a flood) — see Risk 1.
- **Stateless extensions (rule 4):** the `debug` node holds **no durable state** — it reads the wire,
  publishes, settles. Hot-reloading a node-providing extension does not affect it (it is a host
  built-in anyway). A flow with a debug node survives restart with no debug state to lose (v1 has none
  to lose).
- **SDK/WIT:** **none.** The `debug` node is a host built-in descriptor; no `[[node]]` manifest change,
  no WIT world, no host-callback addition. The frozen `tool.call`/`host.call-tool` is untouched.
- **Symmetric nodes (rule 1):** one code path; the SSE route is role-mounted; no `if cloud`.
- **One responsibility per file (FILE-LAYOUT):** `flows/src/builtins/observability.rs` (the `debug`
  descriptor — sibling to `core`/`data`/`parse`/`sequence`/`function`), `host/src/flows/execute_node/debug.rs`
  (the dispatch arm + `publish_run_debug`), `host/src/flows/run_debug_events.rs` (the subject/publish/watch
  trio, mirroring `run_events.rs`), `role/gateway/src/routes/flows_debug.rs` (the SSE route), and on the
  UI `features/flows/debug/` (one component/hook per file: `DebugPanel.tsx`, `useDebugStream.ts`,
  `DebugMessage.tsx`, `JsonTreeView.tsx`, `MarkdownView.tsx`, `TextView.tsx`). No `utils.ts`.

## The `debug` node descriptor

A built-in in the same shape as the data-pack nodes ([`node-descriptor-scope.md`](./node-descriptor-scope.md)):

| field | value |
|---|---|
| `type` | `debug` |
| `kind` | `sink` |
| `title` | `Debug` |
| `category` | `Observability` |
| `inputs` | `["payload"]` |
| `outputs` | `[]` (a sink — no downstream wires) |
| `config_version` | `1` |

```jsonc
// node.config (inline JSON-Schema 2020-12, rendered by SchemaForm)
{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "label":   { "type": "string", "title": "Label", "description": "Shown in the debug panel to attribute this node's messages." },
    "format":  { "type": "string", "enum": ["auto", "json", "text", "markdown"], "default": "auto",
                 "title": "Content type", "description": "How the panel renders the payload. `auto` sniffs at publish time." },
    "collapse_bytes": { "type": "integer", "default": 2048, "minimum": 0,
                 "title": "Collapse threshold (bytes)", "description": "Values larger than this render collapsed with a 'show more'. 0 = never collapse." },
    "rate_limit": { "type": "integer", "default": 50, "minimum": 0,
                 "title": "Max messages/sec", "description": "Publish governor; 0 = use the node default. Breach drops with a sentinel." }
  }
}
```

Two notes. The node is a **sink** (no output port) — it observes and publishes, it does not feed
downstream. A `debug` node therefore never gates a subtree (contrast `switch`/`filter`): removing it
changes only what the panel sees, never what the flow does. And `format`/`collapse_bytes`/`rate_limit`
are **author hints carried on the message** — the host resolves `format` (Decision 5) and echoes the
collapse/rate hints so the panel and the publish governor agree without the browser re-reading the
node config.

## The debug panel (frontend)

A dockable **right-side drawer** in the flows canvas (`features/flows/debug/`, sibling to the palette
and the run controls) — Node-RED's debug sidebar posture. It is a **pure client** of the one new SSE
route; it adds no authority, no new gateway verb beyond `flows.debug.watch`, and no client-durable
state (the message log is component state, cleared on unmount — rule 4).

- **Subscribe on open.** When a flow is open and the panel is visible, it opens
  `GET /flows/{id}/debug/stream?token=`; on close/unmount it cancels. Tauri/tests fall back to a
  no-op (the route is absent) — the panel shows "debug stream unavailable", not an error.
- **Type-aware rendering** per message `format`:
  - `json` → a **collapsible tree** (`JsonTreeView.tsx`, shadcn-styled, no dep — objects/arrays
    expandable, scalars inline, keys muted).
  - `text` → `<pre class="font-mono …">` (`TextView.tsx`), whitespace-preserved.
  - `markdown` → `react-markdown` + `remark-gfm` (`MarkdownView.tsx`) — both already `ui` deps.
- **Auto-collapse.** Any rendered value over `collapse_bytes` (from the message hint, default 2 KiB)
  renders collapsed with a shadcn `Collapsible` "show more / show less" disclosure (Decision 6). `0`
  means never collapse.
- **Per-message chrome.** Each row: timestamp, node `label` (or node id), `run_id` (monospace, short),
  a content-type badge (`JSON`/`TEXT`/`MD`), the rendered value, and **Copy** + **Expand** affordances.
- **Controls.** Clear, Pause/Resume stream, filter-by-node (multi-select from the labels seen), and a
  "scroll to latest on new message" toggle (Node-RED parity).
- **No persistence.** Closing the panel or navigating away drops the log (v1 is motion-only). A future
  "export log" affordance rides the persistence follow-up (Open Q 2).

## Example flow

1. Alice opens a flow (`cooler-control`) that reads a temperature, scales it, and writes a series.
   She drops a **`debug`** node and wires `range → debug` (auto-wire, the envelope's `payload` flows
   in — no `with` binding needed). In the config form she sets `label = "scaled temp"`,
   `format = auto`.
2. She opens the **debug panel** (right drawer); it subscribes to
   `GET /flows/cooler-control/debug/stream?token=…`.
3. She clicks **Run** → `flows.run` → `{run_id}`. As `range` settles, the `debug` node's dispatch arm
   resolves `format`: the payload `23.4` is a number, not JSON-object/array and not a markdown string
   → `text`. It publishes `{node: "debug-1", run_id, ts, format: "text", value: "23.4", label: "scaled temp"}`
   onto `flow_debug:{ws}:cooler-control`.
4. The panel renders the row: `12:04:31 · scaled temp · run 01J… · TEXT · 23.4`. She wires a second
   `debug` node after a `template` that emits markdown → its messages render rendered (headings, bold).
   A third emits a large JSON object → the tree renders **collapsed** at 2 KiB with "show more".
5. She **Pause**s the stream mid-flood, inspects, **Resume**s. A cron-triggered firing later (browser
   closed) produces messages that **no panel sees** (motion-only — they're gone, by design); when she
   reopens, the panel tails from now.

## Testing plan

Per [`scope/testing/testing-scope.md`](../testing/testing-scope.md) — all against the **real** store
(`mem://`) + real bus + real gateway, no mocks, no `*.fake.ts`.

- **Capability-deny (mandatory):**
  - `flows.debug.watch` without `mcp:flows.debug.watch:call` → the SSE handshake is refused (a `403`
    before the first event body).
  - A `flows.run` without `mcp:flows.run:call` never executes the `debug` node (reuse the existing
    flows deny; assert **no** debug message was published — a bus-assertion that the subject stayed
    quiet). State explicitly that executing the node adds **no** new cap to test.
- **Workspace-isolation (mandatory):** a debug message published by a debug node in a ws-A flow lands
  on `flow_debug:{ws-A}:…` only; a ws-B subscriber's `flows.debug.watch` on the same flow id sees
  nothing and a ws-B run of its own flow publishes to `flow_debug:{ws-B}:…` — the `{ws}:` prefix
  holds. Real bus, real ws-scoped subs.
- **Motion-only (regression for the load-bearing principle):** after a run that publishes N debug
  messages, assert **no** new SurrealDB record exists in any `flow_debug*` table (there is no such
  table) and `flow_node_state` for the debug node carries only the envelope (Decision 5 last-value),
  not a debug log. This is the guard against the feature quietly becoming state.
- **Format resolution (Decision 5):** table-driven host-side — number/string/array/object/markdown-
  marked string each resolve to the right `format`; an explicit `format` config overrides `auto`.
- **Publish governor (Risk 1):** a debug node fed by a hot source (N firings/sec > `rate_limit`)
  publishes at most `rate_limit` real messages plus one `dropped: k` sentinel per breach window; the
  flow is unaffected (no back-pressure on the run).
- **Late-attach tail:** a debug message published *before* a subscriber attaches is **not** seen by
  that subscriber (deltas-only, no snapshot — the honest v1 contract); a message published *after*
  attach is seen once.
- **Frontend (Vitest, real spawned gateway — `pnpm test:gateway`, no `*.fake.ts`):**
  - The panel subscribes on open and renders `json`/`text`/`markdown` messages type-aware (seed real
    debug messages by running a real flow with a debug node through the gateway).
  - A value over `collapse_bytes` renders collapsed; "show more" expands to the full value.
  - Filter-by-node hides non-matching rows; Pause stops the fold; Clear empties the log.
  - **Workspace isolation at the UI boundary:** a ws-B token cannot subscribe to a ws-A flow's debug
    stream (the handshake is refused).
  - **Cap-deny at the UI boundary:** without `mcp:flows.debug.watch:call` the panel shows "stream
    unavailable", not a fake log.

## Risks & hard problems (mirror README §11)

1. **Flood / backpressure.** A debug node on a hot wire (a fast `cron`, a chatty MQTT source) can flood
   the bus + every open panel. Mitigations: a **per-node publish governor** (`rate_limit`, default
   50/sec, breach → drop with a `{dropped: k}` sentinel so the panel shows "N messages dropped" rather
   than lagging silently); a **drop-oldest ring at the gateway SSE** (bounded buffer, late consumer
   misses oldest — never an unbounded queue); and the panel's own cap (a max in-memory log size, beyond
   which it drops oldest). Document that debug is **best-effort motion**, not a reliable log.
2. **"Auto" format surprises.** A string that happens to parse as JSON but was meant as text renders as
   a tree; a string with a leading `#` that wasn't markdown renders as a heading. Mitigation: the
   **explicit `format` config is authoritative** (Decision 5); `auto` is the default but the form
   describes its sniff rules; the content-type badge on each row makes the chosen format visible so a
   surprise is diagnosed in one glance.
3. **The debug stream as a hidden wire (the context anti-pattern).** Like `flow-context`'s "invisible
   global state", a flow that "works" only when the panel is open is a footgun — except debug is
   explicitly observational and has no downstream effect (a sink), so the failure mode is "I can't see
   it when I'm not looking", not "the flow behaves differently." Document this clearly: the debug node
   never gates a subtree and removing it changes only the panel.
4. **The persistence follow-up is load-bearing for replay.** v1's motion-only stance means **no replay
   on open** — a user who opens the panel after a cron firing missed those messages. This is honest
   (Node-RED parity) but will be the first user complaint. The follow-up (Open Q 2) must land before
   anyone expects history; until then the panel's empty state says "live tail — messages from attach
   onward" explicitly.
5. **SSE route sprawl.** This is the **third** per-flow/per-run SSE route (`flows.watch`,
   `flows.debug.watch`, and the generic `/bus/{subject}/stream`). Resist generalising into "one
   configurable stream route" prematurely — the three have different auth/partition/event-shape needs;
   copy `flows.watch` verbatim here and let the pattern repeat until a real abstraction earns its place
   (the same posture `flow-runtime-control-scope.md` took).

## Open questions (RESOLVED — shipped this session)

1. **Workspace-wide debug aggregate.** **Rejected `null` in v1** (per the recommendation). v1 ships
   per-flow (`flows.debug.watch {flow_id}` requires the id); a workspace "debug console" is a separate
   UX with its own backpressure story, deferred. The panel is per-flow.
2. **Persistence-to-disc design (the named follow-up).** **Deferred, as designed.** v1 ships
   motion-only with no new table. The follow-up reuses the **series** substrate
   (`debug:{ws}:{flow}:{node}` via `ingest.write`) — queryable/exportable, not a new table. The
   motion-only regression test (`debug_is_motion_only_no_debug_record_is_written`) guards the
   load-bearing principle until it lands.
3. **The `catch` / `status` / `complete` / `link` pack.** **Deferred to sibling scopes**, per the
   `data-nodes`/`flow-context` defer-lists. Recommended as the next pack; resolved that `catch` should
   publish onto the **same** `flow_debug` subject (a `kind: "error"` variant) so the panel renders
   errors inline without a second subscription.
4. **Drop policy under breach.** **Sliding 1s window, `{dropped: k}` sentinel** (the recommendation),
   matching `batch`'s force-release honesty. Implemented + unit-tested in `execute_node/debug.rs`.

## Related

- [`flows-scope.md`](flows-scope.md) — the spine; Decisions **5** (`flow_node_state` last-value — the
  record this scope deliberately does *not* overload), **6** (the `{payload, topic}` envelope the
  debug node reads), **8** (one-job-per-node — the debug node is one such job), **10** (owner node
  publishes the debug motion locally).
- [`node-descriptor-scope.md`](node-descriptor-scope.md) — the descriptor shape the `debug` node wears
  (inline JSON-Schema config, `flows.nodes` registry, the `kind = "sink"` wiring affordance).
- [`flow-runtime-control-scope.md`](flow-runtime-control-scope.md) — the **`flows.watch` SSE pattern**
  (subject + publish + watch trio, gateway SSE route, fire-and-forget motion) this scope copies
  verbatim, re-seamed onto `flow_debug`.
- [`data-nodes-scope.md`](data-nodes-scope.md) — Non-goals explicitly defers the observability-node
  pack (`debug`/`status`/`catch`/`complete`/`link`) to "a separate runtime-observability scope"; this
  is that scope (for `debug`). Its posture on host-resolved built-ins (descriptor in `builtins.rs` +
  dispatch arm + test) is the template.
- [`flow-context-scope.md`](flow-context-scope.md) — gap item 4 names this as the recommended next
  flows scope; its "context as a hidden wire" risk (Risk 1 there) is the mirror of this scope's
  Risk 3.
- [`flows-canvas-scope.md`](flows-canvas-scope.md) — the canvas the debug panel docks into; the panel
  is a sibling surface (right drawer), a pure client of the one new SSE route, adding no canvas
  authority.
- Pattern reused verbatim: `run_events/{subject,publish,watch}.rs` + `routes/run_stream.rs`
  (agent-run watch) and `flows.watch`'s `GET /flows/runs/{run}/stream` — the debug route is the third
  instance.
- README `§3` (rules — esp. **rule 3 state-vs-motion**, the load-bearing principle here; rules 5/6),
  `§6.13` (the gateway SSE/HTTP path), `§6.5` (host dispatch — where the publish governor lives).
- **Skill doc (on ship):** `skills/flows-debug/SKILL.md` — `flows.debug.watch` is an agent-/API-drivable
  live-feed surface; the implementing session writes it from a live run (the SSE handshake, the event
  shape, a curl/`lb-mcp` tail example), per `ABOUT-DOCS.md` → "`skills/`".
