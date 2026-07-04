// Extension discovery for the app nav: `ext.list` against the REAL gateway (a real seeded Install
// record), folded through `extNavEntries` — the cap gate the shell renders from. The mount contract
// is the NEXT slice; this slice only lists (app-shell scope §This session's goal #4).

import { describe, expect, it } from "vitest";

import { extNavEntries, type ExtRow } from "../src/index";
import { realClient, seedExtension } from "./harness";

describe("ext.list → cap-gated nav entries", () => {
  it("lists_installed_extensions_and_gates_nav_entries_on_caps", async () => {
    const app = realClient();
    const session = await app.login("ana", "app-ext-a");
    await seedExtension(app, {
      ext: "proof-panel",
      version: "0.1.0",
      enabled: true,
      ui: { entry: "index.js", label: "Proof Panel", scope: ["proof-panel.proof.derive"] },
    });

    const rows = await app.invoke<ExtRow[]>("ext_list");
    const row = rows.find((r) => r.ext === "proof-panel");
    expect(row).toBeDefined();
    expect(row?.ui?.label).toBe("Proof Panel");

    // The dev-login token carries the broad mcp wildcard set → the entry is visible.
    const visible = extNavEntries(rows, session.caps ?? []);
    expect(visible.map((e) => e.ext)).toContain("proof-panel");

    // A cap set WITHOUT the tool grant hides the entry (convenience gate; the host still
    // re-checks any call server-side).
    const hidden = extNavEntries(rows, ["bus:chan/*:sub"]);
    expect(hidden.map((e) => e.ext)).not.toContain("proof-panel");
  });

  it("extension_installs_are_workspace_scoped_through_the_app_seam", async () => {
    const a = realClient();
    const b = realClient();
    await a.login("ana", "app-ext-b");
    await b.login("bob", "app-ext-c");
    await seedExtension(a, { ext: "only-in-b-ws", version: "0.1.0", enabled: true });

    const inOther = await b.invoke<ExtRow[]>("ext_list");
    expect(inOther.map((r) => r.ext)).not.toContain("only-in-b-ws");
  });
});
