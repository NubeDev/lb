// Web entry for the RN-Web preview. Renders the SAME App.tsx + screens react-native-web maps the
// `react-native` primitives to DOM. The node URL is prefilled from ?node= (default the local dev
// gateway) so the login screen is one tap. Native-only modules (keychain, the streaming-fetch
// polyfill) are swapped for web equivalents via vite aliases — see vite.config.web.mts.

// No streaming-fetch polyfill on web: the browser's native fetch already streams (res.body is a
// real ReadableStream) and ships TextDecoder, so the sdk's SSE client works as-is.
import React from 'react';
import { AppRegistry } from 'react-native';
import '../src/theme/unistyles'; // register themes before any StyleSheet.create runs
import { setNodeUrl } from '../src/lib/node-url.store';
import { setDevLogin } from '../src/lib/dev-defaults';
import App from '../src/App';

const params = new URLSearchParams(location.search);
// Default to the root `make dev` node on 8080 (the common dev loop); override with `?node=` — e.g.
// `?node=http://127.0.0.1:8087` for the app's own throwaway test_gateway (`make -C app dev`). The
// bare `ada` prefill works against either now (the gateway canonicalizes `ada` -> `user:ada`).
const node = params.get('node') ?? 'http://127.0.0.1:8080';
setNodeUrl(node);

// Seed the login prefill for the preview. `vite-plugin-react-native-web` hardcodes RN's `__DEV__`
// global to false, so `dev-defaults`' own `__DEV__` gate leaves the fields empty on web — we set
// them here instead (preview-only code, never in a device bundle). Override via ?user=/?ws=.
setDevLogin({
  user: params.get('user') ?? 'ada',
  workspace: params.get('ws') ?? 'acme',
  nodeUrl: node,
});

AppRegistry.registerComponent('LazybonesShell', () => App);
AppRegistry.runApplication('LazybonesShell', {
  rootTag: document.getElementById('root'),
});
