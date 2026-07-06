// Unit tests for the branding normalization layer (workspace-branding scope). The pure layer that
// turns an opaque prefs blob into a typed `Branding`, fail-closed per axis. Mirrors the discipline
// of `theme-options.test.ts`: prove that garbage never partially applies, the compiled default
// fills unset axes, and each image slot validates a real data-URI while rejecting look-alikes.

import { describe, expect, it } from "vitest";

import {
  DEFAULT_BRANDING,
  MAX_BRAND_IMAGE_BYTES,
  normalizeBranding,
} from "./branding-options";

describe("normalizeBranding", () => {
  it("returns the compiled Lazybones default for a non-object input", () => {
    expect(normalizeBranding(null)).toEqual(DEFAULT_BRANDING);
    expect(normalizeBranding(undefined)).toEqual(DEFAULT_BRANDING);
    expect(normalizeBranding("Acme")).toEqual(DEFAULT_BRANDING);
    expect(normalizeBranding(42)).toEqual(DEFAULT_BRANDING);
  });

  it("fills unset axes from the compiled default, keeps set ones", () => {
    const got = normalizeBranding({ siteName: "Acme" });
    expect(got.siteName).toBe("Acme");
    expect(got.siteAbbr).toBe(DEFAULT_BRANDING.siteAbbr);
    expect(got.tagline).toBe(DEFAULT_BRANDING.tagline);
  });

  it("clamps the abbreviation to 4 chars and the name to 80", () => {
    const got = normalizeBranding({ siteAbbr: "ACMEINC", siteName: "A".repeat(120) });
    expect(got.siteAbbr).toHaveLength(4);
    expect(got.siteName).toHaveLength(80);
  });

  it("accepts an empty tagline (the rail hides the line) but keeps it a string", () => {
    const got = normalizeBranding({ siteName: "Acme", tagline: "" });
    expect(got.tagline).toBe("");
  });

  it("keeps each image slot only when it is a valid image data-URI", () => {
    const png = "data:image/png;base64,iVBORw0KGgo=";
    const svg = "data:image/svg+xml;base64,PHN2Zy8+";
    const ico = "data:image/x-icon;base64,AAABAAE=";
    const got = normalizeBranding({
      logoDataUri: png,
      iconDataUri: svg,
      faviconDataUri: ico,
    });
    expect(got.logoDataUri).toBe(png);
    expect(got.iconDataUri).toBe(svg);
    expect(got.faviconDataUri).toBe(ico);
  });

  it("drops malformed image slots per-field, not whole-blob", () => {
    // Not a data-URI; not base64; not an image mime. Each is dropped, but the rest survives.
    const got = normalizeBranding({
      siteName: "Acme",
      logoDataUri: "https://example.com/logo.png", // not a data: URI
      iconDataUri: "data:image/png,", // no base64 payload
      faviconDataUri: "data:text/plain;base64,aGk=", // not an image mime
    });
    expect(got.siteName).toBe("Acme");
    expect(got.logoDataUri).toBeUndefined();
    expect(got.iconDataUri).toBeUndefined();
    expect(got.faviconDataUri).toBeUndefined();
  });

  it("exposes the v1 size ceiling as a stable constant", () => {
    // 256 KiB — the documented v1 cap. Branding images are small; a larger payload is rejected
    // upstream in `readBrandImage` before the data-URI lands in the blob.
    expect(MAX_BRAND_IMAGE_BYTES).toBe(256 * 1024);
  });

  it("keeps a set loginHeading (the deferred login-page field)", () => {
    const got = normalizeBranding({ loginHeading: "Sign in to Acme" });
    expect(got.loginHeading).toBe("Sign in to Acme");
  });

  it("drops a non-string loginHeading rather than partially applying", () => {
    const got = normalizeBranding({ loginHeading: 42 });
    expect(got.loginHeading).toBeUndefined();
  });
});
