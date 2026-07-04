# `CommandPalette.gateway.test.tsx` + `CommandPalette.agent.gateway.test.tsx` fail with `useTheme must be used within ThemeProvider` — a pre-existing red (open)

- **Symptom:** `pnpm test:gateway src/features/channel/palette/CommandPalette.gateway.test.tsx` (6
  cases) and `src/features/channel/palette/CommandPalette.agent.gateway.test.tsx` (2+ cases) fail
  fast with `Error: useTheme must be used within ThemeProvider`, raised from
  `src/lib/theme/useTheme.ts:8:11` during the `ChannelView` render. Both files render the full
  `<ChannelView>`; the four SIBLING gateway tests that mount `<MessageItem>` directly
  (`PinToDashboard.gateway.test.tsx` 4/4, `CommandPalette.reminders.gateway.test.tsx` 11/11, and the
  new `ResponseViewResultRender.gateway.test.tsx` 3/3) are GREEN.
- **Area:** `frontend` — the gateway test harness × the `ChannelView` render path × in-flight
  motion/theme work in the working tree.
- **Surfaced:** while running the broader channel-palette gateway suite during widget-platform
  Slice C (result-render coverage), 2026-07-04. NOT a Slice C regression — see "Is this mine?"
  below.
- **Status:** **open** (a follow-up for whoever owns the in-flight motion/theme work in the tree).

## Is this mine? (no)

Slice C's only UI change is the ADDITIVE `ui/src/features/channel/ResponseViewResultRender.gateway.test.tsx`
(3/3 green; mounts `<MessageItem>` directly, never `<ChannelView>`). Slice C touched ZERO
theme/motion/ChannelView code. `git diff --stat HEAD -- ui/` for Slice C is exactly one new test
file. And vitest runs each test file in ISOLATION (its own module registry), so adding a new test
file cannot cause failures in unrelated test files. The proof: both failing files fail IDENTICALLY
in isolation (`pnpm test:gateway src/features/channel/palette/CommandPalette.agent.gateway.test.tsx`
→ `useTheme must be used within ThemeProvider` with no other test file in the run).

The cause is in-flight work uncommitted in the working tree before Slice C started. `git status`
shows pre-existing modifications to `ui/src/lib/motion/` consumers and motion-adjacent surfaces
(flows `FlowArmedBanner` → `FlowRuntimeBanner` rename, `panel-builder` `BuilderPane`/`PreviewPane`/
`PreviewToolbar`, `useSource`/`useVizQuery`, `viz.phase3.gateway.test.tsx`, `data-studio-ux`
scope/session, `cellView.test`, `dashboard.types.ts`). One of those changes routed a `Reveal` (or
other `useMotionPref`/`useTheme` consumer) into a `ChannelView` code path the gateway test renders
WITHOUT a `<ThemeProvider>` wrapper — so the test mounts, hits `useTheme`, and throws.

## Repro

```
cd ui && pnpm test:gateway src/features/channel/palette/CommandPalette.gateway.test.tsx
# → 6/6 fail with `useTheme must be used within ThemeProvider`
cd ui && pnpm test:gateway src/features/channel/palette/CommandPalette.agent.gateway.test.tsx
# → 2+ fail with the same error

# Sibling files that mount <MessageItem> directly are GREEN:
cd ui && pnpm test:gateway src/features/channel/palette/CommandPalette.reminders.gateway.test.tsx  # 11/11
cd ui && pnpm test:gateway src/features/channel/PinToDashboard.gateway.test.tsx                     # 4/4
cd ui && pnpm test:gateway src/features/channel/ResponseViewResultRender.gateway.test.tsx           # 3/3 (Slice C)
```

## Fix (the long-term-right call — for the in-flight motion/theme owner)

Two layers, pick the one matching the in-flight design intent:

1. **Preferably:** wrap the `<ChannelView>` in the two failing gateway tests with the same
   `<ThemeProvider>` wrapper the unit-suite uses (the `ChannelView` render path now legitimately
   needs it — the motion/theme work made `Reveal`/`useMotionPref` reach into `useTheme`, which is
   correct IF a provider is present). Mirror whatever `App.test.tsx` or the dashboard gateway tests
   do for their `<DashboardView>` (which already works with theme).
2. **Alternatively:** if the `useTheme` call in `Reveal`/`useMotionPref` is supposed to be OPTIONAL
   (graceful outside a provider, like an `useThemeOptional()`), make it so — the same pattern as
   `useDashboardWsOptional()` (debugging/frontend/ext-widget-standalone-mount-throws-no-dashboard-cache-provider.md).
   A `Reveal` that quietly no-ops without a provider keeps the test harness untouched.

Decide based on whether `Reveal` is supposed to be a provider-required root primitive (path 1) or a
graceful-degradation surface (path 2). The motion/theme owner makes the call.

## Lesson

A motion/theme primitive that calls `useTheme` (a provider-required hook) becomes a NEW transitive
requirement on every component that mounts it. The unit suite catches nothing here (jsdom tests wrap
with the provider); the gateway suite — which renders full `<ChannelView>` without the App shell —
catches it. When upgrading a primitive to provider-required, sweep the gateway-test corpus for
mounts of components that transitively render it (the `ChannelView`-rendering tests are the
sentinel).

## Cross-link

- Slice C session: `sessions/widgets/result-render-coverage-session.md` (surfaced under
  "Pre-existing reds NOT this slice's").
- Pattern precedent: `debugging/frontend/ext-widget-standalone-mount-throws-no-dashboard-cache-provider.md`
  (a strict "must be inside provider X" guard is right for a narrow consumer but a bug for a broader
  one — add an optional variant, don't loosen the strict guard).
