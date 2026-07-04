# Rules — the data stdlib (time / json / stats / Frame) — session

- Date: 2026-07-04
- Scope: ../scope/rules/data-stdlib-scope.md
- Stage: S8 — data plane. See STATUS.md.
- Status: **PHASE 0 (de-risk polars) — DONE, green, reporting at the ⏸ gate.** Phases 1–3 not started.

## Goal

The full data stdlib scope (`data-stdlib-scope.md`) adds ~180 functions to the one rhai cage in four
layers: a `time` clock handle (logical `now`), `json`/shape helpers, a scalar/array `stats` family,
and a polars-backed `Frame`. The scope sequences it in 4 phases with a **⏸ gate after Phase 0**: prove
polars builds clean through the zig-cc toolchain, measure the build-time + binary-size cost, wire it
behind a default-on `frames` feature, and STOP before writing any verbs. **This session is Phase 0
only.**

The whole Frame layer hinges on polars building + being tolerable weight here; if it doesn't, the
scope says "I'd rather rescope than hack around it."

## Phase 0 — what changed

- **`rust/crates/frame/` (new crate `lb-frame`)** — `Cargo.toml` (already authored in the scope) +
  `src/lib.rs`. The crate pins polars `=0.54.4` with a curated minimal feature set (security pin +
  zero-I/O posture). `src/lib.rs` is intentionally small for Phase 0: the `FrameLimits` type (the
  input-governor contract, defaults from the scope's open question), three boundary helpers
  (`frame_from_json` / `frame_to_json` / `frame_col_json` — the two halves of the catalog's
  `frame(records)`/`f.records()` + the `f.col("value")` bridge to `stats::*`), and the `any_value_to_json`
  normalizer. The folder-of-verbs split (`construct.rs`/`filter.rs`/`group.rs`/`window.rs`/`sql.rs`/
  `export.rs`/`limits.rs`) + the rhai `register(engine, &FrameLimits)` entry point are deferred to
  Phase 2 per the scope's sequence — not stubbed, not faked, just not written yet.
- **`rust/Cargo.toml`** — `crates/frame` added to workspace `members` + `lb-frame` workspace dep alias.
- **`rust/crates/rules/Cargo.toml`** — added a default-on **`frames`** cargo feature (severable: turn it
  off and `g.frame()`/`f.*` are absent; the pure verb families in Phases 1 are unaffected) +
  `lb-frame = { workspace = true, optional = true }` (the `dep:` form keeps the crate name distinct
  from the feature name).
- **`rust/crates/rules/src/lib.rs`** — a `#[cfg(feature = "frames")] pub use lb_frame as frame;`
  re-export. The single seam a future `verbs/frame.rs` reaches through (Phase 2). No logic.

## Phase 0 measurements (the deliverable)

**Build time (cold, polars compile — the question the gate exists to answer):**

| Measurement | Value |
|---|---|
| polars + full dep tree, first compile through zigcc | **60.6s wall** (221s user, 2.9 GB peak RSS) |
| `lb-frame` incremental (polars cached) | **0.7s** |
| `cargo build --workspace` (everything, polars cached) | **2m06s wall** (was ~21s for lb-rules alone) |
| Exit status | **0 (green)** |

polars **builds clean through the zig-cc linker** — no toolchain hacks. This is the green light the
whole Frame layer depends on.

**Binary / artifact size:**

| Artifact | Before | After | Δ |
|---|---|---|---|
| `target/debug/node` (debug binary) | 363 MB | 489 MB | **+126 MB (+34.6%)** |
| `target/` dir (debug, all crates) | 192 GB | 202 GB | +10 GB |
| release rlibs (polars+arrow, 27 files) | — | 459 MB | (intermediate, not final linked size) |

The debug `+126 MB` is the dev-environment number. **The deployment-relevant number needs a full
release `cargo build --workspace --release`** (release rlibs are 5–15× smaller than debug, but the
real delta is the *linked* `node` binary, which a release build of just `lb-frame` can't measure).
Flagged as a Phase 0 follow-up below — the gate's question ("is the weight unacceptable?") is
answerable on the debug number for now; release measurement lands before Phase 2 merges.

**Polars feature set (confirmed, security-audited):**

The curated list in `Cargo.toml` (`lazy`, `sql`, `rolling_window`, `pivot`, `strings`, `temporal`,
`dtype-full`, `json`, `describe`, `is_in`, `round_series`, `cum_agg`, `rank`, `diff`, `pct_change`,
`ewma`, `zip_with`) covers the entire Frame catalog. `default-features = false` is set.

## Phase 0 — the decisive finding (security): f.sql cannot reach I/O

The scope's **#1 f.sql risk**: *"assert polars' SQL context cannot reach registration of external
scans, and pin the polars version (a minor bump adding I/O functions to the SQL namespace would widen
the cage silently)."* This is the gate's other make-or-break question. Resolved at runtime, not just
by feature audit:

The `sql` + `lazy` features pull `polars-io` **with** `csv`, `cloud`, `http`, `ipc`, `object_store`,
`polars-parquet`, `reqwest` compiled in transitively. So the *crate code* is present. The real
question is whether the **SQL namespace** polars-sql exposes to a script can *reach* it. Probed
directly against the exact pinned feature set:

```
[safe] SELECT * FROM read_csv('/dev/null')     → 'read_csv' is not a supported table function
[safe] SELECT * FROM read_parquet('/etc/hostname') → 'read_parquet' is not a supported table function
```

`SQLContext` rejects `read_csv`/`read_parquet` at the **registry** — they are not registered as SQL
table functions in this configuration, so they never reach the filesystem. The SQL namespace only
knows the one table we register (`self`). The cage's zero-I/O posture holds at the runtime boundary
that actually matters. (See `tests::sql_cannot_read_csv_from_disk` /
`sql_cannot_read_parquet_from_disk` / `sql_cannot_reach_an_unregistered_table` in
`crates/frame/src/lib.rs` — these are real regression tests, kept for Phase 3's no-new-authority suite.)

The polars version is pinned exact (`=0.54.4`) per the scope's security-pin requirement — a minor
bump that *did* register `read_csv` in SQL would widen the cage, and these tests would fail loudly.

## Tests (Phase 0)

`cargo test -p lb-frame -p lb-rules` — **all green**:

- `lb-frame`: 8 tests — JSON↔Frame round-trip, column pluck, empty-frame, `FrameLimits` defaults,
  `sql_self_only_select_works` (the happy path: `SELECT series, avg(value) AS v FROM self GROUP BY
  series` returns the right groups), + the 3 security probes above.
- `lb-rules`: 15 messaging/cage tests unchanged (the wiring touched nothing in the existing surface).

```
running 8 tests
test tests::frame_limits_default_is_the_scope_value ... ok
test tests::empty_rows_give_empty_frame ... ok
test tests::col_plucks_a_flat_array ... ok
test tests::frame_round_trips_through_json ... ok
test tests::sql_cannot_read_csv_from_disk ... ok
test tests::sql_cannot_read_parquet_from_disk ... ok
test tests::sql_cannot_reach_an_unregistered_table ... ok
test tests::sql_self_only_select_works ... ok
test result: ok. 8 passed; 0 failed
```

## Decisions & alternatives

- **`default-features = false` + curated list, not `default-features = true` + denylist.** The scope
  named the audit direction; this realizes it. Rejected: turning default features on and trying to
  deny-list `csv`/`parquet`/`cloud` after the fact (fragile — every polars minor can add a default
  that re-leaks; allow-listing is the stable security posture).
- **NaN → null normalization deferred to Phase 2, not faked in Phase 0.** An initial Phase 0 draft
  tried to land it; the polars 0.54 `ChunkedArray::set` + `try_apply` composition didn't compile, and
  the scope puts the NaN/null policy under "Risks & hard problems" with its own fixture tests. Rather
  than ship a half-working path, it's deferred with an honest comment — the `any_value_to_json`
  normalizer (used by the column pluck) does handle NaN→null correctly today; the *eager* in-frame
  normalization lands in Phase 2.
- **The `frames` feature is default-ON.** The cage ships the full data stdlib; the feature exists to
  sever polars for a target that can't carry the weight, not to gate it day-to-day.

## Public / scope updates

- **Scope open question "Polars feature set + version pin" — RESOLVED:** exact feature list is the 17
  above (the curated `Cargo.toml` set); version pinned `=0.54.4`. Covers the catalog; `default-features
  = false`. The `sql`+`lazy` pull of `polars-io`'s csv/parquet/cloud crates is **transitive-only and
  runtime-unreachable** (proven by the security probe), so no further feature disabling is needed.
- **Scope open question "max_frame_rows default" — RESOLVED (recommendation taken):** `FrameLimits`
  defaults are `max_frame_rows: 200_000`, `max_frame_cells: 2_000_000`, `max_string_bytes: 256 KB`.
  Calibration against Playground dev hardware is a Phase 2/3 follow-up (the scope's `env::rules::*`
  wiring lands then).

## Dead ends / surprises

- **polars 0.54 API drift from common examples.** `DataFrame::into_lazy()` → `.lazy()`;
  `SQLContext::register` takes `LazyFrame`; `JsonWriter::finish` wants `&mut DataFrame`; `DataFrame`
  has no `is_empty()` (use `shape() == (0,0)`); `DataFrame::sort` takes `SortMultipleOptions` not
  `(cols, bool)`; `AnyValue` variants are `Copy` (no deref); `Column` has no `iter()` (go via
  `as_materialized_series()`). All resolved; noted here so Phase 2 doesn't re-discover them.
- **Debug rlib sizes are enormous** (polars_ops 198 MB, polars_core 485 MB) — this is debug debuginfo
  + monomorphized generics, not the deployment cost. Don't be alarmed; the linked `node` +126 MB is
  the real dev number.

## Follow-ups (out of Phase 0's scope, named so they're not lost)

- **Release `node` binary size delta** — run `cargo build --release --workspace` and measure the
  linked `node` before/after for the deployment-relevant number. Do this before Phase 2 merges.
- **Phases 1–3** as sequenced in the scope: pure verb families → Frame surface → tests + docs.

## Green output (Phase 0 gate)

```
$ cargo build --workspace        # 2m06s, exit 0
$ cargo test -p lb-frame -p lb-rules   # 8 + 15 tests, all ok
$ cargo fmt --check              # exit 0 (clean)
```

⏸ **Reporting at the Phase 0 gate. Awaiting go-ahead for Phase 1.**

---

## Post-Phase-0 add-ons (still pre-Phase-1)

Two asks after the gate: (1) an introspection API for every available cage function with
descriptions, and (2) split the now-too-big `frame/src/lib.rs`. Both done, green.

### Add-on 1 — `rules.help` + the function catalog (single source of truth)

- **`rust/crates/rules/src/catalog.rs`** (new) — `pub const CATALOG: &[FnEntry]`, one row per
  `register_fn` site across `verbs/*.rs`, each with `{name, family, signature, description}`. Built by
  reading every `register_fn` call (not the scope doc), so it can't drift from the code. 56 entries
  across 6 families (`data`/`grid`/`timeseries`/`emit`/`ai`/`messaging`). Hand-curated descriptions
  (the value-add over rhai's raw `gen_fn_signatures()`). The pure verb families (`time`/`json`/
  `stats`/`mathx`) append as they land in Phase 1 — the catalog is append-only.
- **`rust/crates/rules/src/lib.rs`** — `pub mod catalog; pub use catalog::{CATALOG, FnEntry};`.
- **`rust/crates/host/src/rules/mod.rs`** — new `rules.help` match arm in `call_rules_tool`: serializes
  `CATALOG` to `{ "functions": [{name,family,signature,description}, …] }`. Gated
  `mcp:rules.help:call` (the allowlist + cap machinery is generic; no per-verb host code).
- **Decision: curated catalog, not rhai `metadata`.** rhai's `gen_fn_signatures()` needs the
  `metadata` feature and returns raw signatures only — no family grouping, no human descriptions. The
  point of the ask was descriptions, so a curated catalog is the source of truth; the skill doc +
  `rules.help` + (future) UI autocomplete all read it.

### Add-on 2 — split `frame/src/lib.rs` (FILE-LAYOUT)

`lib.rs` (265 lines) conflated limits, the JSON↔Frame boundary, and the SQL security probe tests.
Split into folder-of-concerns:

```
crates/frame/src/
  lib.rs       ← barrel (26 lines): module doc + mod decls + pub use.
  limits.rs    ← FrameLimits + Default (34 lines, the governor contract).
  json.rs      ← frame_from_json / frame_to_json / frame_col_json / any_value_to_json
                 (75 lines, the JSON↔Frame boundary).
crates/frame/tests/
  json_test.rs         ← round-trip + col-pluck + empty-frame.
  limits_test.rs       ← FrameLimits default assertion.
  sql_security_test.rs ← the f.sql security probe (self-scan works; read_csv/read_parquet/
                         unregistered-table rejected). Moved to tests/ because the probe is about
                         polars-sql behavior — it builds its own SQLContext the way Phase 2's
                         f.sql verb will.
```

No behavior change — pure move + relocated imports. Every file well under FILE-LAYOUT's 400-line hard
limit (largest is 112 lines).

### Tests (add-ons)

- `catalog.rs` — 5 unit tests: names unique, names valid rhai paths, every field non-empty + sentence,
  families in the known set, every verb-module family present.
- `host/tests/rules_test.rs` — added `rules.help` to the `each_rules_verb_is_denied_without_its_cap`
  loop + a new `rules_help_returns_the_catalog` positive test (catalog non-empty, `source` entry has
  all 4 fields, every entry has all 4 fields non-empty).

```
$ cargo test -p lb-host --test rules_test
test each_rules_verb_is_denied_without_its_cap ... ok
test rules_help_returns_the_catalog ... ok
... 13 passed; 0 failed

$ cargo test -p lb-rules  # includes catalog unit tests
... all green
```

### Green output (add-ons)

```
$ cargo build --workspace        # exit 0
$ cargo test -p lb-rules -p lb-frame -p lb-host --test rules_test   # all green
$ cargo fmt --check              # exit 0
```

### Skill doc updated
`docs/skills/rules/SKILL.md` — `rules.help` added to the verb table + cap list + a note that the
catalog is the authoritative function list ("this paragraph is a map, the catalog is the territory").
