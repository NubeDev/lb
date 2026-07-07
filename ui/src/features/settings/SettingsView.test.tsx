import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider } from "@/lib/theme";
import { SettingsView, coerceSettingsTab } from "./SettingsView";

// SettingsView is now URL-driven: `tab` comes from the route (`/settings/<tab>`) and clicking a tab
// calls `onTabChange` (the router updates the URL). These unit tests prove the Theme tab is present and
// renders the customizer controls, and that the active tab follows the `tab` prop.

afterEach(() => {
  cleanup();
  localStorage.clear();
});

function renderSettings(tab: string, onTabChange = vi.fn()) {
  return render(
    <ThemeProvider>
      <SettingsView ws="acme" caps={[]} tab={tab} onTabChange={onTabChange} />
    </ThemeProvider>,
  );
}

describe("SettingsView tabs", () => {
  it("coerces an unknown tab segment to the default", () => {
    expect(coerceSettingsTab("theme")).toBe("theme");
    expect(coerceSettingsTab("agent")).toBe("agent");
    expect(coerceSettingsTab("preferences")).toBe("preferences");
    // workspace-branding scope: the new Branding tab is a valid deep-link target.
    expect(coerceSettingsTab("branding")).toBe("branding");
    expect(coerceSettingsTab("bogus")).toBe("preferences");
    expect(coerceSettingsTab(undefined)).toBe("preferences");
  });

  it("shows the Theme tab, and it renders the theme customizer controls", async () => {
    renderSettings("theme");
    // The Theme tab is active (from the URL) and its customizer controls render.
    expect(await screen.findByLabelText("Theme preset")).toBeInTheDocument();
    // The Layout sub-tab is reachable within the Theme tab (a `tab`-role trigger).
    expect(screen.getByRole("tab", { name: /Layout/i })).toBeInTheDocument();
  });

  it("shows the Branding tab and its read-only deny notice for a non-admin session", async () => {
    // caps=[] — no `mcp:prefs.set_default:call`, so the editor surfaces its read-only notice.
    renderSettings("branding");
    expect(await screen.findByText(/requires an administrator/i)).toBeInTheDocument();
    // The brand-name input is present but disabled (a non-admin can see but not edit).
    const name = await screen.findByLabelText("Site name");
    expect(name).toBeDisabled();
  });

  it("clicking a tab requests navigation (URL-driven, not internal state)", async () => {
    const user = userEvent.setup();
    const onTabChange = vi.fn();
    renderSettings("preferences", onTabChange);
    await user.click(screen.getByLabelText("Theme"));
    expect(onTabChange).toHaveBeenCalledWith("theme");
  });

  it("clicking the Branding tab requests navigation", async () => {
    const user = userEvent.setup();
    const onTabChange = vi.fn();
    renderSettings("preferences", onTabChange);
    await user.click(screen.getByLabelText("Branding"));
    expect(onTabChange).toHaveBeenCalledWith("branding");
  });
});
