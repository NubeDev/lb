# Session — flow timestamp display (canonical `ts` → viewer's wall-clock)

Scope: [`scope/prefs/flow-ts-display-scope.md`](../../scope/prefs/flow-ts-display-scope.md).
Also fixed a live "binding broken" regression on the flow read path (below).

## The ask

A flow node emits `{payload, ts}` where `ts` is an epoch integer; a dashboard cell bound to that field
showed the raw number (or, for the `datetime` fallback, 1970). The user wanted it rendered as a
localized date/time **in the backend, per the user's preferences**.

## What shipped

The mechanism already existed — this slice **wired two shipped systems together** rather than building a
formatter:

- **`format.datetime`** (host MCP tool, `lb-prefs`) — resolves tz + date/time style, DST-correct via
  `chrono-tz`. Grant-free utility tier.
- **`fieldconfig/format.ts`** — the ONE viz canonical-in / localized-out bridge, whose own comment
  pre-committed to "become the real `format.*` dispatch when `lb-prefs` ships." That swap is this slice.

### Backend

- **`prefs.get`/`prefs.resolve`/`prefs.set` are now member-level** (`gateway/session/credentials.rs`).
  This was the real blocker: a member could build a dashboard + read a flow but was **denied**
  `prefs.resolve`, so it couldn't resolve the viewer's prefs to format anything. These verbs are
  structurally forced to the caller's own `sub` (a viewer can never name another user), so granting them
  widens nothing — same reasoning that made `format.*` grant-free. `prefs.set_default` stays admin-only.
  The host authorize logic + the prefs deny test are **unchanged** (still green) — only the dev claim set
  widened.

### Client (the composition at the render boundary)

- `lib/prefs/` — `resolve.ts` (`prefs.resolve`), `formatDatetime.ts` (`format.datetime`), `set.ts`
  (`prefs.set`), `prefs.types.ts` (`ResolvedPrefs`), `formatDatetimeCached.ts` (memoize by
  `(instant, tz, styles)`), `useResolvedPrefs.ts` (resolve once per workspace, cached, cleared on
  session change). New IPC verbs `prefs_resolve`/`prefs_set`/`format_datetime` in `ipc/http.ts` (+ a
  `putJson` helper).
- `fieldconfig/units.ts` — `UnitMapping` gains `dateUnit: "s" | "ms"`; new `time:flow-seconds` mapping
  (`dateUnit:"s"`, because the flow clock is epoch SECONDS — `gateway/state.rs` `now()` uses `.as_secs()`).
- `fieldconfig/format.ts` — the `datetime` fallback now honors `dateUnit` (×1000 for seconds) so even the
  pre-resolve render shows the right year, not 1970.
- `fieldconfig/useFormattedValue.ts` — the React seam: sync `formatValue` for numbers/quantities; the
  async prefs-resolved wall-clock for a `datetime` field (cached, so re-renders are synchronous).
- `views/stat/StatPanel.tsx` — routes its value through `useFormattedValue` (hook hoisted above the early
  returns so it's called unconditionally).
- `editor/tabs/FlowsQuerySection.tsx` — picking a `ts`/`*_ts`/`*_at` leaf SUGGESTS the flow-seconds
  datetime unit (only if the author hasn't set one — never clobbers).

### The seconds-vs-ms fix (the load-bearing hazard)

`format.datetime` takes epoch **ms**; the flow clock is epoch **seconds**. The unit is **declared**
(`dateUnit`), never guessed from magnitude (which breaks for pre-2001 / post-2033 instants). A flow `ts`
of `1782864300` → ×1000 → `format.datetime` → a 2026 date.

## Tests (all green, real infra)

- `fieldconfig/format.test.ts` (+5 cases) — `dateUnit` on the mappings; the seconds→ms fallback renders
  2026 not 1970; ms unchanged. Pure.
- `editor/tabs/FlowsQuerySection.test.tsx` (+3 cases) — the timestamp-unit suggestion (`ts`/`*_ts`/`*_at`,
  not arrays/plain, never clobbers). Pure.
- `views/flowTsDisplay.gateway.test.tsx` (3 cases, **real gateway**) — seed a real `{payload, ts}` value
  (via inject→run→settle) + seed the viewer's real `user_prefs` (via `prefs.set`), render the EXACT saved
  Stat cell (v3 `sources[]` + the empty v2 `source` placeholder) through `WidgetHost`:
  - EU/Madrid/24h → `01/07/2026 02:05`; USA/NY/12h → `06/30/2026 8:05 PM`.
  - **Workspace isolation**: same user, Madrid in ws-A vs Tokyo in ws-B → `02:05` vs `2026-07-01 09:05`,
    no bleed.
- Regression suites unchanged: UI unit **232 passed**; flow-binding gateway **16 passed**; Rust prefs
  deny+mcp **8 passed**.

### Live smoke (the running node the user has open)

`prefs.set` Madrid → 204; real `counter-2.ts=1782865624` (seconds) → ×1000 → `format.datetime` →
`01/07/2026 02:27`. The backend path is live on `:8080` (the node binary was rebuilt after the
credentials change). The user rebinds a **Stat/gauge** cell to the `ts` field (JSON view stays canonical
by design) and the picker auto-suggests the datetime unit.

## Also fixed: "binding broken — re-pick" on the flow read view (regression)

The user's screenshot showed a saved flow read cell rendering "binding broken" despite the backend read
working. Root cause in `views/WidgetView.tsx`: `primarySource = cell.source ?? <v3 target>` — but the
gateway round-trips a v3 cell with an **empty placeholder** `source: {tool:"", args:null}`, which is
truthy, so `??` picked it over the real `sources[0]` flow target → `flowBindingOfSource` → null →
`denied` → "binding broken." Fixed to `cell.source?.tool ? cell.source : <v3 target>` (use the v2 source
only when it carries a real tool). The `flowTsDisplay` gateway test renders that exact placeholder-bearing
shape through `WidgetHost`, pinning the fix. Logged in
[`debugging/flows/flow-read-binding-broken-empty-source.md`](../../debugging/flows/flow-read-binding-broken-empty-source.md).

## Follow-ups (not this slice)

- Localized month **names** (`1 Jul 2026`) — the deferred icu4x path; the numeric styles ship now behind
  the same call.
- Live re-render on `prefs.set` (the scope's "re-render on change" hint) — today prefs resolve once per
  session; a `prefs.set` needs a reload to re-render. A later slice.
- Gauge/table flow-timestamp fields — Stat is wired; the same `useFormattedValue` seam extends to them.
