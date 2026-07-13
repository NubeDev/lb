# SDK fix: ship `Caller.admin` (`sdk-v0.4.1`) + push the `*-v0.4.1`/`v0.4.2` tags

**Date:** 2026-07-13
**Status:** OPEN — the design shipped in-tree, the RELEASE did not. Blocks every
downstream embedder that pins the native SDK (surfaced by `cc-app`, which cannot
`cargo build` from published tags).
**Owning repos:** `NubeDev/lb-ext-sdk` (the SDK `Caller` field) → `NubeDev/lb`
(the host stamp + tags) → downstream drops its local `[patch]`.
**Design of record (do NOT re-scope):**
[`../../scope/extensions/native-caller-identity-scope.md`](../../scope/extensions/native-caller-identity-scope.md)
§"What shipped" already documents the `admin: bool` follow-up as
`sdk-v0.4.1` / `node-v0.4.1`. This doc is the RELEASE fix, not a new scope.

## Symptom (downstream)

A consumer pinning `lb-ext-native = { git = ".../lb-ext-sdk", tag = "sdk-v0.4.1" }`
(or an `lb` pin at `node-v0.4.1`/`node-v0.4.2`) fails at resolution:

```
failed to find tag `node-v0.4.2`
reference 'refs/tags/node-v0.4.2' not found
```

`git ls-remote --tags` confirms the remotes hold ONLY `node-v0.4.0`
(`NubeDev/lb`) and `sdk-v0.4.0` (`NubeDev/lb-ext-sdk`). The `*-v0.4.1` / `v0.4.2`
tags were cut locally + proven via a `[patch]`, then never pushed.

## Root cause — code is in-tree, the SDK struct + tags are not

The split is subtle: the HOST half of `node-v0.4.1` IS committed in this `lb`
checkout, but the SDK struct it feeds is stale on the published tag.

- **lb host — DONE in-tree.** `caps_hold_admin` is the authoritative admin signal
  (`rust/crates/host/src/authz/builtin_roles.rs::caps_hold_admin`, the admin-only
  cap delta), and the frame stamp reads it:
  - `rust/crates/host/src/tool_call.rs:647` → `admin: caps_hold_admin(caller.caps())`
  - `rust/crates/host/src/native/caller.rs:26` → same, on the projection path.
- **lb-ext-sdk — STALE on `sdk-v0.4.0`.** The wire `Caller` still has only
  `{ sub, ws, role, delegated }` — NO `admin` field (verified by cloning
  `sdk-v0.4.0`: `crates/lb-ext-native/src/wire.rs`). The host stamps `admin`, but
  a consumer deserializing with the `sdk-v0.4.0` struct drops it (`#[serde(default)]`
  would silently zero it — a fail-OPEN nobody wants for an admin marker).

So the host emits `admin`; no published SDK carries the field to read it. Any
downstream reading `caller.admin` (e.g. `cc-app`'s chokepoint) fails to compile
against published tags, and reading `caller.role` instead is a rule-7 regression
(`role` is cosmetic `member` for admins and guardians alike — the exact bug the
marker fixed).

## The fix (in order — each step gates the next)

### 1. `NubeDev/lb-ext-sdk` — add the field, tag `sdk-v0.4.1`

Add to `crates/lb-ext-native/src/wire.rs` `struct Caller`, after `delegated`:

```rust
    /// True when the host derived — from the caller's CAPS, not the cosmetic
    /// `role` — that this caller holds workspace-admin authority. The
    /// authoritative admin signal for a sidecar's row-filter bypass: `role` is
    /// minted `member` for everyone (admin power rides caps), so a sidecar that
    /// keyed admin off `role` would treat a real admin as a guardian. Host sets
    /// it once via `caps_hold_admin`; an old host that never stamps it leaves
    /// this `false` (fail-CLOSED — least privilege).
    #[serde(default)]
    pub admin: bool,
```

`#[serde(default)]` keeps it wire-compatible: an old host omits the field → it
deserializes `false` (fail-closed, correct). Update the crate version, then:

```
git tag sdk-v0.4.1 && git push origin <branch> --tags
```

### 2. `NubeDev/lb` — push the already-committed host work, tag it

The host code is in-tree (above). Confirm `Cargo.toml` bumps the `lb-ext-native`
dependency to `sdk-v0.4.1`, then cut the tags the downstream pins expect:

- `node-v0.4.1` — the admin-marker host stamp (this fix).
- `node-v0.4.2` — the additive `BootConfig::credential_mode` + `seed_credential`
  (the credential-mode work; scope
  [`../../scope/auth-caps/embedder-credential-mode-scope.md`](../../scope/auth-caps/embedder-credential-mode-scope.md)).

```
git push origin <branch> && git tag node-v0.4.1 node-v0.4.2 && git push origin --tags
```

### 3. Downstream — drop the `[patch]`, the pins already point at v0.4.2

Consumers already declare `node-v0.4.2` / `sdk-v0.4.1` (they built via a local
`[patch]`). Once steps 1–2 land, delete the `[patch]` blocks from the git-ignored
`.cargo/config.toml`; a clean `cargo build --workspace` with NO `[patch]` is the
"am I on releases?" check (WORKFLOW-LB §4). For `cc-app` specifically this
unblocks milestone 08 — see `cc-app/docs/debugging/build/unbuildable-from-releases-unpushed-v0.4.1-tags.md`.

## Guard (so a released marker can't silently regress)

The marker is a safety signal — a fail-open would re-open the cross-family leak.
Assert it end to end, not just in a struct round-trip:

- **SDK unit** — `Caller` with `admin: true` round-trips through JSON; an old
  payload WITHOUT the field deserializes `admin: false` (fail-closed).
- **lb host** — `caps_hold_admin` is `true` for the workspace-admin bundle and
  `false` for the member bundle (already covered:
  `builtin_roles.rs::caps_hold_admin_tracks_the_admin_bundle_only`); a routed
  native `call` from an admin principal stamps `caller.admin == true`, from a
  member `== false`.
- **Downstream contract** — the embedder's rule-7 test (e.g. `cc-app`
  `tests/live_node.rs`) proves admin reads land AND a stranger guardian is denied,
  with the admin token carrying an admin-only cap so `admin` derives true —
  mirroring production. That test is what SURFACED this gap; it must stay green
  against the pushed tags, not a `[patch]`.

## Why not work around it downstream

Deriving admin from `caller.role` (present on `sdk-v0.4.0`) looks tempting but is
wrong on two counts: `role` is cosmetically `member` for everyone (so admins are
mis-classified as guardians → `center.list → []`, `child.get → 403`), and it is
fail-OPEN the day lb ever does populate `role` for an admin. The marker exists
precisely because `role` is not a usable admin signal. Ship the field; don't read
around it.
