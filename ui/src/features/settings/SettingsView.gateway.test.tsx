// Real-gateway tests for the Settings surface (user-prefs + agent-config scopes). No mocks, no fake
// backend (rule 9): every read/write hits a spawned `node` gateway over a real signed session, and
// assertions are made against OBSERVABLE UI state after a real round-trip through the store.
//
// Covers:
//   - PREFERENCES round-trip: an admin sets language + date style + a unit override via `prefs.set`,
//     and a fresh mount reads them back (`prefs.get`) — the real record, not local component state.
//   - AGENT round-trip: an admin picks the default runtime + fills the model endpoint and saves via
//     `agent.config.set`; a fresh mount reads it back (`agent.config.get`).
//   - CAPABILITY GATE (display): a member WITHOUT `mcp:agent.config.set:call` sees the runtime picker
//     and endpoint fields DISABLED (view-only) — and the server would deny a write regardless.
//   - DENY (server is the wall): a member without the write cap is refused by the gateway on
//     `agent.config.set` (opaque), proving the UI gate is convenience only.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SettingsView } from "./SettingsView";
import { getPrefs } from "@/lib/prefs/get";
import { getAgentConfig, setAgentConfig } from "@/lib/agent/config.api";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `settings-${n++}`;

beforeAll(() => useRealGateway());

describe("SettingsView — Preferences (real gateway)", () => {
  it("saves the caller's own axes and reads them back", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInReal("user:ada", ws);

    render(<SettingsView ws={ws} caps={session.caps} />);

    // Set language = Español and date style = ISO, plus a temperature unit override.
    await user.selectOptions(await screen.findByLabelText("Language"), "es");
    await user.selectOptions(screen.getByLabelText("Date style"), "iso");
    await user.selectOptions(screen.getByLabelText("Override — temperature"), "fahrenheit");
    await user.click(screen.getByLabelText("save preferences"));
    await screen.findByText("Saved.");

    // The REAL record now carries them (a read against the live gateway, not component state).
    await waitFor(async () => {
      const stored = await getPrefs();
      expect(stored?.language).toBe("es");
      expect(stored?.date_style).toBe("iso");
      expect(stored?.unit_overrides?.temperature).toBe("fahrenheit");
    });

    // A fresh mount hydrates from the record — the selects show the persisted choices.
    cleanup();
    render(<SettingsView ws={ws} caps={session.caps} />);
    await waitFor(() => {
      expect((screen.getByLabelText("Language") as HTMLSelectElement).value).toBe("es");
      expect((screen.getByLabelText("Date style") as HTMLSelectElement).value).toBe("iso");
    });
  });
});

describe("SettingsView — Agent (real gateway)", () => {
  it("an admin picks a runtime + endpoint and it persists", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInReal("user:ada", ws);

    render(<SettingsView ws={ws} caps={session.caps} />);
    await user.click(screen.getByLabelText("Agent"));

    // The picker lists the node's runtimes (at least `default`); wait for the async fetch to populate
    // the option, then pick it explicitly and fill the endpoint.
    const picker = (await screen.findByLabelText("Default agent runtime")) as HTMLSelectElement;
    await waitFor(() =>
      expect(
        Array.from(picker.options).some((o) => o.value === "default"),
      ).toBe(true),
    );
    await user.selectOptions(picker, "default");
    await user.type(screen.getByLabelText("Provider"), "zaicoding");
    await user.type(screen.getByLabelText("Model"), "glm-4.6");
    await user.type(screen.getByLabelText("API key env var"), "ZAI_API_KEY");
    await user.click(screen.getByLabelText("save agent config"));
    await screen.findByText("Saved.");

    await waitFor(async () => {
      const cfg = await getAgentConfig();
      expect(cfg?.default_runtime).toBe("default");
      expect(cfg?.model_endpoint?.provider).toBe("zaicoding");
      // NAMES ONLY — the env var name is stored, never a key value.
      expect(cfg?.model_endpoint?.api_key_env).toBe("ZAI_API_KEY");
    });
  });

  it("a member without the write cap sees the agent controls read-only, and the server denies a write", async () => {
    const ws = nextWs();
    // Grant only the READ caps a normal member needs to render the tab — NOT `agent.config.set`.
    const session = await signInWithCaps("user:eve", ws, [
      "mcp:agent.config.get:call",
      "mcp:agent.runtimes:call",
    ]);

    render(<SettingsView ws={ws} caps={session.caps} />);
    const user = userEvent.setup();
    await user.click(screen.getByLabelText("Agent"));

    // The picker + endpoint fields are disabled; the "save" button is absent for a read-only viewer.
    expect((await screen.findByLabelText("Default agent runtime")) as HTMLSelectElement).toBeDisabled();
    expect(screen.getByLabelText("Provider")).toBeDisabled();
    expect(screen.queryByLabelText("save agent config")).not.toBeInTheDocument();

    // The server is the real wall: a direct write is refused (opaque) despite any client state.
    await expect(setAgentConfig({ default_runtime: "default" })).rejects.toThrow();
  });
});
