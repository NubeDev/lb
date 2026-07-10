# lb is a LIBRARY now — role + migration status (transitional)

> **Read this if anything here looks like "the app to run" or "where extensions live."** It isn't
> anymore. This file is the single source of truth for lb's current role and the out-of-tree migration.
> It is **temporary** — it will be deleted once the migration is proven (see "Retention window" below).

_Last updated: 2026-07-11._

## What lb is now

**lb is the platform CORE, consumed as a library.** A product host (e.g. **`NubeIO/rubix-ai`**) embeds it
via the **`lb-node`** crate seam (`BootConfig` + `boot_full`/`RunningNode`) as a downstream git dep and
runs *its own* binary. lb is no longer "the product you run" — it is the core a product **embeds**.

- Consume it: `lb-node = { git = "https://github.com/NubeDev/lb", tag = "node-v0.1.11" }` → fill
  `BootConfig` at the binary boundary → `lb_node::boot_full`. See
  [`docs/scope/node-roles/embed-node-scope.md`](docs/scope/node-roles/embed-node-scope.md).
- Symmetric nodes still hold: role = config, never a code branch.

## What has MOVED OUT (authoritative homes are no longer in this repo)

| Concern | Authoritative home now | lb's relationship |
|---|---|---|
| Extension **SDK / contract** (Rust WIT + native wire) | `NubeDev/lb-ext-sdk` (`lb-sdk`, `lb-ext-native`) | lb **consumes** it (git tag `sdk-v0.2.1`). `rust/sdk` is **deleted**. |
| Extension **UI contract** (page/widget mount + Vite preset) | `NubeDev/lb-ext-ui-sdk` (`@nube/ext-ui-sdk`) | lb's shell **imports** it (tag `ui-v0.4.1`), not a local copy. |
| **Product extensions** | `NubeDev/lb-extensions` (public), `NubeIO/rubix-ai-extensions` (private) | built against the **published SDKs**, **zero lb-repo access**. |
| **Product UI shell** | vendored into the product repo (e.g. `rubix-ai/ui`) | the product **owns** its shell; consumes the shared `@nube/*` packages. |

Owning scope for all of the above:
[`docs/scope/extensions/ext-out-of-tree-scope.md`](docs/scope/extensions/ext-out-of-tree-scope.md).

## What is RETAINED IN-TREE — TEMPORARILY (the safety net)

The following are **kept as-is on purpose**, as the reference implementation + fallback **while the
out-of-tree migration is validated**. They are **not authoritative** anymore — the SDK repos and the
out-of-tree extension repos are:

- `rust/extensions/*` — the in-tree extensions (`proof-panel`, `fleet-monitor`, `hello`, `echarts-panel`,
  …). **`federation` is no longer here:** it was **promoted to a first-class core crate** at
  `rust/crates/federation/` (it fails the rule-10 swap test, shares `lb-supervisor` verbatim, and is
  platform datastore-federation surface — see
  `docs/scope/extensions/federation-promote-to-core-scope.md`). It stays a supervised Tier-2 sidecar
  (DB drivers never link into the node); the `rust/extensions/*` cleanup must **not** touch it.
- `ui/` — the in-tree React shell (the reference the product shells are vendored from).

**Do not** point new work at these in-tree copies as the source of truth, and **do not** delete them yet.

## Retention window (when the in-tree copies go)

Keep the retained code **until the downstream migration is proven** — `rubix-ai` (host) +
`rubix-ai-extensions` (its extensions) running against the published contracts, for **~a few weeks**
(target: **late July / early August 2026**). Once that bar is met, the migrated-out in-tree
`rust/extensions/*` and `ui/` are removed in a dedicated cleanup, and **this file
is deleted**. (`federation` is already out of `rust/extensions/` — promoted to `rust/crates/federation/`,
so the cleanup does not touch it.) Until then: this repo intentionally contains both the old in-tree code and the new
library/SDK posture.

## First downstream proof (already green)

- `NubeIO/rubix-ai` boots an embedded lb node and serves an authenticating gateway (`node-v0.1.11`); its
  UI is a vendored copy of `ui/`.
- `NubeIO/rubix-ai-extensions/extensions/proof-panel` is the first extension migrated to the published
  SDKs — zero lb-repo dependency, proven on a real rubix-ai node.
