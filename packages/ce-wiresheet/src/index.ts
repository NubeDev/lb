// `@nube/ce-wiresheet` — public surface.
//
// The editor is a single component parameterised by the engine's REST origin
// (`base`, e.g. `http://192.168.1.50:7878`). It speaks the control engine's
// REST `/api/v0` + binary-WebSocket `/ws` protocol and renders the wiresheet
// on React Flow. Everything below `CeEditor` (the rest/ws/wire/store layer and
// the node/edge components) is internal.
//
// Consumed two ways:
//   - the standalone dev harness (index.html → src/standalone.tsx) for fast HMR,
//   - imported as a library by a host app (e.g. rbx) — `import { CeEditor }` plus
//     `import '@nube/ce-wiresheet/style.css'`.

import "./wiresheet.css"; // bundles Tailwind + the editor's tokens into the lib stylesheet

export { default as CeEditor } from "./CeEditor";
export { setEngineBase, setRestTransport } from "./lib/rest";
export { DirectTransport, wsUrlFromBase, RestError } from "./lib/transport-direct";

// The transport seam. A host (e.g. the LB MCP/Zenoh bridge, S7) implements
// `EngineTransport` outside this package and injects it via `CeEditor`'s
// `transport` prop; `DirectTransport` (above) is the default direct-to-CE
// implementation. See lib/transport.ts.
export type {
  EngineTransport,
  EngineStream,
  EngineRequest,
  EngineRequestError,
  StreamHandlers,
  RequestMethod,
} from "./lib/transport";

export type {
  Component,
  Edge,
  FlexValue,
} from "./lib/engine-types";
