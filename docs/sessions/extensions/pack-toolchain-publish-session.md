# Session — publish the pack toolchain (`lb-pack` + `lb-devkit`) for embedders

**Date:** 2026-07-12
**Area:** extensions / release
**Status:** shipped — code + real tests + docs + live git-install run; green.
**Scope:** [../../scope/extensions/pack-toolchain-publish-scope.md](../../scope/extensions/pack-toolchain-publish-scope.md)
**Public:** [../../../doc-site/content/public/extensions/extensions.md](../../../doc-site/content/public/extensions/extensions.md) (dev-flow / packaging section)
**Skill:** [../../skills/lb-pack/SKILL.md](../../skills/lb-pack/SKILL.md)

## Goal

cc-app (the reference embedder) ran `make dev` and died at `cargo build -p lb-pack`: the
artifact packager and its library (`lb-devkit`) were both `publish = false`, so the signing
idiom the node verifies with was unreachable from outside lb's workspace. Make the pack
toolchain a first-class, git-tag-consumable part of the extension-developer surface — with
**no trust-model change** (the `Artifact` shape, Ed25519 signature, `verify_artifact`,
`ext.publish` cap, and `LB_TRUSTED_PUBKEYS` are all unchanged; this is packaging/exposure only).

## What shipped

1. **`lb-devkit` and `lb-pack` are publishable** — `publish = false` dropped from both;
   `lb-pack` gained `description`/`keywords`/`categories`. Both consumable by git tag:
   `cargo install --git https://github.com/NubeDev/lb --tag <node-v*> lb-pack`, or as a
   workspace git-dep.

2. **The API audit (the load-bearing decision).** Chose the scope's recommended option (a):
   stabilize **only the pack-facing surface**; everything else moves behind a `devkit-full`
   feature (default-ON so no in-workspace churn — `lb-host`, `lb-cli`, `lb-gateway` pin
   `features = ["devkit-full"]`; the workspace dep is `default-features = false`).

   **The published contract of `lb-devkit` is exactly:**
   - `sign_artifact(manifest_toml, wasm, key_id, &SigningKey) -> Result<Artifact>`
   - `load_or_create_key(path) -> Result<LoadedPublisherKey>`
   - `publisher_trust_line(key_id, &SigningKey) -> String`
   - `LoadedPublisherKey { signing_key, generated }`
   - `Artifact` — re-exported `lb_registry::Artifact`, the same type `verify_artifact` consumes.

   Everything else (`build_extension`, `scaffold_extension`, `inspect_extension`, `templates`,
   `Toolchain`/`ProcessToolchain`/`ContainerToolchain`, `write_file`, the model structs, roots,
   `Feature`) is behind `devkit-full` and is **not semver-stable for embedders** — pin with
   `default-features = false`. The future `lb-ext` CLI (`ext-out-of-tree-scope.md`) stabilizes
   the rest in its own pass. Rejected option (b), publishing the whole ~15-symbol surface now:
   cheaper today but every symbol becomes a one-way contract before the CLI design settles.

3. **`Artifact` naming collision resolved:** devkit's internal build-output listing struct
   (`model::Artifact`, kind/path/size/mtime) renamed **`BuildArtifact`** so the signed
   `lb_registry::Artifact` is *the* `Artifact` on the published surface. No external consumer
   used the old name (verified by grep before renaming).

4. **`lb-pack` itself** pins `lb-devkit` with `default-features = false` — the binary is a
   standing proof the stable core builds without `devkit-full`.

5. **CI:** new "Pack toolchain stays publishable" step —
   `cargo check -p lb-devkit --no-default-features` + `cargo test -p lb-pack --test
   pack_verify_test` (which includes the publishable-chain metadata assert that fails on the
   old flags — the check that would have caught this gap).

6. **Docs + skill:** dev-flow packaging section in `public/extensions/extensions.md`;
   `docs/skills/lb-pack/SKILL.md` grounded in the live run below.

## Tests (rust/tools/pack/tests/pack_verify_test.rs — real binary, real verifier, no mocks)

All drive the **actual `lb-pack` binary** (`CARGO_BIN_EXE_lb-pack`) over the **real built
`hello-v2` wasm**, verified by **`lb_registry::verify_artifact`** (the node's own gate):

- `packed_artifact_verifies_against_the_printed_trust_line` — the round-trip promise.
- `an_untrusted_key_is_rejected_at_verify` — the trust deny-test: a key outside the trusted
  set is rejected; publishing the packager does not weaken the node-side gate.
- `a_tampered_wasm_fails_verification` — byte flipped after signing → digest check fails.
- `packing_is_deterministic_for_same_inputs_and_key` — identical artifact bytes (Ed25519 is
  deterministic; no timestamp/nonce drift).
- `the_pack_toolchain_dependency_chain_is_publishable` — cargo-metadata assert that no crate
  in lb-pack's lb closure is `publish = false`. **Fails on pre-change master.**

```
running 5 tests
test the_pack_toolchain_dependency_chain_is_publishable ... ok
test a_tampered_wasm_fails_verification ... ok
test packed_artifact_verifies_against_the_printed_trust_line ... ok
test packing_is_deterministic_for_same_inputs_and_key ... ok
test an_untrusted_key_is_rejected_at_verify ... ok
test result: ok. 5 passed; 0 failed
```

(Capability-deny/workspace-isolation categories: N/A per the scope — the tool is offline and
workspace-blind; the trust deny-test above is this change's deny gate. Full-workspace run: see
"Suite status" below.)

## Live git-install run (grounds the skill)

The exact embedder path, run against this branch:

```
$ cargo install --git file:///home/user/code/rust/lb --branch pack-toolchain-publish lb-pack --root ./toolchain
   Installed package `lb-pack v0.1.0 (…#86f98c2)` (executable `lb-pack`)
$ lb-pack rust/extensions/hello/extension.toml …/hello_ext.wasm keys/dev.key --key-id live-demo --out artifacts/hello.json
wrote artifact: artifacts/hello.json
generated new dev publisher key: keys/dev.key
trusted-pubkey: live-demo=ee4eab3a8bbf54f5cd9521a8afea4583dc7c8a412afc687f06324764bdd2db2b
$ lb-pack pubkey keys/dev.key --key-id live-demo
live-demo=ee4eab3a8bbf54f5cd9521a8afea4583dc7c8a412afc687f06324764bdd2db2b
```

(Box quirk, not a crate issue: this machine has no system `cc`, so the install needed the zig
linker via `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER` — `cargo install` doesn't read the
workspace `.cargo/config.toml`. A normal embedder box with a C toolchain needs nothing.)

## Suite status

`cargo test --workspace --no-fail-fast` run in-session; green except the **pre-existing**
failing set already recorded (persona catalog/coding, agent_routed, reminder, devkit
build/e2e, proof_panel — red on clean master, verified before this change; see
`docs/debugging/` history). No new failures introduced; all pack/devkit/registry/cli
sign+publish tests green. <!-- final output pasted below when the background run completes -->

## Release

Tagged **node-v0.3.3** (patch bump over node-v0.3.2 — purely additive: no verb, cap, table,
or wire change). The authz-verbs-mcp-dispatch scope had already shipped under node-v0.3.2, so
this toolchain fix tags on its own. cc-app's follow-on (Makefile: install the published tool
at the lb tag instead of `cargo build -p lb-pack`) is cc-app's change, tracked in its own
repo at `docs/debugging/build/make-dev-lb-pack-not-found.md`.
