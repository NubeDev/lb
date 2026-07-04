import { describe, expect, it } from "vitest";

import { parseImportedTheme } from "./theme-import";

// A pasted tweakcn theme parses `:root{…}`/`.dark{…}` into base tokens; malformed input FAILS CLOSED
// (returns null so the caller keeps the current theme — no partial apply).

const TWEAKCN = `
:root {
  --background: hsl(0 0% 100%);
  --foreground: hsl(0 0% 0%);
  --card: hsl(0 0% 96%);
  --primary: hsl(217 91% 60%);
  --muted: hsl(220 9% 46%);
  --muted-foreground: hsl(218 11% 65%);
  --border: hsl(220 13% 91%);
}
.dark {
  --background: hsl(0 0% 4%);
  --foreground: hsl(0 0% 98%);
  --card: hsl(0 0% 8%);
  --primary: hsl(213 94% 68%);
  --muted: hsl(220 9% 60%);
  --muted-foreground: hsl(218 11% 70%);
  --border: hsl(215 28% 17%);
}
`;

describe("theme import parser", () => {
  it("parses a :root / .dark tweakcn block into base tokens", () => {
    const t = parseImportedTheme(TWEAKCN);
    expect(t).not.toBeNull();
    expect(t!.light.bg).toBe("0 0% 100%");
    expect(t!.light.accent).toBe("217 91% 60%");
    expect(t!.dark.bg).toBe("0 0% 4%");
    expect(t!.dark.accent).toBe("213 94% 68%");
  });

  it("reuses :root for dark when .dark is absent", () => {
    const t = parseImportedTheme(`:root { --background: #fff; --foreground: #000; --primary: #3b82f6; }`);
    expect(t).not.toBeNull();
    expect(t!.dark.bg).toBe(t!.light.bg);
    expect(t!.dark.accent).toBe(t!.light.accent);
  });

  it("accepts hex and oklch values in the pasted block", () => {
    const t = parseImportedTheme(`
      :root { --background: #ffffff; --foreground: oklch(0.145 0 0); --primary: #3b82f6; }
      .dark { --background: #000; --foreground: #fff; --primary: #60a5fa; }
    `);
    expect(t).not.toBeNull();
    expect(t!.light.accent).toBe("217 91% 60%");
  });

  it("fails closed on malformed / empty / non-theme input", () => {
    expect(parseImportedTheme("")).toBeNull();
    expect(parseImportedTheme("not css at all")).toBeNull();
    expect(parseImportedTheme(":root { color: red; }")).toBeNull(); // no usable identity tokens
    // @ts-expect-error — non-string guard
    expect(parseImportedTheme(null)).toBeNull();
  });
});
