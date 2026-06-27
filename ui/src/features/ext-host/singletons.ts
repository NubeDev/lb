// Publish the shell's live React singletons onto `globalThis.__lb*` at MODULE EVAL (ui-federation
// scope). The importmap shims (`ui/public/shims/*.mjs`) re-export from these globals, so an extension
// `remoteEntry.js` bundle — which externalises `react`/`react-dom`/`react-dom/client`/`react/jsx-runtime`
// — binds to the host's SINGLE React when it is dynamic-imported later. This replaces the old
// `@originjs/vite-plugin-federation` shared-scope mechanism, which shipped a second React into the
// remote and broke hooks ("Invalid hook call"). Imported first thing in `main.tsx` so the globals are
// set before App renders and long before any `ExtHost` effect imports a remote.

import * as ReactNS from "react";
import * as ReactDOMNS from "react-dom";
import * as ReactDOMClientNS from "react-dom/client";
import * as ReactJsxRuntimeNS from "react/jsx-runtime";

import {
  GLOBAL_REACT,
  GLOBAL_REACT_DOM,
  GLOBAL_REACT_DOM_CLIENT,
  GLOBAL_REACT_JSX_RUNTIME,
} from "./globals";

const g = globalThis as unknown as Record<string, unknown>;
g[GLOBAL_REACT] = ReactNS;
g[GLOBAL_REACT_DOM] = ReactDOMNS;
g[GLOBAL_REACT_DOM_CLIENT] = ReactDOMClientNS;
g[GLOBAL_REACT_JSX_RUNTIME] = ReactJsxRuntimeNS;
