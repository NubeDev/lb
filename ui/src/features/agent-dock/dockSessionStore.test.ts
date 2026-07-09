// Unit tests for the dock session-id localStorage helpers (agent-dock scope). The persisted id lets a
// page refresh / new tab reopen the conversation the user left off on, instead of minting a fresh
// empty session every mount. localStorage (cross-tab + cross-restart), NOT sessionStorage — a dock
// session is a per-USER conversation thread, safe and desirable to share. A fresh fake storage per
// test proves no cross-test (and thus no cross-user) leakage — the workspace-isolation analog for
// this UI-only store (CLAUDE §7: workspace is the hard wall; the storage key mirrors it).

import { describe, expect, it } from "vitest";

import {
  readDockSession,
  writeDockSession,
  clearDockSession,
  sessionKey,
  type LocalStorage,
} from "./dockSessionStore";

/** A minimal in-memory LocalStorage fake — ONLY the three methods this module touches. This is NOT
 *  a fake backend (rule 9): the real backend here IS localStorage (a browser primitive with a single
 *  one-arg surface); we cannot run a real browser session per unit test, so we drive the real code
 *  against an in-memory shape that mirrors `Storage` exactly. Mirrors `personaPin.test.ts`'s fake. */
function fakeStorage(): LocalStorage {
  const map = new Map<string, string>();
  return {
    getItem: (k) => (map.has(k) ? map.get(k)! : null),
    setItem: (k, v) => void map.set(k, v),
    removeItem: (k) => void map.delete(k),
  };
}

describe("dockSessionStore (localStorage helpers)", () => {
  it("sessionKey is workspace- AND user-namespaced (lb.agent-dock.session.<ws>.<user-slug>)", () => {
    expect(sessionKey("acme", "user:ada")).toBe("lb.agent-dock.session.acme.user-ada");
    // Different workspace → different key.
    expect(sessionKey("acme", "user:ada")).not.toBe(sessionKey("contoso", "user:ada"));
    // Different principal in the SAME workspace → different key (a shared browser keeps users distinct).
    expect(sessionKey("acme", "user:ada")).not.toBe(sessionKey("acme", "user:bea"));
  });

  it("read returns null when no session is set or storage is unavailable", () => {
    const s = fakeStorage();
    expect(readDockSession("ws", "user:ada", s)).toBeNull();
    expect(readDockSession("ws", "user:ada", undefined)).toBeNull();
  });

  it("write then read round-trips the id (per workspace + principal)", () => {
    const s = fakeStorage();
    writeDockSession("ws-a", "user:ada", "dock-user-ada-01HXXXXXXXX", s);
    writeDockSession("ws-a", "user:bea", "dock-user-bea-01HYYYYYYYY", s);
    writeDockSession("ws-b", "user:ada", "dock-user-ada-01HZZZZZZZZ", s);
    expect(readDockSession("ws-a", "user:ada", s)).toBe("dock-user-ada-01HXXXXXXXX");
    expect(readDockSession("ws-a", "user:bea", s)).toBe("dock-user-bea-01HYYYYYYYY");
    expect(readDockSession("ws-b", "user:ada", s)).toBe("dock-user-ada-01HZZZZZZZZ");
  });

  it("clear drops the session for that workspace + principal only", () => {
    const s = fakeStorage();
    writeDockSession("ws-a", "user:ada", "dock-user-ada-01HXXXXXXXX", s);
    writeDockSession("ws-a", "user:bea", "dock-user-bea-01HYYYYYYYY", s);
    writeDockSession("ws-b", "user:ada", "dock-user-ada-01HZZZZZZZZ", s);
    clearDockSession("ws-a", "user:ada", s);
    expect(readDockSession("ws-a", "user:ada", s)).toBeNull();
    expect(readDockSession("ws-a", "user:bea", s)).toBe("dock-user-bea-01HYYYYYYYY");
    expect(readDockSession("ws-b", "user:ada", s)).toBe("dock-user-ada-01HZZZZZZZZ");
  });

  it("write with an empty/whitespace id is treated as clear (defensive)", () => {
    const s = fakeStorage();
    writeDockSession("ws", "user:ada", "dock-user-ada-01HXXXXXXXX", s);
    writeDockSession("ws", "user:ada", "   ", s);
    expect(readDockSession("ws", "user:ada", s)).toBeNull();
  });

  it("write trims the id before storing (a trailing newline from a copy-paste never corrupts)", () => {
    const s = fakeStorage();
    writeDockSession("ws", "user:ada", "  dock-user-ada-01HXXXXXXXX  \n", s);
    expect(readDockSession("ws", "user:ada", s)).toBe("dock-user-ada-01HXXXXXXXX");
  });

  it("write and clear are no-ops (never throw) when storage is unavailable", () => {
    expect(() => writeDockSession("ws", "user:ada", "dock-user-ada-01HXXXXXXXX", undefined)).not.toThrow();
    expect(() => clearDockSession("ws", "user:ada", undefined)).not.toThrow();
    expect(readDockSession("ws", "user:ada", undefined)).toBeNull();
  });

  it("two separate storages model two browsers — a session in one never appears in the other", () => {
    const browserA = fakeStorage();
    const browserB = fakeStorage();
    writeDockSession("ws", "user:ada", "dock-user-ada-01HXXXXXXXX", browserA);
    expect(readDockSession("ws", "user:ada", browserA)).toBe("dock-user-ada-01HXXXXXXXX");
    expect(readDockSession("ws", "user:ada", browserB)).toBeNull();
  });
});
