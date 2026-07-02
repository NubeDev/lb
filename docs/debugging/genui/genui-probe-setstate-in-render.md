# GenUiView data probe warns "Cannot update a component while rendering a different component"

- Area: frontend
- Status: resolved
- First seen: 2026-07-03
- Resolved: 2026-07-03
- Session: ../../sessions/genui/genui-widget-session.md
- Regression test: ui/src/features/dashboard/views/genui/genui.gateway.test.tsx (`saves … and RENDERS without the adapter` — a `console.error` spy fails on the warning)

## Symptom

Rendering a saved `view:"genui"` cell through `WidgetView` → `GenUiView` (in the gateway test, once
real data resolved) logged:

```
Warning: Cannot update a component (`GenUiView`) while rendering a different component (`TargetProbe`).
To locate the bad setState() call inside `TargetProbe`, follow the stack trace …
    at TargetProbe (…/views/genui/GenUiView.tsx)
    at GenUiView (…/views/genui/GenUiView.tsx)
```

The widget still rendered correctly (the warning is not fatal), but it is a real React contract
violation — a parent `setState` triggered from a child's render — that can cause extra renders and,
in concurrent React, tearing.

## Root cause

`GenUiView` resolves one data target per `sources[]` entry by rendering a `TargetProbe` child per
refId; each probe calls `usePanelData` and reports its resolved `RefData` up to the parent's
`byRef` state via an `onData` callback. The report was written as:

```tsx
const sig = JSON.stringify(data);
useMemo(() => onData(target.refId, data), [target.refId, sig]); // ← runs DURING render
```

`useMemo`'s factory runs **during render**. `onData` calls the parent's `setByRef` — so a child's
render synchronously updated the parent's state. React allows it (it warns and re-renders) but it is
exactly the "setState during render of another component" the warning names.

## Fix

Report **after commit**, from an effect, not from `useMemo`:

```tsx
const sig = JSON.stringify(data);
useEffect(() => {
  onData(target.refId, data);
}, [target.refId, sig]); // content-signature dep so an identical resolve doesn't re-report
```

`useEffect` runs post-commit, so the parent `setState` is a normal, warning-free update. The content
signature `sig` keeps the effect from re-firing on an identical resolve (`usePanelData` returns a
fresh object each render). The `onData` setter already short-circuits when the value is unchanged, so
there is no render loop.

## Regression guard

The genui gateway test spies on `console.error` around the real-data render and asserts it is NOT
called with `"Cannot update a component …"`. Fails-before (the warning fires once real data lands),
passes-after.

## Lesson

A child that reports derived data up to a parent must do it in `useEffect` (post-commit), never in
`useMemo`/render — `useMemo` is for computing a value, not for side effects, and a parent `setState`
from a child's render is the classic "setState-in-render" trap. When a hook can only run inside a
component (like `usePanelData`) and you need one-per-item, a per-item probe component is the right
shape — just make it report from an effect.
