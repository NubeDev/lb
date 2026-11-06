# lb-schedules

Timezone-aware schedules in the rubix-ce document shape (`ScheduleDoc`: `weekly`, `events` and
`exception` maps) and the box's rules for them: `validate` (save-time rules), `evaluate_local`
(what runs at a wall time), `next_change`, `occurrences` (dated windows, yearly copies) and `Zone`
(UTC or IANA, DST edges). Everything is pure and takes local wall time; the host converts instants
through `Zone`.
