# Flow timestamp display scope — canonical `ts` in, prefs-localized wall-clock out

Status: **scoped, building.** A thin slice that connects two already-shipped systems — the
**`lb-prefs` formatter** (`format.datetime`, [`user-prefs-scope.md`](./user-prefs-scope.md)) and the
**dashboard field-config bridge** ([`viz-fieldconfig`](../frontend/), `fieldconfig/format.ts`) — so a
flow node's timestamp field (`ts`, `cron_ts`, any epoch field a node emits) renders in the *viewer's*
resolved timezone + date/time style instead of a raw epoch integer. No new storage, no new formatter,
no new hint schema on the flow: the value stays canonical on the wire and the presentation is resolved
at the read boundary, exactly as [`user-prefs-scope.md`](./user-prefs-scope.md) froze.

One paragraph: the flow engine stamps instants as **epoch seconds** (the whole flow clock is
`SystemTime … .as_secs()` — [`gateway/state.rs`](../../../rust/role/gateway/src/state.rs) `now()`,
[`flows/react_cron.rs`](../../../rust/crates/host/src/flows/react_cron.rs) `scheduled_ts`). A node's
output value is a canonical envelope like `{payload: 99, ts: 1782864300}`; `flows.node_state` returns
it verbatim. When a dashboard cell binds a **timestamp field** of that value, the viz layer must render
it as wall-clock time for the viewer — `01/07/2026 14:45` for a `Europe/Madrid`/EU user,
`07/01/2026 08:45` for an `America/New_York`/USA user — from the *same* canonical `1782864300`. The
mechanism already exists (`format.datetime`); this scope wires it in and fixes the one real hazard
(seconds vs milliseconds).

## Goals

- **Render a bound flow timestamp through `format.datetime`, in the viewer's resolved prefs.** Same
  canonical instant, per-viewer wall-clock — the [`user-prefs-scope.md`](./user-prefs-scope.md) example
  flow, applied to a flow node port instead of an ingest series.
- **Reuse the ONE viz formatting bridge, don't fork it.** `fieldconfig/format.ts` is *already* the
  single canonical-in / localized-out seam for the viz layer, *already* has a `datetime` `UnitKind`, and
  its own comment says "when `lb-prefs` ships, this function becomes the real `format.*` dispatch." That
  swap is this scope. A flow read view formats a timestamp field the *same way* a stat cell formats a
  temperature — through `formatValue`, never a local `new Date()`.
- **Fix the seconds↔milliseconds hazard explicitly, never by magnitude-guessing.** `format.datetime`
  takes epoch **ms**; the flow clock is epoch **seconds**. The unit is *known from the source*, so it is
  carried as data (a `dateUnit: "s" | "ms"` on the field mapping), never inferred from digit count
  (which breaks for pre-2001 and post-2033 instants).
- **Opt-in, per bound field.** A timestamp renders as a date only when the field's unit says so
  (`dateTimeAsIso`, `time:*`, or a flow-timestamp mapping). Absent that, the canonical number renders
  unchanged — honoring [`user-prefs-scope.md`](./user-prefs-scope.md)'s "no forcing all payloads through
  a formatter."

## Non-goals

- **No formatting inside the flow engine or the store.** `flows.node_state` keeps returning canonical
  values; a formatted string in a `flow_node_state` record would be the exact bug §3 forbids. Formatting
  is a *read-boundary presentation*, resolved from the viewer's prefs — which the engine does not know.
- **No new per-field hint schema on the flow / node value.** The "render this field as a date" intent
  lives on the **dashboard cell's `fieldConfig`** (where every other per-field display option already
  lives), not on the flow. The flow is a shared object; presentation is per-viewer, per-panel.
- **No second formatter.** We do not add a flow-specific date renderer. If `format.datetime` cannot yet
  express a pattern (localized month *names* are the deferred icu path), the numeric styles it *does*
  ship are what we use — same limitation the whole prefs slice already accepts.
- **No epoch-unit auto-detection.** Explicitly rejected (see Risks): guessing seconds-vs-ms from the
  integer magnitude is a latent correctness bug. The unit is declared.

## Intent / approach

**The value is canonical; the render is `formatValue(canonical, fieldOptions)`.** Three touch-points,
all on already-existing seams:

1. **`fieldconfig/format.ts` — make the `datetime` case call `format.datetime` (the shipped swap).**
   Today the `datetime` branch of `fallbackFormat` does `new Date(value).toISOString()` — which (a)
   ignores the viewer's tz/style and (b) assumes **ms**, so it renders a flow epoch-seconds `ts` as
   1970. Replace it with a real `format.datetime` MCP call in the viewer's resolved prefs, normalizing
   the instant to ms per the field's declared `dateUnit`. This is the swap the file was built to accept
   — the call site (`formatValue`) and the `fieldConfig` schema are unchanged, so no cell re-save.

2. **`fieldconfig/units.ts` — teach the mapping table the flow-second timestamp + carry `dateUnit`.**
   `UnitMapping` gains an optional `dateUnit: "s" | "ms"` (default `ms`, back-compat with the existing
   `dateTimeAsIso`/`time:*` ms entries). Add a flow-timestamp unit id (e.g. `time:flow-seconds`, label
   "date (flow)") whose `dateUnit` is `"s"`, so binding a flow `ts`/`cron_ts` field picks up the ×1000
   normalization declaratively. The table stays the single source of truth (§8: one responsibility).

3. **The flow read views consume the bridge for a timestamp-bound field.** `StatPanel` already routes
   its value through `formatValue` — so a stat cell bound to `ts` with a datetime unit *already* flows
   through the fixed path once (1)+(2) land. `JsonView` pretty-prints the whole JSON (correct — it is a
   JSON view, not a scalar view); a timestamp there is shown canonically, and a viewer who wants the
   wall-clock binds a **stat/gauge** scalar cell to the `ts` field (the natural viz for one instant).
   The picker, when a bound leaf looks like a timestamp (`ts` / `*_ts` / `*_at`), defaults the field
   unit to the flow-seconds datetime mapping (a suggestion the author can override) — the one small UX
   affordance, in `FlowsQuerySection` / the field tab.

**Why the formatting resolves server-side (the user's instinct, and the scope's rule).** `format.datetime`
is a **host MCP tool** — the tz math + DST rules (`chrono-tz`) + the resolved-prefs chain live in
`lb-prefs`, compiled into every node. The client calls the tool; it does **not** re-implement timezone
arithmetic. This is [`user-prefs-scope.md`](./user-prefs-scope.md)'s "backend-mediated presentation as
MCP tools so every client is identical and thin clients are free." The viz bridge is the *call site*;
`lb-prefs` on the node is the *implementation*. A rich client that already bundles the same data may
still format locally behind the same `formatValue` signature — but it is never forced to.

**Why on the field-config bridge and not folded into `flows.node_state` (the rejected placement).**
`flows.node_state` returns *every* node's *whole* value with no notion of which field a given dashboard
cell binds or how the viewer wants it shown. Folding a `display` projection there would force the flow
verb to either (a) speculatively format every field of every node (it cannot know which integers are
timestamps, and formatting non-timestamps is the canonical-corruption bug), or (b) accept the cell's
per-field render hints as call arguments — coupling a shared flow verb to one consumer's presentation.
Presentation belongs to the panel, keyed by the panel's `fieldConfig`; the read verb stays canonical
and consumer-agnostic. So the projection happens in the viz layer, at `formatValue`, where every other
per-field display decision already lives.

**Rejected alternatives:**
- *Format in the flow engine / store a formatted `ts`.* Rejected — §3 canonical-only; un-reconvertible,
  wrong the moment a second viewer with different prefs reads the same node, breaks the moment a user
  changes their timezone.
- *A flow-specific `__flowFormat` hint on the source args.* Rejected — it duplicates the `fieldConfig`
  per-field option mechanism that already exists for exactly this (unit/decimals/mappings), forking the
  "how is this field displayed" decision into two schemas the editor would have to keep in sync.
- *Auto-detect seconds vs ms from the integer size.* Rejected — a magnitude heuristic is a latent
  correctness bug (fails for historical and far-future instants); the source unit is known, so it is
  declared as data.
- *A new client-side date formatter (`date-fns`/`Intl` in the view).* Rejected — forks the one prefs
  formatter, drifts from the server-rendered notification string for the same instant, re-implements tz
  math the host already owns. The whole point of `format.datetime` is one implementation.

## How it fits the core

- **Tenancy / isolation:** none new — `format.datetime` touches no tenant data (the grant-free utility
  tier); the instant is passed in, the prefs are the *caller's* resolved record (workspace-walled by the
  session token like every read). The flow value read (`flows.node_state`) is already ws-walled.
- **Capabilities:** none new. `format.datetime` is the grant-free utility tier
  ([`user-prefs-scope.md`](./user-prefs-scope.md) resolved decision); `flows.node_state` keeps its
  existing `flows.node_state:call` gate. The mandatory deny/isolation coverage rides the existing flow +
  prefs suites; this slice adds no gated verb.
- **Placement:** `either` — pure formatting on any node, fully offline (`chrono-tz` compiled in), no role
  branch. Symmetric.
- **MCP surface:** **no new verb.** Reuses `format.datetime(instant, opts?)` and `flows.node_state{id}`
  exactly as shipped. The client composes them at the render boundary.
- **Data (SurrealDB):** none new. `fieldConfig` (already on the cell) carries the field unit; the flow
  value stays canonical.
- **Bus (Zenoh):** none.
- **Sync / authority:** none new — prefs are already cloud-authoritative shared data with an edge
  read-cache; a flow value is already node-local state read canonically.
- **Secrets:** none.

## Example flow

The `counter-2` node emits `{payload: 99, ts: 1782864300}` (epoch **seconds**) each scan:

1. **Author** binds a **Stat** cell to `counter-2 › payload (output)`, drills the JSON path to `ts`, and
   the field unit defaults to the flow-seconds datetime mapping (the picker's timestamp suggestion; the
   author can switch it to a plain number).
2. **Viewer A** (`prefs: tz Europe/Madrid, date EU, 24h`) opens the dashboard. `StatPanel` extracts the
   canonical `1782864300`, calls `formatValue(1782864300, { unit: "time:flow-seconds" })`; the bridge
   normalizes ×1000 (`dateUnit: "s"`) and calls `format.datetime(1782864300000)` → resolves A's prefs →
   **`01/07/2026 14:45`**.
3. **Viewer B** (`prefs: tz America/New_York, date USA, 12h`) views the same cell. Same canonical
   `1782864300`; `format.datetime` resolves B's prefs → **`07/01/2026 08:45 AM`**. Zero client-side tz
   math; the flow value is byte-identical for both.
4. A JSON-view cell bound to the whole `payload` still shows canonical `{payload: 99, ts: 1782864300}` —
   the raw envelope, unformatted, because a JSON view is a canonical inspector (opt-in principle holds).

## Testing plan

Mandatory categories from [`scope/testing/testing-scope.md`](../testing/testing-scope.md):

- **Capability deny** — no new gated verb; the existing `flows.node_state` deny test and the `prefs`
  deny suite cover the surfaces. Assert `format.datetime` needs no grant (utility tier) — already covered
  by the prefs suite; referenced, not duplicated.
- **Workspace isolation — specified.** Two workspaces, the *same* global user with **different** `tz`
  prefs in each (`user_prefs[ws-A]=Europe/Madrid`, `user_prefs[ws-B]=America/New_York`); the *same*
  canonical flow `ts` renders in **each workspace's** tz and never reads the other's prefs record. A
  one-workspace test would pass through a leak — disallowed. Across store + MCP (real gateway).
- **Offline / sync** — no new state; the prefs-offline replay test already covers a tz edit applying
  offline. Referenced.

Key unit/integration cases:

- **Seconds↔ms normalization (the bug-prone part)** — `formatValue(1782864300, {unit:"time:flow-seconds"})`
  renders **2026**, not 1970; a `ms`-unit datetime field with `1782864300000` renders the same instant.
  A pure `format.ts` unit test pins both, so the ×1000 can never regress silently.
- **Per-viewer render** — the SAME canonical instant → two different wall-clock strings under two
  resolved prefs (EU/Madrid/24h vs USA/NY/12h), incl. the correct date field order and meridiem. Real
  gateway (`format.datetime` over seeded `user_prefs`).
- **DST boundary** — an instant on each side of a DST transition renders the right offset (delegated to
  `lb-prefs`'s `chrono-tz` DST test; asserted once here through the flow path to prove the wiring, not
  re-testing `chrono-tz`).
- **Canonical guarantee** — after a render, `flows.node_state` still returns the canonical
  `{payload, ts}` with `ts` an integer; no formatted string is persisted (the §3 lint on the flow path).
- **Opt-out** — a `ts` field with a plain-number unit renders the raw integer (no accidental
  auto-formatting); a JSON-view cell shows the canonical envelope.
- **End-to-end through the grid** — a Stat cell exactly as the PanelEditor saves it (v3 `sources[]`, a
  `ts` path, the flow-seconds datetime unit), rendered through `WidgetHost`, shows the viewer's
  wall-clock — the real render path, not a hand-built value (the lesson from the
  [`flow-dashboard-binding-ux`](../flows/flow-dashboard-binding-ux-scope.md) "binding broken" regression:
  test the exact saved shape through the grid host, including the empty v2 `source` placeholder the
  gateway round-trips).

## Risks & hard problems

- **Seconds vs milliseconds is THE hazard.** The flow clock is epoch seconds; `format.datetime` is ms;
  the existing `datetime` fallback (`new Date(value)`) silently assumes ms and renders a flow `ts` as
  1970. The fix is declaring `dateUnit` on the field mapping and normalizing once in `format.ts` — never
  a magnitude guess. This is the one place to get exactly right; a unit test pins it.
- **`format.datetime` is async; `formatValue` is sync.** The viz bridge is a pure synchronous function
  today (renderers call it inline). A real MCP call is async. Resolve this the way the field-config scope
  anticipated: the panel data hook (`usePanelData` / the view's effect) fetches the *resolved prefs*
  once per render (cheap, cached per session) and passes them into `formatValue`, which then calls the
  **pure** `format_datetime(instant, tz, date_style, time_style)` shape synchronously — the same pure
  function `lb-prefs` exposes. I.e. resolve-prefs is the async step (hoisted to the hook), formatting
  stays sync. This keeps every renderer's call site unchanged and avoids a formatter round-trip per
  value. (Pin in the session doc which of the two — a WASM/pure client port of `format_datetime`, or a
  batched `format.datetime` MCP call — ships first; the signature is identical either way.)
- **JSON view shows canonical `ts`.** Intentional (a JSON view is a canonical inspector), but a user may
  expect the date there too. The scope's answer: bind a scalar (stat/gauge) cell for a formatted instant;
  the JSON view stays honest about the stored shape. Revisit only if users ask for per-leaf formatting
  inside the JSON tree (a bigger feature — per-node render overrides).
- **Localized month names not yet available.** `format.datetime` ships numeric styles only (icu月/day
  names deferred). A `time:flow-seconds` field renders `01/07/2026 14:45`, not `1 Jul 2026`. Accepted —
  same limitation the whole prefs slice carries; the icu swap upgrades it behind the same call.
- **Author must pick the timestamp unit.** The picker's "looks like a timestamp → suggest datetime"
  heuristic (`ts`/`*_ts`/`*_at`) is a convenience, not a guarantee; a differently-named epoch field needs
  a manual unit pick. Acceptable — the field-config unit picker is the explicit, discoverable control.

## Resolved decisions

- **The render happens at `fieldconfig/format.ts`, the one viz bridge — not in the flow verb, not in a
  new formatter.** This is the swap point the field-config scope pre-committed to; `lb-prefs` shipping is
  the trigger.
- **The "render as date" intent lives on the cell's `fieldConfig` unit, not on the flow / node value.**
  Presentation is per-panel, per-viewer; the flow is shared and canonical.
- **The epoch unit is declared (`dateUnit`), never detected.** Flow fields are `s`; the existing
  `dateTimeAsIso`/`time:*` entries stay `ms` (default). No magnitude heuristic, ever.
- **`format.datetime` resolves the VIEWER's prefs** (server-mediated / pure-fn behind the same
  signature); no client re-implements tz math. Rich clients may format locally behind `formatValue`, but
  are never required to.
- **No new MCP verb, no new capability, no new storage.** The slice is pure wiring of two shipped
  systems plus the seconds↔ms fix.

## Related

- [`user-prefs-scope.md`](./user-prefs-scope.md) — the `format.datetime` verb + the resolution chain +
  the canonical-in/localized-out contract this slice consumes.
- [`../public/prefs/prefs.md`](../../public/prefs/prefs.md) — the shipped prefs surface.
- [`../flows/flow-dashboard-binding-ux-scope.md`](../flows/flow-dashboard-binding-ux-scope.md) — the
  flow→node→port picker + the read-back path this timestamp binding extends; the "binding broken"
  regression whose lesson (test the real saved shape through the grid host) the testing plan carries.
- [`../flows/flow-persistent-runtime-scope.md`](../flows/flow-persistent-runtime-scope.md) —
  `flows.node_state`, the canonical value read this renders (kept canonical).
- `ui/src/features/dashboard/fieldconfig/format.ts` + `units.ts` — the viz formatting bridge + the unit
  mapping table this slice extends (the `datetime` swap + `dateUnit`).
- README **§3** (canonical-not-formatted, capability-first, one datastore), **§6.5**/**§3.7** (MCP is the
  universal contract — `format.datetime` is a tool), **§6.8** (prefs cloud-authoritative), **§10**
  (minimal profile — `chrono-tz` compiled in, no data-size cost).
</content>
</invoke>
