# Session — domain packs in core (the `pack.*` verb family)

Date: 2026-07-18 · Scope: `docs/scope/packs/pack-core-scope.md` · Issue: NubeDev/lb#79
Downstream: NubeIO/rubix-ai#13 (`docs/scope/packs/domain-packs-scope.md`)

## What shipped

The domain-pack engine, end to end, in core. A blank workspace plus one call is now a working
product, and every embedder gets it — not just the one host that proved it.

- **`crates/packs`** — the PURE half, zero I/O: manifest shape (`deny_unknown_fields`,
  line-numbered errors), the bundle→`Pack` resolution, the ordered object plan + checksums, the
  dry-run linter, the refusal matrix, the receipt record. 22 unit tests (the prototype's 14 ported
  verbatim + the bundle/plan tests the wire format needs).
- **`crates/host/src/pack/`** — the verb module, one responsibility per file: `authorize`, `store`
  (receipts), `sqlite` (materialize), `apply` (the orchestration), `verb`/`validate`/`read` (the
  four entry points), `tool` (the MCP bridge), `error`.
- **The four verbs**, on the one MCP dispatch: `pack.validate`, `pack.apply`, `pack.list`,
  `pack.get`.
- **`rules_run_by_id`** — a small host-internal seam in `rules/` so `rules.run` and a pack's
  run-on-first-apply share ONE model-resolution + routing path and cannot drift.
- **10 integration tests** at the house bar (real `Node::boot()`, `mem://`, no mocks) + the ported
  demo oracle.

## The decisions this PR made (the scope left them to it)

**1. Bundle encoding + size cap.** `{manifest, files}` — the raw `pack.yaml` text plus every
referenced file keyed by the path the manifest names it by. The manifest is kept RAW rather than
re-serialized, because the pack checksum folds those exact bytes; a round-tripped manifest would
change the hash and read as spurious drift. Cap: 8 MiB over manifest + all file bodies, with the
standing doctrine unchanged — a big seed is a generator script, not a pack payload.

**2. Lib crate name.** `lb-packs`, plural, to keep distance from `lb-pack` (`tools/pack`), the
unrelated extension-artifact packager. Nothing here touches that toolchain.

**3. Caps.** `pack.apply` is **admin-only** — it writes through every object family at once
(a datasource, rules that then RUN, dashboards, channels, the workspace-shared agent context).
`pack.list`/`pack.get`/`pack.validate` are **viewer** reads: a receipt is operator documentation
(it is how someone learns what turned this workspace into this product), and `pack.validate` is a
pure dry run a pack author must be able to run in CI without an admin token.

**4. Receipts are first-class and INTERNAL.** Table `pack_receipt`, reached through the store API,
never the public `store.*` verbs. This is what retires the prototype's whole workaround: it wrote
receipts with `store.write` and read them back with a hand-written
`SELECT data FROM pack_receipts WHERE data.pack = '<pack>'` — a query shaped entirely by store
envelope quirks (the `{data:…}` wrapper; a `thing` id that 502s `SELECT *`). Gone.

**5. The sqlite materializer — the one real tension, recorded honestly.** A pack ships raw
`schema.sql`/`seed.sql`, but federation's `Source` trait deliberately refuses caller SQL
(`apply_ddl` takes the migrate planner's allow-listed statements; `write_rows` takes structured
rows). Three options were weighed: register-only (cleanest against the trait, but it kills the demo
oracle — nothing seeds the data); a host-side sqlite materializer; or a new general `exec_sql` seam
on the trait.

Chose the **materializer**, scoped as narrowly as it goes: sqlite only, in-process via the bundled
`rusqlite` (the prototype shelled out to a `sqlite3` CLI — that dependency is dropped), writing a
node-local file under `{LB_DIR|.lazybones}/packs/{ws}/{pack}/`, path-sanitized exactly the way
`ext/install_dir.rs` sanitizes a native extension's home. Any other engine REGISTERS ONLY and the
linter warns the author their SQL will not run.

**The cost, stated plainly:** the datasource is the one object kind that is not pure
bundle-over-the-wire — it touches this node's filesystem. Every other kind is filesystem-free, so a
third party applying a pack with no `datasource` block still needs nothing but a session and caps.
**Follow-up for federation scope:** whether a general, per-source, admin-capped `exec_sql` belongs
on the `Source` trait. That is a federation decision, deliberately not made here.

## The bug this session found (and why it was invisible)

`pack.list` returned `{"packs":[]}` for every workspace, always — while `pack.get` worked perfectly.

`lb_store::write` stores a `{data, rev}` envelope. `read` unwraps it (`SELECT data FROM …`), which
is why `read_receipt`, `pack.get`, and the entire refusal matrix were correct. But `scan` selects
the whole record, so `Row::data` is the ENVELOPE — the decode always failed, and an `if let Ok(…)`
swallowed it. A silent, total, permanent empty roster behind a passing-looking surface.

Fixed by unwrapping the inner `data` — and the `Err` arm is now **loud** (`tracing::warn!`). The
swallow is the actual lesson: a receipt that will not decode is a corrupt record, not an absent
one, and dropping it silently under-reports what is applied to a workspace. Caught only because the
integration test asserted on the real roster; verified again live on a rebuilt node.

## Rule 10

Core knows no pack by name. Every branch in `pack/` is on an object KIND (`rule`, `dashboard`,
`channel`, …), which is data. `bas` vs `ems` differ only in bytes, and the `bas` fixture lives under
`tests/fixtures/packs/` as test data, never as core knowledge.

## No cap smuggling — proven, not asserted

`mcp:pack.apply:call` gets a caller into the orchestration and **nothing more**. Every object is
driven through the same internal function the equivalent public verb calls
(`rules_save`, `dashboard_save_meta`, `datasource_add`, `channel_create`, `memory_set`), and each of
those re-runs its own capability check under the caller's principal.

The integration test `a_cap_denied_partial_recovers_when_the_cap_is_granted` proves both halves: a
principal holding `pack.apply` + `agent.memory.set` but NOT the workspace-scope memory write cap
gets `outcome: partial` with the agent object `denied` and the channel object `applied` — then
re-applying the identical bundle at the same version with full caps re-applies (does not no-op),
fixes the agent object, and reports `ran_rules: false`. That single test covers the partial-recovery
row of the matrix, the per-object cap re-check, and the run-rules-once rule.

## Live verification (the money-shot)

Against a real booted node, workspace `acme`, through the rubix-ai CLI calling these verbs:

```
make pack-validate PACK=bas   → 11-object plan, valid, "applying now would: apply"
make pack-apply PACK=bas      → 11× APPLIED, "done — 'bas' applied, receipt written."
insight.list                  → fdd:sensor-flatline:meter-020-zt   ← the money-shot
                                 energy-intensity-high:Northside Factory
pack.list                     → the roster, complete:true, 11 objects
make pack-apply PACK=bas      → "idempotent no-op"
```

Blank workspace → one command → a real insight raises. The rules ran against the node-materialized
sqlite source through the real federation sidecar.

## Test status

- `cargo test -p lb-packs` — 22 passed.
- `cargo test -p lb-host --lib` — 273 passed (incl. the cap-tier and catalog-coverage invariants).
- `cargo test -p lb-host --test pack_test` — 9 passed, 1 ignored (the demo oracle, which needs the
  federation sidecar built; run with `-- --ignored`).
- `cargo fmt --all --check` — clean.

## Downstream, in the same session

rubix-ai's prototype engine is **deleted**, not deprecated: `manifest/loader/plan/validate/decision/
receipt/apply/sqlite` are gone, and `crates/pack-apply` is now a thin caller (~350 lines: read the
directory into a bundle, call the verb, render the answer). Its dep list is the evidence — no
`serde_yaml`, no `sha2`, no `sqlite3` shell-out. The `bas` pack stays as data, and the e2e now
asserts the receipt through `pack.get` instead of `store.query`.

## Follow-ups (named, not done)

- **federation `exec_sql`** — see decision 5. The general seam question.
- **Upgrade mechanics** (v1→v2): still foreclosed-nothing. Receipts carry per-object `{id, checksum}`
  precisely so skip-if-modified and real upgrades stay cheap to add.
- **`entities` stays a vocabulary** — the instability warning is carried in the manifest doc comment.
