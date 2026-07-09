# Insights widget — visual polish session

## Ask

User reported the Insights dashboard widget/panel "LOOKS so bad" (two screenshots:
tall, airy rows; a redundant severity chip sitting next to the severity dot; badges
stacked vertically in a two-line side column; a pulsing critical dot).

## What was wrong

All look lives in `packages/insights` (the `@nube/insights` widget the
`view:"insights"` dashboard cell mounts via `ui/src/features/dashboard/views/insights/InsightsView.tsx`).
The row (`InsightRow.tsx` + `insights.css`) had:

- `.ins-row { align-items: flex-start }` with a **two-line** `.ins-row-side`
  (severity badge over status badge over time), so single-line rows rendered tall
  and unbalanced — the dot/title pinned to the top, the side column trailing down.
- A **redundant `SeverityBadge`** on every row: the leading colored dot already
  encodes severity, so the CRITICAL/WARNING/INFO chip was duplicate noise.
- A **pulsing** critical dot (`ins-pulse` keyframes) — decorative motion that
  conveys no state change (PRODUCT.md anti-reference: "decorative dashboard
  theatrics"; product register ban: motion that doesn't convey state).
- No separator between rows, so a dense list read as floating text.

## Fix

`packages/insights/src/insights.css`:
- `.ins-row` → `align-items: center`, tighter even padding, `transition: background 150ms`.
- Added an `inset` box-shadow hairline on every non-last row so the list scans as rows.
- `.ins-dot` → removed `margin-top` and the `ins-pulse` animation; added a soft
  `currentColor` ring so it reads as a status indicator, not a stray bullet.
- `.ins-row-main` → column flex (title over meta) with tight gap.
- `.ins-row-side` → single horizontal row (`align-items: center`), status badge + time inline.

`packages/insights/src/InsightRow.tsx`:
- `showSeverity` default flipped `true → false` (the dot carries severity; the chip
  is opt-in for a legend-style row). `showStatus` still defaults `true`.

## Verification

- `cd packages/insights && npx vitest run` → 9 passed (model + widget tests green;
  no test asserted the severity chip, so the default flip is safe).
- `npx tsc --noEmit` → clean.
- Rendered the exact scoped CSS against the reported fixture data via Playwright and
  screenshotted: dense, balanced, single-line side meta, ringed severity dot,
  hairline-separated rows. Matches the intended look.

## Notes

- The stylesheet stays scoped (`.ins-root` prefix, aliases host shadcn vars) — no
  host-app bleed, per the source-picker/nav-rail discipline the file documents.
- No new verbs, no security-surface change: purely presentational.
