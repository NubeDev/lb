# The extension publisher trust gate (and how to turn it off for development)

**Status:** shipped. **Env knob:** `LB_EXT_UNTRUSTED_KEY`. **Default:** gate **ON** (enforced).

This documents the trust gate a node applies to extension artifacts, and the opt-in escape hatch
that disables *half* of it on a development box. If you are here because `make publish` keeps
returning `422 artifact failed verification`, jump to [Turning it off](#turning-it-off-development).

---

## 1. What the gate actually is

Every extension artifact — whether uploaded to `POST /extensions` or pulled from a remote registry —
passes through one function, `lb_registry::verify_artifact`. It runs **two independent checks**:

| # | Check | Question it answers | Waivable? |
|---|-------|--------------------|-----------|
| 1 | **Integrity** | *Are these the bytes that were packed?* Recomputes the SHA-256 digest over `(manifest_toml, wasm)` and compares it to the artifact's claimed `digest_hex`. | **Never.** |
| 2 | **Authenticity** | *Who packed them?* Ed25519-verifies the signature over that digest against the key the node allow-lists for `publisher_key_id` (from `LB_TRUSTED_PUBKEYS`). | Yes — this is the hatch. |

Both failures collapse to `RegistryError::Unverified` → HTTP **422 `artifact failed verification`**.
The error is deliberately opaque: a foreign artifact learns nothing about the allow-list.

Keeping these two separate is the whole design. The escape hatch waives **who signed it**, never
**whether the bytes are intact** — so a corrupt, truncated, or tampered upload is still rejected on a
box with the gate wide open.

### Why integrity still matters with the gate off

The digest binds the **manifest**, and the manifest declares the extension's requested capabilities.
So even waived, this is still caught:

```
# artifact signed, then the manifest edited to inflate its caps
manifest_toml = 'id="hello"\nrequest=["secret:*"]'   →  422 Unverified
```

Without the digest check, "turn off the trust gate" would become "let anyone on the box grant an
extension arbitrary capabilities". That is why it is not one boolean.

---

## 2. Turning it off (development)

Set the env var to the **exact** string `allow` on the node:

```bash
LB_EXT_UNTRUSTED_KEY=allow
```

Both Makefiles already do this for their dev targets — `make dev` / `make cloud` in this repo, and
`make dev` in `rubix-ai`. You do not need to set anything by hand for the normal local loop.

To restore the strict gate for a single run:

```bash
make dev EXT_UNTRUSTED_KEY=required
```

### It is exact-token, on purpose

Only the literal lowercase `allow` disables the check. **Everything else means ON**, including every
value someone might plausibly type expecting the opposite:

```
(unset)  ""  0  1  true  false  on  off  yes  no  ALLOW  Allow  allowed  disable  insecure
                                    ↓
                          gate stays ENFORCED
```

This deliberately breaks lb's usual convention. `LB_DEV_LOGIN` and `LB_BROWSER_SESSION_SECURE` are
*presence* flags — set to anything, they're on. Presence semantics here would be hazardous: an
operator writing `LB_EXT_UNTRUSTED_KEY=off` to **keep** the gate would silently disable it. Failing
closed on unrecognised input is worth the inconsistency.

An unparseable value is logged and ignored, mirroring the existing `LB_TRUSTED_PUBKEYS` idiom where a
malformed entry is skipped with a warning rather than aborting boot:

```
LB_EXT_UNTRUSTED_KEY: ignoring unrecognised value "true" — the publisher trust gate stays
ENABLED. The only value that disables it is "allow".
```

### What it does *not* waive

The hatch is narrow. All of these still apply with it on:

- **Content integrity** — the digest check above.
- **The capability gate** — `mcp:ext.publish:call` is still required. Waiving publisher trust is not
  an authorization bypass.
- **The workspace wall** — a ws-A token still cannot publish into ws-B (§6, checked first).
- **Manifest coherence** — an artifact whose `(ext_id, version)` contradicts its own signed manifest
  is still rejected.

Each of these has a regression test in `rust/crates/host/tests/ext_publish_test.rs`.

---

## 3. How you can tell a node has it on

The main risk of this feature is a bench setting silently surviving into production, so a waived node
announces itself on **three** surfaces:

**1. At boot**, on stderr:

```
WARNING: LB_EXT_UNTRUSTED_KEY=allow — the extension publisher trust gate is DISABLED. This node
accepts extensions signed by ANY key, including keys not in LB_TRUSTED_PUBKEYS, and will pull and
run their code. Content integrity is still verified (a corrupt artifact is still rejected), but
authorship is NOT. ...
```

**2. On every artifact it lets through** — a `warn!` naming the workspace, `ext_id`, `version`, and
the unchecked `publisher_key_id`. A boot warning rotates out of a journal within hours; this is what
an operator debugging *"how did this get installed?"* a week later actually finds.

**3. On `GET /health`** — an extra field, present **only** when waived:

```jsonc
// normal node — byte-identical to what it has always been
{"status":"ok","version":"…","detail":{"store":"ok","gateway":"ok"}}

// waived node
{"status":"ok","version":"…","detail":{"store":"ok","gateway":"ok"},
 "trust_gate":"waived-untrusted-key"}
```

Two deliberate choices here:

- **The status stays `200`/`ok`.** A waived gate is a configuration, not a degraded subsystem.
  Returning `503` would evict a perfectly working bench node from a load balancer's rotation for
  being in exactly the state it was configured to be in.
- **The field is absent, not `null`, when enforced.** Existing probes, matchers, and dashboards see
  the body they have always seen. Nothing has to change to accommodate it.

`/health` is unauthenticated, so this *does* tell anyone who can reach the port that the node accepts
foreign-signed extensions. That trade was made knowingly: the person who discovers an inherited
misconfigured box usually has no credentials for it yet, and hiding the posture behind auth they
don't have defeats the purpose. The value is a bare posture marker — it never names a key, key id,
publisher, path, or env var. **Do not enable this knob on a node facing an untrusted network.**

---

## 4. Scope: where the waiver reaches

| Path | Waived? | Notes |
|------|---------|-------|
| `POST /extensions` (local upload) | ✅ yes | The primary target — the bench-node toil. |
| Registry `pull` / `install_from_registry` | ✅ yes | See the warning below. |
| `lb-cli` local self-publish | ❌ no | Hard-wired `Required`; it builds an allow-list containing exactly the key that signed the bytes, so the check costs nothing and always passes. |
| `registry-host` (redistribution) | ❌ no | A registry host serves artifacts to *other* nodes; waiving there turns one bench box into a fleet-wide supply-chain hole. |

> **⚠️ The pull path is strictly more dangerous than the publish path.**
> An operator uploading to `POST /extensions` chose the bytes. A registry pull fetches them from a
> remote source over an untrusted wire. With the waiver on, a spoofed or compromised registry — or
> anyone who can MITM the fetch — can install code on the node, because the signature that would
> have caught it is the exact check being skipped. The digest still runs, but the digest is a *claim
> carried in the same untrusted response*, so it defends only against corruption in transit, not
> against a hostile source. Development networks you control, only.

---

## 5. How the type-level guarantee survives

`VerifiedArtifact` is the load-bearing seam: `cache_artifact` accepts **only** that type, and the
only way to construct one is `verify_artifact`. That makes "an artifact reaches the cache only by
passing through the verifier" a *compile-time* guarantee rather than a convention the next edit might
forget.

The hatch preserves this rather than punching through it:

- `verify_artifact_with(..)` is still the sole mint. There is no second constructor and no widened
  `cache_artifact` signature.
- The waiver is **carried in the value**, not erased at the boundary:
  `VerifiedArtifact::authenticity() -> Authenticity`. Downstream code can tell whether the publisher
  was ever checked; that is how the publish/pull paths know to emit their warnings.
- The type's doc comment now states the honest, weaker invariant: *integrity always; authenticity
  when `authenticity() == Required`*. Collapsing both postures into one indistinguishable type would
  have made that doc comment a lie for every module citing it.

`Authenticity` is a **parameter, never read from the environment** inside `lb-registry`. The crate
holds no policy: it cannot see an env var and cannot be reconfigured at a distance. A caller wanting
the waiver must name it at the call site — so the hatch is auditable with
`grep -r WaivedUntrustedKey`.

The two-argument `verify_artifact(artifact, trusted)` still exists and hard-codes `Required`. That
makes fail-closed **structural**: adding a new call site cannot accidentally weaken the gate by
omission; it gets the full gate unless someone deliberately rewrites it to call
`verify_artifact_with`.

---

## 6. Files

| Concern | File |
|---|---|
| The two checks + `Authenticity` | `rust/crates/registry/src/verify.rs` |
| The proof-carrying newtype | `rust/crates/registry/src/model/artifact.rs` |
| Env parsing + fail-closed mapping | `rust/role/gateway/src/session/trusted.rs` |
| Gateway state + `with_authenticity` | `rust/role/gateway/src/state.rs` |
| The `/health` field | `rust/role/gateway/src/routes/health.rs` |
| Embedder seam (`BootConfig`) | `rust/node/src/config.rs`, `rust/node/src/builder.rs` |
| Upload path + per-artifact warning | `rust/crates/host/src/ext/publish.rs` |
| Pull path + per-artifact warning | `rust/crates/host/src/registry/pull.rs` |

**Tests:** `rust/crates/registry/src/verify.rs` (unit — including *waived still rejects a digest
mismatch*), `rust/role/gateway/src/session/trusted.rs` (17 garbage values all leave the gate on),
`rust/role/gateway/tests/health_route_test.rs` (the conditional field), and
`rust/crates/host/tests/ext_publish_test.rs` (end-to-end over a real node, store, and wasm
component).

---

## 7. Open questions

- **Durable trust storage + key rotation** remain the deferred registry-scope questions.
  `LB_TRUSTED_PUBKEYS` being env-only is what makes the toil this hatch works around; a durable,
  rotatable per-workspace allow-list would reduce the need for the hatch rather than replace it.
- **Should a waived node refuse to serve a non-loopback bind?** That would make the "bench setting in
  production" failure structurally impossible instead of merely loud. Not built — it would break the
  legitimate case of a bench node on a trusted LAN.
