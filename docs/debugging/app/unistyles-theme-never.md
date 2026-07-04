# Unistyles theme resolves to `never` (StyleSheet.create theme param untyped)

**Area:** app / styling
**Status:** resolved

## Symptom

After wiring react-native-unistyles 3, every `StyleSheet.create((t) => …)` callback and
`useUnistyles().theme` typed `t`/`theme` as `never`:

```
src/ui/Card.tsx: error TS2339: Property 'colors' does not exist on type 'never'.
src/theme/unistyles.ts: error TS2322: Type 'string' is not assignable to type '() => never'.
```

`StyleSheet.configure({ themes: { dark } })`'s `initialTheme: 'dark'` also failed — `keyof
UnistylesThemes` was `never`, i.e. no theme was registered in the type system.

## Root cause (two separate bugs, both had to be fixed)

1. **The augmentation file was excluded from the program.** It was named `src/theme/unistyles.d.ts`,
   sitting next to `src/theme/unistyles.ts`. tsc treats a `foo.d.ts` adjacent to a `foo.ts` as that
   file's *declaration emit output* and silently drops it from the input program. Confirmed with
   `tsc --listFilesOnly` — the `.d.ts` was absent while every sibling `.ts` was present.
2. **Wrong augmentation target.** `UnistylesThemes` is a **module export** of
   `react-native-unistyles` (re-exported from its `./global`), so it must be merged with
   `declare module 'react-native-unistyles' { export interface UnistylesThemes { … } }`. A
   top-level global `interface UnistylesThemes { … }` does not merge into the module export and
   leaves it empty (`never`).

## Fix

- Rename to `src/theme/theme-augment.d.ts` (no `.ts` sibling) so tsc includes it.
- Use module augmentation:

  ```ts
  import type { AppTheme } from './tokens';
  declare module 'react-native-unistyles' {
    export interface UnistylesThemes { dark: AppTheme }
  }
  ```

`pnpm typecheck` → green.

## Regression guard

`pnpm typecheck` (`tsc --noEmit`) is the regression test: if the augmentation stops being included
or reverts to a global merge, the theme collapses to `never` and typecheck fails loudly. Keep the
augmentation file's basename distinct from any `.ts` in the same folder.
