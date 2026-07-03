# Rules workbench — panel UX polish + editor theme fix (session)

Scope: `rules-editor-ux`. Two asks, both in `ui/src`.

## 1. Common code editor ignored dark/light mode

`components/codeeditor/CodeEditor.tsx` wrapped `@uiw/react-codemirror` but passed no
`theme` prop, so CodeMirror pinned its built-in **light** theme — the editor stayed
white on the shell's dark surface (Rules page and any future editor).

**Fix:** read the current mode from `ThemeContext` (the app-wide source of truth — the
`.dark` class on `<html>`) and pass `theme={mode}` to `<CodeMirror>`. Read the context
directly via `useContext(ThemeContext)?.theme.mode ?? "light"` rather than the throwing
`useTheme()` hook, so the shared editor still renders (light) outside a `ThemeProvider`
(isolated tests). One fix in the shared component covers every consumer.

## 2. Authoring panel (Functions | Examples | Data) looked ugly + no code preview

- `panel/PanelTabs.tsx` — replaced the blocky filled-accent segmented buttons with a
  quiet underline tab bar (accent underline on the active tab; IDE register).
- `panel/FunctionEntry.tsx` — rebuilt as a card: header click-to-insert (unchanged
  `aria-label="insert <name>"`), plus a **Preview** disclosure that reveals the exact
  snippet code on a theme-aware `bg-muted-bg` surface. Preview does not insert.
- `panel/FunctionPalette.tsx` — sticky search header with a one-line hint, tighter
  section spacing.

The "preview of the actual code" ask is the FunctionEntry disclosure.

## Tests (real gateway, per testing-scope §0 — no mocks)

`panel/AuthoringPanel.gateway.test.tsx`: added a test asserting the preview discloses the
exact catalog snippet and does **not** touch the editor buffer. Existing insert/search/
example/data-explorer/deny tests still pass.

```
pnpm test:gateway src/features/rules/panel/AuthoringPanel.gateway.test.tsx  → 7 passed
pnpm test:gateway src/features/rules/RulesView.gateway.test.tsx             → 7 passed
```

Capability-deny + workspace-isolation coverage for this surface already lives in the
same file (denied-datasource deny test, per-`nextWs()` isolation) and stays green.

## 3. Deep-linkable rule routes `/rules/$rule`

Added a `/t/$ws/rules/$rule` route (e.g. `#/t/acme/rules/aidan`) so a saved rule is
directly linkable; bare `/rules` stays a fresh buffer. Mirrors the flows detail route:

- `createAppRouter.tsx` — new `rulesDetailRoute` (`/rules/$rule`), a `RulesSurface` that
  passes `ruleId` + `onSelectRule` into `RulesView` and navigates on selection, and a
  `RulesDetailRoute` that cap-gates on the `rules` surface (the detail route isn't wrapped
  by the bare `/rules` CoreGate, so it re-checks `ctx.allowed`).
- `RulesView.tsx` — accepts `ruleId`/`onSelectRule`; a URL-sync effect opens `ruleId`
  (via the real `rules.get`) or resets to a fresh buffer; rail open/create/delete and the
  name-first Save navigate so the URL always reflects the open rule. The id is the stable
  URL key — rename keeps the same id, so no navigation on rename. Falls back to a direct
  `open` when rendered without `onSelectRule` (embedded/test).

Tests (RulesView.gateway.test.tsx): a `ruleId` prop opens the seeded rule (header shows
its name); creating from the rail fires `onSelectRule` with the derived id. Both green;
full suite 9 passed.
