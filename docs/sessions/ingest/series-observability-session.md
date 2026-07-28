# Session — series observability (`series.stats` + `series.retention.status`)

Status: **code + tests complete, green.** Unreleased — no tag cut in this session (the downstream
product session drives the release; see "Handover").

Scope: [`docs/scope/ingest/series-observability-scope.md`](../../scope/ingest/series-observability-scope.md)
Downstream consumer: `NubeIO/rubix-ai` → `docs/scope/ingest/ingest-observability-scope.md` (slice C,
the Ingest health panel), built in the same session against a local `[patch]`.

## The ask, restated

Retention shipped a mechanism, a GC pass, and a reactor that ticks it — and no way to read back what
happened. `run_gc` returned a pass summary that `retention_reactor.rs` logged and dropped, so the
entire observable surface of the subsystem was `eprintln!` on the node's stdout. This session adds
the two read verbs that close that gap, and persists the pass the reactor was already computing.

## What was built

| File | Lines | Job |
|---|---|---|
| `crates/ingest/src/stats.rs` | 174 | `series_stats` — raw count, per-tier rollup rows, wall-clock extent, producer set |
| `crates/ingest/src/pass_record.rs` | 118 | `series_gc_pass` table: `record_pass` / `last_pass`, one upserted row per ws |
| `crates/host/src/ingest/stats.rs` | 33 | `series.stats` verb — gated `mcp:series.stats:call` |
| `crates/host/src/ingest/retention_status.rs` | 74 | `series.retention.status` verb — gated `mcp:series.retention.status:call` |

Edited: `gc.rs` (records the pass), `ingest/lib.rs` + `host/ingest/mod.rs` + `host/lib.rs` (exports),
`host/ingest/tool.rs` (two dispatch arms), `system/catalog.rs` (two `HOST_TOOLS` rows),
`authz/builtin_roles.rs` + `apikey/roles.rs` (capabilities), `store/reserved.rs` +
`packs/validate.rs` (the new reserved table).

## Decisions made while building

All recorded in the scope doc's **Decisions** section (it shipped with no open questions). The three
that shaped the code most:

1. **Raw vs rolled-up needed no new mechanism** — they are already two tables (`series`,
   `series_rollup`). The scope's contingency (add a rollup marker, or drop the split from release 1)
   was not needed. The real subtlety was the opposite of the one anticipated: a rollup row exists
   once *per tier*, so a naive total double-counts a multi-tier policy. Hence `tiers: [{width_ms,
   rows}]` alongside the total, and a doc comment saying why.

2. **`run_gc` writes the record, not the reactor.** Both the periodic reactor and the on-demand
   `series.retention.gc` verb go through `run_gc`, so recording there is the only way both paths
   land in one place. Recording in the reactor would have let a manual GC leave the status stale —
   a status that lies is worse than no status.

3. **The write is unconditional.** An idle pass stamps `last_run_ms`. This is the one behaviour in
   the feature that is easy to implement backwards, so it carries both a comment at the call site
   and a named revert-checked test (below).

## Testing

Real `Store::memory()`, real samples through `write` + `commit_batch`, real `run_gc`, real stored
rows read back. No mocks, no fixtures (rule 9).

| File | Tests | Covers |
|---|---|---|
| `crates/ingest/tests/series_stats_test.rs` | 4 | counts/extent/multi-producer; unknown series → valid zero, not an error; per-tier rollup sums to the total; ws isolation |
| `crates/ingest/tests/series_gc_pass_test.rs` | 6 | record written; second pass **overwrites** (table holds exactly 1 row); **idle pass still stamps**; warnings clipping + `warnings_total`; ws isolation |
| `crates/host/tests/series_observability_host_test.rs` | 5 | capability-deny **both directions**; deny ≠ empty success; `matched_prefix` longest-prefix + no-match; bare prefix as subject; on-demand GC and status share one record; ws isolation over MCP |

### The revert-check (the highest-value assertion)

The scope demanded proof that making the record write conditional turns a test red. `record_pass` in
`gc.rs` was temporarily wrapped in `if pass.evicted_raw > 0 { … }` and the single test re-run:

```
running 1 test
test idle_pass_still_stamps_last_run_ms ... FAILED

---- idle_pass_still_stamps_last_run_ms stdout ----
thread 'idle_pass_still_stamps_last_run_ms' panicked at crates/ingest/tests/series_gc_pass_test.rs:151:10:
an idle pass IS a pass — a frozen last_run_ms reads as a dead reactor

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out
```

`gc.rs` was then restored. The guarantee is genuinely load-bearing, not incidentally true.

### Green output

```
     Running tests/series_gc_pass_test.rs
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
     Running tests/series_stats_test.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s
     Running tests/series_observability_host_test.rs
test result: ok. 5 passed; 0 failed
```

Plus the full `lb-ingest` suite (15 targets) and every host target touching the changed
dispatch/caps/catalog surfaces (`series_plane_host`, `catalog_mcp`, `tools_catalog`,
`builtin_role_upgrade`, `authz_mcp_dispatch`, `persona_menu_full_catalog`, the ingest and series
suites) — all `ok`, 0 failed. `cargo fmt --all --check` clean.

## Notes for whoever picks this up

- **A pre-existing drift found in passing, NOT fixed here:** `series_latest` is absent from both
  `lb_store::RESERVED_TABLES` and `lb_packs::RESERVED_CORE_TABLES`, so a pack can currently write the
  latest-sample pointer table. `series_gc_pass` was added to both, so this session did not repeat the
  mistake. Worth its own issue — it is a wall gap, not an observability one.
- **`GcPass` is `Serialize`-only**; the persisted `GcPassRecord` is a separate type that also derives
  `Deserialize`. Don't merge them without checking every `json!(pass)` call site.
- **`list_policies` projects columns explicitly** (`retention.rs`) — the closed-struct trap. Any new
  `Policy` field must be added there too or it silently reads as absent.

## Handover — release

Slices A + B land as one lb PR → a `node-v*` tag → rubix-ai bumps its pin (`WORKFLOW-LB.md` §4a).
Nothing here is released yet: **the git tree was deliberately left dirty and untagged** (the
requesting session owns all git). The rubix-ai side is developed against the local `[patch]` in that
repo's git-ignored `.cargo/config.toml`, which must be dropped as step 3 of the release, never as a
tidy-up.

---

# Session addendum — `series.producer.health` (slice D), 2026-07-27

Status: **BUILT, green, and proven against a real published extension.** Released in `node-v0.12.0`.

The first pass deferred slice D because nothing would exercise it, and said "revisit when a producer
extension is in scope in the same session". That condition was met, so it was built.

## The false premise that had been written down

The handover recorded the seam as: *`ext.list` already returns `tools: Vec<String>` per installed
extension (`host/src/system/model.rs`)*. **That is not true and never was.** `ExtRow`
(`host/src/ext/row.rs`) carries `ext`/`version`/`tier`/`enabled`/`running`/`health`/`restart_count`/
`ui`/`widgets` and no tool list; the manifest's `tools` is never persisted onto `Install`; and
`system/model.rs` holds the `system.*` observability shapes, not `ExtRow` at all. Anyone starting
from that sentence would have built on a mechanism that does not exist.

The conclusion survived the correction — a tool-name convention is still right — but the mechanism
changed to `node.registry.descriptor_entries()`, mirroring `agent::exfil::tainted_tools`. Recorded in
the scope as Decision 14 so it is not re-derived a third time.

## What was built

| File | Job |
|---|---|
| `crates/host/src/ingest/producer_health.rs` | the verb: discover by convention, map producer→ext by grammar, fan out at depth+1 |
| `crates/host/src/ingest/write.rs` (edited) | `producer_root` / `producer_leaf` / `producer_ext_id` — the INVERSE of `root_producer`, in the same file |
| `crates/ingest/src/stats.rs` (edited) | `series_producers` made public — the producer list without dragging three `count()`s along |
| `crates/host/src/tool_call.rs` (edited) | one dispatch arm; it lives here, not in `call_ingest_tool`, because it needs the registry and the depth |

Plus the usual six touchpoints: `ingest/mod.rs`, `host/lib.rs`, `system/catalog.rs`,
`authz/builtin_roles.rs` (VIEWER tier), `apikey/roles.rs`.

**No `lb-ext-sdk` change**, which was checked before designing rather than assumed — an extension
contributes by declaring one ordinary tool, so there is no coordinated tag across every consumer.

## Testing

`crates/host/tests/series_producer_health_test.rs` — **13 tests, all green.** Real `Node::boot()`,
real store, real samples through `lb_ingest::write` + `drain_workspace`, the real registry, and the
real `call_tool` chokepoint. The reporting extension is a real `LocalDispatch` — the same trait a
wasm instance and a native sidecar implement, and the same one `routed_host_entry_test.rs` uses.

```
running 13 tests
test a_series_with_no_samples_is_an_empty_list_not_an_error ... ok
test a_missing_report_field_stays_absent_and_is_never_defaulted_to_zero ... ok
test the_producer_is_handed_its_own_stream_id_not_the_rooted_form ... ok
test a_refusal_names_the_missing_grant_and_never_looks_like_silence ... ok
test a_reply_in_a_shape_we_cannot_read_is_an_error_not_a_plausible_blank ... ok
test without_the_verb_cap_the_whole_read_is_refused_opaquely ... ok
test an_extension_that_declares_no_health_tool_reports_nothing_and_is_not_an_error ... ok
test a_principal_cannot_read_producer_health_across_the_workspace_wall ... ok
test holding_the_verb_cap_grants_no_reach_into_an_extension_it_could_not_call ... ok
test one_broken_producer_does_not_blank_the_healthy_one_beside_it ... ok
test a_producer_in_another_workspace_is_never_reported ... ok
test a_declaring_extension_is_asked_and_its_report_is_carried_verbatim ... ok
test a_producer_that_is_not_an_extension_says_so_and_nothing_is_wrong ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
```

Plus 7 grammar unit tests in `ingest::write::tests` (`the_reader_inverts_the_writer` is the
load-bearing one — if the reader and writer disagree the strip attributes a stream to the wrong
extension, or to none).

Mandatory categories: **capability-deny in BOTH directions** (the outer verb gate, and the inner
per-extension gate — the latter asserted as a privilege-escalation question, with a permitted and a
forbidden extension in the same response), **workspace isolation** (twice: a foreign producer never
appears, and a cross-ws read is refused — the registry is node-wide, so the wall has to come from the
SAMPLES, which is exactly why that test matters).

## What only the live run could prove

See the rubix-ai session doc for the full stack. The short version: `modbus` was built, signed,
published to a scratch node (HTTP 204) and left polling a real Modbus simulator; the host recorded
the producer `ext:modbus/modbus.sim-net@1000004`, recovered `modbus` from it by grammar, discovered
`ingest.health` by convention and called it. That is the rule-9 proof the first pass could not get.

## Notes for whoever picks this up

- **A gap I found and did NOT close, deliberately.** `ws_drain_lock`
  (`crates/host/src/ingest/drain_lock.rs`, added in `94a8b789`) has **no test coverage**: a grep for
  `ws_drain_lock` finds only its declaration, its two use-sites in `drain.rs`, and a comment in
  `ingest_conflict_storm_test.rs` explaining that the test *mirrors* it with a locally-declared
  mutex because the real lock lives in `lb_host` and cannot be imported into `lb-ingest`. **Deleting
  the lock acquisition in `drain_at_most` would break no test** — the same boot-wiring blind spot
  that issue #108 was created to close, reintroduced two commits later. It is another session's
  in-flight slice and the debugging doc says WS-B is load-bearing (WS-A's 16-retry bound was observed
  to exhaust), so it is named here rather than quietly patched. It is the first thing I would fix
  next in this area.
- **A live/durable producer mismatch, also not fixed here.** `tool_call.rs`'s motion publish and
  `role/gateway/src/routes/ingest.rs` both stamp `sample.producer = principal.sub()` — the BARE sub —
  while `ingest_write` commits `root_producer(sub, declared)`. So a sample committed as
  `ext:modbus/modbus.sim-net@1000004` is published on the bus as `ext:modbus`. Both call sites carry
  a comment claiming they match what `ingest_write` commits; they do not. Nothing in this slice reads
  the bus, so it is out of scope here, but anything correlating the SSE feed's producer with
  `series.stats().producers` will mismatch — and a future producer strip driven off live motion
  would silently attribute every stream to the extension root.

## Released — `node-v0.12.0` (2026-07-28)

Tagged from master `cce4f404` and pushed. The tag carries this slice plus three work-streams that
were already committed-but-untagged since `node-v0.11.0` (series-normalize; the #108 testing
hardening and its pre-cap policy-row deserialization fix; the ingest-conflict-storm retry primitive
and per-ws drain lock), and lb #110 (`charts:`) merged into the same release.

Tagging master was chosen over a cherry-picked release branch: the streams are interleaved across
`gc.rs`, `ingest/tool.rs` and `ingest/lib.rs`, so a pick would fork history to re-resolve conflicts
for no gain, and each stream's session doc already marks it done.

Downstream: `NubeIO/rubix-ai` bumped its `lb-node` pin to this tag and dropped **two** `[patch]`
stanzas — the expected machine-local one and a **committed** one in its `Cargo.toml` that
`WORKFLOW-LB.md` §5 forbids and that overrode the pin regardless.

Verification is recorded in full in the rubix-ai session doc. The short version: lb's **recorded
36-test baseline** means green was never available, so all 29 observed failures were attributed
individually — 9 missing fixture binaries (green once built), 18 baseline, 2 proved pre-existing in a
worktree at `94a8b789`/`15f157f7`. **None attributable to this work.**

One correction worth carrying forward: `flows_plc_reliability::concurrent_same_run_id` looked like a
regression introduced by the conflict-storm commit on a single run per side, and is in fact **flaky**
(base 3/3 fail, HEAD 2/3 pass) — consistent with this repo's own note that the 16-retry bound "was
observed to exhaust, flakily". Worth a stabilisation pass; it will read as a regression to the next
person who diffs it with one run.

---

## 2026-07-28 — rollup bucket ALIGNMENT (issue #111), and two pre-existing GC bugs it flushed out

Scope: `series-observability-scope.md` Decisions **21–24**. Downstream halves: `NubeIO/rubix-ai#54`,
`NubeIO/rubix-ai-extensions#15`.

### What shipped

`Tier.align = Option<Align { origin_ms: i64 }>` — where a tier's rollup buckets start. A tier that
declares none is **byte-identical to what shipped**: `bucket_start(ts, w, None)` is literally
`ts / w * w`, pinned by `absent_align_is_the_epoch_floor`.

New file `crates/ingest/src/align.rs` owns the grid arithmetic and is the ONLY floor in the crate —
the read path (both the pushdown and the fold oracle) and the GC fold call the same function with the
same `(width, align)`, which is the invariant the whole feature turns on. New file
`crates/ingest/src/rollup_window.rs` owns the three bounds a fold needs (`tier_cutoff`,
`evict_cutoff`, `oldest_raw_ts`).

The **DST fork is decided: fixed offset, no IANA dependency** (Decision 21). A real zone means a
variable-width grid — a DST day is 23 or 25 hours — which is a different model, not an extra field.
Said in the panel and in `doc-site` rather than left to be discovered in October.

### The two bugs the grid test found, both PRE-DATING alignment

Neither was in the brief. Both were found by one assertion — running `run_gc` **twice** over the same
data and demanding identical stored rows.

1. **Multi-tier policies double-counted the coarser tier on the first pass.** Every tier folds before
   any raw is evicted, so a two-tier policy had the finer tier's rows on disc while the coarser tier
   was still reading the raw underneath them. A 90-sample bucket stored `180`. Self-healing on the
   next pass (which re-folds from the rollups alone) and wrong on every read in between. Fixed in
   `merge_rollups`: a rollup row overlapping surviving raw is redundant with it, not complementary.

2. **Re-deriving a tier from another tier drifts when the grids do not nest.** A 10-minute bucket
   anchored at `:07` straddles a 90-minute boundary anchored at `:30`, so folding it whole into
   whichever coarse bucket holds its START moves samples across a boundary. A coarse bucket holding
   30 samples re-derived itself as **37** and stayed there. Fixed by folding from RAW ONLY, over the
   window raw still covers (`oldest_raw_ts`) — which also turns an O(all history) pass into an
   O(new data) one, on every series, every 5 minutes.

The stated cost (Decision 23): **adding a tier no longer backfills it** from tiers already on disc.
It takes effect from the current raw window forward. Backfilling would mean writing numbers that are
wrong in a way nothing downstream could detect.

### `snap_cutoff` re-thought, not edited

One cutoff floored by the widest width can only be a boundary on one tier's grid. Now each tier folds
to its own boundary and raw is evicted no further than the least-advanced tier reached. Proven in
`two_tiers_on_different_grids_each_fold_complete_buckets`, which asserts surviving raw still reaches
back past the coarse tier's boundary — if it did not, that tier's next bucket could never complete.

### Green output

```
$ cargo test -p lb-ingest --test series_align_grid_test --test series_align_test
running 4 tests
test a_daily_tier_can_start_at_local_midnight ... ok
test an_unaligned_policy_still_buckets_on_the_epoch_grid ... ok
test a_fold_and_a_read_land_on_the_same_declared_grid ... ok
test two_tiers_on_different_grids_each_fold_complete_buckets ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s

running 6 tests
test a_read_inherits_the_finest_declared_anchor_when_its_own_width_has_none ... ok
test an_inherited_anchor_is_inert_at_a_width_that_divides_it ... ok
test the_longest_matching_prefix_owns_the_grid ... ok
test absent_and_zero_are_the_same_grid_but_not_the_same_value ... ok
test an_aligned_tier_round_trips ... ok
test a_policy_written_before_alignment_reads_back_unaligned ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

$ cargo test -p lb-host --test series_align_host_test
running 7 tests
test an_explicit_null_clears_an_anchor ... ok
test writing_an_anchor_needs_the_retention_set_grant ... ok
test an_anchor_round_trips_and_absence_is_not_a_zero ... ok
test patching_a_tier_without_naming_its_anchor_keeps_it ... ok
test a_zero_width_tier_is_refused ... ok
test retuning_a_width_inherits_the_anchor ... ok
test a_bucketed_read_resolves_the_governing_anchor_and_reports_it ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

$ cargo test -p lb-ingest        # the whole crate, 20 binaries
test result: ok. (every binary; 0 failed)
```

`cargo fmt --all --check` is clean.

### Every test in this slice carries its own revert-check

A test that asserts "the buckets are on the anchored grid" passes just as well against a read that
ignores anchors entirely, if the fixture's anchored grid happens to coincide with the epoch one. So
each one also asserts the epoch grid produces DIFFERENT boundaries, and
`a_fold_and_a_read_land_on_the_same_declared_grid` additionally refuses to run on a fixture where a
stored bucket is on both grids at once.

### Repaired in passing

`node/tests/boot_wiring_test.rs` had not compiled since `updated_by`/`updated_ms` landed with policy
provenance — a pre-existing break that took the ENTIRE workspace test run down with it (nothing else
could be attributed while it stood). One `..Default::default()`; that is what the derive is for.

### What is NOT verified live

The live walk needs the scratch node stack; see the rubix-ai session doc for what was driven there.
Everything above is test-verified against the real `mem://` store, real MCP dispatch, and a real
`run_gc` — no mocks.
