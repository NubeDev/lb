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
`rules.delete`, `rules.help`. **A run cannot widen its invoker (`caller ∩ grant`):** inside the run, every data verb
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
| **Catalog** | — | `{"tool":"rules.help","args":{}}` | — |

- **`rules.run`** takes EITHER an inline `body` (ad-hoc Playground run) OR a saved `rule_id`, plus
  `params` (bound into the script) and `ts` (a logical clock — determinism, no wall-clock). It's the
  hot path: **bounded by the governors, so it's a synchronous call** returning
  `{output, findings, log, ms, ai}`. Long/batch/resumable work belongs in a **chain** (a job), not a
  `rules.run` loop.
- **`rules.save`** upserts `rule:{ws}:{id}` — `id` is the stable key (defaults to `name`); `body` is
  the Rhai source; `params` is a typed declared-param list. A save does NOT execute the body.
- **`output.kind`** is one of `scalar | grid | findings | nothing`.
- **`rules.help`** is the introspection verb: it returns the **function catalog** (the single source
  of truth, `lb_rules::CATALOG`) — `{"functions":[{name,family,signature,description}, …]}` — so an
  agent or the UI can discover every available in-cage function and its description without reading
  this doc. Gated `mcp:rules.help:call` like the others.

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
**emit** (`emit`/`alert`/`log`). **The authoritative list — name, family, signature, description for
every function — is `rules.help`** (the catalog in `lb_rules::CATALOG`); this paragraph is a map, the
catalog is the territory.

### `ai.*` runs against the workspace's SELECTED model (rules-ai-wiring)

`ai.*` in a rule reaches the **same model the workspace picked for its agent** — the agent-catalog
selection written to `agent.config` (Settings → Agent). The `rules.run` bridge resolves that pick to
the node's model and drives a **single-turn** completion (no tools, no agent loop): `ai.complete` is
one model turn; `ai.ask` asks the model to propose SQL, which is then **re-fenced** through the same
`collect` a hand-written `query()` takes before it can run. A rule never gets the agent's tool-calling
loop — its data/emit power comes from the *rule verbs*, gated by the cage + caps.

```bash
# a configured workspace: ai.complete reaches the real model, the result lands in `findings`
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"rules.run","args":{"ts":1,"body":
"let summary = ai.complete(\"which coolers ran hot today?\"); emit(#{ summary: summary });"}}'
# → {"output":{"kind":"findings"},"findings":[{"summary":"…the model answer…"}], "ai":{"calls":1,…}}
```

**Honest "AI configured?" — the model is real only when BOTH hold:** (1) the workspace **selected** a
model endpoint in `agent.config` (the catalog pick), and (2) the node has a **real provider** wired
(not the boot placeholder). Either missing → `ai.*` returns the clear `"AI not configured for rules"`
error, while a **data-only rule still runs**. Today only the sanctioned deterministic `MockProvider`
exists; a rule's `ai.ask` does not hit a live LLM until a real provider adapter is configured — do not
promise otherwise in a workbench prompt. Picking a *different* definition in the catalog changes which
model a subsequent rule run uses — the pick is the single source of truth for "which model do my rules
use", exactly as for the agent.

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

## 5. Propose & approve — gate an effect on a human sign-off (rules-approvals)

A rule can **propose a change and stage the effect it will fire only if a human approves** — "a rule
proposes, a human disposes". `inbox.request_approval` raises a `needs:approval` item AND durably stages
the `on_approve` effect in a **held** state; the outbox relay skips a held effect, so it is *never*
delivered until the item is approved:

```rhai
// Raise an approval item AND stage the page it sends IF approved (staged HELD, not delivered yet).
inbox.request_approval(#{
    id: "refund-proposed",
    channel: "ops",
    body: "Refund proposed — cooler breached",
    route: "team:managers",                              // who should sign off (advisory in v1)
    on_approve: #{ target: "notify", action: "page",     // the HELD effect
                   payload: #{ level: "info", msg: "refund approved" } },
});
```

Then a reviewer (via the Inbox UI, or another rule) resolves the item — and the **approval reactor**
closes the loop:

```rhai
inbox.resolve("refund-proposed", "approved");   // → reactor releases the held effect (held→pending)
// inbox.resolve("refund-proposed", "rejected"); // → reactor DISCARDS it (never delivered)
// inbox.resolve("refund-proposed", "deferred"); // → left held (inert in v1, re-resolve later)
```

The relay then delivers the released effect **exactly once**. Reuses the exact `Item` + `Resolution` +
outbox trio the coding workflow ships — no new approval primitive.

- **Two writes, effect-first.** `request_approval` stages the held effect FIRST, then records the item,
  so a `needs:approval` item never exists without its gated effect (a mid-verb deny leaves at most a
  harmless held effect). Both charge the per-run write budget; both are `caller ∩ grant` gated
  (`mcp:outbox.enqueue:call` for the effect, `mcp:inbox.record:call` for the item).
- **The request is caller-gated; the release is a system transition.** The requester needs the outbox
  stage cap; the *release* runs under the reactor's host authority, gated only by the resolution
  existing — never by re-checking a user cap. So there is **no user verb to force a release**, and a
  released effect can never exceed what was staged. The reviewer sees the exact held effect before
  approving (informed consent).

## Gotchas

- **A run can't widen its invoker** — a data verb hitting a source the *caller* lacks is denied
  mid-run under `caller ∩ grant`, even with a valid body. Hold the target caps too, not just `rules.run`.
- **`rules.run` is synchronous and bounded** — governors cap a single run. Anything long/retrying/
  restart-surviving is a **chain** = an `lb-jobs` job (`rule-chains-scope.md`), not a blocking loop in
  a handler.
- **`save` never executes** — saving persists the body; only `run` evaluates it.
- **`id` defaults to `name`** and upserts in place (no version history).
- **The script sees no secrets** — no provider key (AI-gateway holds it; the rule adapter passes the
  resolved `ModelAccess`, never a key), no DB DSN (`federation` holds it). Same posture for both seams.
- **`ai.*` errors when unconfigured, never fabricates** — a workspace with no selected model (or a node
  with no provider) gets the honest `"AI not configured for rules"` error, not a made-up answer; the
  rest of the rule (data reads, `emit`) runs regardless.
- **Workspace wall** — ws-B can neither `get`/`run` a ws-A saved rule nor read a ws-A source;
  `ai.ask` schema introspection sees only the workspace's own sources.
- **Denials are opaque** — a blocked `source`, a tripped budget, and a rejected `eval` all surface as
  opaque errors before anything runs.
- **A held effect is never delivered until approved** — `request_approval`'s `on_approve` is staged
  `held`, so the relay skips it; nothing fires on the mere *request*. It fires only after
  `inbox.resolve(id, "approved")` and the reactor's release. A rule resolving its *own* request is
  possible (if the caller holds `inbox.resolve`) but is a foot-gun, not the intent — human sign-off is.

## Related

- Scope: `docs/scope/rules/rules-engine-scope.md`; the DAG that chains rules on `lb-jobs`:
  `docs/scope/rules/rule-chains-scope.md`; the `ai.*`→real-model binding:
  `docs/scope/rules/rules-ai-wiring-scope.md`.
- The catalog pick a rule's `ai.*` honors: `docs/scope/agent/agent-catalog-scope.md` (`agent.config`).
- The external-source engine `source("…")` reaches: `docs/skills/datasources/SKILL.md`
  (`federation.query`) — and the SurrealDB-never-a-DataFusion-source hard line.
- Platform series a rule reads: `docs/skills/ingest-series/SKILL.md` (`series.*`).
- `alert` targets: `docs/skills/channels-inbox-outbox/SKILL.md` (inbox/outbox).
- Propose-and-approve loop: `docs/scope/rules/rules-approvals-scope.md` (the `needs:approval` item +
  held effect + release reactor); the messaging verbs it builds on: `rules-messaging-scope.md`.
- README §3 (rules 1/2/4/5/6/7), §6.5 (MCP), §6.7 (AI-gateway), §6.9 (jobs), §6.10 (inbox/outbox).
  Source: `rust/crates/host/src/rules/`; ported from `rust/rubix-cube/rbx-server/src/rules/`
  (MIT/Apache-2.0). Routes: `rust/role/gateway/src/server.rs`.
