# Handover — the `/remind` create form (multi-field conditional form) is not finished

> **UPDATE (2026-07-01): Bug B is now FIXED — `/remind` completes end to end from the palette.** See
> [remind-form-conditional-fields-session.md](remind-form-conditional-fields-session.md) and
> [../../debugging/channels/palette-conditional-required-fields-unreachable.md](../../debugging/channels/palette-conditional-required-fields-unreachable.md).
> The generic `x-lb.showIf` + `requiredWhenShown` form hint (steps 1/2/4 below) shipped, plus a cron
> mount-seed. The remaining items are the **named follow-ups** (optional shown fields; the full
> schema-form renderer) — the long-term recommendation section below still stands as the next slice.

Status: **~~partial~~ resolved for create — the blocking crash and the unreachable per-kind action fields
are both fixed.** This doc is the original honest state + the next steps + the long-term recommendation;
read the UPDATE above and the session doc for what actually shipped.

Context: the channel rich-responses + reminders slice shipped the descriptor-driven contract, the
`/reminders` interactive list (working), and `reminder.create` accepting a flat form. `/remind` (the
create form) is the one surface that is not yet usable end to end. Two live-UI bugs were found by the
user exercising it; one is fixed, one is only partially addressed.

## What works now

- `/reminders` (list) — renders N rows, pause/delete controls drive the real verbs. Fixed this session
  (row-unwrap `ROW_KEYS`, the palette optional-arg fix). Firing (run-now / reactor) is blocked by a
  separate **pre-existing** bug (see `debugging/reminders/reminder-fire-reresolve-misses-token-caps.md`).
- `reminder.create` **verb** — accepts the flat form; assembles the nested `Action` server-side; derives
  `id`; supplies `now` in seconds. Verified against the live node:
  `{schedule, action_kind:"channel-post", channel, body}` → a real reminder. Works when ALL the needed
  fields are supplied.
- The palette optional-arg fix — a command whose args are all optional (e.g. `/reminders` filters) runs
  immediately with no arg box. See `debugging/channels/palette-optional-arg-blocks-submit.md`.

## The two `/remind` bugs the user hit

### Bug A — `missing string arg: action_kind` — FIXED

**Cause:** `/remind` is the first command with **two required INLINE widgets** (`schedule`→`cron`,
`action_kind`→`select`). The palette only ever showed ONE inline widget at a time, and an inline arg was
never counted "filled" (`filledNames` was chip-only), so `activeArg` stuck on `schedule` forever;
`action_kind`'s select never rendered and its value was never collected → the create call omitted
`action_kind`.

**Fix (shipped, `CommandPalette.tsx`):** `filledNames` now also includes inline args that have a
non-empty value in `inlineVals`, so the rail advances through each required inline widget in turn
(cron filled → select activates). Typecheck + palette unit tests green.

### Bug B — `missing string arg: channel` — NOT FIXED (the real remaining work)

**Cause:** after `action_kind` is picked, the **per-kind action fields** (`channel`/`body` for
channel-post; `tool`/`args` for mcp-tool; `target`/`action_action`/`payload` for outbox) are declared
**optional** in the descriptor (`required:["schedule","action_kind"]`), so the rail — which now only
auto-activates REQUIRED args (the correct `/reminders` behaviour) — never surfaces them. You cannot fill
`channel`/`body` from the form.

**And they are genuinely required:** the verb's `action_from_flat` reads `str_arg(input,"channel")` for
channel-post (not defaulted), so a create without `channel` fails `missing string arg: channel`.
Verified live. So `/remind` from the palette currently **cannot** produce a working reminder for any
action kind — it errors on the first missing action field.

Net: Bug A unblocked the form's first half; Bug B means the second half (the action fields) is still
unreachable, so `/remind` is not usable end to end yet.

## Why this is hard (the shape of the problem)

The action fields are **conditionally required**: which ones are needed depends on the value of
`action_kind`. Plain JSON-Schema `required` can't express "required *when* `action_kind=channel-post`"
without `if/then` (draft-07 conditional subschemas), and the palette's arg rail has no conditional-field
model — it walks a flat `required` list. This is the first command that needs a **conditional,
multi-field form**, not a linear arg rail. The rail was built for one-inline-widget commands
(`federation.query`'s sql, `agent.invoke`'s runtime); `/remind` is a form.

## Next steps (concrete, in order)

1. **Add a `showIf` vendor hint to the descriptor** (`reminder/descriptor.rs`): each action field declares
   when it applies and that it is then required, e.g.
   `"channel": { "type":"string", "x-lb": { "showIf": { "action_kind": "channel-post" }, "requiredWhenShown": true } }`.
   Keep it a generic `x-lb` vendor key (JSON-Schema-safe), not reminder-specific — any conditional form
   uses it. (Alternatively use standard JSON-Schema `allOf:[{if:{properties:{action_kind:{const:...}}},
   then:{required:[...]}}]`, but that's heavier to parse UI-side; a small `x-lb.showIf` is the pragmatic
   generic hint, consistent with the rest of the `x-lb` vocabulary.)
2. **Teach the palette the conditional-field model** (`CommandPalette.tsx` + `palette.types.ts`): a field
   with `showIf` is part of the active arg set ONLY when its condition matches the currently-collected
   args; when shown it is treated as required (blocks submit until filled) and gets its widget in the
   rail. This generalises the rail from "walk `required`" to "walk `required ∪ shown-and-required`". A
   `select` value change re-computes which fields are shown.
3. **Render all active fields at once (recommended) OR chain them.** The linear one-widget-at-a-time flow
   is confusing for a real form (fill cron → it vanishes → pick kind → channel appears). Prefer rendering
   the full set of currently-relevant fields together (cron + kind + the kind's fields) as a small form
   block. This is the bigger UI change but the correct one.
4. **Tests:** a palette unit test that `/remind` with `action_kind=channel-post` surfaces `channel`+`body`
   as required and blocks submit until filled, then sends all of them; a real-gateway create-from-`/remind`
   test that a channel-post reminder is created (assert via `reminder.get`) — currently missing because
   the form couldn't complete.
5. **Optional-arg reachability (the sibling gap):** the `/reminders` filters (`status`/`limit`) are also
   unreachable from the rail right now (they never activate). Same conditional/opt-in-field mechanism
   solves both — an "add a filter" affordance is the opt-in for a genuinely-optional field, `showIf` is
   the conditional for a contextually-required one.

## Long-term recommendation (what's best)

**Build a real schema-driven form model, not a linear arg rail.** The arg rail was right for one- or
two-field commands, but the moment a command is a *form* (conditional fields, several fields at once,
grouped sections) the linear "active arg" walk breaks down — `/remind` is the proof. The clean long-term
shape, still 100% backend-driven:

- **The descriptor stays the single source of truth.** `input_schema` (with `x-lb` widget + `showIf` +
  `requiredWhenShown`) fully describes the form; the palette is a generic **form renderer** over it — it
  renders every currently-relevant field with its registered widget, tracks a values object, computes
  validity from `required ∪ shown-required`, and enables submit when valid. No per-command code, no
  reminder knowledge — exactly the principle the rest of this slice holds.
- **Two field tiers, both declarative:** *contextually-required* (`showIf` + `requiredWhenShown` — the
  action fields) and *genuinely-optional* (offered behind an "add field/filter" affordance — the list
  filters). The renderer treats them uniformly from the schema; no special cases.
- **Reuse the widget registry** you already have (cron/select/text/number/…, ∪ `ext:<id>/<widget>`); the
  form model just decides *which* fields to show and *whether* the form is submittable — it does not add
  renderers.
- This makes `/remind`, a future `/deploy`, any extension's create form, etc. all work with **zero** new
  UI — the same payoff as the response side. It is the request-side twin of the `x-lb-render` response
  contract: `x-lb` form hints (`widget`/`showIf`/`requiredWhenShown`) are the request-side declaration,
  the palette is the generic interpreter.

Estimated shape: ~1 descriptor change (add `showIf`/`requiredWhenShown` to the reminder action fields),
~1 focused palette refactor (arg rail → schema-form renderer with a values object + conditional
visibility + validity), the widget registry unchanged, plus the two tests above. Medium effort, but it
retires the whole class (every future create form) rather than patching `/remind` alone.

## Files touched for Bug A (already shipped)

- `ui/src/features/channel/palette/CommandPalette.tsx` — `filledNames` counts valued inline args (so the
  rail advances through multiple required inline widgets); `activeArg`/`submitDisabled` respect the
  schema's `required` (the optional-arg fix).
- `ui/src/lib/channel/palette.types.ts` — `isRequired(schema, key)` helper.
- `ui/src/features/channel/palette/CommandPalette.dispatch.test.tsx` — the only-optional-args immediate-run
  test.

## Related

- `debugging/channels/palette-optional-arg-blocks-submit.md` (the optional-arg fix)
- `debugging/reminders/reminder-fire-reresolve-misses-token-caps.md` (the separate, pre-existing firing bug)
- `scope/channels/channels-rich-responses-scope.md` (the descriptor-driven contract this form is the
  request-side of)
