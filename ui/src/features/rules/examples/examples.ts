// Ready-to-run example rules a newcomer clicks to learn (rules-editor-ux scope). One click loads the
// body into the editor buffer (the parent guards the dirty indicator). Where possible the bodies REUSE
// the ones proven green in the gateway tests (`rules_routes_test.rs` / `RulesView.gateway.test.tsx`) so
// they actually run — an example that lies is worse than none. This is a static catalog of examples
// (data, named by concept — not a `utils` dump).

/** One example rule — a title, a one-line teaching note, and the runnable body. */
export interface RuleExample {
  id: string;
  title: string;
  summary: string;
  body: string;
}

export const EXAMPLES: RuleExample[] = [
  {
    id: "scalar",
    title: "A scalar result",
    summary: "The simplest rule — return a single value. Runs with no data or caps.",
    body: "40 + 2",
  },
  {
    id: "threshold-alert",
    title: "Temperature threshold alert",
    summary: "Read a series' last 24h, keep the hot samples, and raise a critical alert if any.",
    body: [
      'let hot = history("series", "cooler.temp", "24h").filter("value > 5.0");',
      'if hot.size() > 0 {',
      '  alert(#{ level: "critical", series: "cooler.temp", msg: "cooler ran hot" });',
      '}',
    ].join("\n"),
  },
  {
    id: "rollup-aggregate",
    title: "Rollup + aggregate",
    summary: "Hourly-bucket a series and read the peak — the timeseries helpers in one line.",
    body: 'history("series", "cooler.temp", "24h").rollup("1h", "max")',
  },
  {
    id: "findings-emit",
    title: "Findings + log",
    summary: "Record a log line and emit a finding — see the FindingsList + LogPanel below.",
    body: 'log("checking"); emit(#{ level: "warning", msg: "needs review" });',
  },
  {
    id: "federated-query",
    title: "Federated query",
    summary: "Query a registered external datasource by name (needs a datasource registered first).",
    body: 'query("timescale", "SELECT point, value FROM readings ORDER BY ts DESC LIMIT 100")',
  },
  {
    id: "ai-over-query",
    title: "Ask AI about a query",
    summary: "Query an external datasource, then hand the rows to the workspace model to answer in words.",
    body: [
      'let sites = query("timescale", "SELECT * from site");',
      'ai.complete("how many sites are there", sites)',
    ].join("\n"),
  },
];
