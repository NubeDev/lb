# Flows: global schedules in the rubix-ce shape (session)

- Date: 2026-11-04
- Scope: ../../scope/flows/schedule-rubix-ce-shape-scope.md
- Stage: library posture (STATUS.md)
- Status: done

## Goal

Replace the global schedule record with the rubix-ce document, evaluate it by the box's rules, and
check every save against `rev`, so downstream consumers can treat a node schedule as the group for
many device schedules.

## What changed

- `rust/crates/store/src/write_if_rev.rs`: an upsert guarded by the stored `rev`, one transaction.
- `rust/crates/schedules/`: rewritten around the document (types, parsing, zones, validation,
  windows, evaluation, next change); the old engine (`Schedule`, `ScheduleEvaluator`, bulk
  overrides) deleted.
- `rust/crates/host/src/flows/schedule_store/`: the old 433-line `schedule_store.rs` split into a
  folder of verbs (dropped from the file-size baseline); the record takes the new shape; `rev` on
  get/list; old-shape rows refused and listed under `unreadable`; rev-checked save with coded
  refusals; `rev_checked` on list.

## Decisions & alternatives

- Check-and-write as one transaction (IF guard + UPSERT), the pattern undo's conditional restore
  already proved, rather than a single UPSERT ... WHERE statement, whose behaviour on an absent
  record is not pinned anywhere in lb.
- `schedule.save` answers the new `rev`; `schedule.list` adds `rev_checked: true` so the version
  floor works in an empty workspace.
- See the plan's "Decisions the plan had to make" for the full list.

Where the code differs from the plan:

- `write_if_rev` spots a stale write by re-reading the live rev after a refused transaction. The
  plan's LET/THROW match on the error text did not work in SurrealDB.
- A stored row without a rev reads as rev 1 (fixed in `lb-store` `Record`, commit `063596e0`), so
  rows written before revisions existed can be saved over with `rev: 1`.
- `next_transition` is converted to UTC through the record's `Zone` (DST-correct) and is `null` when
  `enabled` is false.
- The flow `schedule` node keeps its last state (cursor untouched, warning logged) while its
  schedule row is unreadable, and evaluates again on the next pass.
- A row whose `schedule` document does not decode is refused by `get`/`evaluate` with
  `schedule_old_shape` ("does not read as a schedule (...)") and listed under `unreadable`, the same
  as an old-shape row.
- 29 February: a yearly entry starting on 29 Feb is skipped in years without one; a yearly end on a
  missing 29 Feb becomes 1 March 00:00 (commit `0b26eab1`).
- Ties: a weekly entry beats an event starting the same minute; otherwise natural key order
  (`w2` before `w10`).
- Accepted edge: on the night DST ends, a dated boundary inside the repeated hour can count twice.

## Tests

Capability-deny: `read_rev.rs::reading_needs_the_read_cap`, `save_rev.rs::saving_needs_the_write_cap`.
Workspace isolation: `read_rev.rs::another_workspace_cannot_see_the_record`,
`save_rev.rs::each_workspace_keeps_its_own_revisions`, `write_if_rev_test.rs::revisions_are_per_workspace`.
Concurrency: `write_if_rev_test.rs::writers_racing_on_one_rev_land_exactly_once`.
Old rows: `read_rev.rs::an_old_shape_row_is_refused_and_listed_as_unreadable`,
`save_rev.rs::an_old_shape_row_is_saved_over_with_its_rev`,
`flow_node.rs::an_old_shape_row_holds_the_node_without_firing`.

Green output (2026-11-05, HEAD `97d65826`):

```text
$ cargo test -p lb-store --test write_if_rev_test
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

$ cargo test -p lb-schedules
     Running unittests src/lib.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/doc_test.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/evaluate_test.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/next_change_test.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/validate_test.rs
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/windows_test.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/zone_test.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo test -p lb-host --test schedule_suite
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s

$ cargo test -p lb-host --lib schedule_store
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 604 filtered out; finished in 0.00s
```

Only these four commands were run, not the full workspace or the full `lb-host` suite.

## Debugging

None.

## Public / scope updates

`doc-site/content/public/flows/schedules.md` (new). Scope doc created.

## Skill docs

n/a: no new agent-drivable surface; the `schedule.*` verbs already existed.

## Dead ends / surprises

- The plan's in-transaction `LET`/`THROW` match for a stale rev did not work in SurrealDB, hence the
  re-read of the live rev after a refused transaction.
- Rows stored before revisions existed had no rev; they now read as rev 1.

## Follow-ups

- rubix-ai bumps its lb pin and moves every schedule reader to the new shape (plan 3).
- Sites back up and recreate their schedules after the upgrade (plan 5).
- STATUS.md updated.

Open:

- Locally on macOS the host tests `system_map_test` and `insights_test` hang on shutdown. Their
  in-process zenoh peers find each other by multicast scouting over the LAN and OrbStack interfaces
  and deadlock in Session close; CI has no multicast, so it passes there. `bus/src/peer.rs:51-56`
  already warns about it. A test bus with scouting off would fix it.
- `schedule.delete` removes the row, so a recreate restarts at rev 1, and a client still holding
  the old rev 1 could overwrite the new schedule.
- No rev-0 concurrent-create race test in the store crate.
