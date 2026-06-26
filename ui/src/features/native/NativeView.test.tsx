// The S7 native-tier (Tier-2 supervisor) story, at the UI level: install spawns + supervises a
// sidecar; restart bumps the surfaced restart COUNT and keeps it running (the supervision proof);
// stop flips it off; and a user without the grant is denied — the same `mcp:native.<verb>:call`
// gates the Rust native tests prove on the backend, surfaced through the real api → invoke → fake.
//
// We drive the actual `native.api` → `invoke` → fake path (no mock of the api): the fake mirrors the
// node's capability gate + supervision, so this exercises the allow/deny/restart branches the user
// actually hits.

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { NativeView } from "./NativeView";
import { __resetNativeFake } from "@/lib/ipc/native.fake";

const WS = "acme";
const INSTALL = "mcp:native.install:call";
const STATUS = "mcp:native.status:call";
const RESTART = "mcp:native.restart:call";
const STOP = "mcp:native.stop:call";
const ALL = [INSTALL, STATUS, RESTART, STOP];

beforeEach(() => __resetNativeFake());
afterEach(() => __resetNativeFake());

describe("NativeView supervision + capability gates", () => {
  it("installs (spawns) a sidecar — it shows running with 0 restarts", async () => {
    render(<NativeView ws={WS} extId="echo-sidecar" author="user:ada" caps={ALL} />);
    await screen.findByText("Not installed on this node.");

    await userEvent.click(screen.getByRole("button", { name: "Install" }));
    await waitFor(() => expect(screen.getByTestId("restart-count")).toHaveTextContent("0"));
    expect(screen.getByText("started")).toBeInTheDocument();
  });

  it("restart bumps the surfaced restart count (the supervision proof)", async () => {
    render(<NativeView ws={WS} extId="echo-sidecar" author="user:ada" caps={ALL} />);
    await userEvent.click(screen.getByRole("button", { name: "Install" }));
    await waitFor(() => expect(screen.getByTestId("restart-count")).toHaveTextContent("0"));

    await userEvent.click(screen.getByRole("button", { name: "Restart" }));
    await waitFor(() => expect(screen.getByTestId("restart-count")).toHaveTextContent("1"));
  });

  it("stop flips the sidecar off", async () => {
    render(<NativeView ws={WS} extId="echo-sidecar" author="user:ada" caps={ALL} />);
    await userEvent.click(screen.getByRole("button", { name: "Install" }));
    await waitFor(() => expect(screen.getByText("yes")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(screen.getByText("stopped")).toBeInTheDocument());
  });

  it("a user WITHOUT the install grant is denied", async () => {
    render(<NativeView ws={WS} extId="echo-sidecar" author="user:cleo" caps={[STATUS]} />);
    await screen.findByText("Not installed on this node.");

    await userEvent.click(screen.getByRole("button", { name: "Install" }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("You don't have access to this sidecar."),
    );
  });
});
