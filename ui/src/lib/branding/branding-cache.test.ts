// Unit tests for `branding-cache.ts` — the workspace-keyed localStorage boot cache that paints the
// brand before the first `prefs.resolve` round-trip (no flash). Mirrors `theme-storage`'s
// discipline: round-trip a brand, isolate by workspace, and degrade honestly when storage is
// unavailable / corrupt / over-quota.

import { afterEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_BRANDING, type Branding } from "./branding-options";
import { clearCachedBrand, loadCachedBrand, saveCachedBrand } from "./branding-cache";

const ACME: Branding = { siteName: "Acme", siteAbbr: "AC", tagline: "ops" };

afterEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

describe("loadCachedBrand", () => {
  it("returns null when no cache exists for the workspace (first-ever visit)", () => {
    expect(loadCachedBrand("acme")).toBeNull();
  });

  it("round-trips a saved brand and isolates per workspace", () => {
    saveCachedBrand("acme", ACME);
    saveCachedBrand("beta", { ...DEFAULT_BRANDING, siteName: "Beta" });

    expect(loadCachedBrand("acme")?.siteName).toBe("Acme");
    expect(loadCachedBrand("beta")?.siteName).toBe("Beta");
    // A different workspace that was never cached is still null.
    expect(loadCachedBrand("gamma")).toBeNull();
  });

  it("normalizes a corrupt/stale cache entry rather than partially applying", () => {
    localStorage.setItem("lb.brand.acme", "{not json");
    expect(loadCachedBrand("acme")).toBeNull();
    // A partial blob fills unset axes from the neutral default (fail-closed per axis).
    localStorage.setItem("lb.brand.acme", JSON.stringify({ siteName: "Acme", logoDataUri: "junk" }));
    const got = loadCachedBrand("acme");
    expect(got?.siteName).toBe("Acme");
    expect(got?.siteAbbr).toBe(DEFAULT_BRANDING.siteAbbr);
    expect(got?.logoDataUri).toBeUndefined();
  });

  it("degrades to null when localStorage is unavailable (private mode / locked webview)", () => {
    const throwing = {
      getItem: () => {
        throw new Error("unavailable");
      },
      setItem: () => {},
      removeItem: () => {},
    };
    expect(loadCachedBrand("acme", throwing)).toBeNull();
  });

  it("ignores an empty workspace id (defensive — the boot script may parse none)", () => {
    expect(loadCachedBrand("")).toBeNull();
    saveCachedBrand("", ACME); // no-op, must not throw
  });
});

describe("saveCachedBrand", () => {
  it("silently swallows a quota error (large image data-URIs) without throwing", () => {
    const throwing = {
      getItem: () => null,
      setItem: () => {
        throw new DOMException("quota exceeded", "QuotaExceededError");
      },
      removeItem: () => {},
    };
    expect(() => saveCachedBrand("acme", ACME, throwing)).not.toThrow();
  });
});

describe("clearCachedBrand", () => {
  it("drops the cached entry for one workspace without touching others", () => {
    saveCachedBrand("acme", ACME);
    saveCachedBrand("beta", { ...DEFAULT_BRANDING, siteName: "Beta" });
    clearCachedBrand("acme");
    expect(loadCachedBrand("acme")).toBeNull();
    expect(loadCachedBrand("beta")?.siteName).toBe("Beta");
  });
});
