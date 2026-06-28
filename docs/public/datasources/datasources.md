# Datasources (public)

Status: **TODO** — filled when the slice ships. Scope:
`../../scope/datasources/datasources-scope.md`.

A native (Tier-2) **`federation` extension** that embeds DataFusion + connectors to query external SQL
sources (MySQL, PostgreSQL/TimescaleDB, …) under `net:*` + a mediated secret, exposed as the read-first,
workspace-pinned `federation.query` MCP verb (plus `datasource.*` admin CRUD and a `federation.mirror`
`lb-jobs` batch). SurrealDB stays the authoritative store; external DBs are federated sources reached
through the gated extension, never a second authority.

(Fill this in on ship: the shipped verbs, supported source kinds, the `net:*` grants, federate-vs-mirror,
and the test counts.)
