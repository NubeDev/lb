# Session — extract `@nube/insights` (reusable insights + dashboard widgets)

Date: 2026-07-09. Scope: `docs/scope/insights/insights-package-scope.md`.

## Ask

Make the insights code reusable so extensions and dashboard widgets can use it — a workspace package,
with the interface/UI and core separated so a widget can bring its own look (the look is optional).
Ship two dashboard widgets: read-only, and one to acknowledge/dismiss.

## What shipped

New package `packages/insights` (`@nube/insights`), modelled on `@nube/source-picker`:

- **Model (pure):** `src/types.ts` (wire vocabulary mirroring `lb_insights` + the `InsightsClient`
  seam), `src/model.ts` (`severityTone`/`statusTone`/`timeAgo`/`originLine`/severity ordering).
- **Hooks:** `src/useInsights.ts` (list) and `src/useInsight.ts` (detail) — read the injected client
  through a ref (host-stability; no re-loop on an unmemoized literal).
- **UI (optional look, scoped `--ins-*`):** `src/InsightsWidget.tsx` (+ `InsightsReadWidget` /
  `InsightsAckWidget` presets), `src/InsightRow.tsx`, `src/InsightActions.tsx` (ack/resolve/dismiss),
  `src/InsightBadge.tsx`, `src/insights.css`.
- **Reference client:** `src/memoryClient.ts` (`memoryClient` + `denyClient`) — real seam
  implementations for demos/tests (not `*.fake.ts`).
- Build/config: `package.json`, `vite.config.ts` (lib ESM+CJS+dts+css, React/lucide external),
  `vitest.config.ts`, `tsconfig.json`, `.gitignore`, `README.md`.

Shell wiring (one bridge point, no shell UI rewrite this slice):

- `ui/package.json` — added `@nube/insights: workspace:*`.
- `ui/src/lib/insights/insights.client.ts` — adapts the shell api + SSE hub onto `InsightsClient`.
- `ui/src/lib/insights/insights.types.ts` — now **re-exports** the package types (one shape).

## Decisions

- **Followed the source-picker template** rather than inventing a new package shape — same
  transport-agnostic seam, same "three layers, look optional" split, same scoped-token theming. The
  reviewer reads both packages as one system.
- **One `InsightsWidget`, two presets** (an `interactive` prop) rather than two duplicated components —
  read-only and acknowledge share the list/empty/error/paging frame; only the row actions differ.
- **Dismiss = local hide**, distinct from resolve (durable). The acknowledge widget tracks dismissed
  ids locally; resolve goes through the client.
- **Kept the shell page on its shadcn components** this slice; bridged via the client + type re-export
  so there is one shape without a risky UI migration. Migrating the page to the package widgets is a
  named follow-up.
- **`import type { JSX }`** in the component files — the dts rollup (`vite-plugin-dts` / api-extractor)
  can't follow the global `JSX` symbol; importing it from `react` fixes the build while keeping the
  explicit return annotations.

## Tests (all green)

- `pnpm --filter @nube/insights test` → **9 passed** (`model.test.ts`, `InsightsWidget.test.tsx`).
  Includes the mandatory **capability-deny** test (`denyClient` → honest error, no fabricated list).
- `pnpm --filter @nube/insights typecheck` → clean; `build` → ESM+CJS+dts+css emitted.
- `ui` full `tsc --noEmit` → clean (the re-export + client compile against the package).

Note: workspace-isolation is enforced server-side by the host (the `insight.*` verbs are ws-scoped);
the package is transport-agnostic and carries no ws logic, so the isolation test lives with the node
verbs, not this UI package.

## Follow-ups

See the scope's "Open questions": packaged detail-drawer + facets components, typed evidence renderer,
and migrating the shell page to consume the widgets.
