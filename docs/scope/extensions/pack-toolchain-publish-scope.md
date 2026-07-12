# Extensions scope — publish the pack toolchain (`lb-pack` + `lb-devkit`) for embedders

Status: **done** — shipped 2026-07-12 ([session](../../sessions/extensions/pack-toolchain-publish-session.md)); promoted to `public/extensions/extensions.md` (dev-flow section) + `docs/skills/lb-pack/SKILL.md`. Tagged `node-v0.3.3`.

> Read with: `ext-out-of-tree-scope.md` (the larger destination — the `lb-ext` CLI over
> `lb-devkit`, shipped with the SDK release; **this scope is the prerequisite slice** that
> makes `lb-devkit`/`lb-pack` consumable at all), `extensions-scope.md` (the signed
> `Artifact` → `verify_artifact` → install path this packages *into*),
> `devkit-container-build-scope.md` (the build side of the same dev flow),
> `reference-extensions-scope.md` (rubix-ai / cc-app as the embedders this unblocks),
> `../release/updates-to-core-release-scope.md` (the git-tag release model these crates join),
> `../testing/testing-scope.md`.

An embedder (rubix-ai, cc-app, the next product host) can build a `*.wasm` extension, but it
**cannot package and sign that extension into the `Artifact` JSON** the gateway's
`POST /extensions` and the UI's `UploadArtifact` require — because the one tool that does it,
`lb-pack` (`rust/tools/pack/`), and the one library it needs, `lb-devkit`
(`rust/crates/devkit/`), are both `publish = false`. They are reachable only from *inside*
lb's own workspace. So the signing idiom the node verifies with — the Ed25519 sign, the
digest, the trust line (`lb-registry`, `ed25519-dalek`) — is locked away from every downstream
host, and each embedder is forced to rediscover the wall (cc-app's `make dev` fails today at
`cargo build -p lb-pack`: no such package) and either copy the tool or hand-roll a second
crypto stack. This scope makes the pack toolchain a **first-class, git-tag-consumable part of
the extension-developer surface**, so an embedder packs+signs against a versioned contract the
same way it embeds the core via `lb-node`.

## Goals

- **`lb-devkit` is consumable by an embedder.** Drop `publish = false`; it becomes a
  git-tag-pinnable library (the same model as `lb-node`/`lb-host` — lb crates are consumed by
  `git = "…/lb", tag = "…"`, not crates.io). It already exposes exactly the embedder-facing
  surface: `sign_artifact`, `load_or_create_key`, `publisher_trust_line`. One source of truth
  for the sign/verify idiom — no second crypto stack downstream (rule 10).
- **`lb-pack` is installable and documented as *the* packaging step.** Drop `publish = false`;
  make it consumable two ways: `cargo install --git https://github.com/NubeDev/lb --tag <tag>
  lb-pack` (the standalone dev-tool path), and as a git-tag dep for a host that wants it as a
  workspace member. It is the documented bridge between `build.sh` and `POST /extensions`.
- **An embedder's Makefile stops assuming a local `lb-pack` crate.** The pattern
  `cargo build -p lb-pack` (which only works inside lb) is replaced by installing/pinning the
  published tool — documented in the extension-authoring guide so the *next* embedder never
  hits cc-app's wall.
- **No new trust model, no new crypto.** The `Artifact` shape, the Ed25519 signature,
  `verify_artifact` → `VerifiedArtifact` → cache → install, the `ext.publish` cap, and
  `LB_TRUSTED_PUBKEYS` are all unchanged (matching `ext-out-of-tree-scope.md` non-goals). This
  is purely a *packaging/exposure* change — the verifier already trusts what `lb-pack` produces
  by construction, because both sides share `lb-devkit`.

## Non-goals

- **Not the full `lb-ext` CLI.** `ext-out-of-tree-scope.md` scopes a thin `lb-ext` CLI
  (`new`/`build`/`pack`/`publish`) shipped with the SDK release. That is the destination; this
  scope is the **prerequisite** — publish the library + the pack binary so that CLI (and every
  embedder, today) has something to build on. `lb-pack` stays a single-purpose `pack` binary
  here; folding it into `lb-ext` is that scope's job.
- **Not crates.io publication.** lb crates are consumed by git tag, not a public registry
  (`updates-to-core-release-scope.md`). "Publish" here means "drop `publish = false` and make
  git-tag/`cargo install`-consumable," nothing more. (Dropping the flag also *permits* a future
  crates.io push, but that is out of scope and unneeded.)
- **Not moving the extensions out of `lb`.** The out-of-tree repo split is its own scope; this
  works with extensions wherever they live.
- **No change to `POST /extensions` / the gateway upload route or the install path.**

## Intent / approach

Three small, additive moves — no logic changes:

1. **`rust/crates/devkit/Cargo.toml` — drop `publish = false`.** Its only lb dependency is
   `lb-registry`, which is **already publishable and self-contained** (external deps only:
   serde, ed25519-dalek, sha2, rand). So the publishable dependency chain is clean —
   `lb-devkit` is currently the *only* `publish = false` crate under `rust/crates/`, and
   removing the flag closes the chain with no cascade. Verify `lb-registry` exposes what
   `lb-devkit` re-exports across the boundary.
2. **`rust/tools/pack/Cargo.toml` — drop `publish = false`** and confirm the `[[bin]]` is
   `cargo install`-friendly (it is: one `main.rs`, deps `lb-devkit` + `serde_json` + `anyhow`).
   Add a short `[package] description` + `keywords`/`categories` so an installed binary reads
   well. Keep the crate name `lb-pack` (embedders and cc-app's Makefile already use it).
3. **Docs — name the toolchain in the extension-authoring flow.** In
   `public/extensions/extensions.md` (dev-flow section) and the reference-extension guide,
   document the packaging step: `cargo install --git …lb --tag <node-v*> lb-pack`, then
   `lb-pack <manifest> <keyfile> --key-id <id> --out artifact.json`, then upload. This is the
   piece `build.sh` never had; making it discoverable is half the value.

**Why publish the *real* crates rather than have each embedder vendor them:** vendoring forks
the sign/verify idiom — the moment `lb-devkit`'s digest or trust-line format evolves, every
vendored copy silently produces artifacts the node rejects. One published `lb-devkit`, pinned
by the *same tag* an embedder already uses for `lb-node`, means "packs correctly" is guaranteed
by version alignment, not by luck. (Rejected: keep `publish = false` and tell embedders to copy
`rust/tools/pack/` — that is exactly what makes lb *bad* for a new embedder, and it is the
status quo that broke cc-app's `make dev`.)

**Why this is safe:** dropping `publish = false` exposes no capability — `lb-pack` only *signs*
with a **local** publisher key the embedder generates; trust is established node-side by
`LB_TRUSTED_PUBKEYS`, unchanged. A packaged artifact from a key the node doesn't trust is
rejected at verify exactly as today. Publishing the packager does not publish trust.

## How it fits the core

- **Tenancy / isolation:** N/A at the packaging layer — `lb-pack` is an offline dev tool that
  reads a `.wasm` + manifest and writes a JSON file; it touches no workspace, store, or bus. The
  workspace wall lives at `POST /extensions` (unchanged).
- **Capabilities:** unchanged. Signing is local; *publishing* to a node still requires the
  `ext.publish` cap on the session token at the gateway. The tool mints no caps and needs none.
- **Placement:** the tool runs on the **developer's / embedder's machine**, not a node. It is a
  build-time artifact producer, never compiled into a running node (as its own header states:
  "A dev tool, not a runtime crate").
- **MCP surface** (API shape §6.1): **none.** This exposes no MCP verb — it is a CLI/library, not
  a tool call. CRUD/get-list/feed/batch all N/A. The artifact it produces is consumed by the
  *existing* `POST /extensions` route; no API is added or changed.
- **Data (SurrealDB):** N/A — no persistence. Output is a file on disk.
- **Bus (Zenoh):** N/A.
- **Sync / authority:** N/A — offline, deterministic (same input + key → same signed artifact).
- **Secrets:** the **publisher signing key** (Ed25519). It is generated locally by
  `load_or_create_key` and stored at a developer-chosen path (`$(KEY_FILE)` in cc-app's
  Makefile); its *public* half goes into the node's `LB_TRUSTED_PUBKEYS` via
  `publisher_trust_line`. This is unchanged — the key handling already lives in `lb-devkit`; this
  scope only makes that (already-correct) handling reachable. **Document the key's sensitivity**
  in the dev-flow guide: it is the identity a node trusts, so treat it like any signing key
  (don't commit it; the trust line is the only thing shared).
- **SDK/WIT impact:** **none to the WIT/plugin boundary.** But this *is* a change to the stable
  **developer-facing crate surface** — `lb-devkit`'s public API (`sign_artifact`,
  `load_or_create_key`, `publisher_trust_line`) becomes a *contract* embedders pin to. **Flag it
  loudly:** once published, those signatures are semver-relevant for downstreams. Audit
  `lb-devkit`'s `pub` surface before the first tag so we don't publish something we must
  immediately break (`ext-out-of-tree-scope.md` will build the `lb-ext` CLI on exactly this
  surface).

## Example flow

An embedder (cc-app) packaging its `care` extension for upload — the flow that fails today:

1. Embedder pins the toolchain to the same lb tag it embeds:
   `cargo install --git https://github.com/NubeDev/lb --tag node-v0.3.x lb-pack` (once).
2. `cd rust/extensions/care && cargo build --release` → produces `care.wasm`.
3. `lb-pack care/extension.toml ~/.cc-app/keys/dev.key --key-id cc-app-dev --out
   .cc-app/artifacts/care.json` — `lb-pack` loads/creates the local Ed25519 key, digests the
   wasm + manifest, and writes the **signed `Artifact` JSON** (the same shape `verify_artifact`
   consumes).
4. First-run only: `lb-pack pubkey ~/.cc-app/keys/dev.key --key-id cc-app-dev` prints the
   `key_id=hexpubkey` trust line; the node is started with it in `LB_TRUSTED_PUBKEYS`.
5. Embedder logs in for a session token carrying `ext.publish`, then `POST /extensions` with the
   artifact → `204` (verified against the trusted key, installed, loaded live).

Before this scope, step 1 has no target (`lb-pack` isn't installable) and step 3's tool doesn't
exist outside lb — so cc-app's `make dev` dies at `cargo build -p lb-pack` with "package ID
specification `lb-pack` did not match any packages."

## Testing plan

Mandatory categories from `../testing/testing-scope.md` that apply, plus toolchain-specific
cases:

- **No-mocks / real-verify round-trip (§0):** the load-bearing test — `lb-pack` packages a real
  built fixture extension (`rust/fixtures/ext/hello` or the devkit-e2e wasm fixture), and the
  **real `verify_artifact`** (the node's own verifier, not a re-implementation) accepts it against
  the matching trust line. Packs-and-verifies by construction is the whole promise; prove it
  end-to-end with the real crypto path.
- **Untrusted-key rejection (capability/trust deny):** an artifact signed with a key **not** in
  `LB_TRUSTED_PUBKEYS` is **rejected** at verify — publishing the packager must not weaken the
  node-side trust gate. (Deny-symmetry, mirroring the grants-routing scope's deny-test discipline.)
- **Tamper-detection:** a byte flipped in the packaged wasm after signing fails verification —
  proving the digest covers the payload (regression guard on the sign/verify idiom).
- **Publishable-chain CI check:** `cargo publish --dry-run -p lb-devkit` and `-p lb-pack` (or the
  git-tag-consumption equivalent) succeed — i.e. no `publish = false` dep leaked into the chain.
  This is the test that would have *caught the gap*: it fails today.
- **Determinism:** same wasm + same key → identical artifact bytes (no timestamp/nonce drift),
  so a CI can cache/compare.

No hot-reload/offline/workspace-isolation category applies (the tool is offline and workspace-blind).

## Risks & hard problems

- **Publishing a crate API is a one-way door.** The moment `lb-devkit` is tag-published, its
  `pub` surface is a downstream contract. The real work is the **pre-publish API audit** (goal
  §SDK impact), not the flag flip. Underestimate this and the first embedder pins a surface we
  break next week. Do the audit; keep the exposed surface minimal (ideally just the three
  functions `lb-pack` uses + the `Artifact` type).
- **Trust-boundary misread.** Someone will worry that "publishing the signer" is a security
  regression. It isn't — signing is local, trust is node-side — but the scope and docs must say
  so explicitly, or a reviewer blocks it for the wrong reason.
- **Divergence from the `lb-ext` CLI.** If `ext-out-of-tree-scope.md`'s CLI later re-homes
  `pack`, we must not end up with two packagers. Mitigation: `lb-pack` stays a thin binary over
  `lb-devkit`; the future CLI *calls the same library*, so there is one implementation regardless
  of the entry point. State this so the two scopes compose instead of forking.

## Open questions

- **`cargo install` vs. workspace-member git-dep — ship one or both?** Recommendation: document
  `cargo install --git …lb --tag <tag> lb-pack` as the primary (a dev tool wants to be a binary on
  PATH), and note the git-dep-as-workspace-member option for a host that prefers `make`-driven
  local builds (cc-app's current Makefile shape). Confirm both resolve against a tag.
- **Should `lb-devkit`'s publish also unblock the `lb-ext` CLI now, or wait?** This scope only
  needs `sign`/`pack`; but since the audit touches the whole `pub` surface, decide whether to
  stabilize the full CLI-facing API in one pass (cheaper) or minimally (safer). Recommend minimal
  now, expand under `ext-out-of-tree-scope.md`.
- **Does cc-app's Makefile change belong here or in cc-app?** The Makefile edit
  (`cargo build -p lb-pack` → install the published tool) is **cc-app's** follow-on, not lb's —
  name it in Related so the embedder session picks it up, but it is out of lb's tree.

## Related

- `ext-out-of-tree-scope.md` — the destination (`lb-ext` CLI + SDK release); this is its
  prerequisite. Back-link this scope from there under its "thin `lb-ext` CLI" goal.
- `extensions-scope.md` — the signed `Artifact` / `verify_artifact` / install path packaged into.
- `devkit-container-build-scope.md`, `reference-extensions-scope.md` — the build side and the
  embedders (rubix-ai, cc-app) this serves.
- `../release/updates-to-core-release-scope.md` — the git-tag release model `lb-devkit`/`lb-pack`
  join.
- **Downstream discovery record:** cc-app's `make dev` fails at `cargo build -p lb-pack`; the
  embedder-side follow-on (Makefile: install the published tool instead of assuming a local
  crate) lands in cc-app once this ships.

## Skill doc

**Yes — this exposes an agent-/developer-drivable surface** (a packaging CLI). The implementing
session must write `skills/lb-pack/SKILL.md` (or fold it into an `lb-ext` skill if that lands
first): a runnable how-to grounded in a live pack→verify run — install, generate a key, pack a
built extension, read the trust line, publish. A stale/missing skill for a drivable dev-tool is a
finding (`SCOPE-WRITTING.md` §6). Until then, the dev-flow section of
`public/extensions/extensions.md` carries the documented commands.
