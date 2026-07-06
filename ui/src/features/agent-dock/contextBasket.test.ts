// Pure unit tests for the context-basket ops (agent-context-basket scope).

import { describe, expect, it } from "vitest";

import type { Item } from "@/lib/channel/channel.types";
import { MAX_CONTEXT_ITEMS, refLabel, toggleRef } from "./contextBasket";

function item(id: string, body: string): Item {
  return { id, channel: "dock", author: "user:ada", body, ts: 1 };
}

describe("toggleRef", () => {
  it("adds an absent id and removes a present one, preserving order", () => {
    let ids = toggleRef([], "a");
    ids = toggleRef(ids, "b");
    expect(ids).toEqual(["a", "b"]);
    expect(toggleRef(ids, "a")).toEqual(["b"]);
  });

  it("refuses to add past the host cap (the request would be rejected server-side)", () => {
    const full = Array.from({ length: MAX_CONTEXT_ITEMS }, (_, i) => `i${i}`);
    expect(toggleRef(full, "extra")).toEqual(full);
    // Removal still works at the cap.
    expect(toggleRef(full, "i0")).toHaveLength(MAX_CONTEXT_ITEMS - 1);
  });
});

describe("refLabel", () => {
  it("labels kind-tagged payloads by kind", () => {
    const items = [
      item("q1", `{"kind":"query_result","source":"s","sql":"x","columns":[],"rows":[]}`),
      item("r1", `{"kind":"rich_result","v":2,"view":"table"}`),
    ];
    expect(refLabel(items, "q1")).toBe("query result");
    expect(refLabel(items, "r1")).toBe("response");
  });

  it("labels chat with a snippet and unknown refs with the id", () => {
    const items = [item("c1", "a note about the sales dip in the northern region")];
    expect(refLabel(items, "c1")).toBe("a note about the sales d…");
    expect(refLabel(items, "ghost")).toBe("ghost");
  });
});
