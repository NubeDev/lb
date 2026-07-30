# Store scope

TODO: Define the SurrealDB data model, namespaces, buckets, live queries, and change-feed requirements.


## Findings from other scopes

- **No explicit flush/durability point (workspace-provision scope, lb#121, 2026-07-30).**
  `lb_store` exposes atomic multi-row transactions (`write_tx`, `write_batch`) but no
  flush/fsync verb — durability is SurrealKV's implicit per-transaction commit, invisible to
  callers. `workspace.provision` wanted "do not return `Ok` until the writes are durable" and could
  not state a flush point; the observed `nube` orphan was a commit lost across an unclean restart.
  surrealdb 2.x exposes no inner kv handle (the same limitation that forced `compact` to work on the
  closed log out-of-band), so an explicit flush likely needs an upstream surrealdb affordance or a
  close/reopen-style operation. Worth deciding whether "durable before Ok" is a guarantee this
  platform wants to be able to make per-verb.
