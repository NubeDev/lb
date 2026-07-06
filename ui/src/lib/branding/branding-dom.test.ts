// Unit tests for `branding-dom.ts` — the ONE place the brand touches the document chrome
// (workspace-branding scope). Mirrors `theme-dom.test.ts`'s discipline: assert the title is set
// from the site name, the favicon link is created or updated from the data-URI, and a brand with
// no favicon data-URI leaves any existing link untouched.

import { afterEach, describe, expect, it } from "vitest";

import { applyBranding } from "./branding-dom";
import { DEFAULT_BRANDING, type Branding } from "./branding-options";

const fresh: Branding = { ...DEFAULT_BRANDING };

afterEach(() => {
  // Reset the document state between tests so a prior favicon link doesn't pollish the next case.
  document.title = "";
  document.querySelectorAll("link[rel='icon']").forEach((el) => el.remove());
});

describe("applyBranding", () => {
  it("sets document.title from the site name", () => {
    applyBranding(document, { ...fresh, siteName: "Acme" });
    expect(document.title).toBe("Acme");
  });

  it("falls back to the compiled default site name when none is set", () => {
    // normalizeBranding always fills siteName, but applyBranding is defensive: empty → default.
    applyBranding(document, { ...fresh, siteName: "" });
    expect(document.title).toBe(DEFAULT_BRANDING.siteName);
  });

  it("creates a <link rel='icon'> when none exists and a favicon data-URI is set", () => {
    const favicon = "data:image/x-icon;base64,AAABAAE=";
    applyBranding(document, { ...fresh, faviconDataUri: favicon });
    const link = document.querySelector<HTMLLinkElement>("link[rel='icon']");
    expect(link).not.toBeNull();
    expect(link?.href).toBe(favicon);
  });

  it("updates an existing <link rel='icon'> href in place", () => {
    const existing = document.createElement("link");
    existing.rel = "icon";
    existing.href = "data:image/png;base64,old=";
    document.head.appendChild(existing);

    const next = "data:image/x-icon;base64,new=";
    applyBranding(document, { ...fresh, faviconDataUri: next });

    const links = document.querySelectorAll("link[rel='icon']");
    expect(links).toHaveLength(1);
    expect(links[0].getAttribute("href")).toBe(next);
  });

  it("leaves an existing favicon link untouched when the brand has no favicon data-URI", () => {
    const existing = document.createElement("link");
    existing.rel = "icon";
    existing.href = "/favicon.ico";
    document.head.appendChild(existing);

    applyBranding(document, { ...fresh, faviconDataUri: undefined });
    const link = document.querySelector<HTMLLinkElement>("link[rel='icon']");
    expect(link?.getAttribute("href")).toBe("/favicon.ico");
  });
});
