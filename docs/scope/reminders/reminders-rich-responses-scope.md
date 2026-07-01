# Reminders scope — reminders as the first rich channel command/response

Status: **SHIPPED** — the corrected, descriptor-driven contract: `reminder.create`'s **descriptor** declares
the flat form; the **verb** builds the `Action` server-side; `reminder.list`'s **descriptor** carries
`result` = the table+row-controls render; the **generic palette** posts `descriptor.result`. The reminder
feature ships **entirely from backend descriptors** — the UI has no reminder knowledge. Kept a scope doc as
the authoritative spec of *why*; promotes to `public/reminders/`.
Topic: `reminders`. Builds on the **shipped** `reminders-scope.md` (the `reminder.*` MCP surface + record +
reactor + `RemindersView`) and — load-bearing — `channels/channels-rich-responses-scope.md` (the contract:
a `ToolDescriptor` carries **both** `input_schema` (the FORM) and `result` (the RESPONSE render); the palette
is a **generic interpreter** that names only `tools.catalog`, renders schema widgets by string, and posts
`descriptor.result` — with **no tool-name branch**). This scope is the **first concrete tenant** of that
contract: it proves the whole request-form / rich-response / control loop end to end against a real,
already-built MCP surface — with the UI holding **zero** reminder-specific knowledge.

We want to **drive reminders entirely from a channel** — `/remind` opens a backend-declared **form** (a
cron-builder + an action picker) whose flat fields the **`reminder.create` verb** shapes into an `Action`
server-side, and `/reminders` answers with an **interactive list** the **`reminder.list` descriptor
declares** (`result` = a table + inline **pause/resume**, **run-now**, and **delete** row controls calling
`reminder.update`/`reminder.fire`/`reminder.delete`). Every pixel is rendered by the **shipped** dashboard
widget renderers (`WidgetView`/`views/*`) over the host-mediated bridge, leashed to the viewer's grant. The
**channel UI has no reminder knowledge at all**: it names one tool (`tools.catalog`), renders
`reminder.create`'s `input_schema` widgets, and posts `reminder.list`'s `descriptor.result` — the same
generic path every command takes. Reminders is the proof that a real feature's whole CRUD surface ships
**from backend descriptors alone**, with **zero bespoke rendering and zero client-side per-tool code**.

## Goals

- **`/remind` = a backend-declared FLAT form; the verb shapes it.** The `reminder.create` **descriptor's**
  `input_schema` (with `x-lb` widget hints) declares a **flat** shape — a **cron** field, an **action-kind
  `select`**, the action's fields, and optional `max_runs`/`enabled`. The palette renders it by string and
  posts the collected fields **verbatim**; the **`reminder.create` verb** does the shaping — it takes the
  flat `schedule` + `action_kind` + fields and builds the nested `Action` **server-side**. The form is the
  descriptor; the shaping is the verb; the UI reshapes **nothing**.
- **`/reminders` = a descriptor-declared, interactive response.** The `reminder.list` **descriptor** carries
  `result` = a `{ v, view:"table", source:{ tool:"reminder.list" }, options:{ … row controls … } }` render
  envelope; the **generic palette posts `descriptor.result`** and the channel mounts the shipped `table`
  view. Each row carries **control** actions (`switch`→`reminder.update{enabled}`, a **run-now**
  button→`reminder.fire`, a **delete** button→`reminder.delete`) that call the write verbs through the
  bridge, gated + ws-scoped. The palette does **not** assemble this render — it reads it off the descriptor.
- **Reuse, not rebuild — and no client per-tool code.** Data + writes ride `reminder.list/create/update/
  fire/delete` (shipped) through the shipped `bridge.call`; rendering rides the shipped `WidgetView`; the
  request/response contract is `channels-rich-responses`. The **only** new code is **backend data**: the
  `reminder.create` descriptor's `input_schema` (+ the verb's flat→`Action` shaping) and the `reminder.list`
  descriptor's `result` render envelope. No new renderer, no reminder-aware channel component.
- **Security identical to `RemindersView`.** Every bridged call is `reminder.<verb>` under the **viewer's**
  grant + workspace (from the token), re-checked at the host — the same gates the shipped view and CLI hit.
  A viewer without `mcp:reminder.list:call` never sees the command (catalog "absent") and can't render the
  list; without `mcp:reminder.update:call` the pause toggle is denied server-side.
- **A `fire`/run-now read verb, if missing.** The response's "run now" control needs a `reminder.fire`
  *tool* (dispatch one firing immediately). The reactor already has the internal fire path; if a
  capability-gated `reminder.fire` verb isn't exposed, add it (bounded, one firing, idempotent) — the one
  possible new host verb this slice needs.

## Non-goals

- **No new render system, view, or trust tier.** This consumes `channels-rich-responses` (which consumes
  the widget-builder v2 contract). If the interactive list needs a view that doesn't exist, that's a
  widget-builder change, not this scope.
- **No reminder engine change.** The record, the cron `next_after`, the `react_to_reminders` scan, the
  fire dispatch, the action union — all shipped and unchanged. This is a **surface** slice.
- **No natural-language scheduling.** `/remind fri 5pm …` NL→cron is a later layer over the same command;
  v1 is the structured cron-builder form (the shipped `react-js-cron` builder, reused as the form's cron
  widget).
- **No new action kinds.** channel-post / mcp-tool / outbox are the shipped three; the form's action
  `select` offers exactly those.
- **No persistence change.** A `/reminders` response is ephemeral channel history (a `render` block in an
  `Item` body); the reminders themselves stay `reminder:{id}` records. Nothing new in the store.
- **No `*.fake.ts`.** Real gateway, real `reminder.*` verbs, real seeded reminders, real reactor — only the
  model-provider HTTP may ever be stubbed (and only if an *agent* creates a reminder; the plain command
  path involves no model).

## Intent / approach

**Reminders already has the whole backend — this slice only moves its front door into the channel.** The
shipped `RemindersView` proves the loop: `reminder.list` → a table with a pause switch + an action editor
+ a cron builder, all over the MCP bridge. The `channels-rich-responses` contract says that same loop is
expressible as **the `ToolDescriptor` carrying both halves** — `input_schema` (the create FORM) and `result`
(the interactive-list RESPONSE render) — rendered by the shipped `WidgetView`, posted by the **generic**
palette with no reminder-specific client code. So the whole slice is **backend descriptors**:

- **Request side (the form) — a flat descriptor + a shaping verb.** The `reminder.create` **descriptor's**
  `input_schema` declares a **flat** form:
  - `schedule` → `{ x-lb:{ widget:"cron" } }` — the `cron` widget in the (open, string-keyed) palette
    registry, wrapping the **already-shipped `CronBuilder`** (`react-js-cron`) reused as an arg widget.
  - `action_kind` → `{ x-lb:{ widget:"select", options:["channel-post","mcp-tool","outbox"] } }` — an
    inline-enum `select` (the sibling of the `source:"<tool>"` select; here the options are static).
  - the action's fields (channel id / tool+args / outbox effect), `max_runs?`, `enabled?` — **flat**.
  Catalog visibility gates on `mcp:reminder.create:call`. The palette posts these **flat fields verbatim**;
  the **`reminder.create` verb builds the nested `Action` server-side** from `action_kind` + fields (the UI
  never assembles the `Action` — that reshaping was the leak the correction removes). The create's
  confirmation is the descriptor's own `result` (a small `view:"stat"|"table"` — "reminder scheduled: next
  Mon 08:00"), posted by the generic path.
- **Response side (the list) — the descriptor's `result`.** The `reminder.list` **descriptor** (gated
  `mcp:reminder.list:call`) carries `result` = a render envelope: `view:"table"`,
  `source:{tool:"reminder.list"}`, and per-row **control** actions in `options` — the shipped control views
  (`switch`/`button`) bound to `reminder.update`/`reminder.fire`/`reminder.delete`, the row `id` templated
  in from the row object (below). The **generic palette posts `descriptor.result`**; `ResponseView` mounts
  it via the shipped `WidgetView`; the bridge leashes the writes to the viewer's grant. The palette does
  **not** hardcode this list render — it reads it off the descriptor like any other command's.
- **Reused wholesale:** `WidgetView`, the `table`/`switch`/`button`/`stat` renderers, `widgetBridge`, the
  palette arg-rail, the `CronBuilder`, `react_to_reminders`, and every `reminder.*` verb except a possible
  new `reminder.fire`.

**Why reminders is the right first tenant.** It has a **complete, shipped, tested** CRUD + list + control
surface and an existing reference UI (`RemindersView`) to diff against — so the slice proves the
rich-responses contract end to end (a real form with a real custom widget `cron`, a real interactive list
with real write controls, real deny/isolation) **without** any speculative backend. If the channel can
fully drive reminders with zero bespoke rendering, the contract is real; if it can't, we learn exactly
where the contract is thin — cheaply, against known-good verbs.

**Rejected alternatives:**

- *Let the channel assemble the `reminder.create` call or hardcode the `/reminders` list render.* Rejected —
  that is the design flaw the `channels-rich-responses` correction removes: it forks a second command system
  into the client, breaks rule 7, and means each reminder change is a UI change. The **descriptor** carries
  the flat form (`input_schema`) AND the list render (`result`); the **verb** shapes the flat fields into the
  `Action`; the UI is a pure schema+render interpreter that never names `reminder.*`.
- *A bespoke `/remind` handler in the channel (parse text → `reminder.create`).* Rejected — it is the
  orphaned-`/agent` mistake again (a second command system beside the palette). The command is a
  descriptor; the form is its `input_schema`.
- *Embed the whole `RemindersView` as a channel iframe/page.* Rejected — that ships a reminders-specific UI
  into the channel; the point is that reminders render through the **generic** widget contract, so *any*
  feature's list can. (A full page stays the nav-surface `RemindersView`.)
- *Add reminders-specific channel `Item` kinds (`reminder_result`, `reminder_list`).* Rejected — the exact
  response-kind-proliferation the rich-responses scope collapses into one `render` block.

## How it fits the core

- **Tenancy / isolation (rule 6):** `reminder.*` is already workspace-walled from the token; the `/remind`
  form's create and the `/reminders` list/controls run under the **viewer's** ws (from their token, never
  the `Item` body or an iframe message). A ws-B viewer rendering a `/reminders` response sees only ws-B
  reminders and can pause/fire/delete only ws-B — the shipped verb wall holds through the rendered
  response. **Mandatory two-session test.**
- **Capabilities (rule 5/7):** no new capability for the surface — it *calls* the shipped `mcp:reminder.
  create|list|update|delete:call` (+ a new `mcp:reminder.fire:call` if `fire` is added). The command's
  catalog visibility gates on the matching verb (create/list); a row control's write is gated by
  `reminder.update`/`delete`/`fire`. **Deny path is a headline test:** a viewer with `list` but not
  `update` sees the table but the pause toggle is denied server-side (opaque), and the `/remind` command is
  absent for a viewer without `create`.
- **Placement (rule 1):** `either`. The command/response are data in channel `Item`s; the reactor that
  actually fires the reminder later is the shipped `react_to_reminders` on whichever node hosts it. No role
  branch.
- **MCP surface (§6.1) — the API shape:**
  - **CRUD:** reused — `reminder.create`/`update`/`delete` (shipped). The form calls create; the list's
    controls call update/delete.
  - **Get / list:** reused — `reminder.list` is the response `source`; `reminder.get` backs a "show one"
    follow-up. Read caps unchanged.
  - **Live feed:** N/A for v1 (a `/reminders` response is a point-in-time table; re-run to refresh). A
    live-updating reminders card (watch reminder changes) is a named follow-up, not this slice — stated.
  - **Batch:** N/A — one form creates one reminder; one control acts on one row. No long batch.
  - **New verb (maybe one):** `reminder.fire` — dispatch **one** firing now (bounded, idempotent on
    `(reminder_id, now)`, the run-now control's target). Add only if not already exposed; it is a small,
    synchronous, gated verb, not a job (the fire itself may enqueue a job internally, as the reactor does).
- **Data (SurrealDB):** none new. Reminders stay `reminder:{id}`; the command/response live in the channel
  `Item` body (durable history). Inline render data (the create-confirmation stat) is bounded.
- **Bus (Zenoh):** the `/reminders` response + the create confirmation are channel motion (fire-and-forget
  over the shipped channel SSE). The reminder's **own** firing (a must-deliver outbox action) already goes
  through the **outbox** inside the reactor — unchanged; the rendered control only *schedules/toggles*, the
  durability is the reminder engine's.
- **Sync / authority:** the response `Item` is a `(table,id)` upsert on the shipped §6.8 channel sync path;
  a `/reminders` table re-renders on a reconnecting edge by re-running `reminder.list` (fresh). The
  reminder records sync on their own shipped path.
- **Secrets:** none reach the renderer. A reminder whose action needs a secret pulls it server-side in the
  fire handler (shipped); the form/list never touch it.
- **SDK/WIT impact — minor, additive.** No new stable boundary of its own: it *uses* the
  `channels-rich-responses` boundaries — the `ToolDescriptor.result` (`x-lb-render`) envelope and the open,
  string-keyed `x-lb.widget` vocabulary. This slice is **pure backend descriptor data**: the
  `reminder.create` descriptor's `input_schema`, that verb's flat→`Action` shaping, and the `reminder.list`
  descriptor's `result` render envelope. It **contributes two widget strings** to the open registry — `cron`
  (wrapping the shipped `CronBuilder`) and the static-`options` `select` — both resolved by string, additive,
  `v`-stamped, `unknown→text` fallback applies. Because the vocabulary is open (built-ins ∪
  `ext:<id>/<widget>`), even these could be extension-contributed; here they are built-ins. Flag: the
  possible new `reminder.fire` verb is a normal gated MCP verb (no boundary change).

## Example flow

1. **Schedule via `/remind`.** Ada types `/` → picks **Remind** (visible because she holds
   `mcp:reminder.create:call`). The arg rail shows a **cron builder** (`x-lb:{widget:"cron"}`, the shipped
   `react-js-cron`), an **action** `select` (`channel-post`/`mcp-tool`/`outbox`), the action's fields, and
   `max_runs?`. She builds "every Mon & Sun 08:00", action = post "standup" to `#ops`, submits. The palette
   calls `reminder.create` through the bridge (her token, her ws); a small `render:{view:"stat"}` confirms
   "scheduled — next Mon 08:00" in history.
2. **List via `/reminders`.** Ada types `/reminders`. The `reminder.list` **descriptor's `result`** is
   `{ view:"table", source:{tool:"reminder.list"} }` with per-row controls; the generic palette posts it and
   the shipped `table` view renders her workspace's reminders. Each row (supplied as the control's
   `VarScope.values`) has a **pause switch** (`reminder.update{ id:${id}, enabled:{{value}} }`), a **run-now**
   button (`reminder.fire{ id:${id} }`), and a **delete** button (`reminder.delete{ id:${id} }`) — `${id}`
   the shipped row-field var, `{{value}}` the shipped interaction slot.
3. **Toggle a reminder.** Ada flips the pause switch on one row → the bridge calls `reminder.update` under
   her grant + ws → the host re-checks `mcp:reminder.update:call` + workspace → the reminder pauses. The
   shipped control view, unchanged.
4. **Run now.** Ada clicks run-now → `reminder.fire{id}` dispatches one firing immediately (idempotent);
   the reminder's action executes through its shipped seam (channel post / tool / outbox).
5. **Deny.** Bob holds `reminder.list` but not `reminder.update`. His `/reminders` renders the table, but
   flipping a pause switch is **denied server-side** (opaque) — the bridge filter and the host agree. The
   `/remind` command is **absent** from his palette (no `create` grant) — no existence leak.
6. **Isolation.** Cleo (a `mcdonalds` session) renders `/reminders`; the `reminder.list` source runs as
   `mcdonalds` (her token) → only `mcdonalds` reminders; she cannot fire/delete an `acme` reminder even if
   an id leaked into a control's args (host re-checks the ws). The wall holds through the rendered controls.
7. **Agent schedules a reminder (optional).** An in-channel agent, asked "remind the team every Monday,"
   emits a `reminder.create` call (or a `/remind`-shaped structured response) under its derived
   `agent ∩ caller` grant — same verb, same gates, no special path.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real gateway, real `reminder.*` verbs + reactor,
real seeded reminders, two real sessions; **no `*.fake.ts`**:

- **Capability deny — per verb.** `/remind` command absent without `mcp:reminder.create:call`; the
  `/reminders` table renders with `list` but a pause toggle is **denied server-side** without
  `mcp:reminder.update:call`; run-now denied without `reminder.fire`; delete denied without
  `reminder.delete`. Deny is opaque; assert the host denies even if the bridge filter were bypassed.
- **Workspace isolation — two sessions.** A ws-B viewer's `/reminders` lists only ws-B reminders and can
  toggle/fire/delete only ws-B; a control whose args name a ws-A reminder id is denied at the host (ws from
  the token). The two-principal test extended to the rendered controls.
- **Token never crosses the boundary.** The session token appears in no bridge arg or iframe payload for
  the create form, the list source, or any row control (reuse the rich-responses assertion).
- **Offline / sync.** The `/reminders` response `Item` replays idempotently on the §6.8 channel path; a
  reconnecting edge re-renders by re-running `reminder.list` (fresh). Reminder records sync on their shipped
  path.
- **Versioning / degradation.** An older UI that doesn't know the `cron` widget falls back to a text input
  that still round-trips the cron string (the `unknown→text` fallback); an unknown row-control view
  degrades honestly.

Plus this slice's cases (real gateway):

- **Create round-trip:** `/remind` → build a cron in the `cron` widget → pick each action kind → submit →
  a real `reminder:{id}` exists (assert via `reminder.get`) with the built cron + action; the confirmation
  `stat` renders.
- **List round-trip:** seed N reminders → `/reminders` → the `table` view renders all N with the right
  columns (next-fire, action, enabled, max_runs) from `reminder.list`.
- **Control e2e:** pause a row → `reminder.get` shows `enabled:false`; run-now → the action's side effect
  occurs (e.g. a channel post appears / an outbox effect enqueues); delete → the reminder tombstones and
  drops from a re-rendered list. Each asserted against the real verb, gated + ws-scoped.
- **`reminder.fire` verb (if added):** deny without its cap; one firing dispatches exactly one action
  (idempotent on a re-click within the same instant); ws-isolation.
- **Parity with `RemindersView`:** the channel `/reminders` controls drive the same verbs the shipped view
  does (a regression guard that the surface didn't fork the contract).

## Risks & hard problems

- **The row-control render must bind the reminder **id** safely — RESOLVED, no `argsTemplate` extension.**
  Each row's pause/fire/delete control needs the row's `id` in its args plus the interaction value for the
  switch. **Resolved:** the control template uses **`${id}`** (a **row field** — the shipped vars engine
  matches `${name}`) for the row id and **`{{value}}`** (the shipped interaction slot) for the switch state,
  with the **row object supplied as the control's `VarScope.values`**. No `argsTemplate` extension is
  needed — `${id}` resolves against the row and `{{value}}` against the interaction, both already shipped.
  Still load-bearing for the interactive list; test it against the real table view.
- **Deny must bite the write, not just hide the control.** Hiding a pause toggle for a viewer without
  `update` is UX; the guarantee is the **host** denying `reminder.update` even if the control fires. Test
  the real ungranted write, per the rich-responses headline.
- **`reminder.fire` idempotency.** Run-now must not double-fire on a double-click or a re-render; make it
  idempotent on `(reminder_id, instant)` exactly as the reactor's scan is. A run-now that double-posts is a
  bug, not a degraded mode.
- **The `cron` widget's antd styling in the channel.** `react-js-cron` is antd-based (the reminders scope
  kept antd out of the global theme). Reused as a palette arg widget it must stay style-contained (the
  shipped `CronBuilder` already wraps it) — don't leak antd into the palette theme.
- **Scope creep into a reminders page.** The temptation is to render the *whole* `RemindersView` in the
  channel. Hold the line: the channel gets the **command form** + the **list-with-controls response**; deep
  editing/authoring stays the nav-surface view. State the boundary.

## Open questions

Decisions taken so the build has no blocking gap; residuals are named follow-ups.

**Resolved (decisions taken):**

- **`/remind` and `/reminders` are palette command descriptors** (named `reminder.create` / `reminder.list`,
  catalog-gated on those caps), NOT bespoke channel handlers. The UI holds **zero** reminder knowledge —
  it names only `tools.catalog`, renders `input_schema` widgets, and posts `descriptor.result`. Decided.
- **`reminder.create` declares a FLAT `input_schema`; the verb shapes the `Action` server-side.** Decided.
  The UI posts the collected flat fields verbatim — no client-side assembly of the nested `Action`.
- **The list render lives on the descriptor as `reminder.list`'s `result`** (`view:"table"`,
  `source:{tool:"reminder.list"}`, per-row control views), posted by the **generic** palette and mounted by
  the shipped `WidgetView`. Decided — the palette does not hardcode it.
- **Two widget strings contributed to the OPEN registry:** `cron` (wraps the shipped `CronBuilder`) and the
  static-`options` `select`. Decided; resolved by string, additive, `unknown→text` fallback (built-ins here,
  but the vocabulary is built-ins ∪ `ext:<id>/<widget>`).
- **Row-id templating — RESOLVED, no `argsTemplate` extension:** `${id}` (the shipped `${name}` row-field
  var) for the row id + `{{value}}` (the shipped interaction slot) for the switch, via the **row object as
  the control's `VarScope.values`**. Decided.
- **Add `reminder.fire`** (a gated, idempotent, one-firing verb) if not already exposed, to back run-now.
  Decided.

**Named follow-ups (not Phase-1 blockers):**

1. **Live-updating `/reminders` card** — a `bridge.watch` over reminder changes (partial re-render vs
   re-mount). Reuses the rich-responses streaming follow-up; not v1.
2. **`/reminders show <id>`** — a single-reminder `get` response with a full editor (edit the cron/action
   inline). v1 is list + coarse controls (pause/fire/delete); inline edit is additive.
3. **NL scheduling** — `/remind fri 5pm …` → an NL→cron layer over `reminder.create` (a later parser, same
   verb).

(Row-id templating is **resolved above** — `${id}` row-field var + `{{value}}` interaction slot via the row
as the control's `VarScope.values`; no longer a follow-up.)

## Related

- `reminders-scope.md` — the **shipped** reminder record + `reminder.*` MCP surface + `react_to_reminders`
  reactor + `RemindersView` + `CronBuilder` this slice surfaces into the channel (consumed, unchanged).
- `channels/channels-rich-responses-scope.md` — the contract this is the **first tenant** of: the `render`
  block, the `x-lb` widget enum, the viewer-grant-leashed bridge, the fixed-vs-generative view split.
- `channels/channels-command-palette-scope.md` — the request surface the `/remind` form is a descriptor in.
- `scope/frontend/dashboard/widget-builder-scope.md` — the v2 `WidgetView`/`table`/`switch`/`button`
  renderers + `argsTemplate` control mechanism the list reuses (via rich-responses).
- `public/reminders/reminders.md` — the shipped reminders truth this promotes alongside on ship.
- README **§6.1** (API shape), **§6.9/§6.10** (jobs/outbox the reminder engine uses), **§6.13** (extension
  UIs / trust tiers), **§7** (tenancy), **§3** (rules 4/5/6/7).
