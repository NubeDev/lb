# Session — widget-builder E2E hardening (live "add a widget" now works end to end)

**Date:** 2026-07-05 · **Branch:** `insights-v1` · **Area:** agent loop wire shape / delegation
ownership / federation engine + supervision / dashboard MCP ergonomics
**Continues:** [persona-menu-full-catalog-session.md](persona-menu-full-catalog-session.md)
(rounds 1–4 there; this session is the live hardening that followed).

## The ask

The live loop on `/dashboards` — `builtin.widget-builder` persona over Z.AI GLM-4.6 (in-house
runtime): *"can you access datasources … add a widget for avg meter usage"*. Each round was driven
against the real dev node (`POST /agent/invoke` headless + the user's dock), root-caused, fixed,
regression-tested, and re-verified live.

## What was found and fixed (in order)

1. **Blind identical retries — orphan tool messages.** GLM repeated the same rejected call 3–5
   turns. The OpenAI-compat adapter fed results back as orphan `role:"tool"` messages (no assistant
   `tool_calls` echo). Measured live with three wire shapes: only the conformant echo+keyed-result
   shape keeps the model anchored. `CallOutcome`/`ToolResult` now carry name+input end to end; the
   adapter emits the conformant shape. →
   [debugging/agent/tool-errors-ignored-orphan-tool-messages.md](../../debugging/agent/tool-errors-ignored-orphan-tool-messages.md)
2. **`information_schema` answered read-only.** Steering rejections didn't stop the model's
   strongest prior; the sidecar now synthesizes `information_schema.tables`/`columns` from the
   source's real catalog per query (new `extensions/federation/src/info_schema.rs`); host gate lets
   the two views through; `pg_catalog` still steers. →
   [debugging/federation/information-schema-now-answered-read-only.md](../../debugging/federation/information-schema-now-answered-read-only.md)
3. **Federation went dark after a few bad queries.** `native/call.rs` conflated an error REPLY
   (`SupervisorError::Child`) with a dead child and burned the 5-restart budget on failed SQL. Only
   `Transport` faults now restart; the sidecar also fences each call in its own task (a panic
   becomes an error reply); bare `COUNT(*)`'s upstream DataFusion internal error is rewritten to a
   steer (`COUNT(id)`). →
   [debugging/federation/error-reply-treated-as-crash-restart-budget-exhausted.md](../../debugging/federation/error-reply-treated-as-crash-restart-budget-exhausted.md)
4. **The saved dashboard was invisible to the user.** `dashboard.save` under the derived
   `agent:session` principal stamped the agent as owner (private ⇒ invisible to everyone).
   `Principal` now records the delegation root (`delegator`); `owner_sub()` = delegator-or-sub;
   dashboard + panel ownership/visibility walls read it. →
   [debugging/agent/agent-created-dashboard-invisible-derived-owner.md](../../debugging/agent/agent-created-dashboard-invisible-derived-owner.md)
5. **`dashboard.save`/`share` arg ergonomics.** Stringified `cells`, missing `widget_type`,
   stringly `now` — real descriptors for save/share, `typed_arg`/`u64_arg` leniency (validators
   still authoritative), `Cell.widget_type` serde-defaulted. →
   [debugging/agent/dashboard-save-cells-sent-as-json-string.md](../../debugging/agent/dashboard-save-cells-sent-as-json-string.md)
6. **"It made a whole new dashboard"** — the dock DOES send the open dashboard
   (page context `search.d`), the persona just wasn't taught the convention. The widget-builder
   identity now says: work on the open dashboard (`dashboard.get` → append → `dashboard.save`,
   keep existing cells); create a new one only when asked or none is open; say which you touched.
   (Answer to the genui question: yes — `dashboard.catalog` embeds `genui_catalog.json`'s component
   list and all 8 grounding skills incl. `core.genui-widget` are injected; verified in the live
   run-start event.)

## Final live verification

Headless `POST /agent/invoke` with the user's exact goal + page context `{d:"keep-dash"}`:
the run discovered the source, wrote a working SQL over `point_reading`, **appended** an
`avg-meter-usage` timeseries cell to `keep-dash` (existing cells preserved, owner `user:ada`), and
`viz.query` over the saved cell's source returns real frames. Sidecar survives 8 consecutive failed
queries; `COUNT(*)` steers; `information_schema.tables` answers.

## Test status

Green: `lb-auth`, `lb-host --lib` (124), `openai_compat_test` (+1 body-shape test),
federation ext units (9), `federation_test`, `native_test`, `dashboard_test`,
`dashboard_genui_test`, `agent_test`, `agent_persona_test` (21), `agent_persona_session_test`,
`agent_answer_fallback_test`, `agent_rehydrate_test`, `agent_skill_test`, `agent_decision_test`,
`agent_memory_test`, `agent_offline_test`, `agent_in_house_wiring_test`.
Pre-existing failures NOT from this session (fail on committed HEAD too): `panel_test` (4 ref-cell
tests), `agent_routed_test`.

## Round 2 (same day): prove-the-query rule + null-tolerant cells

From the next live transcript: the run recovered well but still saved on the first try only after
two `cells: invalid type: null, expected a string` failures, and nothing required the query to be
proven before the save.

- **Prove before save:** the widget-builder persona identity + a new required-workflow section in
  `docs/skills/dashboard-widgets/SKILL.md` now demand the exact `{tool, args}` be proven (non-empty
  frames) before `dashboard.save`; an empty result means fix the query or tell the user, never save
  a dead tile.
- **Null tolerance:** every defaulted field in the dashboard cell model
  (`dashboard/model.rs`) now deserializes an explicit JSON `null` as its default
  (`deserialize_with = "null_default"`) — `#[serde(default)]` alone only covers an ABSENT key, and
  models emit `"title": null` constantly. Regression: `cell_tolerates_explicit_nulls`.

**Verified live:** the next run had ZERO tool errors — `dashboard.get` → `federation.schema` → four
successful query probes → one `dashboard.save` appending a proven `bargauge` cell to `keep-dash`.

## Open follow-ups (recorded, not blocking)

- Extend `owner_sub()` adoption beyond dashboard/panel (assets docs, secrets, nav) — same
  delegation principle, per-verb review.
- Orphaned dev-store record: dashboard `meter-usage` (owner `agent:session`, pre-fix) — invisible
  junk; purge with the dev store when convenient.
- `time_bucket` (Timescale) doesn't exist in DataFusion — the model self-corrects to `DATE_TRUNC`,
  but a steer would save a turn.
- Descriptors for the remaining agent-critical verbs (`viz.query`, `store.query`, `query.run`).
- Per-workspace/persona `MAX_STEPS` ceiling (raised globally 8 → 16 earlier today).
