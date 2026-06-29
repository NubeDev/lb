// The Output verb family — `emit`/`alert`/`log`. Mirrors `rust/crates/rules/src/verbs/emit.rs` exactly
// (rules-editor-ux scope). `emit` records a finding; `alert` records one marked `alert:true` (the host
// routes those to the inbox + outbox after the run); `log` records a line. These are drained into the
// run result (the RunResult's FindingsList + LogPanel).

import type { CatalogGroup } from "./catalog.types";

export const OUTPUT_GROUP: CatalogGroup = {
  category: "output",
  label: "Output",
  blurb: "Record findings, alerts, and log lines from a run.",
  entries: [
    {
      name: "emit",
      signature: "emit(#{ … })",
      summary: "Record a finding (the whole map rides through as data).",
      snippet: 'emit(#{ level: "info", msg: "ok" })',
      category: "output",
    },
    {
      name: "alert",
      signature: "alert(#{ … })",
      summary: "Record a finding marked alert:true (routed to inbox + outbox).",
      snippet: 'alert(#{ level: "critical", msg: "hot" })',
      category: "output",
    },
    {
      name: "log",
      signature: "log(msg)",
      summary: "Record a log line for the run.",
      snippet: 'log("checking thresholds")',
      category: "output",
    },
  ],
};
