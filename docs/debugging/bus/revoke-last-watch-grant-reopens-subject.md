# Revoking the last `bus:*:watch` grant re-opened the subject (open-mode bypass)

- Date: 2026-07-13
- Area: bus (auth-caps)
- Status: **fixed**
- Session: ../../sessions/bus/bus-watch-subject-scope-session.md
- Scope: ../../scope/bus/bus-watch-subject-scope-scope.md — issue #49

## Symptom
Building Gap 2 (revoke-terminates-stream) for subject-scoped `bus.watch`, the first
`bus_watch_revoke_test` FAILED: after `grants.revoke` of the holder's `bus:care.feed.leo:watch`
grant, the open stream did **not** close (the re-check kept returning "authorized"), the timeout
elapsed:
```
the stream closes within a bounded tick after revoke: Elapsed(())
```

## Root cause
The initial scoped-mode rule was: *"the caller is under subject enforcement iff they currently hold
**any** `bus:*:watch` grant; if so require a match, else open."* Revoking the holder's **only**
watch grant left them holding *no* watch grant → they fell back to `WatchMode::Open` → the re-check
re-authorized (open mode allows every subject) → the stream stayed up. Worse than a missed close: a
**fresh re-subscribe would also succeed**, so revoke didn't actually isolate — a real data-isolation
hole, not just a latency miss. The rule derived "mode" from a global "any grant exists" predicate,
which flips the wrong way exactly when the last grant is removed.

## Fix
Anchor the stream-lifetime requirement to the **grant itself**, not to "any grant exists". Two
pieces in `crates/host/src/bus/scoped.rs`:
- `authorize_subject_scoped` now returns a `WatchMode` (`Open` | `Scoped`) — the mode the subscribe
  was allowed under.
- `still_scoped_authorized(store, principal, ws, subject)` returns whether a matching
  `bus:<subject>:watch` grant STILL exists (a live store read).

The re-check (`WatchRecheck`, `role/gateway/src/session/events/recheck.rs`) is **mode-sticky**: it
captures the mode on the first tick; a `Scoped` stream thereafter requires `still_scoped_authorized`
to hold, so revoking the matching grant — *including the caller's last one* — returns false and the
stream closes. It can never drop back to open mode, because the requirement no longer asks "does any
grant exist" — it asks "does THIS grant still exist".

## Regression tests
- `crates/host/tests/bus_test.rs::revoking_the_only_grant_denies_the_subject_it_does_not_reopen` —
  asserts `still_scoped_authorized` is true with the grant, false after revoke (fail-before on the
  old rule, pass-after).
- `role/gateway/tests/bus_watch_revoke_test.rs::revoking_the_scoped_grant_closes_the_open_stream_within_a_tick`
  — the original failing test, now green.

## Lesson
When a "narrow only if some grant is present" rule guards access, deriving the enforcement mode from
a *global* "any grant exists" predicate inverts at the boundary — removing the last grant relaxes
instead of tightens. Anchor the check to the specific grant that authorized the access, so revocation
can only ever remove reach, never restore it.
