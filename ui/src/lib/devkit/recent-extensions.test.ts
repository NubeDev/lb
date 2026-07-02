import { beforeEach, describe, expect, it } from "vitest";

import {
  loadRecent,
  recordRecent,
  removeRecent,
} from "./recent-extensions";

const entry = (path: string, id = "acme.thing") =>
  ({ path, id, tier: "wasm" as const });

describe("recent-extensions store", () => {
  beforeEach(() => localStorage.clear());

  it("starts empty and returns [] on a corrupt store", () => {
    expect(loadRecent()).toEqual([]);
    localStorage.setItem("lb.studio.recent-extensions", "{not json");
    expect(loadRecent()).toEqual([]);
  });

  it("records an entry and reads it back", () => {
    recordRecent(entry("/ext/a"), 1000);
    const list = loadRecent();
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ path: "/ext/a", tier: "wasm", at: 1000 });
  });

  it("moves a re-recorded path to the front (MRU) without duplicating", () => {
    recordRecent(entry("/ext/a"), 1);
    recordRecent(entry("/ext/b"), 2);
    recordRecent(entry("/ext/a"), 3);
    const paths = loadRecent().map((r) => r.path);
    expect(paths).toEqual(["/ext/a", "/ext/b"]);
    expect(loadRecent()[0].at).toBe(3);
  });

  it("caps the list at 5, dropping the oldest", () => {
    for (let i = 0; i < 7; i++) recordRecent(entry(`/ext/${i}`), i);
    const paths = loadRecent().map((r) => r.path);
    expect(paths).toHaveLength(5);
    expect(paths).toEqual(["/ext/6", "/ext/5", "/ext/4", "/ext/3", "/ext/2"]);
  });

  it("forgets a single entry", () => {
    recordRecent(entry("/ext/a"), 1);
    recordRecent(entry("/ext/b"), 2);
    const after = removeRecent("/ext/a");
    expect(after.map((r) => r.path)).toEqual(["/ext/b"]);
    expect(loadRecent().map((r) => r.path)).toEqual(["/ext/b"]);
  });
});
