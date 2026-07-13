# Extensions scope — promote `federation` to a first-class core crate (stays a supervised sidecar)

Status: scope (the ask). Promotes to `public/extensions/extensions.md` (rule-10 / data-plane note) once shipped.

`federation` lives in `rust/extensions/`, next to the product extensions that are leaving for
`lb-extensions` (out-of-tree scope). That location is a lie: `federation` is **core**, not a product
extension, and the out-of-tree scope already carved it out as the "stays" exception. This scope
finishes the job — it **moves the source into `rust/crates/`** so the folder tells the truth, while
leaving its *runtime posture unchanged*: it is still a supervised Tier-2 sidecar, its DB drivers still
never link into the node, and it is still reached only through the same cap-gated dispatch (rule 10 — no
special treatment).

## Why federation is core, not an extension (the swap test, decided already, restated)

1. **Fails the rule-10 swap test.** Rule 10's leak test: if swapping an equivalent extension for another
   would force a change in a core mediation crate, that thing is core surface. `federation` is not
   swappable — the host holds a first-class `federation.*` surface (`crates/host/src/federation/*`:
   query, datasource CRUD, endpoint gating, dbschema, sample/mirror/export) and `FED_ENDPOINTS`/
   `LB_FEDERATION_*` config reaches *into* it. There is no opaque `<id>.<tool>` seam here; the host
   knows this concern by name because it is the query-across-sources face of the platform datastore.
2. **Shares `lb-supervisor` verbatim.** It is built from the supervision substrate itself (the wire
   protocol crate), not a guest of it — exactly like `echo-sidecar`, the other in-tree native that is a
   host fixture, not a product ext.
3. **It is platform datastore-federation surface.** README §2 "one datastore" is a core pillar;
   `federation` is the federated-read face of that pillar. SurrealDB stays the authority; external SQL
   sources are reached only through this gated, `net:*`-bounded process. That is data-plane, core.

## Decisions (open questions resolved)

- **Target location + crate name:** `rust/crates/federation/`, package name **`federation`** (unchanged).
  A sibling of the other core crates, a normal workspace member. Keeping the package name identical is
  deliberate: the binary name, the manifest `exec = "federation"`, `cargo build -p federation`, and the
  host's `<install_dir>/federation` resolution are all **unchanged** — the move is source-relocation
  only, zero runtime/build-invocation surface changes. Rejected `lb-federation`: it would churn the
  binary name / `exec` / every `-p` invocation for no gain, and the crate is a **binary sidecar**, not a
  `lb-*` library other crates depend on (nothing links it — that is the whole point).
- **Relationship to `crates/host/src/federation/*`:** unchanged and correct. The host module is the
  **host side** (resolve source → `net:*` gate → mediate DSN → dispatch `federation.query`); the crate is
  the **child side** (embeds DataFusion, owns the sockets/pools). They stay two halves of one seam,
  communicating over the `lb-supervisor` stdio wire. The host side does **not** move; only the sidecar
  binary's source relocates from `extensions/` to `crates/`.
- **Stays a separate binary/process (Tier-2 sidecar).** The DB drivers (`datafusion`,
  `datafusion-table-providers`, `datafusion-federation`, `tokio-postgres`, `rusqlite`, vendored
  `openssl`) live **only** in this crate and must **never** link into `node`/`host`. It remains a
  `[[bin]]` the host spawns over stdio under `lb-supervisor` — never an in-process library call.
- **How the host locates/spawns it after the move:** **unchanged.** The binary is resolved from the
  shared workspace `target/{debug,release}/federation` dir (`node/src/federation.rs::federation_dir`,
  overridable with `LB_FEDERATION_DIR`) — a location that is *independent of where the crate source
  lives*, because a workspace shares one `target/`. `native/spec.rs::resolve_exec` joins the manifest's
  `exec = "federation"` to that `install_dir`. Nothing here references `extensions/federation`. The only
  source-path couplings to update are three `include_str!("…/extensions/federation/extension.toml")`
  compile-time reads (node + 2 tests) and a handful of doc-comment path mentions.

## What "core-owned" changes (and what it doesn't)

- **Changes:** the source dir (`rust/extensions/federation/` → `rust/crates/federation/`); the workspace
  `members` entry; the `include_str!` manifest paths; doc-comment `extensions/federation/src/...`
  references; the retention framing (MIGRATION.md, out-of-tree scope, `rust/extensions/README.md` now
  say federation is **promoted to core**, not "retained temporarily").
- **Does NOT change (rule 10 — no special treatment):** the `extension.toml` manifest (id `federation`,
  tier `native`, `exec`, caps `net:tls:*:*:connect` + `secret:federation/*:get`, the four tool
  declarations); the `requested ∩ admin_approved` grant computation; the `lb-supervisor` wire; the
  cap-gated dispatch path (`mcp:federation.query:call` etc., workspace-first then caps). It ships WITH
  the node (built by `make federation` / the docker `PKG=federation` path), **not** published to or
  installed from `lb-extensions`. A "core" extension takes the exact same install/auth/dispatch path a
  third-party native would.

## Non-goals

- Do **not** link DB drivers into `node`/`host` (grep must stay clean).
- Do **not** move it to `lb-extensions`.
- Do **not** change the `federation.query` / `datasource.test` / `federation.schema` / `federation.sample`
  (or `federation.write`/`mirror`/`export`) verb surface, their caps, or the `FED_ENDPOINTS`/
  `LB_FEDERATION_*` env contract.
- Do **not** split or move `crates/host/src/federation/*` (that host-module-as-API extraction is a
  separate, later scope, per the out-of-tree scope's non-goals).

## Testing plan (rule 9 — real, not "compiles")

- `cargo build --workspace` green; `federation` builds as a core crate; **grep confirms `node`/`host`
  link no DB driver** (`datafusion`/`postgres`/`tokio-postgres` absent from their dep graph).
- Run the node and exercise federation for real (sqlite no-Docker fallback, testing §0): register a
  datasource, run a SELECT via `federation.query` returning real rows; `datasource.test` green.
- **Capability-DENY (mandatory):** a source whose `host:port` is not in the `net:*` grant is refused
  pre-connect (opaque).
- **Workspace-isolation (mandatory):** ws-B cannot resolve/query ws-A's datasource.
- Confirm the host spawns the sidecar from its resolved `target/` path as a **separate process** (not
  linked into the node) — check the resolved exec path / running process.
- `cargo test --workspace` green for the federation suites (`federation_test`, `federation_sqlite_test`,
  the gateway query test) from the new paths.

## Related

- `ext-out-of-tree-scope.md` — names federation as the "stays core" exception; this scope executes it.
- `datasources` scope docs — the federation/datasource design (verb surface, `net:*` gating, DSN
  mediation) this preserves verbatim.
- `MIGRATION.md` — the retention framing updated by this scope.
- README §2 (one datastore), §3 rules 5/9/10, §6.3 (two tiers).
