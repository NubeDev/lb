// Type shims for untyped runtime modules the polyfill layer loads. Runtime-only — no API surface
// beyond what src/polyfills.ts touches.

declare module 'react-native/Libraries/Utilities/PolyfillFunctions' {
  export function polyfillGlobal(name: string, getValue: () => unknown): void;
}

declare module 'react-native-fetch-api';
declare module 'text-encoding';
declare module 'web-streams-polyfill/ponyfill/es6';
