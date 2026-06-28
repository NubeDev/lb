// The ACP service page (the adapter's static facts), driven against a REAL in-process gateway
// (tool-catalog scope; CLAUDE §9 / testing §0 — no fake backend). Each test logs in to a unique
// workspace and drives the real `AcpServiceView` + hook + api client + HTTP transport against the real
// `system.acp` verb. Covers: the adapter's real protocol version + handled methods + error codes
// render; and the capability-deny (a session WITHOUT `mcp:system.acp:call` gets the opaque error).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";

import { AcpServiceView } from "./AcpServiceView";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `acp-svc-${n++}`;

beforeAll(() => useRealGateway());

describe("AcpServiceView (real gateway)", () => {
  it("renders the adapter's real protocol version, methods, and error codes", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<AcpServiceView ws={ws} />);

    // The protocol badge + the handled session methods come straight from the host's `system.acp`.
    expect(await screen.findByText("v1")).toBeInTheDocument();
    expect(screen.getByText("session/prompt")).toBeInTheDocument();
    expect(screen.getByText("session/new")).toBeInTheDocument();
    // The Methods + Error codes sections render.
    expect(screen.getByLabelText("Methods")).toBeInTheDocument();
    expect(screen.getByLabelText("Error codes")).toBeInTheDocument();
  });

  it("denies a session without the system.acp capability", async () => {
    const ws = nextWs();
    await signInWithCaps("user:mallory", ws, ["mcp:system.overview:call"]);

    render(<AcpServiceView ws={ws} />);

    // The deny surfaces as the page error (opaque 403); no facts render.
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.queryByText("session/prompt")).toBeNull();
  });
});
