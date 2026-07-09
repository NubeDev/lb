# Chart Y-axis title overlapping tick numbers — fix session

## Ask

User reported that on chart panel widgets (the `PlotChart` viz, e.g. a timeseries with
a `seq · payload` Y title) the rotated Y-axis **title covers the tick numbers**
(0 / 20000 / … / 80000). Worst in a narrow panel where the numbers and the title
collide in the same left band. Two screenshots.

## Root cause

`ui/src/features/charts/PlotChart.tsx`, the shared `yAxis`:

```tsx
<YAxis width={54}
  label={{ value: yTitle, angle: -90, position: "insideLeft", offset: 12, ... }} />
```

`width={54}` is the whole left gutter — it holds BOTH the tick-number band (up to
`80000` at 11px ≈ 40px wide) AND the rotated title. `position: "insideLeft"` with
`offset: 12` pushes the title 12px to the **right**, i.e. straight into the numbers.
There was no dedicated lane for the title, so it always overlapped once the tick
values grew past a few px.

## Fix

Give the title its own lane to the LEFT of the numbers, and widen the axis only when
a title is present:

```tsx
<YAxis width={yTitle ? 68 : 44}
  label={yTitle ? { value: yTitle, angle: -90, position: "left", offset: -2,
                    style: { ...axisLabelStyle, textAnchor: "middle" } } : undefined} />
```

- `position: "left"` (Recharts v3) places the rotated title at the far outer edge of
  the axis band, clearing the tick labels.
- `width` is now conditional: 68px with a title (title lane + number band), 44px
  without (numbers alone need far less than the old flat 54px).
- X-axis title was untouched — it uses `insideBottom` with its own `height:40` band,
  which never overlapped (horizontal ticks).

## Verification

- Rendered the real Recharts `YAxis` config (old vs new, plus a narrow-panel case)
  against the reported data via an esbuild bundle + Playwright screenshot. OLD shows
  the title sitting on top of `40000`; NEW (both wide and narrow) shows the title
  cleanly left of every tick number. Screenshot reviewed in-session.
- `npx tsc --noEmit` clean. No PlotChart unit test exists to regress (only
  `downsample.test.ts` in that dir), and the change is presentational-only.

## Notes

- Purely a layout/style change to the shared axis; every chart type (`line`, `area`,
  `bar`, `scatter`) reuses the same `yAxis` element, so all inherit the fix.
