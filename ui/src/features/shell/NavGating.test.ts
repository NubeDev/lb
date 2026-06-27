// Nav cap-gating for the data-console surfaces (data-console scope). The scope's mandatory UI deny:
// "a member without the cap never sees the Data nav entry" — the Data page relaxes the per-record
// membership gate (gate 3), so it is admin-only. This proves the VISIBILITY predicate App.tsx uses to
// decide which surfaces to show. It is convenience only (the gateway re-checks every verb server-side,
// proven in the Rust route test) — but a member should not even see a dead button.

import { describe, expect, it } from "vitest";

import { CAP, hasCap } from "@/lib/session";

// A member's caps: may explore series (Ingest), but NOT the admin DB-browser (Data).
const MEMBER = [
  "mcp:series.list:call",
  "mcp:series.read:call",
  "mcp:series.latest:call",
  "mcp:series.find:call",
  "mcp:ingest.write:call",
];
// An admin additionally holds the gate-3-relaxed store.* caps.
const ADMIN = [...MEMBER, CAP.storeTables, CAP.storeScan, CAP.storeGraph];

describe("data-console nav cap-gating", () => {
  it("shows the Ingest entry to a member (series.list)", () => {
    expect(hasCap(MEMBER, CAP.seriesList)).toBe(true);
  });

  it("HIDES the Data entry from a member (no store.scan — the gate-3 relaxation stays admin-only)", () => {
    expect(hasCap(MEMBER, CAP.storeScan)).toBe(false);
  });

  it("shows the Data entry to an admin (store.scan present)", () => {
    expect(hasCap(ADMIN, CAP.storeScan)).toBe(true);
  });
});
