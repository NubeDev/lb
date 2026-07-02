// The host==client MF1 cross-check (i18n-catalogs scope: "run 2–3 of the authored plural/select
// messages through real intl-messageformat and confirm byte-identical output vs the Rust parser").
// These assertions are the SAME expected strings the Rust `catalog_test.rs` asserts — so the TS
// `intl-messageformat` render and the hand-written Rust parser agree on the pinned subset. If either
// side drifts, one of these two test suites fails.
//
// Typed placeholders ({ts,date}, {v,quantity}) are the host's job (client calls format.*), so they are
// out of this pure-render cross-check by design — the parity of THOSE is the Rust placeholder-parity
// test.

import { describe, expect, it } from "vitest";
import { renderMessage } from "./renderMessage";

describe("MF1 render parity with the Rust host parser", () => {
  it("en plural: one vs other (matches Rust plural_selects_one_vs_other_en)", () => {
    expect(renderMessage("alert.items_pending", { count: 1 }, "en")).toBe(
      "You have 1 pending item",
    );
    expect(renderMessage("alert.items_pending", { count: 5 }, "en")).toBe(
      "You have 5 pending items",
    );
  });

  it("en plural: exact =0 arm (matches Rust plural_exact_zero_arm_matches_before_category)", () => {
    expect(renderMessage("alert.items_pending", { count: 0 }, "en")).toBe(
      "You have no pending items",
    );
  });

  it("es plural: one vs other (matches Rust plural_selects_es_categories)", () => {
    expect(renderMessage("notify.new_messages", { name: "Ada", count: 1 }, "es")).toBe(
      "Ada te envió un mensaje",
    );
    expect(renderMessage("notify.new_messages", { name: "Ada", count: 3 }, "es")).toBe(
      "Ada te envió 3 mensajes",
    );
  });

  it("select: keyword + other fallback (matches Rust select_keyword_and_other_fallback)", () => {
    expect(renderMessage("alert.severity", { level: "critical", detail: "disk full" }, "en")).toBe(
      "Critical alert: disk full",
    );
    expect(renderMessage("alert.severity", { level: "bogus", detail: "x" }, "en")).toBe(
      "Notice: x",
    );
  });

  it("fallback chain: unknown key -> the key literal (never blank)", () => {
    expect(renderMessage("does.not.exist", {}, "es")).toBe("does.not.exist");
  });

  it("unknown locale falls back to the en builtin", () => {
    expect(renderMessage("notify.welcome", { name: "Zoe" }, "fr")).toBe("Welcome, Zoe!");
  });

  it("workspace override shadows the builtin", () => {
    const override = { "notify.welcome": "Hola de nuevo, {name} 👋" };
    expect(renderMessage("notify.welcome", { name: "Ada" }, "es", override)).toBe(
      "Hola de nuevo, Ada 👋",
    );
  });
});
