# Debugging — working history

The project's debugging memory: every issue and how it became working, so nothing is
debugged twice. **Append-only and symptom-led.**

- How this works and the entry template: `../scope/debugging/debugging-scope.md`.
- One file per issue, named by the symptom: `<area>/<symptom-slug>.md`.
- Add a row below when you open an entry; update its status when it closes.

## History (newest first)

| Date | Area | Symptom | Status | Entry |
|---|---|---|---|---|
| 2026-06-26 | agent | the agent is `Denied` reading a substrate doc the caller owns (derived-sub vs the S4 membership gate) | resolved | [agent/agent-reads-doc-it-doesnt-own-is-denied.md](agent/agent-reads-doc-it-doesnt-own-is-denied.md) |
| 2026-06-26 | store | `DEFINE BUCKET` fails to parse on the embedded `kv-mem` store (assets stored as records instead) | resolved | [store/define-bucket-unavailable-in-kv-mem-build.md](store/define-bucket-unavailable-in-kv-mem-build.md) |
| 2026-06-26 | bus | `cargo test --workspace` OOM-killed (137) once tests boot 2 nodes each | resolved | [bus/cargo-test-workspace-ooms-with-many-peers.md](bus/cargo-test-workspace-ooms-with-many-peers.md) |
| 2026-06-26 | bus | a live subscriber receives a message published by a *different* test | resolved | [bus/in-process-peers-share-the-keyspace.md](bus/in-process-peers-share-the-keyspace.md) |
| 2026-06-26 | store | `ORDER BY data.ts` fails: "Missing order idiom in statement selection" | resolved | [store/order-by-needs-selected-idiom.md](store/order-by-needs-selected-idiom.md) |
| 2026-06-26 | store | `.content()` rejects raw `serde_json::Value` | resolved | [store/content-rejects-serde-json-value.md](store/content-rejects-serde-json-value.md) |
| 2026-06-26 | bus | booting a Node in a test panics: Zenoh needs a multi-thread runtime | resolved | [bus/zenoh-needs-multi-thread-runtime.md](bus/zenoh-needs-multi-thread-runtime.md) |
| 2026-06-26 | auth | a freshly-minted token fails verification with BadToken | resolved | [auth/valid-token-fails-verification.md](auth/valid-token-fails-verification.md) |
