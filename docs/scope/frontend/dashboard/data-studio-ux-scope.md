# Dashboard scope — Data Studio editing UX (query → see data, fast)

Status: **SHIPPED (2026-07-04)** — durable facts in
[`../../../public/frontend/dashboard.md`](../../../public/frontend/dashboard.md); build log in
[`../../../sessions/frontend/dashboard/data-studio-ux-session.md`](../../../sessions/frontend/dashboard/data-studio-ux-session.md).
Parent: the editor ([`viz/panel-editor-scope.md`](viz/panel-editor-scope.md)) and the picker
package ([`source-picker-package-scope.md`](source-picker-package-scope.md)).

## The ask

Data Studio is the surface where a user builds and *tests* panels — and today that loop is
bad. Grafana Explore is the bar: pick a source, see rows immediately, tweak the query and
options with instant feedback, always know whether a query ran, how long it took, and why it
returned nothing. Today the builder shows a silent **"no data yet"**, the source picker is a
native `<select>` full of paragraph-length labels, "Apply" looks like a run button but is a
persist button, and there is **no query status of any kind** (no spinner, no row count, no
duration, no error text, no "what SQL actually ran"). Separately: editing chart options
(field/override/transform) **re-queries the datasource** even though the data hasn't changed
— an edit-the-look loop should re-render from the frames already on screen.

Two deliverables:

1. **An honest, fast feedback loop** — query state, errors, data inspector, clear run
   semantics, a picker you can search.
2. **Edit without re-querying** — chart/option edits re-shape the frames already fetched;
   only a *source* change (or explicit Run / time-range change) hits the datasource again.

## What's already right (don't rebuild)

The plumbing is healthier than the UX suggests — this scope is mostly *surfacing* it:

- One shared react-query cache per studio visit; the three preview consumers dedupe to one
  `viz.query` round-trip (`ui/src/features/dashboard/cache/`, `builder/useVizQuery.ts`).
- **Viz-type switching already re-renders from cache** (view is not in `vizQueryKey`), as does
  the table-view toggle. The state machine (`lb/panel-kit` `usePanelEditor`) is sound.
- The 200 ms debounce is on the *key input*, so keystroke bursts collapse to one call.

The one structural gap: `fieldConfig` and `transformations` **are** in the query key
(`useVizQuery.ts` spec build), because the transform pipeline runs server-side inside
`viz.query`. So a decimals/threshold/rename tweak = a gateway round-trip + a datasource hit.

## Intent / approach

### A. Edit-without-requery: split *fetch* from *shape* (the structural fix)

Split `viz.query` into its two halves at the API seam, keeping the pipeline server-side
(one implementation of transforms — no client mirror, which would be a hand-written
re-implementation of node behavior, the exact thing rule 9 bans):

- **Fetch**: `viz.query` keyed on `{sources, scope, tick}` **only** — returns raw frames.
  Cache entry lives for the editor session (raise `gcTime` inside the builder).
- **Shape**: a compute-only mode — `viz.query { frames, transformations, fieldConfig }`
  (inline frames in, no source resolution, no datasource touch) — the server runs the same
  pipeline over the posted frames and returns shaped frames. Same verb, same
  `mcp:viz.query:call` gate; a request with inline `frames` simply skips source resolution.
- The editor then keys shaping on `{rawFramesHash, transformations, fieldConfig}` — an
  option edit re-shapes cached frames (fast, no datasource); a source/SQL/time-range edit
  (or Run) re-fetches.

Rejected alternatives: (1) a client-side transform mirror — duplicate pipeline, guaranteed
drift, violates the one-implementation stance; (2) pure react-query tuning
(`keepPreviousData` + longer `gcTime`) — hides the blank flash but still hammers the
datasource on every option keystroke; we take `keepPreviousData` anyway as polish, it just
isn't the fix.

Also add an explicit **freeze/snapshot toggle** in the preview header ("use current data"):
while frozen, *nothing* re-fetches — even source edits shape against the frozen raw frames —
so a user can iterate on a slow/expensive query (federation, big fleet scans) without
re-running it. Unfreeze = one fresh fetch. This is the literal "refresh with the data
that's there now" ask.

### B. Query status + inspector (make the loop honest)

A **status bar** under the preview, fed by the existing react-query states — no new state
store:

- per-target state: idle / running (spinner) / ok / error; on ok: **row count, frame count,
  duration, "as of \<time\>"**; on error: the gateway error text inline (not a silent empty
  chart). `retry:false` already surfaces denies immediately — show them.
- **why-empty explainer**: query ran fine but 0 rows → say "query returned 0 rows for
  \<from\>–\<to\>" instead of "no data yet"; distinguish *never ran* (no resolvable target
  yet — say what's missing, e.g. "pick a series") from *ran and empty*.
- a **Data inspector** drawer (Grafana's Panel Inspect): raw frames as a table + JSON, the
  exact resolved request `viz.query` sent (post-interpolation SQL / tool+args), and the
  response stats. This is also the debugging surface for rules/flows sources whose output
  shape is wrong ([`rules-as-source-scope.md`](rules-as-source-scope.md) §return-shape).

### C. Run semantics (kill the Apply confusion)

- A real **Run / Refresh** button on the preview header for *every* datasource (today only
  federation has one), with Cmd/Ctrl-Enter everywhere, plus a per-tab **auto-run toggle**
  (default on for cheap sources — series/surreal — off for federation raw SQL, preserving
  today's behavior but making it visible instead of implicit).
- Rename **Apply → "Save to tab"** (and keep "Save as library panel" as is), with the
  button disabled-with-reason until the draft differs. Apply must never look like the thing
  that fetches data.

### D. Source picking (the first 10 seconds)

- Replace the native `<select>` with a searchable grouped list/combobox (shadcn `Command`)
  in `@nube/source-picker`'s optional rich UI — headless model unchanged, the `<select>`
  stays as the minimal renderer for embedders. Entries get a **short name + one-line
  description** split (today's paragraph-labels are unreadable in a dropdown); groups keep
  the existing Series/SQL/Live/Extension/Flows/Rules taxonomy.
- "New panel" lands with the source combobox **focused** so type-to-search is the first
  interaction; picking a source with auto-run on shows data with zero further clicks.

### E. Layout polish (small, after A–D)

Preview keeps a sane min-height with the status bar pinned under it; time range moves next
to Run (it's a query input, not chrome); options sections remember collapsed state per user
(`layout.*` prefs, already member-owned).

## Non-goals

- No client-side transform engine (see A — server stays the one pipeline).
- No new dashboard-render changes: this is the *builder* loop; `DashboardView` rendering is
  untouched except that it benefits from the fetch/shape key split for free.
- No new state library (no zustand); react-query + the panel-kit machine already carry it.
- No AI/genui authoring changes (`genui-scope.md` owns that tab).
- No change to core rule 10 seams — `viz.query` stays the one generic verb; frames-in mode
  treats sources as opaque exactly as before.

## How it fits the core

- **Capabilities:** no new caps. Frames-in `viz.query` rides the existing
  `mcp:viz.query:call`; deny renders in the new status bar instead of a blank panel.
- **Workspace:** unchanged — cache is already ws-keyed (`vizQueryKey(ws, spec)`); frames-in
  requests carry no cross-ws reach (they resolve nothing).
- **MCP surface:** one **additive** change — `viz.query` accepts optional inline `frames`
  (compute-only). No new verbs; get/list/watch/batch unchanged, N/A.
- **State vs motion / datastore / bus / placement / secrets:** N/A — read-path UX only.
- **One responsibility per file:** status bar, inspector, run header, freeze toggle each get
  their own file under `panel-builder/`; `QueryTab.tsx` (328 lines) must not absorb them.
- **SDK/WIT impact:** none.
- **Skill doc:** N/A for the UI; the frames-in `viz.query` mode is API-drivable — the
  implementing session updates the existing viz/panel docs (and any `skills/` entry that
  documents `viz.query`) with the frames-in request shape.

## Example flow

1. User opens Data Studio → **New panel**. Source combobox is focused; they type "temp",
   pick *e2e.temp series*; auto-run fires; chart + status bar: "412 rows · 2 frames · 86 ms
   · as of 14:02".
2. They switch to Bar chart — instant (cached, already true today).
3. They add a *group by hour* transform and set decimals=1 — **no datasource hit**: the
   editor posts the cached raw frames to compute-only `viz.query`; status bar shows
   "shaped from cached data · fetched 14:02".
4. The query is expensive federation SQL, so they hit **freeze**; they iterate on overrides
   and viz types against the frozen frames, then unfreeze → one fresh Run.
5. A typo'd SQL shows the gateway error inline in the status bar; the inspector shows the
   exact SQL sent. Fixed, Run, green.
6. **Save to tab**, then **Save as library panel**.

## Testing plan

Per `docs/scope/testing/testing-scope.md` — real gateway, no fakes:

- **Gateway (`pnpm test:gateway`)**: frames-in `viz.query` returns shaped frames identical
  to the fetch+shape composition for the same spec (parity test); frames-in with no
  `mcp:viz.query:call` grant → **deny** (mandatory cap-deny); frames-in never touches the
  store (seed ws `acme`, post frames while authed to `beta`, assert no `acme` data can leak
  — mandatory ws-isolation, trivially satisfied since nothing resolves, but asserted).
- **Editor unit (vitest)**: option edit (fieldConfig/transform) does **not** change the
  fetch key / triggers no fetch (spy on the bridge); source edit does; freeze blocks all
  fetches and unfreeze fires exactly one; status bar renders running/ok(rows,ms)/error/
  0-rows-explainer from real hook states; Run button fires for series/surreal targets.
- **Picker unit (`packages/source-picker`)**: combobox filters entries by search across
  groups; keyboard select fires `onSelect`; headless model untouched (existing tests stay
  green).
- **Regression**: viz-type switch still serves from cache (no fetch) — pin today's good
  behavior before touching keys.

## Risks & hard problems

- **The key split is the risky edit.** `vizQueryKey` is mirrored in `useVizFrames` (ext
  data tiles) and the dashboard render path; splitting fetch/shape must keep those three in
  lock-step or ext tiles silently double-fetch. Do the split in `cache/queryKeys.ts` once,
  consumed everywhere.
- **Frames-in payload size.** Posting a 490k-point raw frame back to shape it can cost more
  than re-querying. Mitigation: only use compute-only mode when raw frames are under a size
  budget (~2–5 MB serialized); above it, fall back to a normal re-fetch (status bar says
  which happened). Budget is a constant, tuned in the session.
- **Live sources.** `series.watch`/`bus.watch` targets stream via SSE, not `viz.query` —
  freeze must pause the subscription, not just skip fetches.
- **`QueryTab.tsx` at 328 lines** is one feature away from the 400 cap — the run-header and
  status work must land as new files.

## Open questions

1. ~~**Where does compute-only live**~~ **Resolved (shipped):** a `frames` param on
   `viz.query` — one verb, one cap. A request with inline `frames` skips source resolution
   and runs only the pipeline. Split to a sibling verb later only if a shape-only grant is
   ever needed.
2. ~~**Freeze scope**~~ **Resolved (shipped): per-tab.** One toggle in the preview toolbar,
   ambient over the tab's preview subtree.
3. ~~**Combobox placement**~~ **Resolved (shipped): in the package.** `SourceCombobox` is an
   optional export of `@nube/source-picker`; the `<select>` renderer is kept for embedders
   that don't want a popover.
4. **Auto-run toggle default per datasource** — series/surreal auto-run; federation raw SQL
   stays manual (Run/⌘-Enter). A visible per-tab auto-run toggle is the remaining refinement
   (today the behavior is implicit-by-datasource, now with a real Run button everywhere).
   Auto-run for rules sources: on, same as surreal; revisit if authors report it's costly.

## Not-yet-done (deferred refinements)

- A **visible auto-run toggle** (per open question 4) — behavior is correct, the control is
  implicit.
- **Entry short-name + description split** in the picker — the combobox makes long labels
  searchable now; splitting `SourceEntry.label` into name+description is additive later.

## Related

- [`viz/panel-editor-scope.md`](viz/panel-editor-scope.md) — the shipped Phase-1 editor this refines.
- [`source-picker-package-scope.md`](source-picker-package-scope.md), [`rules-as-source-scope.md`](rules-as-source-scope.md).
- [`viz/transformations-scope.md`](viz/transformations-scope.md) — the server-side pipeline the compute-only mode reuses.
- `ui/src/features/dashboard/cache/` + `builder/useVizQuery.ts` — the code seam for A.
- [`../../../public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) — promotion target.
