import { describe, expect, it } from "vitest";

import { loadSession, saveSession } from "./session.storage";
import type { Session } from "./session.types";

class MemoryStorage {
  values = new Map<string, string>();

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }

  removeItem(key: string) {
    this.values.delete(key);
  }
}

const SAMPLE: Session = {
  token: "signed.bearer.token",
  principal: "user:alice",
  workspace: "acme",
  caps: ["chat:send"],
};

describe("session persistence", () => {
  it("round-trips a saved session (survives a refresh)", () => {
    const storage = new MemoryStorage();

    saveSession(SAMPLE, storage);

    expect(loadSession(storage)).toEqual(SAMPLE);
  });

  it("returns null when nothing is stored", () => {
    expect(loadSession(new MemoryStorage())).toBeNull();
  });

  it("clears the stored session on logout (save null)", () => {
    const storage = new MemoryStorage();
    saveSession(SAMPLE, storage);

    saveSession(null, storage);

    expect(loadSession(storage)).toBeNull();
  });

  it("ignores a malformed stored value instead of throwing", () => {
    const storage = new MemoryStorage();
    storage.setItem("lb.session", "{ not json");

    expect(loadSession(storage)).toBeNull();
  });

  it("rejects a stored value missing required fields", () => {
    const storage = new MemoryStorage();
    storage.setItem("lb.session", JSON.stringify({ token: "t" }));

    expect(loadSession(storage)).toBeNull();
  });
});
