import { describe, expect, it } from "vitest";

import type { Artifact } from "./devkit.api";
import {
  buildConfirmed,
  diffArtifacts,
  formatBytes,
  formatDelta,
} from "./artifact-diff";

const wasm = (size: number, mtime: string): Artifact => ({
  kind: "wasm",
  path: "/ext/target/wasm32-wasip2/release/thing_ext.wasm",
  size,
  mtime,
});
const remote = (size: number, mtime: string): Artifact => ({
  kind: "remote-entry",
  path: "/ext/ui/dist/remoteEntry.js",
  size,
  mtime,
});

describe("diffArtifacts", () => {
  it("marks a first-time artifact as new", () => {
    const [d] = diffArtifacts([], [wasm(2048, "2026-07-02T10:00:00Z")]);
    expect(d.change).toBe("new");
    expect(d.before).toBeNull();
    expect(d.after).toBe(2048);
    expect(d.bytesDelta).toBeNull();
  });

  it("marks rebuilt when mtime advances and reports the byte delta", () => {
    const before = [wasm(2048, "2026-07-02T10:00:00Z")];
    const after = [wasm(2100, "2026-07-02T10:05:00Z")];
    const [d] = diffArtifacts(before, after);
    expect(d.change).toBe("rebuilt");
    expect(d.bytesDelta).toBe(52);
  });

  it("marks unchanged when mtime did not advance (a no-op build)", () => {
    const same = wasm(2048, "2026-07-02T10:00:00Z");
    const [d] = diffArtifacts([same], [{ ...same }]);
    expect(d.change).toBe("unchanged");
  });

  it("marks an artifact that disappeared as missing", () => {
    const [d] = diffArtifacts([wasm(2048, "2026-07-02T10:00:00Z")], []);
    expect(d.change).toBe("missing");
    expect(d.after).toBeNull();
  });
});

describe("buildConfirmed", () => {
  it("is true only when every binary was freshly written", () => {
    const before = [wasm(2048, "2026-07-02T10:00:00Z")];
    const after = [
      wasm(2100, "2026-07-02T10:05:00Z"),
      remote(400, "2026-07-02T10:05:00Z"),
    ];
    expect(buildConfirmed(diffArtifacts(before, after))).toBe(true);
  });

  it("is false when the binary was unchanged even if the UI rebuilt", () => {
    const wasmSame = wasm(2048, "2026-07-02T10:00:00Z");
    const before = [wasmSame, remote(400, "2026-07-02T09:00:00Z")];
    const after = [{ ...wasmSame }, remote(420, "2026-07-02T10:05:00Z")];
    expect(buildConfirmed(diffArtifacts(before, after))).toBe(false);
  });

  it("is false when there is no binary at all", () => {
    const after = [remote(400, "2026-07-02T10:05:00Z")];
    expect(buildConfirmed(diffArtifacts([], after))).toBe(false);
  });
});

describe("formatting", () => {
  it("formats bytes at each scale", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(2 * 1024 * 1024)).toBe("2 MB");
  });

  it("formats a signed delta", () => {
    expect(formatDelta(6144)).toBe("+6 KB");
    expect(formatDelta(-128)).toBe("-128 B");
    expect(formatDelta(0)).toBe("0 B");
  });
});
