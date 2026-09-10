# Insights scope — the Case plane (lb half: the record, the verbs, the reactors, the gateway)

Status: **building** (2026-09-10, `feat/case-plane`). Derived from the product scope in
`NubeIO/rubix-ai → docs/scope/insights/case-plane-scope.md`, which keeps the client brief, the
personas, the money framing and the four surfaces. **This file is the upstream half**: the durable
records, the verb surface, the reactors, the caps and the gateway token principal — everything that
must land in `lb` and be released as a `node-v*` tag before rubix-ai can consume it.

> Where the two files disagree about *product intent*, the rubix-ai scope wins. Where they disagree
> about *the lb contract* (a record field, a verb name, a cap), this file wins.

**There are no open questions.** Everything open at filing is decided under *Resolved decisions*.

---

## The one decision everything else follows from

**An insight is a detection. A case is a piece of work. They are different records.**

An insight is keyed by `(workspace, dedup_key)`, re-raised and re-opened by a machine, and
deliberately immune on its human plane (`insight-triage-scope.md`). Work is **many-to-one** with
detections: one gateway drop is 400 detections and one job; one chiller that short-cycles every
summer is one detection key and three jobs over three years. Putting workflow on the detection row
breaks both. So: a **`case`** record that cites one or more insights and owns everything
human-facing. The raise hot path never learns what a purchase order is.

**Rule 10 holds throughout.** `lb-cases`, the reactors and the gateway name no pack, no rule, no
category value and no extension. A case is "a record that cites insights" — a property of the data.
The `category` vocabulary is workspace data (a `tag_vocab` row packs seed); the causal table stays
in the rule.

---

## What lands in lb

| slice | where | wave |
|---|---|---|
| the `tags.*` dispatcher door + the two missing caps | `host/src/tool_call.rs`, `tool_gate.rs`, `authz/builtin_roles.rs` | 1 |
| `Human > Producer` fold in `materialize_facets` | `host/src/insight/facets.rs` (new) | 1 |
| the echo backfill job | `host/src/insight/backfill.rs` (new) | 1 |
| `evidence.subjects[]`, `caveats[]`, `case_id`, `month_hist[12]`, `pattern` on the record | `lb-insights` | 1 |
| the caveat stamp at raise + "a caveated insight is never a breakthrough" | `lb-insights` + `host/src/insight/raise.rs` | 1 |
| `category` validated against the workspace's declared set | `lb-insights` (`vocab.rs`) | 1 |
| the `lb-cases` crate + `host/src/case/` verbs + case-group / hold-down reactors | new crate, new host module | 1 |
| `service_policy` + `policy.sla.*` + the business-hours calendar + the deadline arithmetic | `lb-cases` | 1 |
| the sla-clock reactor | `host/src/case/sla_clock.rs` | 2 |
| `party`, `case_request`, `case.request.*`, the outbox link, the nudges | `lb-cases` + `host/src/case/` | 2 |
| the `GET /r/{token}` gateway token principal | `role/gateway/src/routes/case_request.rs` | 2 |
| `rule.scorecard` | `host/src/case/scorecard.rs` | 2 |

---

## Data model

SurrealDB store rows in the `{ data, rev }` envelope, workspace-scoped, ULID ids, epoch-ms logical
timestamps injected by the host (no wall clock in the crate — testing §3).

### `insight` (`lb-insights`) — the changes it takes

| change | field | meaning |
|---|---|---|
| change | `evidence.subjects: Vec<String>` | point refs the finding was derived from |
| change | tag fold | `human > producer`, newest within a source, in `materialize_facets` |
| change | `category` validation | value must be in the workspace's declared `tag_vocab` set |
| new | `caveats: Vec<String>` | open DQ insight ids whose `subjects` intersect; stamped at raise, echoed on `get` **and** `list` |
| new | `case_id: Option<String>` | back-ref echo so a roster renders the case chip without an N+1 |
| new | `month_hist: [u32; 12]` | non-evicting per-calendar-month firing counters |
| new | `pattern: Pattern` | `new \| chronic \| flapping \| seasonal`, derived on raise |

Every new field is `#[serde(default)]` and skipped when empty, so a record written before the field
landed decodes unchanged and a reader that ignores it is unaffected.

`insight_occ`, `insight_comment`, `insight_sub`, `insight_notify`, `insight_policy`: shape unchanged.
Behaviour change in the notify path only: **a caveated insight is never a breakthrough**.

### `lb-cases` — the new records

**`case`** — `id, title, workflow ∈ {to_action, actioned, waiting_on_po, resolved}, resolution ∈
{fixed, self_cleared, false_positive, accepted_risk, duplicate}?, resolved_ts?, resolved_by?,
waiting_on ∈ {internal, client, contractor}?, assigned_to?, snooze_until?, snooze_reason?,
snoozed_by?, grouping ∈ {verdict, topology, storm, human, single}, primary_insight, category?,
site?, scope?, severity, policy_id?, respond_by?, due_at?, breached_ts?, breach_waiting_on?,
impact_rate?, impact_tier ∈ {claimed, modelled, withheld}?, cost_to_fix?, verified_saving?,
saving_accepted_by?, saving_accepted_ts?, external_ref?, opened_ts, last_activity_ts,
reopened_count, caveated, closed` — `closed` is the derived "not an open case" flag the exclusivity
invariant indexes on.

**`case_member`** — `case_id, insight_id` (unique pair; **every open insight is in exactly one open
case**), `role ∈ {primary, explained, child, storm, duplicate}`, `added_by` (`system:reactor` or a
subject), `human_placed: bool`, `ts`. A separate table, not an array: a storm can be hundreds of
rows and the case must stay small enough to list.

**`case_event`** — `case_id, seq, ts, kind, actor, data ≤ 4 KB`. Append-only, never evicts.
`kind ∈ {opened, merged, split, workflow, assigned, snoozed, request_sent, request_opened, reply,
nudge, breach, fms_ticket, reopened, resolved, verified, saving_accepted, comment}`. `actor` is
`user: | team: | party: | system:` — a contractor reply is attributed to the **party**, not a login.

**`case_request`** — `id, case_id, party_id, ask ∈ {quote, attend, confirm, info}, token_hash,
expires_ts, respond_by, status ∈ {sent, opened, replied, expired, withdrawn}, delivery ∈ {queued,
sent, logged, failed}, reply { kind ∈ {accept, quote, eta, done, need_info, decline}, amount,
currency, eta_ts, text, attachments[] }?, sent_ts, opened_ts?, replied_ts?, nudges_sent, effect_id?`.

**`party`** — `id, kind ∈ {contractor, fm, client, fms}, name, contact {email?, phone?}, sites[],
trades[], default_ask_window_h`. Scorecard fields are derived by query, never stored.

**`service_policy`** — `id, name, match {site?, category?, severity?}, respond_h, resolve_h,
calendar (`Calendar::Always` | `Calendar::Business { tz, hours[7], holidays[] }`), party_window_h,
hold_down_days`. Most specific match wins (site+category+severity > … > empty); the workspace
default has an empty match. Seeded by packs, editable in settings.

### Vocabulary

`category` — a closed set declared per workspace in a `tag_vocab:{key}` store row, enforced at
raise. Packs seed `device_health | system_health | data_quality | action_required | optimisation`.
**lb ships no default value list** — an unseeded workspace validates nothing (open), so rule 10 is
not violated by lb knowing the five.

`scope` — a separate tag: `point | device | system | site | portfolio`.

---

## Verbs

| verb | cap | gate alias | does |
|---|---|---|---|
| `tags.add` / `tags.remove` / `tags.of` / `tags.find` | `tags.add` / `tags.remove` / `tags.find` / `tags.find` | `tags.of → tags.find` | the dispatcher door; `tags.add` with `source: "human"` is the human-correction write |
| `case.get` / `case.list` | `case.get` / `case.list` | — | `list` takes `lane ∈ {mine, waiting, watching}`, filters, sorts `due_at` asc then severity desc, returns member **count** |
| `case.members` / `case.events` | `case.get` | both → `case.get` | the drawer's two lists, paged |
| `case.open` | `case.open` | — | human-opened over ≥1 insight; the reactor's internal call |
| `case.merge` / `case.split` | `case.open` | both → `case.open` | move members; the losing case closes as `duplicate` |
| `case.workflow` | `case.workflow` | — | transition + `waiting_on`; `resolved` requires `resolution` |
| `case.assign` / `case.snooze` / `case.comment` | `case.workflow` | all → `case.workflow` | the triage write path; `insight.assign` / `insight.comment` delegate here |
| `case.request.send` / `case.request.withdraw` | `case.request.send` | withdraw → send | mint the token, enqueue the effect, schedule nudges, set `waiting_on` |
| `case.request.view` / `case.request.reply` | **token-only**, scoped to one request | — | the only two verbs a token principal may call |
| `party.upsert` / `party.list` | `party.upsert` / `party.list` | — | admin |
| `policy.sla.set` / `policy.sla.list` | `policy.sla.set` / `policy.sla.list` | — | admin |
| `rule.scorecard` | `rule.scorecard` | — | viewer; per `origin.ref` × site |

**Cap tiers.** VIEWER gains `case.get`, `case.list`, `rule.scorecard`, `tags.of`. AUTHOR (member)
gains `case.open`, `case.workflow`, `case.request.send`, `tags.remove`. ADMIN gains `party.upsert`,
`party.list`, `policy.sla.set`, `policy.sla.list`.

Every verb is capability-gated, workspace-isolated, and **aliased in `tool_gate.rs` where its cap is
not its name** — a missing alias is `Denied`, not `NotFound`, so only a **positive** test catches it.
Each new verb therefore ships with a positive gate test asserting the alias resolves and the
namesake cap actually exists in a bundle.

---

## Reactors

| reactor | on | does |
|---|---|---|
| **case-group** | insight raise / reopen (inline in `insight_raise`, effect 6) + a reconcile loop | find or open the case: `body.explains[]` (verdict) → else `single`. Every open insight ends in exactly one open case. **Never moves a member a human placed** (`human_placed`). |

**The verdict seam, precisely.** A *citing* record is any insight whose `body.explains` is a
non-empty array; `body.root_cause` names the upstream finding. Both are **generic body keys** — the
reactor reads those two and nothing else out of `body`, so it names no rule, no pack, no equipment
key and no issue value (rule 10). Entries in `explains[]` are **`dedup_key` strings** as the shipped
producer writes them, resolved through `dedup_lookup` with a get-by-id fallback; an entry that
resolves to nothing is skipped with a warning, never an error, because a citing record can arrive
before or after the records it cites. `primary_insight` is the resolved `root_cause` (falling back
to the citing record when absent) — the **upstream fault**, not the record that names it. Arriving
verdict-last **merges** the members' existing `single` cases into the verdict case; arriving
verdict-first opens over whatever resolves and the reconcile loop folds the stragglers in. A member
with `human_placed: true` is never moved in either direction.
| **hold-down** | insight reopen | if the member's case closed as `fixed` within `hold_down_days`: reopen it, `reopened` event *repair did not hold*, `reopened_count += 1`; any other resolution → a new case |
| **sla-clock** | case open · severity escalation | resolve the policy, compute `respond_by`/`due_at` in business hours, schedule the breach reminder; recompute on escalation; **never pause** |

Reactors run under `node:reactor` caps (`reactor_caps()`, extended with the four `case.*` caps),
never the author's, and the loops are spawned under the `BootConfig::reactors` toggle like every
other role — no `if cloud`.

**Why grouping is inline and not only a loop.** The invariant is "every open insight is in exactly
one open case". A scan-only reactor makes that eventually true, which means the queue under-reports
for one tick and a UI that raises-then-lists sees a case-less row. So grouping runs **inline at the
end of `insight_raise`**, exactly where the tag echo and the subscription matcher already run, and a
**reconcile loop** (`spawn_case_reactors`) is the restart-safe backstop that also serves as the
backfill. Both call the same `group_insight` function.

## Gateway — the token principal

`GET /r/{token}` → hash the token (SHA-256, constant-time compare on the stored hash) →
`case_request` → mint `Principal::routed("party:{id}", ws, ["mcp:case.request.view:call",
"mcp:case.request.reply:call"])` **plus a `constraint` naming the one request id**. It is a
**narrowing** of the caps wall, not a bypass: workspace isolation, the deny path and audit all
apply. No account, no password, no session beyond the token. Withdrawn / expired / replied-past-the
window → **410** with a plain page, never a login prompt. IP rate-limited by the same fixed-window
limiter `/public/invite/accept` uses — the route is a token oracle.

---

## Testing plan

Mandatory categories: **capability-deny** and **workspace-isolation** on every new verb and on
`insight.raise` (it is touched). Real store (`mem://`), real tag graph, real outbox with the
recording `Target` the outbox tests already use (the one permitted external — no mocks).

- **Tag fold** — a `producer` and a `human` edge for one key → the echo carries the human value; a
  second raise does not flip it back; reversed insertion order gives the identical echo; two
  `human` edges → the newest `at` wins.
- **Caveat** — a DQ insight open on `point:X`; raise a finding with `subjects: [X]` → `caveats` set,
  no breakthrough delivery; resolve the DQ insight and raise again → caveat cleared.
- **Workflow immunity** — case in `waiting_on_po`; re-raise every member 50 times → workflow,
  assignee, snooze, requests and money unchanged; only member `count` moved.
- **Resolution required** — `case.workflow(resolved)` without `resolution` → `BadInput`.
- **Hold-down** — `fixed` then re-fire inside the window → same case reopened, `reopened_count: 1`;
  `false_positive` then re-fire → a new case.
- **SLA clock** — a business calendar; open Friday 16:00 with `respond_h: 2` → `respond_by` is
  Monday 10:00; across a holiday; escalation recomputes `due_at`; a snooze does not move it.
- **Token principal** — a valid token can `view`/`reply` on its request only; any other request or
  verb → `Denied`; expired/withdrawn → 410; a replayed reply → one event.
- **Reply transitions** — each of the six reply kinds produces the documented `workflow` /
  `waiting_on` and a `party:`-attributed event.
- **Gate aliases** — a positive test per new verb: `gate_tool_for(v)` resolves to a cap that exists
  in a shipped bundle.
- **Green-while-broken guard** — every reactor test first asserts **RED** with the reactor's cap
  removed (`green-while-broken-reactor-tests.md`), then green with it.

---

## Resolved decisions

1. **The tag door keeps the shipped verb names.** The product scope calls it `tags.set`; the crate
   has shipped `tags.add` (an upsert on `(entity, tag, source)`) since the tags scope. Minting
   `tags.set` as a second name for the same write would fork the surface for a cosmetic reason, and
   the alias table exists to collapse names onto caps, not to multiply them. **The door is
   `tags.add` / `tags.remove` / `tags.of` / `tags.find`, dispatched by EXACT name** (not a `tags.`
   prefix — reserving a namespace against a hypothetical extension named `tags` is the mistake
   `ext.list` already avoided). *Rejected: `tags.set` as an alias* — two names for one write is how
   a catalog stops being readable.
2. **`mcp:tags.remove:call` and `mcp:tags.of:call` ship with the door.** They exist in NO role
   bundle today, so a dispatcher entry alone would have made both verbs `Denied` for every caller
   including admins — the exact shipped-but-unusable shape `tool_gate.rs` documents four times.
   `tags.of` rides the viewer's read (it is `tags.find` narrowed to one entity, so it is aliased
   rather than granted separately); `tags.remove` is an author write beside `tags.add`.
3. **Grouping is inline at raise, with a reconcile loop as the backstop** (see Reactors above).
4. **`case_id` is an echo, written by the grouping step, never by a caller.** Same discipline as
   `producer` and the tag echo: host-computed, self-healing on the next firing.
5. **The case owns triage; `insight.assign` / `insight.comment` become delegates.** The backfill
   opens a `single` case for every existing open insight and copies its assignee and comments
   across. *Rejected: read-through* — two writers for one fact, and every consumer has to know
   which one is live.
6. **Storm and topology folding are fast-follows**, and both are **derived by query at reactor
   time** when they land — no counter table, no extra write on the raise hot path.
7. **`delivery` mirrors the effect outcome AND the provider kind**, so a dev node's
   `LoggingEmailProvider` yields `logged`, never `sent`
   (`outbox-delivered-is-not-email-sent.md`). The drawer must never claim an email went out that
   did not.

## Related

- The product half: `NubeIO/rubix-ai → docs/scope/insights/case-plane-scope.md` (personas, money,
  the four surfaces, the client brief).
- `insight-tag-precedence-scope.md` (the fold + the door it names), `insight-triage-scope.md` (the
  immunity invariant this extends), `insight-evidence-scope.md` (`subjects` lands here),
  `insight-notify-scope.md` (breakthrough semantics the caveat and snooze reuse),
  `insight-analysis-scope.md` (`Quantity`).
- Seams reused: the outbox `Target` (`host/src/outbox/target.rs`), reminders, assets, the tag graph.
