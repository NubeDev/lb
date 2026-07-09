---
name: flows-mcp
description: >-
  Author, run, inspect, and manage Lazybones flows (node-graph automations) programmatically over the
  node gateway — list/read/create-update/delete flows, wire typed nodes (triggers, the 28 built-in
  data/JSON/tool/rhai/sink nodes, extension nodes), run them, watch runs settle, suspend/resume/cancel,
  patch or edit one node's config, enable/disable, and inject retained inputs. Use when a task says
  "create/seed/edit a flow", "add/configure a node", "run a flow / read its output", "build a data
  pipeline (csv/json/split/switch/…)", "automate flows", or "call flows.* over MCP/REST".
---

# Managing flows & nodes over MCP / REST

A Lazybones flow is a **typed, versioned node graph** (node-red over the shipped plane): a `flow:{ws}:{id}`
record whose nodes wire together, run as a durable resumable `lb-jobs` session, state in SurrealDB, motion
on Zenoh. This skill drives the whole `flows.*` surface over the node's HTTP gateway (default
`http://127.0.0.1:8080`, override with `VITE_GATEWAY_URL`). `flows` is the **one DAG engine** (the old
`chains.*` was retired).

Two equivalent transports (README rule 7 — capabilities are MCP tools; UI, agents, extensions call them
the same way):

1. **Dedicated REST routes** — `GET/POST/DELETE /flows…` (ergonomic; the gateway supplies the clock `ts`).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for ANY host verb by dotted name
   (`flows.save`, `flows.run`, …). Here **you must pass `ts`** (the logical clock) yourself.

Both derive **workspace + principal from the bearer token**, never from the body (the hard wall, README
§6/§7). Every verb is capability-gated server-side; a denial is **opaque** (you can't tell "forbidden"
from "absent").

## 1. Authenticate

```bash
# dev login: who + which workspace. An empty workspace bootstraps the caller as workspace-admin.
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Send it on every call as `Authorization: Bearer $TOKEN`. The default **member** cap set holds every
`mcp:flows.<verb>:call` plus `store:flow:read` / `store:flow:write`. A node that dispatches another verb
(a `tool` node, a `sink`) also needs **that** verb's own cap — flows never widen (`caller ∩ grant`).

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| Node registry (palette) | `GET /flows/nodes` | `{"tool":"flows.nodes","args":{}}` | — |
| List flows | `GET /flows` | `{"tool":"flows.list","args":{}}` | — |
| Read one flow | `GET /flows/{id}` | `{"tool":"flows.get","args":{"id":"…"}}` | `id` |
| Create/update flow | `POST /flows` | `{"tool":"flows.save","args":{…flow}}` | the full `Flow` (no `workspace`) |
| Delete flow | `DELETE /flows/{id}` | `{"tool":"flows.delete","args":{"id":"…"}}` | `id` |
| Run a flow | `POST /flows/{id}/run` | `{"tool":"flows.run","args":{"id":"…","params":{},"ts":…,"run_id"?,"entry"?}}` | `id,params?,ts*,run_id?,entry?` |
| Run snapshot | `GET /flows/runs/{run_id}` | `{"tool":"flows.runs.get","args":{"run_id":"…"}}` | `run_id` |
| List a flow's runs | `GET /flows/{id}/runs?status=` | `{"tool":"flows.runs.list","args":{"flow_id":"…","status"?}}` | `flow_id,status?` |
| Live run feed (SSE) | `GET /flows/runs/{run_id}/stream?token=…` | — (stream only) | `run_id` |
| Suspend / resume / cancel | `POST /flows/runs/{run_id}/{op}` | `{"tool":"flows.<op>","args":{"run_id":"…","ts":…}}` | `run_id,ts*` |
| Patch a live run's node | `POST /flows/runs/{run_id}/patch` | `{"tool":"flows.patch_run","args":{"run_id":"…","node":"…","config":{…}}}` | `run_id,node,config` |
| Read one saved node's config | `GET /flows/node/{id}/{node}` | `{"tool":"flows.node.get","args":{"id":"…","node":"…"}}` | `id,node` |
| Edit one saved node's config | `POST /flows/node/{id}/{node}` | `{"tool":"flows.node.update","args":{"id":"…","node":"…","config":{…}}}` | `id,node,config` |
| Persistent runtime view | `GET /flows/{id}/node_state` | `{"tool":"flows.node_state","args":{"id":"…"}}` | `id` |
| Enable / disable | `POST /flows/{id}/enable` | `{"tool":"flows.enable","args":{"id":"…","enabled":true,"start_on_boot":false}}` | `id,enabled,start_on_boot?` |
| Inject a retained input / fire | `POST /flows/{id}/inject` | `{"tool":"flows.inject","args":{"id":"…","node":"…","value":…,"port"?,"ts":…}}` | `id,node,value,port?,ts*` |

`* ts` — a caller-supplied **millisecond logical clock** (README §3: no wall-clock inside a verb). The
**dedicated REST routes fill `ts` from the gateway clock**, so their bodies OMIT it; the **`/mcp/call`
path requires you to pass `ts`** on `run`/`resume`/`suspend`/`cancel`/`inject`. `flows.save` is an
idempotent **UPSERT on `id`** — read-modify-write (see §7).

## 3. The flow model

A flow is a validated DAG. `flows.save` takes the whole record (the host sets `workspace` from the token):

```jsonc
{
  "id": "temps", "name": "Sensor pipeline",
  "version": 0,                       // monotonic; the host bumps it on every save/edit
  "params": {},                        // flow-level params (read as ${params.x})
  "failurePolicy": "halt",            // "halt" (prune the failed subtree) | "continue"
  "enabled": true, "startOnBoot": false, "placement": "either",
  "nodes": [ /* … */ ]
}
```

A **node**:

```jsonc
{
  "id": "scale",                       // unique within the flow
  "type": "range",                     // a built-in type or "<ext_id>.<type>"
  "needs": ["parse"],                  // upstream node ids (the DAG edges)
  "with": { },                         // input bindings (the message), see §5
  "config": { "inMin":0,"inMax":1023,"outMin":-40,"outMax":125,"clamp":true }
}
```

**The message envelope.** Every node has an input port `payload` (+ `topic` carried alongside) and emits
`payload`. A node with **one** upstream and no explicit `with.payload` **auto-wires** — it receives the
upstream's whole envelope. `topic` (and the sequence `parts`) carry forward down a linear chain.

**Versioning.** A run **pins** the flow version it started on, so editing a flow (a new version) never
mutates a live run. A structural edit → the next run; a live run's unexecuted node → `flows.patch_run`.

## 4. The node palette — `flows.nodes`

`flows.nodes` returns the merged registry: the **28 built-ins** ∪ every installed extension's `[[node]]`
descriptors, each with its `type`, `kind`, ports, and an inline **JSON-Schema 2020-12** `config` the form
renders from. Read it to discover types + their config shape before you author.

### The 28 built-in node types

| Category | Types | Notes |
|---|---|---|
| **Flow (spine)** | `trigger` `tool` `rhai` `count` `json` `counter` `subflow` `sink` | entry / any-MCP-verb / rhai cage / size / JSON parse↔stringify / running total / child graph / terminal write |
| **Data** | `change` `select` `merge` `map` `flatten` `sort` `range` `aggregate` `template` `unique` | pure reshape/scale/reduce (no `rhai` needed) |
| **Parse** | `csv` `xml` `yaml` `base64` | text ↔ structure; **malformed input FAILS the node** (like `json`) |
| **Sequence** | `split` `join` `batch` | array-carry sequence + durable grouping |
| **Function** | `filter` `switch` `delay` | report-by-exception / conditional routing / durable delay+rate-limit |

The data/JSON pack config shapes (all pure `payload → payload` unless noted):

- `change` `{ops:[{op:"set",path,value}|{op:"delete",path}|{op:"move",from,to}|{op:"copy",from,to}]}`
- `select` `{paths:["a","b.c"]}` · `merge` `{}` (payload = array of objects → deep-merged)
- `map` `{ops:[…]}` (a `change` op set per array element) · `flatten` `{depth?}`
- `sort` `{path?,order?:"asc"|"desc",numeric?}` · `range` `{inMin,inMax,outMin,outMax,clamp?}`
- `aggregate` `{op:"sum"|"min"|"max"|"mean"|"count"|"concat",path?,sep?}` · `template` `{template:"{{dot.path}}"}`
- `csv`/`xml`/`yaml` `{mode:"parse"|"stringify",…}` · `base64` `{mode:"encode"|"decode"}`
- `split` `{id?}` (array → one settle carrying the array + a `parts` descriptor) · `join` `{}` (recombine from `parts`)
- `batch` `{count}` (group N firings → array; buffers + suppresses until the count; capped force-release)
- `filter` `{mode:"changed"|"deadband",deadband?,path?}` (report-by-exception — pass only on change)
- `unique` `{mode:"array"|"stream",path?}` (dedupe an array, or a durable cross-firing seen-set)
- `switch` `{property?,stop_on_first?,rules:[{op,value?,to:[node_ids]}]}` — **routes**: fires only the
  dependents a matched rule names (`op` ∈ eq/neq/lt/lte/gt/gte/contains/in/truthy/falsy/exists/missing/`else`;
  `else` is the fallthrough — only when nothing else matched)
- `delay` `{mode:"delay",ms}` (hold, durable — resumes after restart) or `{mode:"rate",rate_ms}` (space releases)
- `trigger` `{mode:"manual"|"cron"|"event"|"inject"|"boot",cron?,series?,topic?,inject_mode?}`
- `tool` `{verb,args}` (dispatches any MCP verb under YOUR cap) · `rhai` `{source}` · `subflow` `{flow:"id@version"}`
- `sink` `{target:"inbox"|"outbox"|"channel"|"series",name?}` (terminal; destination = `msg.topic ?? name`)

## 5. Wiring — the binding grammar (`with`)

An edge carries **one whole-value reference or a literal** — no templating. In a node's `with`:

- `"${steps.<id>.payload}"` — a field path into an upstream node's envelope (`.payload`, `.topic`,
  `.findings`, `.payload.items.0`, …). `"${steps.<id>}"` = the whole envelope.
- `"${params.<name>}"` — a flow param.
- a literal (any JSON scalar/object/array) — e.g. `{"payload":[1,2,3]}` seeds a value directly.

Single-upstream nodes usually need **no** `with` (auto-wire). A **join** (≥2 upstreams) MUST bind
`payload` explicitly (the save-time lint rejects an unbound join).

## 6. Run, watch, inspect

`flows.run` returns a `{run_id}` and drives in the **background**. Read `flows.runs.get {run_id}` for the
per-node snapshot — `status` ∈ `pending|success|partialFailure|failed|suspended|cancelled`, and
`steps[]` each `{id, outcome: "ok"|"err"|"skipped", output: {payload,…}}`. A `switch`-gated or RBE-
suppressed branch records `outcome:"skipped"`. Poll until terminal, or open the SSE feed
`GET /flows/runs/{run_id}/stream?token=$TOKEN` (folds a `snapshot` then per-node deltas + `run-finished`).

Lifecycle: `flows.suspend` → `flows.resume` (re-drives the remaining frontier; also how a `delay` park
resumes) → or `flows.cancel` (terminal, non-resumable). `flows.patch_run {run_id, node, config}` changes
an **unexecuted** node's config for a live run (validated against its pinned schema).

`flows.node_state {id}` is the **persistent** view — every node's last-value (survives restart, updated
in place) + the flow's armed fields — the steady state to paint when there's no live run.

## 7. Read-modify-write (the rule that bites)

`flows.save` **replaces the whole record** (all `nodes`). To add/edit ONE node you must `flows.get`, mutate
the `nodes` list, and `save` the full list back — or use `flows.node.update {id, node, config}` to change
exactly one node's config without re-posting the graph (it bumps the version like any edit).

## 8. Worked example — build & run a data pipeline

Raw sensor JSON string → parse → split per reading → scale each → route hot vs cold → read the output.

```bash
python3 - <<'PY'
import json, os, time, urllib.request
TOK=os.environ["TOKEN"]; BASE="http://127.0.0.1:8080/mcp/call"
def call(tool,args):
    r=urllib.request.Request(BASE,json.dumps({"tool":tool,"args":args}).encode(),
        {"authorization":f"Bearer {TOK}","content-type":"application/json"})
    return json.load(urllib.request.urlopen(r))
now=lambda: int(time.time()*1000)

flow={
  "id":"temps","name":"Sensor pipeline","version":0,"params":{},"failurePolicy":"continue",
  "enabled":True,"startOnBoot":False,"placement":"either",
  "nodes":[
    # a manual trigger seeded with a JSON string payload (literal binding)
    {"id":"in","type":"trigger","needs":[],
     "with":{"payload":"[{\"id\":\"a\",\"raw\":512},{\"id\":\"b\",\"raw\":1023}]"},"config":{"mode":"manual"}},
    {"id":"parse","type":"json","needs":["in"],"with":{},"config":{"mode":"parse"}},
    {"id":"split","type":"split","needs":["parse"],"with":{},"config":{}},
    # scale each reading's raw 0..1023 → -40..125 via a per-element map op is NOT range;
    # range scales a scalar. Here we aggregate the split array's mean as a demo of an array-native node:
    {"id":"mean","type":"aggregate","needs":["split"],"with":{},"config":{"op":"mean","path":"raw"}},
    # route on the mean: >800 → hot, else cold (only the matched branch runs)
    {"id":"route","type":"switch","needs":["mean"],"with":{},
     "config":{"rules":[{"op":"gt","value":800,"to":["hot"]},{"op":"else","to":["cold"]}]}},
    {"id":"hot","type":"count","needs":["route"],"with":{},"config":{}},
    {"id":"cold","type":"count","needs":["route"],"with":{},"config":{}},
  ],
}
print("save:", call("flows.save", flow))               # → {id, version}
run=call("flows.run", {"id":"temps","params":{},"ts":now()})   # ts REQUIRED on /mcp/call
rid=run["run_id"]
for _ in range(200):                                    # poll to terminal (background drive)
    snap=call("flows.runs.get", {"run_id":rid})
    if snap["status"] in ("success","partialFailure","failed","cancelled"): break
    time.sleep(0.02)
print("status:", snap["status"])
for s in snap["steps"]:
    print(" ", s["id"], s["outcome"], s.get("output",{}).get("payload"))
# mean of [512,1023] = 767.5 → NOT >800 → 'cold' runs (ok), 'hot' is gated (skipped)
PY
```

Edit one node later without re-posting the graph:

```bash
curl -s -X POST http://127.0.0.1:8080/flows/node/temps/route -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"config":{"rules":[{"op":"gte","value":700,"to":["hot"]},{"op":"else","to":["cold"]}]}}'
```

## 9. Retained inputs & control loops (`flows.inject`)

A `trigger` in `inject` mode with `inject_mode:"retain"` holds a value in `flow_input:{ws}:{flow}:{node}`
that **every future run reads** — a slider/switch driving a control loop without a long-lived run. Inject
with a `port` to set a named port; omit it for the node-level `payload`. `inject_mode:"fire"` (or a firing
trigger) starts a one-shot run instead. `flows.node_state` reads the retained values back (`input`/`inputs`).

## Gotchas

- **Workspace/owner come from the token**, never args. To act in another workspace, `login` into it.
- **`ts` is required on `/mcp/call`** run/resume/suspend/cancel/inject; the REST routes fill it themselves.
- **`flows.save` is a full UPSERT of `nodes`** — read-modify-write, or use `flows.node.update` for one node.
- **A join (≥2 upstreams) MUST bind `payload`** in `with`, else save rejects it (the join lint).
- **Parse nodes FAIL on bad input** (`csv`/`xml`/`yaml`/`base64`/`json`) — the node records `err`, not a
  wrong shape; under `failurePolicy:"halt"` that prunes the subtree.
- **`switch`/`filter`/`batch`/`unique` suppress** by settling `skipped` — a gated/suppressed branch does
  NOT run (assert `outcome:"skipped"`, not a missing step).
- **Array-carry `split`/`join`**: a scalar node (`range`/`filter`) between them sees the whole array, not
  an element — use array-native `map`/`sort`/`aggregate` for per-element work.
- **`delay` parks the run** (`status:"suspended"`); a `flows.resume` with an advanced `ts` releases it.
- **Denials are opaque** — a missing cap and a missing flow both surface as forbidden/absent; check your
  token's caps (and the node-tool's own cap for `tool`/`sink` nodes) if a call "vanishes".
- **A running dev node does NOT hot-reload Rust** — if you changed engine `src/*.rs`, rebuild + restart
  before driving the live canvas.

## Related

- Flow model + Decisions (the source of truth): `doc-site/content/public/flows/flows.mdx`;
  spine `docs/scope/flows/flows-scope.md` (Decisions 1–16), descriptor `docs/scope/flows/node-descriptor-scope.md`.
- The data/JSON node pack (the 20 data nodes): `docs/scope/flows/data-nodes-scope.md`,
  session `docs/sessions/flows/data-nodes-session.md`.
- The gateway routes: `role/gateway/src/routes/flows.rs`; the host verbs: `crates/host/src/flows/`.
- Capability / workspace rules: `README.md` §3, §6, §7; `docs/scope/auth-caps/`.
- Sibling skill (same transport model): `docs/skills/dashboard-mcp/SKILL.md`.
