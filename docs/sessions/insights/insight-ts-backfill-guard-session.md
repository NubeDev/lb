# Session — low-level `insight.ts` backfill guard

## Ask

The Insights page showed `first 21/01/1970, 23:27:10` / `last seen … 20623d ago` on an
insight raised interactively "from the rules" workbench. Add a **low-level guard**: if no
timestamp is supplied on a raise, assume it's missing and stamp the wall-clock.

## Root cause

`InsightsList.timeAgo` (`Date.now() - ts`) and `InsightDetail` (`new Date(ts)`) both read
`insight.ts` as **epoch milliseconds**. A producer door that reached the crate-level
`lb_insights::raise` with `ts == 0` (or omitted) landed the record at the Unix epoch →
`new Date(0-ish)` = 1970, and `Date.now()_ms − ~0` ≈ 20623 days.

The `rules.*` MCP bridge (`crates/host/src/rules/mod.rs`) already backfilled `now_ms()` when
a caller omits `ts`, but that guard lived only at that one door — the crate `RaiseInput.ts`
was a **required, un-defaulted** field and any other path handing `0` sailed through to 1970.

## Change (low-level guard, one funnel for every producer door)

- `crates/insights/src/raise.rs` — `RaiseInput.ts` now `#[serde(default)]` (→ `0`) so a door
  may omit it entirely; the crate stays wall-clock-free (testing §3).
- `crates/host/src/insight/raise.rs` — `insight_raise` (the single host funnel every producer
  door reaches: rule handle, flow sink, agent, CLI) now backfills `now_ms()` when `input.ts == 0`.
  A real non-zero `ts` still wins → deterministic callers (flows, tests) stay reproducible.
  Added a local `now_ms()` helper (mirrors `insight::reactor`'s `as_millis`).

## Tests (green)

- New regression `raise_without_ts_backfills_wall_clock_not_epoch` (insights_test.rs): raise
  omitting `ts`, assert `first_ts` is real epoch-millis (`> 1.6e12`), not 0.
- `cargo test -p lb-host --test insights_test --test rules_test` — all green (incl. the
  pre-existing `interactive_rules_run_without_ts_stamps_a_real_clock` and every determinism/
  dedup case that passes an explicit non-zero `ts`).

## Note

The fix is forward-looking. The already-broken record in the dev store keeps its epoch `ts`
(it was written before the guard). Re-raising it (a re-run of the rule) restamps it via dedup's
`last_ts` bump; a full purge of the dev store clears the stale row.

## CORRECTION (the real bug — the `== 0` guard was insufficient)

The 1970 records were NOT `ts == 0`. The gateway `POST /rules/run` route
([routes/rules.rs](../../../rust/role/gateway/src/routes/rules.rs)) injects `gw.now()` as `ts`
when the interactive caller omits it — and **`gw.now()` returns epoch SECONDS** (`as_secs()` in
`state.rs`). `insight.ts` is defined as epoch MILLISECONDS (UI `new Date(ts)`, `insight::reactor`
`as_millis`). So a real seconds value (~1.78e9) flowed through, `new Date(1.78e9)` = Jan 1970,
`Date.now() - 1.78e9` = "20623d ago". Confirmed live: the stored `first_ts` was `1783632013`.

**Real fix (3 parts):**
1. `insight_raise` now calls `normalize_ts(input.ts)`: `0` → wall-clock millis; a value in the
   epoch-seconds band `[1e9, 1e12)` → ×1000; a real millis clock (≥1e12) or a tiny test clock
   (<1e9) passes through. This heals every producer door at the one write funnel.
2. `heal_ts.rs` — a one-shot, idempotent **boot migration** (`heal_insight_timestamps`, called in
   `node/src/main.rs` boot) that rewrites on-disk seconds-band `ts` ×1000. Fixes records already
   written, on the next restart the user does anyway. Idempotent (millis are out of the band).
   Note: insight PARENTS store fields under a `data` envelope (`data.first_ts`/`data.last_ts`);
   occurrence rows are FLAT (`ts`). `UPDATE … RETURN VALUE <col>` (not `RETURN AFTER` — that
   includes the record `id`, a Surreal thing that won't deserialize into JSON).
3. Tests: `raise_with_epoch_seconds_ts_is_normalized_to_millis`,
   `heal_rewrites_seconds_band_ts_to_millis_and_is_idempotent`.
