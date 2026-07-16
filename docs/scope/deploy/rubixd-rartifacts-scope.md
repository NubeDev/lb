# Deploy scope — rubixd + rartifacts (fleet package agent + artifact server)

Status: scope (the ask). Promotes to `doc-site/content/public/deploy/` once shipped.

We want a **per-machine install/update agent (`rubixd`)** and a **remote artifact server
(`rartifacts`)** so a product host built on lb — `rubix-ai`, `ems`, and their companions
(TimescaleDB, extensions) — can be installed, updated, and rolled back on a fleet of
machines without hand-run scripts. You upload a package to rartifacts; rubixd on each
machine checks for updates, downloads the artifact, verifies it, and installs or updates
it through the right backend: **systemd** (via the `service-manager` crate) for bare
binaries, **Docker** (via `bollard`) for containers. A **bundle YAML** declares a set of
packages + instances as desired state (e.g. `timescaledb` + `rubix-ai` + extensions); a
failed update **rolls back automatically**; the same package can be installed **more than
once** on one machine as named instances.

**rartifacts is optional — rubixd is fully standalone.** A rubixd can also be **published
to directly**: it exposes the *same* `POST /packages` REST contract locally, so an
operator (or CI, or a `rubixd publish` CLI) can push a signed package straight onto one
machine with **no rartifacts server in the loop at all**. rubixd verifies and caches it
in its own content-addressed blob store, records it in a **local package index**, and a
bundle resolves against that index exactly as it would against a remote. So rubixd works
in three postures with one code path: **fleet-fed** (polls one or more rartifacts),
**standalone** (only ever pushed to locally), or **both** (local packages plus remotes).
rartifacts remains the answer for *fleets* — one publish fanned out to N machines — but a
single box never depends on it.

Both are **new and out-of-tree**, but their postures differ **on purpose**:

- **rartifacts is an lb product host** (exactly the `ems`/`rubix-ai` pattern): a thin
  binary embedding `lb-node`, with all package logic in a **native (Tier-2)
  `rartifacts` extension** and its operator console as the extension's **federated UI**
  mounted on the minimal shell (`frontend/minimal-shell-scope.md`). It inherits lb's
  identity + api-key machine principals, the capability wall, the one datastore, and
  the extension-UI stack instead of re-implementing them.
- **rubixd is a standalone daemon** — deliberately NOT an lb node. It runs on machines
  *before* any lb host exists, must stay tiny on armv7 boxes, and is the thing that
  installs/rolls back lb hosts — it cannot share their runtime. It reuses lb's
  *conventions* only: the signed-artifact envelope (Ed25519 over a length-prefixed
  SHA-256 digest, publisher-key allow-list — `rust/crates/registry/`), the
  atomic-executable install pattern (`rust/crates/host/src/ext/install_dir.rs`), and
  the cross-build toolchain that already emits `linux-x86_64` / `linux-arm64` /
  `linux-armv7` binaries (`docker/build/`).

**The wire contract between them is plain REST regardless**: rubixd never links lb —
the rartifacts host mounts the package routes (the ems `ems_mount.rs` precedent), so
agents stay dumb HTTP clients and rartifacts' internals can evolve freely.

**This is the umbrella.** The build decomposition — one scope per codeable slice plus a
session-by-session roadmap for long-running AI sessions — lives in
[`rubixd/`](rubixd/README.md) (7 slices) and [`rartifacts/`](rartifacts/README.md)
(5 slices). Build order: rubixd 1–2 first (they create the shared `fleet-spec` +
`fleet-auth` crates), then the two tracks run in parallel; rubixd slice 6 integrates
against a real rartifacts.

## Goals

- **rartifacts**: a REST server that stores packages — systemd binaries (multi-arch),
  Docker image archives, and bundle manifests — each version carrying metadata
  (semver, arch, SHA-256, Ed25519 signature, config schema, health-check spec) and a
  content-addressed blob. Publish is authenticated; download verifies end to end.
- **rubixd**: a small daemon per machine. Reads bundle YAMLs as desired state, reconciles
  against what is installed, polls rartifacts for in-range updates, and drives installs/
  updates/rollbacks through two backends:
  - `systemd` — versioned release dirs + a `current` symlink, generated unit + env file
    per instance, `service-manager` for install/start/stop/status.
  - `docker` — `bollard` for pull/load/create/recreate/stop/remove; containers labeled
    with package/version/instance so rubixd owns only what it created.
- **Rollback on failure.** Every update is a transaction: download → verify → stage →
  swap → health gate → commit. A failed health gate flips back to the previous release
  (symlink / previous image), restarts it, and marks the new version bad (skipped until
  an operator clears it). Rollback is also an operator verb (`rubixd rollback <instance>`).
- **Multi-instance installs.** A bundle names instances (`rubix-main`, `rubix-lab`); each
  gets its own unit/container name, env, ports, and data dir. Package metadata declares
  which config keys must be unique per instance so rubixd can reject a colliding bundle.
- **Data survives updates.** Release artifacts and instance data are physically separate
  (`/opt/rubix/<pkg>/releases/<v>/` vs `/var/lib/rubix/<instance>/`), so an update or
  rollback never touches the store, extension-UI dir, or signing key of a running
  product node (`RUBIX_STORE_PATH`, `RUBIX_EXT_UI_DIR`, `RUBIX_SIGNING_KEY` — see
  `rubix-ai/src/boot.rs`).
- **Each service has its own web UI + one token model.** Both boot generating an admin
  token, **claimable exactly once from the UI**, then presented as `Authorization:
  Bearer` on every REST call — the UI and curl share one auth path
  (`fleet-auth` crate; [`rubixd/token-auth-scope.md`](rubixd/token-auth-scope.md),
  [`rartifacts/token-auth-scope.md`](rartifacts/token-auth-scope.md)). rubixd's UI is
  deliberately small — embedded **Bootstrap 5**, no build step
  ([`rubixd/embedded-ui-scope.md`](rubixd/embedded-ui-scope.md)); rartifacts' is the
  rich operator console — **React + TailwindCSS + shadcn/ui** as the extension's
  **federated UI pages** on the lb minimal shell
  ([`rartifacts/web-ui-scope.md`](rartifacts/web-ui-scope.md)). On rartifacts the
  claim reveals the boot-seeded **admin api-key**; **publisher** and **agent**
  identities are lb api-key principals minted under it.
- **Standalone local publish (rartifacts-optional).** rubixd exposes `POST /packages`
  on its own local REST surface (Bearer-gated by the admin/agent token, the same
  `fleet-auth` path as every other route) — a streaming multipart publish identical to
  rartifacts' contract. It verifies the artifact (SHA-256 + Ed25519 against
  `trusted_pubkeys`) before it touches disk, stores the blob in the local
  content-addressed cache, and registers a row in a **local package index** (the
  embedded ledger). A `rubixd publish <metadata.toml> <blob>` CLI wraps the same route
  for hand/CI pushes. A locally-published package resolves through the ledger with **no
  network**; a bundle references it exactly like a remote one (implicitly, or pinned
  `remote: local`). This is what makes a single machine 100% independent of any server.
- **Unlimited remotes.** One rubixd can connect to **any number of rartifacts servers**
  (`[[remote]]` entries in its config: name, URL, optional agent token). A bundle
  package may pin `remote: <name>`; otherwise remotes are searched in declared order,
  first match wins (deterministic). The blob cache is content-addressed, so the same
  artifact from two remotes is stored once.
- **Registered agents, revocable from the server.** The superadmin token stays the
  root of each rartifacts. Under it, each rubixd instance is **registered as an
  agent** — a named record with its own token, `last_seen`, and machine metadata —
  and rartifacts can **revoke any agent** at any time (instant: tokens are checked
  per request). A revoked agent loses private-package access on its next poll;
  what's already installed keeps running.
- **Public and private packages.** Package `visibility` is `private` (default —
  resolve/download require a registered agent token) or `public` (**no auth needed**
  to list/resolve/download; anonymous listings show only public packages). Publishing
  always requires a publisher token regardless of visibility. So one rartifacts can
  serve open downloads (e.g. a public rubix-ai channel) and gated ones side by side.
- **Bundles order their pieces**: `needs:` edges (e.g. `rubix-ai` after `timescaledb`)
  give a deterministic install/update order; health-gated so a dependent never starts
  against a dead dependency.

## Non-goals

- **Not an orchestrator.** No scheduling, no placement decisions, no cross-machine
  coordination — one rubixd manages one machine from its local desired state. Fleet-wide
  control planes (push a bundle to N machines) are a later slice on top of the same API.
- **Not a config-management system.** rubixd templates env/units from the bundle YAML;
  it does not manage arbitrary files, users, or OS packages (no Ansible replacement).
- **No OCI registry protocol.** Docker delivery is (a) public images pulled straight from
  their registry via bollard, or (b) `docker save` archives stored in rartifacts and
  `docker load`-ed — one auth model, works air-gapped. Running a `distribution/registry`
  was rejected: a second protocol + auth surface for zero v1 benefit.
- **No changes to lb core.** rartifacts reuses lb's *conventions* (and possibly the
  `lb-registry` digest/signing code as a git dep); it never widens an lb crate. lb's own
  extension registry (`ext.publish` → gateway) stays exactly as shipped — rubixd delivers
  the *host binary*; extensions still install through the running node's gated API.
- **Not lb's fly/container deploy.** `fly-deploy-scope.md` ships one cloud container;
  this scope is per-machine (edge-heavy) lifecycle. They coexist.

## Intent / approach

### Package model (shared spec crate)

One `Package` record per (name, version, arch), one blob per artifact, content-addressed
by SHA-256. Kinds:

| kind | payload | installed by |
|---|---|---|
| `systemd` | raw ELF binary (per arch) | release dir + unit via `service-manager` |
| `docker-image` | none (an image *reference*) | `bollard` pull from its registry |
| `docker-archive` | `docker save` tarball (zstd) | `bollard` import (`docker load`) |
| `bundle` | the YAML manifest itself | rubixd reconcile |

Metadata (TOML, mirroring `extension.toml`'s spirit): `[package]` name/version/
description; `[artifact]` arch/sha256/size/signature/publisher_key_id; `[config]` the
env schema — each key with default, `required`, and `per_instance: true` for keys that
must differ across instances (ports, data dirs); `[health]` check spec (`systemd-active`,
`tcp:<port>`, or `http:<path>` + timeout); `[preserve]` data paths that must never live
inside the release dir.

Signing reuses the lb pattern verbatim: Ed25519 over a length-prefixed SHA-256 of
(metadata, payload); rubixd holds a `trusted_pubkeys` allow-list in
`/etc/rubixd/config.toml` — the same trust posture as `LB_TRUSTED_PUBKEYS`. A blob whose
digest or signature fails verification is never executed or loaded, period.

### rartifacts — an lb product host + one native extension

**Decision (revised): rartifacts is built ON lb.** A `rartifacts` host binary embeds
`lb-node` (the ems pattern: env-driven `BootConfig`, boot-seeded `fleet` workspace);
a **native Tier-2 `rartifacts` extension** owns all package logic — `pkg.*` MCP tools
(`pkg.list/get/resolve/publish/promote/yank/set_visibility`), package/artifact/channel/
event records in the node's SurrealDB (workspace-walled, via the host callback), and
the **content-addressed blob dir on disk** (`blobs/<sha256>` — a native extension is
the sanctioned owner of external resources per the reference-extensions doctrine;
multi-GB archives never live in store records). The operator console is the
extension's **federated UI pages** (shadcn/Tailwind — the standard ext-UI stack) on
the **minimal shell**.

The earlier rejection ("the deploy plane must work when the product plane is broken")
is *refined*, not forgotten: what stays rejected is installing the extension on a
**shared production node** as the default. The recommended deployment is a
**dedicated rartifacts node** — its own binary, own store, own lifecycle — so a bad
product rollout still can't take the artifact plane down. That the same extension
*can* be installed on an existing cloud node for small setups is a bonus, not the
default.

What lb gives for free vs. the standalone-axum design it replaces: identity + api-key
machine principals (hashed bearer, instant revoke — `auth-caps/api-keys-scope.md`)
instead of a hand-rolled token table; the capability wall + deny paths; SurrealDB +
audit trail; UI federation instead of a second SPA host. Three things lb does **not**
give, bridged as **host-mounted routes** (the ems `ems_mount.rs` seam — no core crate
is touched, rule 10 intact):

- `POST /packages` — streaming multipart publish (blob → ext blob dir, then
  `pkg.publish` with the digest; MCP bodies are not the place for 8 GiB).
- `GET /packages/*`, `GET /blobs/{sha256}` — the **rubixd wire contract**: plain REST,
  Range/ETag streaming from the ext-owned blob dir, and the **anonymous tier** for
  `public` packages (lb has no anonymous principal — the host routes run public reads
  under a boot-minted, read-only `anonymous` api-key whose caps reach only the
  public-read tool; private = real agent api-key, checked by lb).
- `POST /api/claim` — the one-time superadmin claim (fleet-auth), which reveals the
  boot-seeded **admin api-key** once; thereafter everything is normal lb auth.

Agents and publishers are **api-key principals** with narrow caps (agent →
`mcp:pkg.resolve:call` + blob read; publisher → `pkg.publish/promote` on owned
packages); **revoking an agent = revoking its api-key** (instant, per-request check —
already shipped lb behavior).

### rubixd

Desired state = the applied bundles, kept in `/etc/rubixd/bundles.d/*.yaml` (applied via
`rubixd apply bundle.yaml` or dropped in by hand). Installed state = a local ledger
(`/var/lib/rubixd/state.db`, embedded SurrealDB) recording per instance: package,
version, backend, release path/image id, health, last N versions for rollback, and
bad-version marks. A reconcile loop (also `rubixd reconcile` one-shot) diffs the two and
executes transitions; an update poller re-resolves channel/range pins against rartifacts
on an interval.

**Local package index + standalone publish.** rubixd owns a content-addressed blob cache
(`/var/lib/rubixd/blobs/<sha256>`, keyed by digest — shared by the poller's downloads and
by local uploads, so an artifact present either way is stored once) and a **local package
index** in the ledger (`pkg_local`: one row per (name, version, arch), pointing at its
blob + parsed metadata). `POST /packages` (multipart: metadata TOML + blob stream) and its
`rubixd publish` CLI wrapper stream the blob to a temp file, verify SHA-256 + Ed25519
against `trusted_pubkeys` (**verify-before-store** — a bad blob never lands in the cache),
move it to `blobs/<sha256>` via the atomic temp+rename `install_dir.rs` pattern, and upsert
the index row. Re-publishing an identical digest is idempotent. **Resolution order**: the
local index is consulted **first** (a pinned/matching local version wins deterministically
and needs no network), then configured remotes in declared order — so a standalone box with
zero remotes resolves entirely from what was pushed to it, and a fleet box can still pin a
one-off local build over the channel. `remote: local` in a bundle forces the local index
and errors if the package is absent (no silent fall-through to a remote).

**systemd layout** (multi-instance falls out of it):

```
/opt/rubix/<pkg>/releases/<version>/<binary>   # immutable, verified, shared by instances
/opt/rubix/<pkg>/instances/<instance>/current  # symlink → a release dir
/etc/rubix/<instance>.env                      # rendered from bundle config
/etc/systemd/system/rubix-<pkg>-<instance>.service
/var/lib/rubix/<instance>/                     # data dir — never touched by rubixd
```

Update = stage new release dir (atomic: temp + rename, the `install_dir.rs` pattern) →
stop unit → flip symlink → start unit → health gate → commit (prune to N kept releases)
or roll back (flip symlink back, start, mark bad).

**docker**: container per instance, named `<pkg>-<instance>`, labeled
`rubixd.package/version/instance/bundle`. Update = pull/load new image → stop + rename
old container (kept) → create + start new → health gate → commit (remove old) or roll
back (remove new, restore old). Volumes carry the data; rubixd never deletes a volume.

### Bundle YAML (the user-facing contract)

```yaml
bundle: site-alpha
packages:
  - name: timescaledb
    kind: docker-image
    image: timescale/timescaledb:2.15.2-pg16    # public image, pulled directly
    instances:
      - name: tsdb-main
        env: { POSTGRES_USER: lb, POSTGRES_PASSWORD: "${secret:tsdb-main/pw}", POSTGRES_DB: lb }
        ports: ["5433:5432"]
        volumes: ["tsdb-main-data:/var/lib/postgresql/data"]
        health: { tcp: 5433, timeout: 60s }
  - name: rubix-ai
    kind: systemd
    version: "stable"            # channel; or "0.4.5" exact, or ">=0.4, <0.5"
    needs: [tsdb-main]
    instances:
      - name: rubix-main
        env:
          RUBIX_HOME: /var/lib/rubix/rubix-main
          RUBIX_GATEWAY_ADDR: "0.0.0.0:8099"
          RUBIX_SIGNING_KEY: "${secret:rubix-main/signing_key}"
        health: { http: "http://127.0.0.1:8099/health", timeout: 30s }
      - name: rubix-lab
        env:
          RUBIX_HOME: /var/lib/rubix/rubix-lab
          RUBIX_GATEWAY_ADDR: "0.0.0.0:8199"
          RUBIX_SIGNING_KEY: "${secret:rubix-lab/signing_key}"
        health: { http: "http://127.0.0.1:8199/health", timeout: 30s }
```

`${secret:...}` resolves from a root-owned local secrets file
(`/etc/rubixd/secrets.toml`, 0600) — secrets never live in the bundle YAML or in
rartifacts. Two instances of the same package are just two entries; the `per_instance`
keys in the package's config schema (here the port + `RUBIX_HOME`) are validated unique
at apply time.

### Repo / crate shape

One new repo (working name `rubix-fleet`), FILE-LAYOUT rules applied:

- `crates/fleet-spec` — package/bundle/artifact types + digest/signing (shared; **no
  lb dependency** — rubixd stays lb-free).
- `crates/fleet-auth` — the one-time-claim + bearer mechanics (rubixd's whole auth;
  rartifacts uses it only for the claim bootstrap).
- `crates/rubixd` — the agent (`backend/systemd/`, `backend/docker/` as
  folders-of-verbs, `reconcile/`, `rollback/`).
- `rartifacts/host/` — the product-host binary embedding `lb-node` (git-tag pin, the
  rubix-ai pattern) + the mounted public/publish/claim routes.
- `rartifacts/extensions/rartifacts/` — the native extension (`extension.toml`,
  `pkg.*` tools, blob store) + `ui/` (the federated pages).

Alternatives rejected: two repos (the spec crate would immediately need publishing or
git-pin gymnastics for zero benefit); rubixd-also-on-lb (a full node — SurrealDB,
Zenoh, wasmtime — on every armv7 box it must *bootstrap* inverts the dependency).

## How it fits the core

- **Tenancy / isolation**: rartifacts is a real lb tenant — one boot-seeded `fleet`
  workspace walls all `pkg_*` records; the workspace-isolation test is mandatory and
  real. rubixd sits below lb (N/A); its wall is *ownership* (it only manages
  units/containers it created — label/marker check before any stop/remove).
- **Capabilities**: real on rartifacts — every `pkg.*` tool is a capability-gated MCP
  tool; agent/publisher api-keys carry narrow grants; the deny path is lb's own
  (401/403 from the wall) plus: publish with an untrusted/foreign publisher key → 422;
  private pull with a revoked agent api-key → 401; artifact failing digest/signature
  at the rubixd side → install refused, logged, never executed. rubixd's local
  surface: the fleet-auth bearer (401/410/423 paths).
- **Placement**: rubixd is edge (every managed machine). rartifacts is a normal lb
  node deployment (cloud VM, on-prem; `deploy/common/` container or a rubixd-managed
  systemd install of itself — dedicated node recommended).
- **MCP surface**: `pkg.list | get | resolve | publish | promote | yank |
  set_visibility` on the rartifacts extension (resource-verbs grammar), so the CLI,
  agents, and the federated UI all drive it identically; the rubixd-facing REST is a
  thin host-mounted projection of the same tools. rubixd itself: local REST/CLI only.
- **Data (SurrealDB)**: rartifacts metadata lives in its node's store (workspace
  `fleet`); blobs on disk owned by the native extension (content-addressed — the
  sanctioned native-tier escape hatch; multi-GB archives are not store records).
  rubixd keeps its own tiny embedded ledger.
- **Bus (Zenoh)**: none. Plain HTTPS polling agent→server; simplest thing that works
  offline-tolerant. (Push/bus fan-out is a fleet-control-plane follow-up.)
- **Sync / authority**: rartifacts is authoritative for packages; each rubixd is
  authoritative for its machine's installed state. A machine that cannot reach
  rartifacts keeps running what it has and retries — updates degrade, running services
  do not.
- **Secrets**: local secrets file on the device (root, 0600), referenced by
  `${secret:...}`; publisher/agent tokens in rartifacts hashed at rest, revocable instantly.

## Example flow

1. CI builds `rubix-ai` 0.4.6 for `x86_64` + `aarch64` (the existing `docker/build/`
   cross toolchain), signs each with the release key, `POST /packages` to rartifacts,
   and promotes `stable → 0.4.6`.
2. On a site machine, rubixd's poller re-resolves `rubix-ai@stable`, sees 0.4.6 >
   installed 0.4.5 for both instances.
3. It downloads the `x86_64` blob once, verifies SHA-256 + Ed25519 against
   `trusted_pubkeys`, stages `/opt/rubix/rubix-ai/releases/0.4.6/`.
4. Per instance (serially): stop `rubix-ai-rubix-main.service`, flip `current` →
   0.4.6, start, poll `http://127.0.0.1:8099/health` for up to 30 s.
5. `rubix-main` passes → commit. `rubix-lab` fails its gate → rubixd flips `current`
   back to 0.4.5, starts it, marks `0.4.6` bad **for that instance**, reports the
   failure in `rubixd status` (and, later, upstream). Data dirs were never touched.
6. Operator inspects, clears the bad mark (or a 0.4.7 supersedes it); reconcile retries.

## Testing plan

Per `testing-scope.md` — no mocks, no fake backends:

- **Mandatory categories, translated to this plane**: the isolation test is *ownership*
  (rubixd refuses to stop/remove a unit/container it did not create — real systemd unit
  + real container seeded outside rubixd); the deny test is *trust* (unsigned blob,
  tampered blob, unknown publisher key, bad or revoked agent token — each refused with the right
  status, nothing executed).
- **rartifacts integration**: a real spawned rartifacts node (embedded lb, real store,
  real extension — the lb testing rules apply verbatim, including the capability-deny
  and workspace-isolation tests on the `pkg.*` tools); publish → resolve
  (exact/range/channel) → download → verify roundtrip; re-publish same digest is
  idempotent; range download resumes; anonymous reaches public packages only.
- **rubixd systemd backend**: run against **real user-scope systemd** (`systemd --user`)
  in CI — install v1, verify active; update to v2, verify symlink + active; update to a
  v3 whose binary exits nonzero → assert automatic rollback to v2, unit active, v3
  marked bad. Data-dir file written by v1 still present after every transition.
- **rubixd docker backend**: real local dockerd via bollard — image-archive load,
  create/recreate, health-gate rollback restores the old container, volume survives.
- **Multi-instance**: two instances of one package; colliding `per_instance` key
  rejected at `apply`; instances update independently (one can be rolled back while the
  other commits).
- **Standalone (rartifacts-optional)**: a rubixd configured with **zero remotes** —
  `rubixd publish` (and a raw `POST /packages`) of a signed tiny binary lands in the local
  index; a bundle referencing it installs to `active` with no network reachable at all;
  re-publishing the same digest is idempotent; a tampered/unsigned/foreign-key blob to
  `POST /packages` is refused (422) and **never enters the blob cache**; an unauthenticated
  publish is 401. Resolution-order test: a local version and a remote version present, the
  local one wins (and `remote: local` errors when the package is absent locally).
- **E2E**: rartifacts + rubixd + a real seeded package (a tiny real binary with an HTTP
  health endpoint) — the full example flow above, both backends — **plus** the same flow
  with rartifacts torn down and the package pushed straight to rubixd.
- The only permitted fake: none identified — systemd (user scope), docker, and HTTP are
  all runnable locally. If a CI runner lacks dockerd, the docker suite is *skipped and
  reported*, not faked.

## Risks & hard problems

- **Health checks decide rollbacks** — a weak check (process-alive) commits broken
  releases; a strict one (deep HTTP) flaps on slow boots. Per-package tunable timeout +
  a "startup grace" field; and `rubix-ai` today has **no health endpoint** (needs one —
  open question).
- **Partial bundle failure**: `needs` ordering is easy; *transactional bundles* (roll
  back package A because dependent B failed) are not. v1: per-package transactions +
  ordering only; a bundle reports partial states honestly.
- **rubixd self-update** is the classic bootstrap problem. v1: rubixd updates itself
  last, via the same release-dir + symlink mechanism, with a systemd-watchdog fallback
  to the previous binary if it fails to come up.
- **Arch mismatch**: agent advertises its arch; rartifacts resolves accordingly; the
  agent re-checks the ELF header before install (the ems ARM scope names this guard).
- **Docker archive size** (multi-GB Timescale images) — zstd + ranged downloads + a
  local blob cache keyed by digest; prefer `docker-image` (direct pull) when the
  machine has registry access.
- **The lb posture buys risk along with reuse**: the anonymous api-key leash must be
  provably narrow (a caps test that the anonymous principal reaches *only* the
  public-read path); the minimal shell (`frontend/minimal-shell-scope.md`) is itself
  a scope, not shipped — rartifacts' UI session inherits that dependency; and the
  host binary rides lb git-tag pins (the rubix-ai bump cadence). If these bite, the
  fallback recorded here is the previous standalone-axum design (still in this file's
  history) — the rubixd wire contract is identical either way, by construction.

## Open questions

- Repo naming/home: `rubix-fleet` with three crates is the recommendation — confirm.
- ~~Does `rubix-ai` (and `ems-node`) grow a real `/health` endpoint~~ **decided (v1 gates
  on TCP-connect; `/health` is the ratified convention when they adopt it).** Neither host
  has one today, and the fleet plane does not need them to: bundle health specs already
  support `tcp:<port>` alongside `http:<path>`, so v1 gates on gateway TCP-connect —
  weaker, honest, shipping. [`containerize-scope.md`](containerize-scope.md) §The health
  contract ratifies **`GET /health` → 200 `{"status":"ok",…}` / 503
  `{"status":"degraded",…}`, never `/healthz`, never blocking on a dependency** as the
  fleet-wide convention; `rubixd` and `rartifacts` implement it in their slices, and when
  the product hosts adopt it their bundles switch `tcp:` → `http:` with **no fleet-plane
  change**. Adopting it is those repos' work, not this plane's blocker.
- ~~Auth issuance~~ **decided**: boot-generated admin token with one-time UI claim on
  both services; rartifacts registers agents (revocable per-instance tokens) and mints publisher tokens under it (the
  `token-auth-scope.md` pair). Device enrollment *automation* (vs. hand-placing the
  token file) stays open, cf. `auth-caps/edge-trust-scope.md`.
- Should rartifacts also host **lb extension artifacts** so a bundle can declare
  "publish these extensions into the node after it's healthy"? With rartifacts now an
  lb host this is nearly free — the node already understands the signed `Artifact`
  envelope and could even expose its own registry role. Still scoped out of v1, but
  the bundle schema should leave room (`post_install` hook vs first-class
  `extensions:` list), and the lb-host posture makes the first-class list the likely
  winner.
- Channel semantics: per-package channels only, or bundle-level "release trains"?
- Standalone publish: should a locally-pushed package still **require** a valid signature
  against `trusted_pubkeys` (recommended default — the trust wall holds even off-fleet),
  or is there an opt-in `allow_unsigned_local = true` config knob for dev boxes? Lean
  toward always-verify; the knob is a follow-up if friction is real.
- Should a standalone rubixd be able to **promote channels locally** (`stable → 0.4.6`
  against its own index) so channel-pinned bundles work with no remote, or do local
  publishes only support exact/range pins in v1?
- Windows service backend (rubix-ai cross-builds for windows-x86_64) — later slice;
  `service-manager` already abstracts it, so the door stays open.

## Related

- [`containerize-scope.md`](containerize-scope.md) — official **container images for both
  services** (rartifacts as an **AWS** workload; rubixd for docker-only hosts, where the
  systemd backend typed-degrades and the native unit stays the default for mixed hosts).
  Cross-cutting, lands per-slice; adds no product code.
- `deploy/fly-deploy-scope.md` — the single-container cloud deploy this complements.
- `docker/build/` — the cross-compile toolchain that produces the artifacts.
- `rust/crates/registry/` (`Artifact` v2, `digest.rs`, `verify_artifact`) and
  `rust/crates/host/src/ext/install_dir.rs` — the signing + atomic-install conventions
  reused.
- `docs/scope/extensions/pack-toolchain-publish-scope.md` — the extension publish path
  that stays unchanged (rubixd delivers hosts; nodes install extensions).
- `ems/docs/scope/platform-targets/arm-raspberry-pi-build-scope.md` — the ARM/systemd
  distribution ask this generalizes.
- Consumers: `/home/user/code/rust/rubix-ai` (env contract in `src/boot.rs`,
  `config.example.toml`), `/home/user/code/rust/ems` (`EMS_*` env, `.ems/` state).
