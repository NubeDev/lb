# Two tests share one SQLite fixture path, so whichever loses the race fails

- Date: 2026-07-27
- Area: federation (test harness)
- Status: **fixed**
- Found by: the #108 verification pass — a baseline-vs-tree failure diff, not by the suite complaining
- Entry for: `crates/host/tests/federation_sqlite_test.rs`

## Symptom

`cargo test -p lb-host` reported two failures that were absent from the same suite run at the
pre-change commit, so they looked exactly like a regression introduced by the #108 work:

```
---- federation_end_to_end_sqlite stdout ----
panicked at crates/host/tests/federation_sqlite_test.rs:96:6:
seed fixture rows: SqliteFailure(Error { code: ReadOnly, extended_code: 1032 },
                                Some("attempt to write a readonly database"))

---- federation_delete_removes_a_row_by_key stdout ----
panicked at crates/host/tests/federation_sqlite_test.rs:96:6:
seed fixture rows: SqliteFailure(Error { code: SystemIoFailure, extended_code: 5898 },
                                Some("disk I/O error"))
```

Neither test is touched by anything in #108, and neither error message mentions the real cause.

## Two false leads (worth recording — both were plausible)

1. **"It's a regression."** It appeared in the tree run and not the baseline run. It is not: nothing in
   the change set reaches federation or SQLite.
2. **"It's the disk."** The verification worktree had grown to 142 GB and taken the filesystem from
   50% to 85%, and `disk I/O error` is exactly what that looks like. Freeing it (back to 69%) changed
   nothing — the tests still failed. A real cause that *also* explains an unrelated symptom is the most
   expensive kind of wrong.

## Root cause

`seed_db()` derived its fixture path from the process id alone:

```rust
let path = std::env::temp_dir().join(format!("lb-fed-sqlite-{}.db", std::process::id()));
let _ = std::fs::remove_file(&path);
```

Cargo runs a test binary's tests as **threads of one process**, not as separate processes. Both tests
in this file therefore computed the *same* path, and whichever entered `seed_db` second deleted the
`.db` file the first already had open. SQLite reports a file deleted out from under an open handle as
`ReadOnly` or `SystemIoFailure` — never as "someone unlinked your database".

`--test-threads=1` passing 2/2 while the default run failed 2/2 was the tell.

## Fix

`seed_db(who: &str)` takes a per-test discriminator, so the two tests own separate files:

```rust
let path = std::env::temp_dir().join(format!("lb-fed-sqlite-{}-{who}.db", std::process::id()));
```

`seed_db("end-to-end")` and `seed_db("delete-by-key")`. No production code changed.

## Revert-check

Sabotage = give both call sites the same discriminator again (the original shape):

```
SHARED PATH   → run 1..6: FAILED. 0 passed; 2 failed   (6/6)
DISTINCT PATH → run 1..6: ok.     2 passed; 0 failed   (6/6)
```

## Lesson

This is #108's own thesis turned on the harness: **a test that can flake is a bug**
(`testing-scope.md` §3, Determinism), and this one had been passing or failing on machine timing for as
long as the second test has existed. It survived because a green run and a red run look equally
plausible when the failure message names the filesystem instead of the race.

It also shows why the honest baseline method matters: the diff surfaced it, and reading the failure
rather than trusting either the test name or the error text is what identified it. Had the run been
compared by *count*, this would have been filed as "2 more failures than baseline, probably flaky" and
left in place.

**`process::id()` is not a test-unique key.** Anywhere a test derives a temp path from it, every other
test in that binary shares it.
