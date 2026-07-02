import { bench, describe } from "vitest";
import { cronNextRuns } from "./CronPanel";
import { fmtDateTime } from "../lib/format";

// Data-prep paths for the declarative widgets. These run on a data tick (a 3s
// service poll, a cron edit, a render) — not every frame — but model the cost of
// a large alarm list / history page so a regression (e.g. losing the dedupe Map,
// or an O(n^2) sort) shows up here.

function makeAlarms(n: number) {
  const sev = [0, 1, 2];
  const arr: { name: string; severity: number; source: string; message: string; acknowledged: boolean; raisedAt: number }[] = [];
  for (let i = 0; i < n; i++) {
    arr.push({
      name: `alm-${i % 800}`, // ~800 unique → exercises the dedupe
      severity: sev[i % 3],
      source: `src-${i % 50}`,
      message: `condition ${i} tripped threshold`,
      acknowledged: i % 2 === 0,
      raisedAt: 1765000000 + i,
    });
  }
  return arr;
}

const ALARMS_JSON = JSON.stringify(makeAlarms(2000));

function parseDedupe(raw: string) {
  const a = JSON.parse(raw) as ReturnType<typeof makeAlarms>;
  const byName = new Map<string, (typeof a)[number]>();
  for (const x of a) {
    const p = byName.get(x.name);
    if (!p || (x.raisedAt ?? 0) >= (p.raisedAt ?? 0)) byName.set(x.name, x);
  }
  return [...byName.values()];
}

function filterSort(rows: ReturnType<typeof makeAlarms>, q: string, sev: number) {
  let r = rows;
  if (q) r = r.filter((x) => `${x.source} ${x.message}`.toLowerCase().includes(q));
  r = r.filter((x) => x.severity === sev);
  return [...r].sort((a, b) => (b.raisedAt - a.raisedAt));
}

describe("alarm console per-poll data prep (2000 records → ~800 unique)", () => {
  bench("parse JSON + dedupe by name", () => {
    parseDedupe(ALARMS_JSON);
  });
  const rows = parseDedupe(ALARMS_JSON);
  bench("filter + sort (DataTable view)", () => {
    filterSort(rows, "condition", 2);
  });
});

describe("cron next-run computation", () => {
  bench("next 5 (daily 02:00 — sparse)", () => {
    cronNextRuns("0 2 * * *", 1765000000000, 5);
  });
  bench("next 5 (every 15m — dense)", () => {
    cronNextRuns("*/15 * * * *", 1765000000000, 5);
  });
});

describe("datetime formatting", () => {
  bench("fmtDateTime x1000", () => {
    for (let i = 0; i < 1000; i++) fmtDateTime(1765000000 + i * 60, "datetime");
  });
});
