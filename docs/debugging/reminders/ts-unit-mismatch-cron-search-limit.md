# Reminder create crashes: `CronScheduler time search limit exceeded` (ms vs seconds `ts`)

**Symptom (frontend gateway test stderr):** the first real run of
`ui/src/features/reminders/RemindersView.gateway.test.tsx` failed every `reminder.create` with an
unhandled rejection from the real gateway:

```
Error: bad input: CronScheduler time search limit exceeded.
 ❯ postJson src/lib/ipc/http.ts:525
 ❯ src/features/reminders/useReminders.ts:53  (createReminder)
 ❯ onCreate src/features/reminders/RemindersView.tsx:44
```

Only the UI path tripped it — every Rust integration test (`reminders_reactor_test.rs`,
`reminders_mcp_test.rs`) was green, because they inject small **logical-seconds** `now` values
directly.

## Root cause

The reminder record's `ts`/`now` is a **logical clock in seconds** since the epoch — the same unit
`lb_reminders::next_after` feeds `croner` (`secs_to_dt` does `from_timestamp(secs, 0)`). The host
computes the first fire as `next_after(schedule, now)`.

`useReminders` passed `Date.now()` — **milliseconds** (~1.7e12). Fed to `next_after` as if it were
seconds, that converts to an instant ~55 000 years in the future. `croner`'s
`find_next_occurrence` walks forward from the anchor looking for the next matching slot and aborts
once it passes its internal search horizon — hence "time search limit exceeded", surfaced as a clean
`bad input` (not a panic), but the create never persisted.

The unit boundary was invisible because both sides are bare `u64`/`number` seconds-since-epoch with
no newtype — the only place the mismatch shows is the one seam that bridges JS wall-clock to the
host's logical clock.

## Fix

Convert to seconds at that seam. `ui/src/features/reminders/useReminders.ts`:

```ts
function nowSecs(): number {
  return Math.floor(Date.now() / 1000);
}
```

…used for the `ts` of `create`/`update`/`delete` (replacing the three `Date.now()` calls).

## Regression test (fails-before / passes-after)

`RemindersView.gateway.test.tsx > creates a reminder via the real path and lists it` asserts the
round-tripped reminder has `nextAttemptTs > 0` and a faithful schedule. With the millisecond bug the
create threw (test red); after the `nowSecs()` fix the create persists a sane future fire and the
test is green. The two follow-on tests (pause/resume, delete) also exercise the real `update`/`delete`
`ts` seam.

## Lesson

When a value crosses from JS wall-clock into the host's injected logical clock, it must be converted
to the host's unit (seconds) at the boundary — a bare numeric type carries no unit, so the only
defense is converting at the one seam and testing the real cross-process path (a unit test with an
injected small `now` can never catch this).
