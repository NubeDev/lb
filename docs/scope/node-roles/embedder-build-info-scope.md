# Embedder build-info scope — the product on top has a version, and nothing can publish it

Status: **SHIPPED** (`feat/embedder-build-info`). Promotes to `public/node-roles/node-roles.md`.

Every version lb publishes today is **lb's own**. `GET /health` reports
`env!("CARGO_PKG_VERSION")` of `lb-role-gateway`, and `GET /node` copies that same constant
(`node_identity.rs` → `version: crate::routes::VERSION`). For the stock binary that is exactly
right — lb *is* the product. For an **embedder** it is a dead end: a host that boots lb as a
library through `BootConfig` has no field in which to state what *it* is, so the only version any
operator, installer, or fleet tool can read off the node is the core's.

The failure is quiet, which is what makes it worth a scope. An operator opens the About page or
curls `/node`, reads `"version":"0.1.0"`, and reasonably concludes that is the product build on the
box. It is not — it is the lb gateway crate, and the two version numbers move on entirely
independent release trains. Nothing errors; the answer is just about a different piece of software
than the one asked about.

This scope adds **one optional `BootConfig` field** carrying the embedding product's identity, and
publishes it **beside** lb's version — never instead of it — on the surfaces that already answer
"what is this node".

## Goals

- **A seam an embedder can fill.** `BootConfig.build_info: Option<BuildInfo>` — a `{ name,
  version }` pair describing the program that embedded lb. Optional; `None` reproduces today's
  behaviour byte for byte.
- **Two versions, two fields, never one shadowing the other.** `version` on `/node` and `/health`
  keeps meaning *lb's gateway build*, unchanged. The product's goes in a new `product` object. A
  consumer that reads `version` today keeps reading the same thing tomorrow.
- **Generic, per rule 10.** lb learns the *shape* of an embedder's build identity and nothing about
  any particular embedder. No `rubix-ai` string, no default, no inference — lb never derives,
  guesses, or falls back to a product name. Two opaque strings in, two strings out.
- **One value, every surface.** The same `BuildInfo` feeds `/node`, `/health`, and the mDNS
  advertisement, so the three cannot disagree — the same argument node-identity made for publishing
  one `NodeIdentity` two ways.
- **Additive on the wire.** A new optional key on two JSON bodies. No field changes type, no field
  is removed, no existing test's expectation moves.

## Non-goals

- **Not a build-metadata generator.** lb does not shell out to `git`, add a `build.rs`, or link
  `vergen`. *How* an embedder computes its version string is the embedder's business — lb takes
  the finished string. rubix-ai's half is scoped in
  `NubeIO/rubix-ai` → `docs/scope/platform/build-version-scope.md`.
- **Not a compatibility check.** Nothing gates, warns, or refuses on a product/core version pair.
  This is reporting only. A supported-version matrix is a fleet concern and stays out.
- **Not a schema for the version string.** `version` is free-form text. lb does not parse it, does
  not require semver, does not validate. An embedder shipping a date stamp or a bare SHA is fine.
- **Not authenticated.** These fields ride the existing unauthenticated `/node` and `/health`; this
  scope does not add a gated surface, and does not move either route behind auth.
- **No new env var.** `BootConfig::from_env()` leaves `build_info` at `None`. The stock lb binary
  is not an embedder and has no product to declare — inventing `LB_PRODUCT_NAME` would let an
  operator relabel a node as something it is not, on a surface with no wall in front of it.

## Intent / approach

**One optional struct on `BootConfig`, threaded to the two routes and the advertisement.**

```rust
/// What program embedded this node — the product identity of the host on top of the core.
///
/// lb never derives this: an embedder states it or it is absent. Both fields are opaque display
/// strings, published unauthenticated; neither is parsed, validated, or used for addressing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    /// The product's name, e.g. the embedding crate's package name.
    pub name: String,
    /// The product's build version, free-form. Semver build metadata (`0.1.1+g1a2b3c4`) is the
    /// expected shape but nothing here requires it.
    pub version: String,
}
```

`BootConfig { pub build_info: Option<BuildInfo>, .. }`, defaulting to `None`.

On the wire, additively:

```
GET /node → 200 {"node":"node:<uuid>","name":"plant room","machine_id":"<hash>",
                 "version":"0.1.0",                                  // lb gateway — UNCHANGED
                 "product":{"name":"rubix-ai","version":"0.1.1+g1a2b3c4"},   // NEW, omitted if None
                 "gateway":{"port":8099,"addresses":["192.168.10.77"]}}

GET /health → 200 {"status":"ok","version":"0.1.0","detail":{…},
                   "product":{"name":"rubix-ai","version":"0.1.1+g1a2b3c4"}}  // NEW, omitted if None
```

`#[serde(skip_serializing_if = "Option::is_none")]` — with no embedder the bodies are byte-identical
to today's, which is what keeps this a non-event for the stock binary and for `lb-fleet`.

**The rejected alternative: renaming.** The tidier-looking option is to make `version` mean the
*product* (what people actually mean by "what version is this box running") and move lb's to
`lb_version`. Rejected on two counts. It breaks the wire for every existing reader, including lb's
own `node_identity_route_test.rs` and `health_route_test.rs`, which assert
`body["version"] == env!("CARGO_PKG_VERSION")` — and a body-key-set assertion elsewhere exists
precisely to make unauthenticated-route changes loud. More importantly it makes the *core's* version
the special case, when the core is the thing that is always present; the optional value should be
the optional field. Additive keeps the invariant "`version` is lb" true forever, and that
invariant is cheap to remember.

**Where the value is read.** `BuildInfo` is cloned into `Gateway` state at build time, next to the
health cell — the routes stay `load`-only and a probe never reaches back into config. The
advertisement takes it in `config.rs` where `ad.version` is already set (today from lb's own
`CARGO_PKG_VERSION` at `rust/node/src/config.rs:843`).

## How it fits

- **Rule 10 — the load-bearing one.** lb gains no knowledge of any embedder. It cannot name one,
  has no default, and does not care whether the strings are meaningful. Swapping which product
  embeds lb changes no lb code — the test proving this uses a fabricated product name, not
  rubix-ai's.
- **Capabilities & the deny path.** None, deliberately. Both routes are unauthenticated by
  existing design (same posture as `POST /auth/login`), and this adds no gated verb. The wall
  argument is inherited wholesale from node-identity: **addressing is not authorization**, and
  every byte after the dial is gated exactly as before.
- **What may go on an unauthenticated route.** `product` is identity-of-software, the same class as
  the `version` already published — not workspace, persona, capability, member, or extension data.
  It is chosen by the operator's own build, not derived from tenant state. A deployment that
  considers its product name sensitive omits it: the field is `Option`, and absence is a supported
  posture, not a degraded one.
- **Symmetric nodes.** `build_info` is config on `BootConfig`, like every other role toggle. No
  code branches on it beyond "present or absent" at serialization.
- **State vs motion.** Neither — a boot-time constant, held in memory, never stored, never in the
  outbox. It changes only when the binary changes, which is why it can be `&'static`-adjacent and
  needs no invalidation.
- **One responsibility per file.** `BuildInfo` lands in its own file under the config module rather
  than swelling `config.rs` (already large); the routes each gain a field and a line.
- **SDK/WIT impact:** none. No extension-visible surface changes; this is host config and two HTTP
  bodies.

## Where each piece lives

| piece | file |
|---|---|
| `BuildInfo` type + the "lb never derives this" doctrine | `rust/crates/discovery/src/build_info.rs` (new) — **not** `rust/node/`, see below |
| `BootConfig.build_info` (default `None`; `from_env` leaves it `None`) | `rust/node/src/config.rs` |
| threading it into `Gateway` state at boot | `rust/node/src/builder.rs` |
| `product` on the advertisement (beside the existing `ad.version`) | `rust/node/src/config.rs` |
| `product` on `GET /node` | `rust/role/gateway/src/routes/node_identity.rs` |
| `product` on `GET /health` | `rust/role/gateway/src/routes/health.rs` |
| the field on `Gateway` | `rust/role/gateway/src/state.rs` |
| the shared `product` response object both routes render | `rust/role/gateway/src/routes/product.rs` (new) |
| `product_version` on `Advertisement` + `DiscoveredPeer`, and the `prod` TXT key | `rust/crates/discovery/src/{advertise,browse,peer}.rs` |

**Two placements moved during implementation, both forced by the dependency graph:**

- **`BuildInfo` lives in `lb-discovery`, not `lb-node`.** `lb-role-gateway` must name the type to
  hold it in `Gateway` state, and the gateway cannot depend on `lb-node` — that is the wrong way
  round. `lb-discovery` is the crate both already depend on and the home of the sibling identity
  type `NodeIdentity`, which makes it the natural place for a second opaque identity-of-something
  struct. It is re-exported as `lb_node::BuildInfo`, so an embedder still needs exactly one import
  and the seam reads the same from outside.
- **The `product` response body is its own file**, `routes/product.rs`, rather than declared twice.
  Two routes rendering one value must not be able to render it two ways; one shape, one conversion,
  both routes call it. It owns its two `String`s rather than borrowing the state cell — axum clones
  `Gateway` per request, so a borrow could not outlive the handler.

## Example flow

1. An embedder computes its own version string however it likes and fills the seam:
   `cfg.build_info = Some(BuildInfo { name: "nube-node".into(), version: "2.4.0+gdeadbee".into() })`.
2. `NodeBuilder::new(cfg).boot()` clones it into `Gateway` state alongside the health cell.
3. `curl -s http://box:8099/node` → `"version":"0.1.0"` (lb) **and**
   `"product":{"name":"nube-node","version":"2.4.0+gdeadbee"}`.
4. `curl -s http://box:8099/health` → the same `product` object, from the same source value.
5. A fleet tool reading `version` — written before this landed — reads lb's version, exactly as it
   did yesterday.
6. The stock `lb` binary boots with `build_info: None`; both bodies omit `product` entirely and are
   byte-identical to the current release.

## Testing plan

Real boot, real routes, no mocks (rule 9) — the existing `health_route_test.rs` /
`node_identity_route_test.rs` harnesses already boot a real gateway.

- **Absent ⇒ invisible.** With `build_info: None`, `/node` and `/health` bodies have the exact key
  set they have today. Asserted on the **key set**, not just on a missing lookup, so a future field
  addition to either unauthenticated route trips the test — the guard node-identity already
  established.
- **Present ⇒ published, on both routes, from one source.** One boot with `build_info` set; assert
  both bodies carry the same `product` object.
- **`version` is untouched.** Both routes still report `env!("CARGO_PKG_VERSION")` in `version`
  with `build_info` set — the regression test for the rejected renaming.
- **Rule 10.** The fixtures use a fabricated product (`"nube-node"`), never `rubix-ai`; a grep for
  `rubix` in lb stays empty.
- **Advertisement parity.** The mDNS TXT record carries the product version in `prod` when set and
  omits the key when not, with `version` reading lb's own in **both** cases — a live browse
  round-trip, matching the existing discovery tests. The `version`-unchanged half is the point:
  it is the assertion that makes "add, don't repurpose" true on the LAN as well as on HTTP.
- **Unauthenticated, still.** A garbage bearer changes neither body (inherited assertion, re-run
  with the field present).
- **Serialization.** `None` omits the key rather than emitting `"product":null` — a null would
  force every consumer into a two-case read for no gain.

Not applicable: capability-deny (no gated verb), workspace-isolation (no tenant data),
offline/sync, hot-reload.

## What shipped

All of the above, plus the tests below. Two notes for a reader diffing this against the ask:

- **`Gateway::with_build_info` is installed independently of `with_identity`.** The scope did not
  say so and the obvious wiring — inside the existing `if let Some(identity)` block in
  `builder.rs` — would have been wrong: the two are unrelated, and a node with no durable identity
  still serves `/health`, so tying them would have silently dropped `product` on exactly those
  nodes. Pinned by `health_carries_the_product_even_with_no_node_identity`.
- **The advertisement is stamped in `builder.rs`, not `config.rs`.** `config.rs`'s
  `advertisement_from_env` is the binary's env reader and `build_info` is deliberately not in env,
  so there is nothing for it to read. Boot clones the advertisement and fills `product_version`
  from the same `cfg.build_info` the gateway got — which is what makes the three surfaces one
  value rather than three assignments that could drift.

## Risks & hard problems

1. **Two version fields invite the wrong read.** The mitigation is naming and docs, not mechanism:
   `version` keeps its long-standing meaning, `product` is self-describing, and both route modules
   say plainly which is which. The status quo — one field silently meaning the wrong thing — is
   strictly worse.
2. **An embedder that lies.** Nothing stops a host from publishing any name it likes on an
   unauthenticated route. That is inherent to a field the embedder supplies, and the same is
   already true of `name` on `/node`. It is display text; nothing routes, addresses, or authorizes
   by it, and this scope must not let anything start.
3. **Absence is ambiguous at the far end.** A missing `product` means "not an embedder" *or* "an
   older lb". A consumer that needs to tell those apart uses lb's `version`, which is always
   present — worth stating in the route docs so nobody infers a product from a missing key.

## Open questions

1. **`/health` too, or `/node` only?** This scope proposes both, because a fleet prober usually
   holds `/health` and nothing else, and asking it to make a second call for a version it is
   already half-reading is a poor trade. The counter-argument is that `/health` should stay
   minimal. **Recommendation: both** — the payload cost is two short strings.
2. **Should `BuildInfo` carry more?** Build timestamp, toolchain, target triple all have obvious
   uses on a fleet. Recommendation: **`{name, version}` only** for v1, with the struct
   `#[non_exhaustive]` so fields can be added without a breaking change. An embedder that wants a
   timestamp can put it in the version string today.
3. ~~**mDNS: replace or add?**~~ **Answered: add a `prod` TXT key, leave `version` alone** —
   consistent with the HTTP decision, and agreed with the consumer half, which must be answered
   together with this one.

   One correction to the premise, found reviewing the pair: `ad.version` does **not** reliably
   carry lb's version today. lb sets it from its own `CARGO_PKG_VERSION` at `config.rs:843`, but an
   embedder can overwrite the field afterwards on the `Advertisement` it hands back — and rubix-ai
   does exactly that (`src/boot.rs:345`), so on a real rubix-ai node the LAN already advertises the
   *product* version under a key that means lb's on every other surface. Adding `prod` therefore
   only half-fixes the disagreement; the other half is the embedder dropping its override, which
   the rubix-ai scope now commits to. Worth a line in the advertisement's field docs — `version` is
   lb's *unless an embedder overwrote it*, and if that ambiguity is unacceptable the follow-up is
   for lb to set `version` at serialization time rather than leaving a writable field for the
   embedder to clobber. Not proposed here: it is a behaviour change to an existing field, and the
   documented convention plus one embedder fix covers the case in front of us.

## Related

- `docs/scope/node-roles/embed-node-scope.md` — the owning embed seam this extends by one field.
- `docs/scope/node-roles/fleet-presence-scope.md` — the advertisement this also touches.
- `NubeIO/rubix-ai` → `docs/scope/platform/node-identity-scope.md` — the predecessor that
  established the "generic in lb, product-specific in the embedder" split and the
  unauthenticated-surface wall this inherits.
- `NubeIO/rubix-ai` → `docs/scope/platform/build-version-scope.md` — the consumer half: how the
  product version string is generated and filled in.
