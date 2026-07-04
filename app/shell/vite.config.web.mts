// Vite dev server for the react-native-web PREVIEW of the shell — runs the same App.tsx + screens
// in a browser on this PC, no emulator. `vite-plugin-react-native-web` does the heavy lifting
// (react-native → react-native-web, Flow stripping, platform .web.* resolution); our aliases only
// swap the three native-only touchpoints (keychain, the fetch polyfill, the native stack).

import { defineConfig } from 'vite';
import reactNativeWeb from 'vite-plugin-react-native-web';
import path from 'node:path';

const here = (p: string) => path.resolve(import.meta.dirname, p);

export default defineConfig({
  root: here('web'),
  plugins: [reactNativeWeb()],
  resolve: {
    alias: [
      { find: 'react-native/Libraries/Utilities/PolyfillFunctions', replacement: here('web/polyfills.web.ts') },
      { find: 'react-native-keychain', replacement: here('web/keychain-module.web.ts') },
      { find: 'react-native-screens', replacement: here('web/screens-shim') },
      { find: 'react-native-safe-area-context', replacement: here('web/safe-area-shim') },
      { find: '@react-navigation/native-stack', replacement: here('web/nav-stack.web.ts') },
      { find: '@nube/app-sdk', replacement: here('../sdk/src/index.ts') },
    ],
  },
  server: { host: '127.0.0.1', port: 5310, hmr: { overlay: false } },
});
