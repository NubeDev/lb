# Frontend dashboard — JSON payload builder — Slice 5 (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-config-vars-scope.md (Slice 5)
- Status: done
- Public: ../../public/frontend/dashboard.md → "JSON payload builder"
- Tests: ui/src/features/dashboard/builder/jsonPayload.test.ts (4),
  busBridge.gateway.test.tsx ("Slice 5 — a JSON payload template interpolates + sends" e2e)

## Goal

A CodeMirror JSON editor authoring a JSON template with `${var}`/`{{value}}` slots + a target picker (an
extension write tool via the source picker's Action group, `bus.publish`, or `ingest.write`). On send:
`interpolateArgs(template, scope)` → `bridge.call(target, payload)`. A `bus.publish` is fire-and-forget —
the UI shows "published", never a fake "delivered".

## What shipped

- `ui/src/features/dashboard/builder/JsonPayloadField.tsx` — the authoring surface: a target `<select>`
  (`payloadTargets`: `bus.publish` + `ingest.write` + each installed extension's WRITE tools from
  `extensionEntries`), a subject input (for `bus.publish`), the JSON template editor (reuses the Slice-B
  `CodeEditor`, JS mode highlights JSON), and a Send button. On send: `JSON.parse` → `interpolateArgs(.,
  scope)` (type-preserving) → a leashed `makeWidgetBridge([target]).call(target, args)` (`bus.publish`
  wraps `{subject, payload}`; other tools take the payload directly). Status reads "published" for a
  fire-and-forget publish, "sent" for a write, or the real error — never a fake delivery.
- `ui/src/features/dashboard/builder/CellSettings.tsx` — a control cell (button/switch/slider) gains a
  "JSON payload" section in the ⚙ drawer (the natural authoring home).

## Decisions

- **One sink leash.** The send goes through the SAME `makeWidgetBridge` every tool rides — leashed to the
  chosen target, host-re-checked. A target outside the cell's tool set ∩ grant is denied server-side.
- **No fake delivery (rule 3).** `bus.publish` is fire-and-forget; the UI says "published" (handed to the
  bus), never "delivered". A must-deliver effect targets a write tool that enqueues to the outbox.
- **Reuse the shared lib.** The template runs through `interpolateArgs` — the same engine cells/controls
  use; `${var}`/`{{value}}`/`${__workspace}` all resolve, type-preserved, unknown-left-literal.

## Tests + green output

Unit — `vitest run jsonPayload.test.ts`: **4 passed** (`payloadTargets` offers the platform sinks + an
ext's write tools, not its reads; the add-todo template `{text:"${newTodo}",ws:"${__workspace}"}` resolves
type-preserved; an unknown slot left literal).

Real-gateway — `busBridge.gateway.test.tsx` Slice-5 case: **passed** — the add-todo template interpolates
(`{text:"buy milk", ws:<ws>}`) and **sends over `bus.publish` end to end** → `{ok:true}` (published,
host-walled + gated). A target outside the cell set is bridge-rejected; a reserved subject is host-refused
(the earlier cases in the same file).

## Mandatory categories

- **Capability deny:** the send is leashed (`makeWidgetBridge([target])`) + host-re-checked; a target
  outside `cell.tools ∩ grant` is denied (proven in the bus-bridge deny case + `store_query`/widget tests).
- **JSON payload e2e:** build `{text:"${newTodo}"}` → a real sink (`bus.publish`) → `{ok:true}`, the deny
  when ungranted. (A `todo.add`-style write tool isn't shipped; `bus.publish` is the real, proven sink —
  the same path a `todo.create` would take.)

## Follow-ups

`sql.generate`/AI assist and richer multi-value forms remain named follow-ups. This completes the
widget-config-vars scope's five slices + the bus platform fix.
