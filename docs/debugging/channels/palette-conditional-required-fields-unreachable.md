# `/remind` cannot create a reminder — the per-action-kind fields are unreachable from the palette form

Status: **resolved**.

**Symptom (live UI):** picking `/remind` ("Schedule a reminder (cron + action)") in the channel command
palette, filling the cron `schedule` and the `action_kind` select, then submitting failed at the host
with `bad input: missing string arg: channel` (for `channel-post`; the analogous `tool` / `target` for
the other kinds). The palette form gave **no way to fill** `channel`/`body` (or `tool`/`args`, or
`target`/`action_action`/`payload`) — the fields simply never appeared. So `/remind` could not produce a
working reminder for any action kind.

## Root cause

The reminder-create descriptor declares `required: ["schedule", "action_kind"]` and left every per-kind
action field **optional**. After the optional-arg fix (see
[palette-optional-arg-blocks-submit.md](palette-optional-arg-blocks-submit.md)) the arg rail correctly
auto-activates **only required** args — so an optional field never surfaces. But the action fields are
**genuinely required**, just *conditionally*: `channel` is required **when** `action_kind = channel-post`,
`tool` **when** `mcp-tool`, and so on. The verb's `action_from_flat` reads `str_arg(input, "channel")`
(not defaulted) → the missing-arg error.

Plain JSON-Schema `required` can't express "required **when** `action_kind = channel-post`" without
draft-07 `if/then` conditional subschemas, and the palette's linear arg rail had **no conditional-field
model** — it walked a flat `required` list. `/remind` is the first command that is a *conditional form*,
not a one- or two-field rail.

## Fix — a generic `x-lb.showIf` + `requiredWhenShown` form hint

Both sides declarative, zero reminder knowledge in the UI (the request-side twin of the `x-lb-render`
response contract):

- **Descriptor** (`reminder/descriptor.rs`): each per-kind field carries a generic vendor hint —
  `"channel": { "x-lb": { "showIf": { "action_kind": "channel-post" }, "requiredWhenShown": true } }`.
  `showIf` is a `{arg: value}` condition; `requiredWhenShown` marks a shown field as required. Optional
  shown fields (e.g. `body`) carry `showIf` only. This is JSON-Schema-safe (a vendor `x-lb` block, not
  schema keywords) and generic — any conditional form uses it.
- **Palette types** (`palette.types.ts`): `isShown(schema, key, values)` (the `showIf` map matches the
  currently-collected values) and `isActiveRequired(schema, key, values)` (unconditionally `required`,
  OR shown-and-`requiredWhenShown`). The rail now walks `required ∪ shown-and-required`.
- **Palette** (`CommandPalette.tsx`): a `values` snapshot (chips + inline widget values, e.g. the
  `action_kind` select) feeds `isActiveRequired`/`isShown`. `activeArg` targets active-required unfilled
  args, so once `action_kind` is picked its per-kind fields surface **in turn** (fill cron → pick kind →
  `channel` appears → fill → send). `buildArgs()` collects **only shown** fields, so a stale value from a
  since-hidden field (e.g. a `channel` typed, then `action_kind` switched to `outbox`) never leaks into
  the call.
- **Cron seed** (`CronArg.tsx`): the `react-js-cron` builder only emits `onChange` on an *edit*, so an
  unedited cron left the required `schedule` empty and the rail stuck on it (blocking the form before
  `action_kind` was ever reached). `CronArg` now seeds its shown default (`0 9 * * *`) into the collected
  value on mount — WYSIWYG: the default shown is the default submitted unless the user edits it.

Result: `/remind` completes end to end from the palette — the create-form gateway test drives the real
form and asserts a real channel-post reminder.

## Follow-up (named, not this fix)

- **Optional shown fields** (`body`, `args`, `payload`) are shown-but-not-required, so the rail does not
  auto-activate them — same "add a field" affordance the optional filters need. Tracked, not built.
- **Render fields as a form block** (all currently-relevant fields at once) rather than one-at-a-time —
  the chained rail works but a real form renderer over the schema (`required ∪ shown-required`, a values
  object, submit-when-valid) is the correct long-term shape. See the session doc.

## Regression tests

- `reminder/descriptor.rs` — `per_kind_action_fields_are_conditionally_required`: each per-kind field
  declares the right `showIf`/`requiredWhenShown`.
- `CommandPalette.dispatch.test.tsx` — `surfaces a conditionally-required field once its showIf matches,
  blocks submit, then sends it`: a generic `things.form` command (a `select` gating a
  `showIf`+`requiredWhenShown` field) surfaces the field, blocks submit until filled, then sends it.
- `CommandPalette.reminders.gateway.test.tsx` — `create form e2e: driving the /remind palette form
  creates a real channel-post reminder`: the FULL real-gateway request round-trip through the palette
  form; asserts the real stored reminder via `reminder.list`/`get`, and that no other-kind field leaked.
- `CronArg.test.tsx` — `seeds the shown default into onChange on mount`: the mount-time default emit.
