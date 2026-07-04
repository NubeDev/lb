// Streaming-fetch polyfills for the sdk's SSE client. RN's stock `fetch` (whatwg-fetch over the
// native networking bridge) buffers whole responses — no `res.body` streams — so a live SSE feed
// would never emit. `react-native-fetch-api` re-implements fetch over the incremental networking
// events and exposes real ReadableStream bodies; it needs the web-streams + TextEncoder polyfills
// beneath it. Must load before anything imports `@nube/app-sdk`.

import { polyfillGlobal } from 'react-native/Libraries/Utilities/PolyfillFunctions';
import { ReadableStream } from 'web-streams-polyfill/ponyfill/es6';
import { TextDecoder, TextEncoder } from 'text-encoding';
import { fetch as streamingFetch, Headers, Request, Response } from 'react-native-fetch-api';

polyfillGlobal('ReadableStream', () => ReadableStream);
polyfillGlobal('TextDecoder', () => TextDecoder);
polyfillGlobal('TextEncoder', () => TextEncoder);
polyfillGlobal('Headers', () => Headers);
polyfillGlobal('Request', () => Request);
polyfillGlobal('Response', () => Response);
polyfillGlobal(
  'fetch',
  () => (input: unknown, init?: Record<string, unknown>) =>
    // Ask the native layer for incremental (streamed) text responses — the SSE case.
    streamingFetch(input, { reactNative: { textStreaming: true }, ...init }),
);
