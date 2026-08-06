# scripts/ — build + sign an extension artifact

## Build

```
scripts/build-extension-artifact.sh <name>                          # wasm, or native for amd64/x86_64
scripts/build-extension-artifact.sh <name> --target linux-arm64
scripts/build-extension-artifact.sh <name> --target linux-armv7
```

Native builds need the toolchain image once: `docker build -t lazybones-build docker/build`.

Output: `dist/extensions/<name>.json` (wasm) or `dist/extensions/<name>.zip` (native).

## Available extensions

- **wasm**: `hello`, `hello-v2`, `echarts-panel`, `energy-dashboard`, `github-bridge`, `proof-panel`
- **native**: `federation`, `echo-sidecar`, `fleet-monitor`, `control-engine`, `ros`
- `mqtt` is declared native but has no implementation — not buildable

## Upload

```
curl -X POST <gateway>/extensions \
  -H 'Content-Type: application/zip'   # application/json for wasm-tier output \
  -H "Authorization: Bearer $TOKEN" \
  --data-binary @dist/extensions/<name>.zip
```

## Public key

Written to `dist/extensions/trusted-pubkey.txt` on every build. Add it to the target node's
`LB_TRUSTED_PUBKEYS` env (comma-separated for multiple keys), then restart the node.
