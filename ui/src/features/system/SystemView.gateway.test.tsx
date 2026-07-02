// The System page (admin topology + status console), driven against a REAL in-process gateway
// (system-map scope; CLAUDE §9 / testing §0 — no fake backend). Each test logs in to a UNIQUE
// workspace, seeds real records through the real seed/write path, and drives the real `SystemView` +
// hook + api client + HTTP transport. Covers: the fixed status grid renders with live numbers; a
// dead-lettered outbox effect surfaces the outbox card as Degraded; Refresh re-fetches; the Grid/Graph
// toggle mounts the react-flow topology; and a narrow-viewport responsive smoke (no horizontal
// overflow). (The admin-only capability deny is proven server-side in the Rust route/host tests; the
// nav cap-gating is unit-tested in NavGating.test.ts.)

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SystemView } from "./SystemView";
import { writeSample } from "@/lib/ingest/ingest.api";
import { useRealGateway, signInReal, seedOutbox } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `system-${n++}`;

beforeAll(() => useRealGateway());

describe("SystemView (real gateway)", () => {
  it("renders the fixed status grid with live numbers", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);
    // Two committed samples → the store card reports real, non-zero rows.
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 1, seq: 1, payload: 60.0 });
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 2, seq: 2, payload: 61.4 });

    render(<SystemView ws={ws} />);

    // The fixed subsystem cards are always present (a missing card means "we forgot it").
    expect(await screen.findByLabelText("subsystem store")).toBeInTheDocument();
    expect(screen.getByLabelText("subsystem outbox")).toBeInTheDocument();
    expect(screen.getByLabelText("subsystem gateway")).toBeInTheDocument();

    // The store card carries a live, non-zero row count once the two samples commit. We assert
    // "> 0", not an exact number: one `writeSample` commits through the real ingest path (staging +
    // series tables + the sample rows), so the workspace's total row count is legitimately more than
    // the sample count — the earlier exact `2` only ever "passed" when the pre-fix `use_ns` race
    // dropped some of those writes into the wrong namespace (debugging/store/
    // concurrent-use-ns-namespace-race.md). The card's contract is "live, non-zero rows", not a
    // fixed total.
    const rows = screen.getByLabelText("store rows");
    const rowCount = Number(within(rows).getByText(/^\d+$/).textContent);
    expect(rowCount).toBeGreaterThan(0);
  });

  it("shows the outbox as Degraded when an effect is dead-lettered", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);
    await seedOutbox({
      id: "e1",
      target: "github-target",
      action: "open_pr",
      status: "dead-lettered",
      attempts: 5,
      max_attempts: 5,
      ts: 1,
    });

    render(<SystemView ws={ws} />);

    const outbox = await screen.findByLabelText("subsystem outbox");
    expect(within(outbox).getByLabelText("health Degraded")).toBeInTheDocument();
    expect(within(outbox).getByLabelText("outbox dead-letter")).toHaveTextContent("1");
  });

  it("refreshes the snapshot on demand", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<SystemView ws={ws} />);
    // The empty workspace starts with an Idle outbox (nothing flowing — not a fault).
    const outbox = await screen.findByLabelText("subsystem outbox");
    expect(within(outbox).getByLabelText("health Idle")).toBeInTheDocument();

    // Seed a dead-letter, then Refresh → the same card re-reads as Degraded (no remount).
    await seedOutbox({
      id: "e9",
      target: "github-target",
      action: "open_pr",
      status: "dead-lettered",
      attempts: 5,
      max_attempts: 5,
      ts: 2,
    });
    await user.click(screen.getByLabelText("refresh"));
    expect(
      await within(await screen.findByLabelText("subsystem outbox")).findByLabelText(
        "health Degraded",
      ),
    ).toBeInTheDocument();
  });

  it("reports real Zenoh transport stats on the bus card", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<SystemView ws={ws} />);

    const bus = await screen.findByLabelText("subsystem bus");
    // Real session stats, not handle-presence: peers + routers counts + this node's zid are present.
    expect(within(bus).getByLabelText("bus peers")).toBeInTheDocument();
    expect(within(bus).getByLabelText("bus routers")).toBeInTheDocument();
    expect(within(bus).getByLabelText("bus node zid")).toBeInTheDocument();
  });

  it("drills a card into the page that owns its subsystem", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);
    const navigated: string[] = [];

    render(
      <SystemView
        ws={ws}
        onNavigate={(s) => navigated.push(s)}
        allowedSurfaces={["outbox", "extensions", "data", "system-mcp", "system-acp"]}
      />,
    );

    // The outbox card is a control (it has a page); clicking it navigates to the outbox surface.
    const outbox = await screen.findByLabelText("open outbox");
    await user.click(outbox);
    expect(navigated).toContain("outbox");

    // The MCP + ACP cards now own service pages (tool-catalog scope) → they drill there.
    await user.click(await screen.findByLabelText("open mcp"));
    expect(navigated).toContain("system-mcp");
    await user.click(await screen.findByLabelText("open acp"));
    expect(navigated).toContain("system-acp");

    // The bus card has no dedicated page → it is NOT a link (no "open bus" control).
    expect(screen.queryByLabelText("open bus")).toBeNull();
  });

  it("opens the subsystem detail sheet when a no-page card is clicked", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<SystemView ws={ws} />);

    // The bus card has no owning page → clicking it opens the in-place detail sheet (not a navigation).
    await user.click(await screen.findByLabelText("subsystem bus"));
    const sheet = await screen.findByLabelText("bus detail");
    // The detail renders the subsystem's metrics and — for the bus — the live peer/router zid lists.
    const peersList = within(sheet).getByLabelText("bus peers list");
    expect(peersList).toBeInTheDocument();
    expect(within(sheet).getByLabelText("bus routers list")).toBeInTheDocument();
    // The list reflects the live mesh count from the real session — and must agree with the card's
    // own `peers` metric (both read the same `system.subsystem` snapshot).
    const peerCount = within(sheet).getByLabelText("bus peers").textContent?.match(/\d+/)?.[0];
    expect(peersList).toHaveTextContent(`peers (${peerCount})`);
  });

  it("does not overflow the detail sheet at a narrow (phone) viewport", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(
      <div style={{ width: 360 }}>
        <SystemView ws={ws} />
      </div>,
    );
    // `bus` is a no-page card (gateway/bus have no owning page) → clicking it opens the detail sheet.
    await user.click(await screen.findByLabelText("subsystem bus"));
    const sheet = await screen.findByLabelText("bus detail");
    expect(sheet.scrollWidth).toBeLessThanOrEqual(360);
  });

  it("renders the react-flow topology on toggle", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);

    render(<SystemView ws={ws} />);
    await screen.findByLabelText("subsystem store");

    await user.click(screen.getByRole("tab", { name: "Graph" }));
    expect(await screen.findByTestId("system-graph")).toBeInTheDocument();
  });

  it("does not overflow horizontally at a narrow (phone) viewport", async () => {
    const ws = nextWs();
    await signInReal("user:root", ws);

    const { container } = render(
      <div style={{ width: 360 }}>
        <SystemView ws={ws} />
      </div>,
    );
    await screen.findByLabelText("subsystem store");
    // The status grid is single-column on a phone — the section never exceeds its container width.
    const section = container.querySelector("section");
    expect(section).not.toBeNull();
    expect(section!.scrollWidth).toBeLessThanOrEqual(360);
  });
});
