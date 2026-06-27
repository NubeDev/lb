# SurrealKV "Invalid revision N for type Value" on the second ingest drain (persistent store only)

- Area: store (SurrealKV persistent engine) — surfaced via ingest drain
- Status: documented; engine-level, NOT an application bug. Worked around for the live demo by running the demo node on the in-memory engine.
- First seen: 2026-06-27
- Resolved: no — needs a surrealdb/kv-surrealkv fix or a store-layer mitigation (out of proof-panel's scope)
- Session: ../../sessions/extensions/proof-panel-session.md
- Regression coverage: the ingest round-trip is proven exactly-once on `mem://` (host `ingest_test`, `proof_panel_test`, `ProofPanel.gateway.test.tsx`); a persistent-engine regression test belongs with the store owner.

## Symptom

On the **persistent SurrealKV store** (`LB_STORE_PATH` set), the FIRST ingest write to a workspace
commits fine, but ANY subsequent `ingest.write`/`POST /ingest` (i.e. the next `drain_workspace` that
must read back staging on top of an already-committed `series` table) fails with:

```
store backend error: Versioned error: A deserialization error occured: Invalid revision `N` for type `Value`
```

`N` varies (`0`, `98`, `100`, …). On the in-memory engine (`mem://`, every unit/gateway test) the
same writes succeed indefinitely.

## Reproduce

1. Boot the node with `LB_STORE_PATH=…` (persistent SurrealKV).
2. `POST /mcp/call {tool:"ingest.write", …}` (or `POST /ingest`) once → `{accepted:1}`.
3. Repeat the write → `403` / the "Invalid revision" store error above.

Confirmed with the **untouched** `POST /ingest` route, so it is independent of this slice's
`call_tool` change (the bridge `ingest.write` arm merely reuses the same `ingest_write` +
`drain_workspace` path).

## Investigation

- The error fires in `lb_ingest::commit::drain`'s `resp.take(0)` while deserializing the staged
  `sample` rows — SurrealKV returns a `Value` tagged with a revision byte the reader rejects. It is a
  serialization-version mismatch inside the engine, not in our SQL or our types (the same SQL + types
  work on `kv-mem`).
- It is the same root cause as the pre-existing on-disk store corruption (the initial dev-store threw
  `Invalid revision 0`); a fresh store delays it to the *second* write rather than removing it.
- Pinned: `surrealdb = "2"` with `features = ["kv-mem", "kv-surrealkv"]` (root `Cargo.toml`).

## Fix (workaround used here)

The proof-panel live demo (the Playwright e2e) runs the node on the **in-memory engine** (unset
`LB_STORE_PATH`) — still a REAL node (real caps, bus, ingest path, federation), just ephemeral. This
proves the write→stage→drain→read round-trip end to end without the engine bug. All automated tests
already use `mem://`.

## Follow-up (root fix, not done here — store owner)

Reproduce against `kv-surrealkv` in `lb-store`, bisect the surrealdb 2.x point release that introduced
the revision tag, and either bump/pin the engine to a compatible version or add a store-layer
read-compat shim. Until then, durable on-disk ingest is unreliable across more than one write per
workspace. This blocks the S8 "data survives a node restart" guarantee for the ingest path on real
disk and should be prioritized by the store owner.
