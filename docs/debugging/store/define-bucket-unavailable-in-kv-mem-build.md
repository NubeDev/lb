# `DEFINE BUCKET` fails to parse on the embedded store

- Area: store
- Status: resolved (by-design workaround)
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/files/shared-assets-session.md
- Regression test: rust/crates/assets/tests/store_isolation_test.rs (proves the
  content-as-record path that replaces buckets is workspace-isolated)

## Symptom

README §6.1/§6.12 specify file/doc storage "via SurrealDB buckets" (`DEFINE BUCKET`). When
scoping S4 shared assets, a probe query to confirm the feature was available on our embedded
store failed at parse time:

```
DEFINE BUCKET test BACKEND "memory"
→ Parse error: Unexpected token `an identifier`, expected a define statement keyword
```

## Reproduce

In a throwaway test against the same engine the host uses
(`surrealdb::engine::local::Mem`, the workspace's `surrealdb = { default-features = false,
features = ["kv-mem"] }`):

```rust
let db = Surreal::new::<Mem>(()).await.unwrap();
db.use_ns("probe").use_db("main").await.unwrap();
db.query("DEFINE BUCKET test BACKEND \"memory\"").await; // → parse error
```

## Investigation

- The error is a **parse** error, not a runtime/backend one — the `DEFINE BUCKET` statement
  keyword is not recognized by the query parser in this build at all.
- Our `Cargo.toml` pins `surrealdb` with `default-features = false` and only `kv-mem`. Bucket /
  file storage is a newer, feature-gated capability (README §6.12 itself flags it: "file support
  is recent and currently experimental"). It is simply not compiled into our minimal build.
- Enabling the file-storage features would pull a heavier dependency set and an experimental
  surface for what S4 needs (small text assets: scope docs, skill bodies) — wrong trade now.

## Root cause

`DEFINE BUCKET` (SurrealDB file storage) is not available in the `kv-mem`,
`default-features = false` build the project uses. The README's "via buckets" is the intended
*physical backing at cloud scale*, not a requirement satisfiable on today's embedded engine.

## Fix

Store asset **content as a record value** in the workspace namespace (the existing
`lb_store::{write,read,list}` path), not in a bucket. This keeps every S4 non-negotiable:
"SurrealDB only, no separate blob service", workspace = namespace isolation, state-vs-motion.
The asset verbs (`put_doc`/`get_doc`/…) take/return **opaque content**, so the verb signature is
bucket-compatible: swapping to a real `DEFINE BUCKET` + S3/GCS backend at S7 is config behind the
same verb, not an API re-cut. Recorded as the design decision in
`docs/scope/files/files-scope.md` (Non-goals + Intent) and the open question (the bucket cutover
is the heavy-blob/scale trigger).

## Verification

The assets crate's store verbs work on the real embedded engine and are workspace-isolated:
`cargo test -p lb-assets` → 8 passed (incl. `store_isolation_test`). The host asset service
(18 host tests) reads/writes content through this path with no bucket.

## Prevention

- The files scope now states the bucket-vs-record decision explicitly, so the next person does
  not re-discover the parse failure.
- The asset verb signatures are content-opaque (a guardrail: nothing leaks the SurrealDB record
  shape to callers), so the eventual bucket backend is a drop-in.
- Follow-up (S7): when heavy/binary blobs arrive, enable the file-storage features and back the
  same verbs with `DEFINE BUCKET` over an S3-compatible store; measure before adopting.
