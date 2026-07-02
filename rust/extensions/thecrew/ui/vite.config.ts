/// <reference types="vitest" />
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

import { federationRemote } from "./federation-remote.preset";

// The federation-remote build (thecrew-extension-scope.md §UI lift): a single ESM `remoteEntry.js`
// the shell dynamic-imports (ui-federation scope), the proof-panel pattern. The two federation
// invariants — define `process.env.NODE_ENV` (Vite lib builds don't inject it; three.js/@react-three
// read it → "process is not defined" live) and externalise React (the shell import map supplies the
// single copy → no "Invalid hook call") — now live in the shared `federation-remote.preset`, so a new
// bundling extension no longer rediscovers them live (finding 5). three.js IS bundled — the payoff is
// that only this remote carries the ~1MB engine. Compiled Tailwind CSS is injected at runtime by
// `remoteEntry.ts` via a `?inline` import (cssCodeSplit off → one CSS string).
//
// `pnpm dev` still runs the standalone playground (index.html → main.tsx); the lib build below is
// what `build.sh` emits for publish.
export default defineConfig(federationRemote({ plugins: [react(), tailwindcss()] }));
