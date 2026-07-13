# Session — promote `federation` to a first-class core crate

_2026-07-11._ Scope: [`../../scope/extensions/federation-promote-to-core-scope.md`](../../scope/extensions/federation-promote-to-core-scope.md).

## The ask

`federation` lived in `rust/extensions/`, next to the product extensions leaving for `lb-extensions`.
That location was a lie — federation is **core**. Move its source into `rust/crates/` (folder tells the
truth) while keeping its runtime posture identical (supervised Tier-2 sidecar; DB drivers never link
into the node; reached only through the same cap-gated dispatch — rule 10, no special treatment).

## Why federation is core, not an extension (the decision, restated)

1. **Fails the rule-10 swap test.** The host holds a first-class `federation.*` surface
   (`crates/host/src/federation/*`: query, datasource CRUD, `net:*` endpoint gating, dbschema,
   sample/mirror/export) and `FED_ENDPOINTS`/`LB_FEDERATION_*` config reaches *into* it. No opaque
   `<id>.<tool>` seam — the host knows this concern by name. Not swappable ⇒ core surface.
2. **Shares `lb-supervisor` verbatim** — built from the supervision substrate, not a guest of it (like
   `echo-sidecar`).
3. **Platform datastore-federation surface** — README §2 "one datastore"; federation is the
   federated-read face of that pillar. Data-plane, core.

"Promote to a core crate while staying a supervised sidecar" = the SOURCE moves into `rust/crates/`
(normal workspace member, built + shipped by the node's build/docker path, never published to/installed
from `lb-extensions`), but it stays a separate Tier-2 process spawned under `lb-supervisor` with its
manifest/caps/wire and cap-gated dispatch **unchanged**, and its DB drivers still never link into node.

## What was done

- `git mv rust/extensions/federation → rust/crates/federation` (history preserved). Package name kept
  as `federation` (deliberate — binary name, `exec = "federation"`, `cargo build -p federation`, and the
  host's `<install_dir>/federation` resolution are all unchanged; the move is source-relocation only).
- `rust/Cargo.toml`: dropped `extensions/federation` member, added `crates/federation` (alphabetical).
- Three `include_str!("…/extensions/federation/extension.toml")` compile-time manifest reads repointed
  to the new path: `node/src/federation.rs`, and five host/gateway tests (`federation_test`,
  `federation_sqlite_test`, `schema_designer_test`, `query_test`, `rules_buildings_examples_test`,
  `gateway_query_test`).
- Doc-comment `extensions/federation/src/...` references → `crates/federation/src/...` (host sample.rs,
  rules grid.rs + tests).
- **Nothing else changed by design:** the manifest (`extension.toml` id/tier/`exec`/caps/tools), the
  grant computation, the `lb-supervisor` wire, the cap-gated dispatch, `FED_ENDPOINTS`/`LB_FEDERATION_*`,
  and `node/src/federation.rs::federation_dir` (which resolves the binary from the *shared workspace
  `target/`* — independent of where the crate source lives) are all untouched. `make federation` uses
  `-p federation`; the docker path uses `PKG=federation` — both unchanged.

## Docs updated

`MIGRATION.md`, `docs/scope/extensions/ext-out-of-tree-scope.md`, `rust/extensions/README.md` all now
say federation was **promoted to core** (so the upcoming `rust/extensions/*` cleanup does not touch it);
removed the "retained temporarily / stays a workspace member in extensions/" framing. `docs/STATUS.md`
current-stage updated.

## Proof (rule 9 — real, not "compiles")

**Build green + Tier-2 isolation holds (node/host link NO DB driver):**

```
$ cargo build --workspace
   Compiling federation v0.1.0 (/home/user/code/rust/lb/rust/crates/federation)
   Compiling lb-host v0.1.0 (…)
   Compiling lb-node v0.1.9 (…)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.85s   # EXIT=0

$ cargo tree -p lb-node -e no-dev | grep -iE 'datafusion|tokio-postgres|rusqlite'   # (empty)
$ cargo tree -p lb-host -e no-dev | grep -iE 'datafusion|tokio-postgres|rusqlite'   # (empty)
$ cargo tree -p federation | grep 'datafusion '
├── datafusion v53.1.0                        # drivers live ONLY in the federation crate
$ cargo tree -p lb-node | grep -c 'federation v'
0                                             # node does NOT link the federation crate
```

**The host spawns it as a separate process from the resolved target path:**

```
$ file rust/target/debug/federation
 ELF 64-bit LSB pie executable, x86-64 …      # a real 311MB standalone binary
# native/spec.rs::resolve_exec joins exec="federation" to install_dir → <target/debug>/federation
# node/src/federation.rs::federation_dir resolves that dir from the SHARED workspace target/.
```

**Real federation run — register + SELECT + mandatory deny + isolation (no-Docker sqlite path, testing §0):**

```
$ cargo test -p lb-host --test federation_sqlite_test
     Running tests/federation_sqlite_test.rs
running 1 test
test federation_end_to_end_sqlite ... ok
test result: ok. 1 passed; 0 failed; … finished in 0.91s   # EXIT=0
```

`federation_end_to_end_sqlite` (one real E2E) exercises: a real node spawns the real
`target/debug/federation` sidecar over stdio, registers a sqlite datasource, runs `datasource.test`
(probe green), `federation.query`/`schema`/`sample` returning **real rows** — plus the **mandatory**
categories: capability-DENY (a caller without `mcp:federation.query:call` → opaque `Denied`; sample
deny equally opaque) and workspace-ISOLATION (ws-B "other" cannot resolve/query ws-A's datasource). The
`net:*` endpoint-deny (a source whose `host:port` the grant omits → opaque `Denied`) is covered by the
sibling postgres E2E `federation_test.rs` (Docker-gated on this no-cc box) and re-asserted in the
sqlite flow's grant scoping.

**Moved include-paths compile + pass across the host suites:**

```
$ cargo test -p lb-host --test query_test --test schema_designer_test --test rules_buildings_examples_test
running 10 tests … ok. 10 passed
running 1 test  … ok.  1 passed
running 11 tests … ok. 11 passed   # EXIT=0
```

## Result

Federation is now a first-class core crate at `rust/crates/federation/`, still a supervised Tier-2
sidecar, still cap-gated identically, with DB drivers isolated to the crate. The move is behavior-neutral
(source relocation only). No breakage logged — nothing regressed.
