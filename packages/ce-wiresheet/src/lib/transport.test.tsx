import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, waitFor, cleanup, act } from "@testing-library/react";
import { useValues } from "./store";
import { setRestTransport } from "./rest";
import CeEditor from "../CeEditor";
import type {
  EngineTransport,
  EngineRequest,
  EngineStream,
  StreamHandlers,
} from "./transport";
import type { SchemaMessage } from "./engine-types";
import { MSG_SNAPSHOT } from "./engine-types";
import type { DecodedFrame } from "./wire";

// The interface's conformance spec, written where a test transport is a
// legitimate library feature (upstream), NOT an LB fake: it proves `CeEditor`
// renders a real tree and applies a real value frame through the injected
// `EngineTransport` seam alone — touching ZERO `fetch`/`WebSocket` globals. If
// the editor still reached past the seam to a raw fetch or `new WebSocket`, the
// spies below would throw and fail this test.
//
// This is the exit gate for slice-1-wiresheet-transport-seam.md: "a test renders
// the editor against MockTransport with zero fetch/WebSocket globals touched."

// A root (uid 0) holding one visible child component with two value properties.
// Shape mirrors ce-rest's `ReadNodesResponse` (what getRootNodes unwraps).
const CHILD_UID = 42;
const IN_PROP_UID = 100;
const OUT_PROP_UID = 101;

function seededTree() {
  return {
    nodes: [
      {
        name: "root",
        uid: 0,
        type: "sys::Folder",
        path: "/",
        parent: 0,
        properties: {},
        children: [
          {
            name: "Adder",
            uid: CHILD_UID,
            type: "math::Add",
            path: "/Adder",
            parent: 0,
            metadata: { position: { x: 120, y: 80 } },
            properties: {
              in1: { uid: IN_PROP_UID, componentUid: CHILD_UID, category: 1, value: 0, statusFlags: 0 },
              out: { uid: OUT_PROP_UID, componentUid: CHILD_UID, category: 2, value: 0, statusFlags: 0 },
            },
          },
        ],
      },
    ],
    edges: [],
  };
}

// Captures the stream handlers so the test can drive schema + frame pushes by
// hand — no socket, no timers.
class MockStream implements EngineStream {
  handlers: StreamHandlers;
  sessionId: string | null = null;
  constructor(handlers: StreamHandlers) {
    this.handlers = handlers;
  }
  setSubscriptions(): void {}
  setPropSubscriptions(): void {}
  setTickHz(): void {}
  getTickHz(): number | null {
    return null;
  }
  close(): void {}
}

class MockTransport implements EngineTransport {
  requests: EngineRequest[] = [];
  stream: MockStream | null = null;

  // Arrow-bound so the editor's duck-typed `openStream`/`request` casts (which
  // may detach the method from the instance) keep `this`.
  request = async (req: EngineRequest): Promise<unknown> => {
    this.requests.push(req);
    if (req.method === "GET" && req.path.startsWith("/nodes")) return seededTree();
    if (req.method === "GET" && req.path === "/schema") return [];
    if (req.method === "GET" && req.path.startsWith("/edges")) return [];
    if (req.method === "GET" && req.path.startsWith("/changelog")) {
      return { undoable: [], redoable: [] };
    }
    // Any other read the editor issues at bootstrap: a benign empty payload.
    return {};
  };

  openStream = (handlers: StreamHandlers): EngineStream => {
    this.stream = new MockStream(handlers);
    return this.stream;
  };

  // Drive a schema message (binds the session; loads the slim decode table).
  pushSchema() {
    const msg: SchemaMessage = {
      type: "schema",
      sessionId: "mock-session",
      resumed: false,
      currentSeq: 0,
      properties: [
        { uid: IN_PROP_UID, dataType: 0, statusFlags: 0 },
        { uid: OUT_PROP_UID, dataType: 0, statusFlags: 0 },
      ],
    };
    this.stream!.handlers.onSchema(msg);
  }

  // Drive a decoded value frame: OUT_PROP gets the value 7.
  pushFrame() {
    const frame: DecodedFrame = {
      msgType: MSG_SNAPSHOT,
      timestampMs: 0,
      sections: [
        {
          typeTag: 0, // a value section (not TYPE_STATUS)
          uids: new Uint32Array([OUT_PROP_UID]),
          values: new Float64Array([7]),
        },
      ],
    };
    this.stream!.handlers.onFrame(frame);
  }
}

let fetchSpy: ReturnType<typeof vi.fn>;
let wsSpy: ReturnType<typeof vi.fn>;

// A spy that fails the test if the editor ever reaches for a network global.
function leakGuard(name: string): ReturnType<typeof vi.fn> {
  return vi.fn((..._args: unknown[]): unknown => {
    throw new Error(`CeEditor touched global ${name} — the transport seam leaked`);
  });
}

beforeEach(() => {
  // Any touch of these globals fails the test — the whole point of the seam is
  // that an injected transport reaches the network, the editor never does.
  fetchSpy = leakGuard("fetch");
  wsSpy = leakGuard("WebSocket");
  vi.stubGlobal("fetch", fetchSpy);
  vi.stubGlobal("WebSocket", wsSpy);
  // React Flow observes container size; jsdom has no ResizeObserver.
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  useValues.getState().reset();
});

describe("EngineTransport seam", () => {
  it("renders a tree and applies a frame with zero fetch/WebSocket globals", async () => {
    const transport = new MockTransport();
    // CeEditor sets this itself via the prop, but set it explicitly too so the
    // rest.ts wrappers route through the mock from the very first request.
    setRestTransport(transport);

    const { findByText } = render(<CeEditor base="mock://engine" transport={transport} />);

    // 1. The tree rendered: the seeded child component's name is on the canvas.
    //    This came through transport.request("GET /nodes"), NOT global fetch.
    await findByText("Adder");
    expect(transport.requests.some((r) => r.path.startsWith("/nodes"))).toBe(true);

    // 2. Drive the stream: schema binds the session, then a value frame lands.
    act(() => {
      transport.pushSchema();
      transport.pushFrame();
    });

    // The frame's value reached the values store through the seam's onFrame.
    await waitFor(() => {
      expect(useValues.getState().values.get(OUT_PROP_UID)).toBe(7);
    });

    // 3. The globals were never touched — the editor stayed behind the seam.
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(wsSpy).not.toHaveBeenCalled();
  });
});
