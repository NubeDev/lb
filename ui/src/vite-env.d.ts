/// <reference types="vite/client" />

// Typed env for the gateway URL (S3): set for the browser build so `invoke.ts` routes to the
// real node over HTTP; unset in the Tauri shell and in tests (where the fake/IPC is used).
interface ImportMetaEnv {
  readonly VITE_GATEWAY_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
