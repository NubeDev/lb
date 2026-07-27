# Closing the blind spots a green suite has — integration (session)

- Date: 2026-07-27
- Closes: [NubeDev/lb#108](https://github.com/NubeDev/lb/issues/108)
- Status: **done** (all in-scope boxes), two follow-ups explicitly deferred
- Track logs: `blind-spots-track-a-session.md` (fixture + boot wiring),
  `blind-spots-track-b-session.md` (multi-batch + axis)
- Cross-repo: `rubix-ai-extensions/extensions/modbus/docs/sessions/2026-07-27-*.md`

This is the integration log: what the five parallel tracks produced, how it was verified as one tree,
and the decisions that were mine rather than a track's.

## The ask, restated

Three bugs shipped past a fully green suite. They are already fixed. #108 is not about re-fixing them —
it is about the *class*: **a test that shares the bug's assumption cannot see the bug.** Four shapes,
each needing a standing mechanism so the next one is caught by CI rather than by a person watching a
live node.

## What landed

| # | Item | Where |
|---|---|---|
| 1 | Prior-state fixture | `crates/host/tests/support/prior_state.rs`, `series_prior_state_test.rs` |
| 2 | Boot-wiring assertions | `node/tests/boot_wiring_test.rs` (through `reactors::spawn`) |
| 3 | Multi-batch by default | ingest/insights loop tests raised past `COMMIT_BATCH`/`CAP_EVICT_BATCH`/page |
| 4 | Axis assertions | `series_width_axis_test.rs`, `viz_resolution_axis_test.rs` |
| 5 | Harness-honesty lint | modbus `scripts/check-harness-honesty.sh`, wired into `build.sh` |
| 6 | Bucketed history range | modbus `range` widget option + `bucketWindow.ts` + live test |
| 7 | RetentionPanel UI test | modbus `RetentionPanel.test.tsx` (8 tests) |
| 8 | Narrowed i18n parity rule | modbus `hasUntranslatedMarker.ts` (`\bTODO\b`, case-sensitive) |
| 9 | Canvas flake | modbus `test/offsetParent.ts` + one `findByRole` |

## Three bugs the closing work itself found

The point of the exercise, and the best evidence it was worth doing.

1. **A pre-cap policy row aborts a whole workspace's retention pass** (`debugging/ingest/
   pre-cap-policy-row-aborts-the-retention-pass.md`). `list_policies` projects columns by name, so a row
   written before `max_samples` existed returns `NONE` — a *present null*, which `#[serde(default)]`
   never covers. `run_gc` opens with `list_policies`, so on any node upgraded from before #65 every
   reactor tick errored out and configured horizons silently stopped being enforced. **Found by the new
   prior-state fixture on its first run** — i.e. mechanism 1 paid for itself immediately.

2. **A test helper still carried the exact bug-#1 termination condition.** `durable_redrain_test.rs`'s
   `drain_all` looped on `pass.committed == 0` — the precise equivalence that caused the original drain
   stall — sitting unnoticed inside the test suite meant to guard against it.

3. **Two federation tests shared one SQLite fixture path** (`debugging/federation/
   two-tests-share-one-sqlite-fixture-path.md`). `seed_db()` keyed its temp file on `process::id()`, but
   cargo runs a binary's tests as *threads of one process*. Found by the baseline-vs-tree diff, via two
   false leads (see below).

## Decisions that were mine

1. **The live verification became a test, not a screenshot.** The brief asked me to confirm the bucketed
   widget renders on the live node. No browser automation was available, so instead of a manual check I
   wrote `PointValue.live.test.tsx` — the real widget, the real node, a real `POST /mcp/call`. Better on
   the merits: reproducible, and it stays true tomorrow. *Rejected:* asserting via `curl` alone, which
   proves the host answers but not that the tile renders it — precisely the half-proof that let bug #2
   ship.

2. **That live test's first assertion was wrong, and the fix is the lesson.** It asserted the 24h
   sparkline draws >60 points. It drew 55, because this box has ~11h of history, not 24 — the assertion
   was measuring *node uptime*, not behaviour. Replaced with: ask the host the same question directly and
   assert the tile drew exactly the number of buckets the host returned. That holds on a node with a day
   of history and on one with an hour. A test whose expectation encodes the fixture's accidental size is
   the same family of mistake as testing at one point on an axis.

3. **I wired `remoteEntry.tsx` myself.** Track E built the `range` option correctly but left it inert —
   `remoteEntry.tsx` was outside its file ownership and is the only place `ctx` exists, so the manifest
   option would have rendered in the tile editor and done nothing. A feature reachable from nowhere is
   not shipped.

4. **I fixed the federation flake rather than filing it.** Out of the literal scope, but #108's own
   addendum is about exactly this ("a check that is unreliable is a check that is off"), and I had just
   caused it to surface. Test-side only, revert-checked 6/6 both ways.

5. **I updated `testing-scope.md` §3.2 with what is closed and what is not.** Both lb tracks deliberately
   left it to the integrator. The wording is careful: the *mechanisms* are closed; applying each shape to
   a new area is still per-slice work.

## Verification

Method: a detached worktree at the pre-change commit (`b969b455`) with `rust/.cargo/config.toml` copied
in, same suites both sides, **failure sets diffed — not counts**.

```
BASELINE lb-ingest : 67 passed,   0 failed  (12 suites)  EXIT=0
BASELINE lb-host   : 1446 passed, 103 failed (157 suites)
TREE  ingest+node+insights : 99 passed, 0 failed (25 suites)  EXIT=0
TREE  lb-host              : 1529 passed, 24 failed (159 suites)
```

**The baseline's 103 is inflated and must not be quoted as a real figure.** 75 of those failures are
`missing hello component … Build it first` — a fresh worktree has no built wasm/native fixture
extensions. That inflation only risks *masking* a regression, never inventing one, so the set-diff stays
sound in the direction that matters; I mitigated it by reading every tree-side failure individually
rather than trusting the diff alone.

Failure-set diff, tree minus baseline: **2 candidates, both investigated, neither a regression** —
`federation_end_to_end_sqlite` and `federation_delete_removes_a_row_by_key`, the shared-fixture race
above. Now fixed, so the final tree diff is **zero regressions**.

Gates: `cargo fmt` before the final check, then `check-file-size.sh` →
`OK — 2408 files checked, 114 grandfathered` (backlog unchanged — no grandfathered file grew).

### Two false leads on the federation failures (recorded because both were plausible)

- *"It's a regression"* — present in the tree run, absent from the baseline run. It is not; nothing in
  the change set reaches federation or SQLite.
- *"It's the disk"* — my own baseline worktree had grown to **142 GB** and taken the filesystem from 50%
  to 85%, and `disk I/O error` is exactly what that looks like. Freeing it (back to 69%) changed nothing.
  A wrong cause that also explains the symptom is the expensive kind.

The tell was `--test-threads=1` passing 2/2 while the default run failed 2/2.

## Deferred, explicitly

- **The `[patch]` in `rubix-ai/.cargo/config.toml` stays.** Out of scope by instruction: removing it is a
  release (tag lb, bump the pin), not a cleanup. Recorded instead as a callout in
  `rubix-ai/docs/WORKFLOW-LB.md` §3a, linked to #108, noting the box is running code no other machine
  has.
- **`crates/outbox` has no batch bound**, so there is nothing to page past today — but equally no test
  that would catch a one-batch stall the day one is added.
- **Telemetry's `MAX_PAGE` console read and insights' `MAX_ROWS` truncation branch** are still untested at
  the boundary (the insights paging *loop* is now covered).
- **modbus `en.ts`/`es.ts` grew while already over the 400-line rule** (526→535, 520→528). The modbus repo
  has no ratchet script to enforce it, so this passed silently; flagged rather than hidden. Splitting the
  i18n dictionary into namespaced modules is the right long-term fix and is a refactor of its own.
- **No test for a save whose reload is refused** in `RetentionPanel` (`set` granted, the following `list`
  denied) — the panel would fall back to "not managed here" after a successful write.

## Cross-links

- Scope: `../../scope/testing/testing-scope.md` §2 category 6, §3.2
- Debugging: `../../debugging/ingest/pre-cap-policy-row-aborts-the-retention-pass.md`,
  `../../debugging/federation/two-tests-share-one-sqlite-fixture-path.md`
- Prior slices: `../ingest/series-normalize-session.md`,
  `rubix-ai-extensions/extensions/modbus/docs/sessions/2026-07-26-poll-retention-normalize-session.md`
