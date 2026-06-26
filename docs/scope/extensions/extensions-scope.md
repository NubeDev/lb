# Extensions scope

Status: scope. **S0 decision doc** — fixes the README §13 "extension manifest format" forever
decision (specified first, because everything downstream keys off it). The runtime/hot-reload
implementation lands at S1 (one ext) → S2 (federated UI + hot-reload) → S7 (registry/native).

> Read with: `../../README.md` §6.3 (runtime: two tiers), §6.4 (registry/distribution),
> §13 (manifest is the contract), `../crate-layout/crate-layout-scope.md` (the WIT boundary),
> `../auth-caps/auth-caps-scope.md` (the capability grammar the manifest *requests*).
> The runtime/supervisor/loader can be **re-authored** (not copied) from rubix-cube — see
> `../../STAGES.md` "Reuse: the extension server".

---

## Goal

Define the **extension manifest**: the contract declaring an extension's identity, tier,
placement, required capabilities, and visibility. The loader parses it; the host grants only
what it requests *and* the workspace admin approved at install (§6.4 trust model). Everything
downstream — loading, the caps grant set, registry distribution — keys off this shape.

## Non-goals (S0)

- Native Tier-2 sidecar manifest fields beyond a `tier` marker (§6.3) — S7.
- Registry signing/visibility enforcement (§6.4) — the fields exist now; enforcement is S7.
- UI/module-federation manifest fields (§6.13) — added at S2 when the UI loader lands.

## DECISION (forever): the manifest format

**TOML**, named `extension.toml`, shipped alongside the component. (TOML over JSON5/YAML:
matches Cargo, comments allowed, unambiguous, already in the Rust toolchain.) The S0/S1 shape:

```toml
# extension.toml — the contract the host reads before instantiating anything.
[extension]
id          = "hello"                # unique within a workspace; matches the mcp:<ext> prefix
version     = "0.1.0"                # semver; rollback = previous version (§6.4)
name        = "Hello"                # human label
description = "Trivial echo tool — the S1 spine probe."

[runtime]
tier        = "wasm"                 # "wasm" (Tier 1 default) | "native" (Tier 2 escape hatch)
world       = "lazybones:ext/extension@0.1.0"  # the WIT world it targets (must match host major)
placement   = "either"               # "local-only" | "cloud-only" | "either"  (§6.3)

# Capabilities the extension REQUESTS. The host grants the intersection of this and what the
# workspace admin approved at install. Strings use the auth-caps grammar verbatim.
[capabilities]
request = [
  "store:note:read",
  "store:note:write",
]

# Tools this extension exposes as MCP tools. Each becomes "<id>.<name>" and is gated by
# mcp:<id>.<name>:call (auth-caps). Declared here so the host can register them without
# instantiating the component first.
[[tools]]
name        = "echo"
description = "Echo the input message back."
# input/output JSON shapes are validated at the WIT call-tool boundary (mcp scope).

[visibility]
class = "private"                    # "public" (global catalog) | "private" (one workspace) — §6.4
```

### Why these fields, and the rules that bite

- **`capabilities.request` is a *request*, never a grant.** The whole point of §6.4's trust
  model: "public" ≠ "more privileged". The host computes `granted = requested ∩
  admin_approved` and the running instance's token carries exactly `granted` — nothing the
  manifest asks for is live until an admin approved it. A deny test covers
  "requests `secret:github/token:get` but admin didn't approve → that cap is absent".
- **`runtime.world` is checked against the host's WIT major.** Mismatch → the loader refuses
  to instantiate (crate-layout scope, the §11.2 forever boundary). This is how the stable ABI
  is enforced at load time.
- **`tools` are declared, not discovered.** The host registers `<id>.<name>` from the manifest
  so MCP `resolve` (mcp scope) works without instantiating the component — and so a denied
  call can be authorized/refused without ever starting the extension.
- **`id` is workspace-unique and is the cap/namespace prefix.** It ties manifest → caps
  (`mcp:<id>.*`, `secret:<id>/*`) → store/bus namespacing together under one name.
- **`placement` and `visibility` carry no behavior in S1** (solo node, no registry) but are
  in the contract now so S3/S7 don't re-cut the manifest — exactly what §13 warns against.

**Rejected:** declaring capabilities as a free-form list the host trusts (would make "public"
mean "privileged" — the §11.5 blast-radius risk). **Rejected:** JSON manifest (no comments,
and the manifest is human-authored/reviewed at install — TOML reads better for that).
**Rejected:** inferring tools by instantiating and asking the component (forces a live
instance before authorization can happen; breaks "authorize before dispatch", mcp scope).

## How it fits the core

- **Capability-first / blast radius:** the manifest is *where capabilities are requested* and
  the host is *where they are granted-down to the approved set* — the two are deliberately
  separate steps.
- **Stateless extensions (§3.4):** nothing in the manifest declares durable local state; state
  lives in `store`/on the bus, which is what makes the hot-reload swap (S2) safe.
- **Symmetric nodes:** `placement` is config-like metadata read by the loader; there is no
  `if cloud` — an `either` component is movable, a `local-only`/`cloud-only` one just isn't
  scheduled off-role.

## Testing plan

- **S1:** `ext-loader/tests/manifest_parse_test.rs` — parse `hello/extension.toml`, reject a
  bad `world` major, reject an unknown `tier`. `ext-loader/tests/grant_intersection_test.rs`
  — **mandatory deny:** a requested-but-not-approved cap is absent from the instance token.
- **S2:** hot-reload test — swap `hello@0.1.0` → `0.1.1` with no dropped durable state.
- **S7:** signature verify + visibility (public installable cross-ws; private not).

## Open questions

- Where the admin-approval set is stored (a `caps:install_grant:{ws}/{ext}` record?) — S1 can
  hardcode the approved set for `hello`; formalize the install flow at S4/S7.
- Native (`tier="native"`) manifest fields (exec, supervision, socket) — S7.
- UI/federation fields (`[ui] remote = …`) — add at S2 with the federated loader.
- Multi-tool input/output schema declaration in-manifest vs WIT-only — keep WIT-only for now
  (mcp scope open question on JSON-Schema snapshots).
