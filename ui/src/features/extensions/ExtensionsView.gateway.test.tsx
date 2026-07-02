// The unified extensions console, driven against a REAL spawned gateway (no fake — CLAUDE §9). Installs
// are seeded as **real `Install` records** through the test gateway's `/_seed/extension` route (a real
// `lb_assets::record_install` write, the same path a real install lands on), then read back over the
// real `GET /extensions` route (`ext.list`). Lifecycle (enable/disable/uninstall) routes through the
// real `POST /extensions/{ext}/{enable,disable}` + `DELETE /extensions/{ext}` routes via the view's
// ConfirmDestructive flow. Covers: both tiers list with live state; stop→disabled / start→ok;
// uninstall behind the second gate removes the row; and workspace isolation.
//
// What the real gateway changes vs the old fake: a SEEDED install writes only the durable `Install`
// record — no runtime process is spawned. So a native row (which derives `running` from the live
// SidecarMap) reports `running=false → health="stopped"` and `restart_count=0`, not the fake's
// simulated `running=true`. A wasm row has no separate process, so `running` follows `enabled`
// (`enabled → health="ok"`). Assertions below reflect that real ext.list shape (see
// crates/host/src/ext/row.rs::from_install).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ExtensionsView } from "./ExtensionsView";
import { useRealGateway, signInReal, seedExtension } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `ext-console-${n++}`;

beforeAll(() => useRealGateway());

describe("ExtensionsView (real gateway)", () => {
  it("lists both tiers with live state (wasm runs when enabled; a seeded native has no process)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "hello", version: "v2", tier: "wasm", enabled: true });
    await seedExtension({ ext: "echo-sidecar", version: "v1", tier: "native", enabled: true });
    render(<ExtensionsView ws={ws} />);

    expect(await screen.findByText("hello@v2")).toBeInTheDocument();
    expect(screen.getByText("echo-sidecar@v1")).toBeInTheDocument();
    expect(screen.getAllByText("wasm").length).toBeGreaterThan(0);
    expect(screen.getByText("native")).toBeInTheDocument();
    // Native restart count is surfaced; a seeded install with no running process has 0 restarts.
    expect(screen.getByTestId("restarts-echo-sidecar")).toHaveTextContent("restarts 0");
    // wasm enabled → running → health "ok". (The old fake-only "native running=true" is not
    // reproducible: seeding writes the Install record, it spawns no sidecar — so native is "stopped".)
    const helloRow = screen.getByText("hello@v2").closest("li") as HTMLElement;
    expect(within(helloRow).getByText("ok")).toBeInTheDocument();
  });

  it("stop (disable) routes through a reversible confirm and flips health to disabled", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "hello", version: "v2", tier: "wasm", enabled: true });
    render(<ExtensionsView ws={ws} />);
    await screen.findByText("hello@v2");

    await user.click(screen.getByLabelText("stop hello"));
    expect(screen.getByText("reversible")).toBeInTheDocument();
    await user.click(screen.getByLabelText("confirm action"));

    // Disabled intent → health "disabled" (the boot reconciler won't auto-start it).
    expect(await screen.findByText("disabled")).toBeInTheDocument();
    // Re-start it (enable) — a wasm component is runnable again → health "ok".
    await user.click(screen.getByLabelText("start hello"));
    expect(await screen.findByText("ok")).toBeInTheDocument();
  });

  it("uninstall requires the second gate (binary eviction) and removes the row", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "echo-sidecar", version: "v1", tier: "native", enabled: true });
    render(<ExtensionsView ws={ws} />);
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

  // The signed-artifact upload path (verify-before-store: a valid artifact installs, a
  // tampered/unsigned/malformed one is rejected and nothing is stored) needs a real signed artifact,
  // which the UI never mints. That verification is exercised end-to-end in the Rust registry/host
  // tests (lb_registry verify-before-store + the host publish route); the UI's local malformed-JSON
  // guard lives in UploadArtifact. Not reproduced here — it can't run without a fake. (CLAUDE §9)

  it("shows the Reset affordance only for a native sidecar that has restarted", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // A native install with restarts (a crash-looped sidecar) → the Reset button appears.
    await seedExtension({
      ext: "echo-sidecar",
      version: "v1",
      tier: "native",
      enabled: true,
      restart_count: 3,
    });
    // A wasm install (no process, no restart budget) → never a Reset button.
    await seedExtension({ ext: "hello", version: "v2", tier: "wasm", enabled: true });
    render(<ExtensionsView ws={ws} />);

    await screen.findByText("echo-sidecar@v1");
    // The restart count is surfaced from the seeded native_status record.
    expect(screen.getByTestId("restarts-echo-sidecar")).toHaveTextContent("restarts 3");
    // The recovery affordance is present for the restarted native, absent for the wasm row.
    expect(screen.getByLabelText("reset echo-sidecar")).toBeInTheDocument();
    expect(screen.queryByLabelText("reset hello")).not.toBeInTheDocument();
  });

  it("hides Reset for a native sidecar with a clean (zero) restart count", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "echo-sidecar", version: "v1", tier: "native", enabled: true });
    render(<ExtensionsView ws={ws} />);

    await screen.findByText("echo-sidecar@v1");
    expect(screen.getByTestId("restarts-echo-sidecar")).toHaveTextContent("restarts 0");
    expect(screen.queryByLabelText("reset echo-sidecar")).not.toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's installs", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedExtension({ ext: "hello", version: "v2", tier: "wasm", enabled: true });
    const { unmount } = render(<ExtensionsView ws={wsA} />);
    await screen.findByText("hello@v2");
    unmount();

    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    render(<ExtensionsView ws={wsB} />);
    expect(await screen.findByText("No extensions installed.")).toBeInTheDocument();
  });
});
