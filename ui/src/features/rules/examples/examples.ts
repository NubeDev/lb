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

  // --- Inbox: raise / read / close attention items ------------------------------------------------
  // These need no seeded data — they write to (or read) your own workspace's inbox and work on a fresh
  // node. Each teaches ONE verb so you can see exactly what it does before combining them.
  {
    id: "inbox-record",
    title: "Inbox · raise an item",
    summary: "Raise one attention item on the `ops` channel. Re-running upserts (same id → no dupe).",
    body: 'inbox.record(#{ channel: "ops", id: "check-me", body: "please take a look" });',
  },
  {
    id: "inbox-list",
    title: "Inbox · read items",
    summary: "Read every attention item on a channel — an uncharged read. Run the raise example first.",
    body: 'inbox.list("ops")',
  },
  {
    id: "inbox-record-then-list",
    title: "Inbox · raise then read",
    summary: "Raise an item and read the channel back in one rule — see it land in the Result below.",
    body: [
      'inbox.record(#{ channel: "ops", id: "check-me", body: "please take a look" });',
      'inbox.list("ops")',
    ].join("\n"),
  },
  {
    id: "inbox-resolve",
    title: "Inbox · close an item",
    summary:
      'Resolve an item by id with a verdict — "approved", "rejected", or "deferred" (idempotent, last wins).',
    body: 'inbox.resolve("check-me", "approved");',
  },

  // --- Outbox: stage a must-deliver effect --------------------------------------------------------
  {
    id: "outbox-enqueue",
    title: "Outbox · stage an effect",
    summary: "Stage one must-deliver effect (e.g. a page). The relay drains it — a rule only stages.",
    body: [
      'outbox.enqueue(#{ id: "page-1", target: "notify", action: "page",',
      '                  payload: #{ level: "info", msg: "hello from a rule" } });',
    ].join("\n"),
  },
  {
    id: "outbox-status",
    title: "Outbox · check the queue",
    summary: "Read the workspace's outbox status (pending / failed) — an uncharged read.",
    body: "outbox.status()",
  },

  // --- Channels: post / read the live bus ---------------------------------------------------------
  {
    id: "channel-post",
    title: "Channel · post a message",
    summary: "Post a chat message to the `ops` channel with your own authority (needs bus:chan/ops:Pub).",
    body: 'channel.post("ops", #{ body: "posted from a rule" });',
  },
  {
    id: "channel-read",
    title: "Channel · read history",
    summary: "Read the last 5 messages on `ops` — a bounded snapshot (an uncharged read).",
    body: 'channel.history("ops", 5)',
  },
  {
    id: "channel-list",
    title: "Channel · list channels",
    summary: "List the channels in your workspace — the switcher's read. Works on a fresh workspace.",
    body: "channel.list()",
  },

  // --- The full messaging surface in one rule -----------------------------------------------------
  {
    id: "escalate-and-notify",
    title: "Escalate: inbox + outbox + channel",
    summary:
      "The full messaging surface in one rule — raise an attention item, stage a must-deliver page, and post to the live channel.",
    body: [
      "// Raise an item, stage a page, and post to the channel — the whole toolkit together.",
      'inbox.record(#{ channel: "ops", id: "breach", body: "cooler ran hot" });',
      'outbox.enqueue(#{ id: "page-breach", target: "notify", action: "page",',
      '                  payload: #{ level: "critical", msg: "cooler breach" } });',
      'channel.post("ops", #{ body: "⚠ cooler breach — paging on-call" });',
    ].join("\n"),
  },

  // --- Propose a change, gate an effect on a human approval ---------------------------------------
  {
    id: "propose-and-approve",
    title: "Propose & approve: gate an effect on sign-off",
    summary:
      "Raise a needs:approval item AND stage the effect it will fire IF approved — the email is HELD until a reviewer approves the item (rules-approvals). A rule proposes; a human disposes.",
    body: [
      "// Raise an approval item AND stage the email it sends only IF approved (staged HELD).",
      "inbox.request_approval(#{",
      '  id: "refund-proposed",',
      '  channel: "ops",',
      '  body: "Refund proposed — cooler breached",',
      '  route: "team:managers",              // who should sign off (advisory)',
      '  on_approve: #{ target: "notify", action: "page",   // the HELD effect',
      '                 payload: #{ level: "info", msg: "refund approved" } },',
      "});",
      "// A manager then approves via the Inbox (inbox.resolve), and the reactor releases the effect.",
    ].join("\n"),
  },
];
