---
name: rules
description: >-
  Author and run Lazybones rules over the node gateway — a sandboxed Rhai script that reads data
  (platform series/store or an external datasource), transforms it (a lazy column `Grid`: filter/
  rollup/join/reduce), optionally calls AI, and emits findings/alerts. Save/get/list/delete/run rules
  via `rules.*`. Use when a task says "write/run a rule", "run a Rhai rule in the Playground", "author
  a threshold/alert rule over series", "save a rule", or "call rules verbs over REST/MCP". The engine
  is an in-process `lb-rules` library (ported from rubix-cube), capability-first: the Rhai cage reaches
  nothing but the host's gated verbs; a run cannot widen beyond what its invoker holds.
---

# Authoring & running rules (`rules.*`, the `lb-rules` engine)

A rule is a small **sandboxed Rhai script** that reads data through the platform's gated verbs,
composes a lazy column-oriented `Grid` (filter/rollup/align/join/reduce), optionally calls `ai.*`
through the AI-gateway (with a per-run budget + an nsql re-validation fence), and **emits findings /
raises alerts** (into the inbox, must-deliver notifications through the outbox). The engine is an
**in-process library** (`rust/crates/host/src/rules/`, ported from `rubix-cube`) — not an external
service — and that is exactly why it fits: the Rhai cage can reach neither the store, the bus, nor a
socket; only the verbs, each running the host's `caps::check`.

Two call styles, as with the other host surfaces:

1. **Dedicated REST routes** — the Playground's surface (`/rules…`).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` by dotted name.

Both derive **workspace + principal from the token** (host-set, never script-set). Every verb is
capability-gated; denials are opaque.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities — one per verb: `mcp:rules.run:call`, `rules.save`, `rules.get`, `rules.list`,
`rules.delete`. **A run cannot widen its invoker (`caller ∩ grant`):** inside the run, every data verb
(`data.query`/`series.*`/`federation.query`) hits the host `caps::check` under the *invoker's*
authority — a rule reading a source the caller lacks is denied mid-run, even though the body is valid.
`ai.*` needs the AI-gateway cap.

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| Run (ad-hoc or saved) | `POST /rules/run` | `{"tool":"rules.run","args":{…}}` | `body` \| `rule_id`, `params?`, `ts?` |
| Save (create/update) | `POST /rules` | `{"tool":"rules.save","args":{…}}` | `id`\|`name`, `name?`, `body`, `params?` |
| Get one | `GET /rules/{id}` | `{"tool":"rules.get","args":{"id":"…"}}` | `id` |
| List | `GET /rules` | `{"tool":"rules.list","args":{}}` | — |
| Delete | `DELETE /rules/{id}` | `{"tool":"rules.delete","args":{"id":"…"}}` | `id` |

- **`rules.run`** takes EITHER an inline `body` (ad-hoc Playground run) OR a saved `rule_id`, plus
  `params` (bound into the script) and `ts` (a logical clock — determinism, no wall-clock). It's the
  hot path: **bounded by the governors, so it's a synchronous call** returning
  `{output, findings, log, ms, ai}`. Long/batch/resumable work belongs in a **chain** (a job), not a
  `rules.run` loop.
- **`rules.save`** upserts `rule:{ws}:{id}` — `id` is the stable key (defaults to `name`); `body` is
  the Rhai source; `params` is a typed declared-param list. A save does NOT execute the body.
- **`output.kind`** is one of `scalar | grid | findings | nothing`.

## 3. Writing a rule

To the author, `source("cooler.temp")` (platform) and `source("timescale")` (external) read alike —
one `source(...)` surface, two correct paths beneath (SurrealDB-native `series.*`/`data.query` vs the
`federation` extension's `federation.query`). The `Grid` stays lazy (composed query + context;
materializes only on `collect`/a `Col` reduction/`return`), so chaining never copies data.

```bash
# ad-hoc run of a threshold+alert rule (the Playground path)
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"rules.run","args":{"ts":1719800000000,"body":
"let hot = source(\"cooler.temp\").last(\"24h\").rollup(\"1h\", \"max\").filter(\"max > 5.0\");
 if hot.size() > 0 { alert(#{ level: \"critical\", series: \"cooler.temp\", msg: \"over 5C\" }); }"}}'
# → {"output":{"kind":"findings"},"findings":[{…}],"log":[…],"ms":…,"ai":{…}}

# save it, then run by id
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"rules.save","args":{"id":"cooler-foodsafety","name":"Cooler food-safety","body":"…"}}'
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"rules.run","args":{"rule_id":"cooler-foodsafety"}}'
```

Verb families available in the cage (ported from rubix-cube): **data** (`source`/`query`/`dataset`/
`history`/`span`/`last`/`param`), **timeseries** (`rollup`/`align`/`interpolate`/`gapfill`/`resample`/
`lag`/`delta`/`rate`), **Grid** reductions (`filter`/`select`/`add_col`/`rename`/`group_by`/`join`/
`col`/`size`/`head`), **ai** (`ai.ask`/`complete`/`classify`/`embed` — metered + fenced), and
**emit** (`emit`/`alert`/`log`).

## 4. The cage (why a rule is safe)

Security is "absence of capability + presence of limits", in-process:

- **No I/O surface** — the engine registers no file/net/process API; `eval`/`import` are disabled,
  `set_max_modules(0)`. A rule can do nothing but call the verbs handed to it.
- **Resource governors** — `max_operations`, call-level depth, string/array/map size caps, and a
  wall-clock deadline bound *work and time* (DoS defense), while `caps::check` bounds *authority*.
- **`ai.*` invariants** — a per-run **budget meter** (call + token caps; a `for`-loop of `ai.complete`
  can't overspend) and the **nsql fence** (a model-proposed query is re-validated through the same gate
  a hand-written one is, before it runs — no nsql path skips the validator).
- **`alert` is real** — it raises an **inbox** item and routes a must-deliver notification through the
  **outbox**, never raw pub/sub or a sandbox-internal marker. `emit` collects a finding; `log` records
  a line.

## Gotchas

- **A run can't widen its invoker** — a data verb hitting a source the *caller* lacks is denied
  mid-run under `caller ∩ grant`, even with a valid body. Hold the target caps too, not just `rules.run`.
- **`rules.run` is synchronous and bounded** — governors cap a single run. Anything long/retrying/
  restart-surviving is a **chain** = an `lb-jobs` job (`rule-chains-scope.md`), not a blocking loop in
  a handler.
- **`save` never executes** — saving persists the body; only `run` evaluates it.
- **`id` defaults to `name`** and upserts in place (no version history).
- **The script sees no secrets** — no provider key (AI-gateway holds it), no DB DSN (`federation`
  holds it). Same posture for both seams.
- **Workspace wall** — ws-B can neither `get`/`run` a ws-A saved rule nor read a ws-A source;
  `ai.ask` schema introspection sees only the workspace's own sources.
- **Denials are opaque** — a blocked `source`, a tripped budget, and a rejected `eval` all surface as
  opaque errors before anything runs.

## Related

- Scope: `docs/scope/rules/rules-engine-scope.md`; the DAG that chains rules on `lb-jobs`:
  `docs/scope/rules/rule-chains-scope.md`.
- The external-source engine `source("…")` reaches: `docs/skills/datasources/SKILL.md`
  (`federation.query`) — and the SurrealDB-never-a-DataFusion-source hard line.
- Platform series a rule reads: `docs/skills/ingest-series/SKILL.md` (`series.*`).
- `alert` targets: `docs/skills/channels-inbox-outbox/SKILL.md` (inbox/outbox).
- README §3 (rules 1/2/4/5/6/7), §6.5 (MCP), §6.7 (AI-gateway), §6.9 (jobs), §6.10 (inbox/outbox).
  Source: `rust/crates/host/src/rules/`; ported from `rust/rubix-cube/rbx-server/src/rules/`
  (MIT/Apache-2.0). Routes: `rust/role/gateway/src/server.rs`.
