# Global schedules

A global schedule is a workspace record (`schedule.*`) that flow nodes and dashboard widgets
reference by id.

## Shape

The record carries the rubix-ce schedule document under `schedule`, with the same key names:
`config.names`, `config.default_name`, and the `weekly`, `events` and `exception` maps. Each entry
has a name, times, its own `value` and `color`. Dated entries may carry `kind` (`closed`, `changed`,
`extra`, `holiday`), a `label`, and `yearly` (closures and holidays only). Times are local wall time
in the record's `timezone`. Weekly ends are inclusive (`17:59` runs to 17:59:59); dated ends are
exact.

Since `node-v0.56.0` the old `weekly` / `exceptions` fields are gone. A record still stored in that
shape is refused: `schedule.get` and `schedule.evaluate` answer `schedule_old_shape`, and
`schedule.list` names it under `unreadable` with its `rev`. A row whose `schedule` document is
present but does not decode is treated the same way. Save over it in the new shape with that `rev`,
or delete it. A flow `schedule` node following it holds its last state until then.

## Saving

`schedule.get` and `schedule.list` return `rev`. Send it back with `schedule.save`. A save without
`rev` (or with `0`) creates, and is refused with `schedule_exists` if the id is
taken. If the record changed since you read it the save is refused with `schedule_stale` ("changed
elsewhere, reload"). A save answers `{id, rev}`. Treat `rev` as opaque, and read the schedule again after a delete: a recreated id starts again at
`rev` 1.

## Evaluating

`schedule.evaluate` answers `is_active`, which entry is running (`active_source` `weekly:<key>` or
`event:<key>`, `entry_name`, `value`) and `next_transition`. Weekly entries and events add
together, exceptions switch the schedule off, and where entries overlap the one that started first
gives the name and value. On a tie a weekly entry beats an event, then keys sort naturally (`w2`
before `w10`). `next_transition` is a UTC instant and is `null` when the schedule is disabled.

`exception_active` is true whenever an exception covers now, even if nothing would otherwise have run.

A yearly entry that starts on 29 February is skipped in years without one; a yearly end on a
missing 29 February becomes 1 March at midnight.
