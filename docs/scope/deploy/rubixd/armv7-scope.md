# rubixd scope — armv7 (the Raspberry-Pi-class edge target)

Status: scope (the ask). Cross-cutting on [`README.md`](README.md) (not a numbered slice —
it constrains every slice); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md). Sibling:
[`../containerize-scope.md`](../containerize-scope.md) (which **excludes** armv7 from
images — this scope is why that exclusion is safe).

`rubixd` must run on **32-bit ARM hard-float** boxes (Raspberry Pi class). The parent
scope's premise is an agent that "must stay tiny on armv7 boxes and is the thing that
installs lb hosts" — so armv7 is not an afterthought target, it is the *hardest* one and
the reason rubixd is lb-free at all.

**Read "tiny" as architectural, not as bytes.** It means *not a full lb node* — no
SurrealDB-plus-Zenoh-plus-wasmtime stack on a box rubixd must itself bootstrap (the
umbrella's rejected "rubixd-also-on-lb" alternative). It does **not** mean a byte budget:
the target boxes have **~8 GB**, and the binary is ~26 MB (~0.3%). Size is measured and
recorded here as a fact, never as a constraint to design against.

`make cross` and `rust-toolchain.toml` already **name** `armv7-unknown-linux-gnueabihf`.
This scope makes it **true**: today that target does not build, for three stacked reasons,
all now solved and verified.

## Goals

- **`make cross` produces a working armv7 binary** — the target it has always advertised.
  Verified end to end: a real `ELF 32-bit LSB pie executable, ARM, EABI5 … stripped`.
- **One reproducible cross image** (`deploy/common/Dockerfile.cross`), extending lb's
  proven `docker/build/Dockerfile` — real Debian GCC cross-toolchains, never zig, never
  the `cross` tool. Adds the two things lb's image lacks because lb's node has no RocksDB:
  **`libclang`** (bindgen) and the **armv7 uint128 fix** (below).
- **The armv7 uint128 fix, recorded as a first-class build input**, not a folk remedy:
  `-UHAVE_UINT128_EXTENSION` for the armv7 target only.
- **Runtime verification on real hardware or an emulating runner** — the cross-build proves
  the ELF, not the behaviour. This scope is explicit that a compiled armv7 binary is
  **not** a passed test.
- **The armv7 deployment contract documented** — the `libstdc++` runtime dep, the glibc
  floor, and what a Pi actually needs installed. See `rubix-fleet:docs/DEPLOY.md`. (Binary
  size is recorded there as a fact, not a constraint: the boxes have ~8 GB.)

## Non-goals

- **No armv7 container image.** Ratified from [`../containerize-scope.md`](../containerize-scope.md)
  §Decisions ("armv7 images — decided: never"): the bare-binary path serves armv7 and is
  the posture those boxes want. This scope is what makes that decision *honest* — armv7 is
  fully supported, just not containerized.
- **No armv6 / 32-bit soft-float / musl.** `gnueabihf` (hard-float) only — the Pi-class
  target lb's image already names. Pi Zero/1 (armv6) are out; if one ever matters it is a
  new target, not a tweak.
- **No swapping the store to dodge the problem.** `kv-rocksdb` stays. `kv-mem` is not a
  deployment posture (the ledger's durability is the point), and switching stores to make a
  cross-build easier would be the tail wagging the dog. The RocksDB cost is **accepted and
  quantified** below.
- **No static/musl linking in v1.** Dynamic against the Pi's own glibc + libstdc++, which
  Raspberry Pi OS ships. Named as a reopen trigger, not a v1 ask.

## Intent / approach

### The three stacked failures (each verified, 2026-07-16)

`make cross` fails on armv7 today. Not "might" — it does, and the reasons stack, so fixing
one only reveals the next. That is why this needs a scope rather than a one-line PR:

| # | Failure | Cause | Fix |
|---|---|---|---|
| 1 | `error occurred in cc-rs: failed to find tool "arm-linux-gnueabihf-gcc"` (dies in `psm`) | No C cross-compiler on the box. `make cross` is a bare `cargo build --target`, which assumes a linker+cc nobody installs. | Build inside lb's cross image (real Debian GCC cross-toolchains + the `CC_`/`CXX_`/`AR_`/`CARGO_TARGET_*_LINKER` env matrix it already sets). |
| 2 | `Unable to find libclang` (dies in `surrealdb-librocksdb-sys`) | RocksDB's build runs **bindgen**, which needs libclang. lb's image omits it — lb's node has no RocksDB, so it never needed it. | `clang libclang-dev llvm-dev` + `LIBCLANG_PATH` in the fleet image. |
| 3 | `'__uint128_t' was not declared in this scope` (`rocksdb/util/fastrange.h:62`) | **The real one.** RocksDB's build autodetects `HAVE_UINT128_EXTENSION` on the **64-bit host**, then compiles for a **32-bit target** where `__uint128_t` does not exist. A textbook cross-compile bug: host-detected, target-compiled. | `-UHAVE_UINT128_EXTENSION` via `CXXFLAGS_armv7_unknown_linux_gnueabihf` / `CFLAGS_…`, **armv7 only**. |

Failure 3 is the interesting one and the reason this is not obvious: it is invisible on
x86_64 and aarch64 (both 64-bit — `__uint128_t` exists, the autodetect is correct), so the
matrix looks fine until the one 32-bit target compiles RocksDB's C++ and fails 200 lines
deep in a header. **Verified**: with the flag, armv7 builds in **2m52s**; aarch64 and
x86_64 need no flag and are unaffected.

### The image: extend lb's, don't fork it

lb's `docker/build/Dockerfile` is already the right thing and already names armv7 as *"the
Raspberry-Pi-class edge target"*. It brings genuine `gcc-arm-linux-gnueabihf` +
`g++-arm-linux-gnueabihf`, the rustup target, and — critically — the per-target
`CC_`/`CXX_`/`AR_`/`CARGO_TARGET_*_LINKER` env block that the `cc` crate and the four
RocksDB sys-deps (`bzip2-sys`, `lz4-sys`, `zstd-sys`, `librocksdb-sys`) all read. The
parent scope already cites it as the toolchain rubixd reuses.

So `rubix-fleet:deploy/common/Dockerfile.cross` is **`FROM` lb's image plus two things**:

```dockerfile
FROM ghcr.io/nubedev/lb-cross:<lb-tag>     # or built from lb/docker/build/
# rubixd embeds SurrealDB with kv-rocksdb; librocksdb-sys runs bindgen (needs libclang).
# lb's node has no RocksDB, so its image omits this.
RUN apt-get update && apt-get install -y --no-install-recommends \
      clang libclang-dev llvm-dev && rm -rf /var/lib/apt/lists/*
ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib
# RocksDB autodetects __uint128_t on the 64-bit HOST, then compiles for a 32-bit TARGET
# where it does not exist (rocksdb/util/fastrange.h). armv7 only — the 64-bit targets are correct.
ENV CXXFLAGS_armv7_unknown_linux_gnueabihf="-UHAVE_UINT128_EXTENSION" \
    CFLAGS_armv7_unknown_linux_gnueabihf="-UHAVE_UINT128_EXTENSION"
```

**Alternative rejected — the `cross` tool.** It would solve failure 1 alone, adds a second
toolchain doctrine next to lb's, and leaves failures 2 and 3 for us anyway. Reusing lb's
image means one cross story across both repos, and the parent scope already committed to
it. **Alternative rejected — install cross-gcc on dev boxes.** Then "works on my machine"
becomes the release process, which is the thing the container scope's prerequisites exist
to kill.

**Note the env-var mechanism, not just the flags**: `CXXFLAGS_<target-with-underscores>`
is how `cc-rs` scopes flags per target. Setting bare `CXXFLAGS` would apply
`-UHAVE_UINT128_EXTENSION` to **every** target and silently pessimise the 64-bit builds
(RocksDB falls back to slower 64-bit math). The underscore form is load-bearing.

### `make cross` becomes container-driven

`make cross` today is `for t in $(TARGETS); do cargo build --release -p rubixd --target $$t; done`
— which cannot work for armv7 on a box without cross-gcc, i.e. every box. It becomes a thin
driver over the image (the `deploy/common/` reuse rule from
[`../containerize-scope.md`](../containerize-scope.md): drivers reference, never fork):

- `make cross` — all three targets in the image.
- `make cross-armv7` — one target, for the iteration loop.
- `make cross-native` — the current bare-cargo path, for a fast x86_64 inner loop.

The container-driven build is the **only** supported way to produce a release artifact for
any target, so CI and a laptop produce the same bytes.

## How it fits the core

Mostly N/A — rubixd is deliberately not an lb node, and this scope ships **no product
code**: it is toolchain + build inputs + docs. What applies:

- **Symmetric nodes (rule 1):** upheld and load-bearing. armv7 runs the **same source**
  with **no `#[cfg(target_arch)]` anywhere** — the arch difference lives entirely in
  *build inputs* (one C flag), never in a code branch. If armv7 support ever needs a
  runtime `if arm {}`, that is a finding, not a fix.
- **Arch honesty (the parent scope's guard):** the umbrella already requires the agent to
  advertise its arch, rartifacts to resolve accordingly, and the agent to **re-check the
  ELF header before install**. This scope is the other half — rubixd must *be* installable
  on the arch it claims. An advertised-but-unbuildable target is the same class of lie as a
  wrong ELF check.
- **Skill doc:** **N/A** — no agent-/API-drivable surface. The runbook is
  `rubix-fleet:docs/DEPLOY.md` + the `make cross-*` targets.

## Example flow

1. **Build.** `make cross-armv7` → the fleet cross image builds
   `target/armv7-unknown-linux-gnueabihf/release/rubixd`.
2. **Verify the shape.** `file` reports `ELF 32-bit LSB pie executable, ARM, EABI5 …
   dynamically linked, interpreter /lib/ld-linux-armhf.so.3 … stripped`. **This is not yet
   a pass** — it proves the compiler, not the program.
3. **Verify the behaviour.** Run it on a real Pi (or a qemu-enabled runner): `rubixd
   --version`, then `rubixd status` against a real ledger — the RocksDB path is exactly
   what the uint128 flag touched, so a smoke test that never opens the store proves
   nothing.
4. **Ship.** The binary + `packaging/rubixd.service` land on the Pi per
   `rubix-fleet:docs/DEPLOY.md`; `libstdc++6` is the one runtime dep to confirm.

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md) — no mocks. This scope
ships build inputs, so the tests prove **the artifact is real and works on the arch**:

- **Cross-build gate (CI, every PR).** All three targets build in the image. armv7 is the
  one that regresses silently — a dependency bump that re-enables uint128 detection, or an
  SDK bump that adds a C dep, breaks *only* the 32-bit target. **This gate is the whole
  point**; without it the target rots back to broken, which is exactly the state it is in
  today despite being listed in `TARGETS` since day one.
- **ELF assertion.** `file`/`readelf` on each artifact: armv7 → `ELF 32-bit … ARM, EABI5`;
  aarch64 → `ELF 64-bit … ARM aarch64`; x86_64 → `ELF 64-bit … x86-64`. Cheap, and it
  catches the failure where a "cross" build silently produced a host binary.
- **armv7 runtime smoke — the test that actually matters.** Execute the armv7 binary under
  **qemu-user** (`docker run --platform linux/arm/v7`, binfmt registered on the runner) or
  on real hardware: `rubixd --version` exits 0, **and** a command that **opens the RocksDB
  ledger** (`rubixd status` against a temp `data_root`) succeeds. The uint128 flag changes
  RocksDB's *math path* — a version-print smoke test would pass on a subtly broken store.
  If a runner cannot emulate armv7, the suite **skips and reports** (the house rule from
  [`docker-backend-scope.md`](docker-backend-scope.md)'s dockerd posture) — it never fakes,
  and a skip is never counted as a pass.
- **Ledger round-trip on armv7.** Write, restart, read back — the durability property, on
  the arch whose C build we modified. This is the single highest-value test in the scope.
- **No `cfg(target_arch)` guard.** A grep gate: the arch difference must stay in build
  inputs. If this ever fails, rule 1 has been violated and the fix is wrong.

**Known environment limit, recorded honestly:** the current dev box **cannot execute any
arm32 binary** — no qemu binfmt handler is registered (verified: even
`docker run --platform linux/arm/v7 arm32v7/debian /bin/echo` gives `exec format error`).
So today armv7 is **cross-verified but not runtime-verified**. Registering binfmt on the CI
runner (`docker/setup-qemu-action`) is what closes this, and until it does the runtime
smoke is a real gap — not a formality.

## Risks & hard problems

- **The uint128 flag is a workaround on someone else's build script.** We are correcting
  `surrealdb-librocksdb-sys`'s host-vs-target autodetect from outside. A version bump could
  change the detection, rename the define, or fix it upstream — and our `-U` would then be
  either redundant or (if they add a *different* guard) insufficient. Mitigation: the CI
  armv7 gate is the canary; it fails loudly on the next bump. Worth an upstream issue —
  detecting a 64-bit-host feature while cross-compiling to 32-bit is a genuine bug, not our
  special case.
- **Binary size — measured, and a non-issue.** ~26 MB armv7 (`.text` alone is **22 MB** of
  RocksDB C++), ~28 MB aarch64, ~33 MB x86_64 — *after* `strip = true`, `lto = "thin"`,
  `codegen-units = 1`, `panic = "abort"`. **The target boxes have ~8 GB**, so this is ~0.3%
  of storage: smaller is nice, not required. Recorded because it is a surprising number
  against the release profile's "small binaries" comment — **not** because it constrains
  anything. Do not trade durability, debuggability, or build simplicity to shrink it. The
  size facts stay documented (`docs/DEPLOY.md`) so nobody re-litigates this from surprise.
- **`libstdc++.so.6` is a runtime dependency** (verified via `objdump -p`: `libstdc++.so.6`,
  `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, `ld-linux-armhf.so.3`). Raspberry Pi OS ships
  it; a minimal/distroless-style rootfs might not. This is the practical consequence of
  RocksDB that bites at *install* time, not build time — hence it is in the deploy doc, not
  a footnote.
- **glibc version floor.** Built against Debian bookworm's glibc; the artifact declares
  `for GNU/Linux 3.2.0`. An older Pi OS (buster) could fail at load with a `GLIBC_2.xx not
  found` that looks nothing like an arch problem. Mitigation: state the floor in the deploy
  doc; if a real box is older, that is the musl/static reopen trigger.
- **Build time and CI cost.** ~3 min per target after cache, and RocksDB's C++ dominates a
  cold build. Three targets per PR is real minutes. Mitigation: cache the image and the
  cargo registry; if it becomes the PR bottleneck, run the full matrix on `master` + tags
  and armv7-only on PRs — **never** drop armv7, which is the one that breaks.
- **armv7 is the canary for every future dep.** Any new C/C++ dep repeats this exact
  three-step dance. That is an argument for the CI gate, not against the target.

## Decisions (no open questions)

- **Target triple — decided: `armv7-unknown-linux-gnueabihf` only.** Hard-float, matching
  lb's image and Pi-class hardware. No armv6, no soft-float, no musl in v1.
- **Toolchain — decided: extend lb's `docker/build/Dockerfile`; never `cross`, never zig,
  never host-installed cross-gcc.** One cross doctrine across both repos, as the parent
  scope already committed to. *Reopen if*: lb's image stops carrying armv7 — then
  rubix-fleet owns a full image rather than a two-line extension.
- **The uint128 fix — decided: `-UHAVE_UINT128_EXTENSION` via the per-target
  `CXXFLAGS_armv7_unknown_linux_gnueabihf` env, baked into the image, armv7 only.** Not
  bare `CXXFLAGS` (would pessimise the 64-bit targets). *Reopen if*: upstream fixes the
  host-vs-target detection — then delete it and let the CI gate prove it.
- **Store — decided: `kv-rocksdb` stays on armv7, and size is not an argument against it.**
  The target boxes have ~8 GB; a 26 MB binary is ~0.3% of that. The `libstdc++` dep is the
  only real consequence, and it is an install-time note, not a cost. `kv-mem` is a test
  posture, not a deployment. *Reopen if*: a real armv7 box hits a hard **RAM** wall (RocksDB's
  working set, not the binary) — and the answer then is a different store for *all* arches,
  not an arch-specific one (rule 1).
- **Linking — decided: dynamic (glibc + libstdc++), not musl/static.** Pi OS ships both;
  static-linking RocksDB's C++ is a fight with no current caller. *Reopen if*: a target
  rootfs lacks libstdc++, or the glibc floor bites a real box.
- **armv7 container image — decided: never** (ratified from
  [`../containerize-scope.md`](../containerize-scope.md)). This scope is the reason that is
  safe: armv7 is a **fully supported bare-binary target**, not a neglected one.
- **`make cross` — decided: container-driven, and it is the only supported release path.**
  `make cross-native` stays for a fast x86_64 inner loop. A laptop and CI must emit the
  same bytes.
- **Runtime verification — decided: required, via qemu-user on CI (`docker/setup-qemu-action`),
  and it must open the RocksDB ledger.** A `--version` smoke does not exercise the code path
  the flag touched. Skips-and-reports when a runner cannot emulate; a skip is never a pass.

## Related

- [`../containerize-scope.md`](../containerize-scope.md) — the sibling posture: armv7 is
  **images-excluded**, which this scope makes honest by making the bare binary real. Shares
  the `deploy/common/` reuse rule and the wave-1 prerequisites (a pinned toolchain and a
  committed `Cargo.lock` are what make a cross-build reproducible at all).
- [`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md) — "must stay tiny on
  armv7 boxes" (§Intent), the arch-advertise/ELF-recheck guard (§Risks), and
  `lb docker/build/` as the named cross toolchain (§Intent).
- [`agent-core-scope.md`](agent-core-scope.md) — owns `packaging/rubixd.service`, the unit
  the armv7 deploy installs.
- `lb docker/build/Dockerfile` — the image extended (armv7 = *"the Raspberry-Pi-class edge
  target"*), and `build.sh` — the per-target driver pattern.
- `rubix-fleet:docs/DEPLOY.md` — the operator runbook (docker + arm) this scope's build
  feeds; `rubix-fleet:deploy/common/Dockerfile.cross`, `Makefile` `cross-*` targets,
  `.github/workflows/ci.yml` (the cross-build gate).
- `ems docs/scope/platform-targets/arm-raspberry-pi-build-scope.md` — the ARM/systemd
  distribution ask the parent scope generalises.
