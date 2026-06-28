# Frontend dashboard — the shared `vars` library (the frozen spine) (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-config-vars-scope.md ("The shared `vars` library")
- Stage: post-S8 (STATUS.md "Slices in flight")
- Status: done
- Public: ../../public/frontend/dashboard.md → "The shared vars library"
- Tests: ui/src/lib/vars/vars.test.ts (29 exhaustive unit cases)

## Goal

Build the one shared interpolation library extensions reuse — pure TS, no React, no `@/` shell imports —
implementing the Grafana variable syntax + built-ins + a deep type-preserving JSON substitution. This is
a FROZEN contract the moment an extension links it; it carries `VARS_LIB_V`. It is the spine the rest of
the feature rides (cell args, control actions, SQL vars, JSON payloads, `ctx.vars`).

## What shipped — `ui/src/lib/vars/` (one responsibility per file)

- `types.ts` — `Variable` (one model: name → resolver `{tool,args}` or a static form), `VarScope`
  (`{ values, builtins }`), `FormatHint`, `VarValue`, `emptyScope()`, `VARS_LIB_V = 1` (the freeze marker).
- `interpolate.ts` — `interpolate(template, scope)`: the three Grafana reference forms (`$var`/`${var}`/
  `[[var]]`), the format hints (`json`/`csv`/`singlequote`/`doublequote`/`pipe`/`raw`), multi-value
  (csv+pipe+json now; `{a,b}`/regex/glob are NAMED follow-ups, not built), unknown var **left literal**
  (Grafana behavior — never throws). `formatValue` is exported for reuse.
- `interpolateValue.ts` — `interpolateArgs(argsTree, scope, runtimeValue?)`: deep substitution over a JSON
  value tree, **type-preserving** (a SOLE `${var}` reference returns the raw value — a multi-value becomes
  a real array for a JSON `IN` sink; a number/bool passes through). GENERALIZES the shipped
  `views/argsTemplate.ts` — `{{value}}` is now the built-in `${__value}` / the `runtimeValue` slot.
- `builtins.ts` — `resolveBuiltins({timeRange, identity, dashboardId, workspace, interval, value})`:
  `$__from/$__to/$__range[_s/_ms]/$__interval[_ms]/${__user.login}/${__user.email}/${__dashboard}/
  ${__workspace}/${__value}`. **PURE** — the shell supplies trusted inputs from the token + URL; a missing
  input yields **no key** (the reference stays literal, never a fake empty). Un-spoofable by design: an
  extension never calls this with its own identity (Slice 3 hands it resolved values in `ctx`).
- `parse.ts` — `extractVarNames(template)` + `extractVarNamesDeep(tree)` (refresh deps + the deny-set) +
  `isBuiltinName`.
- `index.ts` — the barrel (the federation-shared singleton surface).

## Decisions

- **One substitution engine, reused.** `views/argsTemplate.ts` `fillArgs` now delegates to
  `interpolateArgs(template, emptyScope(), value)` — the control `{{value}}` slot is the same code path as
  every other interpolation. Its existing test cases stay green (asserted).
- **Type-preservation by sole-reference detection.** A string leaf that is *exactly* one reference is
  replaced with the raw value (array stays array); a leaf with surrounding text (`cpu.${host}`) is
  string-interpolated (formats applied). This is what makes a JSON `IN`/array sink and a typed control
  value both correct from one function.
- **Unknown-left-literal everywhere.** Both `interpolate` and the sole-reference path leave an unknown
  reference exactly as written — a shared link always renders (Grafana parity).

## Tests + green output

`vitest run src/lib/vars/vars.test.ts src/features/dashboard/builder/widgetBuilder.test.ts`:

```
✓ src/lib/vars/vars.test.ts (29 tests)
✓ src/features/dashboard/builder/widgetBuilder.test.ts (19 tests)
Test Files  2 passed (2)
Tests  48 passed (48)
```

The 29 vars cases cover: all three syntaxes (incl. mixed), unknown-left-literal (each syntax), every
format hint (single + multi), every built-in (incl. a missing-input → literal), `interpolateArgs` over a
nested tree (embedded ref, sole multi → array, sole single → raw, non-string passthrough, recursion,
unknown literal), the `{{value}}`/`${__value}` runtime fill (bool/number type-preserved), and
`extractVarNames`/`extractVarNamesDeep`. The widgetBuilder suite proves `fillArgs` still passes after the
delegation.

## Follow-ups

Named (NOT built): the richer multi-value forms (`{a,b}`/regex/glob), `$__from:date` ISO hints. Next:
Slice 2 wires this into a variable model + bar + URL sync.
