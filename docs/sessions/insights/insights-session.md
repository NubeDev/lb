# Insights — the finishing session (S8 insights → shipped)

- Date: 2026-07-05
- Scope: ../../scope/insights/insights-scope.md (umbrella) + insight-occurrences-scope.md +
  insight-subscriptions-scope.md + insight-notify-scope.md (the four source-of-truth docs)
- Stage: S8 (data plane) — Insights was the last "Just scoped" item; this session ships it.
- Status: done

## Goal

Land the **deliverables** for the insights feature, whose code was already written and green across
four layers (lb-insights crate, host wiring, gateway routes, UI) by prior sessions. This session
owns the doc/test/skill/public surface that `HOW-TO-CODE.md` requires, plus the build-blocker skill
doc and the non-trivial bug fixes that surfaced when the whole suite was driven end to end. Exit
gate: every scope doc's open questions current, the skill seeded, the debugging history complete,
STATUS moved, and the full workspace + UI green.

## What changed

### The build-blocker skill — `docs/skills/insights/SKILL.md` (created)

`rust/crates/assets/build.rs` embeds EVERY `docs/skills/<name>/SKILL.md` at build time and the
persona-grounding anti-rot gate fails the build if a dir is missing its SKILL.md. `core.insights`
was already in `DEFAULT_CORE_SKILLS` (default_skills.rs:64) and pinned by `builtin.insights-analyst`'s
`grounding_skills`, so a missing `docs/skills/insights/SKILL.md` would fail-close the persona and
break the build. **Created** the skill, mirroring `skills/query/SKILL.md`'s format: frontmatter
`name: insights` + a `description` that names every trigger phrase, then a body that doubles as the
`core.insights` grounding skill AND the operational walkthrough — authenticate → the verb table →
raise/dedup/list/ack/resolve → occurrences (the ring + the 2 KB cap + `oseq`) → subscriptions
(tag-facet / identity / rule / severity-floor + `throttle_override`) → the digest ladder +
breakthroughs + the kill switch → `insight.watch` SSE → the AI analyst persona → gotchas. Grounded
in the REAL verbs built (every payload mirrors the host integration test fixtures).

### Two root-cause bug fixes (this session found them driving the full suite)

1. **`core_skills_test.rs:308` stale assertion** — `the_default_set_is_the_read_only_core_skills_with_an_env_override`
   hardcoded `DEFAULT_CORE_SKILLS == &["core.lb-cli","core.query","core.store-read"]`, but the
   constant is now the 21-skill persona-grounding set (grown by the persona-grounding session,
   `core.insights` added with the insights-analyst persona). `cargo test --workspace` aborted at
   this binary, so none of the insights binaries ran. Fix: replaced the brittle whole-array
   equality with invariant + canonical-member spot-checks (non-empty, all `core.`, key members
   present, `core.secrets` absent) and renamed the test to `…persona_grounding_set…`. See
   `debugging/insights/core-skills-default-set-assertion-stale.md`.

2. **Insights UI bare `rounded`** — three `<code>` chips in `InsightDetail.tsx`/`InsightsList.tsx`
   used bare `rounded`, failing the radius-scale guard (`radius-scale.guard.test.ts`) and red-ing
   the whole UI unit suite. The radius bug shipped 2026-07-04 with a repo-wide `rounded`→`rounded-md`
   sweep; the insights feature folder was added after and missed the convention. Fix: the three
   chips → `rounded-md` (deliberate `rounded-full` pills untouched). See
   `debugging/insights/insights-ui-used-bare-rounded.md`.

### Doc deliverables (this session)

- `docs/sessions/insights/insights-session.md` — this file.
- `docs/public/insights/insights.md` — replaced the TODO stub with the shipped truth.
- `docs/debugging/insights/*.md` — FIVE entries (the two above + the three scaffold-era root
  causes: envelope-unwrap, oseq-collision, prefs-schema-column) + `debugging/README.md` rows.
- `docs/scope/insights/*` — open questions resolved/refreshed across all four scope docs.
- `docs/STATUS.md` — Insights moved from "Just scoped" to "Just shipped" with the real result.
- `docs/skills/insights/SKILL.md` — the build-blocker skill (above).

## What was already DONE and GREEN (prior sessions — recorded, not redone)

The code was written by prior sessions (scaffold + implement). This session did NOT touch the
insights verb bodies or the host wiring except for the two fixes above. Status as found at session
start:

- **`lb-insights` crate** — all core logic filled: `ladder_step` (pure state machine), `match_subs`,
  `raise` (dedup/re-open + occurrence append, takes `ring_cap`), `list` (takes
  `tag_allow: Option<&HashSet<String>>`), `occurrences` (flat direct query — capped rows are NOT
  data-wrapped), `ack`/`resolve`/`get`/`sub_*`/`policy_*`, `compute_due_digests`, `apply_intents`.
  New files: `table_scan.rs` (scan_all unwraps the data envelope), `notify_store.rs`,
  `notify_apply.rs`. `Occurrence.seq` serializes as `oseq` (capped_insert injects its own seq
  ULID). `NotifyState.last_sent_ts` is `Option<u64>`. `RaiseInput.producer` is `#[serde(default)]`.
  `RaiseOutcome` gained `dedup_key`/`severity`/`kind`.
- **Host wiring** — `insight_raise` threads `&Arc<Node>` (tags via `tags_add`, bus event
  `lb_bus::publish(&node.bus, ws, "insight/events", …)`, matcher→`apply_intents`→`deliver_to_sub`).
  New host files under `crates/host/src/insight/`: `notify.rs`, `reactor.rs`, `watch.rs`.
  `call_insight_tool` takes `&Arc<Node>`; `tool_call.rs` insight arm passes node. `insight.watch`
  in catalog + dev-login caps. `insight_sub_create` uses `authorize_channel` (wildcard-aware).
  `insight_list` resolves tag facets via `lb_tags::find`. Node boot spawns the digest reactor.
  `prefs/src/store/schema.rs` `PREFS_COLUMNS` + `DEFINE FIELD` include `insight_notifications`.
- **Gateway** — `/insights/events` SSE route + `insight_events` handler, mounted before
  `/insights/{id}`.
- **UI** — `useInsights` (paging/SSE/act), `insights.api.ts` (routes verbs through the mcp_call
  bridge), `insights.events.ts` (EventSource client), `insights.types.ts` (`oseq`, `InsightEvent`),
  page/list/detail/actions/facets components filled.
- **Persona** — `builtin.insights-analyst` in `personas.toml` (extends data-analyst, investigate
  verbs only, no raise, grounding `core.insights`). `core.insights` in `DEFAULT_CORE_SKILLS`.

### Scaffold punch-list — superseded

The `insights-scaffold-session.md` punch-list (slice-by-slice `todo!()` inventory) is **fully
superseded** by the implement session's work + this finishing session. The specific scaffold fixes
that the implement session made (recorded so the punch-list's "what's stubbed" rows are accounted
for):

- `RaiseInput.producer` → `#[serde(default)]` (the MCP door deserializes a caller body that omits
  it; the host overwrites it from the principal — a caller value is ignored).
- `Occurrence.seq` → `oseq` on the wire (`#[serde(rename = "oseq")]`) to dodge `capped_insert`'s
  injected `seq` ULID collision — see `debugging/insights/capped-insert-seq-collides-with-occurrence-seq.md`.
- `NotifyState.last_sent_ts` → `Option<u64>` (a fresh key has never sent).
- `scan_all` / `all_notify` unwrap the `data` envelope that `lb_store::scan` returns — see
  `debugging/insights/store-scan-returns-data-envelope-not-the-record.md`.
- `occurrences` reads via a flat direct query, NOT `store::list`/`scan_all` (capped rows are stored
  flat by `capped_insert`, not data-wrapped).
- `prefs/src/store/schema.rs` gained the `insight_notifications` column + `DEFINE FIELD` for both
  tables — see `debugging/insights/prefs-insight-notifications-axis-not-persisted.md`.
- `insight_sub_create` uses `authorize_channel` (wildcard-aware `bus:chan/*:pub` match).
- `insight.list` takes `tag_allow: Option<&HashSet<String>>` (the host pre-resolves tag facets via
  `lb_tags::find` and passes the id allowlist; the crate is tag-graph-agnostic).
- `call_insight_tool` → `&Arc<Node>` (raise needs the bus + tag graph + channel delivery).

## Decisions & alternatives

- **Invariant assertion over brittle equality** for the grown `DEFAULT_CORE_SKILLS`. Chose
  invariants + canonical-member spot-checks over hardcoding the new 21-element array because the set
  is documented to grow with the persona catalog. Rejected "hardcode 21" — it rots on the next
  persona. See `debugging/insights/core-skills-default-set-assertion-stale.md`.
- **`oseq` at the serde boundary, not a Rust rename.** Chose `#[serde(rename = "oseq")]` keeping
  the Rust field `seq` (the domain name is "sequence"). Rejected renaming the Rust field to `oseq`:
  the wire name is the only thing that needs to dodge the collision; the Rust API should stay
  readable.
- **`scan_all` unwraps; `occurrences` uses its own flat query.** Chose to centralize the envelope
  unwrap in `scan_all` (the `write`-based read path) and document the cap-rows exception, rather
  than route `occurrences` through it. Rejected a single universal scan helper: capped rows have a
  different storage shape (flat) and a different paging contract (`oseq` keyset), so one helper
  would need a branch; two named paths are honest.

## Tests

Mandatory categories (per `scope/testing/testing-scope.md` §2) are covered by the prior-session
test bodies, all of which this session drove green:

- **Capability deny (per verb):** `insights_test.rs` — raise/ack/occurrences/sub.create denied
  without their caps; `insight_routes_test.rs` — list/ack denied at the gateway; the read-only
  `core.skills` catalog deny.
- **Workspace isolation:** `list_in_one_workspace_never_returns_another_workspaces_insights`,
  `occurrences_never_leak_across_workspaces`, `cross_workspace_insight_is_opaque_to_the_other_ws`.
- **Scope-named cases:** dedup-lifecycle (bump + re-open), ring-cap eviction + lifetime count,
  2 KB-reject (no orphan row), matcher tag-axis, ladder cooldown (the 5-min-nag headline),
  digest idempotency, kill switch.
- **UI:** `InsightsPage.gateway.test.tsx` (4/4) against a real spawned gateway; radius-scale guard
  (the bare-`rounded` prevention).

### Green output

```
# Wasm guests (the node won't boot without them)
$ make build-wasm
→ building wasm guest: hello
    Finished `release` profile [optimized] target(s) in 0.08s
→ building wasm guest: hello-v2
    Finished `release` profile [optimized] target(s) in 0.05s
EXIT=0

# Workspace build
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.94s
BUILD_EXIT=0

# Per-binary test results (cargo test --workspace aborts at the pre-existing agent_routed_test,
# listed below as NOT mine — so the insights binaries are driven directly):
$ cargo test -p lb-insights --test ladder_test
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-host --test insights_test
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-role-gateway --test insight_routes_test
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-host --test core_skills_test
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# UI
$ pnpm test --run
 Test Files  103 passed (103)
      Tests  631 passed (631)

$ pnpm test:gateway src/features/insights/InsightsPage.gateway.test.tsx
 ✓ src/features/insights/InsightsPage.gateway.test.tsx (4 tests) 368ms
 Test Files  1 passed (1)
      Tests  4 passed (4)

$ pnpm tsc --noEmit
# 4 pre-existing errors, ALL in flows/FlowsCanvas.gateway.test.ts (3) and
# panel-builder/.../transformDebug.gateway.test.tsx (1) — ZERO insight errors.
```

**Pre-existing failures NOT mine (left untouched, per the brief):**
- `agent_routed_test::an_edge_invokes_the_hub_agent_over_the_routed_namespace` — model-routing
  panic at `agent_routed_test.rs:119` (the routed-namespace → hub-agent model resolution). Verified
  pre-existing: my one-line edit was to `core_skills_test.rs`; this binary doesn't touch insights.
  `cargo test --workspace` aborts here, which is why the insights binaries are driven directly above.
- The 4 `tsc` errors in flows/panel-builder (named by the brief).
- `SystemView.gateway`, `sqlSource.gateway` (named by the brief).

## Debugging

Five entries opened + closed this session, each with a regression test that's in the green output
above:

- `debugging/insights/store-scan-returns-data-envelope-not-the-record.md` — `scan` returns the
  data envelope, not the record. Regression: `insights_test::list_in_one_workspace…`.
- `debugging/insights/capped-insert-seq-collides-with-occurrence-seq.md` — `seq` collision → `oseq`.
  Regression: `insights_test::ring_cap_evicts_oldest_but_count_is_lifetime`.
- `debugging/insights/prefs-insight-notifications-axis-not-persisted.md` — SCHEMAFULL dropped the
  kill-switch axis. Regression: `insights_test::member_kill_switch_off_skips_all_deliveries`.
- `debugging/insights/core-skills-default-set-assertion-stale.md` — stale test aborted the suite.
  Regression: the renamed `the_default_set_is_the_persona_grounding_set_with_an_env_override`.
- `debugging/insights/insights-ui-used-bare-rounded.md` — bare `rounded` failed the radius guard.
  Regression: `radius-scale.guard.test.ts`.

All five rows added to `debugging/README.md` (newest first).

## Public / scope updates

- **Promoted:** `docs/public/insights/insights.md` replaced (TODO stub → shipped truth: the record,
  the verbs, the four layers, the test counts, the known follow-ups).
- **Scope open questions resolved/refreshed** across all four scope docs — see the follow-ups
  section below for the specifics (each scope doc's "Open questions" section is now current).

## Skill docs

- **Created** `docs/skills/insights/SKILL.md` — the build-blocker. The build (`assets/build.rs`)
  embeds every `docs/skills/<name>/SKILL.md`; `core.insights` is in `DEFAULT_CORE_SKILLS` and pinned
  by `builtin.insights-analyst`, so a missing SKILL.md would fail-close the persona and fail the
  build. The skill doubles as the `core.insights` grounding skill AND the operational walkthrough
  (raise/list/ack/resolve/occurrences + subscribe-to-a-tag-facet + the digest ladder + the kill
  switch), grounded in the real verbs built. Format mirrors `skills/query/SKILL.md`. Live-verified:
  the payloads mirror the host integration-test fixtures and the gateway routes the test gateway
  serves.

## Dead ends / surprises

- `cargo test --workspace` **aborts at the first failing binary**, not "runs everything and
  reports". The pre-existing `agent_routed_test` failure therefore masks every alphabetically-
  later binary unless the insights suites are driven directly (`-p lb-insights --test …` etc.).
  Worth remembering for any suite that lands late in the alphabet.
- The `core_skills_test` stale assertion looked like "my SKILL.md broke it" at first glance; it
  was actually the opposite — the SKILL.md was the missing piece for the *iteration* test
  (`creating_a_workspace_applies_the_default_core_skill_grants`), and the stale *equality* test
  was an unrelated land-mine the expanded `DEFAULT_CORE_SKILLS` had already armed. Two tests, one
  binary, opposite needs.
- `agent_persona_session_test.rs` is an untracked new file in the working tree (another session's
  in-flight persona work) — left untouched.

## Follow-ups

- **InsightDetail origin deep-link** — **known partial, recorded not silently dropped.** The detail
  drawer shows the origin (`{kind}:{ref}` + `run`), but the deep-link to the rule/flow/run route is
  NOT wired: the workspace id isn't threaded into the drawer, so the URL can't be built. The body
  evidence renderer is also still a JSON dump (the typed table/chart renderer is the dashboard-
  widget precedent follow-up). Tracked in `docs/public/insights/insights.md` and the umbrella scope.
- **Producer doors (umbrella scope §"Producers")** — the rhai cage `insight.raise(#{…})` handle and
  the built-in `insight` flow sink node are scaffolded/deferred. Today producers reach
  `insight.raise` via the MCP verb (agents/extensions/CLI/manual); the two structured doors land
  with a follow-up slice. Recorded in the public doc + the umbrella scope.
- **Scope open questions** — resolved/refreshed in each scope doc:
  - **Umbrella:** Q2 (severity closed v1 — confirmed), Q5 (auto-pin persona — still deferred to
    personas-catalog Q3), Q1/3/4/6 stay open with their current dispositions.
  - **Occurrences:** Q1 (workspace-only ring cap — confirmed for v1), Q2 (series pointer inside
    `data` — confirmed).
  - **Subscriptions:** Q1 (origin_ref exact-only v1 — confirmed), Q2 (team-owned subs — wait for
    demand), Q3 (inbox sink kind — deferred, channel-only v1).
  - **Notify:** Q1 (one cooldown v1 — confirmed), Q2 (quiet hours — deferred), Q3 (escalation
    counts deliveries-worth — confirmed), Q4 (AI-narrated digests — follow-up).
- **STATUS.md updated?** Yes — Insights moved from "Just scoped" to "Just shipped" with the real
  result (crate + host + gateway + UI, test counts).
