---
name: lb-pack
description: >-
  Package a built Lazybones extension into the signed `Artifact` JSON a node accepts. Use when a
  task says "pack / sign / package an extension", "produce the artifact for POST /extensions",
  "generate a publisher key / trust line", "set LB_TRUSTED_PUBKEYS", or when an EMBEDDER (cc-app,
  rubix-ai, any `lb-node` host) needs to publish an extension from outside lb's workspace. `lb-pack`
  is an offline dev tool — it reads `extension.toml` + the built `*.wasm` and writes signed JSON; it
  touches no node, store, bus, or workspace. It signs with a LOCAL Ed25519 key; trust is established
  node-side via `LB_TRUSTED_PUBKEYS`, so packaging grants nothing. Install it pinned to the same lb
  git tag the host embeds. The same signing lives in `lb-devkit` (stable surface: `sign_artifact`,
  `load_or_create_key`, `publisher_trust_line`, `Artifact`) for programmatic use.
---

# Packaging a built extension (`lb-pack`)

`lb-pack` is the bridge between an extension's `build.sh` and the gateway's `POST /extensions`:
it turns `extension.toml` + a built `*.wasm` into the **signed `Artifact` JSON** the node's
`verify_artifact` gate accepts. It uses the exact library the node verifies with (`lb-devkit` →
`lb-registry`), so a packed artifact verifies **by construction** — there is no second crypto stack.

Everything below is from a live run (2026-07-12, branch `pack-toolchain-publish`).

## 1. Install — pinned to the lb tag you embed

```sh
cargo install --git https://github.com/NubeDev/lb --tag node-v0.3.3 lb-pack
```

Pin the **same tag** as your `lb-node` dependency: version alignment is what guarantees the
artifact format matches what your node verifies. (A host that prefers a `make`-driven local
build can instead add `lb-pack` as a git-dep workspace member at the same tag.)

## 2. Pack (generates the key on first run)

```sh
lb-pack <extension.toml> <built.wasm> <key-file> --key-id <id> --out artifact.json
```

Live output (packing the real `hello` fixture):

```
$ lb-pack extensions/hello/extension.toml \
    extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm \
    keys/dev.key --key-id live-demo --out artifacts/hello.json
wrote artifact: artifacts/hello.json
generated new dev publisher key: keys/dev.key
trusted-pubkey: live-demo=ee4eab3a8bbf54f5cd9521a8afea4583dc7c8a412afc687f06324764bdd2db2b
```

- If `<key-file>` doesn't exist, a new Ed25519 seed is generated and persisted there (it tells
  you when it did — a new trust identity now exists).
- `--out` omitted → the artifact JSON goes to stdout; diagnostics stay on stderr.
- Same wasm + same key → **byte-identical artifact** (deterministic; CI-cacheable).

## 3. Trust the key node-side (first run only)

```sh
$ lb-pack pubkey keys/dev.key --key-id live-demo
live-demo=ee4eab3a8bbf54f5cd9521a8afea4583dc7c8a412afc687f06324764bdd2db2b
```

Start the node with that line in `LB_TRUSTED_PUBKEYS` (comma-separated `key_id=hexpubkey`).
Only then does the node accept artifacts from this key — an artifact from any other key is
rejected at verify (`422`), regardless of who uploads it.

**The key file is a signing identity — treat it like one.** Never commit it; only the trust
*line* (the public half) is shared. Whoever holds the key file can produce artifacts your
node trusts.

## 4. Publish

Log in for a session token carrying `ext.publish`, then upload the artifact:

```sh
curl -sf -X POST "$NODE/extensions" \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  --data @artifacts/hello.json    # → 204: verified, installed, loaded live
```

(Or `lb ext publish` from the operator CLI, or the UI's UploadArtifact — same route, same gate.)

## Programmatic use (`lb-devkit`)

```toml
lb-devkit = { git = "https://github.com/NubeDev/lb", tag = "node-v0.3.3", default-features = false }
```

`default-features = false` gives the **stable published contract** only: `sign_artifact`,
`load_or_create_key`, `publisher_trust_line`, `LoadedPublisherKey`, `Artifact`. The default-on
`devkit-full` feature (scaffold/build/inspect/templates/toolchains) is node-side machinery and
NOT semver-stable for embedders.

## Gotchas

- **Verify failures are the digest doing its job**: any byte changed after signing (wasm or
  manifest) fails verification. Re-pack; don't hand-edit artifact JSON.
- The manifest must carry `[extension] id` and `version` — `lb-pack` reads them into the
  artifact envelope (the loader still reparses the manifest as source of truth).
- Packaging ≠ permission: uploading still requires the `ext.publish` cap, and the extension's
  runtime caps are still `granted = requested ∩ admin_approved`. Nothing here bypasses the gate.
- Post-install, a session predating the install lacks the new ext's caps — re-login.
