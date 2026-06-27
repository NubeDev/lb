// The unified extensions console (admin-console + lifecycle-management scopes) — SUPERSEDES the demo
// RegistryView/NativeView; this ports their coverage (both tiers render with live state, lifecycle
// reflects in the table, the native restart count is surfaced) onto the real `ext_*` surface. Driven
// through the real hook + api + the contract-identical ext fake.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ExtensionsView } from "./ExtensionsView";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { __resetExtFake, __seedExt } from "@/lib/ipc/ext.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => __resetExtFake());
afterEach(() => __resetExtFake());

describe("ExtensionsView", () => {
  it("lists both tiers with live state, incl. the native restart count", async () => {
    signIn("acme");
    __seedExt({ ext: "hello", version: "v2", tier: "wasm" });
    __seedExt({ ext: "echo-sidecar", version: "v1", tier: "native", enabled: true, running: true, restart_count: 0 });
    render(<ExtensionsView ws="acme" />);

    expect(await screen.findByText("hello@v2")).toBeInTheDocument();
    expect(screen.getByText("echo-sidecar@v1")).toBeInTheDocument();
    expect(screen.getAllByText("wasm").length).toBeGreaterThan(0);
    expect(screen.getByText("native")).toBeInTheDocument();
    expect(screen.getByTestId("restarts-echo-sidecar")).toHaveTextContent("restarts 0");
  });

  it("stop (disable) routes through a reversible confirm and flips health to stopped", async () => {
    const user = userEvent.setup();
    signIn("acme");
    __seedExt({ ext: "hello", version: "v2", tier: "wasm" });
    render(<ExtensionsView ws="acme" />);
    await screen.findByText("hello@v2");

    await user.click(screen.getByLabelText("stop hello"));
    expect(screen.getByText("reversible")).toBeInTheDocument();
    await user.click(screen.getByLabelText("confirm action"));

    // After disable + a (simulated) reboot the reconciler won't auto-start it → not running.
    expect(await screen.findByText("disabled")).toBeInTheDocument();
    // Re-start it (enable) — back to ok.
    await user.click(screen.getByLabelText("start hello"));
    expect(await screen.findByText("ok")).toBeInTheDocument();
  });

  it("uninstall requires the second gate (binary eviction) and removes the row", async () => {
    const user = userEvent.setup();
    signIn("acme");
    __seedExt({ ext: "echo-sidecar", version: "v1", tier: "native", enabled: true, running: true });
    render(<ExtensionsView ws="acme" />);
    await screen.findByText("echo-sidecar@v1");

    await user.click(screen.getByLabelText("uninstall echo-sidecar"));
    expect(screen.getByText("irreversible")).toBeInTheDocument();
    expect(screen.getByTestId("consequence")).toHaveTextContent(/evicts the cached binary/i);
    // blocked until the second gate is acknowledged
    expect(screen.getByLabelText("confirm action")).toBeDisabled();
    await user.click(screen.getByLabelText("acknowledge"));
    await user.click(screen.getByLabelText("confirm action"));

    expect(await screen.findByText("No extensions installed.")).toBeInTheDocument();
  });

  it("uploads a signed artifact — it appears installed (verify-before-store on the host)", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<ExtensionsView ws="acme" />);
    await screen.findByText("No extensions installed.");

    const artifact = {
      ext_id: "hvac",
      version: "v1",
      manifest_toml: 'name = "hvac"',
      wasm: [0, 1, 2],
      digest_hex: "aa",
      publisher_key_id: "pub-1",
      signature: [9, 9],
      __trusted: true,
    };
    const file = new File([JSON.stringify(artifact)], "hvac.json", { type: "application/json" });
    await user.upload(screen.getByLabelText("artifact file"), file);

    expect(await screen.findByText("hvac@v1")).toBeInTheDocument();
  });

  it("rejects a tampered artifact — nothing is installed (verify-before-store)", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<ExtensionsView ws="acme" />);
    await screen.findByText("No extensions installed.");

    const tampered = {
      ext_id: "evil",
      version: "v1",
      manifest_toml: 'name = "evil"',
      wasm: [0],
      digest_hex: "cc",
      publisher_key_id: "evil",
      signature: [0],
      __trusted: false, // fails the host's signature check
    };
    const file = new File([JSON.stringify(tampered)], "evil.json", { type: "application/json" });
    await user.upload(screen.getByLabelText("artifact file"), file);

    // The error surfaces and nothing is installed.
    expect(await screen.findByRole("alert")).toHaveTextContent(/unverified/i);
    expect(screen.queryByText("evil@v1")).not.toBeInTheDocument();
    expect(screen.getByText("No extensions installed.")).toBeInTheDocument();
  });

  it("a malformed upload file uploads nothing and shows a local error", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<ExtensionsView ws="acme" />);
    await screen.findByText("No extensions installed.");

    const bad = new File(["not json"], "bad.json", { type: "application/json" });
    await user.upload(screen.getByLabelText("artifact file"), bad);

    expect(await screen.findByText(/invalid JSON/i)).toBeInTheDocument();
    expect(screen.getByText("No extensions installed.")).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's installs", async () => {
    signIn("ws-a");
    __seedExt({ ext: "hello", version: "v2", tier: "wasm" });
    const { unmount } = render(<ExtensionsView ws="ws-a" />);
    await screen.findByText("hello@v2");
    unmount();

    signIn("ws-b");
    render(<ExtensionsView ws="ws-b" />);
    expect(await screen.findByText("No extensions installed.")).toBeInTheDocument();
  });
});
