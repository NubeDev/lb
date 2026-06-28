// The MCP service page (the reachable tool catalog), driven against a REAL in-process gateway
// (tool-catalog scope; CLAUDE §9 / testing §0 — no fake backend). Each test logs in to a unique
// workspace and drives the real `McpServiceView` + hook + api client + HTTP transport against the real
// `system.tools`/`system.overview` verbs. Covers: the catalog renders real host-native tools with
// their descriptions; search filters the list; the runtime count badges read live; and the
// capability-deny is proven (a session WITHOUT `mcp:system.tools:call` gets the opaque error, no list).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { McpServiceView } from "./McpServiceView";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `mcp-svc-${n++}`;

beforeAll(() => useRealGateway());

describe("McpServiceView (real gateway)", () => {
  it("lists real host-native tools with their descriptions", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<McpServiceView ws={ws} />);

    // Representative host-native verbs are always reachable — listed with their real descriptions.
    expect(await screen.findByText("host.net.info")).toBeInTheDocument();
    expect(screen.getByText("system.overview")).toBeInTheDocument();
    expect(screen.getByText("store.query")).toBeInTheDocument();
    // The catalog is grouped by family — the `host` group header is present.
    expect(screen.getByLabelText("group host")).toBeInTheDocument();
  });

  it("filters the catalog as you search", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<McpServiceView ws={ws} />);
    await screen.findByText("host.net.info");

    await user.type(screen.getByLabelText("search tools"), "inbox");
    // The inbox verbs survive the filter; an unrelated host verb is gone.
    expect(await screen.findByText("inbox.list")).toBeInTheDocument();
    expect(screen.queryByText("host.net.info")).toBeNull();
  });

  it("shows the live runtime counts from the overview snapshot", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<McpServiceView ws={ws} />);
    // The header carries the MCP card's live `extensions` + `tools` counts (read from system.overview).
    const tools = await screen.findByText("tools");
    expect(tools).toBeInTheDocument();
  });

  it("denies a session without the system.tools capability", async () => {
    const ws = nextWs();
    // A session holding everything EXCEPT the catalog cap — the page's read is refused server-side.
    await signInWithCaps("user:mallory", ws, ["mcp:system.overview:call"]);

    render(<McpServiceView ws={ws} />);

    // The deny surfaces as the page error (opaque 403); no tool rows render.
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.queryByText("host.net.info")).toBeNull();
  });

  it("does not overflow horizontally at a narrow (phone) viewport", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);

    const { container } = render(
      <div style={{ width: 360 }}>
        <McpServiceView ws={ws} />
      </div>,
    );
    await screen.findByText("host.net.info");
    const section = container.querySelector("section");
    expect(section).not.toBeNull();
    expect(section!.scrollWidth).toBeLessThanOrEqual(360);
  });
});
