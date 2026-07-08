# Extensions scope — hermetic devkit builds in a container

Status: shipped. Promoted to `public/extensions/dev-flow.md`. Session:
`sessions/extensions/devkit-container-build-session.md`. Open follow-ups (Docker-only v1, shared
not per-workspace cache, no published pinned image tag) tracked there.

The **Extension Studio** "Build" button (and the `devkit.build` MCP verb behind it) shells
out to `cargo`/`pnpm` **as a bare child of the gateway node process**, inheriting whatever
toolchain, network, and git-credential environment that process happens to have. That is
non-hermetic and it fails in practice: the node inherits VS Code's `GIT_ASKPASS`, so when
`cargo` fetches a **private** git dependency (`control-engine`'s `NubeIO/ce-client-rust`)
the checkout can't authenticate and the build dies with `exit status: 101` right after the
crate downloads — even though the same `cargo build` succeeds from the user's shell. We want
devkit builds to run inside a **pinned build container** with an explicit, build-scoped
credential mount, so a build produces the same result on any machine (and on a cloud node
that has no host toolchain at all) instead of depending on the operator's shell.

## Goals

- `devkit.build` runs in a **hermetic container**: pinned Rust + `wasm32-wasip2`, pinned
  pnpm/node, no dependence on the node process's inherited `PATH`/`GIT_ASKPASS`/`CARGO_HOME`.
- **Private git deps build.** A build-scoped GitHub token is injected into the build (mount,
  not baked into the image, not logged) so private crates like `ce-client-rust` resolve.
- **Same live-log UX.** The Studio page keeps streaming `cargo`/`vite` output over
  `devkit/build/<job_id>`; only the executor changes, not the job/log/publish contract.
- **Reproducible off the operator's machine** — including cloud nodes with no host `cargo`.
- **Opt-in, config-selected.** A node with no container runtime keeps working via the
  existing in-process toolchain; container mode is chosen by config, never a code branch.

## Non-goals

- Not sandboxing the *running* extension — this is about **building** it. Runtime tiers
  (WASM, native supervisor) are unchanged (`native-tier-scope.md`).
- Not a remote/distributed build farm. One local container per build on the node that owns
  the request; no build-cluster scheduling.
- Not changing the publish path (`ext.publish`, server-side signing) or the
  `LB_DEVKIT_ROOT` resolution/traversal rules — those stay exactly as in `ext-sdk-scope.md`.
- Not baking extensions into images (`docker/README.md`: extensions are mounted/fetched,
  not per-image).

## Intent / approach

The `Toolchain` trait (`rust/crates/devkit/src/toolchain.rs`) is **already the seam**:
`build_extension` calls `toolchain.run(cwd, program, args, log)` and streams lines back.
Today the only impl is `ProcessToolchain` (spawns `cargo`/`pnpm` on the host). Add a second
impl, `ContainerToolchain`, that runs the same `cargo`/`pnpm` commands **inside a pinned
build image**, mounting the resolved `LB_DEVKIT_ROOT` subtree, a persistent cargo/pnpm cache
volume, and a build-scoped git credential — streaming stdout/stderr back through the same
`log` callback so the bus/Studio UX is byte-for-byte unchanged.

Selection is **config, not a branch** (README §3 rule 1): the node resolves which toolchain
to hand `build_extension` from role config (`devkit.builder = "process" | "container"`),
defaulting to `process` where no runtime is present. `build_extension`, the `devkit.build`
verb, the job record, and the log subject don't know which executor ran — they compose over
the trait.

The build image is **one image** in `docker/build/` (a build persona of the existing single
`node` image story is *not* appropriate here — this is a toolchain image, not a node role),
pinning the Rust toolchain + `wasm32-wasip2` target + pnpm to match what CI uses, so a
Studio build and a CI build agree.

**Alternative rejected — fix the env in place** (scrub `GIT_ASKPASS`, force `CARGO_HOME`,
inject the token into the node's process env). It papers over the specific symptom without
making builds reproducible: the node still depends on the host having the right `cargo`,
the right targets, and network reachability, and it silently breaks again on the next
inherited-env surprise or on a cloud node with no toolchain. The container is the actual
hermetic boundary; the env-scrub is a patch we'd pay for twice.

**Alternative rejected — always containerize** (drop `ProcessToolchain`). Local dev on a
box with `cargo` already set up shouldn't require a container runtime just to build an
extension, and the in-process path is the fast inner loop. Keep both behind the trait.

## How it fits the core

- **Capabilities:** unchanged gate — `mcp:devkit.build:call` still authorizes the build;
  the **build-scoped git token** is a *secret* mediated by the host (`lb-secrets`), fetched
  server-side and mounted into the container, **never** returned to the page, never in the
  streamed log, never baked into the image. A node without the secret configured runs
  builds that need no private dep; a private-dep build without it fails with a clear
  "missing build credential" line (not a raw git 401 with a token in the URL).
- **Placement:** **either**, but container mode requires a container runtime on the node —
  so a cloud/hub node (which has one) can offer hermetic builds while a headless appliance
  without a runtime falls back to `process` or declines `devkit.build`. Same binary, chosen
  by config (README §3 rule 1). `devkit.*` remains a local-developer surface, not exposed
  cloud-side by default (`ext-sdk-scope.md`).
- **MCP surface:** **no new verbs.** `devkit.build` keeps its shape — returns
  `{job_id, log_subject}`, the build is a **job** (it runs long: `cargo` from cold cache is
  minutes), progress rides `devkit/build/<job_id>`. This is the §6.1 "batch that can run
  long MUST be a job" contract, already satisfied; we reuse it verbatim.
- **Data (SurrealDB):** unchanged — the same `devkit-build` job record
  (`rust/crates/host/src/devkit/build.rs`); no new tables. The cargo/pnpm **cache volume**
  is build scratch on disk, not platform state (not in SurrealDB — it's motion-adjacent
  build artifact, safe to wipe).
- **Bus (Zenoh):** unchanged — JSON string log lines on `ext/devkit/build/<job_id>`,
  fire-and-forget, exactly as today (`ext-sdk-scope.md` "Implementation decisions").
- **Secrets:** the git build token via `lb-secrets` custody — host reads it, injects it as
  a mounted credential file / `git credential` helper inside the container, and it is
  excluded from the log stream by construction (we never put it in a URL cargo would echo).
- **Symmetric nodes / one datastore / stateless extensions:** no `if cloud` branch (config
  selects the toolchain); no new persistence; the devkit service still holds nothing durable
  beyond the job record.

## Example flow (Studio build of `control-engine`, container mode)

1. Node is configured `devkit.builder = "container"` and has the `ce-client-rust` build
   token in `lb-secrets`. User opens **Extension Studio**, picks the `control-engine`
   folder, clicks **Build**.
2. Page calls `devkit.build {path:"…/control-engine"}` (gated `mcp:devkit.build:call`; path
   validated under `LB_DEVKIT_ROOT`). The service enqueues the `devkit-build` job, returns
   `{job_id, log_subject:"devkit/build/<job_id>"}` — **unchanged**.
3. The job hands `build_extension` a `ContainerToolchain`. It `inspect`s the extension
   (`tier = native`), then runs `cargo build --release` **inside the pinned build image**,
   with: the extension subtree mounted read-write, a persistent cargo cache volume mounted,
   and the git token injected as a credential helper so `https://github.com/NubeIO/…`
   resolves.
4. Page opens `GET /bus/stream?subject=devkit/build/<job_id>` and shows the live `cargo`
   log — same as today. The private-dep checkout **succeeds** because the credential is
   present; the token never appears in a log line.
5. Build finishes → job `done`. User clicks **Publish** → existing `ext.publish` path
   (server-side signing from `LB_DIR/keys`), install, live. No restart. **Unchanged.**
6. **Fallback path:** on a node configured `process` (or with no runtime), step 3 runs the
   existing `ProcessToolchain` on the host. Same job, same log subject, same outcome for a
   build that needs no private credential.
7. **Deny / failure paths:** no `mcp:devkit.build:call` → `Denied`, no container starts.
   Container mode selected but no runtime present → the build job fails with a clear
   "container runtime unavailable" line, not a cryptic spawn error. Private dep but no build
   token → fails with "missing build credential for <host>", not a raw 401.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), against the **real** store / bus /
gateway + the **real** toolchain (`cargo`/`pnpm`/the container runtime are the sanctioned
externals behind the `Toolchain` trait, §0 — no mocks, no fake build):

- **Capability deny (mandatory):** `devkit.build` without the grant → `Denied`, **no
  container spawns and no host process spawns**. A `path` outside `LB_DEVKIT_ROOT` (incl.
  traversal/symlink) → refused before any executor, in both `process` and `container` mode.
- **Workspace isolation (mandatory):** the publish a container build produces lands in the
  caller's workspace only; ws-B cannot see ws-A's built/published catalog entry (reuse the
  registry isolation test) — the executor swap must not widen the workspace wall.
- **Toolchain-parity (integration):** the **same** extension built via `ProcessToolchain`
  and via `ContainerToolchain` yields an installable artifact and a `done` job with
  equivalent log structure — proves the trait swap is behavior-preserving.
- **Private-dep credential (integration, real GitHub is the one sanctioned external):**
  a container build of an extension with a private git dep **succeeds with** the mounted
  build token and **fails cleanly without** it — and the streamed log **never contains the
  token** (assert on the captured log bytes). This is the regression test for the
  `control-engine` `exit 101` symptom; log the original under
  `docs/debugging/extensions/`.
- **Fallback selection (unit):** config `builder=process|container` selects the right
  `Toolchain`; container mode with no runtime present fails the job with the clear message,
  not a panic.

## Risks & hard problems

- **Credential leakage into logs.** The whole point is builds that need a secret; the whole
  risk is that secret hitting the streamed bus log. Inject via a credential *helper* / file,
  never a tokenized remote URL (git echoes those). The parity test asserts on log bytes.
- **Cache correctness across builds.** A shared cargo/pnpm cache volume speeds builds but
  must not let one workspace's build see another's private source. Cache is registry/target
  scratch, not source; keep the mounted **source** per-build and per-`LB_DEVKIT_ROOT`.
- **Runtime detection & clear failure.** "Container runtime unavailable" must be a
  first-class, legible failure line — the current failure is a truncated `exit 101` that
  told the operator nothing. Do not repeat that.
- **Log ordering.** `ProcessToolchain::run` today drains **all** stdout then **all** stderr,
  so a failing `cargo` (errors on stderr) shows its real error *after* the whole stdout dump
  — which is why the Studio log looked like it stopped mid-download. Interleave stdout/stderr
  in the container executor (and fix `ProcessToolchain` to match) so the failing line is
  visible where it happened.

## Open questions

- **Runtime target:** Docker only for v1, or abstract the runtime (Docker/Podman) behind a
  small trait now? Recommendation: Docker CLI for v1, one named file, leave room for Podman.
- **Cache volume scope:** one shared cache volume per node, or one per workspace? Shared is
  faster; per-workspace is a stronger wall. Recommendation: shared **cache** (registry/target
  only), per-build **source** mount — decide if that split is sufficient isolation.
- **Where the build token lives:** a dedicated `lb-secrets` entry keyed by git host
  (`build.git.github.com`), or reuse the existing GitHub-bridge credential? Recommendation:
  a distinct, build-scoped secret so a build token isn't the same grant as the app's GitHub
  access.
- **Image distribution:** build the toolchain image locally from `docker/build/`, or publish
  a pinned tag? Recommendation: `docker/build/Dockerfile` + a pinned tag referenced by config.

## Related

- `docs/scope/extensions/ext-sdk-scope.md` — the devkit generate→build→publish flow and the
  `Toolchain` trait this extends; the `LB_DEVKIT_ROOT` / capability / publish contract we
  reuse unchanged.
- `doc-site/content/public/extensions/dev-flow.mdx` — the shipped Studio dev flow (build/publish UX).
- `rust/extensions/control-engine/docs/control-engine-scope.md` — the private-dep extension
  (`NubeIO/ce-client-rust`) whose build failure motivated this scope.
- `docker/README.md` — one-image / config-not-branches conventions; this adds a **toolchain**
  image under `docker/build/`, distinct from the `node` role image.
- `docs/scope/secrets/` (`lb-secrets`) — custody for the build-scoped git token.
- README `§6.3` (native tier / supervisor), `§6.10` (jobs — the build is a job).
