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
