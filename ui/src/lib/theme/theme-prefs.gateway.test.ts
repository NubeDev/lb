// Real-gateway tests for theme persistence over the `prefs` verbs + the `ui_theme` axis (theme-
// customizer scope). No mocks, no fake backend (rule 9): every read/write hits a spawned `node` gateway
// over a real signed session, seeded via the real write path. Covers the mandatory categories:
//   - ROUND-TRIP: a member persists a ThemePreference (`prefs.set` ui_theme) and reads it back
//     (`prefs.resolve` / `prefs.get`) — the real record, not local state; a second read = a fresh boot.
//   - WORKSPACE-DEFAULT: an admin sets the workspace-default theme (`prefs.set_default`); a member with
//     no personal theme resolves to it, and a member WITH one resolves to their own (member wins whole).
//   - CAPABILITY DENY: a member without `mcp:prefs.set:call` is denied on persist; a non-admin without
//     `mcp:prefs.set_default:call` is denied on the workspace default — both opaque server refusals.
//   - WORKSPACE ISOLATION: member A's theme in ws-A is never resolved/read in ws-B.

import { describe, expect, it, beforeAll } from "vitest";

import {
  persistTheme,
  persistWorkspaceDefaultTheme,
  readOwnTheme,
  readResolvedTheme,
} from "./theme-prefs";
import { DEFAULT_THEME, type ThemePreference } from "./theme-options";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `theme-${n++}`;

const TEAL: ThemePreference = {
  mode: "dark",
  preset: "teal",
  radius: "0.75rem",
  look: "default",
  layout: { variant: "floating", collapsible: "offcanvas", side: "right" },
};
const AMBER_LIGHT: ThemePreference = {
  mode: "light",
  preset: "amber",
  radius: "0.5rem",
  look: "default",
  layout: { variant: "sidebar", collapsible: "icon", side: "left" },
};

beforeAll(() => useRealGateway());

describe("theme persistence over prefs (real gateway)", () => {
  it("persists and reads back a member's theme (round-trip + fresh boot)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    await persistTheme(TEAL);

    // Read via the folded chain (resolve) AND the own-record read (get) — both the real record.
    const resolved = await readResolvedTheme();
    expect(resolved).toEqual(TEAL);
    const own = await readOwnTheme();
    expect(own).toEqual(TEAL);

    // A "fresh boot": re-sign-in the same identity/workspace and read again — the theme roamed.
    await signInReal("user:ada", ws);
    expect(await readResolvedTheme()).toEqual(TEAL);
  });

  it("folds member over workspace-default; a member with none inherits the default", async () => {
    const ws = nextWs();
    // An admin holding the admin-gated set_default cap writes the workspace default. (dev-login is a
    // member set that deliberately omits `prefs.set_default`, so we grant it explicitly here — the
    // honest "admin" principal — rather than assume a broader dev grant.)
    await signInWithCaps("user:admin", ws, [
      "mcp:prefs.set_default:call",
      "mcp:prefs.resolve:call",
      "mcp:prefs.set:call",
      "mcp:prefs.get:call",
    ]);
    await persistWorkspaceDefaultTheme(AMBER_LIGHT);

    // A member who set no personal theme resolves to the workspace default.
    await signInReal("user:bob", ws);
    expect(await readResolvedTheme()).toEqual(AMBER_LIGHT);
    expect(await readOwnTheme()).toBeNull(); // no personal record

    // A member WITH a personal theme resolves to their own (member wins whole).
    await persistTheme(TEAL);
    expect(await readResolvedTheme()).toEqual(TEAL);
  });

  it("denies persist for a member without prefs.set, and workspace-default for a non-admin", async () => {
    const ws = nextWs();

    // A member WITHOUT `mcp:prefs.set:call` cannot persist a personal theme.
    await signInWithCaps("user:eve", ws, ["mcp:prefs.resolve:call"]);
    await expect(persistTheme(TEAL)).rejects.toThrow();

    // A non-admin WITHOUT `mcp:prefs.set_default:call` cannot set the workspace default.
    await signInWithCaps("user:eve", ws, ["mcp:prefs.set:call", "mcp:prefs.resolve:call"]);
    await expect(persistWorkspaceDefaultTheme(TEAL)).rejects.toThrow();
  });

  it("isolates a member's theme by workspace (ws-A theme never resolves in ws-B)", async () => {
    const wsA = nextWs();
    const wsB = nextWs();

    await signInReal("user:ada", wsA);
    await persistTheme(TEAL);

    // The SAME identity in ws-B has no theme → resolves to none (falls back to the shell default),
    // never ws-A's teal.
    await signInReal("user:ada", wsB);
    const inB = await readResolvedTheme();
    expect(inB).not.toEqual(TEAL);
    expect(inB).toBeNull();

    // ws-A still has it.
    await signInReal("user:ada", wsA);
    expect(await readResolvedTheme()).toEqual(TEAL);
  });

  it("reset writes the built-in default as the member's explicit theme", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await persistTheme(TEAL);
    await persistTheme(DEFAULT_THEME); // reset = explicit default
    expect(await readResolvedTheme()).toEqual(DEFAULT_THEME);
  });
});
