# Insights package scope — `@nube/insights`, the reusable insights UI machinery

Status: **shipped** (this slice). Extracts the shell's insights UI into a workspace package so the
page, **dashboard widgets**, and extensions share ONE implementation. Sibling of
`docs/scope/frontend/dashboard/source-picker-package-scope.md` — same extraction discipline.

## The ask

The insights list/detail/act logic lived only in `ui/src/features/insights` + `ui/src/lib/insights`,
bound to the shell's `@/lib/*` transport. A dashboard widget or an extension that wants "show me open
critical insights" had to re-implement it. Extract the reusable core into `packages/insights` so any
surface reuses it, and ship the two requested **dashboard widgets** (read-only + acknowledge).

## Shape (mirrors `@nube/source-picker`)

Three layers; **the look is optional**:

1. **Model (pure)** — `types.ts` (the wire vocabulary, mirroring `lb_insights` 1:1) + `model.ts`
   (`severityTone`/`statusTone`/`timeAgo`/`originLine`/severity ordering). No React, no CSS. A host
   with its own design system consumes only this + the hooks.
2. **Hooks** — `useInsights` (list: keyset paging, faceted filter, ack/resolve, optional live tail)
   and `useInsight` (detail: record + occurrence ring + act). Both read an injected `InsightsClient`.
3. **UI (optional)** — `InsightsWidget` (one component, `interactive` prop) with two presets:
   `InsightsReadWidget` (read-only) and `InsightsAckWidget` (ack / resolve / **dismiss**), plus the
   `InsightRow` / `InsightActions` / `SeverityBadge` / `StatusBadge` primitives. Self-themed via
   scoped `--ins-*` tokens (`.ins-root`), host-overridable; `import '@nube/insights/style.css'`.

## The injected seam — `InsightsClient`

Transport-agnostic (CLAUDE §9): the package never imports an API client, `invoke`, or `@/`. The host
implements one bag of functions mapping 1:1 to the `insight.*` MCP verbs:
`list / get / ack / resolve / occurrences` + optional `subscribe` (live tail → head refresh). Every
read may reject (a denied cap) → the hooks surface an **honest error**, never a fabricated list. The
host re-checks `mcp:insight.<verb>:call` + the workspace wall on every call regardless of the UI gate.

- **Shell wiring:** `ui/src/lib/insights/insights.client.ts` adapts the existing api + SSE hub onto
  the seam. `insights.types.ts` now **re-exports** the package types (one shape across the stack).
- **Reference client:** `memoryClient(seed)` / `denyClient()` — real implementations of the seam (not
  `*.fake.ts`: the client IS the boundary) for host demos and tests.

## Read-only vs acknowledge (the two widgets requested)

- **Read-only** (`interactive={false}`, default): a glanceable list — dot + title + meta + status +
  time-ago. No action buttons.
- **Acknowledge** (`interactive`): each open/acked row carries **Ack** (open→acked), **Resolve**
  (→resolved, primary), and **Dismiss** (local hide, NOT a durable status change). Status-driven
  visibility so a stale action can't be re-fired.

## Tests (real, in `packages/insights/src/*.test.tsx`)

- `model.test.ts` — pure tone/ordering/formatter assertions against a fixed `now`.
- `InsightsWidget.test.tsx` — read-only shows no actions; acknowledge acks through the **real**
  `memoryClient` (asserts the record flips to `acked`); dismiss hides locally without a status change;
  **capability-deny** (`denyClient`) surfaces an honest error, never a fabricated list.

All green: `pnpm --filter @nube/insights test` (9 tests), `typecheck`, `build` (ESM+CJS+dts+css). The
UI typechecks clean against the package.

## The dashboard panel (shipped — the new-panel flow)

`view:"insights"` is a first-class dashboard panel type, buildable from the **new-panel wizard**
(`/t/$ws/dashboards/$d/new-panel`). It is **not source-bound** — it reads the `insight.*` verbs through
the shell's `InsightsClient`, so the wizard's Source step offers a "no data source needed" Insights
affordance (a `SOURCELESS_VIEWS` concept the source gate honors). Files:

- `View` union + host `widget_catalog.json` (`kind:"read"`, `data:false`, `action:true`) — the
  renderer↔catalog↔union consistency test keeps them one set.
- `views/insights/InsightsView.tsx` + `options.ts` — mounts `@nube/insights`'s `InsightsWidget` over
  `insightsClient`, folding `options.insights` → `filter` + `interactive`.
- Wizard: `SOURCELESS_VIEWS` (panel-kit) + the Source-step affordance + the relaxed `canAdvance` gate;
  step-2 `InsightsBasics` (the **Read-only toggle** — the user-facing headline choice — + status/severity);
  step-3 option defs (`options/defs/insights.ts`) with an `excludeViews` opt-out so the fieldConfig-less
  list doesn't inherit unit/decimals/thresholds.

**Read-only vs interactive:** one panel type, a `Read only` toggle (default ON). Off ⇒ each row gets
Ack / Resolve / Dismiss inline. The toggle is a UX affordance; every verb is still host-gated.

## Open questions / follow-ups

- Detail-drawer widget preset (the `useInsight` hook exists; no packaged UI yet — the shell keeps its
  shadcn `InsightDetail`).
- Faceted-filter toolbar as a packaged component (today the shell's `InsightFacets` stays shell-side).
- Typed body/evidence renderer (still a JSON dump in the shell detail).
- Migrate the shell page/components to consume the package widgets (kept shell-side this slice to hold
  the shadcn look; the client + type re-export are the bridge).
