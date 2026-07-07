// Unit tests for the persona-pin sessionStorage helpers (persona-session #5). The pin is the sticky
// per-tab override of the page-context match — per-tab is the WHOLE point, so this MUST be
// sessionStorage, not localStorage. A fresh fake storage per test proves no cross-test (and thus no
// cross-tab) leakage — the rule the gateway test exercises end to end over two real clients.

import { describe, expect, it } from "vitest";

import {
  readPersonaPin,
  writePersonaPin,
  clearPersonaPin,
  pinKey,
  type SessionStorage,
} from "./personaPin";

/** A minimal in-memory SessionStorage fake — ONLY the three methods this module touches. This is NOT
 *  a fake backend (rule 9): the real backend here IS sessionStorage (a browser primitive with a single
 *  one-arg surface); we cannot run a real browser session per unit test, so we drive the real code
 *  against an in-memory shape that mirrors `Storage` exactly. */
function fakeStorage(): SessionStorage {
  const map = new Map<string, string>();
  return {
    getItem: (k) => (map.has(k) ? map.get(k)! : null),
    setItem: (k, v) => void map.set(k, v),
    removeItem: (k) => void map.delete(k),
  };
}

describe("personaPin (sessionStorage helpers)", () => {
  it("pinKey is workspace-namespaced (lb.agent-dock.persona-pin.<ws>)", () => {
    expect(pinKey("acme")).toBe("lb.agent-dock.persona-pin.acme");
    expect(pinKey("acme")).not.toBe(pinKey("contoso"));
  });

  it("read returns null when no pin is set or storage is unavailable", () => {
    const s = fakeStorage();
    expect(readPersonaPin("ws", s)).toBeNull();
    expect(readPersonaPin("ws", undefined)).toBeNull();
  });

  it("write then read round-trips the id (per workspace)", () => {
    const s = fakeStorage();
    writePersonaPin("ws-a", "builtin.flow-author", s);
    writePersonaPin("ws-b", "builtin.data-analyst", s);
    expect(readPersonaPin("ws-a", s)).toBe("builtin.flow-author");
    expect(readPersonaPin("ws-b", s)).toBe("builtin.data-analyst");
  });

  it("clear drops the pin for that workspace only", () => {
    const s = fakeStorage();
    writePersonaPin("ws-a", "builtin.flow-author", s);
    writePersonaPin("ws-b", "builtin.data-analyst", s);
    clearPersonaPin("ws-a", s);
    expect(readPersonaPin("ws-a", s)).toBeNull();
    expect(readPersonaPin("ws-b", s)).toBe("builtin.data-analyst");
  });

  it("write with an empty/whitespace id is treated as clear (defensive)", () => {
    const s = fakeStorage();
    writePersonaPin("ws", "builtin.flow-author", s);
    writePersonaPin("ws", "   ", s);
    expect(readPersonaPin("ws", s)).toBeNull();
  });

  it("write and clear are no-ops (never throw) when storage is unavailable", () => {
    expect(() => writePersonaPin("ws", "builtin.flow-author", undefined)).not.toThrow();
    expect(() => clearPersonaPin("ws", undefined)).not.toThrow();
    expect(readPersonaPin("ws", undefined)).toBeNull();
  });

  it("two separate storages model two tabs — a pin in one never appears in the other", () => {
    const tabA = fakeStorage();
    const tabB = fakeStorage();
    writePersonaPin("ws", "builtin.flow-author", tabA);
    expect(readPersonaPin("ws", tabA)).toBe("builtin.flow-author");
    expect(readPersonaPin("ws", tabB)).toBeNull();
    // Pinning something different in tab B leaves tab A's pin untouched.
    writePersonaPin("ws", "builtin.data-analyst", tabB);
    expect(readPersonaPin("ws", tabA)).toBe("builtin.flow-author");
    expect(readPersonaPin("ws", tabB)).toBe("builtin.data-analyst");
  });
});
