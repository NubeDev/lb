// Vite plugin: run the Unistyles 3 Babel transform on OUR source for the RN-Web preview.
//
// On native, `RepackUnistylePlugin` (rspack) does this. On web, `vite-plugin-react-native-web`
// transforms via Rolldown/esbuild — which never runs Babel — so the Unistyles plugin (which
// rewrites `StyleSheet.create` call-sites and tags styled RN components with their dependencies)
// would never fire, and `useUnistyles`/theme updates would render unstyled. This plugin fills
// exactly that gap: a Babel pass scoped to our own `.ts/.tsx` (node_modules excluded — the
// Unistyles web runtime and RN-Web are already correct), matching what the native loader does.

import type { Plugin } from 'vite';
import { transform } from '@babel/core';

const OURS = /\.[cm]?tsx?$/;

export function unistylesBabel(root: string): Plugin {
  return {
    name: 'unistyles-babel',
    enforce: 'post', // after react-native-web (`enforce: 'pre'`) has resolved/aliased.
    async transform(code, id) {
      if (id.includes('node_modules') || !OURS.test(id) || !id.startsWith(root)) return null;
      if (!/react-native-unistyles|StyleSheet|View|Text|Pressable/.test(code)) return null;
      const isTsx = /\.[cm]?tsx$/.test(id);
      const result = transform(code, {
        filename: id,
        babelrc: false,
        configFile: false,
        sourceMaps: true,
        plugins: [
          ['@babel/plugin-syntax-typescript', { isTSX: isTsx, allowNamespaces: true }],
          ['react-native-unistyles/plugin', { root }],
        ],
      });
      if (!result?.code) return null;
      return { code: result.code, map: result.map };
    },
  };
}
