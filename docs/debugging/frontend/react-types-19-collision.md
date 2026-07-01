# frontend — a new workspace package pulled `@types/react@19` and broke `ui` (React 18) typecheck

Status: **resolved (2026-07-02)**. Area: frontend / workspace deps.

## Symptom

After adding `packages/nav-rail/` (`@nube/nav-rail`) as a `workspace:*` dep of `ui/` and running
`pnpm install`, `ui`'s `tsc --noEmit` flooded with dozens of:

```
src/features/theme/ThemeSwitcher.tsx(77,37): error TS2786: 'Check' cannot be used as a JSX component.
  … Type 'bigint' is not assignable to type 'ReactNode'.
```

across every file importing a `lucide-react` icon (`Check`, `GitPullRequest`, `Building2`, …). None
of these files were touched by the change.

## Root cause

The new package declared **React 19** dev/deps and a newer lucide:
`react@^19.2.0`, `@types/react@^19.2.0`, `lucide-react@^0.453.0`. `ui` is on **React 18**
(`react@^18.3.1`, `@types/react@^18.3.12`). Promoting the pnpm workspace to the repo root put both
packages in one store, and pnpm hoisted a **`@types/react@19.2.17`** type world alongside `ui`'s
`18.3.31`. `lucide-react`'s `ForwardRefExoticComponent` return type resolved against the v19
`ReactNode` (which includes `bigint`), which is not assignable to the v18 `ReactNode` `ui` compiles
against — so every icon "cannot be used as a JSX component." The app code was fine; the *type graph*
was split across two React majors.

## Fix

Align the package's dev React/types/lucide to `ui`'s so the whole workspace stays **one type world**
(the package only needs React ≥18 — it's a peer floor, not a hard 19 requirement):

```
"lucide-react": "^0.460.0",          // was ^0.453.0
"@types/react": "^18.3.12",          // was ^19.2.0
"@types/react-dom": "^18.3.1",       // was ^19.2.0
"react": "^18.3.1",                  // was ^19.2.0
"react-dom": "^18.3.1"               // was ^19.2.0
```

`pnpm install`; `packages/nav-rail/node_modules/@types/react` now resolves `18.3.31`, and `ui`'s
`tsc --noEmit` returns to its **pre-change baseline** (2 unrelated, pre-existing errors in
`FlowsCanvas.gateway.test.ts`) — zero new errors from the package. Package + gateway suites green.

## Prevention / regression

- A new workspace UI package must **match the app's React major** in its dev/peer deps. The peer
  range can be permissive (`>=18`), but the *devDependency* pins that drive its own typecheck should
  equal the consumer's, or pnpm will hoist a second `@types/react` world.
- Regression is implicit: `ui`'s `tsc --noEmit` (run in `pnpm build`) fails loudly on any re-split,
  and the nav-rail package's own `pnpm typecheck` + `pnpm build` gate its side.

## Lesson

`@types/react` is effectively a singleton for a React app: two majors in one workspace makes
`lucide-react` (and any `ForwardRef` component) unassignable in the older half. Pin a new UI
package's React types to the app it ships beside — a permissive peer range is not enough when the
package also declares its own dev React.
