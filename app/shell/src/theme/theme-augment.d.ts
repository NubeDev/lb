// Type augmentation so `StyleSheet.create((theme) => …)` and `useUnistyles().theme` are typed
// against our tokens (and `dark` is the only theme name). `UnistylesThemes` is a MODULE export of
// react-native-unistyles (re-exported from its ./global), so we merge via `declare module`. NB: this
// file is named theme-augment (not unistyles.d.ts) on purpose — a `foo.d.ts` next to a `foo.ts` is
// treated by tsc as that file's emit output and silently excluded from the program. Value
// registration is in unistyles.ts; this is types only.

import type { AppTheme } from './tokens';

declare module 'react-native-unistyles' {
  export interface UnistylesThemes {
    dark: AppTheme;
  }
}
