# Platform targets scope

Status: scope (the ask). What a node — and an **extension artifact** — runs *on*: the OS/arch axis.
This matters the moment the **native tier** (S7) ships, because a native sidecar is a
**platform-specific binary** (unlike a portable `.wasm`), so an artifact must declare its target and
a node must only run an artifact built for it.

> Read with: `extensions/native-tier-scope.md` (why the target axis exists — a native binary is not
> portable), `extensions/extensions-scope.md` (the manifest the target field is added to),
> `registry/registry-scope.md` (the catalog/artifact a target tags), `node-roles/node-roles-scope.md`
> (placement × role, the *other* scheduling axis), `README.md` §6.3 (two tiers).

## Goals

- State the supported target axis and that **Tier-1 (wasm) is target-independent** — one `.wasm`
  runs on every node (the portability that makes wasm the default tier).
- Add the **target tag a native artifact needs**: a native binary built for `linux-x86-64` must not
  be run on `linux-aarch64`. The registry catalog/artifact carries a target; a node matches it.
- Keep the axis minimal for this slice (the proven sidecar runs on the dev/CI target) while making
  the field exist so the registry doesn't re-cut the artifact shape later (the §13 warning).

## Non-goals (this slice)

- Mobile / browser node targets, full vs. minimal profiles, cross-compilation matrices — recorded
  as the axis, not built. The slice proves one target (the CI host triple).
- Multi-arch *fat* artifacts (one catalog entry, many binaries) — the catalog can hold one entry
  per `(ext_id, version, target)`; fat-artifact bundling is a follow-up.

## Intent / approach

**Wasm is portable; native is not — so only native artifacts carry a target.** The target axis is a
*property of the artifact*, matched against the *node's* target:

- A **wasm** artifact has target `any` (or the field absent) — it runs identically on every node
  (Component Model + WASI P2 portability, §6.3). This is the whole reason wasm is Tier 1: no
  per-target build, no target matching.
- A **native** artifact declares a **target triple** (e.g. `x86_64-unknown-linux-gnu`). The
  registry catalog entry records it; on install, the node checks the artifact's target against its
  own and **refuses to spawn a mismatched binary** (a clear error, not a crash). This is the native
  manifest's `[native] target` field (native-tier scope) surfaced into the catalog.

The node's own target is config it knows at boot (compile-time `target_triple`); matching is a
string compare on install — **no `if cloud`/`if arch` in core paths**, just a data check at the
install seam, exactly like placement-vs-role (node-roles scope).

## How it fits the core

- **Symmetric nodes:** target matching is a data check at install, not a behavioral branch. The
  *same* binary on every node; what differs is which *extension artifacts* a node will run, decided
  by comparing two strings.
- **Registry / catalog:** the catalog entry gains an optional `target` (absent/`any` for wasm).
  Workspace-namespaced like every catalog entry — no isolation change.
- **Tenancy / capabilities:** target is orthogonal to the workspace wall and the capability gate —
  it only decides *can this node physically run these bytes*, after the gates already passed.

## Testing plan

- Native-tier slice: the proven sidecar is built for the CI host target; a target-mismatch refusal
  test (`refuses_native_artifact_for_wrong_target`) belongs with the native scope's install tests if
  the field is enforced this slice, else a noted follow-up. Wasm artifacts (`target = any`) install
  on every node as today (covered by the existing registry tests).

## Open questions

- The canonical target string (Rust target triple vs. a coarser `os-arch`). *Default: Rust target
  triple, the unambiguous one the binary is actually built for.*
- Fat/multi-arch artifacts (one version, several binaries) — deferred; one entry per target for now.
- Minimal vs. full node profiles (which crates/features a constrained target builds) — recorded as
  an axis; not exercised until a constrained target (e.g. a Pi) is a real node.

## Related

- `extensions/native-tier-scope.md` (the slice that creates the need), `extensions/extensions-scope.md`
  (the `[native] target` manifest field), `registry/registry-scope.md` (the catalog entry tagged),
  `node-roles/node-roles-scope.md` (placement × role — the complementary scheduling axis),
  `README.md` §6.3 (wasm portability vs. native).
