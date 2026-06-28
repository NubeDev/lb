# Rules (public)

Status: **TODO** — filled when the slice ships. Scope:
`../../scope/rules/rules-engine-scope.md` (the `lb-rules` engine) and
`../../scope/rules/rule-chains-scope.md` (the DAG over `lb-jobs`).

A workspace-authored, sandboxed **rules/processing engine** (`lb-rules`) — an embedded `rhai` cage + a
lazy `Grid` + a verb library, ported from `rubix-cube` onto the lazybones chokepoints (data via
`data.query`/`series.*`/`federation.query`, `ai.*` via the AI-gateway, `emit`/`alert` via inbox/outbox)
— plus **rule chains**, a DAG that runs each step as an `lb-jobs` job (cron via the S6 reactor, event
via `bus.watch`). Exposed as `rules.*` / `chains.*` MCP verbs.

(Fill this in on ship: the shipped verbs, the verb library, the chain triggers, and the test counts —
mirror the structure of `../host-tools/host-tools.md` / `../agent-run/agent-run.md`.)
