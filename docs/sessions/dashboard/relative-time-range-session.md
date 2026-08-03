# Session — the relative time-range grammar (host slice)

Date: 2026-08-03.

## Ask

Build the lb half of `docs/scope/dashboard/relative-time-range-scope.md`: one canonical relative
time-range grammar + resolver in the host (`today`, `this-month`, `last-3-months`, `now-4h`, …), a
typed validated `Dashboard.time` default window, the `time.range.resolve` host verb, the schedule
payload's `range` window, the committed conformance fixture the downstream TS
twin asserts, and the `lb-node` re-exports the embedder's report CLI needs (string-in/plain-out —
the downstream manifest carries no chrono/chrono-tz).

## What shipped

### The `timerange` module (`crates/host/src/timerange/`)

- `grammar.rs` — `parse` → `RangeExpr::{Endpoint, Window}`; the full decided grammar: `now`,
  `now±<n><unit>` (`s m h d w M y`, `m`=minute/`M`=month), Grafana snap `/<unit>`, ISO day/instant,
  13-digit epoch ms; range tokens `today`/`yesterday`/`tomorrow`, `this-`/`last-`/`next-<unit>`
  over hour…year, counted `last-<n>-<unit>s` + short `last-<n><unit>` (quarters normalize to 3
  months). Every refusal names the token AND the legal set (`TimeRangeError`, a real
  `std::error::Error`).
- `civil.rs` — the Hinnant `civil_from_days`/`days_from_civil` maths ported from rubix-ai's
  `src/report/preset.rs`, plus clamped `add_months` (31 Mar − 1M = 28 Feb), Monday `weekday`,
  Jan/Apr/Jul/Oct quarters.
- `resolve.rs` — `resolve(from, to?, now_ms, tz) -> ResolvedRange {from_ms, to_ms, from_day,
  to_day}` (`to` exclusive; range token + `to` = refused; endpoint `from` with no `to` ends at
  now) + `resolve_range(…, tz: &str)` (the string-tz embedder form; empty/"UTC" = UTC, unknown =
  `Err`) + `validate(from, to?)` (the save-time gate, fixed clock, UTC) + `parse_tz`.
  **DST offset split (the moment.js rule Grafana inherits, now also pinned by fixture):**
  `s`/`m`/`h` offsets are exact fixed-width ms; `d`/`w`/`M`/`y` are calendar-anchored and
  wall-clock preserving (`now-1d` across the Sydney spring-forward spans 23 real hours). Ambiguous
  local times (fall-back) take the earlier instant; nonexistent ones (spring-forward gap) shift
  +1h.
- ~~`legacy.rs`~~ — **built, then hard-DELETED the same session by decision** (see "Legacy compat
  dropped" below). There is no legacy preset vocabulary in lb at all.
- `tool.rs` — the `time.range.resolve` MCP verb (args `from`, `to?`, `tz?`, `now?` ms) →
  `{fromMs, toMs, fromIso, toIso}`, gated `mcp:time.range.resolve:call`; schema'd descriptor.

Wiring: `tool_call.rs` (`"time."` host-native prefix + dispatch arm), `system/catalog/timerange.rs`
(+ `mod.rs`), `tools/descriptor.rs` (palette schema), `authz/builtin_roles.rs` (the cap on the
**viewer** tier, beside `series.stats`), `lib.rs` (`pub mod timerange` + `call_timerange_tool`).

### `Dashboard.time` (the four layers, `width`'s shape, `kind`'s validation)

- `dashboard/model.rs` — typed `DashboardTime { from, to }`; `Dashboard.time:
  Option<DashboardTime>` (`skip_serializing_if` — a pre-time record round-trips byte-clean).
- `dashboard/save.rs` — `PageMeta.time` (preserve on `None`, set on validated `Some`, an all-empty
  pair CLEARS — the `reportIds` precedent); `check_time` validates through `timerange::validate`
  BEFORE any write, so a bad expression refuses the save and leaves the stored value untouched;
  descriptor schema entry.
- `dashboard/tool.rs` — `opt_time_arg` (absent/null preserves; a bare string reads as `from`).
- `role/gateway/src/routes/dashboard.rs` — `POST /dashboards` body field, inserted only when
  present (preserve-on-omit over the wire).
- `pack/apply.rs` — a pack page may declare `time` with the same keys/validation.

### Schedule payload (`range` — the only named-window form)

- `reminder/range.rs` — generic over the action (rule 10: keys, never target/tool names):
  `check_action_window` validates `{"range":{from,to?,tz?}}` — the ONE named-window form — in an
  `mcp-tool` action's `args` AND an `outbox` action's JSON `payload` at **save** time (create +
  update); a non-JSON payload stays opaque. `resolve_payload_window` resolves `range` at **fire**
  time against the fire clock (tz from `range.tz`, else UTC), injecting concrete `from`/`to` ISO
  days (the `range` stays for audit). A `preset` key is **REFUSED** on both paths via one shared
  `PRESET_REMOVED` message. Hooked in `reminder/create.rs`, `update.rs`, and `fire.rs` (both the
  `McpTool` and `Outbox` arms).

### The conformance fixture

- `docs/contracts/time-range-conformance.json` — **83 rows** of `{expr, to?, nowMs, tz, fromMs,
  toMs, fromIso, toIso}`: every token family, both `last-month` spellings, 29 Feb 2028, 31 Mar →
  `last-1-month`, 1 Jan → `last-month`, the Australia/Sydney 2026 spring-forward (4 Oct) and
  fall-back (5 Apr) including the DST-discriminating `now-1d`/`now-1w` (23h/167h and 25h/169h)
  and crossing `now-4h` (exactly 4h) rows, the Monday week, snaps, ISO/epoch endpoints.
- `crates/host/tests/timerange_conformance_test.rs` RE-GENERATES the table from the resolver and
  asserts byte-equality (drift = red test); regenerate deliberately with
  `UPDATE_CONFORMANCE=1 cargo test -p lb-host --test timerange_conformance_test`.

### `lb-node` re-exports (the embedder seam)

`rust/node/src/lib.rs`: `pub use lb_host::timerange;` (the whole module) plus flat
`resolve_range` / `ResolvedRange` / `TimeRangeError` — string-in/plain-out (tz as an IANA *name*;
ISO days as plain `String` fields; the error is `std::error::Error` so `anyhow` callers `?` it).
This is what lets rubix-ai delete `src/report/preset.rs` outright — its CLI's `--preset <id>` becomes
`--from <expression>` and gets the grammar's semantics.

## Tests (all green)

- Unit (lib): `timerange::civil` (4) + `reminder::range` (4, incl. the `preset`-refused pin);
  grammar/resolve unit tests
  live in sibling files per FILE-LAYOUT (the sources sit near the 400-line ceiling):
  `tests/timerange_grammar_test.rs` (3), `tests/timerange_resolve_test.rs` (10).
- `tests/timerange_test.rs` (2) — **capability-deny with a fresh subject + positive control**
  through the real caps wall over a booted `Node` via `call_tool`; malformed input naming the
  token / the legal set / the bad tz.
- `tests/timerange_conformance_test.rs` (2) — the fixture equality + the pinned-edge coverage
  (23h/25h Sydney days, offset-split widths, Monday week, ≥60 rows).
- `tests/dashboard_time_test.rs` (1) — round-trip, preserve-on-omit across a layout save, loud
  refusal leaving the stored value untouched, explicit clear, byte-clean additivity.
- `tests/dashboard_test.rs::workspace_isolation` — **extended over `Dashboard.time`** (ws-B
  cannot read ws-A's window; a same-id ws-B record never aliases it).
- `tests/reminder_range_test.rs` (1) — save-time validation of `range` through the real MCP bridge
  (create + update, mcp-tool + outbox forms), the `preset` refusal in both carriers, refused saves
  storing/changing nothing.
- Full `lb-host` lib suite: 414 passed; touched suites (`dashboard_test`, `dashboard_kind_test`,
  `reminders_mcp/fire/reactor`, `catalog_mcp_test`, `builtin_role_upgrade_test`, `lb-authz`) all
  green. `cargo fmt --all --check` clean.
- Pre-existing baseline reds NOT touched: `lb-node boot_wiring_test` (stale `reactors::spawn`
  arity) and `lb-role-gateway publish_install_test` (needs the hello-v2 wasm artifact built).

## Legacy compat DROPPED — by decision, not oversight

The seven pre-grammar report preset ids (`yesterday`, `last-24-hours`, `last-7-days`, `last-30-days`,
`last-90-days`, `this-month`, `last-month`) were implemented as a byte-for-byte compat layer earlier in
this same session (`timerange/legacy.rs` + compat tests + `lb-node` re-exports), then **hard-deleted**
on the user's ruling: **there is no production deployment**, so there is no scheduled report whose
window could shift on upgrade — the only thing the layer bought was a second window vocabulary to keep
in sync forever. It is not deprecated, not aliased, not preserved anywhere.

What that changed:

- Deleted `crates/host/src/timerange/legacy.rs` (and its `mod`/`pub use` lines, the module doc bullet,
  and the now-unused `TimeRangeError::UnknownPreset` variant).
- Deleted the `resolve_legacy_preset` / `LEGACY_PRESETS` re-exports from `rust/node/src/lib.rs`.
- `reminder/range.rs`: `range: {from, to?, tz?}` is the ONLY accepted named-window form. A `preset`
  key is now **refused loudly** — at SAVE time (create + update) and at FIRE time (so a row written
  before the removal fails visibly instead of quietly mailing a fallback window every night), in both
  the `mcp-tool` `args` and `outbox` `payload` carriers. The message:
  `"preset" is no longer supported — use range: {from: "..."} (a range expression such as last-month,
  last-7-days, today or now-6h)`.
  Refusing beats ignoring: an ignored `preset` leaves a reminder that LOOKS configured.
- Conformance fixture: **unchanged, still 83 rows**. No row ever pinned a legacy preset id — the
  legacy layer mapped ids onto ordinary grammar expressions, and the fixture only ever contained
  grammar rows. Regenerating with `UPDATE_CONFORMANCE=1` produced a byte-identical file.

Note the two vocabularies genuinely disagreed (legacy `this-month` ended after *today*; the grammar's
token ends at the first of next month), which is exactly why keeping both would have been a standing
trap rather than a kindness.

## Notes / decisions made inside the scope's frame

- Snap `/<unit>` **floors** uniformly (weeks to Monday); the fixture pins it.
- An endpoint `from` with `to` absent ends at `now`; an inverted pair is a loud refusal.
- ISO-day projection: `from_day` = the local day containing `from_ms`; `to_day` exclusive (a
  midnight `to` names its own day, anything later rounds up).
- The `time.range.resolve` clock is caller-injectable (`now` ms); absent → wall-clock (the
  `series.retention.*` posture) — the binary-boundary env rule is untouched.

## Next

- rubix-ai (after the `node-v*` tag + pin bump): vendor the fixture into the vitest twin, switch
  the report CLI's `--preset <id>` to `--from <expression>` over `lb_node::resolve_range`, DELETE
  `src/report/preset.rs` (no compat shim — see below), ship the picker/URL contract per the consumer
  scope. Any stored reminder payload still carrying a `preset` key must be re-authored with
  `range`; the host now refuses it at save and at fire rather than firing a fallback window.
