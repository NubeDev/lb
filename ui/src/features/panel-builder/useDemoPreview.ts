// The demo-data preview hook (data-studio-10x scope, phase 3 "Demo data, honestly seeded") — when the
// user's query returns ZERO rows, the empty preview offers "Preview with demo data": the draft's data
// source is swapped (display-only) to the SHIPPED SQLite demo datasource (`demo-buildings`, registered
// by `make seed-demo-sqlite` — sqlite-datasource-demo-scope). REAL records through the REAL engine
// (`federation.query`), same render path — never a client-fabricated frame (rule 9). The offer shows
// only when the demo datasource actually exists in the caller's workspace (an honest roster read, the
// shared `datasource.list` cache), and demo mode AUTO-YIELDS the moment the user's own query has rows.
// One responsibility: the demo-preview state machine.

import { useEffect, useState } from "react";

import type { Cell } from "@/lib/dashboard";

import { useDatasourceList } from "./tabs/useDatasourceList";

/** The canonical demo datasource `make seed-demo-sqlite` registers (kind `sqlite`). */
export const DEMO_DATASOURCE = "demo-buildings";

/** A real query over the demo building dataset (`point_reading` — one month of 15-min meter data). */
const DEMO_SQL =
  "SELECT time, value FROM point_reading ORDER BY time DESC LIMIT 200";

/** Swap `draft`'s data binding to the demo source (display-only — the saved cell is untouched; the
 *  caller renders THIS cell while demo mode is on). View/options/fieldConfig stay the user's own. */
export function demoSwappedCell(draft: Cell): Cell {
  const tool = "federation.query";
  const args = { source: DEMO_DATASOURCE, sql: DEMO_SQL };
  return {
    ...draft,
    sources: [{ refId: "A", tool, args, datasource: { type: "federation" } }],
    source: { tool, args },
  };
}

export interface DemoPreview {
  /** True when the demo datasource exists in this workspace AND the user's query came back empty —
   *  the empty preview may offer the toggle. */
  available: boolean;
  /** Demo mode is on: render the demo-swapped cell, badged `demo`. */
  active: boolean;
  enable: () => void;
  disable: () => void;
}

export function useDemoPreview(
  ws: string,
  state: { hasTarget: boolean; loading: boolean; rowCount: number },
): DemoPreview {
  const [active, setActive] = useState(false);
  const { options } = useDatasourceList(ws);
  const exists = options.some((o) => o.type === "federation" && o.name === DEMO_DATASOURCE);

  // Auto-yield (a correctness requirement, not polish): the moment the user's OWN query has rows,
  // demo mode turns off — an unbadged demo frame in a control surface would be a lie.
  const { loading, rowCount } = state;
  useEffect(() => {
    if (active && !loading && rowCount > 0) setActive(false);
  }, [active, loading, rowCount]);

  return {
    available: exists && state.hasTarget && !state.loading && state.rowCount === 0,
    active,
    enable: () => setActive(true),
    disable: () => setActive(false),
  };
}
