// Web entry for the RN-Web preview. Renders the SAME App.tsx + screens react-native-web maps the
// `react-native` primitives to DOM. The node URL is prefilled from ?node= (default the local dev
// gateway) so the login screen is one tap. Native-only modules (keychain, the streaming-fetch
// polyfill) are swapped for web equivalents via vite aliases — see vite.config.web.mts.

// No streaming-fetch polyfill on web: the browser's native fetch already streams (res.body is a
// real ReadableStream) and ships TextDecoder, so the sdk's SSE client works as-is.
import React from 'react';
import { AppRegistry } from 'react-native';
import { setNodeUrl } from '../src/lib/node-url.store';
import App from '../src/App';

const params = new URLSearchParams(location.search);
setNodeUrl(params.get('node') ?? 'http://127.0.0.1:8080');

AppRegistry.registerComponent('LazybonesShell', () => App);
AppRegistry.runApplication('LazybonesShell', {
  rootTag: document.getElementById('root'),
});
