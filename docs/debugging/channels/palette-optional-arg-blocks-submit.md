# `/reminders` (and any list command) demands an empty arg box before it will run

Status: **resolved**.

**Symptom (live UI):** picking `/reminders` ("List reminders (interactive)") in the channel command
palette did **not** run the list — instead the arg rail showed an empty **`status…` text input** and the
send button waited on it. Listing reminders should be zero-friction: pick it, see the table. The user
hit this immediately on first use.

## Root cause

`reminder.list`'s descriptor declares two **optional** filter args (`status`, `limit`) and an empty
`required`. But the palette treated **every** arg as something to fill: `activeArg`
(`CommandPalette.tsx`) picked the first *unfilled* arg regardless of whether it was required, and
`submitDisabled`/`allArgsFilled` keyed off that active arg — so an optional `status` became the active
arg, rendered a text box, and blocked submit. A command whose args are all optional was unrunnable
without typing into a filter it never needed.

This is a **generic palette bug**, not reminders-specific — any command with optional args hit it; it
only surfaced now because `reminder.list` is the first shipped command with **only** optional args.

## Fix

Make the arg rail respect the schema's `required` list — optional args are **skippable and never block
submit**:

- New `isRequired(schema, key)` helper (`lib/channel/palette.types.ts`).
- `activeArg` now auto-targets only **required** unfilled args (a required chip arg first, then a
  required inline widget); an optional arg never grabs focus. So a command with only-optional args has
  **no** active arg → the rail shows no text box and send is enabled immediately.
- `buildArgs()` still collects any inline widget value the user *did* set, so a filled optional inline
  filter is included; the plain/`result` submit paths are unchanged.

Result: `/reminders` runs the instant it is picked (posts its `descriptor.result` render, no filter
args). A command with a **required** arg (e.g. the agent's `goal`) still blocks on it exactly as before.

## Follow-up (named, not this fix)

Optional args are currently **not reachable** from the rail at all (they never activate) — fine for v1
(list-with-no-filter is the common case), but filtering `/reminders` by `status`/`limit` from the
palette needs an explicit "add a filter" affordance. Tracked, not built here.

## Regression test

`CommandPalette.dispatch.test.tsx` — `runs a command with only OPTIONAL args immediately — no arg box,
no blocked submit`: accept a result-declaring command whose only arg is optional, assert **no** `status`
arg box renders and pressing send immediately posts the declared render. The existing "merge collected
args into source.args" / "verbatim args" proofs now use a **required** `status` arg (so the rail
activates it) — both still green.
