import { describe, expect, it } from "vitest";

import { fullPathForSurface, surfaceForPath } from "./surface";

// The tenant-prefixed deep-link ↔ surface mapping. Locks in that a Settings sub-tab URL
// (`/settings/<tab>`) still resolves to the `settings` surface — so it keeps the same nav-active state
// and capability gate as the bare `/settings`, exactly like `/system/mcp` → `system`.

describe("surfaceForPath", () => {
  it("maps a bare core path to its surface", () => {
    expect(surfaceForPath("/t/acme/settings")).toBe("settings");
    expect(surfaceForPath("/t/acme/channels")).toBe("channels");
  });

  it("maps a Settings sub-tab deep link to the settings surface (prefix match)", () => {
    expect(surfaceForPath("/t/acme/settings/theme")).toBe("settings");
    expect(surfaceForPath("/t/acme/settings/agent")).toBe("settings");
    expect(surfaceForPath("/t/acme/settings/preferences")).toBe("settings");
    // An unknown sub-tab still resolves to settings (the view coerces the tab itself).
    expect(surfaceForPath("/t/acme/settings/bogus")).toBe("settings");
  });

  it("keeps the existing sub-path precedent working (system/mcp → system)", () => {
    // system/mcp/acp have their OWN surfaces (exact match wins over the /system prefix).
    expect(surfaceForPath("/t/acme/system")).toBe("system");
    expect(surfaceForPath("/t/acme/system/mcp")).toBe("system-mcp");
  });

  it("round-trips the full shareable path for settings", () => {
    expect(fullPathForSurface("acme", "settings")).toBe("/t/acme/settings");
  });
});
