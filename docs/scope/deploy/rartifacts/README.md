# rartifacts — scope index + coding-session roadmap

`rartifacts` is the remote artifact server of the fleet package plane — and it is
**built on lb**: a product-host binary embedding `lb-node` (the `ems`/`rubix-ai`
pattern), with all package logic in a **native (Tier-2) `rartifacts` extension**
(`pkg.*` MCP tools + the content-addressed blob dir) and the operator console as the
extension's **federated UI** (shadcn/Tailwind) on the lb minimal shell. Parent scope:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md) — read it first (the
posture decision and its trade-offs, package model, signing envelope, the plain-REST
wire contract rubixd depends on).

**Code home**: the `rubix-fleet` repo — `rartifacts/host/` (the binary + mounted
routes) and `rartifacts/extensions/rartifacts/` (extension + `ui/`), sharing
`crates/fleet-spec` + `crates/fleet-auth` with rubixd. Because this *is* an lb node,
the lb rules apply in full — capability-deny + workspace-isolation tests mandatory,
no mocks, FILE-LAYOUT — per `docs/HOW-TO-CODE.md` and
`docs/scope/testing/testing-scope.md`.

## Slices

| # | Scope | What ships |
|---|---|---|
| 1 | [`server-core-scope.md`](server-core-scope.md) | Host binary embedding `lb-node`, the native extension skeleton, package records in the store, the ext-owned blob dir, `pkg.list/get`, the read-side host routes |
| 2 | [`token-auth-scope.md`](token-auth-scope.md) | One-time claim of the boot-seeded **admin api-key** (fleet-auth), **agent** + **publisher** api-key principals with narrow caps, server-side revoke, the **anonymous** public tier |
| 3 | [`publish-scope.md`](publish-scope.md) | Streaming multipart publish route → `pkg.publish` (re-hash, Ed25519 verify, ownership, visibility, immutability) |
| 4 | [`resolve-scope.md`](resolve-scope.md) | `pkg.resolve` (exact / semver range / channel), `pkg.promote`/`yank`, ranged blob downloads |
| 5 | [`web-ui-scope.md`](web-ui-scope.md) | Federated UI pages (shadcn/Tailwind) on the minimal shell: claim, packages, publish, channels, agents, access |

## Coding-session roadmap (long-running AI sessions)

One session per slice, in order; each exit gate green for real (a spawned rartifacts
node — embedded lb, real store, real extension, real files) before the next session.
Each session writes `docs/sessions/deploy/rartifacts-<slice>-session.md`, logs
breakage under `docs/debugging/deploy/`, and updates `docs/STATUS.md`.

**Dependencies**: rubixd sessions 1–2 first (`fleet-spec`, `fleet-auth`). Slice 1
needs an lb tag exposing `boot_full`/`BootConfig` (shipped — the rubix-ai seam) and
the native-extension publish path (shipped). Slice 5 depends on the **minimal shell**
(`frontend/minimal-shell-scope.md`) being buildable — verify its status at session
start; if it hasn't shipped, the fallback is vendoring the ems thin-shell pattern,
recorded as debt. After slice 4, rubixd session 6 (bundles/poller) integrates against
a real rartifacts.

1. **Session 1 — host + extension core.** `rartifacts/host/` binary (env-driven
   `BootConfig`: `RARTIFACTS_HOME/STORE_PATH/GATEWAY_ADDR` default `0.0.0.0:9410`,
   `EXT_UI_DIR`, `SIGNING_KEY`; boot-seeds the `fleet` workspace and publishes the
   in-repo extension at boot); the native extension with `pkg` record models
   (`pkg`, `pkg_artifact`, `pkg_channel`, `pkg_event` — store tables via host
   callback), the content-addressed blob dir (atomic temp+rename writes, streamed
   hashing, record-only-after-blob rule), `pkg.list`/`pkg.get` tools, and the
   host-mounted read routes (`GET /packages*` — the rubixd wire contract, projection
   of the tools). **Exit gate**: node boots; a seeded package lists over MCP *and*
   the REST projection; blob round-trips byte-identical; capability-deny +
   workspace-isolation tests green.
2. **Session 2 — identity + claim.** Boot mints the admin api-key (unclaimed);
   `POST /api/claim` (fleet-auth, **6-digit boot code mandatory**) reveals it once;
   agent/publisher api-keys minted via the shipped lb api-key verbs with narrow cap
   bundles; the boot-minted read-only **anonymous** principal; visibility policy in
   the `pkg.*` read tools (anonymous → public only, same-401 no-leak). **Exit gate**:
   claim-once/410/reset suite green; route × principal × visibility matrix green; a
   revoked agent api-key 401s next call; the anonymous-leash caps test proves it
   reaches only public reads.
3. **Session 3 — publish.** The streaming multipart host route + `pkg.publish`:
   server-side re-hash, Ed25519 verify against the publisher's registered pubkeys,
   first-publish ownership, `visibility` set-once-then-`pkg.set_visibility`,
   immutable releases (same-digest idempotent 200 / different-digest 409), yank,
   `pkg_event` audit rows. **Exit gate**: publish→fetch roundtrip per kind; the full
   deny matrix (tampered 422, foreign key 422, non-owner 403, oversize 413) green.
4. **Session 4 — resolve + downloads.** `pkg.resolve` as a pure function (exact /
   range / channel, arch-strict, yank-aware, machine-readable 404 reasons),
   `pkg.promote` (+ demote reason, events), `GET /blobs/{sha256}` with
   Range/ETag/304 streaming from the ext blob dir, visibility-gated. **Exit gate**:
   resolution truth-table green; kill-and-resume download byte-identical; a real
   rubixd fetcher resolves/downloads public (anonymous) and private (agent key).
5. **Session 5 — federated UI.** The extension's `ui/` pages on the minimal shell:
   claim, packages (visibility badge/toggle), publish, channels, **agents roster**
   (last-seen, revoke), access (publisher keys + api-keys). **Exit gate**: browser
   smoke — claim → browse → upload → promote → register agent → revoke it — against
   a live node; every action is a gateway call a curl could make.

**Cross-cutting**: [`../containerize-scope.md`](../containerize-scope.md) — the container
image for this server (the **AWS** workload, and the primary reason that scope exists).
It adds no slice here and needs no code change: slice 1's env-driven `BootConfig` already
defaults `RARTIFACTS_GATEWAY_ADDR` to `0.0.0.0:9410`, and its "release images bake the
pre-published extension artifact" rule (§Risks — boot self-publish stays dev-only) is the
line that keeps a Rust toolchain out of the runtime image. The image lands **with slice
1** — the first point there is a `/health` and a store worth persisting. Its one hard
requirement: the store *and* the blob dir must sit on the same durable volume (`/data`),
never an ephemeral task filesystem.
