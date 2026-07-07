# Insights — scaffolding session

- Date: 2026-07-05
- Scope: ../../scope/insights/insights-scope.md (umbrella) + insight-occurrences-scope.md +
  insight-subscriptions-scope.md + insight-notify-scope.md (the four source-of-truth docs)
- Stage: scaffolding only — **every dir/file/type/signature/test-name is in place; core logic
  is `todo!()`**. The implementing session opens this folder and fills bodies against the scope,
  with zero guesswork about where anything goes.
- Status: scaffold complete; the punch-list below is the implementing session's task list.

## What this session did

The grunt work — high-volume, low-risk, mechanical — so a higher-model session can think only
about algorithms. **Everything is wired end to end except the algorithm bodies.** A green-but-
lying stub is impossible: every deferred body is a `todo!("insights: <what the scope says> —
SCOPE: <file>.md §<section>")` that surfaces a panic on reach (the test plan is intentionally
not green today; it's a ready test plan).

### What's BUILT (real code, compiles, plumbs end to end)

- **New crate `lb-insights`** (`rust/crates/insights/`) — the `lb-inbox`-altitude record + verb
  layer. Cargo.toml mirrors lb-inbox; lib.rs is a barrel; one responsibility per file
  (FILE-LAYOUT). Added to workspace `members` + `workspace.dependencies` as `lb-insights`.
- **Every record struct** with full fields + serde derives, exactly per the scope docs:
  `Insight`, `Occurrence`, `Subscription`, `NotifyState`, `Policy`, plus the enums `Severity`
  (info/warning/critical with `rank`/`at_least`/`max`), `Status` (open/acked/resolved), `Origin`
  /`OriginKind` (rule/flow/agent/ext/manual), `SubSink`/`SubSinkKind`, `SubFilter`, `Intent`/
  `IntentKind`, `ThrottleOverride`, `DormantReason`, `EventKind`/`RaiseEvent`, `Delivery`/
  `DeliveryReason`, `Level`. Table-const helpers + `record_id`/`dedup_lookup`/`event_subject`
  derived per the lb-inbox pattern.
- **Every verb file** as a function with the correct signature, doc comment, and error type —
  body stubbed. One verb per file in `lb-insights`: `raise`, `get`, `list`, `ack`, `resolve`,
  `watch` (bus event shapes), `occurrences`, `occ_append`, `sub_create`/`list`/`get`/`delete`/
  `mute`, `policy_get`/`policy_set`, the **matcher** (`match_subs`), the **ladder state machine**
  (`ladder_step` — signature + types only), and the **digest reactor** (`react_to_insight_digests`).
- **Host service `crates/host/src/insight/`** — capability-gated wrappers + the MCP bridge
  `call_insight_tool` dispatcher (mirrors `nav/tool.rs`). One verb per file; each runs
  `authorize_tool` first then delegates to the `lb_insights` verb; `producer`/`owner`/`acked_by`/
  `resolved_by` are host-forced (never caller-supplied). `sub_create` does the create-time
  `bus:chan/{channel}:pub` no-widening gate.
- **Dispatch wiring** — `tool_call.rs::is_host_native` gained `insight.`; `dispatch_at_depth`
  has a new `insight.` arm routing to `call_insight_tool`; `host/src/lib.rs` declares `mod insight;`
  + re-exports the verbs + `call_insight_tool`; `system/catalog.rs` lists every `insight.*` verb
  (the catalog-coverage test stays honest).
- **Gateway REST surface** — `role/gateway/src/routes/insight.rs` (list/get/ack/resolve/
  occurrences); registered in `routes/mod.rs` + mounted in `server.rs`. The raise verb + subs +
  policy ride the universal MCP bridge (`POST /mcp/call`), per the scope's "one contract" rule.
- **`lb_prefs::Prefs` new axis** `insight_notifications: Option<bool>` (default true via serde;
  the whole-fold nullable prefs pattern). Zero host/gateway plumbing beyond the delivery-time
  read (the scope's explicit instruction).
- **`builtin.insights-analyst` persona** added to `personas.toml` — `extends builtin.data-analyst`
  + the investigation verbs (`insight.get/list/occurrences/ack/resolve`) + `rules.get`; deliberate
  exclusion of `insight.raise` (this persona investigates, doesn't mint); `grounding_skills`
  lists the future `core.insights` skill.
- **UI feature folder `ui/src/features/insights/`** — page shell + list + detail drawer + facets
  sidebar + actions + the `useInsights` data hook + `index.ts` barrel; data layer in
  `ui/src/lib/insights/` (`insights.api.ts` + `insights.types.ts` mirroring the Rust records 1:1).
  Layout + types are real; data binding is real (calls the real gateway); the SSE live tail +
  keyset paging + tag-facet picker (`tags.find`-driven) are TODO in the right places.
- **Test skeletons** at every layer, NAMED for the mandatory + scope-named cases, bodies
  `todo!()` / `it.todo()`:
  - `rust/crates/insights/tests/ladder_test.rs` — the PURE state machine unit surface (zero I/O):
    first-key/escalation/reopen breakthroughs, same-severity non-breakthrough, escalate/decay,
    L0 cooldown, ack-suppression, throttle-override, determinism.
  - `rust/crates/host/tests/insights_test.rs` — the integration headlines over a real `Node`:
    per-verb cap-deny, ws-isolation (list + occurrences), dedup-lifecycle, ring-cap, 2KB-reject,
    matcher-axes, ladder-escalate (the 5-min-nag example), digest-idempotency, kill-switch.
  - `rust/role/gateway/tests/insight_routes_test.rs` — REST + MCP bridge over the real gateway:
    per-route cap-deny, ws-isolation, the raise→list→get→ack→resolve round-trip.
  - `ui/src/features/insights/InsightsPage.gateway.test.tsx` — the page against a real spawned
    gateway: list/ack/ws-isolation at the UI layer.

### Verify (light, per the brief)

- `cargo fmt --all --check` — clean.
- `cargo check --workspace` — clean (only expected dead-code warnings on the stubs the
  implementing session will use).
- `cargo check --workspace --tests` — clean (every test target compiles; the `todo!()` bodies
  panic at RUNTIME, not at compile time — see "Expected runtime failures" below).
- `pnpm tsc --noEmit` (UI) — clean for every new `insights/*` file (pre-existing out-of-scope
  TS reds untouched).
- **NOT run**: `cargo test --workspace` (the logic isn't there — the bodies are `todo!()`; the
  full suite is the implementing session's exit gate, not this scaffold's). The pre-existing
  reds called out in the brief (SystemView.gateway, sqlSource.gateway, agent_routed_test,
  agent_persona_catalog_test's `MockProvider` import) are untouched — verified pre-existing on
  master, not caused by this scaffold.

### Expected runtime failures (intentional — the test plan is a PUNCH-LIST, not green today)

Every test listed below is `todo!()` / `it.todo()` — the test binary compiles, the test runs,
the test panics with `not yet implemented: insights: <what the scope says>`. This is the point:
the implementing session replaces each `todo!()` with the real assertion against the real verb,
and the test goes green. Do NOT mistake a `todo!()` panic for a regression — it's the stub rule
in action.

The compile-time dead-code warnings in `lb-insights` (7 of them: `OCC_TABLE`, `MAX_DATA_BYTES`,
`Policy::TABLE`, `occurrence_data_cap`, `read_insight`, `subscription::TABLE`, `notify_state`-
related) are the same shape — the implementing session uses them when it fills the bodies.

## Punch-list (the implementing session's task list, grouped by slice)

Each row: file → what's stubbed → scope reference. The order is roughly the order to fill (each
slice's tests are the acceptance gate).

### Slice 1 — the parent record + dedup (umbrella scope)

The headline. Get this green and the Insights page renders.

- `crates/insights/src/raise.rs::raise` — the dedup/re-open decision branch. Look up by
  `dedup_lookup`; open/acked → bump count/last_ts (status untouched); resolved → re-open
  (status=open); no match → create (mint ULID, count=1). `producer` host-stamped at the host
  layer already. Apply tags through the host's tag path AFTER the write (this crate is tag-graph-
  agnostic — the host calls `tags_add`). Return `RaiseOutcome { id, status, count, created,
  reopened }`. **SCOPE: insights-scope.md §"Dedup / flap suppression" + §"MCP surface"**.
- `crates/insights/src/raise.rs::read_insight` — a thin `read` + decode (the host uses this to
  read the post-raise state). **SCOPE: insights-scope.md §"MCP surface" (get)**.
- `crates/insights/src/list.rs::list` — the faceted, keyset-paged scan. Filter the cheap axes
  (status/severity) via the store `list` field path; post-filter the tag subset + range in Rust;
  order newest-first by (last_ts, id); keyset-paginate strictly after `query.cursor`; bound the
  page at `query.limit`. **SCOPE: insights-scope.md §"MCP surface" + page-cursor-scope.md**.
- `crates/insights/src/ack.rs::ack` — the transition. Read; absent ⇒ BadInput; resolved ⇒ BadInput
  ("stay resolved — re-open via raise"); already acked ⇒ no-op; else set status + status_by +
  status_ts. **SCOPE: insights-scope.md §"MCP surface" + notify-scope.md §"Ack means 'I know'"**.
- `crates/insights/src/resolve.rs::resolve` — the transition. Read; absent ⇒ BadInput; already
  resolved ⇒ no-op; else set status + status_by + status_ts; attach the optional `note` to the
  record's body under a `resolution` key. **SCOPE: insights-scope.md §"MCP surface"**.
- `crates/insights/src/get.rs::get` — **already implemented** (a thin store read; verify it
  round-trips in the integration test).

### Slice 2 — occurrences (the per-insight transaction ring)

- `crates/insights/src/occ_append.rs::append_occurrence` — 2 KB enforcement (serialize `data`,
  reject > `MAX_DATA_BYTES` as `BadInput` — never silent truncation; the raise verb should
  validate this BEFORE the parent write so a reject leaves no orphan row) + the capped-ring
  append via `lb_store::capped_insert` keyed by `insight_id`, cap from the policy record.
  `cap == 0` ⇒ store nothing (raise still succeeds). **SCOPE: occurrences-scope.md §"The record"
  + §"Verb surface"**.
- `crates/insights/src/occurrences.rs::occurrences` — the keyset-paged ring read. Filter by
  `insight_id` (the store `list` field path); order newest-first by `seq` (desc); keyset-paginate
  strictly-before `cursor.seq`; bound at `limit`; `next` from the oldest returned row.
  **SCOPE: occurrences-scope.md §"Verb surface" + page-cursor-scope.md**.
- Wire the occurrence append INTO `raise` (raise calls `append_occurrence` after the parent
  write; the raise input already carries the optional `occurrence` field).

### Slice 3 — subscriptions + the matcher (subscriptions scope)

- `crates/insights/src/sub_create.rs::sub_create` — count existing; reject ≥ `sub_cap` as
  BadInput; mint ULID; write the Subscription row (owner + principal from the host, NOT the body).
  **SCOPE: subscriptions-scope.md §"Verb surface"**.
- `crates/insights/src/sub_list.rs::sub_list` — admin lens (`all` ⇒ whole table) vs own (`list`
  by `owner`). **SCOPE: subscriptions-scope.md §"Verb surface"**.
- `crates/insights/src/sub_get.rs::sub_get` — thin `read` + decode.
- `crates/insights/src/sub_delete.rs::sub_delete` — `delete` (idempotent).
- `crates/insights/src/sub_mute.rs::sub_mute` — read + flip `muted` + write back; absent ⇒ BadInput.
- `crates/insights/src/match_subs.rs::match_subs` — **THE PURE MATCHER**. For each non-dormant
  sub: `origin_ref` exact-equals view.origin_ref (or absent); `dedup_key` exact-equals (or
  absent); `severity_min` ⇒ `view.severity.at_least(filter.severity_min)`; `tags` ⇒ subset check
  (every `(k,v)` in filter.tags is in view.tags). AND of all provided axes; all absent = "all
  insights". Muted subs STILL produce intents (the notify state accumulates). Return `Vec<Intent>`.
  **SCOPE: subscriptions-scope.md §"The raise-time matcher"**.
- Wire the matcher INTO the raise path (host calls `match_subs` after the record write +
  occurrence append + bus event; feeds intents to the notify engine).

### Slice 4 — the notify ladder + digest reactor (notify scope) — THE BRAINS

This is the single most important slice. The state machine is pure; the reactor is the durable
scan. Get the unit tests in `ladder_test.rs` green first (zero I/O), then the integration
headlines.

- `crates/insights/src/ladder.rs::ladder_step` — **THE PURE STATE MACHINE**. Read the algorithm
  spec carefully (notify-scope.md §"The state machine" + §"Example flow"). The order of checks
  in the body:
  1. `member_kill_switch_on == false` ⇒ skip delivery (accounting continues).
  2. `muted` ⇒ same.
  3. `Tick` ⇒ for each fully-elapsed window at the current level: if `pending.count > 0` ⇒ one
     `Digest` delivery, zero pending, then decay `level - 1`; if `pending.count == 0` ⇒ just decay.
  4. `Intent`:
     a. Breakthrough FIRST (regardless of throttle_override): kind=Reopen OR kind=Escalate OR
        `last_severity < intent.severity` OR no prior state ⇒ deliver now, keep the level.
     b. Ack suppression: if `acked` (and not a breakthrough) ⇒ update pending/window_hits, no
        delivery.
     c. Escalate: `window_hits += 1`; if `>= escalation_threshold` (and no pin) ⇒
        `level = min(level + 1, 4)`, reset window_hits, advance window_start.
     d. L0 within cooldown: post now (`L0Immediate`) + zero the cooldown's pending.
     e. L1..L4: accumulate into pending (the Tick will digest it).
  5. Update `last_severity`; return `(state', deliveries)`.
  **SCOPE: notify-scope.md §"The state machine" + §"Example flow (the 5-minute nag, tamed)"**.
- `crates/insights/src/digest.rs::react_to_insight_digests` — owner-election guard (the flows/
  reminders precedent); scan `insight_notify` rows whose `window_start + window(level) <= now`
  AND `pending.count > 0`; group by sub; compose ONE digest per (sub, window) aggregating all due
  keys (count, max_severity, top-K); feed `Tick` through `ladder_step` for each key (decay +
  advance windows + zero pending); `channel.post` each delivery under the sub's stored principal
  (fire-time re-check; on deny flip dormant per subscriptions scope); idempotent upsert (digest
  item id = `digest:{sub}:{window_start}`). **SCOPE: notify-scope.md §"The digest reactor"**.
- `crates/insights/src/policy_get.rs::policy_get` — thin `read` + decode, or `defaults()` on None.
- `crates/insights/src/policy_set.rs::policy_set` — the validate_ring_cap is wired; finish the
  write (`write(store, ws, TABLE, ws, &value)`).
- Wire the kill-switch read: the digest reactor reads `lb_prefs::Prefs::insight_notifications`
  for the sub's owner at delivery time; `Some(false)` ⇒ skip the post (accounting continues).

### Slice 5 — the producer doors (umbrella scope, deferred)

Once the record works, wire the two structured producer doors:

- **Rules handle** — register `insight.raise(#{…})` in the rhai cage beside `inbox`/`outbox`/
  `channel` (rules-messaging pattern). Host fills `origin = { kind:"rule", ref: rule_id, run:
  run_id }`. Follow `rules_messaging` exactly (caller-gated, write-metered). **SCOPE:
  insights-scope.md §"Producers" #1**.
- **Flow sink node** — the built-in `insight` sink node (descriptor in the built-in pack;
  config = severity/title/dedup_key/tags templated over `{payload, topic}`). Host fills `origin
  = { kind:"flow", ref: flow_id, run: run_id }`. Follow the `template` node precedent.
  **SCOPE: insights-scope.md §"Producers" #2 + flows data-nodes-scope.md**.

### Slice 6 — UI polish + SSE (frontend)

- `ui/src/features/insights/useInsights.ts` — keyset paging (load-more on `next`); the `act`
  callback body (real `ackInsight`/`resolveInsight`); `insight.watch` SSE subscription for live
  list updates.
- `ui/src/features/insights/InsightDetail.tsx` — typed body renderer (table/chart per the body's
  shape — the dashboard widget precedent); origin deep-link routing (rule/flow/run).
- `ui/src/features/insights/InsightFacets.tsx` — tag-facet picker driven by `tags.find` (the
  dashboard variable Query source precedent); `range` (time-window) facet.
- Route the page at `/t/$ws/insights` (the routing scope's deep-linkable target).

### Slice 7 — the persona + skill doc (umbrella scope)

- Add the `core.insights` grounding skill (the core-skills seed pattern) — referenced by the
  `builtin.insights-analyst` persona's `grounding_skills`.
- `skills/insights/SKILL.md` (raise/list/ack/resolve walkthrough grounded in a live run; the
  digest ladder in action; the kill switch).

## Open questions inherited from the scope docs

These are NOT for this scaffold to answer — they're recorded in each scope doc's "Open
questions" section and live with the implementing session that touches the matching slice.

- Umbrella: should `alert(...)` also raise an insight? Severity: closed forever or admin-extensible?
  Per-producer raise quota? Retention default for resolved? Auto-pinning the persona?
- Occurrences Q1: per-insight ring cap at first raise, or workspace-default only v1?
- Subscriptions Q1–3: trailing-`*` glob on origin_ref? team-owned subs? inbox sink kind?
- Notify Q1–4: severity-tiered cooldown? quiet hours? threshold counts raises or deliveries?
  AI-narrated digests?

## Cross-links

- Scope (the source of truth): [`scope/insights/insights-scope.md`](../../scope/insights/insights-scope.md)
  + the three sub-scopes.
- Scope-writing session (the scope's history): [`insights-scope-session.md`](./insights-scope-session.md).
- STATUS.md — NOT updated by this scaffold (no shipped state to report; the implementing session
  moves STATUS when the bodies land). The "Just scoped" line at the top still accurately
  describes now: "Not yet built — next up when picked." This scaffold IS the pick.
- public/ — NOT touched (nothing shipped yet; the implementing session promotes on completion).
