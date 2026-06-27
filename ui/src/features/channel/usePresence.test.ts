// Presence rendering (collaboration scope, slice 3): the roster reducer is idempotent and
// order-independent — join/leave races must not corrupt who's online. The hook wires this pure
// reducer to the SSE presence feed; here we test the reducer directly (no gateway in tests).

import { describe, expect, it } from "vitest";

import { mergePresence } from "./usePresence";

describe("mergePresence", () => {
  it("adds a member that becomes present", () => {
    const online = mergePresence(new Set(), "user:bob", true);
    expect([...online]).toEqual(["user:bob"]);
  });

  it("removes a member that leaves", () => {
    const online = mergePresence(new Set(["user:bob"]), "user:bob", false);
    expect([...online]).toEqual([]);
  });

  it("is idempotent — applying the same change twice yields the same set", () => {
    const once = mergePresence(new Set(["user:ada"]), "user:bob", true);
    const twice = mergePresence(once, "user:bob", true);
    expect([...twice].sort()).toEqual(["user:ada", "user:bob"]);
  });

  it("does not mutate its input (order-independent updates)", () => {
    const before = new Set(["user:ada"]);
    mergePresence(before, "user:bob", true);
    expect([...before]).toEqual(["user:ada"]);
  });
});
