---
name: e2e-insights
description: >
  Use when asked to end-to-end test INSIGHTS — the durable data-finding record (`insight.*` +
  `/insights` REST/SSE). Drive a REAL running node over the REAL surface: raise → dedup → list →
  ack → resolve → re-open, the occurrence ring, channel subscriptions + the digest ladder, and the
  live SSE feed. Then prove the RULE PRODUCER DOOR — a Rhai rule that `insight.raise(#{…})`s a
  durable, deduped fault over the seeded `demo-buildings` data. No mocks — real node, real caps,
  real datasource. Assumes suites are green.
---

# E2e insights runbook — prove the durable data-finding record works as designed

Status: scope (the standard). Design intent:
[`../../scope/insights/insights-scope.md`](../../scope/insights/insights-scope.md) (umbrella) +
`insight-occurrences-scope.md` + `insight-subscriptions-scope.md` + `insight-notify-scope.md` +
[`../../scope/insights/rule-raises-insight-scope.md`](../../scope/insights/rule-raises-insight-scope.md).
Drivable surface: [`../../skills/insights/SKILL.md`](../../skills/insights/SKILL.md).
Checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** The `cargo test -p lb-host` insight suites
are the **scope/session's** job and assumed green — this runbook does **not** re-run them. Its job is
to **drive a live node over the real `insight.*` surface and observe it behave**: an insight that
raises, dedups, lists, acks, resolves, re-opens, and (via a rule) is raised from real data.

**No mocks (testing-scope §0).** `insight.*` is an in-process host service — you drive it for real
over the node's MCP/REST bridge; the rule producer path reads the **real** seeded `demo-buildings`
SQLite datasource, never a fake.

---

## Step 0 — stand up the node (and, for the rule path, seed the datasource)

The direct `insight.*` checks (Steps 2.1–2.4) need only a running node. The **rule producer door**
(Step 2.5) additionally needs the `demo-buildings` datasource — seed it the Docker-free way (see
[`../datasources/README.md`](../datasources/README.md) Step 0):

```bash
make build-wasm && make dev          # boot the node (root on 8080)
make seed-demo-sqlite                # ONLY needed for Step 2.5 (the rule producer path)
```

Authenticate (workspace + principal come from the token — the hard wall):

```bash
BASE=http://127.0.0.1:8080/mcp/call
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
auth=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')
```

---

## Step 1 — read the design (what is "correct"?)

- **[`../../scope/insights/insights-scope.md`](../../scope/insights/insights-scope.md)** — the record
  (`insight:{ws}:{id}`), severity set (`info|warning|critical`), the `open → acked → resolved`
  lifecycle, dedup on `(ws, dedup_key)`, host-forced `producer`/`origin.kind`.
- **[`../../skills/insights/SKILL.md`](../../skills/insights/SKILL.md)** — the drivable verbs
  (`insight.raise|get|list|ack|resolve|occurrences|sub.*|policy.*|watch`), the wire shapes, the digest
  ladder, the kill switch.
- **Identity lives in `dedup_key`/`body`, NEVER the title or tags** — the load-bearing invariant to
  assert (tags are low-cardinality facets; per-entity identity in a tag value blows the tag-node cap).

---

## Step 2 — the checklist (drive it, observe it works as designed)

The four dimensions from [`../README.md`](../README.md#what-to-check--the-functional-dimensions),
plus the rule producer door.

### 2.1 CRUD + lifecycle — raise → dedup → list → ack → resolve → re-open

The `ts` is a caller-supplied **logical clock** (determinism, README §3) — pass a real monotone value.

```bash
# 1. raise — a critical finding (identity in dedup_key/body, NOT the title)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.raise","args":{
  "dedup_key":"e2e:card-4421","severity":"critical","title":"score above threshold",
  "body":{"score":0.93,"amount":412.50},
  "origin":{"kind":"manual","ref":"e2e-runbook"},
  "tags":{"kind":"fraud"},
  "occurrence":{"data":{"score":0.93,"txn":"t-88123"},"severity":"critical"},
  "ts":1719800000000}}'
# → {"id":"01H…","status":"open","count":1,"created":true,"reopened":false,…}

# 2. DEDUP — same dedup_key ⇒ count bumps, status UNTOUCHED (no re-page)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.raise","args":{
  "dedup_key":"e2e:card-4421","severity":"critical","title":"score above threshold",
  "origin":{"kind":"manual","ref":"e2e-runbook"},"ts":1719800001000}}'
# → {"id":"01H…"(same),"count":2,"created":false,…}

# 3. get + list
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.get","args":{"id":"01H…"}}'
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.list","args":{"status":"open","severity":"critical","limit":50}}'

# 4. ack (open → acked)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.ack","args":{"id":"01H…","ts":1719800002000}}'

# 5. resolve (with a note; idempotent)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.resolve","args":{
  "id":"01H…","note":"false positive — verified","ts":1719800003000}}'

# 6. RE-OPEN — resolved + raise again ⇒ status→open, count continues, kind=reopen (a breakthrough)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.raise","args":{
  "dedup_key":"e2e:card-4421","severity":"critical","title":"score above threshold",
  "origin":{"kind":"manual","ref":"e2e-runbook"},"ts":1719800004000}}'
# → {"status":"open","count":3,"reopened":true,"kind":"reopen",…}
```

**Observe:** dedup upserts (never a second record for the same key); ack/resolve move the status;
a raise after resolve **re-opens** (`reopened:true`). **No `update`/`delete` in v1** — correction is
resolve + raise. The REST twins (`GET /insights`, `POST /insights/{id}/ack|resolve`) inject the clock.

### 2.2 Occurrences — the per-insight evidence ring

```bash
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.occurrences","args":{"insight_id":"01H…","limit":50}}'
# → {"items":[{"oseq":3,"ts":…,"severity":"critical","data":{"score":0.93,"txn":"t-88123"}},…]}
```

**Observe:** the wire field is **`oseq`** (not `seq`); the ring is newest-first, capped (default 100,
oldest evict); the parent `count` is the lifetime truth and MAY exceed stored rows. Prove the **2 KB
`occurrence.data` cap** rejects the *whole raise* as `BadInput` (never a partial write): raise with an
oversize `occurrence.data` and assert the deny + that no orphan row landed.

### 2.3 Subscriptions + the digest ladder — push insights into a channel

`insight.sub.create` is **double-gated**: the verb cap AND `bus:chan/{channel}:pub` at create time
(no-widening up front, re-checked at fire).

```bash
# subscribe a channel to a tag facet + severity floor
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.sub.create","args":{
  "sink":{"kind":"channel","channel":"e2e-ops"},
  "filter":{"tags":{"kind":"fraud"},"severity_min":"warning"},
  "now":1719800000000}}'
# → {"id":"01J…"}
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.sub.list","args":{}}'                 # own subs
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.sub.mute","args":{"id":"01J…","muted":true}}'  # keeps sub, stops delivery
```

**Observe (design behaviors to confirm):** breakthroughs (first-ever key on a sub, severity
escalation, re-open after resolve) deliver immediately at any ladder level; `ack` suppresses per-key
deliveries; a digest is **one message per `(sub, window)`**, idempotent per `(sub, window_start)`.
Admin `insight.policy.get/set` tunes the ladder; the per-member kill switch is
`prefs.set {insight_notifications:false}`.

### 2.4 Permissions & Access — the walls hold

```bash
# no token → 401
curl -s -X POST $BASE -H 'content-type: application/json' -d '{"tool":"insight.list","args":{}}' \
  -o /dev/null -w "%{http_code}\n"   # → 401
```

- **Per-verb cap deny (the negative path is the point):** a member without `mcp:insight.raise:call`
  is refused `insight.raise` (opaque deny); `insight.sub.create` also refuses without the channel's
  `bus:chan/{channel}:pub`. Assert the deny.
- **Host-forced identity:** `producer`/`origin.kind` are stamped from the door + principal — a caller
  can't forge them even by putting them in the args. Raise through the manual door and confirm
  `origin.kind:"manual"` (a rule's handle forces `"rule"`, see 2.5).
- **Workspace wall:** raise in `acme`; sign in to `globex`; confirm `globex`'s `insight.list` does
  not see `acme`'s insight and its `insight.get` on the id is not-found. The SSE subject
  `ws/{ws}/insight/events` is ws-scoped — no cross-ws leak.

```bash
TOK_B=$(curl -s -X POST http://127.0.0.1:8080/login -H 'content-type: application/json' \
  -d '{"user":"user:bob","workspace":"globex"}' | jq -r .token)
curl -s -X POST $BASE -H "authorization: Bearer $TOK_B" -H 'content-type: application/json' \
  -d '{"tool":"insight.get","args":{"id":"01H…"}}'   # → not found (ws-A insight invisible to ws-B)
```

### 2.5 Functional — the RULE PRODUCER DOOR raises a durable insight from real data

This is the payoff and the reason insights composes onto rules: a Rhai rule reads the seeded
`demo-buildings` data and **raises a durable, deduped insight** in-body via the `insight` cage handle
(`insight.raise`/`ack`/`close`). Requires Step 0's `make seed-demo-sqlite`. Drive it through
`rules.run`:

```bash
BODY='// Rank every building by energy intensity (total kWh ÷ floor area from the `area` site tag).
let rows = query("demo-buildings", `
  SELECT s.name AS building,
    ROUND(SUM(pr.value) / CAST(REPLACE(a.val,'"'"' m2'"'"','"'"''"'"') AS DOUBLE), 2) AS kwh_per_m2
  FROM point_reading pr
  JOIN point p ON p.id = pr.point_id
  JOIN meter m ON m.id = p.meter_id
  JOIN site  s ON s.id = m.site_id
  JOIN site_tag a ON a.site_id = s.id AND a.tag = '"'"'area'"'"'
  WHERE p.name = '"'"'Energy kWh'"'"'
  GROUP BY s.id, s.name, a.val
  ORDER BY kwh_per_m2 DESC
`).records();
// Each row is a MAP keyed by the SELECT aliases: r.building, r.kwh_per_m2.
for r in rows {
  let building = r.building;
  let intensity = r.kwh_per_m2;
  let key = "energy-intensity-high:" + building;   // stable per-building identity = the dedup key
  if intensity > 1.0 {
    insight.raise(#{
      dedup_key: key,
      severity: if intensity > 2.0 { "critical" } else { "warning" },
      title: building + " energy intensity high",
      body: #{ building: building, kwh_per_m2: intensity, budget: 1.0 },
      tags: #{ area: "energy", building: building },
    });
  }
}
rows'
curl -s -X POST http://127.0.0.1:8080/mcp/call "${auth[@]}" \
  -d "$(jq -n --arg b "$BODY" '{tool:"rules.run",args:{body:$b,ts:1719800000000}}')"
# → {"output":{"kind":"scalar"},…}  — and the durable insights now exist:
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.list","args":{"tags":{"area":"energy"},"limit":50}}'
```

**Observe the whole bridge working:**
- The `insight.list` returns one insight per over-budget building — `origin.kind:"rule"` (host-forced
  from the cage door, un-spoofable even if the body claims otherwise), `dedup_key` =
  `energy-intensity-high:<building>`, severity graded by intensity.
- **Re-run the same rule ⇒ each insight's `count` bumps, no duplicate** (dedup on the stable key).
- **`route:false` no-ops the raise:** a panel-repaint run (`rules.run` from a bound panel source) sets
  `route:false`, so `insight.raise`/`ack`/`close` become no-ops (logged
  `insight.<verb> skipped: read-only panel run`) — a dashboard viewed by ten people never inflates the
  count. A scheduled flow (`route:true`) raises normally. This is the load-bearing rule-raises-insight
  invariant — confirm the no-op on a `route:false` run.
- **`caller ∩ grant`:** the raise needs `mcp:insight.raise:call` re-checked mid-run; a caller without
  it is denied at the `insight.raise` line even though the body is valid. `insight.close` maps to the
  **`insight.resolve`** verb/cap (hold `mcp:insight.resolve:call`, not `…close…`).

### 2.6 Live feed — `insight.watch` (SSE)

```bash
curl -N "http://127.0.0.1:8080/insights/events?token=$TOKEN"   # event: message / data: {"kind":"raise",…}
```

Query-param auth (EventSource can't send a bearer header); `401` on a bad token; `403` (opaque)
without `mcp:insight.watch:call` or across workspaces. Fire a raise in another shell and watch the
event arrive — a missed event is not data loss (`insight.list` is the durable truth).

---

## Step 3 — on a wrong result, diagnose the seam in order

Rule out the cheap false-bugs first:

1. **Rule path silent?** Is `demo-buildings` seeded + registered (`curl /datasources`)? An un-seeded
   source ⇒ a `source(...)` deny inside the rule ⇒ zero rows ⇒ zero insights — not an insights bug.
   Re-run Step 0.
2. **`route:false` no-op mistaken for a bug?** A raise from a panel-repaint run is *supposed* to
   no-op. Drive `rules.run` directly (default `route:true`) to see the raise land.
3. **Cap deny mistaken for a crash?** A missing `mcp:insight.raise:call` (or `insight.resolve` for
   close) denies mid-run — hold the target caps, not just `rules.run`.
4. **Stale node?** `make kill && make dev` after a Rust change (memory: flows-dev-node-no-hot-reload).
5. **1970 timestamps?** A `ts:0`/omitted stamps epoch — pass a real monotone `ts` (the SKILL.md
   backfills the host clock, but pass one explicitly in a test).

Only once the seam is real and still wrong: open `../../debugging/insights/<symptom>.md` per
[`../../scope/debugging/debugging-scope.md`](../../scope/debugging/debugging-scope.md), find the root
cause, add a **regression test** (`cargo test -p lb-host …` fails-before/passes-after), update
`debugging/README.md`.

---

## Step 4 — what to leave behind (definition of done)

- The **observed lifecycle** (raise→dedup→ack→resolve→re-open), the occurrence ring, the subscription,
  AND the rule-raised insights over real data — shown in the session doc (green is a claim you show).
- CRUD/lifecycle + permissions (per-verb deny AND host-forced identity AND `caller ∩ grant`) + access
  (the workspace wall) covered, plus the functional rule-producer-door check.
- **Left inspectable:** the raised insights still present (`insight.list`), the `e2e-ops` subscription
  still registered, the `demo-buildings` source still registered, the node still running. Your **final
  response hands the user the exact page** — e.g. "open **Insights** at http://127.0.0.1:8080 — the
  over-budget buildings are listed as `warning`/`critical` insights raised by the rule from the seeded
  data; I left them + the `e2e-ops` subscription in place so you can check." Do **not** resolve/delete
  the primary insights (correction is resolve+raise, and there's no delete in v1 anyway).
- On any failure: a completed `debugging/insights/…` entry + regression test, cross-linked.

---

## Related

- The rule engine that raises these insights: [`../rules/README.md`](../rules/README.md) — run it
  first to prove the rule surface, then this to prove the producer door.
- The seeded datasource the rule reads: [`../datasources/README.md`](../datasources/README.md).
- The channels a subscription posts into (and the inbox `alert()` lands in):
  `../../skills/channels-inbox-outbox/SKILL.md`.
