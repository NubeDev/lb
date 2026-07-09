# Insights — the rule producer door (`insight.raise`/`ack`/`close` from a rule body)

- Date: 2026-07-09
- Scope: ../../scope/insights/rule-raises-insight-scope.md
- Driven surface: ../../skills/rules/SKILL.md (new §7 "Raising an insight")
- Status: green — `lb-rules` + `lb-host` insight suites pass; full `cargo test --workspace` green
  save for one **pre-existing, unrelated** flaky (`role/cli` reminder gateway round-trip — untouched
  by this slice, a `reminder.create` cap deny in the reminders extension).

## Goal

Build the **rule producer door** the `insight/raise.rs` module doc already promised: a rhai handle so
a rule body can `insight.raise(#{…}) -> id`, `insight.ack(id)`, and `insight.close(id [, note])`
inline — over the **existing** `insight.raise`/`insight.ack`/`insight.resolve` host verbs. No new MCP
verb, no new capability. The parallel to the flow `insight` sink node, for a threshold rule whose whole
job is "notice a fault and record it" without the ceremony of a flow.

## The load-bearing verification (done first, per the scope's risk call-out)

> Does the slice-2 `route:false` flag reach the engine / `RunHandles` today, or does it only live in
> the host's post-run routing?

**Finding: it did NOT reach the cage.** `route` lived purely in the host's post-run routing —
`rules_run` (`host/src/rules/run.rs`) took `route: bool` and used it only at
`if route { route_alerts(...) }`, *after* `engine.run(...)`. The `RuleEngine`, `register(...)`, and
`RunHandles` never received it; `alert()` suppression was host-side (findings collected, then not
routed). So the `route:false` no-op for the insight handle was **not** a one-line handle field — it
needed host plumbing to thread `route` into the cage.

**Decision:** thread it minimally. `RuleEngine` gained a `route: bool` (default `true` via `new`, set
by a `with_route(route)` builder); `run.rs` calls `.with_route(route)`. `register(...)` gained
`route` + `origin_ref` params; the engine passes `self.route` + `rule.name` (the origin ref). The
`HostMessagingSeam` needed **no** change — it is a generic `call_tool(tool, …)` chokepoint that treats
`tool` as opaque data (rule 10) and already gates `insight.*` by `mcp:<tool>:call`. Confirmed by the
real-store deny tests below.

## What shipped

| File | Change |
|---|---|
| `rust/crates/rules/src/verbs/insight.rs` **(new)** | `InsightHandle` over `Arc<dyn MessagingSeam>` + `Arc<WriteMeter>` + `now` + `route` + `origin_ref` + `Arc<Collectors>`. `raise`/`ack`/`close`, each one `seam.call("insight.<verb>", …)`. Mirrors `verbs/channel.rs`: charge the meter AFTER validation AND AFTER the `route:false` short-circuit; inject `ts: now`; default `origin` to `{kind:"rule", ref:<rule id>}` when omitted; return `outcome.id` from `raise`. `close` maps to the `insight.resolve` verb (stated in the doc comment). On `route:false`: no-op, charge nothing, log an honest skip line, return an echoed id. |
| `rust/crates/rules/src/verbs/emit.rs` | Added `Collectors::log(level, msg)` — the honest skip line for a suppressed `route:false` call rides the same log collector `log(...)` writes to. |
| `rust/crates/rules/src/verbs/mod.rs` | `mod insight; pub use insight::InsightHandle;`; `insight::register(engine)`; `insight` field in `RunHandles`; construct + push the handle; `register` gained `route` + `origin_ref`; the "four scope handles" doc comment → five. |
| `rust/crates/rules/src/engine.rs` | `RuleEngine.route` (+ `with_route`); passes `self.route` + `rule.name` into `register`; pushes `insight` as a scope var. |
| `rust/crates/rules/src/catalog.rs` | `insight.raise`/`ack`/`close` rows under a **new `insight` family**; `"insight"` added to the `families_are_the_known_set` + `catalog_has_entries_from_every_verb_module` known sets (lock-step). |
| `rust/crates/host/src/rules/run.rs` | `RuleEngine::new(...).with_route(route)` — threads the run's `route` into the cage; doc comment updated to say `route` now also gates the insight handle. |
| `rust/crates/rules/tests/insight_test.rs` **(new)** | 12 handle-over-seam tests (the sanctioned `RecordingMessaging` boundary): dispatch + `ts` injection, origin default + author-supplied origin kept, missing-field author feedback, ack/close→resolve mapping, meter bound, `route:false` no-op + skip log, opaque deny (raise + close-after-raise), deterministic `ts`, catalog family present. |
| `rust/crates/rules/tests/support/mod.rs` | `RecordingMessaging` returns an echoed `id` for `insight.raise` (the handle returns it). |
| `rust/crates/host/tests/rules_test.rs` | 7 REAL end-to-end tests (real `mem://` store + real seam + real `insight.raise`/`ack`/`resolve` verbs, count `insight:*` before/after): happy-path real write (0→1, producer forced, fields + origin persisted), ack+close lifecycle (open→acked→resolved, un-spoofable actors, idempotent close), capability-deny (opaque, no partial write; + close-after-raise deny), workspace-isolation (independent records per ws; cross-ws close/read denied), deterministic dedup re-run (count==2 not two rows; re-open after close, count continues), `route:false` suppression (writes nothing, run still succeeds). |
| `docs/skills/rules/SKILL.md` | New §7 "Raising an insight" — the `raise`/`ack`/`close` surface, the emit/alert/insight boundary table + one-liner, the `route:false` panel no-op, and gotchas (`close`=`resolve`, re-open still charges, produce-only). |

## Testing (real store/engine/seam, mem://, no mocks — testing-scope §0)

- `cargo test -p lb-rules` → all green (12 `insight_test` + 15 `messaging_test` + 14 lib incl. the
  catalog integrity tests with `insight` added).
- `cargo test -p lb-host --test rules_test` → **21 passed** (7 new insight e2e + the pre-existing 14).
- `cargo fmt` + `cargo build --workspace` → clean.
- `cargo test --workspace` → green except one pre-existing unrelated flaky
  (`role/cli` `create_ls_show_update_rm_round_trips_over_the_real_gateway` — a `reminder.create` cap
  deny in the reminders extension, in files this slice never touched). The load-bearing design finding
  (the `route:false` flag not reaching the cage) is logged in
  `docs/debugging/rules/insight-handle-route-flag-not-in-cage.md`.

## Decisions worth remembering

- **`route:false` suppresses the whole call, not just the record.** An `insight.raise` is a *stronger*
  effect than `alert()` (durable record + notify fan-out). Dedup makes the *record* idempotent, not the
  count-bump / occurrence append / notify re-fire — so a panel repaint must skip the call entirely, or a
  dashboard viewed by ten people would inflate `count` purely from viewing. Verified end-to-end
  (`route_false_run_raises_no_insight`).
- **Origin defaults to the run's provenance.** The author may omit `origin`; the cage stamps
  `{kind:"rule", ref:<rule id>}` (the rule *is* the origin). `producer`/`acked_by`/`resolved_by` stay
  host-forced from the principal — un-spoofable even if put in the map.
- **`close` = `insight.resolve`.** Author-facing name maps to the existing verb/cap; stated loudly in the
  handle doc comment and the skill (a `resolve` grep won't find `close` otherwise).
- **A new `insight` family, not folded into `messaging`.** Insights are a distinct plane (state vs
  motion); the family forces the catalog integrity tests to be updated deliberately (the tripwire is a
  feature).

## Not built (deferred per the scope's non-goals / open questions)

- No read door in the cage (`insight.get`/`list`) — a rule produces, it doesn't browse. Cross-run
  auto-close (open→close-on-recovery) therefore belongs in a flow/job, not a single rule body (noted in
  the skill's worked example).
- Flow `insight` sink node `route` honoring (open question 2) — untouched; the two doors align on
  "read-only run" but that alignment is a follow-up.
