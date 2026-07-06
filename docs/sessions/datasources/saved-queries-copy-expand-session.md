# Session — SavedQueriesDialog: copy + expand-to-view

Scope: `docs/scope/frontend/query-builder/query-workbench-view-scope.md` (slice 3 follow-up)
Status: **done** (code + gateway test green this session)

## The ask

The saved-queries dialog (`SavedQueriesDialog.tsx`) only let the user click a row to load its SQL
into the editor (the author then hits Run). Three row actions were missing:

- **Copy** the SQL to the clipboard (lazy-load via `query.get`).
- **Expand** the row to view the SQL inline, syntax-highlighted — reusing the SAME `SqlEditor`
  the datasource's Code mode uses, not a flat `<pre>`.
- Keep the existing **Open** (load into editor) and **Delete** affordances.

The roster summary (`query.list`) carries no text — only the full record (`query.get`) does — so
copy + expand lazy-load on first interaction.

## What changed

### `ui/src/features/datasources/SavedQueriesDialog.tsx` (rewrite, still one responsibility)

- New prop `onFetchText: (id: string) => Promise<string>` — the parent hooks the shipped
  `query.get` (no new persistence; the workbench hands it `saved.load(id).then((q) => q.text)`).
- New row action: a chevron button (ChevronRight/ChevronDown) toggling `expandedId`. On first
  expand, `ensureText(id)` lazy-loads via `query.get`; the result is cached in `textCache` for the
  dialog session (one fetch per row — verified by the CACHE test below).
- New row action: a copy button (ClipboardCopy → transient Check for 1.5s) writing the SQL via
  `navigator.clipboard?.writeText`. Mirrors the established `DockCopyButton` pattern (graceful
  no-op on clipboard-deny).
- Expanded body renders the SAME CodeMirror `SqlEditor` the workbench's Code mode uses
  (`@/features/dashboard/builder/editors/SqlEditor`), `editable={false}`, `dialect="standard"`
  (federation saved queries are `lang:"raw"` SQL — derived from the target kind, never a name; rule
  10). Wrapped in a `border-t` div labelled `saved query text <id>` so the test can find it.
- Delete now also drops the row's cached text + collapses the row if it was open.
- Dialog description copy updated to mention copy + expand.
- Max list height bumped `max-h-80 → max-h-[60vh]` so an expanded editor + 5 rows doesn't overflow.
- Lint: the row body's raw `<button>` (a layout-driven row select, not a primary action) is now
  suppressed with the codebase's standard `eslint-disable-next-line` justification (matches
  `JsonPathPicker.tsx`'s "tree row select" precedent).

### `ui/src/features/query-workbench/QueryWorkbench.tsx`

- One new prop on the dialog mount: `onFetchText={(id) => saved.load(id).then((q) => q.text)}`.
  The workbench already owned `saved.load` (the `useDatasourceQueries` hook exposes it); no new
  hook, no new API.

## Decisions

- **Read-only expand, not editable.** "View it nice" — edits happen in the workbench after Load.
  An editable expand would tempt a "save back from the dialog" flow that duplicates the Save dialog
  and round-trips state needlessly. Rejected alternative: an editable inline editor that saves in
  place.
- **Lazy-load via the existing `query.get`, not a widened `query.list`.** Widening the roster to
  carry text would pay the text cost for every dialog open even when the user only wants the names
  (and would double the payload for sources with many saved queries). Lazy per-row is the right
  shape: copy/expand are rare, click-to-Open is the common path.
- **Cache keyed by id, lifetime = dialog session.** A second interaction on the SAME row (expand
  then copy) does NOT re-fire `query.get` — the CACHE test pins this. Closing the dialog resets the
  cache (next open is a fresh session).
- **One `SqlEditor`, three homes already.** The reuse is exactly the scope umbrella's
  "one-component/three-homes" rule extended: the editor that powers Code mode in the workbench now
  also powers the read-only view in the saved-queries dialog. No fork.
- **Standard dialect, hardcoded.** Federation saved queries are always `lang:"raw"` SQL against a
  `datasource:<name>` target, and `dialect` is config data derived from the target kind (rule 10)
  — never from the datasource's name. A `platform:` target dialog (if one ships) would pass
  `dialect="surreal"`; that's a follow-up, not a special case here.

## Tests

`ui/src/features/datasources/SavedQueriesDialog.gateway.test.tsx` — new file, real-gateway (no
fakes; rule 9). Each test signs into a UNIQUE workspace, registers a real `datasource:<name>` row,
persists a real `query:{ws}:{id}` record via the shipped `query.save` MCP verb, and drives the
dialog over the real `POST /mcp/call` bridge. The `Range.getClientRects` polyfill (mirror of
`AuthoringPanel.gateway.test.tsx`) is required so CodeMirror mounts its `.cm-content` in jsdom.

- **HEADLINE** — expand a row → `query.get` fires once → the read-only `SqlEditor` renders the
  saved SQL into `.cm-content` (asserts `SELECT` + the table name).
- **COPY** — copy a row → `query.get` fires → the exact SQL lands verbatim on the clipboard
  (asserts `writes[0] === SQL`).
- **CACHE** — expand then copy on the SAME row fires `query.get` exactly once (the second
  interaction rides the cache, never a second fetch).

All three pass:

```
✓ src/features/datasources/SavedQueriesDialog.gateway.test.tsx (3 tests) 401ms
```

`QueryWorkbench.gateway.test.tsx` (5 tests) still green — the new `onFetchText` prop is additive;
the existing load-into-editor path (`onLoad`) is unchanged.

## Non-negotiables held

- Rule 5 (capability-first): copy + expand go through `query.get` — workspace-isolated and
  capability-gated exactly like Open. A deny returns null text → the affordance no-ops (no
  fabricated SQL).
- Rule 6 (workspace wall): the test seeds each row in its own workspace; the dialog only ever sees
  the session workspace's roster.
- Rule 7 (MCP universal contract): no new verb, no new route — copy + expand ride the shipped
  `query.get` MCP tool through the same `mcp_call` bridge.
- Rule 8 (one responsibility per file): the dialog's responsibility is unchanged — "list saved
  queries against THIS datasource and let the user act on one". Three row actions (open / copy /
  expand / delete) are all aspects of that one responsibility.
- Rule 9 (no mocks): every datum is a real `query.save` + real `query.get` over the spawned
  gateway; the only stub is the clipboard (`Object.defineProperty(navigator, "clipboard", …)` — a
  true external a UI test cannot drive honestly in jsdom).
- Rule 10 (core knows no extension): `dialect="standard"` is derived from the federation target
  kind, not from any datasource name; the editor never branches on an id.
