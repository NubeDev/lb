---
name: e2e-rules
description: >
  Use when asked to end-to-end test RULES — the sandboxed Rhai engine (`rules.*` + `lb-rules`).
  Drive a REAL running node over the REAL MCP/REST surface: run an ad-hoc rule, save + run by id,
  read data from the seeded `demo-buildings` SQLite source, prove the cage/cap wall (a run can't
  widen its invoker), and prove the workspace wall. No mocks — real node, real datasource, real
  caps. For the rule→insight producer door, see `../insights/README.md`. Assumes suites are green.
---

# E2e rules runbook — prove the rule engine works as designed against a live node

Status: scope (the standard). Design intent:
[`../../scope/rules/rules-engine-scope.md`](../../scope/rules/rules-engine-scope.md).
Drivable surface: [`../../skills/rules/SKILL.md`](../../skills/rules/SKILL.md).
Checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** The `cargo test -p lb-host` rules suites
are the **scope/session's** job and assumed green — this runbook does **not** re-run them. Its job
is to **drive a live node over the real `rules.*` surface and observe it behave**: a rule that runs,
reads real data from a real datasource, is bounded by the cage, and is gated by real caps.

**No mocks (testing-scope §0).** The engine is an in-process `lb-rules` library — you drive it for
real through the running node's MCP/REST bridge; the data it reads comes from the **real** seeded
`demo-buildings` SQLite datasource (the Docker-free path), never a fake dispatcher.

---

## Step 0 — stand up the node AND seed the datasource (do this first)

A rule that reads `demo-buildings` is **denied at the `source(...)` seam until that datasource
exists**. So seed + register it first — the Docker-free SQLite building dataset, one command
against a running node (see [`../datasources/README.md`](../datasources/README.md) Step 0):

```bash
make build-wasm && make dev          # boot the node (root on 8080, federation sidecar on by default)
make seed-demo-sqlite                # generates .lazybones/data/demo/buildings.db + datasource.add {kind:sqlite}
```

Authenticate (workspace + principal come from the token — host-set, never script-set):

```bash
BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
auth=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')
```

**Verify the source is present** before running a rule that reads it (else every data read is a
`source(...)` deny, not a bug):

```bash
curl -s $BASE/datasources -H "authorization: Bearer $TOKEN"   # 'demo-buildings' present, probe green
```

---

## Step 1 — read the design (what is "correct"?)

- **[`../../scope/rules/rules-engine-scope.md`](../../scope/rules/rules-engine-scope.md)** — the ask:
  the cage (no I/O, `eval`/`import` off), the governors (`max_operations`/deadline/`max_writes`), the
  `caller ∩ grant` authority rule, `output.kind ∈ {scalar|grid|findings|nothing}`.
- **[`../../skills/rules/SKILL.md`](../../skills/rules/SKILL.md)** — the drivable verbs
  (`rules.run|save|get|list|delete|help`), the wire shapes, the `source(...)`/`query(...)` surface,
  the chart-return helpers (`timeseries`/`wide`/`category`).
- The catalog is the territory: `rules.help` returns every in-cage function (name/family/signature/
  description) from `lb_rules::CATALOG` — drive it, don't trust this doc's map over the live catalog.

---

## Step 2 — the checklist (drive it, observe it works as designed)

The four dimensions from [`../README.md`](../README.md#what-to-check--the-functional-dimensions),
against the live node.

### 2.1 CRUD — a saved rule round-trips

```bash
# help (the catalog) — the introspection verb every UI/agent reads
curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"rules.help","args":{}}'   # → {"functions":[…]}

# save — a real intensity rule over the seeded source (SQL in a `backtick` raw string spans lines)
BODY='let rows = query("demo-buildings", `
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
rows'
curl -s -X POST $BASE/mcp/call "${auth[@]}" \
  -d "$(jq -n --arg b "$BODY" '{tool:"rules.save",args:{id:"e2e-intensity",name:"E2E intensity",body:$b}}')"

curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"rules.get","args":{"id":"e2e-intensity"}}'   # read back — body preserved
curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"rules.list","args":{}}'                      # list — e2e-intensity present

# delete — proven on a THROWAWAY rule made just for this check
curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"rules.save","args":{"id":"e2e-throwaway","name":"tmp","body":"1"}}'
curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"rules.delete","args":{"id":"e2e-throwaway"}}'   # → {ok:true}
curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"rules.get","args":{"id":"e2e-throwaway"}}'      # → not found
```

> **Leave `e2e-intensity` in place** — it's the primary artifact for the user to inspect (and it's
> chart-ready if you swap the last line to `category(rows, "building", "kwh_per_m2")`). Only the
> throwaway is deleted. `save` never executes the body; only `run` evaluates it.

### 2.2 Functional — a rule actually runs and reads real data

The point of a rule is that it reads real data through the gated verbs and returns a real result:

```bash
# run BY ID — the saved intensity rule against the seeded source
curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"rules.run","args":{"rule_id":"e2e-intensity","ts":1719800000000}}'
# → {"output":{"kind":"scalar"},"findings":[],"log":[…],"ms":…,"ai":{…}}  with rows in the output

# run AD-HOC — a threshold+alert body (alert raises a real inbox item + must-deliver outbox notify).
# Build the JSON with `jq -n --arg` so the Rhai body's own quotes never fight the shell.
# NOTE the threshold: the lite (1-month) seed totals ~2k–37k kWh/building — pick a value that FIRES
# (a threshold above the max is a valid rule that finds nothing, and reads as "broken").
ADHOC='let rows = query("demo-buildings", `
  SELECT s.name AS building, ROUND(SUM(pr.value),0) AS kwh
  FROM point_reading pr JOIN point p ON p.id=pr.point_id
  JOIN meter m ON m.id=p.meter_id JOIN site s ON s.id=m.site_id
  WHERE p.name = '"'"'Energy kWh'"'"' GROUP BY s.name ORDER BY kwh DESC
`).records();
for r in rows { if r.kwh > 20000 { alert(#{ level: "warning", building: r.building, kwh: r.kwh, msg: r.building + " energy-heavy" }); } }
rows'
curl -s -X POST $BASE/mcp/call "${auth[@]}" \
  -d "$(jq -n --arg b "$ADHOC" '{tool:"rules.run",args:{body:$b,ts:1719800000000}}')"
# → {"output":{"kind":"findings"},"findings":[{…}],…}  — 2 findings (Riverside ~37k, Airport ~27k)
```

**Observe:** the run returns real rows from the seeded SQLite file (buildings ranked by kWh);
`alert()` findings appear in `findings` AND (on a `route:true` run) land in the inbox — confirm via
`GET /inbox`. A `ts` is the run's **logical clock** (determinism, no wall-clock) — pass a real
monotone value.

#### A rule raises a durable INSIGHT (the producer door)

`alert()` is ephemeral (inbox + outbox). To raise a **durable, deduped, lifecycle-bearing** fault
from a rule body, use the `insight` cage handle — the same energy-intensity rank, but each
over-budget building becomes an `insight:{ws}:{id}` record that dedups on a stable per-building key:

```bash
# The `area` site tag is stored as e.g. "18000 m2" → REPLACE(...,' m2','') strips the unit before CAST.
INS='let rows = query("demo-buildings", `
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
  if r.kwh_per_m2 > 1.0 {
    insight.raise(#{
      dedup_key: "energy-intensity-high:" + r.building,   // stable per-building identity = the dedup key
      severity: if r.kwh_per_m2 > 2.0 { "critical" } else { "warning" },
      title: r.building + " energy intensity high",
      body: #{ building: r.building, kwh_per_m2: r.kwh_per_m2, budget: 1.0 },
      tags: #{ area: "energy", building: r.building },     // low-cardinality facets (NOT identity)
    });
  }
}
rows'
curl -s -X POST $BASE/mcp/call "${auth[@]}" \
  -d "$(jq -n --arg b "$INS" '{tool:"rules.run",args:{body:$b,ts:1719800000000}}')"

# the durable insight now exists — read it back through the REAL insight surface
curl -s -X POST $BASE/mcp/call "${auth[@]}" -d '{"tool":"insight.list","args":{"tags":{"area":"energy"},"limit":50}}'
```

**Observe:** on the lite seed, intensity ranks Riverside Data Center at **~4.68 kWh/m²** (over the 2.0
critical threshold) and every other building under 1.0 — so this raises **one `critical` insight**
(Riverside). Confirm via `insight.list`:
- `origin.kind:"rule"` — **host-forced from the cage door**, un-spoofable even if the body claimed
  otherwise;
- `dedup_key:"energy-intensity-high:Riverside Data Center"`;
- **re-run the same rule ⇒ the insight's `count` bumps, no duplicate** (dedup on the stable key).

Caps: the raise needs `mcp:insight.raise:call` re-checked mid-run (`caller ∩ grant`); a
panel-repaint run (`route:false`) no-ops the raise. The full insight lifecycle
(dedup→ack→resolve→re-open) + the producer-door invariants are proven in
[`../insights/README.md`](../insights/README.md) §2.5 — this is the rules-side half of the same bridge.

### 2.3 Permissions — the cage + cap wall holds (the negative path is the point)

Every verb needs its grant; and **a run cannot widen its invoker** (`caller ∩ grant`):

```bash
# no token → 401
curl -s -X POST $BASE/mcp/call -H 'content-type: application/json' -d '{"tool":"rules.list","args":{}}' \
  -o /dev/null -w "%{http_code}\n"     # → 401
```

- **Per-verb cap deny:** a member without `mcp:rules.save:call` is refused `rules.save` (opaque deny),
  even with a valid body. Sign in a member lacking the grant (via `signInWithCaps` in the suite; or a
  dev-login member whose `member_caps()` omits it) and assert the deny.
- **A run can't reach a source the caller lacks:** a rule body that `query(...)`s `demo-buildings`
  run by a caller without `mcp:federation.query:call` is **denied mid-run** at the data verb — the
  body is valid, the authority isn't. This is the cage's whole point (`caps::check` under the
  *invoker's* authority). Assert the mid-run deny, not a crash.
- **The cage has no I/O:** there is no file/net/process surface, `eval`/`import` are off — a body that
  tries them errors before anything runs (governor/cage, not a cap). Governors (`max_operations`,
  deadline, `max_writes`) bound work + time: an `alert()`/`insight.raise` storm loop trips `max_writes`.

### 2.4 Access — the workspace wall holds

A saved rule and the sources it reads are workspace-scoped:

```bash
# add a second-workspace member, then sign in to it (globex)
TOK_B=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"user:bob","workspace":"globex"}' | jq -r .token)
authB=(-H "authorization: Bearer $TOK_B" -H 'content-type: application/json')
curl -s -X POST $BASE/mcp/call "${authB[@]}" -d '{"tool":"rules.get","args":{"id":"e2e-intensity"}}'   # → not found (ws-A rule invisible)
```

`globex` can neither `get`/`run` the `acme` rule `e2e-intensity` nor read `acme`'s `demo-buildings`
source — workspace is checked *before* caps. (`demo-buildings` was registered in `acme` only; the
same file path registered in `globex` would be an independent grant.)

---

## Step 3 — on a wrong result, diagnose the seam in order

Before filing a debug entry, rule out the cheap false-bugs (most "rule broken" reports are one of
these):

1. **Is the datasource seeded + registered?** `curl $BASE/datasources` with `demo-buildings` absent
   ⇒ re-run Step 0 (`make seed-demo-sqlite`, node up first). A `source(...)` deny reads as "rule
   broken" but is a missing source.
2. **Is the caller's authority sufficient?** A mid-run deny on a data verb = the caller lacks the
   source cap, not a rule bug (`caller ∩ grant`). Hold the target caps too, not just `rules.run`.
3. **Stale node?** The node doesn't hot-reload Rust — `make kill && make dev` after a crate change
   (memory: flows-dev-node-no-hot-reload).
4. **1970 timestamps?** A `ts:0` (or omitted) stamps epoch — pass a real monotone `ts`; the viz
   bridge renders epoch-ms/seconds per the frame builder (memory: flow-ts-prefs-display).

Only once the seam is real and still wrong: open `../../debugging/rules/<symptom>.md` per
[`../../scope/debugging/debugging-scope.md`](../../scope/debugging/debugging-scope.md), find the root
cause, add a **regression test** (`cargo test -p lb-host …` fails-before/passes-after), update
`debugging/README.md`.

---

## Step 4 — what to leave behind (definition of done)

- Proof the source was **seeded** (`/datasources` shows `demo-buildings`) and the rule **ran** (the
  observed rows/findings) in the session doc — green is a claim that must be shown.
- CRUD + permissions (the cap deny AND the mid-run `caller ∩ grant` deny) + access (the workspace
  wall) covered, plus the functional run-reads-real-data check.
- **Left inspectable:** the `e2e-intensity` rule still saved, the `demo-buildings` source still
  registered, the node still running. Your **final response hands the user the exact surface** — e.g.
  "open the Rules Playground at http://127.0.0.1:8080, load **e2e-intensity**, and Run — it ranks the
  8 buildings by kWh/m² from the seeded data; I left it saved so you can check." Do **not** delete it.
- On any failure: a completed `debugging/rules/…` entry + regression test, cross-linked.

---

## Related

- The rule→insight producer door (a rule raises a durable, deduped insight from its body):
  [`../insights/README.md`](../insights/README.md) — run that one right after this to prove the bridge.
- The seeded datasource these rules read: [`../datasources/README.md`](../datasources/README.md).
- Charts a rule feeds as a panel source: [`../charts/README.md`](../charts/README.md).
