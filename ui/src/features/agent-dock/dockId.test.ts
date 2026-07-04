// Unit tests for the `dock.` id grammar (agent-dock scope) — mint / slug / filter. Pure, no gateway.

import { describe, expect, it } from "vitest";

import {
  DOCK_PREFIX,
  dockPrefixFor,
  isDockChannel,
  isOwnDockChannel,
  mintDockId,
  mintUlid,
  userSlug,
} from "./dockId";

describe("userSlug", () => {
  it("lowercases and collapses non-alphanumerics to single dashes", () => {
    expect(userSlug("user:Ada@Acme.io")).toBe("user-ada-acme-io");
  });
  it("trims leading/trailing separators and falls back to anon", () => {
    expect(userSlug("!!!")).toBe("anon");
    expect(userSlug("::ada::")).toBe("ada");
  });
});

describe("mintUlid", () => {
  it("is lexicographically time-ordered (later time sorts after)", () => {
    const early = mintUlid(() => 1000, () => 0.1);
    const late = mintUlid(() => 2000, () => 0.1);
    expect(late > early).toBe(true);
  });
  it("varies by randomness within the same millisecond", () => {
    const a = mintUlid(() => 1000, () => 0.1);
    const b = mintUlid(() => 1000, () => 0.9);
    expect(a).not.toBe(b);
  });
});

describe("mintDockId", () => {
  it("mints `dock-{user-slug}-{ulid}` as ONE cap segment (no `.`/`/`)", () => {
    const id = mintDockId("user:ada", () => 1000, () => 0.5);
    expect(id.startsWith(`${DOCK_PREFIX}user-ada-`)).toBe(true);
    // Critical: no `.` or `/` — else `bus:chan/*:pub`'s single `*` can't match the resource segment.
    expect(id).not.toMatch(/[./]/);
  });
});

describe("filters", () => {
  const ada = mintDockId("user:ada", () => 1000, () => 0.5);
  const bob = mintDockId("user:bob", () => 1000, () => 0.5);

  it("isDockChannel matches ANY dock channel, not ordinary channels", () => {
    expect(isDockChannel(ada)).toBe(true);
    expect(isDockChannel(bob)).toBe(true);
    expect(isDockChannel("general")).toBe(false);
    expect(isDockChannel("dockyard")).toBe(false); // must be the `dock-` prefix, not `dock`
  });

  it("isOwnDockChannel matches only the user's own prefix", () => {
    expect(isOwnDockChannel(ada, "user:ada")).toBe(true);
    expect(isOwnDockChannel(bob, "user:ada")).toBe(false);
  });

  it("dockPrefixFor is `dock-{slug}-`", () => {
    expect(dockPrefixFor("user:ada")).toBe("dock-user-ada-");
  });
});
