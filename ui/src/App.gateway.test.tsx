// Full shell routing over the REAL gateway harness (routing scope; CLAUDE §9). These tests mount
// App at hash URLs and assert the routed page + args render through a real signed session.

import { beforeAll, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { App } from "./App";
import { gatewayUrl } from "@/lib/ipc/http";
import {
  signInReal,
  signInWithCaps,
  useRealGateway,
} from "@/test/gateway-session";

let n = 0;
const nextWs = () => `routing-ui-${n++}`;

const MEMBER_CAPS = [
  "bus:chan/*:pub",
  "bus:chan/*:sub",
  "mcp:members.list:call",
  "mcp:workspace.list:call",
];

beforeAll(() => useRealGateway());

function goHash(hash: string) {
  window.history.replaceState(null, "", `/${hash}`);
}

describe("App routing (real gateway)", () => {
  it("renders a route from the hash", async () => {
    goHash("#/members");
    await signInReal("user:ada", nextWs());

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Members" })).toBeInTheDocument();
  });

  it("validates malformed dashboard search params to defaults", async () => {
    goHash("#/dashboards?from=garbage&to=also-bad");
    await signInReal("user:ada", nextWs());

    render(<App />);

    const from = (await screen.findByLabelText("dashboard range from")) as HTMLInputElement;
    const to = screen.getByLabelText("dashboard range to") as HTMLInputElement;
    expect(from.value).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(to.value).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(from.value).not.toBe("garbage");
  });

  it("redirects a cap-denied admin route and the forged admin verb is gateway-denied", async () => {
    goHash("#/admin");
    const session = await signInWithCaps("user:member", nextWs(), MEMBER_CAPS);
    await fetch(`${gatewayUrl()}/channels`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${session.token}`,
      },
      body: JSON.stringify({ channel: "general" }),
    });

    render(<App />);

    expect(await screen.findByRole("heading", { name: "#general" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "general" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Admin")).not.toBeInTheDocument();

    const denied = await fetch(`${gatewayUrl()}/admin/users`, {
      headers: { authorization: `Bearer ${session.token}` },
    });
    expect(denied.status).toBe(403);
  });

  it("rewrites a pasted link whose /t/<ws> targets another workspace to the recipient's own", async () => {
    // The attack: a link minted for workspace `victim-ws` is opened by a session for `recipient-c`.
    // The `/t/<ws>` segment must NOT select the workspace — the guard rewrites it to the token's
    // workspace, and the channel data shown is the recipient's, never the victim's.
    goHash("#/t/victim-ws/channels?c=ops");
    await signInReal("user:cara", "recipient-c");
    render(<App />);

    expect(await screen.findByRole("heading", { name: "#ops" })).toBeInTheDocument();
    expect(screen.getByTitle("Workspace recipient-c")).toBeInTheDocument();
    // The URL itself is corrected: the victim workspace is gone, the recipient's is in its place.
    await waitFor(() =>
      expect(window.location.hash).toBe("#/t/recipient-c/channels?c=ops"),
    );
  });

  it("does not take workspace from a pasted URL", async () => {
    goHash("#/channels?c=ops");
    await signInReal("user:ada", "recipient-a");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "#ops" })).toBeInTheDocument();
    expect(screen.getByTitle("Workspace recipient-a")).toBeInTheDocument();

    cleanup();
    await signInReal("user:ben", "recipient-b");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "#ops" })).toBeInTheDocument();
    expect(screen.getByTitle("Workspace recipient-b")).toBeInTheDocument();
  });

  it("supports back, forward, and reload at the routed page", async () => {
    const user = userEvent.setup();
    goHash("#/channels?c=general");
    await signInReal("user:ada", nextWs());
    const { unmount } = render(<App />);

    await user.click(await screen.findByLabelText("Members"));
    expect(await screen.findByRole("heading", { name: "Members" })).toBeInTheDocument();

    await user.click(screen.getByLabelText("Dashboards"));
    expect(await screen.findByLabelText("dashboard range from")).toBeInTheDocument();

    window.history.back();
    expect(await screen.findByRole("heading", { name: "Members" })).toBeInTheDocument();

    window.history.forward();
    expect(await screen.findByLabelText("dashboard range to")).toBeInTheDocument();

    unmount();
    render(<App />);
    expect(await screen.findByLabelText("dashboard range from")).toBeInTheDocument();
    await waitFor(() =>
      expect(window.location.hash).toMatch(/^#\/t\/[^/]+\/dashboards/),
    );
  });

  it("copies the dashboard URL with range args from the Share affordance", async () => {
    const user = userEvent.setup();
    const writes: string[] = [];
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: (text: string) => { writes.push(text); return Promise.resolve(); } },
    });
    goHash("#/dashboards?from=2026-01-01&to=2026-03-31");
    await signInReal("user:ada", nextWs());

    render(<App />);
    await user.type(await screen.findByLabelText("new dashboard title"), "Ops");
    await user.click(screen.getByLabelText("create dashboard"));
    await user.click(await screen.findByLabelText("copy dashboard link"));

    // The shared link is now tenant-prefixed (`/t/<ws>/…`). The workspace segment is a deep-link
    // hint only — a recipient in another workspace is redirected to their own by the route guard;
    // the gateway re-derives the real workspace from the verified token regardless (§7).
    expect(writes[0]).toMatch(/#\/t\/[^/]+\/dashboards\?from=2026-01-01&to=2026-03-31/);
  });
});
