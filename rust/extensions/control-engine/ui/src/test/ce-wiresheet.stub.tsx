// A TEST DOUBLE of the vendored `@nube/ce-wiresheet` package's SURFACE — NOT a fake of node/bridge
// behavior (rule 9). vitest aliases `@nube/ce-wiresheet` here (see vite.config.ts `test.alias`) so the
// unit tests don't have to load the heavy real editor (xyflow + codemirror, which need the built dist
// and a full DOM). The bridge itself — the only thing S7 actually authored — IS exercised for real
// against a stub bridge INTERFACE in bridge-transport.test.ts + frames.test.ts.
//
// The runtime build resolves `@nube/ce-wiresheet` to the REAL built dist (see vite.config.ts
// `resolve.alias`), so this file never ships. It exists ONLY to keep the unit tests fast + hermetic.
//
// The TYPES below are RE-EXPORTED from the real vendored source so the transport still type-checks
// against the genuine `EngineTransport`/`StreamHandlers`/`DecodedFrame` seam — a divergent copy here
// would defeat the point.

import type { EngineTransport } from "../../../../../../packages/ce-wiresheet/src/lib/transport";

export type {
  EngineTransport,
  EngineStream,
  EngineRequest,
  EngineRequestError,
  StreamHandlers,
  RequestMethod,
} from "../../../../../../packages/ce-wiresheet/src/lib/transport";
export type { DecodedFrame, DecodedSection, DecodedValue } from "../../../../../../packages/ce-wiresheet/src/lib/wire";
export type { TopologyMsg, SchemaMessage } from "../../../../../../packages/ce-wiresheet/src/lib/engine-types";
export {
  TYPE_STATUS,
  TYPE_F64,
  MSG_UPDATE,
  MSG_SNAPSHOT,
} from "../../../../../../packages/ce-wiresheet/src/lib/engine-types";

/** A trivial stand-in for the real vendored `CeEditor`. Renders a marker so the Page test can assert the
 *  editor mounts once an appliance is selected — and opens the injected transport's stream, proving the
 *  Page wires `openStream` (the live half) exactly as the real editor would. */
export function CeEditor({
  base,
  transport,
}: {
  base: string;
  transport?: EngineTransport;
}) {
  if (transport) {
    transport.openStream({
      onSchema: () => {},
      onTopology: () => {},
      onFrame: () => {},
      onStatus: () => {},
    });
  }
  return <div data-testid="ce-editor" data-base={base} />;
}
