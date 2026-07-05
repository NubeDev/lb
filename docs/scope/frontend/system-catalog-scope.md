# Frontend scope — the workspace system catalog (grow `@nube/source-picker` into the one browse/pick surface)

Status: **scope (the ask)** — 2026-07-05. Parent: the shipped
[`dashboard/source-picker-package-scope.md`](dashboard/source-picker-package-scope.md) and the rules
data explorer ([`rules-editor-ux-scope.md`](rules-editor-ux-scope.md)). Promotes to
`public/frontend/` once shipped.

## The ask

Every authoring surface in Lazybones keeps answering the same question — **"what exists in this
workspace, and what can I reference here?"** — datasources, local tables, series (history + live),
flows, rules, channels, insights, inbox, installed extensions. Today the answer is split across two
implementations:

- **`@nube/source-picker`** (packages/) — the transport-agnostic *picker*: model + injected
  `SourceLoaders` + a grouped combobox. Already consumed by the dashboard editor, Data Studio's
  Sources pane, and thecrew.
- **The rules data explorer** (`ui/src/features/rules/panel/DataExplorer.tsx` +
  `useDataExplorer.ts`) — a *browsable tree panel* (sections → click-to-insert entries, honest
  loading/denied/empty per section), hand-rolled inside the feature and welded to `@/lib/*`, so
  nothing else can reuse it.

Make it **one library**: grow `@nube/source-picker` into the workspace **system catalog** — one
model + loader seam, **two UI skins** (the existing combobox picker AND a new browsable explorer
tree), covering every enumerable subsystem — so the rules panel, Data Studio, dashboards, flow node
config, channel composers, and future surfaces (and extension UIs, via the bridge) all browse the
same system through the same package.

## What exists today

| Piece | Where | State |
|---|---|---|
| Model + loaders + combobox | `packages/source-picker` | Shipped: series/live, flows (ports), rules, extensions/widgets, datasources. Deny-tolerant, ws-keyed, transport-agnostic. |
| Explorer tree (sections + click-to-insert) | `ui/src/features/rules/panel/{DataExplorer,useDataExplorer}` | Shipped but host-coupled: 3 sections (datasources / local tables via `SchemaBrowser` / series), `SectionState` tri-state (loading/denied/ready — never a fabricated roster). |
| Local schema reader | `ui/src/lib/schema` + `ui/src/components/schema` | Shared shell lib (`store.schema`), consumed by the rules explorer and the dashboard SQL builder. |

The package is already 70% of the ask. What's missing: (a) **more loaders** — local schema,
channels, insights, inbox; (b) the **explorer skin** — the tree panel is trapped in `features/rules`;
(c) the **section registry** shape that lets a host compose which sections its surface shows.

## Goals

- One package exports the catalog **model**, the **loader seam**, and **both skins**
  (`SourcePicker` combobox + a new `CatalogExplorer` tree).
- The rules panel's `DataExplorer` becomes a thin wrapper over the package (loaders + a
  Rhai-snippet `onSelect` mapping) with zero behavior change.
- New sections over **shipped** verbs only: local tables (`store.schema`), channels
  (`channel.list`), insights (`insight.list`), inbox (`inbox.list`) — each optional, each honest
  on deny.
- Any host — shell feature or federated extension — composes its own catalog by supplying loaders;
  absent loader ⇒ absent section.

## Non-goals

- **No new node verbs.** Subsystems with no roster verb today — **outbox** (only
  `outbox.due`/`outbox.status`, operational not browsable) and **webhooks** (a flow source-node
  config, not a first-class listable record) — are *named follow-ups*, surfaced as absent, never
  fabricated. Same for deep **external-datasource table introspection** (the honest gap already
  noted in the rules explorer): it needs a federation verb first, then becomes just another loader.
- **No query execution, editing, or per-subsystem admin UI** in the package. Its one responsibility
  is *enumerate + pick*. The moment it runs queries or edits records it's `utils/` with a nicer
  name.
- **No host snippet logic.** What a pick *means* — a Rhai `source("x")`, a SQL table name, a
  dashboard cell source — is the host's `onSelect` mapping, exactly as `SourceSelection` works
  today.

## Intent / approach

**Grow the existing package; don't fork a sibling.** One model layer, two presentations:

```
@nube/source-picker  (packages/source-picker — rename deferred, see open questions)
  types     SourceLoaders (+ readSchema/listChannels/listInsights/listInbox), Schema row shapes,
            SectionState<T> (moved in from useDataExplorer), SourceSelection (unchanged)
  model     buildSourceEntries (unchanged) + buildCatalogSections — sections AS DATA
  hooks     useSourcePicker (unchanged) + useCatalog(loaders, ws) — per-section honest tri-state
  ui        <SourcePicker> (unchanged combobox) + <CatalogExplorer sections onSelect> — the tree
            skin extracted from rules' DataExplorer (section header/hint, loading skeleton,
            "Not permitted." deny, teaching empty state, click-to-insert rows, table→column tree)
```

- **Sections are data, ids are opaque (rule 10).** The package ships a *vocabulary* of section
  kinds keyed by which loader fed them; the HOST decides which sections a surface shows by which
  loaders it wires. Extension-contributed sources arrive only through the generic `ext.list`
  loader, never a named case. If supporting a new subsystem means editing the package (beyond
  adding one optional loader + row shape), that's the leak.
- **The explorer moves, the meaning stays.** `CatalogExplorer` renders sections and returns the
  picked entry; the rules panel keeps its own `onInsert` mapping (`source("name")`,
  `history("series", …)`, table/column names). The local-tables child tree (table → columns) moves
  into the package as the section's renderer; the shell's `lib/schema` reader stays as the shell's
  loader adapter.
- **Per-section state is part of the contract.** `useSourcePicker` today collapses a deny into an
  empty group; the explorer must keep the rules panel's *visible* tri-state (`SectionState`) —
  loading skeleton, explicit "Not permitted.", teaching empty. `useCatalog` returns per-section
  state; the combobox path keeps its existing collapse.

**Rejected alternatives.** (a) *A new sibling package* — splits the model in two homes; the picker
and the explorer would drift on loaders/types, the exact duplication trap this scope exists to
close. (b) *`ui/src/lib`* — shares within the shell only; extensions build `--ignore-workspace`
and can't import it (same reasoning that created the package — see the parent scope). (c) *One
mega "SystemBrowser" component* — hosts need different compositions (rules wants insert snippets,
Data Studio wants open-a-tab, dashboards want bind); shipping model + two skins and letting hosts
map the selection is the boundary that has already worked.

## How it fits the core

- **Zero core additions.** No new verb/cap/table/WIT. The package CONSUMES shipped, gated reads
  (`datasource.list`, `store.schema`, `series.list`, `flows.*`, `rules.list`, `ext.list`,
  `channel.list`, `insight.list`, `inbox.list`) via injected loaders; the host still gates every
  call server-side.
- **Workspace is the hard wall** — N/A to change: the workspace comes from the host's transport
  (token/bridge); the package re-keys on `ws` and holds nothing across it. Isolation stays proven
  in host gateway suites.
- **Capability-first** — no new grant. A denied loader read renders an explicit "Not permitted."
  section (explorer) or an empty group (picker) — never a fabricated roster (CLAUDE §9).
- **Symmetric nodes / placement** — transport-agnostic by construction; shell injects
  gateway/Tauri, extensions inject their bridge. No role branch.
- **Core knows no extension (rule 10)** — sections are registry-driven data; the only
  extension-shaped section is fed by generic `ext.list`; ids stay opaque strings.
- **State vs motion** — unchanged; the catalog *labels* sources. `series.read` vs `series.watch`
  stay distinct entries. The catalog itself is a snapshot (list reads), not a feed; a live-updating
  catalog is out of scope.
- **One datastore / durability / stateless / SDK-WIT** — N/A: pure frontend, no persistence, no
  effects, no plugin-boundary change.
- **MCP is the contract** — every entry resolves to `{tool,args}`/`view`, unchanged.
- **API shape** — N/A: consumes shipped get/list verbs; exposes none.
- **One responsibility per file (FILE-LAYOUT)** — one loader-row-shape home (`types.ts` grows),
  one hook per file (`useCatalog.ts`), one skin per file (`CatalogExplorer.tsx` + small
  `Section`/`Empty` pieces if it nears the line budget). No `utils`.
- **No mocks** — package unit tests use an injected fake *loader object* (a pure function seam,
  permitted); the real store/gateway path stays proven by the host suites (rules
  `AuthoringPanel.gateway`, Data Studio gateway, dashboard gateway — all must stay green).
- **Skill doc** — **N/A.** Frontend package refactor + new UI skin over already-skilled verbs; no
  new agent-/API-drivable surface.

## Example flow

1. The rules Playground mounts its authoring panel. The shell wires a `SourceLoaders` adapter
   (`listDatasources`, `readSchema`, `listSeries` — the three sections it shows today).
2. `useCatalog(loaders, ws)` fires the three reads in parallel; each section resolves to
   `ready | denied | loading` independently.
3. `<CatalogExplorer sections onSelect>` renders: Datasources (name + kind·endpoint rows), Local
   tables (table → column tree), Series (row per name). `series.list` was denied for this member →
   that section shows "Not permitted.", the others render normally.
4. The user clicks table `flow_run` → column `status`. `onSelect` fires with the entry; the rules
   host maps it to its insert snippet and drops `status` at the editor cursor. (Data Studio's host
   would instead open a builder tab; a dashboard would bind a cell — same entry, host-owned
   meaning.)
5. thecrew (extension, bridge transport) wires the same loaders over `bridge.call` and gets the
   identical explorer — no shell import, no fork.

## Consumers (the migration)

1. **Rules panel first (parity refactor — the headline).** Move `SectionState` + the tree skin into
   the package; `DataExplorer.tsx` becomes loaders-adapter + snippet mapping (~thin);
   `useDataExplorer.ts` retires into `useCatalog`. Every shipped rules test — unit and
   `AuthoringPanel.gateway` — stays green. This proves the extraction before anything new.
2. **New sections second.** Add the optional `listChannels` / `listInsights` / `listInbox` loaders +
   row shapes; no shell surface is *forced* to show them — first consumer is whichever surface asks
   (candidate: the agent dock's context picker, the channel composer).
3. **Data Studio (optional skin swap).** `SourcesPane` already consumes the package's combobox;
   offering the explorer tree there is a follow-up UX decision, not part of this scope's gate.
4. **Dashboard** — no change (already on the package).

## Testing plan

Per `scope/testing/testing-scope.md`:

- **Parity (the gate):** all shipped rules-explorer tests green after the extraction —
  `AuthoringPanel.gateway.test.tsx` (real gateway: sections render from real seeded records,
  click-to-insert lands the snippet) plus the picker consumers' suites (Data Studio gateway,
  dashboard gateway) untouched and green.
- **Package unit:** `useCatalog` with an injected fake loader object — per-section deny → `denied`
  (not empty-ready), ws re-key, absent loader → absent section; `CatalogExplorer` renders every
  state (skeleton / "Not permitted." / teaching-empty / rows), fires `onSelect` with the right
  entry, table→column tree expands and picks columns.
- **Capability-deny + workspace-isolation (mandatory):** stay in the HOST gateway suites — the
  package has no transport. The rules gateway suite already exercises a real deny rendering the
  explicit denied state; keep/extend that case for one new section (e.g. `channel.list` denied →
  "Not permitted.", never a fake list).
- **Build/federation:** the package keeps building ESM+CJS+dts+scoped CSS; thecrew's standalone
  `--ignore-workspace` build still resolves it.

## Risks & hard problems

- **God-library creep.** The pull to absorb query-running, record editing, or per-subsystem admin
  panes will be constant ("it already lists insights, just let me ack one…"). Hold the line at
  *enumerate + pick*; anything more is a host feature or its own scope.
- **Two state contracts, one hook.** The picker collapses deny→empty; the explorer must surface
  deny explicitly. Getting one loader orchestration to serve both without forking the hook is the
  main design seam — `useCatalog` returns per-section `SectionState` and the picker's collapse
  becomes a trivial projection of it, not a second loader path.
- **Schema tree gravity.** `ui/src/components/schema` (`SchemaBrowser`) is shared with the
  dashboard SQL builder. Move the *render* into the package and re-point both consumers, or leave
  the shell component delegating to the package's — decide early, don't ship two trees.
- **Load-bearing refactor.** The picker has five-plus live consumers across shell + thecrew. Move
  in small green steps (types → hook → skin → rules rewire), running the gateway suites between
  steps — same discipline as the parent extraction.
- **CSS discipline.** The explorer skin must obey the packages/* stylesheet rules (scoped `--sp-*`
  tokens under the root class, aliased shadcn vars, no preflight/global utilities) or it silently
  breaks host apps.

## Open questions

1. **Rename?** `@nube/source-picker` undersells a catalog with an explorer skin.
   **Recommendation: keep the name this pass** — a rename churns imports in dashboard, Data
   Studio, and thecrew for zero behavior; if the package graduates further (e.g.
   `@nube/system-catalog`), do it as its own mechanical follow-up.
2. **Does `SchemaBrowser` move in wholesale** (deleting `ui/src/components/schema`) or does the
   shell component become a re-export? Leaning: move it in — one tree, two consumers re-pointed.
3. **Insights section shape:** `insight.list` only, or also `insight.sub.list` as a child level
   (insight → subscriptions)? Decide with the first real consumer; don't speculate levels nobody
   picks from.
4. **Live catalog:** should sections refresh on bus motion (a new series appears while the panel
   is open)? Deferred — snapshot + ws re-key matches every shipped consumer today; a `watch`-fed
   catalog is its own scope if a surface actually needs it.

## Related

- [`dashboard/source-picker-package-scope.md`](dashboard/source-picker-package-scope.md) — the
  parent extraction this grows; its loaders/types/CSS rules all carry over.
- [`rules-editor-ux-scope.md`](rules-editor-ux-scope.md) — the shipped explorer being generalized
  (and the `lib/schema` extraction it already did).
- [`data-studio-scope.md`](data-studio-scope.md) — the Sources pane consumer;
  [`dashboard/rules-as-source-scope.md`](dashboard/rules-as-source-scope.md) — the rules group.
- `packages/panel`, `packages/nav-rail` — the shared-package pattern (pure, props-driven, scoped
  tokens, React peer dep).
- Named follow-ups this scope creates: **federation table introspection** (external-datasource
  deep browse needs a verb), **outbox/webhook rosters** (need list verbs before they can be
  sections), **live catalog** (bus-fed refresh).
