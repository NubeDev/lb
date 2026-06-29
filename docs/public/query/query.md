# Query (public)

TODO: filled when the saved-PRQL-query surface ships. Scope:
`../../scope/query/prql-query-scope.md`.

Summary of the ask (until shipped): a `query.*` MCP family over an editable `query:{ws}:{id}` record —
author once in **PRQL** (or `lang:"raw"` for dialect-native text), **save and re-edit**, and **run**
against the SurrealDB-native store (`store.query`) or a registered datasource (`federation.query`).
`query.run` composes the target's existing capability (no widening); a rule reuses a saved query via
`source("query:<name>")`. PRQL is the authoring layer only — no new engine, no second authority,
SurrealDB stays the one datastore.
