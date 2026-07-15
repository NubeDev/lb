// The theme seam: `--lbdg-*` set by a host on ANY ancestor must reach the grid's rules.
// That contract breaks the moment a rule declares or consumes a PUBLIC `--lbdg-*` name on
// `.lbdg-root`/`.lbdg-stack` — a specific declaration beats an inherited value, so the host's
// override would be silently ignored and every host whose theme uses different var names
// (shadcn's `--background`/`--card`, say) would render the dark fallback palette instead.
// This is what shipped in v0.1.0 and made ems's studio canvas a black slab on a light theme.
//
// Asserted STATICALLY over the stylesheet, deliberately: jsdom performs no `var()`
// substitution whatsoever (neither `getComputedStyle(el).getPropertyValue("--x")` nor a
// consuming property resolves), so a rendered assertion here would prove nothing.

import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const CSS = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "dashboard.css"), "utf8");

/** The public seam names a host may set on an ancestor. */
const PUBLIC = [
  "bg",
  "panel",
  "panel-2",
  "fg",
  "fg-ch",
  "muted",
  "border",
  "accent",
  "destructive",
];

describe("theme seam", () => {
  it("never DECLARES a public --lbdg-* name (it would shadow the host's override)", () => {
    const declared = CSS.split("\n").filter((l) =>
      PUBLIC.some((n) => new RegExp(`^\\s*--lbdg-${n}\\s*:`).test(l)),
    );
    expect(declared).toEqual([]);
  });

  it("resolves every public name into a private --lbdg-c-* var, ancestor value first", () => {
    for (const name of PUBLIC) {
      const decl = new RegExp(`--lbdg-c-${name}:\\s*var\\(--lbdg-${name},`);
      // `--lbdg-c-accent`/`-destructive` exist only on .lbdg-root; the rest on both blocks.
      expect(decl.test(CSS), `--lbdg-c-${name} must resolve var(--lbdg-${name}, …) first`).toBe(
        true,
      );
    }
  });

  it("rules consume ONLY the private names", () => {
    const leaks = CSS.split("\n").filter(
      (l) =>
        !/^\s*--lbdg-c-[a-z0-9-]+:/.test(l) &&
        PUBLIC.some((n) => new RegExp(`var\\(--lbdg-${n}[,)]`).test(l)),
    );
    expect(leaks).toEqual([]);
  });
});
