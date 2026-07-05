# Insights UI used bare `rounded` — the radius-scale guard failed the whole UI unit suite

- Area: insights/ui + styles
- Status: resolved
- First seen: 2026-07-05
- Resolved: 2026-07-05
- Session: ../../sessions/insights/insights-session.md
- Regression test: `ui/src/styles/radius-scale.guard.test.ts` ("has no bare `rounded` utility in any .tsx")

## Symptom

`pnpm test --run` failed at `src/styles/radius-scale.guard.test.ts` with three offenders — all in
the insights feature:

```
src/features/insights/InsightDetail.tsx:70  <code className="rounded bg-muted px-1">…dedup_key</code>
src/features/insights/InsightDetail.tsx:78  <code className="rounded bg-muted px-1">…origin…</code>
src/features/insights/InsightsList.tsx:50   <code className="rounded bg-muted px-1">…dedup_key</code>
```

The whole UI unit suite (631 tests) was red because of these three lines.

## Reproduce

`pnpm test --run` from `ui/`. The guard test scans every `.tsx` under `src/` for the bare `rounded`
utility (regex `\brounded\b(?!-)`) and fails on any match not in the `rounded-full`/`rounded-none`
allowlist.

## Investigation

The radius-scale guard (`ui/src/styles/radius-scale.guard.test.ts`) is the build-level backstop for
the shipped radius bug (theme-appearance scope): bare `rounded` maps to Tailwind's un-derived
DEFAULT, which the radius control can't move. The repo swept all bare `rounded` → `rounded-md`
across 44 `.tsx` files when the bug shipped (see `debugging/README.md` history, 2026-07-04). The
insights feature folder was added AFTER that sweep and reintroduced bare `rounded` on three
`<code>` chips. The `rounded-full` instances in the same files are deliberate pills (allowlisted).

## Root cause

A new feature folder didn't follow the established `rounded-md` convention (the radius bug's
prevention is a source-level guard, not enforced by the Tailwind config). The three `<code>` chips
copied a bare `rounded` from an older pattern.

## Fix

`ui/src/features/insights/InsightDetail.tsx` (lines 70, 78) + `ui/src/features/insights/InsightsList.tsx`
(line 50) — bare `rounded` → `rounded-md` on the three `<code>` chips. (`rounded-full` pills
untouched — those are intentional.)

## Verification

`pnpm test --run` from `ui/` → 103 files / 631 tests passed, 0 failed.

## Prevention

The guard test IS the prevention — it catches the regression at `pnpm test`. The fix is applying
the convention the guard encodes. (A future Tailwind preset that disables the bare `rounded`
utility would make the class impossible to write; until then the guard holds the line.)
