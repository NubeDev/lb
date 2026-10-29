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
resolution_reason?, waiting_on ∈ {internal, client, contractor}?, assigned_to?, snooze_until?,
snooze_reason?, snoozed_by?, sla_paused_at?, sla_paused_ms, pinned_by[], grouping ∈ {verdict, topology, storm, human, single}, primary_insight, category?,
site?, scope?, severity, policy_id?, respond_by?, due_at?, breached_ts?, breach_waiting_on?,
impact_rate?, impact_tier ∈ {claimed, modelled, withheld}?, cost_to_fix?, verified_saving?,
saving_accepted_by?, saving_accepted_ts?, external_ref?, opened_ts, last_activity_ts,
reopened_count, caveated, closed` — `closed` is the derived "not an open case" flag the exclusivity
invariant indexes on.

**`case_member`** — `case_id, insight_id` (unique pair; **every open insight is in exactly one open
case**), `role ∈ {primary, explained, child, storm, duplicate}`, `added_by` (`system:reactor` or a
subject), `human_placed: bool`, `ts`. A separate table, not an array: a storm can be hundreds of
rows and the case must stay small enough to list.

**`case_event`** — `case_id, eseq, ts, kind, actor, data ≤ 4 KB`. Append-only, never evicts.
(The field is **`eseq`** on the wire — a consumer that reads `seq` gets nothing. Corrected here
against the built record after the UI mirrored the prose and had to be fixed.)
`kind ∈ {opened, merged, split, workflow, assigned, snoozed, request_sent, request_opened, reply,
nudge, breach, fms_ticket, reopened, resolved, verified, saving_accepted, comment, sla_paused,
sla_resumed}`. `actor` is
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
| `case.get` / `case.list` | `case.get` / `case.list` | — | `list` takes `lane ∈ {mine, waiting, watching}`, filters (incl. `ids`, ≤ 200, for a caller that already knows its cases, and `sla` ∈ {breached, due_soon, on_track, no_deadline, met}, read at `now` with a `due_soon_ms` window, 4 h by default), sorts `due_at` asc then severity desc, returns member **count**. The engine reads open cases only unless `include_closed`, and the shipped rows' counts in one grouped read |
| `case.assignees` | `case.list` | → `case.list` | the assign picker's roster: the caller, their teams, and the members of those teams. A **suggestion list, not an allow-list** — `validate_assignee` accepts a wider set on purpose |
| `case.members` / `case.events` | `case.get` | both → `case.get` | the drawer's two lists, paged. `members` **echoes** each cited insight's `title` + `severity` — see below |
| `case.open` | `case.open` | — | human-opened over ≥1 insight; the reactor's internal call |
| `case.merge` / `case.split` | `case.open` | both → `case.open` | move members; the losing case closes as `duplicate` |
| `case.workflow` | `case.workflow` | — | transition + `waiting_on`; `resolved` requires `resolution`, and may carry the closer's own `reason` (≤ 200 chars, cleared on a reopen) |
| `case.assign` / `case.snooze` / `case.comment` | `case.workflow` | all → `case.workflow` | the triage write path; `insight.assign` / `insight.comment` delegate here |
| `case.pin` | `case.list` | → `case.list` | `{id, pinned}`: the CALLER's own bookmark (subject from the session). No event, no `last_activity_ts` bump; `case.list`'s `pinned` axis reads the caller's pins only |
| `case.request.send` / `case.request.withdraw` | `case.request.send` | withdraw → send | mint the token, enqueue the effect, schedule nudges, set `waiting_on` |
| `case.request.view` / `case.request.reply` | **token-only**, scoped to one request | — | the only two verbs a token principal may call |
| `party.upsert` / `party.list` | `party.upsert` / `party.list` | — | admin |
| `policy.sla.set` / `policy.sla.list` | `policy.sla.set` / `policy.sla.list` | — | admin |
| `insight.vocab.list` / `insight.vocab.set` | same names, one per verb | — | read and author a tag key's closed value set + its gating values; **admin**, both halves |
| `party.delete` / `policy.sla.delete` | same names | — | erase a row; **admin**. `party.delete` is REFUSED when any request points at the party — see below |
| `rule.scorecard` | `rule.scorecard` | — | viewer; per `origin.ref` × site |

**The vocabulary needs a WRITE DOOR, for the same reason `tags.*` did.** `tag_vocab` is an
engine-owned record — `raise` refuses a category outside `values`, the caveat stamp reads `gates`,
and the case plane keys its SLA matrix on the same set — and until `insight.vocab.set` there was no
verb to author one. A workspace could only write it through the generic `store.write`, which is a UI
writing a record the engine then has to trust. That is not hypothetical: the generic entity grid
types every column as text and writes `["a"]` as the STRING `"[\"a\"]"`, which decodes to nothing
and silently disarms the validation. `validate_vocab` is the door that cannot be got round: a key
that is one record-id segment, non-blank values, no duplicates, and **`gates ⊆ values`** — a gating
value nothing can be raised as arms a caveat that never fires, which is indistinguishable from a
workspace with nothing to caveat.

Declaring an EMPTY `values` list re-opens the key, so there is deliberately no delete verb: lb's own
rule is that an undeclared vocabulary validates nothing, and "no row" and "a row declaring nothing"
are the same statement. Both halves are **admin**, the read deliberately — a vocabulary reorders
everything built on it, so it is a settings read, not a queue read; `insight.list` and the case row
already echo each record's own category for everyone else.

**Retiring and deleting are different answers.** Both records carry `active: bool` (defaulting
TRUE — a bare `bool` default is `false`, which would read every pre-existing row as retired and empty
every workspace's roster on upgrade), and both have a delete verb, because the two gestures mean
different things:

- **`active: false` retires a row that has been USED.** A retired party leaves `party.list`'s
  default read and `case.request.send` refuses it, while `party_get` still resolves it — so every
  request already sent still renders their name for the history and the nudge ladder. A retired
  policy is skipped by `policy_match`, so it governs no new case, while the cases it already clocked
  keep the `respond_by` / `due_at` stamped on them. Both surfaces pass `include_disabled: true`,
  because an admin cannot re-enable a row they cannot see.
- **`party.delete` erases a row with nothing to preserve** — a typo, a duplicate — and is **REFUSED
  when any `case_request` points at it**, naming the count and pointing at retirement. The
  alternative is a request whose `party_id` resolves to nothing, which turns "who did we send this
  to?" — the question the request plane exists to answer — into a dangling id.
- **`policy.sla.delete` is unconditional**, and the asymmetry is deliberate: a case carries the
  deadlines it was stamped with rather than re-deriving them, so deleting a policy orphans nothing.
  `active: false` exists beside it for the row worth keeping as evidence — a lapsed contract is the
  argument for what was promised when a breach happened.

Both deletes take their OWN cap rather than aliasing the writer's. They are the destructive half,
and "may author the roster, may not erase from it" is a distinction a workspace could reasonably
want — unlike `case.merge`/`case.split`, where the alias exists because the distinction would be
meaningless.

**`case.members` echoes the cited insight's `title` and `severity`.** A membership row holds an id,
so a drawer rendering the page unaided draws a column of ULIDs while an *unauthenticated* contractor
sees a headline. The echo is resolved at READ time by the host — never stored, because a copy of a
title goes stale the moment the insight is retitled and nothing would ever repair it — in ONE query,
through the `insight.case_id` back-reference the grouping pass already writes for exactly this
purpose. The two rejected alternatives: a `get` per member is 200 reads a page; pushing it to the
client is the same N+1 over the wire AND demands `insight.get` caps of a reader who may hold
`case.get` and not (the two planes are two gates — see the rubix-ai scope's Resolved decision #6).
Both fields are optional and omitted when absent, so the change is additive on the wire and a member
whose insight is gone keeps its row with no title rather than failing the page.

`members_all` is the un-paged, un-echoed read beside it, for callers that want the MEMBERSHIP and
not the drawer's page (merge, split, the host's echo walks). The distinction is not tidiness: the
paged read costs a second query to resolve titles those paths never look at.

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
| **sla-clock** | case open · severity escalation | resolve the policy, compute `respond_by`/`due_at` in business hours, schedule the breach reminder; recompute on escalation; **runs only while the case is Open** — `due_at` is the respond target (decision 9, amended 2026-10-29) |
| **snooze-expiry** | the case tick (`release_expired_snoozes`, before self-clear) | end every open case's snooze whose `snooze_until` has passed, through the ordinary un-snooze gesture (actor `system:snooze-expiry`); idempotent — an ended snooze is cleared |

**The end of a snooze puts parked work back in the queue.** However a snooze ends — un-snooze
(`until` at or before now), expiry (the snooze-expiry pass) or a severity puncture — a case in
`actioned` moves back to `to_action` (an ordinary `workflow` event). A snooze is "come back to this
later"; left in `actioned` the case would read as work in hand that nobody holds, and self-clear,
which judges only untouched `to_action` cases, would never see it. `waiting_on_po` is blocked on
somebody else and stays; clearing a snooze that was never set moves nothing. The rule lives once, in
`lb_cases::snooze_release`, and every door that ends a snooze calls it.

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

**The path split — corrected during the build.** `GET /r/{token}` is a **page** URL: a human opens
it in a browser and the existing SPA fallback must serve HTML there, so it is NOT a data route. The
scope originally conflated the page and the endpoint; one path cannot return HTML to a browser and
JSON to that page's own fetch. The data surface is three **public** gateway routes, registered
beside `/public/invite/accept`:

| route | does |
|---|---|
| `GET /public/case/request?token=…` | the view payload |
| `POST /public/case/request/reply` | the reply (token in the body) |
| `POST /public/case/request/attachment?token=…` | multipart upload → `{ id, name, size }` |

The attachment route is a **gateway route, not a third MCP verb** — the token principal's surface
stays exactly two caps; the upload mints the same principal, checks the request id, and writes the
asset under the host's own authority.

The **view payload carries the plotted rows itself**
(`evidence { series: [ { name, points } ], threshold, unit }`). That is load-bearing rather than
convenient: a principal holding two caps is `Denied` on every query-plane verb, so the page
physically cannot fetch a chart. Shipping the rows in the response keeps the page's entire surface
equal to one verb's response — which is exactly what the risk note asks for.

Each route hashes the presented token (SHA-256, constant-time compare on the stored hash) →
`case_request` → mints `Principal::routed("party:{id}", ws, ["mcp:case.request.view:call",
"mcp:case.request.reply:call"])` **plus a `constraint` naming the one request id**. It is a
**narrowing** of the caps wall, not a bypass: workspace isolation, the deny path and audit all
apply. No account, no password, no session beyond the token. Withdrawn / expired / replied-past-the
window → **410** with a plain page, never a login prompt — and the body states the possibilities as
a **disjunction** ("may have been completed, withdrawn, or simply timed out"), never committing to
which. "This token expired" tells a prober the string was real. IP rate-limited by the same
fixed-window limiter `/public/invite/accept` uses — the route is a token oracle.

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

## Contract corrections found by building it

The scope was written before the records existed; three prose details did not survive contact with
the built code. Recorded here rather than silently edited, because a downstream consumer that
mirrored the prose is exactly how these bite:

| the scope said | the record does | how it was found |
|---|---|---|
| `case_event.seq` | **`eseq`** (a `#[serde(rename)]`) | the UI mirrored the prose; a reader of `seq` gets nothing |
| `case_request` list returns `{ case, member_count }` | `CaseRow` is `Case` **flattened** plus `member_count` | same |
| `external_ref: { system, id, url }` | a plain opaque **string** | same |

The lesson for the next slice: **mirror the Rust, not the scope.** A scope is the ask; the record is
the contract. Where they disagree after the code lands, the record wins and this file is corrected.

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

### 2026-10 — the Insights review (personal pins, SLA pause on hold, closing reasons, My Insights lanes)

8. **A pin is a person's own bookmark.** `case.pin` adds or removes the caller's subject in the
   case's `pinned_by`; the `pinned` list axis reads only the caller's (`pinned_for`, set by the host
   from the session and never read from the wire). It is not work on the case: no history event, no
   `last_activity_ts` bump, and it rides the read grant, so a viewer may pin. *Rejected: a team
   flag* — the product asked for personal pins (a team pin was built first and reversed the same
   day).
9. **The SLA pauses for a snooze with an end date, and only for that.** This reverses "never
   pause" for one gesture. A snooze already needs a reason; to pause it must also end within a
   year (`MAX_SLA_PAUSE_MS`) — an open-ended snooze keeps the clock running, so a pause cannot
   become a way to never breach. `waiting_on` (an email out, a purchase order) still never pauses.
   While paused the case reads as it stood at `sla_paused_at` and no breach is recorded; when the
   snooze ends (un-snooze, expiry, or a severity puncture) the business time it paused, on the
   policy's calendar, is added to `sla_paused_ms`, and the deadlines — now a pure function of
   `(policy, opened_ts, sla_paused_ms)` — move forward by exactly that. History: `sla_paused` /
   `sla_resumed {paused_ms}`.

   > **Amended 2026-10-29 — the clock runs only while the case is Open.** The SLA is now the time
   > to a first response. `due_at` is the policy's `respond_h` on its calendar (`resolve_h` stays on
   > the policy, unused by the clock), and the clock stops the moment the case leaves Open — actioned,
   > assigned, emailed (`waiting_on` included), on hold with or without an end date, or closed — and
   > carries on from the time already used if it comes back to Open: only time spent Open counts.
   > The one write path (`lb_cases` `save`, via `sla_follow`) starts and ends the pause on every
   > move, so no verb has to remember; the host's case tick (`follow_sla_clocks`) pauses cases
   > stored before the rule from their last activity and recomputes every open case's deadline,
   > which on the first tick after the upgrade moves them onto the respond target. A recomputed
   > deadline is not activity (`save_quiet`). Out of Open the case reads `paused` and no breach is
   > recorded; a breach already recorded stays history — through an action, a close and a manual
   > reopen; a close without one reads `met`. A reopen moves the deadline at once (`case.workflow`
   > recomputes it). The "open-ended snooze keeps the clock running" and "`waiting_on` never
   > pauses" clauses above describe the earlier rule.
10. **A closing reason in the closer's words rides `resolved`.** The closed `resolution` set stays
    what the hold-down and the scorecard read; `reason` is the label the person chose ("Fixed
    remotely" under `fixed`, "No action required" under `accepted_risk`), kept on the case and the
    `resolved` event. *Rejected: more `Resolution` variants* — the scorecard's precision formula
    and the hold-down both read the closed set, and a product's wording does not belong in it.
11. **My Insights' five lanes are server lanes, personal and disjoint.** `case.list` takes `lane`
    `action` / `assigned` / `emailed` / `hold` / `closed` beside the ownership three; `my_lane`
    puts each case in at most one, in that precedence order: closed by me in the last 7 days →
    snoozed and mine (or snoozed by me) → emailed by me with the ball still out → mine → assigned
    by me to someone else. "By me" needed two new facts on the case: `assigned_by` (set by
    `case.assign`, cleared with the assignment) and `emailed_by` (set by `case.request.send`).
    *Rejected: lanes computed in the UI* — a second client (ESR) would have to copy the rule, and
    "by me" is not on the wire without the two fields anyway.
12. **A close holds until the alarm fires again.** Closing a case does not resolve its insight, so
    the reconcile backstop used to reopen a recent `fixed` ("repair did not hold") or open a new
    case within a tick, for an alarm that had not fired again. It now skips an insight whose last
    case closed at or after the insight's `last_ts` (`close_stands`). A new firing still goes
    through the inline grouping at raise, so the hold-down rules are unchanged. *Rejected:
    resolving the insight on close* — it would mark an alarm cleared while it is still active.
13. **One state at a time; the latest action is the state.** A case reads as exactly one of Open,
    Assigned, Emailed, On hold, Closed (`case_status`), with the owner beside it, never instead
    of it. Each verb ends the state before it:
    - **Assign** someone new ends a running hold and an lb-sent email-out (`emailed_by`); the
      request row stays and a reply still lands. Assigning the owner they already have changes
      nothing.
    - **Unassign** leaves a hold or an email-out in place (it is then what holds the work); work
      nothing else holds goes back to Open.
    - **Email** (`case.request.send`) ends a hold; **hold** (`case.snooze`) ends an lb-sent
      email-out. Either one, by a person (`user:`), on work with no owner makes that person the
      owner (`assigned` event); a system actor never takes ownership.
    - **Hold ending** goes back to Assigned when there is an owner, else Open.
    - **Close** ends the hold and the email-out and keeps the owner on record. **Reopen** to Open
      clears the owner too (`assigned` event): a reopened job goes back to the queue, as a new one.

    | Start | Assign me | Assign other | Unassign | Email | Hold | End hold | Close | Reopen |
    |---|---|---|---|---|---|---|---|---|
    | Open · – | Assigned · me | Assigned · other | – | Emailed · me | On hold · me | – | Closed · – | – |
    | Assigned · me | no change | Assigned · other | Open · – | Emailed · me | On hold · me | – | Closed · me | – |
    | Assigned · other | Assigned · me | Assigned · other | Open · – | Emailed · other | On hold · other | – | Closed · other | – |
    | Emailed · me | no change | Assigned · other | Emailed · – | Emailed · me | On hold · me | – | Closed · me | – |
    | Emailed · other | Assigned · me | Assigned · other | Emailed · – | Emailed · other | On hold · other | – | Closed · other | – |
    | Emailed · – | Assigned · me | Assigned · other | – | Emailed · me | On hold · me | – | Closed · – | – |
    | On hold · me | no change | Assigned · other | On hold · – | Emailed · me | On hold · me | Assigned · me | Closed · me | – |
    | On hold · other | Assigned · me | Assigned · other | On hold · – | Emailed · other | On hold · other | Assigned · other | Closed · other | – |
    | On hold · – | Assigned · me | Assigned · other | – | Emailed · me | On hold · me | Open · – | Closed · – | – |
    | Closed | – | – | – | – | – | – | – | Open · – |

    *Rejected: Reopen assigns the reopener* (the mock-up's choice) — a case that reopens itself on
    a new firing has no one to give it to, and the bulk Reopen already went to Open.

## Related

- The product half: `NubeIO/rubix-ai → docs/scope/insights/case-plane-scope.md` (personas, money,
  the four surfaces, the client brief).
- `insight-tag-precedence-scope.md` (the fold + the door it names), `insight-triage-scope.md` (the
  immunity invariant this extends), `insight-evidence-scope.md` (`subjects` lands here),
  `insight-notify-scope.md` (breakthrough semantics the caveat and snooze reuse),
  `insight-analysis-scope.md` (`Quantity`).
- Seams reused: the outbox `Target` (`host/src/outbox/target.rs`), reminders, assets, the tag graph.
