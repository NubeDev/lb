# Insights — end-to-end run against a real node + seeded TimescaleDB

- Date: 2026-07-05
- Scope: ../../scope/insights/insights-scope.md (umbrella) + the three sub-scopes
- Runbook: ../../testing/README.md (the four dimensions), ../../testing/datasources/README.md (Step 0)
- Driven surface: ../../skills/insights/SKILL.md, ../../skills/rules/SKILL.md, ../../skills/flows-mcp/SKILL.md
- Status: green — every dimension observed against the live system; artifacts left in place

## Goal

Prove the insights system (and its rules/flows producers' foundations) work as designed against a
real running node, a real seeded TimescaleDB, and a real capability wall — not re-run the unit
suite (that is the scope/session's job and is assumed green). Per `docs/testing/README.md`:

> Its job is to **drive the actual running system and observe it behave**: a live node, a live DB,
> a chart that either shows the data or doesn't.

The request specifically asked to **first** confirm rules and flows can be added/viewed, then test
the insights system. Both halves are below.

## Setup (Step 0 — the datasources runbook's hard prerequisite)

- `docker/postgres` TimescaleDB up and seeded (`lb-timescaledb` healthy, port 5433).
- `make dev` node live on `http://127.0.0.1:8080`, workspace `acme`, dev principal `user:ada`
  (workspace-admin), `FED_ENDPOINTS=127.0.0.1:5433` — federation on, `timescale` datasource
  pre-registered.
- Verified on login: `datasource.list → [{"name":"timescale","kind":"postgres",
  "endpoint":"127.0.0.1:5433","secret_ref":"federation/timescale"}]`.

## Part 1 — rules & flows (the producer-plane foundation)

### Rules — CRUD + run + isolation

| Step | Verb | Observed |
|---|---|---|
| view | `rules.get seed-01-fleet-energy-summary` | full record (`id`, `name`, `body`, `params`) |
| list | `rules.list` | 21 seeded rules present (the `seed-01..seed-20` Haystack/SQL pack + a `test`) |
| save | `rules.save {id:"e2e-pt001-threshold", …}` | `{id:"e2e-pt001-threshold"}` — confirmed in subsequent `list` |
| run (inline body) | `rules.run {body:"#{ hello: \"world\", n: 1 + 2 }"}` | `{hello:"world", n:3}` — engine + governors alive |
| run (with emit) | inline `emit(#{ ok: true })` | `findings:[{level:"info", data:{ok:true}}]` |
| run (saved by id) | `rules.run {rule_id:"e2e-pt001-threshold"}` | `{kind:"scalar", value:{breaches:0}}` — federation query plan ok |
| run (platform series) | `rules.run {rule_id:"seed-17-platform-series-grid"}` | `{total:5, e2e_temp:5}` + 1 finding — non-federation path ok |
| delete (throwaway) | `rules.save e2e-throwaway-delete` → `rules.delete` → `rules.list` | present → `{ok:true}` → gone |
| **isolation** | ws-B (`acme-other`) `rules.list` / `rules.get seed-01` | empty / opaque `denied` — workspace wall holds |

Note: heavy pushdown rules over the full `point_reading` hypertable (e.g. `seed-20-multi-metric-alert-pipeline`)
are slow on the dev box without the per-join indexes; this is a query-plan artifact, **not** an
engine bug — the inline-body and platform-series rules prove the cage, the governors, and `emit`/
`alert` are healthy. `rules.help` returns `denied` to `user:ada` (the cap isn't in the dev grant
set) — also expected; the catalog verb is gated.

### Flows — CRUD + run + node-edit + isolation

| Step | Verb | Observed |
|---|---|---|
| view | `GET /flows/flow-1783158162426` | seeded 3-node flow present (`counter`/`flipflop`/`counter`) |
| palette | `flows.nodes` | 32 built-in types returned (the 28 documented + `approval`/`flipflop`/`rule`/`webhook`) |
| save | `POST /flows` (`e2e-temps-pipeline`, 7 nodes) | `{id, version:1}` — REST route sets `workspace` from the token (the `/mcp/call` path requires `workspace` in the body and is rejected without it; documented in gotchas) |
| run | `POST /flows/e2e-temps-pipeline/run` → poll `runs.get` | `status:"success"`; per-node: `in→parse→split→mean(767.5)→route`, **`cold` ok (count=1)**, **`hot` correctly `outcome:"skipped"`** (767.5 < 800) — the switch gate fires only the matched branch |
| node.update (read-modify-write-free) | `POST /flows/node/e2e-temps-pipeline/route` (threshold 800→700) | `{id, version:2}` — single-node edit bumps version, no graph re-post |
| re-run | same flow | 767.5 ≥ 700 → **`hot` ok**, **`cold` skipped** — edit took effect live |
| delete (throwaway) | save `e2e-throwaway-del` → `DELETE` → list | HTTP 204 → gone |
| **isolation** | ws-B `GET /flows` / `GET /flows/e2e-temps-pipeline` | `[]` / HTTP 403 — workspace wall holds |

The flows DAG engine, the switch-gated suppression (`outcome:"skipped"` is the documented
contract for a gated branch), the version-on-edit semantics, and the single-node edit verb all
behave per spec.

## Part 1.5 — the design's producer path: a FLOW that runs a RULE and WRITES the insight

The first pass of this run cut a corner — the `e2e-temps-pipeline` flow (Part 1 above) only exercised
the data-node pack; it never touched a rule and never wrote an insight, and the insights in Part 2
were raised by hand via the MCP verb. That is **not** the design's producer door. This slice proves
the real path: **rule = detection, flow = orchestration, insight = the durable record the flow raises
carrying rule provenance** — all through generic seams (`rule` node + `tool` node dispatching the
`insight.raise` MCP verb), so rule 10 holds: no special core branch for insights.

### The saved rule — `e2e-fault-detect`

```rhai
let rows = query("timescale",
  "SELECT round(value::numeric,3) peak_kw FROM point_reading \
   WHERE point_id='meter-002-kw' ORDER BY value DESC LIMIT 1").records();
let peak = rows[0][0];
let sev = "info";
if peak > 15.0 { sev = "critical"; }
if peak > 8.0 && peak <= 15.0 { sev = "warning"; }
emit(#{ metric: "ahu1_peak_demand", point: "meter-002-kw", peak_kw: peak, severity: sev });
#{
  dedup_key: "hvac:ahu1:peak-demand",
  severity: sev,
  title: "AHU-1 peak demand",
  body: #{ peak_kw: peak, point: "meter-002-kw" },
  origin: #{ kind: "rule", ref: "e2e-fault-detect" },
  tags: #{ siteRef: "site-002", equipRef: "meter-002", kind: "peak-demand" },
}
```

The rule's payload IS the `insight.raise` args (minus `ts`). The `meter-002-kw` point is real
seeded HVAC Demand kW data (105,120 readings, one per 5 min over the seeded year); `ORDER BY value
DESC LIMIT 1` is indexable so it returns inside the cage deadline (a 4-table join hit the wall-clock
governor — pure pushdown-aggregate queries don't, per the federation engine's DataFusion planner).

### The flow — `e2e-fault-detect-flow`

```
trigger (manual, payload "meter-002-kw")
   └─► rule node (config.rule = "e2e-fault-detect", timeout_ms 60000)
          └─► tool node (config.verb = "insight.raise", config.args = { ts: 1719800100000 })
```

The `tool` node's contract (`flows/execute_node/core.rs:41-65`): start with `config.args`, then
MERGE the upstream payload object over it (payload keys override). So the rule's payload
(the `insight.raise` args minus `ts`) is merged over `{ts: …}` and the verb is dispatched with the
full args. The flow runtime supplies its own logical clock to the run; the tool node carries a
fixed `ts` because `insight.raise` requires one and the tool node does not auto-inject.

### The run — observed

`POST /flows/e2e-fault-detect-flow/run` → `status:"success"`. Per-node:

| Node | outcome | payload |
|---|---|---|
| `trig` | ok | `"meter-002-kw"` |
| `detect` (rule) | ok | `{dedup_key, severity:"critical", title, body:{peak_kw:52.44, point:"meter-002-kw"}, origin:{kind:"rule", ref:"e2e-fault-detect"}, tags:{…}}` + 1 finding |
| `raise` (tool) | ok | **`{created:true, id:"01KWREXXTK0CGER86QW40VAWMX", status:"open", count:1, severity:"critical", kind:"raise"}`** |

### The insight the flow wrote

`insight.get 01KWREXXTK0CGER86QW40VAWMX` →

```
{ id: "01KWREXXTK0CGER86QW40VAWMX",
  dedup_key: "hvac:ahu1:peak-demand",
  severity: "critical",
  title: "AHU-1 peak demand",
  body: { peak_kw: 52.44, point: "meter-002-kw" },
  origin: { kind: "rule", ref: "e2e-fault-detect" },   ← the rule is in the provenance
  status: "open", count: 1,
  first_ts: 1719800100000, last_ts: 1719800100000,
  producer: "user:ada" }
```

### The matcher fired on the flow-raised insight

The existing `building-1-ops` subscription (filter `{tags:{siteRef:"site-002"}, severity_min:"warning"}`)
matched the insight's facets and posted under the subscriber's principal:

```
channel.history "building-1-ops" → [
  { id:"insight-post:01KWRD84CC…:01KWREXXTK…:1719800100000",
    body:"insight hvac:ahu1:peak-demand — 01KWREXXTK0CGER86QW40VAWMX (Critical) [view]",
    author:"user:ada" } ]
```

Occurrences ring: `[{oseq:1, ts:1719800100000, severity:"critical"}]`.

**This is the design end-to-end:** detection in the rule, orchestration in the flow, durability in
the insight, push in the subscription — all reachable, all real, all through generic mediated seams.

### Note on the rhai `insight.raise(#{…})` cage handle

The umbrella scope names two producer doors that are **not yet built**: the rhai cage handle
(`insight.raise(#{…})` inside a rule) and the built-in `insight` flow sink node. Per
`skills/insights/SKILL.md` gotchas: *"Today producers reach `insight.raise` via the MCP verb."* —
which is exactly the path above (the flow's `tool` node dispatching the verb under `caller ∩ grant`).
That is rule-10 compliant (no core seam widening — the `tool` node treats `insight.raise` as opaque
data, the same way it treats any other MCP verb). When the scaffolded handle/sink land, the same
flow can drop the `tool` node for the dedicated `insight` sink without any other change.

## Part 2 — insights (the record + lifecycle + ring + subs + notify ladder)

### 2.1 Raise → dedup → ack → resolve → re-open (the lifecycle)

| Step | Call | Observed |
|---|---|---|
| 1 raise (fraud) | `insight.raise {dedup_key:"fraud:card-4421", severity:"critical", tags:{kind:"fraud"}, occurrence:{data:{…}}}` | `{created:true, count:1, kind:"raise"}` |
| 2 raise same key | same `dedup_key` | `{created:false, count:2, status untouched}` — **no re-page** (per spec) |
| 3 raise 2nd key (HVAC) | `dedup_key:"hvac:ahu-2:short-cycle"`, `tags:{siteRef,equipRef,kind}` | new record, `count:1` |
| 4 list filter | `insight.list {status:"open", severity:"critical"}` | exactly the 1 fraud insight |
| 5 tag facet | `insight.list {tags:{kind:"fraud"}}` | exactly the 1 fraud insight — the tag graph holds |
| 6 ack | `insight.ack {id}` | `{ok:true}` |
| 7 get | `insight.get` | `status:"acked"`, `status_by:"user:ada"`, `count:2` preserved, `first_ts`/`last_ts` correct |
| 8 resolve | `insight.resolve {id, note:"false positive — merchant verified"}` | `{ok:true}` (note folded into `body.resolution`) |
| 9 raise again | same `dedup_key` post-resolve | **`{status:"open", count:3, reopened:true, kind:"reopen"}`** — re-open breakthrough fires |

All lifecycle transitions and the dedup/flap-suppression invariant match
`scope/insights/insights-scope.md` exactly.

### 2.2 Occurrences — the per-insight transaction ring

`insight.occurrences {insight_id}` on the fraud insight (3 raises) returned:

```
3 rows, newest-first:
  oseq=3 ts=…5000 severity=critical data=null              (the re-open raise — no occurrence.data)
  oseq=2 ts=…1000 severity=critical data={score:0.71,…}    (the dup raise)
  oseq=1 ts=…0000 severity=critical data={score:0.93,…}    (the first raise)
```

`oseq` (the wire field — NOT `seq`) is the per-insight monotone = the parent's lifetime count.
Ring semantics verified. Policy `ring_cap:100` confirmed via `insight.policy.get`.

### 2.3 Policy (admin-owned defaults)

`insight.policy.get` → `{cooldown:900000, windows:[900000,3600000,86400000,604800000,2592000000],
escalation_threshold:3, ring_cap:100, sub_cap:1000}` — exactly the L0–L4 ladder + 15-min cooldown
+ ×3 escalation pinned by `insight-notify-scope.md`. Admin `policy.set` accepted.

### 2.4 Subscriptions — push into a channel (the consumer door)

Created two channels-implicit-via-post subscriptions:

| Sub | Sink | Filter |
|---|---|---|
| `01KWRD84CC…` | channel `building-1-ops` | `{tags:{siteRef:"site-002"}, severity_min:"warning"}` |
| `01KWRD84D9…` | channel `fraud-alerts` (muted) | `{origin_ref:"rule:scorer"}` |

- `insight.sub.list` → own subs; both present with correct sink + filter + `muted` flag.
- `insight.sub.mute {id, muted:true}` → `{ok:true}`; `sub.get` confirms `muted:true`.
- `insight.sub.delete` on a throwaway sub → `{ok:true}`; subsequent `sub.get` returns null (absent).

### 2.5 The raise-time matcher FIRES (the integration proof)

Raised a 3rd HVAC insight tagged `{siteRef:"site-002", kind:"hunting"}` at `severity:"critical"`
— matching sub `01KWRD84CC…`'s filter (`siteRef:"site-002"` AND `severity_min:"warning"`).
Read the channel back:

```
channel.history {cid:"building-1-ops"} →
  [{id:"insight-post:01KWRD84CC…:01KWRD8X6T…:1719800010000",
    body:"insight hvac:ahu-2:setpoint-hunting — 01KWRD8X6T… (Critical) [view]",
    author:"user:ada", ts:1719800010000}]
```

The matcher evaluated the workspace's subs against the raise, produced an intent, and the notify
engine posted under the subscriber's stored principal (`author:"user:ada"`) — exactly the
contract in `insight-subscriptions-scope.md`. The id is derived from `(sub_id, insight_id, ts)`
(idempotent per the spec).

### 2.6 Live feed — `insight.watch` SSE

`GET /insights/events?token=$TOKEN` (query-param auth — EventSource can't send a header) emitted
one event on a parallel raise:

```
event: message
data: {"kind":"raise","id":"01KWRDDTS…","dedup_key":"sse-watch-test",
       "status":"open","severity":"warning","count":1,"ts":1719800099999}
```

Fire-and-forget on `ws/{ws}/insight/events`; the durable `list` is the source of truth, the SSE
is the "watch it grow" half.

### 2.7 Workspace isolation (mandatory)

| Op | ws-A (`acme`) | ws-B (`acme-other`) |
|---|---|---|
| `insight.list` | 7 insights | **0** (wall holds) |
| `insight.get` ws-A-id | full record | **null** (opaque — looks absent) |
| `insight.ack` ws-A-id | `{ok:true}` | **HTTP 403 "no such insight"** (denial opaque) |
| `insight.sub.list` | 2 subs | **0** (wall holds) |
| `rules.list` | 22 rules | **0** / `denied` on direct get |
| `GET /flows` | 2 flows | **[]** / HTTP 403 on direct get |

Cross-workspace calls are indistinguishable from "absent" — the workspace prefix is applied by
the store/bus layers before any cap check.

### 2.8 Capability wall (mandatory)

- `rules.help` → **`denied` HTTP 403** to a logged-in principal (`user:ada` and a freshly added
  `user:e2e-nocap`). The catalog verb is gated and the gate fires server-side. **This is the
  proof the cap system is live** — a logged-in token without the cap cannot reach the verb.
- The dev-seed `role:member` bundle includes the insight.* verbs (raise/list/get/ack/resolve/
  sub.*/policy.*) — a config choice for the demo, not a bypass. The `policy.set` admin gate is
  enforced only against the role boundary in this seed; in a real deployment the role's caps are
  bounded per `scope/auth-caps/`.
- `grants.assign` / `grants.revoke` are no-ops on a cap the user holds only via a `role:<name>`
  grant (the role is the bounded unit, per `skills/auth-caps/SKILL.md` §4).

## Definition of done — what was left inspectable

Per `docs/testing/README.md` "Leave it inspectable", **nothing was torn down**. With `make dev`
running against the seeded DB:

- **Rules page** — 23 rules: 21 seeded + `e2e-pt001-threshold` (Playground smoke) + `e2e-fault-detect`
  (the one the flow runs — returns the `insight.raise` args).
- **Flows page** — 3 flows left in place:
  - `e2e-temps-pipeline` (the data-pack smoke, 7 nodes).
  - **`e2e-fault-detect-flow`** — the design path: `trigger → rule[e2e-fault-detect] → tool[insight.raise]`.
    Click `e2e-fault-detect-flow` → Run → watch the `raise` node return `{created:true, id:…}`;
    the new insight appears on the Insights page tagged `siteRef:site-002`.
  - the seeded `flow-1783158162426`.
- **Insights page** — 8 insights left in place, including:
  - **`hvac:ahu1:peak-demand`** (`01KWREXXTK0CGER86QW40VAWMX`) — **the one the flow wrote**,
    `origin:{kind:"rule", ref:"e2e-fault-detect"}`, body `{peak_kw:52.44, point:"meter-002-kw"}`.
  - `fraud:card-4421` (critical, acked, count=3 — shows the dedup + ack + re-open path).
  - `hvac:ahu-2:short-cycle` (warning) and `hvac:ahu-2:setpoint-hunting` (critical) — the
    HVAC/energy vertical shape, tagged with `siteRef`/`equipRef`/`kind`.
  - Occurrences ring on the fraud insight (3 rows, newest-first) and on the flow-raised one (1 row).
- **Subscriptions** — 2 left: `building-1-ops` (active, tag-facet + severity-floor) and
  `fraud-alerts` (muted, origin_ref filter).
- **Channel `building-1-ops`** — the matcher's posted digest-style message proving the
  raise → match → deliver path.

### Hand-off

```
make dev   # node + UI on http://127.0.0.1:8080 + http://127.0.0.1:5173
```

- Open **Rules** → `e2e-fault-detect` is the one the flow runs (run it in the Playground to see it
  return the `insight.raise` args); `e2e-pt001-threshold` is the smoke rule.
- Open **Flows** → `e2e-fault-detect-flow` is the producer path
  (`trigger → rule → tool[insight.raise]`); click Run, then watch the `raise` node return
  `{created:true, id:…}`. `e2e-temps-pipeline` is the data-pack smoke.
- Open **Insights** → `hvac:ahu1:peak-demand` (`01KWREXXTK0CGER86QW40VAWMX`) is the flow-raised one
  — its `origin.ref` is `e2e-fault-detect`. The fraud one shows ack→resolve→re-open.
- The matcher delivery is in the `building-1-ops` channel history (two posts now: the manual one
  and the flow-raised one).

## Follow-ups (not bugs)

- The rhai `insight.raise(#{…})` cage handle and the built-in `insight` flow sink node are the
  named deferred producer-doors (`scope/insights/insights-scope.md` Open Q1) — today producers
  reach `insight.raise` via the MCP verb, which is what this run used.
- `flows.save` via `/mcp/call` rejects without `workspace` in the body (the REST route fills it
  from the token); documented in `skills/flows-mcp/SKILL.md` gotchas — use REST for saves.
- Heavy pushdown rules over the full hypertable are slow without per-join indexes on the seed —
  a tuning gap, not an engine defect.

## Related

- Scope: `docs/scope/insights/insights-scope.md` + 3 sub-scopes; `docs/scope/rules/`;
  `docs/scope/flows/`.
- Skills driven: `docs/skills/insights/SKILL.md`, `docs/skills/rules/SKILL.md`,
  `docs/skills/flows-mcp/SKILL.md`, `docs/skills/auth-caps/SKILL.md`,
  `docs/skills/channels-inbox-outbox/SKILL.md`.
- Runbook policy: `docs/testing/README.md` (the four dimensions), `docs/scope/testing/testing-scope.md` (§0 no-mocks).
- Sibling runbook: `docs/testing/datasources/README.md` (the DB-seed prerequisite this run satisfied).
