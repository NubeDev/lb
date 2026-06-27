# Extension dev flow: build → pack → publish → install → load

How to get an extension onto a running node during development, end to end. The same signed-`Artifact`
shape flows through the CLI packager, the gateway's `POST /extensions`, and the UI's "Upload signed
artifact" — there is one wire format and one trust model (README §6.4).

## The chain

```
extension.toml + *.wasm                 (build.sh / `make build-wasm`)
        │  lb-pack: digest → Ed25519-sign → Artifact JSON
        ▼
signed Artifact JSON                     (.lazybones/extensions/<ext>.artifact.json)
        │  POST /extensions  (or the UI uploader)  — Bearer token, ws from the token
        ▼
gateway → lb_host::ext_publish:
   1. authorize  mcp:ext.publish:call          (capability gate — 403 if absent)
   2. verify_artifact against LB_TRUSTED_PUBKEYS (signature gate — 422 if untrusted/tampered)
   3. cache the verified bytes + record the catalog entry
   4. install_extension: persist the Install grant + load the component into the runtime
        ▼
extension is callable immediately (no restart) — and reloaded on the next boot
```

Two independent gates: the capability gate (may this caller publish?) and the signature gate (are these
the bytes a trusted publisher signed?). A fully-granted caller handing over a tampered or
foreign-signed artifact is still refused, and **nothing is stored**.

## One-command dev loop

```sh
make cloud          # node + gateway; trusts the dev publisher key automatically (LB_TRUSTED_PUBKEYS)
make publish-ext    # build hello-v2 → sign → POST /extensions → 204 (installed + loaded live)
```

Override the target extension: `make publish-ext EXT=my-ext` (expects
`rust/extensions/my-ext/extension.toml` + its built wasm).

Just produce the artifact for the UI's uploader instead of POSTing it:

```sh
make pack           # → .lazybones/extensions/<EXT>.artifact.json
```

## The packager (`lb-pack`)

`lb-pack` is the bridge `build.sh` never had — it turns a built wasm + manifest into the signed
`Artifact` JSON, using the **same** `digest` + Ed25519 idiom the node verifies with (no second crypto
stack), so a packaged artifact verifies by construction.

```sh
# sign an artifact
lb-pack <manifest.toml> <ext.wasm> <key-file> [--key-id <id>] [--out <artifact.json>]

# print the publisher's trusted-key line for LB_TRUSTED_PUBKEYS (generates the key on first run)
lb-pack pubkey <key-file> [--key-id <id>]      # → dev-publisher=<64-hex-pubkey>
```

The key file holds the publisher's 32-byte Ed25519 seed (hex); it is generated and persisted on first
use, so the dev publisher identity is stable across runs.

## Trust is the environment, never the upload

A node accepts an upload only from a publisher key in its allow-list — set by the **environment**, so an
attacker cannot self-trust by signing with their own key:

```sh
LB_TRUSTED_PUBKEYS="dev-publisher=<hexpubkey>,other=<hexpubkey>"   # comma-separated key_id=hexpubkey
```

Unset/empty → an empty allow-list → every upload is `422`. `make dev`/`make cloud` derive this from the
dev key automatically (`lb-pack pubkey`); to run the node by hand, `export LB_TRUSTED_PUBKEYS=$(make -s
trusted-pubkey)`.

## Local layout (`.lazybones/`)

All dev state lives under one root (so one `rm -rf .lazybones` resets a box):

```
.lazybones/data/dev-store          the SurrealKV node store (LB_STORE_PATH)
.lazybones/keys/dev-publisher.key  the dev publisher Ed25519 seed (hex)
.lazybones/extensions/*.artifact.json  packaged signed artifacts
```

Override the root with `make … LB_DIR=/path`.

## Survives a restart

The durable `Install` record + the digest-keyed verified cache are the source of truth. On boot the
node runs `load_enabled` for its workspace: it re-loads every **enabled** wasm install from the cache
(a **disabled** one is deliberately left off — `disable` is durable intent the boot reconciler honors).
So a published extension is back after a restart with no re-upload.

## HTTP status reference

| Status | Meaning |
|---|---|
| `204` | published, verified, installed, and **loaded live** |
| `403` | the token lacks `mcp:ext.publish:call` |
| `422` | the gateway does not trust this publisher key, or the artifact is tampered/unsigned |

See `docs/sessions/extensions/dev-publish-flow-session.md` for the implementation and tests.
