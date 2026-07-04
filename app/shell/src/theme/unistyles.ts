// Register Unistyles ONCE, before any component that calls StyleSheet.create runs. `index.js`
// (native) and `web/index.web.tsx` (preview) both import this at the top of their entry, ahead of
// App. Splitting registration out of the token file keeps tokens a pure data module.

import { StyleSheet } from 'react-native-unistyles';
import { darkTheme } from './tokens';

StyleSheet.configure({
  themes: { dark: darkTheme },
  settings: { initialTheme: 'dark' },
});
