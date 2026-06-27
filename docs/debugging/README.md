# Debugging — working history

The project's debugging memory: every issue and how it became working, so nothing is
debugged twice. **Append-only and symptom-led.**

- How this works and the entry template: `../scope/debugging/debugging-scope.md`.
- One file per issue, named by the symptom: `<area>/<symptom-slug>.md`.
- Add a row below when you open an entry; update its status when it closes.

## History (newest first)

| Date | Area | Symptom | Status | Entry |
|---|---|---|---|---|
| 2026-06-27 | store | SurrealKV "Invalid revision N for type Value" on the SECOND ingest drain (persistent store only; `mem://` is fine) — durable on-disk ingest unreliable past one write/ws | documented (engine-level; demo runs on `mem://`) | [store/surrealkv-invalid-revision-on-drain-reread.md](store/surrealkv-invalid-revision-on-drain-reread.md) |
| 2026-06-27 | build | Rust build fails `linker 'cc' not found` — no C compiler on the box and no root to apt-install one (`ring` also needs to compile C) | resolved | [build/no-c-compiler-linker-cc-not-found.md](build/no-c-compiler-linker-cc-not-found.md) |
| 2026-06-27 | extensions | the host-mediated bridge (`/mcp/call`) can't dispatch a host-native `series.*` verb — a federated page's reads `NotFound`/403 | resolved | [extensions/bridge-cannot-dispatch-host-native-series.md](extensions/bridge-cannot-dispatch-host-native-series.md) |
| 2026-06-27 | extensions | `series.find` can't discover a series seeded with `labels` — the ingest write path never tags it | documented | [extensions/series-find-needs-tag-edges-not-labels.md](extensions/series-find-needs-tag-edges-not-labels.md) |
| 2026-06-27 | build | the whole desktop hard-freezes (forced reboot) during Rust builds; no OOM log (suspected swap death-spiral from parallel `rust-lld`) | mitigated | [build/host-freezes-during-rust-build.md](build/host-freezes-during-rust-build.md) |
| 2026-06-27 | extensions | a federated extension page won't load in the Vite **dev server** (`make dev`/`make ui`) — `getUrl(...).then is not a function`; the federation host runtime only exists in a production build | worked-around (`make ui-preview`) | [extensions/federated-remote-fails-in-dev-server.md](extensions/federated-remote-fails-in-dev-server.md) |
| 2026-06-27 | tags | `DEFINE TABLE … AS SELECT … GROUP` defines but never populates on SurrealKV (tag_counts empty) → per-query | resolved | [tags/materialized-view-does-not-populate.md](tags/materialized-view-does-not-populate.md) |
| 2026-06-27 | tags | HNSW `<\|K\|>` knn returns nothing; the two-arg `<\|K,EF\|>` form is required | resolved | [tags/hnsw-knn-needs-ef-arg.md](tags/hnsw-knn-needs-ef-arg.md) |
| 2026-06-27 | tags | `type::thing("series:node.cpu_temp")` mis-parses a dotted entity id (tag add fails) | resolved | [tags/dotted-entity-id-needs-two-arg.md](tags/dotted-entity-id-needs-two-arg.md) |
| 2026-06-27 | tags | a `tagged` edge silently drops fields literally named `key`/`value` | resolved | [tags/relation-drops-key-value-fields.md](tags/relation-drops-key-value-fields.md) |
| 2026-06-27 | ingest | `DELETE … ORDER BY … LIMIT 1` unsupported (drop-oldest eviction) | resolved | [ingest/delete-order-by-limit-unsupported.md](ingest/delete-order-by-limit-unsupported.md) |
| 2026-06-27 | ingest | `series.read(seq <= u64::MAX)` returns nothing (huge int coerces to float) | resolved | [ingest/u64-max-bound-coerces-to-float.md](ingest/u64-max-bound-coerces-to-float.md) |
| 2026-06-27 | store | workspace fails to build: modules referenced but never declared (`mod …` missing) | resolved | [store/half-wired-modules-block-workspace-build.md](store/half-wired-modules-block-workspace-build.md) |
| 2026-06-26 | extensions | a ws-B caller can run an extension only ws-A installed (the loaded instance is node-global; the wall is caps + store, not the instance) | resolved | [extensions/loaded-extension-instance-is-node-global.md](extensions/loaded-extension-instance-is-node-global.md) |
| 2026-06-26 | agent | the agent is `Denied` reading a substrate doc the caller owns (derived-sub vs the S4 membership gate) | resolved | [agent/agent-reads-doc-it-doesnt-own-is-denied.md](agent/agent-reads-doc-it-doesnt-own-is-denied.md) |
| 2026-06-26 | store | `DEFINE BUCKET` fails to parse on the embedded `kv-mem` store (assets stored as records instead) | resolved | [store/define-bucket-unavailable-in-kv-mem-build.md](store/define-bucket-unavailable-in-kv-mem-build.md) |
| 2026-06-26 | bus | `cargo test --workspace` OOM-killed (137) once tests boot 2 nodes each | resolved | [bus/cargo-test-workspace-ooms-with-many-peers.md](bus/cargo-test-workspace-ooms-with-many-peers.md) |
| 2026-06-26 | bus | a live subscriber receives a message published by a *different* test | resolved | [bus/in-process-peers-share-the-keyspace.md](bus/in-process-peers-share-the-keyspace.md) |
| 2026-06-26 | store | `ORDER BY data.ts` fails: "Missing order idiom in statement selection" | resolved | [store/order-by-needs-selected-idiom.md](store/order-by-needs-selected-idiom.md) |
| 2026-06-26 | store | `.content()` rejects raw `serde_json::Value` | resolved | [store/content-rejects-serde-json-value.md](store/content-rejects-serde-json-value.md) |
| 2026-06-26 | bus | booting a Node in a test panics: Zenoh needs a multi-thread runtime | resolved | [bus/zenoh-needs-multi-thread-runtime.md](bus/zenoh-needs-multi-thread-runtime.md) |
| 2026-06-26 | auth | a freshly-minted token fails verification with BadToken | resolved | [auth/valid-token-fails-verification.md](auth/valid-token-fails-verification.md) |
