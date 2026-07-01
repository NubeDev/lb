// Unit test for the x-lb widget registry (channel rich responses scope). NO gateway — the registry is
// pure resolution over an OPEN vocabulary (UI built-ins ∪ extension-contributed widgets). Covers: every
// built-in widget resolves to its kind; an `ext:<id>/<widget>` string resolves to an `ext` entry (OPEN —
// not text, no crash); an unknown/absent widget falls back to `text` (never crashes); an `entity` hint
// resolves to the entity picker; the inline flag is carried as data.

import { describe, expect, it } from "vitest";

import type { XLbHint, WidgetKind } from "@/lib/channel/palette.types";
import { resolveWidget } from "./registry";

describe("the x-lb widget registry", () => {
  it("resolves each x-lb.widget to its widget kind", () => {
    const kinds: WidgetKind[] = ["sql", "runtime", "select", "cron", "boolean", "number", "date", "text"];
    for (const w of kinds) {
      expect(resolveWidget({ widget: w }).kind).toBe(w);
    }
  });

  it("falls back to text for an unknown widget (never crashes)", () => {
    // A hint the UI does not know (a newer author widget) degrades to the text input.
    expect(resolveWidget({ widget: "totally-new-widget" as unknown as WidgetKind }).kind).toBe("text");
    // An absent widget (a plain arg) → text too.
    expect(resolveWidget(undefined).kind).toBe("text");
    expect(resolveWidget({}).kind).toBe("text");
  });

  it("resolves an ext:<id>/<widget> string to an ext entry (OPEN vocabulary — not text, no crash)", () => {
    const entry = resolveWidget({ widget: "ext:acme/gizmo" });
    expect(entry.kind).toBe("ext");
    // The ext entry carries the viewKey so the arg renderer can mount the extension's widget.
    expect(entry.viewKey).toBe("ext:acme/gizmo");
    // It is NOT the text fallback — an ext widget is a first-class resolution, not an unknown.
    expect(entry.kind).not.toBe("text");
  });

  it("resolves an entity hint to the entity picker", () => {
    const hint: XLbHint = { entity: "datasource" };
    expect(resolveWidget(hint).kind).toBe("entity");
  });

  it("marks the inline widgets inline and the chip widgets not", () => {
    for (const w of ["sql", "runtime", "select", "cron", "boolean"] as WidgetKind[]) {
      expect(resolveWidget({ widget: w }).inline).toBe(true);
    }
    for (const w of ["number", "date", "text"] as WidgetKind[]) {
      expect(resolveWidget({ widget: w }).inline).toBe(false);
    }
    expect(resolveWidget({ entity: "channel" }).inline).toBe(false);
  });
});
