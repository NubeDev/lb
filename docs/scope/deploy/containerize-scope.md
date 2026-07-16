# Deploy scope — containerizing rubixd + rartifacts (images, compose, AWS)

Status: scope (the ask). Parent: [`rubixd-rartifacts-scope.md`](rubixd-rartifacts-scope.md).
Cross-track: applies to both [`rubixd/`](rubixd/README.md) and
[`rartifacts/`](rartifacts/README.md). Promotes to `doc-site/content/public/deploy/` once
shipped.

We want **official container images for both fleet services** so `rartifacts` can run as a
normal cloud workload (AWS ECS/EC2, or any host with a container runtime) and `rubixd` can
run on **docker-only hosts** — machines with a docker daemon and no intent to manage bare
systemd units. Today neither service has a Dockerfile, a `.dockerignore`, a compose file,
or a CI image job; `packaging/` in `rubix-fleet` holds a `.gitkeep`. This scope defines
the **image contract** (build, base, arch matrix, env, volumes, ports, health) for both
binaries, the **compose** files that run them locally, and the **AWS-shaped deployment**
rartifacts targets — landing **per-slice, as each service becomes runnable**, not as one
big-bang container track.

The distinction this doc turns on, because the repo's existing vocabulary collides with
it: **slice 5 (`rubixd/docker-backend-scope.md`) makes rubixd a docker _client_** that
manages workload containers. **This scope makes rubixd and rartifacts _containerized
workloads themselves_.** Orthogonal asks; the repo has neither today, and the interaction
between them (an agent in a container driving the host's daemon) is the hard problem
§Risks names.

## Goals

- **`rartifacts` image — the primary deliverable.** A multi-stage build of the host binary
  + its federated extension UI, running the node's gateway on `0.0.0.0:9410`, with all
  durable state (SurrealDB store + the content-addressed `blobs/` dir) under **one volume
  at `/data`**. This is the AWS workload; it has no host coupling and is the reason the
  scope exists.
- **`rubixd` image — docker-only hosts, honestly degraded.** The same binary, containerized,
  with `/var/run/docker.sock` bind-mounted so the **slice-5 docker backend works fully**.
  The **systemd backend reports `BackendUnavailable`** — a typed, visible plan entry, not a
  crash and not a silent skip. `docs`/`status` state which backends are live in the current
  posture, so an operator never wonders why a `systemd`-kind package won't install.
- **The systemd unit stays the blessed path for mixed hosts.** A machine that installs both
  bare binaries and containers runs rubixd **native**, from `packaging/rubixd.service`
  (slice 1). The image is an *additional* posture for docker-only boxes, never the
  recommended default for an edge machine. The scope ships both and says which is which.
- **One image, many drivers** — the `deploy/common/` reuse mechanism proven by
  [`fly-deploy-scope.md`](fly-deploy-scope.md), applied in `rubix-fleet`: one Dockerfile
  per service, referenced verbatim by local compose, CI, and the AWS driver. A driver may
  only *reference* the common assets, never fork them.
- **The arch matrix mirrors `make cross`.** Images build for `linux/amd64` + `linux/arm64`
  as one manifest list. **`armv7` is images-excluded** — the `rust-toolchain.toml` target
  stays (bare binaries still ship there), but a containerized armv7 box is not a posture we
  support; `docker buildx` armv7 + a RocksDB C++ build is cost without a caller.
- **`bind_addr` gets an env override.** rubixd's default is `127.0.0.1:9420`
  (`config/model.rs:21`) — correct for a native daemon, **useless in a container**. This
  scope adds `RUBIXD_BIND_ADDR` (and the image sets `0.0.0.0:9420`), so containerizing does
  not require mounting a whole config TOML just to change one field.
- **CI proves the images build** on every PR — build-only, no push, matching the
  fly-deploy precedent and its stated reason to revisit.

## Non-goals

- **No AWS control-plane IaC.** No Terraform/CDK/CloudFormation, no ECS task definitions,
  no ALB/Route53 wiring, no account bootstrap. This scope ships an image that *runs*
  cleanly on ECS/EC2 and documents the contract it needs (volume, env, ports, health);
  choosing and codifying the AWS topology is a follow-up once there's a real account and a
  real region. Naming it here is the boundary, not a promise.
- **No Kubernetes, no Helm, no push automation in v1.** Registry is **decided — GHCR**
  (§Decisions); CI **builds and discards** until there is a staging environment to receive
  a push. No `/livez`/`/readyz` either: one `/health` per service, 200/503 (§The health
  contract).
- **No container-first rubixd on mixed hosts.** Reaching host systemd from a container
  (host PID namespace + `/run/systemd/private` or dbus mounts) was **considered and
  rejected** — see §Intent. Mixed hosts run the native unit.
- **Not `rubixd/docker-backend-scope.md`.** That slice's bollard driver, ownership labels,
  and volume-safety rules are *unchanged and assumed* by this doc. This scope does not
  touch backend code.
- **Not lb's `fly-deploy-scope.md`.** That ships one lb *node* to Fly. This ships the two
  *fleet-plane* services. They share the `deploy/common/` **convention** and nothing else —
  different repo, different binaries, no shared files.
- **No `rubixd` self-update via container.** Image-based rubixd updates are `docker pull` +
  recreate by whatever supervises it; the release-dir/symlink self-update path
  (parent scope §Risks) is the native posture's problem and stays there.

## Intent / approach

### Two services, two postures — because the coupling differs

**rartifacts is a clean container.** It is an lb product host: env-driven `BootConfig`,
one gateway port, one state dir. `server-core-scope.md` §Goals already specifies
`RARTIFACTS_GATEWAY_ADDR` **defaulting to `0.0.0.0:9410`** — it was designed
container-ready before anyone wrote a Dockerfile. It touches no host namespace. Its image
is ordinary multi-stage work and carries no architectural tension.

**rubixd is not, and pretending otherwise is the trap.** Its whole job is to drive the
*host* — slice 3 shells host systemd via `service-manager`, slice 5 drives the host's
dockerd via bollard. Containerizing it means an agent inside a container managing the
machine outside it. The two backends do **not** degrade equally:

| Backend | In-container | Why |
|---|---|---|
| **docker** (slice 5) | ✅ **Full function** | `-v /var/run/docker.sock:/var/run/docker.sock` — bollard talks to the host daemon over the socket. Containers it creates are *siblings*, on the host, exactly as in the native posture. Labels, `.prev` rename, volume safety: all unchanged. |
| **systemd** (slice 3) | ❌ **`BackendUnavailable`** | Reaching host systemd needs `--pid=host` + `/run/systemd/private` or a dbus socket mount — the container stops being a boundary and becomes a worse-behaved root shell. Rejected. |

So the posture is **docker-optional, host-native default**:

- **Mixed host** (bare binaries + containers) → **native rubixd**, `packaging/rubixd.service`.
  Both backends live. This is the recommendation and stays the documented default.
- **Docker-only host** (nothing installed but containers) → **rubixd image**, socket
  mounted. Docker backend live; systemd-kind packages typed-refuse.

The **degradation must be legible, not incidental**. `backend/mod.rs` already defines
`BackendError::Unavailable`, and `docker-backend-scope.md` already establishes the
precedent verbatim — "no reachable dockerd → docker-kind transitions become typed
`BackendUnavailable` plan entries (status shows why), never a panic". This scope applies
that same rule with the polarity flipped: **in-container, systemd-kind transitions become
typed `BackendUnavailable`**, `rubixd status` names the posture and the live backends, and
applying a systemd-kind bundle to a containerized rubixd is a **clean typed refusal at
plan time** — never a partial install, never a panic.

Detection is **capability probing, not a container check**. rubixd asks "can I reach
systemd? can I reach dockerd?" and reports what it finds. There is no `if in_container {}`
branch — that would be the fleet-plane's own version of `if cloud {…}`, which lb's rule 1
rejects and which this scope has no reason to introduce. A native rubixd on a host with no
dockerd degrades through the *identical* code path.

**Alternative rejected — container-first everywhere.** One posture, one artifact, thin host
footprint; but it demands host PID + dbus mounts on every edge box to keep slice 3 alive,
which dissolves the isolation that motivated the container and hands a container root-
equivalent host control. The systemd unit is *already* the natural packaging for a
per-machine agent — `service-manager` is a dependency rubixd has anyway. Containerizing to
avoid a systemd unit, in the service whose purpose is writing systemd units, is a circle.

**Socket mount is real privilege — say it plainly.** `/var/run/docker.sock` is root-
equivalent on the host. It is defensible here only because rubixd's *purpose* is managing
the host's containers: it is not an escalation, it is the job description. The image
documents it as such, mounts it **read-write** (bollard needs create/stop/remove), and adds
no other host mount. It is not a default in any compose file an operator might copy
casually — the docker-only compose that includes it is named for that posture.

### Assets — `rubix-fleet:deploy/`, mirroring the proven convention

```
deploy/
  common/
    Dockerfile.rubixd          # multi-stage, static-ish, arch matrix
    Dockerfile.rartifacts      # + the federated ext UI build stage
    entrypoint-rubixd.sh       # ensure dirs → probe backends → exec rubixd
    entrypoint-rartifacts.sh   # ensure /data/{store,blobs} → exec rartifacts
    README.md                  # the env/volume/port contract table
  compose/
    rartifacts.yml             # the server, alone — the AWS-shaped local twin
    rubixd-docker-host.yml     # rubixd + the socket mount (docker-only posture)
    fleet.yml                  # both + a seeded package — the E2E local fleet
.dockerignore                  # ONE file, repo root — see below
```

**The `.dockerignore` lesson is imported, not relearned.** fly-deploy discovered at
implementation time that Docker resolves `.dockerignore` from the **context root**, never
next to the Dockerfile; a `deploy/common/.dockerignore` is **silently ignored** by
`docker compose build` and classic `docker build` (only `buildx --ignorefile` honors it).
So: exactly **one** `.dockerignore` at the `rubix-fleet` repo root, denylist-style
(`target/`, `.git/`, `docs/`, `rartifacts/extensions/rartifacts/ui/node_modules/`). That
finding is written down in lb's own scope history — paying for it twice would be
embarrassing.

### The build stages

**`Dockerfile.rubixd`** — builder on the pinned toolchain → `cargo build --release -p
rubixd` → runtime on `debian:bookworm-slim`. Two build realities the exploration surfaced:

- **RocksDB pulls a C++ toolchain.** The ledger is embedded SurrealDB with
  `kv-rocksdb` (`Cargo.toml`), so the builder stage needs `clang`/`libclang`/`cmake` and
  the build is *slow*. Layer-cache dependencies separately from source (the standard
  cargo-chef-shaped split) or every one-line change rebuilds RocksDB.
- **`panic = "abort"` + `strip = true`** are already set for edge size; the image inherits
  a small binary for free. Runtime stays `-slim` rather than `scratch`/`distroless` —
  RocksDB wants a libc, and slice 5's bollard path benefits from having a shell for
  operator debugging on a box that by definition has no other tooling.

**`Dockerfile.rartifacts`** — same builder shape, plus a **pnpm stage** for the extension's
federated React/Tailwind/shadcn UI, plus the boot self-publish consideration:
`server-core-scope.md` §Risks already says boot self-publish is **dev-mode only** and
"release images bake the published artifact". This scope holds that line: **the image
bakes the pre-published, signed extension artifact**; it does not build-and-sign at boot
inside a container. That is the rule that keeps the runtime image from needing a Rust
toolchain.

### The container contract (both services)

| | `rubixd` | `rartifacts` |
|---|---|---|
| **Port** | `9420` | `9410` |
| **Bind** | `RUBIXD_BIND_ADDR=0.0.0.0:9420` (**new env, this scope**) | `RARTIFACTS_GATEWAY_ADDR=0.0.0.0:9410` (already the default) |
| **Config** | `RUBIXD_CONFIG` → mounted `/etc/rubixd/config.toml` (optional — missing file is a valid standalone posture, by design) | `RARTIFACTS_*` env only (no config file) |
| **State volume** | `/var/lib/rubixd` (RocksDB `state/`, `blobs/`) | `/data` (`RARTIFACTS_HOME`; `store/`, `blobs/`) |
| **Other mounts** | `/etc/rubixd/bundles.d/` (desired state), `/var/run/docker.sock` (**rw, docker-only posture**) | none |
| **Health** | `GET /health` on `:9420`, open (slice 2 — already scoped) | `GET /health` on `:9410`, open (slice 1 — already scoped) |
| **Secrets** | `/etc/rubixd/secrets.toml` (0600) mounted, **never baked** | `RARTIFACTS_SIGNING_KEY` as env/secret, **never baked** |
| **User** | non-root + the `docker` group (socket access) | non-root |

`rubixd`'s `RUBIXD_CONFIG` is currently the **only** env var in the entire codebase.
`RUBIXD_BIND_ADDR` is the one addition this scope justifies — config-file-only binding
makes the trivial container case require a file mount for a single field. The precedent is
lb's own `gateway_signing_key()` seam (fly-deploy §Open questions): a thin, named,
binary-boundary env read, defaulting to the existing behavior, touching no core logic.
Anything beyond bind-addr stays in the TOML — this is not an invitation to env-ify the
whole config.

### The health contract — `/health`, one route, two states

Containers, load balancers, and rubixd's own health gates all need to ask "is this thing
up?" — and they are the *same question asked by different callers*, so they get **one
answer**. The decisions, settled here for both services and every future fleet binary:

**`/health`, never `/healthz`.** The `z` suffix is a Kubernetes/Borg-ism that exists to
avoid collisions in an app's own namespace. We ship no k8s (§Non-goals), no fleet service
has a `/health` collision, and both fleet scopes already say `/health` —
`rubixd/token-auth-scope.md` §Goals lists it as unauthenticated, and
`rartifacts/server-core-scope.md` §Goals mounts it open. `/healthz` appears **nowhere** in
either repo. This is a naming ratification, not a change.

**One route, not three.** No `/livez`, `/readyz`, `/startupz`. The split exists for
orchestrators that take *different actions* per probe (restart vs. de-register vs. wait) —
ECS and an ALB take exactly two actions, and both are covered by the status code. A single
route with a **typed body** carries the nuance without three code paths to keep honest:

```
GET /health  →  200  {"status":"ok",       "version":"0.1.0", "detail":{…}}
             →  503  {"status":"degraded", "version":"0.1.0", "detail":{…}}
```

- **200 = serving.** Take traffic.
- **503 = alive but not serving** (store not open, blob dir unwritable, extension not yet
  published). The process is up and answering — so an orchestrator that restarts on
  *connection failure* correctly does **not** restart it, while an LB that de-registers on
  *non-200* correctly stops sending traffic. That is the liveness/readiness distinction,
  expressed in the status code, with no second route.
- **Connection refused = dead.** Restart it. The absence of an answer is the liveness
  signal — which is why `/health` must never block on a slow dependency: it reports state
  it already knows, it does not go probing. **A health check that can hang is a health
  check that lies.**

**Unauthenticated, on the main port, and cheap.** Open (an LB has no bearer token; both
scopes already carve it out of auth), on the service's one port (no second admin listener
to expose and secure), and it reads in-memory state only — **no store query, no disk I/O,
no network call**. It leaks only `status` + `version`; `detail` names *which* subsystem is
degraded, never a path, DSN, or key. A 503 body says `{"store":"unavailable"}`, not the
store's location.

**Per service:**

| | `rubixd :9420/health` | `rartifacts :9410/health` |
|---|---|---|
| **200 when** | ledger open + backend probe done | node booted, store open, blob dir writable, extension published |
| **503 when** | ledger unopenable | store or blob dir unavailable, extension not yet live |
| **`detail`** | `{"ledger":"ok","backends":{"docker":"available","systemd":"unavailable"}}` | `{"store":"ok","blobs":"ok","ext":"ok"}` |

rubixd's `detail` is where the **posture becomes machine-readable** — the same
backend-availability facts `status` prints, in the response an operator or a probe already
has to make. **Backend unavailability is not degraded**: a docker-only box is *correctly
configured*, running exactly as intended, and must return **200**. Degraded is reserved for
"I cannot do my job at all" — otherwise every docker-only host flaps red forever and the
signal becomes noise the day it matters.

**The product-host gap this does not close.** `rubix-ai` and `ems-node` have **no health
endpoint today** (grepped: zero hits) — which is the umbrella's own open question, because
it is what rollback health gates fire on. That is *those* repos' scope, not this one. The
fleet plane's answer is already correct and unchanged: bundle health specs support
`tcp:<port>` as well as `http:<path>` (parent scope §Package model), so a product host
without `/health` gates on TCP-connect until it grows one — weaker, honest, and not a
blocker. **This scope ratifies `/health` as the convention those hosts should adopt** when
they do, so the fleet plane and the product plane speak one dialect.

### AWS shape — the decided topology

rartifacts on AWS is deliberately boring, and the topology is **decided** — this scope
does not codify it (no IaC, §Non-goals), but it does not leave it as a shrug either:

**EC2 + EBS, single instance, not Fargate.** rartifacts embeds SurrealDB on local disk;
that store wants a **block device**, not a network filesystem. Fargate's only durable
option is EFS (NFS) — and an embedded LSM store on NFS is the classic way to get file-lock
weirdness and corruption under load. So: **one EC2 instance, one gp3 EBS volume mounted at
`/data`, the image run by the host's docker via a systemd unit.** One artifact plane, one
disk, no cluster.

- **Volume — the one thing that must be right.** `/data` holds `store/` *and* `blobs/`,
  both on the **same EBS volume**, never the instance's ephemeral root and never a
  container-scoped anonymous volume. This is the single AWS-shaped failure that silently
  loses every published artifact, and it is a config mistake, not a code bug — which is
  exactly why the testing plan asserts the **negative** (no volume → data gone on restart)
  rather than trusting a README sentence. Blobs are content-addressed and write-once, so
  they would tolerate EFS; the store would not, and splitting them across two volume kinds
  to buy nothing is how you get a backup story with two halves.
- **TLS at the ALB**, plain HTTP to `:9410` behind it. The container never terminates TLS
  (fly-deploy's posture, same reason: the proxy owns certs). ALB target-group health check
  → `GET /health`, matcher `200`, so a 503 de-registers the target without killing it.
- **Secrets Manager → env** for `RARTIFACTS_SIGNING_KEY`, injected by the unit at start,
  **never baked into a layer** and never logged.
- **Backups: EBS snapshots**, daily. The store is the authority for package *metadata*;
  blobs are re-derivable only if the publisher still has them — treat the volume as
  precious. A snapshot restore is the whole DR plan, and it is enough for one artifact
  plane.
- **Scaling: none, deliberately.** One instance. rartifacts serves reads (resolve +
  blob download) that are cheap and cacheable at the ALB/CloudFront if it ever matters;
  multi-AZ HA for an artifact server is a solution to a problem no fleet this size has.
  When it does, the answer is CloudFront in front of the blob routes — **not** a second
  writer against a shared store.

**Fargate/ECR reconsidered and rejected for the runtime**: Fargate forces EFS for state
(above), and ECS's task-definition ceremony buys orchestration we explicitly do not want
(§Non-goals: not an orchestrator — the same principle the parent scope applies to rubixd).
**ECR is still the registry** (below) — the rejection is Fargate as the *compute*, not AWS
as the home.

`compose/rartifacts.yml` is the local twin of exactly this shape — same image, same
`/data`, same env — which is how the topology gets tested on a laptop without an AWS
account, and why a deploy is a boring afternoon instead of a discovery exercise.

## How it fits the core

Most lb platform rows are **N/A** — rubixd is deliberately not an lb node, and this scope
ships **no product code, no MCP verb, no capability, no record, no route**. What applies:

- **Symmetric nodes (rule 1):** upheld, and load-bearing. The container runs the **same
  binary** as the native install, selected into a posture by **config + what it can
  reach** — never a compile-time or runtime `if container {}`. Backend availability is
  probed, and a native rubixd on a dockerd-less host degrades identically.
- **Capabilities / tenancy:** unchanged. rartifacts' image serves the same gateway; every
  request still derives principal + workspace from its token. The image adds **no seam that
  bypasses the wall** — that is the property the testing plan asserts rather than assumes.
- **Ownership wall (rubixd's isolation analog):** unchanged and, notably, **strengthened by
  being tested here** — a socket-mounted rubixd must obey the identical
  `rubixd.managed=true` label check before any destructive verb. A container that
  manages the host's docker is exactly where "it only touches what it created" stops being
  theoretical.
- **Secrets:** signing keys and `secrets.toml` are **mounted or injected, never baked into
  a layer and never logged**. An image layer is not a secret store; fly-deploy's
  signing-key risk is the same lesson.
- **One responsibility per file:** the `deploy/` assets are one file per concern
  (Dockerfile per service, entrypoint per service, compose per posture) — FILE-LAYOUT
  applies to shell and YAML by spirit.
- **Skill doc:** **N/A.** This adds no agent-/API-drivable surface. The operator runbook is
  `deploy/common/README.md` + the `make docker-*` targets — not an MCP tool. (Same call,
  same reason, as fly-deploy.)

## Example flow

1. **Local rartifacts (the AWS twin).** `make docker-rartifacts` builds
   `deploy/common/Dockerfile.rartifacts`; `docker compose -f deploy/compose/rartifacts.yml
   up` runs it with a named volume on `/data`. `GET :9410/health` → 200. Claim the admin
   api-key, `POST /packages` a signed tiny binary, `GET /packages` lists it.
2. **Restart proof.** `docker compose restart` → the package is still listed and its blob
   still downloads. This is the whole AWS volume story, proven locally in seconds.
3. **Docker-only host.** `docker compose -f deploy/compose/rubixd-docker-host.yml up` —
   rubixd with `/var/run/docker.sock` mounted rw and `RUBIXD_BIND_ADDR=0.0.0.0:9420`.
   `rubixd status` reports posture `container`, backends `docker: available, systemd:
   unavailable (no host systemd)`.
4. **Sibling container install.** Apply a bundle with a `docker-image` package
   (`timescale/timescaledb:…`, instance `tsdb-main`). rubixd pulls via bollard through the
   socket; the container appears **on the host** beside rubixd's own, labeled
   `rubixd.managed=true`. Health-gate `tcp:5433` → commit.
5. **The honest refusal.** Apply a bundle with a `systemd`-kind package to the same
   containerized rubixd → **plan shows `BackendUnavailable` with the reason**, nothing is
   installed, exit code for `status` is still 0. No partial state, no panic.
6. **Mixed host, for contrast.** The same bundle on a **native** rubixd (systemd unit,
   no container) installs both packages — both backends live. This is the default
   posture, and step 5 is the price of the other one.

## Testing plan

Per [`testing/testing-scope.md`](../testing/testing-scope.md) — **no mocks, no fake
backends**. This scope ships assets, not product code, so the tests prove *the images build
and boot real services*:

- **Image builds (CI gate).** A `deploy-image` job builds both Dockerfiles on every PR —
  green is the gate, and it is what keeps the assets from rotting. Build-only, no push.
  Mirrors fly-deploy's shipped decision.
- **rartifacts boot smoke (real node, real store).** Run the built image; assert
  `GET /health` 200, claim the admin key **once** (second claim → 410), `POST /packages`
  a real signed tiny binary, `GET /packages` lists it, `GET /blobs/{sha256}` returns it
  byte-identical. Real embedded lb + real SurrealDB in the container — the same discipline
  `server-core-scope.md` mandates.
- **Volume persistence (the AWS-shaped failure).** Restart the container; the package row
  **and** its blob survive. Then the negative: run with **no** volume mounted, publish,
  restart → gone. Asserting the failure mode is what makes the volume requirement real
  rather than a README sentence.
- **Capability-deny + workspace-isolation, through the image.** `server-core-scope.md`
  mandates both against a spawned node; re-assert **at least one of each through the
  containerized gateway** — the point is proving the image introduces no bypass seam. A
  principal without the `pkg.list` grant → 403 from the container exactly as from a local
  node.
- **rubixd in-container, docker backend (real dockerd via the socket).** The **socket-
  mounted** posture running the *same* transaction suite slice 5 defines: image pull,
  create/start, health gate, update → `.prev` kept → commit, and **health-gate rollback
  restores the old container**. Skips-and-reports when no dockerd, never fakes.
- **Ownership deny through the socket (the isolation test that matters most here).** Seed
  a hand-run, **unlabeled** container named like a target on the host; the containerized
  rubixd must refuse every destructive verb with `ForeignContainer`. A socket-mounted agent
  is precisely where a broken ownership wall would be catastrophic — this is not a
  duplicate of slice 5's test, it is that test in the posture that can do the most damage.
- **Volume safety through the socket.** Remove an instance → container gone, **volume
  present**, `status` lists it orphaned. rubixd never removes a volume — posture-invariant.
- **Backend degradation is typed, not accidental.** In-container: a systemd-kind
  transition yields `BackendUnavailable` in the plan (not a panic, not a silent skip);
  `status` names the live backends. **The symmetry test:** a *native* rubixd on a host with
  no dockerd degrades through the identical path — proving posture is probed, not branched
  (rule 1).
- **`RUBIXD_BIND_ADDR`.** Set → binds `0.0.0.0` and is reachable from outside the
  container. **Unset → the `127.0.0.1:9420` default is unchanged** (the native posture
  cannot regress). Malformed → typed config error at boot, not a panic.
- **Multi-arch.** The manifest list resolves for `linux/amd64` + `linux/arm64`; an
  `arm64` image actually boots and answers `/health` (emulated is acceptable in CI — a
  built-but-never-run image is not proof).
- **Secrets never in the image.** Assert no signing key material in any layer
  (`docker history` / a layer grep) — cheap, and it is the mistake that is invisible until
  it is public.

Debug entries land under `debugging/deploy/` on first bug. Expected failure modes, from
the fly-deploy precedent: build-context bloat, volume-mount misses, and entrypoint
ordering.

## Risks & hard problems

- **The socket mount is root-equivalent on the host.** Mounting `/var/run/docker.sock`
  gives the container full control of the host's docker — a container escape in rubixd is
  a host compromise. Justified only because it *is* rubixd's job, and only for the
  docker-only posture. Mitigation: it is never a copy-paste default (the compose file is
  named for its posture and says so), no other host mount is added, the ownership wall is
  tested through the socket specifically, and the native unit remains the recommendation
  for anything mixed.
- **The posture split is a documentation problem more than a code problem.** Two ways to
  run one agent, with different capability sets, is exactly how an operator ends up
  wondering why a `systemd` package "silently didn't install". Mitigation: the refusal is
  **typed and surfaced in `status` and in the plan**, not a log line — and `status` names
  the posture unprompted. If this still confuses people in practice, that is a signal the
  image should be *rartifacts-only* and rubixd should stay native, and this scope should
  be reopened rather than papered over.
- **RocksDB in the builder is slow and fat.** A C++ toolchain (`clang`, `cmake`) in the
  builder stage, and a naive Dockerfile rebuilds it on every source change. Mitigation:
  dependency/source layer split; watch it in CI, and if image build time becomes the PR
  bottleneck, `kv-mem` is **not** an acceptable shortcut for a real deployment (the
  parent scope's ledger durability is the point).
- **Nothing runs yet — this scope is ahead of the code.** rubixd's binary prints a version
  string (`main.rs`, 9 lines); its REST server is slice 2; `src/cli` **does not exist on
  disk** though `lib.rs` declares `pub mod cli;`. rartifacts is ~40 lines of doc comments
  with its **lb-node git dep commented out** and is workspace-excluded. An image built
  today packages a no-op. Mitigation: this is *why* the scope lands in waves (§Decisions →
  landing order) — the images arrive when there is something to containerize.
- **Build reproducibility gaps, today.** `Cargo.lock` is **untracked in git** and stale
  (68 packages; no tokio/axum/surrealdb/clap), and `rust-toolchain.toml` floats on
  `stable`. A container build is where "it worked on my machine" turns into a shipped
  artifact that cannot be rebuilt. **Not a risk to accept — wave-1 work owned by this
  scope** (§Decisions → prerequisites).
- **armv7 exclusion may bite.** If a real armv7 site ever wants the docker posture, the
  RocksDB cross-build under buildx emulation is the wall we deferred. Accepted knowingly:
  the bare-binary path serves armv7 and is the posture those boxes want anyway.
- **Keeping the drivers honest** (the fly-deploy lesson, verbatim): the value is "the same
  image everywhere". If compose or CI quietly forks a Dockerfile, the guarantee is gone.
  Mitigation: drivers may only *reference* `deploy/common/`; CI builds those exact files,
  so drift is visible.

## Decisions (no open questions)

Every question this scope raised is **decided below**. Each records the call, the reason,
and what would have to change to reopen it — a decision with a stated trigger is a decision;
one without is a deferral wearing a costume.

- **Landing order — decided: per-slice, three waves.**
  1. **Now**: the repo-root `.dockerignore`, the toolchain pin, `Cargo.lock` committed, and
     `RUBIXD_BIND_ADDR`. Cheap, unblock everything, carry no image.
  2. **rartifacts slice 1**: the rartifacts image. That slice is the first moment there is
     a `/health` and a store worth persisting — and it is the AWS workload that motivates
     this scope.
  3. **rubixd slice 5**: the rubixd image. Before the docker backend exists, a containerized
     rubixd has **zero live backends** and is a no-op with a socket mount — all risk, no
     function.
  *Reopen if*: rartifacts slice 1 slips badly and an AWS deadline lands first — then the
  image ships against a stub host, and the smoke test shrinks to `/health` only.

- **Prerequisites — decided: they belong to this scope, and land in wave 1.** Committing
  `Cargo.lock` and pinning `rust-toolchain.toml` are one-liners that only *this* scope
  needs (a native `cargo build` tolerates both gaps; a reproducible image does not), so
  making slice 1 own them would be borrowing someone else's blocker. **Exception**: the
  `lib.rs` `pub mod cli;` vs. missing `src/cli` mismatch is **slice 1's** — it is that
  slice's own deliverable and the workspace not compiling is its bug, not the container's.
  Wave 1 is therefore: commit the lock, pin the toolchain, `.dockerignore`,
  `RUBIXD_BIND_ADDR`.

- **Toolchain pin — decided: pin to an explicit stable version**, bumped deliberately.
  Floating `stable` means the image you rebuild in six months is not the image you shipped,
  and a rustc regression becomes an unattributable CI failure. The pin is one line and the
  bump is a one-line PR. `rust-toolchain.toml` keeps its full target list — the pin
  constrains the *version*, not the arch matrix.

- **Registry — decided: GHCR, and CI stays build-only in v1.** GHCR sits next to the repo,
  needs no cloud account to be useful, and authenticates with the token CI already has.
  ECR's advantage is IAM-native pulls from AWS — real, but it buys latency and egress
  savings the fleet's pull volume cannot notice, at the cost of a second registry to keep
  in sync. The **EC2 host pulls from GHCR** with a read token; that is a `docker login` in
  the instance's user-data, not an architecture. CI **builds and discards** — matching
  fly-deploy's shipped call, because a push pipeline with no staging environment to receive
  it is ceremony. *Reopen when*: there is a staging environment (then CI pushes on tag), or
  AWS egress shows up on a bill (then ECR becomes a pull-through cache, not a second home).

- **`/health` — decided: one unauthenticated route per service, `/health`, never
  `/healthz`, 200/503 as above.** See §The health contract for the full rationale. Two
  clarifications this resolves: rubixd's slice 2 **already scoped `GET /health`** as an
  unauthenticated route (its §Goals and testing plan both name it) — my earlier framing that
  it was missing was wrong; the gap fly-deploy hit was **lb's gateway**, a different binary
  in a different repo. What slice 2 gains from this scope is the **body contract** (typed
  status + version + detail) and the rule that it must **never block on a dependency**.
  rartifacts' slice 1 §Goals already mounts `GET /health` open; it gains the same body
  contract. Both amendments are one line each and are recorded in those scopes.

- **Product-host health (`rubix-ai`, `ems-node`) — decided: out of scope here, and not a
  blocker.** They have no `/health` today. The fleet plane already handles this correctly:
  bundle health specs support `tcp:<port>` alongside `http:<path>`, so a host without
  `/health` gates on TCP-connect — weaker, honest, and shipping. This scope **ratifies
  `/health` + the 200/503 body as the convention they should adopt**, so when they grow one
  the fleet plane needs no change. Adopting it is those repos' work; the umbrella's open
  question on this stays theirs, and this scope removes any need for it to block containers.

- **Does the rubixd image ship at all — decided: yes, but in wave 3, and it is severable.**
  The honest case against it: if every real host is mixed, it serves nobody and the posture
  split is pure cost. The case for shipping it: docker-only hosts are a real and growing
  posture, the image is ~30 lines of Dockerfile once rartifacts' exists, and the socket-
  mounted **ownership-wall test** (§Testing plan) is worth having regardless — it is the
  configuration where a broken wall does the most damage. It lands **last**, so if a real
  docker-only host never materializes, wave 3 is simply never run and **nothing else in this
  scope is invalidated**. That severability is deliberate, and it is the compromise between
  "ship it" and "you'll never use it".

- **Base image — decided: `debian:bookworm-slim`** for both runtimes. RocksDB wants a libc,
  so `scratch` is out; between slim and distroless, the deciding argument is that **a
  docker-only host has no other tooling** — when rubixd misbehaves at 2am, `docker exec`
  into a shell is the entire debugging story, and distroless deletes it to save ~30 MB on a
  server workload where nobody is counting. *Reopen if*: a security review demands the
  smaller surface, in which case distroless + a debug-tagged sibling image is the shape.

- **armv7 images — decided: never.** Not "later". The bare-binary path serves armv7 and is
  the posture those boxes want; `rust-toolchain.toml` keeps the target for exactly that.
  Building RocksDB under buildx emulation for a host class that does not want containers is
  cost with no caller. *Reopen if*: a real armv7 site asks for the docker posture — and the
  first question back is why that box runs containers at all.

- **Multi-arch mechanism — decided: `docker buildx` manifest lists**, `linux/amd64` +
  `linux/arm64`, native runners where CI has them and emulation where it does not. The CI
  gate requires the `arm64` image to **actually boot and answer `/health`** — a built-but-
  never-run image is not proof, and emulated-boot is cheap enough that there is no excuse.

## Related

- [`rubixd-rartifacts-scope.md`](rubixd-rartifacts-scope.md) — the umbrella (package model,
  postures, the `deploy/common/` container mention under §How it fits the core → Placement).
- [`fly-deploy-scope.md`](fly-deploy-scope.md) — the **precedent this reuses**: one image /
  many drivers, the repo-root `.dockerignore` finding, build-only CI, the signing-key env
  seam. Different repo and different binaries — convention shared, files not.
- [`rubixd/docker-backend-scope.md`](rubixd/docker-backend-scope.md) — slice 5: rubixd as a
  docker *client*. Assumed and unchanged by this scope; the **inverse** of it.
- [`rubixd/systemd-backend-scope.md`](rubixd/systemd-backend-scope.md) — the backend that
  goes `BackendUnavailable` in-container; the reason the native unit stays default.
- [`rubixd/agent-core-scope.md`](rubixd/agent-core-scope.md) — owns `packaging/rubixd.service`
  (slice 1), config load, and the `Backend` trait seam this scope's probing rides.
- [`rubixd/token-auth-scope.md`](rubixd/token-auth-scope.md) — the REST server whose
  `bind_addr` this scope env-overrides; the `/health` open question lands here.
- [`rartifacts/server-core-scope.md`](rartifacts/server-core-scope.md) — `RARTIFACTS_*` env
  contract, the `0.0.0.0:9410` default, the blob dir, and the "release images bake the
  published artifact" rule this scope holds.
- `lb docker/build/` — the existing cross-compile toolchain (`build.sh`, `Dockerfile`) that
  already emits the multi-arch matrix; the reference for the builder stage.
- Assets (to be created, in `rubix-fleet`): `deploy/common/`, `deploy/compose/`, the
  repo-root `.dockerignore`, `Makefile` `docker-*` targets.
- Skill: **N/A** — no agent-/API-drivable surface. Runbook = `deploy/common/README.md`.
