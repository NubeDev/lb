# Insights scope — a rule raises (and acks/closes) an insight from its body

Status: scope (the ask). Promotes to `doc-site/content/public/insights/` once shipped.

A rule can already reach the messaging plane in one line — `inbox.record`, `outbox.enqueue`,
`channel.post` — each rhai handle riding the generic `MessagingSeam::call(tool, input)`
chokepoint (rules-messaging-scope). It **cannot** raise an insight except by wiring a flow and
routing through the flow's `insight` sink node. That is the wrong amount of ceremony for a
threshold rule whose whole job is "notice a fault and record it". We want the rule to do it
inline:

```rhai
let key = insight.raise(#{
    dedup_key: "cooler-temp-high",
    severity: "warning",
    title: "Cooler temp high",
    body: #{ series: "cooler.temp", value: 9.1 },
    tags: #{ area: "hvac" },
});
// …and, when the same rule later sees the fault clear or a human standing behind the run:
insight.ack(key);      // open → acked
insight.close(key);    // * → resolved   (the user's "close, not just raise")
```

The host verbs **already exist** — `insight.raise` / `insight.ack` / `insight.resolve`
(`rust/crates/host/src/insight/{raise,ack,resolve}.rs`), each capability-gated, each forcing
the actor (`producer` / `acked_by` / `resolved_by`) to the principal, never caller-supplied.
`insight/raise.rs`'s own module doc already promises this door: *"The rule door (the rhai
handle) and the flow door (the `insight` sink node) reach this same verb through the same
gate."* This scope builds that **rule door** — the intended-but-unbuilt half — and nothing
else: **no new MCP verb, no new capability**. One new rhai handle over the seam the messaging
handles already use.

## Goals

- A rule body raises an insight in one call (`insight.raise(#{…}) -> id`), reaching the
  **existing** `insight.raise` verb through the existing generic seam.
- The same handle **acks** and **closes** an insight (`insight.ack(id)`, `insight.close(id)`)
  over the existing `insight.ack` / `insight.resolve` verbs — the user's "acknowledge and
  close, not just raise". A rule that opens a fault can also stand it down when the fault
  clears, in the same body, without a flow.
- Every call re-runs the host gate (`workspace pin` + `mcp:insight.<verb>:call` under
  `caller ∩ grant`); a rule holding `rules.run` but **not** the insight cap is denied
  mid-run, **opaquely**, with no partial write.
- Raising/acking/closing are **charged** against the per-run `WriteMeter`, so an alert-storm
  rule trips `max_writes` exactly as an `outbox.enqueue` / `channel.post` loop already does.
- A re-run with the same `dedup_key` **upserts** via the existing `(ws, dedup_key)` dedup —
  one insight, `count` incremented, never a duplicate.
- A **`route:false`** (read-only panel) run does **not** raise, ack, or close — the same rule
  drawing a panel every 30 s must not stamp a durable record + notify fan-out on every repaint.

## Non-goals

- **No new host verb and no new capability.** The three producer/lifecycle verbs and their
  grants (`mcp:insight.raise|ack|resolve:call`) already ship. This is a cage-side door onto
  them, nothing more (rule 7: one MCP contract, reached the same way by rule/flow/agent/UI).
- **Not a read surface in v1.** `insight.get` / `insight.list` / `insight.watch` are **not**
  exposed to the cage here — a rule *produces* insights; it discovers/queries them through
  the data plane (`source("store")`) if it must. (Open question 5 revisits a read verb.)
- **Not subscriptions/notify/policy.** `insight.sub.*`, `insight.policy.*` stay agent/UI/CLI
  verbs — an author-facing rule door onto workspace configuration is out of scope.
- **Not a replacement for `emit`/`alert`.** Findings are the run *result*; `insight.raise` is
  a durable cross-cutting record. The boundary is stated below and in the skill doc.
- No flow changes — the flow `insight` sink node is untouched; this is the parallel door.

## Intent / approach

A new **`InsightHandle`**, structurally identical to `ChannelHandle`
(`rust/crates/rules/src/verbs/channel.rs`): it holds `Arc<dyn MessagingSeam>` + the shared
`Arc<WriteMeter>` + the run's logical `now`, and each method is one
`self.seam.call("insight.<verb>", json!({…}))`. It is pushed as a top-level scope variable
`insight` alongside `ai` / `inbox` / `outbox` / `channel` in `RunHandles`.

- `insight.raise(map) -> String` — charge the meter (after validation), inject `ts: now`,
  hand the map's `dedup_key`/`severity`/`title`/`body`/`origin?`/`tags`/`occurrence?` to the
  seam, return `outcome.id` so the author can ack/close it later in the same body.
- `insight.ack(id)` / `insight.close(id [, note])` — charge the meter, call `insight.ack` /
  `insight.resolve` with `{ id, ts: now }` (+ optional `note` on close). `close` is the
  author-facing name for the `insight.resolve` verb — "close" reads as the lifecycle end a
  rule author means, and matches the ask; the underlying verb/cap names are unchanged.

Because the map's `producer` (and the verb's `acked_by`/`resolved_by`) are **host-forced from
the principal**, the handle never carries or forwards an actor field — a rule cannot forge one
even by putting it in the map (the host overwrites it, `raise.rs:38`).

**Rejected alternative — a method on an existing handle** (`inbox.raise_insight(…)` or a
generic `mcp.call("insight.raise", …)`). Rejected because (a) insights are a distinct plane
from the inbox/outbox/channel messaging plane — folding a fourth surface onto `inbox` blurs
"attention item" vs "durable finding" for the author, the exact confusion the emit/alert
boundary below fights; and (b) a generic `mcp.call` cage escape hatch would let a rule reach
*any* verb by string, detonating the whole point of the curated catalog (rule 5 — "what a rule
body may call, and nothing else"). A dedicated `insight` handle keeps the surface small,
named, and catalogued, exactly like `channel`.

**Rejected alternative — fold the catalog rows into the `messaging` family.** Rejected: the
catalog `family` names the plane, and `rules.help` / the skill doc / UI autocomplete group by
it. Insights are not messaging (state vs motion: an inbox item is attention motion; an insight
is a durable record). A new **`insight`** family keeps the grouping honest — and it forces the
`families_are_the_known_set` + `catalog_has_entries_from_every_verb_module` integrity tests to
be updated in lock-step (a feature, not a chore: the test is the tripwire that a family was
added deliberately).

## How it fits the core

- **Tenancy / isolation (rule 6):** the seam is closed over the run's pinned workspace; every
  `call_tool` re-pins ws before the cap check. A rule in ws-A physically cannot raise, ack, or
  close into ws-B — the `dedup_key` and the insight `id` are both ws-scoped keys, and a
  cross-ws `id` resolves to "not found" / opaque deny, never another tenant's record.
  **Isolation-tested** (below).
- **Capabilities (rule 5):** no new grant. `insight.raise` needs `mcp:insight.raise:call`,
  `ack` needs `mcp:insight.ack:call`, `close` needs `mcp:insight.resolve:call` — each
  re-checked *inside* the seam's `call_tool` under `caller ∩ grant`, mid-run. A rule with
  `rules.run` but without the insight cap is denied **opaquely** (`SeamError::Denied` →
  bare "denied", no verb name leaked) with **no partial write** — the meter is charged, but
  the host verb never runs, so no record lands. The `producer`/`acked_by`/`resolved_by` are
  un-spoofable (host-forced from the principal).
- **Symmetric nodes (rule 1):** the handle is pure cage code; no `if cloud`. A rule raises an
  insight identically on edge and cloud — the verb runs wherever the run runs.
- **One datastore (rule 2):** the insight record + occurrence ring land in SurrealDB via the
  existing `lb_insights::raise`; the handle adds no persistence.
- **State vs motion (rule 3):** the insight **record** is state (SurrealDB); the raise's live
  `RaiseEvent` on `ws/{ws}/insight/events` and the notify deliveries are motion (Zenoh /
  channel posts) — both already owned by `insight_raise`'s host layer, untouched here.
- **MCP surface (§6.1):** **no verb added.** An existing **write** verb (`insight.raise`) and
  two existing **lifecycle** verbs (`ack`/`resolve`) become reachable from the cage. Get/list
  N/A (non-goal — a rule produces, it doesn't browse). Live-feed N/A (the raise *emits* the
  bus event; a rule doesn't subscribe). Batch N/A (a rule body raises per-fault, bounded by
  the write meter — an insight-storm is a bug the `max_writes` governor already stops, not a
  batch API).
- **No mocks (rule 9):** the tests raise into a **real** `mem://` store through the real
  engine + real seam + real `insight.raise` verb, and count records before/after. No
  `*.fake.ts`, no stubbed seam.
- **Stateless (rule 4):** the handle holds only the run's seam/meter/`now` — no durable state.
- **One responsibility per file (rule 8):** one new file `verbs/insight.rs` (the handle);
  edits to `verbs/mod.rs` (register + push) and `catalog.rs` (rows + family) only.
- **Rule 10 (core knows no extension):** insights are a **core plane**; the handle names no
  extension id and the seam treats the tool string as opaque data. Fine.
- **SDK/WIT impact:** none — the cage surface is not the plugin boundary.

## The `route:false` tension (design question 3 — decided)

This is the load-bearing decision. `rules-for-widgets` slice 2 gave `rules.run`/`rules.eval`
an optional **`route: false`** (default `true`) so a **panel-driven** run — a dashboard
auto-refreshing every 30 s — does **not** stamp a fresh inbox item + must-deliver outbox entry
on every repaint from the rule's `alert()`. The question: does a repainting panel that calls
`insight.raise` still write?

**Decision: `route:false` suppresses `insight.raise`, `insight.ack`, and `insight.close`
too — the handle is a no-op on a read-only run** (returns a synthetic/echoed id for `raise`,
`()` for ack/close, charges nothing). Rationale:

- An `insight.raise` is a *stronger* motion-producing effect than an `alert()`: it writes a
  durable record **and** fans out the raise-time matcher → notify ladder → channel deliveries.
  If `alert()` (the weaker effect) is suppressed on a panel repaint, raising an insight — the
  strictly heavier one — must be too, or slice 2's whole promise ("a repainting dashboard does
  not spam attention surfaces") is a lie the moment a rule author reaches for `insight.raise`
  instead of `alert()`.
- Dedup does **not** save us. Yes, `(ws, dedup_key)` collapses the record to one insight — but
  every repaint still bumps `count`, appends an occurrence row, and re-fires the notify
  matcher. A panel viewed by ten people, refreshing every 30 s, would inflate `count` and
  re-deliver notifications purely from *viewing*, which is nonsense.
- The suppression must be **honest, not silent**: on a `route:false` run the handle records a
  cage `log` line ("insight.raise skipped: read-only panel run") so the author isn't confused
  by a missing record — the same honesty rule the workbench applies to a suppressed alert.

**Rejected alternative — always write, rely on dedup.** Rejected for the count/occurrence/
notify inflation above: dedup makes the *record* idempotent, not the *side effects*.

Mechanically, `route` is a run-level flag the host already threads into the run (slice 2). The
`InsightHandle` is constructed with that flag (a `route: bool` field, like `now`); each method
short-circuits before charging the meter when `route == false`. (Symmetric with how the host
skips `alert()` routing — the flag lives on the run, the handle honors it.)

## The emit/alert vs insight.raise boundary (design question 4)

State it so authors aren't confused about which to reach for:

| You want… | Use | Lives where | Lifespan |
| --- | --- | --- | --- |
| A value in the run's **result** (a computed number, a row, a note the caller reads back) | `emit(#{…})` / `log(…)` | The `RuleOutput.findings` this run returns | Ephemeral — gone after the run |
| To **route attention** now (raise an inbox item + a must-deliver outbox notification) | `alert(#{…})` | Inbox + outbox (motion), subject to `route:false` | The inbox item's own lifecycle |
| A **durable, queryable, deduped** cross-cutting record ("all open critical HVAC faults this week") with severity + occurrence history + a lifecycle | `insight.raise(#{…})` | The `insight:{ws}:{id}` record + occurrence ring (state) | open → acked → resolved, deduped on `dedup_key` |

The one-liner: **`emit` is what this run found; `alert` is "someone look now"; `insight.raise`
is "this fault exists in the world until closed".** An author who wants a durable fault record
that survives the run and dedups across runs reaches for `insight.raise`; an author who wants
the run's output rows reaches for `emit`. They compose freely (a rule can `emit` a summary
*and* `insight.raise` the underlying fault).

## Fencing (design question 5 — decided: none needed)

`channel.post` fences `kind:"agent"`/`kind:"query"` (a bounded, synchronous rule must not
kick off an unbounded agent/query *run*). `insight.raise`/`ack`/`close` need **no such
fence**: they are bounded writes with no run-spawning payload — raising an insight cannot cause
a rule to execute, only a notify fan-out the host already bounds (the matcher is a bounded
subscription scan, deliveries are bounded channel posts). The only bound that matters is the
**`WriteMeter`** (an insight-storm loop trips `max_writes`), and that is already in place. So:
no worker-kind fence, and the scope says so explicitly rather than leaving it as an unresolved
"maybe".

## Metering (design question 2 — decided)

Raising/acking/closing are **charged** against the per-run `WriteMeter`, exactly like
`channel.post` / `outbox.enqueue` — each is a motion-producing write. The meter is charged
**only after validation** (mirroring `ChannelHandle::post`, which charges *after* the
worker-kind fence passes): for `insight.raise` that means after the map has the required
`dedup_key`/`severity`/`title` and after the `route:false` short-circuit. This makes the
`max_writes` governor an alert-storm bound: a rule that loops raising insights trips
`max_writes` and the run fails with the meter error, no different from a `channel.post` loop.
(A rejected raise on a `route:false` run charges **nothing** — it never gets past the
short-circuit.)

## Determinism / re-run behavior (design question 3, dedup half — decided)

A scheduled flow (`rules.eval`, `route:true`) that re-runs a rule with the same `dedup_key`
must **upsert/re-open** through the existing `(ws, dedup_key)` dedup, **not** create a
duplicate — this is already the raise verb's load-bearing branch (`raise.rs`: open/acked ⇒
bump `count`+`last_ts`; resolved ⇒ re-open; no key ⇒ create). The handle inherits it for free:
it forwards `dedup_key` verbatim and injects `ts: now` (the run's logical clock, no wall-clock
— testing §3), so a deterministic re-run at the same `now` upserts idempotently. The
`insight.ack`/`insight.resolve` verbs are likewise idempotent on the insight `id`. Nothing new
to build here — the scope's job is to **guarantee the handle preserves** this by passing the
key/id/ts straight through and adding no client-side id generation.

## Example flow

A threshold rule watching cooler temperature, run two ways:

1. **Scheduled (a cron flow, `route:true`).** The rule computes `max(cooler.temp) = 9.1`, over
   its 5 °C threshold. It calls `insight.raise(#{ dedup_key:"cooler-temp-high",
   severity:"warning", title:"Cooler temp high", body:#{series:"cooler.temp", value:9.1},
   tags:#{area:"hvac"} })`.
2. The handle charges the write meter (`seq=1`), injects `ts: now`, and calls the seam →
   `call_tool(node, principal, ws, "insight.raise", …)` → cap check `mcp:insight.raise:call`
   under `caller ∩ grant` → `insight_raise`. First firing: a new `insight:{ws}:{id}` record,
   `producer` = the run's principal, tags applied, `RaiseEvent` on the ws subject, notify
   matcher fires. Returns `id`.
3. **Next cron tick**, temp still 9.1. Same `dedup_key` → the verb **bumps `count` to 2**,
   status untouched (an acked fault doesn't re-page) — **one** insight, not two.
4. Temp drops to 3 °C. The rule sees it clear and calls `insight.close(key)` →
   `mcp:insight.resolve:call` re-checked → `* → resolved`. Charged (`seq=…`). The fault record
   is now closed, still queryable in history.
5. **Panel repaint (`route:false`).** The dashboard renders the *same* rule as a chart source
   every 30 s. Each repaint's `insight.raise`/`ack`/`close` is a **no-op** — the handle
   short-circuits, charges nothing, logs "skipped: read-only panel run", returns an echoed id.
   The Insights page stays quiet; `count` does not inflate from viewing.
6. **Deny path.** A rule holding `rules.run` but not `mcp:insight.raise:call` reaches step 2;
   the seam's `call_tool` cap check fails → `SeamError::Denied` → the handle raises a bare
   "denied" author error mid-run. No record lands (no partial write); the run fails opaquely.

## Testing plan

Real store + real engine + real seam (testing-scope §0 — no mocks, seed real records). Model
on `rust/crates/rules/tests/messaging_test.rs` (the handle-over-seam pattern) and
`rust/crates/host/tests/rules_test.rs` (the host-wired end-to-end run). Mandatory categories:

1. **Happy path (real write).** A rule body calling `insight.raise(#{…})` lands a **real**
   insight record — count `insight:{ws}:*` before/after in the `mem://` store (0 → 1), assert
   `producer` = the run principal, `dedup_key`/`severity`/`title` persisted, one occurrence row.
2. **Ack + close (the user's ask).** The same rule calls `insight.ack(id)` then
   `insight.close(id)`; assert the record goes `open → acked → resolved`, `acked_by` /
   `resolved_by` = the principal (un-spoofable), both idempotent (a second `close` is a no-op).
3. **Capability-deny (mandatory).** A principal with `rules.run` but **not**
   `mcp:insight.raise:call` runs a raising rule → **opaque** mid-run deny (`Denied`, no verb
   leaked), and **no partial write** (record count unchanged). Repeat for a rule that raises OK
   but lacks `mcp:insight.resolve:call` on `close` — denied mid-run after the raise landed.
4. **Workspace-isolation (mandatory).** A ws-A rule cannot raise into ws-B: same `dedup_key`
   in two workspaces yields two **independent** insights; a ws-A rule handed a ws-B insight
   `id` to `close` gets "not found" / opaque deny, and ws-B's record is untouched.
5. **Deterministic re-run / dedup.** Run the same rule twice at the same logical `now` with
   the same `dedup_key` → **one** insight, `count == 2`, not two records. Then re-run after a
   `close` → the insight **re-opens** (`resolved → open`, count continues).
6. **WriteMeter bound.** A rule looping `insight.raise` past `max_writes` **trips the governor**
   — the run fails with the meter error, exactly like a `channel.post` loop (assert the same
   error class).
7. **`route:false` suppression.** A raising/acking/closing rule run with `route:false` writes
   **nothing** — record count unchanged, meter charged nothing, and the run still succeeds
   (findings/emit still returned). Compare against the same rule at `route:true` (writes).
8. **Catalog integrity.** The new `insight` family rows are present; `catalog_is_complete` /
   `families_are_the_known_set` / `catalog_has_entries_from_every_verb_module` pass with
   `insight` added to the known set (updated in lock-step).

## Stubs — files that will change (one responsibility per file, no utils)

| File | Change |
| --- | --- |
| `rust/crates/rules/src/verbs/insight.rs` **(new)** | The `InsightHandle` (seam + meter + `now` + `route` flag) with `raise` / `ack` / `close`; a `register(engine)` that registers the three rhai fns. Mirrors `verbs/channel.rs`. |
| `rust/crates/rules/src/verbs/mod.rs` | `mod insight; pub use insight::InsightHandle;` — call `insight::register(engine)`, add `insight: InsightHandle` to `RunHandles`, construct + push it as the `insight` scope var (parallel to `channel`). Update the `RunHandles` doc comment ("four scope handles" → five). |
| `rust/crates/rules/src/catalog.rs` | Add the `insight.raise` / `insight.ack` / `insight.close` rows under a **new `insight` family**; add `"insight"` to the `families_are_the_known_set` + `catalog_has_entries_from_every_verb_module` known sets (lock-step, per the maintenance rule). |
| `rust/crates/host/src/rules/seam.rs` | **Likely no change** — `HostMessagingSeam::call` is a *generic* `call_tool(tool, …)` chokepoint that treats `tool` as opaque data (rule 10) and already gates any verb by `mcp:<tool>:call`. `insight.raise`/`ack`/`resolve` route through it unmodified. **Confirm** the host constructs the run's `route` flag and threads it into `RunHandles` (slice-2 plumbing) so the new handle can honor it — that wiring, if not already present, is the one host edit. |
| `rust/crates/rules/tests/messaging_test.rs` (or a new `insight_test.rs`) + `rust/crates/host/tests/rules_test.rs` | The testing plan above. A new `insight_test.rs` is cleaner (one responsibility) if the messaging file is already large. |
| `docs/skills/rules/SKILL.md` | A new **"raising an insight"** chapter — the `insight.raise/ack/close` surface, the `route:false` panel no-op, and the emit/alert/insight boundary table. |

## Risks & hard problems

- **The `route:false` thread must actually reach the handle.** The whole suppression decision
  depends on the run knowing its `route` flag at handle-construction time. If slice 2 kept
  `route` purely in the host's post-run routing (not passed into the engine), this scope needs
  the host to thread it into `RunHandles`. **Verify slice-2 plumbing first** — this is the
  most likely place the build stalls. (Confirmed direction, not confirmed wiring.)
- **Author confusion emit vs alert vs insight** — three overlapping "record something" verbs.
  Mitigated by the boundary table (here + in the skill), but it's a real cognitive load; the
  skill chapter must lead with the one-liner distinction, not the mechanics.
- **`close` naming vs the `insight.resolve` verb.** The author-facing `close` maps to the
  `resolve` verb/cap. A reader grepping for `resolve` won't find `close` and vice-versa —
  the handle's doc comment must state the mapping loudly (as `channel.post`'s does for its
  fence).
- **Metering an idempotent re-open.** A `route:true` re-run charges the meter even when the
  raise only bumps `count` (no new record). That's correct (it *is* a write attempt), but an
  author might expect "no new record = no charge". Document it: the meter counts write
  *attempts*, not new rows.

## Open questions

1. **`origin` on the raise map.** `RaiseInput.origin` is required at the verb but a rule author
   rarely wants to hand-build one. Should the handle **default `origin`** to the run's
   provenance (rule id + run id, which the cage knows) when the map omits it — so
   `insight.raise` needs only `dedup_key`/`severity`/`title`? (Lean: yes — the rule *is* the
   origin; making the author supply it invites copy-paste errors. Decide the exact `Origin`
   shape the cage can synthesize.)
2. **Does the flows `insight` sink node get the same `route` honoring?** Slice 2's open
   questions asked whether the flows `rule` node exposes the `route` knob. If a flow can run a
   rule `route:false`, the `insight` sink node inside that flow should suppress too — align the
   two doors so "read-only run" means the same thing through both.
3. **`insight.ack`/`close` without a prior `raise` in the same run.** A rule may want to
   ack/close an insight raised by a *different* run (id passed as a param). The handle allows
   it (it just calls the verb by id), but should the catalog/skill steer authors toward
   raise→ack→close within one body, or explicitly bless cross-run lifecycle from a rule?
4. **`severity` as a string vs enum at the cage.** The map uses `severity: "warning"`; the
   verb wants a `Severity`. Confirm the seam's JSON round-trip deserializes the string into the
   enum cleanly (it should — `RaiseInput` derives `Deserialize`), and that a bad severity
   string is **BadInput author feedback** (surfaced verbatim), not an opaque deny.
5. **A minimal read door later?** v1 is produce-only. A follow-up `insight.get(id)` (read,
   uncharged) would let a rule branch on an insight's current status before acking/closing.
   Out of scope now; noted so the non-goal is a deliberate deferral, not an oversight.

## Related

- [`insights-scope.md`](insights-scope.md) — the umbrella (the record, the producer doors, the
  page); this scope builds the promised **rule producer door**.
- [`insights-package-scope.md`](insights-package-scope.md) — the `lb-insights` crate this rides.
- [`insight-occurrences-scope.md`](insight-occurrences-scope.md) — the per-raise occurrence
  ring a rule raise appends to.
- [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md),
  [`insight-notify-scope.md`](insight-notify-scope.md) — the matcher + notify fan-out a
  raise triggers (the reason a `route:false` panel run must not raise).
- [`../rules/rules-engine-scope.md`](../rules/rules-engine-scope.md) — the cage, `RuleOutput`,
  the `WriteMeter`/`max_writes` governor, `emit`/`alert` routing.
- [`../rules/rules-messaging-scope.md`](../rules/rules-messaging-scope.md) — the
  `inbox`/`outbox`/`channel` handles this handle mirrors, over the same `MessagingSeam`.
- [`../frontend/dashboard/rules-for-widgets-scope.md`](../frontend/dashboard/rules-for-widgets-scope.md)
  slice 2 — the `route:false` read-only-run flag whose semantics this scope extends to insights.
- `README.md` §3 rules **5** (capability-first), **6** (workspace wall), **7** (MCP is the
  contract), **10** (core knows no extension).
- `docs/skills/rules/SKILL.md` — gains the "raising an insight" chapter (the implementing
  session owns writing it, grounded in a live run).
</content>
</invoke>
